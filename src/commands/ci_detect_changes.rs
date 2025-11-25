use crate::collection::{CollectionDiscovery, DependencyGraph};
use crate::context::Context;
use crate::output;
use crate::template::DynamicProjectEnvironmentResource;
use anyhow::{Context as _, Result};
use serde::Serialize;
use std::collections::{HashSet, HashMap};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Serialize, Clone)]
pub struct ChangedProject {
    pub name: String,
    #[serde(rename = "env")]
    pub environment: String,
    pub path: String,
}

pub struct CiDetectChangesCommand;

impl CiDetectChangesCommand {
    /// Execute the detect-changes command
    pub fn execute(
        ctx: &Context,
        base_ref: &str,
        head_ref: &str,
        environment_filter: Option<&str>,
        output_format: &str,
    ) -> Result<()> {
        // Step 1: Check if infrastructure file changed
        if Self::has_infrastructure_changes(base_ref, head_ref)? {
            output::warning("Infrastructure configuration file changed (.pmp.infrastructure.yaml)");
            output::dimmed("Skipping project CI - infrastructure changes should be deployed separately");
            std::process::exit(2); // Exit code 2 = infrastructure change
        }

        // Step 2: Get changed files from git diff
        let changed_files = Self::get_changed_files(base_ref, head_ref)?;

        if changed_files.is_empty() {
            output::info("No files changed");
            println!("[]"); // Empty JSON array
            return Ok(());
        }

        // Step 3: Parse changed files to extract projects
        let changed_projects = Self::extract_projects_from_paths(&changed_files, environment_filter)?;

        if changed_projects.is_empty() {
            output::info("No project files changed");
            println!("[]"); // Empty JSON array
            return Ok(());
        }

        // Step 4: Load infrastructure and discover projects
        let (_infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. Run 'pmp infrastructure init' first.")?;

        // Discover all projects
        let project_refs = CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &infrastructure_root)?;

        // Step 5: Build map of all project environments
        let mut project_envs: HashMap<(String, String), PathBuf> = HashMap::new();

        for project_ref in &project_refs {
            let project_path = infrastructure_root.join(&project_ref.path);
            let environments_dir = project_path.join("environments");

            if let Ok(env_entries) = ctx.fs.read_dir(&environments_dir) {
                for env_path in env_entries {
                    let env_file = env_path.join(".pmp.environment.yaml");
                    if ctx.fs.exists(&env_file)
                        && let Ok(resource) = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file) {
                            let key = (resource.metadata.name.clone(), resource.metadata.environment_name.clone());
                            project_envs.insert(key, env_path);
                        }
                }
            }
        }

        // Step 6: Include all dependent projects
        let affected_projects = Self::include_dependents(
            &changed_projects,
            &project_envs,
            ctx,
            &infrastructure_root,
        )?;

        // Step 6: Format and output results
        Self::output_results(&affected_projects, output_format)?;

