use crate::collection::{CategorySelector, CollectionDiscovery};
use crate::schema::SchemaValidator;
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

        // Check if we're in a ProjectCollection with defined categories
        let collection_info = CollectionDiscovery::find_collection()?;
        let selected_category = if let Some((collection, _)) = &collection_info {
            // If collection has defined categories, use hierarchical selection
            if let Some(categories) = &collection.spec.categories {
                println!("\nThis collection has defined categories. Please select one:");
                CategorySelector::select_category(categories)
                    .context("Failed to select category")?
            } else {
                // Collection exists but no categories defined, use template categories
                Self::select_template_category(&templates)?
            }
        } else {
            // Not in a collection, use template categories
            Self::select_template_category(&templates)?
        };

        // Step 2: Select template from category
        // Group templates by category
        let grouped = TemplateDiscovery::group_by_category(&templates);
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

        // Step 3: Prompt for project name (after category, before other inputs)
        println!("\nPlease provide the project name:");
        let mut project_name = SchemaValidator::prompt_for_project_name()
            .context("Failed to get project name")?;

        // Validate the project name against the final path that will be used
        loop {
            let final_path = if let Some(path) = output_path {
                std::path::PathBuf::from(path)
            } else if let Some((collection, collection_root)) = &collection_info {
                if collection.spec.organize_by_category {
                    // Organize by category: collection_root/projects/category/project_name
                    collection_root.join("projects").join(&selected_category).join(&project_name)
                } else {
                    // No category organization: collection_root/projects/project_name
                    collection_root.join("projects").join(&project_name)
                }
            } else {
                // Default to current directory
                std::path::PathBuf::from(".")
            };

            // Check if the final path already exists
            if final_path.exists() {
                println!("\n⚠ Warning: A project with this name already exists at: {}", final_path.display());
                println!("Please choose a different name:");
                project_name = SchemaValidator::prompt_for_project_name()
                    .context("Failed to get project name")?;
            } else {
                break;
            }
        }

        // Step 4: Select environment if available
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

        // Step 5: Collect and validate other inputs based on schema
        let schema_path = selected_template.schema_path();

        if !schema_path.exists() {
            anyhow::bail!(
                "Schema file not found: {}",
                schema_path.display()
            );
        }

        println!("\nPlease provide the following information:");
        let mut inputs = if let Some((_, env_config)) = &selected_env_config {
            SchemaValidator::collect_and_validate_inputs_with_env(&schema_path, Some(env_config), Some(project_name.clone()))
                .context("Failed to collect inputs")?
        } else {
            SchemaValidator::collect_and_validate_inputs_with_env(&schema_path, None, Some(project_name.clone()))
                .context("Failed to collect inputs")?
        };

        // Step 5.5: Validate category is a leaf (if in a collection with defined categories)
        if let Some((collection, _)) = &collection_info {
            // Validate that the selected category is a leaf category (if collection has categories defined)
            if let Some(categories) = &collection.spec.categories {
                let all_leaf_paths = CategorySelector::get_all_leaf_paths(categories);
                if !all_leaf_paths.contains(&selected_category) {
                    anyhow::bail!(
                        "Category '{}' is not a leaf category. Projects can only be assigned to leaf categories.\nAvailable leaf categories: {}",
                        selected_category,
                        all_leaf_paths.join(", ")
                    );
                }
            }
        }

        // Add environment to inputs if selected
        if let Some((env_name, _)) = selected_env_config {
            inputs.insert("pmp_environment".to_string(), serde_json::Value::String(env_name));
        }

        // Add resource apiVersion and kind for rendering the generated project's .pmp.project.yaml
        inputs.insert(
            "pmp_resource_api_version".to_string(),
            serde_json::Value::String(selected_template.resource.spec.resource.api_version.clone()),
        );
        inputs.insert(
            "pmp_resource_kind".to_string(),
            serde_json::Value::String(selected_template.resource.spec.resource.kind.clone()),
        );

        // Step 6: Determine output directory (using the validated project_name)
        let output_dir_path = if let Some(path) = output_path {
            std::path::PathBuf::from(path)
        } else {
            // Check if we're in a ProjectCollection that organizes by category
            if let Some((collection, collection_root)) = &collection_info {
                if collection.spec.organize_by_category {
                    // Organize by category: collection_root/projects/category/project_name
                    collection_root.join("projects").join(&selected_category).join(&project_name)
                } else {
                    // No category organization: collection_root/projects/project_name
                    collection_root.join("projects").join(&project_name)
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

        // Step 7: If we're in a ProjectCollection, inform the user the project will be auto-discovered
        if let Some((collection, _)) = &collection_info {
            let project_kind = selected_template.resource.spec.resource.kind.clone();

            println!(
                "\n✓ Project created in collection '{}' and will be automatically discovered",
                collection.metadata.name
            );
            println!("  Name: {}", &project_name);
            println!("  Kind: {}", project_kind);
            if collection.spec.organize_by_category {
                println!("  Category: {}", selected_category);
            }
        }

        println!("\nNext steps:");
        println!("  1. Review the generated files");
        println!("  2. Run 'pmp preview' to see what will be created");
        println!("  3. Run 'pmp apply' to apply the infrastructure");

        Ok(())
    }

    /// Helper method to select a template category (when not using collection categories)
    fn select_template_category(
        templates: &[crate::template::discovery::TemplateInfo],
    ) -> Result<String> {
        // Group templates by category
        let grouped = TemplateDiscovery::group_by_category(templates);

        if grouped.is_empty() {
            anyhow::bail!("No template categories found");
        }

        // Select category
        let mut categories: Vec<String> = grouped.keys().cloned().collect();
        categories.sort();

        let selected_category = Select::new("Select a category:", categories)
            .prompt()
            .context("Failed to select category")?;

        Ok(selected_category)
    }
}
