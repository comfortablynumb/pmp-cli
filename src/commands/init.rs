use crate::output;
use crate::template::TemplateDiscovery;
use crate::template::metadata::{
    Environment, ProjectCollectionMetadata, ProjectCollectionResource,
    ProjectCollectionSpec, ResourceKindFilter,
};
use anyhow::{Context, Result};
use inquire::{Confirm, MultiSelect, Select, Text};
use std::collections::HashMap;
use std::path::PathBuf;

/// Handles the 'init' command - initializes or edits a ProjectCollection
pub struct InitCommand;

impl InitCommand {
    /// Execute the init command
    pub fn execute(
        name: Option<&str>,
        description: Option<&str>,
        template_packs_path: Option<&str>,
    ) -> Result<()> {
        let current_dir = std::env::current_dir()
            .context("Failed to get current directory")?;

        let collection_file = current_dir.join(".pmp.project-collection.yaml");

        // Check if collection already exists and route accordingly
        if collection_file.exists() {
            Self::edit_existing_collection(&collection_file, template_packs_path)?;
        } else {
            Self::create_new_collection(&current_dir, &collection_file, name, description, template_packs_path)?;
        }

        Ok(())
    }

    /// Create a new ProjectCollection
    fn create_new_collection(
        current_dir: &PathBuf,
        collection_file: &PathBuf,
        name: Option<&str>,
        description: Option<&str>,
        template_packs_path: Option<&str>,
    ) -> Result<()> {
        output::section("Initialize Project Collection");
        output::key_value("Directory", &current_dir.display().to_string());
        output::blank();

        // Get collection name (use CLI arg or prompt with default)
        let collection_name = if let Some(n) = name {
            n.to_string()
        } else {
            Text::new("Collection name:")
                .with_default("My Infrastructure")
                .prompt()
                .context("Failed to get collection name")?
        };

        // Step 2: Get description (use CLI arg or prompt, optional)
        let collection_description = if let Some(d) = description {
            Some(d.to_string())
        } else {
            let desc = Text::new("Description (optional):")
                .with_default("")
                .prompt()
                .context("Failed to get description")?;
            if desc.is_empty() {
                None
            } else {
                Some(desc)
            }
        };

        // Step 3: Discover templates to get available resource kinds
        let custom_paths = if let Some(path) = template_packs_path {
            vec![path]
        } else {
            vec![]
        };

        let templates = TemplateDiscovery::discover_templates_with_custom_paths(&custom_paths)
            .context("Failed to discover templates")?;

        // Step 4: Extract unique resource kinds from templates
        let mut resource_kinds_map: HashMap<String, ResourceKindFilter> = HashMap::new();
        for template in &templates {
            let key = format!(
                "{}/{}",
                template.resource.spec.api_version,
                template.resource.spec.kind
            );
            resource_kinds_map.insert(
                key,
                ResourceKindFilter {
                    api_version: template.resource.spec.api_version.clone(),
                    kind: template.resource.spec.kind.clone(),
                },
            );
        }

        // Step 5: Present resource kinds as multi-select
        let resource_kinds = if resource_kinds_map.is_empty() {
            output::warning("No templates found in the system.");
            output::dimmed("The ProjectCollection will be created without any resource kinds.");
            output::dimmed("You can add templates later and update the collection file.");
            output::blank();
            vec![]
        } else {
            let kind_options: Vec<String> = resource_kinds_map.keys().cloned().collect();

            let selected_keys = MultiSelect::new(
                "Select resource kinds to allow in this collection:",
                kind_options.clone()
            )
            .prompt()
            .context("Failed to select resource kinds")?;

            selected_keys
                .iter()
                .filter_map(|key| resource_kinds_map.get(key).cloned())
                .collect()
        };

        // Step 6: Collect environments
        let mut environments: HashMap<String, Environment> = HashMap::new();

        output::subsection("Environments");
        output::dimmed("Let's add environments to the collection.");
        output::dimmed("You need at least one environment.");
        output::blank();

        loop {
            // Prompt for environment key
            let env_key = loop {
                let key = Text::new("Environment key (lowercase, alphanumeric, hyphens):")
                    .prompt()
                    .context("Failed to get environment key")?;

                // Validate environment key
                if !ProjectCollectionResource::is_valid_environment_name(&key) {
                    output::warning("Invalid environment key. Must be lowercase alphanumeric and may contain hyphens.");
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
            let env_name = Text::new("Environment display name:")
                .with_default(&Self::capitalize_first(&env_key))
                .prompt()
                .context("Failed to get environment name")?;

            // Prompt for optional description
            let env_description = Text::new("Environment description (optional):")
                .with_default("")
                .prompt()
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
            let add_another = Confirm::new("Add another environment?")
                .with_default(false)
                .prompt()
                .context("Failed to get confirmation")?;

            if !add_another {
                break;
            }
        }

        // Ensure at least one environment was added
        if environments.is_empty() {
            anyhow::bail!("At least one environment is required.");
        }

        // Step 7: Create the ProjectCollectionResource
        let collection = ProjectCollectionResource {
            api_version: "pmp.io/v1".to_string(),
            kind: "ProjectCollection".to_string(),
            metadata: ProjectCollectionMetadata {
                name: collection_name.clone(),
                description: collection_description,
            },
            spec: ProjectCollectionSpec {
                resource_kinds,
                environments,
                hooks: None,
                executor: None,
            },
        };

        // Step 8: Save the collection file
        collection.save(&collection_file)
            .context("Failed to save .pmp.project-collection.yaml")?;

        // Step 9: Create the projects directory
        let projects_dir = current_dir.join("projects");
        std::fs::create_dir_all(&projects_dir)
            .context("Failed to create projects directory")?;

        // Step 10: Display success message
        output::blank();
        output::success("ProjectCollection created successfully!");

        output::subsection("Summary");
        output::key_value_highlight("Collection", &collection_name);
        output::key_value("File", &collection_file.display().to_string());
        output::key_value("Projects directory", &projects_dir.display().to_string());
        output::key_value("Resource kinds", &collection.spec.resource_kinds.len().to_string());
        output::key_value("Environments", &collection.spec.environments.len().to_string());

        let next_steps_list = vec![
            format!("Review and edit {} if needed", collection_file.display()),
            "Run 'pmp create' to create a new project from a template".to_string(),
        ];
        output::next_steps(&next_steps_list);

        Ok(())
    }

    /// Edit an existing ProjectCollection
    fn edit_existing_collection(
        collection_file: &PathBuf,
        template_packs_path: Option<&str>,
    ) -> Result<()> {
        output::section("Edit Project Collection");
        output::key_value("File", &collection_file.display().to_string());
        output::blank();

        // Load existing collection
        let mut collection = ProjectCollectionResource::from_file(collection_file)
            .context("Failed to load existing collection")?;

        // Iterative editing loop
        loop {
            // Display current collection info
            Self::display_collection_summary(&collection);

            // Present editing options with hints
            let options = vec![
                "Metadata - Edit collection name and description",
                "Resource kinds - Add or remove allowed resource kinds from templates",
                "Environments - Add, edit, or remove environments",
                "Exit - Save and exit",
            ];

            let choice = Select::new("What would you like to edit?", options)
                .prompt()
                .context("Failed to select option")?;

            // Handle the selection
            match choice {
                opt if opt.starts_with("Metadata") => {
                    Self::edit_metadata(&mut collection)?;

                    // Save after editing
                    collection.save(collection_file)
                        .context("Failed to save collection")?;
                    output::success(&format!("Changes saved to {}", collection_file.display()));
                    output::blank();

                    // Ask if they want to continue
                    let continue_editing = Confirm::new("Continue editing?")
                        .with_default(true)
                        .prompt()
                        .context("Failed to get confirmation")?;

                    if !continue_editing {
                        break;
                    }
                }
                opt if opt.starts_with("Resource kinds") => {
                    Self::edit_resource_kinds(&mut collection, template_packs_path)?;

                    // Save after editing
                    collection.save(collection_file)
                        .context("Failed to save collection")?;
                    output::success(&format!("Changes saved to {}", collection_file.display()));
                    output::blank();

                    // Ask if they want to continue
                    let continue_editing = Confirm::new("Continue editing?")
                        .with_default(true)
                        .prompt()
                        .context("Failed to get confirmation")?;

                    if !continue_editing {
                        break;
                    }
                }
                opt if opt.starts_with("Environments") => {
                    Self::edit_environments(&mut collection)?;

                    // Save after editing
                    collection.save(collection_file)
                        .context("Failed to save collection")?;
                    output::success(&format!("Changes saved to {}", collection_file.display()));
                    output::blank();

                    // Ask if they want to continue
                    let continue_editing = Confirm::new("Continue editing?")
                        .with_default(true)
                        .prompt()
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
        output::success("Done editing ProjectCollection!");
        output::key_value("File", &collection_file.display().to_string());

        Ok(())
    }

    /// Display a summary of the current collection
    fn display_collection_summary(collection: &ProjectCollectionResource) {
        output::subsection("Current Collection");
        output::key_value_highlight("Name", &collection.metadata.name);
        if let Some(desc) = &collection.metadata.description {
            output::key_value("Description", desc);
        }
        output::key_value("Resource kinds", &collection.spec.resource_kinds.len().to_string());
        for rk in &collection.spec.resource_kinds {
            output::list_item(&format!("{}/{}", rk.api_version, rk.kind));
        }
        output::key_value("Environments", &collection.spec.environments.len().to_string());
        for (key, env) in &collection.spec.environments {
            output::list_item(&format!("{} ({})", key, env.name));
        }
        output::blank();
    }

    /// Edit collection metadata
    fn edit_metadata(collection: &mut ProjectCollectionResource) -> Result<()> {
        output::subsection("Editing Metadata");

        let new_name = Text::new("Collection name:")
            .with_default(&collection.metadata.name)
            .prompt()
            .context("Failed to get collection name")?;

        let current_desc = collection.metadata.description.as_deref().unwrap_or("");
        let new_desc = Text::new("Description (optional):")
            .with_default(current_desc)
            .prompt()
            .context("Failed to get description")?;

        collection.metadata.name = new_name;
        collection.metadata.description = if new_desc.is_empty() {
            None
        } else {
            Some(new_desc)
        };

        output::success("Metadata updated");
        output::blank();
        Ok(())
    }

    /// Edit resource kinds with pre-selection of current kinds
    fn edit_resource_kinds(
        collection: &mut ProjectCollectionResource,
        template_packs_path: Option<&str>,
    ) -> Result<()> {
        output::subsection("Editing Resource Kinds");

        // Discover templates
        let custom_paths = if let Some(path) = template_packs_path {
            vec![path]
        } else {
            vec![]
        };

        let templates = TemplateDiscovery::discover_templates_with_custom_paths(&custom_paths)
            .context("Failed to discover templates")?;

        // Extract unique resource kinds from templates
        let mut resource_kinds_map: HashMap<String, ResourceKindFilter> = HashMap::new();
        for template in &templates {
            let key = format!(
                "{}/{}",
                template.resource.spec.api_version,
                template.resource.spec.kind
            );
            resource_kinds_map.insert(
                key,
                ResourceKindFilter {
                    api_version: template.resource.spec.api_version.clone(),
                    kind: template.resource.spec.kind.clone(),
                },
            );
        }

        if resource_kinds_map.is_empty() {
            output::warning("No templates found in the system.");
            output::dimmed("Cannot edit resource kinds.");
            output::blank();
            return Ok(());
        }

        let kind_options: Vec<String> = resource_kinds_map.keys().cloned().collect();

        // Find which current resource kinds should be pre-selected
        let current_kind_keys: Vec<String> = collection
            .spec
            .resource_kinds
            .iter()
            .map(|rk| format!("{}/{}", rk.api_version, rk.kind))
            .collect();

        // Find indices of currently selected kinds
        let default_indices: Vec<usize> = kind_options
            .iter()
            .enumerate()
            .filter(|(_, opt)| current_kind_keys.contains(opt))
            .map(|(idx, _)| idx)
            .collect();

        let selected_keys = MultiSelect::new(
            "Select resource kinds to allow in this collection:",
            kind_options.clone()
        )
        .with_default(&default_indices)
        .prompt()
        .context("Failed to select resource kinds")?;

        collection.spec.resource_kinds = selected_keys
            .iter()
            .filter_map(|key| resource_kinds_map.get(key).cloned())
            .collect();

        output::success(&format!("Resource kinds updated ({} selected)", collection.spec.resource_kinds.len()));
        output::blank();
        Ok(())
    }

    /// Edit environments with add/edit/remove options
    fn edit_environments(collection: &mut ProjectCollectionResource) -> Result<()> {
        output::subsection("Editing Environments");

        loop {
            let action = Select::new(
                "What would you like to do?",
                vec![
                    "Add new environment",
                    "Edit existing environment",
                    "Remove environment",
                    "Done editing environments"
                ]
            )
            .prompt()
            .context("Failed to select action")?;

            match action {
                "Add new environment" => {
                    Self::add_environment(&mut collection.spec.environments)?;
                }
                "Edit existing environment" => {
                    Self::edit_single_environment(&mut collection.spec.environments)?;
                }
                "Remove environment" => {
                    Self::remove_environment(&mut collection.spec.environments)?;
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
    fn add_environment(environments: &mut HashMap<String, Environment>) -> Result<()> {
        // Prompt for environment key
        let env_key = loop {
            let key = Text::new("Environment key (lowercase, alphanumeric, hyphens):")
                .prompt()
                .context("Failed to get environment key")?;

            // Validate environment key
            if !ProjectCollectionResource::is_valid_environment_name(&key) {
                output::warning("Invalid environment key. Must be lowercase alphanumeric and may contain hyphens.");
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
        let env_name = Text::new("Environment display name:")
            .with_default(&Self::capitalize_first(&env_key))
            .prompt()
            .context("Failed to get environment name")?;

        // Prompt for optional description
        let env_description = Text::new("Environment description (optional):")
            .with_default("")
            .prompt()
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
    fn edit_single_environment(environments: &mut HashMap<String, Environment>) -> Result<()> {
        if environments.is_empty() {
            output::warning("No environments to edit.");
            output::blank();
            return Ok(());
        }

        // Select environment to edit
        let env_key = Self::select_environment(environments, "Select environment to edit:")?;

        let env = environments.get_mut(&env_key).unwrap();

        // Edit display name
        let new_name = Text::new("Environment display name:")
            .with_default(&env.name)
            .prompt()
            .context("Failed to get environment name")?;

        // Edit description
        let current_desc = env.description.as_deref().unwrap_or("");
        let new_desc = Text::new("Environment description (optional):")
            .with_default(current_desc)
            .prompt()
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
    fn remove_environment(environments: &mut HashMap<String, Environment>) -> Result<()> {
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
        let env_key = Self::select_environment(environments, "Select environment to remove:")?;

        // Confirm removal
        let confirm = Confirm::new(&format!("Are you sure you want to remove environment '{}'?", env_key))
            .with_default(false)
            .prompt()
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
        environments: &HashMap<String, Environment>,
        prompt: &str
    ) -> Result<String> {
        let env_options: Vec<String> = environments
            .iter()
            .map(|(key, env)| format!("{} ({})", key, env.name))
            .collect();

        let selected = Select::new(prompt, env_options.clone())
            .prompt()
            .context("Failed to select environment")?;

        // Extract the key from the selected option (format: "key (name)")
        let env_key = selected.split(" (").next().unwrap().to_string();

        Ok(env_key)
    }

    /// Capitalize the first letter of a string
    fn capitalize_first(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }
}
