use crate::collection::CollectionDiscovery;
use crate::hooks::HooksRunner;
use crate::iac::{executor::IacConfig, IacExecutor, OpenTofuExecutor};
use crate::template::ProjectResource;
use anyhow::{Context, Result};
use std::path::Path;

/// Handles the 'apply' command - runs IaC apply with hooks
pub struct ApplyCommand;

impl ApplyCommand {
    /// Execute the apply command
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

        // Get IaC configuration
        let iac_config = resource.get_iac_config();

        // Load collection to get hooks
        let (collection, _collection_root) = CollectionDiscovery::find_collection()?
            .context("ProjectCollection is required to run commands")?;

        let hooks = collection.get_hooks();

        // Get executor
        let executor = Self::get_executor(&iac_config.executor)?;

        // Check if executor is installed
        println!("\nChecking if {} is installed...", executor.get_name());

        if !executor.check_installed()? {
            anyhow::bail!(
                "{} is not installed or not available in PATH",
                executor.get_name()
            );
        }

        println!("✓ {} is available", executor.get_name());

        // Run pre-apply hooks
        if !hooks.pre_apply.is_empty() {
            HooksRunner::run_hooks(&hooks.pre_apply, project_dir, "pre-apply")?;
        }

        // Build IaC config
        let iac_execution_config = IacConfig {
            plan_command: None,
            apply_command: None,
        };

        // Run apply
        println!("\nRunning {} apply...", executor.get_name());
        let output = executor.apply(&iac_execution_config, project_dir)?;

        // Print output
        println!("{}", String::from_utf8_lossy(&output.stdout));

        if !output.stderr.is_empty() {
            eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        }

        if !output.status.success() {
            anyhow::bail!("Apply command failed");
        }

        // Run post-apply hooks
        if !hooks.post_apply.is_empty() {
            HooksRunner::run_hooks(&hooks.post_apply, project_dir, "post-apply")?;
        }

        println!("\n✓ Apply completed successfully");

        Ok(())
    }

    /// Get the appropriate executor based on name
    fn get_executor(name: &str) -> Result<Box<dyn IacExecutor>> {
        match name {
            "opentofu" => Ok(Box::new(OpenTofuExecutor::new())),
            _ => anyhow::bail!("Unknown IaC executor: {}", name),
        }
    }
}
