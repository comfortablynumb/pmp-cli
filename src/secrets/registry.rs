//! Provider registry for secrets management.
//!
//! Provides a central registry for looking up secrets providers by type.

use super::provider::SecretsProvider;
use super::{AwsSecretsManagerProvider, VaultProvider};
use std::collections::HashMap;
use std::sync::Arc;

/// Registry of available secrets providers.
///
/// Provides lookup of providers by type name (e.g., "vault", "aws_secrets_manager").
pub struct SecretsProviderRegistry {
    providers: HashMap<String, Arc<dyn SecretsProvider>>,
}

impl SecretsProviderRegistry {
    /// Create a new registry with all built-in providers.
    pub fn new() -> Self {
        let mut providers: HashMap<String, Arc<dyn SecretsProvider>> = HashMap::new();
        providers.insert("vault".to_string(), Arc::new(VaultProvider::new()));
        providers.insert(
            "aws_secrets_manager".to_string(),
            Arc::new(AwsSecretsManagerProvider::new()),
        );
        Self { providers }
    }

    /// Get a provider by type name.
    ///
    /// Returns None if no provider is registered for the given type.
    pub fn get(&self, provider_type: &str) -> Option<Arc<dyn SecretsProvider>> {
        self.providers.get(provider_type).cloned()
    }

    /// Get list of supported provider types.
    pub fn supported_types(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a provider type is supported.
    pub fn is_supported(&self, provider_type: &str) -> bool {
        self.providers.contains_key(provider_type)
    }
}

impl Default for SecretsProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_vault() {
        let registry = SecretsProviderRegistry::new();
        assert!(registry.is_supported("vault"));

        let provider = registry.get("vault");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().get_type(), "vault");
    }

    #[test]
    fn test_registry_has_aws() {
        let registry = SecretsProviderRegistry::new();
        assert!(registry.is_supported("aws_secrets_manager"));

        let provider = registry.get("aws_secrets_manager");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().get_type(), "aws_secrets_manager");
    }

    #[test]
    fn test_registry_unknown_type() {
        let registry = SecretsProviderRegistry::new();
        assert!(!registry.is_supported("unknown"));
        assert!(registry.get("unknown").is_none());
    }

    #[test]
    fn test_supported_types() {
        let registry = SecretsProviderRegistry::new();
        let types = registry.supported_types();
        assert!(types.contains(&"vault"));
        assert!(types.contains(&"aws_secrets_manager"));
    }
}
