use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::output;
use crate::template::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use std::path::Path;

pub struct CloneCommand;

impl CloneCommand {
    /// Execute the clone command
    pub fn execute(
        ctx: &Context,
        source_project: Option<&str>,
        new_name: &str,
        environment: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Clone Project");

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
            ctx.output.dimmed("No projects found to clone.");
            return Ok(());
        }

        // Select source project
        let source_project_ref = if let Some(name) = source_project {
            projects
                .iter()
                .find(|p| p.name.to_lowercase() == name.to_lowercase())
                .context(format!("Project '{}' not found", name))?
        } else {
            // Interactive selection
            let project_options: Vec<String> = projects
                .iter()
                .map(|p| format!("{} ({})", p.name, p.kind))
                .collect();

            let selected =
                ctx.input
                    .select("Select project to clone:", project_options.clone(), None)?;

            // Find the selected project by matching the display string
            let selected_idx = project_options.iter().position(|s| s == &selected).unwrap();

            &projects[selected_idx]
        };

        ctx.output.subsection("Source Project");
        ctx.output.key_value("Name", &source_project_ref.name);
        ctx.output.key_value("Kind", &source_project_ref.kind);
        output::blank();

        // Get source project path
        let source_project_path = infrastructure_root.join(&source_project_ref.path);

        // Discover source environments
        let source_envs_dir = source_project_path.join("environments");
        let mut source_environments = Vec::new();

        if ctx.fs.exists(&source_envs_dir) {
            let env_entries = ctx.fs.read_dir(&source_envs_dir)?;
            for env_path in env_entries {
                let env_file = env_path.join(".pmp.environment.yaml");
                if ctx.fs.exists(&env_file)
                    && let Ok(env_name) = env_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .context("Failed to get environment name")
                {
                    source_environments.push(env_name.to_string());
                }
            }
        }

        if source_environments.is_empty() {
            anyhow::bail!("Source project has no environments to clone");
        }

        // Select environments to clone
        let environments_to_clone = if let Some(env) = environment {
            if !source_environments.contains(&env.to_string()) {
                anyhow::bail!(
                    "Environment '{}' not found in source project. Available: {}",
                    env,
                    source_environments.join(", ")
                );
            }
            vec![env.to_string()]
        } else {
            // Interactive multi-select
            ctx.output.subsection("Select Environments to Clone");
            ctx.input.multi_select(
                "Select environments (space to select, enter when done):",
                source_environments.clone(),
                None,
            )?
        };

        if environments_to_clone.is_empty() {
            anyhow::bail!("No environments selected");
        }

        ctx.output.subsection("Cloning Configuration");
        ctx.output.key_value("Source", &source_project_ref.name);
        ctx.output.key_value("New Name", new_name);
        ctx.output
            .key_value("Environments", &environments_to_clone.join(", "));
        output::blank();

        // Confirm clone
        let confirmed = ctx.input.confirm("Proceed with cloning?", Some(true))?;

        if !confirmed {
            ctx.output.dimmed("Clone cancelled.");
            return Ok(());
        }

        // Create new project directory
        // Project path format: projects/{project_name}
        let new_project_path = infrastructure_root.join("projects").join(new_name);

        if ctx.fs.exists(&new_project_path) {
            anyhow::bail!("Project '{}' already exists", new_name);
        }

        ctx.fs.create_dir_all(&new_project_path)?;

        // Clone .pmp.project.yaml
        let source_project_yaml = source_project_path.join(".pmp.project.yaml");
        let new_project_yaml = new_project_path.join(".pmp.project.yaml");

        let mut project_yaml_content = ctx.fs.read_to_string(&source_project_yaml)?;

        // Replace project name in YAML
        project_yaml_content = project_yaml_content.replace(
            &format!("name: {}", source_project_ref.name),
            &format!("name: {}", new_name),
        );

        ctx.fs.write(&new_project_yaml, &project_yaml_content)?;

        ctx.output
            .success(&format!("Created project identifier: {}", new_name));

        // Clone each environment
        let new_envs_dir = new_project_path.join("environments");
        ctx.fs.create_dir_all(&new_envs_dir)?;

        for env_name in &environments_to_clone {
            ctx.output
                .subsection(&format!("Cloning environment: {}", env_name));

            let source_env_path = source_envs_dir.join(env_name);
            let new_env_path = new_envs_dir.join(env_name);

            ctx.fs.create_dir_all(&new_env_path)?;

            // Copy all files from source environment
            Self::copy_directory_contents(ctx, &source_env_path, &new_env_path, new_name)?;

            ctx.output
                .success(&format!("Cloned environment: {}", env_name));
        }

        output::blank();
        ctx.output.success("Project cloned successfully!");
        ctx.output
            .key_value("Location", &new_project_path.display().to_string());

        output::blank();
        ctx.output.info("Next steps:");
        ctx.output
            .dimmed("1. Review and customize the cloned project configuration");
        ctx.output
            .dimmed("2. Update any environment-specific settings");
        ctx.output.dimmed(&format!(
            "3. Initialize: cd {} && tofu init",
            new_project_path.display()
        ));

        Ok(())
    }

    /// Copy directory contents recursively
    fn copy_directory_contents(
        ctx: &Context,
        source: &Path,
        target: &Path,
        new_project_name: &str,
    ) -> Result<()> {
        let entries = ctx.fs.walk_dir(source, 10)?;

        for entry in entries {
            if ctx.fs.is_file(&entry) {
                let relative_path = entry
                    .strip_prefix(source)
                    .context("Failed to get relative path")?;
                let target_file = target.join(relative_path);

                // Create parent directory if needed
                if let Some(parent) = target_file.parent() {
                    ctx.fs.create_dir_all(parent)?;
                }

                // Read source file
                let mut content = ctx.fs.read_to_string(&entry)?;

                // Replace project name references in .pmp.environment.yaml
                if entry.file_name() == Some(std::ffi::OsStr::new(".pmp.environment.yaml")) {
                    // Parse and update the environment resource
                    if let Ok(mut env_resource) =
                        DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &entry)
                    {
                        env_resource.metadata.name = new_project_name.to_string();
                        content = serde_yaml::to_string(&env_resource)?;
                    }
                }

                // Write to target
                ctx.fs.write(&target_file, &content)?;
            }
        }

        Ok(())
    }
}
