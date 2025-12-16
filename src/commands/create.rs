use crate::collection::CollectionDiscovery;
use crate::commands::apply::ApplyCommand;
use crate::output;
use crate::schema::SchemaValidator;
use crate::template::metadata::{
    AddedPlugin, AddedPluginReference, InputType, PluginProjectReference,
};
use crate::template::utils::interpolate_all;
use crate::template::{
    TemplateDiscovery, TemplateInfo, TemplatePackInfo, TemplateRenderer, TemplateResolver,
};
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

/// Type alias for template pack with templates and their configurations
type PackWithTemplates = (
    TemplatePackInfo,
    Vec<(
        TemplateInfo,
        Option<crate::template::metadata::TemplateConfig>,
    )>,
);

/// Handles the 'create' command - creates projects from templates
pub struct CreateCommand;

/// Helper enum for category navigation options
enum OptionType {
    Back,
    Category(String),
    Template(String, String), // (pack, template)
}

/// Represents an input collection item (either template or plugin)
#[derive(Debug, Clone)]
enum InputCollectionItem {
    Template {
        order: i32,
    },
    Plugin {
        order: i32,
        config: crate::template::metadata::AllowedPluginConfig,
    },
}

/// Plugin metadata collected during input collection phase
#[derive(Debug, Clone)]
struct CollectedPluginInfo {
    template_pack_name: String,
    plugin_name: String,
    plugin_path: std::path::PathBuf,
    inputs: HashMap<String, Value>,
    reference_projects: Vec<(
        crate::template::metadata::ProjectReference,
        crate::template::metadata::DynamicProjectEnvironmentResource,
    )>,
    raw_module_inputs: Option<HashMap<String, String>>,
    plugin_spec: crate::template::metadata::PluginSpec,
}

impl CreateCommand {
    /// Build ordered list of input collection items (template + installed plugins)
    fn build_input_collection_order(
        template_spec: &crate::template::metadata::TemplateSpec,
    ) -> Vec<InputCollectionItem> {
        let mut items = Vec::new();

        // Add template item
        items.push(InputCollectionItem::Template {
            order: template_spec.order,
        });

        // Add installed plugins
        if let Some(plugins_config) = &template_spec.plugins {
            for plugin_config in &plugins_config.installed {
                items.push(InputCollectionItem::Plugin {
                    order: plugin_config.order,
                    config: plugin_config.clone(),
                });
            }
        }

        // Sort by order (ascending), maintaining YAML order when equal
        // Since we use stable sort, items with the same order maintain their insertion order
        // Template is always inserted first, so it has precedence over plugins with same order
        items.sort_by_key(|item| match item {
            InputCollectionItem::Template { order, .. } => *order,
            InputCollectionItem::Plugin { order, .. } => *order,
        });

        items
    }

    /// Prompt user to select a project that satisfies a plugin dependency
    fn prompt_for_dependency_selection(
        ctx: &crate::context::Context,
        dependency: &crate::template::metadata::PluginDependency,
        projects: &[crate::template::metadata::ProjectReference],
        collection_root: &Path,
    ) -> Result<(
        crate::template::metadata::ProjectReference,
        crate::template::metadata::DynamicProjectEnvironmentResource,
    )> {
        let dependency_ref = &dependency.project;

        let compatible_projects: Vec<_> = projects
            .iter()
            .filter_map(|project| {
                let project_path = collection_root.join(&project.path);
                let environments_dir = project_path.join("environments");

                if let Ok(env_entries) = ctx.fs.read_dir(&environments_dir) {
                    for env_path in env_entries {
                        let env_file = env_path.join(".pmp.environment.yaml");

                        if ctx.fs.exists(&env_file)
                            && let Ok(env_resource) =
                                crate::template::metadata::DynamicProjectEnvironmentResource::from_file(
                                    &*ctx.fs,
                                    &env_file,
                                )
                            && env_resource.api_version == dependency_ref.api_version
                            && env_resource.kind == dependency_ref.kind
                        {
                            if !dependency_ref.label_selector.is_empty() {
                                if !env_resource.metadata.labels.is_empty() {
                                    let matches = dependency_ref.label_selector.iter().all(
                                        |(key, value)| {
                                            env_resource
                                                .metadata
                                                .labels
                                                .get(key)
                                                .map(|v| v == value)
                                                .unwrap_or(false)
                                        },
                                    );

                                    if !matches {
                                        continue;
                                    }
                                } else {
                                    continue;
                                }
                            }

                            return Some((project.clone(), env_resource));
                        }
                    }
                }
                None
            })
            .collect();

        anyhow::ensure!(
            !compatible_projects.is_empty(),
            "No compatible projects found for dependency"
        );

        if let Some(description) = &dependency_ref.description {
            ctx.output.dimmed(&format!("  {}", description));
        } else {
            let mut info_parts = vec![format!(
                "Plugin requires a reference to a {} project",
                dependency_ref.kind
            )];

            if !dependency_ref.label_selector.is_empty() {
                let labels_str = dependency_ref
                    .label_selector
                    .iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(", ");
                info_parts.push(format!("(labels: {})", labels_str));
            }

            if let Some(dep_name) = &dependency.dependency_name {
                info_parts.push(format!("[dependency: {}]", dep_name));
            }

            ctx.output.dimmed(&format!("  {}:", info_parts.join(" ")));
        }

        let project_names: Vec<String> = compatible_projects
            .iter()
            .map(|(p, env)| {
                let mut parts = vec![format!("{} ({})", p.name, env.metadata.environment_name)];

                if !env.metadata.labels.is_empty() {
                    let labels_str = env
                        .metadata
                        .labels
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect::<Vec<_>>()
                        .join(", ");
                    parts.push(format!("[{}]", labels_str));
                }

                parts.join(" ")
            })
            .collect();

        let selected_display =
            ctx.input
                .select("Select reference project:", project_names.clone(), None)?;

        let selected_idx = project_names
            .iter()
            .position(|name| name == &selected_display)
            .context("Selected project not found in list")?;

        let (selected_project, selected_env) = &compatible_projects[selected_idx];
        Ok((selected_project.clone(), selected_env.clone()))
    }

    /// Collect inputs for a single installed plugin without rendering
    #[allow(clippy::too_many_arguments)]
    fn collect_plugin_info(
        ctx: &crate::context::Context,
        installed_config: &crate::template::metadata::AllowedPluginConfig,
        template_packs: &[TemplatePackInfo],
        projects: &[crate::template::metadata::ProjectReference],
        collection_root: &Path,
        project_name: &str,
        environment_name: &str,
        preconfigured_plugin: Option<&crate::commands::project_group::PreConfiguredPluginData>,
    ) -> Result<Option<CollectedPluginInfo>> {
        // Find the template pack containing this plugin
        let template_pack = template_packs
            .iter()
            .find(|pack| pack.resource.metadata.name == installed_config.template_pack_name);

        let template_pack = match template_pack {
            Some(pack) => pack,
            None => {
                ctx.output.warning(&format!(
                    "  Template pack '{}' not found. Skipping plugin '{}'.",
                    installed_config.template_pack_name, installed_config.plugin_name
                ));
                return Ok(None);
            }
        };

        // Discover plugins in this template pack
        let plugins = TemplateDiscovery::discover_plugins_in_pack(
            &*ctx.fs,
            &*ctx.output,
            &template_pack.path,
            &template_pack.resource.metadata.name,
        )?;

        // Find the specific plugin
        let plugin_info = plugins
            .iter()
            .find(|p| p.resource.metadata.name == installed_config.plugin_name);

        let plugin_info = match plugin_info {
            Some(info) => info,
            None => {
                ctx.output.warning(&format!(
                    "  Plugin '{}' not found in template pack '{}'. Skipping.",
                    installed_config.plugin_name, installed_config.template_pack_name
                ));
                return Ok(None);
            }
        };

        // Check if plugin requires reference projects
        let reference_projects_and_envs: Vec<(
            crate::template::metadata::ProjectReference,
            crate::template::metadata::DynamicProjectEnvironmentResource,
        )> = if !plugin_info.resource.spec.dependencies.is_empty() {
            let mut refs = Vec::new();

            if let Some(preconfig) = preconfigured_plugin {
                // Use pre-configured references
                refs.extend(preconfig.resolved_references.clone());

                // Prompt only for unresolved dependencies
                for dependency in &preconfig.unresolved_dependencies {
                    match Self::prompt_for_dependency_selection(
                        ctx,
                        dependency,
                        projects,
                        collection_root,
                    ) {
                        Ok(selected) => refs.push(selected),
                        Err(_) => {
                            let _dep_display = dependency
                                .dependency_name
                                .as_deref()
                                .unwrap_or(&dependency.project.kind);
                            ctx.output.warning(&format!(
                                "  Plugin '{}' requires a {} project{}, but none found. Skipping.",
                                installed_config.plugin_name,
                                dependency.project.kind,
                                dependency
                                    .dependency_name
                                    .as_ref()
                                    .map(|n| format!(" (dependency: {})", n))
                                    .unwrap_or_default()
                            ));
                            return Ok(None);
                        }
                    }
                }
            } else {
                // Original flow - prompt for all dependencies
                for dependency in &plugin_info.resource.spec.dependencies {
                    match Self::prompt_for_dependency_selection(
                        ctx,
                        dependency,
                        projects,
                        collection_root,
                    ) {
                        Ok(selected) => refs.push(selected),
                        Err(_) => {
                            let _dep_display = dependency
                                .dependency_name
                                .as_deref()
                                .unwrap_or(&dependency.project.kind);
                            ctx.output.warning(&format!(
                                "  Plugin '{}' requires a {} project{}, but none found. Skipping.",
                                installed_config.plugin_name,
                                dependency.project.kind,
                                dependency
                                    .dependency_name
                                    .as_ref()
                                    .map(|n| format!(" (dependency: {})", n))
                                    .unwrap_or_default()
                            ));
                            return Ok(None);
                        }
                    }
                }
            }

            refs
        } else {
            Vec::new()
        };

        // Merge plugin inputs with installed config inputs
        let mut merged_inputs = plugin_info.resource.spec.inputs.clone();
        // Append installed config inputs, overriding any existing inputs with the same name
        for installed_input in &installed_config.inputs {
            // Remove any existing input with the same name
            merged_inputs.retain(|input_def| input_def.name != installed_input.name);
            // Add the installed config input
            merged_inputs.push(installed_input.clone());
        }

        // Check if user input override is disabled
        let plugin_inputs = if installed_config.disable_user_input_override {
            // Use defaults without asking
            ctx.output.dimmed("  Using default values...");
            Self::build_default_inputs(&merged_inputs, project_name, Some(environment_name))?
        } else if let Some(preconfig) = preconfigured_plugin
            && !preconfig.input_configs.is_empty()
        {
            // Use pre-configured inputs
            ctx.output.dimmed("  Using pre-configured values...");
            Self::build_inputs_with_preconfig(
                ctx,
                &merged_inputs,
                &preconfig.input_configs,
                project_name,
                environment_name,
            )?
        } else {
            // Ask user if they want to customize inputs
            let customize = ctx.input.confirm(
                "  Do you want to customize inputs for this plugin?",
                Some(false),
            )?;

            if customize {
                ctx.output.dimmed("  Collecting plugin inputs...");
                Self::collect_plugin_inputs(
                    ctx,
                    &merged_inputs,
                    project_name,
                    Some(environment_name),
                )?
            } else {
                // Use defaults
                ctx.output.dimmed("  Using default values...");
                Self::build_default_inputs(&merged_inputs, project_name, Some(environment_name))?
            }
        };

        Ok(Some(CollectedPluginInfo {
            template_pack_name: installed_config.template_pack_name.clone(),
            plugin_name: installed_config.plugin_name.clone(),
            plugin_path: plugin_info.path.clone(),
            inputs: plugin_inputs,
            reference_projects: reference_projects_and_envs,
            raw_module_inputs: installed_config.raw_module_inputs.clone(),
            plugin_spec: plugin_info.resource.spec.clone(),
        }))
    }

    /// Render collected plugins to disk
    #[allow(clippy::too_many_arguments)]
    fn render_collected_plugins(
        ctx: &crate::context::Context,
        collected_plugins: Vec<CollectedPluginInfo>,
        environment_path: &Path,
        project_api_version: &str,
        project_kind: &str,
        project_name: &str,
        environment_name: &str,
    ) -> Result<Vec<AddedPlugin>> {
        let mut added_plugins = Vec::new();

        for plugin_info in collected_plugins {
            // Render plugin files
            let mut module_path = environment_path
                .join("modules")
                .join(&plugin_info.template_pack_name)
                .join(&plugin_info.plugin_name);

            // Add reference project name to path ONLY if:
            // 1. Plugin spec has dependencies
            // 2. Number of reference projects matches dependencies
            // 3. Reference project name differs from plugin name (avoid duplication)
            if !plugin_info.plugin_spec.dependencies.is_empty()
                && plugin_info.reference_projects.len()
                    == plugin_info.plugin_spec.dependencies.len()
                && let Some((first_ref, _)) = plugin_info.reference_projects.first()
                && first_ref.name != plugin_info.plugin_name
            {
                module_path = module_path.join(&first_ref.name);
            }

            // Derive pack path from plugin path ({pack}/plugins/{plugin_name}/)
            let pack_path = plugin_info
                .plugin_path
                .parent() // plugins/
                .and_then(|p| p.parent()); // pack root

            let renderer = TemplateRenderer::new_with_partials(&*ctx.fs, pack_path)
                .context("Failed to initialize template renderer with partials")?;
            let plugin_context = Some((
                plugin_info.template_pack_name.as_str(),
                plugin_info.plugin_name.as_str(),
            ));

            let _generated_files = renderer
                .render_template(
                    ctx,
                    &plugin_info.plugin_path,
                    &module_path,
                    &plugin_info.inputs,
                    plugin_context,
                )
                .context("Failed to render plugin files")?;

            // Build AddedPlugin struct
            let plugin_project_ref = PluginProjectReference {
                api_version: project_api_version.to_string(),
                kind: project_kind.to_string(),
                name: project_name.to_string(),
                environment: environment_name.to_string(),
            };

            // Build reference metadata from resolved projects and plugin spec
            let reference_projects_metadata: Vec<AddedPluginReference> = plugin_info
                .reference_projects
                .iter()
                .enumerate()
                .filter_map(|(idx, (_ref_project, ref_env))| {
                    // Get corresponding dependency from plugin spec
                    let dependency = plugin_info.plugin_spec.dependencies.get(idx)?;

                    // Extract data_source_name from remote_state config
                    let data_source_name = dependency
                        .project
                        .remote_state
                        .as_ref()?
                        .data_source_name
                        .clone();

                    Some(AddedPluginReference {
                        api_version: ref_env.api_version.clone(),
                        kind: ref_env.kind.clone(),
                        name: ref_env.metadata.name.clone(),
                        environment: ref_env.metadata.environment_name.clone(),
                        data_source_name,
                        dependency_name: dependency.dependency_name.clone(),
                    })
                })
                .collect();

            // Validate: dependency count should match reference count
            if reference_projects_metadata.len() != plugin_info.plugin_spec.dependencies.len() {
                anyhow::bail!(
                    "Plugin '{}': reference count mismatch (expected {}, got {})",
                    plugin_info.plugin_name,
                    plugin_info.plugin_spec.dependencies.len(),
                    reference_projects_metadata.len()
                );
            }

            added_plugins.push(AddedPlugin {
                template_pack_name: plugin_info.template_pack_name,
                name: plugin_info.plugin_name,
                project: plugin_project_ref,
                reference_projects: reference_projects_metadata,
                inputs: plugin_info.inputs.clone(),
                files: Vec::new(), // Files will be populated during rendering
                plugin_spec: Some(plugin_info.plugin_spec.clone()),
                raw_module_inputs: plugin_info.raw_module_inputs.clone(),
            });
        }

        Ok(added_plugins)
    }

    /// Build default inputs from input specs without prompting user
    fn build_default_inputs(
        inputs_spec: &[crate::template::metadata::InputDefinition],
        project_name: &str,
        environment_name: Option<&str>,
    ) -> Result<HashMap<String, Value>> {
        let mut inputs = HashMap::new();

        // Add project name variables (underscore and hyphen versions)
        let project_name_underscores = project_name.replace('-', "_");
        let project_name_hyphens = project_name.replace('_', "-");
        inputs.insert(
            "_project_name_underscores".to_string(),
            Value::String(project_name_underscores),
        );
        inputs.insert(
            "_project_name_hyphens".to_string(),
            Value::String(project_name_hyphens),
        );

        for input_def in inputs_spec {
            // Skip project name variables (already added)
            if input_def.name == "_project_name_underscores"
                || input_def.name == "_project_name_hyphens"
            {
                continue;
            }

            if let Some(default) = &input_def.default {
                // Build variables map for interpolation
                let vars =
                    Self::get_interpolation_variables(&inputs, project_name, environment_name);

                // Interpolate both ${env:...} and ${var:...} patterns in the default value
                let interpolated_value =
                    crate::template::utils::interpolate_value_all(default, &vars)?;

                inputs.insert(input_def.name.clone(), interpolated_value);
            }
        }

        // Auto-populate project_name if empty
        if let Some(Value::String(s)) = inputs.get("project_name")
            && s.is_empty()
        {
            inputs.insert(
                "project_name".to_string(),
                Value::String(project_name.to_string()),
            );
        }

        // Auto-populate project_description if empty (currently no source for description, so leave empty)
        if let Some(Value::String(s)) = inputs.get("project_description")
            && s.is_empty()
        {
            // Leave empty for now - description would need to be passed as a parameter
            inputs.insert(
                "project_description".to_string(),
                Value::String(String::new()),
            );
        }

        Ok(inputs)
    }

    /// Build inputs merging pre-configured values with defaults
    fn build_inputs_with_preconfig(
        _ctx: &crate::context::Context,
        inputs_spec: &[crate::template::metadata::InputDefinition],
        preconfig_inputs: &HashMap<String, crate::template::metadata::ProjectGroupInputConfig>,
        project_name: &str,
        environment_name: &str,
    ) -> Result<HashMap<String, Value>> {
        let mut inputs = HashMap::new();

        // Add project name variables
        let project_name_underscores = project_name.replace('-', "_");
        let project_name_hyphens = project_name.replace('_', "-");
        inputs.insert(
            "_project_name_underscores".to_string(),
            Value::String(project_name_underscores),
        );
        inputs.insert(
            "_project_name_hyphens".to_string(),
            Value::String(project_name_hyphens),
        );

        for input_def in inputs_spec {
            if input_def.name == "_project_name_underscores"
                || input_def.name == "_project_name_hyphens"
            {
                continue;
            }

            if let Some(config) = preconfig_inputs.get(&input_def.name) {
                if let Some(value) = &config.value {
                    inputs.insert(input_def.name.clone(), value.clone());
                } else if config.use_default
                    && let Some(default) = &input_def.default
                {
                    let vars = Self::get_interpolation_variables(
                        &inputs,
                        project_name,
                        Some(environment_name),
                    );
                    let interpolated_value =
                        crate::template::utils::interpolate_value_all(default, &vars)?;
                    inputs.insert(input_def.name.clone(), interpolated_value);
                }
            } else if let Some(default) = &input_def.default {
                let vars = Self::get_interpolation_variables(
                    &inputs,
                    project_name,
                    Some(environment_name),
                );
                let interpolated_value =
                    crate::template::utils::interpolate_value_all(default, &vars)?;
                inputs.insert(input_def.name.clone(), interpolated_value);
            }
        }

        // Auto-populate project_name if empty
        if let Some(Value::String(s)) = inputs.get("project_name")
            && s.is_empty()
        {
            inputs.insert(
                "project_name".to_string(),
                Value::String(project_name.to_string()),
            );
        }

        // Auto-populate project_description if empty
        if let Some(Value::String(s)) = inputs.get("project_description")
            && s.is_empty()
        {
            inputs.insert(
                "project_description".to_string(),
                Value::String(String::new()),
            );
        }

        Ok(inputs)
    }

    /// Collect plugin inputs from user (same as update.rs)
    fn collect_plugin_inputs(
        ctx: &crate::context::Context,
        inputs_spec: &[crate::template::metadata::InputDefinition],
        project_name: &str,
        environment_name: Option<&str>,
    ) -> Result<HashMap<String, Value>> {
        let mut inputs = HashMap::new();

        // Add project name variables (underscore and hyphen versions)
        let project_name_underscores = project_name.replace('-', "_");
        let project_name_hyphens = project_name.replace('_', "-");
        inputs.insert(
            "_project_name_underscores".to_string(),
            Value::String(project_name_underscores),
        );
        inputs.insert(
            "_project_name_hyphens".to_string(),
            Value::String(project_name_hyphens),
        );

        // Collect each input defined in the plugin
        for input_def in inputs_spec {
            // Skip project name variables (already added)
            if input_def.name == "_project_name_underscores"
                || input_def.name == "_project_name_hyphens"
            {
                continue;
            }

            // Check if input should be shown based on conditions
            if !input_def.should_show(&inputs) {
                // Conditions not met, use default value if available
                if let Some(default) = &input_def.default {
                    // Get variables for interpolation
                    let vars =
                        Self::get_interpolation_variables(&inputs, project_name, environment_name);

                    // Interpolate variables in the default value
                    let interpolated_value =
                        crate::template::utils::interpolate_value_all(default, &vars)?;
                    inputs.insert(input_def.name.clone(), interpolated_value);
                }
                continue; // Skip prompting for this input
            }

            // Get variables for interpolation
            let vars = Self::get_interpolation_variables(&inputs, project_name, environment_name);

            // Interpolate variables in the description (supports both ${env:...} and ${var:...})
            let description = if let Some(desc) = &input_def.description {
                interpolate_all(desc, &vars)?
            } else {
                input_def.name.to_string()
            };

            // Interpolate variables in the default value (supports both ${env:...} and ${var:...})
            let interpolated_default = if let Some(default) = &input_def.default {
                Some(crate::template::utils::interpolate_value_all(
                    default, &vars,
                )?)
            } else {
                None
            };

            let value = if let Some(enum_values) = &input_def.enum_values {
                // This is a select input
                let mut sorted_enum_values = enum_values.clone();
                sorted_enum_values.sort();

                // Find the default value index in the sorted list
                let default_index = if let Some(Value::String(default_val)) = &interpolated_default
                {
                    sorted_enum_values.iter().position(|v| v == default_val)
                } else {
                    None
                };

                let selected = ctx
                    .input
                    .select(&description, sorted_enum_values, default_index)
                    .context("Failed to get input")?;

                Value::String(selected)
            } else if let Some(default) = &interpolated_default {
                // Determine type from default value
                match default {
                    Value::Bool(b) => {
                        let answer = ctx
                            .input
                            .confirm(&description, Some(*b))
                            .context("Failed to get input")?;
                        Value::Bool(answer)
                    }
                    Value::Number(n) => {
                        let answer = ctx
                            .input
                            .text(&description, Some(&n.to_string()))
                            .context("Failed to get input")?;

                        // Try to parse as number
                        if let Ok(num) = answer.parse::<i64>() {
                            Value::Number(num.into())
                        } else if let Ok(num) = answer.parse::<f64>() {
                            Value::Number(serde_json::Number::from_f64(num).unwrap())
                        } else {
                            Value::String(answer)
                        }
                    }
                    Value::String(s) => {
                        // Don't pass empty string as default to avoid "()" display
                        let default = if s.is_empty() { None } else { Some(s.as_str()) };
                        let answer = ctx
                            .input
                            .text(&description, default)
                            .context("Failed to get input")?;
                        Value::String(answer)
                    }
                    Value::Array(_arr) => {
                        // For arrays, use the default value directly
                        default.clone()
                    }
                    _ => {
                        // Fallback to string input
                        let answer = ctx
                            .input
                            .text(&description, None)
                            .context("Failed to get input")?;
                        Value::String(answer)
                    }
                }
            } else {
                // No default, prompt for string
                let answer = ctx
                    .input
                    .text(&description, None)
                    .context("Failed to get input")?;
                Value::String(answer)
            };

            inputs.insert(input_def.name.clone(), value);
        }

        // Auto-populate project_name if empty
        if let Some(Value::String(s)) = inputs.get("project_name")
            && s.is_empty()
        {
            inputs.insert(
                "project_name".to_string(),
                Value::String(project_name.to_string()),
            );
        }

        // Auto-populate project_description if empty
        if let Some(Value::String(s)) = inputs.get("project_description")
            && s.is_empty()
        {
            inputs.insert(
                "project_description".to_string(),
                Value::String(String::new()),
            );
        }

        Ok(inputs)
    }

    /// Parse inputs from JSON or YAML string
    pub fn parse_inputs(inputs_str: &str) -> Result<HashMap<String, Value>> {
        // Try JSON first
        if let Ok(value) = serde_json::from_str::<Value>(inputs_str) {
            if let Value::Object(map) = value {
                return Ok(map.into_iter().collect());
            }
            anyhow::bail!("Inputs must be a JSON/YAML object, not a primitive value");
        }

        // Try YAML
        if let Ok(value) = serde_yaml::from_str::<Value>(inputs_str) {
            if let Value::Object(map) = value {
                return Ok(map.into_iter().collect());
            }
            anyhow::bail!("Inputs must be a JSON/YAML object, not a primitive value");
        }

        anyhow::bail!("Failed to parse inputs as JSON or YAML")
    }

