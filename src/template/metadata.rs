use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

// ============================================================================
// TemplatePack Resource (Kubernetes-style)
// ============================================================================

/// Kubernetes-style TemplatePack resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplatePackResource {
    /// API version (e.g., "pmp.io/v1")
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind of resource (always "TemplatePack")
    pub kind: String,

    /// Metadata about the template pack
    pub metadata: TemplatePackMetadata,

    /// TemplatePack specification (empty)
    #[serde(default)]
    pub spec: TemplatePackSpec,
}

/// TemplatePack metadata (Kubernetes-style)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplatePackMetadata {
    /// Name of the template pack
    pub name: String,

    /// Description of what this template pack contains
    #[serde(default)]
    pub description: Option<String>,
}

/// TemplatePack specification (empty struct)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TemplatePackSpec {}

// ============================================================================
// Template Resource (Kubernetes-style)
// ============================================================================

/// Kubernetes-style Template resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateResource {
    /// API version (e.g., "pmp.io/v1")
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind of resource (always "Template")
    pub kind: String,

    /// Metadata about the template
    pub metadata: TemplateMetadata,

    /// Template specification
    pub spec: TemplateSpec,
}

/// Template metadata (Kubernetes-style)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMetadata {
    /// Name of the template
    pub name: String,

    /// Description of what this template creates
    #[serde(default)]
    pub description: Option<String>,

    /// Labels for categorization and filtering (Kubernetes-style)
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

/// Template specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSpec {
    /// API version of the generated resource (e.g., "pmp.io/v1")
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind of the generated resource (e.g., "KubernetesWorkload", "Infrastructure")
    pub kind: String,

    /// Executor name (e.g., "opentofu", "terraform") (REQUIRED)
    pub executor: String,

    /// Inputs applied to all environments
    #[serde(default)]
    pub inputs: HashMap<String, InputSpec>,

    /// Environment-specific overrides
    #[serde(default)]
    pub environments: HashMap<String, EnvironmentOverrides>,

    /// Plugins configuration (allowed plugins from other templates)
    #[serde(default)]
    pub plugins: Option<PluginsConfig>,

    /// Dependencies on other projects
    /// If set, user must select projects matching these dependencies when creating a project
    #[serde(default)]
    pub dependencies: Vec<TemplateDependency>,
}

/// Plugins configuration in template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginsConfig {
    /// List of allowed plugins from other templates
    #[serde(default)]
    pub allowed: Vec<AllowedPluginConfig>,

    /// List of plugins that are automatically installed during project creation
    #[serde(default)]
    pub installed: Vec<AllowedPluginConfig>,
}

/// Configuration for an allowed plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowedPluginConfig {
    /// Name of the template pack containing the plugin
    pub template_pack_name: String,

    /// Name of the plugin to allow
    pub plugin_name: String,

    /// Optional input overrides/requirements for this plugin
    #[serde(default)]
    pub inputs: HashMap<String, InputSpec>,
}

/// Reference to a template that provides the plugin (used for requires_project_with_template)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginTemplateRef {
    /// API version of the template
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind of the template
    pub kind: String,

    /// Optional remote state configuration
    #[serde(default)]
    pub remote_state: Option<RemoteStateConfig>,
}

/// Configuration for required fields from remote state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredField {
    /// Optional alias for the field name when passed to the module
    /// If not provided, the original field name is used
    #[serde(default)]
    pub alias: Option<String>,
}

/// Remote state configuration for a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteStateConfig {
    /// Map of remote state output names to their configuration
    /// Key: name of the output in the reference project's remote state
    /// Value: configuration for how to use this output
    pub required_fields: HashMap<String, RequiredField>,
}

/// Reference to a project required by a template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateProjectRef {
    /// API version of the required project
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind of the required project
    pub kind: String,

    /// Optional label selector for filtering compatible projects
    /// All labels must match (AND logic)
    #[serde(default)]
    pub label_selector: HashMap<String, String>,

    /// Remote state configuration for accessing the reference project
    #[serde(default)]
    pub remote_state: Option<TemplateRemoteStateConfig>,
}

