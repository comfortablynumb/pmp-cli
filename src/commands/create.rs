use crate::schema::SchemaValidator;
use crate::template::{TemplateDiscovery, TemplateRenderer};
use anyhow::{Context, Result};
use inquire::Select;
use std::path::Path;

/// Handles the 'create' command - creates projects from templates
pub struct CreateCommand;

impl CreateCommand {
    /// Execute the create command
    pub fn execute(output_path: Option<&str>) -> Result<()> {
        // Discover all templates
        let templates = TemplateDiscovery::discover_templates()
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
            .map(|t| format!("{} - {}", t.metadata.name, t.metadata.description))
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

        println!("\nUsing template: {}", selected_template.metadata.name);
        println!("Description: {}", selected_template.metadata.description);

        // Step 3: Collect and validate inputs based on schema
        let schema_path = selected_template.schema_path();

        if !schema_path.exists() {
            anyhow::bail!(
                "Schema file not found: {}",
                schema_path.display()
            );
        }

        println!("\nPlease provide the following information:");
        let inputs = SchemaValidator::collect_and_validate_inputs(&schema_path)
            .context("Failed to collect inputs")?;

        // Step 4: Determine output directory
        let output_dir = if let Some(path) = output_path {
            Path::new(path)
        } else {
            Path::new(".")
        };

        // Step 5: Render template
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