    /// Execute the create command
    #[allow(clippy::too_many_arguments)]
    pub fn execute(
        ctx: &crate::context::Context,
        output_path: Option<&str>,
        template_packs_paths: Option<&str>,
        inputs_str: Option<&str>,
        template_spec: Option<&str>,
        auto_apply: bool,
        project_name: Option<&str>,
        environment_name: Option<&str>,
    ) -> Result<()> {
        // Parse pre-defined inputs if provided
        let predefined_inputs: Option<HashMap<String, Value>> = if let Some(inputs) = inputs_str {
            Some(Self::parse_inputs(inputs)?)
        } else {
            None
        };
        // Step 1: Infrastructure is REQUIRED
        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. No .pmp.infrastructure.yaml found in current directory or parent directories.\n\nPlease create an Infrastructure first or navigate to an existing one.")?;

        ctx.output.section("Infrastructure");
        ctx.output
            .key_value_highlight("Name", &infrastructure.metadata.name);
        if let Some(desc) = &infrastructure.metadata.description {
            ctx.output.key_value("Description", desc);
        }

        // Step 2: Validate infrastructure configuration
        if infrastructure.spec.categories.is_empty() {
            anyhow::bail!(
                "Infrastructure must define categories.\n\nPlease add categories to organize templates in the Infrastructure."
            );
        }

        // Note: template_packs can be empty - we'll include all discovered packs in that case

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

        let all_template_packs = TemplateDiscovery::discover_template_packs_with_custom_paths(
            &*ctx.fs,
            &*ctx.output,
            &custom_paths,
        )
        .context("Failed to discover template packs")?;

        // Check if template packs exist, offer installation if not
        if !crate::template::check_and_offer_installation(
            &*ctx.fs,
            &*ctx.output,
            &all_template_packs,
        )? {
            anyhow::bail!("Cannot proceed without template packs.");
        }

        // Re-discover template packs after potential installation
        let all_template_packs = if all_template_packs.is_empty() {
            TemplateDiscovery::discover_template_packs_with_custom_paths(
                &*ctx.fs,
                &*ctx.output,
                &custom_paths,
            )
            .context("Failed to re-discover template packs after installation")?
        } else {
            all_template_packs
        };

        // Step 4: Build flat list of allowed templates from category tree
        let allowed_templates =
            Self::collect_templates_from_categories(&infrastructure.spec.categories);

        if allowed_templates.is_empty() {
            anyhow::bail!(
                "No templates defined in category tree.\n\nPlease add templates to categories in the Infrastructure."
            );
        }

        // Step 4.5: Filter template packs to only configured ones (or include all if template_packs is empty)
        let configured_pack_names: Option<std::collections::HashSet<&String>> =
            if infrastructure.spec.template_packs.is_empty() {
                None // No filtering - include all packs
            } else {
                Some(infrastructure.spec.template_packs.keys().collect())
            };

        // Step 5: Filter template packs by checking their templates against category tree
        let mut filtered_packs_with_templates: Vec<PackWithTemplates> = Vec::new();

        for pack in &all_template_packs {
            let pack_name = &pack.resource.metadata.name;

            // Skip packs not in template_packs config (if configured)
            if let Some(ref configured_names) = configured_pack_names
                && !configured_names.contains(pack_name)
            {
                continue;
            }

            let templates_in_pack =
                TemplateDiscovery::discover_templates_in_pack(&*ctx.fs, &*ctx.output, &pack.path)
                    .context("Failed to discover templates in pack")?;

            // Filter templates that are in the category tree
            let matching_templates: Vec<(
                TemplateInfo,
                Option<crate::template::metadata::TemplateConfig>,
            )> = templates_in_pack
                .into_iter()
                .filter_map(|t| {
                    let template_name = &t.resource.metadata.name;

                    // Check if this template is in the category tree
                    if allowed_templates.contains(&(pack_name.clone(), template_name.clone())) {
                        // Get template configuration from template_packs (if any)
                        let config = infrastructure
                            .get_template_config(pack_name, template_name)
                            .map(|override_config| {
                                // Convert TemplateOverrideConfig to TemplateConfig for compatibility
                                crate::template::metadata::TemplateConfig {
                                    template_pack_name: pack_name.clone(),
                                    allowed: true,
                                    defaults: override_config.defaults.clone(),
                                }
                            });

                        Some((t, config))
                    } else {
                        None // Template not in category tree
                    }
                })
                .collect();

            // Only include packs that have at least one matching template
            if !matching_templates.is_empty() {
                filtered_packs_with_templates.push((pack.clone(), matching_templates));
            }
        }

        if filtered_packs_with_templates.is_empty() {
            anyhow::bail!(
                "No template packs contain templates that match the categories in this infrastructure.\n\nCategories: {}",
                Self::format_category_names(&infrastructure.spec.categories)
            );
        }

        ctx.output.blank();

        // Step 5: Select template (either from --template flag or via category navigation)
        let (selected_pack_name, selected_template_name) = if let Some(template_str) = template_spec
        {
            // Parse template specification in format: pack-name/template-name
            let parts: Vec<&str> = template_str.split('/').collect();
            if parts.len() != 2 {
                anyhow::bail!(
                    "Invalid template format: '{}'\n\nExpected format: template-pack-name/template-name\n\nExample: kubernetes-workloads/http-api",
                    template_str
                );
            }

            let pack_name = parts[0].to_string();
            let template_name = parts[1].to_string();

            // Validate that the template exists in filtered packs
            let mut template_found = false;
            for (pack, templates) in &filtered_packs_with_templates {
                if pack.resource.metadata.name == pack_name {
                    for (template, _config) in templates {
                        if template.resource.metadata.name == template_name {
                            template_found = true;
                            break;
                        }
                    }
                    if template_found {
                        break;
                    }
                }
            }

            if !template_found {
                anyhow::bail!(
                    "Template '{}/{}' not found or not allowed in this infrastructure.\n\nAvailable templates:\n{}",
                    pack_name,
                    template_name,
                    Self::format_available_templates(&filtered_packs_with_templates)
                );
            }

            ctx.output.subsection("Template Selection");
            ctx.output.dimmed(&format!(
                "Using specified template: {}/{}",
                pack_name, template_name
            ));
            ctx.output.blank();

            (pack_name, template_name)
        } else {
            // Navigate category tree interactively
            ctx.output.subsection("Template Selection");
            ctx.output.dimmed("Browse templates by category");
            ctx.output.blank();

            let selected_template_ref = Self::navigate_categories_and_select_template(
                ctx,
                &infrastructure.spec.categories,
                &filtered_packs_with_templates,
            )?;

            (
                selected_template_ref.0.clone(),
                selected_template_ref.1.clone(),
            )
        };

        // Find all templates matching the name (may have multiple versions)
        // Also capture the pack path for partial discovery
        let mut matching_templates: Vec<(
            TemplateInfo,
            Option<crate::template::metadata::TemplateConfig>,
            std::path::PathBuf, // pack_path
        )> = Vec::new();

        for (pack, templates) in &filtered_packs_with_templates {
            for (template, config) in templates {
                if pack.resource.metadata.name == selected_pack_name
                    && template.resource.metadata.name == selected_template_name
                {
                    matching_templates.push((template.clone(), config.clone(), pack.path.clone()));
                }
            }
        }

        if matching_templates.is_empty() {
            anyhow::bail!("Selected template not found in discovered templates");
        }

        // If multiple versions exist, let user select one
        let (selected_template, template_config, selected_version, selected_pack_path) =
            if matching_templates.len() == 1 {
                let (template, config, pack_path) = matching_templates.into_iter().next().unwrap();
                let version = template
                    .version
                    .as_ref()
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "0.0.1".to_string());
                (template, config, version, pack_path)
            } else {
                // Sort by version descending (latest first)
                matching_templates.sort_by(|a, b| {
                    match (&b.0.version, &a.0.version) {
                        (Some(vb), Some(va)) => vb.cmp(va),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    }
                });

                ctx.output.subsection("Select Version");
                ctx.output.dimmed(&format!(
                    "Template '{}' has {} versions available",
                    selected_template_name,
                    matching_templates.len()
                ));
                ctx.output.blank();

                let version_options: Vec<String> = matching_templates
                    .iter()
                    .map(|(t, _, _)| {
                        t.version
                            .as_ref()
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "0.0.1 (legacy)".to_string())
                    })
                    .collect();

                let selected_version_str = ctx
                    .input
                    .select("Version:", version_options.clone(), Some(0))
                    .context("Failed to select version")?;

                let version_index = version_options
                    .iter()
                    .position(|v| v == &selected_version_str)
                    .context("Version not found")?;

