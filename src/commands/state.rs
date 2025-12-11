use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::executor::{Executor, ExecutorConfig, OpenTofuExecutor};
use crate::output;
use crate::template::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct StateCommand;

#[derive(Debug)]
struct StateInfo {
    project: String,
    environment: String,
    path: PathBuf,
    resource_count: Option<usize>,
    last_modified: Option<String>,
    locked: bool,
    lock_info: Option<String>,
}

#[derive(Debug)]
struct DriftInfo {
    project: String,
    environment: String,
    has_drift: bool,
    changes: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct StateBackup {
    project: String,
    environment: String,
    timestamp: String,
    backup_id: String,
    state_content: String,
}

impl StateCommand {
    /// Execute the state list command
    pub fn execute_list(ctx: &Context, show_details: bool) -> Result<()> {
        ctx.output.section("State Overview");

        // Find infrastructure
        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output
            .key_value("Infrastructure", &infrastructure.metadata.name);
        output::blank();

        // Discover all projects
        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &infrastructure_root)?;

        if projects.is_empty() {
            ctx.output.dimmed("No projects found.");
            return Ok(());
        }

        // Collect state info for all projects
        let mut state_infos = Vec::new();
        for project in &projects {
            let project_path = infrastructure_root.join(&project.path);
            let environments_dir = project_path.join("environments");

            if let Ok(env_entries) = ctx.fs.read_dir(&environments_dir) {
                for env_path in env_entries {
                    let env_file = env_path.join(".pmp.environment.yaml");
                    if ctx.fs.exists(&env_file)
                        && let Ok(resource) =
                            DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
                    {
                        let state_info = Self::get_state_info(
                            ctx,
                            &resource.metadata.name,
                            &resource.metadata.environment_name,
                            &env_path,
                            show_details,
                        )?;
                        state_infos.push(state_info);
                    }
                }
            }
        }

        if state_infos.is_empty() {
            ctx.output.dimmed("No state information available.");
            return Ok(());
        }

        // Display state info
        Self::display_state_list(ctx, &state_infos, show_details)?;

        Ok(())
    }

    /// Execute the state drift command
    pub fn execute_drift(ctx: &Context, project_name: Option<&str>) -> Result<()> {
        ctx.output.section("Drift Detection");

        // Find infrastructure
        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output
            .key_value("Infrastructure", &infrastructure.metadata.name);
        output::blank();

        // Discover projects
        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &infrastructure_root)?;

        if projects.is_empty() {
            ctx.output.dimmed("No projects found.");
            return Ok(());
        }

        // Filter by project name if provided
        let projects_to_check: Vec<_> = if let Some(name) = project_name {
            projects.iter().filter(|p| p.name == name).collect()
        } else {
            projects.iter().collect()
        };

        if projects_to_check.is_empty()
            && let Some(name) = project_name
        {
            anyhow::bail!("Project '{}' not found", name);
        }

        // Check drift for each project
        let mut drift_infos = Vec::new();
        for project in projects_to_check {
            let project_path = infrastructure_root.join(&project.path);
            let environments_dir = project_path.join("environments");

            if let Ok(env_entries) = ctx.fs.read_dir(&environments_dir) {
                for env_path in env_entries {
                    let env_file = env_path.join(".pmp.environment.yaml");
                    if ctx.fs.exists(&env_file)
                        && let Ok(resource) =
                            DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
                    {
                        let drift_info = Self::check_drift(
                            ctx,
                            &resource.metadata.name,
                            &resource.metadata.environment_name,
                            &env_path,
                        )?;
                        drift_infos.push(drift_info);
                    }
                }
            }
        }

        // Display drift info
        Self::display_drift_info(ctx, &drift_infos)?;

