use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use crate::traits::FileSystem;

use super::index::{PackInfo, RegistryIndex};
use super::source::{InstallResult, RegistrySource};

const CACHE_TTL_SECS: u64 = 3600; // 1 hour
const CACHE_DIR: &str = "cache/marketplace";

/// HTTP client trait for testing
pub trait HttpClient: Send + Sync {
    fn get(&self, url: &str) -> Result<String>;
}

/// Real HTTP client using reqwest
pub struct ReqwestClient;

impl HttpClient for ReqwestClient {
    fn get(&self, url: &str) -> Result<String> {
        let response = reqwest::blocking::get(url)
            .with_context(|| format!("Failed to fetch URL: {}", url))?;

        if !response.status().is_success() {
            bail!(
                "HTTP request failed with status {}: {}",
                response.status(),
                url
            );
        }

        response
            .text()
            .with_context(|| format!("Failed to read response body from: {}", url))
    }
}

/// URL-based registry source
pub struct UrlSource<'a, H: HttpClient> {
    name: String,
    url: String,
    http_client: H,
    fs: &'a dyn FileSystem,
    cache_enabled: bool,
}

impl<'a> UrlSource<'a, ReqwestClient> {
    /// Create a new URL source with the default HTTP client
    pub fn new(name: &str, url: &str, fs: &'a dyn FileSystem) -> Self {
        Self {
            name: name.to_string(),
            url: url.to_string(),
            http_client: ReqwestClient,
            fs,
            cache_enabled: true,
        }
    }
}

impl<'a, H: HttpClient> UrlSource<'a, H> {
    /// Create a new URL source with a custom HTTP client (for testing)
    pub fn with_client(name: &str, url: &str, fs: &'a dyn FileSystem, client: H) -> Self {
        Self {
            name: name.to_string(),
            url: url.to_string(),
            http_client: client,
            fs,
            cache_enabled: true,
        }
    }

    /// Disable caching (useful for testing)
    pub fn without_cache(mut self) -> Self {
        self.cache_enabled = false;
        self
    }

    /// Get cache file path for this registry
    fn cache_path(&self) -> Result<PathBuf> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Unable to determine home directory"))?;

        let safe_name = self.name.replace(['/', '\\', ':'], "_");
        Ok(home_dir.join(".pmp").join(CACHE_DIR).join(format!("{}.json", safe_name)))
    }

    /// Check if cache is valid (exists and not expired)
    fn is_cache_valid(&self) -> bool {
        if !self.cache_enabled {
            return false;
        }

        let cache_path = match self.cache_path() {
            Ok(p) => p,
            Err(_) => return false,
        };

        if !self.fs.exists(&cache_path) {
            return false;
        }

        // Check file modification time
        let metadata = match std::fs::metadata(&cache_path) {
            Ok(m) => m,
            Err(_) => return false,
        };

        let modified = match metadata.modified() {
            Ok(t) => t,
            Err(_) => return false,
        };

        let age = SystemTime::now()
            .duration_since(modified)
            .unwrap_or(Duration::from_secs(u64::MAX));

        age.as_secs() < CACHE_TTL_SECS
    }

    /// Load index from cache
    fn load_from_cache(&self) -> Result<RegistryIndex> {
        let cache_path = self.cache_path()?;
        let content = self.fs.read_to_string(&cache_path)?;
        let index: RegistryIndex = serde_json::from_str(&content)?;
        Ok(index)
    }

    /// Save index to cache
    fn save_to_cache(&self, index: &RegistryIndex) -> Result<()> {
        let cache_path = self.cache_path()?;
        let content = serde_json::to_string_pretty(index)?;
        self.fs.write(&cache_path, &content)?;
        Ok(())
    }

    /// Fetch index from URL
    fn fetch_index(&self) -> Result<RegistryIndex> {
        let content = self.http_client.get(&self.url)?;
        let index: RegistryIndex = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse registry index from: {}", self.url))?;
        Ok(index)
    }

    /// Get index (from cache if valid, otherwise fetch)
    fn get_index(&self) -> Result<RegistryIndex> {
        if self.is_cache_valid() {
            if let Ok(index) = self.load_from_cache() {
                return Ok(index);
            }
        }

        let index = self.fetch_index()?;

        // Save to cache (ignore errors)
        let _ = self.save_to_cache(&index);

        Ok(index)
    }
}

impl<'a, H: HttpClient> RegistrySource for UrlSource<'a, H> {
    fn name(&self) -> &str {
        &self.name
    }

    fn list_packs(&self) -> Result<Vec<PackInfo>> {
        let index = self.get_index()?;
        Ok(index.packs)
    }

    fn install(
        &self,
        pack_name: &str,
        version: Option<&str>,
        dest: &Path,
    ) -> Result<InstallResult> {
        let pack = self
            .get_pack_info(pack_name)?
            .ok_or_else(|| anyhow::anyhow!("Pack '{}' not found in registry", pack_name))?;

        // Determine version to install
        let version_info = if let Some(v) = version {
            pack.versions
                .iter()
                .find(|pv| pv.version == v)
                .ok_or_else(|| anyhow::anyhow!("Version '{}' not found for pack '{}'", v, pack_name))?
        } else {
            pack.latest_version()
                .ok_or_else(|| anyhow::anyhow!("No versions available for pack '{}'", pack_name))?
        };

        let git_ref = version_info.git_ref();
        let install_path = dest.join(pack_name);

        // Clone the repository
        clone_repository(&pack.repository, git_ref, &install_path)?;

        Ok(InstallResult {
            pack_name: pack_name.to_string(),
            version: version_info.version.clone(),
            install_path,
        })
    }