                let (template, config, pack_path) =
                    matching_templates.into_iter().nth(version_index).unwrap();
                let version = selected_version_str.replace(" (legacy)", "");
                (template, config, version, pack_path)
            };

        // Display selected template info
        ctx.output.subsection("Selected Template");
        ctx.output
            .key_value_highlight("Template", &selected_template.resource.metadata.name);
        ctx.output.key_value("Version", &selected_version);
        if let Some(desc) = &selected_template.resource.metadata.description {
            ctx.output.key_value("Description", desc);
        }
        ctx.output.blank();

        // Step 5.5: Resolve template inheritance (if extends is set)
        // Capture path before potential move
        let original_template_path = selected_template.path.clone();
        let (selected_template, resolved_base_paths) =
            if selected_template.resource.spec.extends.is_some() {
                // Find the template pack for inheritance resolution
                let template_pack = all_template_packs
                    .iter()
                    .find(|p| p.resource.metadata.name == selected_pack_name)
                    .context("Template pack not found for inheritance resolution")?;

                ctx.output
                    .dimmed("Resolving template inheritance chain...");

                let resolved = TemplateResolver::resolve(
                    &*ctx.fs,
                    &*ctx.output,
                    &selected_template,
                    template_pack,
                    &all_template_packs,
                )
                .context("Failed to resolve template inheritance")?;

                // Show inheritance chain
                if resolved.inheritance_chain.len() > 1 {
                    ctx.output.key_value(
                        "Inheritance",
                        &resolved.inheritance_chain.join(" -> "),
                    );
                }
                ctx.output.blank();

                // Create a new TemplateInfo with the merged resource
                let merged_template = TemplateInfo {
                    resource: resolved.resource,
                    path: resolved.path,
                    version: resolved.version,
                };

                (merged_template, resolved.base_paths)
            } else {
                // No inheritance, use template as-is
                (selected_template, vec![original_template_path])
            };

        // Step 5 (OLD): Select template pack - REPLACED BY CATEGORY NAVIGATION
        /*
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

            let selected_pack_display = ctx.input.select("Select a template pack:", pack_options.clone(), None)
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

            let selected_template_display = ctx.input.select("Template:", template_options.clone(), None)
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
        */

        // Step 6.5: Handle template reference projects (if template requires them)
        let mut template_reference_projects = Vec::new();

        if !selected_template.resource.spec.dependencies.is_empty() {
            ctx.output.subsection("Template Reference Projects");
            ctx.output
                .dimmed("This template requires reference projects to be selected.");
            output::blank();

            // Discover all projects in the collection
            let projects = CollectionDiscovery::discover_projects(
                &*ctx.fs,
                &*ctx.output,
                &infrastructure_root,
            )?;

            for (ref_index, dep) in selected_template
                .resource
                .spec
                .dependencies
                .iter()
                .enumerate()
            {
                let template_ref = &dep.project;
                let ref_number = ref_index + 1;
                let total_refs = selected_template.resource.spec.dependencies.len();

                // Show description if available, otherwise show kind and label selectors
                if let Some(description) = &template_ref.description {
                    ctx.output.dimmed(&format!(
                        "Reference {} of {}: {}",
                        ref_number, total_refs, description
                    ));
                } else {
                    let mut info_parts = vec![
                        format!("Reference {} of {}", ref_number, total_refs),
                        format!("Kind: {}", template_ref.kind),
                    ];

                    if !template_ref.label_selector.is_empty() {
                        let labels_str = template_ref
                            .label_selector
                            .iter()
                            .map(|(k, v)| format!("{}={}", k, v))
                            .collect::<Vec<_>>()
                            .join(", ");
                        info_parts.push(format!("Labels: {}", labels_str));
                    }

                    if let Some(remote_state) = &template_ref.remote_state {
                        info_parts.push(format!("Data source: {}", remote_state.data_source_name));
                    }

                    ctx.output.dimmed(&info_parts.join(" | "));
                }
                output::blank();

                // Filter projects by required apiVersion and kind
                let mut compatible_projects = Vec::new();
                for project in &projects {
                    let project_path = infrastructure_root.join(&project.path);
                    let environments_dir = project_path.join("environments");

                    if let Ok(env_entries) = ctx.fs.read_dir(&environments_dir) {
                        for env_path in env_entries {
                            let env_file = env_path.join(".pmp.environment.yaml");
                            if ctx.fs.exists(&env_file)
                                && let Ok(env_resource) = crate::template::metadata::DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file) {
                                    // Check apiVersion/kind match AND template is in category tree
                                    if env_resource.api_version == template_ref.api_version
                                        && env_resource.kind == template_ref.kind
                                        && Self::labels_match(&project.labels, &template_ref.label_selector)
                                        && env_resource.spec.template.as_ref()
                                            .map(|t| infrastructure.is_template_in_category_tree(&t.template_pack_name, &t.name))
                                            .unwrap_or(false) {
                                        compatible_projects.push((project.clone(), project_path.clone()));
                                        break;
                                    }
                                }
                        }
                    }
                }

                if compatible_projects.is_empty() {
                    anyhow::bail!(
                        "No compatible projects found for template reference.\n\nRequired: {} (Kind: {})",
                        template_ref.api_version,
                        template_ref.kind
                    );
                }

                // Sort projects by name for consistent display
                compatible_projects.sort_by(|a, b| a.0.name.cmp(&b.0.name));
                compatible_projects.dedup_by(|a, b| a.0.name == b.0.name);

                let project_options: Vec<String> = compatible_projects
                    .iter()
                    .map(|(proj, _)| {
                        // Show project name with labels if available
                        if !proj.labels.is_empty() {
                            let labels_str = proj
                                .labels
                                .iter()
                                .map(|(k, v)| format!("{}={}", k, v))
                                .collect::<Vec<_>>()
                                .join(", ");
                            return format!("{} [{}]", proj.name, labels_str);
                        }
                        proj.name.clone()
                    })
                    .collect();

                let selected_project_display = ctx
                    .input
                    .select("Select reference project:", project_options.clone(), None)
                    .context("Failed to select reference project")?;

                let project_index = project_options
                    .iter()
                    .position(|opt| opt == &selected_project_display)
                    .context("Project not found")?;

                let (selected_project, selected_project_path) = &compatible_projects[project_index];

                output::blank();
                ctx.output
                    .key_value_highlight("Reference Project", &selected_project.name);
                ctx.output.key_value("Resource Kind", &template_ref.kind);

                // Discover environments from the selected reference project
                let reference_environments =
                    CollectionDiscovery::discover_environments(&*ctx.fs, selected_project_path)?;

                if reference_environments.is_empty() {
                    anyhow::bail!(
                        "No environments found in reference project: {}",
                        selected_project.name
                    );
                }

                let reference_env_name = if reference_environments.len() == 1 {
                    reference_environments[0].clone()
                } else {
                    ctx.input
                        .select(
                            "Select reference environment:",
                            reference_environments.clone(),
                            None,
                        )
                        .context("Failed to select reference environment")?
                };

                output::blank();
                ctx.output
                    .key_value("Reference Environment", &reference_env_name);

                // Load reference project's environment resource to get its details
                let reference_env_path = selected_project_path
                    .join("environments")
                    .join(&reference_env_name);
                let reference_env_file = reference_env_path.join(".pmp.environment.yaml");

                if !ctx.fs.exists(&reference_env_file) {
                    anyhow::bail!(
                        "Reference environment file not found: {:?}",
                        reference_env_file
                    );
                }

                let loaded_env_resource =
                    crate::template::metadata::DynamicProjectEnvironmentResource::from_file(
                        &*ctx.fs,
                        &reference_env_file,
                    )
                    .context("Failed to load reference environment resource")?;

                // Store the template reference project
                let data_source_name = template_ref
                    .remote_state
                    .as_ref()
                    .map(|rs| rs.data_source_name.clone())
                    .unwrap_or_else(|| format!("ref_{}", ref_index));

                template_reference_projects.push(
                    crate::template::metadata::TemplateReferenceProject {
                        api_version: loaded_env_resource.api_version.clone(),
                        kind: loaded_env_resource.kind.clone(),
                        name: loaded_env_resource.metadata.name.clone(),
                        environment: reference_env_name,
                        data_source_name,
                    },
                );

                output::blank();
            }
        }

        // Step 7: Select environment from Infrastructure
        let selected_environment = if let Some(env_id) = environment_name {
            // Environment specified via --environment flag (using environment ID/key)
            // Validate that the environment ID exists
            if !infrastructure.spec.environments.contains_key(env_id) {
                anyhow::bail!(
                    "Environment '{}' not found in infrastructure.\n\nAvailable environment IDs: {}",
                    env_id,
                    infrastructure
                        .spec
                        .environments
                        .keys()
                        .map(|k| k.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }

            let env = &infrastructure.spec.environments[env_id];
            ctx.output.subsection("Environment");
            ctx.output.environment_badge(&env.name);
            ctx.output
                .dimmed(&format!("Using environment ID: {}", env_id));
            ctx.output.blank();

            env_id.to_string()
        } else if infrastructure.spec.environments.is_empty() {
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

            let env_options: Vec<String> = sorted_envs
                .iter()
                .map(|(_, env)| {
                    if let Some(desc) = &env.description {
                        format!("{} - {}", env.name, desc)
                    } else {
                        env.name.clone()
                    }
                })
                .collect();

            let selected_env_display = ctx
                .input
                .select("Environment:", env_options.clone(), None)
                .context("Failed to select environment")?;

            // Find the key for the selected environment
            let env_index = env_options
                .iter()
                .position(|opt| opt == &selected_env_display)
                .context("Environment not found")?;

            sorted_envs
                .get(env_index)
                .map(|(key, _)| (*key).clone())
                .context("Environment key not found")?
        };

        // Step 8: Select project dependencies (for dependency-only templates with executor: none)
        let mut project_dependencies = Vec::new();

        // Skip dependency selection if the template has pre-defined projects (ProjectGroup)
        // Those projects will be auto-created with create: true
        if selected_template.resource.spec.executor.name() == "none"
            && selected_template.resource.spec.projects.is_empty()
        {
            ctx.output.subsection("Project Dependencies");
            ctx.output.dimmed(
                "This is a dependency-only project. Select the projects that this group should manage."
            );
            output::blank();

            // Discover all projects in the collection
            let projects = CollectionDiscovery::discover_projects(
                &*ctx.fs,
                &*ctx.output,
                &infrastructure_root,
            )?;

            if projects.is_empty() {
                ctx.output
                    .warning("No existing projects found in this collection.");
                ctx.output.dimmed(
                    "You can add dependencies later by editing the .pmp.environment.yaml file.",
                );
                output::blank();
            } else {
                // Allow user to select multiple projects
                let project_options: Vec<String> = projects
                    .iter()
                    .map(|proj| format!("{} ({})", proj.name, proj.kind))
                    .collect();

                let selected_project_displays = ctx
                    .input
                    .multi_select(
                        "Select projects to include in this group (space to select, enter when done):",
                        project_options.clone(),
                        None,
                    )
                    .context("Failed to select projects")?;

                if selected_project_displays.is_empty() {
                    ctx.output.warning("No projects selected.");
                    ctx.output.dimmed(
                        "You can add dependencies later by editing the .pmp.environment.yaml file.",
                    );
                    output::blank();
                } else {
                    // For each selected project, allow user to select environments
                    for selected_display in selected_project_displays {
                        // Find the project index
                        let index = project_options
                            .iter()
                            .position(|opt| opt == &selected_display)
                            .context("Project not found")?;

                        let selected_project = &projects[index];
                        let project_path = infrastructure_root.join(&selected_project.path);

                        output::blank();
                        ctx.output
                            .key_value_highlight("Project", &selected_project.name);

                        // Discover environments from the selected project
                        let project_environments =
                            CollectionDiscovery::discover_environments(&*ctx.fs, &project_path)?;

                        if project_environments.is_empty() {
                            ctx.output.warning(&format!(
                                "No environments found in project: {}. Skipping.",
                                selected_project.name
                            ));
                            continue;
                        }

                        // Allow user to select multiple environments
                        let selected_envs = ctx
                            .input
                            .multi_select(
                                &format!("Select environments for {} (space to select, enter when done):", selected_project.name),
                                project_environments.clone(),
                                None,
                            )
                            .context("Failed to select environments")?;

                        if selected_envs.is_empty() {
                            ctx.output.warning(&format!(
                                "No environments selected for {}. Skipping.",
                                selected_project.name
                            ));
                            continue;
                        }

                        ctx.output
                            .key_value("Environments", &selected_envs.join(", "));

                        // Store as ProjectDependency
                        project_dependencies.push(crate::template::metadata::ProjectDependency {
                            project: crate::template::metadata::DependencyProject {
                                name: selected_project.name.clone(),
                                environments: selected_envs,
                                create: false, // User-selected dependencies don't auto-create
                            },
                        });
                    }

                    output::blank();
                    ctx.output.success(&format!(
                        "Added {} project(s) as dependencies",
                        project_dependencies.len()
                    ));
                    output::blank();
                }
            }
        } else if selected_template.resource.spec.executor.name() == "none"
            && !selected_template.resource.spec.projects.is_empty()
        {
            // ProjectGroup with pre-defined projects - inform user about auto-creation
            ctx.output.subsection("Project Dependencies");
            ctx.output.info(&format!(
                "This project group has {} pre-defined project(s) that will be created automatically.",
                selected_template.resource.spec.projects.projects().len()
            ));
            output::blank();
        }

        // Step 9: Validate resource kind
        // Validate resource kind contains only alphanumeric characters
        let resource_kind = &selected_template.resource.spec.kind;
        if !resource_kind.chars().all(|c| c.is_alphanumeric()) {
            anyhow::bail!(
                "Resource kind '{}' must contain only alphanumeric characters (found invalid characters)",
                resource_kind
            );
        }

        // Step 9: Get project name (from flag or prompt)
        ctx.output.subsection("Project Configuration");
        let mut project_name = if let Some(name) = project_name {
            // Project name specified via --name flag
            ctx.output
                .dimmed(&format!("Using specified project name: {}", name));
            ctx.output.blank();

            // Validate the provided name
            if let Err(e) = SchemaValidator::validate_project_name(name) {
                anyhow::bail!("Invalid project name '{}': {}", name, e);
            }

            name.to_string()
        } else {
            // Prompt for project name
            SchemaValidator::prompt_for_project_name(ctx).context("Failed to get project name")?
        };

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
                match CollectionDiscovery::discover_projects(
                    &*ctx.fs,
                    &*ctx.output,
                    &infrastructure_root,
                ) {
                    Ok(projects) => projects.iter().any(|p| p.name == project_name),
                    Err(_) => false, // If discovery fails, just check path existence
                }
            };

            if name_exists {
                ctx.output.blank();
                ctx.output.warning(&format!(
                    "A project named '{}' already exists in this infrastructure.",
                    project_name
                ));
                ctx.output
                    .dimmed("Project names must be unique across the entire infrastructure.");
                ctx.output.dimmed("Please choose a different name:");
                project_name = SchemaValidator::prompt_for_project_name(ctx)
                    .context("Failed to get project name")?;
            } else {
                break;
            }
        }

        // Step 10: Build ordered list of input collection items (template + plugins)
        let input_collection_order =
            Self::build_input_collection_order(&selected_template.resource.spec);

        // Discover projects early (needed for plugins that require reference projects)
        let discovered_projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &infrastructure_root)?;

        // Step 11: Collect inputs in order (template + plugins)
        let mut inputs = HashMap::new();
        let mut collected_plugins = Vec::new();

        for item in input_collection_order {
            match item {
                InputCollectionItem::Template { .. } => {
                    // Collect template inputs
                    ctx.output.subsection("Template Inputs");
                    ctx.output
                        .dimmed("Please provide the following information:");

                    // Start with base inputs from template spec
                    let mut merged_inputs = selected_template.resource.spec.inputs.clone();

                    // Override with environment-specific inputs if they exist
                    if let Some(env_overrides) = selected_template
                        .resource
                        .spec
                        .environments
                        .get(&selected_environment)
                    {
                        for env_input in &env_overrides.overrides.inputs {
                            // Remove any existing input with the same name
                            merged_inputs.retain(|input_def| input_def.name != env_input.name);
                            // Add the environment-specific input
                            merged_inputs.push(env_input.clone());
                        }
                    }

                    // Apply infrastructure-level overrides from template config (if any)
                    // Precedence: Template base → Environment overrides → Collection overrides → User input
                    let collection_overrides = template_config
                        .as_ref()
                        .map(|config| &config.defaults.inputs);

                    // Collect inputs from user (respecting collection overrides and predefined inputs)
                    inputs = Self::collect_template_inputs_with_overrides(
                        ctx,
                        &merged_inputs,
                        &project_name,
                        Some(&selected_environment),
                        collection_overrides,
                        predefined_inputs.as_ref(),
                    )
                    .context("Failed to collect inputs")?;
                }
                InputCollectionItem::Plugin { config, .. } => {
                    // Collect plugin inputs
                    ctx.output.blank();
                    ctx.output.dimmed(&format!(
                        "Installing plugin: {}/{}",
                        config.template_pack_name, config.plugin_name
                    ));

                    if let Some(plugin_info) = Self::collect_plugin_info(
                        ctx,
                        &config,
                        &all_template_packs,
                        &discovered_projects,
                        &infrastructure_root,
                        &project_name,
                        &selected_environment,
                        None,
                    )? {
                        collected_plugins.push(plugin_info);
                    }
                }
            }
        }

        // Step 11: Add internal fields for template rendering
        inputs.insert(
            "_environment".to_string(),
            serde_json::Value::String(selected_environment.clone()),
        );
        inputs.insert(
            "_resource_api_version".to_string(),
            serde_json::Value::String(selected_template.resource.spec.api_version.clone()),
        );
        inputs.insert(
            "_resource_kind".to_string(),
            serde_json::Value::String(selected_template.resource.spec.kind.clone()),
        );

        // Add hyphenated version of project name for template rendering (legacy - kept for compatibility)
        let project_name_hyphens = project_name.replace('_', "-");
        inputs.insert(
            "_project_name_hyphens".to_string(),
            serde_json::Value::String(project_name_hyphens),
        );

        // Add underscore version of project name for interpolation (hyphens to underscores)
        let project_name_underscores = project_name.replace('-', "_");
        inputs.insert(
            "_project_name_underscores".to_string(),
            serde_json::Value::String(project_name_underscores),
        );

        // Step 11b: Collect secret references for inputs with secret_manager enabled
        let collected_secrets = Self::collect_secret_inputs(
            ctx,
            &selected_template.resource.spec.inputs,
            infrastructure.spec.secrets.as_ref(),
        )?;

        // Step 12: Determine project root path
        // Project path format: projects/{project_name}
        let project_root = if let Some(path) = output_path {
            std::path::PathBuf::from(path)
        } else {
            infrastructure_root.join("projects").join(&project_name)
        };

        // Step 13: Determine environment path
        let environment_path = project_root
            .join("environments")
            .join(&selected_environment);

        // Step 14: Create the directories
        if !ctx.fs.exists(&project_root) {
            ctx.fs.create_dir_all(&project_root).context(format!(
                "Failed to create project root directory: {}",
                project_root.display()
            ))?;
        }
        if !ctx.fs.exists(&environment_path) {
            ctx.fs.create_dir_all(&environment_path).context(format!(
                "Failed to create environment directory: {}",
                environment_path.display()
            ))?;
        }

        // Step 15: Render collected plugins and template into environment directory
        ctx.output.subsection("Generating Project Files");
        let template_src = &selected_template.path;

        if !ctx.fs.exists(template_src) {
            anyhow::bail!("Template directory not found: {}", template_src.display());
        }

        // Render collected plugins first
        let added_plugins = Self::render_collected_plugins(
            ctx,
            collected_plugins,
            &environment_path,
            &selected_template.resource.spec.api_version,
            &selected_template.resource.spec.kind,
            &project_name,
            &selected_environment,
        )?;

        // Add _plugins variable to inputs for template rendering
        let mut template_inputs = inputs.clone();

        if !added_plugins.is_empty() {
            let plugins_data = serde_json::json!({
                "added": added_plugins
            });
            template_inputs.insert("_plugins".to_string(), plugins_data);
        }

        // Then render template (with inheritance support)
        // Render from all base paths in order (base first, child last)
        // Child files overwrite base files with the same name
        ctx.output.dimmed("Rendering template...");
        let renderer = TemplateRenderer::new_with_partials(&*ctx.fs, Some(&selected_pack_path))
            .context("Failed to initialize template renderer with partials")?;

        for base_path in &resolved_base_paths {
            if ctx.fs.exists(base_path) {
                let _generated_files = renderer
                    .render_template(
                        ctx,
                        base_path,
                        environment_path.as_path(),
                        &template_inputs,
                        None,
                    )
                    .context("Failed to render template")?;
            }
        }

        // Step 15.5: Generate common file (e.g., _common.tf) if executor config is present
        // The executor itself decides whether to generate anything (only opentofu does)
        let template_executor_name = selected_template.resource.spec.executor.name();
        if let Some(executor_config) = &infrastructure.spec.executor
            && !executor_config.config.is_empty()
        {
            // Create executor instance based on template's executor
            let executor: Box<dyn crate::executor::Executor> = match template_executor_name {
                "opentofu" => Box::new(crate::executor::OpenTofuExecutor::new()),
                "none" => Box::new(crate::executor::NoneExecutor::new()),
                _ => anyhow::bail!("Unknown executor: {}", template_executor_name),
            };

            let metadata = crate::executor::ProjectMetadata {
                api_version: &selected_template.resource.spec.api_version,
                kind: &selected_template.resource.spec.kind,
                environment: &selected_environment,
                project_name: &project_name,
            };

            // Pass plugins if any were added
            let plugins_ref = if !added_plugins.is_empty() {
                Some(added_plugins.as_slice())
            } else {
                None
            };

            executor
                .generate_common_file(
                    ctx,
                    &environment_path,
                    &executor_config.config,
                    &metadata,
                    plugins_ref,
                    &template_reference_projects,
                    &collected_secrets,
                    infrastructure.spec.secrets.as_ref(),
                )
                .context("Failed to generate common file")?;
        }

        // Step 16: Auto-generate .pmp.project.yaml file (identifier only)
        ctx.output.dimmed("  Generating .pmp.project.yaml...");
        Self::generate_project_identifier_yaml(
            ctx,
            &project_root,
            &project_name,
            inputs.get("description").and_then(|v| v.as_str()),
            &selected_template.resource.metadata.labels,
        )
        .context("Failed to generate .pmp.project.yaml file")?;

        // Step 17: Auto-generate .pmp.environment.yaml file (with spec)
        ctx.output.dimmed("  Generating .pmp.environment.yaml...");

        Self::generate_project_environment_yaml(
            ctx,
            &environment_path,
            &selected_environment,
            &project_name,
            &selected_template.resource,
            &inputs,
            &collected_secrets,
            &selected_pack_name,
            &selected_template.resource.metadata.name,
            &selected_version,
            &template_reference_projects,
            &added_plugins,
            &project_dependencies,
            None, // No executor override in interactive mode
        )
        .context("Failed to generate .pmp.environment.yaml file")?;

        ctx.output.blank();
        ctx.output.success("Project created successfully!");

        ctx.output.subsection("Project Details");
        ctx.output
            .key_value("Infrastructure", &infrastructure.metadata.name);
        ctx.output.key_value_highlight("Name", &project_name);
        ctx.output
            .key_value("Kind", &selected_template.resource.spec.kind);
        ctx.output.environment_badge(&selected_environment);
        ctx.output
            .key_value("Project root", &project_root.display().to_string());
        ctx.output
            .key_value("Environment path", &environment_path.display().to_string());

        // Execute apply if --apply flag is set, otherwise ask user
        ctx.output.blank();
        let should_apply = if auto_apply {
            ctx.output
                .dimmed("Auto-applying infrastructure (--apply flag set)...");
            true
        } else {
            ctx.input
                .confirm("Do you want to execute 'apply' now?", Some(false))
                .context("Failed to get confirmation")?
        };

        if should_apply {
            ctx.output.blank();
            let env_path_str = environment_path
                .to_str()
                .context("Failed to convert environment path to string")?;
            ApplyCommand::execute(ctx, Some(env_path_str), false, false, None, &[])?;
        } else {
            let next_steps_list = vec![
                format!(
                    "Review the generated files in {}",
                    environment_path.display()
                ),
                "Run 'pmp preview' to see what will be created".to_string(),
                "Run 'pmp apply' to apply the infrastructure".to_string(),
            ];
            output::next_steps(&next_steps_list);
        }

        Ok(())
    }

    /// Collect inputs from user based on template input specifications
    /// Collect template inputs with infrastructure-level overrides
    fn collect_template_inputs_with_overrides(
        ctx: &crate::context::Context,
        inputs_spec: &[crate::template::metadata::InputDefinition],
        project_name: &str,
        environment_name: Option<&str>,
        collection_overrides: Option<
            &std::collections::HashMap<String, crate::template::metadata::InputOverride>,
        >,
        predefined_inputs: Option<&HashMap<String, Value>>,
    ) -> Result<std::collections::HashMap<String, serde_json::Value>> {
        let mut inputs = std::collections::HashMap::new();

        // Add project name variables (underscore and hyphen versions)
        let project_name_underscores = project_name.replace('-', "_");
        let project_name_hyphens = project_name.replace('_', "-");
        inputs.insert(
            "_project_name_underscores".to_string(),
            serde_json::Value::String(project_name_underscores),
        );
        inputs.insert(
            "_project_name_hyphens".to_string(),
            serde_json::Value::String(project_name_hyphens),
        );

        // Collect each input defined in the template
        for input_def in inputs_spec {
            // Check if there's a predefined value for this input
            if let Some(predefined) = predefined_inputs.and_then(|p| p.get(&input_def.name)) {
                // Use the predefined value directly (with variable interpolation)
                let vars =
                    Self::get_interpolation_variables(&inputs, project_name, environment_name);
                let value = crate::template::utils::interpolate_value_all(predefined, &vars)?;
                inputs.insert(input_def.name.clone(), value);
                continue;
            }

            // Check if input should be shown based on conditions
            if !input_def.should_show(&inputs) {
                // Conditions not met, use default value if available
                if let Some(default) = &input_def.default {
                    // Get variables for interpolation
                    let vars =
                        Self::get_interpolation_variables(&inputs, project_name, environment_name);

                    // Interpolate variables in the default value
                    let interpolated_value =
                        crate::template::utils::interpolate_value_all(default, &vars)?;
                    inputs.insert(input_def.name.clone(), interpolated_value);
                }
                continue; // Skip prompting for this input
            }

            // Check if there's a infrastructure-level override for this input
            let override_config =
                collection_overrides.and_then(|overrides| overrides.get(&input_def.name));

            let value = if let Some(override_cfg) = override_config {
                if !override_cfg.show_as_default {
                    // Use the override value directly without prompting the user
                    // Still need to interpolate variables in the override value (supports ${env:...} and ${var:...})
                    let vars =
                        Self::get_interpolation_variables(&inputs, project_name, environment_name);
                    crate::template::utils::interpolate_value_all(&override_cfg.value, &vars)?
                } else {
                    // Show the override value as the default and let user override
                    Self::prompt_for_input_with_default(
                        ctx,
                        &input_def.name,
                        &input_def.to_input_spec(),
                        Some(&override_cfg.value),
                        &inputs,
                        project_name,
                        environment_name,
                    )?
                }
            } else {
                // No collection override, use normal flow
                Self::prompt_for_input_with_default(
                    ctx,
                    &input_def.name,
                    &input_def.to_input_spec(),
                    None,
                    &inputs,
                    project_name,
                    environment_name,
                )?
            };

            inputs.insert(input_def.name.clone(), value);
        }

        Ok(inputs)
    }

    /// Helper function to get available variables for interpolation
    fn get_interpolation_variables(
        inputs: &HashMap<String, Value>,
        project_name: &str,
        environment_name: Option<&str>,
    ) -> HashMap<String, Value> {
        let mut vars = HashMap::new();

        // Add project name variables (underscore and hyphen versions)
        let project_name_underscores = project_name.replace('-', "_");
        let project_name_hyphens = project_name.replace('_', "-");
        vars.insert(
            "_project_name_underscores".to_string(),
            Value::String(project_name_underscores),
        );
        vars.insert(
            "_project_name_hyphens".to_string(),
            Value::String(project_name_hyphens),
        );

        // Add environment name if provided
        if let Some(env_name) = environment_name {
            vars.insert(
                "_environment_name".to_string(),
                Value::String(env_name.to_string()),
            );
        }

        // Add all collected inputs so far (for progressive interpolation)
        for (key, value) in inputs {
            vars.insert(key.clone(), value.clone());
        }

        vars
    }

    /// Prompt for a single input, optionally with a infrastructure-level default override
    fn prompt_for_input_with_default(
        ctx: &crate::context::Context,
        input_name: &str,
        input_spec: &crate::template::metadata::InputSpec,
        collection_default: Option<&serde_json::Value>,
        current_inputs: &HashMap<String, Value>,
        project_name: &str,
        environment_name: Option<&str>,
    ) -> Result<serde_json::Value> {
        // Get variables for interpolation
        let vars =
            Self::get_interpolation_variables(current_inputs, project_name, environment_name);

        // Interpolate variables in the description (supports both ${env:...} and ${var:...})
        let description = if let Some(desc) = &input_spec.description {
            interpolate_all(desc, &vars)?
        } else {
            input_name.to_string()
        };

        // Use collection default if provided, otherwise use template default
        let mut effective_default = collection_default.or(input_spec.default.as_ref()).cloned();

        // Interpolate variables in the default value (supports both ${env:...} and ${var:...})
        if let Some(ref default_val) = effective_default {
            effective_default = Some(crate::template::utils::interpolate_value_all(
                default_val,
                &vars,
            )?);
        }

        // Check for explicit input type
        if let Some(ref input_type) = input_spec.input_type {
            return Self::prompt_for_typed_input(
                ctx,
                &description,
                input_type,
                effective_default.as_ref(),
            );
        }

        // Legacy behavior: check for enum_values (deprecated)
        if let Some(enum_values) = &input_spec.enum_values {
            let mut sorted_enum_values = enum_values.clone();
            sorted_enum_values.sort();

            let default_str = effective_default
                .as_ref()
                .and_then(|v| v.as_str())
                .or_else(|| sorted_enum_values.first().map(|s| s.as_str()));

            let selected = if let Some(default) = default_str {
                let starting_cursor = sorted_enum_values.iter().position(|v| v == default);
                ctx.input
                    .select(&description, sorted_enum_values.clone(), starting_cursor)
                    .context("Failed to get input")?
            } else {
                ctx.input
                    .select(&description, sorted_enum_values, None)
                    .context("Failed to get input")?
            };

            return Ok(serde_json::Value::String(selected));
        }

        // Infer type from default value
        if let Some(default) = effective_default.as_ref() {
            match default {
                serde_json::Value::Bool(b) => {
                    let answer = ctx
                        .input
                        .confirm(&description, Some(*b))
                        .context("Failed to get input")?;
                    Ok(serde_json::Value::Bool(answer))
                }
                serde_json::Value::Number(n) => {
                    let answer = ctx
                        .input
                        .text(&description, Some(&n.to_string()))
                        .context("Failed to get input")?;

                    // Try to parse as number
                    if let Ok(num) = answer.parse::<i64>() {
                        Ok(serde_json::Value::Number(num.into()))
                    } else if let Ok(num) = answer.parse::<f64>() {
                        Ok(serde_json::Value::Number(
                            serde_json::Number::from_f64(num).unwrap(),
                        ))
                    } else {
                        Ok(serde_json::Value::String(answer))
                    }
                }
                serde_json::Value::String(s) => {
                    // Don't pass empty string as default to avoid "()" display
                    let default = if s.is_empty() { None } else { Some(s.as_str()) };
                    let answer = ctx
                        .input
                        .text(&description, default)
                        .context("Failed to get input")?;
                    Ok(serde_json::Value::String(answer))
                }
                serde_json::Value::Null => {
                    // Null default is treated as no default
                    let answer = ctx
                        .input
                        .text(&description, None)
                        .context("Failed to get input")?;
                    Ok(serde_json::Value::String(answer))
                }
                _ => {
                    // Fallback to string input
                    let prompt_text = format!("{} [required]", description);
                    let answer = ctx
                        .input
                        .text(&prompt_text, None)
                        .context("Failed to get input")?;
                    Ok(serde_json::Value::String(answer))
                }
            }
        } else {
            // No default, prompt for string
            let prompt_text = format!("{} [required]", description);
            let answer = ctx
                .input
                .text(&prompt_text, None)
                .context("Failed to get input")?;
            Ok(serde_json::Value::String(answer))
        }
    }

    /// Prompt for a typed input based on InputType
    fn prompt_for_typed_input(
        ctx: &crate::context::Context,
        description: &str,
        input_type: &InputType,
        default: Option<&Value>,
    ) -> Result<Value> {
        // Extract default_str for string-based inputs
        let default_str = default.and_then(|v| v.as_str());

        match input_type {
            InputType::String => {
                let is_empty = default_str.map(|s| s.is_empty()).unwrap_or(true);
                let prompt_text = if is_empty {
                    format!("{} [required]", description)
                } else {
                    description.to_string()
                };
                // Don't pass empty string as default to avoid "()" display
                let default = if is_empty { None } else { default_str };
                let answer = ctx
                    .input
                    .text(&prompt_text, default)
                    .context("Failed to get input")?;
                Ok(Value::String(answer))
            }
            InputType::Boolean => {
                // Implement as select with Yes/No options
                let options = vec!["Yes".to_string(), "No".to_string()];

                let selected = ctx
                    .input
                    .select(description, options, None)
                    .context("Failed to get input")?;

                Ok(Value::Bool(selected == "Yes"))
            }
            InputType::Number { min, max, integer } => {
                let default_num = default.and_then(|v| v.as_f64());
                let prompt_text =
                    Self::build_number_prompt(description, default_num, *min, *max, *integer);

                loop {
                    let answer = ctx
                        .input
                        .text(&prompt_text, default_num.map(|n| n.to_string()).as_deref())
                        .context("Failed to get input")?;

                    if *integer {
                        // Parse as integer
                        match answer.parse::<i64>() {
                            Ok(num) => {
                                let num_f64 = num as f64;
                                // Validate range
                                if let Some(min_val) = min
                                    && num_f64 < *min_val
                                {
                                    ctx.output.warning(&format!("Value must be >= {}", min_val));
                                    continue;
                                }
                                if let Some(max_val) = max
                                    && num_f64 > *max_val
                                {
                                    ctx.output.warning(&format!("Value must be <= {}", max_val));
                                    continue;
                                }
                                return Ok(Value::Number(num.into()));
                            }
                            Err(_) => {
                                ctx.output.warning("Please enter a valid integer");
                                continue;
                            }
                        }
                    } else {
                        // Parse as float
                        match answer.parse::<f64>() {
                            Ok(num) => {
                                // Validate range
                                if let Some(min_val) = min
                                    && num < *min_val
                                {
                                    ctx.output.warning(&format!("Value must be >= {}", min_val));
                                    continue;
                                }
                                if let Some(max_val) = max
                                    && num > *max_val
                                {
                                    ctx.output.warning(&format!("Value must be <= {}", max_val));
                                    continue;
                                }
                                return Ok(Value::Number(
                                    serde_json::Number::from_f64(num).context("Invalid number")?,
                                ));
                            }
                            Err(_) => {
                                ctx.output.warning("Please enter a valid number");
                                continue;
                            }
                        }
                    }
                }
            }
            InputType::Select { options } => {
                // Build list of display labels
                let labels: Vec<String> = options.iter().map(|opt| opt.label.clone()).collect();

                // Find default index if there's a default value
                let default_idx = if let Some(def_val) = default.and_then(|v| v.as_str()) {
                    options.iter().position(|opt| opt.value == def_val)
                } else {
                    None
                };

                let _ = default_idx; // Suppress unused warning (would be used for cursor positioning)

                let selected_label = ctx
                    .input
                    .select(description, labels, None)
                    .context("Failed to get input")?;

                // Find the corresponding value
                let selected_option = options
                    .iter()
                    .find(|opt| opt.label == selected_label)
                    .context("Selected option not found")?;

                Ok(Value::String(selected_option.value.clone()))
            }
            InputType::MultiSelect { options, min, max } => {
                // Build list of display labels
                let labels: Vec<String> = options.iter().map(|opt| opt.label.clone()).collect();

                // Find default indices if there's a default value
                let default_indices = if let Some(Value::Array(defaults)) = default {
                    let indices: Vec<usize> = defaults
                        .iter()
                        .filter_map(|v| v.as_str())
                        .filter_map(|val| options.iter().position(|opt| opt.value == val))
                        .collect();
                    Some(indices)
                } else {
                    None
                };

                let prompt_text = Self::build_multiselect_prompt(description, *min, *max);

                loop {
                    let selected_labels = ctx
                        .input
                        .multi_select(&prompt_text, labels.clone(), default_indices.as_deref())
                        .context("Failed to get input")?;

                    // Validate min/max selections
                    if let Some(min_val) = min
                        && selected_labels.len() < *min_val
                    {
                        ctx.output
                            .warning(&format!("Please select at least {} option(s)", min_val));
                        continue;
                    }
                    if let Some(max_val) = max
                        && selected_labels.len() > *max_val
                    {
                        ctx.output
                            .warning(&format!("Please select at most {} option(s)", max_val));
                        continue;
                    }

                    // Find the corresponding values
                    let selected_values: Vec<Value> = selected_labels
                        .iter()
                        .filter_map(|label| {
                            options
                                .iter()
                                .find(|opt| &opt.label == label)
                                .map(|opt| Value::String(opt.value.clone()))
                        })
                        .collect();

                    return Ok(Value::Array(selected_values));
                }
            }
            InputType::Password => {
                let answer = ctx
                    .input
                    .password(description)
                    .context("Failed to get input")?;
                Ok(Value::String(answer))
            }
            InputType::ProjectSelect {
                api_version,
                kind,
                labels,
            } => Self::prompt_for_project_select(
                ctx,
                description,
                api_version.as_deref(),
                kind.as_deref(),
                labels,
                false,
            ),
            InputType::MultiProjectSelect {
                api_version,
                kind,
                labels,
                min,
                max,
            } => Self::prompt_for_multiproject_select(
                ctx,
                description,
                api_version.as_deref(),
                kind.as_deref(),
                labels,
                *min,
                *max,
            ),
            InputType::Path {
                must_exist,
                directories_only,
                files_only,
            } => Self::prompt_for_path(
                ctx,
                description,
                default_str,
                *must_exist,
                *directories_only,
                *files_only,
            ),
            InputType::Url {
                allowed_schemes,
                check_reachable,
            } => Self::prompt_for_url(
                ctx,
                description,
                default_str,
                allowed_schemes,
                *check_reachable,
            ),
            InputType::Date { min, max } => Self::prompt_for_date(
                ctx,
                description,
                default_str,
                min.as_deref(),
                max.as_deref(),
            ),
            InputType::DateTime { min, max } => Self::prompt_for_datetime(
                ctx,
                description,
                default_str,
                min.as_deref(),
                max.as_deref(),
            ),
            InputType::Json { prettify } => {
                Self::prompt_for_json(ctx, description, default_str, *prettify)
            }
            InputType::Yaml { prettify } => {
                Self::prompt_for_yaml(ctx, description, default_str, *prettify)
            }
            InputType::List {
                separator,
                min,
                max,
                trim_items,
                remove_empty,
            } => Self::prompt_for_list(
                ctx,
                description,
                default_str,
                separator,
                *min,
                *max,
                *trim_items,
                *remove_empty,
            ),
            InputType::Email => Self::prompt_for_email(ctx, description, default_str),
            InputType::IpAddress {
                ipv4_only,
                ipv6_only,
            } => Self::prompt_for_ip_address(ctx, description, default_str, *ipv4_only, *ipv6_only),
            InputType::Cidr {
                ipv4_only,
                ipv6_only,
            } => Self::prompt_for_cidr(ctx, description, default_str, *ipv4_only, *ipv6_only),
            InputType::Port => Self::prompt_for_port(ctx, description, default_str),
            InputType::Object { fields } => {
                Self::prompt_for_object(ctx, description, fields, default)
            }
            InputType::RepeatableObject {
                fields,
                min,
                max,
                add_another_prompt,
            } => Self::prompt_for_repeatable_object(
                ctx,
                description,
                fields,
                *min,
                *max,
                add_another_prompt.as_deref(),
                default,
            ),
            InputType::Color { allow_alpha } => {
                Self::prompt_for_color(ctx, description, default_str, *allow_alpha)
            }
            InputType::Duration {
                min_seconds,
                max_seconds,
            } => {
                Self::prompt_for_duration(ctx, description, default_str, *min_seconds, *max_seconds)
            }
            InputType::Cron => Self::prompt_for_cron(ctx, description, default_str),
            InputType::KeyValue {
                key_value_separator,
                pair_separator,
                min,
                max,
            } => Self::prompt_for_keyvalue(
                ctx,
                description,
                default_str,
                key_value_separator,
                pair_separator,
                *min,
                *max,
            ),
            InputType::Semver {
                allow_prerelease,
                allow_build,
            } => Self::prompt_for_semver(
                ctx,
                description,
                default_str,
                *allow_prerelease,
                *allow_build,
            ),
        }
    }

    /// Build a number prompt with range information
    fn build_number_prompt(
        description: &str,
        default: Option<f64>,
        min: Option<f64>,
        max: Option<f64>,
        integer: bool,
    ) -> String {
        let mut prompt = description.to_string();

        let mut constraints = Vec::new();
        let type_str = if integer { "integer" } else { "number" };

        if let Some(min_val) = min {
            constraints.push(format!("min: {}", min_val));
        }
        if let Some(max_val) = max {
            constraints.push(format!("max: {}", max_val));
        }

        // Build constraint text without default (inquire will show the default)
        let constraint_text = if !constraints.is_empty() {
            format!("{}, {}", type_str, constraints.join(", "))
        } else if default.is_none() {
            format!("{} - required", type_str)
        } else {
            type_str.to_string()
        };

        prompt.push_str(&format!(" [{}]", constraint_text));
        prompt
    }

    /// Build a multiselect prompt with selection constraints
    fn build_multiselect_prompt(
        description: &str,
        min: Option<usize>,
        max: Option<usize>,
    ) -> String {
        let mut prompt = description.to_string();

        let mut constraints = Vec::new();
        if let Some(min_val) = min {
            constraints.push(format!("min: {}", min_val));
        }
        if let Some(max_val) = max {
            constraints.push(format!("max: {}", max_val));
        }

        if !constraints.is_empty() {
            prompt.push_str(&format!(" [{}]", constraints.join(", ")));
        }

        prompt
    }

    /// Prompt for project selection based on filters
    fn prompt_for_project_select(
        ctx: &crate::context::Context,
        description: &str,
        api_version: Option<&str>,
        kind: Option<&str>,
        labels: &std::collections::HashMap<String, String>,
        _allow_multiple: bool,
    ) -> Result<Value> {
        // Get collection root
        let collection_root = ctx
            .fs
            .current_dir()
            .context("Failed to get current directory")?;

        // Discover all projects
        let all_projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &collection_root)?;

        // Filter projects based on criteria
        let filtered_projects: Vec<_> = all_projects
            .iter()
            .filter(|project| {
                // Filter by kind if specified
                if let Some(k) = kind
                    && project.kind != *k
                {
                    return false;
                }

                // Filter by api_version and labels by checking environments
                let project_path = collection_root.join(&project.path);
                let environments_dir = project_path.join("environments");

                if let Ok(env_entries) = ctx.fs.read_dir(&environments_dir) {
                    for env_path in env_entries {
                        let env_file = env_path.join(".pmp.environment.yaml");
                        if ctx.fs.exists(&env_file)
                            && let Ok(env_resource) = crate::template::metadata::DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
                        {
                            // Check api_version if specified
                            if let Some(av) = api_version
                                && env_resource.api_version != av
                            {
                                continue;
                            }

                            // Check labels if specified
                            if !Self::labels_match(&env_resource.metadata.labels, labels) {
                                continue;
                            }

                            return true; // At least one environment matches
                        }
                    }
                }

                false
            })
            .collect();

        if filtered_projects.is_empty() {
            anyhow::bail!("No projects found matching the specified criteria");
        }

        // Build selection options
        let project_names: Vec<String> = filtered_projects
            .iter()
            .map(|p| format!("{} ({})", p.name, p.kind))
            .collect();

        let selected = ctx
            .input
            .select(description, project_names, None)
            .context("Failed to select project")?;

        // Extract project name from selection
        let project_name = selected.split(" (").next().unwrap_or(&selected).to_string();

        Ok(Value::String(project_name))
    }

    /// Prompt for multiple project selection based on filters
    fn prompt_for_multiproject_select(
        ctx: &crate::context::Context,
        description: &str,
        api_version: Option<&str>,
        kind: Option<&str>,
        labels: &std::collections::HashMap<String, String>,
        min: Option<usize>,
        max: Option<usize>,
    ) -> Result<Value> {
        // Get collection root
        let collection_root = ctx
            .fs
            .current_dir()
            .context("Failed to get current directory")?;

        // Discover all projects
        let all_projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &collection_root)?;

        // Filter projects based on criteria (same logic as single select)
        let filtered_projects: Vec<_> = all_projects
            .iter()
            .filter(|project| {
                if let Some(k) = kind
                    && project.kind != *k
                {
                    return false;
                }

                let project_path = collection_root.join(&project.path);
                let environments_dir = project_path.join("environments");

                if let Ok(env_entries) = ctx.fs.read_dir(&environments_dir) {
                    for env_path in env_entries {
                        let env_file = env_path.join(".pmp.environment.yaml");
                        if ctx.fs.exists(&env_file)
                            && let Ok(env_resource) = crate::template::metadata::DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
                        {
                            if let Some(av) = api_version
                                && env_resource.api_version != av
                            {
                                continue;
                            }

                            // Check labels if specified
                            if !Self::labels_match(&env_resource.metadata.labels, labels) {
                                continue;
                            }

                            return true;
                        }
                    }
                }

                false
            })
            .collect();

        if filtered_projects.is_empty() {
            anyhow::bail!("No projects found matching the specified criteria");
        }

        // Build selection options
        let project_names: Vec<String> = filtered_projects
            .iter()
            .map(|p| format!("{} ({})", p.name, p.kind))
            .collect();

        let prompt_text = Self::build_multiselect_prompt(description, min, max);

        loop {
            let selected_projects = ctx
                .input
                .multi_select(&prompt_text, project_names.clone(), None)
                .context("Failed to select projects")?;

            // Validate min/max selections
            if let Some(min_val) = min
                && selected_projects.len() < min_val
            {
                ctx.output
                    .warning(&format!("Please select at least {} project(s)", min_val));
                continue;
            }
            if let Some(max_val) = max
                && selected_projects.len() > max_val
            {
                ctx.output
                    .warning(&format!("Please select at most {} project(s)", max_val));
                continue;
            }

            // Extract project names from selections
            let project_names: Vec<Value> = selected_projects
                .iter()
                .map(|s| {
                    let name = s.split(" (").next().unwrap_or(s).to_string();
                    Value::String(name)
                })
                .collect();

            return Ok(Value::Array(project_names));
        }
    }

    /// Check if project labels match the required label selector
    /// All labels in selector must be present and match (AND logic)
    fn labels_match(
        project_labels: &std::collections::HashMap<String, String>,
        selector: &std::collections::HashMap<String, String>,
    ) -> bool {
        if selector.is_empty() {
            return true; // No label requirements
        }

        for (key, value) in selector.iter() {
            match project_labels.get(key) {
                Some(project_value) if project_value == value => continue,
                _ => return false,
            }
        }
        true
    }

    /// Prompt for file/directory path input
    fn prompt_for_path(
        ctx: &crate::context::Context,
        description: &str,
        default: Option<&str>,
        must_exist: bool,
        directories_only: bool,
        files_only: bool,
    ) -> Result<Value> {
        let mut prompt = description.to_string();

        if directories_only {
            prompt.push_str(" [directory]");
        } else if files_only {
            prompt.push_str(" [file]");
        } else {
            prompt.push_str(" [path]");
        }

        loop {
            let answer = ctx
                .input
                .text(&prompt, default)
                .context("Failed to get input")?;

            if must_exist {
                let path = std::path::Path::new(&answer);
                if !path.exists() {
                    ctx.output
                        .error(&format!("Path does not exist: {}", answer));
                    continue;
                }
                if directories_only && !path.is_dir() {
                    ctx.output.error("Path must be a directory");
                    continue;
                }
                if files_only && !path.is_file() {
                    ctx.output.error("Path must be a file");
                    continue;
                }
            }

            return Ok(Value::String(answer));
        }
    }

    /// Prompt for URL input
    fn prompt_for_url(
        ctx: &crate::context::Context,
        description: &str,
        default: Option<&str>,
        allowed_schemes: &[String],
        _check_reachable: bool,
    ) -> Result<Value> {
        let prompt = format!("{} [URL]", description);

        loop {
            let answer = ctx
                .input
                .text(&prompt, default)
                .context("Failed to get input")?;

            // Basic URL validation
            if let Err(e) = url::Url::parse(&answer) {
                ctx.output.error(&format!("Invalid URL: {}", e));
                continue;
            }

            // Check allowed schemes if specified
            if !allowed_schemes.is_empty()
                && let Ok(parsed_url) = url::Url::parse(&answer)
                && !allowed_schemes.contains(&parsed_url.scheme().to_string())
            {
                ctx.output.error(&format!(
                    "URL scheme must be one of: {}",
                    allowed_schemes.join(", ")
                ));
                continue;
            }

            return Ok(Value::String(answer));
        }
    }

    /// Prompt for date input (ISO 8601 format: YYYY-MM-DD)
    fn prompt_for_date(
        ctx: &crate::context::Context,
        description: &str,
        default: Option<&str>,
        _min: Option<&str>,
        _max: Option<&str>,
    ) -> Result<Value> {
        let prompt = format!("{} [YYYY-MM-DD]", description);

        loop {
            let answer = ctx
                .input
                .text(&prompt, default)
                .context("Failed to get input")?;

            // Validate date format
            if let Err(e) = chrono::NaiveDate::parse_from_str(&answer, "%Y-%m-%d") {
                ctx.output.error(&format!("Invalid date format: {}", e));
                continue;
            }

            return Ok(Value::String(answer));
        }
    }

    /// Prompt for datetime input (ISO 8601 format)
    fn prompt_for_datetime(
        ctx: &crate::context::Context,
        description: &str,
        default: Option<&str>,
        _min: Option<&str>,
        _max: Option<&str>,
    ) -> Result<Value> {
        let prompt = format!("{} [ISO 8601: YYYY-MM-DDTHH:MM:SSZ]", description);

        loop {
            let answer = ctx
                .input
                .text(&prompt, default)
                .context("Failed to get input")?;

            // Validate datetime format
            if let Err(e) = chrono::DateTime::parse_from_rfc3339(&answer) {
                ctx.output.error(&format!("Invalid datetime format: {}", e));
                continue;
            }

            return Ok(Value::String(answer));
        }
    }

    /// Prompt for JSON input
    fn prompt_for_json(
        ctx: &crate::context::Context,
        description: &str,
        default: Option<&str>,
        prettify: bool,
    ) -> Result<Value> {
        let prompt = format!("{} [JSON]", description);

        loop {
            let answer = ctx
                .input
                .text(&prompt, default)
                .context("Failed to get input")?;

            // Validate JSON
            match serde_json::from_str::<serde_json::Value>(&answer) {
                Ok(value) => {
                    let result = if prettify {
                        serde_json::to_string_pretty(&value).unwrap_or(answer)
                    } else {
                        answer
                    };
                    return Ok(Value::String(result));
                }
                Err(e) => {
                    ctx.output.error(&format!("Invalid JSON: {}", e));
                    continue;
                }
            }
        }
    }

    /// Prompt for YAML input
    fn prompt_for_yaml(
        ctx: &crate::context::Context,
        description: &str,
        default: Option<&str>,
        prettify: bool,
    ) -> Result<Value> {
        let prompt = format!("{} [YAML]", description);

        loop {
            let answer = ctx
                .input
                .text(&prompt, default)
                .context("Failed to get input")?;

            // Validate YAML
            match serde_yaml::from_str::<serde_yaml::Value>(&answer) {
                Ok(value) => {
                    let result = if prettify {
                        serde_yaml::to_string(&value).unwrap_or(answer)
                    } else {
                        answer
                    };
                    return Ok(Value::String(result));
                }
                Err(e) => {
                    ctx.output.error(&format!("Invalid YAML: {}", e));
                    continue;
                }
            }
        }
    }

    /// Prompt for list input (comma-separated values)
    #[allow(clippy::too_many_arguments)]
    fn prompt_for_list(
        ctx: &crate::context::Context,
        description: &str,
        default: Option<&str>,
        separator: &str,
        min: Option<usize>,
        max: Option<usize>,
        trim_items: bool,
        remove_empty: bool,
    ) -> Result<Value> {
        let mut prompt = format!("{} [list, separator: '{}']", description, separator);

        if let Some(min_val) = min {
            prompt.push_str(&format!(" (min: {})", min_val));
        }
        if let Some(max_val) = max {
            prompt.push_str(&format!(" (max: {})", max_val));
        }

        loop {
            let answer = ctx
                .input
                .text(&prompt, default)
                .context("Failed to get input")?;

            let mut items: Vec<String> = answer
                .split(separator)
                .map(|s| {
                    if trim_items {
                        s.trim().to_string()
                    } else {
                        s.to_string()
                    }
                })
                .collect();

            if remove_empty {
                items.retain(|s| !s.is_empty());
            }

            if let Some(min_val) = min
                && items.len() < min_val
            {
                ctx.output
                    .error(&format!("List must have at least {} items", min_val));
                continue;
            }

            if let Some(max_val) = max
                && items.len() > max_val
            {
                ctx.output
                    .error(&format!("List must have at most {} items", max_val));
                continue;
            }

            let values: Vec<Value> = items.into_iter().map(Value::String).collect();
            return Ok(Value::Array(values));
        }
    }

    /// Prompt for object input (structured object with named fields)
    fn prompt_for_object(
        ctx: &crate::context::Context,
        description: &str,
        fields: &[crate::template::metadata::InputDefinition],
        _default: Option<&Value>,
    ) -> Result<Value> {
        ctx.output.info(&format!(
            "{} (object with {} fields)",
            description,
            fields.len()
        ));

        let mut object = serde_json::Map::new();

        for field in fields {
            // Interpolate field description and default
            let field_desc = field.description.as_deref().unwrap_or(&field.name);

            let field_label = format!("  {} → {}", field.name, field_desc);

            // Get the default value for this field
            let field_default = field.default.as_ref();

            // Determine input type and prompt for value
            let value = if let Some(ref input_type) = field.input_type {
                Self::prompt_for_typed_input(ctx, &field_label, input_type, field_default)?
            } else {
                // Fallback to string input
                Self::prompt_for_typed_input(
                    ctx,
                    &field_label,
                    &crate::template::metadata::InputType::String,
                    field_default,
                )?
            };

            object.insert(field.name.clone(), value);
        }

        Ok(Value::Object(object))
    }

    /// Prompt for repeatable object input (array of structured objects)
    #[allow(clippy::too_many_arguments)]
    fn prompt_for_repeatable_object(
        ctx: &crate::context::Context,
        description: &str,
        fields: &[crate::template::metadata::InputDefinition],
        min: Option<usize>,
        max: Option<usize>,
        add_another_prompt: Option<&str>,
        _default: Option<&Value>,
    ) -> Result<Value> {
        let mut items = Vec::new();
        let min_items = min.unwrap_or(0);
        let max_items = max.unwrap_or(100);

        let item_description = add_another_prompt
            .and_then(|p| p.strip_suffix('?'))
            .and_then(|p| p.strip_prefix("Add another "))
            .unwrap_or("item");

        loop {
            let current_count = items.len();

            // Show current items count if any exist
            if current_count > 0 {
                ctx.output.info(&format!(
                    "Current {} count: {}",
                    item_description, current_count
                ));
            }

            // Check if we've reached the maximum
            if current_count >= max_items {
                ctx.output
                    .info(&format!("Maximum of {} items reached", max_items));
                break;
            }

            // Build action options
            let mut options = Vec::new();

            // Always allow adding if we haven't reached max and minimum not yet met
            if current_count < max_items {
                if current_count < min_items {
                    // Must add more items to meet minimum
                    options.push(format!("Add new {}", item_description));
                } else {
                    // Can add, remove, or finish
                    options.push(format!("Add new {}", item_description));

                    if current_count > min_items {
                        options.push(format!("Remove {}", item_description));
                    }

                    options.push("Done".to_string());
                }
            }

            // If only one option (must add), don't show menu
            if options.len() == 1 && current_count < min_items {
                // Must add items to meet minimum
                let item_label = format!("{} #{}", description, current_count + 1);
                let object = Self::prompt_for_object(ctx, &item_label, fields, None)?;
                items.push(object);
                continue;
            }

            // Show action menu
            let action_prompt = format!("What would you like to do with {}?", item_description);
            let selected_action = ctx
                .input
                .select(&action_prompt, options.clone(), Some(0))
                .context("Failed to get action selection")?;

            if selected_action.starts_with("Add new") {
                // Add new item
                let item_label = format!("{} #{}", description, current_count + 1);
                let object = Self::prompt_for_object(ctx, &item_label, fields, None)?;
                items.push(object);
            } else if selected_action.starts_with("Remove") {
                // Remove existing item
                Self::remove_item_from_list(ctx, &mut items, item_description)?;
            } else if selected_action == "Done" {
                break;
            }
        }

        // Validate minimum items
        if items.len() < min_items {
            return Err(anyhow::anyhow!(
                "Minimum {} items required, only {} provided",
                min_items,
                items.len()
            ));
        }

        Ok(Value::Array(items))
    }

    /// Helper function to remove an item from a list
    fn remove_item_from_list(
        ctx: &crate::context::Context,
        items: &mut Vec<Value>,
        item_description: &str,
    ) -> Result<()> {
        if items.is_empty() {
            ctx.output
                .warning(&format!("No {} to remove", item_description));
            return Ok(());
        }

        // Build list of items to display
        let item_options: Vec<String> = items
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                // Try to create a summary of the object
                let summary = Self::summarize_item(item);
                format!("#{} - {}", idx + 1, summary)
            })
            .collect();

        // Add cancel option
        let mut options_with_cancel = item_options.clone();
        options_with_cancel.push("Cancel".to_string());

        let prompt = format!("Select {} to remove:", item_description);
        let selected = ctx
            .input
            .select(&prompt, options_with_cancel, Some(0))
            .context("Failed to get selection")?;

        if selected == "Cancel" {
            return Ok(());
        }

        // Find the selected index and remove it
        if let Some(selected_idx) = item_options.iter().position(|opt| opt == &selected) {
            items.remove(selected_idx);
            ctx.output.info(&format!(
                "Removed {} #{}",
                item_description,
                selected_idx + 1
            ));
        }

        Ok(())
    }

    /// Create a summary string for a Value (used for displaying items)
    fn summarize_item(value: &Value) -> String {
        match value {
            Value::Object(obj) => {
                // Show first few key-value pairs
                let pairs: Vec<String> = obj
                    .iter()
                    .take(3)
                    .map(|(k, v)| {
                        let val_str = match v {
                            Value::String(s) => s.clone(),
                            Value::Number(n) => n.to_string(),
                            Value::Bool(b) => b.to_string(),
                            _ => "...".to_string(),
                        };
                        format!("{}: {}", k, val_str)
                    })
                    .collect();

                if pairs.is_empty() {
                    "empty object".to_string()
                } else {
                    pairs.join(", ")
                }
            }
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Array(arr) => format!("[{} items]", arr.len()),
            Value::Null => "null".to_string(),
        }
    }

    /// Prompt for email input
    fn prompt_for_email(
        ctx: &crate::context::Context,
        description: &str,
        default: Option<&str>,
    ) -> Result<Value> {
        let prompt = format!("{} [email]", description);

        loop {
            let answer = ctx
                .input
                .text(&prompt, default)
                .context("Failed to get input")?;

            // Basic email validation (contains @ and domain)
            if !answer.contains('@') || !answer.contains('.') {
                ctx.output.error("Invalid email format");
                continue;
            }

            return Ok(Value::String(answer));
        }
    }

    /// Prompt for IP address input
    fn prompt_for_ip_address(
        ctx: &crate::context::Context,
        description: &str,
        default: Option<&str>,
        ipv4_only: bool,
        ipv6_only: bool,
    ) -> Result<Value> {
        let ip_type = if ipv4_only {
            "IPv4"
        } else if ipv6_only {
            "IPv6"
        } else {
            "IP"
        };
        let prompt = format!("{} [{}]", description, ip_type);

        loop {
            let answer = ctx
                .input
                .text(&prompt, default)
                .context("Failed to get input")?;

            match answer.parse::<std::net::IpAddr>() {
                Ok(addr) => {
                    if ipv4_only && !addr.is_ipv4() {
                        ctx.output.error("Must be an IPv4 address");
                        continue;
                    }
                    if ipv6_only && !addr.is_ipv6() {
                        ctx.output.error("Must be an IPv6 address");
                        continue;
                    }
                    return Ok(Value::String(answer));
                }
                Err(_) => {
                    ctx.output.error("Invalid IP address");
                    continue;
                }
            }
        }
    }

    /// Prompt for CIDR notation input
    fn prompt_for_cidr(
        ctx: &crate::context::Context,
        description: &str,
        default: Option<&str>,
        ipv4_only: bool,
        ipv6_only: bool,
    ) -> Result<Value> {
        let cidr_type = if ipv4_only {
            "IPv4 CIDR"
        } else if ipv6_only {
            "IPv6 CIDR"
        } else {
            "CIDR"
        };
        let prompt = format!("{} [{}]", description, cidr_type);

        loop {
            let answer = ctx
                .input
                .text(&prompt, default)
                .context("Failed to get input")?;

            // Parse CIDR notation (IP/prefix)
            let parts: Vec<&str> = answer.split('/').collect();
            if parts.len() != 2 {
                ctx.output
                    .error("Invalid CIDR format. Use: IP/prefix (e.g., 192.168.1.0/24)");
                continue;
            }

            match parts[0].parse::<std::net::IpAddr>() {
                Ok(addr) => {
                    if ipv4_only && !addr.is_ipv4() {
                        ctx.output.error("Must be an IPv4 CIDR");
                        continue;
                    }
                    if ipv6_only && !addr.is_ipv6() {
                        ctx.output.error("Must be an IPv6 CIDR");
                        continue;
                    }

                    // Validate prefix length
                    if let Ok(prefix) = parts[1].parse::<u8>() {
                        let max_prefix = if addr.is_ipv4() { 32 } else { 128 };
                        if prefix > max_prefix {
                            ctx.output
                                .error(&format!("Prefix length must be 0-{}", max_prefix));
                            continue;
                        }
                        return Ok(Value::String(answer));
                    } else {
                        ctx.output.error("Invalid prefix length");
                        continue;
                    }
                }
                Err(_) => {
                    ctx.output.error("Invalid IP address in CIDR");
                    continue;
                }
            }
        }
    }

    /// Prompt for port number input
    fn prompt_for_port(
        ctx: &crate::context::Context,
        description: &str,
        default: Option<&str>,
    ) -> Result<Value> {
        let prompt = format!("{} [port: 1-65535]", description);

        loop {
            let answer = ctx
                .input
                .text(&prompt, default)
                .context("Failed to get input")?;

            match answer.parse::<u16>() {
                Ok(port) => {
                    if port == 0 {
                        ctx.output.error("Port must be between 1 and 65535");
                        continue;
                    }
                    return Ok(Value::Number(serde_json::Number::from(port)));
                }
                Err(_) => {
                    ctx.output.error("Invalid port number");
                    continue;
                }
            }
        }
    }

    /// Prompt for hex color input
    fn prompt_for_color(
        ctx: &crate::context::Context,
        description: &str,
        default: Option<&str>,
        allow_alpha: bool,
    ) -> Result<Value> {
        let pattern_desc = if allow_alpha {
            "#RRGGBB or #RRGGBBAA"
        } else {
            "#RRGGBB"
        };
        let prompt = format!("{} [color: {}]", description, pattern_desc);

        loop {
            let answer = ctx
                .input
                .text(&prompt, default)
                .context("Failed to get input")?;

            // Validate hex color format
            if !answer.starts_with('#') {
                ctx.output.error("Color must start with '#'");
                continue;
            }

            let hex = &answer[1..];
            let expected_len = if allow_alpha { vec![6, 8] } else { vec![6] };

            if !expected_len.contains(&hex.len()) {
                ctx.output
                    .error(&format!("Invalid color format. Expected {}", pattern_desc));
                continue;
            }

            if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                ctx.output
                    .error("Color must contain only hex digits (0-9, A-F)");
                continue;
            }

            return Ok(Value::String(answer.to_uppercase()));
        }
    }

    /// Prompt for duration input
    fn prompt_for_duration(
        ctx: &crate::context::Context,
        description: &str,
        default: Option<&str>,
        min_seconds: Option<u64>,
        max_seconds: Option<u64>,
    ) -> Result<Value> {
        let mut prompt = format!("{} [duration: e.g., 1h30m, 5d, 2w]", description);

        if let Some(min) = min_seconds {
            prompt.push_str(&format!(" (min: {}s)", min));
        }
        if let Some(max) = max_seconds {
            prompt.push_str(&format!(" (max: {}s)", max));
        }

        loop {
            let answer = ctx
                .input
                .text(&prompt, default)
                .context("Failed to get input")?;

            // Parse duration string (supports: s, m, h, d, w)
            match Self::parse_duration(&answer) {
                Ok(seconds) => {
                    // Validate range
                    if let Some(min) = min_seconds
                        && seconds < min
                    {
                        ctx.output
                            .error(&format!("Duration must be at least {} seconds", min));
                        continue;
                    }

                    if let Some(max) = max_seconds
                        && seconds > max
                    {
                        ctx.output
                            .error(&format!("Duration must be at most {} seconds", max));
                        continue;
                    }

                    return Ok(Value::Number(serde_json::Number::from(seconds)));
                }
                Err(e) => {
                    ctx.output.error(&format!("Invalid duration: {}", e));
                    continue;
                }
            }
        }
    }

    /// Parse duration string into seconds
    fn parse_duration(input: &str) -> Result<u64> {
        let mut total_seconds: u64 = 0;
        let mut current_num = String::new();

        for ch in input.chars() {
            if ch.is_ascii_digit() {
                current_num.push(ch);
            } else if ch.is_ascii_alphabetic() {
                if current_num.is_empty() {
                    return Err(anyhow::anyhow!("Invalid duration format"));
                }

                let num: u64 = current_num.parse()?;
                current_num.clear();

                let multiplier = match ch {
                    's' => 1,
                    'm' => 60,
                    'h' => 3600,
                    'd' => 86400,
                    'w' => 604800,
                    _ => return Err(anyhow::anyhow!("Unknown unit '{}'. Use: s, m, h, d, w", ch)),
                };

                total_seconds += num * multiplier;
            } else if !ch.is_whitespace() {
                return Err(anyhow::anyhow!("Invalid character: '{}'", ch));
            }
        }

        if !current_num.is_empty() {
            return Err(anyhow::anyhow!(
                "Duration must include a unit (s, m, h, d, w)"
            ));
        }

        if total_seconds == 0 {
            return Err(anyhow::anyhow!("Duration must be greater than 0"));
        }

        Ok(total_seconds)
    }

    /// Prompt for cron expression input
    fn prompt_for_cron(
        ctx: &crate::context::Context,
        description: &str,
        default: Option<&str>,
    ) -> Result<Value> {
        let prompt = format!("{} [cron: e.g., */5 * * * *]", description);

        loop {
            let answer = ctx
                .input
                .text(&prompt, default)
                .context("Failed to get input")?;

            // Basic cron validation (5 or 6 fields)
            let fields: Vec<&str> = answer.split_whitespace().collect();
            if fields.len() != 5 && fields.len() != 6 {
                ctx.output.error(
                    "Cron expression must have 5 fields (minute hour day month weekday) or 6 fields (with seconds)",
                );
                continue;
            }

            // Validate each field contains valid characters
            let valid_chars = "0123456789*,-/";
            let mut is_valid = true;
            for field in &fields {
                if !field
                    .chars()
                    .all(|c| valid_chars.contains(c) || c.is_alphabetic())
                {
                    ctx.output
                        .error(&format!("Invalid characters in field: {}", field));
                    is_valid = false;
                    break;
                }
            }

            if is_valid {
                return Ok(Value::String(answer));
            }
        }
    }

    /// Prompt for key-value pairs input
    #[allow(clippy::too_many_arguments)]
    fn prompt_for_keyvalue(
        ctx: &crate::context::Context,
        description: &str,
        default: Option<&str>,
        key_value_separator: &str,
        pair_separator: &str,
        min: Option<usize>,
        max: Option<usize>,
    ) -> Result<Value> {
        let mut prompt = format!(
            "{} [key-value pairs, e.g., key1{}value1{}key2{}value2]",
            description, key_value_separator, pair_separator, key_value_separator
        );

        if let Some(min_val) = min {
            prompt.push_str(&format!(" (min: {})", min_val));
        }
        if let Some(max_val) = max {
            prompt.push_str(&format!(" (max: {})", max_val));
        }

        loop {
            let answer = ctx
                .input
                .text(&prompt, default)
                .context("Failed to get input")?;

            let mut pairs = serde_json::Map::new();

            if answer.trim().is_empty() {
                if let Some(min_val) = min
                    && min_val > 0
                {
                    ctx.output
                        .error(&format!("At least {} pairs required", min_val));
                    continue;
                }
                return Ok(Value::Object(pairs));
            }

            let pair_strings: Vec<&str> = answer.split(pair_separator).collect();

            // Validate count
            if let Some(min_val) = min
                && pair_strings.len() < min_val
            {
                ctx.output
                    .error(&format!("At least {} pairs required", min_val));
                continue;
            }

            if let Some(max_val) = max
                && pair_strings.len() > max_val
            {
                ctx.output
                    .error(&format!("At most {} pairs allowed", max_val));
                continue;
            }

            let mut valid = true;
            for pair in pair_strings {
                let kv: Vec<&str> = pair.splitn(2, key_value_separator).collect();
                if kv.len() != 2 {
                    ctx.output.error(&format!(
                        "Invalid pair format: '{}'. Expected 'key{}value'",
                        pair.trim(),
                        key_value_separator
                    ));
                    valid = false;
                    break;
                }

                let key = kv[0].trim();
                let value = kv[1].trim();

                if key.is_empty() {
                    ctx.output.error("Key cannot be empty");
                    valid = false;
                    break;
                }

                pairs.insert(key.to_string(), Value::String(value.to_string()));
            }

            if valid {
                return Ok(Value::Object(pairs));
            }
        }
    }

    /// Prompt for semantic version input
    fn prompt_for_semver(
        ctx: &crate::context::Context,
        description: &str,
        default: Option<&str>,
        allow_prerelease: bool,
        allow_build: bool,
    ) -> Result<Value> {
        let mut examples = vec!["1.2.3"];
        if allow_prerelease {
            examples.push("2.0.0-beta.1");
        }
        if allow_build {
            examples.push("1.0.0+build.123");
        }

        let prompt = format!("{} [semver: e.g., {}]", description, examples.join(", "));

        loop {
            let answer = ctx
                .input
                .text(&prompt, default)
                .context("Failed to get input")?;

            // Parse semver: MAJOR.MINOR.PATCH[-PRERELEASE][+BUILD]
            match Self::parse_semver(&answer, allow_prerelease, allow_build) {
                Ok(_) => return Ok(Value::String(answer)),
                Err(e) => {
                    ctx.output.error(&format!("Invalid semver: {}", e));
                    continue;
                }
            }
        }
    }

    /// Parse and validate semantic version
    fn parse_semver(input: &str, allow_prerelease: bool, allow_build: bool) -> Result<()> {
        // Split on + for build metadata
        let (version_and_pre, build) = if let Some(pos) = input.find('+') {
            if !allow_build {
                return Err(anyhow::anyhow!("Build metadata not allowed"));
            }
            (&input[..pos], Some(&input[pos + 1..]))
        } else {
            (input, None)
        };

        // Validate build metadata if present
        if let Some(b) = build
            && (b.is_empty()
                || !b
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '.' || c == '-'))
        {
            return Err(anyhow::anyhow!("Invalid build metadata"));
        }

        // Split on - for prerelease
        let (version, prerelease) = if let Some(pos) = version_and_pre.find('-') {
            if !allow_prerelease {
                return Err(anyhow::anyhow!("Prerelease versions not allowed"));
            }
            (&version_and_pre[..pos], Some(&version_and_pre[pos + 1..]))
        } else {
            (version_and_pre, None)
        };

        // Validate prerelease if present
        if let Some(pre) = prerelease
            && (pre.is_empty()
                || !pre
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '.' || c == '-'))
        {
            return Err(anyhow::anyhow!("Invalid prerelease identifier"));
        }

        // Parse MAJOR.MINOR.PATCH
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() != 3 {
            return Err(anyhow::anyhow!(
                "Version must have exactly 3 parts: MAJOR.MINOR.PATCH"
            ));
        }

        for (i, part) in parts.iter().enumerate() {
            if part.is_empty() {
                return Err(anyhow::anyhow!("Version part cannot be empty"));
            }

            // Check for leading zeros (not allowed except for "0")
            if part.len() > 1 && part.starts_with('0') {
                return Err(anyhow::anyhow!("Version parts cannot have leading zeros"));
            }

            if part.parse::<u32>().is_err() {
                let label = match i {
                    0 => "MAJOR",
                    1 => "MINOR",
                    2 => "PATCH",
                    _ => "version part",
                };
                return Err(anyhow::anyhow!("{} must be a valid number", label));
            }
        }

        Ok(())
    }

    /// Recursively collect all templates from the category tree
    fn collect_templates_from_categories(
        categories: &[crate::template::metadata::Category],
    ) -> std::collections::HashSet<(String, String)> {
        let mut templates = std::collections::HashSet::new();

        for category in categories {
            // Add templates from this category
            for template in &category.templates {
                templates.insert((template.template_pack.clone(), template.template.clone()));
            }

            // Recursively add templates from subcategories
            let sub_templates = Self::collect_templates_from_categories(&category.subcategories);
            templates.extend(sub_templates);
        }

        templates
    }

    /// Format category names for display in error messages
    fn format_category_names(categories: &[crate::template::metadata::Category]) -> String {
        let names: Vec<String> = categories.iter().map(|c| c.name.clone()).collect();

        if names.is_empty() {
            "(none)".to_string()
        } else {
            names.join(", ")
        }
    }

    /// Format available templates for display in error messages
    fn format_available_templates(packs_with_templates: &[PackWithTemplates]) -> String {
        let mut result = Vec::new();
        for (pack, templates) in packs_with_templates {
            for (template, _config) in templates {
                result.push(format!(
                    "  - {}/{}",
                    pack.resource.metadata.name, template.resource.metadata.name
                ));
            }
        }

        if result.is_empty() {
            "(none)".to_string()
        } else {
            result.join("\n")
        }
    }

    /// Clear previous lines from terminal output
    /// Uses ANSI escape codes to move cursor up and clear lines
    fn clear_previous_lines(count: usize) {
        use std::io::{self, Write};
        for _ in 0..count {
            // Move cursor up one line and clear it
            print!("\x1B[1A\x1B[2K");
        }
        // Flush to ensure the escape codes are applied immediately
        io::stdout().flush().ok();
    }

    /// Navigate category tree and select a template
    /// Returns (template_pack_name, template_name)
    fn navigate_categories_and_select_template(
        ctx: &crate::context::Context,
        categories: &[crate::template::metadata::Category],
        filtered_packs_with_templates: &[PackWithTemplates],
    ) -> Result<(String, String)> {
        // Build set of discovered templates
        let discovered_templates: std::collections::HashSet<(String, String)> =
            filtered_packs_with_templates
                .iter()
                .flat_map(|(pack, templates)| {
                    templates.iter().map(move |(template, _)| {
                        (
                            pack.resource.metadata.name.clone(),
                            template.resource.metadata.name.clone(),
                        )
                    })
                })
                .collect();

        // Helper to check if a category has any content (recursively)
        fn has_content(
            category: &crate::template::metadata::Category,
            discovered: &std::collections::HashSet<(String, String)>,
        ) -> bool {
            // Check if this category has any discovered templates
            let has_templates = category
                .templates
                .iter()
                .any(|t| discovered.contains(&(t.template_pack.clone(), t.template.clone())));

            // Check if any subcategories have content
            let has_subcategories = category
                .subcategories
                .iter()
                .any(|sub| has_content(sub, discovered));

            has_templates || has_subcategories
        }

        // Navigation state: stack of category IDs
        let mut nav_stack: Vec<String> = Vec::new();

        loop {
            // Find current category
            let (current_category, current_subcategories) = if nav_stack.is_empty() {
                // At root level - no current category, just root categories
                (None, categories)
            } else {
                // Navigate to the selected category
                let mut current_cats = categories;
                let mut found_category: Option<&crate::template::metadata::Category> = None;

                for (idx, category_id) in nav_stack.iter().enumerate() {
                    let cat = current_cats
                        .iter()
                        .find(|c| &c.id == category_id)
                        .ok_or_else(|| anyhow::anyhow!("Category not found: {}", category_id))?;

                    if idx == nav_stack.len() - 1 {
                        // This is the last category in the stack - this is our current category
                        found_category = Some(cat);
                    } else {
                        // Navigate deeper
                        current_cats = &cat.subcategories;
                    }
                }

                let current_cat = found_category.unwrap();
                (Some(current_cat), &current_cat.subcategories[..])
            };

            // Build options for current level
            let mut options: Vec<String> = Vec::new();
            let mut option_types: Vec<OptionType> = Vec::new();

            // Add subcategories (only those with content)
            for category in current_subcategories {
                if has_content(category, &discovered_templates) {
                    let display = if let Some(desc) = &category.description {
                        format!("📁 {} - {}", category.name, desc)
                    } else {
                        format!("📁 {}", category.name)
                    };
                    options.push(display);
                    option_types.push(OptionType::Category(category.id.clone()));
                }
            }

            // Add templates from current category (only if we're inside a category)
            if let Some(cat) = current_category {
                // We're inside a specific category - show its templates
                for template_ref in &cat.templates {
                    if discovered_templates.contains(&(
                        template_ref.template_pack.clone(),
                        template_ref.template.clone(),
                    )) {
                        let desc = filtered_packs_with_templates
                            .iter()
                            .find(|(p, _)| p.resource.metadata.name == template_ref.template_pack)
                            .and_then(|(_, templates)| {
                                templates
                                    .iter()
                                    .find(|(t, _)| {
                                        t.resource.metadata.name == template_ref.template
                                    })
                                    .and_then(|(t, _)| t.resource.metadata.description.as_deref())
                            })
                            .unwrap_or("");

                        let display = if desc.is_empty() {
                            format!("📄 {}", template_ref.template)
                        } else {
                            format!("📄 {} - {}", template_ref.template, desc)
                        };
                        options.push(display);
                        option_types.push(OptionType::Template(
                            template_ref.template_pack.clone(),
                            template_ref.template.clone(),
                        ));
                    }
                }
            }
            // Note: At root level, we only show categories, not templates

            // Add "Back" option at the end if not at root
            if !nav_stack.is_empty() {
                options.push("← Back".to_string());
                option_types.push(OptionType::Back);
            }

            if options.is_empty() {
                anyhow::bail!("No templates or categories available");
            }

            // Show selection prompt (empty string to avoid repeated prompts during navigation)
            let selected = ctx
                .input
                .select("", options.clone(), None)
                .context("Failed to select")?;

            // Find which option was selected
            let selected_index = options
                .iter()
                .position(|opt| opt == &selected)
                .context("Selection not found")?;

            match &option_types[selected_index] {
                OptionType::Back => {
                    // Clear the previous selection output (1 line)
                    Self::clear_previous_lines(1);
                    nav_stack.pop();
                }
                OptionType::Category(category_id) => {
                    // Clear the previous selection output (1 line)
                    Self::clear_previous_lines(1);
                    nav_stack.push(category_id.clone());
                }
                OptionType::Template(pack, template) => {
                    // Don't clear for template selection - this is the final choice
                    return Ok((pack.clone(), template.clone()));
                }
            }
        }
    }

    /// Create a project programmatically without interactive prompts
    /// Used by ProjectGroupHandler to create projects defined in spec.projects
    #[allow(clippy::too_many_arguments)]
    pub fn create_project_non_interactive(
        ctx: &crate::context::Context,
        project_name: &str,
        template_pack_name: &str,
        template_name: &str,
        environment_name: &str,
        inputs: &std::collections::HashMap<String, serde_json::Value>,
        use_all_defaults: bool,
        reference_projects: &[crate::template::metadata::TemplateReferenceProject],
        template_packs_paths: Option<&str>,
        executor_override: Option<&crate::template::metadata::ExecutorConfigOverride>,
        plugin_configs: Option<
            &HashMap<String, crate::commands::project_group::PreConfiguredPluginData>,
        >,
    ) -> Result<()> {
        use crate::template::renderer::TemplateRenderer;

        // Step 1: Find infrastructure
        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required")?;

        // Step 2: Discover template packs
        let flag_paths: Vec<String> = if let Some(paths) = template_packs_paths {
            crate::template::discovery::parse_colon_separated_paths(paths)
        } else {
            vec![]
        };

        let env_paths: Vec<String> = std::env::var("PMP_TEMPLATE_PACKS_PATHS")
            .ok()
            .map(|p| crate::template::discovery::parse_colon_separated_paths(&p))
            .unwrap_or_default();

        let mut all_paths = flag_paths;
        all_paths.extend(env_paths);
        let custom_paths: Vec<&str> = all_paths.iter().map(|s| s.as_str()).collect();

        let all_template_packs = TemplateDiscovery::discover_template_packs_with_custom_paths(
            &*ctx.fs,
            &*ctx.output,
            &custom_paths,
        )?;

        // Step 3: Find the template pack
        let template_pack = all_template_packs
            .iter()
            .find(|pack| {
                let pack_name = pack.resource.metadata.name.to_lowercase().replace(' ', "-");
                pack_name == template_pack_name.to_lowercase()
                    || pack.resource.metadata.name.to_lowercase()
                        == template_pack_name.to_lowercase()
            })
            .context(format!("Template pack '{}' not found", template_pack_name))?;

        // Step 4: Discover templates in the pack and find the template
        let templates = TemplateDiscovery::discover_templates_in_pack(
            &*ctx.fs,
            &*ctx.output,
            &template_pack.path,
        )?;

        let template = templates
            .iter()
            .find(|t| t.resource.metadata.name.to_lowercase() == template_name.to_lowercase())
            .context(format!(
                "Template '{}' not found in pack '{}'",
                template_name, template_pack_name
            ))?;

        // Step 4.5: Resolve template inheritance (if extends is set)
        let (template, resolved_base_paths) = if template.resource.spec.extends.is_some() {
            ctx.output
                .dimmed("Resolving template inheritance chain...");

            let resolved = TemplateResolver::resolve(
                &*ctx.fs,
                &*ctx.output,
                template,
                template_pack,
                &all_template_packs,
            )
            .context("Failed to resolve template inheritance")?;

            // Show inheritance chain
            if resolved.inheritance_chain.len() > 1 {
                ctx.output.key_value(
                    "Inheritance",
                    &resolved.inheritance_chain.join(" -> "),
                );
            }

            // Create a new TemplateInfo with the merged resource
            let merged_template = TemplateInfo {
                resource: resolved.resource,
                path: resolved.path.clone(),
                version: resolved.version,
            };

            (merged_template, resolved.base_paths)
        } else {
            // No inheritance, use template as-is
            (template.clone(), vec![template.path.clone()])
        };

        // Step 5: Build inputs - use provided inputs and fill with defaults
        let mut final_inputs: std::collections::HashMap<String, serde_json::Value> = inputs.clone();

        // Add internal variables for template rendering
        final_inputs.insert(
            "_name".to_string(),
            serde_json::Value::String(project_name.to_string()),
        );
        final_inputs.insert(
            "_project_name_underscores".to_string(),
            serde_json::Value::String(project_name.replace('-', "_")),
        );
        final_inputs.insert(
            "_project_name_hyphens".to_string(),
            serde_json::Value::String(project_name.replace('_', "-")),
        );
        final_inputs.insert(
            "_environment".to_string(),
            serde_json::Value::String(environment_name.to_string()),
        );
        final_inputs.insert(
            "_environment_name".to_string(),
            serde_json::Value::String(environment_name.to_string()),
        );
        final_inputs.insert(
            "_resource_api_version".to_string(),
            serde_json::Value::String(template.resource.spec.api_version.clone()),
        );
        final_inputs.insert(
            "_resource_kind".to_string(),
            serde_json::Value::String(template.resource.spec.kind.clone()),
        );

        // Fill in defaults for any missing inputs
        for input_def in &template.resource.spec.inputs {
            if !final_inputs.contains_key(&input_def.name) && use_all_defaults {
                // Use default value if available
                if let Some(default) = &input_def.default {
                    final_inputs.insert(input_def.name.clone(), default.clone());
                }
            }
        }

        // Step 5.5: Process installed plugins from template spec
        let mut collected_plugins = Vec::new();

        if let Some(plugins_config) = &template.resource.spec.plugins
            && !plugins_config.installed.is_empty()
        {
            // Discover existing projects (needed for plugins that require reference projects)
            let discovered_projects = CollectionDiscovery::discover_projects(
                &*ctx.fs,
                &*ctx.output,
                &infrastructure_root,
            )?;

            for installed_plugin in &plugins_config.installed {
                // In non-interactive mode, force use of defaults by setting disable_user_input_override
                let mut plugin_config = installed_plugin.clone();
                plugin_config.disable_user_input_override = true;

                // Get pre-configured data for this plugin if available
                let preconfig =
                    plugin_configs.and_then(|configs| configs.get(&installed_plugin.plugin_name));

                if let Some(plugin_info) = Self::collect_plugin_info(
                    ctx,
                    &plugin_config,
                    &all_template_packs,
                    &discovered_projects,
                    &infrastructure_root,
                    project_name,
                    environment_name,
                    preconfig,
                )? {
                    collected_plugins.push(plugin_info);
                }
            }
        }

        // Step 6: Determine project paths
        // Project folder uses the original project name (preserving hyphens)
        let project_root = infrastructure_root.join("projects").join(project_name);
        let environment_path = project_root.join("environments").join(environment_name);

        // Step 7: Create directory structure
        ctx.fs.create_dir_all(&environment_path)?;

        // Step 8: Render collected plugins
        let added_plugins = if !collected_plugins.is_empty() {
            Self::render_collected_plugins(
                ctx,
                collected_plugins,
                &environment_path,
                &template.resource.spec.api_version,
                &template.resource.spec.kind,
                project_name,
                environment_name,
            )?
        } else {
            Vec::new()
        };

        // Step 9: Add _plugins to inputs for template rendering
        if !added_plugins.is_empty() {
            let plugins_data = serde_json::json!({
                "added": added_plugins
            });
            final_inputs.insert("_plugins".to_string(), plugins_data);
        }

        // Step 10: Render template files (with inheritance support)
        // Render from all base paths in order (base first, child last)
        // Child files overwrite base files with the same name
        let renderer = TemplateRenderer::new_with_partials(&*ctx.fs, Some(&template_pack.path))
            .context("Failed to initialize template renderer with partials")?;

        for base_path in &resolved_base_paths {
            if ctx.fs.exists(base_path) {
                renderer
                    .render_template(
                        ctx,
                        base_path,
                        environment_path.as_path(),
                        &final_inputs,
                        None,
                    )
                    .context("Failed to render template")?;
            }
        }

        // Step 11: Generate _common.tf if needed (only opentofu executor does this)
        let template_executor_name = template.resource.spec.executor.name();
        if let Some(executor_config) = &infrastructure.spec.executor
            && !executor_config.config.is_empty()
        {
            let executor: Box<dyn crate::executor::Executor> = match template_executor_name {
                "opentofu" => Box::new(crate::executor::OpenTofuExecutor::new()),
                _ => Box::new(crate::executor::NoneExecutor::new()),
            };

            let metadata = crate::executor::ProjectMetadata {
                api_version: &template.resource.spec.api_version,
                kind: &template.resource.spec.kind,
                environment: environment_name,
                project_name,
            };

            executor
                .generate_common_file(
                    ctx,
                    &environment_path,
                    &executor_config.config,
                    &metadata,
                    Some(&added_plugins),
                    reference_projects,
                    &std::collections::HashMap::new(), // No secrets in predefined project mode
                    infrastructure.spec.secrets.as_ref(),
                )
                .context("Failed to generate common file")?;
        }

        // Step 12: Generate .pmp.project.yaml
        Self::generate_project_identifier_yaml(
            ctx,
            &project_root,
            project_name,
            None, // No description
            &template.resource.metadata.labels,
        )?;

        // Step 13: Generate .pmp.environment.yaml
        let template_version = template
            .version
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "0.0.1".to_string());

        Self::generate_project_environment_yaml(
            ctx,
            &environment_path,
            environment_name,
            project_name,
            &template.resource,
            &final_inputs,
            &std::collections::HashMap::new(), // No secrets in predefined project mode
            template_pack_name,
            template_name,
            &template_version,
            reference_projects,
            &added_plugins,
            &[], // No project dependencies (the caller handles this)
            executor_override,
        )?;

        ctx.output.success(&format!(
            "Created project '{}' in environment '{}'",
            project_name, environment_name
        ));

        Ok(())
    }

    /// Generate the .pmp.project.yaml file for the project (identifier only, no spec)
    fn generate_project_identifier_yaml(
        ctx: &crate::context::Context,
        project_root: &std::path::Path,
        project_name: &str,
        description: Option<&str>,
        template_labels: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        use crate::template::metadata::{ProjectMetadata, ProjectResource};

        // Create ProjectResource structure without spec
        let project = ProjectResource {
            api_version: "pmp.io/v1".to_string(),
            kind: "Project".to_string(),
            metadata: ProjectMetadata {
                name: project_name.to_string(),
                description: description.map(|s| s.to_string()),
                labels: template_labels.clone(), // Propagate from template
            },
            spec: None,
        };

        // Serialize to YAML
        let yaml_content = serde_yaml::to_string(&project)
            .context("Failed to serialize project identifier to YAML")?;

        // Write to .pmp.project.yaml file
        let pmp_yaml_path = project_root.join(".pmp.project.yaml");
        ctx.fs
            .write(&pmp_yaml_path, &yaml_content)
            .with_context(|| {
                format!(
                    "Failed to write .pmp.project.yaml file: {:?}",
                    pmp_yaml_path
                )
            })?;

        ctx.output
            .dimmed(&format!("  Created: {}", pmp_yaml_path.display()));

        Ok(())
    }

    /// Generate the .pmp.environment.yaml file for the project environment (with spec)
    #[allow(clippy::too_many_arguments)]
    fn generate_project_environment_yaml(
        ctx: &crate::context::Context,
        environment_path: &std::path::Path,
        environment_name: &str,
        project_name: &str,
        template: &crate::template::metadata::TemplateResource,
        inputs: &std::collections::HashMap<String, serde_json::Value>,
        secrets: &std::collections::HashMap<String, crate::template::metadata::SecretReference>,
        template_pack_name: &str,
        template_name: &str,
        template_version: &str,
        template_reference_projects: &[crate::template::metadata::TemplateReferenceProject],
        added_plugins: &[crate::template::metadata::AddedPlugin],
        project_dependencies: &[crate::template::metadata::ProjectDependency],
        executor_override: Option<&crate::template::metadata::ExecutorConfigOverride>,
    ) -> Result<()> {
        use crate::template::metadata::{
            DependencyProject, DynamicProjectEnvironmentMetadata,
            DynamicProjectEnvironmentResource, EnvironmentReference, ProjectDependency,
            ProjectPlugins, ProjectSpec, ResourceDefinition, TemplateReference,
        };

        // Copy projects from template spec
        let template_projects = template.spec.projects.clone();

        // Generate dependencies from template projects (if any)
        // Each project in spec.projects becomes a dependency
        let mut all_dependencies = project_dependencies.to_vec();
        for project_config in template_projects.projects() {
            // Add each project as a dependency with the current environment
            // Set create: true because ProjectGroup dependencies should be auto-created
            all_dependencies.push(ProjectDependency {
                project: DependencyProject {
                    name: project_config.name.clone(),
                    environments: vec![environment_name.to_string()],
                    create: true, // Auto-create dependencies from ProjectGroup
                },
            });
        }

        // Merge executor configuration with override if provided
        let base_name = template.spec.executor.name().to_string();
        let base_config = template.spec.executor.config().cloned();

        // Merge executor override if provided
        let merged_config = if let Some(override_cfg) = executor_override {
            if let Some(override_specific) = &override_cfg.config {
                // Merge commands from base and override
                let mut merged_commands = if let Some(base_cfg) = base_config {
                    base_cfg.commands.clone()
                } else {
                    std::collections::HashMap::new()
                };

                // Override/add commands from the executor override
                for (cmd_name, cmd_config) in &override_specific.commands {
                    merged_commands.insert(cmd_name.clone(), cmd_config.clone());
                }

                Some(crate::template::metadata::ExecutorSpecificConfig {
                    commands: merged_commands,
                })
            } else {
                base_config
            }
        } else {
            base_config
        };

        let merged_executor_config = crate::template::metadata::ExecutorProjectConfig {
            name: base_name,
            config: merged_config,
        };

        // Create DynamicProjectEnvironmentResource structure with apiVersion/kind from template
        let project_env = DynamicProjectEnvironmentResource {
            api_version: template.spec.api_version.clone(),
            kind: template.spec.kind.clone(),
            metadata: DynamicProjectEnvironmentMetadata {
                name: project_name.to_string(),
                environment_name: environment_name.to_string(),
                description: inputs
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                labels: template.metadata.labels.clone(), // Propagate labels from template
                created_at: Some(chrono::Utc::now()), // Set creation timestamp for TTL calculations
            },
            spec: ProjectSpec {
                resource: ResourceDefinition {
                    api_version: template.spec.api_version.clone(),
                    kind: template.spec.kind.clone(),
                },
                executor: merged_executor_config,
                inputs: inputs.clone(),
                secrets: secrets.clone(),
                custom: None, // Templates no longer have custom field
                plugins: if !added_plugins.is_empty() {
                    Some(ProjectPlugins {
                        added: added_plugins.to_vec(),
                    })
                } else {
                    None
                },
                template: Some(TemplateReference {
                    template_pack_name: template_pack_name.to_string(),
                    name: template_name.to_string(),
                    version: template_version.to_string(),
                }),
                environment: Some(EnvironmentReference {
                    name: environment_name.to_string(),
                }),
                template_reference_projects: template_reference_projects.to_vec(),
                dependencies: all_dependencies,
                projects: template_projects,
                hooks: template.spec.hooks.clone(), // Copy hooks from template
                time_limit: None, // Time limit can be added manually to environment files
            },
        };

        // Serialize to YAML
        let yaml_content = serde_yaml::to_string(&project_env)
            .context("Failed to serialize project environment to YAML")?;

        // Write to .pmp.environment.yaml file
        let pmp_env_yaml_path = environment_path.join(".pmp.environment.yaml");
        ctx.fs
            .write(&pmp_env_yaml_path, &yaml_content)
            .with_context(|| {
                format!(
                    "Failed to write .pmp.environment.yaml file: {:?}",
                    pmp_env_yaml_path
                )
            })?;

        ctx.output
            .dimmed(&format!("  Created: {}", pmp_env_yaml_path.display()));

        Ok(())
    }

    // ============================================================================
    // Secrets Management Functions
    // ============================================================================

    /// Collect secret references for inputs that have secret_manager.enabled = true
    fn collect_secret_inputs(
        ctx: &crate::context::Context,
        inputs_spec: &[crate::template::metadata::InputDefinition],
        secrets_config: Option<&crate::template::metadata::SecretsConfig>,
    ) -> Result<HashMap<String, crate::template::metadata::SecretReference>> {
        use crate::secrets::SecretsProviderRegistry;

        let mut secrets = HashMap::new();

        // Check if any inputs have secret_manager enabled
        let secret_inputs: Vec<_> = inputs_spec
            .iter()
            .filter(|input| {
                input
                    .secret_manager
                    .as_ref()
                    .map(|sm| sm.enabled)
                    .unwrap_or(false)
            })
            .collect();

        if secret_inputs.is_empty() {
            return Ok(secrets);
        }

        // Verify secrets config is available
        let secrets_cfg = secrets_config.ok_or_else(|| {
            anyhow::anyhow!(
                "Template has inputs with secret_manager enabled, but no secrets configuration found in infrastructure.\n\
                Please add a 'secrets' section to your .pmp.infrastructure.yaml"
            )
        })?;

        if secrets_cfg.managers.is_empty() {
            anyhow::bail!(
                "Template has inputs with secret_manager enabled, but no secret managers are configured.\n\
                Please add managers to the 'secrets' section in your .pmp.infrastructure.yaml"
            );
        }

        ctx.output.blank();
        ctx.output.subsection("Secret Manager Configuration");

        let registry = SecretsProviderRegistry::new();

        for input_def in secret_inputs {
            let secret_config = input_def.secret_manager.as_ref().unwrap();
            let description = input_def
                .description
                .as_deref()
                .unwrap_or(&input_def.name);

            ctx.output.dimmed(&format!("Configuring secret: {}", description));

            let secret_ref = Self::prompt_for_secret_input(
                ctx,
                &input_def.name,
                description,
                secret_config,
                secrets_cfg,
                &registry,
            )?;

            secrets.insert(input_def.name.clone(), secret_ref);
        }

        Ok(secrets)
    }

    /// Prompt user to select a secret manager and provide secret ID
    fn prompt_for_secret_input(
        ctx: &crate::context::Context,
        input_name: &str,
        description: &str,
        secret_config: &crate::template::metadata::SecretManagerInputConfig,
        secrets_cfg: &crate::template::metadata::SecretsConfig,
        registry: &crate::secrets::SecretsProviderRegistry,
    ) -> Result<crate::template::metadata::SecretReference> {
        use crate::secrets::sanitize_name;
        use crate::template::metadata::SecretReference;

        // Step 1: Select secret manager (or use pre-configured one)
        let selected_manager = if let Some(ref manager_name) = secret_config.manager {
            // Use pre-configured manager
            secrets_cfg
                .managers
                .iter()
                .find(|m| &m.name == manager_name)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Pre-configured secret manager '{}' not found in infrastructure",
                        manager_name
                    )
                })?
        } else {
            // Prompt user to select manager
            let mut manager_names: Vec<_> = secrets_cfg.managers.iter().collect();
            manager_names.sort_by(|a, b| a.name.cmp(&b.name));

            let options: Vec<String> = manager_names
                .iter()
                .map(|m| {
                    let provider = registry.get(&m.manager_type);
                    let type_desc = provider
                        .map(|p| p.get_description().to_string())
                        .unwrap_or_else(|| m.manager_type.clone());
                    format!("{} ({})", m.name, type_desc)
                })
                .collect();

            let selected = ctx
                .input
                .select(
                    &format!("Select secret manager for '{}'", description),
                    options,
                    None,
                )
                .context("Failed to select secret manager")?;

            let manager_name = selected.split(" (").next().unwrap_or(&selected);
            secrets_cfg
                .managers
                .iter()
                .find(|m| m.name == manager_name)
                .ok_or_else(|| anyhow::anyhow!("Selected manager not found"))?
        };

        // Get the provider for validation
        let provider = registry.get(&selected_manager.manager_type).ok_or_else(|| {
            anyhow::anyhow!(
                "Unsupported secret manager type: {}",
                selected_manager.manager_type
            )
        })?;

        // Step 2: Get secret ID (or use pre-configured one)
        let secret_id = if let Some(ref id) = secret_config.secret_id {
            // Use pre-configured secret ID
            provider.validate_secret_id(id)?;
            id.clone()
        } else {
            // Prompt user for secret ID
            let prompt = provider.get_secret_id_prompt();
            let example = provider.get_secret_id_example();

            ctx.output.dimmed(&format!("  Example: {}", example));

            loop {
                let answer = ctx
                    .input
                    .text(&format!("{} for '{}'", prompt, description), None)
                    .context("Failed to get secret ID")?;

                match provider.validate_secret_id(&answer) {
                    Ok(()) => break answer,
                    Err(e) => {
                        ctx.output.warning(&format!("Invalid secret ID: {}", e));
                        continue;
                    }
                }
            }
        };

        // Generate data source name
        let data_source_name = format!("secret_{}", sanitize_name(input_name));

        Ok(SecretReference {
            manager: selected_manager.name.clone(),
            secret_id,
            data_source_name,
            secret_key: secret_config.secret_key.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use crate::executor::registry::DefaultExecutorRegistry;
    use crate::traits::user_input::MockResponse;
    use crate::traits::{
        FileSystem, MockCommandExecutor, MockFileSystem, MockOutput, MockUserInput,
    };
    use std::path::PathBuf;
    use std::sync::Arc;

    /// Helper to create a test context with mocks
    fn create_test_context(fs: Arc<MockFileSystem>, input: MockUserInput) -> Context {
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
        fs.write(&pack_path.join(".pmp.template-pack.yaml"), &pack_yaml)
            .unwrap();

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
        fs.write(&template_dir.join(".pmp.template.yaml"), &template_yaml)
            .unwrap();

        // Create src/ subdirectory with a simple template file
        // Templates can optionally have a src/ subdirectory (required if not using plugin-only templates)
        fs.write(&template_dir.join("src/main.tf.hbs"), "# Test template")
            .unwrap();

        pack_path
    }

    /// Helper to set up an infrastructure
    /// Creates the infrastructure file in the current directory
    fn setup_infrastructure(fs: &MockFileSystem, resource_kinds_yaml: &str) {
        // Convert old resource_kinds format to new category format for tests
        // This simulates what the migration logic does
        setup_infrastructure_with_categories(
            fs,
            &convert_resource_kinds_to_categories(resource_kinds_yaml),
            &extract_template_packs_config(resource_kinds_yaml),
        );
    }

    fn setup_infrastructure_with_categories(
        fs: &MockFileSystem,
        categories_yaml: &str,
        template_packs_yaml: &str,
    ) {
        let infrastructure_yaml = format!(
            r#"apiVersion: pmp.io/v1
kind: Infrastructure
metadata:
  name: Test Infrastructure
  description: Test infrastructure
spec:
  categories:
{}
  template_packs:
{}
  environments:
    dev:
      name: Development
      description: Development environment"#,
            categories_yaml, template_packs_yaml
        );
        // Create in actual current directory (for discovery to work)
        let current_dir = std::env::current_dir().unwrap();
        fs.write(
            &current_dir.join(".pmp.infrastructure.yaml"),
            &infrastructure_yaml,
        )
        .unwrap();
    }

    /// Convert old resource_kinds YAML to new categories format
    fn convert_resource_kinds_to_categories(resource_kinds_yaml: &str) -> String {
        // Parse the resource kinds and create corresponding categories
        // For simplicity in tests, we'll extract apiVersion/kind and create a category per resource

        // This is a simplified conversion for test purposes
        // The real migration logic in metadata.rs handles this more comprehensively

        if resource_kinds_yaml.contains("kind: TestResource") {
            // Check if there's no templates section - this means ALL templates are allowed (old default behavior)
            let has_templates_section = resource_kinds_yaml.contains("templates:");

            if !has_templates_section {
                // No templates section means all templates allowed - use a default set
                return r#"    - id: pmp_io_v1_testresource
      name: TestResource (pmp.io/v1)
      description: Test resource type
      templates:
        - template_pack: test-pack
          template: test-template"#
                    .to_string();
            }

            // Build template list based on what's mentioned and allowed
            let mut templates = Vec::new();

            // Check for template-a
            if resource_kinds_yaml.contains("template-a") {
                // Look for the allowed flag after template-a
                let after_template_a = resource_kinds_yaml
                    .split("template-a:")
                    .nth(1)
                    .unwrap_or("");
                let next_template = after_template_a.split("template-").next().unwrap_or("");
                if !next_template.contains("allowed: false") {
                    templates.push("template-a");
                }
            }

            // Check for template-b
            if resource_kinds_yaml.contains("template-b") {
                let after_template_b = resource_kinds_yaml
                    .split("template-b:")
                    .nth(1)
                    .unwrap_or("");
                let next_template = after_template_b.split("template-").next().unwrap_or("");
                if !next_template.contains("allowed: false") {
                    templates.push("template-b");
                }
            }

            // Check for allowed-template
            if resource_kinds_yaml.contains("allowed-template") {
                templates.push("allowed-template");
            }

            // Check for test-template
            if resource_kinds_yaml.contains("test-template") {
                let after_test_template = resource_kinds_yaml
                    .split("test-template:")
                    .nth(1)
                    .unwrap_or("");
                if !after_test_template.is_empty() {
                    let next_section = after_test_template
                        .split('\n')
                        .take(3)
                        .collect::<Vec<_>>()
                        .join("\n");
                    if !next_section.contains("allowed: false") {
                        templates.push("test-template");
                    }
                } else {
                    templates.push("test-template");
                }
            }

            // Check for blocked-template (only if explicitly allowed: true or not mentioned)
            if resource_kinds_yaml.contains("blocked-template") {
                let after_blocked = resource_kinds_yaml
                    .split("blocked-template:")
                    .nth(1)
                    .unwrap_or("");
                let next_section = after_blocked
                    .split('\n')
                    .take(3)
                    .collect::<Vec<_>>()
                    .join("\n");
                if !next_section.contains("allowed: false") {
                    templates.push("blocked-template");
                }
            }

            if templates.is_empty() {
                r#"    - id: pmp_io_v1_testresource
      name: TestResource (pmp.io/v1)
      description: Test resource type
      templates: []"#
                    .to_string()
            } else {
                let template_entries: Vec<String> = templates
                    .iter()
                    .map(|t| {
                        format!(
                            "        - template_pack: test-pack\n          template: {}",
                            t
                        )
                    })
                    .collect();

                format!(
                    r#"    - id: pmp_io_v1_testresource
      name: TestResource (pmp.io/v1)
      description: Test resource type
      templates:
{}"#,
                    template_entries.join("\n")
                )
            }
        } else if resource_kinds_yaml.contains("kind: KubernetesWorkload") {
            r#"    - id: pmp_io_v1_kubernetesworkload
      name: KubernetesWorkload (pmp.io/v1)
      description: Kubernetes workload
      templates:
        - template_pack: k8s-pack
          template: api-service"#
                .to_string()
        } else {
            "    []".to_string()
        }
    }

    /// Extract template_packs configuration from resource_kinds YAML
    fn extract_template_packs_config(resource_kinds_yaml: &str) -> String {
        // Extract template pack configurations from the old format
        // For test purposes, we'll check for specific patterns

        // Check for template-a/template-b scenario
        if resource_kinds_yaml.contains("template-a") && resource_kinds_yaml.contains("template-b")
        {
            r#"    test-pack:
      templates:
        template-a:
          defaults:
            inputs:
              setting_a:
                value: override-a
                show_as_default: false
        template-b:
          defaults: {}"#
                .to_string()
        } else if resource_kinds_yaml.contains("defaults:")
            && resource_kinds_yaml.contains("inputs:")
        {
            // Extract the actual field name and value from the YAML
            // Look for pattern: field_name:\n              value: "..."
            let inputs_section = resource_kinds_yaml.split("inputs:").nth(1).unwrap_or("");
            let lines: Vec<&str> = inputs_section.lines().collect();

            let mut field_name = String::new();
            let mut field_value = String::new();
            let mut show_as_default = String::from("true");

            for line in lines.iter() {
                let trimmed = line.trim();
                if trimmed.ends_with(':')
                    && !trimmed.starts_with("value:")
                    && !trimmed.starts_with("show_as_default:")
                    && !trimmed.is_empty()
                {
                    field_name = trimmed.trim_end_matches(':').to_string();
                }
                if trimmed.starts_with("value:") {
                    field_value = trimmed
                        .strip_prefix("value:")
                        .unwrap_or("")
                        .trim()
                        .to_string();
                }
                if trimmed.starts_with("show_as_default:") {
                    show_as_default = trimmed
                        .strip_prefix("show_as_default:")
                        .unwrap_or("true")
                        .trim()
                        .to_string();
                }
            }

            if !field_name.is_empty() && !field_value.is_empty() {
                format!(
                    r#"    test-pack:
      templates:
        test-template:
          defaults:
            inputs:
              {}:
                value: {}
                show_as_default: {}"#,
                    field_name, field_value, show_as_default
                )
            } else {
                r#"    test-pack:
      templates:
        test-template:
          defaults: {}"#
                    .to_string()
            }
        } else {
            r#"    test-pack:
      templates:
        allowed-template:
          defaults: {}
        test-template:
          defaults: {}"#
                .to_string()
        }
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
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        )); // category selection
        input.add_response(MockResponse::Select(
            "📄 allowed-template - Test template".to_string(),
        )); // template selection
        input.add_response(MockResponse::Text("test-project".to_string())); // project name
        input.add_response(MockResponse::Text("1".to_string())); // replica_count
        input.add_response(MockResponse::Confirm(false)); // apply after create

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx, None,  // output_path
            None,  // template_packs_paths
            None,  // inputs_str
            None,  // template_spec
            false, // auto_apply
            None,  // project_name
            None,  // environment_name
        );

        // Verify template was allowed and project was created
        assert!(
            result.is_ok(),
            "Create command should succeed: {:?}",
            result
        );
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
            &ctx, None,  // output_path
            None,  // template_packs_paths
            None,  // inputs_str
            None,  // template_spec
            false, // auto_apply
            None,  // project_name
            None,  // environment_name
        );

        // Should fail because no templates are available
        assert!(
            result.is_err(),
            "Create command should fail when all templates are blocked"
        );
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("No templates defined in category tree")
                || err_msg.contains("No template packs contain templates")
                || err_msg.contains("allowed in this infrastructure"),
            "Error should mention no matching templates: {}",
            err_msg
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
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        )); // category selection
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        )); // template selection
        input.add_response(MockResponse::Text("test-project".to_string())); // project name
        // User should be prompted for replica_count with default of 5
        input.add_response(MockResponse::Text("3".to_string())); // Override the default to 3
        input.add_response(MockResponse::Confirm(false)); // apply after create

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx, None,  // output_path
            None,  // template_packs_paths
            None,  // inputs_str
            None,  // template_spec
            false, // auto_apply
            None,  // project_name
            None,  // environment_name
        );

        assert!(
            result.is_ok(),
            "Create command should succeed: {:?}",
            result
        );

        // Verify the environment file was created with user's input (3, not the collection default 5)
        let current_dir = std::env::current_dir().unwrap();
        let env_file_path =
            current_dir.join("projects/test-project/environments/dev/.pmp.environment.yaml");
        assert!(
            fs.has_file(&env_file_path),
            "Environment file should be created"
        );

        let env_content = fs.get_file_contents(&env_file_path).unwrap();
        assert!(
            env_content.contains("replica_count: 3"),
            "Environment file should contain user's input value"
        );
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
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        )); // category selection
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        )); // template selection
        input.add_response(MockResponse::Text("test-project".to_string())); // project name
        // User should NOT be prompted for replica_count (it's fixed at 5)
        input.add_response(MockResponse::Text("prod".to_string())); // environment_name (still asked)
        input.add_response(MockResponse::Confirm(false)); // apply after create

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx, None,  // output_path
            None,  // template_packs_paths
            None,  // inputs_str
            None,  // template_spec
            false, // auto_apply
            None,  // project_name
            None,  // environment_name
        );

        assert!(
            result.is_ok(),
            "Create command should succeed: {:?}",
            result
        );

        // Verify the environment file was created with collection's fixed value
        let current_dir = std::env::current_dir().unwrap();
        let env_file_path =
            current_dir.join("projects/test-project/environments/dev/.pmp.environment.yaml");
        assert!(
            fs.has_file(&env_file_path),
            "Environment file should be created"
        );

        let env_content = fs.get_file_contents(&env_file_path).unwrap();
        assert!(
            env_content.contains("replica_count: 5"),
            "Environment file should contain collection's fixed value"
        );
        assert!(
            env_content.contains("environment_name: prod"),
            "Environment file should contain user's input for other fields"
        );
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
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        )); // category selection
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        )); // template selection
        input.add_response(MockResponse::Text("test-project".to_string())); // project name
        input.add_response(MockResponse::Text("2".to_string())); // replica_count
        input.add_response(MockResponse::Confirm(false)); // apply after create

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx, None,  // output_path
            None,  // template_packs_paths
            None,  // inputs_str
            None,  // template_spec
            false, // auto_apply
            None,  // project_name
            None,  // environment_name
        );

        assert!(
            result.is_ok(),
            "Create command should succeed with old config format: {:?}",
            result
        );

        // Verify project was created
        let current_dir = std::env::current_dir().unwrap();
        let env_file_path =
            current_dir.join("projects/test-project/environments/dev/.pmp.environment.yaml");
        assert!(
            fs.has_file(&env_file_path),
            "Environment file should be created"
        );
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
        fs.write(&template_dir.join(".pmp.template.yaml"), template_yaml)
            .unwrap();
        fs.write(&template_dir.join("src/main.tf.hbs"), "# Template B")
            .unwrap();

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
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        )); // category selection
        input.add_response(MockResponse::Select(
            "📄 template-a - Test template".to_string(),
        )); // template selection
        input.add_response(MockResponse::Text("test-project".to_string())); // project name
        // setting_a should not be prompted (show_as_default: false)
        input.add_response(MockResponse::Confirm(false)); // apply after create

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx, None,  // output_path
            None,  // template_packs_paths
            None,  // inputs_str
            None,  // template_spec
            false, // auto_apply
            None,  // project_name
            None,  // environment_name
        );

        assert!(
            result.is_ok(),
            "Create command should succeed: {:?}",
            result
        );

        // Verify the environment file was created with template-a's configuration
        let current_dir = std::env::current_dir().unwrap();
        let env_file_path =
            current_dir.join("projects/test-project/environments/dev/.pmp.environment.yaml");
        assert!(
            fs.has_file(&env_file_path),
            "Environment file should be created"
        );

        let env_content = fs.get_file_contents(&env_file_path).unwrap();
        assert!(
            env_content.contains("setting_a: override-a"),
            "Environment file should contain template-a's override"
        );
    }

    // ============================================================================
    // String Interpolation Tests
    // ============================================================================

    #[test]
    fn test_string_interpolation_in_description() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Set up template pack with interpolated description
        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r#"    project_id:
      default: "default-id"
      description: "Project ID for ${var:_project_name_underscores}""#,
        );

        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource"#,
        );

        // Set up mock user input
        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        )); // category selection
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        )); // template selection
        input.add_response(MockResponse::Text("my-project".to_string())); // project name
        input.add_response(MockResponse::Text("custom-id".to_string())); // project_id (should see interpolated description)
        input.add_response(MockResponse::Confirm(false)); // apply after create

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx, None,  // output_path
            None,  // template_packs_paths
            None,  // inputs_str
            None,  // template_spec
            false, // auto_apply
            None,  // project_name
            None,  // environment_name
        );

        assert!(
            result.is_ok(),
            "Create command should succeed: {:?}",
            result
        );

        // Verify the environment file was created
        let current_dir = std::env::current_dir().unwrap();
        let env_file_path =
            current_dir.join("projects/my-project/environments/dev/.pmp.environment.yaml");
        assert!(
            fs.has_file(&env_file_path),
            "Environment file should be created"
        );

        let env_content = fs.get_file_contents(&env_file_path).unwrap();
        assert!(
            env_content.contains("project_id: custom-id"),
            "Environment file should contain user's input"
        );
    }

    #[test]
    fn test_string_interpolation_in_default_string() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Set up template pack with interpolated default value
        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r#"    project_id:
      default: "proj-${var:_project_name_underscores}"
      description: "Project ID""#,
        );

        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource"#,
        );

        // Set up mock user input
        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        )); // category selection
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        )); // template selection
        input.add_response(MockResponse::Text("my-app".to_string())); // project name
        input.add_response(MockResponse::Text("proj-my_app".to_string())); // Accept the interpolated default (underscores)
        input.add_response(MockResponse::Confirm(false)); // apply after create

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx, None,  // output_path
            None,  // template_packs_paths
            None,  // inputs_str
            None,  // template_spec
            false, // auto_apply
            None,  // project_name
            None,  // environment_name
        );

        assert!(
            result.is_ok(),
            "Create command should succeed: {:?}",
            result
        );

        // Verify the interpolated default was used
        let current_dir = std::env::current_dir().unwrap();
        let env_file_path =
            current_dir.join("projects/my-app/environments/dev/.pmp.environment.yaml");
        assert!(
            fs.has_file(&env_file_path),
            "Environment file should be created"
        );

        let env_content = fs.get_file_contents(&env_file_path).unwrap();
        assert!(
            env_content.contains("project_id: proj-my_app"),
            "Default value should be interpolated with project name (underscores)"
        );
    }

    // NOTE: Progressive interpolation tests removed because HashMap doesn't guarantee iteration order.
    // Inputs referencing other inputs may be processed in unpredictable order.
    // If ordered input processing is needed, consider using IndexMap or BTreeMap.

    #[test]
    fn test_string_interpolation_in_infrastructure_override() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Set up template pack
        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r#"    docker_image:
      default: "default/image"
      description: "Docker image""#,
        );

        // Set up infrastructure with interpolated override
        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource
      templates:
        test-template:
          template_pack_name: test-pack
          defaults:
            inputs:
              docker_image:
                value: "registry/${var:_project_name_underscores}:latest"
                show_as_default: false"#,
        );

        // Set up mock user input
        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        )); // category selection
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        )); // template selection
        input.add_response(MockResponse::Text("my-service".to_string())); // project name
        // docker_image should not be prompted (show_as_default: false)
        input.add_response(MockResponse::Confirm(false)); // apply after create

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx, None,  // output_path
            None,  // template_packs_paths
            None,  // inputs_str
            None,  // template_spec
            false, // auto_apply
            None,  // project_name
            None,  // environment_name
        );

        assert!(
            result.is_ok(),
            "Create command should succeed: {:?}",
            result
        );

        // Verify infrastructure override interpolation
        let current_dir = std::env::current_dir().unwrap();
        let env_file_path =
            current_dir.join("projects/my-service/environments/dev/.pmp.environment.yaml");
        assert!(
            fs.has_file(&env_file_path),
            "Environment file should be created"
        );

        let env_content = fs.get_file_contents(&env_file_path).unwrap();
        assert!(
            env_content.contains("docker_image: registry/my_service:latest"),
            "Infrastructure override should be interpolated with underscores"
        );
    }

    #[test]
    fn test_string_interpolation_environment_name() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Set up template pack with interpolated default value using _environment_name
        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r#"    bucket_name:
      default: "${var:_project_name_underscores}-${var:_environment_name}"
      description: "Bucket name with environment""#,
        );

        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource"#,
        );

        // Set up mock user input
        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        )); // category selection
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        )); // template selection
        input.add_response(MockResponse::Text("myapp".to_string())); // project name
        input.add_response(MockResponse::Text("myapp-dev".to_string())); // Accept the interpolated default
        input.add_response(MockResponse::Confirm(false)); // apply after create

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx, None,  // output_path
            None,  // template_packs_paths
            None,  // inputs_str
            None,  // template_spec
            false, // auto_apply
            None,  // project_name
            None,  // environment_name
        );

        assert!(
            result.is_ok(),
            "Create command should succeed: {:?}",
            result
        );

        // Verify the interpolated default was used with environment name
        let current_dir = std::env::current_dir().unwrap();
        let env_file_path =
            current_dir.join("projects/myapp/environments/dev/.pmp.environment.yaml");
        assert!(
            fs.has_file(&env_file_path),
            "Environment file should be created"
        );

        let env_content = fs.get_file_contents(&env_file_path).unwrap();
        assert!(
            env_content.contains("bucket_name: myapp-dev"),
            "Default value should be interpolated with project name and environment name. Got: {}",
            env_content
        );
    }

    #[test]
    fn test_input_type_select_with_options() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Set up template pack with Select input type
        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r#"    environment:
      type: select
      options:
        - label: "Development"
          value: "dev"
        - label: "Production"
          value: "prod"
      default: "dev"
      description: "Deployment environment""#,
        );

        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource"#,
        );

        // Set up mock user input
        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        )); // category selection
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        )); // template selection
        input.add_response(MockResponse::Text("test-project".to_string())); // project name
        input.add_response(MockResponse::Select("Production".to_string())); // environment selection (by label)
        input.add_response(MockResponse::Confirm(false)); // apply after create

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx, None,  // output_path
            None,  // template_packs_paths
            None,  // inputs_str
            None,  // template_spec
            false, // auto_apply
            None,  // project_name
            None,  // environment_name
        );

        assert!(
            result.is_ok(),
            "Create command should succeed: {:?}",
            result
        );

        // Verify select input was processed
        let current_dir = std::env::current_dir().unwrap();
        let env_file_path =
            current_dir.join("projects/test-project/environments/dev/.pmp.environment.yaml");
        assert!(
            fs.has_file(&env_file_path),
            "Environment file should be created"
        );

        let env_content = fs.get_file_contents(&env_file_path).unwrap();
        assert!(
            env_content.contains("environment: prod"),
            "Should contain selected value"
        );
    }

    #[test]
    fn test_input_type_number_with_constraints() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Set up template pack with Number input type with min/max
        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r#"    replica_count:
      type: number
      min: 1
      max: 10
      default: 3
      description: "Number of replicas""#,
        );

        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource"#,
        );

        // Set up mock user input
        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        )); // category selection
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        )); // template selection
        input.add_response(MockResponse::Text("test-project".to_string())); // project name
        input.add_response(MockResponse::Text("5".to_string())); // replica_count
        input.add_response(MockResponse::Confirm(false)); // apply after create

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx, None,  // output_path
            None,  // template_packs_paths
            None,  // inputs_str
            None,  // template_spec
            false, // auto_apply
            None,  // project_name
            None,  // environment_name
        );

        assert!(
            result.is_ok(),
            "Create command should succeed: {:?}",
            result
        );

        // Verify number input was processed
        let current_dir = std::env::current_dir().unwrap();
        let env_file_path =
            current_dir.join("projects/test-project/environments/dev/.pmp.environment.yaml");
        assert!(
            fs.has_file(&env_file_path),
            "Environment file should be created"
        );

        let env_content = fs.get_file_contents(&env_file_path).unwrap();
        assert!(
            env_content.contains("replica_count: 5"),
            "Should contain number value"
        );
    }

    #[test]
    fn test_input_type_boolean() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Set up template pack with Boolean input type
        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r#"    enable_monitoring:
      type: boolean
      default: true
      description: "Enable monitoring""#,
        );

        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource"#,
        );

        // Set up mock user input
        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        )); // category selection
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        )); // template selection
        input.add_response(MockResponse::Text("test-project".to_string())); // project name
        input.add_response(MockResponse::Select("Yes".to_string())); // enable_monitoring (boolean as select)
        input.add_response(MockResponse::Confirm(false)); // apply after create

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx, None,  // output_path
            None,  // template_packs_paths
            None,  // inputs_str
            None,  // template_spec
            false, // auto_apply
            None,  // project_name
            None,  // environment_name
        );

        assert!(
            result.is_ok(),
            "Create command should succeed: {:?}",
            result
        );

        // Verify boolean input was processed
        let current_dir = std::env::current_dir().unwrap();
        let env_file_path =
            current_dir.join("projects/test-project/environments/dev/.pmp.environment.yaml");
        assert!(
            fs.has_file(&env_file_path),
            "Environment file should be created"
        );

        let env_content = fs.get_file_contents(&env_file_path).unwrap();
        assert!(
            env_content.contains("enable_monitoring: true"),
            "Should contain boolean value"
        );
    }

    #[test]
    fn test_environment_specific_input_overrides() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Set up template pack with environment-specific overrides
        let current_dir = std::env::current_dir().unwrap();
        let pack_path = current_dir.join(".pmp/template-packs/test-pack");

        // Create template pack file
        let pack_yaml = r#"apiVersion: pmp.io/v1
