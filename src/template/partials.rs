use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Information about a discovered Handlebars partial
#[derive(Debug, Clone)]
pub struct PartialInfo {
    /// Name of the partial (filename without .hbs extension)
    pub name: String,
    /// Content of the partial template
    pub content: String,
    /// Source file path
    pub source: PathBuf,
}

/// Discovers Handlebars partials from template packs and global locations
pub struct PartialDiscovery;

impl PartialDiscovery {
    /// Discover all partials with proper priority ordering
    /// Priority (highest to lowest):
    /// 1. Pack partials from {pack}/partials/*.hbs
    /// 2. Global partials from ~/.pmp/partials/*.hbs
    ///
    /// When the same partial name exists in multiple locations,
    /// the higher priority version wins.
    pub fn discover_all(
        fs: &dyn crate::traits::FileSystem,
        pack_path: Option<&Path>,
    ) -> Result<Vec<PartialInfo>> {
        let mut partials_by_name: HashMap<String, PartialInfo> = HashMap::new();

        // 1. Load global partials first (lowest priority)
        if let Some(home_dir) = dirs::home_dir() {
            let global_partials_path = home_dir.join(".pmp").join("partials");

            if fs.exists(&global_partials_path) {
                let global_partials = Self::load_partials_from_dir(fs, &global_partials_path)?;

                for partial in global_partials {
                    partials_by_name.insert(partial.name.clone(), partial);
                }
            }
        }

        // 2. Load pack partials (highest priority, overrides global)
        if let Some(pack) = pack_path {
            let pack_partials_path = pack.join("partials");

            if fs.exists(&pack_partials_path) {
                let pack_partials = Self::load_partials_from_dir(fs, &pack_partials_path)?;

                for partial in pack_partials {
                    partials_by_name.insert(partial.name.clone(), partial);
                }
            }
        }

        Ok(partials_by_name.into_values().collect())
    }

    /// Load partials from a specific directory
    /// Looks for *.hbs files and loads them
    fn load_partials_from_dir(
        fs: &dyn crate::traits::FileSystem,
        partials_dir: &Path,
    ) -> Result<Vec<PartialInfo>> {
        let mut partials = Vec::new();

        if !fs.exists(partials_dir) {
            return Ok(partials);
        }

        let entries = fs.read_dir(partials_dir)?;

        for entry_path in entries {
            if !fs.is_file(&entry_path) {
                continue;
            }

            // Check for .hbs extension
            let extension = entry_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");

            if extension != "hbs" {
                continue;
            }

            // Get partial name (filename without extension)
            let name = entry_path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            if name.is_empty() {
                continue;
            }

            // Read content
            let content = fs.read_to_string(&entry_path)?;

            partials.push(PartialInfo {
                name,
                content,
                source: entry_path,
            });
        }

        Ok(partials)
    }

    /// Discover partials from multiple pack paths
    /// Used when working with template inheritance where multiple packs may contribute partials
    #[allow(dead_code)]
    pub fn discover_from_paths(
        fs: &dyn crate::traits::FileSystem,
        pack_paths: &[&Path],
    ) -> Result<Vec<PartialInfo>> {
        let mut partials_by_name: HashMap<String, PartialInfo> = HashMap::new();

        // 1. Load global partials first (lowest priority)
        if let Some(home_dir) = dirs::home_dir() {
            let global_partials_path = home_dir.join(".pmp").join("partials");

            if fs.exists(&global_partials_path) {
                let global_partials = Self::load_partials_from_dir(fs, &global_partials_path)?;

                for partial in global_partials {
                    partials_by_name.insert(partial.name.clone(), partial);
                }
            }
        }

        // 2. Load pack partials in order (later paths have higher priority)
        for pack_path in pack_paths {
            let pack_partials_path = pack_path.join("partials");

            if fs.exists(&pack_partials_path) {
                let pack_partials = Self::load_partials_from_dir(fs, &pack_partials_path)?;

                for partial in pack_partials {
                    partials_by_name.insert(partial.name.clone(), partial);
                }
            }
        }

        Ok(partials_by_name.into_values().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{FileSystem, MockFileSystem};

    #[test]
    fn test_partial_discovery_empty() {
        let fs = MockFileSystem::new();
        let partials = PartialDiscovery::discover_all(&fs, None).unwrap();
        assert!(partials.is_empty());
    }

    #[test]
    fn test_partial_discovery_pack_partials() {
        let fs = MockFileSystem::new();

        let pack_path = PathBuf::from("/pack");
        let partials_dir = pack_path.join("partials");
        let partial_file = partials_dir.join("common_tags.hbs");

        fs.create_dir_all(&partials_dir).unwrap();
        fs.write(&partial_file, "tags = { ManagedBy = \"PMP\" }")
            .unwrap();

        let partials = PartialDiscovery::discover_all(&fs, Some(&pack_path)).unwrap();

        assert_eq!(partials.len(), 1);
        assert_eq!(partials[0].name, "common_tags");
        assert!(partials[0].content.contains("ManagedBy"));
    }

    #[test]
    fn test_partial_discovery_priority() {
        let fs = MockFileSystem::new();

        // Create global partial
        let home = PathBuf::from(dirs::home_dir().unwrap_or_default());
        let global_partials_dir = home.join(".pmp").join("partials");
        let global_partial = global_partials_dir.join("shared.hbs");

        fs.create_dir_all(&global_partials_dir).unwrap();
        fs.write(&global_partial, "global content").unwrap();

        // Create pack partial with same name
        let pack_path = PathBuf::from("/pack");
        let pack_partials_dir = pack_path.join("partials");
        let pack_partial = pack_partials_dir.join("shared.hbs");

        fs.create_dir_all(&pack_partials_dir).unwrap();
        fs.write(&pack_partial, "pack content").unwrap();

        let partials = PartialDiscovery::discover_all(&fs, Some(&pack_path)).unwrap();

        // Pack partial should win
        let shared = partials.iter().find(|p| p.name == "shared").unwrap();
        assert_eq!(shared.content, "pack content");
    }

    #[test]
    fn test_partial_name_extraction() {
        let fs = MockFileSystem::new();

        let pack_path = PathBuf::from("/pack");
        let partials_dir = pack_path.join("partials");

        fs.create_dir_all(&partials_dir).unwrap();

        // Various filenames
        fs.write(&partials_dir.join("simple.hbs"), "content1")
            .unwrap();
        fs.write(&partials_dir.join("multi.part.name.hbs"), "content2")
            .unwrap();
        fs.write(&partials_dir.join("not_a_partial.txt"), "ignored")
            .unwrap();

        let partials = PartialDiscovery::discover_all(&fs, Some(&pack_path)).unwrap();

        assert_eq!(partials.len(), 2);

        let names: Vec<&str> = partials.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"simple"));
        assert!(names.contains(&"multi.part.name"));
    }
}