/// Remote state configuration for a template reference project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateRemoteStateConfig {
    /// Name of the data source in _common.tf (e.g., "postgres_instance")
    /// Will be prefixed with "template_ref_" in the actual data source name
    pub data_source_name: String,
}

/// Dependency on another project (used in templates)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateDependency {
    /// Project reference containing apiVersion, kind, and remote_state config
    pub project: TemplateProjectRef,
}

// ============================================================================
// Plugin Resource (Kubernetes-style)
// ============================================================================

/// Kubernetes-style Plugin resource
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct PluginResource {
    /// API version (e.g., "pmp.io/v1")
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind of resource (always "Plugin")
    pub kind: String,

    /// Metadata about the plugin
    pub metadata: PluginMetadata,

    /// Plugin specification
    pub spec: PluginSpec,
}

/// Plugin metadata (Kubernetes-style)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct PluginMetadata {
    /// Name of the plugin
    pub name: String,

    /// Description of what this plugin provides
    #[serde(default)]
    pub description: Option<String>,
}

/// Plugin specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct PluginSpec {
    /// Role/purpose of this plugin (e.g., "network", "storage")
    pub role: String,

    /// Inputs for this plugin
    #[serde(default)]
    pub inputs: HashMap<String, InputSpec>,

    /// Optional requirement for a reference project with specific template
    /// If set, user must select a project matching this template when adding the plugin
    #[serde(default)]
    pub requires_project_with_template: Option<PluginTemplateRef>,
}

// ============================================================================
// Legacy/Deprecated Structures
// ============================================================================

/// Resource specification in template (DEPRECATED - used for migration only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSpec {
    /// API version of the generated resource (e.g., "pmp.io/v1")
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind of the generated resource (e.g., "Workload", "Infrastructure")
    pub kind: String,

    /// Executor name (e.g., "opentofu", "terraform") (REQUIRED)
    pub executor: String,

    /// Inputs applied to all environments
    #[serde(default)]
    pub inputs: HashMap<String, InputSpec>,

    /// Environment-specific overrides
    #[serde(default)]
    pub environments: HashMap<String, EnvironmentOverrides>,
}

/// Defines the resource that will be generated by the template (simplified)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceDefinition {
    /// API version of the generated resource (e.g., "pmp.io/v1")
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind of the generated resource (e.g., "Workload", "Infrastructure")
    pub kind: String,
}

/// Input specification for a template input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputSpec {
    /// Possible enum values for this input
    #[serde(default)]
    pub enum_values: Option<Vec<String>>,

    /// Default value
    #[serde(default)]
    pub default: Option<Value>,

    /// Description of the input
    #[serde(default)]
    pub description: Option<String>,
}

/// Environment-specific overrides
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentOverrides {
    /// Override inputs for this environment
    #[serde(default)]
    pub overrides: OverridesSpec,
}

/// Overrides specification
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OverridesSpec {
    /// Input overrides
    #[serde(default)]
    pub inputs: HashMap<String, InputSpec>,
}

// ============================================================================
// Project Resource (Kubernetes-style)
// ============================================================================

/// Kubernetes-style Project resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectResource {
    /// API version (e.g., "pmp.io/v1")
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind of resource (always "Project")
    pub kind: String,

    /// Metadata about the project
    pub metadata: ProjectMetadata,

    /// Project specification (optional - only in .pmp.environment.yaml)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub spec: Option<ProjectSpec>,
}

/// Project metadata (Kubernetes-style)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    /// Name of the project
    pub name: String,

    /// Description of this project
    #[serde(default)]
    pub description: Option<String>,

    /// Labels inherited from template or defined by user
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

/// Reference to the project where a plugin is added
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginProjectReference {
    /// API version of the resource kind
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind of the resource
    pub kind: String,

    /// Name of the project
    pub name: String,

    /// Environment name
    pub environment: String,
}

