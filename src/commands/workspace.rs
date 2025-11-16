use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::output;
use crate::template::metadata::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct WorkspaceCommand;

#[derive(Debug, Serialize, Deserialize)]
pub struct Workspace {
    pub name: String,
    pub created_at: String,
    pub created_by: String,
    pub description: Option<String>,
    pub project: String,
    pub environment: String,
    pub state_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub current_workspace: Option<String>,
    pub workspaces: Vec<String>,
}

impl WorkspaceCommand {
    pub fn execute_list(ctx: &Context, path: Option<&str>) -> Result<()> {
        ctx.output.section("Workspaces");
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

        // Get workspaces
        let config = Self::get_workspace_config(ctx, &infrastructure_root, &resource)?;
        let workspaces = Self::get_workspaces(ctx, &infrastructure_root, &resource)?;

        if workspaces.is_empty() {
            ctx.output.info("No workspaces found");
            ctx.output.dimmed("Use 'pmp workspace new' to create a workspace");
            return Ok(());
        }

        ctx.output.subsection("Available Workspaces");
        output::blank();

        for workspace in &workspaces {
            let current_marker = if Some(&workspace.name) == config.current_workspace.as_ref() {
                "* "
            } else {
                "  "
            };

            ctx.output.dimmed(&format!("{}{}", current_marker, workspace.name));
            if let Some(desc) = &workspace.description {
                ctx.output.dimmed(&format!("    {}", desc));
            }
            ctx.output.dimmed(&format!("    Created by: {} at {}", workspace.created_by, workspace.created_at));
        }

        output::blank();
        if let Some(current) = &config.current_workspace {
            ctx.output.key_value("Current workspace", current);
        } else {
            ctx.output.dimmed("No active workspace (using default)");
        }

        Ok(())
    }

    pub fn execute_new(ctx: &Context, name: &str, description: Option<&str>) -> Result<()> {
        ctx.output.section("Create Workspace");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        let current_path = std::env::current_dir()?;

        // Check if we're in an environment directory
        let env_file = current_path.join(".pmp.environment.yaml");
        if !ctx.fs.exists(&env_file) {
            ctx.output
                .warning("Not in an environment directory.");
            return Ok(());
        }

        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;

        ctx.output.key_value("Project", &resource.metadata.name);
        ctx.output.key_value("Environment", &resource.metadata.environment_name);
        ctx.output.key_value("Workspace", name);
        output::blank();

        // Check if workspace already exists
        let workspaces = Self::get_workspaces(ctx, &infrastructure_root, &resource)?;
        if workspaces.iter().any(|w| w.name == name) {
            ctx.output.warning("Workspace already exists");
            return Ok(());
        }

        // Get description if not provided
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

        // Create workspace
        let user = Self::get_current_user()?;
        let workspace = Workspace {
            name: name.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            created_by: user,
            description: desc,
            project: resource.metadata.name.clone(),
            environment: resource.metadata.environment_name.clone(),
            state_path: format!("workspaces/{}/terraform.tfstate", name),
        };

        // Save workspace
        Self::save_workspace(ctx, &infrastructure_root, &workspace)?;

        // Initialize workspace state directory
        Self::init_workspace_state(ctx, &infrastructure_root, &workspace)?;

        ctx.output.success("Workspace created");
        ctx.output.dimmed("Use 'pmp workspace select' to switch to this workspace");

        Ok(())
    }

    pub fn execute_select(ctx: &Context, name: &str) -> Result<()> {
        ctx.output.section("Select Workspace");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        let current_path = std::env::current_dir()?;

        // Check if we're in an environment directory
        let env_file = current_path.join(".pmp.environment.yaml");
        if !ctx.fs.exists(&env_file) {
            ctx.output
                .warning("Not in an environment directory.");
            return Ok(());
        }

        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;

        // Check if workspace exists
        let workspaces = Self::get_workspaces(ctx, &infrastructure_root, &resource)?;
        if !workspaces.iter().any(|w| w.name == name) {
            ctx.output.warning("Workspace does not exist");
            ctx.output.dimmed("Use 'pmp workspace list' to see available workspaces");
            return Ok(());
        }

        // Update config
        let mut config = Self::get_workspace_config(ctx, &infrastructure_root, &resource)?;
        config.current_workspace = Some(name.to_string());
        Self::save_workspace_config(ctx, &infrastructure_root, &resource, &config)?;

        ctx.output.success(&format!("Switched to workspace '{}'", name));

        Ok(())
    }

