use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::analyzer::{ProviderInfo, ResourceInfo, StateAnalysis};

/// Parses Terraform/OpenTofu state files
pub struct StateParser {
    state_path: PathBuf,
}

impl StateParser {
    pub fn new(state_path: &Path) -> Self {
        Self {
            state_path: state_path.to_path_buf(),
        }
    }

    /// Parse the state file
    pub fn parse(&self) -> Result<StateAnalysis> {
        // Read state file
        let content = fs::read_to_string(&self.state_path)
            .with_context(|| format!("Failed to read state file: {}", self.state_path.display()))?;

        // Parse JSON
        let state: Value =
            serde_json::from_str(&content).context("Failed to parse state file as JSON")?;

        // Extract resources
        let resources = self.extract_resources(&state)?;

        // Extract providers
        let providers = self.extract_providers(&state, &resources);

        // Extract outputs
        let outputs = self.extract_outputs(&state);

        Ok(StateAnalysis {
            resources,
            providers,
            outputs,
        })
    }

    /// Extract resources from state
    fn extract_resources(&self, state: &Value) -> Result<Vec<ResourceInfo>> {
        let mut resources = Vec::new();

        // Get resources array from state
        if let Some(state_resources) = state.get("resources").and_then(|r| r.as_array()) {
            for resource in state_resources {
                // Get resource type and name
                let resource_type = resource
                    .get("type")
                    .and_then(|t| t.as_str())
                    .unwrap_or("unknown");
                let resource_name = resource
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown");

                // Get provider
                let provider = resource
                    .get("provider")
                    .and_then(|p| p.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                // Extract provider name from full provider string
                // e.g., "provider[\"registry.terraform.io/hashicorp/aws\"]" -> "aws"
                let provider_name = self.extract_provider_name(&provider);

                // Get instances
                if let Some(instances) = resource.get("instances").and_then(|i| i.as_array()) {
                    for (idx, instance) in instances.iter().enumerate() {
                        // Build resource address
                        let address = if instances.len() == 1 {
                            format!("{}.{}", resource_type, resource_name)
                        } else {
                            format!("{}.{}[{}]", resource_type, resource_name, idx)
                        };

                        // Get attributes
                        let mut attributes = HashMap::new();

                        if let Some(attrs) = instance.get("attributes").and_then(|a| a.as_object())
                        {
                            for (key, value) in attrs {
                                attributes.insert(key.clone(), value.clone());
                            }
                        }

                        resources.push(ResourceInfo {
                            address,
                            resource_type: resource_type.to_string(),
                            provider: provider_name.clone(),
                            attributes,
                        });
                    }
                }
            }
        }

        Ok(resources)
    }

    /// Extract provider name from full provider string
    fn extract_provider_name(&self, provider: &str) -> String {
        // Extract from: provider["registry.terraform.io/hashicorp/aws"]
        if let Some(start) = provider.rfind('/') {
            let name = &provider[start + 1..];

            if let Some(end) = name.find(']') {
                return name[..end].trim_matches('"').to_string();
            }

            if let Some(end) = name.find('"') {
                return name[..end].to_string();
            }

            return name.trim_matches('"').to_string();
        }

        provider.to_string()
    }

    /// Extract providers from state and resources
    fn extract_providers(&self, _state: &Value, resources: &[ResourceInfo]) -> Vec<ProviderInfo> {
        let mut providers_map: HashMap<String, ProviderInfo> = HashMap::new();

        // Collect unique providers from resources
        for resource in resources {
            providers_map
                .entry(resource.provider.clone())
                .or_insert_with(|| ProviderInfo {
                    name: resource.provider.clone(),
                    version: None,
                });
        }

        providers_map.into_values().collect()
    }

    /// Extract outputs from state
    fn extract_outputs(&self, state: &Value) -> HashMap<String, Value> {
        let mut outputs = HashMap::new();

        if let Some(state_outputs) = state.get("outputs").and_then(|o| o.as_object()) {
            for (key, value) in state_outputs {
                if let Some(output_value) = value.get("value") {
                    outputs.insert(key.clone(), output_value.clone());
                }
            }
        }

        outputs
    }

    /// Get Terraform version from state
    pub fn get_terraform_version(&self, state: &Value) -> Option<String> {
        state
            .get("terraform_version")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Get state format version
    pub fn get_format_version(&self, state: &Value) -> Option<u64> {
        state.get("version").and_then(|v| v.as_u64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_provider_name() {
        let parser = StateParser::new(Path::new("test.tfstate"));

        assert_eq!(
            parser.extract_provider_name("provider[\"registry.terraform.io/hashicorp/aws\"]"),
            "aws"
        );
        assert_eq!(
            parser.extract_provider_name("provider[\"registry.terraform.io/hashicorp/random\"]"),
            "random"
        );
        assert_eq!(parser.extract_provider_name("aws"), "aws");
    }
}
