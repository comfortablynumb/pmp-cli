//! HashiCorp Vault secrets provider implementation.

use super::provider::{
    sanitize_name, DataSourceParams, DataSourceResult, RequiredProvider, SecretsProvider,
};
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

/// HashiCorp Vault secrets provider.
///
/// Generates `vault_generic_secret` data sources for fetching secrets at apply time.
pub struct VaultProvider;

impl VaultProvider {
    /// Create a new Vault provider.
    pub fn new() -> Self {
        Self
    }
}

impl Default for VaultProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretsProvider for VaultProvider {
    fn get_type(&self) -> &str {
        "vault"
    }

    fn get_description(&self) -> &str {
        "HashiCorp Vault"
    }

    fn validate_config(&self, config: &HashMap<String, Value>) -> Result<()> {
        // For static config, address is required
        // For project-based config, this will be empty and validated separately
        if config.is_empty() {
            return Ok(());
        }

        if !config.contains_key("address") {
            anyhow::bail!("Vault configuration requires 'address' field");
        }

        Ok(())
    }

    fn validate_secret_id(&self, secret_id: &str) -> Result<()> {
        if secret_id.is_empty() {
            anyhow::bail!("Vault secret path cannot be empty");
        }

        if secret_id.contains("..") {
            anyhow::bail!("Vault secret path cannot contain '..'");
        }

        Ok(())
    }

    fn generate_data_source(&self, params: &DataSourceParams) -> Result<DataSourceResult> {
        let data_source_name = format!("secret_{}", sanitize_name(params.input_name));

        let mut hcl = String::new();
        hcl.push_str(&format!(
            "data \"vault_generic_secret\" \"{}\" {{\n",
            data_source_name
        ));
        hcl.push_str(&format!("  path = \"{}\"\n", params.secret_id));
        hcl.push_str("}\n");

        let output_expression = if let Some(key) = params.secret_key {
            format!(
                "data.vault_generic_secret.{}.data[\"{}\"]",
                data_source_name, key
            )
        } else {
            format!(
                "data.vault_generic_secret.{}.data[\"value\"]",
                data_source_name
            )
        };

        Ok(DataSourceResult {
            hcl,
            data_source_name,
            output_expression,
        })
    }

    fn get_secret_id_prompt(&self) -> &str {
        "Vault secret path"
    }

    fn get_secret_id_example(&self) -> &str {
        "secret/data/myapp/database"
    }

    fn generate_provider_block(&self, config: &HashMap<String, Value>) -> Result<Option<String>> {
        if config.is_empty() {
            return Ok(None);
        }

        let mut hcl = String::new();
        hcl.push_str("provider \"vault\" {\n");

        if let Some(address) = config.get("address").and_then(|v| v.as_str()) {
            hcl.push_str(&format!("  address = \"{}\"\n", address));
        }

        if let Some(namespace) = config.get("namespace").and_then(|v| v.as_str()) {
            hcl.push_str(&format!("  namespace = \"{}\"\n", namespace));
        }

        if let Some(token_env) = config.get("token_env").and_then(|v| v.as_str()) {
            hcl.push_str(&format!(
                "  token = \"${{env:{}}}\"\n",
                token_env
            ));
        }

        hcl.push_str("}\n");

        Ok(Some(hcl))
    }

    fn get_required_provider(&self) -> RequiredProvider {
        RequiredProvider {
            name: "vault".to_string(),
            source: "hashicorp/vault".to_string(),
            version: "~> 4.0".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_type() {
        let provider = VaultProvider::new();
        assert_eq!(provider.get_type(), "vault");
    }

    #[test]
    fn test_validate_empty_config() {
        let provider = VaultProvider::new();
        let config = HashMap::new();
        assert!(provider.validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_config_without_address() {
        let provider = VaultProvider::new();
        let mut config = HashMap::new();
        config.insert("namespace".to_string(), Value::String("test".to_string()));
        assert!(provider.validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_config_with_address() {
        let provider = VaultProvider::new();
        let mut config = HashMap::new();
        config.insert(
            "address".to_string(),
            Value::String("https://vault.example.com".to_string()),
        );
        assert!(provider.validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_secret_id_empty() {
        let provider = VaultProvider::new();
        assert!(provider.validate_secret_id("").is_err());
    }

    #[test]
    fn test_validate_secret_id_valid() {
        let provider = VaultProvider::new();
        assert!(provider.validate_secret_id("secret/data/myapp/db").is_ok());
    }

    #[test]
    fn test_validate_secret_id_path_traversal() {
        let provider = VaultProvider::new();
        assert!(provider.validate_secret_id("secret/../other").is_err());
    }

    #[test]
    fn test_generate_data_source() {
        let provider = VaultProvider::new();
        let config = HashMap::new();
        let params = DataSourceParams {
            input_name: "database_password",
            secret_id: "secret/data/myapp/db",
            config: &config,
            secret_key: None,
        };

        let result = provider.generate_data_source(&params).unwrap();
        assert_eq!(result.data_source_name, "secret_database_password");
        assert!(result.hcl.contains("vault_generic_secret"));
        assert!(result.hcl.contains("secret/data/myapp/db"));
        assert!(result.output_expression.contains("data.vault_generic_secret"));
    }

    #[test]
    fn test_generate_data_source_with_key() {
        let provider = VaultProvider::new();
        let config = HashMap::new();
        let params = DataSourceParams {
            input_name: "db_pass",
            secret_id: "secret/data/myapp/db",
            config: &config,
            secret_key: Some("password"),
        };

        let result = provider.generate_data_source(&params).unwrap();
        assert!(result.output_expression.contains("[\"password\"]"));
    }

    #[test]
    fn test_generate_provider_block() {
        let provider = VaultProvider::new();
        let mut config = HashMap::new();
        config.insert(
            "address".to_string(),
            Value::String("https://vault.example.com".to_string()),
        );
        config.insert(
            "namespace".to_string(),
            Value::String("production".to_string()),
        );

        let result = provider.generate_provider_block(&config).unwrap();
        assert!(result.is_some());

        let hcl = result.unwrap();
        assert!(hcl.contains("provider \"vault\""));
        assert!(hcl.contains("address = \"https://vault.example.com\""));
        assert!(hcl.contains("namespace = \"production\""));
    }

    #[test]
    fn test_get_required_provider() {
        let provider = VaultProvider::new();
        let req = provider.get_required_provider();
        assert_eq!(req.name, "vault");
        assert_eq!(req.source, "hashicorp/vault");
    }
}
