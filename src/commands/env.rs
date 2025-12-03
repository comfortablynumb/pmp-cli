use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::output;
use crate::template::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use std::collections::{HashMap, HashSet};

pub struct EnvCommand;

#[derive(Debug)]
struct EnvDiff {
    only_in_source: Vec<String>,
    only_in_target: Vec<String>,
    different_values: Vec<(String, String, String)>, // (key, source_value, target_value)
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
