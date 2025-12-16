use std::collections::HashSet;

use super::discovery::Provider;

/// Registry for tracking available providers
///
/// Note: Since InfrastructureDiscovery trait uses async methods, we cannot use
/// dynamic dispatch (dyn). Instead, this registry tracks which providers are
/// available and concrete provider types are used directly.
#[derive(Default)]
#[allow(dead_code)]
pub struct ProviderRegistry {
    available_providers: HashSet<Provider>,
}

#[allow(dead_code)]
impl ProviderRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            available_providers: HashSet::new(),
        }
    }

    /// Register a provider as available
    pub fn register(&mut self, provider: Provider) -> &mut Self {
        self.available_providers.insert(provider);
        self
    }

    /// Check if a provider is registered
    pub fn has_provider(&self, provider: Provider) -> bool {
        self.available_providers.contains(&provider)
    }

    /// Get all registered providers
    pub fn registered_providers(&self) -> Vec<Provider> {
        self.available_providers.iter().cloned().collect()
    }

    /// Get the count of registered providers
    pub fn len(&self) -> usize {
        self.available_providers.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.available_providers.is_empty()
    }

    /// Create a registry with all supported providers
    pub fn with_all_providers() -> Self {
        let mut registry = Self::new();
        registry.register(Provider::Aws);
        registry.register(Provider::Azure);
        registry.register(Provider::Gcp);
        registry
    }
}

/// Builder for creating a configured provider registry
#[allow(dead_code)]
pub struct ProviderRegistryBuilder {
    registry: ProviderRegistry,
}

#[allow(dead_code)]
impl ProviderRegistryBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            registry: ProviderRegistry::new(),
        }
    }

    /// Add a provider to the registry
    pub fn with_provider(mut self, provider: Provider) -> Self {
        self.registry.register(provider);
        self
    }

    /// Build the registry
    pub fn build(self) -> ProviderRegistry {
        self.registry
    }
}

impl Default for ProviderRegistryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_register_and_check() {
        let mut registry = ProviderRegistry::new();
        registry.register(Provider::Aws);

        assert!(registry.has_provider(Provider::Aws));
        assert!(!registry.has_provider(Provider::Azure));
    }

    #[test]
    fn test_registry_builder() {
        let registry = ProviderRegistryBuilder::new()
            .with_provider(Provider::Aws)
            .with_provider(Provider::Azure)
            .build();

        assert_eq!(registry.len(), 2);
        assert!(registry.has_provider(Provider::Aws));
        assert!(registry.has_provider(Provider::Azure));
    }

    #[test]
    fn test_registry_with_all_providers() {
        let registry = ProviderRegistry::with_all_providers();

        assert!(registry.has_provider(Provider::Aws));
        assert!(registry.has_provider(Provider::Azure));
        assert!(registry.has_provider(Provider::Gcp));
    }
}
