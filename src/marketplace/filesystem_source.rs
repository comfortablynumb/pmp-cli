use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};

use crate::template::metadata::TemplatePackResource;
use crate::traits::FileSystem;

use super::index::{PackInfo, PackVersion};
use super::source::{InstallResult, RegistrySource};

const TEMPLATE_PACK_FILE: &str = ".pmp.template-pack.yaml";
const TEMPLATES_DIR: &str = "templates";
const VERSIONS_DIR: &str = "versions";

/// Filesystem-based registry source
pub struct FilesystemSource<'a> {
    name: String,
    path: PathBuf,
    fs: &'a dyn FileSystem,
}

impl<'a> FilesystemSource<'a> {
    /// Create a new filesystem source
    pub fn new(name: &str, path: PathBuf, fs: &'a dyn FileSystem) -> Self {
        Self {
            name: name.to_string(),
            path,
            fs,
        }
    }

    /// Discover template packs in the directory
    fn discover_packs(&self) -> Result<Vec<DiscoveredPack>> {
        let mut packs = Vec::new();

        if !self.fs.exists(&self.path) {
            return Ok(packs);
        }

        // Look for .pmp.template-pack.yaml files
        let entries = self.fs.read_dir(&self.path)?;

        for entry in entries {
            if !self.fs.is_dir(&entry) {
                continue;
            }

            let pack_file = entry.join(TEMPLATE_PACK_FILE);

            if self.fs.exists(&pack_file) {
                match self.parse_pack(&entry, &pack_file) {
                    Ok(pack) => packs.push(pack),
                    Err(e) => {
                        // Log warning but continue discovering other packs
                        eprintln!(
                            "Warning: Failed to parse template pack at {:?}: {}",
                            entry, e
                        );
                    }
                }
            }
        }

        Ok(packs)
    }

    /// Parse a template pack from its directory
    fn parse_pack(&self, pack_dir: &Path, pack_file: &Path) -> Result<DiscoveredPack> {
        let content = self.fs.read_to_string(pack_file)?;
        let resource: TemplatePackResource = serde_yaml::from_str(&content)
            .with_context(|| format!("Failed to parse {:?}", pack_file))?;

        let versions = self.discover_versions(pack_dir)?;

        Ok(DiscoveredPack {
            name: resource.metadata.name.clone(),
            description: resource.metadata.description.clone(),
            path: pack_dir.to_path_buf(),
            versions,
        })
    }

    /// Discover versions for a template pack
    fn discover_versions(&self, pack_dir: &Path) -> Result<Vec<String>> {
        let mut versions = Vec::new();

        let templates_dir = pack_dir.join(TEMPLATES_DIR);

        if !self.fs.exists(&templates_dir) {
            return Ok(versions);
        }

        // Check each template for versions
        let template_entries = self.fs.read_dir(&templates_dir)?;

        for template_dir in template_entries {
            if !self.fs.is_dir(&template_dir) {
                continue;
            }

            let versions_dir = template_dir.join(VERSIONS_DIR);

            if self.fs.exists(&versions_dir) && self.fs.is_dir(&versions_dir) {
                let version_entries = self.fs.read_dir(&versions_dir)?;

                for version_dir in version_entries {
                    if !self.fs.is_dir(&version_dir) {
                        continue;
                    }

                    if let Some(version_name) = version_dir.file_name() {
                        let version_str = version_name.to_string_lossy().to_string();

                        // Validate semver
                        if semver::Version::parse(&version_str).is_ok() {
                            if !versions.contains(&version_str) {
                                versions.push(version_str);
                            }
                        }
                    }
                }
            }
        }

        // Sort versions descending
        versions.sort_by(|a, b| {
            let va = semver::Version::parse(a).ok();
            let vb = semver::Version::parse(b).ok();
            vb.cmp(&va)
        });

        Ok(versions)
    }
}

impl<'a> RegistrySource for FilesystemSource<'a> {
    fn name(&self) -> &str {
        &self.name
    }

    fn list_packs(&self) -> Result<Vec<PackInfo>> {
        let discovered = self.discover_packs()?;

        let packs = discovered
            .into_iter()
            .map(|d| d.into_pack_info())
            .collect();

        Ok(packs)
    }

    fn install(
        &self,
        pack_name: &str,
        version: Option<&str>,
        dest: &Path,
    ) -> Result<InstallResult> {
        let discovered = self.discover_packs()?;

        let pack = discovered
            .iter()
            .find(|p| p.name == pack_name)
            .ok_or_else(|| anyhow::anyhow!("Pack '{}' not found in filesystem registry", pack_name))?;

        // Determine version
        let ver = if let Some(v) = version {
            if !pack.versions.contains(&v.to_string()) {
                bail!("Version '{}' not found for pack '{}'", v, pack_name);
            }
            v.to_string()
        } else {
            pack.versions.first().cloned().unwrap_or_else(|| "latest".to_string())
        };

        let install_path = dest.join(pack_name);

        // Copy pack to destination
        copy_directory(&pack.path, &install_path, self.fs)?;

        Ok(InstallResult {
            pack_name: pack_name.to_string(),
            version: ver,
            install_path,
        })
    }

    fn is_available(&self) -> bool {
        self.fs.exists(&self.path) && self.fs.is_dir(&self.path)
    }
}

/// Internal struct for discovered packs
struct DiscoveredPack {
    name: String,
    description: Option<String>,
    path: PathBuf,
    versions: Vec<String>,
}

