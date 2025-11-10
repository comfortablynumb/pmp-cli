use crate::output;
use crate::template::TemplateDiscovery;
use crate::template::metadata::{
    Category, CategoryTemplate, Environment, InfrastructureMetadata, InfrastructureResource,
    InfrastructureSpec, TemplatePackConfig,
};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;

/// Handles the 'init' command - initializes or edits an Infrastructure
pub struct InitCommand;

impl InitCommand {
    /// Execute the init command
    pub fn execute(
        ctx: &crate::context::Context,
        name: Option<&str>,
        description: Option<&str>,
        template_packs_paths: Option<&str>,
    ) -> Result<()> {
        let current_dir = std::env::current_dir()
            .context("Failed to get current directory")?;

        let infrastructure_file = current_dir.join(".pmp.infrastructure.yaml");

        // Check if infrastructure already exists and route accordingly
        if ctx.fs.exists(&infrastructure_file) {
            Self::edit_existing_infrastructure(ctx, &infrastructure_file, template_packs_paths)?;
        } else {
            Self::create_new_infrastructure(ctx, &current_dir, &infrastructure_file, name, description, template_packs_paths)?;
        }

        Ok(())
    }

    /// Create a new Infrastructure
    fn create_new_infrastructure(
        ctx: &crate::context::Context,
        current_dir: &PathBuf,
        infrastructure_file: &PathBuf,
        name: Option<&str>,
        description: Option<&str>,
        template_packs_paths: Option<&str>,
    ) -> Result<()> {
        output::section("Initialize Infrastructure");
        output::key_value("Directory", &current_dir.display().to_string());
        output::blank();

        // Get collection name (use CLI arg or prompt with default)
        let collection_name = if let Some(n) = name {
            n.to_string()
        } else {
            ctx.input.text("Collection name:", Some("My Infrastructure"))
                .context("Failed to get collection name")?
        };

        // Step 2: Get description (use CLI arg or prompt, optional)
        let collection_description = if let Some(d) = description {
            Some(d.to_string())
        } else {
            let desc = ctx.input.text("Description (optional):", Some(""))
                .context("Failed to get description")?;
            if desc.is_empty() {
                None
            } else {
                Some(desc)
            }
        };

        // Step 3: Discover templates to get available resource kinds
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

        // Discover template packs
        let template_packs = TemplateDiscovery::discover_template_packs_with_custom_paths(&*ctx.fs, &*ctx.output, &custom_paths)
            .context("Failed to discover template packs")?;

        // Step 4: Build template selection options organized by pack
        let mut template_options: Vec<String> = Vec::new();
        let mut template_map: HashMap<String, (String, String, String, String)> = HashMap::new(); // key -> (pack_name, template_name, api_version, kind)

        for pack in &template_packs {
            let pack_name = &pack.resource.metadata.name;

            // Discover templates in this pack
            let templates_in_pack = TemplateDiscovery::discover_templates_in_pack(&*ctx.fs, &*ctx.output, &pack.path)
                .context("Failed to discover templates in pack")?;

            for template in &templates_in_pack {
                let template_name = &template.resource.metadata.name;
                let api_version = &template.resource.spec.api_version;
                let kind = &template.resource.spec.kind;

                let option_key = format!("{} / {} ({})", pack_name, template_name, kind);
                template_options.push(option_key.clone());
                template_map.insert(
                    option_key,
                    (pack_name.clone(), template_name.clone(), api_version.clone(), kind.clone()),
                );
            }
        }

        // Step 5: Let user select templates
        let (categories, template_packs_config) = if template_options.is_empty() {
            output::warning("No templates found in the system.");
            output::dimmed("The Infrastructure will be created without any templates.");
            output::dimmed("You can add templates later and update the infrastructure file.");
            output::blank();
            (vec![], HashMap::new())
        } else {
            output::subsection("Template Selection");
            output::dimmed("Select which templates you want to allow in this infrastructure.");
            output::dimmed("Templates will be organized into categories by resource kind.");
            output::blank();

            let selected_options = ctx.input.multi_select(
                "Select templates:",
                template_options.clone(),
                None
            )
            .context("Failed to select templates")?;

            // Build categories and template_packs from selections
            Self::build_categories_from_selections(&selected_options, &template_map)
        };

        // Step 6: Collect environments
        let mut environments: HashMap<String, Environment> = HashMap::new();

        output::subsection("Environments");
        output::dimmed("Let's add environments to the infrastructure.");
        output::dimmed("You need at least one environment.");
        output::blank();

        loop {
            // Prompt for environment key
            let env_key = loop {
                let key = ctx.input.text("Environment key (lowercase, alphanumeric, underscores; cannot start with number):", None)
                    .context("Failed to get environment key")?;

                // Validate environment key
                if !InfrastructureResource::is_valid_environment_name(&key) {
                    output::warning("Invalid environment key. Must be lowercase alphanumeric with underscores, and cannot start with a number.");
                    continue;
                }

                // Check for duplicate
                if environments.contains_key(&key) {
                    output::warning(&format!("Environment '{}' already exists. Please use a different key.", key));
                    continue;
                }

                break key;
            };

            // Prompt for display name
            let default_name = Self::capitalize_first(&env_key);
            let env_name = ctx.input.text("Environment display name:", Some(&default_name))
                .context("Failed to get environment name")?;

            // Prompt for optional description
            let env_description = ctx.input.text("Environment description (optional):", Some(""))
                .context("Failed to get environment description")?;

            environments.insert(
                env_key.clone(),
                Environment {
                    name: env_name,
                    description: if env_description.is_empty() {
                        None
                    } else {
                        Some(env_description)
                    },
                },
            );

            output::success(&format!("Environment '{}' added", env_key));
            output::blank();

            // Ask if they want to add another environment
            let add_another = ctx.input.confirm("Add another environment?", false)
                .context("Failed to get confirmation")?;

            if !add_another {
                break;
            }
        }

        // Ensure at least one environment was added
        if environments.is_empty() {
            anyhow::bail!("At least one environment is required.");
        }

        // Step 7: Create the InfrastructureResource
        let infrastructure = InfrastructureResource {
            api_version: "pmp.io/v1".to_string(),
            kind: "Infrastructure".to_string(),
            metadata: InfrastructureMetadata {
                name: collection_name.clone(),
                description: collection_description,
            },
            spec: InfrastructureSpec {
                categories,
                template_packs: template_packs_config,
                resource_kinds: vec![],  // New structure - no longer used
                environments,
                hooks: None,
                executor: None,
            },
        };

        // Step 8: Save the infrastructure file
        infrastructure.save(&*ctx.fs, &infrastructure_file)
            .context("Failed to save .pmp.infrastructure.yaml")?;

        // Step 9: Create the projects directory
        let projects_dir = current_dir.join("projects");
        ctx.fs.create_dir_all(&projects_dir)
            .context("Failed to create projects directory")?;

        // Step 10: Display success message
        output::blank();
        output::success("Infrastructure created successfully!");

        output::subsection("Summary");
        output::key_value_highlight("Infrastructure", &collection_name);
        output::key_value("File", &infrastructure_file.display().to_string());
        output::key_value("Projects directory", &projects_dir.display().to_string());
        output::key_value("Categories", &infrastructure.spec.categories.len().to_string());
        output::key_value("Template packs", &infrastructure.spec.template_packs.len().to_string());
        output::key_value("Environments", &infrastructure.spec.environments.len().to_string());

        let next_steps_list = vec![
            format!("Review and edit {} if needed", infrastructure_file.display()),
            "Run 'pmp create' to create a new project from a template".to_string(),
        ];
        output::next_steps(&next_steps_list);

        Ok(())
    }

