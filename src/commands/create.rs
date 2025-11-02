use crate::collection::CollectionDiscovery;
use crate::output;
use crate::schema::SchemaValidator;
use crate::template::{TemplateDiscovery, TemplateRenderer, TemplatePackInfo, TemplateInfo};
use anyhow::{Context, Result};

/// Handles the 'create' command - creates projects from templates
pub struct CreateCommand;

impl CreateCommand {
    /// Execute the create command
    pub fn execute(ctx: &crate::context::Context, output_path: Option<&str>, template_packs_paths: Option<&str>) -> Result<()> {
        // Step 1: ProjectCollection is REQUIRED
        let (collection, collection_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("ProjectCollection is required. No .pmp.project-collection.yaml found in current directory or parent directories.\n\nPlease create a ProjectCollection first or navigate to an existing one.")?;

        ctx.output.section("Project Collection");
        ctx.output.key_value_highlight("Name", &collection.metadata.name);
        if let Some(desc) = &collection.metadata.description {
            ctx.output.key_value("Description", desc);
        }

        // Step 2: Get resource kinds from collection
        let allowed_resource_kinds = &collection.spec.resource_kinds;

        if allowed_resource_kinds.is_empty() {
            anyhow::bail!(
                "ProjectCollection must define resource_kinds.\n\nPlease add resource kinds to the ProjectCollection."
            );
        }

        // Step 3: Discover template packs
        // Parse flag paths (colon-separated)
        let flag_paths: Vec<String> = if let Some(paths) = template_packs_paths {
            crate::template::discovery::parse_colon_separated_paths(paths)
        } else {
            vec![]
        };

        // Parse environment variable paths (colon-separated)
        let env_paths: Vec<String> = std::env::var("PMP_TEMPLATE_PACKS_PATHS")
            .ok()
            .map(|p| crate::template::discovery::parse_colon_separated_paths(&p))
            .unwrap_or_default();

        // Combine paths: flag paths have priority over env paths
        let mut all_paths = flag_paths;
        all_paths.extend(env_paths);

        // Convert to Vec<&str> for the discovery function
        let custom_paths: Vec<&str> = all_paths.iter().map(|s| s.as_str()).collect();

        let all_template_packs = TemplateDiscovery::discover_template_packs_with_custom_paths(&*ctx.fs, &*ctx.output, &custom_paths)
            .context("Failed to discover template packs")?;

        if all_template_packs.is_empty() {
            anyhow::bail!(
                "No template packs found. Please create template packs in ~/.pmp/template-packs or .pmp/template-packs"
            );
        }

        // Step 4: Filter template packs by checking their templates against collection's allowed resource kinds
        let mut filtered_packs_with_templates: Vec<(TemplatePackInfo, Vec<(TemplateInfo, Option<crate::template::metadata::TemplateConfig>)>)> = Vec::new();

        for pack in all_template_packs {
            let templates_in_pack = TemplateDiscovery::discover_templates_in_pack(&*ctx.fs, &*ctx.output, &pack.path)
                .context("Failed to discover templates in pack")?;

            let pack_name = &pack.resource.metadata.name;

            // Filter templates that match allowed resource kinds and template-specific configuration
            let matching_templates: Vec<(TemplateInfo, Option<crate::template::metadata::TemplateConfig>)> = templates_in_pack
                .into_iter()
                .filter_map(|t| {
                    // Find a matching resource kind filter
                    for filter in allowed_resource_kinds.iter() {
                        if filter.matches_template(&t.resource.spec) {
                            // Check template-specific configuration
                            let template_name = &t.resource.metadata.name;
                            match filter.get_template_config(template_name, pack_name) {
                                Some(Some(config)) => {
                                    // Template is explicitly configured and allowed
                                    return Some((t, Some(config.clone())));
                                }
                                Some(None) => {
                                    // Template is explicitly not allowed
                                    return None;
                                }
                                None => {
                                    // No template-specific config, allow by default
                                    return Some((t, None));
                                }
                            }
                        }
                    }
                    None // No matching resource kind filter
                })
                .collect();

            // Only include packs that have at least one matching template
            if !matching_templates.is_empty() {
                filtered_packs_with_templates.push((pack, matching_templates));
            }
        }

        if filtered_packs_with_templates.is_empty() {
            anyhow::bail!(
                "No template packs contain templates that match the resource kinds allowed in this collection.\n\nAllowed resource kinds: {}",
                allowed_resource_kinds.iter()
                    .map(|r| format!("{}/{}", r.api_version, r.kind))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        ctx.output.blank();
        ctx.output.info(&format!("Found {} compatible template pack(s)", filtered_packs_with_templates.len()));

        // Step 5: Select template pack
        let (selected_pack, available_templates) = if filtered_packs_with_templates.len() == 1 {
            // Only one pack, use it automatically
            let (pack, templates) = filtered_packs_with_templates.into_iter().next().unwrap();
            ctx.output.subsection("Template Pack");
            ctx.output.key_value_highlight("Pack", &pack.resource.metadata.name);
            if let Some(desc) = &pack.resource.metadata.description {
                ctx.output.key_value("Description", desc);
            }
            (pack, templates)
        } else {
            // Multiple packs, let user choose
            // Sort packs by name for consistent display
            filtered_packs_with_templates.sort_by(|a, b| {
                a.0.resource.metadata.name.cmp(&b.0.resource.metadata.name)
            });

            let pack_options: Vec<String> = filtered_packs_with_templates
                .iter()
                .map(|(pack, _templates)| {
                    let desc = pack.resource.metadata.description.as_deref().unwrap_or("");
                    if desc.is_empty() {
                        pack.resource.metadata.name.clone()
                    } else {
                        format!("{} - {}", pack.resource.metadata.name, desc)
                    }
                })
                .collect();

            let selected_pack_display = ctx.input.select("Select a template pack:", pack_options.clone())
                .context("Failed to select template pack")?;

            let pack_index = pack_options
                .iter()
                .position(|opt| opt == &selected_pack_display)
                .context("Template pack not found")?;

            let (pack, templates) = filtered_packs_with_templates.into_iter().nth(pack_index).unwrap();

            ctx.output.subsection("Selected Template Pack");
            ctx.output.key_value_highlight("Pack", &pack.resource.metadata.name);
            if let Some(desc) = &pack.resource.metadata.description {
                ctx.output.key_value("Description", desc);
            }

            (pack, templates)
        };

        // Step 6: Select template from pack (auto-select if only 1)
        let (selected_template, template_config) = if available_templates.len() == 1 {
            // Only one template, use it automatically
            let (template, config) = available_templates.into_iter().next().unwrap();
            ctx.output.subsection("Template");
            ctx.output.key_value_highlight("Template", &template.resource.metadata.name);
            if let Some(desc) = &template.resource.metadata.description {
                ctx.output.key_value("Description", desc);
            }
            (template, config)
        } else {
            // Multiple templates, let user choose
            ctx.output.subsection("Select a template");

            // Sort templates by name for consistent display
            let mut sorted_templates = available_templates;
            sorted_templates.sort_by(|a, b| {
                a.0.resource.metadata.name.cmp(&b.0.resource.metadata.name)
            });

            let template_options: Vec<String> = sorted_templates
                .iter()
                .map(|(t, _config)| {
                    let desc = t.resource.metadata.description.as_deref().unwrap_or("");
                    if desc.is_empty() {
                        t.resource.metadata.name.clone()
                    } else {
                        format!("{} - {}", t.resource.metadata.name, desc)
                    }
                })
                .collect();

            let selected_template_display = ctx.input.select("Template:", template_options.clone())
                .context("Failed to select template")?;

            let template_index = template_options
                .iter()
                .position(|opt| opt == &selected_template_display)
                .context("Template not found")?;

            let (template, config) = sorted_templates.into_iter().nth(template_index).unwrap();

            ctx.output.subsection("Selected Template");
            ctx.output.key_value_highlight("Template", &template.resource.metadata.name);
            if let Some(desc) = &template.resource.metadata.description {
                ctx.output.key_value("Description", desc);
            }

            (template, config)
        };

        // Step 7: Select environment from ProjectCollection
        let selected_environment = if collection.spec.environments.is_empty() {
            anyhow::bail!("ProjectCollection must define at least one environment");
        } else if collection.spec.environments.len() == 1 {
            // Only one environment, use it automatically
            let (env_key, env) = collection.spec.environments.iter().next().unwrap();
            ctx.output.subsection("Environment");
            ctx.output.environment_badge(&env.name);
            if let Some(desc) = &env.description {
                ctx.output.key_value("Description", desc);
            }
            env_key.clone()
        } else {
            // Multiple environments, let user choose
            ctx.output.subsection("Select an environment");

            // Sort environments by name for consistent display
            let mut sorted_envs: Vec<_> = collection.spec.environments.iter().collect();
            sorted_envs.sort_by(|a, b| a.1.name.cmp(&b.1.name));

            let env_options: Vec<String> = sorted_envs.iter().map(|(_, env)| {
                    if let Some(desc) = &env.description {
                        format!("{} - {}", env.name, desc)
                    } else {
                        env.name.clone()
                    }
                })
                .collect();

            let selected_env_display = ctx.input.select("Environment:", env_options.clone())
                .context("Failed to select environment")?;

            // Find the key for the selected environment
            let env_index = env_options.iter().position(|opt| opt == &selected_env_display)
                .context("Environment not found")?;

            sorted_envs
                .get(env_index)
                .map(|(key, _)| (*key).clone())
                .context("Environment key not found")?
        };

        // Step 8: Validate resource kind
        // Validate resource kind contains only alphanumeric characters
        let resource_kind = &selected_template.resource.spec.kind;
        if !resource_kind.chars().all(|c| c.is_alphanumeric()) {
            anyhow::bail!(
                "Resource kind '{}' must contain only alphanumeric characters (found invalid characters)",
                resource_kind
            );
        }

        // Step 9: Prompt for project name
        ctx.output.subsection("Project Configuration");
        let mut project_name = SchemaValidator::prompt_for_project_name(ctx)
            .context("Failed to get project name")?;

        // Validate the project name doesn't already exist anywhere in the collection
        loop {
            let check_path = if let Some(path) = output_path {
                std::path::PathBuf::from(path)
            } else {
                collection_root.join("projects").join(&project_name)
            };

            // Check if path exists OR if project name already exists in collection
            let name_exists = if check_path.exists() {
                true
            } else {
                // Check if any existing project has this name
                match CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &collection_root) {
                    Ok(projects) => projects.iter().any(|p| p.name == project_name),
                    Err(_) => false, // If discovery fails, just check path existence
                }
            };

            if name_exists {
                ctx.output.blank();
                ctx.output.warning(&format!("A project named '{}' already exists in this collection.", project_name));
                ctx.output.dimmed("Project names must be unique across the entire collection.");
                ctx.output.dimmed("Please choose a different name:");
                project_name = SchemaValidator::prompt_for_project_name(ctx)
                    .context("Failed to get project name")?;
            } else {
                break;
            }
        }

        // Step 10: Collect inputs based on template's input definitions
        ctx.output.subsection("Template Inputs");
        ctx.output.dimmed("Please provide the following information:");

        // Start with base inputs from template spec
        let mut merged_inputs = selected_template.resource.spec.inputs.clone();

        // Override with environment-specific inputs if they exist
        if let Some(env_overrides) = selected_template.resource.spec.environments.get(&selected_environment) {
            for (input_name, input_spec) in &env_overrides.overrides.inputs {
                merged_inputs.insert(input_name.clone(), input_spec.clone());
            }
        }

        // Apply collection-level overrides from template config (if any)
        // Precedence: Template base → Environment overrides → Collection overrides → User input
        let collection_overrides = if let Some(ref config) = template_config {
            Some(&config.defaults.inputs)
        } else {
            None
        };

        // Collect inputs from user (respecting collection overrides)
        let mut inputs = Self::collect_template_inputs_with_overrides(ctx, &merged_inputs, &project_name, collection_overrides)
            .context("Failed to collect inputs")?;

        // Step 11: Add internal fields for template rendering
        inputs.insert(
            "environment".to_string(),
            serde_json::Value::String(selected_environment.clone()),
        );
        inputs.insert(
            "resource_api_version".to_string(),
            serde_json::Value::String(selected_template.resource.spec.api_version.clone()),
        );
        inputs.insert(
            "resource_kind".to_string(),
            serde_json::Value::String(selected_template.resource.spec.kind.clone()),
        );

        // Step 12: Determine project root path
        let project_root = if let Some(path) = output_path {
            std::path::PathBuf::from(path)
        } else {
            collection_root.join("projects").join(&project_name)
        };

        // Step 13: Determine environment path
        let environment_path = project_root.join("environments").join(&selected_environment);

        // Step 14: Create the directories
        if !project_root.exists() {
            std::fs::create_dir_all(&project_root)
                .context(format!("Failed to create project root directory: {}", project_root.display()))?;
        }
        if !environment_path.exists() {
            std::fs::create_dir_all(&environment_path)
                .context(format!("Failed to create environment directory: {}", environment_path.display()))?;
        }

        // Step 15: Render template into environment directory
        ctx.output.subsection("Generating Project Files");
        ctx.output.dimmed("Rendering template...");
        let renderer = TemplateRenderer::new();
        let template_src = &selected_template.path;

        if !template_src.exists() {
            anyhow::bail!(
                "Template directory not found: {}",
                template_src.display()
            );
        }

        let _generated_files = renderer
            .render_template(ctx, template_src, environment_path.as_path(), &inputs, None)
            .context("Failed to render template")?;

        // Step 15.5: Generate common file (e.g., _common.tf) if executor config is present
        if let Some(executor_config) = &collection.spec.executor {
            if !executor_config.config.is_empty() {
                // Create executor instance (for now, create directly; will use registry in Phase 3)
                let executor: Box<dyn crate::executor::Executor> = match executor_config.name.as_str() {
                    "opentofu" => Box::new(crate::executor::OpenTofuExecutor::new()),
                    _ => anyhow::bail!("Unknown executor: {}", executor_config.name),
                };

                if executor.supports_backend() {
                    ctx.output.dimmed(&format!("  Generating common file with backend configuration..."));
                    let metadata = crate::executor::ProjectMetadata {
                        api_version: &selected_template.resource.spec.api_version,
                        kind: &selected_template.resource.spec.kind,
                        environment: &selected_environment,
                        project_name: &project_name,
                    };
                    executor.generate_common_file(
                        &environment_path,
                        &executor_config.config,
                        &metadata,
                        None, // No plugins on initial project creation
                    ).context("Failed to generate common file")?;
                }
            }
        }

        // Step 16: Auto-generate .pmp.project.yaml file (identifier only)
        ctx.output.dimmed("  Generating .pmp.project.yaml...");
        Self::generate_project_identifier_yaml(
            ctx,
            &project_root,
            &project_name,
            inputs.get("description").and_then(|v| v.as_str()),
        ).context("Failed to generate .pmp.project.yaml file")?;

        // Step 17: Auto-generate .pmp.environment.yaml file (with spec)
        ctx.output.dimmed("  Generating .pmp.environment.yaml...");
        Self::generate_project_environment_yaml(
            ctx,
            &environment_path,
            &selected_environment,
            &project_name,
            &selected_template.resource,
            &inputs,
            &selected_pack.resource.metadata.name,
            &selected_template.resource.metadata.name,
        ).context("Failed to generate .pmp.environment.yaml file")?;

        ctx.output.blank();
        ctx.output.success("Project created successfully!");

        ctx.output.subsection("Project Details");
        ctx.output.key_value("Collection", &collection.metadata.name);
        ctx.output.key_value_highlight("Name", &project_name);
        ctx.output.key_value("Kind", &selected_template.resource.spec.kind);
        ctx.output.environment_badge(&selected_environment);
        ctx.output.key_value("Project root", &project_root.display().to_string());
        ctx.output.key_value("Environment path", &environment_path.display().to_string());

        let next_steps_list = vec![
            format!("Review the generated files in {}", environment_path.display()),
            "Run 'pmp preview' to see what will be created".to_string(),
            "Run 'pmp apply' to apply the infrastructure".to_string(),
        ];
        output::next_steps(&next_steps_list);

        Ok(())
    }

    /// Collect inputs from user based on template input specifications
    /// Collect template inputs with collection-level overrides
    fn collect_template_inputs_with_overrides(
        ctx: &crate::context::Context,
        inputs_spec: &std::collections::HashMap<String, crate::template::metadata::InputSpec>,
        project_name: &str,
        collection_overrides: Option<&std::collections::HashMap<String, crate::template::metadata::InputOverride>>,
    ) -> Result<std::collections::HashMap<String, serde_json::Value>> {
        let mut inputs = std::collections::HashMap::new();

        // Always add name
        inputs.insert("name".to_string(), serde_json::Value::String(project_name.to_string()));

        // Collect each input defined in the template
        for (input_name, input_spec) in inputs_spec {
            // Check if there's a collection-level override for this input
            let override_config = collection_overrides.and_then(|overrides| overrides.get(input_name));

            let value = if let Some(override_cfg) = override_config {
                if !override_cfg.show_as_default {
                    // Use the override value directly without prompting the user
                    override_cfg.value.clone()
                } else {
                    // Show the override value as the default and let user override
                    Self::prompt_for_input_with_default(ctx, input_name, input_spec, Some(&override_cfg.value))?
                }
            } else {
                // No collection override, use normal flow
                Self::prompt_for_input_with_default(ctx, input_name, input_spec, None)?
            };

            inputs.insert(input_name.clone(), value);
        }

        Ok(inputs)
    }

    /// Prompt for a single input, optionally with a collection-level default override
    fn prompt_for_input_with_default(
        ctx: &crate::context::Context,
        input_name: &str,
        input_spec: &crate::template::metadata::InputSpec,
        collection_default: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let description = input_spec.description.as_deref().unwrap_or(input_name);

        // Use collection default if provided, otherwise use template default
        let effective_default = collection_default.or(input_spec.default.as_ref());

        if let Some(enum_values) = &input_spec.enum_values {
            // This is a select input
            let mut sorted_enum_values = enum_values.clone();
            sorted_enum_values.sort();

            let default_str = effective_default
                .and_then(|v| v.as_str())
                .or_else(|| sorted_enum_values.first().map(|s| s.as_str()));

            let selected = if let Some(default) = default_str {
                let starting_cursor = sorted_enum_values.iter().position(|v| v == default).unwrap_or(0);
                let _ = starting_cursor; // Suppress unused warning
                ctx.input.select(description, sorted_enum_values.clone())
                    .context("Failed to get input")?
            } else {
                ctx.input.select(description, sorted_enum_values)
                    .context("Failed to get input")?
            };

            Ok(serde_json::Value::String(selected))
        } else if let Some(default) = effective_default {
            // Determine type from default value
            match default {
                serde_json::Value::Bool(b) => {
                    let answer = ctx.input.confirm(description, *b)
                        .context("Failed to get input")?;
                    Ok(serde_json::Value::Bool(answer))
                }
                serde_json::Value::Number(n) => {
                    let prompt_text = format!("{} (default: {})", description, n);
                    let answer = ctx.input.text(&prompt_text, Some(&n.to_string()))
                        .context("Failed to get input")?;

                    // Try to parse as number
                    if let Ok(num) = answer.parse::<i64>() {
                        Ok(serde_json::Value::Number(num.into()))
                    } else if let Ok(num) = answer.parse::<f64>() {
                        Ok(serde_json::Value::Number(serde_json::Number::from_f64(num).unwrap()))
                    } else {
                        Ok(serde_json::Value::String(answer))
                    }
                }
                serde_json::Value::String(s) => {
                    let prompt_text = format!("{} (default: {})", description, s);
                    let answer = ctx.input.text(&prompt_text, Some(s))
                        .context("Failed to get input")?;
                    Ok(serde_json::Value::String(answer))
                }
                _ => {
                    // Fallback to string input
                    let answer = ctx.input.text(description, None)
                        .context("Failed to get input")?;
                    Ok(serde_json::Value::String(answer))
                }
            }
        } else {
            // No default, prompt for string
            let answer = ctx.input.text(description, None)
                .context("Failed to get input")?;
            Ok(serde_json::Value::String(answer))
        }
    }

    /// Generate the .pmp.project.yaml file for the project (identifier only, no spec)
    fn generate_project_identifier_yaml(
        ctx: &crate::context::Context,
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

        ctx.output.dimmed(&format!("  Created: {}", pmp_yaml_path.display()));

        Ok(())
    }

    /// Generate the .pmp.environment.yaml file for the project environment (with spec)
    fn generate_project_environment_yaml(
        ctx: &crate::context::Context,
        environment_path: &std::path::Path,
        environment_name: &str,
        project_name: &str,
        template: &crate::template::metadata::TemplateResource,
        inputs: &std::collections::HashMap<String, serde_json::Value>,
        template_pack_name: &str,
        template_name: &str,
    ) -> Result<()> {
        use crate::template::metadata::{
            DynamicProjectEnvironmentResource, DynamicProjectEnvironmentMetadata, ProjectSpec, ResourceDefinition,
            TemplateReference, EnvironmentReference,
        };

        // Create DynamicProjectEnvironmentResource structure with apiVersion/kind from template
        let project_env = DynamicProjectEnvironmentResource {
            api_version: template.spec.api_version.clone(),
            kind: template.spec.kind.clone(),
            metadata: DynamicProjectEnvironmentMetadata {
                name: project_name.to_string(),
                environment_name: environment_name.to_string(),
                description: inputs.get("description")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            },
            spec: ProjectSpec {
                resource: ResourceDefinition {
                    api_version: template.spec.api_version.clone(),
                    kind: template.spec.kind.clone(),
                },
                executor: crate::template::metadata::ExecutorProjectConfig {
                    name: template.spec.executor.clone(),
                },
                inputs: inputs.clone(),
                custom: None,  // Templates no longer have custom field
                plugins: None,  // No plugins added yet
                template: Some(TemplateReference {
                    template_pack_name: template_pack_name.to_string(),
                    name: template_name.to_string(),
                }),
                environment: Some(EnvironmentReference {
                    name: environment_name.to_string(),
                }),
            },
        };

        // Serialize to YAML
        let yaml_content = serde_yaml::to_string(&project_env)
            .context("Failed to serialize project environment to YAML")?;

        // Write to .pmp.environment.yaml file
        let pmp_env_yaml_path = environment_path.join(".pmp.environment.yaml");
        std::fs::write(&pmp_env_yaml_path, yaml_content)
            .with_context(|| format!("Failed to write .pmp.environment.yaml file: {:?}", pmp_env_yaml_path))?;

        ctx.output.dimmed(&format!("  Created: {}", pmp_env_yaml_path.display()));

        Ok(())
    }

}
