use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::executor::{Executor, ExecutorConfig, OpenTofuExecutor};
use crate::output;
use crate::template::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct DriftCommand;

#[derive(Debug, Serialize, Deserialize)]
pub struct DriftReport {
    pub project: String,
    pub environment: String,
    pub timestamp: String,
    pub has_drift: bool,
    pub changes: Vec<DriftChange>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DriftChange {
    pub resource_address: String,
    pub change_type: ChangeType,
    pub attribute: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    Added,
    Modified,
    Removed,
}

impl DriftCommand {
    /// Execute the drift detect command
    pub fn execute_detect(ctx: &Context, path: Option<&str>, format: Option<&str>) -> Result<()> {
        ctx.output.section("Drift Detection");

        // Find infrastructure
        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output
            .key_value("Infrastructure", &infrastructure.metadata.name);
        output::blank();

        // Determine what to check
        let current_path = if let Some(p) = path {
            std::path::PathBuf::from(p)
        } else {
            std::env::current_dir()?
        };

        // Check if we're in an environment context
        let env_yaml = current_path.join(".pmp.environment.yaml");

        if ctx.fs.exists(&env_yaml) {
            // We're in an environment directory
            Self::detect_single_environment(ctx, &current_path, format)?;
        } else {
            // Scan all projects
            Self::detect_all_projects(ctx, &infrastructure_root, format)?;
        }

        Ok(())
    }

    /// Execute the drift report command
    pub fn execute_report(
        ctx: &Context,
        path: Option<&str>,
        output_file: Option<&str>,
        format: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Drift Report");

        let current_path = if let Some(p) = path {
            std::path::PathBuf::from(p)
        } else {
            std::env::current_dir()?
        };

        let env_yaml = current_path.join(".pmp.environment.yaml");

        if !ctx.fs.exists(&env_yaml) {
            anyhow::bail!(
                "Not in an environment directory. Navigate to a project environment or use --path"
            );
        }

        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_yaml)?;
        let report = Self::generate_drift_report(ctx, &current_path, &resource)?;

        // Render report
        Self::render_report(ctx, &report, format.unwrap_or("text"), output_file)?;

        Ok(())
    }

    /// Execute the drift reconcile command
    pub fn execute_reconcile(ctx: &Context, path: Option<&str>, auto_approve: bool) -> Result<()> {
        ctx.output.section("Drift Reconciliation");

        let current_path = if let Some(p) = path {
            std::path::PathBuf::from(p)
        } else {
            std::env::current_dir()?
        };

        let env_yaml = current_path.join(".pmp.environment.yaml");

        if !ctx.fs.exists(&env_yaml) {
            anyhow::bail!(
                "Not in an environment directory. Navigate to a project environment or use --path"
            );
        }

        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_yaml)?;

        ctx.output.key_value("Project", &resource.metadata.name);
        ctx.output
            .key_value("Environment", &resource.metadata.environment_name);
        output::blank();

        // First detect drift
        let report = Self::generate_drift_report(ctx, &current_path, &resource)?;

        if !report.has_drift {
            ctx.output
                .success("No drift detected. Infrastructure matches configuration.");
            return Ok(());
        }

        // Show drift summary
        ctx.output.info(&format!(
            "Detected {} change(s) in infrastructure",
            report.changes.len()
        ));
        output::blank();

        for change in &report.changes {
            let change_symbol = match change.change_type {
                ChangeType::Added => "+",
                ChangeType::Modified => "~",
                ChangeType::Removed => "-",
            };
            ctx.output.dimmed(&format!(
                "{} {} [{}]: {} → {}",
                change_symbol,
                change.resource_address,
                change.attribute,
                change.expected,
                change.actual
            ));
        }
        output::blank();

        // Confirm reconciliation
        let confirmed = if auto_approve {
            true
        } else {
            ctx.input.confirm(
                "Apply changes to reconcile drift? This will run 'tofu apply'.",
                false,
            )?
        };

        if !confirmed {
            ctx.output.dimmed("Reconciliation cancelled.");
            return Ok(());
        }

        // Execute reconciliation (apply)
        Self::reconcile_drift(ctx, &current_path, &resource)?;

