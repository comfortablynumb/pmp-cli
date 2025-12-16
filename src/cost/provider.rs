use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Cost estimation result for a single resource
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostResource {
    pub name: String,
    pub resource_type: String,
    pub monthly_cost: f64,
    pub hourly_cost: Option<f64>,
    pub metadata: HashMap<String, String>,
}

/// Cost breakdown for a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdown {
    pub project_name: String,
    pub environment: String,
    pub currency: String,
    pub monthly_cost: f64,
    pub hourly_cost: Option<f64>,
    pub resources: Vec<CostResource>,
}

/// Cost estimation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostEstimate {
    pub breakdown: CostBreakdown,
    pub warnings: Vec<String>,
}

/// Cost difference between current and planned state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostDiff {
    pub current_monthly: f64,
    pub planned_monthly: f64,
    pub diff_monthly: f64,
    pub diff_percentage: f64,
    pub resources_added: Vec<CostResource>,
    pub resources_removed: Vec<CostResource>,
    pub resources_changed: Vec<CostResourceChange>,
}

/// Represents a change in a resource's cost
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostResourceChange {
    pub name: String,
    pub resource_type: String,
    pub previous_monthly: f64,
    pub new_monthly: f64,
    pub diff_monthly: f64,
}

/// Trait for cost estimation providers (enables future alternatives like OpenInfraQuote)
pub trait CostProvider: Send + Sync {
    /// Check if the provider is installed and available
    fn check_installed(&self) -> Result<bool>;

    /// Get the name of this provider
    fn get_name(&self) -> &str;

    /// Estimate costs for a Terraform/OpenTofu directory
    fn estimate(&self, working_dir: &Path) -> Result<CostEstimate>;

    /// Compare costs between current state and plan
    fn diff(&self, working_dir: &Path, plan_file: Option<&Path>) -> Result<CostDiff>;

    /// Generate detailed cost report in specified format
    fn report(&self, working_dir: &Path, format: &str) -> Result<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cost_resource_serialization() {
        let resource = CostResource {
            name: "aws_instance.web".to_string(),
            resource_type: "aws_instance".to_string(),
            monthly_cost: 73.0,
            hourly_cost: Some(0.1),
            metadata: HashMap::new(),
        };

        let json = serde_json::to_string(&resource).unwrap();
        let deserialized: CostResource = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, "aws_instance.web");
        assert_eq!(deserialized.monthly_cost, 73.0);
    }

    #[test]
    fn test_cost_breakdown_serialization() {
        let breakdown = CostBreakdown {
            project_name: "my-project".to_string(),
            environment: "production".to_string(),
            currency: "USD".to_string(),
            monthly_cost: 150.0,
            hourly_cost: Some(0.2),
            resources: vec![],
        };

        let json = serde_json::to_string(&breakdown).unwrap();
        let deserialized: CostBreakdown = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.project_name, "my-project");
        assert_eq!(deserialized.currency, "USD");
    }

    #[test]
    fn test_cost_diff_serialization() {
        let diff = CostDiff {
            current_monthly: 100.0,
            planned_monthly: 150.0,
            diff_monthly: 50.0,
            diff_percentage: 50.0,
            resources_added: vec![],
            resources_removed: vec![],
            resources_changed: vec![],
        };

        let json = serde_json::to_string(&diff).unwrap();
        let deserialized: CostDiff = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.diff_monthly, 50.0);
        assert_eq!(deserialized.diff_percentage, 50.0);
    }

    #[test]
    fn test_cost_estimate_with_warnings() {
        let estimate = CostEstimate {
            breakdown: CostBreakdown {
                project_name: "test".to_string(),
                environment: "dev".to_string(),
                currency: "USD".to_string(),
                monthly_cost: 100.0,
                hourly_cost: None,
                resources: vec![],
            },
            warnings: vec!["Some resources could not be estimated".to_string()],
        };

        assert_eq!(estimate.warnings.len(), 1);
    }
}
