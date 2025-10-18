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
    pub fn discover_projects(collection_root: &Path) -> Result<Vec<ProjectReference>> {
        let projects_dir = collection_root.join("projects");

        if !projects_dir.exists() {
            return Ok(Vec::new());
        }

        let mut projects = Vec::new();

        // Walk through the projects directory looking for .pmp.project.yaml files
        // When a project is found, skip scanning its subdirectories
        let mut walker = WalkDir::new(&projects_dir)
            .min_depth(1)
            .into_iter();

        while let Some(entry) = walker.next() {
            if let Ok(entry) = entry {
                let path = entry.path();

                if path.is_file() && path.file_name() == Some(std::ffi::OsStr::new(".pmp.project.yaml"))
                    && let Some(project_dir) = path.parent() {
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
                                // Format: projects/<category>/<project-name>
                                let path_components: Vec<&str> = relative_path.split(std::path::MAIN_SEPARATOR).collect();
                                let category = if path_components.len() > 2 && path_components[0] == "projects" {
                                    Some(path_components[1].to_string())
                                } else {
                                    None
                                };

                                projects.push(ProjectReference {
                                    name: resource.metadata.name,
                                    kind: resource.kind,
                                    path: relative_path,
                                    category,
                                });

                                // Skip scanning this project's subdirectories
                                walker.skip_current_dir();
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
