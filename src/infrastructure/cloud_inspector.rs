//! PMP Cloud Inspector Export Parser
//!
//! This module provides types and functionality for parsing exports from
//! pmp-cloud-inspector, allowing users to import discovered cloud resources
//! into PMP-managed Terraform/OpenTofu projects.
//!
//! # Export Format
//!
//! The export format follows the pmp-cloud-inspector schema with:
//! - `schema_version`: Version of the export schema (e.g., "1.0.0")
//! - `resources`: Array of discovered cloud resources
//! - `metadata`: Collection statistics and cost summaries
//!
//! # Usage
//!
//! ```bash
//! pmp import infrastructure from-export ./cloud-inventory.json
//! pmp import infrastructure from-export ./cloud-inventory.yaml --filter aws:ec2:*
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported schema versions for pmp-cloud-inspector exports
pub const SUPPORTED_SCHEMA_VERSIONS: &[&str] = &["1.0.0", "1.0", "1"];

/// Minimum required schema version
pub const MINIMUM_SCHEMA_VERSION: (u32, u32, u32) = (1, 0, 0);

/// Root structure of a pmp-cloud-inspector export file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudInspectorExport {
    /// Schema version of the export format
    #[serde(default)]
    pub schema_version: Option<String>,
    /// List of all discovered cloud resources
    pub resources: Vec<CloudInspectorResource>,
    /// Metadata about the resource collection
    pub metadata: CollectionMetadata,
}

/// Result of schema version validation
#[derive(Debug)]
pub enum SchemaVersionStatus {
    /// Version is valid and supported
    Valid,
    /// Version is missing (will warn but continue)
    Missing,
    /// Version is newer than supported (will warn but continue)
    Newer(String),
    /// Version is older than minimum required
    TooOld(String),
    /// Version format is invalid
    Invalid(String),
}

/// Parse a semantic version string into (major, minor, patch)
fn parse_semver(version: &str) -> Option<(u32, u32, u32)> {
    let parts: Vec<&str> = version.split('.').collect();

    let major = parts.first()?.parse().ok()?;
    let minor = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);

    Some((major, minor, patch))
}

/// Validate the schema version of a cloud inspector export
pub fn validate_schema_version(version: Option<&str>) -> SchemaVersionStatus {
    let version = match version {
        None => return SchemaVersionStatus::Missing,
        Some(v) => v.trim(),
    };

    if version.is_empty() {
        return SchemaVersionStatus::Missing;
    }

    let parsed = match parse_semver(version) {
        None => return SchemaVersionStatus::Invalid(version.to_string()),
        Some(v) => v,
    };

    let (min_major, min_minor, min_patch) = MINIMUM_SCHEMA_VERSION;

    // Check if version is too old
    if parsed.0 < min_major
        || (parsed.0 == min_major && parsed.1 < min_minor)
        || (parsed.0 == min_major && parsed.1 == min_minor && parsed.2 < min_patch)
    {
        return SchemaVersionStatus::TooOld(version.to_string());
    }

    // Check if version is in supported list
    if SUPPORTED_SCHEMA_VERSIONS.contains(&version) {
        return SchemaVersionStatus::Valid;
    }

    // Version is newer than what we know about
    SchemaVersionStatus::Newer(version.to_string())
}

/// A discovered cloud resource from pmp-cloud-inspector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudInspectorResource {
    /// Unique identifier for the resource (provider-specific format)
    pub id: String,
    /// Resource type in format 'provider:service:resource'
    #[serde(rename = "type")]
    pub resource_type: String,
    /// Human-readable name of the resource
    pub name: String,
    /// Cloud provider identifier
    pub provider: CloudInspectorProvider,
    /// Account, subscription, or project identifier
    #[serde(default)]
    pub account: Option<String>,
    /// Geographic region or location
    #[serde(default)]
    pub region: Option<String>,
    /// Amazon Resource Name or equivalent identifier
    #[serde(default)]
    pub arn: Option<String>,
    /// Key-value tags or labels
    #[serde(default)]
    pub tags: HashMap<String, String>,
    /// Resource-specific properties
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
    /// Original API response data (if include_raw was enabled)
    #[serde(default)]
    pub raw_data: Option<HashMap<String, serde_json::Value>>,
    /// Relationships to other resources
    #[serde(default)]
    pub relationships: Vec<ResourceRelationship>,
    /// Cost information
    #[serde(default)]
    pub cost: Option<ResourceCost>,
    /// Creation timestamp
    #[serde(default)]
    pub created_at: Option<String>,
    /// Last modification timestamp
    #[serde(default)]
    pub updated_at: Option<String>,
}

