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
    projects: Vec<ProjectReference>,
}

impl CollectionManager {
    /// Load the collection from the current directory or parent directories
    pub fn load(ctx: &crate::context::Context) -> Result<Self> {
        let (collection, root_path) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("No ProjectCollection found in current directory or parent directories")?;

        // Discover projects in the "projects" folder
        let projects = CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &root_path)?;

        Ok(Self {
            collection,
            root_path,
            projects,
        })
    }

    /// Load the collection from a specific path
    #[allow(dead_code)]
    pub fn load_from_path(ctx: &crate::context::Context, path: &Path) -> Result<Self> {
        let (collection, root_path) = CollectionDiscovery::find_collection_in_path(&*ctx.fs, path)?
            .context("No ProjectCollection found at the specified path")?;

        // Discover projects in the "projects" folder
        let projects = CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &root_path)?;

        Ok(Self {
            collection,
            root_path,
            projects,
        })
    }

    /// Create a new ProjectCollection at the specified path
    #[allow(dead_code)]
    pub fn create(ctx: &crate::context::Context, path: &Path, name: String, description: Option<String>) -> Result<Self> {
        let pmp_file = path.join(".pmp.project-collection.yaml");

        if ctx.fs.exists(&pmp_file) {
            anyhow::bail!("A .pmp.project-collection.yaml file already exists at this location");
        }

        let collection = ProjectCollectionResource {
            api_version: "pmp.io/v1".to_string(),
            kind: "ProjectCollection".to_string(),
            metadata: ProjectCollectionMetadata { name, description },
            spec: ProjectCollectionSpec {
                resource_kinds: vec![],
                environments: std::collections::HashMap::new(),
                hooks: None,
                executor: None,
            },
        };

        collection.save(&*ctx.fs, &pmp_file)?;

        // Create the projects directory
        let projects_dir = path.join("projects");
        ctx.fs.create_dir_all(&projects_dir)?;

        Ok(Self {
            collection,
            root_path: path.to_path_buf(),
            projects: vec![],
        })
    }

    /// Refresh the list of discovered projects
    #[allow(dead_code)]
    pub fn refresh_projects(&mut self, ctx: &crate::context::Context) -> Result<()> {
        self.projects = CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &self.root_path)?;
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

}