/// Reference to a project required by a template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateReferenceProject {
    /// API version of the resource kind
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind of the resource
    pub kind: String,

    /// Name of the reference project
    pub name: String,

    /// Environment name of the reference project
    pub environment: String,

    /// Data source name for this reference in _common.tf
    pub data_source_name: String,
}

/// Information about a plugin that has been added to a project environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddedPlugin {
    /// Name of the template pack containing the plugin
    pub template_pack_name: String,

    /// Name of the plugin
    pub name: String,

    /// Reference to the project where this plugin is added
    pub project: PluginProjectReference,

    /// Reference to the project that this plugin connects to (for plugins with requires_project_with_template)
    /// This is used to generate terraform_remote_state data sources in the main project's _common.tf
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference_project: Option<PluginProjectReference>,

    /// Inputs used when adding the plugin
    pub inputs: HashMap<String, Value>,

    /// List of files generated by the plugin
    pub files: Vec<String>,

    /// Plugin specification (for generating remote state parameters)
    /// Stored when the plugin is added to avoid re-loading during _common.tf generation
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_spec: Option<PluginSpec>,
}

/// Plugins configuration for a project environment
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectPlugins {
    /// Plugins that have been added to this environment
    #[serde(default)]
    pub added: Vec<AddedPlugin>,
}

/// Reference to the template used to generate this project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateReference {
    /// Name of the template pack
    pub template_pack_name: String,

    /// Name of the template within the pack
    pub name: String,
}

/// Reference to the environment for this project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentReference {
    /// Name of the environment
    pub name: String,
}

/// Project specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSpec {
    /// Resource definition that this project implements (REQUIRED)
    pub resource: ResourceDefinition,

    /// Executor configuration (executor/provider)
    pub executor: ExecutorProjectConfig,

    /// User inputs collected during project creation
    pub inputs: HashMap<String, Value>,

    /// Optional: Custom fields from template
    #[serde(default)]
    pub custom: Option<HashMap<String, Value>>,

    /// Plugins that have been added to this environment
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugins: Option<ProjectPlugins>,

    /// Reference to the template used to generate this project
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<TemplateReference>,

    /// Reference to the environment for this project
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<EnvironmentReference>,

    /// Reference projects required by the template
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub template_reference_projects: Vec<TemplateReferenceProject>,
}

// ============================================================================
// ProjectEnvironment Resource (Kubernetes-style)
// ============================================================================

/// Kubernetes-style ProjectEnvironment resource
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ProjectEnvironmentResource {
    /// API version (e.g., "pmp.io/v1")
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind of resource (always "ProjectEnvironment")
    pub kind: String,

    /// Metadata about the project environment
    pub metadata: ProjectEnvironmentMetadata,

    /// Project environment specification
    pub spec: ProjectSpec,
}

/// ProjectEnvironment metadata (Kubernetes-style)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ProjectEnvironmentMetadata {
    /// Name of the environment
    pub name: String,

    /// Name of the project this environment belongs to
    pub project_name: String,

    /// Description of this environment
    #[serde(default)]
    pub description: Option<String>,
}

/// Dynamic project environment metadata (new format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicProjectEnvironmentMetadata {
    /// Name of the project
    pub name: String,

    /// Name of the environment
    pub environment_name: String,

    /// Description of this environment
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Dynamic project environment resource (new format with dynamic kind)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicProjectEnvironmentResource {
    /// API version from template (e.g., "pmp.io/v1")
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind from template (e.g., "KubernetesWorkload")
    pub kind: String,

    /// Metadata about the project environment
    pub metadata: DynamicProjectEnvironmentMetadata,

    /// Project environment specification
    pub spec: ProjectSpec,
}

/// Executor configuration in project spec
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorProjectConfig {
    /// Executor name (e.g., "opentofu", "terraform")
    pub name: String,
}


