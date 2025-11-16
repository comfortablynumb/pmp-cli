use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::output;
use crate::template::metadata::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

pub struct TagsCommand;

#[derive(Debug, Serialize, Deserialize)]
pub struct TagConfig {
    pub tags: HashMap<String, String>,
    pub last_updated: String,
    pub updated_by: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TagAuditReport {
    pub generated_at: String,
    pub total_projects: usize,
    pub compliant_projects: usize,
    pub non_compliant_projects: Vec<ComplianceIssue>,
    pub required_tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ComplianceIssue {
    pub project: String,
    pub environment: String,
    pub missing_tags: Vec<String>,
    pub invalid_tags: Vec<(String, String)>,
}

impl TagsCommand {
    pub fn execute_add(
        ctx: &Context,
        path: Option<&str>,
        tag_pairs: Vec<String>,
    ) -> Result<()> {
        ctx.output.section("Add Tags");
        output::blank();

        let (_infrastructure, infrastructure_root) =
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
        ctx.output.key_value("Environment", &resource.metadata.environment_name);
        output::blank();

        // Parse tag pairs
        let mut tags = HashMap::new();
        for pair in &tag_pairs {
            let parts: Vec<&str> = pair.splitn(2, '=').collect();
            if parts.len() == 2 {
                tags.insert(parts[0].to_string(), parts[1].to_string());
            } else {
                ctx.output.warning(&format!("Invalid tag format: {}", pair));
                ctx.output.dimmed("Use KEY=VALUE format");
            }
        }

        if tags.is_empty() {
            // Interactive mode
            loop {
                let key = ctx.input.text("Tag key (empty to finish):", None)?;
                if key.is_empty() {
                    break;
                }

                let value = ctx.input.text(&format!("Value for '{}':", key), None)?;
                tags.insert(key, value);
            }
        }

        if tags.is_empty() {
            ctx.output.info("No tags to add");
            return Ok(());
        }

        // Load existing tags
        let mut tag_config = Self::load_tags(ctx, &infrastructure_root, &resource)
            .unwrap_or_else(|_| TagConfig {
                tags: HashMap::new(),
                last_updated: chrono::Utc::now().to_rfc3339(),
                updated_by: Self::get_current_user().unwrap_or_default(),
            });

        // Merge tags
        for (key, value) in tags {
            ctx.output.dimmed(&format!("Adding tag: {} = {}", key, value));
            tag_config.tags.insert(key, value);
        }

        // Update metadata
        tag_config.last_updated = chrono::Utc::now().to_rfc3339();
        tag_config.updated_by = Self::get_current_user().unwrap_or_default();

        // Save tags
        Self::save_tags(ctx, &infrastructure_root, &resource, &tag_config)?;

        ctx.output.success("Tags added");
        ctx.output.dimmed(&format!("Total tags: {}", tag_config.tags.len()));

        Ok(())
    }

    pub fn execute_remove(
        ctx: &Context,
        path: Option<&str>,
        tag_keys: Vec<String>,
    ) -> Result<()> {
        ctx.output.section("Remove Tags");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        let current_path = if let Some(p) = path {
            Path::new(p).to_path_buf()
        } else {
            std::env::current_dir()?
        };

        let env_file = current_path.join(".pmp.environment.yaml");
        if !ctx.fs.exists(&env_file) {
            ctx.output
                .warning("Not in an environment directory.");
            return Ok(());
        }

        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;

        // Load existing tags
        let mut tag_config = Self::load_tags(ctx, &infrastructure_root, &resource)?;

        // Remove tags
        for key in &tag_keys {
            if tag_config.tags.remove(key).is_some() {
                ctx.output.dimmed(&format!("Removed tag: {}", key));
            } else {
                ctx.output.warning(&format!("Tag not found: {}", key));
            }
        }

        // Update metadata
        tag_config.last_updated = chrono::Utc::now().to_rfc3339();
        tag_config.updated_by = Self::get_current_user().unwrap_or_default();

        // Save tags
        Self::save_tags(ctx, &infrastructure_root, &resource, &tag_config)?;

        ctx.output.success("Tags removed");

        Ok(())
    }

    pub fn execute_list(ctx: &Context, path: Option<&str>) -> Result<()> {
        ctx.output.section("Project Tags");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        let current_path = if let Some(p) = path {
            Path::new(p).to_path_buf()
        } else {
            std::env::current_dir()?
        };

        let env_file = current_path.join(".pmp.environment.yaml");
        if !ctx.fs.exists(&env_file) {
            ctx.output
                .warning("Not in an environment directory.");
            return Ok(());
        }

        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;

        ctx.output.key_value("Project", &resource.metadata.name);
        ctx.output.key_value("Environment", &resource.metadata.environment_name);
        output::blank();

        // Load tags
        let tag_config = Self::load_tags(ctx, &infrastructure_root, &resource)?;

        if tag_config.tags.is_empty() {
            ctx.output.info("No tags defined");
            return Ok(());
        }

        ctx.output.subsection("Tags");
        output::blank();

        let mut keys: Vec<_> = tag_config.tags.keys().collect();
        keys.sort();

        for key in keys {
            let value = &tag_config.tags[key];
            ctx.output.key_value(key, value);
        }

        output::blank();
        ctx.output.dimmed(&format!("Last updated: {} by {}", tag_config.last_updated, tag_config.updated_by));

        Ok(())
    }

    pub fn execute_audit(ctx: &Context, required_tags: Vec<String>) -> Result<()> {
        ctx.output.section("Tag Compliance Audit");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        // Get all projects
        let projects = crate::collection::CollectionDiscovery::discover_projects(
            &*ctx.fs,
            &*ctx.output,
            &infrastructure_root,
        )?;

        ctx.output.dimmed(&format!("Auditing {} projects...", projects.len()));
        output::blank();

        let mut report = TagAuditReport {
            generated_at: chrono::Utc::now().to_rfc3339(),
            total_projects: 0,
            compliant_projects: 0,
            non_compliant_projects: Vec::new(),
            required_tags: required_tags.clone(),
        };

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

                report.total_projects += 1;

                let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;

                // Check tags
                if let Ok(tag_config) = Self::load_tags(ctx, &infrastructure_root, &resource) {
                    let mut missing_tags = Vec::new();

                    for required_tag in &required_tags {
                        if !tag_config.tags.contains_key(required_tag) {
                            missing_tags.push(required_tag.clone());
                        }
                    }

                    if !missing_tags.is_empty() {
                        report.non_compliant_projects.push(ComplianceIssue {
                            project: resource.metadata.name.clone(),
                            environment: resource.metadata.environment_name.clone(),
                            missing_tags,
                            invalid_tags: Vec::new(),
                        });
                    } else {
                        report.compliant_projects += 1;
                    }
                } else {
                    // No tags at all
                    report.non_compliant_projects.push(ComplianceIssue {
                        project: resource.metadata.name.clone(),
                        environment: resource.metadata.environment_name.clone(),
                        missing_tags: required_tags.clone(),
                        invalid_tags: Vec::new(),
                    });
                }
            }
        }

        // Display report
        ctx.output.subsection("Audit Summary");
        output::blank();

        ctx.output.key_value("Total Projects", &report.total_projects.to_string());
        ctx.output.key_value("Compliant", &format!("{} ({:.1}%)",
            report.compliant_projects,
            (report.compliant_projects as f64 / report.total_projects as f64) * 100.0
        ));
        ctx.output.key_value("Non-Compliant", &report.non_compliant_projects.len().to_string());
        output::blank();

        if !report.non_compliant_projects.is_empty() {
            ctx.output.subsection("Non-Compliant Projects");
            output::blank();

            for issue in &report.non_compliant_projects {
                ctx.output.dimmed(&format!("{}/{}", issue.project, issue.environment));
                if !issue.missing_tags.is_empty() {
                    ctx.output.dimmed(&format!("  Missing: {}", issue.missing_tags.join(", ")));
                }
            }

            output::blank();
        }

        // Save report
        Self::save_audit_report(ctx, &infrastructure_root, &report)?;

        if report.non_compliant_projects.is_empty() {
            ctx.output.success("All projects are compliant!");
        } else {
            ctx.output.warning("Some projects are not compliant");
        }

        Ok(())
    }

