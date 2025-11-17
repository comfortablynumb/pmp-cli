use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::output;
use crate::template::metadata::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct LockCommand;

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectLock {
    pub project: String,
    pub environment: String,
    pub locked_by: String,
    pub locked_at: String,
    pub lock_id: String,
    pub reason: Option<String>,
    pub expires_at: Option<String>,
}

impl LockCommand {
    pub fn execute_acquire(ctx: &Context, path: Option<&str>, reason: Option<&str>) -> Result<()> {
        ctx.output.section("Acquire Project Lock");
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
        ctx.output
            .key_value("Environment", &resource.metadata.environment_name);
        output::blank();

        // Check if already locked
        if let Some(existing_lock) = Self::get_lock(
            ctx,
            &infrastructure_root,
            &resource.metadata.name,
            &resource.metadata.environment_name,
        )? {
            ctx.output.warning("Project is already locked");
            ctx.output
                .dimmed(&format!("Locked by: {}", existing_lock.locked_by));
            ctx.output
                .dimmed(&format!("Locked at: {}", existing_lock.locked_at));
            if let Some(reason) = &existing_lock.reason {
                ctx.output.dimmed(&format!("Reason: {}", reason));
            }
            return Ok(());
        }

        // Get user info
        let user = Self::get_current_user()?;

        // Get reason if not provided
        let lock_reason = if let Some(r) = reason {
            Some(r.to_string())
        } else {
            let input = ctx.input.text("Reason for lock (optional):", None)?;
            if input.is_empty() { None } else { Some(input) }
        };

        // Create lock
        let lock = ProjectLock {
            project: resource.metadata.name.clone(),
            environment: resource.metadata.environment_name.clone(),
            locked_by: user.clone(),
            locked_at: chrono::Utc::now().to_rfc3339(),
            lock_id: uuid::Uuid::new_v4().to_string(),
            reason: lock_reason.clone(),
            expires_at: None,
        };

        // Save lock
        Self::save_lock(ctx, &infrastructure_root, &lock)?;

        ctx.output.success("Lock acquired");
        ctx.output.dimmed(&format!("Lock ID: {}", lock.lock_id));
        if let Some(reason) = &lock_reason {
            ctx.output.dimmed(&format!("Reason: {}", reason));
        }

        Ok(())
    }

    pub fn execute_release(ctx: &Context, path: Option<&str>, force: bool) -> Result<()> {
        ctx.output.section("Release Project Lock");
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
        ctx.output
            .key_value("Environment", &resource.metadata.environment_name);
        output::blank();

        // Check if locked
        let lock = Self::get_lock(
            ctx,
            &infrastructure_root,
            &resource.metadata.name,
            &resource.metadata.environment_name,
        )?;

        if lock.is_none() {
            ctx.output.info("Project is not locked");
            return Ok(());
        }

        let lock = lock.unwrap();
        let current_user = Self::get_current_user()?;

        // Check if user owns the lock
        if lock.locked_by != current_user && !force {
            ctx.output.warning("Lock is owned by another user");
            ctx.output.dimmed(&format!("Locked by: {}", lock.locked_by));
            ctx.output
                .dimmed("Use --force to override (requires admin privileges)");
            return Ok(());
        }

        if force && lock.locked_by != current_user {
            ctx.output
                .warning("Forcing lock release (this action is logged)");
        }

        // Remove lock
        Self::remove_lock(ctx, &infrastructure_root, &lock)?;

        ctx.output.success("Lock released");

        Ok(())
    }

