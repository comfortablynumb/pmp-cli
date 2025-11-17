use crate::context::Context;
use crate::output;
use crate::template::discovery::TemplateDiscovery;
use crate::template::metadata::{InfrastructureResource, InfrastructureTemplateResource};
use anyhow::{Context as AnyhowContext, Result};
use inquire::{Select, Text};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct InfrastructureCommand;

impl InfrastructureCommand {
    /// Initialize a new infrastructure
    pub fn execute_init(
        ctx: &Context,
        name: Option<&str>,
        description: Option<&str>,
        template_packs_paths: Option<&str>,
    ) -> Result<()> {
        // Delegate to the existing init command
        crate::commands::InitCommand::execute(ctx, name, description, template_packs_paths)
    }

    /// Create a new infrastructure from an infrastructure template
    pub fn execute_create(
        ctx: &Context,
        output: Option<&str>,
        template_packs_paths: Option<&str>,
    ) -> Result<()> {
        output::section("Creating infrastructure from template");

        // Determine output directory
        let output_dir = if let Some(output_path) = output {
            PathBuf::from(output_path)
        } else {
            std::env::current_dir().context("Failed to get current directory")?
        };

        // Check if directory already has an infrastructure
        let infra_file = output_dir.join(".pmp.infrastructure.yaml");
        if ctx.fs.exists(&infra_file) {
            anyhow::bail!(
                "Infrastructure already exists at {}. Cannot create a new infrastructure in this directory.",
                output_dir.display()
            );
        }

        // Get template pack paths
        let template_pack_paths = Self::get_template_pack_paths(template_packs_paths)?;

        // Discover infrastructure templates
        output::subsection("Discovering infrastructure templates...");
        let templates = Self::discover_infrastructure_templates(ctx, &template_pack_paths)?;

        if templates.is_empty() {
            anyhow::bail!(
                "No infrastructure templates found in the template packs.\n\
                \nTo use this feature, create an infrastructure template:\n\
                1. In your template pack, create a file: .pmp.infrastructure-template.yaml\n\
                2. Define the infrastructure template with apiVersion, kind: InfrastructureTemplate, metadata, and spec"
            );
        }

        // Let user select a template
        let template_choices: Vec<String> = templates
            .iter()
            .map(|(pack_name, template)| {
                format!(
                    "{} - {} ({})",
                    pack_name,
                    template.metadata.name,
                    template
                        .metadata
                        .description
                        .as_ref()
                        .unwrap_or(&"No description".to_string())
                )
            })
            .collect();

        let selected_idx = if templates.len() == 1 {
            output::info(&format!(
                "Auto-selecting the only available template: {}",
                template_choices[0]
            ));
            0
        } else {
            Select::new(
                "Select an infrastructure template:",
                template_choices.clone(),
            )
            .prompt()
            .context("Failed to get template selection")?;

            template_choices
                .iter()
                .position(|c| c == &template_choices[0])
                .unwrap_or(0)
        };

        let (template_pack_name, template) = &templates[selected_idx];
        let template_pack_path = &template_pack_paths
            .iter()
            .find(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n == template_pack_name)
                    .unwrap_or(false)
            })
            .context("Failed to find template pack path")?;

        output::subsection(&format!(
            "Selected template: {} from pack {}",
            template.metadata.name, template_pack_name
        ));

        // Collect inputs
        output::subsection("Collecting inputs...");
        let inputs = Self::collect_template_inputs(template)?;

        // Get infrastructure name
        let infra_name = Text::new("Infrastructure name:")
            .with_default("My Infrastructure")
            .prompt()
            .context("Failed to get infrastructure name")?;

        let infra_description = Text::new("Infrastructure description (optional):")
            .with_default("")
            .prompt()
            .ok();

        // Render the infrastructure
        output::subsection("Rendering infrastructure...");
        Self::render_infrastructure_from_template(
            ctx,
            template_pack_path,
            template,
            &infra_name,
            infra_description.as_deref(),
            &inputs,
            &output_dir,
        )?;

        output::success(&format!(
            "Infrastructure '{}' created successfully at {}",
            infra_name,
            output_dir.display()
        ));
        output::info(&format!(
            "Next steps:\n  cd {}\n  pmp project create",
            output_dir.display()
        ));

        Ok(())
    }

    /// List all infrastructures in the current directory tree
    pub fn execute_list(_ctx: &Context) -> Result<()> {
        output::subsection("Listing infrastructures...");

        // Find all .pmp.infrastructure.yaml files
        let current_dir = std::env::current_dir().context("Failed to get current directory")?;
        let infra_files = Self::find_infrastructure_files(&current_dir)?;

        if infra_files.is_empty() {
            output::info("No infrastructures found in the current directory tree.");
            return Ok(());
        }

        output::subsection(&format!("Found {} infrastructure(s):", infra_files.len()));
        for (path, name) in infra_files {
            println!("  • {} ({})", name, path.display());
        }

        Ok(())
    }

    /// Switch to a different infrastructure (placeholder for future multi-infrastructure support)
    pub fn execute_switch(_ctx: &Context, _name: &str) -> Result<()> {
        output::info("Infrastructure switching is not yet implemented.");
        output::info("This feature will allow managing multiple infrastructures in the future.");
        Ok(())
    }

    // Helper functions

    fn get_template_pack_paths(template_packs_paths: Option<&str>) -> Result<Vec<PathBuf>> {
        let mut paths = vec![
            dirs::home_dir()
                .map(|p| p.join(".pmp").join("template-packs"))
                .unwrap_or_else(|| PathBuf::from(".pmp/template-packs")),
            PathBuf::from(".pmp/template-packs"),
        ];

        if let Some(custom_paths) = template_packs_paths {
            for path in custom_paths.split(':') {
                if !path.is_empty() {
                    paths.push(PathBuf::from(path));
                }
            }
        }

        Ok(paths)
    }

    fn discover_infrastructure_templates(
        ctx: &Context,
        template_pack_paths: &[PathBuf],
    ) -> Result<Vec<(String, InfrastructureTemplateResource)>> {
        let mut templates = Vec::new();

        for base_path in template_pack_paths {
            if !ctx.fs.exists(base_path) {
                continue;
            }

            // Use existing template pack discovery
            let template_packs = TemplateDiscovery::discover_template_packs(
                &*ctx.fs,
                &*ctx.output,
            )?;

            for pack in template_packs {
                // Look for .pmp.infrastructure-template.yaml in the template pack root
                let infra_template_path = pack.path.join(".pmp.infrastructure-template.yaml");

                if ctx.fs.exists(&infra_template_path) {
                    match InfrastructureTemplateResource::from_file(&*ctx.fs, &infra_template_path)
                    {
                        Ok(template) => {
                            templates.push((pack.resource.metadata.name.clone(), template));
                        }
                        Err(e) => {
                            output::warning(&format!(
                                "Failed to load infrastructure template from {}: {}",
                                infra_template_path.display(),
                                e
                            ));
                        }
                    }
                }
            }
        }

        Ok(templates)
    }

    fn collect_template_inputs(
        template: &InfrastructureTemplateResource,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let mut inputs = HashMap::new();

        for input_def in &template.spec.inputs {
            let prompt_text = if let Some(ref desc) = input_def.description {
                format!("{} ({}):", input_def.name, desc)
            } else {
                format!("{}:", input_def.name)
            };

            let default_value = input_def
                .default
                .as_ref()
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let value = Text::new(&prompt_text)
                .with_default(default_value)
                .prompt()
                .context("Failed to get input")?;

            inputs.insert(input_def.name.clone(), serde_json::Value::String(value));
        }

        Ok(inputs)
    }

    fn render_infrastructure_from_template(
        ctx: &Context,
        _template_pack_path: &Path,
        template: &InfrastructureTemplateResource,
        infra_name: &str,
        infra_description: Option<&str>,
        _inputs: &HashMap<String, serde_json::Value>,
        output_dir: &Path,
    ) -> Result<()> {
        // Create the infrastructure resource
        let infrastructure = InfrastructureResource {
            api_version: "pmp.io/v1".to_string(),
            kind: "Infrastructure".to_string(),
            metadata: crate::template::metadata::InfrastructureMetadata {
                name: infra_name.to_string(),
                description: infra_description.map(String::from),
            },
            spec: crate::template::metadata::InfrastructureSpec {
                categories: template.spec.categories.clone(),
                template_packs: template.spec.template_packs.clone(),
                resource_kinds: Vec::new(),
                environments: template.spec.environments.clone(),
                hooks: template.spec.hooks.clone(),
                executor: template.spec.executor.clone(),
            },
        };

        // Note: Infrastructure templates define the structure but don't render files
        // The infrastructure.yaml itself is generated from the spec

        // Create the output directory if it doesn't exist
        ctx.fs.create_dir_all(output_dir)?;

        // Save the infrastructure file
        let infra_file_path = output_dir.join(".pmp.infrastructure.yaml");
        infrastructure.save(&*ctx.fs, &infra_file_path)?;

        // Create projects directory
        let projects_dir = output_dir.join("projects");
        ctx.fs.create_dir_all(&projects_dir)?;

        Ok(())
    }

    fn find_infrastructure_files(base_dir: &Path) -> Result<Vec<(PathBuf, String)>> {
        let mut results = Vec::new();

        Self::find_infrastructure_files_recursive(base_dir, &mut results)?;

        Ok(results)
    }

    fn find_infrastructure_files_recursive(
        dir: &Path,
        results: &mut Vec<(PathBuf, String)>,
    ) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        // Check for .pmp.infrastructure.yaml in this directory
        let infra_file = dir.join(".pmp.infrastructure.yaml");
        if infra_file.exists() {
            // Try to read the infrastructure name
            if let Ok(content) = std::fs::read_to_string(&infra_file) {
                if let Ok(infra) = serde_yaml::from_str::<InfrastructureResource>(&content) {
                    results.push((dir.to_path_buf(), infra.metadata.name));
                    // Don't recurse into subdirectories of an infrastructure
                    return Ok(());
                }
            }
        }

        // Recurse into subdirectories
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_dir() {
                        let path = entry.path();
                        // Skip common directories that shouldn't contain infrastructures
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            if !name.starts_with('.') && name != "node_modules" && name != "target" {
                                Self::find_infrastructure_files_recursive(&path, results)?;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
