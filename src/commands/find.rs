use crate::collection::CollectionManager;
use crate::template::metadata::ProjectReference;
use anyhow::Result;

pub struct FindCommand;

impl FindCommand {
    /// Execute the find command
    pub fn execute(
        name: Option<&str>,
        category: Option<&str>,
        kind: Option<&str>,
    ) -> Result<()> {
        // Load the collection
        let manager = CollectionManager::load()?;

        // Find projects based on criteria
        let projects = if let Some(name) = name {
            manager.find_by_name(name)
        } else if let Some(category) = category {
            manager.find_by_category(category)
        } else if let Some(kind) = kind {
            manager.find_by_kind(kind)
        } else {
            // If no criteria specified, return all projects
            manager.get_all_projects().iter().collect()
        };

        // Display results
        Self::display_results(&manager, &projects)?;

        Ok(())
    }

    /// Display the search results
    fn display_results(manager: &CollectionManager, projects: &[&ProjectReference]) -> Result<()> {
        if projects.is_empty() {
            println!("No projects found.");
            return Ok(());
        }

        println!(
            "Found {} project(s) in collection '{}':\n",
            projects.len(),
            manager.get_metadata().name
        );

        for project in projects {
            println!("  Name:     {}", project.name);
            println!("  Kind:     {}", project.kind);
            println!("  Path:     {}", project.path);
            if let Some(category) = &project.category {
                println!("  Category: {}", category);
            }

            let full_path = manager.get_project_path(project);
            println!("  Full path: {}", full_path.display());
            println!();
        }

        Ok(())
    }
}
