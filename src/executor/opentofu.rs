use super::executor::{Executor, ExecutorConfig, ProjectMetadata};
use crate::template::metadata::AddedPlugin;
use anyhow::{Context, Result};
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::collections::HashMap;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Once};

// ============================================================================
// Backend Configuration Functions
// ============================================================================

/// Calculate a unique table name for PostgreSQL backend based on project metadata
/// Format: tf_{sha1_hex_lowercase}
/// Input string: {apiVersion}_{kind}__{environment}__{project_name}
fn calculate_table_name(
    api_version: &str,
    kind: &str,
    environment: &str,
    project_name: &str,
) -> String {
    // Create the input string for hashing
    let input = format!(
        "{}_{}__{}__{}",
        api_version, kind, environment, project_name
    );

    // Calculate SHA1 hash
    let mut hasher = Sha1::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();

    // Convert to lowercase hex string and prepend "tf_"
    format!("tf_{:x}", result)
}

/// Generate _common.tf content with backend configuration
///
/// For PostgreSQL backends, if project metadata is provided, a unique table_name
/// will be automatically generated based on apiVersion, kind, environment, and project name.
pub fn generate_backend_config(
    executor_config: &HashMap<String, Value>,
    api_version: Option<&str>,
    kind: Option<&str>,
    environment: Option<&str>,
    project_name: Option<&str>,
) -> Result<String> {
    // Check if backend configuration exists
    let backend_config = match executor_config.get("backend") {
        Some(Value::Object(map)) => map,
        Some(_) => anyhow::bail!("Backend configuration must be an object"),
        None => return Ok(String::new()), // No backend config, return empty
    };

    // Extract backend type
    let backend_type = match backend_config.get("type") {
        Some(Value::String(t)) => t,
        Some(_) => anyhow::bail!("Backend type must be a string"),
        None => anyhow::bail!("Backend configuration must specify a 'type' field"),
    };

    // Validate backend type is supported
    validate_backend_type(backend_type)?;

    // Generate HCL content
    let mut hcl = String::new();
    hcl.push_str("# Auto-generated backend configuration from project collection\n");
    hcl.push_str("# Do not edit manually - changes will be overwritten\n\n");
    hcl.push_str("terraform {\n");
    hcl.push_str(&format!("  backend \"{}\" {{\n", backend_type));

    // Collect backend parameters
    let mut params_map: HashMap<String, Value> = backend_config
        .iter()
        .filter(|(key, _)| *key != "type")
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    // For PostgreSQL backend, auto-inject table_name if project metadata is provided
    if backend_type == "pg"
        && let (Some(api_ver), Some(knd), Some(env), Some(proj)) =
            (api_version, kind, environment, project_name)
    {
        let table_name = calculate_table_name(api_ver, knd, env, proj);
        params_map.insert("table_name".to_string(), Value::String(table_name));
    }

    // Sort parameters for consistent output
    let mut params: Vec<_> = params_map.iter().collect();
    params.sort_by_key(|(key, _)| *key);

    for (key, value) in params {
        let param_line = format_hcl_parameter(key, value)?;
        hcl.push_str(&format!("    {}\n", param_line));
    }

    hcl.push_str("  }\n");
    hcl.push_str("}\n");

    Ok(hcl)
}

/// Validate that the backend type is supported by OpenTofu
fn validate_backend_type(backend_type: &str) -> Result<()> {
    const SUPPORTED_BACKENDS: &[&str] = &[
        "local",
        "s3",
        "azurerm",
        "gcs",
        "http",
        "kubernetes",
        "pg",
        "consul",
        "cos",
        "oss",
        "remote",
    ];

    if !SUPPORTED_BACKENDS.contains(&backend_type) {
        anyhow::bail!(
            "Unsupported backend type '{}'. Supported backends: {}",
            backend_type,
            SUPPORTED_BACKENDS.join(", ")
        );
    }

    Ok(())
}