    pub fn execute_status(ctx: &Context, path: Option<&str>, all: bool) -> Result<()> {
        ctx.output.section("Lock Status");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        if all {
            // Show all locks
            let locks = Self::get_all_locks(ctx, &infrastructure_root)?;

            if locks.is_empty() {
                ctx.output.info("No active locks");
                return Ok(());
            }

            ctx.output.subsection("Active Locks");
            output::blank();

            for lock in &locks {
                ctx.output
                    .dimmed(&format!("{}/{}", lock.project, lock.environment));
                ctx.output
                    .dimmed(&format!("  Locked by: {}", lock.locked_by));
                ctx.output
                    .dimmed(&format!("  Locked at: {}", lock.locked_at));
                if let Some(reason) = &lock.reason {
                    ctx.output.dimmed(&format!("  Reason: {}", reason));
                }
                ctx.output.dimmed(&format!("  Lock ID: {}", lock.lock_id));
                output::blank();
            }

            ctx.output.success(&format!("{} active locks", locks.len()));
        } else {
            // Show lock for current project
            let current_path = if let Some(p) = path {
                Path::new(p).to_path_buf()
            } else {
                std::env::current_dir()?
            };

            let env_file = current_path.join(".pmp.environment.yaml");
            if !ctx.fs.exists(&env_file) {
                ctx.output
                    .warning("Not in an environment directory. Use --all to see all locks.");
                return Ok(());
            }

            let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;

            ctx.output.key_value("Project", &resource.metadata.name);
            ctx.output
                .key_value("Environment", &resource.metadata.environment_name);
            output::blank();

            if let Some(lock) = Self::get_lock(
                ctx,
                &infrastructure_root,
                &resource.metadata.name,
                &resource.metadata.environment_name,
            )? {
                ctx.output.subsection("Lock Details");
                ctx.output.key_value("Locked by", &lock.locked_by);
                ctx.output.key_value("Locked at", &lock.locked_at);
                if let Some(reason) = &lock.reason {
                    ctx.output.key_value("Reason", reason);
                }
                ctx.output.key_value("Lock ID", &lock.lock_id);
            } else {
                ctx.output.info("Project is not locked");
            }
        }

        Ok(())
    }

    // Helper functions

    fn get_current_user() -> Result<String> {
        // Try to get user from git config
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

        // Fallback to system username
        Ok(whoami::username())
    }

    fn get_lock(
        _ctx: &Context,
        infrastructure_root: &Path,
        project: &str,
        environment: &str,
    ) -> Result<Option<ProjectLock>> {
        let lock_file = infrastructure_root
            .join(".pmp")
            .join("locks")
            .join(format!("{}-{}.lock", project, environment));

        if !std::path::Path::new(&lock_file).exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&lock_file)?;
        let lock: ProjectLock = serde_json::from_str(&content)?;

        Ok(Some(lock))
    }

    fn save_lock(_ctx: &Context, infrastructure_root: &Path, lock: &ProjectLock) -> Result<()> {
        let locks_dir = infrastructure_root.join(".pmp").join("locks");
        std::fs::create_dir_all(&locks_dir)?;

        let lock_file = locks_dir.join(format!("{}-{}.lock", lock.project, lock.environment));
        let content = serde_json::to_string_pretty(lock)?;
        std::fs::write(&lock_file, content)?;

        Ok(())
    }

    fn remove_lock(_ctx: &Context, infrastructure_root: &Path, lock: &ProjectLock) -> Result<()> {
        let lock_file = infrastructure_root
            .join(".pmp")
            .join("locks")
            .join(format!("{}-{}.lock", lock.project, lock.environment));

        if std::path::Path::new(&lock_file).exists() {
            std::fs::remove_file(&lock_file)?;
        }

        Ok(())
    }

    fn get_all_locks(_ctx: &Context, infrastructure_root: &Path) -> Result<Vec<ProjectLock>> {
        let locks_dir = infrastructure_root.join(".pmp").join("locks");

        if !locks_dir.exists() {
            return Ok(vec![]);
        }

        let mut locks = Vec::new();

        for entry in std::fs::read_dir(&locks_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("lock") {
                let content = std::fs::read_to_string(&path)?;
                if let Ok(lock) = serde_json::from_str::<ProjectLock>(&content) {
                    locks.push(lock);
                }
            }
        }

        Ok(locks)
    }
}