/// Cloud provider identifiers supported by pmp-cloud-inspector
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CloudInspectorProvider {
    Aws,
    Azure,
    Gcp,
    Github,
    Gitlab,
    Jfrog,
    Okta,
    Auth0,
    Jira,
    Opsgenie,
}

#[allow(dead_code)]
impl CloudInspectorProvider {
    /// Get the display name for the provider
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Aws => "AWS",
            Self::Azure => "Azure",
            Self::Gcp => "GCP",
            Self::Github => "GitHub",
            Self::Gitlab => "GitLab",
            Self::Jfrog => "JFrog",
            Self::Okta => "Okta",
            Self::Auth0 => "Auth0",
            Self::Jira => "Jira",
            Self::Opsgenie => "Opsgenie",
        }
    }

    /// Get the provider as a string identifier
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Aws => "aws",
            Self::Azure => "azure",
            Self::Gcp => "gcp",
            Self::Github => "github",
            Self::Gitlab => "gitlab",
            Self::Jfrog => "jfrog",
            Self::Okta => "okta",
            Self::Auth0 => "auth0",
            Self::Jira => "jira",
            Self::Opsgenie => "opsgenie",
        }
    }

    /// Get the Terraform provider name
    pub fn terraform_provider(&self) -> &'static str {
        match self {
            Self::Aws => "aws",
            Self::Azure => "azurerm",
            Self::Gcp => "google",
            Self::Github => "github",
            Self::Gitlab => "gitlab",
            Self::Jfrog => "artifactory",
            Self::Okta => "okta",
            Self::Auth0 => "auth0",
            Self::Jira => "jira",
            Self::Opsgenie => "opsgenie",
        }
    }

    /// Check if this provider is infrastructure-related (can be imported to Terraform)
    pub fn is_infrastructure_provider(&self) -> bool {
        matches!(self, Self::Aws | Self::Azure | Self::Gcp)
    }

    /// Check if this provider has Terraform provider support
    pub fn has_terraform_support(&self) -> bool {
        true // All listed providers have Terraform providers
    }
}

impl std::fmt::Display for CloudInspectorProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Type of relationship between resources
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    /// Parent-child containment (e.g., VPC contains Subnets)
    Contains,
    /// Inverse of contains - resource is part of another
    BelongsTo,
    /// Resource is attached or associated
    AttachedTo,
    /// Identity assumption (e.g., Service assumes IAM Role)
    Assumes,
    /// Access permission relationship
    HasAccess,
    /// Generic reference to another resource
    References,
    /// Dependency relationship
    DependsOn,
}

/// A relationship between two resources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRelationship {
    /// Type of relationship
    #[serde(rename = "type")]
    pub relationship_type: RelationType,
    /// ID of the target resource
    pub target_id: String,
    /// Type of the target resource
    pub target_type: String,
    /// Additional relationship properties
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
}

/// Cost information for a resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceCost {
    /// Estimated monthly cost
    #[serde(default)]
    pub monthly_estimate: Option<f64>,
    /// Currency code (ISO 4217)
    #[serde(default = "default_currency")]
    pub currency: String,
    /// Cost breakdown by component
    #[serde(default)]
    pub breakdown: HashMap<String, f64>,
    /// Last cost calculation timestamp
    #[serde(default)]
    pub last_updated: Option<String>,
}

fn default_currency() -> String {
    "USD".to_string()
}

/// Metadata about the resource collection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionMetadata {
    /// Collection creation timestamp
    pub timestamp: String,
    /// Total number of resources
    pub total_count: usize,
    /// Count by resource type
    pub by_type: HashMap<String, usize>,
    /// Count by provider
    pub by_provider: HashMap<String, usize>,
    /// Count by account/subscription/project
    #[serde(default)]
    pub by_account: HashMap<String, usize>,
    /// Count by region
    #[serde(default)]
    pub by_region: HashMap<String, usize>,
    /// Count by type and region (nested)
    #[serde(default)]
    pub by_type_and_region: HashMap<String, HashMap<String, usize>>,
    /// Total cost summary
    #[serde(default)]
    pub total_cost: Option<CostSummary>,
}

