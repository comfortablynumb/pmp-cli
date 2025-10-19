use crate::collection::{CategorySelector, CollectionManager};
use crate::template::metadata::ProjectReference;
use anyhow::{Context, Result};
use inquire::{Select, Text};

pub struct FindCommand;

impl FindCommand {
    /// Execute the find command
    pub fn execute(
        name: Option<&str>,
        category: Option<&str>,
        _kind: Option<&str>, // Keep for compatibility but don't use
    ) -> Result<()> {
        // Load the collection
        let manager = CollectionManager::load()?;

        // If no search criteria provided via CLI, ask interactively
        let projects = if name.is_none() && category.is_none() {
            Self::interactive_search(&manager)?
        } else {
            // Find projects based on CLI criteria
            if let Some(name) = name {
                manager.find_by_name(name)
            } else if let Some(category) = category {
                manager.find_by_category(category)
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
            "Search by category",
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
            "Search by category" => {
                // Check if the collection has defined categories
                let selected_category = if let Some(categories) = manager.get_categories() {
                    // Use hierarchical category selection
                    CategorySelector::select_category(categories)
                        .context("Failed to select category")?
                } else {
                    // Fallback to listing unique categories from existing projects
                    let categories: Vec<String> = manager
                        .get_all_projects()
                        .iter()
                        .filter_map(|p| p.category.clone())
                        .collect::<std::collections::HashSet<_>>()
                        .into_iter()
                        .collect();

                    if categories.is_empty() {
                        println!("No categories found in the collection.");
                        return Ok(vec![]);
                    }

                    let mut sorted_categories = categories;
                    sorted_categories.sort();

                    Select::new("Select a category:", sorted_categories)
                        .prompt()
                        .context("Failed to select category")?
                };

                Ok(manager.find_by_category(&selected_category))
            }
            "Show all projects" => Ok(manager.get_all_projects().iter().collect()),
            _ => Ok(vec![]),
        }
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
