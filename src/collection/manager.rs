use crate::collection::discovery::CollectionDiscovery;
use crate::template::metadata::{
    InfrastructureMetadata, InfrastructureResource, InfrastructureSpec, ProjectReference,
};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Manager for Infrastructure operations
pub struct CollectionManager {
    infrastructure: InfrastructureResource,
    root_path: PathBuf,
    projects: Vec<ProjectReference>,
}

impl CollectionManager {
    /// Load the infrastructure from the current directory or parent directories
    pub fn load(ctx: &crate::context::Context) -> Result<Self> {
        let (infrastructure, root_path) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("No Infrastructure found in current directory or parent directories")?;

        // Discover projects in the "projects" folder
        let projects = CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &root_path)?;

        Ok(Self {
            infrastructure,
            root_path,
            projects,
        })
    }

    /// Load the infrastructure from a specific path
    #[allow(dead_code)]
    pub fn load_from_path(ctx: &crate::context::Context, path: &Path) -> Result<Self> {
        let (infrastructure, root_path) = CollectionDiscovery::find_collection_in_path(&*ctx.fs, path)?
            .context("No Infrastructure found at the specified path")?;

        // Discover projects in the "projects" folder
        let projects = CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &root_path)?;

        Ok(Self {
            infrastructure,
            root_path,
            projects,
        })
    }

    /// Create a new Infrastructure at the specified path
    #[allow(dead_code)]
    pub fn create(ctx: &crate::context::Context, path: &Path, name: String, description: Option<String>) -> Result<Self> {
        let pmp_file = path.join(".pmp.infrastructure.yaml");

        if ctx.fs.exists(&pmp_file) {
            anyhow::bail!("A .pmp.infrastructure.yaml file already exists at this location");
        }

        let infrastructure = InfrastructureResource {
            api_version: "pmp.io/v1".to_string(),
            kind: "Infrastructure".to_string(),
            metadata: InfrastructureMetadata { name, description },
            spec: InfrastructureSpec {
                categories: vec![],
                template_packs: std::collections::HashMap::new(),
                resource_kinds: vec![],
                environments: std::collections::HashMap::new(),
                hooks: None,
                executor: None,
            },
        };

        infrastructure.save(&*ctx.fs, &pmp_file)?;

        // Create the projects directory
        let projects_dir = path.join("projects");
        ctx.fs.create_dir_all(&projects_dir)?;

        Ok(Self {
            infrastructure,
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

    /// Get all projects in the infrastructure
    pub fn get_all_projects(&self) -> &[ProjectReference] {
        &self.projects
    }

    /// Get the infrastructure metadata
    pub fn get_metadata(&self) -> &InfrastructureMetadata {
        &self.infrastructure.metadata
    }

    /// Get the infrastructure root path
    #[allow(dead_code)]
    pub fn get_root_path(&self) -> &Path {
        &self.root_path
    }

    /// Get the full path to a project
    pub fn get_project_path(&self, project: &ProjectReference) -> PathBuf {
        self.root_path.join(&project.path)
    }

}