    pub fn execute_report(ctx: &Context, output_file: Option<&str>, format: Option<&str>) -> Result<()> {
        ctx.output.section("Tag Report");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        // Get all projects
        let projects = crate::collection::CollectionDiscovery::discover_projects(
            &*ctx.fs,
            &*ctx.output,
            &infrastructure_root,
        )?;

        // Collect tag data
        let mut tag_data: HashMap<String, Vec<(String, String, String)>> = HashMap::new();

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

                let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;

                if let Ok(tag_config) = Self::load_tags(ctx, &infrastructure_root, &resource) {
                    for (key, value) in &tag_config.tags {
                        tag_data
                            .entry(key.clone())
                            .or_default()
                            .push((
                                resource.metadata.name.clone(),
                                resource.metadata.environment_name.clone(),
                                value.clone(),
                            ));
                    }
                }
            }
        }

        let fmt = format.unwrap_or("text");

        match fmt {
            "json" => {
                let json = serde_json::to_string_pretty(&tag_data)?;
                if let Some(file) = output_file {
                    std::fs::write(file, json)?;
                    ctx.output.success(&format!("Report saved to {}", file));
                } else {
                    println!("{}", json);
                }
            }
            "csv" => {
                let mut csv = "Tag,Project,Environment,Value\n".to_string();
                for (tag_key, entries) in &tag_data {
                    for (project, env, value) in entries {
                        csv.push_str(&format!("{},{},{},{}\n", tag_key, project, env, value));
                    }
                }

                if let Some(file) = output_file {
                    std::fs::write(file, csv)?;
                    ctx.output.success(&format!("Report saved to {}", file));
                } else {
                    println!("{}", csv);
                }
            }
            _ => {
                // Text format
                ctx.output.subsection("Tags by Key");
                output::blank();

                let mut keys: Vec<_> = tag_data.keys().collect();
                keys.sort();

                for key in keys {
                    let entries = &tag_data[key];
                    ctx.output.dimmed(&format!("{}: {} projects", key, entries.len()));
                }

                output::blank();
                ctx.output.success(&format!("{} unique tag keys found", tag_data.len()));
            }
        }

        Ok(())
    }

    // Helper functions

    fn load_tags(
        _ctx: &Context,
        infrastructure_root: &Path,
        resource: &DynamicProjectEnvironmentResource,
    ) -> Result<TagConfig> {
        let tags_file = infrastructure_root
            .join(".pmp")
            .join("tags")
            .join(format!("{}-{}.json", resource.metadata.name, resource.metadata.environment_name));

        if !tags_file.exists() {
            anyhow::bail!("No tags found");
        }

        let content = std::fs::read_to_string(&tags_file)?;
        let config: TagConfig = serde_json::from_str(&content)?;

        Ok(config)
    }

    fn save_tags(
        _ctx: &Context,
        infrastructure_root: &Path,
        resource: &DynamicProjectEnvironmentResource,
        config: &TagConfig,
    ) -> Result<()> {
        let tags_dir = infrastructure_root.join(".pmp").join("tags");
        std::fs::create_dir_all(&tags_dir)?;

        let tags_file = tags_dir.join(format!("{}-{}.json", resource.metadata.name, resource.metadata.environment_name));
        let content = serde_json::to_string_pretty(config)?;
        std::fs::write(&tags_file, content)?;

        Ok(())
    }

    fn save_audit_report(
        _ctx: &Context,
        infrastructure_root: &Path,
        report: &TagAuditReport,
    ) -> Result<()> {
        let reports_dir = infrastructure_root.join(".pmp").join("reports");
        std::fs::create_dir_all(&reports_dir)?;

        let timestamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let report_file = reports_dir.join(format!("tag-audit-{}.json", timestamp));
        let content = serde_json::to_string_pretty(report)?;
        std::fs::write(&report_file, content)?;

        Ok(())
    }

    fn get_current_user() -> Result<String> {
        if let Ok(output) = std::process::Command::new("git")
            .args(["config", "user.email"])
            .output()
            && output.status.success()
        {
            let email = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !email.is_empty() {
                return Ok(email);
            }
        }

        Ok(whoami::username())
    }
}
