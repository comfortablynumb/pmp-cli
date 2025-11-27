use super::executor::{Executor, ExecutorConfig, ProjectMetadata};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once};

// Global state for CTRL+C handling - shared across all executions
lazy_static::lazy_static! {
    static ref CHILD_PROCESS: Arc<Mutex<Option<Child>>> = Arc::new(Mutex::new(None));
    static ref INTERRUPTED: AtomicBool = AtomicBool::new(false);
}
static HANDLER_INIT: Once = Once::new();

/// OpenTofu executor implementation
pub struct OpenTofuExecutor;

impl OpenTofuExecutor {
    pub fn new() -> Self {
        Self
    }

    /// Initialize the CTRL+C handler (only runs once per process)
    fn init_signal_handler() {
        HANDLER_INIT.call_once(|| {
            let _ = ctrlc::set_handler(move || {
                INTERRUPTED.store(true, Ordering::SeqCst);
                if let Ok(mut child_guard) = CHILD_PROCESS.lock() {
                    if let Some(child) = child_guard.as_mut() {
                        // Kill the child process
                        let _ = child.kill();
                    }
                }
                std::process::exit(130); // Standard exit code for SIGINT
            });
        });
    }

    /// Execute a command with proper signal handling to kill child processes on CTRL+C
    fn execute_with_signal_handling(
        &self,
        command: &str,
        args: &[&str],
        working_dir: &str,
    ) -> Result<()> {
        // Initialize handler if not already done
        Self::init_signal_handler();

        // Reset interrupted flag for this execution
        INTERRUPTED.store(false, Ordering::SeqCst);

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
            let mut child_guard = CHILD_PROCESS.lock().unwrap();
            *child_guard = Some(child);
        }

        // Wait for the child to complete
        let status = {
            let mut child_guard = CHILD_PROCESS.lock().unwrap();
            if let Some(ref mut c) = *child_guard {
                c.wait().context("Failed to wait for child process")?
            } else {
                anyhow::bail!("Child process handle lost");
            }
        };

        // Clear the child process handle
        {
            let mut child_guard = CHILD_PROCESS.lock().unwrap();
            *child_guard = None;
        }

        // Check if we were interrupted
        if INTERRUPTED.load(Ordering::SeqCst) {
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
        let result = Command::new("tofu").arg("--version").output();

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

    fn plan(
        &self,
        config: &ExecutorConfig,
        working_dir: &str,
        extra_args: &[String],
    ) -> Result<()> {
        let command = config
            .plan_command
            .as_deref()
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

    fn apply(
        &self,
        config: &ExecutorConfig,
        working_dir: &str,
        extra_args: &[String],
    ) -> Result<()> {
        let command = config
            .apply_command
            .as_deref()
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

    fn destroy(
        &self,
        config: &ExecutorConfig,
        working_dir: &str,
        extra_args: &[String],
    ) -> Result<()> {
        let command = config
            .destroy_command
            .as_deref()
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

    fn refresh(
        &self,
        config: &ExecutorConfig,
        working_dir: &str,
        extra_args: &[String],
    ) -> Result<()> {
        let command = config
            .refresh_command
            .as_deref()
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
        ctx: &crate::context::Context,
        environment_path: &Path,
        executor_config: &HashMap<String, serde_json::Value>,
        project_metadata: &ProjectMetadata,
        plugins: Option<&[crate::template::metadata::AddedPlugin]>,
        template_reference_projects: &[crate::template::metadata::TemplateReferenceProject],
    ) -> Result<()> {
        use super::backend::{
            generate_backend_config, generate_data_source_backends, generate_module_blocks,
            generate_plugin_override_variables, generate_template_data_source_backends,
        };

        ctx.output
            .dimmed("  Generating _common.tf with backend configuration...");

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
        let template_data_sources_hcl =
            generate_template_data_source_backends(template_reference_projects, executor_config)
                .context("Failed to generate template data source backends")?;

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

        ctx.output
            .dimmed(&format!("  Created: {}", common_tf_path.display()));

        Ok(())
    }

    fn file_extension(&self) -> &str {
        ".tf"
    }
}

impl Default for OpenTofuExecutor {
    fn default() -> Self {
        Self::new()
    }
}

// Additional methods for OpenTofuExecutor (not part of the Executor trait)
impl OpenTofuExecutor {
    /// Run plan and capture output for drift detection
    /// Uses -detailed-exitcode which returns:
    /// - 0: no changes
    /// - 1: error
    /// - 2: changes detected
    pub fn plan_with_output(
        &self,
        working_dir: &str,
        extra_args: &[&str],
    ) -> Result<Output> {
        let mut args = vec!["plan", "-detailed-exitcode", "-no-color"];
        args.extend(extra_args);

        let output = Command::new("tofu")
            .args(&args)
            .current_dir(working_dir)
            .output()
            .context("Failed to execute tofu plan command")?;

        Ok(output)
    }
}
