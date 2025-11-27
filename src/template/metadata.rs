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

    /// Inputs applied to all environments (supports both array and object format)
    #[serde(default, deserialize_with = "deserialize_inputs")]
    pub inputs: Vec<InputDefinition>,

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

    /// Order for input collection (default: 0)
    /// Lower values are collected first. When equal with plugins, template has precedence.
    #[serde(default)]
    pub order: i32,

    /// Projects to create as part of this project group (for ProjectGroup templates)
    /// These projects will be created when a project from this template is created
    /// and will be added as dependencies to the environment
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub projects: Vec<ProjectGroupProject>,

    /// Hooks to run before and after commands
    /// These hooks will be added to the generated environment file
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<HooksConfig>,
}

/// Custom deserializer for inputs that supports both HashMap and Vec formats
fn deserialize_inputs<'de, D>(deserializer: D) -> Result<Vec<InputDefinition>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{MapAccess, SeqAccess, Visitor};
    use std::fmt;

    struct InputsVisitor;

    impl<'de> Visitor<'de> for InputsVisitor {
        type Value = Vec<InputDefinition>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a sequence of input definitions or a map of input names to specs")
        }

        // Handle array format (new)
        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut inputs = Vec::new();
            while let Some(input_def) = seq.next_element::<InputDefinition>()? {
                inputs.push(input_def);
            }
            Ok(inputs)
        }

        // Handle object format (legacy)
        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
        {
            let mut inputs = Vec::new();
            while let Some((name, input_spec)) = map.next_entry::<String, InputSpec>()? {
                inputs.push(InputDefinition {
                    name,
                    input_type: input_spec.input_type,
                    enum_values: input_spec.enum_values,
                    default: input_spec.default,
                    description: input_spec.description,
                    validation: input_spec.validation,
                });
            }
            Ok(inputs)
        }
    }

    deserializer.deserialize_any(InputsVisitor)
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

    /// Optional input overrides/requirements for this plugin (supports both array and object format)
    #[serde(default, deserialize_with = "deserialize_inputs")]
    pub inputs: Vec<InputDefinition>,

    /// Order for input collection (default: 0)
    /// Lower values are collected first. When equal, maintains YAML order.
    #[serde(default)]
    pub order: i32,

    /// Raw module inputs that will be passed as-is (unquoted) to the module in _common.tf
    /// Key: parameter name, Value: raw HCL expression (e.g., "var.some_value", "local.computed")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_module_inputs: Option<HashMap<String, String>>,
}

/// Reference to a template that provides the plugin (used for requires_project_with_template)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginTemplateRef {
    /// API version of the template
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind of the template
    pub kind: String,

    /// Optional label selectors for filtering projects
    #[serde(default)]
    pub label_selector: Option<HashMap<String, String>>,

    /// Optional description to show to user when selecting reference project
    /// If provided, this is shown instead of kind and label selectors
    #[serde(default)]
    pub description: Option<String>,

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

    /// Optional description to show to user when selecting reference project
    /// If provided, this is shown instead of kind and label selectors
    #[serde(default)]
    pub description: Option<String>,

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

    /// Inputs for this plugin (supports both array and object format)
    #[serde(default, deserialize_with = "deserialize_inputs")]
    pub inputs: Vec<InputDefinition>,

    /// Optional requirement for a reference project with specific template
    /// If set, user must select a project matching this template when adding the plugin
    #[serde(default)]
    pub requires_project_with_template: Option<PluginTemplateRef>,
}

// ============================================================================
// InfrastructureTemplate Resource (Kubernetes-style)
// ============================================================================

/// Kubernetes-style InfrastructureTemplate resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfrastructureTemplateResource {
    /// API version (e.g., "pmp.io/v1")
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Kind of resource (always "InfrastructureTemplate")
    pub kind: String,

    /// Metadata about the infrastructure template
    pub metadata: InfrastructureTemplateMetadata,

    /// InfrastructureTemplate specification
    pub spec: InfrastructureTemplateSpec,
}

/// InfrastructureTemplate metadata (Kubernetes-style)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfrastructureTemplateMetadata {
    /// Name of the infrastructure template
    pub name: String,

    /// Description of what this infrastructure template creates
    #[serde(default)]
    pub description: Option<String>,
}

/// InfrastructureTemplate specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfrastructureTemplateSpec {
    /// Inputs for this infrastructure template (supports both array and object format)
    #[serde(default, deserialize_with = "deserialize_inputs")]
    pub inputs: Vec<InputDefinition>,
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

    /// Inputs applied to all environments (supports both array and object format)
    #[serde(default, deserialize_with = "deserialize_inputs")]
    pub inputs: Vec<InputDefinition>,

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

