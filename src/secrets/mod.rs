//! Secrets management integration for PMP.
//!
//! This module provides integration with external secret managers (HashiCorp Vault, AWS Secrets Manager)
//! allowing templates to reference secrets that are fetched at Terraform apply time via native data sources.

pub mod provider;
mod registry;
mod vault;
mod aws;

pub use provider::{
    DataSourceParams, DataSourceResult, SecretsProvider, sanitize_name,
};
pub use registry::SecretsProviderRegistry;
pub use vault::VaultProvider;
pub use aws::AwsSecretsManagerProvider;