// ============================================================================
// Infrastructure Resource (Kubernetes-style)
// ============================================================================

/// Kubernetes-style Infrastructure resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfrastructureResource {
    /// API version (e.g., "pmp.io/v1")
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind of resource (always "Infrastructure")
    pub kind: String,

    /// Metadata about the infrastructure
    pub metadata: InfrastructureMetadata,

    /// Infrastructure specification
    pub spec: InfrastructureSpec,
}

/// Infrastructure metadata (Kubernetes-style)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfrastructureMetadata {
    /// Name of the infrastructure
    pub name: String,

    /// Description of this infrastructure
    #[serde(default)]
    pub description: Option<String>,
}

/// Infrastructure specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfrastructureSpec {
    /// Allowed resource kinds in this infrastructure
    #[serde(default)]
    pub resource_kinds: Vec<ResourceKindFilter>,

    /// Available environments for projects in this infrastructure
    pub environments: HashMap<String, Environment>,

    /// Optional: Hooks configuration for all projects in this infrastructure
    #[serde(default)]
    pub hooks: Option<HooksConfig>,

    /// Optional: Executor configuration for all projects in this infrastructure
    #[serde(default)]
    pub executor: Option<ExecutorCollectionConfig>,
}

/// Executor configuration at the infrastructure level
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorCollectionConfig {
    /// Executor name (e.g., "opentofu", "terraform")
    pub name: String,

    /// Executor-specific configuration (e.g., backend configuration)
    #[serde(default)]
    pub config: HashMap<String, Value>,
}

/// Hooks configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HooksConfig {
    /// Commands to run before preview
    #[serde(default)]
    pub pre_preview: Vec<String>,

    /// Commands to run after preview
    #[serde(default)]
    pub post_preview: Vec<String>,

    /// Commands to run before apply
    #[serde(default)]
    pub pre_apply: Vec<String>,

    /// Commands to run after apply
    #[serde(default)]
    pub post_apply: Vec<String>,

    /// Commands to run before destroy
    #[serde(default)]
    pub pre_destroy: Vec<String>,

    /// Commands to run after destroy
    #[serde(default)]
    pub post_destroy: Vec<String>,

    /// Commands to run before refresh
    #[serde(default)]
    pub pre_refresh: Vec<String>,

    /// Commands to run after refresh
    #[serde(default)]
    pub post_refresh: Vec<String>,
}

/// Input override configuration for infrastructure-level input customization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputOverride {
    /// The value to use for this input
    pub value: Value,

    /// If true, show this as the default value and allow user to override.
    /// If false, use this value directly without prompting the user.
    #[serde(default = "default_show_as_default")]
    pub show_as_default: bool,
}

fn default_show_as_default() -> bool {
    true
}

/// Template defaults configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TemplateDefaults {
    /// Input overrides for this template
    #[serde(default)]
    pub inputs: HashMap<String, InputOverride>,
}

/// Template configuration in resource kind filter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateConfig {
    /// Name of the template pack this template belongs to
    pub template_pack_name: String,

    /// Whether this template is allowed to be used (default: true)
    #[serde(default = "default_allowed")]
    pub allowed: bool,

    /// Default input overrides
    #[serde(default)]
    pub defaults: TemplateDefaults,
}

fn default_allowed() -> bool {
    true
}

/// Filter for allowed resource kinds
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceKindFilter {
    /// API version (e.g., "pmp.io/v1")
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind (e.g., "Workload", "Infrastructure")
    pub kind: String,

    /// Optional: Template-specific configurations
    /// Key: template name, Value: template configuration
    #[serde(default)]
    pub templates: Option<HashMap<String, TemplateConfig>>,
}

impl ResourceKindFilter {
    /// Check if this filter matches a resource definition
    #[allow(dead_code)]
    pub fn matches(&self, resource: &ResourceDefinition) -> bool {
        self.api_version == resource.api_version && self.kind == resource.kind
    }

