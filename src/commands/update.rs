use crate::collection::{CollectionDiscovery, CollectionManager};
use crate::output;
use crate::template::{
    TemplateDiscovery, TemplateRenderer, ProjectResource, DynamicProjectEnvironmentResource,
    PluginInfo, TemplateInfo, ProjectReference,
};
use crate::template::metadata::{ProjectCollectionResource, AllowedPluginConfig};
use anyhow::{Context, Result};
use inquire::Select;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use serde_json::Value;

/// Handles the 'update' command - regenerates project environment files from the original template
pub struct UpdateCommand;

/// Information about a plugin and its compatible projects
#[derive(Debug, Clone)]
struct PluginWithProjects {
    plugin_info: PluginInfo,
    #[allow(dead_code)]
    template_pack_path: PathBuf,
    compatible_projects: Vec<CompatibleProject>,
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

impl UpdateCommand {
    /// Execute the update command
    pub fn execute(project_path: Option<&str>, templates_path: Option<&str>) -> Result<()> {
        // Determine working directory
        let work_dir = if let Some(path) = project_path {
            PathBuf::from(path)
        } else {
            std::env::current_dir().context("Failed to get current directory")?
        };

        // Detect context and get environment path
        let (env_path, project_name, env_name) = Self::detect_and_select_environment(&work_dir)?;

        // Load environment resource to get current configuration
        let env_file = env_path.join(".pmp.environment.yaml");
        if !env_file.exists() {
            anyhow::bail!("Environment file not found: {:?}", env_file);
        }

        let current_env_resource = DynamicProjectEnvironmentResource::from_file(&env_file)
            .context("Failed to load environment resource")?;

        output::section("Update Environment");
        output::key_value_highlight("Project", &project_name);
        output::environment_badge(&env_name);
        output::key_value("Resource Kind", &current_env_resource.kind);

        if let Some(desc) = &current_env_resource.metadata.description {
            output::key_value("Description", desc);
        }

        // Load collection to ensure we're in a valid collection context
        let (collection, collection_root) = CollectionDiscovery::find_collection()?
            .context("ProjectCollection is required to run commands")?;

        // Discover plugins with compatible projects
        let plugins_with_projects = Self::discover_plugins_with_compatible_projects(
            &collection_root,
            templates_path,
        )?;

        // If there are plugins with compatible projects, ask user what they want to do
        if !plugins_with_projects.is_empty() {
            output::blank();
            let options = vec!["Update the project", "Execute plugin"];
            let action = Select::new("What would you like to do?", options)
                .prompt()
                .context("Failed to select action")?;

            if action == "Execute plugin" {
                // Execute plugin flow with project selection
                return Self::execute_plugin_with_project_selection(
                    &collection_root,
                    &collection,
                    plugins_with_projects,
                    templates_path,
                );
            }
        }

        // Discover templates
        output::subsection("Template Discovery");
        output::dimmed("Discovering templates...");
        let custom_paths = if let Some(path) = templates_path {
            vec![path]
        } else {
            vec![]
        };

        let all_templates = TemplateDiscovery::discover_templates_with_custom_paths(&custom_paths)
            .context("Failed to discover templates")?;

        if all_templates.is_empty() {
            anyhow::bail!("No templates found. Please create templates in ~/.pmp/templates or .pmp/templates");
        }

        // Find the template matching the current resource kind
        let matching_template = all_templates
            .iter()
            .find(|t| {
                t.resource.spec.api_version == current_env_resource.api_version
                    && t.resource.spec.kind == current_env_resource.kind
            })
            .context(format!(
                "No template found for resource kind: {}/{}",
                current_env_resource.api_version,
                current_env_resource.kind
            ))?;

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
            for (input_name, input_spec) in &env_overrides.overrides.inputs {
                merged_inputs.insert(input_name.clone(), input_spec.clone());
            }
        }

        // Prompt for inputs with current values as defaults
        output::subsection("Update Inputs");
        output::dimmed("Please provide the following information (current values shown as defaults):");

        let mut new_inputs = Self::collect_template_inputs_with_defaults(
            &merged_inputs,
            current_inputs,
            &project_name,
        ).context("Failed to collect inputs")?;

