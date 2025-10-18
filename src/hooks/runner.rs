use anyhow::{Context, Result};
use std::process::Command;

/// Executes pre and post hooks for commands
pub struct HooksRunner;

impl HooksRunner {
    /// Execute a list of hook commands in sequence
    /// Returns an error if any hook fails
    pub fn run_hooks(hooks: &[String], working_dir: &str, hook_type: &str) -> Result<()> {
        if hooks.is_empty() {
            return Ok(());
        }

        println!("Running {} hooks...", hook_type);

        for (index, hook_command) in hooks.iter().enumerate() {
            println!("  [{}] Executing: {}", index + 1, hook_command);

            let output = Self::execute_command(hook_command, working_dir)
                .with_context(|| format!("Failed to execute {} hook: {}", hook_type, hook_command))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!(
                    "{} hook failed: {}\nError: {}",
                    hook_type,
                    hook_command,
                    stderr
                );
            }

            // Print stdout if there's any output
            let stdout = String::from_utf8_lossy(&output.stdout);

            if !stdout.trim().is_empty() {
                println!("  Output: {}", stdout.trim());
            }
        }

        println!("{} hooks completed successfully", hook_type);
        Ok(())
    }

    /// Execute a single shell command
    fn execute_command(command: &str, working_dir: &str) -> Result<std::process::Output> {
        // Use shell to execute the command for better compatibility
        #[cfg(target_os = "windows")]
        let output = Command::new("cmd")
            .args(["/C", command])
            .current_dir(working_dir)
            .output()?;

        #[cfg(not(target_os = "windows"))]
        let output = Command::new("sh")
            .args(["-c", command])
            .current_dir(working_dir)
            .output()?;

        Ok(output)
    }
}