impl DiscoveredPack {
    fn into_pack_info(self) -> PackInfo {
        let versions = self
            .versions
            .into_iter()
            .map(|v| PackVersion::new(&v))
            .collect();

        let mut pack = PackInfo::new(&self.name, &format!("file://{}", self.path.display()));

        if let Some(desc) = self.description {
            pack = pack.with_description(&desc);
        }

        pack.with_versions(versions)
    }
}

/// Copy a directory recursively
fn copy_directory(src: &Path, dest: &Path, fs: &dyn FileSystem) -> Result<()> {
    if dest.exists() {
        bail!(
            "Destination already exists: {}\n\
             Remove it first or use 'pmp marketplace update' to update.",
            dest.display()
        );
    }

    fs.create_dir_all(dest)?;

    let entries = fs.read_dir(src)?;

    for entry in entries {
        let dest_path = dest.join(entry.file_name().unwrap());

        if fs.is_dir(&entry) {
            copy_directory(&entry, &dest_path, fs)?;
        } else {
            let content = fs.read_to_string(&entry)?;
            fs.write(&dest_path, &content)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::MockFileSystem;
    use std::path::PathBuf;

    fn setup_mock_pack(fs: &MockFileSystem, base: &Path, name: &str) {
        let pack_dir = base.join(name);

        // Create pack file
        let pack_file = pack_dir.join(TEMPLATE_PACK_FILE);
        let pack_content = format!(
            r#"apiVersion: pmp.io/v1
kind: TemplatePack
metadata:
  name: {}
  description: Test pack
spec: {{}}"#,
            name
        );
        fs.write(&pack_file, &pack_content).unwrap();
    }

    fn setup_mock_pack_with_versions(
        fs: &MockFileSystem,
        base: &Path,
        name: &str,
        versions: &[&str],
    ) {
        setup_mock_pack(fs, base, name);

        let pack_dir = base.join(name);
        let templates_dir = pack_dir.join(TEMPLATES_DIR).join("template1");
        let versions_dir = templates_dir.join(VERSIONS_DIR);

        fs.create_dir_all(&versions_dir).unwrap();

        for version in versions {
            let version_dir = versions_dir.join(version);
            fs.create_dir_all(&version_dir).unwrap();

            // Add a template file
            let template_file = version_dir.join(".pmp.template.yaml");
            fs.write(&template_file, "apiVersion: pmp.io/v1").unwrap();
        }
    }

    #[test]
    fn test_filesystem_source_list_packs() {
        let fs = MockFileSystem::new();
        let base = PathBuf::from("/packs");
        fs.create_dir_all(&base).unwrap();

        setup_mock_pack(&fs, &base, "pack1");
        setup_mock_pack(&fs, &base, "pack2");

        let source = FilesystemSource::new("test", base, &fs);
        let packs = source.list_packs().unwrap();

        assert_eq!(packs.len(), 2);

        let names: Vec<_> = packs.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"pack1"));
        assert!(names.contains(&"pack2"));
    }

    #[test]
    fn test_filesystem_source_discover_versions() {
        let fs = MockFileSystem::new();
        let base = PathBuf::from("/packs");
        fs.create_dir_all(&base).unwrap();

        setup_mock_pack_with_versions(&fs, &base, "versioned-pack", &["1.0.0", "2.0.0", "1.5.0"]);

        let source = FilesystemSource::new("test", base, &fs);
        let packs = source.list_packs().unwrap();

        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].name, "versioned-pack");
        assert_eq!(packs[0].versions.len(), 3);

        // Should be sorted descending
        assert_eq!(packs[0].versions[0].version, "2.0.0");
        assert_eq!(packs[0].versions[1].version, "1.5.0");
        assert_eq!(packs[0].versions[2].version, "1.0.0");
    }

    #[test]
    fn test_filesystem_source_search() {
        let fs = MockFileSystem::new();
        let base = PathBuf::from("/packs");
        fs.create_dir_all(&base).unwrap();

        setup_mock_pack(&fs, &base, "aws-vpc");
        setup_mock_pack(&fs, &base, "azure-vnet");

        let source = FilesystemSource::new("test", base, &fs);

        let results = source.search("aws").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "aws-vpc");
    }

    #[test]
    fn test_filesystem_source_get_pack_info() {
        let fs = MockFileSystem::new();
        let base = PathBuf::from("/packs");
        fs.create_dir_all(&base).unwrap();

        setup_mock_pack(&fs, &base, "test-pack");

        let source = FilesystemSource::new("test", base, &fs);

        let pack = source.get_pack_info("test-pack").unwrap();
        assert!(pack.is_some());
        assert_eq!(pack.unwrap().name, "test-pack");

        let pack = source.get_pack_info("nonexistent").unwrap();
        assert!(pack.is_none());
    }

    #[test]
    fn test_filesystem_source_is_available() {
        let fs = MockFileSystem::new();
        let base = PathBuf::from("/packs");

        let source = FilesystemSource::new("test", base.clone(), &fs);
        assert!(!source.is_available());

        fs.create_dir_all(&base).unwrap();
        assert!(source.is_available());
    }

    #[test]
    fn test_filesystem_source_empty_directory() {
        let fs = MockFileSystem::new();
        let base = PathBuf::from("/packs");
        fs.create_dir_all(&base).unwrap();

        let source = FilesystemSource::new("test", base, &fs);
        let packs = source.list_packs().unwrap();

        assert!(packs.is_empty());
    }

    #[test]
    fn test_filesystem_source_nonexistent_directory() {
        let fs = MockFileSystem::new();
        let base = PathBuf::from("/nonexistent");

        let source = FilesystemSource::new("test", base, &fs);
        let packs = source.list_packs().unwrap();

        assert!(packs.is_empty());
    }
}