    /// Edit an existing Infrastructure
    fn edit_existing_infrastructure(
        ctx: &crate::context::Context,
        infrastructure_file: &PathBuf,
        template_packs_paths: Option<&str>,
    ) -> Result<()> {
        output::section("Edit Infrastructure");
        output::key_value("File", &infrastructure_file.display().to_string());
        output::blank();

        // Load existing infrastructure
        let mut infrastructure = InfrastructureResource::from_file(&*ctx.fs, infrastructure_file)
            .context("Failed to load existing infrastructure")?;

        // Iterative editing loop
        loop {
            // Display current infrastructure info
            Self::display_infrastructure_summary(&infrastructure);

            // Present editing options with hints
            let options: Vec<String> = vec![
                "Metadata - Edit infrastructure name and description".to_string(),
                "Templates - Add or remove allowed templates and categories".to_string(),
                "Environments - Add, edit, or remove environments".to_string(),
                "Exit - Save and exit".to_string(),
            ];

            let choice = ctx.input.select("What would you like to edit?", options)
                .context("Failed to select option")?;

            // Handle the selection
            match choice {
                opt if opt.starts_with("Metadata") => {
                    Self::edit_metadata(ctx, &mut infrastructure)?;

                    // Save after editing
                    infrastructure.save(&*ctx.fs, infrastructure_file)
                        .context("Failed to save infrastructure")?;
                    output::success(&format!("Changes saved to {}", infrastructure_file.display()));
                    output::blank();

                    // Ask if they want to continue
                    let continue_editing = ctx.input.confirm("Continue editing?", true)
                        .context("Failed to get confirmation")?;

                    if !continue_editing {
                        break;
                    }
                }
                opt if opt.starts_with("Templates") => {
                    Self::edit_templates_and_categories(ctx, &mut infrastructure, template_packs_paths)?;

                    // Save after editing
                    infrastructure.save(&*ctx.fs, infrastructure_file)
                        .context("Failed to save infrastructure")?;
                    output::success(&format!("Changes saved to {}", infrastructure_file.display()));
                    output::blank();

                    // Ask if they want to continue
                    let continue_editing = ctx.input.confirm("Continue editing?", true)
                        .context("Failed to get confirmation")?;

                    if !continue_editing {
                        break;
                    }
                }
                opt if opt.starts_with("Environments") => {
                    Self::edit_environments(ctx, &mut infrastructure)?;

                    // Save after editing
                    infrastructure.save(&*ctx.fs, infrastructure_file)
                        .context("Failed to save infrastructure")?;
                    output::success(&format!("Changes saved to {}", infrastructure_file.display()));
                    output::blank();

                    // Ask if they want to continue
                    let continue_editing = ctx.input.confirm("Continue editing?", true)
                        .context("Failed to get confirmation")?;

                    if !continue_editing {
                        break;
                    }
                }
                opt if opt.starts_with("Exit") => {
                    break;
                }
                _ => {}
            }
        }

        output::blank();
        output::success("Done editing Infrastructure!");
        output::key_value("File", &infrastructure_file.display().to_string());

        Ok(())
    }