kind: TemplatePack
metadata:
  name: test-pack
  description: Test template pack
spec: {}"#;
        fs.write(&pack_path.join(".pmp.template-pack.yaml"), pack_yaml)
            .unwrap();

        // Create template with environment-specific overrides
        let template_dir = pack_path.join("templates/test-template");
        let template_yaml = r#"apiVersion: pmp.io/v1
kind: Template
metadata:
  name: test-template
  description: Test template
spec:
  apiVersion: pmp.io/v1
  kind: TestResource
  executor: opentofu
  inputs:
    replica_count:
      default: 1
      description: Number of replicas
  environments:
    production:
      overrides:
        inputs:
          replica_count:
            default: 3
            description: Number of replicas (production default)"#;
        fs.write(&template_dir.join(".pmp.template.yaml"), template_yaml)
            .unwrap();
        fs.write(&template_dir.join("src/main.tf.hbs"), "# Test template")
            .unwrap();

        // Set up infrastructure with production environment
        let infrastructure_yaml = r#"apiVersion: pmp.io/v1
kind: Infrastructure
metadata:
  name: Test Infrastructure
  description: Test infrastructure
spec:
  categories:
    - id: pmp_io_v1_testresource
      name: TestResource (pmp.io/v1)
      description: Test resource type
      templates:
        - template_pack: test-pack
          template: test-template
  template_packs:
    test-pack:
      templates:
        test-template:
          defaults: {}
  environments:
    dev:
      name: Development
      description: Development environment
    production:
      name: Production
      description: Production environment"#;
        fs.write(
            &current_dir.join(".pmp.infrastructure.yaml"),
            infrastructure_yaml,
        )
        .unwrap();

        // Set up mock user input - select production environment
        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        )); // category selection
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        )); // template selection
        input.add_response(MockResponse::Select(
            "Production - Production environment".to_string(),
        )); // Select production environment
        input.add_response(MockResponse::Text("test-project".to_string())); // project name
        input.add_response(MockResponse::Text("3".to_string())); // replica_count (should default to 3 for production)
        input.add_response(MockResponse::Confirm(false)); // apply after create

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx, None,  // output_path
            None,  // template_packs_paths
            None,  // inputs_str
            None,  // template_spec
            false, // auto_apply
            None,  // project_name
            None,  // environment_name
        );

        assert!(
            result.is_ok(),
            "Create command should succeed: {:?}",
            result
        );

        // Verify environment-specific default was used
        let env_file_path =
            current_dir.join("projects/test-project/environments/production/.pmp.environment.yaml");
        assert!(
            fs.has_file(&env_file_path),
            "Environment file should be created in production directory"
        );

        let env_content = fs.get_file_contents(&env_file_path).unwrap();
        assert!(
            env_content.contains("replica_count: 3"),
            "Should use production-specific default"
        );
    }

    #[test]
    fn test_project_creation_basic_end_to_end() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Set up template pack
        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r#"    app_name:
      default: "myapp"
      description: "Application name""#,
        );

        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource"#,
        );

        // Set up mock user input
        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        )); // category selection
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        )); // template selection
        input.add_response(MockResponse::Text("test-project".to_string())); // project name
        input.add_response(MockResponse::Text("myapp".to_string())); // app_name
        input.add_response(MockResponse::Confirm(false)); // apply after create

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx, None,  // output_path
            None,  // template_packs_paths
            None,  // inputs_str
            None,  // template_spec
            false, // auto_apply
            None,  // project_name
            None,  // environment_name
        );

        assert!(
            result.is_ok(),
            "Create command should succeed: {:?}",
            result
        );

        // Verify project files were created
        let current_dir = std::env::current_dir().unwrap();
        let project_yaml_path = current_dir.join("projects/test-project/.pmp.project.yaml");
        let env_yaml_path =
            current_dir.join("projects/test-project/environments/dev/.pmp.environment.yaml");

        assert!(
            fs.has_file(&project_yaml_path),
            ".pmp.project.yaml should be created"
        );
        assert!(
            fs.has_file(&env_yaml_path),
            ".pmp.environment.yaml should be created"
        );

        let env_content = fs.get_file_contents(&env_yaml_path).unwrap();
        assert!(
            env_content.contains("app_name: myapp"),
            "Environment file should contain input values"
        );
    }

    #[test]
    fn test_helper_module_creates_files() {
        use crate::test_helpers::{create_comprehensive_template_pack, create_test_infrastructure};

        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Create comprehensive template pack
        create_comprehensive_template_pack(&fs);

        // Create infrastructure
        create_test_infrastructure(&fs, &["dev", "staging", "prod"]);

        // Verify infrastructure file was created
        let current_dir = fs.current_dir().unwrap();
        let infra_file = current_dir.join(".pmp.infrastructure.yaml");
        assert!(
            fs.has_file(&infra_file),
            "Infrastructure file should exist at {:?}",
            infra_file
        );

        // Verify template pack was created
        let template_pack_file =
            current_dir.join(".pmp/template-packs/comprehensive-pack/.pmp.template-pack.yaml");
        assert!(
            fs.has_file(&template_pack_file),
            "Template pack file should exist at {:?}",
            template_pack_file
        );

        // Verify template was created
        let template_file = current_dir.join(
            ".pmp/template-packs/comprehensive-pack/templates/full-featured-template/.pmp.template.yaml",
        );
        assert!(
            fs.has_file(&template_file),
            "Template file should exist at {:?}",
            template_file
        );

        // Verify plugin was created
        let plugin_file = current_dir.join(
            ".pmp/template-packs/comprehensive-pack/plugins/monitoring-plugin/.pmp.plugin.yaml",
        );
        assert!(
            fs.has_file(&plugin_file),
            "Plugin file should exist at {:?}",
            plugin_file
        );
    }

    #[test]
    fn test_comprehensive_template_pack_features() {
        use crate::test_helpers::{create_comprehensive_template_pack, create_test_infrastructure};

        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Create comprehensive template pack with all features
        create_comprehensive_template_pack(&fs);

        // Create infrastructure configuration
        create_test_infrastructure(&fs, &["dev", "staging", "prod"]);

        // Set up mock user input - use the simple template for this test
        // NOTE: full-featured-template has dependencies which require existing projects,
        // so we use simple-template here. The comprehensive template pack still demonstrates
        // all features through its structure.
        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 Applications - Application templates".to_string(),
        )); // category
        input.add_response(MockResponse::Select(
            "📄 simple-template - Simple template for basic tests".to_string(),
        )); // template
        input.add_response(MockResponse::Select("Dev - Dev environment".to_string())); // environment
        input.add_response(MockResponse::Text("test-app".to_string())); // project name

        // Input responses for simple template
        input.add_response(MockResponse::Text("simple-resource".to_string())); // name

        input.add_response(MockResponse::Confirm(false)); // apply after create

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx, None,  // output_path
            None,  // template_packs_paths
            None,  // inputs_str
            None,  // template_spec
            false, // auto_apply
            None,  // project_name
            None,  // environment_name
        );

        // Verify command succeeded
        assert!(
            result.is_ok(),
            "Create command should succeed with comprehensive template: {:?}",
            result
        );

        // Verify project files were created
        let current_dir = fs.current_dir().unwrap();
        let project_yaml_path = current_dir.join("projects/test-app/.pmp.project.yaml");
        let env_yaml_path =
            current_dir.join("projects/test-app/environments/dev/.pmp.environment.yaml");

        assert!(
            fs.has_file(&project_yaml_path),
            ".pmp.project.yaml should be created"
        );
        assert!(
            fs.has_file(&env_yaml_path),
            ".pmp.environment.yaml should be created"
        );

        // Verify template was rendered
        let simple_tf_path = current_dir.join("projects/test-app/environments/dev/simple.tf");
        assert!(
            fs.has_file(&simple_tf_path),
            "simple.tf should be created from template"
        );

        let simple_tf_content = fs.get_file_contents(&simple_tf_path).unwrap();
        assert!(
            simple_tf_content.contains("Simple resource: simple-resource"),
            "Template should be rendered with name variable"
        );

        // Verify environment file contains input
        let env_content = fs.get_file_contents(&env_yaml_path).unwrap();
        assert!(
            env_content.contains("name: simple-resource"),
            "Environment should contain name input"
        );

        // Verify the comprehensive template pack structure exists (even if not used in this test)
        let full_template_path = current_dir.join(
            ".pmp/template-packs/comprehensive-pack/templates/full-featured-template/.pmp.template.yaml"
        );
        assert!(
            fs.has_file(&full_template_path),
            "Full-featured template should exist in the template pack"
        );

        let full_template_content = fs.get_file_contents(&full_template_path).unwrap();
        assert!(
            full_template_content.contains("type: number"),
            "Full template should have number input type"
        );
        assert!(
            full_template_content.contains("type: boolean"),
            "Full template should have boolean input type"
        );
        assert!(
            full_template_content.contains("type: select"),
            "Full template should have select input type"
        );
        assert!(
            full_template_content.contains("type: project_reference"),
            "Full template should have project_reference input type"
        );
        assert!(
            full_template_content.contains("type: projects_reference"),
            "Full template should have projects_reference input type"
        );
        assert!(
            full_template_content.contains("show_if"),
            "Full template should have conditional inputs"
        );
        assert!(
            full_template_content.contains("environments:"),
            "Full template should have environment-specific inputs"
        );

        // Verify plugins exist
        let monitoring_plugin_path = current_dir.join(
            ".pmp/template-packs/comprehensive-pack/plugins/monitoring-plugin/.pmp.plugin.yaml",
        );
        assert!(
            fs.has_file(&monitoring_plugin_path),
            "Monitoring plugin should exist"
        );

        let backup_plugin_path = current_dir
            .join(".pmp/template-packs/comprehensive-pack/plugins/backup-plugin/.pmp.plugin.yaml");
        assert!(
            fs.has_file(&backup_plugin_path),
            "Backup plugin should exist"
        );

        let backup_plugin_content = fs.get_file_contents(&backup_plugin_path).unwrap();
        assert!(
            backup_plugin_content.contains("dependencies:"),
            "Backup plugin should have dependencies"
        );
    }

    #[test]
    fn test_create_project_with_opentofu_and_plugins() {
        use crate::test_helpers::{create_opentofu_infrastructure, create_opentofu_template_pack};

        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Create OpenTofu template pack
        create_opentofu_template_pack(&fs);

        // Create infrastructure configuration
        create_opentofu_infrastructure(&fs, &["dev", "staging", "prod"]);

        // Set up mock user input for project creation
        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 Web Applications - Web application templates".to_string(),
        )); // category
        input.add_response(MockResponse::Select(
            "📄 webapp - Web application with monitoring".to_string(),
        )); // template
        input.add_response(MockResponse::Select("Dev - Dev environment".to_string())); // environment
        input.add_response(MockResponse::Text("my-webapp".to_string())); // project name

        // Pre-installed monitoring plugin inputs (order=50, comes first)
        input.add_response(MockResponse::Confirm(false)); // "Do you want to customize inputs for this plugin?" - use defaults

        // Template inputs (order=100, comes second)
        input.add_response(MockResponse::Text("awesome-app".to_string())); // app_name
        input.add_response(MockResponse::Text("3000".to_string())); // port
        input.add_response(MockResponse::Select("Yes".to_string())); // enable_tls (boolean type is implemented as Yes/No select)
        input.add_response(MockResponse::Select("Production".to_string())); // environment_type - select by LABEL, not value

        input.add_response(MockResponse::Confirm(false)); // apply after create

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(
            &ctx, None,  // output_path
            None,  // template_packs_paths
            None,  // inputs_str
            None,  // template_spec
            false, // auto_apply
            None,  // project_name
            None,  // environment_name
        );

        // Verify command succeeded
        assert!(
            result.is_ok(),
            "Create command should succeed: {:?}",
            result
        );

        // Verify project structure
        let current_dir = fs.current_dir().unwrap();
        let project_path = current_dir.join("projects/my-webapp");
        let env_path = project_path.join("environments/dev");

        // Check .pmp.project.yaml exists
        assert!(
            fs.has_file(&project_path.join(".pmp.project.yaml")),
            "Project YAML should exist"
        );

        // Check .pmp.environment.yaml exists
        let env_yaml_path = env_path.join(".pmp.environment.yaml");
        assert!(fs.has_file(&env_yaml_path), "Environment YAML should exist");

        // Verify environment YAML contains our inputs
        let env_content = fs.get_file_contents(&env_yaml_path).unwrap();
        assert!(
            env_content.contains("app_name: awesome-app"),
            "Environment YAML should contain app_name"
        );
        assert!(
            env_content.contains("port: 3000"),
            "Environment YAML should contain port"
        );
        assert!(
            env_content.contains("enable_tls: true"),
            "Environment YAML should contain enable_tls"
        );
        assert!(
            env_content.contains("environment_type: production"),
            "Environment YAML should contain environment_type"
        );

        // Check main.tf was generated
        let main_tf_path = env_path.join("main.tf");
        assert!(fs.has_file(&main_tf_path), "main.tf should exist");

        let main_tf_content = fs.get_file_contents(&main_tf_path).unwrap();
        assert!(
            main_tf_content.contains("# Web Application: awesome-app"),
            "main.tf should contain app name"
        );
        assert!(
            main_tf_content.contains("# Port: 3000"),
            "main.tf should contain port"
        );
        assert!(
            main_tf_content.contains("# TLS: true"),
            "main.tf should contain TLS setting"
        );
        assert!(
            main_tf_content.contains("# Environment: production"),
            "main.tf should contain environment type"
        );
        assert!(
            main_tf_content.contains("container_port = 3000"),
            "main.tf should use port variable"
        );
        assert!(
            main_tf_content.contains("type = \"LoadBalancer\""),
            "main.tf should use LoadBalancer for production"
        );

        // Check _common.tf was generated with S3 backend
        let common_tf_path = env_path.join("_common.tf");
        assert!(fs.has_file(&common_tf_path), "_common.tf should exist");

        let common_tf_content = fs.get_file_contents(&common_tf_path).unwrap();
        assert!(
            common_tf_content.contains("backend \"s3\""),
            "_common.tf should contain S3 backend configuration"
        );
        assert!(
            common_tf_content.contains("bucket = \"my-terraform-state\""),
            "_common.tf should contain bucket name"
        );
        assert!(
            common_tf_content.contains("key = \"my-webapp/dev/terraform.tfstate\""),
            "_common.tf should contain interpolated key with project and environment"
        );
        assert!(
            common_tf_content.contains("region = \"us-east-1\""),
            "_common.tf should contain region"
        );
        assert!(
            common_tf_content.contains("encrypt = true"),
            "_common.tf should contain encrypt setting"
        );

        // Check monitoring plugin module was generated
        let monitoring_tf_path = env_path
            .join("modules")
            .join("opentofu-pack")
            .join("monitoring")
            .join("monitoring.tf");
        assert!(
            fs.has_file(&monitoring_tf_path),
            "monitoring.tf from pre-installed plugin should exist in modules directory"
        );

        let monitoring_content = fs.get_file_contents(&monitoring_tf_path).unwrap();
        assert!(
            monitoring_content.contains("# Metrics: true"),
            "monitoring.tf should show metrics enabled"
        );
        assert!(
            monitoring_content.contains("# Scrape interval: 30s"),
            "monitoring.tf should use plugin default scrape interval"
        );
        assert!(
            monitoring_content.contains("scrape_interval: 30s"),
            "monitoring.tf should contain scrape interval in config"
        );
        assert!(
            monitoring_content.contains("resource \"kubernetes_config_map\" \"prometheus\""),
            "monitoring.tf should contain prometheus config map"
        );
        assert!(
            monitoring_content.contains("resource \"kubernetes_deployment\" \"prometheus\""),
            "monitoring.tf should contain prometheus deployment"
        );

        // Verify logging and backup plugins were NOT installed (they're only allowed)
        assert!(
            !fs.has_file(
                &env_path
                    .join("modules")
                    .join("opentofu-pack")
                    .join("logging")
                    .join("logging.tf")
            ),
            "logging.tf should NOT exist (not pre-installed)"
        );
        assert!(
            !fs.has_file(
                &env_path
                    .join("modules")
                    .join("opentofu-pack")
                    .join("backup")
                    .join("backup.tf")
            ),
            "backup.tf should NOT exist (not pre-installed)"
        );
    }

    #[test]
    fn test_update_project_change_inputs() {
        use crate::commands::update::UpdateCommand;
        use crate::test_helpers::{create_opentofu_infrastructure, create_opentofu_template_pack};

        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Create OpenTofu template pack and infrastructure
        create_opentofu_template_pack(&fs);
        create_opentofu_infrastructure(&fs, &["dev", "staging", "prod"]);

        // === STEP 1: Create initial project ===
        let input_create = MockUserInput::new();
        input_create.add_response(MockResponse::Select(
            "📁 Web Applications - Web application templates".to_string(),
        ));
        input_create.add_response(MockResponse::Select(
            "📄 webapp - Web application with monitoring".to_string(),
        ));
        input_create.add_response(MockResponse::Select("Dev - Dev environment".to_string()));
        input_create.add_response(MockResponse::Text("my-webapp".to_string()));
        // Plugin inputs first (order=50)
        input_create.add_response(MockResponse::Confirm(true)); // "Do you want to customize inputs for this plugin?"
        input_create.add_response(MockResponse::Confirm(true)); // metrics_enabled
        input_create.add_response(MockResponse::Text("15s".to_string())); // scrape_interval
        // Template inputs second (order=100)
        input_create.add_response(MockResponse::Text("awesome-app".to_string())); // app_name
        input_create.add_response(MockResponse::Text("3000".to_string())); // port
        input_create.add_response(MockResponse::Select("Yes".to_string())); // enable_tls (boolean = Yes/No select)
        input_create.add_response(MockResponse::Select("Production".to_string())); // environment_type - select by LABEL
        input_create.add_response(MockResponse::Confirm(false)); // apply

        let ctx_create = create_test_context(Arc::clone(&fs), input_create);
        CreateCommand::execute(&ctx_create, None, None, None, None, false, None, None).unwrap();

        // === STEP 2: Update project with changed inputs ===
        let input_update = MockUserInput::new();
        // First, select "Update the project" action
        input_update.add_response(MockResponse::Select("Update the project".to_string()));
        // Update TEMPLATE inputs only (plugin inputs are not re-collected during update unless new plugins are added)
        input_update.add_response(MockResponse::Text("awesome-app".to_string())); // app_name
        input_update.add_response(MockResponse::Text("8080".to_string())); // port (changed from 3000)
        input_update.add_response(MockResponse::Confirm(false)); // enable_tls (changed from true to false)
        input_update.add_response(MockResponse::Select("Staging".to_string())); // environment_type (changed from Production) - select by LABEL
        input_update.add_response(MockResponse::Confirm(true)); // "Regenerate environment files with these inputs?"
        input_update.add_response(MockResponse::Confirm(false)); // "Do you want to execute 'apply' now?" - no, just regenerate files

        let ctx_update = create_test_context(Arc::clone(&fs), input_update);

        // Run update command
        let current_dir = fs.current_dir().unwrap();
        let env_path_str = current_dir.join("projects/my-webapp/environments/dev");
        let result = UpdateCommand::execute(
            &ctx_update,
            Some(env_path_str.to_str().unwrap()), // absolute path to environment
            None,                                 // template_packs_paths
            None,                                 // inputs_str
        );

        assert!(
            result.is_ok(),
            "Update command should succeed: {:?}",
            result
        );

        // === STEP 3: Verify updated files ===
        let current_dir = fs.current_dir().unwrap();
        let env_path = current_dir.join("projects/my-webapp/environments/dev");

        // Verify environment YAML was updated
        let env_yaml_path = env_path.join(".pmp.environment.yaml");
        let env_content = fs.get_file_contents(&env_yaml_path).unwrap();
        assert!(
            env_content.contains("port: 8080"),
            "Environment YAML should have updated port"
        );
        assert!(
            env_content.contains("enable_tls: false"),
            "Environment YAML should have updated enable_tls"
        );
        assert!(
            env_content.contains("environment_type: staging"),
            "Environment YAML should have updated environment_type"
        );

        // Verify main.tf was regenerated with new values
        let main_tf_path = env_path.join("main.tf");
        let main_tf_content = fs.get_file_contents(&main_tf_path).unwrap();
        assert!(
            main_tf_content.contains("# Port: 8080"),
            "main.tf should have updated port"
        );
        assert!(
            main_tf_content.contains("# TLS: false"),
            "main.tf should have updated TLS setting"
        );
        assert!(
            main_tf_content.contains("# Environment: staging"),
            "main.tf should have updated environment type"
        );
        assert!(
            main_tf_content.contains("container_port = 8080"),
            "main.tf should use updated port variable"
        );
        assert!(
            main_tf_content.contains("type = \"ClusterIP\""),
            "main.tf should use ClusterIP for staging (not production)"
        );
        assert!(
            !main_tf_content.contains("# TLS configuration"),
            "main.tf should not include TLS configuration when disabled"
        );

        // Verify monitoring.tf was regenerated but kept original plugin input values
        let monitoring_tf_path = env_path
            .join("modules")
            .join("opentofu-pack")
            .join("monitoring")
            .join("monitoring.tf");
        let monitoring_content = fs.get_file_contents(&monitoring_tf_path).unwrap();
        assert!(
            monitoring_content.contains("# Scrape interval: 15s"),
            "monitoring.tf should keep original scrape interval (15s) when only template inputs are updated"
        );
        assert!(
            monitoring_content.contains("scrape_interval: 15s"),
            "monitoring.tf should contain original interval in config"
        );

        // Verify _common.tf still exists and has correct backend config
        let common_tf_path = env_path.join("_common.tf");
        let common_tf_content = fs.get_file_contents(&common_tf_path).unwrap();
        assert!(
            common_tf_content.contains("backend \"s3\""),
            "_common.tf should still have S3 backend after update"
        );
    }

    #[test]
    fn test_update_project_add_new_plugin() {
        use crate::commands::update::UpdateCommand;
        use crate::test_helpers::{create_opentofu_infrastructure, create_opentofu_template_pack};

        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Create OpenTofu template pack and infrastructure
        create_opentofu_template_pack(&fs);
        create_opentofu_infrastructure(&fs, &["dev", "staging", "prod"]);

        // === STEP 1: Create initial project ===
        let input_create = MockUserInput::new();
        input_create.add_response(MockResponse::Select(
            "📁 Web Applications - Web application templates".to_string(),
        ));
        input_create.add_response(MockResponse::Select(
            "📄 webapp - Web application with monitoring".to_string(),
        ));
        input_create.add_response(MockResponse::Select("Dev - Dev environment".to_string()));
        input_create.add_response(MockResponse::Text("my-webapp".to_string()));
        // Plugin inputs first (order=50)
        input_create.add_response(MockResponse::Confirm(true)); // "Do you want to customize inputs for this plugin?"
        input_create.add_response(MockResponse::Confirm(true)); // metrics_enabled
        input_create.add_response(MockResponse::Text("15s".to_string())); // scrape_interval
        // Template inputs second (order=100)
        input_create.add_response(MockResponse::Text("awesome-app".to_string())); // app_name
        input_create.add_response(MockResponse::Text("3000".to_string())); // port
        input_create.add_response(MockResponse::Select("Yes".to_string())); // enable_tls (boolean = Yes/No select)
        input_create.add_response(MockResponse::Select("Development".to_string())); // environment_type - select by LABEL
        input_create.add_response(MockResponse::Confirm(false)); // apply

        let ctx_create = create_test_context(Arc::clone(&fs), input_create);
        CreateCommand::execute(&ctx_create, None, None, None, None, false, None, None).unwrap();

        // === STEP 2: Update project - add logging plugin ===
        let input_update = MockUserInput::new();
        // First, select "Add Plugin" action
        input_update.add_response(MockResponse::Select("Add Plugin".to_string()));

        // Select logging plugin to add (single select, not multi-select)
        input_update.add_response(MockResponse::Select(
            "logging - Centralized logging".to_string(),
        ));

        // Logging plugin inputs
        input_update.add_response(MockResponse::Text("debug".to_string())); // log_level - treated as text input
        input_update.add_response(MockResponse::Text("90".to_string())); // retention_days

        input_update.add_response(MockResponse::Confirm(false)); // apply

        let ctx_update = create_test_context(Arc::clone(&fs), input_update);

        // Run update command to add logging plugin
        let current_dir = fs.current_dir().unwrap();
        let env_path_str = current_dir.join("projects/my-webapp/environments/dev");
        let result = UpdateCommand::execute(
            &ctx_update,
            Some(env_path_str.to_str().unwrap()), // absolute path to environment
            None,                                 // template_packs_paths
            None,                                 // inputs_str
        );

        assert!(
            result.is_ok(),
            "Update command should succeed when adding plugin: {:?}",
            result
        );

        // === STEP 3: Verify new plugin was added ===
        let current_dir = fs.current_dir().unwrap();
        let env_path = current_dir.join("projects/my-webapp/environments/dev");

        // Verify logging.tf was created
        let logging_tf_path = env_path
            .join("modules")
            .join("opentofu-pack")
            .join("logging")
            .join("logging.tf");
        assert!(
            fs.has_file(&logging_tf_path),
            "logging.tf should be created after adding plugin"
        );

        let logging_content = fs.get_file_contents(&logging_tf_path).unwrap();
        assert!(
            logging_content.contains("# Log level: debug"),
            "logging.tf should contain configured log level"
        );
        assert!(
            logging_content.contains("# Retention: 90 days"),
            "logging.tf should contain configured retention"
        );
        assert!(
            logging_content.contains("log_level debug"),
            "logging.tf should use log level in fluent config"
        );
        assert!(
            logging_content.contains("resource \"kubernetes_config_map\" \"fluentd\""),
            "logging.tf should contain fluentd config map"
        );
        assert!(
            logging_content.contains("resource \"kubernetes_daemonset\" \"fluentd\""),
            "logging.tf should contain fluentd daemonset"
        );

        // Verify environment YAML includes logging plugin inputs
        let env_yaml_path = env_path.join(".pmp.environment.yaml");
        let env_content = fs.get_file_contents(&env_yaml_path).unwrap();
        assert!(
            env_content.contains("log_level: debug"),
            "Environment YAML should include logging plugin inputs"
        );
        assert!(
            env_content.contains("retention_days: 90"),
            "Environment YAML should include retention days"
        );

        // Verify monitoring.tf still exists (pre-installed plugin should remain)
        let monitoring_tf_path = env_path
            .join("modules")
            .join("opentofu-pack")
            .join("monitoring")
            .join("monitoring.tf");
        assert!(
            fs.has_file(&monitoring_tf_path),
            "monitoring.tf should still exist after update"
        );

        // Verify main.tf still exists with original content
        let main_tf_path = env_path.join("main.tf");
        assert!(fs.has_file(&main_tf_path), "main.tf should still exist");

        // Verify _common.tf still exists
        let common_tf_path = env_path.join("_common.tf");
        assert!(
            fs.has_file(&common_tf_path),
            "_common.tf should still exist"
        );

        // === STEP 4: Add another plugin (backup) ===
        let input_update2 = MockUserInput::new();
        // First, select "Add Plugin" action
        input_update2.add_response(MockResponse::Select("Add Plugin".to_string()));

        // Select backup plugin to add (single select, not multi-select)
        input_update2.add_response(MockResponse::Select(
            "backup - Automated backups".to_string(),
        ));

        // Backup plugin inputs
        input_update2.add_response(MockResponse::Text("0 3 * * *".to_string())); // backup_schedule
        input_update2.add_response(MockResponse::Text("14".to_string())); // backup_retention

        input_update2.add_response(MockResponse::Confirm(false)); // apply

        let ctx_update2 = create_test_context(Arc::clone(&fs), input_update2);

        let current_dir = fs.current_dir().unwrap();
        let env_path_str = current_dir.join("projects/my-webapp/environments/dev");
        let result2 = UpdateCommand::execute(
            &ctx_update2,
            Some(env_path_str.to_str().unwrap()), // absolute path to environment
            None,                                 // template_packs_paths
            None,                                 // inputs_str
        );

        assert!(
            result2.is_ok(),
            "Update command should succeed when adding second plugin: {:?}",
            result2
        );

        // Verify backup.tf was created
        let backup_tf_path = env_path
            .join("modules")
            .join("opentofu-pack")
            .join("backup")
            .join("backup.tf");
        assert!(
            fs.has_file(&backup_tf_path),
            "backup.tf should be created after adding backup plugin"
        );

        let backup_content = fs.get_file_contents(&backup_tf_path).unwrap();
        assert!(
            backup_content.contains("# Schedule: 0 3 * * *"),
            "backup.tf should contain configured schedule"
        );
        assert!(
            backup_content.contains("# Retention: 14 backups"),
            "backup.tf should contain configured retention"
        );
        assert!(
            backup_content.contains("schedule = \"0 3 * * *\""),
            "backup.tf should use schedule in cron job"
        );
        assert!(
            backup_content.contains("value = \"14\""),
            "backup.tf should use retention in env var"
        );

        // Verify all three plugin files now exist
        assert!(
            fs.has_file(&monitoring_tf_path),
            "All plugins should coexist: monitoring.tf"
        );
        assert!(
            fs.has_file(&logging_tf_path),
            "All plugins should coexist: logging.tf"
        );
        assert!(
            fs.has_file(&backup_tf_path),
            "All plugins should coexist: backup.tf"
        );
    }

    // ============================================================================
    // New Input Types Tests
    // ============================================================================

    #[test]
    fn test_yaml_deserialization_duration() {
        // Test that Duration type can be deserialized from YAML
        let yaml = r##"
name: test_duration
type: duration
description: Test duration
min_seconds: 60
max_seconds: 86400
default: "1h"
"##;

        let result: Result<crate::template::metadata::InputDefinition, _> =
            serde_yaml::from_str(yaml);
        assert!(
            result.is_ok(),
            "Failed to deserialize duration input: {:?}",
            result
        );

        let input_def = result.unwrap();
        assert_eq!(input_def.name, "test_duration");
        assert!(input_def.input_type.is_some(), "Input type should be Some");

        match input_def.input_type.unwrap() {
            crate::template::metadata::InputType::Duration {
                min_seconds,
                max_seconds,
            } => {
                assert_eq!(min_seconds, Some(60));
                assert_eq!(max_seconds, Some(86400));
            }
            other => panic!("Expected Duration, got {:?}", other),
        }
    }

    #[test]
    fn test_input_type_object() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Set up template pack with Object input type
        let current_dir = std::env::current_dir().unwrap();
        let pack_path = current_dir.join(".pmp/template-packs/test-pack");

        let pack_yaml = r#"apiVersion: pmp.io/v1