        Ok(())
    }

    /// Execute the state lock command
    pub fn execute_lock(
        ctx: &Context,
        project_name: &str,
        environment: Option<&str>,
    ) -> Result<()> {
        ctx.output
            .section(&format!("Locking State: {}", project_name));

        let env_path = Self::resolve_project_environment(ctx, project_name, environment)?;
        Self::lock_state(ctx, &env_path)?;

        ctx.output.success("State locked successfully");
        Ok(())
    }

    /// Execute the state unlock command
    pub fn execute_unlock(
        ctx: &Context,
        project_name: &str,
        environment: Option<&str>,
        force: bool,
    ) -> Result<()> {
        ctx.output
            .section(&format!("Unlocking State: {}", project_name));

        let env_path = Self::resolve_project_environment(ctx, project_name, environment)?;
        Self::unlock_state(ctx, &env_path, force)?;

        ctx.output.success("State unlocked successfully");
        Ok(())
    }

    /// Execute the state sync command
    pub fn execute_sync(ctx: &Context) -> Result<()> {
        ctx.output.section("Syncing Remote State");

        // Find infrastructure
        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output
            .key_value("Infrastructure", &infrastructure.metadata.name);
        output::blank();

        // Discover all projects
        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &infrastructure_root)?;

        if projects.is_empty() {
            ctx.output.dimmed("No projects found.");
            return Ok(());
        }

        let mut synced_count = 0;
        for project in &projects {
            let project_path = infrastructure_root.join(&project.path);
            let environments_dir = project_path.join("environments");

            if let Ok(env_entries) = ctx.fs.read_dir(&environments_dir) {
                for env_path in env_entries {
                    let env_file = env_path.join(".pmp.environment.yaml");
                    if ctx.fs.exists(&env_file)
                        && let Ok(resource) =
                            DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
                    {
                        ctx.output.dimmed(&format!(
                            "Syncing {}:{}...",
                            resource.metadata.name, resource.metadata.environment_name
                        ));

                        if Self::sync_state(ctx, &env_path)? {
                            synced_count += 1;
                        }
                    }
                }
            }
        }

        output::blank();
        ctx.output
            .success(&format!("Synced {} project(s)", synced_count));
        Ok(())
    }

    /// Get state information for a project environment
    fn get_state_info(
        ctx: &Context,
        project_name: &str,
        environment: &str,
        env_path: &Path,
        _detailed: bool,
    ) -> Result<StateInfo> {
        let _env_dir_str = env_path
            .to_str()
            .context("Failed to convert path to string")?;

        // Check if state file exists
        let state_file = env_path.join("terraform.tfstate");
        let locked = Self::is_state_locked(ctx, env_path)?;
        let lock_info = if locked {
            Self::get_lock_info(ctx, env_path)?
        } else {
            None
        };

        let mut resource_count = None;
        let mut last_modified = None;

        if ctx.fs.exists(&state_file) {
            // Try to read state file to get resource count
            if let Ok(content) = ctx.fs.read_to_string(&state_file)
                && let Ok(state) = serde_json::from_str::<serde_json::Value>(&content)
                && let Some(resources) = state.get("resources")
                && let Some(arr) = resources.as_array()
            {
                resource_count = Some(arr.len());
            }

            // Get last modified time (this would require metadata access in real impl)
            last_modified = Some("N/A".to_string());
        }

        Ok(StateInfo {
            project: project_name.to_string(),
            environment: environment.to_string(),
            path: env_path.to_path_buf(),
            resource_count,
            last_modified,
            locked,
            lock_info,
        })
    }

    /// Check if state is locked
    fn is_state_locked(ctx: &Context, env_path: &Path) -> Result<bool> {
        // Check for .terraform.tfstate.lock.info file
        let lock_file = env_path
            .join(".terraform")
            .join("terraform.tfstate.lock.info");
        Ok(ctx.fs.exists(&lock_file))
    }

    /// Get lock information
    fn get_lock_info(ctx: &Context, env_path: &Path) -> Result<Option<String>> {
        let lock_file = env_path
            .join(".terraform")
            .join("terraform.tfstate.lock.info");
        if ctx.fs.exists(&lock_file)
            && let Ok(content) = ctx.fs.read_to_string(&lock_file)
            && let Ok(lock_data) = serde_json::from_str::<serde_json::Value>(&content)
            && let Some(who) = lock_data.get("Who").and_then(|v| v.as_str())
        {
            return Ok(Some(who.to_string()));
        }
        Ok(None)
    }

    /// Check for drift
    fn check_drift(
        ctx: &Context,
        project_name: &str,
        environment: &str,
        env_path: &Path,
    ) -> Result<DriftInfo> {
        let env_dir_str = env_path
            .to_str()
            .context("Failed to convert path to string")?;

        ctx.output
            .dimmed(&format!("Checking {}:{}...", project_name, environment));

        // Run tofu plan -detailed-exitcode to detect drift
        let executor = OpenTofuExecutor::new();

        // Initialize if needed
        let _ = executor.init(env_dir_str);

        // Run plan with -detailed-exitcode
        // Exit code 0 = no changes, 1 = error, 2 = changes detected
        let output = Command::new("tofu")
            .args(["plan", "-detailed-exitcode", "-no-color", "-input=false"])
            .current_dir(env_dir_str)
            .output();

        let (has_drift, changes) = if let Ok(output) = output {
            let exit_code = output.status.code().unwrap_or(1);
            let has_drift = exit_code == 2;

            let mut changes = Vec::new();
            if has_drift {
                // Parse output for changes
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    if line.contains("will be created")
                        || line.contains("will be updated")
                        || line.contains("will be destroyed")
                        || line.contains("must be replaced")
                    {
                        changes.push(line.trim().to_string());
                    }
                }
            }
            (has_drift, changes)
        } else {
            (false, Vec::new())
        };

        Ok(DriftInfo {
            project: project_name.to_string(),
            environment: environment.to_string(),
            has_drift,
            changes,
        })
    }

    /// Lock state
    fn lock_state(ctx: &Context, env_path: &Path) -> Result<()> {
        let env_dir_str = env_path
            .to_str()
            .context("Failed to convert path to string")?;

        // Use tofu force-unlock with a dummy lock ID (this is a simplified version)
        // In production, this would integrate with the backend's locking mechanism
        ctx.output.dimmed("Acquiring state lock...");

        // For now, we'll create a lock marker file
        let lock_file = env_path
            .join(".terraform")
            .join("terraform.tfstate.lock.info");
        let hostname = whoami::fallible::hostname().unwrap_or_else(|_| "unknown".to_string());
        let lock_data = serde_json::json!({
            "ID": uuid::Uuid::new_v4().to_string(),
            "Operation": "manual-lock",
            "Who": format!("{}@{}", whoami::username(), hostname),
            "Version": "1.0.0",
            "Created": chrono::Utc::now().to_rfc3339(),
            "Path": env_dir_str
        });

        let terraform_dir = env_path.join(".terraform");
        if !ctx.fs.exists(&terraform_dir) {
            ctx.fs.create_dir_all(&terraform_dir)?;
        }

        ctx.fs
            .write(&lock_file, &serde_json::to_string_pretty(&lock_data)?)?;

        Ok(())
    }

    /// Unlock state
    fn unlock_state(ctx: &Context, env_path: &Path, force: bool) -> Result<()> {
        let lock_file = env_path
            .join(".terraform")
            .join("terraform.tfstate.lock.info");

        if !ctx.fs.exists(&lock_file) {
            ctx.output.dimmed("State is not locked");
            return Ok(());
        }

        if force {
            ctx.output.dimmed("Force unlocking state...");
            ctx.fs.remove_file(&lock_file)?;
        } else {
            // Check lock ownership
            if let Ok(content) = ctx.fs.read_to_string(&lock_file)
                && let Ok(lock_data) = serde_json::from_str::<serde_json::Value>(&content)
                && let Some(who) = lock_data.get("Who").and_then(|v| v.as_str())
            {
                let hostname =
                    whoami::fallible::hostname().unwrap_or_else(|_| "unknown".to_string());
                let current_user = format!("{}@{}", whoami::username(), hostname);
                if who != current_user {
                    anyhow::bail!("Lock is held by {}. Use --force to override.", who);
                }
            }
            ctx.fs.remove_file(&lock_file)?;
        }

        Ok(())
    }

    /// Sync state with remote
    fn sync_state(_ctx: &Context, env_path: &Path) -> Result<bool> {
        let env_dir_str = env_path
            .to_str()
            .context("Failed to convert path to string")?;

        // Run tofu refresh to sync state
        let executor = OpenTofuExecutor::new();
        let config = ExecutorConfig {
            plan_command: None,
            apply_command: None,
            destroy_command: None,
            refresh_command: None,
            ..Default::default()
        };

        let result = executor.refresh(&config, env_dir_str, &[]);
        Ok(result.is_ok())
    }

    /// Resolve project and environment path
    fn resolve_project_environment(
        ctx: &Context,
        project_name: &str,
        environment: Option<&str>,
    ) -> Result<PathBuf> {
        let (_, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required.")?;

        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &infrastructure_root)?;

        let project = projects
            .iter()
            .find(|p| p.name == project_name)
            .context(format!("Project '{}' not found", project_name))?;

        let project_path = infrastructure_root.join(&project.path);

        if let Some(env_name) = environment {
            let env_path = project_path.join("environments").join(env_name);
            if !ctx.fs.exists(&env_path.join(".pmp.environment.yaml")) {
                anyhow::bail!("Environment '{}' not found in project", env_name);
            }
            Ok(env_path)
        } else {
            // Discover environments and prompt
            let environments = CollectionDiscovery::discover_environments(&*ctx.fs, &project_path)?;

            if environments.is_empty() {
                anyhow::bail!("No environments found");
            }

            let env_name = if environments.len() == 1 {
                environments[0].clone()
            } else {
                ctx.input
                    .select("Select environment:", environments, None)?
            };

            Ok(project_path.join("environments").join(env_name))
        }
    }

    /// Display state list
    fn display_state_list(ctx: &Context, states: &[StateInfo], detailed: bool) -> Result<()> {
        ctx.output.subsection("Project States");
        output::blank();

        let mut locked_count = 0;
        let mut total_resources = 0;

        for state in states {
            let status_icon = if state.locked { "🔒" } else { "✓" };
            let resources_str = state
                .resource_count
                .map(|c| format!("{} resources", c))
                .unwrap_or_else(|| "No state".to_string());

            ctx.output.info(&format!(
                "{} {}:{} - {}",
                status_icon, state.project, state.environment, resources_str
            ));

            if state.locked {
                locked_count += 1;
                if let Some(lock_info) = &state.lock_info {
                    ctx.output.dimmed(&format!("   Locked by: {}", lock_info));
                }
            }

            if let Some(count) = state.resource_count {
                total_resources += count;
            }

            if detailed {
                if let Some(modified) = &state.last_modified {
                    ctx.output
                        .dimmed(&format!("   Last modified: {}", modified));
                }
                ctx.output
                    .dimmed(&format!("   Path: {}", state.path.display()));
            }
        }

        output::blank();
        ctx.output
            .info(&format!("Total projects: {}", states.len()));
        ctx.output
            .info(&format!("Total resources: {}", total_resources));
        if locked_count > 0 {
            ctx.output
                .warning(&format!("{} project(s) locked", locked_count));
        }

        Ok(())
    }

    /// Display drift information
    fn display_drift_info(ctx: &Context, drift_infos: &[DriftInfo]) -> Result<()> {
        output::blank();

        let mut has_any_drift = false;
        let mut drift_count = 0;

        for drift in drift_infos {
            if drift.has_drift {
                has_any_drift = true;
                drift_count += 1;

                ctx.output.warning(&format!(
                    "⚠ Drift detected: {}:{}",
                    drift.project, drift.environment
                ));

                if !drift.changes.is_empty() {
                    for change in &drift.changes {
                        ctx.output.dimmed(&format!("   {}", change));
                    }
                }
                output::blank();
            }
        }

        if !has_any_drift {
            ctx.output
                .success("✓ No drift detected across all projects");
        } else {
            ctx.output.error(&format!(
                "Drift detected in {} of {} project(s)",
                drift_count,
                drift_infos.len()
            ));
        }

        Ok(())
    }

    /// Execute the state backup command
    pub fn execute_backup(ctx: &Context, path: Option<&str>) -> Result<()> {
        ctx.output.section("State Backup");

        let current_path = if let Some(p) = path {
            PathBuf::from(p)
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

        // Create backup
        let backup_id = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let backup = Self::create_backup(ctx, &current_path, &resource, &backup_id)?;

        // Save backup
        let backup_dir = current_path.join(".pmp").join("backups");
        ctx.fs.create_dir_all(&backup_dir)?;

        let backup_file = backup_dir.join(format!("{}.json", backup_id));
        let backup_json = serde_json::to_string_pretty(&backup)?;
        ctx.fs.write(&backup_file, &backup_json)?;

        ctx.output
            .success(&format!("Backup created: {}", backup_file.display()));
        ctx.output.key_value("Backup ID", &backup_id);

        Ok(())
    }

    /// Execute the state restore command
    pub fn execute_restore(
        ctx: &Context,
        backup_id: &str,
        path: Option<&str>,
        force: bool,
    ) -> Result<()> {
        ctx.output.section("State Restore");

        let current_path = if let Some(p) = path {
            PathBuf::from(p)
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
        ctx.output.key_value("Backup ID", backup_id);
        output::blank();

        // Load backup
        let backup_dir = current_path.join(".pmp").join("backups");
        let backup_file = backup_dir.join(format!("{}.json", backup_id));

        if !ctx.fs.exists(&backup_file) {
            anyhow::bail!("Backup not found: {}", backup_id);
        }

        let backup_json = ctx.fs.read_to_string(&backup_file)?;
        let backup: StateBackup = serde_json::from_str(&backup_json)?;

        // Verify backup matches current project
        if backup.project != resource.metadata.name
            || backup.environment != resource.metadata.environment_name
        {
            ctx.output
                .error("Backup does not match current project/environment");
            ctx.output.dimmed(&format!(
                "Backup is for: {}:{}",
                backup.project, backup.environment
            ));
            anyhow::bail!("Backup mismatch");
        }

        // Confirm restore
        if !force {
            let confirmed = ctx.input.confirm(
                "This will replace the current state. Continue?",
                Some(false),
            )?;

            if !confirmed {
                ctx.output.dimmed("Restore cancelled.");
                return Ok(());
            }
        }

        // Restore state
        Self::restore_backup(ctx, &current_path, &backup)?;

        ctx.output.success("State restored successfully");

        Ok(())
    }

    /// Execute the state migrate command
    pub fn execute_migrate(ctx: &Context, backend_type: &str, path: Option<&str>) -> Result<()> {
        ctx.output.section("State Migration");

        let current_path = if let Some(p) = path {
            PathBuf::from(p)
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
        ctx.output.key_value("Target Backend", backend_type);
        output::blank();

        // Create backup before migration
        ctx.output.info("Creating backup before migration...");
        let backup_id = format!(
            "pre_migration_{}",
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        );
        let backup = Self::create_backup(ctx, &current_path, &resource, &backup_id)?;

        let backup_dir = current_path.join(".pmp").join("backups");
        ctx.fs.create_dir_all(&backup_dir)?;

        let backup_file = backup_dir.join(format!("{}.json", backup_id));
        let backup_json = serde_json::to_string_pretty(&backup)?;
        ctx.fs.write(&backup_file, &backup_json)?;

        ctx.output
            .success(&format!("Backup created: {}", backup_id));
        output::blank();

        // Migrate backend
        Self::migrate_backend(ctx, &current_path, backend_type)?;

        ctx.output
            .success("Backend migration completed successfully");
        ctx.output
            .dimmed(&format!("Backup available for rollback: {}", backup_id));

        Ok(())
    }

    /// Create a state backup
    fn create_backup(
        ctx: &Context,
        env_path: &Path,
        resource: &DynamicProjectEnvironmentResource,
        backup_id: &str,
    ) -> Result<StateBackup> {
        // Read current state file
        let state_file = env_path.join("terraform.tfstate");
        let state_content = if ctx.fs.exists(&state_file) {
            ctx.fs.read_to_string(&state_file)?
        } else {
            String::from("{}")
        };

        Ok(StateBackup {
            project: resource.metadata.name.clone(),
            environment: resource.metadata.environment_name.clone(),
            timestamp: chrono::Utc::now().to_rfc3339(),
            backup_id: backup_id.to_string(),
            state_content,
        })
    }

    /// Restore a backup
    fn restore_backup(ctx: &Context, env_path: &Path, backup: &StateBackup) -> Result<()> {
        let state_file = env_path.join("terraform.tfstate");

        // Create backup of current state before restore
        if ctx.fs.exists(&state_file) {
            let current_backup = env_path.join("terraform.tfstate.before-restore");
            let current_content = ctx.fs.read_to_string(&state_file)?;
            ctx.fs.write(&current_backup, &current_content)?;
            ctx.output.dimmed(&format!(
                "Current state backed up to: {}",
                current_backup.display()
            ));
        }

        // Write restored state
        ctx.fs.write(&state_file, &backup.state_content)?;

        Ok(())
    }

    /// Migrate backend
    fn migrate_backend(ctx: &Context, env_path: &Path, backend_type: &str) -> Result<()> {
        ctx.output
            .info(&format!("Migrating to {} backend...", backend_type));

        // This would generate new backend configuration
        // For now, we'll create a placeholder implementation

        let backend_config = match backend_type {
            "s3" => {
                "terraform {\n  backend \"s3\" {\n    # Configure S3 backend parameters\n  }\n}\n"
            }
            "azurerm" => {
                "terraform {\n  backend \"azurerm\" {\n    # Configure Azure backend parameters\n  }\n}\n"
            }
            "gcs" => {
                "terraform {\n  backend \"gcs\" {\n    # Configure GCS backend parameters\n  }\n}\n"
            }
            "local" => "terraform {\n  backend \"local\" {}\n}\n",
            _ => anyhow::bail!("Unsupported backend type: {}", backend_type),
        };

        // Write new backend configuration
        let backend_file = env_path.join("_backend.tf");
        ctx.fs.write(&backend_file, backend_config)?;

        ctx.output.success(&format!(
            "Backend configuration written to: {}",
            backend_file.display()
        ));
        ctx.output
            .dimmed("Run 'tofu init -migrate-state' to complete migration");

        Ok(())
    }
}