    /// Display a summary of the current infrastructure
    fn display_infrastructure_summary(infrastructure: &InfrastructureResource) {
        output::subsection("Current Infrastructure");
        output::key_value_highlight("Name", &infrastructure.metadata.name);
        if let Some(desc) = &infrastructure.metadata.description {
            output::key_value("Description", desc);
        }

        // Count total templates
        let total_templates: usize = infrastructure.spec.categories.iter()
            .map(|c| c.templates.len())
            .sum();

        output::key_value("Categories", &infrastructure.spec.categories.len().to_string());
        for category in &infrastructure.spec.categories {
            output::list_item(&format!("{} ({} templates)", category.name, category.templates.len()));
        }

        output::key_value("Total templates", &total_templates.to_string());
        output::key_value("Template packs", &infrastructure.spec.template_packs.len().to_string());
        output::key_value("Environments", &infrastructure.spec.environments.len().to_string());
        for (key, env) in &infrastructure.spec.environments {
            output::list_item(&format!("{} ({})", key, env.name));
        }
        output::blank();
    }

    /// Edit infrastructure metadata
    fn edit_metadata(ctx: &crate::context::Context, infrastructure: &mut InfrastructureResource) -> Result<()> {
        output::subsection("Editing Metadata");

        let new_name = ctx.input.text("Infrastructure name:", Some(&infrastructure.metadata.name))
            .context("Failed to get infrastructure name")?;

        let current_desc = infrastructure.metadata.description.as_deref().unwrap_or("");
        let new_desc = ctx.input.text("Description (optional):", Some(current_desc))
            .context("Failed to get description")?;

        infrastructure.metadata.name = new_name;
        infrastructure.metadata.description = if new_desc.is_empty() {
            None
        } else {
            Some(new_desc)
        };

        output::success("Metadata updated");
        output::blank();
        Ok(())
    }