/// Aggregated cost summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummary {
    /// Total estimated monthly cost
    #[serde(default)]
    pub total: Option<f64>,
    /// Currency code
    #[serde(default = "default_currency")]
    pub currency: String,
    /// Cost by provider
    #[serde(default)]
    pub by_provider: HashMap<String, f64>,
    /// Cost by region
    #[serde(default)]
    pub by_region: HashMap<String, f64>,
    /// Cost by resource type
    #[serde(default)]
    pub by_type: HashMap<String, f64>,
    /// Cost by tag
    #[serde(default)]
    pub by_tag: HashMap<String, f64>,
}

/// Filter options for importing resources
#[derive(Debug, Clone, Default)]
pub struct ImportFilter {
    /// Filter by provider
    pub providers: Vec<CloudInspectorProvider>,
    /// Filter by resource type pattern (supports wildcards)
    pub type_patterns: Vec<String>,
    /// Filter by region
    pub regions: Vec<String>,
    /// Filter by tags (all must match)
    pub tags: HashMap<String, String>,
    /// Filter by account/subscription/project
    pub accounts: Vec<String>,
    /// Maximum number of resources to import
    pub limit: Option<usize>,
}

impl ImportFilter {
    /// Create an empty filter that matches everything
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by specific providers
    pub fn with_providers(mut self, providers: Vec<CloudInspectorProvider>) -> Self {
        self.providers = providers;
        self
    }

    /// Filter by resource type patterns (e.g., "aws:ec2:*", "github:*")
    pub fn with_type_patterns(mut self, patterns: Vec<String>) -> Self {
        self.type_patterns = patterns;
        self
    }

    /// Filter by regions
    pub fn with_regions(mut self, regions: Vec<String>) -> Self {
        self.regions = regions;
        self
    }

    /// Filter by tags
    pub fn with_tags(mut self, tags: HashMap<String, String>) -> Self {
        self.tags = tags;
        self
    }

    /// Limit number of resources
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Check if a resource matches this filter
    pub fn matches(&self, resource: &CloudInspectorResource) -> bool {
        if !self.matches_provider(resource) {
            return false;
        }

        if !self.matches_type_pattern(resource) {
            return false;
        }

        if !self.matches_region(resource) {
            return false;
        }

        if !self.matches_tags(resource) {
            return false;
        }

        if !self.matches_account(resource) {
            return false;
        }

        true
    }

    fn matches_provider(&self, resource: &CloudInspectorResource) -> bool {
        if self.providers.is_empty() {
            return true;
        }
        self.providers.contains(&resource.provider)
    }

    fn matches_type_pattern(&self, resource: &CloudInspectorResource) -> bool {
        if self.type_patterns.is_empty() {
            return true;
        }

        for pattern in &self.type_patterns {
            if matches_glob_pattern(pattern, &resource.resource_type) {
                return true;
            }
        }
        false
    }

    fn matches_region(&self, resource: &CloudInspectorResource) -> bool {
        if self.regions.is_empty() {
            return true;
        }

        if let Some(region) = &resource.region {
            return self.regions.iter().any(|r| r == region);
        }

        // Resources without region match if no region filter specified
        false
    }

    fn matches_tags(&self, resource: &CloudInspectorResource) -> bool {
        if self.tags.is_empty() {
            return true;
        }

        for (key, value) in &self.tags {
            match resource.tags.get(key) {
                Some(v) if v == value => continue,
                _ => return false,
            }
        }
        true
    }

    fn matches_account(&self, resource: &CloudInspectorResource) -> bool {
        if self.accounts.is_empty() {
            return true;
        }

        if let Some(account) = &resource.account {
            return self.accounts.iter().any(|a| a == account);
        }

        false
    }
}

