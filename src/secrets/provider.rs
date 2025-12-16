//! SecretsProvider trait and related types for secrets management integration.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Parameters for generating Terraform data source
pub struct DataSourceParams<'a> {
    /// Name of the input (used to generate unique data source names)
    pub input_name: &'a str,
    /// Secret identifier (ARN for AWS, path for Vault)
    pub secret_id: &'a str,
    /// Manager-specific configuration
    pub config: &'a HashMap<String, Value>,
    /// Optional: specific key within the secret (for JSON secrets)
    pub secret_key: Option<&'a str>,
}

/// Result from generating a Terraform data source
#[derive(Debug, Clone)]
pub struct DataSourceResult {
    /// The HCL code for the data source
    pub hcl: String,
    /// Name of the generated data source (e.g., "secret_database_password")
    pub data_source_name: String,
    /// Terraform expression to access the secret value
    /// e.g., "data.vault_generic_secret.secret_database_password.data[\"value\"]"
    pub output_expression: String,
}

/// Information about a secret manager for display purposes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretManagerInfo {
    /// Unique name of the manager
    pub name: String,
    /// Type of the manager (e.g., "vault", "aws_secrets_manager")
    pub manager_type: String,
    /// Human-readable description
    pub description: String,
}

/// Trait for secrets provider implementations.
///
/// Each provider (Vault, AWS Secrets Manager, etc.) implements this trait
/// to handle configuration validation and Terraform data source generation.
pub trait SecretsProvider: Send + Sync {
    /// Get the provider type name (e.g., "vault", "aws_secrets_manager")
    fn get_type(&self) -> &str;

    /// Get human-readable description for display
    fn get_description(&self) -> &str;

    /// Validate manager configuration.
    ///
    /// Called when loading infrastructure configuration to ensure
    /// required fields are present.
    fn validate_config(&self, config: &HashMap<String, Value>) -> Result<()>;

    /// Validate a secret ID format.
    ///
    /// Called during user input to provide early feedback on invalid secret IDs.
    fn validate_secret_id(&self, secret_id: &str) -> Result<()>;

    /// Generate Terraform data source HCL for fetching the secret.
    ///
    /// Returns the HCL code and metadata about the generated data source.
    fn generate_data_source(&self, params: &DataSourceParams) -> Result<DataSourceResult>;

    /// Get the prompt text for secret ID input.
    ///
    /// Displayed to the user when asking for the secret identifier.
    fn get_secret_id_prompt(&self) -> &str;

    /// Get example secret ID for help text.
    ///
    /// Shown as a hint to the user for the expected format.
    fn get_secret_id_example(&self) -> &str;

    /// Generate provider block HCL if needed.
    ///
    /// Some providers (like Vault) need a provider block configuration.
    /// Returns None if no provider block is needed.
    fn generate_provider_block(&self, config: &HashMap<String, Value>) -> Result<Option<String>>;

    /// Get required Terraform provider configuration.
    ///
    /// Returns the provider source and version constraint for required_providers block.
    fn get_required_provider(&self) -> RequiredProvider;
}

/// Required provider configuration for Terraform
#[derive(Debug, Clone)]
pub struct RequiredProvider {
    /// Provider name (e.g., "vault", "aws")
    pub name: String,
    /// Provider source (e.g., "hashicorp/vault")
    pub source: String,
    /// Version constraint (e.g., "~> 3.0")
    pub version: String,
}

/// Sanitize a name for use in Terraform identifiers.
///
/// Converts to lowercase and replaces non-alphanumeric characters with underscores.
pub fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_name() {
        assert_eq!(sanitize_name("database_password"), "database_password");
        assert_eq!(sanitize_name("Database-Password"), "database_password");
        assert_eq!(sanitize_name("my.secret.name"), "my_secret_name");
        assert_eq!(sanitize_name("SECRET_123"), "secret_123");
    }
}
