use super::provider::{
    CostBreakdown, CostDiff, CostEstimate, CostProvider, CostResource, CostResourceChange,
};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Output};

/// Infracost JSON output structure for breakdown command
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InfracostOutput {
    #[allow(dead_code)]
    version: Option<String>,
    currency: Option<String>,
    projects: Vec<InfracostProject>,
    total_monthly_cost: Option<String>,
    #[allow(dead_code)]
    total_hourly_cost: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InfracostProject {
    name: Option<String>,
    breakdown: Option<InfracostBreakdown>,
    diff: Option<InfracostProjectDiff>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InfracostBreakdown {
    resources: Option<Vec<InfracostResource>>,
    total_monthly_cost: Option<String>,
    total_hourly_cost: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InfracostResource {
    name: String,
    resource_type: Option<String>,
    monthly_cost: Option<String>,
    hourly_cost: Option<String>,
    #[allow(dead_code)]
    metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Infracost diff output structure
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InfracostDiffOutput {
    #[allow(dead_code)]
    version: Option<String>,
    #[allow(dead_code)]
    currency: Option<String>,
    projects: Vec<InfracostProject>,
    total_monthly_cost: Option<String>,
    diff_total_monthly_cost: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InfracostProjectDiff {
    resources: Option<Vec<InfracostDiffResource>>,
    #[allow(dead_code)]
    total_monthly_cost: Option<String>,
    #[allow(dead_code)]
    total_hourly_cost: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InfracostDiffResource {
    name: String,
    resource_type: Option<String>,
    monthly_cost: Option<String>,
    #[allow(dead_code)]
    hourly_cost: Option<String>,
}

/// Infracost provider implementation
pub struct InfracostProvider {
    api_key_env: Option<String>,
}

impl InfracostProvider {
    pub fn new() -> Self {
        Self { api_key_env: None }
    }

    pub fn with_api_key_env(api_key_env: &str) -> Self {
        Self {
            api_key_env: Some(api_key_env.to_string()),
        }
    }

    fn run_infracost(&self, args: &[&str], working_dir: &Path) -> Result<Output> {
        let mut cmd = Command::new("infracost");
        cmd.args(args).current_dir(working_dir);

        if let Some(ref env_var) = self.api_key_env {
            if let Ok(key) = std::env::var(env_var) {
                cmd.env("INFRACOST_API_KEY", key);
            }
        }

        cmd.output().context("Failed to execute infracost command")
    }

    fn parse_cost_string(cost_str: &Option<String>) -> f64 {
        cost_str
            .as_ref()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0)
    }

    fn convert_resource(resource: &InfracostResource) -> CostResource {
        CostResource {
            name: resource.name.clone(),
            resource_type: resource.resource_type.clone().unwrap_or_default(),
            monthly_cost: Self::parse_cost_string(&resource.monthly_cost),
            hourly_cost: resource
                .hourly_cost
                .as_ref()
                .and_then(|s| s.parse().ok()),
            metadata: HashMap::new(),
        }
    }
}

impl Default for InfracostProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl CostProvider for InfracostProvider {
    fn check_installed(&self) -> Result<bool> {
        let result = Command::new("infracost").arg("--version").output();

        match result {
            Ok(output) => Ok(output.status.success()),
            Err(_) => Ok(false),
        }
    }

    fn get_name(&self) -> &str {
        "infracost"
    }

    fn estimate(&self, working_dir: &Path) -> Result<CostEstimate> {
        let output = self.run_infracost(
            &["breakdown", "--path", ".", "--format", "json"],
            working_dir,
        )?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Infracost failed: {}", stderr);
        }

        let infracost_output: InfracostOutput = serde_json::from_slice(&output.stdout)
            .context("Failed to parse Infracost JSON output")?;

        let project = infracost_output.projects.first();
        let breakdown_data = project.and_then(|p| p.breakdown.as_ref());

        let resources: Vec<CostResource> = breakdown_data
            .and_then(|b| b.resources.as_ref())
            .map(|resources| resources.iter().map(Self::convert_resource).collect())
            .unwrap_or_default();

        let monthly_cost = breakdown_data
            .map(|b| Self::parse_cost_string(&b.total_monthly_cost))
            .unwrap_or_else(|| Self::parse_cost_string(&infracost_output.total_monthly_cost));

        let hourly_cost = breakdown_data
            .and_then(|b| b.total_hourly_cost.as_ref())
            .and_then(|s| s.parse().ok());

        let breakdown = CostBreakdown {
            project_name: project
                .and_then(|p| p.name.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            environment: String::new(),
            currency: infracost_output.currency.unwrap_or_else(|| "USD".to_string()),
            monthly_cost,
            hourly_cost,
            resources,
        };

        Ok(CostEstimate {
            breakdown,
            warnings: vec![],
        })
    }

    fn diff(&self, working_dir: &Path, plan_file: Option<&Path>) -> Result<CostDiff> {
        let args: Vec<&str> = if let Some(plan) = plan_file {
            vec![
                "diff",
                "--path",
                plan.to_str().unwrap_or("."),
                "--format",
                "json",
            ]
        } else {
            vec!["diff", "--path", ".", "--format", "json"]
        };

        let output = self.run_infracost(&args, working_dir)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Infracost diff failed: {}", stderr);
        }

        let diff_output: InfracostDiffOutput = serde_json::from_slice(&output.stdout)
            .context("Failed to parse Infracost diff JSON output")?;

        let total_monthly = Self::parse_cost_string(&diff_output.total_monthly_cost);
        let diff_monthly = Self::parse_cost_string(&diff_output.diff_total_monthly_cost);

        let current_monthly = total_monthly - diff_monthly;
        let diff_percentage = if current_monthly > 0.0 {
            (diff_monthly / current_monthly) * 100.0
        } else if diff_monthly > 0.0 {
            100.0
        } else {
            0.0
        };

        let mut resources_added = Vec::new();
        let mut resources_changed = Vec::new();

        for project in &diff_output.projects {
            if let Some(ref diff) = project.diff {
                if let Some(ref resources) = diff.resources {
                    for resource in resources {
                        let cost = Self::parse_cost_string(&resource.monthly_cost);

                        if cost > 0.0 {
                            resources_added.push(CostResource {
                                name: resource.name.clone(),
                                resource_type: resource.resource_type.clone().unwrap_or_default(),
                                monthly_cost: cost,
                                hourly_cost: None,
                                metadata: HashMap::new(),
                            });
                        } else if cost != 0.0 {
                            resources_changed.push(CostResourceChange {
                                name: resource.name.clone(),
                                resource_type: resource.resource_type.clone().unwrap_or_default(),
                                previous_monthly: 0.0,
                                new_monthly: cost.abs(),
                                diff_monthly: cost,
                            });
                        }
                    }
                }
            }
        }

        Ok(CostDiff {
            current_monthly,
            planned_monthly: total_monthly,
            diff_monthly,
            diff_percentage,
            resources_added,
            resources_removed: vec![],
            resources_changed,
        })
    }

    fn report(&self, working_dir: &Path, format: &str) -> Result<String> {
        let output = self.run_infracost(
            &["breakdown", "--path", ".", "--format", format],
            working_dir,
        )?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Infracost report failed: {}", stderr);
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cost_string() {
        assert_eq!(InfracostProvider::parse_cost_string(&Some("100.50".to_string())), 100.50);
        assert_eq!(InfracostProvider::parse_cost_string(&Some("0".to_string())), 0.0);
        assert_eq!(InfracostProvider::parse_cost_string(&None), 0.0);
        assert_eq!(InfracostProvider::parse_cost_string(&Some("invalid".to_string())), 0.0);
    }

    #[test]
    fn test_parse_infracost_breakdown_output() {
        let json = r#"{
            "version": "0.2",
            "currency": "USD",
            "projects": [{
                "name": "test-project",
                "breakdown": {
                    "resources": [
                        {
                            "name": "aws_instance.web",
                            "resourceType": "aws_instance",
                            "monthlyCost": "73.00",
                            "hourlyCost": "0.10"
                        }
                    ],
                    "totalMonthlyCost": "73.00",
                    "totalHourlyCost": "0.10"
                }
            }],
            "totalMonthlyCost": "73.00"
        }"#;

        let output: InfracostOutput = serde_json::from_str(json).unwrap();

        assert_eq!(output.currency, Some("USD".to_string()));
        assert_eq!(output.projects.len(), 1);

        let project = &output.projects[0];
        assert_eq!(project.name, Some("test-project".to_string()));

        let breakdown = project.breakdown.as_ref().unwrap();
        let resources = breakdown.resources.as_ref().unwrap();
        assert_eq!(resources.len(), 1);
        assert_eq!(resources[0].name, "aws_instance.web");
    }

    #[test]
    fn test_parse_infracost_diff_output() {
        let json = r#"{
            "version": "0.2",
            "currency": "USD",
            "projects": [{
                "name": "test-project",
                "diff": {
                    "resources": [
                        {
                            "name": "aws_instance.new",
                            "resourceType": "aws_instance",
                            "monthlyCost": "50.00"
                        }
                    ],
                    "totalMonthlyCost": "50.00"
                }
            }],
            "totalMonthlyCost": "150.00",
            "diffTotalMonthlyCost": "50.00"
        }"#;

        let output: InfracostDiffOutput = serde_json::from_str(json).unwrap();

        assert_eq!(output.total_monthly_cost, Some("150.00".to_string()));
        assert_eq!(output.diff_total_monthly_cost, Some("50.00".to_string()));
    }

    #[test]
    fn test_convert_resource() {
        let infracost_resource = InfracostResource {
            name: "aws_instance.test".to_string(),
            resource_type: Some("aws_instance".to_string()),
            monthly_cost: Some("100.00".to_string()),
            hourly_cost: Some("0.14".to_string()),
            metadata: None,
        };

        let cost_resource = InfracostProvider::convert_resource(&infracost_resource);

        assert_eq!(cost_resource.name, "aws_instance.test");
        assert_eq!(cost_resource.resource_type, "aws_instance");
        assert_eq!(cost_resource.monthly_cost, 100.0);
        assert_eq!(cost_resource.hourly_cost, Some(0.14));
    }

    #[test]
    fn test_provider_name() {
        let provider = InfracostProvider::new();
        assert_eq!(provider.get_name(), "infracost");
    }

    #[test]
    fn test_provider_with_api_key() {
        let provider = InfracostProvider::with_api_key_env("MY_API_KEY");
        assert_eq!(provider.api_key_env, Some("MY_API_KEY".to_string()));
    }
}