kind: TemplatePack
metadata:
  name: test-pack
spec: {}"#;
        fs.write(&pack_path.join(".pmp.template-pack.yaml"), pack_yaml)
            .unwrap();

        let template_dir = pack_path.join("templates/test-template");
        let template_yaml = r#"apiVersion: pmp.io/v1
kind: Template
metadata:
  name: test-template
  description: Test template
spec:
  apiVersion: pmp.io/v1
  kind: TestResource
  executor: opentofu
  inputs:
    - name: database_config
      type: object
      description: Database configuration
      fields:
        - name: host
          type: string
          description: Database host
          default: "localhost"
        - name: port
          type: number
          description: Database port
          default: 5432
        - name: ssl_enabled
          type: boolean
          description: Enable SSL
          default: true"#;
        fs.write(&template_dir.join(".pmp.template.yaml"), template_yaml)
            .unwrap();

        let template_content = r#"# Database Configuration
# Host: {{database_config.host}}
# Port: {{database_config.port}}
# SSL: {{database_config.ssl_enabled}}

resource "database" "main" {
  host        = "{{database_config.host}}"
  port        = {{database_config.port}}
  ssl_enabled = {{bool database_config.ssl_enabled}}
}"#;
        fs.write(&template_dir.join("src/main.tf.hbs"), template_content)
            .unwrap();

        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource"#,
        );

        // Set up mock user input
        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        ));
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        ));
        input.add_response(MockResponse::Text("test-project".to_string()));

        // Object field inputs
        input.add_response(MockResponse::Text("db.example.com".to_string())); // host
        input.add_response(MockResponse::Text("5432".to_string())); // port
        input.add_response(MockResponse::Select("Yes".to_string())); // ssl_enabled

        input.add_response(MockResponse::Confirm(false)); // apply after create

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(&ctx, None, None, None, None, false, None, None);

        assert!(
            result.is_ok(),
            "Create command should succeed: {:?}",
            result
        );

        // Verify rendered template
        let main_tf_path = current_dir.join("projects/test-project/environments/dev/main.tf");
        assert!(fs.has_file(&main_tf_path), "main.tf should be created");

        let main_tf_content = fs.get_file_contents(&main_tf_path).unwrap();
        assert!(
            main_tf_content.contains("# Host: db.example.com"),
            "Template should render object field: host"
        );
        assert!(
            main_tf_content.contains("# Port: 5432"),
            "Template should render object field: port"
        );
        assert!(
            main_tf_content.contains("# SSL: true"),
            "Template should render object field: ssl_enabled"
        );
        assert!(
            main_tf_content.contains("host        = \"db.example.com\""),
            "Template should use object field in resource"
        );
        assert!(
            main_tf_content.contains("port        = 5432"),
            "Template should use numeric object field"
        );
        assert!(
            main_tf_content.contains("ssl_enabled = true"),
            "Template should use boolean object field"
        );
    }

    #[test]
    fn test_input_type_repeatable_object() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        let current_dir = std::env::current_dir().unwrap();
        let pack_path = current_dir.join(".pmp/template-packs/test-pack");

        let pack_yaml = r#"apiVersion: pmp.io/v1