/// Input type specification
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum InputType {
    /// String input (default)
    String,
    /// Boolean (yes/no) input - implemented as select with Yes/No options
    Boolean,
    /// Number input with optional constraints
    Number {
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<f64>,
        /// If true, only allows integer values (default: false)
        #[serde(default, skip_serializing_if = "is_false")]
        integer: bool,
    },
    /// Select input with enum options (single selection)
    Select { options: Vec<EnumOption> },
    /// Multi-select input allowing multiple selections
    MultiSelect {
        options: Vec<EnumOption>,
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<usize>,
    },
    /// Password input (hidden text)
    Password,
    /// Project selector - allows selecting a project based on filters
    #[serde(rename = "project_select")]
    ProjectSelect {
        #[serde(skip_serializing_if = "Option::is_none")]
        api_version: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        kind: Option<String>,
        #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
        labels: std::collections::HashMap<String, String>,
    },
    /// Multi-project selector - allows selecting multiple projects
    #[serde(rename = "multiproject_select")]
    MultiProjectSelect {
        #[serde(skip_serializing_if = "Option::is_none")]
        api_version: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        kind: Option<String>,
        #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
        labels: std::collections::HashMap<String, String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<usize>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<usize>,
    },
    /// File or directory path input
    Path {
        /// If true, only accept existing paths
        #[serde(default, skip_serializing_if = "is_false")]
        must_exist: bool,
        /// If true, only accept directories
        #[serde(default, skip_serializing_if = "is_false")]
        directories_only: bool,
        /// If true, only accept files
        #[serde(default, skip_serializing_if = "is_false")]
        files_only: bool,
    },
    /// URL input with optional reachability check
    #[serde(rename = "url")]
    Url {
        /// Allowed URL schemes (e.g., ["http", "https"])
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        allowed_schemes: Vec<String>,
        /// If true, verify the URL is reachable
        #[serde(default, skip_serializing_if = "is_false")]
        check_reachable: bool,
    },
    /// Date input in ISO 8601 format (YYYY-MM-DD)
    Date {
        /// Minimum date (ISO 8601 format)
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<String>,
        /// Maximum date (ISO 8601 format)
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<String>,
    },
    /// DateTime input in ISO 8601 format (YYYY-MM-DDTHH:MM:SSZ)
    DateTime {
        /// Minimum datetime (ISO 8601 format)
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<String>,
        /// Maximum datetime (ISO 8601 format)
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<String>,
    },
    /// JSON object input (validates JSON syntax)
    #[serde(rename = "json")]
    Json {
        /// If true, format/prettify the JSON before storing
        #[serde(default, skip_serializing_if = "is_false")]
        prettify: bool,
    },
    /// YAML object input (validates YAML syntax)
    #[serde(rename = "yaml")]
    Yaml {
        /// If true, format the YAML before storing
        #[serde(default, skip_serializing_if = "is_false")]
        prettify: bool,
    },
    /// List input (comma-separated values)
    List {
        /// Separator character (default: ",")
        #[serde(default = "default_list_separator")]
        separator: String,
        /// Minimum number of items
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<usize>,
        /// Maximum number of items
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<usize>,
        /// If true, remove whitespace from each item
        #[serde(default = "default_true", skip_serializing_if = "is_true")]
        trim_items: bool,
        /// If true, remove empty items
        #[serde(default = "default_true", skip_serializing_if = "is_true")]
        remove_empty: bool,
    },
    /// Email input with validation
    Email,
    /// IP address input (supports IPv4 and IPv6)
    #[serde(rename = "ip")]
    IpAddress {
        /// If true, only accept IPv4 addresses
        #[serde(default, skip_serializing_if = "is_false")]
        ipv4_only: bool,
        /// If true, only accept IPv6 addresses
        #[serde(default, skip_serializing_if = "is_false")]
        ipv6_only: bool,
    },
    /// CIDR notation input (e.g., 192.168.1.0/24)
    #[serde(rename = "cidr")]
    Cidr {
        /// If true, only accept IPv4 CIDR
        #[serde(default, skip_serializing_if = "is_false")]
        ipv4_only: bool,
        /// If true, only accept IPv6 CIDR
        #[serde(default, skip_serializing_if = "is_false")]
        ipv6_only: bool,
    },
    /// Port number input (1-65535)
    Port,
}

/// Helper function for serde to determine if a bool is false
fn is_false(b: &bool) -> bool {
    !b
}

/// Helper function for serde to determine if a bool is true
fn is_true(b: &bool) -> bool {
    *b
}

/// Default value for list separator
fn default_list_separator() -> String {
    ",".to_string()
}

/// Default value for true boolean
fn default_true() -> bool {
    true
}

/// Enum option with display text and value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnumOption {
    /// Display text shown to the user
    pub label: String,
    /// Actual value used in templates
    pub value: String,
}

/// Validation rules for inputs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InputValidation {
    /// URL validation - checks if the input is a valid URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<UrlValidation>,

    /// Email validation - checks if the input is a valid email
    #[serde(default, skip_serializing_if = "is_false")]
    pub email: bool,

    /// Confirmation validation - asks for the same value again and compares
    #[serde(default, skip_serializing_if = "is_false")]
    pub confirm: bool,

    /// Minimum value/length validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,

    /// Maximum value/length validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,

    /// Regex pattern validation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub regex: Option<String>,

    /// Custom validation error message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

/// URL validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlValidation {
    /// Whether to make an HTTP request to verify the URL is accessible
    #[serde(default, skip_serializing_if = "is_false")]
    pub check_reachable: bool,

    /// Allowed URL schemes (e.g., ["http", "https"])
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_schemes: Vec<String>,
}

/// Input specification for a template input
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputSpec {
    /// Input type specification
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub input_type: Option<InputType>,

    /// Possible enum values for this input (deprecated, use input_type with Select)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,

    /// Default value (supports variable interpolation like ${var:_name})
    #[serde(default)]
    pub default: Option<Value>,

    /// Description of the input
    #[serde(default)]
    pub description: Option<String>,

    /// Validation rules
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<InputValidation>,
}

/// Input definition with a name (used in array format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputDefinition {
    /// Name of the input
    pub name: String,

    /// Input type specification
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub input_type: Option<InputType>,

    /// Possible enum values for this input (deprecated, use input_type with Select)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enum_values: Option<Vec<String>>,

    /// Default value (supports variable interpolation like ${var:_name})
    #[serde(default)]
    pub default: Option<Value>,

    /// Description of the input
    #[serde(default)]
    pub description: Option<String>,

    /// Validation rules
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub validation: Option<InputValidation>,
}

impl InputDefinition {
    /// Convert to InputSpec (without the name)
    pub fn to_input_spec(&self) -> InputSpec {
        InputSpec {
            input_type: self.input_type.clone(),
            enum_values: self.enum_values.clone(),
            default: self.default.clone(),
            description: self.description.clone(),
            validation: self.validation.clone(),
        }
    }
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
    /// Input overrides (supports both array and object format)
    #[serde(default, deserialize_with = "deserialize_inputs")]
    pub inputs: Vec<InputDefinition>,
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

