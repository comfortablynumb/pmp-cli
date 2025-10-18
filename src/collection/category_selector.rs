use crate::template::metadata::Category;
use anyhow::{Context, Result};
use inquire::Select;
use std::collections::HashMap;

/// Handles hierarchical category selection with navigation
pub struct CategorySelector;

impl CategorySelector {
    /// Navigate through hierarchical categories and return the selected leaf category path
    pub fn select_category(categories: &HashMap<String, Category>) -> Result<String> {
        Self::navigate_categories(categories, Vec::new())
    }

    /// Recursively navigate through category hierarchy
    fn navigate_categories(
        categories: &HashMap<String, Category>,
        path: Vec<String>,
    ) -> Result<String> {
        // Sort category keys for consistent display
        let mut category_keys: Vec<String> = categories.keys().cloned().collect();
        category_keys.sort();

        // Build display options with descriptions
        let mut options: Vec<String> = category_keys
            .iter()
            .map(|key| {
                let category = &categories[key];
                if let Some(desc) = &category.description {
                    format!("{} - {}", category.name, desc)
                } else {
                    category.name.clone()
                }
            })
            .collect();

        // Add "Go back" option if we're not at the root level
        let has_back_option = !path.is_empty();
        if has_back_option {
            options.insert(0, "← Go back".to_string());
        }

        // Show current path
        let prompt_message = if path.is_empty() {
            "Select a category:".to_string()
        } else {
            format!("Select a category (current path: {})", path.join(" > "))
        };

        let selection = Select::new(&prompt_message, options)
            .prompt()
            .context("Failed to select category")?;

        // Handle "Go back" option
        if has_back_option && selection == "← Go back" {
            return Err(anyhow::anyhow!("GO_BACK")); // Special signal to go back
        }

        // Find the selected category key
        let selected_index = if has_back_option {
            // Account for the "Go back" option
            category_keys
                .iter()
                .position(|key| {
                    let category = &categories[key];
                    let display = if let Some(desc) = &category.description {
                        format!("{} - {}", category.name, desc)
                    } else {
                        category.name.clone()
                    };
                    display == selection
                })
                .context("Selected category not found")?
        } else {
            category_keys
                .iter()
                .position(|key| {
                    let category = &categories[key];
                    let display = if let Some(desc) = &category.description {
                        format!("{} - {}", category.name, desc)
                    } else {
                        category.name.clone()
                    };
                    display == selection
                })
                .context("Selected category not found")?
        };

        let selected_key = &category_keys[selected_index];
        let selected_category = &categories[selected_key];

        // Build the new path
        let mut new_path = path.clone();
        new_path.push(selected_key.clone());

        // If this is a leaf category, return the path
        if selected_category.is_leaf() {
            return Ok(new_path.join("/"));
        }

        // Otherwise, navigate to children
        if let Some(children) = &selected_category.children {
            match Self::navigate_categories(children, new_path.clone()) {
                Ok(result) => Ok(result),
                Err(e) => {
                    if e.to_string() == "GO_BACK" {
                        // User chose to go back, show this level again
                        Self::navigate_categories(categories, path)
                    } else {
                        // Real error, propagate it
                        Err(e)
                    }
                }
            }
        } else {
            // No children but not a leaf? Return the current path
            Ok(new_path.join("/"))
        }
    }

    /// Get all leaf category paths from a category hierarchy (for validation)
    pub fn get_all_leaf_paths(categories: &HashMap<String, Category>) -> Vec<String> {
        let mut paths = Vec::new();
        for (key, category) in categories {
            paths.extend(category.get_leaf_paths(key));
        }
        paths.sort();
        paths
    }
}
