use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Supported infrastructure providers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
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
impl Provider {
    /// Get the display name for the provider
    pub fn display_name(&self) -> &'static str {
        match self {
            Provider::Aws => "AWS",
            Provider::Azure => "Azure",
            Provider::Gcp => "GCP",
            Provider::Github => "GitHub",
            Provider::Gitlab => "GitLab",
            Provider::Jfrog => "JFrog",
            Provider::Okta => "Okta",
            Provider::Auth0 => "Auth0",
            Provider::Jira => "Jira",
            Provider::Opsgenie => "Opsgenie",
        }
    }

    /// Get the provider identifier string
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::Aws => "aws",
            Provider::Azure => "azure",
            Provider::Gcp => "gcp",
            Provider::Github => "github",
            Provider::Gitlab => "gitlab",
            Provider::Jfrog => "jfrog",
            Provider::Okta => "okta",
            Provider::Auth0 => "auth0",
            Provider::Jira => "jira",
            Provider::Opsgenie => "opsgenie",
        }
    }

    /// Get the Terraform provider name
    pub fn terraform_provider(&self) -> &'static str {
        match self {
            Provider::Aws => "aws",
            Provider::Azure => "azurerm",
            Provider::Gcp => "google",
            Provider::Github => "github",
            Provider::Gitlab => "gitlab",
            Provider::Jfrog => "artifactory",
            Provider::Okta => "okta",
            Provider::Auth0 => "auth0",
            Provider::Jira => "jira",
            Provider::Opsgenie => "opsgenie",
        }
    }

    /// Check if this is an infrastructure provider (AWS, Azure, GCP)
    pub fn is_infrastructure_provider(&self) -> bool {
        matches!(self, Provider::Aws | Provider::Azure | Provider::Gcp)
    }

    /// Parse provider from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "aws" => Some(Provider::Aws),
            "azure" | "azurerm" => Some(Provider::Azure),
            "gcp" | "google" => Some(Provider::Gcp),
            "github" => Some(Provider::Github),
            "gitlab" => Some(Provider::Gitlab),
            "jfrog" | "artifactory" => Some(Provider::Jfrog),
            "okta" => Some(Provider::Okta),
            "auth0" => Some(Provider::Auth0),
            "jira" => Some(Provider::Jira),
            "opsgenie" => Some(Provider::Opsgenie),
            _ => None,
        }
    }

    /// Get all supported providers
    pub fn all() -> Vec<Provider> {
        vec![
            Provider::Aws,
            Provider::Azure,
            Provider::Gcp,
            Provider::Github,
            Provider::Gitlab,
            Provider::Jfrog,
            Provider::Okta,
            Provider::Auth0,
            Provider::Jira,
            Provider::Opsgenie,
        ]
    }

    /// Get infrastructure providers only (AWS, Azure, GCP)
    pub fn infrastructure_providers() -> Vec<Provider> {
        vec![Provider::Aws, Provider::Azure, Provider::Gcp]
    }
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Type of relationship between resources
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyType {
    /// Resource is contained within parent (e.g., subnet in VPC)
    Parent,
    /// Resource references another resource (e.g., instance uses security group)
    Reference,
    /// Loose association (e.g., tags, policies)
    Association,
}

/// A dependency relationship to another resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDependency {
    /// The resource type (e.g., aws_vpc)
    pub resource_type: String,
    /// The resource ID (e.g., vpc-12345)
    pub resource_id: String,
    /// Type of dependency relationship
    pub relationship: DependencyType,
    /// Optional description of the relationship
    pub description: Option<String>,
}

/// A discovered infrastructure resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredResource {
    /// The provider (AWS, Azure, GCP)
    pub provider: Provider,
    /// The Terraform/OpenTofu resource type (e.g., aws_vpc)
    pub resource_type: String,
    /// The provider-specific resource ID (e.g., vpc-12345)
    pub resource_id: String,
    /// Human-readable name (from Name tag or similar)
    pub name: Option<String>,
    /// Region where the resource exists
    pub region: Option<String>,
    /// Resource tags/labels
    pub tags: HashMap<String, String>,
    /// Additional resource attributes
    pub attributes: HashMap<String, serde_json::Value>,
    /// Dependencies on other resources
    pub dependencies: Vec<ResourceDependency>,
}

