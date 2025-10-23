use crate::collection::CollectionManager;
use crate::template::metadata::ProjectReference;
use anyhow::{Context, Result};
use inquire::{Select, Text};

pub struct FindCommand;

impl FindCommand {
    /// Execute the find command
    pub fn execute(
        name: Option<&str>,
        kind: Option<&str>,
    ) -> Result<()> {
        // Load the collection
        let manager = CollectionManager::load()?;

        // If no search criteria provided via CLI, ask interactively
        let projects = if name.is_none() && kind.is_none() {
            Self::interactive_search(&manager)?
        } else {
            // Find projects based on CLI criteria
            if let Some(name) = name {
                manager.find_by_name(name)
            } else if let Some(kind) = kind {
                manager.find_by_kind(kind)
            } else {
                // If no criteria specified, return all projects
                manager.get_all_projects().iter().collect()
            }
        };

        // Display results
        Self::display_results(&manager, &projects)?;

        Ok(())
    }

    /// Interactive search - ask user for search type and criteria
    fn interactive_search(manager: &CollectionManager) -> Result<Vec<&ProjectReference>> {
        // Ask user what type of search they want to perform
        let search_type_options = vec![
            "Search by name",
            "Search by kind",
            "Show all projects"
        ];
        let search_type = Select::new("How would you like to search?", search_type_options)
            .prompt()
            .context("Failed to select search type")?;

        match search_type {
            "Search by name" => {
                let search_term = Text::new("Enter project name (or part of it):")
                    .prompt()
                    .context("Failed to get search term")?;
                Ok(manager.find_by_name(&search_term))
            }
            "Search by kind" => {
                let search_term = Text::new("Enter resource kind:")
                    .prompt()
                    .context("Failed to get search term")?;
                Ok(manager.find_by_kind(&search_term))
            }
            "Show all projects" => Ok(manager.get_all_projects().iter().collect()),
            _ => Ok(vec![]),
        }
    }

    /// Display the search results
    fn display_results(manager: &CollectionManager, projects: &[&ProjectReference]) -> Result<()> {
        use crate::collection::CollectionDiscovery;
        use crate::template::{ProjectResource, ProjectEnvironmentResource};

        if projects.is_empty() {
            println!("No projects found.");
            return Ok(());
        }

        println!(
            "Found {} project(s) in collection '{}':\n",
            projects.len(),
            manager.get_metadata().name
        );

        // If multiple projects, let user select one
        let selected_project = if projects.len() == 1 {
            projects[0]
        } else {
            let project_options: Vec<String> = projects
                .iter()
                .map(|p| format!("{} ({})", p.name, p.kind))
                .collect();

            let selected = Select::new("Select a project to view details:", project_options.clone())
                .prompt()
                .context("Failed to select project")?;

            let index = project_options.iter().position(|opt| opt == &selected)
                .context("Project not found")?;

            projects[index]
        };

        let full_path = manager.get_project_path(selected_project);

        println!("\nProject Details:");
        println!("  Name: {}", selected_project.name);
        println!("  Kind: {}", selected_project.kind);
        println!("  Path: {}", selected_project.path);
        println!("  Full path: {}", full_path.display());

        // Load project metadata
        let project_file = full_path.join(".pmp.project.yaml");
        if !project_file.exists() {
            anyhow::bail!("Project file not found: {:?}", project_file);
        }

        let project_resource = ProjectResource::from_file(&project_file)
            .context("Failed to load project resource")?;

        if let Some(desc) = &project_resource.metadata.description {
            println!("  Description: {}", desc);
        }

        // Discover environments
        let environments = CollectionDiscovery::discover_environments(&full_path)
            .context("Failed to discover environments")?;

        if environments.is_empty() {
            println!("\nNo environments found for this project.");
            return Ok(());
        }

        println!("\nAvailable environments: {}", environments.join(", "));

        // Select environment
        let selected_env = if environments.len() == 1 {
            println!("Using environment: {}", &environments[0]);
            environments[0].clone()
        } else {
            Select::new("Select an environment:", environments.clone())
                .prompt()
                .context("Failed to select environment")?
        };

        // Load environment resource
        let env_path = full_path.join("environments").join(selected_env);
        let env_file = env_path.join(".pmp.environment.yaml");

        if !env_file.exists() {
            anyhow::bail!("Environment file not found: {:?}", env_file);
        }

        let env_resource = ProjectEnvironmentResource::from_file(&env_file)
            .context("Failed to load environment resource")?;

        println!("\nEnvironment Details:");
        println!("  Name: {}", env_resource.metadata.name);
        println!("  Project: {}", env_resource.metadata.project_name);
        if let Some(desc) = &env_resource.metadata.description {
            println!("  Description: {}", desc);
        }
        println!("  Resource: {}/{}", env_resource.spec.resource.api_version, env_resource.spec.resource.kind);
        println!("  Executor: {}", env_resource.spec.executor.name);
        println!("  Environment path: {}", env_path.display());

        println!("\nInputs:");
        for (key, value) in &env_resource.spec.inputs {
            println!("  {}: {}", key, value);
        }

        Ok(())
    }
}