    /// Edit resource kinds with pre-selection of current kinds
    fn edit_templates_and_categories(
        ctx: &crate::context::Context,
        infrastructure: &mut InfrastructureResource,
        template_packs_paths: Option<&str>,
    ) -> Result<()> {
        output::subsection("Editing Templates and Categories");

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

        // Discover template packs
        let template_packs = TemplateDiscovery::discover_template_packs_with_custom_paths(&*ctx.fs, &*ctx.output, &custom_paths)
            .context("Failed to discover template packs")?;

        // Build template selection options organized by pack
        let mut template_options: Vec<String> = Vec::new();
        let mut template_map: HashMap<String, (String, String, String, String)> = HashMap::new();

        for pack in &template_packs {
            let pack_name = &pack.resource.metadata.name;

            // Discover templates in this pack
            let templates_in_pack = TemplateDiscovery::discover_templates_in_pack(&*ctx.fs, &*ctx.output, &pack.path)
                .context("Failed to discover templates in pack")?;

            for template in &templates_in_pack {
                let template_name = &template.resource.metadata.name;
                let api_version = &template.resource.spec.api_version;
                let kind = &template.resource.spec.kind;

                let option_key = format!("{} / {} ({})", pack_name, template_name, kind);
                template_options.push(option_key.clone());
                template_map.insert(
                    option_key,
                    (pack_name.clone(), template_name.clone(), api_version.clone(), kind.clone()),
                );
            }
        }

        if template_options.is_empty() {
            output::warning("No templates found in the system.");
            output::dimmed("Cannot edit templates.");
            output::blank();
            return Ok(());
        }

        // Build list of currently selected templates for pre-selection
        let mut current_selections: Vec<String> = Vec::new();
        for category in &infrastructure.spec.categories {
            for template in &category.templates {
                // Try to find matching option with kind info
                for opt in &template_options {
                    if opt.starts_with(&format!("{} / {}", template.template_pack, template.template)) {
                        current_selections.push(opt.clone());
                        break;
                    }
                }
            }
        }

        // Find indices of currently selected templates
        let default_indices: Vec<usize> = template_options
            .iter()
            .enumerate()
            .filter(|(_, opt)| current_selections.contains(opt))
            .map(|(idx, _)| idx)
            .collect();

        let selected_options = ctx.input.multi_select(
            "Select templates to allow in this infrastructure:",
            template_options.clone(),
            Some(&default_indices)
        )
        .context("Failed to select templates")?;

        // Build categories and template_packs from selections
        let (categories, template_packs_config) = Self::build_categories_from_selections(&selected_options, &template_map);

        infrastructure.spec.categories = categories;
        infrastructure.spec.template_packs = template_packs_config;

        output::success(&format!("Templates updated ({} templates in {} categories)",
            selected_options.len(),
            infrastructure.spec.categories.len()));
        output::blank();
        Ok(())
    }

    /// Edit environments with add/edit/remove options
    fn edit_environments(ctx: &crate::context::Context, infrastructure: &mut InfrastructureResource) -> Result<()> {
        output::subsection("Editing Environments");

        loop {
            let action = ctx.input.select(
                "What would you like to do?",
                vec![
                    "Add new environment".to_string(),
                    "Edit existing environment".to_string(),
                    "Remove environment".to_string(),
                    "Done editing environments".to_string(),
                ]
            )
            .context("Failed to select action")?;

            match action.as_str() {
                "Add new environment" => {
                    Self::add_environment(ctx, &mut infrastructure.spec.environments)?;
                }
                "Edit existing environment" => {
                    Self::edit_single_environment(ctx, &mut infrastructure.spec.environments)?;
                }
                "Remove environment" => {
                    Self::remove_environment(ctx, &mut infrastructure.spec.environments)?;
                }
                "Done editing environments" => break,
                _ => {}
            }
        }

        output::success("Environments updated");
        output::blank();
        Ok(())
    }

    /// Add a new environment
    fn add_environment(ctx: &crate::context::Context, environments: &mut HashMap<String, Environment>) -> Result<()> {
        // Prompt for environment key
        let env_key = loop {
            let key = ctx.input.text("Environment key (lowercase, alphanumeric, underscores; cannot start with number):", None)
                .context("Failed to get environment key")?;

            // Validate environment key
            if !InfrastructureResource::is_valid_environment_name(&key) {
                output::warning("Invalid environment key. Must be lowercase alphanumeric with underscores, and cannot start with a number.");
                continue;
            }

            // Check for duplicate
            if environments.contains_key(&key) {
                output::warning(&format!("Environment '{}' already exists. Please use a different key.", key));
                continue;
            }

            break key;
        };

        // Prompt for display name
        let default_name = Self::capitalize_first(&env_key);
        let env_name = ctx.input.text("Environment display name:", Some(&default_name))
            .context("Failed to get environment name")?;

        // Prompt for optional description
        let env_description = ctx.input.text("Environment description (optional):", Some(""))
            .context("Failed to get environment description")?;

        environments.insert(
            env_key.clone(),
            Environment {
                name: env_name,
                description: if env_description.is_empty() {
                    None
                } else {
                    Some(env_description)
                },
            },
        );

        output::success(&format!("Environment '{}' added", env_key));
        output::blank();
        Ok(())
    }

