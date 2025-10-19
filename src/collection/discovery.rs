use crate::template::metadata::{ProjectCollectionResource, ProjectReference, ProjectResource};
use anyhow::Result;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Discovery for ProjectCollection resources
pub struct CollectionDiscovery;

impl CollectionDiscovery {
    /// Try to find a ProjectCollection in the current directory or parent directories
    pub fn find_collection() -> Result<Option<(ProjectCollectionResource, PathBuf)>> {
        let current_dir = std::env::current_dir()?;
        Self::find_collection_in_path(&current_dir)
    }

    /// Try to find a ProjectCollection starting from a specific path
    pub fn find_collection_in_path(
        start_path: &Path,
    ) -> Result<Option<(ProjectCollectionResource, PathBuf)>> {
        let mut current = start_path.to_path_buf();

        loop {
            let pmp_file = current.join(".pmp.project-collection.yaml");

            if pmp_file.exists() {
                // Try to load as ProjectCollection
                if let Ok(collection) = ProjectCollectionResource::from_file(&pmp_file) {
                    return Ok(Some((collection, current)));
                }
            }

            // Move to parent directory
            if !current.pop() {
                break;
            }
        }

        Ok(None)
    }

    /// Check if the current directory is inside a ProjectCollection
    #[allow(dead_code)]
    pub fn is_in_collection() -> Result<bool> {
        Ok(Self::find_collection()?.is_some())
    }

    /// Get the path to the collection root directory
    #[allow(dead_code)]
    pub fn get_collection_root() -> Result<Option<PathBuf>> {
        Ok(Self::find_collection()?.map(|(_, path)| path))
    }

    /// Discover all projects in the "projects" folder of a collection
    /// Scans all levels of subdirectories to find .pmp.yaml files
    pub fn discover_projects(collection_root: &Path) -> Result<Vec<ProjectReference>> {
        let projects_dir = collection_root.join("projects");

        if !projects_dir.exists() {
            return Ok(Vec::new());
        }

        let mut projects = Vec::new();

        // Walk through the projects directory recursively looking for .pmp.yaml files
        // Scan all levels, no depth limit
        for entry in WalkDir::new(&projects_dir)
            .min_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Look for .pmp.yaml files (not .pmp.project.yaml)
            if path.is_file() && path.file_name() == Some(std::ffi::OsStr::new(".pmp.yaml")) {
                if let Some(project_dir) = path.parent() {
                    // Try to load as a Project resource
                    match ProjectResource::from_file(path) {
                        Ok(resource) => {
                            // Calculate relative path from collection root
                            let relative_path = project_dir
                                .strip_prefix(collection_root)
                                .unwrap_or(project_dir)
                                .to_string_lossy()
                                .to_string();

                            // Extract category from path if organize_by_category is used
                            // Format: projects/<category>/... or projects/<resource-kind>/<project-name>
                            let path_components: Vec<&str> = relative_path
                                .split(std::path::MAIN_SEPARATOR)
                                .collect();
                            let category = if path_components.len() > 2 && path_components[0] == "projects" {
                                Some(path_components[1].to_string())
                            } else {
                                None
                            };

                            projects.push(ProjectReference {
                                name: resource.metadata.name.clone(),
                                kind: resource.kind.clone(),
                                path: relative_path,
                                category,
                                search_categories: resource.spec.search_categories.clone(),
                            });
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to load project from {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        Ok(projects)
    }
}