#[allow(dead_code)]
impl DiscoveredResource {
    /// Create a new discovered resource
    pub fn new(
        provider: Provider,
        resource_type: String,
        resource_id: String,
    ) -> Self {
        Self {
            provider,
            resource_type,
            resource_id,
            name: None,
            region: None,
            tags: HashMap::new(),
            attributes: HashMap::new(),
            dependencies: Vec::new(),
        }
    }

    /// Set the resource name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the region
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = Some(region.into());
        self
    }

    /// Add a tag
    pub fn with_tag(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Self {
        self.tags.insert(key.into(), value.into());
        self
    }

    /// Add tags from a HashMap
    pub fn with_tags(mut self, tags: HashMap<String, String>) -> Self {
        self.tags.extend(tags);
        self
    }

    /// Add an attribute
    pub fn with_attribute(
        mut self,
        key: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        self.attributes.insert(key.into(), value);
        self
    }

    /// Add a dependency
    pub fn with_dependency(mut self, dependency: ResourceDependency) -> Self {
        self.dependencies.push(dependency);
        self
    }

    /// Generate a suggested Terraform resource name
    pub fn suggested_tf_name(&self) -> String {
        if let Some(name) = &self.name {
            sanitize_tf_name(name)
        } else {
            sanitize_tf_name(&self.resource_id)
        }
    }

    /// Get a display string for the resource
    pub fn display_string(&self) -> String {
        if let Some(name) = &self.name {
            format!(
                "{} ({}) - {}",
                self.resource_type, self.resource_id, name
            )
        } else {
            format!("{} ({})", self.resource_type, self.resource_id)
        }
    }
}

/// Filter for resource discovery
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct DiscoveryFilter {
    /// Filter by resource types (empty = all supported types)
    pub resource_types: Vec<String>,
    /// Filter by tags (all must match)
    pub tags: HashMap<String, String>,
    /// Filter by regions (empty = all regions)
    pub regions: Vec<String>,
    /// Filter by name pattern (glob-style)
    pub name_pattern: Option<String>,
    /// Maximum number of resources to return
    pub limit: Option<usize>,
}

#[allow(dead_code)]
impl DiscoveryFilter {
    /// Create an empty filter (matches everything)
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by specific resource types
    pub fn with_resource_types(mut self, types: Vec<String>) -> Self {
        self.resource_types = types;
        self
    }

    /// Filter by tags
    pub fn with_tags(mut self, tags: HashMap<String, String>) -> Self {
        self.tags = tags;
        self
    }

    /// Filter by regions
    pub fn with_regions(mut self, regions: Vec<String>) -> Self {
        self.regions = regions;
        self
    }

    /// Filter by name pattern
    pub fn with_name_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.name_pattern = Some(pattern.into());
        self
    }

    /// Limit number of results
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

/// Trait for infrastructure resource discovery
///
/// Note: This trait uses `async fn` in trait which is not dyn-compatible.
/// For dynamic dispatch, use concrete provider types directly or an enum wrapper.
#[allow(dead_code)]
pub trait InfrastructureDiscovery: Send + Sync {
    /// Get the provider this discovery implementation supports
    fn provider(&self) -> Provider;

    /// Get the list of supported resource types
    fn supported_resource_types(&self) -> Vec<&'static str>;
}

/// Where to import resources
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImportDestination {
    /// Import into an existing PMP project
    ExistingProject {
        project_name: String,
        environment: String,
    },
    /// Create a new project for the imported resources
    NewProject {
        project_name: String,
        environment: String,
    },
}

impl ImportDestination {
    /// Get the project name
    pub fn project_name(&self) -> &str {
        match self {
            ImportDestination::ExistingProject { project_name, .. } => {
                project_name
            }
            ImportDestination::NewProject { project_name, .. } => project_name,
        }
    }