kind: TemplatePack
metadata:
  name: test-pack
spec: {}"#;
        fs.write(&pack_path.join(".pmp.template-pack.yaml"), pack_yaml)
            .unwrap();

        let template_dir = pack_path.join("templates/test-template");
        let template_yaml = r#"apiVersion: pmp.io/v1
kind: Template
metadata:
  name: test-template
  description: Test template
spec:
  apiVersion: pmp.io/v1
  kind: TestResource
  executor: opentofu
  inputs:
    - name: team_members
      type: repeatable_object
      description: Team members with roles
      min: 0
      max: 50
      add_another_prompt: "Add another team member?"
      fields:
        - name: username
          type: string
          description: Username
        - name: role
          type: select
          description: Role
          options:
            - label: "Member"
              value: "member"
            - label: "Maintainer"
              value: "maintainer"
          default: "member""#;
        fs.write(&template_dir.join(".pmp.template.yaml"), template_yaml)
            .unwrap();

        let template_content = r#"# Team Members
{{#each team_members}}
resource "team_membership" "member_{{@index}}" {
  username = "{{username}}"
  role     = "{{role}}"
}
{{/each}}"#;
        fs.write(&template_dir.join("src/main.tf.hbs"), template_content)
            .unwrap();

        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource"#,
        );

        // Set up mock user input
        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        ));
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        ));
        input.add_response(MockResponse::Text("test-project".to_string()));

        // First team member (when min=0 and empty, first prompt allows Add or Done)
        input.add_response(MockResponse::Select("Add new team member".to_string()));
        input.add_response(MockResponse::Text("alice".to_string())); // username
        input.add_response(MockResponse::Select("Maintainer".to_string())); // role

        // Second team member
        input.add_response(MockResponse::Select("Add new team member".to_string()));
        input.add_response(MockResponse::Text("bob".to_string())); // username
        input.add_response(MockResponse::Select("Member".to_string())); // role

        // Third team member
        input.add_response(MockResponse::Select("Add new team member".to_string()));
        input.add_response(MockResponse::Text("charlie".to_string())); // username
        input.add_response(MockResponse::Select("Member".to_string())); // role

        // Stop adding members
        input.add_response(MockResponse::Select("Done".to_string()));

        input.add_response(MockResponse::Confirm(false)); // apply after create

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run create command
        let result = CreateCommand::execute(&ctx, None, None, None, None, false, None, None);

        assert!(
            result.is_ok(),
            "Create command should succeed: {:?}",
            result
        );

        // Verify rendered template
        let main_tf_path = current_dir.join("projects/test-project/environments/dev/main.tf");
        assert!(fs.has_file(&main_tf_path), "main.tf should be created");

        let main_tf_content = fs.get_file_contents(&main_tf_path).unwrap();

        // Verify all three team members are rendered
        assert!(
            main_tf_content.contains("resource \"team_membership\" \"member_0\""),
            "Template should render first member"
        );
        assert!(
            main_tf_content.contains("username = \"alice\""),
            "Template should render alice"
        );
        assert!(
            main_tf_content.contains("role     = \"maintainer\""),
            "Template should render maintainer role"
        );

        assert!(
            main_tf_content.contains("resource \"team_membership\" \"member_1\""),
            "Template should render second member"
        );
        assert!(
            main_tf_content.contains("username = \"bob\""),
            "Template should render bob"
        );

        assert!(
            main_tf_content.contains("resource \"team_membership\" \"member_2\""),
            "Template should render third member"
        );
        assert!(
            main_tf_content.contains("username = \"charlie\""),
            "Template should render charlie"
        );

        // Count member blocks
        let member_count = main_tf_content
            .matches("resource \"team_membership\"")
            .count();
        assert_eq!(member_count, 3, "Should have exactly 3 team members");
    }

    #[test]
    fn test_input_type_color() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r##"    brand_color:
      type: color
      description: Brand color
      allow_alpha: true
      default: "#3B82F6""##,
        );

        // Add template file
        let current_dir = std::env::current_dir().unwrap();
        let template_content = r#"# Brand Color: {{brand_color}}

