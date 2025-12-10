use crate::collection::{CollectionDiscovery, CollectionManager};
use crate::commands::apply::ApplyCommand;
use crate::output;
use crate::template::metadata::{
    AddedPlugin, AddedPluginReference, AllowedPluginConfig, InfrastructureResource,
    PluginProjectReference, ProjectPlugins,
};
use crate::template::{
    DynamicProjectEnvironmentResource, PluginInfo, ProjectReference, ProjectResource,
    TemplateDiscovery, TemplateInfo, TemplateRenderer,
};
use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Handles the 'update' command - regenerates project environment files from the original template
pub struct UpdateCommand;

/// Information about a plugin and its compatible projects
#[derive(Debug, Clone)]
struct PluginWithProjects {
    plugin_info: PluginInfo,
    #[allow(dead_code)]
    template_pack_path: PathBuf,
    // One Vec of compatible projects per dependency (Vec index matches dependency index)
    compatible_projects_by_dependency: Vec<Vec<CompatibleProject>>,
}

/// Information about a project compatible with a plugin
#[derive(Debug, Clone)]
struct CompatibleProject {
    project_ref: ProjectReference,
    project_path: PathBuf,
    #[allow(dead_code)]
    template_info: TemplateInfo,
    allowed_plugin_config: AllowedPluginConfig,
}

/// Information about a collected plugin (for installed plugins during update)
#[derive(Debug, Clone)]
struct CollectedPluginInfo {
    template_pack_name: String,
    plugin_name: String,
    plugin_path: PathBuf,
    inputs: HashMap<String, Value>,
    reference_projects: Vec<(crate::template::metadata::ProjectReference, DynamicProjectEnvironmentResource)>,
    raw_module_inputs: Option<HashMap<String, String>>,
    plugin_spec: crate::template::metadata::PluginSpec,
}