        Ok(())
    }

    /// Check if infrastructure configuration file changed
    fn has_infrastructure_changes(base_ref: &str, head_ref: &str) -> Result<bool> {
        let output = Command::new("git")
            .args([
                "diff",
                "--name-only",
                &format!("{}...{}", base_ref, head_ref),
            ])
            .output()
            .context("Failed to run git diff")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Git diff failed: {}", stderr);
        }

        let files = String::from_utf8_lossy(&output.stdout);

        // Check if .pmp.infrastructure.yaml changed
        Ok(files.lines().any(|line| {
            line.trim() == ".pmp.infrastructure.yaml" ||
            line.trim().ends_with("/.pmp.infrastructure.yaml")
        }))
    }

    /// Get list of changed files from git diff
    fn get_changed_files(base_ref: &str, head_ref: &str) -> Result<Vec<String>> {
        let output = Command::new("git")
            .args([
                "diff",
                "--name-only",
                &format!("{}...{}", base_ref, head_ref),
            ])
            .output()
            .context("Failed to run git diff")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Git diff failed: {}", stderr);
        }

        let files = String::from_utf8_lossy(&output.stdout);
        Ok(files.lines().map(|s| s.to_string()).collect())
    }

    /// Extract project name and environment from file paths
    /// Expected path format: projects/{project_name}/environments/{environment}/...
    fn extract_projects_from_paths(
        paths: &[String],
        environment_filter: Option<&str>,
    ) -> Result<HashSet<(String, String)>> {
        let mut projects = HashSet::new();

        for path in paths {
            // Parse path: projects/{name}/environments/{env}/*
            let parts: Vec<&str> = path.split('/').collect();

            // Check if this is a project environment file
            if parts.len() >= 4 && parts[0] == "projects" && parts[2] == "environments" {
                let project_name = parts[1].to_string();
                let environment = parts[3].to_string();

                // Apply environment filter if specified
                if let Some(filter_env) = environment_filter
                    && environment != filter_env
                {
                    continue;
                }

                projects.insert((project_name, environment));
            }
        }

        Ok(projects)
    }

    /// Include all projects that depend on the changed projects
    fn include_dependents(
        changed_projects: &HashSet<(String, String)>,
        project_envs: &HashMap<(String, String), PathBuf>,
        ctx: &Context,
        infrastructure_root: &Path,
    ) -> Result<Vec<ChangedProject>> {
        let mut affected = HashSet::new();

        // Add initially changed projects
        for (name, env) in changed_projects {
            affected.insert((name.clone(), env.clone()));
        }

        // For each changed project, find all projects that depend on it
        // We need to check ALL projects and build their dependency graphs
        for project_key in project_envs.keys() {
            let (proj_name, proj_env) = project_key;

            // Try to build dependency graph for this project
            if let Ok(dep_graph) = DependencyGraph::build(
                &*ctx.fs,
                infrastructure_root,
                proj_name,
                proj_env,
            ) {
                // Check if this project depends on any of the changed projects
                for (changed_name, changed_env) in changed_projects {
                    // Check if this project's dependency graph includes the changed project
                    if let Ok(execution_order) = dep_graph.execution_order() {
                        for node in &execution_order {
                            if node.project_name == *changed_name && node.environment_name == *changed_env {
                                // This project depends on a changed project, so include it
                                affected.insert((proj_name.clone(), proj_env.clone()));
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Build result list with full paths
        let mut result = Vec::new();

        for (name, env) in affected {
            if let Some(path) = project_envs.get(&(name.clone(), env.clone())) {
                result.push(ChangedProject {
                    name: name.clone(),
                    environment: env.clone(),
                    path: path.display().to_string(),
                });
            }
        }

        // Sort for deterministic output
        result.sort_by(|a, b| {
            a.name.cmp(&b.name).then_with(|| a.environment.cmp(&b.environment))
        });

        Ok(result)
    }

    /// Output results in the specified format
    fn output_results(projects: &[ChangedProject], format: &str) -> Result<()> {
        match format {
            "json" => {
                let json = serde_json::to_string_pretty(projects)
                    .context("Failed to serialize to JSON")?;
                println!("{}", json);
            }
            "yaml" => {
                let yaml = serde_yaml::to_string(projects)
                    .context("Failed to serialize to YAML")?;
                println!("{}", yaml);
            }
            _ => {
                anyhow::bail!("Unsupported output format: {}. Use 'json' or 'yaml'", format);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_projects_from_valid_paths() {
        let paths = vec![
            "projects/my-api/environments/dev/main.tf".to_string(),
            "projects/my-api/environments/dev/variables.tf".to_string(),
            "projects/postgres-db/environments/production/main.tf".to_string(),
        ];

        let result = CiDetectChangesCommand::extract_projects_from_paths(&paths, None).unwrap();

        assert_eq!(result.len(), 2);
        assert!(result.contains(&("my-api".to_string(), "dev".to_string())));
        assert!(result.contains(&("postgres-db".to_string(), "production".to_string())));
    }

    #[test]
    fn test_extract_projects_with_environment_filter() {
        let paths = vec![
            "projects/my-api/environments/dev/main.tf".to_string(),
            "projects/my-api/environments/production/main.tf".to_string(),
        ];

        let result =
            CiDetectChangesCommand::extract_projects_from_paths(&paths, Some("dev")).unwrap();

        assert_eq!(result.len(), 1);
        assert!(result.contains(&("my-api".to_string(), "dev".to_string())));
    }

    #[test]
    fn test_extract_projects_ignores_non_project_paths() {
        let paths = vec![
            ".pmp.infrastructure.yaml".to_string(),
            "README.md".to_string(),
            "docs/guide.md".to_string(),
            "projects/my-api/environments/dev/main.tf".to_string(),
        ];

        let result = CiDetectChangesCommand::extract_projects_from_paths(&paths, None).unwrap();

        assert_eq!(result.len(), 1);
        assert!(result.contains(&("my-api".to_string(), "dev".to_string())));
    }

    #[test]
    fn test_extract_projects_from_empty_paths() {
        let paths: Vec<String> = vec![];
        let result = CiDetectChangesCommand::extract_projects_from_paths(&paths, None).unwrap();
        assert!(result.is_empty());
    }
}
