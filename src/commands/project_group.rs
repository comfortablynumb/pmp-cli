//! Project Group operations - handles creating/updating projects defined in a project group

use crate::collection::CollectionDiscovery;
use crate::executor::{Executor, ExecutorConfig, OpenTofuExecutor};
use crate::hooks::{HookOutcome, HooksRunner};
use crate::template::metadata::{ProjectGroupProject, ProjectGroupReferenceProject, ProjectReference, TemplateReferenceProject};
use crate::template::{DynamicProjectEnvironmentResource, TemplateDiscovery};
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Handles project group operations
pub struct ProjectGroupHandler;

impl ProjectGroupHandler {
    /// Process projects defined in a project group's spec.projects
    /// This will create or update each project defined in the configuration
    pub fn process_projects(
        ctx: &crate::context::Context,
        env_resource: &DynamicProjectEnvironmentResource,
        environment_name: &str,
        template_packs_paths: Option<&str>,
    ) -> Result<()> {
        let projects = &env_resource.spec.projects;

        if projects.is_empty() {
            return Ok(());
        }

        ctx.output.blank();
        ctx.output.subsection("Processing Project Group Projects");
        ctx.output.dimmed(&format!(
            "Found {} project(s) to process...",
            projects.len()
        ));

        for project_config in projects {
            Self::process_single_project(ctx, project_config, environment_name, template_packs_paths)?;
        }

        ctx.output.blank();
        ctx.output.success(&format!(
            "Processed {} project(s) from project group",
            projects.len()
        ));

        Ok(())
    }

    /// Execute a command (preview, apply, destroy) on all configured projects
    /// For destroy, projects are executed in reverse order
    pub fn execute_command_on_projects(
        ctx: &crate::context::Context,
        env_resource: &DynamicProjectEnvironmentResource,
        environment_name: &str,
        command: &str,
        extra_args: &[String],
    ) -> Result<()> {
        let projects = &env_resource.spec.projects;

        if projects.is_empty() {
            return Ok(());
        }

        ctx.output.blank();
        ctx.output.subsection(&format!(
            "Executing {} on Project Group Projects",
            command
        ));

        // For destroy, execute in reverse order
        let projects_to_execute: Vec<_> = if command == "destroy" {
            projects.iter().rev().collect()
        } else {
            projects.iter().collect()
        };

        ctx.output.dimmed(&format!(
            "Executing {} on {} project(s){}...",
            command,
            projects_to_execute.len(),
            if command == "destroy" { " (reverse order)" } else { "" }
        ));

        for (i, project_config) in projects_to_execute.iter().enumerate() {
            ctx.output.blank();
            ctx.output.subsection(&format!(
                "Step {}/{}: {} - {}",
                i + 1,
                projects_to_execute.len(),
                project_config.name,
                command
            ));

            Self::execute_command_on_single_project(
                ctx,
                project_config,
                environment_name,
                command,
                extra_args,
            )?;
        }

        ctx.output.blank();
        ctx.output.success(&format!(
            "{} completed for {} project(s) in project group",
            command,
            projects_to_execute.len()
        ));

        Ok(())
    }