impl UpdateCommand {
    /// Execute the update command
    pub fn execute(
        ctx: &crate::context::Context,
        project_path: Option<&str>,
        template_packs_paths: Option<&str>,
        inputs_str: Option<&str>,
    ) -> Result<()> {
        // Parse pre-defined inputs if provided
        let predefined_inputs: Option<HashMap<String, Value>> = if let Some(inputs) = inputs_str {
            Some(crate::commands::CreateCommand::parse_inputs(inputs)?)
        } else {
            None
        };
        // Determine working directory
        let work_dir = if let Some(path) = project_path {
            PathBuf::from(path)
        } else {
            std::env::current_dir().context("Failed to get current directory")?
        };

        // Detect context and get environment path
        let (env_path, project_name, env_name) =
            Self::detect_and_select_environment(ctx, &work_dir)?;

        // Load environment resource to get current configuration
        let env_file = env_path.join(".pmp.environment.yaml");
        if !ctx.fs.exists(&env_file) {
            anyhow::bail!("Environment file not found: {:?}", env_file);
        }

        let current_env_resource =
            DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
                .context("Failed to load environment resource")?;

        output::section("Update Environment");
        output::key_value_highlight("Project", &project_name);
        output::environment_badge(&env_name);
        output::key_value("Resource Kind", &current_env_resource.kind);

        if let Some(desc) = &current_env_resource.metadata.description {
            output::key_value("Description", desc);
        }

        // Load collection to ensure we're in a valid collection context
        let (collection, collection_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required to run commands")?;

        // Discover plugins with compatible projects
        let plugins_with_projects = Self::discover_plugins_with_compatible_projects(
            ctx,
            &collection,
            &collection_root,
            template_packs_paths,
            &current_env_resource, // Pass target project info for filtering
        )?;

        // Check if there are any plugins currently added
        let has_plugins = current_env_resource
            .spec
            .plugins
            .as_ref()
            .map(|p| !p.added.is_empty())
            .unwrap_or(false);

        // If there are plugins with compatible projects or plugins to remove, ask user what they want to do
        if !plugins_with_projects.is_empty() || has_plugins {
            output::blank();
            let mut options: Vec<String> = vec!["Update the project".to_string()];
            if !plugins_with_projects.is_empty() {
                options.push("Add Plugin".to_string());
            }
            if has_plugins {
                options.push("Remove Plugin".to_string());
                options.push("Update Plugin Inputs".to_string());
            }

            let action = ctx
                .input
                .select("What would you like to do?", options, None)
                .context("Failed to select action")?;

            if action == "Add Plugin" {
                // Add plugin flow with project selection
                // Pass the current project's context (PROJECT A) as the target
                return Self::add_plugin_with_project_selection(
                    ctx,
                    &collection_root,
                    &collection,
                    plugins_with_projects,
                    template_packs_paths,
                    &env_path,
                    &project_name,
                    &env_name,
                    current_env_resource,
                );
            }

            if action == "Remove Plugin" {
                // Remove plugin flow
                return Self::remove_plugin_interactive(
                    ctx,
                    &collection_root,
                    &collection,
                    &env_path,
                    &project_name,
                    &env_name,
                    current_env_resource,
                    template_packs_paths,
                );
            }

            if action == "Update Plugin Inputs" {
                // Update plugin inputs flow
                return Self::update_plugin_inputs_interactive(
                    ctx,
                    &collection_root,
                    &collection,
                    &env_path,
                    &project_name,
                    &env_name,
                    current_env_resource,
                    template_packs_paths,
                );
            }
        }

        // Discover template packs (same as create command)
        output::subsection("Template Discovery");
        output::dimmed("Discovering template packs...");

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
        let _all_template_packs = if all_template_packs.is_empty() {
            TemplateDiscovery::discover_template_packs_with_custom_paths(
                &*ctx.fs,
                &*ctx.output,
                &custom_paths,
            )
            .context("Failed to re-discover template packs after installation")?
        } else {
            all_template_packs
        };

        // Find the original template using the metadata stored in the environment resource
        let matching_template =
            Self::find_original_template(ctx, &current_env_resource, template_packs_paths)?;

        // Note: matching_pack is no longer tracked separately since we find the exact original template
        // The template pack information is stored in current_env_resource.spec.template.template_pack_name

        output::key_value_highlight("Template", &matching_template.resource.metadata.name);
        if let Some(desc) = &matching_template.resource.metadata.description {
            output::key_value("Description", desc);
        }

        // Continue with normal update flow...
        // Get current inputs (these will be used as defaults)
        let current_inputs = &current_env_resource.spec.inputs;

        // Get template inputs, merging base inputs with environment-specific overrides
        let mut merged_inputs = matching_template.resource.spec.inputs.clone();

        // Override with environment-specific inputs if they exist
        if let Some(env_overrides) = matching_template.resource.spec.environments.get(&env_name) {
            for env_input in &env_overrides.overrides.inputs {
                // Remove any existing input with the same name
                merged_inputs.retain(|input_def| input_def.name != env_input.name);
                // Add the environment-specific input
                merged_inputs.push(env_input.clone());
            }
        }

        // Prompt for inputs with current values as defaults
        output::subsection("Update Inputs");
        output::dimmed(
            "Please provide the following information (current values shown as defaults):",
        );

        let mut new_inputs = Self::collect_template_inputs_with_defaults(
            ctx,
            &merged_inputs,
            current_inputs,
            &project_name,
            &env_name,
            predefined_inputs.as_ref(),
        )
        .context("Failed to collect inputs")?;

        // Add internal fields for template rendering
        new_inputs.insert(
            "_environment".to_string(),
            serde_json::Value::String(env_name.clone()),
        );
        new_inputs.insert(
            "_resource_api_version".to_string(),
            serde_json::Value::String(matching_template.resource.spec.api_version.clone()),
        );
        new_inputs.insert(
            "_resource_kind".to_string(),
            serde_json::Value::String(matching_template.resource.spec.kind.clone()),
        );

        // Process installed plugins from template spec
        let mut newly_collected_plugins = Vec::new();
        if let Some(plugins_config) = &matching_template.resource.spec.plugins
            && !plugins_config.installed.is_empty()
        {
            output::blank();
            output::subsection("Processing Installed Plugins");
            output::dimmed("Checking for new plugins defined in template...");

            // Get currently added plugins from environment
            let current_plugins = current_env_resource
                .spec
                .plugins
                .as_ref()
                .map(|p| &p.added)
                .map(|p| p.as_slice())
                .unwrap_or(&[]);

            // Discover projects (needed for plugins that require reference projects)
            let discovered_projects =
                CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &collection_root)?;

            // Check each installed plugin
            for installed_config in &plugins_config.installed {
                // Check if this plugin is already added to the environment
                let already_added = current_plugins.iter().any(|added_plugin| {
                    added_plugin.template_pack_name == installed_config.template_pack_name
                        && added_plugin.name == installed_config.plugin_name
                });

                if already_added {
                    output::dimmed(&format!(
                        "  Plugin {}/{} is already added, skipping",
                        installed_config.template_pack_name, installed_config.plugin_name
                    ));
                    continue;
                }

                // Plugin not added yet, collect inputs for it
                output::blank();
                output::dimmed(&format!(
                    "  Installing new plugin: {}/{}",
                    installed_config.template_pack_name, installed_config.plugin_name
                ));

                // Reuse collect_plugin_info from create command
                if let Some(plugin_info) = Self::collect_plugin_info_for_update(
                    ctx,
                    installed_config,
                    &_all_template_packs,
                    &discovered_projects,
                    &collection_root,
                    &project_name,
                    &env_name,
                    &current_env_resource,
                )? {
                    newly_collected_plugins.push(plugin_info);
                }
            }
        }

        // Add plugins data for template rendering (including existing and newly collected)
        let mut all_plugins_for_rendering =
            if let Some(plugins) = &current_env_resource.spec.plugins {
                plugins.clone()
            } else {
                ProjectPlugins { added: Vec::new() }
            };

        // Enrich existing plugins with plugin_spec (for plugins added before this fix)
        // This ensures _common.tf generation works correctly with required_fields
        for existing_plugin in &mut all_plugins_for_rendering.added {
            if existing_plugin.plugin_spec.is_none() {
                // Try to load plugin spec from template packs
                if let Some(spec) = Self::load_plugin_spec(
                    ctx,
                    &_all_template_packs,
                    &existing_plugin.template_pack_name,
                    &existing_plugin.name,
                ) {
                    existing_plugin.plugin_spec = Some(spec);
                }
            }

            // Clean up stale reference_projects if plugin has no dependencies
            if let Some(spec) = &existing_plugin.plugin_spec
                && spec.dependencies.is_empty()
                && !existing_plugin.reference_projects.is_empty()
            {
                output::dimmed(&format!(
                    "  Cleaning stale references for plugin {}/{} (no dependencies)",
                    existing_plugin.template_pack_name, existing_plugin.name
                ));
                existing_plugin.reference_projects.clear();
            }
        }

        // Render the newly collected plugins to get AddedPlugin structs
        if !newly_collected_plugins.is_empty() {
            // We'll render these plugins after user confirmation, but we need to prepare the data
            new_inputs.insert(
                "_plugins".to_string(),
                serde_json::to_value(&all_plugins_for_rendering)
                    .context("Failed to serialize plugins")?,
            );
        } else if current_env_resource.spec.plugins.is_some() {
            new_inputs.insert(
                "_plugins".to_string(),
                serde_json::to_value(&all_plugins_for_rendering)
                    .context("Failed to serialize plugins")?,
            );
        }

        // Confirm before regenerating
        let confirm = ctx
            .input
            .confirm("Regenerate environment files with these inputs?", Some(true))
            .context("Failed to get confirmation")?;

        if !confirm {
            output::dimmed("Update cancelled");
            return Ok(());
        }

        // Render newly collected plugins first (if any)
        let mut newly_added_plugins = Vec::new();
        if !newly_collected_plugins.is_empty() {
            output::subsection("Rendering New Plugins");
            output::dimmed("Rendering newly installed plugins...");

            for plugin_info in newly_collected_plugins {
                // Render plugin files
                let mut module_path = env_path
                    .join("modules")
                    .join(&plugin_info.template_pack_name)
                    .join(&plugin_info.plugin_name);

                // Add reference project name to path ONLY if:
                // 1. Plugin spec has dependencies
                // 2. Number of reference projects matches dependencies
                // 3. Reference project name differs from plugin name (avoid duplication)
                if !plugin_info.plugin_spec.dependencies.is_empty()
                    && plugin_info.reference_projects.len() == plugin_info.plugin_spec.dependencies.len()
                    && let Some((first_ref, _)) = plugin_info.reference_projects.first()
                    && first_ref.name != plugin_info.plugin_name
                {
                    module_path = module_path.join(&first_ref.name);
                }

                let plugin_renderer = TemplateRenderer::new();
                let plugin_context = Some((
                    plugin_info.template_pack_name.as_str(),
                    plugin_info.plugin_name.as_str(),
                ));

                let _generated_files = plugin_renderer
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
                    api_version: matching_template.resource.spec.api_version.clone(),
                    kind: matching_template.resource.spec.kind.clone(),
                    name: project_name.clone(),
                    environment: env_name.clone(),
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

                newly_added_plugins.push(AddedPlugin {
                    template_pack_name: plugin_info.template_pack_name.clone(),
                    name: plugin_info.plugin_name.clone(),
                    project: plugin_project_ref,
                    reference_projects: reference_projects_metadata,
                    inputs: plugin_info.inputs,
                    files: Vec::new(), // Will be populated when files are generated
                    plugin_spec: Some(plugin_info.plugin_spec.clone()),
                    raw_module_inputs: plugin_info.raw_module_inputs.clone(),
                });
            }

            // Merge newly added plugins with existing plugins
            all_plugins_for_rendering
                .added
                .extend(newly_added_plugins.clone());
        }

        // Re-render ALL existing plugins to pick up template changes
        if !all_plugins_for_rendering.added.is_empty() {
            output::subsection("Updating Existing Plugins");
            output::dimmed("Regenerating existing plugin modules from templates...");

            for existing_plugin in &all_plugins_for_rendering.added {
                // Find the template pack containing this plugin
                let template_pack = _all_template_packs
                    .iter()
                    .find(|pack| pack.resource.metadata.name == existing_plugin.template_pack_name);

                let template_pack = match template_pack {
                    Some(pack) => pack,
                    None => {
                        ctx.output.warning(&format!(
                            "  Template pack '{}' not found. Skipping plugin '{}'.",
                            existing_plugin.template_pack_name, existing_plugin.name
                        ));
                        continue;
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
                    .find(|p| p.resource.metadata.name == existing_plugin.name);

                let plugin_info = match plugin_info {
                    Some(info) => info,
                    None => {
                        ctx.output.warning(&format!(
                            "  Plugin '{}' not found in template pack '{}'. Skipping.",
                            existing_plugin.name, existing_plugin.template_pack_name
                        ));
                        continue;
                    }
                };

                // Build module path
                let mut module_path = env_path
                    .join("modules")
                    .join(&existing_plugin.template_pack_name)
                    .join(&existing_plugin.name);

                // Add reference project name to path ONLY if:
                // 1. Plugin spec has dependencies
                // 2. Number of reference projects matches dependencies
                // 3. Reference project name differs from plugin name (avoid duplication)
                if let Some(spec) = &existing_plugin.plugin_spec
                    && !spec.dependencies.is_empty()
                    && existing_plugin.reference_projects.len() == spec.dependencies.len()
                    && let Some(first_ref) = existing_plugin.reference_projects.first()
                    && first_ref.name != existing_plugin.name
                {
                    module_path = module_path.join(&first_ref.name);
                }

                // Delete existing plugin module directory to ensure clean regeneration
                if ctx.fs.exists(&module_path) {
                    ctx.fs.remove_dir_all(&module_path).with_context(|| {
                        format!("Failed to delete existing plugin module: {:?}", module_path)
                    })?;
                }

                // Re-render plugin from template
                let plugin_renderer = TemplateRenderer::new();
                let plugin_context = Some((
                    existing_plugin.template_pack_name.as_str(),
                    existing_plugin.name.as_str(),
                ));

                let _generated_files = plugin_renderer
                    .render_template(
                        ctx,
                        &plugin_info.path,
                        &module_path,
                        &existing_plugin.inputs,
                        plugin_context,
                    )
                    .context("Failed to re-render plugin files")?;

                ctx.output.dimmed(&format!(
                    "  Regenerated: {}/{}",
                    existing_plugin.template_pack_name, existing_plugin.name
                ));
            }
        }

        // Update _plugins in new_inputs with the final state (after all plugins are collected and rendered)
        if !all_plugins_for_rendering.added.is_empty() {
            new_inputs.insert(
                "_plugins".to_string(),
                serde_json::to_value(&all_plugins_for_rendering)
                    .context("Failed to serialize plugins for template rendering")?,
            );
        }

        // Render template into environment directory
        output::subsection("Regenerating Files");
        output::dimmed("Regenerating template files...");
        let renderer = TemplateRenderer::new();
        let template_src = &matching_template.path;

        if !ctx.fs.exists(template_src) {
            anyhow::bail!("Template directory not found: {}", template_src.display());
        }

        let _generated_files = renderer
            .render_template(ctx, template_src, env_path.as_path(), &new_inputs, None)
            .context("Failed to render template")?;

        // Generate common file if executor config is present
        if let Some(executor_config) = &collection.spec.executor
            && !executor_config.config.is_empty()
        {
            // Create executor instance based on template's executor
            let template_executor_name = matching_template.resource.spec.executor.name();
            let executor: Box<dyn crate::executor::Executor> = match template_executor_name {
                "opentofu" => Box::new(crate::executor::OpenTofuExecutor::new()),
                "none" => Box::new(crate::executor::NoneExecutor::new()),
                _ => anyhow::bail!("Unknown executor: {}", template_executor_name),
            };

            // Use merged plugins list (existing + newly added)
            let plugins = if !all_plugins_for_rendering.added.is_empty() {
                Some(all_plugins_for_rendering.added.as_slice())
            } else {
                None
            };

            // Check for new template dependencies and add them BEFORE generating _common.tf
            let merged_template_reference_projects = Self::merge_template_dependencies(
                ctx,
                &matching_template.resource,
                &current_env_resource,
                &env_name,
                &env_path,
            )
            .context("Failed to merge template dependencies")?;

            let metadata = crate::executor::ProjectMetadata {
                api_version: &matching_template.resource.spec.api_version,
                kind: &matching_template.resource.spec.kind,
                environment: &env_name,
                project_name: &project_name,
            };
            executor
                .generate_common_file(
                    ctx,
                    &env_path,
                    &executor_config.config,
                    &metadata,
                    plugins,
                    &merged_template_reference_projects,
                )
                .context("Failed to generate common file")?;
        } else {
            // No executor, so no common file to generate
            // Dependencies will be merged for environment YAML generation below
        }

        // Regenerate .pmp.environment.yaml file
        output::dimmed("  Updating .pmp.environment.yaml...");

        // Get template pack name from the environment resource (preserving original)
        let template_pack_name = current_env_resource
            .spec
            .template
            .as_ref()
            .map(|t| t.template_pack_name.as_str())
            .unwrap_or(&matching_template.resource.metadata.name);

        // Pass merged plugins to environment file generation
        let plugins_for_yaml = if !all_plugins_for_rendering.added.is_empty() {
            Some(&all_plugins_for_rendering)
        } else {
            None
        };

        // Merge template dependencies (do this again to ensure we have it for YAML generation)
        let merged_template_reference_projects = Self::merge_template_dependencies(
            ctx,
            &matching_template.resource,
            &current_env_resource,
            &env_name,
            &env_path,
        )
        .context("Failed to merge template dependencies")?;

        Self::generate_project_environment_yaml(
            ctx,
            &env_path,
            &env_name,
            &project_name,
            &matching_template.resource,
            &new_inputs,
            template_pack_name,
            &matching_template.resource.metadata.name,
            plugins_for_yaml,
            &current_env_resource,
            &merged_template_reference_projects,
        )
        .context("Failed to update .pmp.environment.yaml file")?;

        output::blank();
        output::success("Environment updated successfully!");

        output::subsection("Updated Environment");
        output::key_value("Project", &project_name);
        output::environment_badge(&env_name);
        output::key_value("Path", &env_path.display().to_string());

        // Ask if user wants to execute apply
        output::blank();
        let should_apply = ctx
            .input
            .confirm("Do you want to execute 'apply' now?", Some(false))
            .context("Failed to get confirmation")?;

        if should_apply {
            output::blank();
            let env_path_str = env_path
                .to_str()
                .context("Failed to convert environment path to string")?;
            ApplyCommand::execute(ctx, Some(env_path_str), &[])?;
        } else {
            let next_steps_list = vec![
                format!("Review the regenerated files in {}", env_path.display()),
                "Run 'pmp preview' to see what changes will be applied".to_string(),
                "Run 'pmp apply' to apply the infrastructure".to_string(),
            ];
            output::next_steps(&next_steps_list);
        }

        Ok(())
    }

    /// Discover all plugins that have compatible projects in the collection
    /// Returns a list of plugins with their compatible projects
    fn discover_plugins_with_compatible_projects(
        ctx: &crate::context::Context,
        infrastructure: &InfrastructureResource,
        collection_root: &Path,
        template_packs_paths: Option<&str>,
        target_env_resource: &DynamicProjectEnvironmentResource,
    ) -> Result<Vec<PluginWithProjects>> {
        let mut result = Vec::new();

        // Discover all projects in the collection
        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, collection_root)?;

        // Discover all template packs
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
        )?;

        // Filter to only packs configured in infrastructure (if template_packs is configured)
        let template_packs: Vec<_> = if infrastructure.spec.template_packs.is_empty() {
            all_template_packs // No filtering - include all packs
        } else {
            all_template_packs
                .into_iter()
                .filter(|pack| {
                    infrastructure
                        .spec
                        .template_packs
                        .contains_key(&pack.resource.metadata.name)
                })
                .collect()
        };

        // For each template pack
        for pack_info in template_packs {
            // Discover templates and plugins in this pack
            let templates = TemplateDiscovery::discover_templates_in_pack(
                &*ctx.fs,
                &*ctx.output,
                &pack_info.path,
            )?;
            let plugins = TemplateDiscovery::discover_plugins_in_pack(
                &*ctx.fs,
                &*ctx.output,
                &pack_info.path,
                &pack_info.resource.metadata.name,
            )?;

            // For each plugin in this pack
            for plugin_info in plugins {
                // Now find compatible reference projects based on plugin's requirements
                let mut compatible_projects_by_dependency: Vec<Vec<CompatibleProject>> = Vec::new();

                // Check if plugin requires reference projects
                if !plugin_info.resource.spec.dependencies.is_empty() {
                    // For each dependency, find compatible projects
                    for dependency in &plugin_info.resource.spec.dependencies {
                        let dependency_ref = &dependency.project;
                        let mut dep_compatible_projects = Vec::new();

                        // Find projects matching this dependency
                        for project in &projects {
                            let project_path = collection_root.join(&project.path);
                            let environments_dir = project_path.join("environments");

                            // Find the first environment to get resource details
                            if let Ok(env_entries) = ctx.fs.read_dir(&environments_dir) {
                                for env_path in env_entries {
                                    let env_file = env_path.join(".pmp.environment.yaml");

                                    if ctx.fs.exists(&env_file)
                                        && let Ok(env_resource) =
                                            DynamicProjectEnvironmentResource::from_file(
                                                &*ctx.fs, &env_file,
                                            )
                                    {
                                        // Check if this project matches the required template
                                        if env_resource.api_version == dependency_ref.api_version
                                            && env_resource.kind == dependency_ref.kind
                                        {
                                        // Find the template info for this project
                                        if let Some(template_info) = templates.iter().find(|t| {
                                            t.resource.spec.api_version == env_resource.api_version
                                                && t.resource.spec.kind == env_resource.kind
                                        }) {
                                            // Get allowed plugin config from target template
                                            let allowed_config = templates
                                                .iter()
                                                .find(|t| {
                                                    t.resource.spec.api_version
                                                        == target_env_resource.api_version
                                                        && t.resource.spec.kind
                                                            == target_env_resource.kind
                                                })
                                                .and_then(|t| t.resource.spec.plugins.as_ref())
                                                .and_then(|pc| {
                                                    pc.allowed.iter().find(|a| {
                                                        a.plugin_name
                                                            == plugin_info.resource.metadata.name
                                                            && a.template_pack_name
                                                                == pack_info.resource.metadata.name
                                                    })
                                                })
                                                .cloned()
                                                .unwrap_or_else(|| AllowedPluginConfig {
                                                    template_pack_name: pack_info
                                                        .resource
                                                        .metadata
                                                        .name
                                                        .clone(),
                                                    plugin_name: plugin_info
                                                        .resource
                                                        .metadata
                                                        .name
                                                        .clone(),
                                                    inputs: Vec::new(),
                                                    order: 0,
                                                    raw_module_inputs: None,
                                                    disable_user_input_override: false,
                                                });

                                            dep_compatible_projects.push(CompatibleProject {
                                                project_ref: project.clone(),
                                                project_path: project_path.clone(),
                                                template_info: template_info.clone(),
                                                allowed_plugin_config: allowed_config,
                                            });
                                            break; // Only need one environment to confirm the match
                                        }
                                    }
                                }
                            }
                        }
                    }

                        compatible_projects_by_dependency.push(dep_compatible_projects);
                    }

                    // Only include plugin if ALL dependencies have at least one compatible project
                    let all_dependencies_satisfied = compatible_projects_by_dependency.iter()
                        .all(|projects| !projects.is_empty());

                    if all_dependencies_satisfied {
                        result.push(PluginWithProjects {
                            plugin_info: plugin_info.clone(),
                            template_pack_path: pack_info.path.clone(),
                            compatible_projects_by_dependency,
                        });
                    }
                } else {
                    // Plugin doesn't require a reference project - add it without compatible projects
                    // We still need to provide the allowed config from the target template
                    let _allowed_config = templates
                        .iter()
                        .find(|t| {
                            t.resource.spec.api_version == target_env_resource.api_version
                                && t.resource.spec.kind == target_env_resource.kind
                        })
                        .and_then(|t| t.resource.spec.plugins.as_ref())
                        .and_then(|pc| {
                            pc.allowed.iter().find(|a| {
                                a.plugin_name == plugin_info.resource.metadata.name
                                    && a.template_pack_name == pack_info.resource.metadata.name
                            })
                        })
                        .cloned()
                        .unwrap_or_else(|| AllowedPluginConfig {
                            template_pack_name: pack_info.resource.metadata.name.clone(),
                            plugin_name: plugin_info.resource.metadata.name.clone(),
                            inputs: Vec::new(),
                            order: 0,
                            raw_module_inputs: None,
                            disable_user_input_override: false,
                        });

                    // Add plugin with empty compatible projects list (no reference project needed)
                    result.push(PluginWithProjects {
                        plugin_info: plugin_info.clone(),
                        template_pack_path: pack_info.path.clone(),
                        compatible_projects_by_dependency: vec![], // Empty - no reference project required
                    });
                }
            }
        }

        Ok(result)
    }


