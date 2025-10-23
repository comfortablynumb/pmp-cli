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

        // Walk through the projects directory recursively looking for .pmp.project.yaml files
        // Scan all levels, no depth limit
        for entry in WalkDir::new(&projects_dir)
            .min_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Look for .pmp.project.yaml files
            if path.is_file() && path.file_name() == Some(std::ffi::OsStr::new(".pmp.project.yaml"))
                && let Some(project_dir) = path.parent() {
                    // Try to load as a Project resource
                    match ProjectResource::from_file(path) {
                        Ok(resource) => {
                            // Get the resource kind from the first environment we find
                            let kind = Self::get_project_kind(project_dir)?;

                            // Calculate relative path from collection root
                            let relative_path = project_dir
                                .strip_prefix(collection_root)
                                .unwrap_or(project_dir)
                                .to_string_lossy()
                                .to_string();

                            projects.push(ProjectReference {
                                name: resource.metadata.name.clone(),
                                kind,
                                path: relative_path,
                            });
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to load project from {:?}: {}", path, e);
                        }
                    }
                }
        }

        Ok(projects)
    }

    /// Get the resource kind from a project by reading the first environment
    fn get_project_kind(project_dir: &Path) -> Result<String> {
        use crate::template::ProjectEnvironmentResource;

        let environments_dir = project_dir.join("environments");

        if !environments_dir.exists() {
            anyhow::bail!("No environments directory found in project: {:?}", project_dir);
        }

        // Find the first .pmp.environment.yaml file
        for entry in WalkDir::new(&environments_dir)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if path.is_file() && path.file_name() == Some(std::ffi::OsStr::new(".pmp.environment.yaml")) {
                match ProjectEnvironmentResource::from_file(path) {
                    Ok(resource) => {
                        return Ok(resource.spec.resource.kind.clone());
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to load environment from {:?}: {}", path, e);
                    }
                }
            }
        }

        anyhow::bail!("No valid environment found in project: {:?}", project_dir)
    }

    /// Discover all environments in a project
    pub fn discover_environments(project_dir: &Path) -> Result<Vec<String>> {
        use anyhow::Context;

        let environments_dir = project_dir.join("environments");

        if !environments_dir.exists() {
            return Ok(Vec::new());
        }

        let mut environments = Vec::new();

        // Look for subdirectories containing .pmp.environment.yaml files
        for entry in std::fs::read_dir(&environments_dir)
            .context("Failed to read environments directory")?
        {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if path.is_dir() {
                let env_file = path.join(".pmp.environment.yaml");
                if env_file.exists()
                    && let Some(env_name) = path.file_name() {
                        environments.push(env_name.to_string_lossy().to_string());
                    }
            }
        }

        environments.sort();
        Ok(environments)
    }
}