    /// Raw module inputs that will be passed as-is (unquoted) to the module in _common.tf
    /// Key: parameter name, Value: raw HCL expression (e.g., "var.some_value", "local.computed")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_module_inputs: Option<HashMap<String, String>>,
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

/// Reference to a dependency project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyProject {
    /// Name of the project
    pub name: String,

    /// List of environments of the dependency project
    pub environments: Vec<String>,

    /// If true, create the project if it doesn't exist (default: false)
    /// This is typically set to true for dependencies generated from ProjectGroup's spec.projects
    #[serde(default)]
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub create: bool,
}

/// Project dependency configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDependency {
    /// Project reference
    pub project: DependencyProject,
}

// ============================================================================
// Project Group Configuration (for grouping template)
// ============================================================================

/// Input configuration for a project in a project group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGroupInputConfig {
    /// The value to use for this input (optional if use_default is true)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<Value>,

    /// If true, use the template's default value for this input
    #[serde(default)]
    pub use_default: bool,
}

/// Reference project configuration for a project in a project group
/// This is a simplified configuration that references projects by name
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGroupReferenceProject {
    /// Name of the reference project
    pub name: String,

    /// Optional: Environment name (defaults to same environment as the project group)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,

    /// Optional: Data source name for this reference in _common.tf
    /// If not provided, will be auto-generated as ref_0, ref_1, etc.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_source_name: Option<String>,
}

/// A project configuration within a project group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectGroupProject {
    /// Name of the project to create
    pub name: String,

    /// Name of the template pack to use
    pub template_pack: String,

    /// Name of the template within the pack
    pub template: String,

    /// Optional: Individual input configurations
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub inputs: HashMap<String, ProjectGroupInputConfig>,

    /// If true, use all default values from the template (don't prompt for any inputs)
    #[serde(default)]
    pub use_all_defaults: bool,

    /// Optional: Reference projects to pass when creating/updating this project
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reference_projects: Vec<ProjectGroupReferenceProject>,
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

    /// Dependencies on other projects
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<ProjectDependency>,

    /// Projects to manage (for project group template)
    /// When this field is populated, the project group will create/update these projects
    /// and execute commands on them when apply/preview/destroy is run
    #[serde(default)]
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub projects: Vec<ProjectGroupProject>,

    /// Hooks to run before and after commands
    /// These hooks are copied from the template and can be overridden per environment
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<HooksConfig>,
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

/// Category for organizing templates in a hierarchical structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Category {
    /// Unique identifier for this category
    pub id: String,

    /// Display name for this category
    pub name: String,

    /// Optional description of this category
    #[serde(default)]
    pub description: Option<String>,

    /// Nested subcategories
    #[serde(default)]
    pub subcategories: Vec<Category>,

    /// Templates directly in this category
    #[serde(default)]
    pub templates: Vec<CategoryTemplate>,
}

/// Reference to a template within a category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryTemplate {
    /// Template pack name
    #[serde(rename = "template_pack")]
    pub template_pack: String,

    /// Template name
    pub template: String,
}

/// Configuration for a specific template pack
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TemplatePackConfig {
    /// Template-specific configurations
    /// Key: template name, Value: template override configuration
    #[serde(default)]
    pub templates: HashMap<String, TemplateOverrideConfig>,
}

/// Override configuration for a specific template
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TemplateOverrideConfig {
    /// Default input overrides for this template
    #[serde(default)]
    pub defaults: TemplateDefaults,
}

/// Infrastructure specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfrastructureSpec {
    /// Hierarchical category structure for organizing templates
    #[serde(default)]
    pub categories: Vec<Category>,

    /// Template pack configurations with template-level defaults
    /// Key: template pack name, Value: template pack configuration
    #[serde(default)]
    pub template_packs: HashMap<String, TemplatePackConfig>,

    /// DEPRECATED: Old resource_kinds field for backward compatibility
    /// This field is only used during migration from old format
    #[serde(default, skip_serializing)]
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

/// Configuration for a command hook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandHookConfig {
    /// The shell command to execute
    pub command: String,
}

/// Configuration for a confirm hook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmHookConfig {
    /// The question to ask the user
    pub question: String,

    /// If true, exit (cancel) the command when user cancels/declines the confirmation
    #[serde(default = "default_exit_on_cancel")]
    pub exit_on_cancel: bool,

    /// If true, exit (cancel) the command when user confirms
    #[serde(default)]
    pub exit_on_confirm: bool,
}

fn default_exit_on_cancel() -> bool {
    true
}

/// Configuration for a set_environment hook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetEnvironmentHookConfig {
    /// The environment variable name to set
    pub name: String,

    /// The prompt message to show the user
    pub prompt: String,

    /// If true, use password-like input (hidden characters)
    #[serde(default)]
    pub sensitive: bool,
}

/// A hook that can be executed before or after a command
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum Hook {
    /// Execute a shell command
    #[serde(rename = "command")]
    Command(CommandHookConfig),

    /// Ask user for confirmation before proceeding
    #[serde(rename = "confirm")]
    Confirm(ConfirmHookConfig),

    /// Ask user for a value and set it as an environment variable
    #[serde(rename = "set_environment")]
    SetEnvironment(SetEnvironmentHookConfig),
}

