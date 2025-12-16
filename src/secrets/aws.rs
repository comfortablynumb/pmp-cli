//! AWS Secrets Manager provider implementation.

use super::provider::{
    sanitize_name, DataSourceParams, DataSourceResult, RequiredProvider, SecretsProvider,
};
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

/// AWS Secrets Manager provider.
///
/// Generates `aws_secretsmanager_secret_version` data sources for fetching secrets at apply time.
pub struct AwsSecretsManagerProvider;

impl AwsSecretsManagerProvider {
    /// Create a new AWS Secrets Manager provider.
    pub fn new() -> Self {
        Self
    }
}

impl Default for AwsSecretsManagerProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretsProvider for AwsSecretsManagerProvider {
    fn get_type(&self) -> &str {
        "aws_secrets_manager"
    }

    fn get_description(&self) -> &str {
        "AWS Secrets Manager"
    }

    fn validate_config(&self, _config: &HashMap<String, Value>) -> Result<()> {
        // AWS provider config is optional - can use default credentials
        Ok(())
    }

    fn validate_secret_id(&self, secret_id: &str) -> Result<()> {
        if secret_id.is_empty() {
            anyhow::bail!("AWS secret ARN or name cannot be empty");
        }

        // If it starts with "arn:", validate basic ARN format
        if secret_id.starts_with("arn:") {
            if !secret_id.contains(":secretsmanager:") {
                anyhow::bail!(
                    "Invalid Secrets Manager ARN format. Expected: arn:aws:secretsmanager:..."
                );
            }
        }

        Ok(())
    }

    fn generate_data_source(&self, params: &DataSourceParams) -> Result<DataSourceResult> {
        let data_source_name = format!("secret_{}", sanitize_name(params.input_name));

        let mut hcl = String::new();
        hcl.push_str(&format!(
            "data \"aws_secretsmanager_secret_version\" \"{}\" {{\n",
            data_source_name
        ));
        hcl.push_str(&format!("  secret_id = \"{}\"\n", params.secret_id));
        hcl.push_str("}\n");

        let output_expression = if let Some(key) = params.secret_key {
            format!(
                "jsondecode(data.aws_secretsmanager_secret_version.{}.secret_string)[\"{}\"]",
                data_source_name, key
            )
        } else {
            format!(
                "data.aws_secretsmanager_secret_version.{}.secret_string",
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
        "AWS Secrets Manager ARN or secret name"
    }

    fn get_secret_id_example(&self) -> &str {
        "arn:aws:secretsmanager:us-east-1:123456789012:secret:myapp/db-password-AbCdEf"
    }

    fn generate_provider_block(&self, config: &HashMap<String, Value>) -> Result<Option<String>> {
        if config.is_empty() {
            return Ok(None);
        }

        let mut hcl = String::new();
        hcl.push_str("provider \"aws\" {\n");

        if let Some(region) = config.get("region").and_then(|v| v.as_str()) {
            hcl.push_str(&format!("  region = \"{}\"\n", region));
        }

        if let Some(profile) = config.get("profile").and_then(|v| v.as_str()) {
            hcl.push_str(&format!("  profile = \"{}\"\n", profile));
        }

        hcl.push_str("}\n");

        Ok(Some(hcl))
    }

    fn get_required_provider(&self) -> RequiredProvider {
        RequiredProvider {
            name: "aws".to_string(),
            source: "hashicorp/aws".to_string(),
            version: "~> 5.0".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_type() {
        let provider = AwsSecretsManagerProvider::new();
        assert_eq!(provider.get_type(), "aws_secrets_manager");
    }

    #[test]
    fn test_validate_empty_config() {
        let provider = AwsSecretsManagerProvider::new();
        let config = HashMap::new();
        assert!(provider.validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_secret_id_empty() {
        let provider = AwsSecretsManagerProvider::new();
        assert!(provider.validate_secret_id("").is_err());
    }

    #[test]
    fn test_validate_secret_id_name() {
        let provider = AwsSecretsManagerProvider::new();
        assert!(provider.validate_secret_id("myapp/db-password").is_ok());
    }

    #[test]
    fn test_validate_secret_id_valid_arn() {
        let provider = AwsSecretsManagerProvider::new();
        let arn = "arn:aws:secretsmanager:us-east-1:123456789012:secret:myapp/db-AbCdEf";
        assert!(provider.validate_secret_id(arn).is_ok());
    }

    #[test]
    fn test_validate_secret_id_invalid_arn() {
        let provider = AwsSecretsManagerProvider::new();
        // S3 ARN, not Secrets Manager
        let arn = "arn:aws:s3:::my-bucket";
        assert!(provider.validate_secret_id(arn).is_err());
    }

    #[test]
    fn test_generate_data_source() {
        let provider = AwsSecretsManagerProvider::new();
        let config = HashMap::new();
        let params = DataSourceParams {
            input_name: "database_password",
            secret_id: "arn:aws:secretsmanager:us-east-1:123456789012:secret:db-pass",
            config: &config,
            secret_key: None,
        };

        let result = provider.generate_data_source(&params).unwrap();
        assert_eq!(result.data_source_name, "secret_database_password");
        assert!(result.hcl.contains("aws_secretsmanager_secret_version"));
        assert!(result.output_expression.contains("secret_string"));
    }

    #[test]
    fn test_generate_data_source_with_key() {
        let provider = AwsSecretsManagerProvider::new();
        let config = HashMap::new();
        let params = DataSourceParams {
            input_name: "db_pass",
            secret_id: "myapp/db-password",
            config: &config,
            secret_key: Some("password"),
        };

        let result = provider.generate_data_source(&params).unwrap();
        assert!(result.output_expression.contains("jsondecode"));
        assert!(result.output_expression.contains("[\"password\"]"));
    }

    #[test]
    fn test_generate_provider_block() {
        let provider = AwsSecretsManagerProvider::new();
        let mut config = HashMap::new();
        config.insert(
            "region".to_string(),
            Value::String("us-west-2".to_string()),
        );

        let result = provider.generate_provider_block(&config).unwrap();
        assert!(result.is_some());

        let hcl = result.unwrap();
        assert!(hcl.contains("provider \"aws\""));
        assert!(hcl.contains("region = \"us-west-2\""));
    }

    #[test]
    fn test_get_required_provider() {
        let provider = AwsSecretsManagerProvider::new();
        let req = provider.get_required_provider();
        assert_eq!(req.name, "aws");
        assert_eq!(req.source, "hashicorp/aws");
    }
}
