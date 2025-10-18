use super::executor::{IacConfig, IacExecutor};
use anyhow::{Context, Result};
use std::process::{Command, Output};

/// OpenTofu executor implementation
pub struct OpenTofuExecutor;

impl OpenTofuExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl IacExecutor for OpenTofuExecutor {
    fn check_installed(&self) -> Result<bool> {
        // Try to run 'tofu --version' to check if OpenTofu is installed
        let result = Command::new("tofu")
            .arg("--version")
            .output();

        match result {
            Ok(output) => Ok(output.status.success()),
            Err(_) => Ok(false), // Command not found or failed to execute
        }
    }

    fn plan(&self, config: &IacConfig, working_dir: &str) -> Result<Output> {
        let command = config
            .plan_command
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or(self.default_plan_command());

        // Parse the command string into command and args
        let parts: Vec<&str> = command.split_whitespace().collect();

        if parts.is_empty() {
            anyhow::bail!("Empty command provided");
        }

        let output = Command::new(parts[0])
            .args(&parts[1..])
            .current_dir(working_dir)
            .output()
            .context("Failed to execute plan command")?;

        Ok(output)
    }

    fn apply(&self, config: &IacConfig, working_dir: &str) -> Result<Output> {
        let command = config
            .apply_command
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or(self.default_apply_command());

        // Parse the command string into command and args
        let parts: Vec<&str> = command.split_whitespace().collect();

        if parts.is_empty() {
            anyhow::bail!("Empty command provided");
        }

        let output = Command::new(parts[0])
            .args(&parts[1..])
            .current_dir(working_dir)
            .output()
            .context("Failed to execute apply command")?;

        Ok(output)
    }

    fn get_name(&self) -> &str {
        "opentofu"
    }

    fn default_plan_command(&self) -> &str {
        "tofu plan"
    }

    fn default_apply_command(&self) -> &str {
        "tofu apply"
    }
}

impl Default for OpenTofuExecutor {
    fn default() -> Self {
        Self::new()
    }
}