        Ok(())
    }

    /// Detect drift in a single environment
    fn detect_single_environment(
        ctx: &Context,
        env_path: &Path,
        format: Option<&str>,
    ) -> Result<()> {
        let env_yaml = env_path.join(".pmp.environment.yaml");
        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_yaml)?;

        ctx.output.subsection(&format!(
            "Project: {} ({})",
            resource.metadata.name, resource.metadata.environment_name
        ));
        output::blank();

        let report = Self::generate_drift_report(ctx, env_path, &resource)?;

        if report.has_drift {
            ctx.output.error(&format!(
                "Drift detected: {} change(s)",
                report.changes.len()
            ));

            if format.is_none() || format == Some("text") {
                output::blank();
                Self::display_drift_changes(ctx, &report.changes);
            }
        } else {
            ctx.output
                .success("No drift detected. Infrastructure matches configuration.");
        }

        Ok(())
    }

    /// Detect drift in all projects
    fn detect_all_projects(
        ctx: &Context,
        infrastructure_root: &Path,
        format: Option<&str>,
    ) -> Result<()> {
        ctx.output.subsection("Scanning All Projects");
        output::blank();

        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, infrastructure_root)?;

        if projects.is_empty() {
            ctx.output.dimmed("No projects found.");
            return Ok(());
        }

        let mut total_drift_count = 0;
        let mut projects_with_drift = Vec::new();

        for project in &projects {
            let project_path = infrastructure_root.join(&project.path);
            let environments_dir = project_path.join("environments");

            if let Ok(env_entries) = ctx.fs.read_dir(&environments_dir) {
                for env_path in env_entries {
                    let env_file = env_path.join(".pmp.environment.yaml");
                    if ctx.fs.exists(&env_file)
                        && let Ok(resource) =
                            DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
                        && let Ok(report) = Self::generate_drift_report(ctx, &env_path, &resource)
                        && report.has_drift
                    {
                        total_drift_count += report.changes.len();
                        projects_with_drift.push(format!(
                            "{}:{} ({} changes)",
                            resource.metadata.name,
                            resource.metadata.environment_name,
                            report.changes.len()
                        ));
                    }
                }
            }
        }

        if total_drift_count == 0 {
            ctx.output.success("No drift detected across all projects.");
        } else {
            ctx.output.error(&format!(
                "Drift detected in {} project(s) with {} total change(s)",
                projects_with_drift.len(),
                total_drift_count
            ));
            output::blank();

            if format.is_none() || format == Some("text") {
                for project_info in &projects_with_drift {
                    ctx.output.info(&format!("• {}", project_info));
                }
            }
        }

        Ok(())
    }

    /// Generate drift report for an environment
    fn generate_drift_report(
        ctx: &Context,
        env_path: &Path,
        resource: &DynamicProjectEnvironmentResource,
    ) -> Result<DriftReport> {
        // Execute terraform/opentofu refresh and plan to detect drift
        let executor_name = &resource.spec.executor.name;

        if executor_name != "opentofu" && executor_name != "terraform" {
            anyhow::bail!("Drift detection only supports opentofu/terraform executors");
        }

        let executor = OpenTofuExecutor::new();
        let env_path_str = env_path.to_str().context("Invalid path")?;

        // Run refresh to update state
        ctx.output.dimmed("Refreshing state...");
        let config = ExecutorConfig {
            plan_command: None,
            apply_command: None,
            destroy_command: None,
            refresh_command: None,
            ..Default::default()
        };

        executor.refresh(&config, env_path_str, &[])?;

        // Run plan to detect changes with output capture
        ctx.output.dimmed("Detecting changes...");

        let plan_output = executor.plan_with_output(env_path_str, &[])?;

        // Parse plan output to extract changes
        let changes = Self::parse_plan_output(&plan_output)?;

        Ok(DriftReport {
            project: resource.metadata.name.clone(),
            environment: resource.metadata.environment_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            has_drift: !changes.is_empty(),
            changes,
        })
    }

    /// Parse Terraform/OpenTofu plan output to extract drift changes
    fn parse_plan_output(output: &std::process::Output) -> Result<Vec<DriftChange>> {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut changes = Vec::new();

        // Parse the plan output
        // Terraform/OpenTofu plan shows changes in this format:
        // # resource_address will be created/updated/destroyed
        // + attribute = "value"  (added)
        // ~ attribute = "old" -> "new"  (modified)
        // - attribute = "value"  (removed)

        let lines: Vec<&str> = stdout.lines().collect();
        let mut current_resource = String::new();

        // Regex patterns for parsing
        let resource_pattern = regex::Regex::new(
            r"^#\s+(.+?)\s+(will be|must be)\s+(created|updated|destroyed|replaced)",
        )
        .unwrap();
        let change_pattern = regex::Regex::new(r"^\s+([+~-])\s+(.+?)\s+=\s+(.+)$").unwrap();
        let arrow_pattern = regex::Regex::new(r#""(.+?)"\s+->\s+"(.+?)""#).unwrap();

        for line in lines {
            // Check for resource declaration
            if let Some(caps) = resource_pattern.captures(line) {
                current_resource = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
            }
            // Check for attribute changes
            else if let Some(caps) = change_pattern.captures(line) {
                let symbol = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let attribute = caps.get(2).map(|m| m.as_str()).unwrap_or("").trim();
                let value_part = caps.get(3).map(|m| m.as_str()).unwrap_or("").trim();

                let change_type = match symbol {
                    "+" => ChangeType::Added,
                    "-" => ChangeType::Removed,
                    "~" => ChangeType::Modified,
                    _ => continue,
                };

                // For modifications, try to extract old -> new values
                let (expected, actual) = if change_type == ChangeType::Modified {
                    if let Some(arrow_caps) = arrow_pattern.captures(value_part) {
                        let old = arrow_caps
                            .get(1)
                            .map(|m| m.as_str())
                            .unwrap_or("")
                            .to_string();
                        let new = arrow_caps
                            .get(2)
                            .map(|m| m.as_str())
                            .unwrap_or("")
                            .to_string();
                        (old, new)
                    } else {
                        // Fallback if pattern doesn't match
                        ("(unknown)".to_string(), value_part.to_string())
                    }
                } else if change_type == ChangeType::Added {
                    ("(not set)".to_string(), value_part.to_string())
                } else {
                    (value_part.to_string(), "(removed)".to_string())
                };

                if !current_resource.is_empty() {
                    changes.push(DriftChange {
                        resource_address: current_resource.clone(),
                        change_type,
                        attribute: attribute.to_string(),
                        expected,
                        actual,
                    });
                }
            }
        }

        // If no detailed changes were found but exit code was 2, add a summary
        if changes.is_empty() && output.status.code() == Some(2) {
            changes.push(DriftChange {
                resource_address: "(multiple resources)".to_string(),
                change_type: ChangeType::Modified,
                attribute: "configuration".to_string(),
                expected: "declared state".to_string(),
                actual: "actual state differs".to_string(),
            });
        }

        Ok(changes)
    }

    /// Display drift changes
    fn display_drift_changes(ctx: &Context, changes: &[DriftChange]) {
        for change in changes {
            let symbol = match change.change_type {
                ChangeType::Added => "+",
                ChangeType::Modified => "~",
                ChangeType::Removed => "-",
            };

            ctx.output.dimmed(&format!(
                "[{}] {} [{}]:",
                symbol, change.resource_address, change.attribute
            ));
            ctx.output
                .dimmed(&format!("  Expected: {}", change.expected));
            ctx.output.dimmed(&format!("  Actual:   {}", change.actual));
            output::blank();
        }
    }

    /// Render drift report
    fn render_report(
        ctx: &Context,
        report: &DriftReport,
        format: &str,
        output_file: Option<&str>,
    ) -> Result<()> {
        let content = match format {
            "json" => serde_json::to_string_pretty(report)?,
            "yaml" => serde_yaml::to_string(report)?,
            _ => {
                let mut text = String::new();
                text.push_str(&format!(
                    "Drift Report: {} ({})\n",
                    report.project, report.environment
                ));
                text.push_str(&format!("Timestamp: {}\n", report.timestamp));
                text.push_str(&format!("Has Drift: {}\n", report.has_drift));
                text.push_str(&format!("Changes: {}\n\n", report.changes.len()));

                for change in &report.changes {
                    text.push_str(&format!(
                        "[{:?}] {} [{}]:\n",
                        change.change_type, change.resource_address, change.attribute
                    ));
                    text.push_str(&format!("  Expected: {}\n", change.expected));
                    text.push_str(&format!("  Actual:   {}\n\n", change.actual));
                }

                text
            }
        };

        if let Some(file) = output_file {
            ctx.fs.write(&std::path::PathBuf::from(file), &content)?;
            ctx.output
                .success(&format!("Drift report written to: {}", file));
        } else {
            ctx.output.info(&content);
        }

        Ok(())
    }

    /// Reconcile drift by applying changes
    fn reconcile_drift(
        ctx: &Context,
        env_path: &Path,
        _resource: &DynamicProjectEnvironmentResource,
    ) -> Result<()> {
        let executor = OpenTofuExecutor::new();
        let env_path_str = env_path.to_str().context("Invalid path")?;

        ctx.output.info("Applying changes to reconcile drift...");
        output::blank();

        let config = ExecutorConfig {
            plan_command: None,
            apply_command: None,
            destroy_command: None,
            refresh_command: None,
            ..Default::default()
        };

        executor.apply(&config, env_path_str, &[])?;

        ctx.output
            .success("Drift reconciled successfully. Infrastructure now matches configuration.");

        Ok(())
    }
}