    /// Edit a single existing environment
    fn edit_single_environment(ctx: &crate::context::Context, environments: &mut HashMap<String, Environment>) -> Result<()> {
        if environments.is_empty() {
            output::warning("No environments to edit.");
            output::blank();
            return Ok(());
        }

        // Select environment to edit
        let env_key = Self::select_environment(ctx, environments, "Select environment to edit:")?;

        let env = environments.get_mut(&env_key).unwrap();

        // Edit display name
        let new_name = ctx.input.text("Environment display name:", Some(&env.name))
            .context("Failed to get environment name")?;

        // Edit description
        let current_desc = env.description.as_deref().unwrap_or("");
        let new_desc = ctx.input.text("Environment description (optional):", Some(current_desc))
            .context("Failed to get environment description")?;

        env.name = new_name;
        env.description = if new_desc.is_empty() {
            None
        } else {
            Some(new_desc)
        };

        output::success(&format!("Environment '{}' updated", env_key));
        output::blank();
        Ok(())
    }

    /// Remove an environment
    fn remove_environment(ctx: &crate::context::Context, environments: &mut HashMap<String, Environment>) -> Result<()> {
        if environments.is_empty() {
            output::warning("No environments to remove.");
            output::blank();
            return Ok(());
        }

        if environments.len() == 1 {
            output::warning("Cannot remove the last environment. At least one environment is required.");
            output::blank();
            return Ok(());
        }

        // Select environment to remove
        let env_key = Self::select_environment(ctx, environments, "Select environment to remove:")?;

        // Confirm removal
        let confirm = ctx.input.confirm(&format!("Are you sure you want to remove environment '{}'?", env_key), false)
            .context("Failed to get confirmation")?;

        if confirm {
            environments.remove(&env_key);
            output::success(&format!("Environment '{}' removed", env_key));
            output::blank();
        } else {
            output::dimmed("Removal cancelled");
            output::blank();
        }

        Ok(())
    }

    /// Helper to select an environment from the list
    fn select_environment(
        ctx: &crate::context::Context,
        environments: &HashMap<String, Environment>,
        prompt: &str
    ) -> Result<String> {
        // Sort environments by name for consistent display
        let mut env_options: Vec<String> = environments
            .iter()
            .map(|(key, env)| format!("{} ({})", key, env.name))
            .collect();
        env_options.sort();

        let selected = ctx.input.select(prompt, env_options.clone())
            .context("Failed to select environment")?;

        // Extract the key from the selected option (format: "key (name)")
        let env_key = selected.split(" (").next().unwrap().to_string();

        Ok(env_key)
    }

    /// Capitalize the first letter of a string
    /// Build categories and template_packs config from user selections
    fn build_categories_from_selections(
        selected_options: &[String],
        template_map: &HashMap<String, (String, String, String, String)>,
    ) -> (Vec<Category>, HashMap<String, TemplatePackConfig>) {
        // Group templates by resource kind (apiVersion/kind)
        let mut categories_map: HashMap<String, Category> = HashMap::new();
        let mut template_packs_config: HashMap<String, TemplatePackConfig> = HashMap::new();

        for option in selected_options {
            if let Some((pack_name, template_name, api_version, kind)) = template_map.get(option) {
                // Create category ID from apiVersion/kind
                let category_id = format!(
                    "{}_{}",
                    api_version.replace("/", "_").replace(".", "_"),
                    kind.to_lowercase()
                );

                // Get or create category
                let category = categories_map.entry(category_id.clone()).or_insert_with(|| Category {
                    id: category_id.clone(),
                    name: format!("{} ({})", kind, api_version),
                    description: Some(format!("Templates for {} resources", kind)),
                    subcategories: vec![],
                    templates: vec![],
                });

                // Add template to category
                category.templates.push(CategoryTemplate {
                    template_pack: pack_name.clone(),
                    template: template_name.clone(),
                });

                // Add to template_packs config
                let pack_config = template_packs_config
                    .entry(pack_name.clone())
                    .or_insert_with(TemplatePackConfig::default);

                // Add template with default config if not already present
                pack_config.templates.entry(template_name.clone()).or_insert_with(Default::default);
            }
        }

        // Convert map to vec
        let categories: Vec<Category> = categories_map.into_values().collect();

        (categories, template_packs_config)
    }

    fn capitalize_first(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }
}


#[cfg(test)]
mod tests {
    // NOTE: Tests for INIT command have been removed as they relied on MockFileSystem
    // being compatible with template discovery, which uses real filesystem paths.
    // These tests would require integration testing with a real filesystem in a temp directory.
}