    /// Execute a command on a single project
    fn execute_command_on_single_project(
        ctx: &crate::context::Context,
        project_config: &ProjectGroupProject,
        environment_name: &str,
        command: &str,
        extra_args: &[String],
    ) -> Result<()> {
        // Find the project
        let (_infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required")?;

        let existing_projects = CollectionDiscovery::discover_projects(
            &*ctx.fs,
            &*ctx.output,
            &infrastructure_root,
        )?;

        let project_info = existing_projects
            .iter()
            .find(|p| p.name == project_config.name);

        let Some(project_info) = project_info else {
            ctx.output.warning(&format!(
                "Project '{}' not found, skipping {}",
                project_config.name, command
            ));
            return Ok(());
        };

        // project_info.path is relative to infrastructure_root (e.g., "projects/vault")
        let project_path = infrastructure_root.join(&project_info.path);
        let env_path = project_path
            .join("environments")
            .join(environment_name);

        if !ctx.fs.exists(&env_path) {
            ctx.output.warning(&format!(
                "Environment '{}' not found for project '{}', skipping {}",
                environment_name, project_config.name, command
            ));
            return Ok(());
        }

        // Load environment resource
        let env_file = env_path.join(".pmp.environment.yaml");
        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
            .context("Failed to load environment resource")?;

        // Get executor configuration
        let executor_config = resource.get_executor_config();

        // Skip if executor is "none" (dependency-only project)
        if executor_config.name == "none" {
            ctx.output.dimmed(&format!(
                "Skipping {} ({}) - dependency-only project",
                project_config.name, environment_name
            ));
            return Ok(());
        }

        // Get executor
        let executor = Self::get_executor(&executor_config.name)?;

        // Execute the command
        Self::execute_with_executor(
            ctx,
            &env_path,
            executor.as_ref(),
            command,
            extra_args,
        )
    }

    /// Execute a command using the executor
    fn execute_with_executor(
        ctx: &crate::context::Context,
        env_path: &PathBuf,
        executor: &dyn Executor,
        command: &str,
        extra_args: &[String],
    ) -> Result<()> {
        let env_dir_str = env_path
            .to_str()
            .context("Failed to convert environment path to string")?;

        // Load collection to get infrastructure-level hooks
        let (collection, _collection_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required to run commands")?;

        let infrastructure_hooks = collection.get_hooks();

        // Load environment resource to get environment-level hooks
        let env_file = env_path.join(".pmp.environment.yaml");
        let env_resource = crate::template::DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
            .context("Failed to load environment resource")?;

        // Merge infrastructure hooks with environment hooks
        let hooks = crate::commands::ExecutionHelper::merge_hooks(
            &infrastructure_hooks,
            env_resource.spec.hooks.as_ref(),
        );

        // Run pre-hooks
        let pre_hooks = match command {
            "preview" => &hooks.pre_preview,
            "apply" => &hooks.pre_apply,
            "destroy" => &hooks.pre_destroy,
            _ => return Err(anyhow::anyhow!("Unknown command: {}", command)),
        };

        if !pre_hooks.is_empty() {
            if HooksRunner::run_hooks(pre_hooks, env_dir_str, &format!("pre-{}", command))?
                == HookOutcome::Cancel
            {
                ctx.output.warning(&format!("{} cancelled by pre-{} hook", command, command));
                return Ok(());
            }
        }

        // Initialize executor
        ctx.output.dimmed(&format!("Initializing {}...", executor.get_name()));
        let init_output = executor.init(env_dir_str)?;

        if !init_output.status.success() {
            if !init_output.stdout.is_empty() {
                if let Ok(stdout_str) = String::from_utf8(init_output.stdout.clone()) {
                    ctx.output.error(&stdout_str);
                }
            }
            if !init_output.stderr.is_empty() {
                if let Ok(stderr_str) = String::from_utf8(init_output.stderr.clone()) {
                    ctx.output.error(&stderr_str);
                }
            }
            anyhow::bail!(
                "Initialization failed with exit code: {:?}",
                init_output.status.code()
            );
        }

        // Build executor config
        let execution_config = ExecutorConfig {
            plan_command: None,
            apply_command: None,
            destroy_command: None,
            refresh_command: None,
        };

        // Execute the command
        match command {
            "preview" => {
                ctx.output.dimmed(&format!("Executing {} plan...", executor.get_name()));
                executor.plan(&execution_config, env_dir_str, extra_args)?;
            }
            "apply" => {
                ctx.output.dimmed(&format!("Executing {} apply...", executor.get_name()));
                executor.apply(&execution_config, env_dir_str, extra_args)?;
            }
            "destroy" => {
                ctx.output.dimmed(&format!("Executing {} destroy...", executor.get_name()));
                executor.destroy(&execution_config, env_dir_str, extra_args)?;
            }
            _ => return Err(anyhow::anyhow!("Unknown command: {}", command)),
        }

        // Run post-hooks
        let post_hooks = match command {
            "preview" => &hooks.post_preview,
            "apply" => &hooks.post_apply,
            "destroy" => &hooks.post_destroy,
            _ => return Err(anyhow::anyhow!("Unknown command: {}", command)),
        };

        if !post_hooks.is_empty() {
            if HooksRunner::run_hooks(post_hooks, env_dir_str, &format!("post-{}", command))?
                == HookOutcome::Cancel
            {
                ctx.output.warning(&format!("Post-{} hooks cancelled further execution", command));
                return Ok(());
            }
        }

        ctx.output.success(&format!("{} completed", command));
        Ok(())
    }

    /// Get the appropriate executor based on name
    fn get_executor(name: &str) -> Result<Box<dyn Executor>> {
        match name {
            "opentofu" => Ok(Box::new(OpenTofuExecutor::new())),
            _ => anyhow::bail!("Unknown executor: {}", name),
        }
    }

    /// Process a single project from the project group configuration
    fn process_single_project(
        ctx: &crate::context::Context,
        project_config: &ProjectGroupProject,
        environment_name: &str,
        template_packs_paths: Option<&str>,
    ) -> Result<()> {
        ctx.output.blank();
        ctx.output.dimmed(&format!(
            "Processing project: {} (template: {}/{})",
            project_config.name, project_config.template_pack, project_config.template
        ));

        // Check if project already exists
        let (_infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required")?;

        let existing_projects = CollectionDiscovery::discover_projects(
            &*ctx.fs,
            &*ctx.output,
            &infrastructure_root,
        )?;

        let project_exists = existing_projects
            .iter()
            .any(|p| p.name == project_config.name);

        // Load the template to get its dependencies (needed for data_source_name resolution)
        let template_dependencies = Self::load_template_dependencies(
            ctx,
            &project_config.template_pack,
            &project_config.template,
            template_packs_paths,
        )?;

        // Build inputs for the project
        let inputs = Self::build_project_inputs(
            ctx,
            project_config,
            template_packs_paths,
        )?;

        // Convert inputs to JSON string for passing to create/update command
        let inputs_json = if !inputs.is_empty() {
            Some(serde_json::to_string(&inputs)?)
        } else {
            None
        };

        // Resolve reference projects if configured
        let resolved_reference_projects = if !project_config.reference_projects.is_empty() {
            Self::resolve_reference_projects(
                ctx,
                &project_config.reference_projects,
                &template_dependencies,
                environment_name,
                &infrastructure_root,
                &existing_projects,
            )?
        } else {
            Vec::new()
        };

        if project_exists {
            // Update existing project
            ctx.output.dimmed(&format!(
                "  Project '{}' exists, updating...",
                project_config.name
            ));

            // Find project path
            let project_info = existing_projects
                .iter()
                .find(|p| p.name == project_config.name)
                .unwrap();

            // project_info.path is relative to infrastructure_root (e.g., "projects/vault")
            let project_path = infrastructure_root.join(&project_info.path);
            let env_path = project_path
                .join("environments")
                .join(environment_name);

            if ctx.fs.exists(&env_path) {
                // Update reference projects in environment file if configured
                if !resolved_reference_projects.is_empty() {
                    Self::update_reference_projects_in_env_file(
                        ctx,
                        &env_path,
                        &resolved_reference_projects,
                    )?;
                }

                crate::commands::UpdateCommand::execute(
                    ctx,
                    Some(env_path.to_str().unwrap()),
                    template_packs_paths,
                    inputs_json.as_deref(),
                )?;
            } else {
                ctx.output.warning(&format!(
                    "  Environment '{}' not found for project '{}', skipping update",
                    environment_name, project_config.name
                ));
            }
        } else {
            // Create new project
            ctx.output.dimmed(&format!(
                "  Creating new project '{}'...",
                project_config.name
            ));

            // For creation, we need to use a programmatic approach
            // We'll call the create command logic directly with the inputs
            Self::create_project_programmatically(
                ctx,
                project_config,
                environment_name,
                template_packs_paths,
                &inputs,
                &resolved_reference_projects,
            )?;
        }

        ctx.output.success(&format!(
            "  Processed project: {}",
            project_config.name
        ));

        Ok(())
    }

    /// Resolve reference projects by name to their full details
    /// Uses the template's dependencies to get the correct data_source_name
    fn resolve_reference_projects(
        ctx: &crate::context::Context,
        reference_configs: &[ProjectGroupReferenceProject],
        template_dependencies: &[crate::template::metadata::TemplateDependency],
        default_environment: &str,
        infrastructure_root: &Path,
        existing_projects: &[ProjectReference],
    ) -> Result<Vec<TemplateReferenceProject>> {
        let mut resolved = Vec::new();

        for (index, ref_config) in reference_configs.iter().enumerate() {
            // Find the reference project
            let project_info = existing_projects
                .iter()
                .find(|p| p.name == ref_config.name)
                .context(format!(
                    "Reference project '{}' not found in collection",
                    ref_config.name
                ))?;

            // project_info.path is relative to infrastructure_root (e.g., "projects/vault")
            let project_path = infrastructure_root.join(&project_info.path);

            // Use configured environment or default to same environment
            let ref_env_name = ref_config
                .environment
                .as_deref()
                .unwrap_or(default_environment);

            let env_path = project_path
                .join("environments")
                .join(ref_env_name);

            let env_file = env_path.join(".pmp.environment.yaml");

            if !ctx.fs.exists(&env_file) {
                anyhow::bail!(
                    "Environment '{}' not found for reference project '{}'",
                    ref_env_name,
                    ref_config.name
                );
            }

            // Load the reference project's environment resource
            let ref_resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
                .context(format!(
                    "Failed to load environment resource for reference project '{}'",
                    ref_config.name
                ))?;

            // Get data_source_name from the template's dependency configuration
            // Match by api_version and kind to find the corresponding dependency
            let data_source_name = template_dependencies
                .iter()
                .find(|dep| {
                    dep.project.api_version == ref_resource.api_version
                        && dep.project.kind == ref_resource.kind
                })
                .and_then(|dep| {
                    dep.project
                        .remote_state
                        .as_ref()
                        .map(|rs| rs.data_source_name.clone())
                })
                // Fallback: use ref_config.data_source_name if provided, or generate default
                .or_else(|| ref_config.data_source_name.clone())
                .unwrap_or_else(|| format!("ref_{}", index));

            resolved.push(TemplateReferenceProject {
                api_version: ref_resource.api_version.clone(),
                kind: ref_resource.kind.clone(),
                name: ref_resource.metadata.name.clone(),
                environment: ref_env_name.to_string(),
                data_source_name: data_source_name.clone(),
            });

            ctx.output.dimmed(&format!(
                "  Resolved reference project: {} ({}) -> data_source: template_ref_{}",
                ref_config.name, ref_env_name, data_source_name
            ));
        }

        Ok(resolved)
    }

    /// Update the template_reference_projects in an environment file
    fn update_reference_projects_in_env_file(
        ctx: &crate::context::Context,
        env_path: &Path,
        reference_projects: &[TemplateReferenceProject],
    ) -> Result<()> {
        let env_file = env_path.join(".pmp.environment.yaml");

        // Load current environment resource
        let mut env_resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
            .context("Failed to load environment resource")?;

        // Update template_reference_projects
        env_resource.spec.template_reference_projects = reference_projects.to_vec();

        // Serialize and write back
        let yaml_content = serde_yaml::to_string(&env_resource)
            .context("Failed to serialize environment resource")?;

        ctx.fs.write(&env_file, &yaml_content)
            .context("Failed to write environment file")?;

        ctx.output.dimmed(&format!(
            "  Updated reference projects in environment file ({} references)",
            reference_projects.len()
        ));

        Ok(())
    }

    /// Build inputs for a project based on the project group configuration
    fn build_project_inputs(
        ctx: &crate::context::Context,
        project_config: &ProjectGroupProject,
        template_packs_paths: Option<&str>,
    ) -> Result<HashMap<String, Value>> {
        let mut inputs = HashMap::new();

        // If use_all_defaults is true, we don't need to build any inputs
        // The create/update command will use template defaults
        if project_config.use_all_defaults {
            return Ok(inputs);
        }

        // Find the template to get its input definitions
        let flag_paths: Vec<String> = if let Some(paths) = template_packs_paths {
            crate::template::discovery::parse_colon_separated_paths(paths)
        } else {
            vec![]
        };

        let env_paths: Vec<String> = std::env::var("PMP_TEMPLATE_PACKS_PATHS")
            .ok()
            .map(|p| crate::template::discovery::parse_colon_separated_paths(&p))
            .unwrap_or_default();

        let mut all_paths = flag_paths;
        all_paths.extend(env_paths);
        let custom_paths: Vec<&str> = all_paths.iter().map(|s| s.as_str()).collect();

        let all_template_packs = TemplateDiscovery::discover_template_packs_with_custom_paths(
            &*ctx.fs,
            &*ctx.output,
            &custom_paths,
        )?;

        // Find the template pack and template
        let template_pack = all_template_packs
            .iter()
            .find(|pack| {
                pack.resource
                    .metadata
                    .name
                    .to_lowercase()
                    .replace(' ', "-")
                    == project_config.template_pack.to_lowercase()
                    || pack.resource.metadata.name.to_lowercase()
                        == project_config.template_pack.to_lowercase()
            });

        if let Some(pack) = template_pack {
            // Discover templates in the pack
            let templates = TemplateDiscovery::discover_templates_in_pack(
                &*ctx.fs,
                &*ctx.output,
                &pack.path,
            )?;

            let template = templates.iter().find(|t| {
                t.resource.metadata.name.to_lowercase() == project_config.template.to_lowercase()
            });

            if let Some(tmpl) = template {
                // Build inputs based on the configuration
                for input_def in &tmpl.resource.spec.inputs {
                    if let Some(input_config) = project_config.inputs.get(&input_def.name) {
                        // Use the configured input
                        if let Some(value) = &input_config.value {
                            inputs.insert(input_def.name.clone(), value.clone());
                        } else if input_config.use_default {
                            // use_default is true but no value specified
                            // The create/update command will use the template default
                            if let Some(default) = &input_def.default {
                                inputs.insert(input_def.name.clone(), default.clone());
                            }
                        }
                    }
                    // If no config for this input, it will be prompted or use default
                }
            }
        }

        Ok(inputs)
    }

    /// Load template dependencies for data_source_name resolution
    fn load_template_dependencies(
        ctx: &crate::context::Context,
        template_pack_name: &str,
        template_name: &str,
        template_packs_paths: Option<&str>,
    ) -> Result<Vec<crate::template::metadata::TemplateDependency>> {
        // Discover template packs
        let flag_paths: Vec<String> = if let Some(paths) = template_packs_paths {
            crate::template::discovery::parse_colon_separated_paths(paths)
        } else {
            vec![]
        };

        let env_paths: Vec<String> = std::env::var("PMP_TEMPLATE_PACKS_PATHS")
            .ok()
            .map(|p| crate::template::discovery::parse_colon_separated_paths(&p))
            .unwrap_or_default();

        let mut all_paths = flag_paths;
        all_paths.extend(env_paths);
        let custom_paths: Vec<&str> = all_paths.iter().map(|s| s.as_str()).collect();

        let all_template_packs = TemplateDiscovery::discover_template_packs_with_custom_paths(
            &*ctx.fs,
            &*ctx.output,
            &custom_paths,
        )?;

        // Find the template pack
        let template_pack = all_template_packs
            .iter()
            .find(|pack| {
                pack.resource
                    .metadata
                    .name
                    .to_lowercase()
                    .replace(' ', "-")
                    == template_pack_name.to_lowercase()
                    || pack.resource.metadata.name.to_lowercase()
                        == template_pack_name.to_lowercase()
            });

        if let Some(pack) = template_pack {
            // Discover templates in the pack
            let templates = TemplateDiscovery::discover_templates_in_pack(
                &*ctx.fs,
                &*ctx.output,
                &pack.path,
            )?;

            let template = templates.iter().find(|t| {
                t.resource.metadata.name.to_lowercase() == template_name.to_lowercase()
            });

            if let Some(tmpl) = template {
                return Ok(tmpl.resource.spec.dependencies.clone());
            }
        }

        // Return empty if template not found (shouldn't happen, but handle gracefully)
        Ok(Vec::new())
    }

    /// Create a project programmatically without interactive prompts
    fn create_project_programmatically(
        ctx: &crate::context::Context,
        project_config: &ProjectGroupProject,
        environment_name: &str,
        template_packs_paths: Option<&str>,
        inputs: &HashMap<String, Value>,
        reference_projects: &[TemplateReferenceProject],
    ) -> Result<()> {
        // Use the non-interactive project creation from CreateCommand
        crate::commands::CreateCommand::create_project_non_interactive(
            ctx,
            &project_config.name,
            &project_config.template_pack,
            &project_config.template,
            environment_name,
            inputs,
            project_config.use_all_defaults,
            reference_projects,
            template_packs_paths,
        )
    }
}