        // Add internal fields for template rendering
        new_inputs.insert(
            "environment".to_string(),
            serde_json::Value::String(env_name.clone()),
        );
        new_inputs.insert(
            "resource_api_version".to_string(),
            serde_json::Value::String(matching_template.resource.spec.api_version.clone()),
        );
        new_inputs.insert(
            "resource_kind".to_string(),
            serde_json::Value::String(matching_template.resource.spec.kind.clone()),
        );

        // Confirm before regenerating
        let confirm = inquire::Confirm::new("Regenerate environment files with these inputs?")
            .with_default(true)
            .prompt()
            .context("Failed to get confirmation")?;

        if !confirm {
            output::dimmed("Update cancelled");
            return Ok(());
        }

        // Render template into environment directory
        output::subsection("Regenerating Files");
        output::dimmed("Regenerating template files...");
        let renderer = TemplateRenderer::new();
        let template_src = &matching_template.path;

        if !template_src.exists() {
            anyhow::bail!(
                "Template directory not found: {}",
                template_src.display()
            );
        }

        renderer
            .render_template(&template_src, env_path.as_path(), &new_inputs)
            .context("Failed to render template")?;

        // Generate _common.tf if executor config is present (OpenTofu only)
        if matching_template.resource.spec.executor == "opentofu" {
            if let Some(executor_config) = &collection.spec.executor {
                if executor_config.name == "opentofu" && !executor_config.config.is_empty() {
                    output::dimmed("  Updating _common.tf with backend configuration...");
                    Self::generate_common_tf(
                        &env_path,
                        &executor_config.config,
                    ).context("Failed to generate _common.tf file")?;
                }
            }
        }

        // Regenerate .pmp.environment.yaml file
        output::dimmed("  Updating .pmp.environment.yaml...");
        Self::generate_project_environment_yaml(
            &env_path,
            &env_name,
            &project_name,
            &matching_template.resource,
            &new_inputs,
        ).context("Failed to update .pmp.environment.yaml file")?;

        output::blank();
        output::success("Environment updated successfully!");

        output::subsection("Updated Environment");
        output::key_value("Project", &project_name);
        output::environment_badge(&env_name);
        output::key_value("Path", &env_path.display().to_string());

        let next_steps_list = vec![
            format!("Review the regenerated files in {}", env_path.display()),
            "Run 'pmp preview' to see what changes will be applied".to_string(),
            "Run 'pmp apply' to apply the infrastructure".to_string(),
        ];
        output::next_steps(&next_steps_list);

        Ok(())
    }

