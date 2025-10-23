use anyhow::Result;
use std::process::Output;

/// Configuration for executor execution, typically loaded from .pmp.yaml
#[derive(Debug, Clone)]
pub struct ExecutorConfig {
    /// Optional custom plan command (overrides default)
    pub plan_command: Option<String>,
    /// Optional custom apply command (overrides default)
    pub apply_command: Option<String>,
}

/// Trait for Infrastructure as Code executors (OpenTofu, Terraform, etc.)
pub trait Executor {
    /// Check if the executor is installed and available
    /// Typically runs a help or version command to verify
    fn check_installed(&self) -> Result<bool>;

    /// Initialize the executor in the working directory
    /// For OpenTofu/Terraform, this runs 'tofu init' or 'terraform init'
    fn init(&self, working_dir: &str) -> Result<Output>;

    /// Execute the plan command (preview changes)
    /// Uses custom command from config if provided, otherwise uses default
    /// Runs interactively with inherited stdio for user interaction
    fn plan(&self, config: &ExecutorConfig, working_dir: &str) -> Result<()>;

    /// Execute the apply command (apply changes)
    /// Uses custom command from config if provided, otherwise uses default
    /// Runs interactively with inherited stdio for user interaction
    fn apply(&self, config: &ExecutorConfig, working_dir: &str) -> Result<()>;

    /// Get the name of this executor (e.g., "opentofu", "terraform")
    fn get_name(&self) -> &str;

    /// Get the default plan command for this executor
    fn default_plan_command(&self) -> &str;

    /// Get the default apply command for this executor
    fn default_apply_command(&self) -> &str;
}
