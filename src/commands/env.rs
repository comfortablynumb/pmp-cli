use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::executor::{Executor, ExecutorConfig, OpenTofuExecutor};
use crate::hooks::{HookOutcome, HooksRunner};
use crate::output;
use crate::template::time_limit::{format_expiration_status, is_expired};
use crate::template::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub struct EnvCommand;

#[derive(Debug)]
struct EnvDiff {
    only_in_source: Vec<String>,
    only_in_target: Vec<String>,
    different_values: Vec<(String, String, String)>, // (key, source_value, target_value)
}

/// Information about an expired environment
#[derive(Debug)]
struct ExpiredEnvironment {
    project_name: String,
    environment_name: String,
    environment_path: PathBuf,
    expiration_status: String,
    kind: String,
}

impl EnvCommand {
    /// Execute the env diff command
    pub fn execute_diff(ctx: &Context, source_env: &str, target_env: &str) -> Result<()> {
        ctx.output.section("Environment Comparison");

        // Find infrastructure
        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output
            .key_value("Infrastructure", &infrastructure.metadata.name);
        ctx.output.key_value("Source Environment", source_env);
        ctx.output.key_value("Target Environment", target_env);
        output::blank();

        // Discover all projects
        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &infrastructure_root)?;

        if projects.is_empty() {
            ctx.output.dimmed("No projects found.");
            return Ok(());
        }

        let mut has_differences = false;
        let mut projects_compared = 0;

