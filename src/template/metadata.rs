use serde::{Deserialize, Serialize};

/// Metadata for a PMP template (stored in template's .pmp.yaml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMetadata {
    /// Name of the template
    pub name: String,

    /// Description of what this template creates
    pub description: String,

    /// Categories this template belongs to (e.g., "workload", "infrastructure")
    pub categories: Vec<String>,

    /// Optional: Path to JSON Schema file (defaults to "schema.json")
    #[serde(default = "default_schema_path")]
    pub schema_path: String,

    /// Optional: Path to template source directory (defaults to "src")
    #[serde(default = "default_src_path")]
    pub src_path: String,
}

fn default_schema_path() -> String {
    "schema.json".to_string()
}

fn default_src_path() -> String {
    "src".to_string()
}

/// Metadata for a PMP project (stored in project's .pmp.yaml)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    /// Type of resource (e.g., "workload", "infrastructure")
    pub resource_type: String,

    /// Description of this project
    pub description: String,

    /// Optional: IaC configuration
    #[serde(default)]
    pub iac: Option<IacProjectConfig>,

    /// Optional: Hooks configuration
    #[serde(default)]
    pub hooks: Option<HooksConfig>,
}

/// IaC configuration in project metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IacProjectConfig {
    /// Executor name (e.g., "opentofu", "terraform")
    #[serde(default = "default_executor")]
    pub executor: String,

    /// Optional: Custom commands
    #[serde(default)]
    pub commands: Option<IacCommands>,
}

fn default_executor() -> String {
    "opentofu".to_string()
}

/// Custom IaC commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IacCommands {
    /// Optional: Custom plan command
    pub plan: Option<String>,

    /// Optional: Custom apply command
    pub apply: Option<String>,
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
}

impl TemplateMetadata {
    /// Load template metadata from a .pmp.yaml file
    pub fn from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let metadata: TemplateMetadata = serde_yaml::from_str(&content)?;
        Ok(metadata)
    }
}

impl ProjectMetadata {
    /// Load project metadata from a .pmp.yaml file
    pub fn from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let metadata: ProjectMetadata = serde_yaml::from_str(&content)?;
        Ok(metadata)
    }

    /// Get the IaC configuration, or return defaults
    pub fn get_iac_config(&self) -> IacProjectConfig {
        self.iac.clone().unwrap_or_else(|| IacProjectConfig {
            executor: default_executor(),
            commands: None,
        })
    }

    /// Get the hooks configuration, or return empty hooks
    pub fn get_hooks(&self) -> HooksConfig {
        self.hooks.clone().unwrap_or_default()
    }
}