    /// Discover all plugins that have compatible projects in the collection
    /// Returns a list of plugins with their compatible projects
    fn discover_plugins_with_compatible_projects(
        collection_root: &Path,
        templates_path: Option<&str>,
    ) -> Result<Vec<PluginWithProjects>> {
        let mut result = Vec::new();

        // Discover all projects in the collection
        let projects = CollectionDiscovery::discover_projects(collection_root)?;

        // Discover all template packs
        let custom_paths = if let Some(path) = templates_path {
            vec![path]
        } else {
            vec![]
        };

        let template_packs = TemplateDiscovery::discover_template_packs_with_custom_paths(&custom_paths)?;

        // For each template pack
        for pack_info in template_packs {
            // Discover templates and plugins in this pack
            let templates = TemplateDiscovery::discover_templates_in_pack(&pack_info.path)?;
            let plugins = TemplateDiscovery::discover_plugins_in_pack(&pack_info.path)?;

            // For each plugin in this pack
            for plugin_info in plugins {
                let mut compatible_projects = Vec::new();

                // Find templates that allow this plugin
                for template_info in &templates {
                    if let Some(plugins_config) = &template_info.resource.spec.plugins {
                        // Find the allowed plugin config for this plugin
                        if let Some(allowed_config) = plugins_config.allowed.iter().find(|a| {
                            a.plugin == plugin_info.resource.metadata.name
                        }) {
                            // Find projects with matching apiVersion and kind
                            for project in &projects {
                                // Load the first environment to get api_version and kind
                                let project_path = collection_root.join(&project.path);
                                let environments_dir = project_path.join("environments");

                                // Find the first environment to get resource details
                                if let Ok(env_entries) = std::fs::read_dir(&environments_dir) {
                                    for env_entry in env_entries.filter_map(|e| e.ok()) {
                                        let env_path = env_entry.path();
                                        let env_file = env_path.join(".pmp.environment.yaml");

                                        if env_file.exists() {
                                            if let Ok(env_resource) = DynamicProjectEnvironmentResource::from_file(&env_file) {
                                                // Check if this project matches the template's resource type
                                                if env_resource.api_version == template_info.resource.spec.api_version
                                                    && env_resource.kind == template_info.resource.spec.kind
                                                {
                                                    compatible_projects.push(CompatibleProject {
                                                        project_ref: project.clone(),
                                                        project_path: project_path.clone(),
                                                        template_info: template_info.clone(),
                                                        allowed_plugin_config: allowed_config.clone(),
                                                    });
                                                    break; // Only need one environment to confirm the match
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Only include plugin if it has at least one compatible project
                if !compatible_projects.is_empty() {
                    result.push(PluginWithProjects {
                        plugin_info: plugin_info.clone(),
                        template_pack_path: pack_info.path.clone(),
                        compatible_projects,
                    });
                }
            }
        }

        Ok(result)
    }

    /// Execute a plugin with project selection
    /// User selects plugin -> project -> environment -> provides inputs -> executes
    fn execute_plugin_with_project_selection(
        _collection_root: &Path,
        _collection: &ProjectCollectionResource,
        plugins_with_projects: Vec<PluginWithProjects>,
        _templates_path: Option<&str>,
    ) -> Result<()> {
        output::subsection("Plugin Execution");

        // 1. Let user select plugin
        let plugin_options: Vec<String> = plugins_with_projects.iter()
            .map(|p| {
                let desc = p.plugin_info.resource.metadata.description.as_deref().unwrap_or("");
                let project_count = p.compatible_projects.len();
                if desc.is_empty() {
                    format!("{} ({} compatible project{})",
                        p.plugin_info.resource.metadata.name,
                        project_count,
                        if project_count == 1 { "" } else { "s" })
                } else {
                    format!("{} - {} ({} compatible project{})",
                        p.plugin_info.resource.metadata.name,
                        desc,
                        project_count,
                        if project_count == 1 { "" } else { "s" })
                }
            })
            .collect();

        let selected_plugin_display = Select::new("Select a plugin to execute:", plugin_options.clone())
            .prompt()
            .context("Failed to select plugin")?;

        let plugin_index = plugin_options.iter()
            .position(|opt| opt == &selected_plugin_display)
            .context("Plugin not found")?;

        let selected_plugin_with_projects = &plugins_with_projects[plugin_index];

        output::blank();
        output::key_value_highlight("Plugin", &selected_plugin_with_projects.plugin_info.resource.metadata.name);
        if let Some(desc) = &selected_plugin_with_projects.plugin_info.resource.metadata.description {
            output::key_value("Description", desc);
        }

        // 2. Let user select compatible project
        output::blank();
        let project_options: Vec<String> = selected_plugin_with_projects.compatible_projects.iter()
            .map(|cp| {
                format!("{} ({})", cp.project_ref.name, cp.project_ref.kind)
            })
            .collect();

        let selected_project_display = Select::new("Select a project:", project_options.clone())
            .prompt()
            .context("Failed to select project")?;

        let project_index = project_options.iter()
            .position(|opt| opt == &selected_project_display)
            .context("Project not found")?;

        let selected_compatible_project = &selected_plugin_with_projects.compatible_projects[project_index];

        output::blank();
        output::key_value_highlight("Project", &selected_compatible_project.project_ref.name);
        output::key_value("Resource Kind", &selected_compatible_project.project_ref.kind);

        // 3. Let user select environment
        let environments = CollectionDiscovery::discover_environments(&selected_compatible_project.project_path)?;

        if environments.is_empty() {
            anyhow::bail!("No environments found in project: {}", selected_compatible_project.project_ref.name);
        }

        let env_name = if environments.len() == 1 {
            environments[0].clone()
        } else {
            Select::new("Select environment:", environments.clone())
                .prompt()
                .context("Failed to select environment")?
        };

        output::blank();
        output::environment_badge(&env_name);

        let env_path = selected_compatible_project.project_path.join("environments").join(&env_name);
        let env_file = env_path.join(".pmp.environment.yaml");

        if !env_file.exists() {
            anyhow::bail!("Environment file not found: {:?}", env_file);
        }

        let current_env_resource = DynamicProjectEnvironmentResource::from_file(&env_file)
            .context("Failed to load environment resource")?;

        // 4. Get plugin inputs and merge with allowed plugin config
        let allowed_plugin_config = &selected_compatible_project.allowed_plugin_config;
        let mut merged_inputs = selected_plugin_with_projects.plugin_info.resource.spec.inputs.clone();

        // Override with allowed plugin input specs
        for (input_name, input_spec) in &allowed_plugin_config.inputs {
            merged_inputs.insert(input_name.clone(), input_spec.clone());
        }

        // 5. Collect inputs from user
        output::subsection("Plugin Inputs");
        output::dimmed("Please provide the following information:");

        let mut plugin_inputs = Self::collect_plugin_inputs(&merged_inputs, &selected_compatible_project.project_ref.name)?;

        // 6. Add internal fields (inherit from parent template/project)
        plugin_inputs.insert("name".to_string(), Value::String(selected_compatible_project.project_ref.name.clone()));
        plugin_inputs.insert("environment".to_string(), Value::String(env_name.clone()));

        // Inherit namespace from parent project
        if let Some(namespace) = current_env_resource.spec.inputs.get("namespace") {
            if !plugin_inputs.contains_key("namespace") {
                plugin_inputs.insert("namespace".to_string(), namespace.clone());
            }
        }

        // Inherit database_name from parent project
        if let Some(database_name) = current_env_resource.spec.inputs.get("database_name") {
            if !plugin_inputs.contains_key("database_name") || plugin_inputs.get("database_name").and_then(|v| v.as_str()) == Some("") {
                plugin_inputs.insert("database_name".to_string(), database_name.clone());
            }
        }

        // 7. Render plugin files
        output::subsection("Rendering Plugin Files");
        output::dimmed(&format!("Rendering plugin '{}'...", selected_plugin_with_projects.plugin_info.resource.metadata.name));

        let renderer = TemplateRenderer::new();
        renderer.render_template(&selected_plugin_with_projects.plugin_info.path, &env_path, &plugin_inputs)
            .context("Failed to render plugin files")?;

        output::blank();
        output::success(&format!("Plugin '{}' executed successfully!", selected_plugin_with_projects.plugin_info.resource.metadata.name));

        output::subsection("Next Steps");
        output::dimmed("The plugin has created new resources in your environment:");
        output::key_value("Project", &selected_compatible_project.project_ref.name);
        output::key_value("Environment", &env_name);
        output::key_value("Environment path", &env_path.display().to_string());
        output::blank();
        output::dimmed("To apply the changes:");
        output::dimmed("  1. Run 'pmp preview' to see what will be created");
        output::dimmed("  2. Run 'pmp apply' to apply the infrastructure");

        Ok(())
    }

    /// Detect context and select project/environment
    /// Returns: (environment_path, project_name, environment_name)
    fn detect_and_select_environment(work_dir: &Path) -> Result<(PathBuf, String, String)> {
        // Check if we're in an environment directory
        if let Some(env_info) = Self::check_in_environment(work_dir)? {
            return Ok(env_info);
        }

        // Check if we're in a project directory
        if let Some((project_path, project_name)) = Self::check_in_project(work_dir)? {
            let env_name = Self::select_environment(&project_path)?;
            let env_path = project_path.join("environments").join(&env_name);
            return Ok((env_path, project_name, env_name));
        }

        // We're in the collection or elsewhere - use find/search UI
        Self::select_project_and_environment()
    }

    /// Check if we're inside an environment directory
    fn check_in_environment(dir: &Path) -> Result<Option<(PathBuf, String, String)>> {
        let env_file = dir.join(".pmp.environment.yaml");

        if env_file.exists() {
            let resource = DynamicProjectEnvironmentResource::from_file(&env_file)?;
            let env_name = resource.metadata.environment_name.clone();
            let project_name = resource.metadata.name.clone();

            return Ok(Some((dir.to_path_buf(), project_name, env_name)));
        }

        Ok(None)
    }

    /// Check if we're inside a project directory (but not in an environment)
    fn check_in_project(dir: &Path) -> Result<Option<(PathBuf, String)>> {
        let project_file = dir.join(".pmp.project.yaml");

        if project_file.exists() {
            let resource = ProjectResource::from_file(&project_file)?;
            return Ok(Some((dir.to_path_buf(), resource.metadata.name.clone())));
        }

        Ok(None)
    }

    /// Select an environment from a project
    fn select_environment(project_path: &Path) -> Result<String> {
        let environments = CollectionDiscovery::discover_environments(project_path)
            .context("Failed to discover environments")?;

        if environments.is_empty() {
            anyhow::bail!("No environments found in project: {:?}", project_path);
        }

        if environments.len() == 1 {
            output::environment_badge(&environments[0]);
            return Ok(environments[0].clone());
        }

        let selected = Select::new("Select an environment:", environments.clone())
            .prompt()
            .context("Failed to select environment")?;

        Ok(selected)
    }

    /// Select project and environment using find/search UI
    fn select_project_and_environment() -> Result<(PathBuf, String, String)> {
        let manager = CollectionManager::load()
            .context("Failed to load collection")?;

        let all_projects = manager.get_all_projects();

        if all_projects.is_empty() {
            anyhow::bail!("No projects found in collection");
        }

        // Select project
        let project_options: Vec<String> = all_projects
            .iter()
            .map(|p| format!("{} ({})", p.name, p.kind))
            .collect();

        let selected_project_display = Select::new("Select a project:", project_options.clone())
            .prompt()
            .context("Failed to select project")?;

        let project_index = project_options.iter().position(|opt| opt == &selected_project_display)
            .context("Project not found")?;

        let selected_project = &all_projects[project_index];
        let project_path = manager.get_project_path(selected_project);

        // Select environment
        let env_name = Self::select_environment(&project_path)?;
        let env_path = project_path.join("environments").join(&env_name);

        Ok((env_path, selected_project.name.clone(), env_name))
    }

    /// Collect inputs from user based on template input specifications, using current values as defaults
    fn collect_template_inputs_with_defaults(
        inputs_spec: &std::collections::HashMap<String, crate::template::metadata::InputSpec>,
        current_inputs: &std::collections::HashMap<String, serde_json::Value>,
        project_name: &str,
    ) -> Result<std::collections::HashMap<String, serde_json::Value>> {
        use inquire::{Select, Text, Confirm};

        let mut inputs = std::collections::HashMap::new();

        // Always add name
        inputs.insert("name".to_string(), serde_json::Value::String(project_name.to_string()));

        // Collect each input defined in the template
        for (input_name, input_spec) in inputs_spec {
            let description = input_spec.description.as_deref().unwrap_or(input_name.as_str());

            // Get the current value for this input
            let current_value = current_inputs.get(input_name);

            let value = if let Some(enum_values) = &input_spec.enum_values {
                // This is a select input
                let default_str = current_value
                    .and_then(|v| v.as_str())
                    .or_else(|| input_spec.default.as_ref().and_then(|v| v.as_str()))
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
            } else {
                // Determine the default value (prefer current value over template default)
                let default_value = current_value.or(input_spec.default.as_ref());

                match default_value {
                    Some(serde_json::Value::Bool(b)) => {
                        let answer = Confirm::new(description)
                            .with_default(*b)
                            .prompt()
                            .context("Failed to get input")?;
                        serde_json::Value::Bool(answer)
                    }
                    Some(serde_json::Value::Number(n)) => {
                        let prompt_text = format!("{} (current: {})", description, n);
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
                    Some(serde_json::Value::String(s)) => {
                        let prompt_text = format!("{} (current: {})", description, s);
                        let answer = Text::new(&prompt_text)
                            .with_default(s)
                            .prompt()
                            .context("Failed to get input")?;
                        serde_json::Value::String(answer)
                    }
                    _ => {
                        // No current value or default, prompt for string
                        let answer = Text::new(description)
                            .prompt()
                            .context("Failed to get input")?;
                        serde_json::Value::String(answer)
                    }
                }
            };

            inputs.insert(input_name.clone(), value);
        }

        Ok(inputs)
    }

    /// Generate the .pmp.environment.yaml file for the project environment (with spec)
    fn generate_project_environment_yaml(
        environment_path: &Path,
        environment_name: &str,
        project_name: &str,
        template: &crate::template::metadata::TemplateResource,
        inputs: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        use crate::template::metadata::{
            DynamicProjectEnvironmentResource, DynamicProjectEnvironmentMetadata, ProjectSpec, ResourceDefinition,
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
            },
        };

        // Serialize to YAML
        let yaml_content = serde_yaml::to_string(&project_env)
            .context("Failed to serialize project environment to YAML")?;

        // Write to .pmp.environment.yaml file
        let pmp_env_yaml_path = environment_path.join(".pmp.environment.yaml");
        std::fs::write(&pmp_env_yaml_path, yaml_content)
            .with_context(|| format!("Failed to write .pmp.environment.yaml file: {:?}", pmp_env_yaml_path))?;

        output::dimmed(&format!("  Updated: {}", pmp_env_yaml_path.display()));

        Ok(())
    }

    /// Generate _common.tf file with backend configuration
    fn generate_common_tf(
        environment_path: &Path,
        executor_config: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        use crate::executor::generate_backend_config;

        // Generate backend HCL
        let backend_hcl = generate_backend_config(executor_config)
            .context("Failed to generate backend configuration")?;

        if backend_hcl.is_empty() {
            // No backend config to write
            return Ok(());
        }

        // Write to _common.tf file
        let common_tf_path = environment_path.join("_common.tf");
        std::fs::write(&common_tf_path, backend_hcl)
            .with_context(|| format!("Failed to write _common.tf file: {:?}", common_tf_path))?;

        output::dimmed(&format!("  Updated: {}", common_tf_path.display()));

        Ok(())
    }

    /// Collect plugin inputs from user
    fn collect_plugin_inputs(
        inputs_spec: &HashMap<String, crate::template::metadata::InputSpec>,
        project_name: &str,
    ) -> Result<HashMap<String, Value>> {
        use inquire::{Select, Text, Confirm};

        let mut inputs = HashMap::new();

        // Always add name
        inputs.insert("name".to_string(), Value::String(project_name.to_string()));

        // Collect each input defined in the plugin
        for (input_name, input_spec) in inputs_spec {
            // Skip if it's the 'name' field (already added)
            if input_name == "name" {
                continue;
            }

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

                Value::String(selected)
            } else if let Some(default) = &input_spec.default {
                // Determine type from default value
                match default {
                    Value::Bool(b) => {
                        let answer = Confirm::new(description)
                            .with_default(*b)
                            .prompt()
                            .context("Failed to get input")?;
                        Value::Bool(answer)
                    }
                    Value::Number(n) => {
                        let prompt_text = format!("{} (default: {})", description, n);
                        let answer = Text::new(&prompt_text)
                            .with_default(&n.to_string())
                            .prompt()
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
                        let prompt_text = if !s.is_empty() {
                            format!("{} (default: {})", description, s)
                        } else {
                            description.to_string()
                        };
                        let answer = Text::new(&prompt_text)
                            .with_default(s)
                            .prompt()
                            .context("Failed to get input")?;
                        Value::String(answer)
                    }
                    Value::Array(_arr) => {
                        // For arrays, use the default value directly
                        default.clone()
                    }
                    _ => {
                        // Fallback to string input
                        let answer = Text::new(description)
                            .prompt()
                            .context("Failed to get input")?;
                        Value::String(answer)
                    }
                }
            } else {
                // No default, prompt for string
                let answer = Text::new(description)
                    .prompt()
                    .context("Failed to get input")?;
                Value::String(answer)
            };

            inputs.insert(input_name.clone(), value);
        }

        Ok(inputs)
    }
}