        for project in &projects {
            let project_path = infrastructure_root.join(&project.path);
            let source_env_path = project_path.join("environments").join(source_env);
            let target_env_path = project_path.join("environments").join(target_env);

            // Check if both environments exist
            if !ctx.fs.exists(&source_env_path) || !ctx.fs.exists(&target_env_path) {
                continue;
            }

            projects_compared += 1;

            // Compare environment configurations
            let source_yaml = source_env_path.join(".pmp.environment.yaml");
            let target_yaml = target_env_path.join(".pmp.environment.yaml");

            if !ctx.fs.exists(&source_yaml) || !ctx.fs.exists(&target_yaml) {
                continue;
            }

            let source_resource =
                DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &source_yaml)?;
            let target_resource =
                DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &target_yaml)?;

            // Compare inputs
            let diff =
                Self::compare_inputs(&source_resource.spec.inputs, &target_resource.spec.inputs);

            if !diff.only_in_source.is_empty()
                || !diff.only_in_target.is_empty()
                || !diff.different_values.is_empty()
            {
                has_differences = true;

                ctx.output.subsection(&format!("Project: {}", project.name));

                if !diff.only_in_source.is_empty() {
                    ctx.output.dimmed(&format!(
                        "  Only in {}: {}",
                        source_env,
                        diff.only_in_source.join(", ")
                    ));
                }

                if !diff.only_in_target.is_empty() {
                    ctx.output.dimmed(&format!(
                        "  Only in {}: {}",
                        target_env,
                        diff.only_in_target.join(", ")
                    ));
                }

                for (key, source_val, target_val) in &diff.different_values {
                    ctx.output
                        .dimmed(&format!("  {}: {} → {}", key, source_val, target_val));
                }

                output::blank();
            }
        }

        if !has_differences {
            ctx.output
                .success("No differences found between environments");
        } else {
            ctx.output.info(&format!(
                "Compared {} project(s) - differences found",
                projects_compared
            ));
        }

        Ok(())
    }

    /// Execute the env promote command
    pub fn execute_promote(
        ctx: &Context,
        source_env: &str,
        target_env: &str,
        project_filter: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Environment Promotion");

        // Find infrastructure
        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output
            .key_value("Infrastructure", &infrastructure.metadata.name);
        ctx.output.key_value("Source Environment", source_env);
        ctx.output.key_value("Target Environment", target_env);
        output::blank();

        // Discover all projects
        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &infrastructure_root)?;

        if projects.is_empty() {
            ctx.output.dimmed("No projects found.");
            return Ok(());
        }

        // Filter projects if specified
        let projects_to_promote: Vec<_> = if let Some(filter) = project_filter {
            projects
                .iter()
                .filter(|p| p.name.to_lowercase().contains(&filter.to_lowercase()))
                .collect()
        } else {
            projects.iter().collect()
        };

        if projects_to_promote.is_empty() {
            ctx.output.dimmed("No matching projects found.");
            return Ok(());
        }

        ctx.output.info(&format!(
            "Found {} project(s) to promote",
            projects_to_promote.len()
        ));
        output::blank();

        // Confirm promotion
        let confirmed = ctx.input.confirm(
            &format!(
                "Promote {} → {}? This will overwrite target configurations.",
                source_env, target_env
            ),
            Some(false),
        )?;

        if !confirmed {
            ctx.output.dimmed("Promotion cancelled.");
            return Ok(());
        }

        let mut promoted_count = 0;

        for project in &projects_to_promote {
            let project_path = infrastructure_root.join(&project.path);
            let source_env_path = project_path.join("environments").join(source_env);
            let target_env_path = project_path.join("environments").join(target_env);

            // Check if both environments exist
            if !ctx.fs.exists(&source_env_path) {
                ctx.output.dimmed(&format!(
                    "Skipping {} - source environment not found",
                    project.name
                ));
                continue;
            }

            if !ctx.fs.exists(&target_env_path) {
                ctx.output.dimmed(&format!(
                    "Skipping {} - target environment not found",
                    project.name
                ));
                continue;
            }

            // Promote configuration
            let source_yaml = source_env_path.join(".pmp.environment.yaml");
            let target_yaml = target_env_path.join(".pmp.environment.yaml");

            if ctx.fs.exists(&source_yaml) && ctx.fs.exists(&target_yaml) {
                let mut source_resource =
                    DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &source_yaml)?;
                let target_resource =
                    DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &target_yaml)?;

                // Update inputs while preserving environment name
                source_resource.metadata.environment_name = target_env.to_string();

                // Preserve dependencies (they may be environment-specific)
                // Only promote inputs
                let promoted_inputs = source_resource.spec.inputs.clone();

                // Create backup
                let backup_path = target_yaml.with_extension("yaml.backup");
                let backup_content = ctx.fs.read_to_string(&target_yaml)?;
                ctx.fs.write(&backup_path, &backup_content)?;

                // Write promoted configuration
                let mut updated_resource = target_resource.clone();
                updated_resource.spec.inputs = promoted_inputs;

                let yaml_content = serde_yaml::to_string(&updated_resource)?;
                ctx.fs.write(&target_yaml, &yaml_content)?;

                ctx.output.success(&format!(
                    "Promoted {} (backup: {})",
                    project.name,
                    backup_path.display()
                ));
                promoted_count += 1;
            }
        }

        output::blank();
        ctx.output.success(&format!(
            "Promoted {} project(s) from {} to {}",
            promoted_count, source_env, target_env
        ));

        Ok(())
    }

    /// Execute the env sync command
    pub fn execute_sync(ctx: &Context, project_filter: Option<&str>) -> Result<()> {
        ctx.output.section("Environment Synchronization");

        // Find infrastructure
        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output
            .key_value("Infrastructure", &infrastructure.metadata.name);
        output::blank();

        // Discover all projects
        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &infrastructure_root)?;

        if projects.is_empty() {
            ctx.output.dimmed("No projects found.");
            return Ok(());
        }

        // Filter projects if specified
        let projects_to_sync: Vec<_> = if let Some(filter) = project_filter {
            projects
                .iter()
                .filter(|p| p.name.to_lowercase().contains(&filter.to_lowercase()))
                .collect()
        } else {
            projects.iter().collect()
        };

        if projects_to_sync.is_empty() {
            ctx.output.dimmed("No matching projects found.");
            return Ok(());
        }

        let mut synced_count = 0;

        for project in &projects_to_sync {
            let project_path = infrastructure_root.join(&project.path);
            let envs_dir = project_path.join("environments");

            if !ctx.fs.exists(&envs_dir) {
                continue;
            }

            // Discover all environments for this project
            let env_entries = ctx.fs.read_dir(&envs_dir)?;
            let mut environments = Vec::new();

            for env_path in env_entries {
                let env_file = env_path.join(".pmp.environment.yaml");
                if ctx.fs.exists(&env_file)
                    && let Ok(env_name) = env_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .context("Failed to get environment name")
                {
                    environments.push((env_name.to_string(), env_file));
                }
            }

            if environments.len() < 2 {
                continue; // Need at least 2 environments to sync
            }

            // Find common inputs across all environments
            let mut common_inputs: Option<HashMap<String, serde_json::Value>> = None;

            for (_env_name, env_file) in &environments {
                let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, env_file)?;

                if common_inputs.is_none() {
                    common_inputs = Some(resource.spec.inputs.clone());
                } else {
                    let current_common = common_inputs.as_ref().unwrap();
                    let mut new_common = HashMap::new();

                    for (key, value) in current_common {
                        if let Some(other_value) = resource.spec.inputs.get(key)
                            && value == other_value
                        {
                            new_common.insert(key.clone(), value.clone());
                        }
                    }

                    common_inputs = Some(new_common);
                }
            }

            if let Some(common) = common_inputs
                && !common.is_empty()
            {
                ctx.output.subsection(&format!("Project: {}", project.name));
                ctx.output.dimmed(&format!(
                    "  Common inputs across {} environments: {}",
                    environments.len(),
                    common.keys().cloned().collect::<Vec<_>>().join(", ")
                ));
                synced_count += 1;
            }
        }

        output::blank();
        ctx.output.success(&format!(
            "Analyzed {} project(s) for common settings",
            synced_count
        ));

        Ok(())
    }

    /// Execute the env variables command
    pub fn execute_variables(
        ctx: &Context,
        environment: Option<&str>,
        project_filter: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Environment Variables");

        // Find infrastructure
        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output
            .key_value("Infrastructure", &infrastructure.metadata.name);
        if let Some(env) = environment {
            ctx.output.key_value("Environment", env);
        }
        output::blank();

        // Discover all projects
        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &infrastructure_root)?;

        if projects.is_empty() {
            ctx.output.dimmed("No projects found.");
            return Ok(());
        }

        // Filter projects if specified
        let projects_to_show: Vec<_> = if let Some(filter) = project_filter {
            projects
                .iter()
                .filter(|p| p.name.to_lowercase().contains(&filter.to_lowercase()))
                .collect()
        } else {
            projects.iter().collect()
        };

        if projects_to_show.is_empty() {
            ctx.output.dimmed("No matching projects found.");
            return Ok(());
        }

        // Collect all variables
        let mut all_variables: HashMap<String, Vec<(String, String, serde_json::Value)>> =
            HashMap::new(); // env -> [(project, key, value)]

        for project in &projects_to_show {
            let project_path = infrastructure_root.join(&project.path);
            let envs_dir = project_path.join("environments");

            if !ctx.fs.exists(&envs_dir) {
                continue;
            }

            let env_entries = ctx.fs.read_dir(&envs_dir)?;

            for env_path in env_entries {
                let env_file = env_path.join(".pmp.environment.yaml");
                if !ctx.fs.exists(&env_file) {
                    continue;
                }

                if let Ok(env_name) = env_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .context("Failed to get environment name")
                {
                    // Filter by environment if specified
                    if let Some(env_filter) = environment
                        && env_name != env_filter
                    {
                        continue;
                    }

                    let resource =
                        DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;

                    for (key, value) in &resource.spec.inputs {
                        all_variables
                            .entry(env_name.to_string())
                            .or_default()
                            .push((project.name.clone(), key.clone(), value.clone()));
                    }
                }
            }
        }

        // Display variables grouped by environment
        for (env_name, variables) in all_variables {
            ctx.output.subsection(&format!("Environment: {}", env_name));

            // Group by variable name for better overview
            let mut by_variable: HashMap<String, Vec<(String, serde_json::Value)>> = HashMap::new();

            for (project, key, value) in variables {
                by_variable.entry(key).or_default().push((project, value));
            }

            for (var_name, projects) in by_variable {
                ctx.output.dimmed(&format!("  {}", var_name));
                for (project, value) in projects {
                    let value_str = match value {
                        serde_json::Value::String(s) => s,
                        _ => value.to_string(),
                    };
                    ctx.output
                        .dimmed(&format!("    {} = {}", project, value_str));
                }
            }

            output::blank();
        }

        Ok(())
    }

    /// Execute the env purge command - destroy all expired environments
    pub fn execute_purge(
        ctx: &Context,
        force: bool,
        environment_filter: Option<&str>,
        skip_confirmation: bool,
    ) -> Result<()> {
        ctx.output.section("Environment Expiration Check");

        // Find infrastructure
        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output
            .key_value("Infrastructure", &infrastructure.metadata.name);
        output::blank();

        // Discover all projects
        ctx.output.dimmed("Scanning for expired environments...");
        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &infrastructure_root)?;

        if projects.is_empty() {
            ctx.output.dimmed("No projects found.");
            return Ok(());
        }

        // Find all expired environments
        let expired_environments =
            Self::discover_expired_environments(ctx, &infrastructure_root, &projects, environment_filter)?;

        if expired_environments.is_empty() {
            output::blank();
            ctx.output.success("No expired environments found.");
            return Ok(());
        }

        // Display expired environments
        output::blank();
        ctx.output.info(&format!(
            "Found {} expired environment(s):",
            expired_environments.len()
        ));
        output::blank();

        // Display table header
        ctx.output.dimmed(&format!(
            "  {:<20} {:<15} {:<25} {}",
            "Project", "Environment", "Kind", "Expired"
        ));
        ctx.output.dimmed(&format!(
            "  {:<20} {:<15} {:<25} {}",
            "-------", "-----------", "----", "-------"
        ));

        for env in &expired_environments {
            ctx.output.dimmed(&format!(
                "  {:<20} {:<15} {:<25} {}",
                env.project_name, env.environment_name, env.kind, env.expiration_status
            ));
        }

        output::blank();

        if !force {
            ctx.output
                .info("Run with --force to execute destruction");
            return Ok(());
        }

        // Force mode - confirm and execute destruction
        if !skip_confirmation {
            ctx.output.blank();
            ctx.output.warning(
                "WARNING: This will destroy all expired environments and their resources!",
            );
            ctx.output.blank();

            let confirmation = ctx
                .input
                .text("Type 'yes' to confirm destruction:", None)
                .context("Failed to get confirmation")?;

            if confirmation.trim().to_lowercase() != "yes" {
                ctx.output.dimmed("Destruction cancelled.");
                return Ok(());
            }
        }

        // Execute destruction on each expired environment
        output::blank();
        ctx.output.subsection("Destroying Expired Environments");

        let mut destroyed_count = 0;
        let mut failed_count = 0;

        for (idx, env) in expired_environments.iter().enumerate() {
            ctx.output.info(&format!(
                "Step {}/{}: {} ({})",
                idx + 1,
                expired_environments.len(),
                env.project_name,
                env.environment_name
            ));

            match Self::destroy_environment(ctx, env) {
                Ok(()) => {
                    ctx.output.success(&format!(
                        "  Destroyed {} ({})",
                        env.project_name, env.environment_name
                    ));
                    destroyed_count += 1;
                }
                Err(e) => {
                    ctx.output.error(&format!(
                        "  Failed to destroy {} ({}): {}",
                        env.project_name, env.environment_name, e
                    ));
                    failed_count += 1;
                }
            }
        }

        // Display summary
        output::blank();
        ctx.output.subsection("Purge Summary");
        ctx.output.key_value("Destroyed", &destroyed_count.to_string());
        ctx.output.key_value("Failed", &failed_count.to_string());

        if failed_count == 0 {
            output::blank();
            ctx.output.success("Purge completed successfully");
        } else {
            output::blank();
            ctx.output
                .warning(&format!("Purge completed with {} failure(s)", failed_count));
        }

        Ok(())
    }

    /// Discover all expired environments
    fn discover_expired_environments(
        ctx: &Context,
        infrastructure_root: &std::path::Path,
        projects: &[crate::template::metadata::ProjectReference],
        environment_filter: Option<&str>,
    ) -> Result<Vec<ExpiredEnvironment>> {
        let mut expired = Vec::new();

        for project in projects {
            let project_path = infrastructure_root.join(&project.path);
            let envs_dir = project_path.join("environments");

            if !ctx.fs.exists(&envs_dir) {
                continue;
            }

            let env_entries = ctx.fs.read_dir(&envs_dir)?;

            for env_path in env_entries {
                let env_file = env_path.join(".pmp.environment.yaml");

                if !ctx.fs.exists(&env_file) {
                    continue;
                }

                let env_name = match env_path.file_name().and_then(|n| n.to_str()) {
                    Some(name) => name.to_string(),
                    None => continue,
                };

                // Filter by environment if specified
                if let Some(filter) = environment_filter {
                    if env_name.to_lowercase() != filter.to_lowercase() {
                        continue;
                    }
                }

                // Load environment resource
                let resource = match DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
                {
                    Ok(r) => r,
                    Err(_) => continue,
                };

                // Check if time_limit is configured
                if let Some(time_limit) = &resource.spec.time_limit {
                    // Check if expired
                    let created_at = resource.metadata.created_at.as_ref();

                    match is_expired(time_limit, created_at) {
                        Ok(true) => {
                            let expiration_status =
                                format_expiration_status(time_limit, created_at)
                                    .unwrap_or_else(|_| "expired".to_string());

                            expired.push(ExpiredEnvironment {
                                project_name: project.name.clone(),
                                environment_name: env_name,
                                environment_path: env_path.clone(),
                                expiration_status,
                                kind: resource.kind.clone(),
                            });
                        }
                        Ok(false) => {}
                        Err(_) => {}
                    }
                }
            }
        }

        Ok(expired)
    }

    /// Destroy a single environment
    fn destroy_environment(ctx: &Context, env: &ExpiredEnvironment) -> Result<()> {
        let env_file = env.environment_path.join(".pmp.environment.yaml");
        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
            .context("Failed to load environment resource")?;

        let executor_config = resource.get_executor_config();

        // Skip if executor is "none" (dependency-only projects)
        if executor_config.name == "none" {
            ctx.output.dimmed(&format!(
                "  Skipping {} ({}) - dependency-only project",
                env.project_name, env.environment_name
            ));
            return Ok(());
        }

        // Get executor
        let executor = Self::get_executor(&executor_config.name)?;

        let env_dir_str = env
            .environment_path
            .to_str()
            .context("Failed to convert environment path to string")?;

        // Load collection to get infrastructure-level hooks
        let (collection, _) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required to run commands")?;

        let infrastructure_hooks = collection.get_hooks();

        // Merge hooks
        let hooks = crate::commands::ExecutionHelper::merge_hooks(
            &infrastructure_hooks,
            resource.spec.hooks.as_ref(),
        );

        // Run pre-destroy hooks
        if !hooks.pre_destroy.is_empty()
            && HooksRunner::run_hooks(&hooks.pre_destroy, env_dir_str, "pre-destroy")?
                == HookOutcome::Cancel
        {
            ctx.output.warning(&format!(
                "  Destroy cancelled by pre-destroy hook for {} ({})",
                env.project_name, env.environment_name
            ));
            return Ok(());
        }

        // Initialize executor
        ctx.output.dimmed("  Initializing...");
        let init_output = executor.init(env_dir_str)?;

        if !init_output.status.success() {
            anyhow::bail!(
                "Initialization failed with exit code: {:?}",
                init_output.status.code()
            );
        }

        // Build executor config
        let mut command_options = std::collections::HashMap::new();

        if let Some(config) = &executor_config.config {
            for (cmd_name, cmd_config) in &config.commands {
                command_options.insert(cmd_name.clone(), cmd_config.options.clone());
            }
        }

        let execution_config = ExecutorConfig {
            plan_command: None,
            apply_command: None,
            destroy_command: None,
            refresh_command: None,
            test_command: None,
            command_options,
        };

        // Run destroy
        ctx.output.dimmed("  Running destroy...");
        executor.destroy(&execution_config, env_dir_str, &[])?;

        // Run post-destroy hooks
        if !hooks.post_destroy.is_empty() {
            let _ = HooksRunner::run_hooks(&hooks.post_destroy, env_dir_str, "post-destroy");
        }

        Ok(())
    }

    /// Get executor by name
    fn get_executor(name: &str) -> Result<Box<dyn Executor>> {
        match name {
            "opentofu" | "terraform" => Ok(Box::new(OpenTofuExecutor)),
            _ => anyhow::bail!("Unknown executor: {}", name),
        }
    }

    /// Compare inputs between two environments
    fn compare_inputs(
        source: &HashMap<String, serde_json::Value>,
        target: &HashMap<String, serde_json::Value>,
    ) -> EnvDiff {
        let source_keys: HashSet<_> = source.keys().collect();
        let target_keys: HashSet<_> = target.keys().collect();

        let only_in_source: Vec<String> = source_keys
            .difference(&target_keys)
            .map(|k| k.to_string())
            .collect();

        let only_in_target: Vec<String> = target_keys
            .difference(&source_keys)
            .map(|k| k.to_string())
            .collect();

        let mut different_values = Vec::new();

        for key in source_keys.intersection(&target_keys) {
            let source_val = &source[*key];
            let target_val = &target[*key];

            if source_val != target_val {
                different_values.push((
                    key.to_string(),
                    Self::value_to_string(source_val),
                    Self::value_to_string(target_val),
                ));
            }
        }

        EnvDiff {
            only_in_source,
            only_in_target,
            different_values,
        }
    }

    /// Convert JSON value to string
    fn value_to_string(value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            _ => value.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_value_to_string_string() {
        let value = json!("hello");
        assert_eq!(EnvCommand::value_to_string(&value), "hello");
    }

    #[test]
    fn test_value_to_string_number() {
        let value = json!(42);
        assert_eq!(EnvCommand::value_to_string(&value), "42");
    }

    #[test]
    fn test_value_to_string_float() {
        let value = json!(3.14);
        assert_eq!(EnvCommand::value_to_string(&value), "3.14");
    }

    #[test]
    fn test_value_to_string_bool_true() {
        let value = json!(true);
        assert_eq!(EnvCommand::value_to_string(&value), "true");
    }

    #[test]
    fn test_value_to_string_bool_false() {
        let value = json!(false);
        assert_eq!(EnvCommand::value_to_string(&value), "false");
    }

    #[test]
    fn test_value_to_string_array() {
        let value = json!([1, 2, 3]);
        assert_eq!(EnvCommand::value_to_string(&value), "[1,2,3]");
    }

    #[test]
    fn test_value_to_string_object() {
        let value = json!({"key": "value"});
        assert_eq!(EnvCommand::value_to_string(&value), "{\"key\":\"value\"}");
    }

    #[test]
    fn test_compare_inputs_identical() {
        let mut source = HashMap::new();
        source.insert("key1".to_string(), json!("value1"));
        source.insert("key2".to_string(), json!(42));

        let target = source.clone();
        let diff = EnvCommand::compare_inputs(&source, &target);

        assert!(diff.only_in_source.is_empty());
        assert!(diff.only_in_target.is_empty());
        assert!(diff.different_values.is_empty());
    }

    #[test]
    fn test_compare_inputs_only_in_source() {
        let mut source = HashMap::new();
        source.insert("key1".to_string(), json!("value1"));
        source.insert("key2".to_string(), json!("value2"));

        let mut target = HashMap::new();
        target.insert("key1".to_string(), json!("value1"));

        let diff = EnvCommand::compare_inputs(&source, &target);

        assert_eq!(diff.only_in_source, vec!["key2".to_string()]);
        assert!(diff.only_in_target.is_empty());
        assert!(diff.different_values.is_empty());
    }

    #[test]
    fn test_compare_inputs_only_in_target() {
        let mut source = HashMap::new();
        source.insert("key1".to_string(), json!("value1"));

        let mut target = HashMap::new();
        target.insert("key1".to_string(), json!("value1"));
        target.insert("key2".to_string(), json!("value2"));

        let diff = EnvCommand::compare_inputs(&source, &target);

        assert!(diff.only_in_source.is_empty());
        assert_eq!(diff.only_in_target, vec!["key2".to_string()]);
        assert!(diff.different_values.is_empty());
    }

    #[test]
    fn test_compare_inputs_different_values() {
        let mut source = HashMap::new();
        source.insert("key1".to_string(), json!("value1"));
        source.insert("replicas".to_string(), json!(2));

        let mut target = HashMap::new();
        target.insert("key1".to_string(), json!("value1"));
        target.insert("replicas".to_string(), json!(5));

        let diff = EnvCommand::compare_inputs(&source, &target);

        assert!(diff.only_in_source.is_empty());
        assert!(diff.only_in_target.is_empty());
        assert_eq!(diff.different_values.len(), 1);
        assert_eq!(diff.different_values[0].0, "replicas");
        assert_eq!(diff.different_values[0].1, "2");
        assert_eq!(diff.different_values[0].2, "5");
    }

    #[test]
    fn test_compare_inputs_complex_diff() {
        let mut source = HashMap::new();
        source.insert("common".to_string(), json!("same"));
        source.insert("only_source".to_string(), json!("a"));
        source.insert("different".to_string(), json!("source_val"));

        let mut target = HashMap::new();
        target.insert("common".to_string(), json!("same"));
        target.insert("only_target".to_string(), json!("b"));
        target.insert("different".to_string(), json!("target_val"));

        let diff = EnvCommand::compare_inputs(&source, &target);

        assert_eq!(diff.only_in_source, vec!["only_source".to_string()]);
        assert_eq!(diff.only_in_target, vec!["only_target".to_string()]);
        assert_eq!(diff.different_values.len(), 1);
        assert_eq!(diff.different_values[0].0, "different");
    }

    #[test]
    fn test_compare_inputs_empty_source() {
        let source = HashMap::new();

        let mut target = HashMap::new();
        target.insert("key1".to_string(), json!("value1"));

        let diff = EnvCommand::compare_inputs(&source, &target);

        assert!(diff.only_in_source.is_empty());
        assert_eq!(diff.only_in_target, vec!["key1".to_string()]);
        assert!(diff.different_values.is_empty());
    }

    #[test]
    fn test_compare_inputs_empty_target() {
        let mut source = HashMap::new();
        source.insert("key1".to_string(), json!("value1"));

        let target = HashMap::new();
        let diff = EnvCommand::compare_inputs(&source, &target);

        assert_eq!(diff.only_in_source, vec!["key1".to_string()]);
        assert!(diff.only_in_target.is_empty());
        assert!(diff.different_values.is_empty());
    }

    #[test]
    fn test_compare_inputs_both_empty() {
        let source = HashMap::new();
        let target = HashMap::new();

        let diff = EnvCommand::compare_inputs(&source, &target);

        assert!(diff.only_in_source.is_empty());
        assert!(diff.only_in_target.is_empty());
        assert!(diff.different_values.is_empty());
    }
}
