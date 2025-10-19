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
        // Step 1: ProjectCollection is REQUIRED
        let (collection, collection_root) = CollectionDiscovery::find_collection()?
            .context("ProjectCollection is required. No .pmp.project-collection.yaml found in current directory or parent directories.\n\nPlease create a ProjectCollection first or navigate to an existing one.")?;

        println!("Using ProjectCollection: {}", collection.metadata.name);
        if let Some(desc) = &collection.metadata.description {
            println!("Description: {}", desc);
        }

        // Step 2: Select category first (categories define which resource kinds are allowed)
        let selected_category = if let Some(categories) = &collection.spec.categories {
            println!("\nPlease select a category:");
            CategorySelector::select_category(categories)
                .context("Failed to select category")?
        } else {
            anyhow::bail!("ProjectCollection must define categories with resource kinds");
        };

        // Step 3: Get the category to find allowed resource kinds
        let category_resource_kinds = Self::get_category_resource_kinds(
            &collection.spec.categories,
            &selected_category
        )?;

        if category_resource_kinds.is_empty() {
            anyhow::bail!(
                "Category '{}' has no resource kinds defined.\n\nPlease add resource kinds to this category in the ProjectCollection.",
                selected_category
            );
        }

        // Step 4: Discover templates
        let custom_paths = if let Some(path) = templates_path {
            vec![path]
        } else {
            vec![]
        };

        let all_templates = TemplateDiscovery::discover_templates_with_custom_paths(&custom_paths)
            .context("Failed to discover templates")?;

        if all_templates.is_empty() {
            anyhow::bail!(
                "No templates found. Please create templates in ~/.pmp/templates or .pmp/templates"
            );
        }

        // Step 5: Filter templates by category's allowed resource kinds
        let filtered_templates: Vec<_> = all_templates
            .iter()
            .filter(|t| {
                category_resource_kinds.iter().any(|filter| {
                    filter.matches(&t.resource.spec.resource)
                })
            })
            .collect();

        if filtered_templates.is_empty() {
            anyhow::bail!(
                "No templates match the resource kinds allowed in category '{}'.\n\nAllowed resource kinds: {}",
                selected_category,
                category_resource_kinds.iter()
                    .map(|r| format!("{}/{}", r.api_version, r.kind))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        println!("\nFound {} compatible template(s) for category '{}'", filtered_templates.len(), selected_category);

        // Step 6: Select template
        let template_options: Vec<String> = filtered_templates
            .iter()
            .map(|t| {
                let desc = t.resource.metadata.description.as_deref().unwrap_or("");
                if desc.is_empty() {
                    t.resource.metadata.name.clone()
                } else {
                    format!("{} - {}", t.resource.metadata.name, desc)
                }
            })
            .collect();

        let selected_template_name = Select::new("Select a template:", template_options.clone())
            .prompt()
            .context("Failed to select template")?;

        let template_index = template_options
            .iter()
            .position(|opt| opt.starts_with(&selected_template_name) || opt == &selected_template_name)
            .context("Template not found")?;

        let selected_template = filtered_templates[template_index];

        println!("\nUsing template: {}", selected_template.resource.metadata.name);
        if let Some(desc) = &selected_template.resource.metadata.description {
            println!("Description: {}", desc);
        }

        // Step 7: Select environment from ProjectCollection
        let selected_environment = if collection.spec.environments.is_empty() {
            anyhow::bail!("ProjectCollection must define at least one environment");
        } else if collection.spec.environments.len() == 1 {
            // Only one environment, use it automatically
            let (env_key, env) = collection.spec.environments.iter().next().unwrap();
            println!("\nUsing environment: {}", env.name);
            if let Some(desc) = &env.description {
                println!("Description: {}", desc);
            }
            env_key.clone()
        } else {
            // Multiple environments, let user choose
            println!("\nSelect an environment:");
            let env_options: Vec<String> = collection.spec.environments
                .iter()
                .map(|(_key, env)| {
                    if let Some(desc) = &env.description {
                        format!("{} - {}", env.name, desc)
                    } else {
                        env.name.clone()
                    }
                })
                .collect();

            let selected_env_display = Select::new("Environment:", env_options.clone())
                .prompt()
                .context("Failed to select environment")?;

            // Find the key for the selected environment
            let env_index = env_options.iter().position(|opt| opt == &selected_env_display)
                .context("Environment not found")?;

            let env_key = collection.spec.environments
                .keys()
                .nth(env_index)
                .context("Environment key not found")?
                .clone();

            env_key
        };

        // Step 8: Validate resource kind
        // Validate resource kind contains only alphanumeric characters
        let resource_kind = &selected_template.resource.spec.resource.kind;
        if !resource_kind.chars().all(|c| c.is_alphanumeric()) {
            anyhow::bail!(
                "Resource kind '{}' must contain only alphanumeric characters (found invalid characters)",
                resource_kind
            );
        }

        // Convert resource kind from CamelCase to snake_case for directory name
        let resource_kind_snake = Self::camel_to_snake_case(resource_kind);

        // Step 9: Prompt for project name
        println!("\nPlease provide the project name:");
        let mut project_name = SchemaValidator::prompt_for_project_name()
            .context("Failed to get project name")?;

        // Validate the project name doesn't already exist
        loop {
            let final_path = if let Some(path) = output_path {
                std::path::PathBuf::from(path)
            } else if collection.spec.organize_by_category {
                collection_root.join("projects").join(&selected_category).join(&resource_kind_snake).join(&project_name)
            } else {
                collection_root.join("projects").join(&resource_kind_snake).join(&project_name)
            };

            if final_path.exists() {
                println!("\n⚠ Warning: A project with this name already exists at: {}", final_path.display());
                println!("Please choose a different name:");
                project_name = SchemaValidator::prompt_for_project_name()
                    .context("Failed to get project name")?;
            } else {
                break;
            }
        }

        // Step 10: Validate category is a leaf (if collection has defined categories)
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

        // Step 11: Collect inputs based on template's input definitions
        println!("\nPlease provide the following information:");
        let mut inputs = SchemaValidator::collect_inputs(
            &selected_template.resource.spec.inputs,
            project_name.clone()
        ).context("Failed to collect inputs")?;

        // Step 12: Add internal fields for template rendering
        inputs.insert(
            "environment".to_string(),
            serde_json::Value::String(selected_environment),
        );
        inputs.insert(
            "resource_api_version".to_string(),
            serde_json::Value::String(selected_template.resource.spec.resource.api_version.clone()),
        );
        inputs.insert(
            "resource_kind".to_string(),
            serde_json::Value::String(selected_template.resource.spec.resource.kind.clone()),
        );

        // Step 13: Determine final output path
        let final_path = if let Some(path) = output_path {
            std::path::PathBuf::from(path)
        } else if collection.spec.organize_by_category {
            collection_root.join("projects").join(&selected_category).join(&resource_kind_snake).join(&project_name)
        } else {
            collection_root.join("projects").join(&resource_kind_snake).join(&project_name)
        };

        // Step 14: Create the output directory
        if !final_path.exists() {
            std::fs::create_dir_all(&final_path)
                .context(format!("Failed to create output directory: {}", final_path.display()))?;
        }

        // Step 15: Render template
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
            .render_template(&template_src, final_path.as_path(), &inputs)
            .context("Failed to render template")?;

        println!("\n✓ Project created successfully in: {}", final_path.display());
        println!("\n✓ Project created in collection '{}' and will be automatically discovered", collection.metadata.name);
        println!("  Name: {}", &project_name);
        println!("  Kind: {}", selected_template.resource.spec.resource.kind);
        if collection.spec.organize_by_category {
            println!("  Category: {}", selected_category);
        }

        println!("\nNext steps:");
        println!("  1. Review the generated files");
        println!("  2. Run 'pmp preview' to see what will be created");
        println!("  3. Run 'pmp apply' to apply the infrastructure");

        Ok(())
    }

    /// Get resource kinds allowed in a category (including from parent categories)
    fn get_category_resource_kinds(
        categories: &Option<std::collections::HashMap<String, crate::template::metadata::Category>>,
        category_path: &str,
    ) -> Result<Vec<crate::template::metadata::ResourceKindFilter>> {
        let categories = categories.as_ref()
            .context("No categories defined")?;

        // Split the path into components
        let path_parts: Vec<&str> = category_path.split('/').collect();

        let mut current_categories = categories;
        let mut resource_kinds = Vec::new();

        // Walk through the path
        for part in path_parts {
            if let Some(cat) = current_categories.get(part) {
                // Add this category's resource kinds
                resource_kinds.extend(cat.resource_kinds.clone());

                // Move to children if they exist
                if let Some(children) = &cat.children {
                    current_categories = children;
                } else {
                    break;
                }
            } else {
                anyhow::bail!("Category '{}' not found in path '{}'", part, category_path);
            }
        }

        Ok(resource_kinds)
    }

    /// Convert CamelCase to snake_case
    fn camel_to_snake_case(s: &str) -> String {
        let mut result = String::new();
        let mut prev_is_upper = false;

        for (i, ch) in s.chars().enumerate() {
            if ch.is_uppercase() {
                if i > 0 && !prev_is_upper {
                    result.push('_');
                }
                result.push(ch.to_ascii_lowercase());
                prev_is_upper = true;
            } else {
                result.push(ch);
                prev_is_upper = false;
            }
        }

        result
    }
}