    fn is_available(&self) -> bool {
        self.fetch_index().is_ok()
    }
}

/// Clone a git repository to a destination path
fn clone_repository(repo_url: &str, git_ref: &str, dest: &Path) -> Result<()> {
    // Check if git is available
    if !is_git_available() {
        bail!(
            "Git is not installed or not available in PATH.\n\
             Please install git to install template packs from remote registries."
        );
    }

    // Check if destination exists
    if dest.exists() {
        bail!(
            "Destination already exists: {}\n\
             Remove it first or use 'pmp marketplace update' to update.",
            dest.display()
        );
    }

    // Clone with specific branch/tag
    let status = Command::new("git")
        .arg("clone")
        .arg("--branch")
        .arg(git_ref)
        .arg("--depth")
        .arg("1")
        .arg(repo_url)
        .arg(dest)
        .status()
        .context("Failed to execute git clone")?;

    if !status.success() {
        bail!(
            "Failed to clone repository: {}\n\
             Please check the repository URL and git ref: {}",
            repo_url,
            git_ref
        );
    }

    Ok(())
}

/// Check if git command is available
fn is_git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::MockFileSystem;

    struct MockHttpClient {
        response: Option<String>,
        error: Option<String>,
    }

    impl MockHttpClient {
        fn with_response(response: &str) -> Self {
            Self {
                response: Some(response.to_string()),
                error: None,
            }
        }

        fn with_error(error: &str) -> Self {
            Self {
                response: None,
                error: Some(error.to_string()),
            }
        }
    }

    impl HttpClient for MockHttpClient {
        fn get(&self, _url: &str) -> Result<String> {
            if let Some(ref response) = self.response {
                Ok(response.clone())
            } else if let Some(ref error) = self.error {
                Err(anyhow::anyhow!("{}", error))
            } else {
                Err(anyhow::anyhow!("No response configured"))
            }
        }
    }

    fn create_test_index_json() -> String {
        r#"{
            "apiVersion": "pmp.io/v1",
            "kind": "RegistryIndex",
            "metadata": {
                "name": "test-registry"
            },
            "packs": [
                {
                    "name": "test-pack",
                    "description": "Test pack for testing",
                    "repository": "https://github.com/test/pack",
                    "versions": [
                        { "version": "1.0.0", "tag": "v1.0.0" }
                    ],
                    "tags": ["test"]
                }
            ]
        }"#
        .to_string()
    }

    #[test]
    fn test_url_source_list_packs() {
        let fs = MockFileSystem::new();
        let client = MockHttpClient::with_response(&create_test_index_json());

        let source = UrlSource::with_client(
            "test",
            "https://example.com/index.json",
            &fs,
            client,
        )
        .without_cache();

        let packs = source.list_packs().unwrap();

        assert_eq!(packs.len(), 1);
        assert_eq!(packs[0].name, "test-pack");
    }

    #[test]
    fn test_url_source_search() {
        let fs = MockFileSystem::new();
        let client = MockHttpClient::with_response(&create_test_index_json());

        let source = UrlSource::with_client(
            "test",
            "https://example.com/index.json",
            &fs,
            client,
        )
        .without_cache();

        let results = source.search("test").unwrap();
        assert_eq!(results.len(), 1);

        let results = source.search("nonexistent").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_url_source_get_pack_info() {
        let fs = MockFileSystem::new();
        let client = MockHttpClient::with_response(&create_test_index_json());

        let source = UrlSource::with_client(
            "test",
            "https://example.com/index.json",
            &fs,
            client,
        )
        .without_cache();

        let pack = source.get_pack_info("test-pack").unwrap();
        assert!(pack.is_some());
        assert_eq!(pack.unwrap().name, "test-pack");

        let pack = source.get_pack_info("nonexistent").unwrap();
        assert!(pack.is_none());
    }

    #[test]
    fn test_url_source_http_error() {
        let fs = MockFileSystem::new();
        let client = MockHttpClient::with_error("Network error");

        let source = UrlSource::with_client(
            "test",
            "https://example.com/index.json",
            &fs,
            client,
        )
        .without_cache();

        let result = source.list_packs();
        assert!(result.is_err());
    }

    #[test]
    fn test_url_source_invalid_json() {
        let fs = MockFileSystem::new();
        let client = MockHttpClient::with_response("not valid json");

        let source = UrlSource::with_client(
            "test",
            "https://example.com/index.json",
            &fs,
            client,
        )
        .without_cache();

        let result = source.list_packs();
        assert!(result.is_err());
    }

    #[test]
    fn test_url_source_is_available() {
        let fs = MockFileSystem::new();
        let client = MockHttpClient::with_response(&create_test_index_json());

        let source = UrlSource::with_client(
            "test",
            "https://example.com/index.json",
            &fs,
            client,
        )
        .without_cache();

        assert!(source.is_available());
    }

    #[test]
    fn test_url_source_not_available() {
        let fs = MockFileSystem::new();
        let client = MockHttpClient::with_error("Connection refused");

        let source = UrlSource::with_client(
            "test",
            "https://example.com/index.json",
            &fs,
            client,
        )
        .without_cache();

        assert!(!source.is_available());
    }
}