    /// Get the environment
    pub fn environment(&self) -> &str {
        match self {
            ImportDestination::ExistingProject { environment, .. } => {
                environment
            }
            ImportDestination::NewProject { environment, .. } => environment,
        }
    }

    /// Check if this is a new project
    pub fn is_new_project(&self) -> bool {
        matches!(self, ImportDestination::NewProject { .. })
    }
}

/// Status of an import operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportStatus {
    /// Pending execution
    Pending,
    /// Currently being imported
    InProgress,
    /// Successfully imported
    Succeeded,
    /// Import failed
    Failed,
    /// Skipped (e.g., already exists in state)
    Skipped,
}

/// Result of importing a single resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceImportResult {
    /// The resource that was imported
    pub resource: DiscoveredResource,
    /// Import status
    pub status: ImportStatus,
    /// Error message if failed
    pub error: Option<String>,
    /// Generated Terraform resource name
    pub tf_name: Option<String>,
}

/// Sanitize a string to be a valid Terraform resource name
fn sanitize_tf_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect();

    let sanitized = sanitized.trim_matches('_').to_string();

    if sanitized.is_empty() {
        return "resource".to_string();
    }

    if sanitized.chars().next().unwrap().is_ascii_digit() {
        format!("r_{}", sanitized)
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_from_str() {
        assert_eq!(Provider::from_str("aws"), Some(Provider::Aws));
        assert_eq!(Provider::from_str("AWS"), Some(Provider::Aws));
        assert_eq!(Provider::from_str("azure"), Some(Provider::Azure));
        assert_eq!(Provider::from_str("azurerm"), Some(Provider::Azure));
        assert_eq!(Provider::from_str("gcp"), Some(Provider::Gcp));
        assert_eq!(Provider::from_str("google"), Some(Provider::Gcp));
        assert_eq!(Provider::from_str("invalid"), None);
    }

    #[test]
    fn test_discovered_resource_builder() {
        let resource = DiscoveredResource::new(
            Provider::Aws,
            "aws_vpc".to_string(),
            "vpc-12345".to_string(),
        )
        .with_name("main-vpc")
        .with_region("us-east-1")
        .with_tag("Environment", "production");

        assert_eq!(resource.provider, Provider::Aws);
        assert_eq!(resource.resource_type, "aws_vpc");
        assert_eq!(resource.resource_id, "vpc-12345");
        assert_eq!(resource.name, Some("main-vpc".to_string()));
        assert_eq!(resource.region, Some("us-east-1".to_string()));
        assert_eq!(
            resource.tags.get("Environment"),
            Some(&"production".to_string())
        );
    }

    #[test]
    fn test_sanitize_tf_name() {
        assert_eq!(sanitize_tf_name("my-vpc"), "my_vpc");
        assert_eq!(sanitize_tf_name("My VPC"), "my_vpc");
        assert_eq!(sanitize_tf_name("123-vpc"), "r_123_vpc");
        assert_eq!(sanitize_tf_name("___test___"), "test");
        assert_eq!(sanitize_tf_name("---"), "resource");
    }

    #[test]
    fn test_discovery_filter_builder() {
        let filter = DiscoveryFilter::new()
            .with_resource_types(vec!["aws_vpc".to_string()])
            .with_regions(vec!["us-east-1".to_string()])
            .with_limit(100);

        assert_eq!(filter.resource_types, vec!["aws_vpc"]);
        assert_eq!(filter.regions, vec!["us-east-1"]);
        assert_eq!(filter.limit, Some(100));
    }

    #[test]
    fn test_import_destination() {
        let existing = ImportDestination::ExistingProject {
            project_name: "my-project".to_string(),
            environment: "production".to_string(),
        };
        assert_eq!(existing.project_name(), "my-project");
        assert_eq!(existing.environment(), "production");
        assert!(!existing.is_new_project());

        let new_proj = ImportDestination::NewProject {
            project_name: "new-project".to_string(),
            environment: "staging".to_string(),
        };
        assert!(new_proj.is_new_project());
    }
}
