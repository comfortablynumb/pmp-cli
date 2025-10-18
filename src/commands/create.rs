use crate::schema::SchemaValidator;
use crate::template::{TemplateDiscovery, TemplateRenderer};
use anyhow::{Context, Result};
use inquire::Select;
use std::path::Path;

/// Handles the 'create' command - creates projects from templates
pub struct CreateCommand;

impl CreateCommand {
    /// Execute the create command
    pub fn execute(output_path: Option<&str>, templates_path: Option<&str>) -> Result<()> {
        // Discover all templates
        let custom_paths = if let Some(path) = templates_path {
            vec![path]
        } else {
            vec![]
        };

        let templates = TemplateDiscovery::discover_templates_with_custom_paths(&custom_paths)
            .context("Failed to discover templates")?;

        if templates.is_empty() {
            anyhow::bail!(
                "No templates found. Please create templates in ~/.pmp/templates or .pmp/templates"
            );
        }

        // Group templates by category
        let grouped = TemplateDiscovery::group_by_category(&templates);

        if grouped.is_empty() {
            anyhow::bail!("No template categories found");
        }

        // Step 1: Select category
        let mut categories: Vec<String> = grouped.keys().cloned().collect();
        categories.sort();

        let selected_category = Select::new("Select a category:", categories)
            .prompt()
            .context("Failed to select category")?;

        // Step 2: Select template from category
        let category_templates = grouped
            .get(&selected_category)
            .context("Category not found")?;

        let template_options: Vec<String> = category_templates
            .iter()
            .map(|t| {
                let desc = t.resource.metadata.description.as_deref().unwrap_or("");
                format!("{} - {}", t.resource.metadata.name, desc)
            })
            .collect();

        let selected_template_name = Select::new("Select a template:", template_options.clone())
            .prompt()
            .context("Failed to select template")?;

        // Find the index of the selected template
        let template_index = template_options
            .iter()
            .position(|opt| opt == &selected_template_name)
            .context("Template not found")?;

        let selected_template = category_templates[template_index];

        println!("\nUsing template: {}", selected_template.resource.metadata.name);

        if let Some(desc) = &selected_template.resource.metadata.description {
            println!("Description: {}", desc);
        }

        // Step 3: Select environment if available
        let selected_env_config = if let Some(environments) = &selected_template.resource.spec.environments {
            if !environments.is_empty() {
                let mut env_names: Vec<String> = environments.keys().cloned().collect();
                env_names.sort();

                println!("\nThis template supports multiple environments:");
                let selected_env_name = Select::new("Select an environment:", env_names)
                    .prompt()
                    .context("Failed to select environment")?;

                let env_config = environments.get(&selected_env_name)
                    .context("Environment not found")?;

                if let Some(desc) = &env_config.description {
                    println!("Environment: {} - {}", selected_env_name, desc);
                } else {
                    println!("Environment: {}", selected_env_name);
                }

                Some((selected_env_name, env_config.clone()))
            } else {
                None
            }
        } else {
            None
        };

        // Step 4: Collect and validate inputs based on schema
        let schema_path = selected_template.schema_path();

        if !schema_path.exists() {
            anyhow::bail!(
                "Schema file not found: {}",
                schema_path.display()
            );
        }

        println!("\nPlease provide the following information:");
        let mut inputs = if let Some((_, env_config)) = &selected_env_config {
            SchemaValidator::collect_and_validate_inputs_with_env(&schema_path, Some(env_config))
                .context("Failed to collect inputs")?
        } else {
            SchemaValidator::collect_and_validate_inputs(&schema_path)
                .context("Failed to collect inputs")?
        };

        // Add environment to inputs if selected
        if let Some((env_name, _)) = selected_env_config {
            inputs.insert("environment".to_string(), serde_json::Value::String(env_name));
        }

        // Add resource apiVersion and kind for rendering the generated project's .pmp.yaml
        inputs.insert(
            "resource_api_version".to_string(),
            serde_json::Value::String(selected_template.resource.spec.resource.api_version.clone()),
        );
        inputs.insert(
            "resource_kind".to_string(),
            serde_json::Value::String(selected_template.resource.spec.resource.kind.clone()),
        );

        // Step 5: Determine output directory
        let output_dir = if let Some(path) = output_path {
            Path::new(path)
        } else {
            Path::new(".")
        };

        // Step 6: Render template
        println!("\nRendering template...");
        let renderer = TemplateRenderer::new();
        let template_src = selected_template.src_path();

        if !template_src.exists() {
            anyhow::bail!(
                "Template src directory not found: {}",
                template_src.display()
            );
        }

        renderer
            .render_template(&template_src, output_dir, &inputs)
            .context("Failed to render template")?;

        println!("\n✓ Project created successfully in: {}", output_dir.display());
        println!("\nNext steps:");
        println!("  1. Review the generated files");
        println!("  2. Run 'pmp preview' to see what will be created");
        println!("  3. Run 'pmp apply' to apply the infrastructure");

        Ok(())
    }
}
