use crate::collection::discovery::CollectionDiscovery;
use crate::template::metadata::{
    ProjectCollectionMetadata, ProjectCollectionResource, ProjectCollectionSpec, ProjectReference,
};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Manager for ProjectCollection operations
pub struct CollectionManager {
    collection: ProjectCollectionResource,
    root_path: PathBuf,
}

impl CollectionManager {
    /// Load the collection from the current directory or parent directories
    pub fn load() -> Result<Self> {
        let (collection, root_path) = CollectionDiscovery::find_collection()?
            .context("No ProjectCollection found in current directory or parent directories")?;

        Ok(Self {
            collection,
            root_path,
        })
    }

    /// Load the collection from a specific path
    #[allow(dead_code)]
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let (collection, root_path) = CollectionDiscovery::find_collection_in_path(path)?
            .context("No ProjectCollection found at the specified path")?;

        Ok(Self {
            collection,
            root_path,
        })
    }

    /// Create a new ProjectCollection at the specified path
    #[allow(dead_code)]
    pub fn create(path: &Path, name: String, description: Option<String>) -> Result<Self> {
        let pmp_file = path.join(".pmp.yaml");

        if pmp_file.exists() {
            anyhow::bail!("A .pmp.yaml file already exists at this location");
        }

        let collection = ProjectCollectionResource {
            api_version: "pmp.io/v1".to_string(),
            kind: "ProjectCollection".to_string(),
            metadata: ProjectCollectionMetadata { name, description },
            spec: ProjectCollectionSpec {
                projects: vec![],
                organize_by_category: false,
            },
        };

        collection.save(&pmp_file)?;

        Ok(Self {
            collection,
            root_path: path.to_path_buf(),
        })
    }

    /// Add a project to the collection
    #[allow(dead_code)]
    pub fn add_project(
        &mut self,
        name: String,
        kind: String,
        path: String,
        category: Option<String>,
    ) -> Result<()> {
        // Check for duplicates
        if self.collection.has_project(&name, &kind) {
            anyhow::bail!(
                "A project with name '{}' and kind '{}' already exists in the collection",
                name,
                kind
            );
        }

        let project_ref = ProjectReference {
            name,
            kind,
            path,
            category,
        };

        self.collection.add_project(project_ref);
        self.save()?;

        Ok(())
    }

    /// Remove a project from the collection
    #[allow(dead_code)]
    pub fn remove_project(&mut self, name: &str, kind: &str) -> Result<()> {
        if !self.collection.remove_project(name, kind) {
            anyhow::bail!(
                "Project with name '{}' and kind '{}' not found in collection",
                name,
                kind
            );
        }

        self.save()?;
        Ok(())
    }

    /// Find projects by name
    pub fn find_by_name(&self, name: &str) -> Vec<&ProjectReference> {
        self.collection.find_by_name(name)
    }

    /// Find projects by category
    pub fn find_by_category(&self, category: &str) -> Vec<&ProjectReference> {
        self.collection.find_by_category(category)
    }

    /// Find projects by kind
    pub fn find_by_kind(&self, kind: &str) -> Vec<&ProjectReference> {
        self.collection.find_by_kind(kind)
    }

    /// Get all projects in the collection
    pub fn get_all_projects(&self) -> &[ProjectReference] {
        &self.collection.spec.projects
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
        self.save()?;
        Ok(())
    }

    /// Check if the collection organizes projects by category
    #[allow(dead_code)]
    pub fn organizes_by_category(&self) -> bool {
        self.collection.spec.organize_by_category
    }

    /// Save the collection to disk
    #[allow(dead_code)]
    fn save(&self) -> Result<()> {
        let pmp_file = self.root_path.join(".pmp.yaml");
        self.collection.save(&pmp_file)
    }
}
