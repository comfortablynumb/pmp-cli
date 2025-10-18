use crate::template::metadata::ProjectCollectionResource;
use anyhow::Result;
use std::path::{Path, PathBuf};

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
            let pmp_file = current.join(".pmp.yaml");

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
}
