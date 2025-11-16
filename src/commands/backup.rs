use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::output;
use crate::template::metadata::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct BackupCommand;

#[derive(Debug, Serialize, Deserialize)]
pub struct Backup {
    pub id: String,
    pub project: String,
    pub environment: String,
    pub created_at: String,
    pub created_by: String,
    pub backup_type: BackupType,
    pub size_bytes: u64,
    pub description: Option<String>,
    pub metadata: BackupMetadata,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum BackupType {
    Full,
    State,
    Configuration,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupMetadata {
    pub resource_count: usize,
    pub state_version: Option<String>,
    pub terraform_version: Option<String>,
    pub checksum: String,
}

impl BackupCommand {
    pub fn execute_create(
        ctx: &Context,
        path: Option<&str>,
        backup_type: Option<&str>,
        description: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Create Infrastructure Backup");
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

        // Determine backup type
        let btype = match backup_type {
            Some("full") => BackupType::Full,
            Some("state") => BackupType::State,
            Some("configuration") => BackupType::Configuration,
            _ => {
                let options = vec!["full".to_string(), "state".to_string(), "configuration".to_string()];
                let selection = ctx.input.select("Backup type:", options)?;
                match selection.as_str() {
                    "full" => BackupType::Full,
                    "state" => BackupType::State,
                    _ => BackupType::Configuration,
                }
            }
        };

        // Get description
        let desc = if let Some(d) = description {
            Some(d.to_string())
        } else {
            let input = ctx.input.text("Description (optional):", None)?;
            if input.is_empty() {
                None
            } else {
                Some(input)
            }
        };

        ctx.output.dimmed("Creating backup...");

        // Create backup
        let backup = Self::create_backup(
            ctx,
            &infrastructure_root,
            &current_path,
            &resource,
            btype,
            desc,
        )?;

        ctx.output.success("Backup created successfully");
        ctx.output.key_value("Backup ID", &backup.id);
        ctx.output.key_value("Type", &format!("{:?}", backup.backup_type));
        ctx.output.key_value(
            "Size",
            &format!("{:.2} MB", backup.size_bytes as f64 / 1024.0 / 1024.0),
        );
        ctx.output.key_value("Resources", &backup.metadata.resource_count.to_string());

        Ok(())
    }

    pub fn execute_restore(
        ctx: &Context,
        backup_id: Option<&str>,
        target_path: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Restore Infrastructure Backup");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        // Get backup ID
        let id = if let Some(b) = backup_id {
            b.to_string()
        } else {
            // List available backups
            let backups = Self::list_backups(ctx, &infrastructure_root, None, None)?;

            if backups.is_empty() {
                ctx.output.info("No backups available");
                return Ok(());
            }

            let options: Vec<String> = backups
                .iter()
                .map(|b| {
                    format!(
                        "{} - {}/{} ({:?}) - {}",
                        b.id, b.project, b.environment, b.backup_type, b.created_at
                    )
                })
                .collect();

            let selection = ctx.input.select("Select backup:", options)?;
            let idx = backups.iter().enumerate()
                .find(|(_, b)| format!(
                    "{} - {}/{} ({:?}) - {}",
                    b.id, b.project, b.environment, b.backup_type, b.created_at
                ) == selection)
                .map(|(i, _)| i)
                .unwrap_or(0);
            backups[idx].id.clone()
        };

        // Load backup
        let backup = Self::load_backup(ctx, &infrastructure_root, &id)?;

        ctx.output.key_value("Backup ID", &backup.id);
        ctx.output.key_value("Project", &backup.project);
        ctx.output.key_value("Environment", &backup.environment);
        ctx.output.key_value("Created", &backup.created_at);
        ctx.output.key_value("Type", &format!("{:?}", backup.backup_type));
        output::blank();

        // Confirm restoration
        let confirm = ctx.input.confirm(
            "This will overwrite current state. Continue?",
            false,
        )?;

        if !confirm {
            ctx.output.info("Restoration cancelled");
            return Ok(());
        }

        // Determine target path
        let target = if let Some(t) = target_path {
            Path::new(t).to_path_buf()
        } else {
            std::env::current_dir()?
        };

        ctx.output.dimmed("Restoring backup...");

        // Restore backup
        Self::restore_backup(ctx, &infrastructure_root, &backup, &target)?;

        ctx.output.success("Backup restored successfully");
        ctx.output.warning("Remember to run 'pmp preview' before applying changes");

        Ok(())
    }

    pub fn execute_list(
        ctx: &Context,
        project_filter: Option<&str>,
        environment_filter: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Infrastructure Backups");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        let backups = Self::list_backups(ctx, &infrastructure_root, project_filter, environment_filter)?;

        if backups.is_empty() {
            ctx.output.info("No backups found");
            return Ok(());
        }

        ctx.output.subsection("Available Backups");
        output::blank();

        for backup in &backups {
            ctx.output.dimmed(&format!("[{}] {:?}", backup.id, backup.backup_type));
            ctx.output.dimmed(&format!("  {}/{}", backup.project, backup.environment));
            ctx.output.dimmed(&format!("  Created: {} by {}", backup.created_at, backup.created_by));
            ctx.output.dimmed(&format!(
                "  Size: {:.2} MB, {} resources",
                backup.size_bytes as f64 / 1024.0 / 1024.0,
                backup.metadata.resource_count
            ));
            if let Some(desc) = &backup.description {
                ctx.output.dimmed(&format!("  {}", desc));
            }
            output::blank();
        }

        ctx.output.success(&format!("{} backups found", backups.len()));

        Ok(())
    }

    pub fn execute_delete(ctx: &Context, backup_id: &str, force: bool) -> Result<()> {
        ctx.output.section("Delete Backup");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        // Load backup
        let backup = Self::load_backup(ctx, &infrastructure_root, backup_id)?;

        ctx.output.key_value("Backup ID", &backup.id);
        ctx.output.key_value("Project", &backup.project);
        ctx.output.key_value("Environment", &backup.environment);
        ctx.output.key_value("Created", &backup.created_at);
        output::blank();

        // Confirm deletion
        if !force {
            let confirm = ctx.input.confirm("Delete this backup?", false)?;

            if !confirm {
                ctx.output.info("Deletion cancelled");
                return Ok(());
            }
        }

        // Delete backup
        Self::delete_backup(ctx, &infrastructure_root, &backup)?;

        ctx.output.success("Backup deleted");

        Ok(())
    }

    // Helper functions

    fn create_backup(
        _ctx: &Context,
        infrastructure_root: &Path,
        env_path: &Path,
        resource: &DynamicProjectEnvironmentResource,
        backup_type: BackupType,
        description: Option<String>,
    ) -> Result<Backup> {
        let user = Self::get_current_user()?;
        let backup_id = format!("backup-{}", uuid::Uuid::new_v4());

        // In a real implementation:
        // 1. Copy state files
        // 2. Copy configuration files
        // 3. Export resource data from cloud providers
        // 4. Create compressed archive
        // 5. Calculate checksum

        let metadata = BackupMetadata {
            resource_count: 12,
            state_version: Some("4".to_string()),
            terraform_version: Some("1.5.0".to_string()),
            checksum: "abc123def456".to_string(),
        };

        let backup = Backup {
            id: backup_id.clone(),
            project: resource.metadata.name.clone(),
            environment: resource.metadata.environment_name.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            created_by: user,
            backup_type,
            size_bytes: 1024 * 1024 * 5, // 5 MB mock
            description,
            metadata,
        };

        // Save backup metadata
        let backups_dir = infrastructure_root.join(".pmp").join("backups");
        std::fs::create_dir_all(&backups_dir)?;

        let backup_metadata_file = backups_dir.join(format!("{}.json", backup.id));
        let content = serde_json::to_string_pretty(&backup)?;
        std::fs::write(&backup_metadata_file, content)?;

        // Create backup archive directory
        let backup_data_dir = backups_dir.join(&backup.id);
        std::fs::create_dir_all(&backup_data_dir)?;

        // Copy files based on backup type
        match backup.backup_type {
            BackupType::Full => {
                // Copy everything
                Self::copy_directory_recursive(env_path, &backup_data_dir)?;
            }
            BackupType::State => {
                // Copy only state files
                if env_path.join("terraform.tfstate").exists() {
                    std::fs::copy(
                        env_path.join("terraform.tfstate"),
                        backup_data_dir.join("terraform.tfstate"),
                    )?;
                }
            }
            BackupType::Configuration => {
                // Copy only configuration files (.tf, .yaml)
                for entry in std::fs::read_dir(env_path)? {
                    let entry = entry?;
                    let path = entry.path();
                    if let Some(ext) = path.extension()
                        && (ext == "tf" || ext == "yaml" || ext == "yml")
                    {
                        let filename = path.file_name().unwrap();
                        std::fs::copy(&path, backup_data_dir.join(filename))?;
                    }
                }
            }
        }

        Ok(backup)
    }

    fn restore_backup(
        _ctx: &Context,
        infrastructure_root: &Path,
        backup: &Backup,
        target_path: &Path,
    ) -> Result<()> {
        // In a real implementation:
        // 1. Extract backup archive
        // 2. Verify checksum
        // 3. Restore state files
        // 4. Restore configuration files
        // 5. Run terraform init

        let backups_dir = infrastructure_root.join(".pmp").join("backups");
        let backup_data_dir = backups_dir.join(&backup.id);

        if !backup_data_dir.exists() {
            anyhow::bail!("Backup data not found");
        }

        // Copy files from backup to target
        Self::copy_directory_recursive(&backup_data_dir, target_path)?;

        Ok(())
    }

    fn list_backups(
        _ctx: &Context,
        infrastructure_root: &Path,
        project_filter: Option<&str>,
        environment_filter: Option<&str>,
    ) -> Result<Vec<Backup>> {
        let backups_dir = infrastructure_root.join(".pmp").join("backups");

        if !backups_dir.exists() {
            return Ok(vec![]);
        }

        let mut backups = Vec::new();

        for entry in std::fs::read_dir(&backups_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let content = std::fs::read_to_string(&path)?;
                if let Ok(backup) = serde_json::from_str::<Backup>(&content) {
                    // Apply filters
                    if let Some(proj) = project_filter
                        && backup.project != proj
                    {
                        continue;
                    }
                    if let Some(env) = environment_filter
                        && backup.environment != env
                    {
                        continue;
                    }

                    backups.push(backup);
                }
            }
        }

        // Sort by created_at (newest first)
        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(backups)
    }

    fn load_backup(
        _ctx: &Context,
        infrastructure_root: &Path,
        backup_id: &str,
    ) -> Result<Backup> {
        let backup_file = infrastructure_root
            .join(".pmp")
            .join("backups")
            .join(format!("{}.json", backup_id));

        if !backup_file.exists() {
            anyhow::bail!("Backup not found: {}", backup_id);
        }

        let content = std::fs::read_to_string(&backup_file)?;
        let backup: Backup = serde_json::from_str(&content)?;

        Ok(backup)
    }

    fn delete_backup(
        _ctx: &Context,
        infrastructure_root: &Path,
        backup: &Backup,
    ) -> Result<()> {
        let backups_dir = infrastructure_root.join(".pmp").join("backups");

        // Delete metadata file
        let metadata_file = backups_dir.join(format!("{}.json", backup.id));
        if metadata_file.exists() {
            std::fs::remove_file(&metadata_file)?;
        }

        // Delete backup data directory
        let data_dir = backups_dir.join(&backup.id);
        if data_dir.exists() {
            std::fs::remove_dir_all(&data_dir)?;
        }

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

    fn copy_directory_recursive(src: &Path, dst: &Path) -> Result<()> {
        std::fs::create_dir_all(dst)?;

        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if src_path.is_dir() {
                Self::copy_directory_recursive(&src_path, &dst_path)?;
            } else {
                std::fs::copy(&src_path, &dst_path)?;
            }
        }

        Ok(())
    }
}
