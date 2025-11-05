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
        // Step 1: Infrastructure is REQUIRED
        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. No .pmp.infrastructure.yaml found in current directory or parent directories.\n\nPlease create an Infrastructure first or navigate to an existing one.")?;

        ctx.output.section("Infrastructure");
        ctx.output.key_value_highlight("Name", &infrastructure.metadata.name);
        if let Some(desc) = &infrastructure.metadata.description {
            ctx.output.key_value("Description", desc);
        }

        // Step 2: Get resource kinds from infrastructure
        let allowed_resource_kinds = &infrastructure.spec.resource_kinds;

        if allowed_resource_kinds.is_empty() {
            anyhow::bail!(
                "Infrastructure must define resource_kinds.\n\nPlease add resource kinds to the Infrastructure."
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
                "No template packs contain templates that match the resource kinds allowed in this infrastructure.\n\nAllowed resource kinds: {}",
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

        // Step 6.5: Handle template reference projects (if template requires them)
        let mut template_reference_projects = Vec::new();

        if !selected_template.resource.spec.dependencies.is_empty() {
            ctx.output.subsection("Template Reference Projects");
            ctx.output.dimmed("This template requires reference projects to be selected.");
            output::blank();

            // Discover all projects in the collection
            let projects = CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &infrastructure_root)?;

            for (ref_index, dep) in selected_template.resource.spec.dependencies.iter().enumerate() {
                let template_ref = &dep.project;
                let ref_number = ref_index + 1;
                let total_refs = selected_template.resource.spec.dependencies.len();

                ctx.output.dimmed(&format!(
                    "Reference {} of {}: {} (Kind: {})",
                    ref_number, total_refs,
                    template_ref.remote_state.as_ref().map(|rs| rs.data_source_name.as_str()).unwrap_or("unknown"),
                    template_ref.kind
                ));
                output::blank();

                // Filter projects by required apiVersion and kind
                let mut compatible_projects = Vec::new();
                for project in &projects {
                    let project_path = infrastructure_root.join(&project.path);
                    let environments_dir = project_path.join("environments");

                    if let Ok(env_entries) = ctx.fs.read_dir(&environments_dir) {
                        for env_path in env_entries {
                            let env_file = env_path.join(".pmp.environment.yaml");
                            if ctx.fs.exists(&env_file) {
                                if let Ok(env_resource) = crate::template::metadata::DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file) {
                                    if env_resource.api_version == template_ref.api_version
                                        && env_resource.kind == template_ref.kind {
                                        compatible_projects.push((project.clone(), project_path.clone()));
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }

                if compatible_projects.is_empty() {
                    anyhow::bail!(
                        "No compatible projects found for template reference.\n\nRequired: {} (Kind: {})",
                        template_ref.api_version, template_ref.kind
                    );
                }

                // Sort projects by name for consistent display
                compatible_projects.sort_by(|a, b| a.0.name.cmp(&b.0.name));
                compatible_projects.dedup_by(|a, b| a.0.name == b.0.name);

                let project_options: Vec<String> = compatible_projects.iter()
                    .map(|(proj, _)| format!("{} ({})", proj.name, template_ref.kind))
                    .collect();

                let selected_project_display = ctx.input.select(
                    &format!("Select reference project ({}):", template_ref.kind),
                    project_options.clone()
                ).context("Failed to select reference project")?;

                let project_index = project_options.iter()
                    .position(|opt| opt == &selected_project_display)
                    .context("Project not found")?;

                let (selected_project, selected_project_path) = &compatible_projects[project_index];

                output::blank();
                ctx.output.key_value_highlight("Reference Project", &selected_project.name);
                ctx.output.key_value("Resource Kind", &template_ref.kind);

                // Discover environments from the selected reference project
                let reference_environments = CollectionDiscovery::discover_environments(&*ctx.fs, &selected_project_path)?;

                if reference_environments.is_empty() {
                    anyhow::bail!("No environments found in reference project: {}", selected_project.name);
                }

                let reference_env_name = if reference_environments.len() == 1 {
                    reference_environments[0].clone()
                } else {
                    ctx.input.select("Select reference environment:", reference_environments.clone())
                        .context("Failed to select reference environment")?
                };

                output::blank();
                ctx.output.key_value("Reference Environment", &reference_env_name);

                // Load reference project's environment resource to get its details
                let reference_env_path = selected_project_path.join("environments").join(&reference_env_name);
                let reference_env_file = reference_env_path.join(".pmp.environment.yaml");

                if !ctx.fs.exists(&reference_env_file) {
                    anyhow::bail!("Reference environment file not found: {:?}", reference_env_file);
                }

                let loaded_env_resource = crate::template::metadata::DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &reference_env_file)
                    .context("Failed to load reference environment resource")?;

                // Store the template reference project
                let data_source_name = template_ref.remote_state.as_ref()
                    .map(|rs| rs.data_source_name.clone())
                    .unwrap_or_else(|| format!("ref_{}", ref_index));

                template_reference_projects.push(crate::template::metadata::TemplateReferenceProject {
                    api_version: loaded_env_resource.api_version.clone(),
                    kind: loaded_env_resource.kind.clone(),
                    name: loaded_env_resource.metadata.name.clone(),
                    environment: reference_env_name,
                    data_source_name,
                });

                output::blank();
            }
        }

        // Step 7: Select environment from Infrastructure
        let selected_environment = if infrastructure.spec.environments.is_empty() {
            anyhow::bail!("Infrastructure must define at least one environment");
        } else if infrastructure.spec.environments.len() == 1 {
            // Only one environment, use it automatically
            let (env_key, env) = infrastructure.spec.environments.iter().next().unwrap();
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
            let mut sorted_envs: Vec<_> = infrastructure.spec.environments.iter().collect();
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
                infrastructure_root.join("projects").join(&project_name)
            };

            // Check if path exists OR if project name already exists in collection
            let name_exists = if ctx.fs.exists(&check_path) {
                true
            } else {
                // Check if any existing project has this name
                match CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &infrastructure_root) {
                    Ok(projects) => projects.iter().any(|p| p.name == project_name),
                    Err(_) => false, // If discovery fails, just check path existence
                }
            };

            if name_exists {
                ctx.output.blank();
                ctx.output.warning(&format!("A project named '{}' already exists in this infrastructure.", project_name));
                ctx.output.dimmed("Project names must be unique across the entire infrastructure.");
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

        // Apply infrastructure-level overrides from template config (if any)
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
        // Convert resource kind to snake_case for directory name
        let resource_kind_snake = resource_kind
            .chars()
            .fold(String::new(), |mut acc, c| {
                if c.is_uppercase() && !acc.is_empty() {
                    acc.push('_');
                }
                acc.push(c.to_ascii_lowercase());
                acc
            });

        let project_root = if let Some(path) = output_path {
            std::path::PathBuf::from(path)
        } else {
            infrastructure_root.join("projects").join(&resource_kind_snake).join(&project_name)
        };

        // Step 13: Determine environment path
        let environment_path = project_root.join("environments").join(&selected_environment);

        // Step 14: Create the directories
        if !ctx.fs.exists(&project_root) {
            ctx.fs.create_dir_all(&project_root)
                .context(format!("Failed to create project root directory: {}", project_root.display()))?;
        }
        if !ctx.fs.exists(&environment_path) {
            ctx.fs.create_dir_all(&environment_path)
                .context(format!("Failed to create environment directory: {}", environment_path.display()))?;
        }

        // Step 15: Render template into environment directory
        ctx.output.subsection("Generating Project Files");
        ctx.output.dimmed("Rendering template...");
        let renderer = TemplateRenderer::new();
        let template_src = &selected_template.path;

        if !ctx.fs.exists(template_src) {
            anyhow::bail!(
                "Template directory not found: {}",
                template_src.display()
            );
        }

        let _generated_files = renderer
            .render_template(ctx, template_src, environment_path.as_path(), &inputs, None)
            .context("Failed to render template")?;

        // Step 15.5: Generate common file (e.g., _common.tf) if executor config is present
        if let Some(executor_config) = &infrastructure.spec.executor {
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
                        &template_reference_projects,
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
            &template_reference_projects,
        ).context("Failed to generate .pmp.environment.yaml file")?;

        ctx.output.blank();
        ctx.output.success("Project created successfully!");

        ctx.output.subsection("Project Details");
        ctx.output.key_value("Infrastructure", &infrastructure.metadata.name);
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
    /// Collect template inputs with infrastructure-level overrides
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
            // Check if there's a infrastructure-level override for this input
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

    /// Prompt for a single input, optionally with a infrastructure-level default override
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
        ctx.fs.write(&pmp_yaml_path, &yaml_content)
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
        template_reference_projects: &[crate::template::metadata::TemplateReferenceProject],
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
                template_reference_projects: template_reference_projects.to_vec(),
            },
        };

        // Serialize to YAML
        let yaml_content = serde_yaml::to_string(&project_env)
            .context("Failed to serialize project environment to YAML")?;

        // Write to .pmp.environment.yaml file
        let pmp_env_yaml_path = environment_path.join(".pmp.environment.yaml");
        ctx.fs.write(&pmp_env_yaml_path, &yaml_content)
            .with_context(|| format!("Failed to write .pmp.environment.yaml file: {:?}", pmp_env_yaml_path))?;

        ctx.output.dimmed(&format!("  Created: {}", pmp_env_yaml_path.display()));

        Ok(())
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{MockFileSystem, MockUserInput, MockOutput, MockCommandExecutor, FileSystem};
    use crate::traits::user_input::MockResponse;
    use crate::context::Context;
    use crate::executor::registry::DefaultExecutorRegistry;
    use std::sync::Arc;
    use std::path::PathBuf;

    /// Helper to create a test context with mocks
    fn create_test_context(
        fs: Arc<MockFileSystem>,
        input: MockUserInput,
    ) -> Context {
        Context {
            fs,
            input: Arc::new(input),
            output: Arc::new(MockOutput::new()),
            command: Arc::new(MockCommandExecutor::new()),
            executor_registry: Arc::new(DefaultExecutorRegistry::with_defaults()),
        }
    }

    /// Helper to set up a basic template pack in the mock filesystem
    fn setup_template_pack(
        fs: &MockFileSystem,
        pack_name: &str,
        template_name: &str,
        resource_kind: &str,
        inputs: &str,
    ) -> PathBuf {
        // Use actual current directory for template pack discovery to work
        let current_dir = std::env::current_dir().unwrap();
        let pack_path = current_dir.join(".pmp/template-packs").join(pack_name);

        // Create template pack file
        let pack_yaml = format!(
            r#"apiVersion: pmp.io/v1
kind: TemplatePack
metadata:
  name: {}
  description: Test template pack
spec: {{}}"#,
            pack_name
        );
        fs.write(&pack_path.join(".pmp.template-pack.yaml"), &pack_yaml).unwrap();

        // Create template directory
        let template_dir = pack_path.join("templates").join(template_name);

        // Create template file
        let template_yaml = format!(
            r#"apiVersion: pmp.io/v1
kind: Template
metadata:
  name: {}
  description: Test template
spec:
  apiVersion: pmp.io/v1
  kind: {}
  executor: opentofu
  inputs:
{}"#,
            template_name, resource_kind, inputs
        );
        fs.write(&template_dir.join(".pmp.template.yaml"), &template_yaml).unwrap();

        // Create src/ subdirectory with a simple template file
        // Templates must have a src/ subdirectory according to the renderer
        fs.write(&template_dir.join("src/main.tf.hbs"), "# Test template").unwrap();

        pack_path
    }

    /// Helper to set up an infrastructure
    /// Creates the infrastructure file in the current directory
    fn setup_infrastructure(
        fs: &MockFileSystem,
        resource_kinds_yaml: &str,
    ) {
        let infrastructure_yaml = format!(
            r#"apiVersion: pmp.io/v1
kind: Infrastructure
metadata:
  name: Test Infrastructure
  description: Test infrastructure
spec:
  resource_kinds:
{}
  environments:
    dev:
      name: Development
      description: Development environment"#,
            resource_kinds_yaml
        );
        // Create in actual current directory (for discovery to work)
        let current_dir = std::env::current_dir().unwrap();
        fs.write(&current_dir.join(".pmp.infrastructure.yaml"), &infrastructure_yaml).unwrap();
    }

    #[test]
    fn test_template_filtering_allowed_true() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Set up template pack with a template
        setup_template_pack(
            &fs,
            "test-pack",
            "allowed-template",
            "TestResource",
            r#"    replica_count:
      default: 1
      description: Number of replicas"#,
        );

        // Set up collection with template allowed
        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource
      templates:
        allowed-template:
          template_pack_name: test-pack
          allowed: true"#,
        );

        // Set up mock user input
        let input = MockUserInput::new();
        input.add_response(MockResponse::Text("test_project".to_string())); // project name
        input.add_response(MockResponse::Text("1".to_string())); // replica_count

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx,
            None, // output_path
            None, // template_packs_paths
        );

        // Verify template was allowed and project was created
        assert!(result.is_ok(), "Create command should succeed: {:?}", result);
    }

    #[test]
    fn test_template_filtering_allowed_false() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Set up template pack with a template
        setup_template_pack(
            &fs,
            "test-pack",
            "blocked-template",
            "TestResource",
            r#"    replica_count:
      default: 1
      description: Number of replicas"#,
        );

        // Set up collection with template blocked
        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource
      templates:
        blocked-template:
          template_pack_name: test-pack
          allowed: false"#,
        );

        let input = MockUserInput::new();
        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx,
            None,
            None,
        );

        // Should fail because no templates are available
        assert!(result.is_err(), "Create command should fail when all templates are blocked");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("No template packs contain templates") ||
            err_msg.contains("allowed in this infrastructure"),
            "Error should mention no matching templates: {}", err_msg
        );
    }

    #[test]
    fn test_input_override_show_as_default_true() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Set up template pack
        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r#"    replica_count:
      default: 1
      description: Number of replicas"#,
        );

        // Set up collection with input override (show_as_default: true)
        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource
      templates:
        test-template:
          template_pack_name: test-pack
          allowed: true
          defaults:
            inputs:
              replica_count:
                value: 5
                show_as_default: true"#,
        );

        // Set up mock user input
        let input = MockUserInput::new();
        input.add_response(MockResponse::Text("test_project".to_string())); // project name
        // User should be prompted for replica_count with default of 5
        input.add_response(MockResponse::Text("3".to_string())); // Override the default to 3

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx,
            None,
            None,
        );

        assert!(result.is_ok(), "Create command should succeed: {:?}", result);

        // Verify the environment file was created with user's input (3, not the collection default 5)
        let current_dir = std::env::current_dir().unwrap();
        let env_file_path = current_dir.join("projects/test_resource/test_project/environments/dev/.pmp.environment.yaml");
        assert!(fs.has_file(&env_file_path), "Environment file should be created");

        let env_content = fs.get_file_contents(&env_file_path).unwrap();
        assert!(env_content.contains("replica_count: 3"), "Environment file should contain user's input value");
    }

    #[test]
    fn test_input_override_show_as_default_false() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Set up template pack
        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r#"    replica_count:
      default: 1
      description: Number of replicas
    environment_name:
      default: dev
      description: Environment name"#,
        );

        // Set up collection with input override (show_as_default: false)
        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource
      templates:
        test-template:
          template_pack_name: test-pack
          allowed: true
          defaults:
            inputs:
              replica_count:
                value: 5
                show_as_default: false"#,
        );

        // Set up mock user input
        let input = MockUserInput::new();
        input.add_response(MockResponse::Text("test_project".to_string())); // project name
        // User should NOT be prompted for replica_count (it's fixed at 5)
        input.add_response(MockResponse::Text("prod".to_string())); // environment_name (still asked)

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx,
            None,
            None,
        );

        assert!(result.is_ok(), "Create command should succeed: {:?}", result);

        // Verify the environment file was created with collection's fixed value
        let current_dir = std::env::current_dir().unwrap();
        let env_file_path = current_dir.join("projects/test_resource/test_project/environments/dev/.pmp.environment.yaml");
        assert!(fs.has_file(&env_file_path), "Environment file should be created");

        let env_content = fs.get_file_contents(&env_file_path).unwrap();
        assert!(env_content.contains("replica_count: 5"), "Environment file should contain collection's fixed value");
        assert!(env_content.contains("environment_name: prod"), "Environment file should contain user's input for other fields");
    }

    #[test]
    fn test_backward_compatibility_no_templates_field() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Set up template pack
        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r#"    replica_count:
      default: 1
      description: Number of replicas"#,
        );

        // Set up collection WITHOUT templates field (backward compatible)
        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource"#,
        );

        // Set up mock user input
        let input = MockUserInput::new();
        input.add_response(MockResponse::Text("test_project".to_string())); // project name
        input.add_response(MockResponse::Text("2".to_string())); // replica_count

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx,
            None,
            None,
        );

        assert!(result.is_ok(), "Create command should succeed with old config format: {:?}", result);

        // Verify project was created
        let current_dir = std::env::current_dir().unwrap();
        let env_file_path = current_dir.join("projects/test_resource/test_project/environments/dev/.pmp.environment.yaml");
        assert!(fs.has_file(&env_file_path), "Environment file should be created");
    }

    #[test]
    fn test_multiple_templates_with_different_configurations() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Set up two templates in the same pack
        setup_template_pack(
            &fs,
            "test-pack",
            "template-a",
            "TestResource",
            r#"    setting_a:
      default: "a"
      description: Setting A"#,
        );

        // Add second template (manually since helper only does one)
        let current_dir = std::env::current_dir().unwrap();
        let template_dir = current_dir.join(".pmp/template-packs/test-pack/templates/template-b");
        let template_yaml = r#"apiVersion: pmp.io/v1
