use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Registry index fetched from URL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryIndex {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub metadata: RegistryIndexMetadata,
    pub packs: Vec<PackInfo>,
}

impl RegistryIndex {
    pub fn new(name: &str, description: Option<&str>) -> Self {
        Self {
            api_version: "pmp.io/v1".to_string(),
            kind: "RegistryIndex".to_string(),
            metadata: RegistryIndexMetadata {
                name: name.to_string(),
                description: description.map(|s| s.to_string()),
                generated_at: Some(Utc::now()),
            },
            packs: Vec::new(),
        }
    }

    pub fn with_packs(mut self, packs: Vec<PackInfo>) -> Self {
        self.packs = packs;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryIndexMetadata {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<DateTime<Utc>>,
}

/// Template pack information in the registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub repository: String,
    #[serde(default)]
    pub versions: Vec<PackVersion>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
}

impl PackInfo {
    pub fn new(name: &str, repository: &str) -> Self {
        Self {
            name: name.to_string(),
            description: None,
            repository: repository.to_string(),
            versions: Vec::new(),
            tags: Vec::new(),
            author: None,
            license: None,
        }
    }

    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn with_versions(mut self, versions: Vec<PackVersion>) -> Self {
        self.versions = versions;
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    pub fn with_author(mut self, author: &str) -> Self {
        self.author = Some(author.to_string());
        self
    }

    pub fn with_license(mut self, license: &str) -> Self {
        self.license = Some(license.to_string());
        self
    }

    /// Get the latest version, if any
    pub fn latest_version(&self) -> Option<&PackVersion> {
        self.versions.first()
    }

    /// Check if pack matches a search query
    pub fn matches_query(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();

        self.name.to_lowercase().contains(&query_lower)
            || self
                .description
                .as_ref()
                .map(|d| d.to_lowercase().contains(&query_lower))
                .unwrap_or(false)
            || self.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
    }
}

/// Version information for a template pack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackVersion {
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub released_at: Option<DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changelog: Option<String>,
}

impl PackVersion {
    pub fn new(version: &str) -> Self {
        Self {
            version: version.to_string(),
            tag: None,
            released_at: None,
            changelog: None,
        }
    }

    pub fn with_tag(mut self, tag: &str) -> Self {
        self.tag = Some(tag.to_string());
        self
    }

    pub fn with_released_at(mut self, released_at: DateTime<Utc>) -> Self {
        self.released_at = Some(released_at);
        self
    }

    /// Get git ref for cloning (tag if available, otherwise version)
    pub fn git_ref(&self) -> &str {
        self.tag.as_ref().unwrap_or(&self.version)
    }
}

/// Search result with source information
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub pack: PackInfo,
    pub source_name: String,
    pub installed: bool,
    pub installed_version: Option<String>,
}

impl SearchResult {
    pub fn new(pack: PackInfo, source_name: &str) -> Self {
        Self {
            pack,
            source_name: source_name.to_string(),
            installed: false,
            installed_version: None,
        }
    }

    pub fn with_installed(mut self, version: Option<&str>) -> Self {
        self.installed = true;
        self.installed_version = version.map(|v| v.to_string());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_index_creation() {
        let index = RegistryIndex::new("test", Some("Test registry"));

        assert_eq!(index.api_version, "pmp.io/v1");
        assert_eq!(index.kind, "RegistryIndex");
        assert_eq!(index.metadata.name, "test");
        assert_eq!(index.metadata.description, Some("Test registry".to_string()));
        assert!(index.metadata.generated_at.is_some());
    }

    #[test]
    fn test_pack_info_creation() {
        let pack = PackInfo::new("aws-networking", "https://github.com/pmp/aws-networking")
            .with_description("AWS VPC and networking")
            .with_author("pmp-project")
            .with_license("MIT")
            .with_tags(vec!["aws".to_string(), "networking".to_string()]);

        assert_eq!(pack.name, "aws-networking");
        assert_eq!(pack.description, Some("AWS VPC and networking".to_string()));
        assert_eq!(pack.author, Some("pmp-project".to_string()));
        assert_eq!(pack.tags.len(), 2);
    }

    #[test]
    fn test_pack_info_matches_query() {
        let pack = PackInfo::new("aws-networking", "https://github.com/pmp/aws")
            .with_description("VPC and subnet templates")
            .with_tags(vec!["cloud".to_string(), "infrastructure".to_string()]);

        // Match by name
        assert!(pack.matches_query("aws"));
        assert!(pack.matches_query("AWS")); // Case insensitive

        // Match by description
        assert!(pack.matches_query("vpc"));
        assert!(pack.matches_query("subnet"));

        // Match by tags
        assert!(pack.matches_query("cloud"));
        assert!(pack.matches_query("infra"));

        // No match
        assert!(!pack.matches_query("azure"));
    }

    #[test]
    fn test_pack_version_git_ref() {
        let version_only = PackVersion::new("1.2.0");
        assert_eq!(version_only.git_ref(), "1.2.0");

        let with_tag = PackVersion::new("1.2.0").with_tag("v1.2.0");
        assert_eq!(with_tag.git_ref(), "v1.2.0");
    }

    #[test]
    fn test_pack_info_latest_version() {
        let pack = PackInfo::new("test", "https://example.com")
            .with_versions(vec![
                PackVersion::new("2.0.0"),
                PackVersion::new("1.0.0"),
            ]);

        let latest = pack.latest_version().unwrap();
        assert_eq!(latest.version, "2.0.0");
    }

    #[test]
    fn test_registry_index_serialization() {
        let pack = PackInfo::new("test-pack", "https://github.com/test/pack")
            .with_description("Test pack")
            .with_versions(vec![PackVersion::new("1.0.0").with_tag("v1.0.0")]);

        let index = RegistryIndex::new("test-registry", Some("Test"))
            .with_packs(vec![pack]);

        let json = serde_json::to_string_pretty(&index).unwrap();

        assert!(json.contains("\"apiVersion\": \"pmp.io/v1\""));
        assert!(json.contains("\"kind\": \"RegistryIndex\""));
        assert!(json.contains("\"name\": \"test-pack\""));
        assert!(json.contains("\"version\": \"1.0.0\""));
    }

    #[test]
    fn test_registry_index_deserialization() {
        let json = r#"{
            "apiVersion": "pmp.io/v1",
            "kind": "RegistryIndex",
            "metadata": {
                "name": "test",
                "description": "Test registry"
            },
            "packs": [
                {
                    "name": "test-pack",
                    "repository": "https://github.com/test/pack",
                    "versions": [
                        { "version": "1.0.0", "tag": "v1.0.0" }
                    ],
                    "tags": ["test"]
                }
            ]
        }"#;

        let index: RegistryIndex = serde_json::from_str(json).unwrap();

        assert_eq!(index.metadata.name, "test");
        assert_eq!(index.packs.len(), 1);
        assert_eq!(index.packs[0].name, "test-pack");
        assert_eq!(index.packs[0].versions[0].version, "1.0.0");
    }
}
