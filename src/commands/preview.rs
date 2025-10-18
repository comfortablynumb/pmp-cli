use crate::hooks::HooksRunner;
use crate::iac::{executor::IacConfig, IacExecutor, OpenTofuExecutor};
use crate::template::ProjectResource;
use anyhow::{Context, Result};
use std::path::Path;

/// Handles the 'preview' command - runs IaC plan with hooks
pub struct PreviewCommand;

impl PreviewCommand {
    /// Execute the preview command
    pub fn execute(project_path: Option<&str>) -> Result<()> {
        // Determine project directory
        let project_dir = project_path.unwrap_or(".");
        let project_path_obj = Path::new(project_dir);

        // Load project metadata
        let metadata_path = project_path_obj.join(".pmp.project.yaml");

        if !metadata_path.exists() {
            anyhow::bail!(
                "No .pmp.project.yaml found in {}. Is this a PMP project?",
                project_dir
            );
        }

        let resource = ProjectResource::from_file(&metadata_path)
            .context("Failed to load project resource")?;

        println!("Project: {}", resource.metadata.name);

        if let Some(desc) = &resource.metadata.description {
            println!("Description: {}", desc);
        }

        println!("Kind: {}", resource.kind);

        // Get IaC configuration
        let iac_config = resource.get_iac_config();
        let hooks = resource.get_hooks();

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

        // Run pre-preview hooks
        if !hooks.pre_preview.is_empty() {
            HooksRunner::run_hooks(&hooks.pre_preview, project_dir, "pre-preview")?;
        }

        // Build IaC config
        let iac_execution_config = IacConfig {
            plan_command: iac_config.commands.as_ref().and_then(|c| c.plan.clone()),
            apply_command: None,
        };

        // Run plan
        println!("\nRunning {} plan...", executor.get_name());
        let output = executor.plan(&iac_execution_config, project_dir)?;

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
    fn get_executor(name: &str) -> Result<Box<dyn IacExecutor>> {
        match name {
            "opentofu" => Ok(Box::new(OpenTofuExecutor::new())),
            _ => anyhow::bail!("Unknown IaC executor: {}", name),
        }
    }
}