    /// Check if this filter matches a resource spec (DEPRECATED)
    #[allow(dead_code)]
    pub fn matches_spec(&self, resource: &ResourceSpec) -> bool {
        self.api_version == resource.api_version && self.kind == resource.kind
    }

    /// Check if this filter matches a template spec
    pub fn matches_template(&self, template: &TemplateSpec) -> bool {
        self.api_version == template.api_version && self.kind == template.kind
    }

    /// Check if a specific template is allowed and get its configuration
    /// Returns:
    /// - Some(Some(config)) if template is configured and allowed
    /// - Some(None) if template is configured but not allowed
    /// - None if no template-specific configuration exists (allow by default)
    pub fn get_template_config(&self, template_name: &str, template_pack_name: &str) -> Option<Option<&TemplateConfig>> {
        if let Some(ref templates) = self.templates {
            if let Some(config) = templates.get(template_name) {
                // Check if template pack name matches
                if config.template_pack_name == template_pack_name {
                    // Return config only if allowed
                    if config.allowed {
                        return Some(Some(config));
                    } else {
                        return Some(None); // Explicitly not allowed
                    }
                }
                // Template pack name doesn't match, treat as not configured
            }
        }
        None // No template-specific configuration
    }
}

impl ResourceSpec {
    /// Convert to a simplified ResourceDefinition
    #[allow(dead_code)]
    pub fn to_definition(&self) -> ResourceDefinition {
        ResourceDefinition {
            api_version: self.api_version.clone(),
            kind: self.kind.clone(),
        }
    }
}

/// Environment definition in Infrastructure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Environment {
    /// Display name
    pub name: String,

    /// Optional: Description
    #[serde(default)]
    pub description: Option<String>,
}

/// Reference to a project in the infrastructure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectReference {
    /// Name of the project
    pub name: String,

    /// Kind of the project (e.g., "KubernetesWorkload", "Infrastructure")
    pub kind: String,

    /// Path to the project relative to the infrastructure root
    pub path: String,

    /// Labels for categorization and filtering
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

// ============================================================================
// Implementation
// ============================================================================

impl TemplatePackResource {
    /// Load template pack resource from a .pmp.template-pack.yaml file
    pub fn from_file(fs: &dyn crate::traits::FileSystem, path: &std::path::Path) -> anyhow::Result<Self> {
        let content = fs.read_to_string(path)?;
        let resource: TemplatePackResource = serde_yaml::from_str(&content)?;

        // Validate kind
        if resource.kind != "TemplatePack" {
            anyhow::bail!("Expected kind 'TemplatePack', got '{}'", resource.kind);
        }

        Ok(resource)
    }

    /// Get the path to the templates directory
    #[allow(dead_code)]
    pub fn templates_dir(&self, base_path: &std::path::Path) -> std::path::PathBuf {
        base_path.join("templates")
    }
}

impl TemplateResource {
    /// Load template resource from a .pmp.template.yaml file
    pub fn from_file(fs: &dyn crate::traits::FileSystem, path: &std::path::Path) -> anyhow::Result<Self> {
        let content = fs.read_to_string(path)?;
        let resource: TemplateResource = serde_yaml::from_str(&content)?;

        // Validate kind
        if resource.kind != "Template" {
            anyhow::bail!("Expected kind 'Template', got '{}'", resource.kind);
        }

        // Validate resource kind contains only alphanumeric characters
        let resource_kind = &resource.spec.kind;
        if !resource_kind.chars().all(|c| c.is_alphanumeric()) {
            anyhow::bail!(
                "Resource kind '{}' must contain only alphanumeric characters",
                resource_kind
            );
        }

        Ok(resource)
    }
}

impl PluginResource {
    /// Load plugin resource from a .pmp.plugin.yaml file
    #[allow(dead_code)]
    pub fn from_file(fs: &dyn crate::traits::FileSystem, path: &std::path::Path) -> anyhow::Result<Self> {
        let content = fs.read_to_string(path)?;
        let resource: PluginResource = serde_yaml::from_str(&content)?;

        // Validate kind
        if resource.kind != "Plugin" {
            anyhow::bail!("Expected kind 'Plugin', got '{}'", resource.kind);
        }

        Ok(resource)
    }
}