    pub fn execute_delete(ctx: &Context, name: &str, force: bool) -> Result<()> {
        ctx.output.section("Delete Workspace");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        let current_path = std::env::current_dir()?;

        // Check if we're in an environment directory
        let env_file = current_path.join(".pmp.environment.yaml");
        if !ctx.fs.exists(&env_file) {
            ctx.output
                .warning("Not in an environment directory.");
            return Ok(());
        }

        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;

        // Check if workspace exists
        let workspaces = Self::get_workspaces(ctx, &infrastructure_root, &resource)?;
        let workspace = workspaces.iter().find(|w| w.name == name);

        if workspace.is_none() {
            ctx.output.warning("Workspace does not exist");
            return Ok(());
        }

        // Check if it's the current workspace
        let config = Self::get_workspace_config(ctx, &infrastructure_root, &resource)?;
        if Some(name) == config.current_workspace.as_deref() {
            ctx.output.warning("Cannot delete current workspace");
            ctx.output.dimmed("Switch to another workspace first");
            return Ok(());
        }

        // Confirm deletion
        if !force {
            let confirm = ctx.input.confirm(
                &format!("Delete workspace '{}' and all its state?", name),
                false,
            )?;

            if !confirm {
                ctx.output.info("Deletion cancelled");
                return Ok(());
            }
        }

        // Delete workspace
        Self::delete_workspace(ctx, &infrastructure_root, workspace.unwrap())?;

        ctx.output.success("Workspace deleted");

        Ok(())
    }

    pub fn execute_show(ctx: &Context, name: Option<&str>) -> Result<()> {
        ctx.output.section("Workspace Details");
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        let current_path = std::env::current_dir()?;

        // Check if we're in an environment directory
        let env_file = current_path.join(".pmp.environment.yaml");
        if !ctx.fs.exists(&env_file) {
            ctx.output
                .warning("Not in an environment directory.");
            return Ok(());
        }

        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;

        // Get workspace name
        let workspace_name = if let Some(n) = name {
            n.to_string()
        } else {
            let config = Self::get_workspace_config(ctx, &infrastructure_root, &resource)?;
            config.current_workspace.unwrap_or_else(|| "default".to_string())
        };

        // Get workspace
        let workspaces = Self::get_workspaces(ctx, &infrastructure_root, &resource)?;
        let workspace = workspaces.iter().find(|w| w.name == workspace_name);

        if let Some(ws) = workspace {
            ctx.output.key_value("Name", &ws.name);
            ctx.output.key_value("Project", &ws.project);
            ctx.output.key_value("Environment", &ws.environment);
            ctx.output.key_value("Created by", &ws.created_by);
            ctx.output.key_value("Created at", &ws.created_at);
            if let Some(desc) = &ws.description {
                ctx.output.key_value("Description", desc);
            }
            ctx.output.key_value("State path", &ws.state_path);
        } else {
            ctx.output.info(&format!("Workspace '{}' not found", workspace_name));
        }

        Ok(())
    }

    // Helper functions

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

    fn get_workspace_config(
        _ctx: &Context,
        infrastructure_root: &Path,
        resource: &DynamicProjectEnvironmentResource,
    ) -> Result<WorkspaceConfig> {
        let config_file = infrastructure_root
            .join(".pmp")
            .join("workspaces")
            .join(format!("{}-{}.json", resource.metadata.name, resource.metadata.environment_name));

        if !config_file.exists() {
            return Ok(WorkspaceConfig {
                current_workspace: None,
                workspaces: vec![],
            });
        }

        let content = std::fs::read_to_string(&config_file)?;
        let config: WorkspaceConfig = serde_json::from_str(&content)?;

        Ok(config)
    }

