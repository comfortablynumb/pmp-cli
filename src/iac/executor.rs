use anyhow::Result;
use std::process::Output;

/// Configuration for IaC execution, typically loaded from .pmp.yaml
#[derive(Debug, Clone)]
pub struct IacConfig {
    /// Optional custom plan command (overrides default)
    pub plan_command: Option<String>,
    /// Optional custom apply command (overrides default)
    pub apply_command: Option<String>,
}

/// Trait for Infrastructure as Code executors (OpenTofu, Terraform, etc.)
pub trait IacExecutor {
    /// Check if the IaC executor is installed and available
    /// Typically runs a help or version command to verify
    fn check_installed(&self) -> Result<bool>;

    /// Execute the plan command (preview changes)
    /// Uses custom command from config if provided, otherwise uses default
    fn plan(&self, config: &IacConfig, working_dir: &str) -> Result<Output>;

    /// Execute the apply command (apply changes)
    /// Uses custom command from config if provided, otherwise uses default
    fn apply(&self, config: &IacConfig, working_dir: &str) -> Result<Output>;

    /// Get the name of this executor (e.g., "opentofu", "terraform")
    fn get_name(&self) -> &str;

    /// Get the default plan command for this executor
    fn default_plan_command(&self) -> &str;

    /// Get the default apply command for this executor
    fn default_apply_command(&self) -> &str;
}
