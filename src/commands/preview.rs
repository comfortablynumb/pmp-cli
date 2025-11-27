use crate::collection::{CollectionDiscovery, CollectionManager};
use crate::commands::project_group::ProjectGroupHandler;
use crate::executor::{Executor, ExecutorConfig, OpenTofuExecutor};
use crate::hooks::{HookOutcome, HooksRunner};
use crate::template::{DynamicProjectEnvironmentResource, ProjectResource};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Handles the 'preview' command - runs executor plan with hooks
pub struct PreviewCommand;

impl PreviewCommand {
    /// Execute the preview command
    pub fn execute(
        ctx: &crate::context::Context,
        project_path: Option<&str>,
        extra_args: &[String],
    ) -> Result<()> {
        // Check for template packs before proceeding
        let env_paths: Vec<String> = std::env::var("PMP_TEMPLATE_PACKS_PATHS")
            .ok()
            .map(|p| crate::template::discovery::parse_colon_separated_paths(&p))
            .unwrap_or_default();
        let custom_paths: Vec<&str> = env_paths.iter().map(|s| s.as_str()).collect();

        let template_packs =
            crate::template::TemplateDiscovery::discover_template_packs_with_custom_paths(
                &*ctx.fs,
                &*ctx.output,
                &custom_paths,
            )
            .context("Failed to discover template packs")?;

        if !crate::template::check_and_offer_installation(&*ctx.fs, &*ctx.output, &template_packs)?
        {
            anyhow::bail!("Cannot proceed without template packs.");
        }

        // Determine working directory
        let work_dir = if let Some(path) = project_path {
            PathBuf::from(path)
        } else {
            std::env::current_dir().context("Failed to get current directory")?
        };

        // Detect context and get environment path
        let (env_path, project_name, env_name) =
            Self::detect_and_select_environment(ctx, &work_dir)?;

        // Load environment resource
        let env_file = env_path.join(".pmp.environment.yaml");
        if !ctx.fs.exists(&env_file) {
            anyhow::bail!("Environment file not found: {:?}", env_file);
        }

        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
            .context("Failed to load environment resource")?;

        ctx.output.section("Preview");
        ctx.output.key_value_highlight("Project", &project_name);
        ctx.output.environment_badge(&env_name);

        if let Some(desc) = &resource.metadata.description {
            ctx.output.key_value("Description", desc);
        }

        ctx.output.key_value("Kind", &resource.kind);

        // Get executor configuration
        let executor_config = resource.get_executor_config();

        // Check if this is a ProjectGroup with spec.projects defined
        // ProjectGroups have special handling - they execute preview on their defined projects
        if executor_config.name == "none" && !resource.spec.projects.is_empty() {
            // Load collection to get infrastructure-level hooks
            let (collection, _collection_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required to run commands")?;

            let infrastructure_hooks = collection.get_hooks();

            // Merge infrastructure hooks with environment hooks
            let hooks = crate::commands::ExecutionHelper::merge_hooks(
                &infrastructure_hooks,
                resource.spec.hooks.as_ref(),
            );
            let env_dir_str = env_path
                .to_str()
                .context("Failed to convert environment path to string")?;

            // Run pre-preview hooks
            if !hooks.pre_preview.is_empty() {
                if HooksRunner::run_hooks(&hooks.pre_preview, env_dir_str, "pre-preview")?
                    == HookOutcome::Cancel
                {
                    ctx.output.blank();
                    ctx.output.warning("Preview cancelled by pre-preview hook");
                    return Ok(());
                }
            }

            // Show ProjectGroup info
            ctx.output.blank();
            ctx.output.subsection("Project Group Projects");
            ctx.output.dimmed(&format!(
                "This project group has {} configured project(s):",
                resource.spec.projects.len()
            ));
            for project in &resource.spec.projects {
                ctx.output.dimmed(&format!(
                    "  - {} (template: {}/{})",
                    project.name, project.template_pack, project.template
                ));
            }

            // Execute preview on all configured projects
            ProjectGroupHandler::execute_command_on_projects(
                ctx,
                &resource,
                &env_name,
                "preview",
                extra_args,
            )?;

            // Run post-preview hooks
            if !hooks.post_preview.is_empty() {
                if HooksRunner::run_hooks(&hooks.post_preview, env_dir_str, "post-preview")?
                    == HookOutcome::Cancel
                {
                    ctx.output.blank();
                    ctx.output.warning("Post-preview hooks cancelled further execution");
                    return Ok(());
                }
            }

            ctx.output.blank();
            ctx.output.success("Preview completed successfully");
            return Ok(());
        }

        // Check for dependencies (non-ProjectGroup projects)
        let maybe_graph = crate::commands::ExecutionHelper::check_and_display_dependencies(
            ctx,
            &env_path,
            &project_name,
            &env_name,
            "preview",
        )?;

        if let Some(graph) = maybe_graph {
            // Execute preview on entire dependency graph
            crate::commands::ExecutionHelper::execute_on_graph(
                ctx,
                &graph,
                "preview",
                crate::commands::ExecutionHelper::execute_preview_on_node,
            )?;

            ctx.output.blank();
            ctx.output
                .success("Preview completed successfully for all projects");
            return Ok(());
        }

        // No dependencies - proceed with single project execution

        // Load collection to get infrastructure-level hooks
        let (collection, _collection_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required to run commands")?;

        let infrastructure_hooks = collection.get_hooks();

        // Merge infrastructure hooks with environment hooks
        let hooks = crate::commands::ExecutionHelper::merge_hooks(
            &infrastructure_hooks,
            resource.spec.hooks.as_ref(),
        );

        // Get executor
        let executor = Self::get_executor(&executor_config.name)?;

        // Check if executor is installed
        ctx.output.subsection("Prerequisites");
        ctx.output.dimmed(&format!(
            "Checking if {} is installed...",
            executor.get_name()
        ));

        if !executor.check_installed()? {
            anyhow::bail!(
                "{} is not installed or not available in PATH",
                executor.get_name()
            );
        }

        ctx.output.status_check(executor.get_name(), true);

        // Convert env_path to string for executor
        let env_dir_str = env_path
            .to_str()
            .context("Failed to convert environment path to string")?;

        // Run pre-preview hooks
        if !hooks.pre_preview.is_empty() {
            if HooksRunner::run_hooks(&hooks.pre_preview, env_dir_str, "pre-preview")?
                == HookOutcome::Cancel
            {
                ctx.output.blank();
                ctx.output.warning("Preview cancelled by pre-preview hook");
                return Ok(());
            }
        }

        // Run helm repo update if configured
        crate::commands::ExecutionHelper::run_helm_repo_update_if_needed(
            ctx,
            &collection,
            executor.get_name(),
        )?;

        // Initialize executor
        ctx.output.subsection("Initialization");
        ctx.output
            .dimmed(&format!("Initializing {}...", executor.get_name()));
        let init_output = executor.init(env_dir_str)?;

        if !init_output.status.success() {
            // Display captured stdout and stderr before failing
            if !init_output.stdout.is_empty()
                && let Ok(stdout_str) = String::from_utf8(init_output.stdout.clone())
            {
                ctx.output.error(&stdout_str);
            }
            if !init_output.stderr.is_empty()
                && let Ok(stderr_str) = String::from_utf8(init_output.stderr.clone())
            {
                ctx.output.error(&stderr_str);
            }
            anyhow::bail!(
                "Initialization failed with exit code: {:?}",
                init_output.status.code()
            );
        }

        ctx.output.success("Initialization completed");

        // Build executor config
        let execution_config = ExecutorConfig {
            plan_command: None,
            apply_command: None,
            destroy_command: None,
            refresh_command: None,
        };

        // Run plan
        ctx.output.subsection("Running Plan");
        ctx.output
            .dimmed(&format!("Executing {} plan...", executor.get_name()));
        executor.plan(&execution_config, env_dir_str, extra_args)?;

        // Run post-preview hooks
        if !hooks.post_preview.is_empty() {
            if HooksRunner::run_hooks(&hooks.post_preview, env_dir_str, "post-preview")?
                == HookOutcome::Cancel
            {
                ctx.output.blank();
                ctx.output.warning("Post-preview hooks cancelled further execution");
                return Ok(());
            }
        }

        ctx.output.blank();
        ctx.output.success("Preview completed successfully");

        Ok(())
    }

    /// Detect context and select project/environment
    /// Returns: (environment_path, project_name, environment_name)
    fn detect_and_select_environment(
        ctx: &crate::context::Context,
        work_dir: &Path,
    ) -> Result<(PathBuf, String, String)> {
        // Check if we're in an environment directory
        if let Some(env_info) = Self::check_in_environment(ctx, work_dir)? {
            return Ok(env_info);
        }

        // Check if we're in a project directory
        if let Some((project_path, project_name)) = Self::check_in_project(ctx, work_dir)? {
            let env_name = Self::select_environment(ctx, &project_path)?;
            let env_path = project_path.join("environments").join(&env_name);
            return Ok((env_path, project_name, env_name));
        }

        // We're in the collection or elsewhere - use find/search UI
        Self::select_project_and_environment(ctx)
    }

    /// Check if we're inside an environment directory
    fn check_in_environment(
        ctx: &crate::context::Context,
        dir: &Path,
    ) -> Result<Option<(PathBuf, String, String)>> {
        let env_file = dir.join(".pmp.environment.yaml");

        if ctx.fs.exists(&env_file) {
            let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;
            let env_name = resource.metadata.environment_name.clone();
            let project_name = resource.metadata.name.clone();

            return Ok(Some((dir.to_path_buf(), project_name, env_name)));
        }

        Ok(None)
    }

    /// Check if we're inside a project directory (but not in an environment)
    fn check_in_project(
        ctx: &crate::context::Context,
        dir: &Path,
    ) -> Result<Option<(PathBuf, String)>> {
        let project_file = dir.join(".pmp.project.yaml");

        if ctx.fs.exists(&project_file) {
            let resource = ProjectResource::from_file(&*ctx.fs, &project_file)?;
            return Ok(Some((dir.to_path_buf(), resource.metadata.name.clone())));
        }

        Ok(None)
    }

    /// Select an environment from a project
    fn select_environment(ctx: &crate::context::Context, project_path: &Path) -> Result<String> {
        let environments = CollectionDiscovery::discover_environments(&*ctx.fs, project_path)
            .context("Failed to discover environments")?;

        if environments.is_empty() {
            anyhow::bail!("No environments found in project: {:?}", project_path);
        }

        if environments.len() == 1 {
            ctx.output.environment_badge(&environments[0]);
            return Ok(environments[0].clone());
        }

        let selected = ctx
            .input
            .select("Select an environment:", environments)
            .context("Failed to select environment")?;

        Ok(selected)
    }

    /// Select project and environment using find/search UI
    fn select_project_and_environment(
        ctx: &crate::context::Context,
    ) -> Result<(PathBuf, String, String)> {
        let manager = CollectionManager::load(ctx).context("Failed to load collection")?;

        let all_projects = manager.get_all_projects();

        if all_projects.is_empty() {
            anyhow::bail!("No projects found in collection");
        }

        // Select project
        // Sort projects by name for consistent display
        let mut sorted_projects: Vec<_> = all_projects.iter().collect();
        sorted_projects.sort_by(|a, b| a.name.cmp(&b.name));

        let project_options: Vec<String> = sorted_projects
            .iter()
            .map(|p| format!("{} ({})", p.name, p.kind))
            .collect();

        let selected_project_display = ctx
            .input
            .select("Select a project:", project_options.clone())
            .context("Failed to select project")?;

        let project_index = project_options
            .iter()
            .position(|opt| opt == &selected_project_display)
            .context("Project not found")?;

        let selected_project = sorted_projects[project_index];
        let project_path = manager.get_project_path(selected_project);

        // Select environment
        let env_name = Self::select_environment(ctx, &project_path)?;
        let env_path = project_path.join("environments").join(&env_name);

        Ok((env_path, selected_project.name.clone(), env_name))
    }

    /// Get the appropriate executor based on name
    fn get_executor(name: &str) -> Result<Box<dyn Executor>> {
        match name {
            "opentofu" => Ok(Box::new(OpenTofuExecutor::new())),
            _ => anyhow::bail!("Unknown executor: {}", name),
        }
    }
}
