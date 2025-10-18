use crate::collection::CollectionDiscovery;
use crate::schema::SchemaValidator;
use crate::template::metadata::ProjectReference;
use crate::template::{TemplateDiscovery, TemplateRenderer};
use anyhow::{Context, Result};
use inquire::Select;

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
        let output_dir_path = if let Some(path) = output_path {
            std::path::PathBuf::from(path)
        } else {
            // Check if we're in a ProjectCollection that organizes by category
            if let Ok(Some((collection, collection_root))) = CollectionDiscovery::find_collection() {
                if collection.spec.organize_by_category {
                    // Get the project name from schema to create a subdirectory
                    let project_name = inputs
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unnamed");

                    // Organize by category: collection_root/category/project_name
                    collection_root.join(&selected_category).join(project_name)
                } else {
                    // Default to current directory
                    std::path::PathBuf::from(".")
                }
            } else {
                // Default to current directory
                std::path::PathBuf::from(".")
            }
        };

        // Create the output directory if it doesn't exist
        if !output_dir_path.exists() {
            std::fs::create_dir_all(&output_dir_path)
                .context(format!("Failed to create output directory: {}", output_dir_path.display()))?;
        }

        let output_dir = output_dir_path.as_path();

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

        // Step 7: Check if we're in a ProjectCollection and register the project
        if let Ok(Some((mut collection, collection_root))) = CollectionDiscovery::find_collection() {
            // Get the project name from inputs
            let project_name = inputs
                .get("name")
                .and_then(|v| v.as_str())
                .context("Project name not found in inputs")?
                .to_string();

            let project_kind = selected_template.resource.spec.resource.kind.clone();

            // Check for duplicates
            if collection.has_project(&project_name, &project_kind) {
                println!(
                    "\n⚠ Warning: A project with name '{}' and kind '{}' already exists in the collection",
                    project_name, project_kind
                );
                println!("Skipping automatic registration.");
            } else {
                // Calculate the relative path from collection root to the project
                let output_path_abs = output_dir.canonicalize()
                    .context("Failed to get absolute path for output directory")?;
                let collection_root_abs = collection_root.canonicalize()
                    .context("Failed to get absolute path for collection root")?;

                let relative_path = output_path_abs
                    .strip_prefix(&collection_root_abs)
                    .context("Project is not inside the collection directory")?;

                let relative_path_str = relative_path
                    .to_str()
                    .context("Failed to convert path to string")?
                    .replace('\\', "/"); // Normalize to forward slashes

                // Get the category from the selected category
                let category = Some(selected_category.clone());

                // Create project reference
                let project_ref = ProjectReference {
                    name: project_name.clone(),
                    kind: project_kind.clone(),
                    path: relative_path_str.clone(),
                    category,
                };

                // Add to collection
                collection.add_project(project_ref);

                // Save the collection
                let collection_path = collection_root.join(".pmp.yaml");
                collection
                    .save(&collection_path)
                    .context("Failed to save project collection")?;

                println!(
                    "\n✓ Project registered in collection '{}' as '{}/{}'",
                    collection.metadata.name, project_kind, project_name
                );
            }
        }

        println!("\nNext steps:");
        println!("  1. Review the generated files");
        println!("  2. Run 'pmp preview' to see what will be created");
        println!("  3. Run 'pmp apply' to apply the infrastructure");

        Ok(())
    }
}
