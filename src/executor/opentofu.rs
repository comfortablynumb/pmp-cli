use super::executor::{Executor, ExecutorConfig, ProjectMetadata};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
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

    fn plan(&self, config: &ExecutorConfig, working_dir: &str, extra_args: &[String]) -> Result<()> {
        let command = config
            .plan_command.as_deref()
            .unwrap_or(self.default_plan_command());

        // Parse the command string into command and args
        let parts: Vec<&str> = command.split_whitespace().collect();

        if parts.is_empty() {
            anyhow::bail!("Empty command provided");
        }

        // Combine command args with extra args
        let mut all_args: Vec<&str> = parts[1..].to_vec();
        let extra_args_refs: Vec<&str> = extra_args.iter().map(|s| s.as_str()).collect();
        all_args.extend(extra_args_refs);

        // Execute with signal handling
        self.execute_with_signal_handling(parts[0], &all_args, working_dir)?;

        Ok(())
    }

    fn apply(&self, config: &ExecutorConfig, working_dir: &str, extra_args: &[String]) -> Result<()> {
        let command = config
            .apply_command.as_deref()
            .unwrap_or(self.default_apply_command());

        // Parse the command string into command and args
        let parts: Vec<&str> = command.split_whitespace().collect();

        if parts.is_empty() {
            anyhow::bail!("Empty command provided");
        }

        // Combine command args with extra args
        let mut all_args: Vec<&str> = parts[1..].to_vec();
        let extra_args_refs: Vec<&str> = extra_args.iter().map(|s| s.as_str()).collect();
        all_args.extend(extra_args_refs);

        // Execute with signal handling
        self.execute_with_signal_handling(parts[0], &all_args, working_dir)?;

        Ok(())
    }

    fn destroy(&self, config: &ExecutorConfig, working_dir: &str, extra_args: &[String]) -> Result<()> {
        let command = config
            .destroy_command.as_deref()
            .unwrap_or(self.default_destroy_command());

        // Parse the command string into command and args
        let parts: Vec<&str> = command.split_whitespace().collect();

        if parts.is_empty() {
            anyhow::bail!("Empty command provided");
        }

        // Combine command args with extra args
        let mut all_args: Vec<&str> = parts[1..].to_vec();
        let extra_args_refs: Vec<&str> = extra_args.iter().map(|s| s.as_str()).collect();
        all_args.extend(extra_args_refs);

        // Execute with signal handling
        self.execute_with_signal_handling(parts[0], &all_args, working_dir)?;

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

    fn default_destroy_command(&self) -> &str {
        "tofu destroy"
    }

    fn refresh(&self, config: &ExecutorConfig, working_dir: &str, extra_args: &[String]) -> Result<()> {
        let command = config
            .refresh_command.as_deref()
            .unwrap_or(self.default_refresh_command());

        // Parse the command string into command and args
        let parts: Vec<&str> = command.split_whitespace().collect();

        if parts.is_empty() {
            anyhow::bail!("Empty command provided");
        }

        // Combine command args with extra args
        let mut all_args: Vec<&str> = parts[1..].to_vec();
        let extra_args_refs: Vec<&str> = extra_args.iter().map(|s| s.as_str()).collect();
        all_args.extend(extra_args_refs);

        // Execute with signal handling
        self.execute_with_signal_handling(parts[0], &all_args, working_dir)?;

        Ok(())
    }

    fn default_refresh_command(&self) -> &str {
        "tofu refresh"
    }

    fn generate_common_file(
        &self,
        environment_path: &Path,
        executor_config: &HashMap<String, serde_json::Value>,
        project_metadata: &ProjectMetadata,
        plugins: Option<&[crate::template::metadata::AddedPlugin]>,
        template_reference_projects: &[crate::template::metadata::TemplateReferenceProject],
    ) -> Result<()> {
        use super::backend::{generate_backend_config, generate_module_blocks, generate_data_source_backends, generate_plugin_override_variables, generate_template_data_source_backends};

        // Generate backend HCL with project metadata for table name generation
        let backend_hcl = generate_backend_config(
            executor_config,
            Some(project_metadata.api_version),
            Some(project_metadata.kind),
            Some(project_metadata.environment),
            Some(project_metadata.project_name),
        )
        .context("Failed to generate backend configuration")?;

        // Generate data source backends for template reference projects
        let template_data_sources_hcl = generate_template_data_source_backends(
            template_reference_projects,
            executor_config,
        ).context("Failed to generate template data source backends")?;

        // Generate data source backends for plugins with reference projects
        let plugin_data_sources_hcl = if let Some(plugin_list) = plugins {
            generate_data_source_backends(plugin_list, executor_config)
                .context("Failed to generate plugin data source backends")?
        } else {
            String::new()
        };

        // Generate plugin override variables
        let variables_hcl = if let Some(plugin_list) = plugins {
            generate_plugin_override_variables(plugin_list)
        } else {
            String::new()
        };

        // Generate module blocks for plugins
        let modules_hcl = if let Some(plugin_list) = plugins {
            generate_module_blocks(plugin_list)
        } else {
            String::new()
        };

        // Combine backend, data sources, variables, and modules
        let mut combined_hcl = backend_hcl;
        if !template_data_sources_hcl.is_empty() {
            combined_hcl.push_str(&template_data_sources_hcl);
        }
        if !plugin_data_sources_hcl.is_empty() {
            combined_hcl.push_str(&plugin_data_sources_hcl);
        }
        if !variables_hcl.is_empty() {
            combined_hcl.push_str(&variables_hcl);
        }
        if !modules_hcl.is_empty() {
            combined_hcl.push_str(&modules_hcl);
        }

        if combined_hcl.is_empty() {
            // No backend config, data sources, variables, or modules to write
            return Ok(());
        }

        // Write to _common.tf file
        let common_tf_path = environment_path.join("_common.tf");
        std::fs::write(&common_tf_path, combined_hcl)
            .with_context(|| format!("Failed to write _common.tf file: {:?}", common_tf_path))?;

        crate::output::dimmed(&format!("  Created: {}", common_tf_path.display()));

        Ok(())
    }

    fn file_extension(&self) -> &str {
        ".tf"
    }

    fn supports_backend(&self) -> bool {
        true
    }
}

impl Default for OpenTofuExecutor {
    fn default() -> Self {
        Self::new()
    }
}