resource "theme" "main" {
  primary_color = "{{brand_color}}"
}"#;
        fs.write(
            &current_dir
                .join(".pmp/template-packs/test-pack/templates/test-template/src/main.tf.hbs"),
            template_content,
        )
        .unwrap();

        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource"#,
        );

        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        ));
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        ));
        input.add_response(MockResponse::Text("test-project".to_string()));
        input.add_response(MockResponse::Text("#FF5733".to_string())); // brand_color
        input.add_response(MockResponse::Confirm(false));

        let ctx = create_test_context(Arc::clone(&fs), input);

        let result = CreateCommand::execute(&ctx, None, None, None, None, false, None, None);

        assert!(result.is_ok(), "Create should succeed: {:?}", result);

        let main_tf_path = current_dir.join("projects/test-project/environments/dev/main.tf");
        let main_tf_content = fs.get_file_contents(&main_tf_path).unwrap();

        assert!(
            main_tf_content.contains("# Brand Color: #FF5733"),
            "Template should render color value"
        );
        assert!(
            main_tf_content.contains("primary_color = \"#FF5733\""),
            "Template should use color in resource"
        );
    }

    #[test]
    fn test_input_type_duration() {
        let fs = Arc::new(MockFileSystem::new());

        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r##"    cache_ttl:
      type: duration
      description: Cache TTL
      min_seconds: 60
      max_seconds: 86400
      default: "1h""##,
        );

        let current_dir = std::env::current_dir().unwrap();
        let template_content = r#"# Cache TTL: {{cache_ttl}} seconds

resource "cache" "main" {
  ttl_seconds = {{cache_ttl}}
}"#;
        fs.write(
            &current_dir
                .join(".pmp/template-packs/test-pack/templates/test-template/src/main.tf.hbs"),
            template_content,
        )
        .unwrap();

        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource"#,
        );

        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        ));
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        ));
        input.add_response(MockResponse::Text("test-project".to_string()));
        input.add_response(MockResponse::Text("2h30m".to_string())); // cache_ttl = 9000 seconds
        input.add_response(MockResponse::Confirm(false));

        let ctx = create_test_context(Arc::clone(&fs), input);
        let result = CreateCommand::execute(&ctx, None, None, None, None, false, None, None);

        assert!(result.is_ok(), "Create should succeed: {:?}", result);

        // Check environment file first
        let env_yaml_path =
            current_dir.join("projects/test-project/environments/dev/.pmp.environment.yaml");
        let env_content = fs.get_file_contents(&env_yaml_path).unwrap();
        assert!(
            env_content.contains("cache_ttl: 9000"),
            "Environment file should contain parsed duration in seconds. Actual:\n{}",
            env_content
        );

        let main_tf_path = current_dir.join("projects/test-project/environments/dev/main.tf");
        let main_tf_content = fs.get_file_contents(&main_tf_path).unwrap();

        assert!(
            main_tf_content.contains("# Cache TTL: 9000 seconds"),
            "Template should render duration as seconds. Actual content:\n{}",
            main_tf_content
        );
        assert!(
            main_tf_content.contains("ttl_seconds = 9000"),
            "Template should use duration value. Actual content:\n{}",
            main_tf_content
        );
    }

    #[test]
    fn test_input_type_cron() {
        let fs = Arc::new(MockFileSystem::new());

        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r##"    backup_schedule:
      type: cron
      description: Backup schedule
      default: "0 2 * * *""##,
        );

        let current_dir = std::env::current_dir().unwrap();
        let template_content = r#"# Backup Schedule: {{backup_schedule}}

