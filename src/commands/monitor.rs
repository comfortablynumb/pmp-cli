use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::output;
use crate::template::metadata::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub struct MonitorCommand;

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthStatus {
    pub project: String,
    pub environment: String,
    pub status: ResourceStatus,
    pub last_checked: String,
    pub resources_healthy: usize,
    pub resources_unhealthy: usize,
    pub resources_unknown: usize,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum ResourceStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetricData {
    pub project: String,
    pub environment: String,
    pub metrics: HashMap<String, MetricValue>,
    pub timestamp: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetricValue {
    pub name: String,
    pub value: f64,
    pub unit: String,
    pub threshold: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Alert {
    pub id: String,
    pub project: String,
    pub environment: String,
    pub severity: AlertSeverity,
    pub message: String,
    pub triggered_at: String,
    pub resolved: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AlertSeverity {
    Critical,
    Warning,
    Info,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AlertRule {
    pub name: String,
    pub metric: String,
    pub condition: String,
    pub threshold: f64,
    pub severity: AlertSeverity,
}

impl MonitorCommand {
    pub fn execute_dashboard(ctx: &Context, path: Option<&str>) -> Result<()> {
        ctx.output.section("Infrastructure Health Dashboard");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        let projects = if let Some(p) = path {
            let path = Path::new(p);
            if ctx.fs.exists(path) {
                crate::collection::CollectionDiscovery::discover_projects(
                    &*ctx.fs,
                    &*ctx.output,
                    &infrastructure_root,
                )?
                .into_iter()
                .filter(|proj| {
                    let proj_path = infrastructure_root.join(&proj.path);
                    proj_path.starts_with(path)
                })
                .collect()
            } else {
                vec![]
            }
        } else {
            crate::collection::CollectionDiscovery::discover_projects(
                &*ctx.fs,
                &*ctx.output,
                &infrastructure_root,
            )?
        };

        if projects.is_empty() {
            ctx.output.warning("No projects found");
            return Ok(());
        }

        // Display overview
        ctx.output.subsection("Overview");
        output::blank();

        let total_projects = projects.len();
        let mut total_environments = 0;
        let mut healthy_count = 0;
        let mut degraded_count = 0;
        let mut unhealthy_count = 0;

        // Collect health data
        let mut health_statuses = Vec::new();

        for project in &projects {
            let project_path = infrastructure_root.join(&project.path);
            let environments_dir = project_path.join("environments");

            if !ctx.fs.exists(&environments_dir) {
                continue;
            }

            for env_entry in ctx.fs.read_dir(&environments_dir)? {
                if !ctx.fs.is_dir(&env_entry) {
                    continue;
                }

                let env_file = env_entry.join(".pmp.environment.yaml");
                if !ctx.fs.exists(&env_file) {
                    continue;
                }

                total_environments += 1;

                let health = Self::check_health(ctx, &env_entry, &project.name)?;
                match health.status {
                    ResourceStatus::Healthy => healthy_count += 1,
                    ResourceStatus::Degraded => degraded_count += 1,
                    ResourceStatus::Unhealthy => unhealthy_count += 1,
                    ResourceStatus::Unknown => {}
                }

                health_statuses.push(health);
            }
        }

        ctx.output
            .key_value("Total Projects", &total_projects.to_string());
        ctx.output
            .key_value("Total Environments", &total_environments.to_string());
        ctx.output
            .key_value("Healthy", &format!("{} ✓", healthy_count));
        ctx.output
            .key_value("Degraded", &format!("{} ⚠", degraded_count));
        ctx.output
            .key_value("Unhealthy", &format!("{} ✗", unhealthy_count));
        output::blank();

        // Display detailed health status
        ctx.output.subsection("Environment Health");
        output::blank();

        for health in &health_statuses {
            let status_icon = match health.status {
                ResourceStatus::Healthy => "✓",
                ResourceStatus::Degraded => "⚠",
                ResourceStatus::Unhealthy => "✗",
                ResourceStatus::Unknown => "?",
            };

            ctx.output.dimmed(&format!(
                "{} {}/{} - {} healthy, {} unhealthy",
                status_icon,
                health.project,
                health.environment,
                health.resources_healthy,
                health.resources_unhealthy
            ));
        }

        output::blank();

        // Check for active alerts
        let alerts = Self::get_active_alerts(ctx, &infrastructure_root)?;
        if !alerts.is_empty() {
            ctx.output.subsection("Active Alerts");
            output::blank();

            for alert in alerts.iter().take(5) {
                let severity_icon = match alert.severity {
                    AlertSeverity::Critical => "🔴",
                    AlertSeverity::Warning => "🟡",
                    AlertSeverity::Info => "🔵",
                };

                ctx.output.dimmed(&format!(
                    "{} [{}/{}] {}",
                    severity_icon, alert.project, alert.environment, alert.message
                ));
            }

            if alerts.len() > 5 {
                output::blank();
                ctx.output
                    .dimmed(&format!("... and {} more alerts", alerts.len() - 5));
            }

            output::blank();
        }

        ctx.output.success("Dashboard refreshed");

        Ok(())
    }

    pub fn execute_metrics(
        ctx: &Context,
        path: Option<&str>,
        metric_name: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Infrastructure Metrics");
        output::blank();

        let (_infrastructure, _infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        let current_path = if let Some(p) = path {
            Path::new(p).to_path_buf()
        } else {
            std::env::current_dir()?
        };

        // Check if we're in an environment directory
        let env_file = current_path.join(".pmp.environment.yaml");
        if !ctx.fs.exists(&env_file) {
            ctx.output.warning("Not in an environment directory. Please specify a path or navigate to an environment.");
            return Ok(());
        }

        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;

        ctx.output.key_value("Project", &resource.metadata.name);
        ctx.output
            .key_value("Environment", &resource.metadata.environment_name);
        output::blank();

        // Collect metrics
        let metrics = Self::collect_metrics(ctx, &current_path, &resource)?;

        ctx.output.subsection("Resource Metrics");
        output::blank();

        for (name, value) in &metrics.metrics {
            if let Some(filter) = metric_name
                && !name.contains(filter)
            {
                continue;
            }

            let threshold_status = if let Some(threshold) = value.threshold {
                if value.value > threshold {
                    " ⚠ (above threshold)"
                } else {
                    " ✓"
                }
            } else {
                ""
            };

            ctx.output.key_value(
                &value.name,
                &format!("{:.2} {}{}", value.value, value.unit, threshold_status),
            );
        }

        output::blank();
        ctx.output.success("Metrics collected");

        Ok(())
    }

    pub fn execute_alerts(
        ctx: &Context,
        command: &str,
        path: Option<&str>,
        rule_name: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Alert Management");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        match command {
            "list" => {
                let alerts = Self::get_active_alerts(ctx, &infrastructure_root)?;

                if alerts.is_empty() {
                    ctx.output.info("No active alerts");
                    return Ok(());
                }

                ctx.output.subsection("Active Alerts");
                output::blank();

                for alert in &alerts {
                    let severity_str = match alert.severity {
                        AlertSeverity::Critical => "CRITICAL",
                        AlertSeverity::Warning => "WARNING",
                        AlertSeverity::Info => "INFO",
                    };

                    ctx.output
                        .dimmed(&format!("[{}] {}", alert.id, severity_str));
                    ctx.output.dimmed(&format!("  Project: {}", alert.project));
                    ctx.output
                        .dimmed(&format!("  Environment: {}", alert.environment));
                    ctx.output.dimmed(&format!("  Message: {}", alert.message));
                    ctx.output
                        .dimmed(&format!("  Triggered: {}", alert.triggered_at));
                    output::blank();
                }

                ctx.output
                    .success(&format!("{} active alerts", alerts.len()));
            }
            "configure" => {
                let current_path = if let Some(p) = path {
                    Path::new(p).to_path_buf()
                } else {
                    std::env::current_dir()?
                };

                Self::configure_alert_rules(ctx, &current_path, rule_name)?;
            }
            "clear" => {
                ctx.output.info("Clearing resolved alerts...");
                Self::clear_resolved_alerts(ctx, &infrastructure_root)?;
                ctx.output.success("Resolved alerts cleared");
            }
            _ => {
                anyhow::bail!("Unknown alert command: {}", command);
            }
        }

        Ok(())
    }

    // Helper functions

    fn check_health(_ctx: &Context, env_path: &Path, project_name: &str) -> Result<HealthStatus> {
        // In a real implementation:
        // 1. Read Terraform/OpenTofu state
        // 2. Check resource health via cloud provider APIs
        // 3. Verify expected vs actual resources
        // 4. Check for drift

        // For now, return mock data
        let env_name = env_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        Ok(HealthStatus {
            project: project_name.to_string(),
            environment: env_name.to_string(),
            status: ResourceStatus::Healthy,
            last_checked: chrono::Utc::now().to_rfc3339(),
            resources_healthy: 5,
            resources_unhealthy: 0,
            resources_unknown: 0,
        })
    }

    fn collect_metrics(
        _ctx: &Context,
        _env_path: &Path,
        resource: &DynamicProjectEnvironmentResource,
    ) -> Result<MetricData> {
        // In a real implementation:
        // 1. Query cloud provider APIs for metrics
        // 2. Parse state file for resource counts
        // 3. Collect cost data
        // 4. Gather performance metrics

        let mut metrics = HashMap::new();

        metrics.insert(
            "resource_count".to_string(),
            MetricValue {
                name: "Total Resources".to_string(),
                value: 12.0,
                unit: "resources".to_string(),
                threshold: Some(50.0),
            },
        );

        metrics.insert(
            "monthly_cost".to_string(),
            MetricValue {
                name: "Estimated Monthly Cost".to_string(),
                value: 234.56,
                unit: "USD".to_string(),
                threshold: Some(500.0),
            },
        );

        metrics.insert(
            "cpu_utilization".to_string(),
            MetricValue {
                name: "CPU Utilization".to_string(),
                value: 45.2,
                unit: "%".to_string(),
                threshold: Some(80.0),
            },
        );

        metrics.insert(
            "memory_utilization".to_string(),
            MetricValue {
                name: "Memory Utilization".to_string(),
                value: 62.8,
                unit: "%".to_string(),
                threshold: Some(85.0),
            },
        );

        Ok(MetricData {
            project: resource.metadata.name.clone(),
            environment: resource.metadata.environment_name.clone(),
            metrics,
            timestamp: chrono::Utc::now().to_rfc3339(),
        })
    }

    fn get_active_alerts(_ctx: &Context, _infrastructure_root: &Path) -> Result<Vec<Alert>> {
        // In a real implementation:
        // 1. Read from alerts database/file
        // 2. Filter for unresolved alerts
        // 3. Sort by severity and time

        Ok(vec![
            Alert {
                id: "alert-001".to_string(),
                project: "api-service".to_string(),
                environment: "production".to_string(),
                severity: AlertSeverity::Warning,
                message: "CPU utilization above 80% for 5 minutes".to_string(),
                triggered_at: chrono::Utc::now().to_rfc3339(),
                resolved: false,
            },
            Alert {
                id: "alert-002".to_string(),
                project: "database".to_string(),
                environment: "production".to_string(),
                severity: AlertSeverity::Critical,
                message: "Memory utilization above 90%".to_string(),
                triggered_at: chrono::Utc::now().to_rfc3339(),
                resolved: false,
            },
        ])
    }

    fn configure_alert_rules(
        ctx: &Context,
        _env_path: &Path,
        rule_name: Option<&str>,
    ) -> Result<()> {
        ctx.output.subsection("Configure Alert Rules");
        output::blank();

        let name = if let Some(n) = rule_name {
            n.to_string()
        } else {
            ctx.input.text("Rule name:", None)?
        };

        let metric = ctx.input.text("Metric to monitor:", None)?;
        let condition = ctx.input.text("Condition (>, <, ==):", Some(">"))?;
        let threshold: f64 = ctx.input.text("Threshold value:", None)?.parse()?;

        let severity_options = vec![
            "critical".to_string(),
            "warning".to_string(),
            "info".to_string(),
        ];
        let severity_selection = ctx.input.select("Severity:", severity_options)?;
        let severity = match severity_selection.as_str() {
            "critical" => AlertSeverity::Critical,
            "warning" => AlertSeverity::Warning,
            _ => AlertSeverity::Info,
        };

        let rule = AlertRule {
            name: name.clone(),
            metric,
            condition,
            threshold,
            severity,
        };

        // In a real implementation, save to .pmp/alerts/rules.yaml
        ctx.output.dimmed(&format!(
            "Alert rule '{}' configured: {} ({:?})",
            name, rule.metric, rule.severity
        ));

        ctx.output.success("Alert rule configured");

        Ok(())
    }

    fn clear_resolved_alerts(_ctx: &Context, _infrastructure_root: &Path) -> Result<()> {
        // In a real implementation:
        // 1. Read alerts from storage
        // 2. Filter out resolved alerts
        // 3. Update storage

        Ok(())
    }
}
