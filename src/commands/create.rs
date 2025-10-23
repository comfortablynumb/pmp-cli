use crate::collection::CollectionDiscovery;
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

        // Step 2: Get resource kinds from collection
        let allowed_resource_kinds = &collection.spec.resource_kinds;

        if allowed_resource_kinds.is_empty() {
            anyhow::bail!(
                "ProjectCollection must define resource_kinds.\n\nPlease add resource kinds to the ProjectCollection."
            );
        }

        // Step 3: Discover templates
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

        // Step 4: Filter templates by collection's allowed resource kinds
        let filtered_templates: Vec<_> = all_templates
            .iter()
            .filter(|t| {
                allowed_resource_kinds.iter().any(|filter| {
                    filter.matches_spec(&t.resource.spec.resource)
                })
            })
            .collect();

        if filtered_templates.is_empty() {
            anyhow::bail!(
                "No templates match the resource kinds allowed in this collection.\n\nAllowed resource kinds: {}",
                allowed_resource_kinds.iter()
                    .map(|r| format!("{}/{}", r.api_version, r.kind))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        println!("\nFound {} compatible template(s)", filtered_templates.len());

        // Step 5: Select template
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

        // Step 6: Select environment from ProjectCollection
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
            let env_options: Vec<String> = collection.spec.environments.values().map(|env| {
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

            collection.spec.environments
                .keys()
                .nth(env_index)
                .context("Environment key not found")?
                .clone()
        };

        // Step 7: Validate resource kind
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

        // Step 8: Prompt for project name
        println!("\nPlease provide the project name:");
        let mut project_name = SchemaValidator::prompt_for_project_name()
            .context("Failed to get project name")?;

        // Validate the project name doesn't already exist
        loop {
            let check_path = if let Some(path) = output_path {
                std::path::PathBuf::from(path)
            } else {
                collection_root.join("projects").join(&resource_kind_snake).join(&project_name)
            };

            if check_path.exists() {
                println!("\n⚠ Warning: A project with this name already exists at: {}", check_path.display());
                println!("Please choose a different name:");
                project_name = SchemaValidator::prompt_for_project_name()
                    .context("Failed to get project name")?;
            } else {
                break;
            }
        }

        // Step 9: Collect inputs based on template's input definitions
        println!("\nPlease provide the following information:");

        // Start with base inputs from $.spec.resource.inputs
        let mut merged_inputs = selected_template.resource.spec.resource.inputs.clone();

        // Override with environment-specific inputs if they exist
        if let Some(env_overrides) = selected_template.resource.spec.resource.environments.get(&selected_environment) {
            for (input_name, input_spec) in &env_overrides.overrides.inputs {
                merged_inputs.insert(input_name.clone(), input_spec.clone());
            }
        }

        // Collect inputs from user
        let mut inputs = Self::collect_template_inputs(&merged_inputs, &project_name)
            .context("Failed to collect inputs")?;

        // Step 10: Add internal fields for template rendering
        inputs.insert(
            "environment".to_string(),
            serde_json::Value::String(selected_environment.clone()),
        );
        inputs.insert(
            "resource_api_version".to_string(),
            serde_json::Value::String(selected_template.resource.spec.resource.api_version.clone()),
        );
        inputs.insert(
            "resource_kind".to_string(),
            serde_json::Value::String(selected_template.resource.spec.resource.kind.clone()),
        );

        // Step 11: Determine project root path
        let project_root = if let Some(path) = output_path {
            std::path::PathBuf::from(path)
        } else {
            collection_root.join("projects").join(&resource_kind_snake).join(&project_name)
        };

        // Step 12: Determine environment path
        let environment_path = project_root.join("environments").join(&selected_environment);

        // Step 13: Create the directories
        if !project_root.exists() {
            std::fs::create_dir_all(&project_root)
                .context(format!("Failed to create project root directory: {}", project_root.display()))?;
        }
        if !environment_path.exists() {
            std::fs::create_dir_all(&environment_path)
                .context(format!("Failed to create environment directory: {}", environment_path.display()))?;
        }

        // Step 14: Render template into environment directory
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
            .render_template(&template_src, environment_path.as_path(), &inputs)
            .context("Failed to render template")?;

        // Step 15: Auto-generate .pmp.project.yaml file (identifier only)
        println!("  Generating .pmp.project.yaml...");
        Self::generate_project_identifier_yaml(
            &project_root,
            &project_name,
            inputs.get("description").and_then(|v| v.as_str()),
        ).context("Failed to generate .pmp.project.yaml file")?;

        // Step 16: Auto-generate .pmp.environment.yaml file (with spec)
        println!("  Generating .pmp.environment.yaml...");
        Self::generate_project_environment_yaml(
            &environment_path,
            &selected_environment,
            &project_name,
            &selected_template.resource,
            &inputs,
        ).context("Failed to generate .pmp.environment.yaml file")?;

        println!("\n✓ Project created successfully!");
        println!("\n✓ Project created in collection '{}'", collection.metadata.name);
        println!("  Name: {}", &project_name);
        println!("  Kind: {}", selected_template.resource.spec.resource.kind);
        println!("  Environment: {}", &selected_environment);
        println!("  Project root: {}", project_root.display());
        println!("  Environment path: {}", environment_path.display());

        println!("\nNext steps:");
        println!("  1. Review the generated files in {}", environment_path.display());
        println!("  2. Run 'pmp preview' to see what will be created");
        println!("  3. Run 'pmp apply' to apply the infrastructure");

        Ok(())
    }

    /// Collect inputs from user based on template input specifications
    fn collect_template_inputs(
        inputs_spec: &std::collections::HashMap<String, crate::template::metadata::InputSpec>,
        project_name: &str,
    ) -> Result<std::collections::HashMap<String, serde_json::Value>> {
        use inquire::{Select, Text, Confirm};

        let mut inputs = std::collections::HashMap::new();

        // Always add name
        inputs.insert("name".to_string(), serde_json::Value::String(project_name.to_string()));

        // Collect each input defined in the template
        for (input_name, input_spec) in inputs_spec {
            let description = input_spec.description.as_deref().unwrap_or(input_name.as_str());

            let value = if let Some(enum_values) = &input_spec.enum_values {
                // This is a select input
                let default_str = input_spec.default
                    .as_ref()
                    .and_then(|v| v.as_str())
                    .or_else(|| enum_values.first().map(|s| s.as_str()));

                let selected = if let Some(default) = default_str {
                    Select::new(description, enum_values.clone())
                        .with_starting_cursor(enum_values.iter().position(|v| v == default).unwrap_or(0))
                        .prompt()
                        .context("Failed to get input")?
                } else {
                    Select::new(description, enum_values.clone())
                        .prompt()
                        .context("Failed to get input")?
                };

                serde_json::Value::String(selected)
            } else if let Some(default) = &input_spec.default {
                // Determine type from default value
                match default {
                    serde_json::Value::Bool(b) => {
                        let answer = Confirm::new(description)
                            .with_default(*b)
                            .prompt()
                            .context("Failed to get input")?;
                        serde_json::Value::Bool(answer)
                    }
                    serde_json::Value::Number(n) => {
                        let prompt_text = format!("{} (default: {})", description, n);
                        let answer = Text::new(&prompt_text)
                            .with_default(&n.to_string())
                            .prompt()
                            .context("Failed to get input")?;

                        // Try to parse as number
                        if let Ok(num) = answer.parse::<i64>() {
                            serde_json::Value::Number(num.into())
                        } else if let Ok(num) = answer.parse::<f64>() {
                            serde_json::Value::Number(serde_json::Number::from_f64(num).unwrap())
                        } else {
                            serde_json::Value::String(answer)
                        }
                    }
                    serde_json::Value::String(s) => {
                        let prompt_text = format!("{} (default: {})", description, s);
                        let answer = Text::new(&prompt_text)
                            .with_default(s)
                            .prompt()
                            .context("Failed to get input")?;
                        serde_json::Value::String(answer)
                    }
                    _ => {
                        // Fallback to string input
                        let answer = Text::new(description)
                            .prompt()
                            .context("Failed to get input")?;
                        serde_json::Value::String(answer)
                    }
                }
            } else {
                // No default, prompt for string
                let answer = Text::new(description)
                    .prompt()
                    .context("Failed to get input")?;
                serde_json::Value::String(answer)
            };

            inputs.insert(input_name.clone(), value);
        }

        Ok(inputs)
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

    /// Generate the .pmp.project.yaml file for the project (identifier only, no spec)
    fn generate_project_identifier_yaml(
        project_root: &std::path::Path,
        project_name: &str,
        description: Option<&str>,
    ) -> Result<()> {
        use crate::template::metadata::{ProjectResource, ProjectMetadata};

        // Create ProjectResource structure without spec
        let project = ProjectResource {
            api_version: "pmp.io/v1".to_string(),
            kind: "Project".to_string(),
            metadata: ProjectMetadata {
                name: project_name.to_string(),
                description: description.map(|s| s.to_string()),
            },
            spec: None,
        };

        // Serialize to YAML
        let yaml_content = serde_yaml::to_string(&project)
            .context("Failed to serialize project identifier to YAML")?;

        // Write to .pmp.project.yaml file
        let pmp_yaml_path = project_root.join(".pmp.project.yaml");
        std::fs::write(&pmp_yaml_path, yaml_content)
            .with_context(|| format!("Failed to write .pmp.project.yaml file: {:?}", pmp_yaml_path))?;

        println!("  Created: {}", pmp_yaml_path.display());

        Ok(())
    }

    /// Generate the .pmp.environment.yaml file for the project environment (with spec)
    fn generate_project_environment_yaml(
        environment_path: &std::path::Path,
        environment_name: &str,
        project_name: &str,
        template: &crate::template::metadata::TemplateResource,
        inputs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        use crate::template::metadata::{
            ProjectEnvironmentResource, ProjectEnvironmentMetadata, ProjectSpec, ResourceDefinition,
        };

        // Create ProjectEnvironmentResource structure
        let project_env = ProjectEnvironmentResource {
            api_version: "pmp.io/v1".to_string(),
            kind: "ProjectEnvironment".to_string(),
            metadata: ProjectEnvironmentMetadata {
                name: environment_name.to_string(),
                project_name: project_name.to_string(),
                description: inputs.get("description")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            },
            spec: ProjectSpec {
                resource: ResourceDefinition {
                    api_version: template.spec.resource.api_version.clone(),
                    kind: template.spec.resource.kind.clone(),
                },
                executor: crate::template::metadata::ExecutorProjectConfig {
                    name: template.spec.resource.executor.clone(),
                },
                inputs: inputs.clone(),
                custom: template.spec.custom.clone(),
            },
        };

        // Serialize to YAML
        let yaml_content = serde_yaml::to_string(&project_env)
            .context("Failed to serialize project environment to YAML")?;

        // Write to .pmp.environment.yaml file
        let pmp_env_yaml_path = environment_path.join(".pmp.environment.yaml");
        std::fs::write(&pmp_env_yaml_path, yaml_content)
            .with_context(|| format!("Failed to write .pmp.environment.yaml file: {:?}", pmp_env_yaml_path))?;

        println!("  Created: {}", pmp_env_yaml_path.display());

        Ok(())
    }
}
