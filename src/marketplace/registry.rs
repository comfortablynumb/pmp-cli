use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::traits::FileSystem;

const REGISTRIES_FILE: &str = "registries.yaml";
const PMP_DIR: &str = ".pmp";

/// Registry configuration resource (Kubernetes-style)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryResource {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub metadata: RegistryMetadata,
    pub spec: RegistrySpec,
}

impl RegistryResource {
    pub fn new(name: &str, source: RegistrySourceConfig) -> Self {
        Self {
            api_version: "pmp.io/v1".to_string(),
            kind: "Registry".to_string(),
            metadata: RegistryMetadata {
                name: name.to_string(),
                description: None,
            },
            spec: RegistrySpec {
                source,
                priority: 50,
                enabled: true,
            },
        }
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.spec.priority = priority;
        self
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.metadata.description = Some(description.to_string());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryMetadata {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrySpec {
    pub source: RegistrySourceConfig,
    #[serde(default = "default_priority")]
    pub priority: i32,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_priority() -> i32 {
    50
}

fn default_true() -> bool {
    true
}

/// Registry source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum RegistrySourceConfig {
    /// URL-based registry (fetches JSON index from URL)
    Url { url: String },
    /// Filesystem-based registry (scans local directory)
    Filesystem { path: String },
}

/// List of registries stored in registries.yaml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryList {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub items: Vec<RegistryResource>,
}

impl Default for RegistryList {
    fn default() -> Self {
        Self {
            api_version: "pmp.io/v1".to_string(),
            kind: "RegistryList".to_string(),
            items: Vec::new(),
        }
    }
}

/// Manager for registry operations
pub struct RegistryManager<'a> {
    fs: &'a dyn FileSystem,
}

impl<'a> RegistryManager<'a> {
    pub fn new(fs: &'a dyn FileSystem) -> Self {
        Self { fs }
    }

    /// Get path to registries file
    fn get_registries_path(&self) -> Result<PathBuf> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Unable to determine home directory"))?;

        Ok(home_dir.join(PMP_DIR).join(REGISTRIES_FILE))
    }

    /// Load registries from file
    pub fn load_registries(&self) -> Result<Vec<RegistryResource>> {
        let path = self.get_registries_path()?;

        if !self.fs.exists(&path) {
            return Ok(Vec::new());
        }

        let content = self.fs.read_to_string(&path)?;
        let list: RegistryList = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse registries file: {:?}", path))?;

        Ok(list.items)
    }

    /// Save registries to file
    pub fn save_registries(&self, registries: &[RegistryResource]) -> Result<()> {
        let path = self.get_registries_path()?;

        let list = RegistryList {
            api_version: "pmp.io/v1".to_string(),
            kind: "RegistryList".to_string(),
            items: registries.to_vec(),
        };

        let content = serde_yaml::to_string(&list)
            .context("Failed to serialize registries")?;

        self.fs.write(&path, &content)?;

        Ok(())
    }

    /// Add a new registry
    pub fn add_registry(&self, registry: RegistryResource) -> Result<()> {
        let mut registries = self.load_registries()?;

        // Check for duplicate name
        if registries.iter().any(|r| r.metadata.name == registry.metadata.name) {
            bail!(
                "Registry with name '{}' already exists",
                registry.metadata.name
            );
        }

        registries.push(registry);
        self.save_registries(&registries)
    }

    /// Remove a registry by name
    pub fn remove_registry(&self, name: &str) -> Result<()> {
        let mut registries = self.load_registries()?;
        let initial_len = registries.len();

        registries.retain(|r| r.metadata.name != name);

        if registries.len() == initial_len {
            bail!("Registry '{}' not found", name);
        }

        self.save_registries(&registries)
    }

    /// Get a registry by name
    pub fn get_registry(&self, name: &str) -> Result<Option<RegistryResource>> {
        let registries = self.load_registries()?;
        Ok(registries.into_iter().find(|r| r.metadata.name == name))
    }

    /// Get all enabled registries sorted by priority (highest first)
    pub fn get_enabled_registries(&self) -> Result<Vec<RegistryResource>> {
        let mut registries = self.load_registries()?;

        registries.retain(|r| r.spec.enabled);
        registries.sort_by(|a, b| b.spec.priority.cmp(&a.spec.priority));

        Ok(registries)
    }