impl ProjectResource {
    /// Load project resource from a .pmp.project.yaml file
    pub fn from_file(fs: &dyn crate::traits::FileSystem, path: &std::path::Path) -> anyhow::Result<Self> {
        let content = fs.read_to_string(path)?;
        let resource: ProjectResource = serde_yaml::from_str(&content)?;

        // Validate kind
        if resource.kind != "Project" {
            anyhow::bail!("Expected kind 'Project', got '{}'", resource.kind);
        }

        Ok(resource)
    }
}

impl ProjectEnvironmentResource {
    /// Load project environment resource from a .pmp.environment.yaml file
    #[allow(dead_code)]
    pub fn from_file(fs: &dyn crate::traits::FileSystem, path: &std::path::Path) -> anyhow::Result<Self> {
        let content = fs.read_to_string(path)?;
        let resource: ProjectEnvironmentResource = serde_yaml::from_str(&content)?;

        // Validate kind
        if resource.kind != "ProjectEnvironment" {
            anyhow::bail!("Expected kind 'ProjectEnvironment', got '{}'", resource.kind);
        }

        Ok(resource)
    }

    /// Get the executor configuration
    #[allow(dead_code)]
    pub fn get_executor_config(&self) -> &ExecutorProjectConfig {
        &self.spec.executor
    }
}

impl DynamicProjectEnvironmentResource {
    /// Load dynamic project environment resource from a .pmp.environment.yaml file
    /// This supports any apiVersion/kind combination (no validation)
    pub fn from_file(fs: &dyn crate::traits::FileSystem, path: &std::path::Path) -> anyhow::Result<Self> {
        let content = fs.read_to_string(path)?;
        let resource: DynamicProjectEnvironmentResource = serde_yaml::from_str(&content)?;
        Ok(resource)
    }

    /// Get the executor configuration
    pub fn get_executor_config(&self) -> &ExecutorProjectConfig {
        &self.spec.executor
    }
}

impl InfrastructureResource {
    /// Load infrastructure resource from a .pmp.yaml file
    pub fn from_file(fs: &dyn crate::traits::FileSystem, path: &std::path::Path) -> anyhow::Result<Self> {
        let content = fs.read_to_string(path)?;
        let resource: InfrastructureResource = serde_yaml::from_str(&content)?;

        // Validate kind
        if resource.kind != "Infrastructure" {
            anyhow::bail!(
                "Expected kind 'Infrastructure', got '{}'",
                resource.kind
            );
        }

        // Validate environment names
        for env_key in resource.spec.environments.keys() {
            if !Self::is_valid_environment_name(env_key) {
                anyhow::bail!(
                    "Invalid environment name '{}'. Environment names must be lowercase alphanumeric with underscores, and cannot start with a number. \
                     Note: As of this version, hyphens are no longer supported. Please rename '{}' to '{}' in your .pmp.infrastructure.yaml file.",
                    env_key,
                    env_key,
                    env_key.replace('-', "_")
                );
            }
        }

        Ok(resource)
    }

    /// Validate environment name format
    pub fn is_valid_environment_name(name: &str) -> bool {
        !name.is_empty()
            && !name.chars().next().map_or(false, |c| c.is_ascii_digit())
            && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    }

    /// Save the infrastructure to a .pmp.yaml file
    pub fn save(&self, fs: &dyn crate::traits::FileSystem, path: &std::path::Path) -> anyhow::Result<()> {
        let content = serde_yaml::to_string(self)?;
        fs.write(path, &content)?;
        Ok(())
    }

    /// Get the hooks configuration, or return empty hooks
    pub fn get_hooks(&self) -> HooksConfig {
        self.spec.hooks.clone().unwrap_or_default()
    }
}
