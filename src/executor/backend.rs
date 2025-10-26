use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

/// Generate _common.tf content with backend configuration
pub fn generate_backend_config(executor_config: &HashMap<String, Value>) -> Result<String> {
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

    // Add all backend parameters except 'type'
    let mut params: Vec<_> = backend_config
        .iter()
        .filter(|(key, _)| *key != "type")
        .collect();

    // Sort parameters for consistent output
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

        let result = generate_backend_config(&config).unwrap();

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

        let result = generate_backend_config(&config).unwrap();

        assert!(result.contains("backend \"azurerm\" {"));
        assert!(result.contains("storage_account_name = \"mystorageaccount\""));
        assert!(result.contains("container_name = \"tfstate\""));
    }

    #[test]
    fn test_no_backend_config() {
        let config = HashMap::new();
        let result = generate_backend_config(&config).unwrap();
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

        let result = generate_backend_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_escape_hcl_string() {
        assert_eq!(escape_hcl_string("simple"), "simple");
        assert_eq!(escape_hcl_string("with\"quotes"), "with\\\"quotes");
        assert_eq!(escape_hcl_string("with\\backslash"), "with\\\\backslash");
        assert_eq!(escape_hcl_string("with\nnewline"), "with\\nnewline");
    }
}