resource "backup" "main" {
  schedule = "{{backup_schedule}}"
}"#;
        fs.write(
            &current_dir
                .join(".pmp/template-packs/test-pack/templates/test-template/src/main.tf.hbs"),
            template_content,
        )
        .unwrap();

        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource"#,
        );

        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        ));
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        ));
        input.add_response(MockResponse::Text("test-project".to_string()));
        input.add_response(MockResponse::Text("0 3 * * *".to_string())); // backup_schedule
        input.add_response(MockResponse::Confirm(false));

        let ctx = create_test_context(Arc::clone(&fs), input);
        let result = CreateCommand::execute(&ctx, None, None, None, None, false, None, None);

        assert!(result.is_ok(), "Create should succeed: {:?}", result);

        let main_tf_path = current_dir.join("projects/test-project/environments/dev/main.tf");
        let main_tf_content = fs.get_file_contents(&main_tf_path).unwrap();

        assert!(
            main_tf_content.contains("# Backup Schedule: 0 3 * * *"),
            "Template should render cron expression"
        );
        assert!(
            main_tf_content.contains("schedule = \"0 3 * * *\""),
            "Template should use cron expression"
        );
    }

    #[test]
    fn test_input_type_keyvalue() {
        let fs = Arc::new(MockFileSystem::new());

        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r##"    labels:
      type: keyvalue
      description: Resource labels
      key_value_separator: "="
      pair_separator: ","
      min: 0
      max: 20
      default: """##,
        );

        let current_dir = std::env::current_dir().unwrap();
        let template_content = r#"# Labels
{{#each labels}}
# - {{@key}}: {{this}}
{{/each}}

resource "app" "main" {
  labels = {
{{#each labels}}
    {{@key}} = "{{this}}"
{{/each}}
  }
}"#;
        fs.write(
            &current_dir
                .join(".pmp/template-packs/test-pack/templates/test-template/src/main.tf.hbs"),
            template_content,
        )
        .unwrap();

        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource"#,
        );

        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        ));
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        ));
        input.add_response(MockResponse::Text("test-project".to_string()));
        input.add_response(MockResponse::Text(
            "env=prod,team=platform,version=1.0".to_string(),
        )); // labels
        input.add_response(MockResponse::Confirm(false));

        let ctx = create_test_context(Arc::clone(&fs), input);
        let result = CreateCommand::execute(&ctx, None, None, None, None, false, None, None);

        assert!(result.is_ok(), "Create should succeed: {:?}", result);

        let main_tf_path = current_dir.join("projects/test-project/environments/dev/main.tf");
        let main_tf_content = fs.get_file_contents(&main_tf_path).unwrap();

        assert!(
            main_tf_content.contains("# - env: prod"),
            "Template should render label: env"
        );
        assert!(
            main_tf_content.contains("# - team: platform"),
            "Template should render label: team"
        );
        assert!(
            main_tf_content.contains("# - version: 1.0"),
            "Template should render label: version"
        );
        assert!(
            main_tf_content.contains("env = \"prod\""),
            "Template should use label in resource"
        );
        assert!(
            main_tf_content.contains("team = \"platform\""),
            "Template should use team label"
        );
    }

    #[test]
    fn test_input_type_semver() {
        let fs = Arc::new(MockFileSystem::new());

        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r##"    app_version:
      type: semver
      description: Application version
      allow_prerelease: true
      allow_build: true
      default: "1.0.0""##,
        );

        let current_dir = std::env::current_dir().unwrap();
        let template_content = r#"# Application Version: {{app_version}}

resource "deployment" "main" {
  version = "{{app_version}}"
}"#;
        fs.write(
            &current_dir
                .join(".pmp/template-packs/test-pack/templates/test-template/src/main.tf.hbs"),
            template_content,
        )
        .unwrap();

        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource"#,
        );

        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        ));
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        ));
        input.add_response(MockResponse::Text("test-project".to_string()));
        input.add_response(MockResponse::Text("2.1.3-beta.1+20230615".to_string())); // app_version
        input.add_response(MockResponse::Confirm(false));

        let ctx = create_test_context(Arc::clone(&fs), input);
        let result = CreateCommand::execute(&ctx, None, None, None, None, false, None, None);

        assert!(result.is_ok(), "Create should succeed: {:?}", result);

        let main_tf_path = current_dir.join("projects/test-project/environments/dev/main.tf");
        let main_tf_content = fs.get_file_contents(&main_tf_path).unwrap();

        assert!(
            main_tf_content.contains("# Application Version: 2.1.3-beta.1+20230615"),
            "Template should render semver with prerelease and build"
        );
        assert!(
            main_tf_content.contains("version = \"2.1.3-beta.1+20230615\""),
            "Template should use semver in resource"
        );
    }

    // =====================================================================
    // COMPREHENSIVE INTEGRATION TESTS
    // Testing full project lifecycle with templates, plugins, and updates
    // =====================================================================

    #[test]
    #[ignore] // Integration test: requires template discovery to use injected mock filesystem
    fn test_create_project_with_installed_plugin_team_members() {
        let fs = Arc::new(MockFileSystem::new());
        let current_dir = std::env::current_dir().unwrap();

        // Use standard template pack setup with a plugin reference
        let pack_path = current_dir.join(".pmp/template-packs/test-pack");

        // Create template pack file
        fs.write(
            &pack_path.join(".pmp.template-pack.yaml"),
            r#"apiVersion: pmp.io/v1
kind: TemplatePack
metadata:
  name: test-pack
  description: Test template pack
spec: {}"#,
        )
        .unwrap();

        // Create template with installed plugin
        let template_dir = pack_path.join("templates/test-template");
        fs.write(
            &template_dir.join(".pmp.template.yaml"),
            r#"apiVersion: pmp.io/v1
kind: Template
metadata:
  name: test-template
  description: Test template
spec:
  apiVersion: pmp.io/v1
  kind: TestResource
  executor:
    name: opentofu

  inputs:
    - name: app_name
      type: string
      description: Application name
      default: "myapp"

  plugins:
    installed:
      - name: team
        template_pack: test-pack
        plugin: team"#,
        )
        .unwrap();

        fs.write(
            &template_dir.join("src/main.tf.hbs"),
            r#"resource "app" "main" {
  name = "{{app_name}}"
}"#,
        )
        .unwrap();

        // Create team plugin with repeatable_object input
        let plugin_dir = pack_path.join("plugins/team");
        fs.write(
            &plugin_dir.join(".pmp.plugin.yaml"),
            r#"apiVersion: pmp.io/v1
kind: Plugin
metadata:
  name: team
  description: Team management
spec:
  role: access-management

  inputs:
    - name: team_name
      type: string
      description: Team name
      default: "my-team"

    - name: team_members
      type: repeatable_object
      description: Team members
      min: 0
      max: 10
      add_another_prompt: "Add another member?"
      fields:
        - name: username
          type: string
          description: Username
        - name: role
          type: select
          description: Member role
          options:
            - label: "Member"
              value: "member"
            - label: "Maintainer"
              value: "maintainer"
          default: "member""#,
        )
        .unwrap();

        fs.write(
            &plugin_dir.join("src/plugin.tf.hbs"),
            r#"resource "team" "main" {
  name = "{{team_name}}"
}

{{#each team_members}}
resource "team_membership" "member_{{@index}}" {
  team_id  = team.main.id
  username = "{{username}}"
  role     = "{{role}}"
}
{{/each}}"#,
        )
        .unwrap();

        // Setup infrastructure
        setup_infrastructure(
            &fs,
            r#"    - apiVersion: pmp.io/v1
      kind: TestResource"#,
        );

        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        ));
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        ));
        input.add_response(MockResponse::Text("plugin-test-project".to_string()));
        input.add_response(MockResponse::Text("awesome-app".to_string())); // app_name

        // Plugin: team_name
        input.add_response(MockResponse::Text("platform-team".to_string()));

        // Plugin: team_members (add 3 members)
        input.add_response(MockResponse::Confirm(true)); // Add first member
        input.add_response(MockResponse::Text("alice".to_string())); // username
        input.add_response(MockResponse::Select("Maintainer".to_string())); // role

        input.add_response(MockResponse::Confirm(true)); // Add second member
        input.add_response(MockResponse::Text("bob".to_string()));
        input.add_response(MockResponse::Select("Member".to_string()));

        input.add_response(MockResponse::Confirm(true)); // Add third member
        input.add_response(MockResponse::Text("charlie".to_string()));
        input.add_response(MockResponse::Select("Member".to_string()));

        input.add_response(MockResponse::Confirm(false)); // No more members

        input.add_response(MockResponse::Confirm(false)); // No more environments

        let ctx = create_test_context(Arc::clone(&fs), input);
        let result = CreateCommand::execute(&ctx, None, None, None, None, false, None, None);

        assert!(
            result.is_ok(),
            "Project creation should succeed: {:?}",
            result
        );

        // Verify main template file
        let main_tf_path =
            current_dir.join("projects/plugin-test-project/environments/dev/main.tf");
        let main_tf_content = fs.get_file_contents(&main_tf_path).unwrap();
        assert!(
            main_tf_content.contains("name = \"awesome-app\""),
            "Main template should have app name"
        );

        // Verify plugin file exists and has correct content
        let plugin_tf_path =
            current_dir.join("projects/plugin-test-project/environments/dev/plugin_team.tf");
        let plugin_tf_content = fs.get_file_contents(&plugin_tf_path).unwrap();

        assert!(
            plugin_tf_content.contains("name = \"platform-team\""),
            "Plugin should have team name"
        );
        assert!(
            plugin_tf_content.contains("username = \"alice\""),
            "Plugin should have alice as member"
        );
        assert!(
            plugin_tf_content.contains("role     = \"maintainer\""),
            "Alice should be maintainer"
        );
        assert!(
            plugin_tf_content.contains("username = \"bob\""),
            "Plugin should have bob as member"
        );
        assert!(
            plugin_tf_content.contains("username = \"charlie\""),
            "Plugin should have charlie as member"
        );
        assert_eq!(
            plugin_tf_content.matches("team_membership").count(),
            3,
            "Should have exactly 3 team memberships"
        );

        // Verify environment file has plugin inputs stored
        let env_yaml_path =
            current_dir.join("projects/plugin-test-project/environments/dev/.pmp.environment.yaml");
        let env_content = fs.get_file_contents(&env_yaml_path).unwrap();
        assert!(
            env_content.contains("team_name: platform-team"),
            "Environment should store team_name"
        );
        assert!(
            env_content.contains("username: alice"),
            "Environment should store alice in team_members"
        );
    }

    #[test]
    fn test_create_project_with_backend_configuration() {
        let fs = Arc::new(MockFileSystem::new());
        let current_dir = std::env::current_dir().unwrap();

        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r#"    app_name:
      type: string
      description: Application name
      default: "myapp""#,
        );

        fs.write(
            &current_dir
                .join(".pmp/template-packs/test-pack/templates/test-template/src/main.tf.hbs"),
            r#"resource "app" "main" {
  name = "{{app_name}}"
}"#,
        )
        .unwrap();

        // Setup infrastructure with backend configuration
        let infra_yaml = format!(
            r#"apiVersion: pmp.io/v1
kind: Infrastructure
metadata:
  name: Test Infrastructure
  description: Test infrastructure
spec:
  executor:
    name: opentofu
    config:
      backend:
        type: s3
        bucket: my-terraform-state
        region: us-west-2
        key_prefix: projects

  categories:
    - id: pmp_io_v1_testresource
      name: TestResource
      templates:
        - template_pack: test-pack
          template: test-template
  template_packs:

  environments:
    dev:
      name: Development
      description: Development environment"#
        );

        fs.write(&current_dir.join(".pmp.infrastructure.yaml"), &infra_yaml)
            .unwrap();

        let input = MockUserInput::new();
        input.add_response(MockResponse::Select("📁 TestResource".to_string()));
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        ));
        input.add_response(MockResponse::Text("backend-test-project".to_string()));
        input.add_response(MockResponse::Text("coolapp".to_string())); // app_name
        input.add_response(MockResponse::Confirm(false));

        let ctx = create_test_context(Arc::clone(&fs), input);
        let result = CreateCommand::execute(&ctx, None, None, None, None, false, None, None);

        assert!(
            result.is_ok(),
            "Project creation should succeed: {:?}",
            result
        );

        // Verify _common.tf exists with backend configuration
        let common_tf_path =
            current_dir.join("projects/backend-test-project/environments/dev/_common.tf");
        assert!(
            fs.exists(&common_tf_path),
            "_common.tf should be generated with backend config"
        );

        let common_tf_content = fs.get_file_contents(&common_tf_path).unwrap();
        assert!(
            common_tf_content.contains("backend \"s3\""),
            "Should have s3 backend configuration"
        );
        assert!(
            common_tf_content.contains("bucket = \"my-terraform-state\""),
            "Should have correct bucket name"
        );
        assert!(
            common_tf_content.contains("region = \"us-west-2\""),
            "Should have correct region"
        );
        // S3 backend uses key_prefix which gets combined with project/environment at runtime
        assert!(
            common_tf_content.contains("key_prefix = \"projects\""),
            "Should have correct key_prefix for state files"
        );
    }

    #[test]
    fn test_create_project_with_multiple_input_types() {
        let fs = Arc::new(MockFileSystem::new());
        let current_dir = std::env::current_dir().unwrap();

        // Create a comprehensive template with various input types
        let template_yaml = r##"apiVersion: pmp.io/v1
kind: Template
metadata:
  name: comprehensive
  description: Template with all input types
spec:
  apiVersion: pmp.io/v1
  kind: TestResource
  executor:
    name: opentofu

  inputs:
    - name: app_name
      type: string
      description: Application name
      default: "myapp"

    - name: replicas
      type: number
      description: Number of replicas
      default: 3
      min: 1
      max: 10

    - name: enable_monitoring
      type: boolean
      description: Enable monitoring
      default: true

    - name: environment_type
      type: select
      description: Environment type
      options:
        - label: "Development"
          value: "dev"
        - label: "Production"
          value: "prod"
      default: "dev"

    - name: theme_color
      type: color
      description: Theme color
      default: "#3B82F6"

    - name: cache_ttl
      type: duration
      description: Cache TTL
      default: "1h"

    - name: backup_schedule
      type: cron
      description: Backup schedule
      default: "0 2 * * *"

    - name: labels
      type: keyvalue
      description: Resource labels
      default: "env=dev,team=platform"

    - name: app_version
      type: semver
      description: App version
      default: "1.0.0"

    - name: database
      type: object
      description: Database configuration
      fields:
        - name: host
          type: string
          default: "localhost"
        - name: port
          type: number
          default: 5432
        - name: ssl_enabled
          type: boolean
          default: true"##;

        fs.write(
            &current_dir.join(".pmp/template-packs/test-pack/.pmp.template-pack.yaml"),
            r#"apiVersion: pmp.io/v1
kind: TemplatePack
metadata:
  name: test-pack
spec: {}"#,
        )
        .unwrap();

        fs.write(
            &current_dir
                .join(".pmp/template-packs/test-pack/templates/comprehensive/.pmp.template.yaml"),
            template_yaml,
        )
        .unwrap();

        let template_content = r#"# Application: {{app_name}}
# Replicas: {{replicas}}
# Monitoring: {{bool enable_monitoring}}
# Environment: {{environment_type}}
# Color: {{theme_color}}
# Cache TTL: {{cache_ttl}} seconds
# Backup: {{backup_schedule}}
# Version: {{app_version}}

resource "app" "main" {
  name     = "{{app_name}}"
  replicas = {{replicas}}
  monitoring_enabled = {{bool enable_monitoring}}
  environment = "{{environment_type}}"
  theme_color = "{{theme_color}}"
  cache_ttl_seconds = {{cache_ttl}}
  backup_cron = "{{backup_schedule}}"
  version = "{{app_version}}"

  {{#each labels}}
  label_{{@key}} = "{{this}}"
  {{/each}}

  database {
    host = "{{database.host}}"
    port = {{database.port}}
    ssl_enabled = {{bool database.ssl_enabled}}
  }
}"#;

        fs.write(
            &current_dir
                .join(".pmp/template-packs/test-pack/templates/comprehensive/src/main.tf.hbs"),
            template_content,
        )
        .unwrap();

        // Setup infrastructure for comprehensive template
        setup_infrastructure_with_categories(
            &fs,
            r#"    - id: pmp_io_v1_testresource
      name: TestResource (pmp.io/v1)
      description: Test resource type
      templates:
        - template_pack: test-pack
          template: comprehensive"#,
            "",
        );

        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        ));
        input.add_response(MockResponse::Select(
            "📄 comprehensive - Template with all input types".to_string(),
        ));
        input.add_response(MockResponse::Text("comprehensive-project".to_string()));

        // Inputs in order
        input.add_response(MockResponse::Text("awesome-app".to_string())); // app_name
        input.add_response(MockResponse::Text("5".to_string())); // replicas
        input.add_response(MockResponse::Select("Yes".to_string())); // enable_monitoring
        input.add_response(MockResponse::Select("Production".to_string())); // environment_type
        input.add_response(MockResponse::Text("#FF5733".to_string())); // theme_color
        input.add_response(MockResponse::Text("2h30m".to_string())); // cache_ttl = 9000
        input.add_response(MockResponse::Text("0 3 * * *".to_string())); // backup_schedule
        input.add_response(MockResponse::Text("env=prod,team=backend".to_string())); // labels
        input.add_response(MockResponse::Text("2.5.1".to_string())); // app_version

        // Object: database
        input.add_response(MockResponse::Text("db.example.com".to_string())); // database.host
        input.add_response(MockResponse::Text("5432".to_string())); // database.port
        input.add_response(MockResponse::Select("Yes".to_string())); // database.ssl_enabled

        input.add_response(MockResponse::Confirm(false)); // No more environments

        let ctx = create_test_context(Arc::clone(&fs), input);
        let result = CreateCommand::execute(&ctx, None, None, None, None, false, None, None);

        assert!(
            result.is_ok(),
            "Project creation should succeed: {:?}",
            result
        );

        let main_tf_path =
            current_dir.join("projects/comprehensive-project/environments/dev/main.tf");
        let main_tf_content = fs.get_file_contents(&main_tf_path).unwrap();

        // Verify all input types rendered correctly
        assert!(
            main_tf_content.contains("name     = \"awesome-app\""),
            "String input"
        );
        assert!(main_tf_content.contains("replicas = 5"), "Number input");
        assert!(
            main_tf_content.contains("monitoring_enabled = true"),
            "Boolean input"
        );
        assert!(
            main_tf_content.contains("environment = \"prod\""),
            "Select input"
        );
        assert!(
            main_tf_content.contains("theme_color = \"#FF5733\""),
            "Color input"
        );
        assert!(
            main_tf_content.contains("cache_ttl_seconds = 9000"),
            "Duration input (parsed)"
        );
        assert!(
            main_tf_content.contains("backup_cron = \"0 3 * * *\""),
            "Cron input"
        );
        assert!(
            main_tf_content.contains("version = \"2.5.1\""),
            "Semver input"
        );
        assert!(
            main_tf_content.contains("label_env = \"prod\""),
            "KeyValue input"
        );
        assert!(
            main_tf_content.contains("label_team = \"backend\""),
            "KeyValue input"
        );
        assert!(
            main_tf_content.contains("host = \"db.example.com\""),
            "Object input"
        );
        assert!(main_tf_content.contains("port = 5432"), "Object input");
        assert!(
            main_tf_content.contains("ssl_enabled = true"),
            "Object input"
        );
    }

    #[test]
    #[ignore] // Integration test: requires mock executor to avoid running real tofu commands
    fn test_create_project_multiple_environments() {
        let fs = Arc::new(MockFileSystem::new());
        let current_dir = std::env::current_dir().unwrap();

        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r#"    app_name:
      type: string
      description: Application name
      default: "myapp""#,
        );

        fs.write(
            &current_dir
                .join(".pmp/template-packs/test-pack/templates/test-template/src/main.tf.hbs"),
            r#"resource "app" "main" {
  name = "{{app_name}}"
  environment = "{{_environment_name}}"
}"#,
        )
        .unwrap();

        // Infrastructure with multiple environments
        let infra_yaml = format!(
            r#"apiVersion: pmp.io/v1
kind: Infrastructure
metadata:
  name: Test Infrastructure
  description: Test infrastructure
spec:
  categories:
    - id: pmp_io_v1_testresource
      name: TestResource
      templates:
        - template_pack: test-pack
          template: test-template
  template_packs:

  environments:
    dev:
      name: Development
      description: Development environment
    staging:
      name: Staging
      description: Staging environment
    prod:
      name: Production
      description: Production environment"#
        );

        fs.write(&current_dir.join(".pmp.infrastructure.yaml"), &infra_yaml)
            .unwrap();

        let input = MockUserInput::new();
        input.add_response(MockResponse::Select("📁 TestResource".to_string()));
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        ));
        input.add_response(MockResponse::Select(
            "Development - Development environment".to_string(),
        )); // Initial environment
        input.add_response(MockResponse::Text("multi-env-project".to_string()));
        input.add_response(MockResponse::Text("myapp".to_string())); // app_name for dev

        // Add staging environment
        input.add_response(MockResponse::Confirm(true));
        input.add_response(MockResponse::Select(
            "Staging - Staging environment".to_string(),
        ));
        input.add_response(MockResponse::Text("myapp".to_string())); // app_name for staging

        // Add prod environment
        input.add_response(MockResponse::Confirm(true));
        input.add_response(MockResponse::Select(
            "Production - Production environment".to_string(),
        ));
        input.add_response(MockResponse::Text("myapp".to_string())); // app_name for prod

        input.add_response(MockResponse::Confirm(false)); // No more environments

        let ctx = create_test_context(Arc::clone(&fs), input);
        let result = CreateCommand::execute(&ctx, None, None, None, None, false, None, None);

        assert!(
            result.is_ok(),
            "Project creation should succeed: {:?}",
            result
        );

        // Verify dev environment
        let dev_tf_path = current_dir.join("projects/multi-env-project/environments/dev/main.tf");
        let dev_tf_content = fs.get_file_contents(&dev_tf_path).unwrap();
        assert!(
            dev_tf_content.contains("environment = \"dev\""),
            "Dev environment should have correct env variable"
        );

        // Verify staging environment
        let staging_tf_path =
            current_dir.join("projects/multi-env-project/environments/staging/main.tf");
        let staging_tf_content = fs.get_file_contents(&staging_tf_path).unwrap();
        assert!(
            staging_tf_content.contains("environment = \"staging\""),
            "Staging environment should have correct env variable"
        );

        // Verify prod environment
        let prod_tf_path = current_dir.join("projects/multi-env-project/environments/prod/main.tf");
        let prod_tf_content = fs.get_file_contents(&prod_tf_path).unwrap();
        assert!(
            prod_tf_content.contains("environment = \"prod\""),
            "Prod environment should have correct env variable"
        );

        // Verify project file
        let project_yaml_path = current_dir.join("projects/multi-env-project/.pmp.project.yaml");
        let project_content = fs.get_file_contents(&project_yaml_path).unwrap();
        assert!(
            project_content.contains("kind: TestResource"),
            "Project should have correct kind"
        );
    }

    #[test]
    fn test_create_project_with_infrastructure_defaults() {
        let fs = Arc::new(MockFileSystem::new());
        let current_dir = std::env::current_dir().unwrap();

        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r#"    app_name:
      type: string
      description: Application name
      default: "myapp"

    replicas:
      type: number
      description: Number of replicas
      default: 1"#,
        );

        fs.write(
            &current_dir
                .join(".pmp/template-packs/test-pack/templates/test-template/src/main.tf.hbs"),
            r#"resource "app" "main" {
  name = "{{app_name}}"
  replicas = {{replicas}}
}"#,
        )
        .unwrap();

        // Infrastructure with template defaults
        setup_infrastructure_with_categories(
            &fs,
            r#"    - id: pmp_io_v1_testresource
      name: TestResource
      templates:
        - template_pack: test-pack
          template: test-template"#,
            r#"    test-pack:
      templates:
        test-template:
          defaults:
            inputs:
              replicas:
                value: 5
                show_as_default: false"#,
        );

        let input = MockUserInput::new();
        input.add_response(MockResponse::Select("📁 TestResource".to_string()));
        input.add_response(MockResponse::Select(
            "📄 test-template - Test template".to_string(),
        ));
        input.add_response(MockResponse::Text("defaults-project".to_string()));
        input.add_response(MockResponse::Text("coolapp".to_string())); // app_name
        // Note: replicas should NOT be prompted because show_as_default: false
        input.add_response(MockResponse::Confirm(false));

        let ctx = create_test_context(Arc::clone(&fs), input);
        let result = CreateCommand::execute(&ctx, None, None, None, None, false, None, None);

        assert!(
            result.is_ok(),
            "Project creation should succeed: {:?}",
            result
        );

        let main_tf_path = current_dir.join("projects/defaults-project/environments/dev/main.tf");
        let main_tf_content = fs.get_file_contents(&main_tf_path).unwrap();

        assert!(
            main_tf_content.contains("replicas = 5"),
            "Should use infrastructure default value (5) not template default (1)"
        );

        let env_yaml_path =
            current_dir.join("projects/defaults-project/environments/dev/.pmp.environment.yaml");
        let env_content = fs.get_file_contents(&env_yaml_path).unwrap();
        assert!(
            env_content.contains("replicas: 5"),
            "Environment file should have infrastructure default"
        );
    }

    #[test]
    fn test_create_project_with_conditional_inputs() {
        let fs = Arc::new(MockFileSystem::new());
        let current_dir = std::env::current_dir().unwrap();

        let template_yaml = r#"apiVersion: pmp.io/v1
kind: Template
metadata:
  name: conditional
  description: Template with conditional inputs
spec:
  apiVersion: pmp.io/v1
  kind: TestResource
  executor:
    name: opentofu

  inputs:
    - name: enable_monitoring
      type: boolean
      description: Enable monitoring
      default: false

    - name: monitoring_endpoint
      type: string
      description: Monitoring endpoint URL
      default: "http://localhost:9090"
      show_if:
        - field: enable_monitoring
          condition: equals
          value: true

    - name: environment_type
      type: select
      description: Environment type
      options:
        - label: "Development"
          value: "dev"
        - label: "Production"
          value: "prod"
      default: "dev"

    - name: production_replicas
      type: number
      description: Number of replicas for production
      default: 5
      show_if:
        - field: environment_type
          condition: equals
          value: "prod""#;

        fs.write(
            &current_dir.join(".pmp/template-packs/test-pack/.pmp.template-pack.yaml"),
            r#"apiVersion: pmp.io/v1
kind: TemplatePack
metadata:
  name: test-pack
spec: {}"#,
        )
        .unwrap();

        fs.write(
            &current_dir
                .join(".pmp/template-packs/test-pack/templates/conditional/.pmp.template.yaml"),
            template_yaml,
        )
        .unwrap();

        fs.write(
            &current_dir
                .join(".pmp/template-packs/test-pack/templates/conditional/src/main.tf.hbs"),
            r#"resource "app" "main" {
{{#if enable_monitoring}}
  monitoring_endpoint = "{{monitoring_endpoint}}"
{{/if}}
  environment = "{{environment_type}}"
{{#if (eq environment_type "prod")}}
  replicas = {{production_replicas}}
{{/if}}
}"#,
        )
        .unwrap();

        // Setup infrastructure for conditional template
        setup_infrastructure_with_categories(
            &fs,
            r#"    - id: pmp_io_v1_testresource
      name: TestResource (pmp.io/v1)
      description: Test resource type
      templates:
        - template_pack: test-pack
          template: conditional"#,
            "",
        );

        let input = MockUserInput::new();
        input.add_response(MockResponse::Select(
            "📁 TestResource (pmp.io/v1) - Test resource type".to_string(),
        ));
        input.add_response(MockResponse::Select(
            "📄 conditional - Template with conditional inputs".to_string(),
        ));
        input.add_response(MockResponse::Text("conditional-project".to_string()));

        input.add_response(MockResponse::Select("Yes".to_string())); // enable_monitoring = true
        input.add_response(MockResponse::Text(
            "http://monitoring.example.com".to_string(),
        )); // monitoring_endpoint (shown)
        input.add_response(MockResponse::Select("Production".to_string())); // environment_type = prod
        input.add_response(MockResponse::Text("10".to_string())); // production_replicas (shown)

        input.add_response(MockResponse::Confirm(false));

        let ctx = create_test_context(Arc::clone(&fs), input);
        let result = CreateCommand::execute(&ctx, None, None, None, None, false, None, None);

        assert!(
            result.is_ok(),
            "Project creation should succeed: {:?}",
            result
        );

        let main_tf_path =
            current_dir.join("projects/conditional-project/environments/dev/main.tf");
        let main_tf_content = fs.get_file_contents(&main_tf_path).unwrap();

        assert!(
            main_tf_content.contains("monitoring_endpoint = \"http://monitoring.example.com\""),
            "Conditional input should be rendered when condition is met"
        );
        assert!(
            main_tf_content.contains("environment = \"prod\""),
            "Should have prod environment"
        );
        assert!(
            main_tf_content.contains("replicas = 10"),
            "Conditional production_replicas should be rendered"
        );
    }
}
