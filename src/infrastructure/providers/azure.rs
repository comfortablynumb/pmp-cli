use crate::infrastructure::discovery::{InfrastructureDiscovery, Provider};

/// Azure infrastructure discovery using Azure SDK
///
/// Discovers resources from Azure subscriptions using the Azure SDK.
/// Requires valid Azure credentials configured via:
/// - Environment variables (AZURE_CLIENT_ID, AZURE_CLIENT_SECRET, AZURE_TENANT_ID)
/// - Azure CLI authentication (az login)
/// - Managed Identity (for Azure VMs, AKS, etc.)
#[allow(dead_code)]
pub struct AzureDiscovery {
    subscription_id: String,
}

#[allow(dead_code)]
impl AzureDiscovery {
    /// Create a new Azure discovery instance for a subscription
    pub fn new(subscription_id: impl Into<String>) -> Self {
        Self {
            subscription_id: subscription_id.into(),
        }
    }

    /// Get the configured subscription ID
    pub fn subscription_id(&self) -> &str {
        &self.subscription_id
    }
}

impl InfrastructureDiscovery for AzureDiscovery {
    fn provider(&self) -> Provider {
        Provider::Azure
    }

    fn supported_resource_types(&self) -> Vec<&'static str> {
        vec![
            // Resource Group
            "azurerm_resource_group",
            // Networking
            "azurerm_virtual_network",
            "azurerm_subnet",
            "azurerm_network_security_group",
            "azurerm_public_ip",
            "azurerm_network_interface",
            "azurerm_lb",
            // Compute
            "azurerm_linux_virtual_machine",
            "azurerm_windows_virtual_machine",
            // Storage
            "azurerm_storage_account",
            // Database
            "azurerm_sql_server",
            "azurerm_sql_database",
            "azurerm_postgresql_server",
            // AKS
            "azurerm_kubernetes_cluster",
            // App Service
            "azurerm_app_service_plan",
            "azurerm_app_service",
            // Key Vault
            "azurerm_key_vault",
        ]
    }
}

#[allow(dead_code)]
impl AzureDiscovery {
    /// List available Azure regions
    pub fn list_regions(&self) -> Vec<String> {
        vec![
            "eastus".to_string(),
            "eastus2".to_string(),
            "westus".to_string(),
            "westus2".to_string(),
            "westus3".to_string(),
            "centralus".to_string(),
            "northcentralus".to_string(),
            "southcentralus".to_string(),
            "northeurope".to_string(),
            "westeurope".to_string(),
            "uksouth".to_string(),
            "ukwest".to_string(),
            "francecentral".to_string(),
            "germanywestcentral".to_string(),
            "southeastasia".to_string(),
            "eastasia".to_string(),
            "japaneast".to_string(),
            "japanwest".to_string(),
            "australiaeast".to_string(),
            "australiasoutheast".to_string(),
            "brazilsouth".to_string(),
            "canadacentral".to_string(),
            "canadaeast".to_string(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_azure_discovery_provider() {
        let discovery = AzureDiscovery::new("00000000-0000-0000-0000-000000000000");
        assert_eq!(discovery.provider(), Provider::Azure);
    }

    #[test]
    fn test_azure_supported_types() {
        let discovery = AzureDiscovery::new("sub-123");
        let types = discovery.supported_resource_types();

        assert!(types.contains(&"azurerm_virtual_network"));
        assert!(types.contains(&"azurerm_linux_virtual_machine"));
        assert!(types.contains(&"azurerm_storage_account"));
    }
}
