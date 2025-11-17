use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::output;
use crate::template::metadata::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct AuditCommand;

#[derive(Debug, Serialize, Deserialize)]
pub struct AuditLogEntry {
    pub id: String,
    pub timestamp: String,
    pub project: String,
    pub environment: String,
    pub action: String,
    pub user: String,
    pub changes: ChangesSummary,
    pub status: AuditStatus,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChangesSummary {
    pub resources_added: usize,
    pub resources_modified: usize,
    pub resources_deleted: usize,
    pub total_changes: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum AuditStatus {
    Success,
    Failed,
    Partial,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StateDiff {
    pub project: String,
    pub environment: String,
    pub from_state: String,
    pub to_state: String,
    pub differences: Vec<ResourceDiff>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceDiff {
    pub resource_type: String,
    pub resource_name: String,
    pub change_type: ChangeType,
    pub attribute_changes: Vec<AttributeChange>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Unchanged,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AttributeChange {
    pub attribute: String,
    pub old_value: Option<String>,
    pub new_value: Option<String>,
}

impl AuditCommand {
    pub fn execute_log(
        ctx: &Context,
        path: Option<&str>,
        limit: Option<usize>,
        action_filter: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Deployment Audit Log");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        let current_path = if let Some(p) = path {
            Path::new(p).to_path_buf()
        } else {
            std::env::current_dir()?
        };

        // Get audit logs
        let logs = Self::get_audit_logs(ctx, &infrastructure_root, &current_path)?;

        // Filter by action if specified
        let filtered_logs: Vec<_> = if let Some(filter) = action_filter {
            logs.into_iter()
                .filter(|log| log.action.contains(filter))
                .collect()
        } else {
            logs
        };

        // Apply limit
        let display_count = limit.unwrap_or(20);
        let display_logs: Vec<_> = filtered_logs.iter().take(display_count).collect();

        if display_logs.is_empty() {
            ctx.output.info("No audit logs found");
            return Ok(());
        }

        ctx.output.subsection("Recent Deployments");
        output::blank();

        for log in &display_logs {
            let status_icon = match log.status {
                AuditStatus::Success => "✓",
                AuditStatus::Failed => "✗",
                AuditStatus::Partial => "⚠",
            };

            ctx.output
                .dimmed(&format!("[{}] {}", log.id, log.timestamp));
            ctx.output.dimmed(&format!(
                "  {} {} - {}/{}",
                status_icon, log.action, log.project, log.environment
            ));
            ctx.output.dimmed(&format!("  User: {}", log.user));
            ctx.output.dimmed(&format!(
                "  Changes: +{} ~{} -{}",
                log.changes.resources_added,
                log.changes.resources_modified,
                log.changes.resources_deleted
            ));
            output::blank();
        }

        if filtered_logs.len() > display_count {
            ctx.output.dimmed(&format!(
                "... and {} more entries (use --limit to see more)",
                filtered_logs.len() - display_count
            ));
            output::blank();
        }

        ctx.output
            .success(&format!("Showing {} audit entries", display_logs.len()));

        Ok(())
    }

    pub fn execute_diff(
        ctx: &Context,
        path: Option<&str>,
        from_state: Option<&str>,
        to_state: Option<&str>,
        output_format: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("State Diff Analysis");
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
            ctx.output
                .warning("Not in an environment directory. Please specify a path.");
            return Ok(());
        }

        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;

        ctx.output.key_value("Project", &resource.metadata.name);
        ctx.output
            .key_value("Environment", &resource.metadata.environment_name);
        output::blank();

        // Determine states to compare
        let from = from_state.unwrap_or("previous");
        let to = to_state.unwrap_or("current");

        ctx.output.subsection("Comparing States");
        ctx.output.dimmed(&format!("From: {}", from));
        ctx.output.dimmed(&format!("To: {}", to));
        output::blank();

        // Generate diff
        let diff = Self::generate_state_diff(ctx, &current_path, &resource, from, to)?;

        let format = output_format.unwrap_or("text");

        match format {
            "json" => {
                let json = serde_json::to_string_pretty(&diff)?;
                println!("{}", json);
            }
            "yaml" => {
                let yaml = serde_yaml::to_string(&diff)?;
                println!("{}", yaml);
            }
            _ => {
                // Text format
                Self::display_diff_text(ctx, &diff)?;
            }
        }

        Ok(())
    }

    // Helper functions

    fn get_audit_logs(
        _ctx: &Context,
        infrastructure_root: &Path,
        _current_path: &Path,
    ) -> Result<Vec<AuditLogEntry>> {
        // In a real implementation:
        // 1. Read from .pmp/audit/logs.jsonl or similar
        // 2. Parse log entries
        // 3. Sort by timestamp (newest first)

        let _audit_dir = infrastructure_root.join(".pmp").join("audit");

        // Return mock data for now
        Ok(vec![
            AuditLogEntry {
                id: "audit-001".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                project: "api-service".to_string(),
                environment: "production".to_string(),
                action: "apply".to_string(),
                user: "alice@example.com".to_string(),
                changes: ChangesSummary {
                    resources_added: 2,
                    resources_modified: 3,
                    resources_deleted: 0,
                    total_changes: 5,
                },
                status: AuditStatus::Success,
            },
            AuditLogEntry {
                id: "audit-002".to_string(),
                timestamp: chrono::Utc::now()
                    .checked_sub_signed(chrono::Duration::hours(2))
                    .unwrap()
                    .to_rfc3339(),
                project: "database".to_string(),
                environment: "staging".to_string(),
                action: "destroy".to_string(),
                user: "bob@example.com".to_string(),
                changes: ChangesSummary {
                    resources_added: 0,
                    resources_modified: 0,
                    resources_deleted: 5,
                    total_changes: 5,
                },
                status: AuditStatus::Success,
            },
            AuditLogEntry {
                id: "audit-003".to_string(),
                timestamp: chrono::Utc::now()
                    .checked_sub_signed(chrono::Duration::hours(6))
                    .unwrap()
                    .to_rfc3339(),
                project: "api-service".to_string(),
                environment: "staging".to_string(),
                action: "apply".to_string(),
                user: "alice@example.com".to_string(),
                changes: ChangesSummary {
                    resources_added: 1,
                    resources_modified: 2,
                    resources_deleted: 1,
                    total_changes: 4,
                },
                status: AuditStatus::Partial,
            },
        ])
    }

    fn generate_state_diff(
        _ctx: &Context,
        _env_path: &Path,
        resource: &DynamicProjectEnvironmentResource,
        from: &str,
        to: &str,
    ) -> Result<StateDiff> {
        // In a real implementation:
        // 1. Read state files for both versions
        // 2. Parse Terraform/OpenTofu state
        // 3. Compare resources and attributes
        // 4. Generate detailed diff

        // Return mock data for now
        Ok(StateDiff {
            project: resource.metadata.name.clone(),
            environment: resource.metadata.environment_name.clone(),
            from_state: from.to_string(),
            to_state: to.to_string(),
            differences: vec![
                ResourceDiff {
                    resource_type: "aws_instance".to_string(),
                    resource_name: "web_server".to_string(),
                    change_type: ChangeType::Modified,
                    attribute_changes: vec![
                        AttributeChange {
                            attribute: "instance_type".to_string(),
                            old_value: Some("t2.micro".to_string()),
                            new_value: Some("t2.small".to_string()),
                        },
                        AttributeChange {
                            attribute: "tags.Environment".to_string(),
                            old_value: Some("dev".to_string()),
                            new_value: Some("staging".to_string()),
                        },
                    ],
                },
                ResourceDiff {
                    resource_type: "aws_s3_bucket".to_string(),
                    resource_name: "assets".to_string(),
                    change_type: ChangeType::Added,
                    attribute_changes: vec![AttributeChange {
                        attribute: "bucket".to_string(),
                        old_value: None,
                        new_value: Some("my-assets-bucket".to_string()),
                    }],
                },
                ResourceDiff {
                    resource_type: "aws_db_instance".to_string(),
                    resource_name: "legacy_db".to_string(),
                    change_type: ChangeType::Deleted,
                    attribute_changes: vec![AttributeChange {
                        attribute: "instance_class".to_string(),
                        old_value: Some("db.t2.micro".to_string()),
                        new_value: None,
                    }],
                },
            ],
        })
    }

    fn display_diff_text(ctx: &Context, diff: &StateDiff) -> Result<()> {
        ctx.output.subsection("Resource Changes");
        output::blank();

        let mut added = 0;
        let mut modified = 0;
        let mut deleted = 0;

        for resource_diff in &diff.differences {
            match resource_diff.change_type {
                ChangeType::Added => {
                    added += 1;
                    ctx.output.dimmed(&format!(
                        "+ {} {}",
                        resource_diff.resource_type, resource_diff.resource_name
                    ));
                    for attr in &resource_diff.attribute_changes {
                        if let Some(new_val) = &attr.new_value {
                            ctx.output
                                .dimmed(&format!("    {}: {}", attr.attribute, new_val));
                        }
                    }
                }
                ChangeType::Modified => {
                    modified += 1;
                    ctx.output.dimmed(&format!(
                        "~ {} {}",
                        resource_diff.resource_type, resource_diff.resource_name
                    ));
                    for attr in &resource_diff.attribute_changes {
                        ctx.output.dimmed(&format!(
                            "    {}: {} -> {}",
                            attr.attribute,
                            attr.old_value.as_ref().unwrap_or(&"null".to_string()),
                            attr.new_value.as_ref().unwrap_or(&"null".to_string())
                        ));
                    }
                }
                ChangeType::Deleted => {
                    deleted += 1;
                    ctx.output.dimmed(&format!(
                        "- {} {}",
                        resource_diff.resource_type, resource_diff.resource_name
                    ));
                }
                ChangeType::Unchanged => {}
            }
            output::blank();
        }

        ctx.output.subsection("Summary");
        ctx.output
            .key_value("Resources Added", &format!("+{}", added));
        ctx.output
            .key_value("Resources Modified", &format!("~{}", modified));
        ctx.output
            .key_value("Resources Deleted", &format!("-{}", deleted));
        output::blank();

        ctx.output.success("Diff analysis complete");

        Ok(())
    }
}
