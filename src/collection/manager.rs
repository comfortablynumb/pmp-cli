use crate::collection::discovery::CollectionDiscovery;
use crate::template::metadata::{
    ProjectCollectionMetadata, ProjectCollectionResource, ProjectCollectionSpec, ProjectReference,
};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::fs;

/// Manager for ProjectCollection operations
pub struct CollectionManager {
    collection: ProjectCollectionResource,
    root_path: PathBuf,
    projects: Vec<ProjectReference>,
}

impl CollectionManager {
    /// Load the collection from the current directory or parent directories
    pub fn load() -> Result<Self> {
        let (collection, root_path) = CollectionDiscovery::find_collection()?
            .context("No ProjectCollection found in current directory or parent directories")?;

        // Discover projects in the "projects" folder
        let projects = CollectionDiscovery::discover_projects(&root_path)?;

        Ok(Self {
            collection,
            root_path,
            projects,
        })
    }

    /// Load the collection from a specific path
    #[allow(dead_code)]
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let (collection, root_path) = CollectionDiscovery::find_collection_in_path(path)?
            .context("No ProjectCollection found at the specified path")?;

        // Discover projects in the "projects" folder
        let projects = CollectionDiscovery::discover_projects(&root_path)?;

        Ok(Self {
            collection,
            root_path,
            projects,
        })
    }

    /// Create a new ProjectCollection at the specified path
    #[allow(dead_code)]
    pub fn create(path: &Path, name: String, description: Option<String>) -> Result<Self> {
        let pmp_file = path.join(".pmp.project-collection.yaml");

        if pmp_file.exists() {
            anyhow::bail!("A .pmp.project-collection.yaml file already exists at this location");
        }

        let collection = ProjectCollectionResource {
            api_version: "pmp.io/v1".to_string(),
            kind: "ProjectCollection".to_string(),
            metadata: ProjectCollectionMetadata { name, description },
            spec: ProjectCollectionSpec {
                organize_by_category: false,
            },
        };

        collection.save(&pmp_file)?;

        // Create the projects directory
        let projects_dir = path.join("projects");
        fs::create_dir_all(&projects_dir)?;

        Ok(Self {
            collection,
            root_path: path.to_path_buf(),
            projects: vec![],
        })
    }

    /// Refresh the list of discovered projects
    #[allow(dead_code)]
    pub fn refresh_projects(&mut self) -> Result<()> {
        self.projects = CollectionDiscovery::discover_projects(&self.root_path)?;
        Ok(())
    }

    /// Find projects by name (case-insensitive)
    pub fn find_by_name(&self, name: &str) -> Vec<&ProjectReference> {
        let name_lower = name.to_lowercase();
        self.projects
            .iter()
            .filter(|p| p.name.to_lowercase().contains(&name_lower))
            .collect()
    }

    /// Find projects by category
    pub fn find_by_category(&self, category: &str) -> Vec<&ProjectReference> {
        self.projects
            .iter()
            .filter(|p| {
                p.category
                    .as_ref()
                    .map(|c| c.eq_ignore_ascii_case(category))
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Find projects by kind
    pub fn find_by_kind(&self, kind: &str) -> Vec<&ProjectReference> {
        self.projects
            .iter()
            .filter(|p| p.kind.eq_ignore_ascii_case(kind))
            .collect()
    }

    /// Get all projects in the collection
    pub fn get_all_projects(&self) -> &[ProjectReference] {
        &self.projects
    }

    /// Get the collection metadata
    pub fn get_metadata(&self) -> &ProjectCollectionMetadata {
        &self.collection.metadata
    }

    /// Get the collection root path
    #[allow(dead_code)]
    pub fn get_root_path(&self) -> &Path {
        &self.root_path
    }

    /// Get the full path to a project
    pub fn get_project_path(&self, project: &ProjectReference) -> PathBuf {
        self.root_path.join(&project.path)
    }

    /// Set whether to organize projects by category
    #[allow(dead_code)]
    pub fn set_organize_by_category(&mut self, organize: bool) -> Result<()> {
        self.collection.spec.organize_by_category = organize;
        let pmp_file = self.root_path.join(".pmp.project-collection.yaml");
        self.collection.save(&pmp_file)?;
        Ok(())
    }

    /// Check if the collection organizes projects by category
    #[allow(dead_code)]
    pub fn organizes_by_category(&self) -> bool {
        self.collection.spec.organize_by_category
    }
}