/// Hooks configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HooksConfig {
    /// Hooks to run before preview
    #[serde(default)]
    pub pre_preview: Vec<Hook>,

    /// Hooks to run after preview
    #[serde(default)]
    pub post_preview: Vec<Hook>,

    /// Hooks to run before apply
    #[serde(default)]
    pub pre_apply: Vec<Hook>,

    /// Hooks to run after apply
    #[serde(default)]
    pub post_apply: Vec<Hook>,

    /// Hooks to run before destroy
    #[serde(default)]
    pub pre_destroy: Vec<Hook>,

    /// Hooks to run after destroy
    #[serde(default)]
    pub post_destroy: Vec<Hook>,

    /// Hooks to run before refresh
    #[serde(default)]
    pub pre_refresh: Vec<Hook>,

    /// Hooks to run after refresh
    #[serde(default)]
    pub post_refresh: Vec<Hook>,
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

    /// Check if this filter matches a template spec (DEPRECATED - use categories instead)
    #[allow(dead_code)]
    pub fn matches_template(&self, template: &TemplateSpec) -> bool {
        self.api_version == template.api_version && self.kind == template.kind
    }

    /// Check if a specific template is allowed and get its configuration (DEPRECATED - use template_packs instead)
    /// Returns:
    /// - Some(Some(config)) if template is configured and allowed
    /// - Some(None) if template is configured but not allowed
    /// - None if no template-specific configuration exists (allow by default)
    #[allow(dead_code)]
    pub fn get_template_config(
        &self,
        template_name: &str,
        template_pack_name: &str,
    ) -> Option<Option<&TemplateConfig>> {
        if let Some(ref templates) = self.templates
            && let Some(config) = templates.get(template_name)
        {
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
    pub fn from_file(
        fs: &dyn crate::traits::FileSystem,
        path: &std::path::Path,
    ) -> anyhow::Result<Self> {
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
    pub fn from_file(
        fs: &dyn crate::traits::FileSystem,
        path: &std::path::Path,
    ) -> anyhow::Result<Self> {
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

        // Validate that user-defined input names do not start with underscore
        // (underscore prefix is reserved for PMP-provided variables)
        for input in &resource.spec.inputs {
            if input.name.starts_with('_') {
                anyhow::bail!(
                    "Input name '{}' is invalid: input names starting with underscore are reserved for PMP-provided variables",
                    input.name
                );
            }
        }

        // Validate environment-specific input overrides
        for (env_name, env_overrides) in &resource.spec.environments {
            for input in &env_overrides.overrides.inputs {
                if input.name.starts_with('_') {
                    anyhow::bail!(
                        "Input name '{}' in environment '{}' is invalid: input names starting with underscore are reserved for PMP-provided variables",
                        input.name,
                        env_name
                    );
                }
            }
        }

        Ok(resource)
    }
}

impl PluginResource {
    /// Load plugin resource from a .pmp.plugin.yaml file
    #[allow(dead_code)]
    pub fn from_file(
        fs: &dyn crate::traits::FileSystem,
        path: &std::path::Path,
    ) -> anyhow::Result<Self> {
        let content = fs.read_to_string(path)?;
        let resource: PluginResource = serde_yaml::from_str(&content)?;

        // Validate kind
        if resource.kind != "Plugin" {
            anyhow::bail!("Expected kind 'Plugin', got '{}'", resource.kind);
        }

        // Validate that user-defined input names do not start with underscore
        // (underscore prefix is reserved for PMP-provided variables)
        for input in &resource.spec.inputs {
            if input.name.starts_with('_') {
                anyhow::bail!(
                    "Plugin input name '{}' is invalid: input names starting with underscore are reserved for PMP-provided variables",
                    input.name
                );
            }
        }

        Ok(resource)
    }
}

impl InfrastructureTemplateResource {
    /// Load infrastructure template resource from a .pmp.infrastructure-template.yaml file
    pub fn from_file(
        fs: &dyn crate::traits::FileSystem,
        path: &std::path::Path,
    ) -> anyhow::Result<Self> {
        let content = fs.read_to_string(path)?;
        let resource: InfrastructureTemplateResource = serde_yaml::from_str(&content)?;

        // Validate kind
        if resource.kind != "InfrastructureTemplate" {
            anyhow::bail!("Expected kind 'InfrastructureTemplate', got '{}'", resource.kind);
        }

        Ok(resource)
    }
}

impl ProjectResource {
    /// Load project resource from a .pmp.project.yaml file
    pub fn from_file(
        fs: &dyn crate::traits::FileSystem,
        path: &std::path::Path,
    ) -> anyhow::Result<Self> {
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
    pub fn from_file(
        fs: &dyn crate::traits::FileSystem,
        path: &std::path::Path,
    ) -> anyhow::Result<Self> {
        let content = fs.read_to_string(path)?;
        let resource: ProjectEnvironmentResource = serde_yaml::from_str(&content)?;

        // Validate kind
        if resource.kind != "ProjectEnvironment" {
            anyhow::bail!(
                "Expected kind 'ProjectEnvironment', got '{}'",
                resource.kind
            );
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
    pub fn from_file(
        fs: &dyn crate::traits::FileSystem,
        path: &std::path::Path,
    ) -> anyhow::Result<Self> {
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
    /// Load infrastructure resource from a .pmp.yaml file with automatic migration
    pub fn from_file(
        fs: &dyn crate::traits::FileSystem,
        path: &std::path::Path,
    ) -> anyhow::Result<Self> {
        let content = fs.read_to_string(path)?;
        let mut resource: InfrastructureResource = serde_yaml::from_str(&content)?;

        // Validate kind
        if resource.kind != "Infrastructure" {
            anyhow::bail!("Expected kind 'Infrastructure', got '{}'", resource.kind);
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

        // Auto-migrate from old format if resource_kinds is present but categories is empty
        if !resource.spec.resource_kinds.is_empty() && resource.spec.categories.is_empty() {
            // Old format detected - migrate to new format
            resource = Self::migrate_to_category_format(resource)?;

            // Create backup of old file
            let backup_path = path.with_extension("yaml.backup");
            fs.write(&backup_path, &content)?;

            // Save migrated version
            resource.save(fs, path)?;
        }

        Ok(resource)
    }

    /// Migrate from old resource_kinds format to new category format
    fn migrate_to_category_format(mut resource: InfrastructureResource) -> anyhow::Result<Self> {
        let mut categories = Vec::new();
        let mut template_packs: HashMap<String, TemplatePackConfig> = HashMap::new();

        // Create one top-level category per resource kind
        for resource_kind in &resource.spec.resource_kinds {
            let category_id = format!(
                "{}_{}",
                resource_kind
                    .api_version
                    .replace("/", "_")
                    .replace(".", "_"),
                resource_kind.kind.to_lowercase()
            );

            // Extract templates from this resource kind
            let mut category_templates = Vec::new();
            if let Some(ref templates_config) = resource_kind.templates {
                for (template_name, template_config) in templates_config {
                    // Only include allowed templates
                    if template_config.allowed {
                        category_templates.push(CategoryTemplate {
                            template_pack: template_config.template_pack_name.clone(),
                            template: template_name.clone(),
                        });

                        // Move template defaults to template_packs config
                        let pack_config = template_packs
                            .entry(template_config.template_pack_name.clone())
                            .or_default();

                        pack_config.templates.insert(
                            template_name.clone(),
                            TemplateOverrideConfig {
                                defaults: template_config.defaults.clone(),
                            },
                        );
                    }
                }
            }

            // Create category for this resource kind
            categories.push(Category {
                id: category_id,
                name: format!("{} ({})", resource_kind.kind, resource_kind.api_version),
                description: Some(format!(
                    "Migrated from resource kind: {}/{}",
                    resource_kind.api_version, resource_kind.kind
                )),
                subcategories: Vec::new(),
                templates: category_templates,
            });
        }

        // Update resource with new structure
        resource.spec.categories = categories;
        resource.spec.template_packs = template_packs;
        resource.spec.resource_kinds = Vec::new(); // Clear old field

        Ok(resource)
    }

    /// Validate environment name format
    pub fn is_valid_environment_name(name: &str) -> bool {
        !name.is_empty()
            && !name.chars().next().is_some_and(|c| c.is_ascii_digit())
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    }

    /// Save the infrastructure to a .pmp.yaml file
    pub fn save(
        &self,
        fs: &dyn crate::traits::FileSystem,
        path: &std::path::Path,
    ) -> anyhow::Result<()> {
        let content = serde_yaml::to_string(self)?;
        fs.write(path, &content)?;
        Ok(())
    }

    /// Get the hooks configuration, or return empty hooks
    pub fn get_hooks(&self) -> HooksConfig {
        self.spec.hooks.clone().unwrap_or_default()
    }

    /// Check if a template is present in the category tree
    pub fn is_template_in_category_tree(&self, template_pack: &str, template_name: &str) -> bool {
        Self::search_template_in_categories(&self.spec.categories, template_pack, template_name)
    }

    /// Recursively search for a template in the category tree
    fn search_template_in_categories(
        categories: &[Category],
        template_pack: &str,
        template_name: &str,
    ) -> bool {
        for category in categories {
            // Check templates in this category
            for template in &category.templates {
                if template.template_pack == template_pack && template.template == template_name {
                    return true;
                }
            }

            // Check subcategories recursively
            if Self::search_template_in_categories(
                &category.subcategories,
                template_pack,
                template_name,
            ) {
                return true;
            }
        }
        false
    }

    /// Get template configuration from template_packs
    pub fn get_template_config(
        &self,
        template_pack: &str,
        template_name: &str,
    ) -> Option<&TemplateOverrideConfig> {
        self.spec
            .template_packs
            .get(template_pack)?
            .templates
            .get(template_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{FileSystem, MockFileSystem};
    use std::sync::Arc;

    #[test]
    fn test_category_structure_basic() {
        let category = Category {
            id: "test_category".to_string(),
            name: "Test Category".to_string(),
            description: Some("Test description".to_string()),
            subcategories: vec![],
            templates: vec![CategoryTemplate {
                template_pack: "pack1".to_string(),
                template: "template1".to_string(),
            }],
        };

        assert_eq!(category.id, "test_category");
        assert_eq!(category.name, "Test Category");
        assert_eq!(category.templates.len(), 1);
        assert_eq!(category.subcategories.len(), 0);
    }

    #[test]
    fn test_category_with_subcategories() {
        let category = Category {
            id: "parent".to_string(),
            name: "Parent Category".to_string(),
            description: None,
            subcategories: vec![
                Category {
                    id: "child1".to_string(),
                    name: "Child 1".to_string(),
                    description: None,
                    subcategories: vec![],
                    templates: vec![],
                },
                Category {
                    id: "child2".to_string(),
                    name: "Child 2".to_string(),
                    description: None,
                    subcategories: vec![],
                    templates: vec![],
                },
            ],
            templates: vec![],
        };

        assert_eq!(category.subcategories.len(), 2);
        assert_eq!(category.subcategories[0].id, "child1");
        assert_eq!(category.subcategories[1].id, "child2");
    }

    #[test]
    fn test_is_template_in_category_tree_found() {
        let infrastructure = InfrastructureResource {
            api_version: "pmp.io/v1".to_string(),
            kind: "Infrastructure".to_string(),
            metadata: InfrastructureMetadata {
                name: "Test".to_string(),
                description: None,
            },
            spec: InfrastructureSpec {
                categories: vec![Category {
                    id: "cat1".to_string(),
                    name: "Category 1".to_string(),
                    description: None,
                    subcategories: vec![],
                    templates: vec![CategoryTemplate {
                        template_pack: "pack1".to_string(),
                        template: "template1".to_string(),
                    }],
                }],
                template_packs: HashMap::new(),
                resource_kinds: vec![],
                environments: HashMap::new(),
                hooks: None,
                executor: None,
            },
        };

        assert!(infrastructure.is_template_in_category_tree("pack1", "template1"));
        assert!(!infrastructure.is_template_in_category_tree("pack1", "template2"));
        assert!(!infrastructure.is_template_in_category_tree("pack2", "template1"));
    }

    #[test]
    fn test_is_template_in_category_tree_nested() {
        let infrastructure = InfrastructureResource {
            api_version: "pmp.io/v1".to_string(),
            kind: "Infrastructure".to_string(),
            metadata: InfrastructureMetadata {
                name: "Test".to_string(),
                description: None,
            },
            spec: InfrastructureSpec {
                categories: vec![Category {
                    id: "parent".to_string(),
                    name: "Parent".to_string(),
                    description: None,
                    subcategories: vec![Category {
                        id: "child".to_string(),
                        name: "Child".to_string(),
                        description: None,
                        subcategories: vec![],
                        templates: vec![CategoryTemplate {
                            template_pack: "nested_pack".to_string(),
                            template: "nested_template".to_string(),
                        }],
                    }],
                    templates: vec![],
                }],
                template_packs: HashMap::new(),
                resource_kinds: vec![],
                environments: HashMap::new(),
                hooks: None,
                executor: None,
            },
        };

        // Should find template in nested subcategory
        assert!(infrastructure.is_template_in_category_tree("nested_pack", "nested_template"));
    }

    #[test]
    fn test_get_template_config_found() {
        let mut template_packs = HashMap::new();
        let mut pack_config = TemplatePackConfig::default();
        pack_config.templates.insert(
            "template1".to_string(),
            TemplateOverrideConfig {
                defaults: TemplateDefaults {
                    inputs: {
                        let mut inputs = HashMap::new();
                        inputs.insert(
                            "input1".to_string(),
                            InputOverride {
                                value: serde_json::Value::String("value1".to_string()),
                                show_as_default: true,
                            },
                        );
                        inputs
                    },
                },
            },
        );
        template_packs.insert("pack1".to_string(), pack_config);

        let infrastructure = InfrastructureResource {
            api_version: "pmp.io/v1".to_string(),
            kind: "Infrastructure".to_string(),
            metadata: InfrastructureMetadata {
                name: "Test".to_string(),
                description: None,
            },
            spec: InfrastructureSpec {
                categories: vec![],
                template_packs,
                resource_kinds: vec![],
                environments: HashMap::new(),
                hooks: None,
                executor: None,
            },
        };

        let config = infrastructure.get_template_config("pack1", "template1");
        assert!(config.is_some());
        assert_eq!(config.unwrap().defaults.inputs.len(), 1);

        let no_config = infrastructure.get_template_config("pack1", "nonexistent");
        assert!(no_config.is_none());
    }

    #[test]
    fn test_migration_from_resource_kinds_to_categories() {
        let fs = Arc::new(MockFileSystem::new());

        // Create old format infrastructure file
        let old_format = r#"apiVersion: pmp.io/v1
kind: Infrastructure
metadata:
  name: Test Infrastructure
  description: Test
spec:
  resource_kinds:
    - apiVersion: pmp.io/v1
      kind: TestResource
      templates:
        template1:
          template_pack_name: pack1
          allowed: true
          defaults:
            inputs:
              input1:
                value: "test_value"
                show_as_default: false
  environments:
    dev:
      name: Development
      description: Dev environment
"#;

        let path = std::path::PathBuf::from("/test/.pmp.infrastructure.yaml");
        fs.write(&path, old_format).unwrap();

        // Load and verify migration happens
        let infrastructure = InfrastructureResource::from_file(&*fs, &path).unwrap();

        // Should have categories now
        assert!(!infrastructure.spec.categories.is_empty());
        assert_eq!(infrastructure.spec.categories.len(), 1);

        let category = &infrastructure.spec.categories[0];
        assert_eq!(category.id, "pmp_io_v1_testresource");
        assert!(category.name.contains("TestResource"));
        assert_eq!(category.templates.len(), 1);
        assert_eq!(category.templates[0].template_pack, "pack1");
        assert_eq!(category.templates[0].template, "template1");

        // Should have template_packs config
        assert!(infrastructure.spec.template_packs.contains_key("pack1"));
        let pack_config = infrastructure.spec.template_packs.get("pack1").unwrap();
        assert!(pack_config.templates.contains_key("template1"));

        // Old resource_kinds should be cleared
        assert!(infrastructure.spec.resource_kinds.is_empty());

        // Backup should be created
        let backup_path = path.with_extension("yaml.backup");
        assert!(fs.exists(&backup_path));
    }

    #[test]
    fn test_migration_filters_blocked_templates() {
        let fs = Arc::new(MockFileSystem::new());

        let old_format = r#"apiVersion: pmp.io/v1
kind: Infrastructure
metadata:
  name: Test Infrastructure
spec:
  resource_kinds:
    - apiVersion: pmp.io/v1
      kind: TestResource
      templates:
        allowed_template:
          template_pack_name: pack1
          allowed: true
        blocked_template:
          template_pack_name: pack1
          allowed: false
  environments:
    dev:
      name: Development
"#;

        let path = std::path::PathBuf::from("/test2/.pmp.infrastructure.yaml");
        fs.write(&path, old_format).unwrap();

        let infrastructure = InfrastructureResource::from_file(&*fs, &path).unwrap();

        // Should only include allowed template
        assert_eq!(infrastructure.spec.categories.len(), 1);
        let category = &infrastructure.spec.categories[0];
        assert_eq!(category.templates.len(), 1);
        assert_eq!(category.templates[0].template, "allowed_template");

        // Template pack config should only have allowed template
        let pack_config = infrastructure.spec.template_packs.get("pack1").unwrap();
        assert!(pack_config.templates.contains_key("allowed_template"));
        assert!(!pack_config.templates.contains_key("blocked_template"));
    }

    #[test]
    fn test_no_migration_for_new_format() {
        let fs = Arc::new(MockFileSystem::new());

        // Create new format infrastructure file
        let new_format = r#"apiVersion: pmp.io/v1
kind: Infrastructure
metadata:
  name: Test Infrastructure
spec:
  categories:
    - id: test_cat
      name: Test Category
      templates:
        - template_pack: pack1
          template: template1
  template_packs:
    pack1:
      templates:
        template1:
          defaults: {}
  environments:
    dev:
      name: Development
"#;

        let path = std::path::PathBuf::from("/test3/.pmp.infrastructure.yaml");
        fs.write(&path, new_format).unwrap();

        let infrastructure = InfrastructureResource::from_file(&*fs, &path).unwrap();

        // Should not migrate (no backup created)
        let backup_path = path.with_extension("yaml.backup");
        assert!(!fs.exists(&backup_path));

        // Should have original categories
        assert_eq!(infrastructure.spec.categories.len(), 1);
        assert_eq!(infrastructure.spec.categories[0].id, "test_cat");
    }

    #[test]
    fn test_migration_handles_multiple_resource_kinds() {
        let fs = Arc::new(MockFileSystem::new());

        let old_format = r#"apiVersion: pmp.io/v1
kind: Infrastructure
metadata:
  name: Multi Resource Test
spec:
  resource_kinds:
    - apiVersion: pmp.io/v1
      kind: ResourceA
    - apiVersion: pmp.io/v1
      kind: ResourceB
  environments:
    dev:
      name: Development
"#;

        let path = std::path::PathBuf::from("/test4/.pmp.infrastructure.yaml");
        fs.write(&path, old_format).unwrap();

        let infrastructure = InfrastructureResource::from_file(&*fs, &path).unwrap();

        // Should create one category per resource kind
        assert_eq!(infrastructure.spec.categories.len(), 2);

        let category_names: Vec<String> = infrastructure
            .spec
            .categories
            .iter()
            .map(|c| c.name.clone())
            .collect();

        assert!(category_names.iter().any(|n| n.contains("ResourceA")));
        assert!(category_names.iter().any(|n| n.contains("ResourceB")));
    }

    #[test]
    fn test_template_in_multiple_categories() {
        let infrastructure = InfrastructureResource {
            api_version: "pmp.io/v1".to_string(),
            kind: "Infrastructure".to_string(),
            metadata: InfrastructureMetadata {
                name: "Test".to_string(),
                description: None,
            },
            spec: InfrastructureSpec {
                categories: vec![
                    Category {
                        id: "cat1".to_string(),
                        name: "Category 1".to_string(),
                        description: None,
                        subcategories: vec![],
                        templates: vec![CategoryTemplate {
                            template_pack: "pack1".to_string(),
                            template: "shared_template".to_string(),
                        }],
                    },
                    Category {
                        id: "cat2".to_string(),
                        name: "Category 2".to_string(),
                        description: None,
                        subcategories: vec![],
                        templates: vec![CategoryTemplate {
                            template_pack: "pack1".to_string(),
                            template: "shared_template".to_string(),
                        }],
                    },
                ],
                template_packs: HashMap::new(),
                resource_kinds: vec![],
                environments: HashMap::new(),
                hooks: None,
                executor: None,
            },
        };

        // Same template should be found in both categories
        assert!(infrastructure.is_template_in_category_tree("pack1", "shared_template"));
    }

    #[test]
    fn test_environment_name_validation() {
        // Valid environment names
        assert!(InfrastructureResource::is_valid_environment_name("dev"));
        assert!(InfrastructureResource::is_valid_environment_name(
            "production"
        ));
        assert!(InfrastructureResource::is_valid_environment_name(
            "staging_1"
        ));
        assert!(InfrastructureResource::is_valid_environment_name("test123"));
        assert!(InfrastructureResource::is_valid_environment_name("dev_env"));

        // Invalid environment names
        assert!(!InfrastructureResource::is_valid_environment_name("Dev")); // uppercase
        assert!(!InfrastructureResource::is_valid_environment_name(
            "dev-env"
        )); // hyphen not allowed
        assert!(!InfrastructureResource::is_valid_environment_name("123dev")); // starts with number
        assert!(!InfrastructureResource::is_valid_environment_name(
            "dev env"
        )); // space
        assert!(!InfrastructureResource::is_valid_environment_name(
            "dev.env"
        )); // dot
    }

    #[test]
    fn test_category_template_serialization() {
        let template = CategoryTemplate {
            template_pack: "test-pack".to_string(),
            template: "test-template".to_string(),
        };

        let yaml = serde_yaml::to_string(&template).unwrap();
        assert!(yaml.contains("template_pack: test-pack"));
        assert!(yaml.contains("template: test-template"));

        let deserialized: CategoryTemplate = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.template_pack, "test-pack");
        assert_eq!(deserialized.template, "test-template");
    }

    #[test]
    fn test_empty_categories_and_template_packs() {
        let infrastructure = InfrastructureResource {
            api_version: "pmp.io/v1".to_string(),
            kind: "Infrastructure".to_string(),
            metadata: InfrastructureMetadata {
                name: "Empty Test".to_string(),
                description: None,
            },
            spec: InfrastructureSpec {
                categories: vec![],
                template_packs: HashMap::new(),
                resource_kinds: vec![],
                environments: HashMap::new(),
                hooks: None,
                executor: None,
            },
        };

        // Should handle empty structures gracefully
        assert!(!infrastructure.is_template_in_category_tree("any", "template"));
        assert!(
            infrastructure
                .get_template_config("any", "template")
                .is_none()
        );
    }

    #[test]
    fn test_input_definition_array_format_deserialization() {
        // Test array format with various input types
        let yaml = r#"
apiVersion: pmp.io/v1
kind: Template
metadata:
  name: test-template
  description: Test template
spec:
  apiVersion: pmp.io/v1
  kind: TestResource
  executor: opentofu
  inputs:
    - name: string_input
      description: A string input
      default: "test"
    - name: number_input
      description: A number input
      default: 42
    - name: bool_input
      description: A boolean input
      default: true
    - name: enum_input
      description: An enum input
      default: option1
      enum_values:
        - option1
        - option2
        - option3
  environments: {}
"#;

        let template: TemplateResource = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(template.spec.inputs.len(), 4);

        // Verify inputs are in order
        assert_eq!(template.spec.inputs[0].name, "string_input");
        assert_eq!(template.spec.inputs[1].name, "number_input");
        assert_eq!(template.spec.inputs[2].name, "bool_input");
        assert_eq!(template.spec.inputs[3].name, "enum_input");

        // Verify values
        assert_eq!(
            template.spec.inputs[0].default,
            Some(serde_json::Value::String("test".to_string()))
        );
        assert_eq!(
            template.spec.inputs[1].default,
            Some(serde_json::Value::Number(42.into()))
        );
        assert_eq!(
            template.spec.inputs[2].default,
            Some(serde_json::Value::Bool(true))
        );
    }

    #[test]
    fn test_input_definition_object_format_deserialization() {
        // Test legacy object format (backward compatibility)
        let yaml = r#"
apiVersion: pmp.io/v1
kind: Template
metadata:
  name: test-template
  description: Test template
spec:
  apiVersion: pmp.io/v1
  kind: TestResource
  executor: opentofu
  inputs:
    string_input:
      description: A string input
      default: "test"
    number_input:
      description: A number input
      default: 42
  environments: {}
"#;

        let template: TemplateResource = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(template.spec.inputs.len(), 2);

        // Note: Object format doesn't guarantee order, but items should be present
        let names: Vec<_> = template
            .spec
            .inputs
            .iter()
            .map(|i| i.name.as_str())
            .collect();
        assert!(names.contains(&"string_input"));
        assert!(names.contains(&"number_input"));
    }

    #[test]
    fn test_plugin_inputs_array_format() {
        let yaml = r#"
apiVersion: pmp.io/v1
kind: Plugin
metadata:
  name: test-plugin
  description: Test plugin
spec:
  role: test-role
  inputs:
    - name: plugin_input1
      description: First plugin input
      default: "value1"
    - name: plugin_input2
      description: Second plugin input
      default: 100
"#;

        let plugin: PluginResource = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(plugin.spec.inputs.len(), 2);
        assert_eq!(plugin.spec.inputs[0].name, "plugin_input1");
        assert_eq!(plugin.spec.inputs[1].name, "plugin_input2");
    }

    #[test]
    fn test_project_spec_with_dependencies() {
        let yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: test-project
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies:
    - project:
        name: database-project
        environments:
          - dev
          - staging
    - project:
        name: network-project
        environments:
          - dev
"#;

        let resource: DynamicProjectEnvironmentResource = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(resource.spec.dependencies.len(), 2);

        let dep1 = &resource.spec.dependencies[0];
        assert_eq!(dep1.project.name, "database-project");
        assert_eq!(dep1.project.environments.len(), 2);
        assert_eq!(dep1.project.environments[0], "dev");
        assert_eq!(dep1.project.environments[1], "staging");

        let dep2 = &resource.spec.dependencies[1];
        assert_eq!(dep2.project.name, "network-project");
        assert_eq!(dep2.project.environments.len(), 1);
        assert_eq!(dep2.project.environments[0], "dev");
    }

    #[test]
    fn test_project_spec_empty_dependencies() {
        let yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: test-project
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies: []
"#;

        let resource: DynamicProjectEnvironmentResource = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(resource.spec.dependencies.len(), 0);
    }

    #[test]
    fn test_project_spec_no_dependencies_field() {
        // Test backward compatibility - dependencies field is optional
        let yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: test-project
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
"#;

        let resource: DynamicProjectEnvironmentResource = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(resource.spec.dependencies.len(), 0);
    }

    #[test]
    fn test_project_spec_dependencies_serialization() {
        use std::collections::HashMap;

        let project_spec = ProjectSpec {
            resource: ResourceDefinition {
                api_version: "pmp.io/v1".to_string(),
                kind: "TestResource".to_string(),
            },
            executor: ExecutorProjectConfig {
                name: "opentofu".to_string(),
            },
            inputs: HashMap::new(),
            custom: None,
            plugins: None,
            template: None,
            environment: None,
            template_reference_projects: Vec::new(),
            projects: Vec::new(),
            dependencies: vec![
                ProjectDependency {
                    project: DependencyProject {
                        name: "db-project".to_string(),
                        environments: vec!["dev".to_string(), "staging".to_string()],
                        create: false,
                    },
                },
                ProjectDependency {
                    project: DependencyProject {
                        name: "network-project".to_string(),
                        environments: vec!["dev".to_string()],
                        create: false,
                    },
                },
            ],
            hooks: None,
        };

        let yaml = serde_yaml::to_string(&project_spec).unwrap();

        // Verify the YAML contains the dependencies
        assert!(yaml.contains("dependencies:"));
        assert!(yaml.contains("db-project"));
        assert!(yaml.contains("network-project"));
        assert!(yaml.contains("- dev"));
        assert!(yaml.contains("- staging"));

        // Deserialize back and verify
        let deserialized: ProjectSpec = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.dependencies.len(), 2);
        assert_eq!(deserialized.dependencies[0].project.name, "db-project");
        assert_eq!(deserialized.dependencies[0].project.environments.len(), 2);
        assert_eq!(deserialized.dependencies[1].project.name, "network-project");
        assert_eq!(deserialized.dependencies[1].project.environments.len(), 1);
    }

    #[test]
    fn test_dependency_project_single_environment() {
        let dep = DependencyProject {
            name: "test-project".to_string(),
            environments: vec!["production".to_string()],
            create: false,
        };

        assert_eq!(dep.name, "test-project");
        assert_eq!(dep.environments.len(), 1);
        assert_eq!(dep.environments[0], "production");
    }

    #[test]
    fn test_dependency_project_multiple_environments() {
        let dep = DependencyProject {
            name: "multi-env-project".to_string(),
            environments: vec![
                "dev".to_string(),
                "staging".to_string(),
                "production".to_string(),
            ],
            create: false,
        };

        assert_eq!(dep.name, "multi-env-project");
        assert_eq!(dep.environments.len(), 3);
        assert!(dep.environments.contains(&"dev".to_string()));
        assert!(dep.environments.contains(&"staging".to_string()));
        assert!(dep.environments.contains(&"production".to_string()));
    }
}
