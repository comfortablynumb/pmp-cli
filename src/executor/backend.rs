use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use sha1::{Sha1, Digest};
use crate::template::metadata::AddedPlugin;

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
    let input = format!("{}_{}__{}__{}", api_version, kind, environment, project_name);

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
    if backend_type == "pg" {
        if let (Some(api_ver), Some(knd), Some(env), Some(proj)) =
            (api_version, kind, environment, project_name) {
            let table_name = calculate_table_name(api_ver, knd, env, proj);
            params_map.insert("table_name".to_string(), Value::String(table_name));
        }
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
            let items: Result<Vec<String>> = arr
                .iter()
                .map(|v| format_hcl_value(v))
                .collect();
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
        // Only process plugins with reference projects
        if plugin.reference_project.is_none() {
            continue;
        }

        let ref_project = plugin.reference_project.as_ref().unwrap();

        // Get plugin spec from stored data
        let plugin_spec = match &plugin.plugin_spec {
            Some(spec) => spec,
            None => continue, // Skip if plugin spec not available
        };

        // Get remote_state configuration
        let remote_state = match &plugin_spec.requires_project_with_template {
            Some(template_ref) => match &template_ref.remote_state {
                Some(rs) => rs,
                None => continue, // No remote_state config, skip
            },
            None => continue,
        };

        // Generate variables for each required field
        for (field_name, _field_config) in &remote_state.required_fields {
            let var_name = format!(
                "pmp_plugin_{}_{}_{}_{}",
                plugin.template_pack_name.to_lowercase(),
                plugin.name.to_lowercase(),
                ref_project.name.to_lowercase(),
                field_name.to_lowercase()
            );

            if !has_variables {
                hcl.push_str("\n# Plugin override variables (set via TF_VAR_* environment variables)\n");
                has_variables = true;
            }

            hcl.push_str(&format!("variable \"{}\" {{\n", var_name));
            hcl.push_str("  type    = string\n");
            hcl.push_str("  default = null\n");
            hcl.push_str(&format!(
                "  description = \"Override for {}.outputs.{} (env: TF_VAR_{})\"\n",
                ref_project.name, field_name, var_name
            ));
            hcl.push_str("}\n\n");
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
        // Include reference project name for uniqueness
        let module_name = format!("{}_{}_{}", plugin.template_pack_name, plugin.name, plugin.project.name);
        let source_path = format!("./modules/{}/{}/{}", plugin.template_pack_name, plugin.name, plugin.project.name);

        hcl.push_str(&format!("module \"{}\" {{\n", module_name));
        hcl.push_str(&format!("  source = \"{}\"\n", source_path));

        // If plugin has a reference project, pass parameters from remote state
        if let Some(ref_project) = &plugin.reference_project {
            let data_source_name = format!("plugin_{}_{}_{}",
                plugin.template_pack_name,
                plugin.name,
                ref_project.name
            );

            // Get plugin spec from stored data
            let plugin_spec = match &plugin.plugin_spec {
                Some(spec) => spec,
                None => {
                    // Plugin spec not available - skip parameters
                    hcl.push_str("}\n\n");
                    continue;
                }
            };

            // Get remote_state configuration
            let remote_state = match &plugin_spec.requires_project_with_template {
                Some(template_ref) => match &template_ref.remote_state {
                    Some(rs) => rs,
                    None => {
                        // No remote_state config - skip parameters
                        hcl.push_str("}\n\n");
                        continue;
                    }
                },
                None => {
                    hcl.push_str("}\n\n");
                    continue;
                }
            };

            hcl.push_str("\n  # Parameters from reference project (with optional overrides)\n");

            // Generate module parameters for each required field
            for (field_name, field_config) in &remote_state.required_fields {
                // Determine the parameter name (use alias if provided, otherwise use original field name)
                let param_name = match &field_config.alias {
                    Some(alias) => alias.clone(),
                    None => field_name.clone(),
                };

                // Generate variable name for override
                let var_name = format!(
                    "pmp_plugin_{}_{}_{}_{}",
                    plugin.template_pack_name.to_lowercase(),
                    plugin.name.to_lowercase(),
                    ref_project.name.to_lowercase(),
                    field_name.to_lowercase()
                );

                // Generate coalesce expression: prefer env var, fallback to remote state
                hcl.push_str(&format!(
                    "  {} = coalesce(var.{}, data.terraform_remote_state.{}.outputs.{})\n",
                    param_name, var_name, data_source_name, field_name
                ));
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

    // Filter plugins that have reference projects
    let plugins_with_refs: Vec<&AddedPlugin> = plugins
        .iter()
        .filter(|p| p.reference_project.is_some())
        .collect();

    if plugins_with_refs.is_empty() {
        return Ok(String::new());
    }

    let mut hcl = String::new();
    hcl.push_str("\n# Data sources for plugin reference projects\n");

    for plugin in plugins_with_refs {
        let reference = plugin.reference_project.as_ref().unwrap();

        // Data source name: plugin_{template_pack_name}_{plugin_name}_{reference_project_name}
        let data_source_name = format!("plugin_{}_{}_{}",
            plugin.template_pack_name,
            plugin.name,
            reference.name
        );

        // Get backend type from executor config
        let backend_type = executor_config
            .get("backend")
            .and_then(|b| b.get("type"))
            .and_then(|t| t.as_str())
            .unwrap_or("local");

        // Generate backend config pointing to reference project's state
        let backend_config_hcl = generate_backend_config_map(
            executor_config,
            Some(&reference.api_version),
            Some(&reference.kind),
            Some(&reference.environment),
            Some(&reference.name),
        )?;

        // Generate data source block
        hcl.push_str(&format!("data \"terraform_remote_state\" \"{}\" {{\n", data_source_name));
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

    let mut hcl = String::new();
    hcl.push_str("\n# Data sources for template reference projects\n");

    for template_ref in template_refs {
        // Data source name: template_ref_{data_source_name}
        let data_source_name = format!("template_ref_{}", template_ref.data_source_name);

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
        hcl.push_str(&format!("data \"terraform_remote_state\" \"{}\" {{\n", data_source_name));
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
    if backend_type == "pg" {
        if let (Some(api), Some(k), Some(env), Some(name)) = (api_version, kind, environment, project_name) {
            let table_name = calculate_table_name(api, k, env, name);
            config_lines.push(format!("    table_name = \"{}\"", escape_hcl_string(&table_name)));
        }
    }

    Ok(config_lines.join("\n") + "\n")
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
            Some("my-db")
        ).unwrap();

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