    /// Add a plugin with project selection
    /// User selects plugin -> project (for reference) -> provides inputs -> adds plugin to target project
    #[allow(clippy::too_many_arguments)]
    fn add_plugin_with_project_selection(
        ctx: &crate::context::Context,
        _collection_root: &Path,
        collection: &InfrastructureResource,
        plugins_with_projects: Vec<PluginWithProjects>,
        _template_packs_paths: Option<&str>,
        target_env_path: &Path,
        target_project_name: &str,
        target_env_name: &str,
        target_env_resource: DynamicProjectEnvironmentResource,
    ) -> Result<()> {
        // Validate that collection has backend configured
        if collection.spec.executor.is_none() {
            anyhow::bail!(
                "Cannot add plugins: project collection must have a backend configured in spec.executor"
            );
        }

        let _executor_config = if let Some(executor) = &collection.spec.executor {
            if executor.config.is_empty() {
                anyhow::bail!(
                    "Cannot add plugins: project collection must have backend configuration in spec.executor.config"
                );
            }
            &executor.config
        } else {
            anyhow::bail!(
                "Cannot add plugins: project collection must have a backend configured in spec.executor"
            );
        };

        output::subsection("Add Plugin");

        // 1. Let user select plugin
        let plugin_options: Vec<String> = plugins_with_projects
            .iter()
            .map(|p| {
                let desc = p
                    .plugin_info
                    .resource
                    .metadata
                    .description
                    .as_deref()
                    .unwrap_or("");

                let dependency_count = p.plugin_info.resource.spec.dependencies.len();

                if desc.is_empty() {
                    if dependency_count > 0 {
                        format!(
                            "{} ({} {})",
                            p.plugin_info.resource.metadata.name,
                            dependency_count,
                            if dependency_count == 1 { "dependency" } else { "dependencies" }
                        )
                    } else {
                        p.plugin_info.resource.metadata.name.clone()
                    }
                } else if dependency_count > 0 {
                    format!(
                        "{} - {} ({} {})",
                        p.plugin_info.resource.metadata.name,
                        desc,
                        dependency_count,
                        if dependency_count == 1 { "dependency" } else { "dependencies" }
                    )
                } else {
                    format!(
                        "{} - {}",
                        p.plugin_info.resource.metadata.name,
                        desc
                    )
                }
            })
            .collect();

        let selected_plugin_display = ctx
            .input
            .select("Select a plugin to add:", plugin_options.clone(), None)
            .context("Failed to select plugin")?;

        let plugin_index = plugin_options
            .iter()
            .position(|opt| opt == &selected_plugin_display)
            .context("Plugin not found")?;

        let selected_plugin_with_projects = &plugins_with_projects[plugin_index];

        output::blank();
        output::key_value_highlight(
            "Plugin",
            &selected_plugin_with_projects
                .plugin_info
                .resource
                .metadata
                .name,
        );
        if let Some(desc) = &selected_plugin_with_projects
            .plugin_info
            .resource
            .metadata
            .description
        {
            output::key_value("Description", desc);
        }

        // Check if this plugin requires reference projects
        let requires_reference = !selected_plugin_with_projects
            .plugin_info
            .resource
            .spec
            .dependencies
            .is_empty();

        // Variables to hold reference project/environment info for all dependencies
        let mut selected_reference_projects: Vec<(
            DynamicProjectEnvironmentResource,
            AllowedPluginConfig,
        )> = Vec::new();

        if requires_reference {
            // 2. For each dependency, let user select a compatible project
            output::blank();
            output::dimmed(&format!(
                "This plugin has {} {}. You'll select a reference project for each.",
                selected_plugin_with_projects.plugin_info.resource.spec.dependencies.len(),
                if selected_plugin_with_projects.plugin_info.resource.spec.dependencies.len() == 1 {
                    "dependency"
                } else {
                    "dependencies"
                }
            ));

            for (dep_idx, dependency) in selected_plugin_with_projects
                .plugin_info
                .resource
                .spec
                .dependencies
                .iter()
                .enumerate()
            {
                output::blank();

                // Show dependency name if available
                if let Some(dep_name) = &dependency.dependency_name {
                    output::key_value("Dependency", dep_name);
                } else {
                    output::key_value("Dependency", &format!("#{}", dep_idx + 1));
                }

                if let Some(desc) = &dependency.project.description {
                    output::dimmed(desc);
                }

                // Get compatible projects for this specific dependency
                let compatible_projects = &selected_plugin_with_projects
                    .compatible_projects_by_dependency[dep_idx];

                // Sort projects by name for consistent display
                let mut sorted_projects = compatible_projects.clone();
                sorted_projects.sort_by(|a, b| a.project_ref.name.cmp(&b.project_ref.name));

                let project_options: Vec<String> = sorted_projects
                    .iter()
                    .map(|cp| format!("{} ({})", cp.project_ref.name, cp.project_ref.kind))
                    .collect();

                let selected_project_display = ctx
                    .input
                    .select("Select a reference project:", project_options.clone(), None)
                    .context("Failed to select project")?;

                let project_index = project_options
                    .iter()
                    .position(|opt| opt == &selected_project_display)
                    .context("Project not found")?;

                let selected_compatible_project = &sorted_projects[project_index];

                output::blank();
                output::key_value_highlight(
                    "Selected",
                    &selected_compatible_project.project_ref.name,
                );

                // 3. Discover environments from the selected project (for reference)
                let reference_environments = CollectionDiscovery::discover_environments(
                    &*ctx.fs,
                    &selected_compatible_project.project_path,
                )?;

                if reference_environments.is_empty() {
                    anyhow::bail!(
                        "No environments found in reference project: {}",
                        selected_compatible_project.project_ref.name
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

                output::dimmed(&format!("Environment: {}", reference_env_name));

                // Load reference project's environment resource to get its details
                let reference_env_path = selected_compatible_project
                    .project_path
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
                    DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &reference_env_file)
                        .context("Failed to load reference environment resource")?;

                selected_reference_projects.push((
                    loaded_env_resource,
                    selected_compatible_project.allowed_plugin_config.clone(),
                ));
            }
        }

        output::blank();
        output::key_value_highlight("Target Project", target_project_name);
        output::environment_badge(target_env_name);
        output::dimmed("Plugin will be added to this project/environment");

        // Check for duplicate plugin
        if let Some(plugins) = &target_env_resource.spec.plugins {
            let is_duplicate = if requires_reference {
                // Plugin with requirements: check if already added with same set of reference projects
                // Get first reference project name for duplicate checking
                let first_ref_name = selected_reference_projects
                    .first()
                    .map(|(env_res, _)| &env_res.metadata.name);

                if let Some(ref_name) = first_ref_name {
                    plugins.added.iter().any(|p| {
                        p.template_pack_name
                            == selected_plugin_with_projects.plugin_info.template_pack_name
                            && p.name
                                == selected_plugin_with_projects
                                    .plugin_info
                                    .resource
                                    .metadata
                                    .name
                            && p.project.name == *ref_name
                    })
                } else {
                    false
                }
            } else {
                // Plugin without requirements: check if already added at all
                plugins.added.iter().any(|p| {
                    p.template_pack_name
                        == selected_plugin_with_projects.plugin_info.template_pack_name
                        && p.name
                            == selected_plugin_with_projects
                                .plugin_info
                                .resource
                                .metadata
                                .name
                })
            };

            if is_duplicate {
                output::blank();
                if requires_reference {
                    let first_ref_name = selected_reference_projects
                        .first()
                        .map(|(env_res, _)| &env_res.metadata.name)
                        .unwrap();
                    output::error(&format!(
                        "Plugin '{}' from pack '{}' with reference project '{}' is already added to this environment.",
                        selected_plugin_with_projects
                            .plugin_info
                            .resource
                            .metadata
                            .name,
                        selected_plugin_with_projects.plugin_info.template_pack_name,
                        first_ref_name
                    ));
                    output::dimmed(
                        "Note: The same plugin can be added multiple times if referencing different projects.",
                    );
                } else {
                    output::error(&format!(
                        "Plugin '{}' from pack '{}' is already added to this environment.",
                        selected_plugin_with_projects
                            .plugin_info
                            .resource
                            .metadata
                            .name,
                        selected_plugin_with_projects.plugin_info.template_pack_name,
                    ));
                    output::dimmed(
                        "Note: Plugins without reference project requirements can only be added once.",
                    );
                }
                anyhow::bail!("Duplicate plugin detected");
            }
        }

        // 4. Get plugin inputs and merge with allowed plugin config
        let mut merged_inputs = selected_plugin_with_projects
            .plugin_info
            .resource
            .spec
            .inputs
            .clone();

        // Override with allowed plugin input specs from first reference (or use plugin's default inputs if no references)
        if let Some((_, allowed_config)) = selected_reference_projects.first() {
            for allowed_input in &allowed_config.inputs {
                // Remove any existing input with the same name
                merged_inputs.retain(|input_def| input_def.name != allowed_input.name);
                // Add the allowed config input
                merged_inputs.push(allowed_input.clone());
            }
        }

        // 5. Collect inputs from user
        output::subsection("Plugin Inputs");
        output::dimmed("Please provide the following information:");

        let mut plugin_inputs =
            Self::collect_plugin_inputs(ctx, &merged_inputs, target_project_name, target_env_name)?;

        // 6. Add internal fields (inherit from target project/environment)
        let project_name_underscores = target_project_name.replace('-', "_");
        let project_name_hyphens = target_project_name.replace('_', "-");
        plugin_inputs.insert(
            "_project_name_underscores".to_string(),
            Value::String(project_name_underscores),
        );
        plugin_inputs.insert(
            "_project_name_hyphens".to_string(),
            Value::String(project_name_hyphens),
        );
        plugin_inputs.insert(
            "_environment".to_string(),
            Value::String(target_env_name.to_string()),
        );

        // Inherit namespace from target project (if not already set)
        if let Some(namespace) = target_env_resource.spec.inputs.get("namespace")
            && !plugin_inputs.contains_key("namespace")
        {
            plugin_inputs.insert("namespace".to_string(), namespace.clone());
        }

        // Inherit database_name from first reference project if available
        if let Some((ref_env_res, _)) = selected_reference_projects.first() {
            if let Some(database_name) = ref_env_res.spec.inputs.get("database_name")
                && (!plugin_inputs.contains_key("database_name")
                    || plugin_inputs.get("database_name").and_then(|v| v.as_str()) == Some(""))
            {
                plugin_inputs.insert("database_name".to_string(), database_name.clone());
            }

            // Add first reference project name as a special variable for plugin templates
            // This allows plugins to construct data source names for remote state access
            plugin_inputs.insert(
                "_reference_project_name".to_string(),
                Value::String(ref_env_res.metadata.name.clone()),
            );
        }

        // 7. Render plugin files to modules directory inside target environment
        output::subsection("Adding Plugin");
        output::dimmed(&format!(
            "Adding plugin '{}'...",
            selected_plugin_with_projects
                .plugin_info
                .resource
                .metadata
                .name
        ));

        // Create module path: environment/modules/{template_pack_name}/{plugin_name}/[{reference_project_name}/]
        let mut module_path = target_env_path
            .join("modules")
            .join(&selected_plugin_with_projects.plugin_info.template_pack_name)
            .join(
                &selected_plugin_with_projects
                    .plugin_info
                    .resource
                    .metadata
                    .name,
            );

        // Add first reference project name to path if this plugin has dependencies
        if let Some((ref_env_res, _)) = selected_reference_projects.first() {
            module_path = module_path.join(&ref_env_res.metadata.name);
        }

        let renderer = TemplateRenderer::new();
        let plugin_context = Some((
            selected_plugin_with_projects
                .plugin_info
                .template_pack_name
                .as_str(),
            selected_plugin_with_projects
                .plugin_info
                .resource
                .metadata
                .name
                .as_str(),
        ));
        let generated_files = renderer
            .render_template(
                ctx,
                &selected_plugin_with_projects.plugin_info.path,
                &module_path,
                &plugin_inputs,
                plugin_context,
            )
            .context("Failed to render plugin files")?;

        // 8. Update target project's .pmp.environment.yaml to track the added plugin
        output::dimmed("  Updating .pmp.environment.yaml...");
        let mut env_resource = target_env_resource;

        // Initialize plugins field if it doesn't exist
        if env_resource.spec.plugins.is_none() {
            env_resource.spec.plugins = Some(ProjectPlugins::default());
        }

        // Add the plugin to the added list (with reference to the selected project, or target project if no reference)
        if let Some(plugins) = &mut env_resource.spec.plugins {
            let plugin_project_ref = if let Some((ref_env_res, _)) = selected_reference_projects.first() {
                // Plugin has reference projects - use first one
                PluginProjectReference {
                    api_version: ref_env_res.api_version.clone(),
                    kind: ref_env_res.kind.clone(),
                    name: ref_env_res.metadata.name.clone(),
                    environment: ref_env_res.metadata.environment_name.clone(),
                }
            } else {
                // Plugin doesn't require reference projects - use target project info
                PluginProjectReference {
                    api_version: env_resource.api_version.clone(),
                    kind: env_resource.kind.clone(),
                    name: env_resource.metadata.name.clone(),
                    environment: target_env_name.to_string(),
                }
            };

            // Create reference project metadata for ALL dependencies
            let reference_projects_metadata: Vec<AddedPluginReference> = selected_reference_projects
                .iter()
                .enumerate()
                .filter_map(|(idx, (ref_env, _))| {
                    // Get corresponding dependency from plugin spec
                    let dependency = selected_plugin_with_projects
                        .plugin_info
                        .resource
                        .spec
                        .dependencies
                        .get(idx)?;

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

            plugins.added.push(AddedPlugin {
                template_pack_name: selected_plugin_with_projects
                    .plugin_info
                    .template_pack_name
                    .clone(),
                name: selected_plugin_with_projects
                    .plugin_info
                    .resource
                    .metadata
                    .name
                    .clone(),
                project: plugin_project_ref,
                reference_projects: reference_projects_metadata,
                inputs: plugin_inputs.clone(),
                files: generated_files,
                plugin_spec: Some(
                    selected_plugin_with_projects
                        .plugin_info
                        .resource
                        .spec
                        .clone(),
                ),
                raw_module_inputs: selected_reference_projects
                    .first()
                    .and_then(|(_, config)| config.raw_module_inputs.clone()),
            });
        }

        // Save the updated environment file (target project's environment file)
        let target_env_file = target_env_path.join(".pmp.environment.yaml");
        let yaml_content = serde_yaml::to_string(&env_resource)
            .context("Failed to serialize environment resource to YAML")?;
        ctx.fs
            .write(&target_env_file, &yaml_content)
            .with_context(|| format!("Failed to write environment file: {:?}", target_env_file))?;

        // Regenerate common file to include the new plugin module (only for opentofu)
        if let Some(executor_config) = &collection.spec.executor
            && !executor_config.config.is_empty()
        {
            let project_executor_name = &env_resource.spec.executor.name;
            let executor: Box<dyn crate::executor::Executor> = match project_executor_name.as_str()
            {
                "opentofu" => Box::new(crate::executor::OpenTofuExecutor::new()),
                "none" => Box::new(crate::executor::NoneExecutor::new()),
                _ => anyhow::bail!("Unknown executor: {}", project_executor_name),
            };

            let plugins = env_resource
                .spec
                .plugins
                .as_ref()
                .map(|p| p.added.as_slice());
            let template_reference_projects = &env_resource.spec.template_reference_projects;
            let metadata = crate::executor::ProjectMetadata {
                api_version: &env_resource.api_version,
                kind: &env_resource.kind,
                environment: target_env_name,
                project_name: target_project_name,
            };
            executor
                .generate_common_file(
                    ctx,
                    target_env_path,
                    &executor_config.config,
                    &metadata,
                    plugins,
                    template_reference_projects,
                )
                .context("Failed to regenerate common file")?;
        }

        output::blank();
        output::success(&format!(
            "Plugin '{}' added successfully!",
            selected_plugin_with_projects
                .plugin_info
                .resource
                .metadata
                .name
        ));

        output::subsection("Next Steps");
        output::dimmed("The plugin has been added to:");
        output::key_value("Project", target_project_name);
        output::key_value("Environment", target_env_name);
        output::key_value("Environment path", &target_env_path.display().to_string());
        output::blank();
        if !selected_reference_projects.is_empty() {
            if selected_reference_projects.len() == 1 {
                let ref_proj_name = &selected_reference_projects[0].0.metadata.name;
                output::dimmed(&format!(
                    "This plugin provides access to reference project: {}",
                    ref_proj_name
                ));
            } else {
                output::dimmed(&format!(
                    "This plugin provides access to {} reference projects:",
                    selected_reference_projects.len()
                ));
                for (ref_env, _) in &selected_reference_projects {
                    output::dimmed(&format!("  - {}", ref_env.metadata.name));
                }
            }
            output::blank();
        }
        output::blank();
        output::dimmed("To apply the changes:");
        output::dimmed("  1. Run 'pmp preview' to see what will be created");
        output::dimmed("  2. Run 'pmp apply' to apply the infrastructure");

        Ok(())
    }

    /// Remove a plugin from the current environment
    #[allow(clippy::too_many_arguments)]
    fn remove_plugin_interactive(
        ctx: &crate::context::Context,
        _collection_root: &Path,
        collection: &InfrastructureResource,
        env_path: &Path,
        project_name: &str,
        env_name: &str,
        mut env_resource: DynamicProjectEnvironmentResource,
        _template_packs_paths: Option<&str>,
    ) -> Result<()> {
        output::section("Remove Plugin");

        // Check if there are any plugins to remove
        let plugins = match &env_resource.spec.plugins {
            Some(p) if !p.added.is_empty() => &p.added,
            _ => {
                output::warning("No plugins are currently added to this environment.");
                return Ok(());
            }
        };

        // Create display options for the user
        let plugin_options: Vec<String> = plugins
            .iter()
            .map(|p| {
                format!(
                    "{}/{} (referencing: {})",
                    p.template_pack_name, p.name, p.project.name
                )
            })
            .collect();

        output::subsection("Select Plugin to Remove");
        let selected_display = ctx
            .input
            .select(
                "Which plugin would you like to remove?",
                plugin_options.clone(),
                None,
            )
            .context("Failed to select plugin")?;

        let plugin_index = plugin_options
            .iter()
            .position(|opt| opt == &selected_display)
            .context("Plugin not found")?;

        // Clone plugin info to avoid borrowing issues
        let plugin_name = plugins[plugin_index].name.clone();
        let plugin_pack = plugins[plugin_index].template_pack_name.clone();
        let plugin_ref_project = plugins[plugin_index].project.name.clone();

        output::blank();
        output::key_value_highlight("Plugin", &plugin_name);
        output::key_value("Pack", &plugin_pack);
        output::key_value("Reference Project", &plugin_ref_project);

        // Confirm removal
        let confirm = ctx
            .input
            .confirm("Are you sure you want to remove this plugin?", Some(false))
            .context("Failed to get confirmation")?;

        if !confirm {
            output::dimmed("Plugin removal cancelled.");
            return Ok(());
        }

        output::blank();
        output::subsection("Removing Plugin");

        // Delete plugin directory
        let plugin_path = env_path
            .join("modules")
            .join(&plugin_pack)
            .join(&plugin_name)
            .join(&plugin_ref_project);

        if ctx.fs.exists(&plugin_path) {
            ctx.fs
                .remove_dir_all(&plugin_path)
                .with_context(|| format!("Failed to remove plugin directory: {:?}", plugin_path))?;
            output::dimmed(&format!("  Deleted: {}", plugin_path.display()));
        } else {
            output::warning(&format!(
                "Plugin directory not found (may have been manually deleted): {}",
                plugin_path.display()
            ));
        }

        // Remove plugin from environment resource
        if let Some(plugins) = &mut env_resource.spec.plugins {
            plugins.added.remove(plugin_index);
            output::dimmed("  Removed plugin from environment metadata");
        }

        // Save updated environment file
        let env_file = env_path.join(".pmp.environment.yaml");
        let yaml_content = serde_yaml::to_string(&env_resource)
            .context("Failed to serialize environment resource to YAML")?;
        ctx.fs
            .write(&env_file, &yaml_content)
            .with_context(|| format!("Failed to write environment file: {:?}", env_file))?;

        // Regenerate common file without the removed plugin (only for opentofu)
        if let Some(executor_config) = &collection.spec.executor
            && !executor_config.config.is_empty()
        {
            let project_executor_name = &env_resource.spec.executor.name;
            let executor: Box<dyn crate::executor::Executor> = match project_executor_name.as_str()
            {
                "opentofu" => Box::new(crate::executor::OpenTofuExecutor::new()),
                "none" => Box::new(crate::executor::NoneExecutor::new()),
                _ => anyhow::bail!("Unknown executor: {}", project_executor_name),
            };

            let plugins = env_resource
                .spec
                .plugins
                .as_ref()
                .map(|p| p.added.as_slice());
            let template_reference_projects = &env_resource.spec.template_reference_projects;
            let metadata = crate::executor::ProjectMetadata {
                api_version: &env_resource.api_version,
                kind: &env_resource.kind,
                environment: env_name,
                project_name,
            };
            executor
                .generate_common_file(
                    ctx,
                    env_path,
                    &executor_config.config,
                    &metadata,
                    plugins,
                    template_reference_projects,
                )
                .context("Failed to regenerate common file")?;
        }

        output::blank();
        output::success(&format!("Plugin '{}' removed successfully!", plugin_name));

        output::subsection("Next Steps");
        output::dimmed("The plugin has been removed from:");
        output::key_value("Project", project_name);
        output::key_value("Environment", env_name);
        output::blank();
        output::dimmed("To apply the changes:");
        output::dimmed("  1. Run 'pmp preview' to see what will be removed");
        output::dimmed("  2. Run 'pmp apply' to apply the infrastructure changes");

        Ok(())
    }

    /// Update inputs for an existing plugin
    #[allow(clippy::too_many_arguments)]
    fn update_plugin_inputs_interactive(
        ctx: &crate::context::Context,
        _collection_root: &Path,
        collection: &InfrastructureResource,
        env_path: &Path,
        project_name: &str,
        env_name: &str,
        mut env_resource: DynamicProjectEnvironmentResource,
        template_packs_paths: Option<&str>,
    ) -> Result<()> {
        output::section("Update Plugin Inputs");

        // Check if there are any plugins to update
        let plugins = match &env_resource.spec.plugins {
            Some(p) if !p.added.is_empty() => &p.added,
            _ => {
                output::warning("No plugins are currently added to this environment.");
                return Ok(());
            }
        };

        // Create display options for the user
        let plugin_options: Vec<String> = plugins
            .iter()
            .map(|p| {
                if let Some(first_ref) = p.reference_projects.first() {
                    format!(
                        "{}/{} (referencing: {})",
                        p.template_pack_name, p.name, first_ref.name
                    )
                } else {
                    format!("{}/{} (self-referencing)", p.template_pack_name, p.name)
                }
            })
            .collect();

        output::subsection("Select Plugin to Update");
        let selected_display = ctx
            .input
            .select(
                "Which plugin would you like to update?",
                plugin_options.clone(),
                None,
            )
            .context("Failed to select plugin")?;

        let plugin_index = plugin_options
            .iter()
            .position(|opt| opt == &selected_display)
            .context("Plugin not found")?;

        // Clone plugin info to avoid borrowing issues
        let plugin_to_update = plugins[plugin_index].clone();
        let plugin_name = plugin_to_update.name.clone();
        let plugin_pack = plugin_to_update.template_pack_name.clone();
        let current_inputs = plugin_to_update.inputs.clone();

        output::blank();
        output::key_value_highlight("Plugin", &plugin_name);
        output::key_value("Pack", &plugin_pack);

        // Discover template packs to reload the plugin spec
        output::blank();
        output::subsection("Loading Plugin Specification");

        // Build paths for discovery
        let mut all_paths: Vec<String> = vec![];

        // Add flag paths if provided
        if let Some(paths) = template_packs_paths {
            all_paths.extend(paths.split(':').map(|s| s.to_string()));
        }

        // Add environment variable paths
        if let Ok(env_paths_str) = std::env::var("PMP_TEMPLATE_PACKS_PATHS") {
            let env_paths: Vec<String> = env_paths_str.split(':').map(|s| s.to_string()).collect();
            all_paths.extend(env_paths);
        }

        // Convert to Vec<&str> for the discovery function
        let custom_paths: Vec<&str> = all_paths.iter().map(|s| s.as_str()).collect();

        let template_packs = TemplateDiscovery::discover_template_packs_with_custom_paths(
            &*ctx.fs,
            &*ctx.output,
            &custom_paths,
        )
        .context("Failed to discover template packs")?;

        // Find the plugin in the discovered packs
        let mut plugin_info_found: Option<crate::template::PluginInfo> = None;

        for pack in &template_packs {
            if pack.resource.metadata.name == plugin_pack {
                // Discover plugins in this pack
                let plugins = TemplateDiscovery::discover_plugins_in_pack(
                    &*ctx.fs,
                    &*ctx.output,
                    &pack.path,
                    &pack.resource.metadata.name,
                )?;

                // Find the specific plugin
                plugin_info_found = plugins
                    .into_iter()
                    .find(|p| p.resource.metadata.name == plugin_name);
                break;
            }
        }

        let plugin_info = plugin_info_found.context(format!(
            "Plugin '{}' from pack '{}' not found in template packs",
            plugin_name, plugin_pack
        ))?;

        output::dimmed(&format!(
            "  Loaded plugin specification: {}/{}",
            plugin_pack, plugin_name
        ));

        // Collect new inputs with current values as defaults
        output::blank();
        output::subsection("Update Plugin Inputs");
        output::dimmed("Current values shown as defaults. Press Enter to keep current value.");
        output::blank();

        let new_inputs = Self::collect_plugin_inputs_with_defaults(
            ctx,
            &plugin_info.resource.spec.inputs,
            &current_inputs,
            project_name,
            env_name,
        )?;

        // Confirm update
        output::blank();
        let confirm = ctx
            .input
            .confirm("Update plugin with these new inputs?", Some(true))
            .context("Failed to get confirmation")?;

        if !confirm {
            output::dimmed("Plugin update cancelled.");
            return Ok(());
        }

        output::blank();
        output::subsection("Updating Plugin");

        // Determine plugin path (with or without reference project subdirectory)
        let mut plugin_path = env_path
            .join("modules")
            .join(&plugin_pack)
            .join(&plugin_name);

        if let Some(first_ref) = plugin_to_update.reference_projects.first() {
            plugin_path = plugin_path.join(&first_ref.name);
        }

        // Re-render plugin files with new inputs
        let renderer = crate::template::TemplateRenderer::new();
        let plugin_context = Some((plugin_pack.as_str(), plugin_name.as_str()));

        let generated_files = renderer
            .render_template(
                ctx,
                &plugin_info.path,
                &plugin_path,
                &new_inputs,
                plugin_context,
            )
            .context("Failed to re-render plugin files")?;

        output::dimmed(&format!(
            "  Regenerated {} file(s) in: {}",
            generated_files.len(),
            plugin_path.display()
        ));

        // Update plugin in environment resource
        if let Some(plugins) = &mut env_resource.spec.plugins {
            plugins.added[plugin_index].inputs = new_inputs;
            plugins.added[plugin_index].files = generated_files;
            output::dimmed("  Updated plugin metadata in environment");
        }

        // Save updated environment file
        let env_file = env_path.join(".pmp.environment.yaml");
        let yaml_content = serde_yaml::to_string(&env_resource)
            .context("Failed to serialize environment resource to YAML")?;
        ctx.fs
            .write(&env_file, &yaml_content)
            .with_context(|| format!("Failed to write environment file: {:?}", env_file))?;
        output::dimmed(&format!("  Updated: {}", env_file.display()));

        // Regenerate common file with updated plugin (only for opentofu)
        if let Some(executor_config) = &collection.spec.executor
            && !executor_config.config.is_empty()
        {
            let project_executor_name = &env_resource.spec.executor.name;
            let executor: Box<dyn crate::executor::Executor> = match project_executor_name.as_str()
            {
                "opentofu" => Box::new(crate::executor::OpenTofuExecutor::new()),
                "none" => Box::new(crate::executor::NoneExecutor::new()),
                _ => anyhow::bail!("Unknown executor: {}", project_executor_name),
            };

            let plugins = env_resource
                .spec
                .plugins
                .as_ref()
                .map(|p| p.added.as_slice());
            let template_reference_projects = &env_resource.spec.template_reference_projects;
            let metadata = crate::executor::ProjectMetadata {
                api_version: &env_resource.api_version,
                kind: &env_resource.kind,
                environment: env_name,
                project_name,
            };
            executor
                .generate_common_file(
                    ctx,
                    env_path,
                    &executor_config.config,
                    &metadata,
                    plugins,
                    template_reference_projects,
                )
                .context("Failed to regenerate common file")?;
        }

        output::blank();
        output::success(&format!("Plugin '{}' updated successfully!", plugin_name));

        output::subsection("Next Steps");
        output::dimmed("The plugin inputs have been updated for:");
        output::key_value("Project", project_name);
        output::key_value("Environment", env_name);
        output::blank();
        output::dimmed("To apply the changes:");
        output::dimmed("  1. Run 'pmp preview' to see what will be changed");
        output::dimmed("  2. Run 'pmp apply' to apply the infrastructure changes");

        Ok(())
    }

    /// Detect context and select project/environment
    /// Returns: (environment_path, project_name, environment_name)
    fn detect_and_select_environment(
        ctx: &crate::context::Context,
        work_dir: &Path,
    ) -> Result<(PathBuf, String, String)> {
        // Check if we're in an environment directory
        if let Some(env_info) = Self::check_in_environment(ctx, work_dir)? {
            return Ok(env_info);
        }

        // Check if we're in a project directory
        if let Some((project_path, project_name)) = Self::check_in_project(ctx, work_dir)? {
            let env_name = Self::select_environment(ctx, &project_path)?;
            let env_path = project_path.join("environments").join(&env_name);
            return Ok((env_path, project_name, env_name));
        }

        // We're in the collection or elsewhere - use find/search UI
        Self::select_project_and_environment(ctx)
    }

    /// Check if we're inside an environment directory
    fn check_in_environment(
        ctx: &crate::context::Context,
        dir: &Path,
    ) -> Result<Option<(PathBuf, String, String)>> {
        let env_file = dir.join(".pmp.environment.yaml");

        if ctx.fs.exists(&env_file) {
            let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;
            let env_name = resource.metadata.environment_name.clone();
            let project_name = resource.metadata.name.clone();

            return Ok(Some((dir.to_path_buf(), project_name, env_name)));
        }

        Ok(None)
    }

    /// Check if we're inside a project directory (but not in an environment)
    fn check_in_project(
        ctx: &crate::context::Context,
        dir: &Path,
    ) -> Result<Option<(PathBuf, String)>> {
        let project_file = dir.join(".pmp.project.yaml");

        if ctx.fs.exists(&project_file) {
            let resource = ProjectResource::from_file(&*ctx.fs, &project_file)?;
            return Ok(Some((dir.to_path_buf(), resource.metadata.name.clone())));
        }

        Ok(None)
    }

    /// Select an environment from a project
    fn select_environment(ctx: &crate::context::Context, project_path: &Path) -> Result<String> {
        let environments = CollectionDiscovery::discover_environments(&*ctx.fs, project_path)
            .context("Failed to discover environments")?;

        if environments.is_empty() {
            anyhow::bail!("No environments found in project: {:?}", project_path);
        }

        if environments.len() == 1 {
            output::environment_badge(&environments[0]);
            return Ok(environments[0].clone());
        }

        let selected = ctx
            .input
            .select("Select an environment:", environments.clone(), None)
            .context("Failed to select environment")?;

        Ok(selected)
    }

    /// Select project and environment using find/search UI
    fn select_project_and_environment(
        ctx: &crate::context::Context,
    ) -> Result<(PathBuf, String, String)> {
        let manager = CollectionManager::load(ctx).context("Failed to load collection")?;

        let all_projects = manager.get_all_projects();

        if all_projects.is_empty() {
            anyhow::bail!("No projects found in collection");
        }

        // Select project
        // Sort projects by name for consistent display
        let mut sorted_projects: Vec<_> = all_projects.iter().collect();
        sorted_projects.sort_by(|a, b| a.name.cmp(&b.name));

        let project_options: Vec<String> = sorted_projects
            .iter()
            .map(|p| format!("{} ({})", p.name, p.kind))
            .collect();

        let selected_project_display = ctx
            .input
            .select("Select a project:", project_options.clone(), None)
            .context("Failed to select project")?;

        let project_index = project_options
            .iter()
            .position(|opt| opt == &selected_project_display)
            .context("Project not found")?;

        let selected_project = sorted_projects[project_index];
        let project_path = manager.get_project_path(selected_project);

        // Select environment
        let env_name = Self::select_environment(ctx, &project_path)?;
        let env_path = project_path.join("environments").join(&env_name);

        Ok((env_path, selected_project.name.clone(), env_name))
    }

    /// Collect inputs from user based on template input specifications, using current values as defaults
    fn collect_template_inputs_with_defaults(
        ctx: &crate::context::Context,
        inputs_spec: &[crate::template::metadata::InputDefinition],
        current_inputs: &std::collections::HashMap<String, serde_json::Value>,
        project_name: &str,
        environment_name: &str,
        predefined_inputs: Option<&HashMap<String, Value>>,
    ) -> Result<std::collections::HashMap<String, serde_json::Value>> {
        use crate::template::utils::{interpolate_all, interpolate_value_all};

        let mut inputs = std::collections::HashMap::new();

        // Add project name variables (underscore and hyphen versions)
        let project_name_underscores = project_name.replace('-', "_");
        let project_name_hyphens = project_name.replace('_', "-");
        inputs.insert(
            "_project_name_underscores".to_string(),
            serde_json::Value::String(project_name_underscores.clone()),
        );
        inputs.insert(
            "_project_name_hyphens".to_string(),
            serde_json::Value::String(project_name_hyphens.clone()),
        );

        // Collect each input defined in the template
        for input_def in inputs_spec {
            // Skip project name variables
            if input_def.name == "_project_name_underscores"
                || input_def.name == "_project_name_hyphens"
            {
                continue;
            }

            // Check if there's a predefined value for this input
            if let Some(predefined) = predefined_inputs.and_then(|p| p.get(&input_def.name)) {
                // Build vars for interpolation
                let mut vars = std::collections::HashMap::new();
                vars.insert(
                    "_project_name_underscores".to_string(),
                    serde_json::Value::String(project_name_underscores.clone()),
                );
                vars.insert(
                    "_project_name_hyphens".to_string(),
                    serde_json::Value::String(project_name_hyphens.clone()),
                );
                vars.insert(
                    "_environment_name".to_string(),
                    serde_json::Value::String(environment_name.to_string()),
                );
                for (key, value) in &inputs {
                    vars.insert(key.clone(), value.clone());
                }
                // Use the predefined value directly (with variable interpolation)
                let value = interpolate_value_all(predefined, &vars)?;
                inputs.insert(input_def.name.clone(), value);
                continue;
            }

            // Check if input should be shown based on conditions
            if !input_def.should_show(&inputs) {
                // Conditions not met, use default value if available
                if let Some(default) = &input_def.default {
                    // Build vars for interpolation
                    let mut vars = std::collections::HashMap::new();
                    vars.insert(
                        "_project_name_underscores".to_string(),
                        serde_json::Value::String(project_name_underscores.clone()),
                    );
                    vars.insert(
                        "_project_name_hyphens".to_string(),
                        serde_json::Value::String(project_name_hyphens.clone()),
                    );
                    vars.insert(
                        "_environment_name".to_string(),
                        serde_json::Value::String(environment_name.to_string()),
                    );
                    for (key, value) in &inputs {
                        vars.insert(key.clone(), value.clone());
                    }
                    // Interpolate variables in the default value
                    let interpolated_value = interpolate_value_all(default, &vars)?;
                    inputs.insert(input_def.name.clone(), interpolated_value);
                }
                continue; // Skip prompting for this input
            }

            // Get variables for interpolation
            let mut vars = std::collections::HashMap::new();
            vars.insert(
                "_project_name_underscores".to_string(),
                serde_json::Value::String(project_name_underscores.clone()),
            );
            vars.insert(
                "_project_name_hyphens".to_string(),
                serde_json::Value::String(project_name_hyphens.clone()),
            );

            // Add environment name
            vars.insert(
                "_environment_name".to_string(),
                serde_json::Value::String(environment_name.to_string()),
            );

            for (key, value) in &inputs {
                vars.insert(key.clone(), value.clone());
            }

            // Interpolate variables in the description (supports both ${env:...} and ${var:...})
            let description = if let Some(desc) = &input_def.description {
                interpolate_all(desc, &vars)?
            } else {
                input_def.name.to_string()
            };

            // Get the current value for this input
            let current_value = current_inputs.get(&input_def.name);

            // Interpolate the default value (supports both ${env:...} and ${var:...})
            let interpolated_default = if let Some(default) = &input_def.default {
                Some(interpolate_value_all(default, &vars)?)
            } else {
                None
            };

            let value = if let Some(input_type) = &input_def.input_type {
                // Handle based on input type
                match input_type {
                    crate::template::metadata::InputType::Select { options } => {
                        // Build list of display labels
                        let labels: Vec<String> = options.iter().map(|opt| opt.label.clone()).collect();

                        // Get current value string
                        let current_value_str = current_value.and_then(|v| v.as_str());
                        let default_value_str = interpolated_default.as_ref().and_then(|v| v.as_str());

                        // Find default index based on current value or default
                        let default_idx = current_value_str
                            .or(default_value_str)
                            .and_then(|val| options.iter().position(|opt| opt.value == val));

                        let selected_label = ctx
                            .input
                            .select(&description, labels, default_idx)
                            .context("Failed to get input")?;

                        // Find the corresponding value
                        let selected_option = options
                            .iter()
                            .find(|opt| opt.label == selected_label)
                            .ok_or_else(|| anyhow::anyhow!("Selected option not found"))?;

                        serde_json::Value::String(selected_option.value.clone())
                    }
                    _ => {
                        // For other input types, fall back to value-based handling
                        let default_value = current_value.or(interpolated_default.as_ref());
                        Self::collect_input_by_value(ctx, &description, default_value)?
                    }
                }
            } else if let Some(enum_values) = &input_def.enum_values {
                // Deprecated: This is a select input using old format
                // Sort enum values alphabetically for display
                let mut sorted_enum_values = enum_values.clone();
                sorted_enum_values.sort();

                let default_str = current_value
                    .and_then(|v| v.as_str())
                    .or_else(|| interpolated_default.as_ref().and_then(|v| v.as_str()))
                    .or_else(|| sorted_enum_values.first().map(|s| s.as_str()));

                // Find the default value index in the sorted list
                let default_index = default_str.and_then(|default_val| {
                    sorted_enum_values.iter().position(|v| v == default_val)
                });

                let selected = ctx
                    .input
                    .select(&description, sorted_enum_values, default_index)
                    .context("Failed to get input")?;

                serde_json::Value::String(selected)
            } else {
                // Determine the default value (prefer current value over template default)
                let default_value = current_value.or(interpolated_default.as_ref());
                Self::collect_input_by_value(ctx, &description, default_value)?
            };

            inputs.insert(input_def.name.clone(), value);
        }

        Ok(inputs)
    }

    /// Find the original template used to create a project
    ///
    /// Uses the template pack name and template name stored in the environment resource's
    /// spec.template field to locate the exact template that was originally used.
    fn find_original_template(
        ctx: &crate::context::Context,
        env_resource: &DynamicProjectEnvironmentResource,
        template_packs_paths: Option<&str>,
    ) -> Result<TemplateInfo> {
        // Get original template info from environment resource
        let template_ref = env_resource.spec.template.as_ref()
            .context("Environment resource missing template reference. This environment may have been created with an older version of PMP.")?;

        let original_pack_name = &template_ref.template_pack_name;
        let original_template_name = &template_ref.name;

        // Discover template packs
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

        // Find the exact template pack and template that was originally used
        for pack in all_template_packs {
            // Only check packs that match the original template pack name
            if pack.resource.metadata.name != *original_pack_name {
                continue;
            }

            let templates_in_pack =
                TemplateDiscovery::discover_templates_in_pack(&*ctx.fs, &*ctx.output, &pack.path)?;

            // Find template matching both the name and resource kind
            if let Some(template) = templates_in_pack.into_iter().find(|t| {
                t.resource.metadata.name == *original_template_name
                    && t.resource.spec.api_version == env_resource.api_version
                    && t.resource.spec.kind == env_resource.kind
            }) {
                return Ok(template);
            }
        }

        anyhow::bail!(
            "Original template not found: pack='{}', template='{}', kind='{}/{}'.\n\
             The template pack may have been moved, renamed, or deleted.",
            original_pack_name,
            original_template_name,
            env_resource.api_version,
            env_resource.kind
        )
    }

    /// Collect inputs for a single installed plugin (for update command)
    #[allow(clippy::too_many_arguments)]
    fn collect_plugin_info_for_update(
        ctx: &crate::context::Context,
        installed_config: &crate::template::metadata::AllowedPluginConfig,
        template_packs: &[crate::template::TemplatePackInfo],
        projects: &[crate::template::metadata::ProjectReference],
        collection_root: &Path,
        project_name: &str,
        environment_name: &str,
        current_env_resource: &DynamicProjectEnvironmentResource,
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
        let reference_projects_and_envs: Vec<(crate::template::metadata::ProjectReference, crate::template::metadata::DynamicProjectEnvironmentResource)> =
            if !plugin_info.resource.spec.dependencies.is_empty() {
                let mut refs = Vec::new();

                for dependency in &plugin_info.resource.spec.dependencies {
                    let dependency_ref = &dependency.project;

                    // First, check if the current project being updated matches the requirements (self-reference)
                    let current_project_matches = current_env_resource.api_version
                        == dependency_ref.api_version
                        && current_env_resource.kind == dependency_ref.kind
                        && if !dependency_ref.label_selector.is_empty() {
                            // Check if current project's labels match the selector
                            dependency_ref.label_selector.iter().all(|(key, value)| {
                                current_env_resource
                                    .metadata
                                    .labels
                                    .get(key)
                                    .map(|v| v == value)
                                    .unwrap_or(false)
                            })
                        } else {
                            true // No label selector, so it matches
                        };

                    if current_project_matches {
                        // Use the current project as a self-reference
                        ctx.output.dimmed(&format!(
                            "  Plugin requires reference to a {} project - using current project (self-reference)",
                            dependency_ref.kind
                        ));

                        // Create a ProjectReference for the current project
                        let current_project_ref = crate::template::metadata::ProjectReference {
                            name: project_name.to_string(),
                            kind: current_env_resource.kind.clone(),
                            path: format!("projects/{}", project_name), // Standard path format
                            labels: current_env_resource.metadata.labels.clone(),
                        };

                        refs.push((current_project_ref, current_env_resource.clone()));
                    } else {
                // Find compatible projects (same logic as create command)
                let compatible_projects: Vec<_> = projects.iter()
                .filter_map(|project| {
                    let project_path = collection_root.join(&project.path);
                    let environments_dir = project_path.join("environments");

                    if let Ok(env_entries) = ctx.fs.read_dir(&environments_dir) {
                        for env_path in env_entries {
                            let env_file = env_path.join(".pmp.environment.yaml");
                            if ctx.fs.exists(&env_file)
                                && let Ok(env_resource) = crate::template::metadata::DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
                                    && env_resource.api_version == dependency_ref.api_version
                                        && env_resource.kind == dependency_ref.kind
                                    {
                                        // Check label selectors if provided
                                        if !dependency_ref.label_selector.is_empty() {
                                            // Check labels from the environment resource (not the project file)
                                            if !env_resource.metadata.labels.is_empty() {
                                                // All required labels must match
                                                let matches = dependency_ref.label_selector.iter().all(|(key, value)| {
                                                    env_resource.metadata.labels.get(key).map(|v| v == value).unwrap_or(false)
                                                });
                                                if !matches {
                                                    continue;
                                                }
                                            } else {
                                                // Environment has no labels, can't match selector
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

                        if compatible_projects.is_empty() {
                            let _dep_display = dependency.dependency_name.as_deref()
                                .unwrap_or(&dependency_ref.kind);
                            ctx.output.warning(&format!(
                                "  Plugin '{}' requires a {} project{}, but none found. Skipping.",
                                installed_config.plugin_name,
                                dependency_ref.kind,
                                dependency.dependency_name.as_ref()
                                    .map(|n| format!(" (dependency: {})", n))
                                    .unwrap_or_default()
                            ));
                            return Ok(None);
                        }

                        // Let user select a compatible project
                        let project_names: Vec<String> = compatible_projects
                            .iter()
                            .map(|(p, env)| {
                                // Show project name with environment and labels if available
                                let mut parts =
                                    vec![format!("{} ({})", p.name, env.metadata.environment_name)];
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

                        let selected_display = ctx
                            .input
                            .select("  Select reference project:", project_names.clone(), None)?;

                        // Find the matching project by display name
                        let selected_idx = project_names
                            .iter()
                            .position(|name| name == &selected_display)
                            .context("Selected project not found in list")?;

                        let (selected_project, selected_env) = &compatible_projects[selected_idx];
                        refs.push((selected_project.clone(), selected_env.clone()));
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
            Self::build_default_plugin_inputs(&merged_inputs, project_name, environment_name)?
        } else {
            // Ask user if they want to customize inputs
            let customize = ctx
                .input
                .confirm("  Do you want to customize inputs for this plugin?", Some(false))?;

            if customize {
                ctx.output.dimmed("  Collecting plugin inputs...");
                Self::collect_plugin_inputs(ctx, &merged_inputs, project_name, environment_name)?
            } else {
                // Use defaults
                ctx.output.dimmed("  Using default values...");
                Self::build_default_plugin_inputs(&merged_inputs, project_name, environment_name)?
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

    /// Load plugin spec from template pack
    /// Returns None if plugin not found
    fn load_plugin_spec(
        ctx: &crate::context::Context,
        template_packs: &[crate::template::TemplatePackInfo],
        template_pack_name: &str,
        plugin_name: &str,
    ) -> Option<crate::template::metadata::PluginSpec> {
        // Find the template pack
        let template_pack = template_packs
            .iter()
            .find(|pack| pack.resource.metadata.name == template_pack_name)?;

        // Discover plugins in this template pack
        let plugins = TemplateDiscovery::discover_plugins_in_pack(
            &*ctx.fs,
            &*ctx.output,
            &template_pack.path,
            &template_pack.resource.metadata.name,
        )
        .ok()?;

        // Find the specific plugin
        let plugin_info = plugins
            .iter()
            .find(|p| p.resource.metadata.name == plugin_name)?;

        Some(plugin_info.resource.spec.clone())
    }

    /// Build default inputs for plugins without prompting user
    fn build_default_plugin_inputs(
        inputs_spec: &[crate::template::metadata::InputDefinition],
        project_name: &str,
        environment_name: &str,
    ) -> Result<HashMap<String, Value>> {
        let mut inputs = HashMap::new();

        // Add project name variables (underscore and hyphen versions)
        let project_name_underscores = project_name.replace('-', "_");
        let project_name_hyphens = project_name.replace('_', "-");
        inputs.insert(
            "_project_name_underscores".to_string(),
            Value::String(project_name_underscores.clone()),
        );
        inputs.insert(
            "_project_name_hyphens".to_string(),
            Value::String(project_name_hyphens.clone()),
        );

        for input_def in inputs_spec {
            if input_def.name == "_project_name_underscores"
                || input_def.name == "_project_name_hyphens"
            {
                continue;
            }

            if let Some(default) = &input_def.default {
                // Build variables map for interpolation
                let mut vars = HashMap::new();
                vars.insert(
                    "_project_name_underscores".to_string(),
                    Value::String(project_name_underscores.clone()),
                );
                vars.insert(
                    "_project_name_hyphens".to_string(),
                    Value::String(project_name_hyphens.clone()),
                );
                vars.insert(
                    "_environment_name".to_string(),
                    Value::String(environment_name.to_string()),
                );

                for (key, value) in &inputs {
                    vars.insert(key.clone(), value.clone());
                }

                // Interpolate both ${env:...} and ${var:...} patterns in the default value
                let interpolated_value =
                    crate::template::utils::interpolate_value_all(default, &vars)?;

                inputs.insert(input_def.name.clone(), interpolated_value);
            }
        }

        Ok(inputs)
    }

    /// Merge template dependencies with existing template reference projects
    /// Returns a list containing both existing references and newly added ones
    fn merge_template_dependencies(
        ctx: &crate::context::Context,
        template: &crate::template::metadata::TemplateResource,
        current_env: &DynamicProjectEnvironmentResource,
        environment_name: &str,
        environment_path: &Path,
    ) -> Result<Vec<crate::template::metadata::TemplateReferenceProject>> {
        use crate::collection::CollectionDiscovery;
        use crate::output;

        let mut merged_refs = current_env.spec.template_reference_projects.clone();

        // If template has no dependencies, just return existing references
        if template.spec.dependencies.is_empty() {
            return Ok(merged_refs);
        }

        // Find new dependencies that don't exist in current environment
        let mut new_dependencies = Vec::new();
        for dep in &template.spec.dependencies {
            // Calculate the actual data_source_name that would be generated
            // This matches the logic in lines 2663-2668 below
            let template_ref = &dep.project;
            let calculated_data_source_name = template_ref
                .remote_state
                .as_ref()
                .map(|rs| rs.data_source_name.clone())
                .or_else(|| dep.dependency_name.clone())
                .unwrap_or_else(|| format!("ref_{}", merged_refs.len()));

            let exists = merged_refs.iter().any(|existing_ref| {
                // Check if dependency already exists by matching the actual data_source_name
                existing_ref.data_source_name == calculated_data_source_name
            });

            if !exists {
                new_dependencies.push(dep);
            }
        }

        // If no new dependencies, return existing references
        if new_dependencies.is_empty() {
            return Ok(merged_refs);
        }

        // Get infrastructure root from environment path
        let infrastructure_root = environment_path
            .parent() // environments dir
            .and_then(|p| p.parent()) // project dir
            .and_then(|p| p.parent()) // projects dir
            .and_then(|p| p.parent()) // infrastructure root
            .ok_or_else(|| anyhow::anyhow!("Failed to determine infrastructure root"))?;

        output::subsection("New Template Dependencies Detected");
        output::dimmed(&format!(
            "Template requires {} new reference project(s).",
            new_dependencies.len()
        ));
        output::blank();

        // Discover all projects in the collection
        let projects = CollectionDiscovery::discover_projects(
            &*ctx.fs,
            &*ctx.output,
            infrastructure_root,
        )?;

        // Process each new dependency
        for (ref_index, dep) in new_dependencies.iter().enumerate() {
            let template_ref = &dep.project;
            let ref_number = ref_index + 1;
            let total_refs = new_dependencies.len();

            // Show description if available, otherwise show kind and label selectors
            if let Some(desc) = &template_ref.description {
                output::dimmed(&format!("[{}/{}] {}", ref_number, total_refs, desc));
            } else {
                output::dimmed(&format!(
                    "[{}/{}] Reference project matching apiVersion: {}, kind: {}",
                    ref_number, total_refs, template_ref.api_version, template_ref.kind
                ));
            }

            // Select reference project
            let (reference_project_name, reference_env_name) =
                Self::select_reference_project(ctx, template_ref, &projects, environment_name)?;

            // Load the reference environment resource
            let reference_project_path = infrastructure_root
                .join("projects")
                .join(&reference_project_name);
            let reference_env_path = reference_project_path
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
                .or_else(|| dep.dependency_name.clone())
                .unwrap_or_else(|| format!("ref_{}", merged_refs.len()));

            merged_refs.push(crate::template::metadata::TemplateReferenceProject {
                api_version: loaded_env_resource.api_version.clone(),
                kind: loaded_env_resource.kind.clone(),
                name: loaded_env_resource.metadata.name.clone(),
                environment: reference_env_name,
                data_source_name,
            });

            output::blank();
        }

        Ok(merged_refs)
    }

    /// Select a reference project matching the template requirements
    fn select_reference_project(
        ctx: &crate::context::Context,
        template_ref: &crate::template::metadata::TemplateProjectRef,
        projects: &[crate::template::metadata::ProjectReference],
        _environment_name: &str,
    ) -> Result<(String, String)> {
        use crate::collection::CollectionDiscovery;
        use crate::output;

        // Get infrastructure root and infrastructure resource
        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .ok_or_else(|| anyhow::anyhow!("Failed to find infrastructure collection"))?;

        // Filter projects by required apiVersion and kind
        let mut compatible_projects = Vec::new();
        for project in projects {
            let project_path = infrastructure_root.join(&project.path);
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
                    {
                        // Check apiVersion/kind match AND template is in category tree
                        // Also check labels - match against EITHER project labels OR environment resource labels
                        if env_resource.api_version == template_ref.api_version
                            && env_resource.kind == template_ref.kind
                            && (Self::labels_match(&project.labels, &template_ref.label_selector)
                                || Self::labels_match(
                                    &env_resource.metadata.labels,
                                    &template_ref.label_selector,
                                ))
                            && env_resource
                                .spec
                                .template
                                .as_ref()
                                .map(|t| {
                                    infrastructure.is_template_in_category_tree(
                                        &t.template_pack_name,
                                        &t.name,
                                    )
                                })
                                .unwrap_or(false)
                        {
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
                    format!("{} ({})", proj.name, labels_str)
                } else {
                    proj.name.clone()
                }
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
        output::key_value_highlight("Reference Project", &selected_project.name);
        output::key_value("Resource Kind", &template_ref.kind);

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
        output::key_value("Reference Environment", &reference_env_name);

        Ok((selected_project.name.clone(), reference_env_name))
    }

    /// Check if project labels match the label selector
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

    /// Generate the .pmp.environment.yaml file for the project environment (with spec)
    #[allow(clippy::too_many_arguments)]
    fn generate_project_environment_yaml(
        ctx: &crate::context::Context,
        environment_path: &Path,
        environment_name: &str,
        project_name: &str,
        template: &crate::template::metadata::TemplateResource,
        inputs: &std::collections::HashMap<String, serde_json::Value>,
        template_pack_name: &str,
        template_name: &str,
        merged_plugins: Option<&crate::template::metadata::ProjectPlugins>,
        current_env: &DynamicProjectEnvironmentResource,
        merged_template_reference_projects: &[crate::template::metadata::TemplateReferenceProject],
    ) -> Result<()> {
        use crate::template::metadata::{
            DynamicProjectEnvironmentMetadata, DynamicProjectEnvironmentResource,
            EnvironmentReference, ProjectSpec, ResourceDefinition, TemplateReference,
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
                // Preserve existing labels if present, otherwise use template labels
                labels: if !current_env.metadata.labels.is_empty() {
                    current_env.metadata.labels.clone()
                } else {
                    template.metadata.labels.clone()
                },
            },
            spec: ProjectSpec {
                resource: ResourceDefinition {
                    api_version: template.spec.api_version.clone(),
                    kind: template.spec.kind.clone(),
                },
                executor: crate::template::metadata::ExecutorProjectConfig {
                    name: template.spec.executor.name().to_string(),
                    config: template.spec.executor.config().cloned(),
                },
                inputs: inputs.clone(),
                custom: None, // Templates no longer have custom field
                plugins: merged_plugins.cloned(), // Use merged plugins (existing + newly added)
                template: Some(TemplateReference {
                    template_pack_name: template_pack_name.to_string(),
                    name: template_name.to_string(),
                }),
                environment: Some(EnvironmentReference {
                    name: environment_name.to_string(),
                }),
                template_reference_projects: merged_template_reference_projects.to_vec(), // Use merged list
                dependencies: current_env.spec.dependencies.clone(), // Preserve from current env
                projects: current_env.spec.projects.clone(),         // Preserve from current env
                hooks: current_env
                    .spec
                    .hooks
                    .clone()
                    .or_else(|| template.spec.hooks.clone()), // Preserve existing hooks, or use template hooks
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

        output::dimmed(&format!("  Updated: {}", pmp_env_yaml_path.display()));

        Ok(())
    }

    /// Collect plugin inputs from user
    fn collect_plugin_inputs(
        ctx: &crate::context::Context,
        inputs_spec: &[crate::template::metadata::InputDefinition],
        project_name: &str,
        environment_name: &str,
    ) -> Result<HashMap<String, Value>> {
        use crate::template::utils::{interpolate_all, interpolate_value_all};

        let mut inputs = HashMap::new();

        // Add project name variables (underscore and hyphen versions)
        let project_name_underscores = project_name.replace('-', "_");
        let project_name_hyphens = project_name.replace('_', "-");
        inputs.insert(
            "_project_name_underscores".to_string(),
            Value::String(project_name_underscores.clone()),
        );
        inputs.insert(
            "_project_name_hyphens".to_string(),
            Value::String(project_name_hyphens.clone()),
        );

        // Collect each input defined in the plugin
        for input_def in inputs_spec {
            // Skip project name variables
            if input_def.name == "_project_name_underscores"
                || input_def.name == "_project_name_hyphens"
            {
                continue;
            }

            // Get variables for interpolation
            let mut vars = HashMap::new();
            vars.insert(
                "_project_name_underscores".to_string(),
                Value::String(project_name_underscores.clone()),
            );
            vars.insert(
                "_project_name_hyphens".to_string(),
                Value::String(project_name_hyphens.clone()),
            );

            // Add environment name
            vars.insert(
                "_environment_name".to_string(),
                Value::String(environment_name.to_string()),
            );

            for (key, value) in &inputs {
                vars.insert(key.clone(), value.clone());
            }

            // Interpolate variables in the description (supports both ${env:...} and ${var:...})
            let description = if let Some(desc) = &input_def.description {
                interpolate_all(desc, &vars)?
            } else {
                input_def.name.to_string()
            };

            // Interpolate variables in the default value (supports both ${env:...} and ${var:...})
            let interpolated_default = if let Some(default) = &input_def.default {
                Some(interpolate_value_all(default, &vars)?)
            } else {
                None
            };

            let value = if let Some(input_type) = &input_def.input_type {
                // Handle based on input type
                match input_type {
                    crate::template::metadata::InputType::Select { options } => {
                        // Build list of display labels
                        let labels: Vec<String> = options.iter().map(|opt| opt.label.clone()).collect();

                        // Get default value string
                        let default_value_str = interpolated_default.as_ref().and_then(|v| v.as_str());

                        // Find default index based on default value
                        let default_idx = default_value_str
                            .and_then(|val| options.iter().position(|opt| opt.value == val));

                        let selected_label = ctx
                            .input
                            .select(&description, labels, default_idx)
                            .context("Failed to get input")?;

                        // Find the corresponding value
                        let selected_option = options
                            .iter()
                            .find(|opt| opt.label == selected_label)
                            .ok_or_else(|| anyhow::anyhow!("Selected option not found"))?;

                        Value::String(selected_option.value.clone())
                    }
                    _ => {
                        // For other input types, fall back to value-based handling
                        Self::collect_input_by_value(ctx, &description, interpolated_default.as_ref())?
                    }
                }
            } else if let Some(enum_values) = &input_def.enum_values {
                // Deprecated: This is a select input using old format
                // Sort enum values alphabetically for display
                let mut sorted_enum_values = enum_values.clone();
                sorted_enum_values.sort();

                let default_str = interpolated_default
                    .as_ref()
                    .and_then(|v| v.as_str())
                    .or_else(|| sorted_enum_values.first().map(|s| s.as_str()));

                // Find the default value index in the sorted list
                let default_index = default_str.and_then(|default_val| {
                    sorted_enum_values.iter().position(|v| v == default_val)
                });

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
                    Value::Null => {
                        // Null default is treated as no default
                        let answer = ctx
                            .input
                            .text(&description, None)
                            .context("Failed to get input")?;
                        Value::String(answer)
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

    /// Collect plugin inputs from user with current values as defaults
    fn collect_plugin_inputs_with_defaults(
        ctx: &crate::context::Context,
        inputs_spec: &[crate::template::metadata::InputDefinition],
        current_inputs: &HashMap<String, Value>,
        project_name: &str,
        environment_name: &str,
    ) -> Result<HashMap<String, Value>> {
        use crate::template::utils::{interpolate_all, interpolate_value_all};

        let mut inputs = HashMap::new();

        // Add project name variables (underscore and hyphen versions)
        let project_name_underscores = project_name.replace('-', "_");
        let project_name_hyphens = project_name.replace('_', "-");
        inputs.insert(
            "_project_name_underscores".to_string(),
            Value::String(project_name_underscores.clone()),
        );
        inputs.insert(
            "_project_name_hyphens".to_string(),
            Value::String(project_name_hyphens.clone()),
        );

        // Collect each input defined in the plugin
        for input_def in inputs_spec {
            // Skip project name variables
            if input_def.name == "_project_name_underscores"
                || input_def.name == "_project_name_hyphens"
            {
                continue;
            }

            // Get variables for interpolation
            let mut vars = HashMap::new();
            vars.insert(
                "_project_name_underscores".to_string(),
                Value::String(project_name_underscores.clone()),
            );
            vars.insert(
                "_project_name_hyphens".to_string(),
                Value::String(project_name_hyphens.clone()),
            );

            // Add environment name
            vars.insert(
                "_environment_name".to_string(),
                Value::String(environment_name.to_string()),
            );

            for (key, value) in &inputs {
                vars.insert(key.clone(), value.clone());
            }

            // Interpolate variables in the description (supports both ${env:...} and ${var:...})
            let description = if let Some(desc) = &input_def.description {
                interpolate_all(desc, &vars)?
            } else {
                input_def.name.to_string()
            };

            // Get the current value for this input
            let current_value = current_inputs.get(&input_def.name);

            // Interpolate variables in the default value (supports both ${env:...} and ${var:...})
            let interpolated_default = if let Some(default) = &input_def.default {
                Some(interpolate_value_all(default, &vars)?)
            } else {
                None
            };

            let value = if let Some(enum_values) = &input_def.enum_values {
                // This is a select input
                // Sort enum values alphabetically for display
                let mut sorted_enum_values = enum_values.clone();
                sorted_enum_values.sort();

                let default_str = current_value
                    .and_then(|v| v.as_str())
                    .or_else(|| interpolated_default.as_ref().and_then(|v| v.as_str()))
                    .or_else(|| sorted_enum_values.first().map(|s| s.as_str()));

                // Find the default value index in the sorted list
                let default_index = default_str.and_then(|default_val| {
                    sorted_enum_values.iter().position(|v| v == default_val)
                });

                let selected = ctx
                    .input
                    .select(&description, sorted_enum_values, default_index)
                    .context("Failed to get input")?;

                Value::String(selected)
            } else {
                // Determine the default value (prefer current value over template default)
                let default_value = current_value.or(interpolated_default.as_ref());

                match default_value {
                    Some(Value::Bool(b)) => {
                        let answer = ctx
                            .input
                            .confirm(&description, Some(*b))
                            .context("Failed to get input")?;
                        Value::Bool(answer)
                    }
                    Some(Value::Number(n)) => {
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
                    Some(Value::String(s)) => {
                        // Don't pass empty string as default to avoid "()" display
                        let default = if s.is_empty() { None } else { Some(s.as_str()) };
                        let answer = ctx
                            .input
                            .text(&description, default)
                            .context("Failed to get input")?;
                        Value::String(answer)
                    }
                    Some(Value::Array(_arr)) => {
                        // For arrays, use the default value directly
                        default_value.unwrap().clone()
                    }
                    Some(_) => {
                        // Fallback to string input for other types
                        let current_str = current_value
                            .and_then(|v| serde_json::to_string(v).ok())
                            .unwrap_or_default();
                        // Don't pass empty string as default to avoid "()" display
                        let default = if current_str.is_empty() {
                            None
                        } else {
                            Some(current_str.as_str())
                        };
                        let answer = ctx
                            .input
                            .text(&description, default)
                            .context("Failed to get input")?;
                        Value::String(answer)
                    }
                    None => {
                        // No current or default value, prompt for string
                        let answer = ctx
                            .input
                            .text(&description, None)
                            .context("Failed to get input")?;
                        Value::String(answer)
                    }
                }
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

    /// Helper method to collect input based on its value type
    fn collect_input_by_value(
        ctx: &crate::context::Context,
        description: &str,
        default_value: Option<&serde_json::Value>,
    ) -> Result<serde_json::Value> {
        match default_value {
            Some(serde_json::Value::Bool(b)) => {
                let answer = ctx
                    .input
                    .confirm(description, Some(*b))
                    .context("Failed to get input")?;
                Ok(serde_json::Value::Bool(answer))
            }
            Some(serde_json::Value::Number(n)) => {
                let answer = ctx
                    .input
                    .text(description, Some(&n.to_string()))
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
            Some(serde_json::Value::String(s)) => {
                // Don't pass empty string as default to avoid "()" display
                let default = if s.is_empty() { None } else { Some(s.as_str()) };
                let answer = ctx
                    .input
                    .text(description, default)
                    .context("Failed to get input")?;
                Ok(serde_json::Value::String(answer))
            }
            _ => {
                // No current value or default, prompt for string
                let prompt_text = format!("{} [required]", description);
                let answer = ctx
                    .input
                    .text(&prompt_text, None)
                    .context("Failed to get input")?;
                Ok(serde_json::Value::String(answer))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    // NOTE: Tests for UPDATE command have been removed as they relied on MockFileSystem
    // being compatible with project/environment discovery, which uses real filesystem paths.
    // These tests would require integration testing with a real filesystem in a temp directory.
}