    fn save_workspace_config(
        _ctx: &Context,
        infrastructure_root: &Path,
        resource: &DynamicProjectEnvironmentResource,
        config: &WorkspaceConfig,
    ) -> Result<()> {
        let workspaces_dir = infrastructure_root.join(".pmp").join("workspaces");
        std::fs::create_dir_all(&workspaces_dir)?;

        let config_file = workspaces_dir.join(format!("{}-{}.json", resource.metadata.name, resource.metadata.environment_name));
        let content = serde_json::to_string_pretty(config)?;
        std::fs::write(&config_file, content)?;

        Ok(())
    }

    fn get_workspaces(
        _ctx: &Context,
        infrastructure_root: &Path,
        resource: &DynamicProjectEnvironmentResource,
    ) -> Result<Vec<Workspace>> {
        let workspaces_dir = infrastructure_root.join(".pmp").join("workspaces");

        if !workspaces_dir.exists() {
            return Ok(vec![]);
        }

        let mut workspaces = Vec::new();
        let prefix = format!("{}-{}-", resource.metadata.name, resource.metadata.environment_name);

        for entry in std::fs::read_dir(&workspaces_dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(filename) = path.file_name().and_then(|n| n.to_str())
                && filename.starts_with(&prefix) && filename.ends_with(".workspace.json")
            {
                let content = std::fs::read_to_string(&path)?;
                if let Ok(workspace) = serde_json::from_str::<Workspace>(&content) {
                    workspaces.push(workspace);
                }
            }
        }

        workspaces.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(workspaces)
    }

    fn save_workspace(
        _ctx: &Context,
        infrastructure_root: &Path,
        workspace: &Workspace,
    ) -> Result<()> {
        let workspaces_dir = infrastructure_root.join(".pmp").join("workspaces");
        std::fs::create_dir_all(&workspaces_dir)?;

        let workspace_file = workspaces_dir.join(format!(
            "{}-{}-{}.workspace.json",
            workspace.project, workspace.environment, workspace.name
        ));

        let content = serde_json::to_string_pretty(workspace)?;
        std::fs::write(&workspace_file, content)?;

        // Update config
        let config_file = workspaces_dir.join(format!("{}-{}.json", workspace.project, workspace.environment));
        let mut config = if config_file.exists() {
            let content = std::fs::read_to_string(&config_file)?;
            serde_json::from_str::<WorkspaceConfig>(&content)?
        } else {
            WorkspaceConfig {
                current_workspace: None,
                workspaces: vec![],
            }
        };

        if !config.workspaces.contains(&workspace.name) {
            config.workspaces.push(workspace.name.clone());
        }

        let content = serde_json::to_string_pretty(&config)?;
        std::fs::write(&config_file, content)?;

        Ok(())
    }

    fn init_workspace_state(
        _ctx: &Context,
        infrastructure_root: &Path,
        workspace: &Workspace,
    ) -> Result<()> {
        let state_dir = infrastructure_root
            .join(".pmp")
            .join("workspaces")
            .join(&workspace.name);

        std::fs::create_dir_all(&state_dir)?;

        Ok(())
    }

    fn delete_workspace(
        _ctx: &Context,
        infrastructure_root: &Path,
        workspace: &Workspace,
    ) -> Result<()> {
        // Delete workspace file
        let workspace_file = infrastructure_root.join(".pmp").join("workspaces").join(format!(
            "{}-{}-{}.workspace.json",
            workspace.project, workspace.environment, workspace.name
        ));

        if workspace_file.exists() {
            std::fs::remove_file(&workspace_file)?;
        }

        // Delete state directory
        let state_dir = infrastructure_root
            .join(".pmp")
            .join("workspaces")
            .join(&workspace.name);

        if state_dir.exists() {
            std::fs::remove_dir_all(&state_dir)?;
        }

        // Update config
        let config_file = infrastructure_root
            .join(".pmp")
            .join("workspaces")
            .join(format!("{}-{}.json", workspace.project, workspace.environment));

        if config_file.exists() {
            let content = std::fs::read_to_string(&config_file)?;
            let mut config: WorkspaceConfig = serde_json::from_str(&content)?;

            config.workspaces.retain(|w| w != &workspace.name);

            let content = serde_json::to_string_pretty(&config)?;
            std::fs::write(&config_file, content)?;
        }

        Ok(())
    }
}
