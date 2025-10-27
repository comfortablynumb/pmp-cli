use super::executor::{Executor, ExecutorConfig};
use anyhow::{Context, Result};
use std::process::{Command, Output, Stdio, Child};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

/// OpenTofu executor implementation
pub struct OpenTofuExecutor;

impl OpenTofuExecutor {
    pub fn new() -> Self {
        Self
    }

    /// Execute a command with proper signal handling to kill child processes on CTRL+C
    fn execute_with_signal_handling(
        &self,
        command: &str,
        args: &[&str],
        working_dir: &str,
    ) -> Result<()> {
        // Set up shared state for the child process
        let child_process: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
        let child_clone = Arc::clone(&child_process);
        let interrupted = Arc::new(AtomicBool::new(false));
        let interrupted_clone = Arc::clone(&interrupted);

        // Set up CTRL+C handler
        ctrlc::set_handler(move || {
            interrupted_clone.store(true, Ordering::SeqCst);
            if let Ok(mut child_guard) = child_clone.lock() {
                if let Some(child) = child_guard.as_mut() {
                    // Kill the child process
                    let _ = child.kill();
                }
            }
            std::process::exit(130); // Standard exit code for SIGINT
        })
        .context("Failed to set CTRL+C handler")?;

        // Spawn the child process
        let child = Command::new(command)
            .args(args)
            .current_dir(working_dir)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .context("Failed to spawn child process")?;

        // Store the child process handle
        {
            let mut child_guard = child_process.lock().unwrap();
            *child_guard = Some(child);
        }

        // Wait for the child to complete
        let status = {
            let mut child_guard = child_process.lock().unwrap();
            if let Some(ref mut c) = *child_guard {
                c.wait().context("Failed to wait for child process")?
            } else {
                anyhow::bail!("Child process handle lost");
            }
        };

        // Clear the child process handle
        {
            let mut child_guard = child_process.lock().unwrap();
            *child_guard = None;
        }

        // Check if we were interrupted
        if interrupted.load(Ordering::SeqCst) {
            anyhow::bail!("Command interrupted by user");
        }

        // Check exit status
        if !status.success() {
            anyhow::bail!("Command failed with exit code: {:?}", status.code());
        }

        Ok(())
    }
}

impl Executor for OpenTofuExecutor {
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

    fn init(&self, working_dir: &str) -> Result<Output> {
        let output = Command::new("tofu")
            .arg("init")
            .current_dir(working_dir)
            .output()
            .context("Failed to execute tofu init command")?;

        Ok(output)
    }

    fn plan(&self, config: &ExecutorConfig, working_dir: &str) -> Result<()> {
        let command = config
            .plan_command.as_deref()
            .unwrap_or(self.default_plan_command());

        // Parse the command string into command and args
        let parts: Vec<&str> = command.split_whitespace().collect();

        if parts.is_empty() {
            anyhow::bail!("Empty command provided");
        }

        // Execute with signal handling
        self.execute_with_signal_handling(parts[0], &parts[1..], working_dir)?;

        Ok(())
    }

    fn apply(&self, config: &ExecutorConfig, working_dir: &str) -> Result<()> {
        let command = config
            .apply_command.as_deref()
            .unwrap_or(self.default_apply_command());

        // Parse the command string into command and args
        let parts: Vec<&str> = command.split_whitespace().collect();

        if parts.is_empty() {
            anyhow::bail!("Empty command provided");
        }

        // Execute with signal handling
        self.execute_with_signal_handling(parts[0], &parts[1..], working_dir)?;

        Ok(())
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