    /// Expand path with home directory support
    pub fn expand_path(path: &str) -> PathBuf {
        if path.starts_with("~/") {
            if let Some(home) = dirs::home_dir() {
                return home.join(&path[2..]);
            }
        }

        PathBuf::from(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::MockFileSystem;

    #[test]
    fn test_registry_resource_creation() {
        let registry = RegistryResource::new(
            "test",
            RegistrySourceConfig::Url {
                url: "https://example.com/index.json".to_string(),
            },
        )
        .with_priority(100)
        .with_description("Test registry");

        assert_eq!(registry.metadata.name, "test");
        assert_eq!(registry.spec.priority, 100);
        assert_eq!(
            registry.metadata.description,
            Some("Test registry".to_string())
        );
    }

    #[test]
    fn test_registry_source_config_serialization() {
        let url_source = RegistrySourceConfig::Url {
            url: "https://example.com/index.json".to_string(),
        };

        let yaml = serde_yaml::to_string(&url_source).unwrap();
        assert!(yaml.contains("type: url"));
        assert!(yaml.contains("https://example.com/index.json"));

        let fs_source = RegistrySourceConfig::Filesystem {
            path: "~/my-packs".to_string(),
        };

        let yaml = serde_yaml::to_string(&fs_source).unwrap();
        assert!(yaml.contains("type: filesystem"));
        assert!(yaml.contains("~/my-packs"));
    }

    #[test]
    fn test_registry_list_default() {
        let list = RegistryList::default();

        assert_eq!(list.api_version, "pmp.io/v1");
        assert_eq!(list.kind, "RegistryList");
        assert!(list.items.is_empty());
    }

    #[test]
    fn test_expand_path() {
        let path = RegistryManager::expand_path("~/test/path");
        assert!(!path.to_string_lossy().starts_with("~/"));

        let absolute = RegistryManager::expand_path("/absolute/path");
        assert_eq!(absolute, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_load_empty_registries() {
        let fs = MockFileSystem::new();
        let manager = RegistryManager::new(&fs);

        let registries = manager.load_registries().unwrap();
        assert!(registries.is_empty());
    }

    #[test]
    fn test_add_and_load_registry() {
        let fs = MockFileSystem::new();
        let manager = RegistryManager::new(&fs);

        let registry = RegistryResource::new(
            "test",
            RegistrySourceConfig::Url {
                url: "https://example.com/index.json".to_string(),
            },
        );

        manager.add_registry(registry).unwrap();

        let registries = manager.load_registries().unwrap();
        assert_eq!(registries.len(), 1);
        assert_eq!(registries[0].metadata.name, "test");
    }

    #[test]
    fn test_add_duplicate_registry_fails() {
        let fs = MockFileSystem::new();
        let manager = RegistryManager::new(&fs);

        let registry = RegistryResource::new(
            "test",
            RegistrySourceConfig::Url {
                url: "https://example.com/index.json".to_string(),
            },
        );

        manager.add_registry(registry.clone()).unwrap();
        let result = manager.add_registry(registry);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_remove_registry() {
        let fs = MockFileSystem::new();
        let manager = RegistryManager::new(&fs);

        let registry = RegistryResource::new(
            "test",
            RegistrySourceConfig::Url {
                url: "https://example.com/index.json".to_string(),
            },
        );

        manager.add_registry(registry).unwrap();
        manager.remove_registry("test").unwrap();

        let registries = manager.load_registries().unwrap();
        assert!(registries.is_empty());
    }

    #[test]
    fn test_remove_nonexistent_registry_fails() {
        let fs = MockFileSystem::new();
        let manager = RegistryManager::new(&fs);

        let result = manager.remove_registry("nonexistent");

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_get_enabled_registries_sorted() {
        let fs = MockFileSystem::new();
        let manager = RegistryManager::new(&fs);

        let low = RegistryResource::new(
            "low",
            RegistrySourceConfig::Url {
                url: "https://low.com".to_string(),
            },
        )
        .with_priority(10);

        let high = RegistryResource::new(
            "high",
            RegistrySourceConfig::Url {
                url: "https://high.com".to_string(),
            },
        )
        .with_priority(100);

        manager.add_registry(low).unwrap();
        manager.add_registry(high).unwrap();

        let registries = manager.get_enabled_registries().unwrap();

        assert_eq!(registries.len(), 2);
        assert_eq!(registries[0].metadata.name, "high");
        assert_eq!(registries[1].metadata.name, "low");
    }
}
