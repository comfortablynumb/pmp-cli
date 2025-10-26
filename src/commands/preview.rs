use crate::collection::{CollectionDiscovery, CollectionManager};
use crate::executor::{Executor, ExecutorConfig, OpenTofuExecutor};
use crate::hooks::HooksRunner;
use crate::output;
use crate::template::{ProjectResource, DynamicProjectEnvironmentResource};
use anyhow::{Context, Result};
use inquire::Select;
use std::path::{Path, PathBuf};

/// Handles the 'preview' command - runs executor plan with hooks
pub struct PreviewCommand;

impl PreviewCommand {
    /// Execute the preview command
    pub fn execute(project_path: Option<&str>) -> Result<()> {
        // Determine working directory
        let work_dir = if let Some(path) = project_path {
            PathBuf::from(path)
        } else {
            std::env::current_dir().context("Failed to get current directory")?
        };

        // Detect context and get environment path
        let (env_path, project_name, env_name) = Self::detect_and_select_environment(&work_dir)?;

        // Load environment resource
        let env_file = env_path.join(".pmp.environment.yaml");
        if !env_file.exists() {
            anyhow::bail!("Environment file not found: {:?}", env_file);
        }

        let resource = DynamicProjectEnvironmentResource::from_file(&env_file)
            .context("Failed to load environment resource")?;

        output::section("Preview");
        output::key_value_highlight("Project", &project_name);
        output::environment_badge(&env_name);

        if let Some(desc) = &resource.metadata.description {
            output::key_value("Description", desc);
        }

        output::key_value("Kind", &resource.kind);

        // Get executor configuration
        let executor_config = resource.get_executor_config();

        // Load collection to get hooks
        let (collection, _collection_root) = CollectionDiscovery::find_collection()?
            .context("ProjectCollection is required to run commands")?;

        let hooks = collection.get_hooks();

        // Get executor
        let executor = Self::get_executor(&executor_config.name)?;

        // Check if executor is installed
        output::subsection("Prerequisites");
        output::dimmed(&format!("Checking if {} is installed...", executor.get_name()));

        if !executor.check_installed()? {
            anyhow::bail!(
                "{} is not installed or not available in PATH",
                executor.get_name()
            );
        }

        output::status_check(executor.get_name(), true);

        // Convert env_path to string for executor
        let env_dir_str = env_path.to_str()
            .context("Failed to convert environment path to string")?;

        // Run pre-preview hooks
        if !hooks.pre_preview.is_empty() {
            HooksRunner::run_hooks(&hooks.pre_preview, env_dir_str, "pre-preview")?;
        }

        // Initialize executor
        output::subsection("Initialization");
        output::dimmed(&format!("Initializing {}...", executor.get_name()));
        let init_output = executor.init(env_dir_str)?;

        if !init_output.status.success() {
            anyhow::bail!("Initialization failed");
        }

        output::success("Initialization completed");

        // Build executor config
        let execution_config = ExecutorConfig {
            plan_command: None,
            apply_command: None,
        };

        // Run plan
        output::subsection("Running Plan");
        output::dimmed(&format!("Executing {} plan...", executor.get_name()));
        executor.plan(&execution_config, env_dir_str)?;

        // Run post-preview hooks
        if !hooks.post_preview.is_empty() {
            HooksRunner::run_hooks(&hooks.post_preview, env_dir_str, "post-preview")?;
        }

        output::blank();
        output::success("Preview completed successfully");

        Ok(())
    }

    /// Detect context and select project/environment
    /// Returns: (environment_path, project_name, environment_name)
    fn detect_and_select_environment(work_dir: &Path) -> Result<(PathBuf, String, String)> {
        // Check if we're in an environment directory
        if let Some(env_info) = Self::check_in_environment(work_dir)? {
            return Ok(env_info);
        }

        // Check if we're in a project directory
        if let Some((project_path, project_name)) = Self::check_in_project(work_dir)? {
            let env_name = Self::select_environment(&project_path)?;
            let env_path = project_path.join("environments").join(&env_name);
            return Ok((env_path, project_name, env_name));
        }

        // We're in the collection or elsewhere - use find/search UI
        Self::select_project_and_environment()
    }

    /// Check if we're inside an environment directory
    fn check_in_environment(dir: &Path) -> Result<Option<(PathBuf, String, String)>> {
        let env_file = dir.join(".pmp.environment.yaml");

        if env_file.exists() {
            let resource = DynamicProjectEnvironmentResource::from_file(&env_file)?;
            let env_name = resource.metadata.environment_name.clone();
            let project_name = resource.metadata.name.clone();

            return Ok(Some((dir.to_path_buf(), project_name, env_name)));
        }

        Ok(None)
    }

    /// Check if we're inside a project directory (but not in an environment)
    fn check_in_project(dir: &Path) -> Result<Option<(PathBuf, String)>> {
        let project_file = dir.join(".pmp.project.yaml");

        if project_file.exists() {
            let resource = ProjectResource::from_file(&project_file)?;
            return Ok(Some((dir.to_path_buf(), resource.metadata.name.clone())));
        }

        Ok(None)
    }

    /// Select an environment from a project
    fn select_environment(project_path: &Path) -> Result<String> {
        let environments = CollectionDiscovery::discover_environments(project_path)
            .context("Failed to discover environments")?;

        if environments.is_empty() {
            anyhow::bail!("No environments found in project: {:?}", project_path);
        }

        if environments.len() == 1 {
            output::environment_badge(&environments[0]);
            return Ok(environments[0].clone());
        }

        let selected = Select::new("Select an environment:", environments.clone())
            .prompt()
            .context("Failed to select environment")?;

        Ok(selected)
    }

    /// Select project and environment using find/search UI
    fn select_project_and_environment() -> Result<(PathBuf, String, String)> {
        let manager = CollectionManager::load()
            .context("Failed to load collection")?;

        let all_projects = manager.get_all_projects();

        if all_projects.is_empty() {
            anyhow::bail!("No projects found in collection");
        }

        // Select project
        let project_options: Vec<String> = all_projects
            .iter()
            .map(|p| format!("{} ({})", p.name, p.kind))
            .collect();

        let selected_project_display = Select::new("Select a project:", project_options.clone())
            .prompt()
            .context("Failed to select project")?;

        let project_index = project_options.iter().position(|opt| opt == &selected_project_display)
            .context("Project not found")?;

        let selected_project = &all_projects[project_index];
        let project_path = manager.get_project_path(selected_project);

        // Select environment
        let env_name = Self::select_environment(&project_path)?;
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
