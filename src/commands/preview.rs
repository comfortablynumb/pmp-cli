use crate::collection::CollectionDiscovery;
use crate::executor::{Executor, ExecutorConfig, OpenTofuExecutor};
use crate::hooks::HooksRunner;
use crate::template::ProjectResource;
use anyhow::{Context, Result};
use std::path::Path;

/// Handles the 'preview' command - runs executor plan with hooks
pub struct PreviewCommand;

impl PreviewCommand {
    /// Execute the preview command
    pub fn execute(project_path: Option<&str>) -> Result<()> {
        // Determine project directory
        let project_dir = project_path.unwrap_or(".");
        let project_path_obj = Path::new(project_dir);

        // Load project metadata
        let metadata_path = project_path_obj.join(".pmp.yaml");

        if !metadata_path.exists() {
            anyhow::bail!(
                "No .pmp.yaml found in {}. Is this a PMP project?",
                project_dir
            );
        }

        let resource = ProjectResource::from_file(&metadata_path)
            .context("Failed to load project resource")?;

        println!("Project: {}", resource.metadata.name);

        if let Some(desc) = &resource.metadata.description {
            println!("Description: {}", desc);
        }

        println!("Kind: {}", resource.spec.resource.kind);

        // Get executor configuration
        let executor_config = resource.get_executor_config();

        // Load collection to get hooks
        let (collection, _collection_root) = CollectionDiscovery::find_collection()?
            .context("ProjectCollection is required to run commands")?;

        let hooks = collection.get_hooks();

        // Get executor
        let executor = Self::get_executor(&executor_config.name)?;

        // Check if executor is installed
        println!("\nChecking if {} is installed...", executor.get_name());

        if !executor.check_installed()? {
            anyhow::bail!(
                "{} is not installed or not available in PATH",
                executor.get_name()
            );
        }

        println!("✓ {} is available", executor.get_name());

        // Run pre-preview hooks
        if !hooks.pre_preview.is_empty() {
            HooksRunner::run_hooks(&hooks.pre_preview, project_dir, "pre-preview")?;
        }

        // Initialize executor
        println!("\nInitializing {}...", executor.get_name());
        let init_output = executor.init(project_dir)?;

        // Print init output
        if !init_output.stdout.is_empty() {
            println!("{}", String::from_utf8_lossy(&init_output.stdout));
        }

        if !init_output.stderr.is_empty() {
            eprintln!("{}", String::from_utf8_lossy(&init_output.stderr));
        }

        if !init_output.status.success() {
            anyhow::bail!("Initialization failed");
        }

        println!("✓ Initialization completed");

        // Build executor config
        let execution_config = ExecutorConfig {
            plan_command: None,
            apply_command: None,
        };

        // Run plan
        println!("\nRunning {} plan...", executor.get_name());
        let output = executor.plan(&execution_config, project_dir)?;

        // Print output
        println!("{}", String::from_utf8_lossy(&output.stdout));

        if !output.stderr.is_empty() {
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        }

        if !output.status.success() {
            anyhow::bail!("Plan command failed");
        }

        // Run post-preview hooks
        if !hooks.post_preview.is_empty() {
            HooksRunner::run_hooks(&hooks.post_preview, project_dir, "post-preview")?;
        }

        println!("\n✓ Preview completed successfully");

        Ok(())
    }

    /// Get the appropriate executor based on name
    fn get_executor(name: &str) -> Result<Box<dyn Executor>> {
        match name {
            "opentofu" => Ok(Box::new(OpenTofuExecutor::new())),
            _ => anyhow::bail!("Unknown executor: {}", name),
        }
    }
}
