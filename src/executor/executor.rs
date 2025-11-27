use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use std::process::Output;

/// Configuration for executor execution, typically loaded from .pmp.yaml
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Optional custom plan command (overrides default)
    pub plan_command: Option<String>,
    /// Optional custom apply command (overrides default)
    pub apply_command: Option<String>,
    /// Optional custom destroy command (overrides default)
    pub destroy_command: Option<String>,
    /// Optional custom refresh command (overrides default)
    pub refresh_command: Option<String>,
}

/// Project metadata for backend table name generation
#[derive(Debug, Clone)]
pub struct ProjectMetadata<'a> {
    pub api_version: &'a str,
    pub kind: &'a str,
    pub environment: &'a str,
    pub project_name: &'a str,
}

/// Trait for Infrastructure as Code executors (OpenTofu, Terraform, etc.)
pub trait Executor: Send + Sync {
    /// Check if the executor is installed and available
    /// Typically runs a help or version command to verify
    fn check_installed(&self) -> Result<bool>;

    /// Initialize the executor in the working directory
    /// For OpenTofu/Terraform, this runs 'tofu init' or 'terraform init'
    fn init(&self, working_dir: &str) -> Result<Output>;

    /// Execute the plan command (preview changes)
    /// Uses custom command from config if provided, otherwise uses default
    /// Runs interactively with inherited stdio for user interaction
    /// Additional args can be passed via extra_args (e.g., from -- separator)
    fn plan(&self, config: &ExecutorConfig, working_dir: &str, extra_args: &[String])
    -> Result<()>;

    /// Execute the apply command (apply changes)
    /// Uses custom command from config if provided, otherwise uses default
    /// Runs interactively with inherited stdio for user interaction
    /// Additional args can be passed via extra_args (e.g., from -- separator)
    fn apply(
        &self,
        config: &ExecutorConfig,
        working_dir: &str,
        extra_args: &[String],
    ) -> Result<()>;

    /// Execute the destroy command (destroy infrastructure)
    /// Uses custom command from config if provided, otherwise uses default
    /// Runs interactively with inherited stdio for user interaction
    /// Additional args can be passed via extra_args (e.g., from -- separator)
    fn destroy(
        &self,
        config: &ExecutorConfig,
        working_dir: &str,
        extra_args: &[String],
    ) -> Result<()>;

    /// Execute the refresh command (update state with real infrastructure)
    /// Uses custom command from config if provided, otherwise uses default
    /// Runs interactively with inherited stdio for user interaction
    /// Additional args can be passed via extra_args (e.g., from -- separator)
    fn refresh(
        &self,
        config: &ExecutorConfig,
        working_dir: &str,
        extra_args: &[String],
    ) -> Result<()>;

    /// Get the name of this executor (e.g., "opentofu", "terraform")
    fn get_name(&self) -> &str;

    /// Get the default plan command for this executor
    fn default_plan_command(&self) -> &str;

    /// Get the default apply command for this executor
    fn default_apply_command(&self) -> &str;

    /// Get the default destroy command for this executor
    fn default_destroy_command(&self) -> &str;

    /// Get the default refresh command for this executor
    fn default_refresh_command(&self) -> &str;

    /// Generate common infrastructure file (e.g., _common.tf) with backend and module configuration
    /// Default implementation does nothing (only OpenTofu executor generates _common.tf)
    fn generate_common_file(
        &self,
        _ctx: &crate::context::Context,
        _environment_path: &Path,
        _executor_config: &HashMap<String, serde_json::Value>,
        _project_metadata: &ProjectMetadata,
        _plugins: Option<&[crate::template::metadata::AddedPlugin]>,
        _template_reference_projects: &[crate::template::metadata::TemplateReferenceProject],
    ) -> Result<()> {
        // Default: do nothing - only OpenTofu executor generates _common.tf
        Ok(())
    }

    /// Get the file extension used by this executor (e.g., ".tf" for OpenTofu/Terraform)
    /// Default implementation returns empty string
    #[allow(dead_code)]
    fn file_extension(&self) -> &str {
        ""
    }
}