kind: Template
metadata:
  name: template-b
  description: Template B
spec:
  apiVersion: pmp.io/v1
  kind: TestResource
  executor: opentofu
  inputs:
    setting_b:
      default: "b"
      description: Setting B"#;
        fs.write(&template_dir.join(".pmp.template.yaml"), template_yaml).unwrap();
        fs.write(&template_dir.join("src/main.tf.hbs"), "# Template B").unwrap();

        // Set up collection with different configurations for each template
        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource
      templates:
        template-a:
          template_pack_name: test-pack
          allowed: true
          defaults:
            inputs:
              setting_a:
                value: "override-a"
                show_as_default: false
        template-b:
          template_pack_name: test-pack
          allowed: false"#,
        );

        // Set up mock user input
        let input = MockUserInput::new();
        // Only template-a should be available, template-b is blocked
        input.add_response(MockResponse::Text("test_project".to_string())); // project name
        // setting_a should not be prompted (show_as_default: false)

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx,
            None,
            None,
        );

        assert!(result.is_ok(), "Create command should succeed: {:?}", result);

        // Verify the environment file was created with template-a's configuration
        let current_dir = std::env::current_dir().unwrap();
        let env_file_path = current_dir.join("projects/test_resource/test_project/environments/dev/.pmp.environment.yaml");
        assert!(fs.has_file(&env_file_path), "Environment file should be created");

        let env_content = fs.get_file_contents(&env_file_path).unwrap();
        assert!(env_content.contains("setting_a: override-a"), "Environment file should contain template-a's override");
    }
}
