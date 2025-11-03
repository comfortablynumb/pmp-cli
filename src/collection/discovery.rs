use crate::template::metadata::{InfrastructureResource, ProjectReference, ProjectResource};
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Discovery for Infrastructure resources
pub struct CollectionDiscovery;

impl CollectionDiscovery {
    /// Try to find an Infrastructure in the current directory or parent directories
    pub fn find_collection(fs: &dyn crate::traits::FileSystem) -> Result<Option<(InfrastructureResource, PathBuf)>> {
        let current_dir = std::env::current_dir()?;
        Self::find_collection_in_path(fs, &current_dir)
    }

    /// Try to find an Infrastructure starting from a specific path
    pub fn find_collection_in_path(
        fs: &dyn crate::traits::FileSystem,
        start_path: &Path,
    ) -> Result<Option<(InfrastructureResource, PathBuf)>> {
        let mut current = start_path.to_path_buf();

        loop {
            let pmp_file = current.join(".pmp.infrastructure.yaml");

            if fs.exists(&pmp_file) {
                // Try to load as Infrastructure
                if let Ok(infrastructure) = InfrastructureResource::from_file(fs, &pmp_file) {
                    return Ok(Some((infrastructure, current)));
                }
            }

            // Move to parent directory
            if !current.pop() {
                break;
            }
        }

        Ok(None)
    }

    /// Check if the current directory is inside an Infrastructure
    #[allow(dead_code)]
    pub fn is_in_collection(fs: &dyn crate::traits::FileSystem) -> Result<bool> {
        Ok(Self::find_collection(fs)?.is_some())
    }

    /// Get the path to the infrastructure root directory
    #[allow(dead_code)]
    pub fn get_collection_root(fs: &dyn crate::traits::FileSystem) -> Result<Option<PathBuf>> {
        Ok(Self::find_collection(fs)?.map(|(_, path)| path))
    }

    /// Discover all projects in the "projects" folder of an infrastructure
    /// Scans all levels of subdirectories to find .pmp.yaml files
    pub fn discover_projects(fs: &dyn crate::traits::FileSystem, output: &dyn crate::traits::Output, infrastructure_root: &Path) -> Result<Vec<ProjectReference>> {
        let projects_dir = infrastructure_root.join("projects");

        if !fs.exists(&projects_dir) {
            return Ok(Vec::new());
        }

        let mut projects = Vec::new();

        // Walk through the projects directory recursively looking for .pmp.project.yaml files
        // Scan all levels, no depth limit (using a high value)
        let entries = fs.walk_dir(&projects_dir, 100)?;

        for path in entries {
            // Look for .pmp.project.yaml files
            if fs.is_file(&path) && path.file_name() == Some(std::ffi::OsStr::new(".pmp.project.yaml"))
                && let Some(project_dir) = path.parent() {
                    // Try to load as a Project resource
                    match ProjectResource::from_file(fs, &path) {
                        Ok(resource) => {
                            // Get the resource kind from the first environment we find
                            let kind = Self::get_project_kind(fs, output, project_dir)?;

                            // Calculate relative path from infrastructure root
                            let relative_path = project_dir
                                .strip_prefix(infrastructure_root)
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
                            output.warning(&format!("Failed to load project from {:?}: {}", path, e));
                        }
                    }
                }
        }

        Ok(projects)
    }

    /// Get the resource kind from a project by reading the first environment
    fn get_project_kind(fs: &dyn crate::traits::FileSystem, output: &dyn crate::traits::Output, project_dir: &Path) -> Result<String> {
        use crate::template::DynamicProjectEnvironmentResource;

        let environments_dir = project_dir.join("environments");

        if !fs.exists(&environments_dir) {
            anyhow::bail!("No environments directory found in project: {:?}", project_dir);
        }

        // Find the first .pmp.environment.yaml file
        let entries = fs.walk_dir(&environments_dir, 2)?;

        for path in entries {
            if fs.is_file(&path) && path.file_name() == Some(std::ffi::OsStr::new(".pmp.environment.yaml")) {
                match DynamicProjectEnvironmentResource::from_file(fs, &path) {
                    Ok(resource) => {
                        return Ok(resource.kind.clone());
                    }
                    Err(e) => {
                        output.warning(&format!("Failed to load environment from {:?}: {}", path, e));
                    }
                }
            }
        }

        anyhow::bail!("No valid environment found in project: {:?}", project_dir)
    }

    /// Discover all environments in a project
    pub fn discover_environments(fs: &dyn crate::traits::FileSystem, project_dir: &Path) -> Result<Vec<String>> {
        use anyhow::Context;

        let environments_dir = project_dir.join("environments");

        if !fs.exists(&environments_dir) {
            return Ok(Vec::new());
        }

        let mut environments = Vec::new();

        // Look for subdirectories containing .pmp.environment.yaml files
        let entries = fs.read_dir(&environments_dir)
            .context("Failed to read environments directory")?;

        for entry_path in entries {
            if fs.is_dir(&entry_path) {
                let env_file = entry_path.join(".pmp.environment.yaml");
                if fs.exists(&env_file)
                    && let Some(env_name) = entry_path.file_name() {
                        environments.push(env_name.to_string_lossy().to_string());
                    }
            }
        }

        environments.sort();
        Ok(environments)
    }
}
