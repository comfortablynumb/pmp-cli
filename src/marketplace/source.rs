use anyhow::Result;
use std::path::Path;

use super::index::PackInfo;

/// Trait for registry source implementations
pub trait RegistrySource: Send + Sync {
    /// Get the registry name
    fn name(&self) -> &str;

    /// List all packs in this registry
    fn list_packs(&self) -> Result<Vec<PackInfo>>;

    /// Search for packs matching a query
    fn search(&self, query: &str) -> Result<Vec<PackInfo>> {
        let packs = self.list_packs()?;
        Ok(packs.into_iter().filter(|p| p.matches_query(query)).collect())
    }

    /// Get info for a specific pack by name
    fn get_pack_info(&self, name: &str) -> Result<Option<PackInfo>> {
        let packs = self.list_packs()?;
        Ok(packs.into_iter().find(|p| p.name == name))
    }

    /// Install a pack to the destination directory
    fn install(
        &self,
        pack_name: &str,
        version: Option<&str>,
        dest: &Path,
    ) -> Result<InstallResult>;

    /// Check if the registry is available/healthy
    fn is_available(&self) -> bool {
        true
    }
}

/// Result of a pack installation
#[derive(Debug, Clone)]
pub struct InstallResult {
    pub pack_name: String,
    pub version: String,
    pub install_path: std::path::PathBuf,
}

/// Mock registry source for testing
#[cfg(test)]
pub struct MockRegistrySource {
    name: String,
    packs: Vec<PackInfo>,
}

#[cfg(test)]
impl MockRegistrySource {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            packs: Vec::new(),
        }
    }

    pub fn with_packs(mut self, packs: Vec<PackInfo>) -> Self {
        self.packs = packs;
        self
    }
}

#[cfg(test)]
impl RegistrySource for MockRegistrySource {
    fn name(&self) -> &str {
        &self.name
    }

    fn list_packs(&self) -> Result<Vec<PackInfo>> {
        Ok(self.packs.clone())
    }

    fn install(
        &self,
        pack_name: &str,
        version: Option<&str>,
        dest: &Path,
    ) -> Result<InstallResult> {
        let pack = self
            .get_pack_info(pack_name)?
            .ok_or_else(|| anyhow::anyhow!("Pack not found: {}", pack_name))?;

        let ver = version
            .map(|v| v.to_string())
            .or_else(|| pack.latest_version().map(|v| v.version.clone()))
            .unwrap_or_else(|| "latest".to_string());

        Ok(InstallResult {
            pack_name: pack_name.to_string(),
            version: ver,
            install_path: dest.join(pack_name),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marketplace::index::PackVersion;

    #[test]
    fn test_mock_registry_source() {
        let pack = PackInfo::new("test-pack", "https://github.com/test/pack")
            .with_description("Test pack")
            .with_versions(vec![PackVersion::new("1.0.0")]);

        let source = MockRegistrySource::new("test").with_packs(vec![pack]);

        assert_eq!(source.name(), "test");

        let packs = source.list_packs().unwrap();
        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].name, "test-pack");
    }

    #[test]
    fn test_mock_registry_search() {
        let pack1 = PackInfo::new("aws-vpc", "https://github.com/test/vpc")
            .with_tags(vec!["aws".to_string()]);
        let pack2 = PackInfo::new("azure-vnet", "https://github.com/test/vnet")
            .with_tags(vec!["azure".to_string()]);

        let source = MockRegistrySource::new("test").with_packs(vec![pack1, pack2]);

        let results = source.search("aws").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "aws-vpc");
    }

    #[test]
    fn test_mock_registry_get_pack_info() {
        let pack = PackInfo::new("test-pack", "https://github.com/test/pack");

        let source = MockRegistrySource::new("test").with_packs(vec![pack]);

        let found = source.get_pack_info("test-pack").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "test-pack");

        let not_found = source.get_pack_info("nonexistent").unwrap();
        assert!(not_found.is_none());
    }

    #[test]
    fn test_mock_registry_install() {
        let pack = PackInfo::new("test-pack", "https://github.com/test/pack")
            .with_versions(vec![PackVersion::new("1.0.0")]);

        let source = MockRegistrySource::new("test").with_packs(vec![pack]);
        let dest = std::path::Path::new("/tmp/packs");

        let result = source.install("test-pack", None, dest).unwrap();

        assert_eq!(result.pack_name, "test-pack");
        assert_eq!(result.version, "1.0.0");
        assert_eq!(result.install_path, dest.join("test-pack"));
    }
}