/// Simple glob pattern matching supporting * wildcard
fn matches_glob_pattern(pattern: &str, text: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if !pattern.contains('*') {
        return pattern == text;
    }

    let parts: Vec<&str> = pattern.split('*').collect();

    if parts.len() == 2 {
        let prefix = parts[0];
        let suffix = parts[1];

        if !prefix.is_empty() && !text.starts_with(prefix) {
            return false;
        }

        if !suffix.is_empty() && !text.ends_with(suffix) {
            return false;
        }

        return true;
    }

    // For more complex patterns, do simple contains check
    let prefix = parts[0];
    let suffix = parts[parts.len() - 1];

    if !prefix.is_empty() && !text.starts_with(prefix) {
        return false;
    }

    if !suffix.is_empty() && !text.ends_with(suffix) {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_version_valid() {
        assert!(matches!(
            validate_schema_version(Some("1.0.0")),
            SchemaVersionStatus::Valid
        ));
        assert!(matches!(
            validate_schema_version(Some("1.0")),
            SchemaVersionStatus::Valid
        ));
        assert!(matches!(
            validate_schema_version(Some("1")),
            SchemaVersionStatus::Valid
        ));
    }

    #[test]
    fn test_schema_version_missing() {
        assert!(matches!(
            validate_schema_version(None),
            SchemaVersionStatus::Missing
        ));
        assert!(matches!(
            validate_schema_version(Some("")),
            SchemaVersionStatus::Missing
        ));
    }

    #[test]
    fn test_schema_version_newer() {
        assert!(matches!(
            validate_schema_version(Some("2.0.0")),
            SchemaVersionStatus::Newer(_)
        ));
        assert!(matches!(
            validate_schema_version(Some("1.1.0")),
            SchemaVersionStatus::Newer(_)
        ));
    }

    #[test]
    fn test_schema_version_too_old() {
        assert!(matches!(
            validate_schema_version(Some("0.9.0")),
            SchemaVersionStatus::TooOld(_)
        ));
        assert!(matches!(
            validate_schema_version(Some("0.1.0")),
            SchemaVersionStatus::TooOld(_)
        ));
    }

    #[test]
    fn test_schema_version_invalid() {
        assert!(matches!(
            validate_schema_version(Some("invalid")),
            SchemaVersionStatus::Invalid(_)
        ));
        assert!(matches!(
            validate_schema_version(Some("v1.0.0")),
            SchemaVersionStatus::Invalid(_)
        ));
    }

    #[test]
    fn test_glob_pattern_matching() {
        assert!(matches_glob_pattern("*", "anything"));
        assert!(matches_glob_pattern("aws:*", "aws:ec2:instance"));
        assert!(matches_glob_pattern("aws:ec2:*", "aws:ec2:instance"));
        assert!(matches_glob_pattern("*:instance", "aws:ec2:instance"));
        assert!(!matches_glob_pattern("azure:*", "aws:ec2:instance"));
        assert!(matches_glob_pattern("aws:ec2:instance", "aws:ec2:instance"));
        assert!(!matches_glob_pattern("aws:ec2:vpc", "aws:ec2:instance"));
    }

    #[test]
    fn test_import_filter_matches_provider() {
        let resource = CloudInspectorResource {
            id: "i-123".to_string(),
            resource_type: "aws:ec2:instance".to_string(),
            name: "test".to_string(),
            provider: CloudInspectorProvider::Aws,
            account: None,
            region: None,
            arn: None,
            tags: HashMap::new(),
            properties: HashMap::new(),
            raw_data: None,
            relationships: vec![],
            cost: None,
            created_at: None,
            updated_at: None,
        };

        let filter = ImportFilter::new();
        assert!(filter.matches(&resource));

        let filter = ImportFilter::new().with_providers(vec![CloudInspectorProvider::Aws]);
        assert!(filter.matches(&resource));

        let filter = ImportFilter::new().with_providers(vec![CloudInspectorProvider::Azure]);
        assert!(!filter.matches(&resource));
    }

    #[test]
    fn test_import_filter_matches_type() {
        let resource = CloudInspectorResource {
            id: "i-123".to_string(),
            resource_type: "aws:ec2:instance".to_string(),
            name: "test".to_string(),
            provider: CloudInspectorProvider::Aws,
            account: None,
            region: None,
            arn: None,
            tags: HashMap::new(),
            properties: HashMap::new(),
            raw_data: None,
            relationships: vec![],
            cost: None,
            created_at: None,
            updated_at: None,
        };

        let filter = ImportFilter::new().with_type_patterns(vec!["aws:ec2:*".to_string()]);
        assert!(filter.matches(&resource));

        let filter = ImportFilter::new().with_type_patterns(vec!["aws:s3:*".to_string()]);
        assert!(!filter.matches(&resource));
    }

    #[test]
    fn test_provider_terraform_names() {
        assert_eq!(CloudInspectorProvider::Aws.terraform_provider(), "aws");
        assert_eq!(CloudInspectorProvider::Azure.terraform_provider(), "azurerm");
        assert_eq!(CloudInspectorProvider::Gcp.terraform_provider(), "google");
        assert_eq!(CloudInspectorProvider::Github.terraform_provider(), "github");
    }
}