/// Format a single HCL parameter based on its value type
fn format_hcl_parameter(key: &str, value: &Value) -> Result<String> {
    match value {
        Value::String(s) => Ok(format!("{} = \"{}\"", key, escape_hcl_string(s))),
        Value::Number(n) => Ok(format!("{} = {}", key, n)),
        Value::Bool(b) => Ok(format!("{} = {}", key, b)),
        Value::Array(arr) => {
            let items: Result<Vec<String>> = arr
                .iter()
                .map(|v| match v {
                    Value::String(s) => Ok(format!("\"{}\"", escape_hcl_string(s))),
                    Value::Number(n) => Ok(n.to_string()),
                    Value::Bool(b) => Ok(b.to_string()),
                    _ => anyhow::bail!("Unsupported array element type in backend config"),
                })
                .collect();
            Ok(format!("{} = [{}]", key, items?.join(", ")))
        }
        Value::Object(obj) => {
            // For nested objects, format as HCL blocks
            let mut items = Vec::new();
            for (k, v) in obj {
                items.push(format!("{} = {}", k, format_hcl_value(v)?));
            }
            Ok(format!("{} = {{ {} }}", key, items.join(", ")))
        }
        Value::Null => Ok(format!("{} = null", key)),
    }
}

/// Format a value for HCL (helper function)
fn format_hcl_value(value: &Value) -> Result<String> {
    match value {
        Value::String(s) => Ok(format!("\"{}\"", escape_hcl_string(s))),
        Value::Number(n) => Ok(n.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Array(arr) => {
            let items: Result<Vec<String>> = arr.iter().map(format_hcl_value).collect();
            Ok(format!("[{}]", items?.join(", ")))
        }
        Value::Object(obj) => {
            let items: Result<Vec<String>> = obj
                .iter()
                .map(|(k, v)| Ok(format!("{} = {}", k, format_hcl_value(v)?)))
                .collect();
            Ok(format!("{{ {} }}", items?.join(", ")))
        }
        Value::Null => Ok("null".to_string()),
    }
}

/// Escape special characters in HCL strings
fn escape_hcl_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Generate Terraform variables for plugin override environment variables
///
/// Creates variables that allow runtime overrides of remote state outputs via environment variables.
/// Variables are named: pmp_plugin_{pack}_{plugin}_{ref_project}_{field}
/// Environment variables are named: TF_VAR_pmp_plugin_{pack}_{plugin}_{ref_project}_{field}
///
/// # Arguments
/// * `plugins` - Slice of AddedPlugin structs containing plugin metadata and specs
///
/// # Returns
/// HCL string containing variable declarations, or empty string if no variables needed
pub fn generate_plugin_override_variables(plugins: &[AddedPlugin]) -> String {
    if plugins.is_empty() {
        return String::new();
    }

    let mut hcl = String::new();
    let mut has_variables = false;

    for plugin in plugins {
        // Get plugin spec from stored data
        let plugin_spec = match &plugin.plugin_spec {
            Some(spec) => spec,
            None => continue, // Skip if plugin spec not available
        };

        // Process ALL reference projects for this plugin
        for plugin_ref in &plugin.reference_projects {
            // Find matching dependency in plugin spec by data_source_name
            let dependency = plugin_spec.dependencies.iter().find(|dep| {
                dep.project
                    .remote_state
                    .as_ref()
                    .map(|rs| rs.data_source_name == plugin_ref.data_source_name)
                    .unwrap_or(false)
            });

            let remote_state = match dependency {
                Some(dep) => match &dep.project.remote_state {
                    Some(rs) => rs,
                    None => continue,
                },
                None => continue,
            };

            // Generate variables for each required field
            for field_name in remote_state.required_fields.keys() {
                let var_name = format!(
                    "pmp_plugin_{}_{}_{}_{}_{}",
                    plugin.template_pack_name.to_lowercase(),
                    plugin.name.to_lowercase(),
                    plugin_ref.data_source_name.to_lowercase(), // NEW: uniqueness
                    plugin_ref.name.to_lowercase(),
                    field_name.to_lowercase()
                );

            if !has_variables {
                hcl.push_str(
                    "\n# Plugin override variables (set via TF_VAR_* environment variables)\n",
                );
                has_variables = true;
            }

                hcl.push_str(&format!("variable \"{}\" {{\n", var_name));
                hcl.push_str("  type    = string\n");
                hcl.push_str("  default = null\n");
                hcl.push_str(&format!(
                    "  description = \"Override for plugin_{}_{}_{}.outputs.{} (env: TF_VAR_{})\"\n",
                    plugin.template_pack_name,
                    plugin.name,
                    plugin_ref.data_source_name,
                    field_name,
                    var_name
                ));
                hcl.push_str("}\n\n");
            }
        }
    }

    hcl
}

/// Generate module blocks for added plugins
///
/// Creates Terraform module blocks that reference plugin modules in the modules/ directory.
/// Module source paths are relative to the environment directory.
/// For plugins with reference projects, passes parameters from the reference project's remote state.
///
/// # Arguments
/// * `plugins` - Slice of AddedPlugin structs containing plugin metadata and specs
///
/// # Returns
/// HCL string containing module blocks, or empty string if no plugins
pub fn generate_module_blocks(plugins: &[AddedPlugin]) -> String {
    if plugins.is_empty() {
        return String::new();
    }

    let mut hcl = String::new();
    hcl.push_str("\n# Plugin modules\n");

    for plugin in plugins {
        // Construct module name and source path based on whether plugin has dependencies
        // Only use reference project name if the plugin spec defines dependencies
        // AND the reference_projects count matches the dependencies count

        // Debug: Print plugin info
        eprintln!("DEBUG: Plugin {}/{}", plugin.template_pack_name, plugin.name);
        eprintln!("  - Has spec: {}", plugin.plugin_spec.is_some());
        if let Some(spec) = &plugin.plugin_spec {
            eprintln!("  - Dependencies count: {}", spec.dependencies.len());
        }
        eprintln!("  - Reference projects count: {}", plugin.reference_projects.len());
        for (idx, ref_proj) in plugin.reference_projects.iter().enumerate() {
            eprintln!("    [{}] name: {}", idx, ref_proj.name);
        }

        let has_valid_dependencies = plugin
            .plugin_spec
            .as_ref()
            .map(|spec| {
                !spec.dependencies.is_empty()
                    && plugin.reference_projects.len() == spec.dependencies.len()
            })
            .unwrap_or(false);

        eprintln!("  - has_valid_dependencies: {}", has_valid_dependencies);

        // Check if the first reference project name is different from the plugin name
        let should_append_ref_name = has_valid_dependencies
            && plugin.reference_projects.first()
                .map(|first_ref| first_ref.name != plugin.name)
                .unwrap_or(false);

        eprintln!("  - should_append_ref_name: {}", should_append_ref_name);

        let (module_name, source_path) = if should_append_ref_name
            && let Some(first_ref) = plugin.reference_projects.first()
        {
            // Plugin has dependencies and ref name differs - use reference project name in path
            (
                format!(
                    "{}_{}_{}",
                    plugin.template_pack_name, plugin.name, first_ref.name
                ),
                format!(
                    "./modules/{}/{}/{}",
                    plugin.template_pack_name, plugin.name, first_ref.name
                ),
            )
        } else {
            // Plugin has no dependencies OR ref name matches plugin name - no suffix needed
            (
                format!("{}_{}", plugin.template_pack_name, plugin.name),
                format!("./modules/{}/{}", plugin.template_pack_name, plugin.name),
            )
        };

        hcl.push_str(&format!("module \"{}\" {{\n", module_name));
        hcl.push_str(&format!("  source = \"{}\"\n", source_path));

        // Generate parameters from ALL reference projects
        if !plugin.reference_projects.is_empty() {
            hcl.push_str("\n  # Parameters from reference projects (with optional overrides)\n");

            // Get plugin spec from stored data
            let plugin_spec = match &plugin.plugin_spec {
                Some(spec) => spec,
                None => {
                    // Plugin spec not available - skip parameters
                    hcl.push_str("}\n\n");
                    continue;
                }
            };

            for plugin_ref in &plugin.reference_projects {
                // Find matching dependency in plugin spec
                let dependency = plugin_spec.dependencies.iter().find(|dep| {
                    dep.project
                        .remote_state
                        .as_ref()
                        .map(|rs| rs.data_source_name == plugin_ref.data_source_name)
                        .unwrap_or(false)
                });

                let remote_state = match dependency {
                    Some(dep) => match &dep.project.remote_state {
                        Some(rs) => rs,
                        None => continue,
                    },
                    None => continue,
                };

                // Terraform data source name
                let tf_data_source_name = format!(
                    "plugin_{}_{}_{}",
                    plugin.template_pack_name,
                    plugin.name,
                    plugin_ref.data_source_name
                );

                // Add comment if dependency_name exists
                if let Some(dep_name) = &plugin_ref.dependency_name {
                    hcl.push_str(&format!("  # From dependency: {}\n", dep_name));
                }

                // Generate module parameters for each required field
                for (field_name, field_config) in &remote_state.required_fields {
                    // Use alias if provided, otherwise original field name
                    let param_name = field_config
                        .alias
                        .as_ref()
                        .unwrap_or(field_name);

                    // Override variable name
                    let var_name = format!(
                        "pmp_plugin_{}_{}_{}_{}_{}",
                        plugin.template_pack_name.to_lowercase(),
                        plugin.name.to_lowercase(),
                        plugin_ref.data_source_name.to_lowercase(),
                        plugin_ref.name.to_lowercase(),
                        field_name.to_lowercase()
                    );

                    // Coalesce: env var override → remote state output
                    hcl.push_str(&format!(
                        "  {} = coalesce(var.{}, data.terraform_remote_state.{}.outputs.{})\n",
                        param_name, var_name, tf_data_source_name, field_name
                    ));
                }
            }
        }

        // Add raw module inputs if provided
        if let Some(raw_inputs) = &plugin.raw_module_inputs
            && !raw_inputs.is_empty()
        {
            hcl.push_str("\n  # Raw module inputs (HCL expressions)\n");
            for (key, expression) in raw_inputs {
                hcl.push_str(&format!("  {} = {}\n", key, expression));
            }
        }

        hcl.push_str("}\n\n");
    }

    hcl
}

/// Generate terraform_remote_state data source blocks for plugins with reference projects
///
/// # Arguments
/// * `plugins` - Slice of AddedPlugin structs containing plugin metadata
/// * `executor_config` - Executor configuration from collection (contains backend settings)
///
/// # Returns
/// HCL string containing data source blocks, or empty string if no plugins with references
pub fn generate_data_source_backends(
    plugins: &[AddedPlugin],
    executor_config: &HashMap<String, serde_json::Value>,
) -> Result<String> {
    if plugins.is_empty() {
        return Ok(String::new());
    }

    // Flatten all plugin references into (plugin, ref) pairs
    let plugin_refs: Vec<(&AddedPlugin, &crate::template::metadata::AddedPluginReference)> =
        plugins
            .iter()
            .flat_map(|plugin| {
                plugin
                    .reference_projects
                    .iter()
                    .map(move |plugin_ref| (plugin, plugin_ref))
            })
            .collect();

    if plugin_refs.is_empty() {
        return Ok(String::new());
    }

    // Deduplicate to avoid duplicate data sources
    let mut seen = std::collections::HashSet::new();
    let mut unique_refs = Vec::new();

    for (plugin, plugin_ref) in plugin_refs {
        let tf_data_source_name = format!(
            "plugin_{}_{}_{}",
            plugin.template_pack_name, plugin.name, plugin_ref.data_source_name
        );

        if seen.insert(tf_data_source_name) {
            unique_refs.push((plugin, plugin_ref));
        }
    }

    let mut hcl = String::new();
    hcl.push_str("\n# Data sources for plugin reference projects\n");

    for (plugin, plugin_ref) in unique_refs {
        let tf_data_source_name = format!(
            "plugin_{}_{}_{}",
            plugin.template_pack_name, plugin.name, plugin_ref.data_source_name
        );

        // Get backend type
        let backend_type = executor_config
            .get("backend")
            .and_then(|b| b.get("type"))
            .and_then(|t| t.as_str())
            .unwrap_or("local");

        // Generate backend config for reference project
        let backend_config_hcl = generate_backend_config_map(
            executor_config,
            Some(&plugin_ref.api_version),
            Some(&plugin_ref.kind),
            Some(&plugin_ref.environment),
            Some(&plugin_ref.name),
        )?;

        // Optional comment with dependency name
        if let Some(dep_name) = &plugin_ref.dependency_name {
            hcl.push_str(&format!("# Dependency: {}\n", dep_name));
        }

        // Generate data source block
        hcl.push_str(&format!(
            "data \"terraform_remote_state\" \"{}\" {{\n",
            tf_data_source_name
        ));
        hcl.push_str(&format!("  backend = \"{}\"\n", backend_type));

        if !backend_config_hcl.is_empty() {
            hcl.push_str("  config = {\n");
            hcl.push_str(&backend_config_hcl);
            hcl.push_str("  }\n");
        }

        hcl.push_str("}\n\n");
    }

    Ok(hcl)
}

/// Generate terraform_remote_state data source blocks for template reference projects
///
/// # Arguments
/// * `template_refs` - Slice of TemplateReferenceProject structs containing reference project metadata
/// * `executor_config` - Executor configuration from collection (contains backend settings)
///
/// # Returns
/// HCL string containing data source blocks, or empty string if no template references
pub fn generate_template_data_source_backends(
    template_refs: &[crate::template::metadata::TemplateReferenceProject],
    executor_config: &HashMap<String, serde_json::Value>,
) -> Result<String> {
    if template_refs.is_empty() {
        return Ok(String::new());
    }

    // Deduplicate template references by data_source_name
    // This handles cases where the YAML file has duplicate entries
    let mut seen = std::collections::HashSet::new();
    let unique_refs: Vec<_> = template_refs
        .iter()
        .filter(|r| seen.insert(&r.data_source_name))
        .collect();

    let mut hcl = String::new();
    hcl.push_str("\n# Data sources for template reference projects\n");

    for template_ref in unique_refs {
        // Data source name: template_ref_{data_source_name}
        let tf_data_source_name = format!("template_ref_{}", template_ref.data_source_name);

        // Get backend type from executor config
        let backend_type = executor_config
            .get("backend")
            .and_then(|b| b.get("type"))
            .and_then(|t| t.as_str())
            .unwrap_or("local");

        // Generate backend config pointing to reference project's state
        let backend_config_hcl = generate_backend_config_map(
            executor_config,
            Some(&template_ref.api_version),
            Some(&template_ref.kind),
            Some(&template_ref.environment),
            Some(&template_ref.name),
        )?;

        // Generate data source block
        hcl.push_str(&format!(
            "data \"terraform_remote_state\" \"{}\" {{\n",
            tf_data_source_name
        ));
        hcl.push_str(&format!("  backend = \"{}\"\n", backend_type));

        if !backend_config_hcl.is_empty() {
            hcl.push_str("  config = {\n");
            hcl.push_str(&backend_config_hcl);
            hcl.push_str("  }\n");
        }

        hcl.push_str("}\n\n");
    }

    Ok(hcl)
}

/// Generate backend configuration as a map (for data source config blocks)
/// Returns the config map content (without wrapping config = {})
fn generate_backend_config_map(
    executor_config: &HashMap<String, serde_json::Value>,
    api_version: Option<&str>,
    kind: Option<&str>,
    environment: Option<&str>,
    project_name: Option<&str>,
) -> Result<String> {
    let backend_config = executor_config
        .get("backend")
        .and_then(|b| b.as_object())
        .context("No backend configuration found in executor config")?;

    let backend_type = backend_config
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("local");

    // If local backend, return empty (local doesn't need config in data source)
    if backend_type == "local" {
        return Ok(String::new());
    }

    let mut config_lines = Vec::new();

    // Process each backend config parameter
    for (key, value) in backend_config.iter() {
        if key == "type" || key == "table_name" {
            continue; // Skip the type field and table_name (will be auto-injected)
        }

        // For other fields, use the value from config
        let value_str = match value {
            serde_json::Value::String(s) => format!("\"{}\"", escape_hcl_string(s)),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            _ => continue, // Skip complex types
        };

        config_lines.push(format!("    {} = {}", key, value_str));
    }

    // Auto-inject table_name for PostgreSQL backends
    if backend_type == "pg"
        && let (Some(api), Some(k), Some(env), Some(name)) =
            (api_version, kind, environment, project_name)
    {
        let table_name = calculate_table_name(api, k, env, name);
        config_lines.push(format!(
            "    table_name = \"{}\"",
            escape_hcl_string(&table_name)
        ));
    }

    Ok(config_lines.join("\n") + "\n")
}

// ============================================================================
// OpenTofu Executor Implementation
// ============================================================================

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
                if let Ok(mut child_guard) = CHILD_PROCESS.lock()
                    && let Some(child) = child_guard.as_mut()
                {
                    // Kill the child process
                    let _ = child.kill();
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

        // Combine command args with template command options and extra args
        let mut all_args: Vec<&str> = parts[1..].to_vec();

        // Add command-specific options from template configuration
        if let Some(options) = config.command_options.get("plan") {
            for opt in options {
                all_args.push(opt.as_str());
            }
        }

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

        // Combine command args with template command options and extra args
        let mut all_args: Vec<&str> = parts[1..].to_vec();

        // Add command-specific options from template configuration
        if let Some(options) = config.command_options.get("apply") {
            for opt in options {
                all_args.push(opt.as_str());
            }
        }

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

        // Combine command args with template command options and extra args
        let mut all_args: Vec<&str> = parts[1..].to_vec();

        // Add command-specific options from template configuration
        if let Some(options) = config.command_options.get("destroy") {
            for opt in options {
                all_args.push(opt.as_str());
            }
        }

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

        // Combine command args with template command options and extra args
        let mut all_args: Vec<&str> = parts[1..].to_vec();

        // Add command-specific options from template configuration
        if let Some(options) = config.command_options.get("refresh") {
            for opt in options {
                all_args.push(opt.as_str());
            }
        }

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
    pub fn plan_with_output(&self, working_dir: &str, extra_args: &[&str]) -> Result<Output> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_generate_s3_backend() {
        let config_json = json!({
            "backend": {
                "type": "s3",
                "bucket": "my-terraform-state",
                "key": "project/terraform.tfstate",
                "region": "us-west-2",
                "encrypt": true,
                "dynamodb_table": "terraform-locks"
            }
        });

        // Convert serde_json::Map to HashMap
        let mut config = HashMap::new();
        for (k, v) in config_json.as_object().unwrap() {
            config.insert(k.clone(), v.clone());
        }

        let result = generate_backend_config(&config, None, None, None, None).unwrap();

        assert!(result.contains("terraform {"));
        assert!(result.contains("backend \"s3\" {"));
        assert!(result.contains("bucket = \"my-terraform-state\""));
        assert!(result.contains("key = \"project/terraform.tfstate\""));
        assert!(result.contains("region = \"us-west-2\""));
        assert!(result.contains("encrypt = true"));
        assert!(result.contains("dynamodb_table = \"terraform-locks\""));
    }

    #[test]
    fn test_generate_azurerm_backend() {
        let config_json = json!({
            "backend": {
                "type": "azurerm",
                "storage_account_name": "mystorageaccount",
                "container_name": "tfstate",
                "key": "prod.terraform.tfstate"
            }
        });

        // Convert serde_json::Map to HashMap
        let mut config = HashMap::new();
        for (k, v) in config_json.as_object().unwrap() {
            config.insert(k.clone(), v.clone());
        }

        let result = generate_backend_config(&config, None, None, None, None).unwrap();

        assert!(result.contains("backend \"azurerm\" {"));
        assert!(result.contains("storage_account_name = \"mystorageaccount\""));
        assert!(result.contains("container_name = \"tfstate\""));
    }

    #[test]
    fn test_no_backend_config() {
        let config = HashMap::new();
        let result = generate_backend_config(&config, None, None, None, None).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_invalid_backend_type() {
        let config_json = json!({
            "backend": {
                "type": "invalid_backend"
            }
        });

        // Convert serde_json::Map to HashMap
        let mut config = HashMap::new();
        for (k, v) in config_json.as_object().unwrap() {
            config.insert(k.clone(), v.clone());
        }

        let result = generate_backend_config(&config, None, None, None, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_pg_backend_with_table_name() {
        let config_json = json!({
            "backend": {
                "type": "pg",
                "conn_str": "postgres://user:pass@localhost/db",
                "schema_name": "terraform_remote_state"
            }
        });

        // Convert serde_json::Map to HashMap
        let mut config = HashMap::new();
        for (k, v) in config_json.as_object().unwrap() {
            config.insert(k.clone(), v.clone());
        }

        let result = generate_backend_config(
            &config,
            Some("pmp.io/v1"),
            Some("Database"),
            Some("development"),
            Some("my-db"),
        )
        .unwrap();

        assert!(result.contains("backend \"pg\" {"));
        assert!(result.contains("conn_str = \"postgres://user:pass@localhost/db\""));
        assert!(result.contains("schema_name = \"terraform_remote_state\""));
        // Should contain auto-generated table_name
        assert!(result.contains("table_name = \"tf_"));
    }

    #[test]
    fn test_calculate_table_name() {
        let table_name = calculate_table_name("pmp.io/v1", "Database", "development", "my-db");

        // Should start with "tf_"
        assert!(table_name.starts_with("tf_"));

        // Should be 43 characters (tf_ + 40 char SHA1 hex)
        assert_eq!(table_name.len(), 43);

        // Should be lowercase
        assert_eq!(table_name, table_name.to_lowercase());

        // Should be deterministic
        let table_name2 = calculate_table_name("pmp.io/v1", "Database", "development", "my-db");
        assert_eq!(table_name, table_name2);

        // Different inputs should produce different table names
        let table_name3 = calculate_table_name("pmp.io/v1", "Database", "production", "my-db");
        assert_ne!(table_name, table_name3);
    }

    #[test]
    fn test_escape_hcl_string() {
        assert_eq!(escape_hcl_string("simple"), "simple");
        assert_eq!(escape_hcl_string("with\"quotes"), "with\\\"quotes");
        assert_eq!(escape_hcl_string("with\\backslash"), "with\\\\backslash");
        assert_eq!(escape_hcl_string("with\nnewline"), "with\\nnewline");
    }
}
