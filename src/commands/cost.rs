use crate::collection::{CollectionDiscovery, CollectionManager};
use crate::cost::{CostDiff, CostEstimate, CostProvider, InfracostProvider};
use crate::template::metadata::CostConfig;
use crate::template::{DynamicProjectEnvironmentResource, ProjectResource};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Handles cost estimation commands
pub struct CostCommand;

impl CostCommand {
    /// Execute the cost estimate subcommand
    pub fn execute_estimate(
        ctx: &crate::context::Context,
        project_path: Option<&str>,
        format: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Cost Estimation");

        let work_dir = Self::resolve_working_dir(project_path)?;
        let (env_path, project_name, env_name) =
            Self::detect_and_select_environment(ctx, &work_dir)?;

        let (collection, _) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required for cost estimation")?;

        let cost_config = collection.spec.cost.as_ref();

        let provider = Self::create_provider(cost_config)?;

        Self::check_provider_installed(ctx, &*provider)?;

        ctx.output.key_value_highlight("Project", &project_name);
        ctx.output.environment_badge(&env_name);
        ctx.output.blank();

        ctx.output.subsection("Running Cost Analysis");

        let output_format = format.unwrap_or("table");

        if output_format == "json" || output_format == "html" {
            let report = provider.report(&env_path, output_format)?;
            ctx.output.info(&report);
        } else {
            let estimate = provider.estimate(&env_path)?;
            Self::display_estimate(ctx, &estimate, cost_config)?;
        }

        ctx.output.blank();
        ctx.output.success("Cost estimation completed");

        Ok(())
    }

    /// Execute the cost diff subcommand
    pub fn execute_diff(
        ctx: &crate::context::Context,
        project_path: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Cost Comparison");

        let work_dir = Self::resolve_working_dir(project_path)?;
        let (env_path, project_name, env_name) =
            Self::detect_and_select_environment(ctx, &work_dir)?;

        let (collection, _) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required for cost estimation")?;

        let cost_config = collection.spec.cost.as_ref();

        let provider = Self::create_provider(cost_config)?;

        Self::check_provider_installed(ctx, &*provider)?;

        ctx.output.key_value_highlight("Project", &project_name);
        ctx.output.environment_badge(&env_name);
        ctx.output.blank();

        ctx.output.subsection("Comparing Costs");

        let diff = provider.diff(&env_path, None)?;
        Self::display_diff(ctx, &diff, cost_config)?;

        ctx.output.blank();
        ctx.output.success("Cost comparison completed");

        Ok(())
    }

    /// Execute the cost report subcommand
    pub fn execute_report(
        ctx: &crate::context::Context,
        project_path: Option<&str>,
        format: Option<&str>,
        output_file: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Cost Report");

        let work_dir = Self::resolve_working_dir(project_path)?;
        let (env_path, project_name, env_name) =
            Self::detect_and_select_environment(ctx, &work_dir)?;

        let (collection, _) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required for cost estimation")?;

        let cost_config = collection.spec.cost.as_ref();

        let provider = Self::create_provider(cost_config)?;

        Self::check_provider_installed(ctx, &*provider)?;

        ctx.output.key_value_highlight("Project", &project_name);
        ctx.output.environment_badge(&env_name);
        ctx.output.blank();

        let report_format = format.unwrap_or("table");
        let report = provider.report(&env_path, report_format)?;

        if let Some(file_path) = output_file {
            ctx.fs.write(&PathBuf::from(file_path), &report)?;
            ctx.output
                .success(&format!("Report written to: {}", file_path));
        } else {
            ctx.output.info(&report);
        }

        Ok(())
    }

    fn resolve_working_dir(project_path: Option<&str>) -> Result<PathBuf> {
        if let Some(path) = project_path {
            Ok(PathBuf::from(path))
        } else {
            std::env::current_dir().context("Failed to get current directory")
        }
    }

    /// Create cost provider based on configuration
    pub fn create_provider(cost_config: Option<&CostConfig>) -> Result<Box<dyn CostProvider>> {
        let provider_name = cost_config
            .map(|c| c.provider.as_str())
            .unwrap_or("infracost");

        match provider_name {
            "infracost" => {
                let api_key_env = cost_config.and_then(|c| c.api_key_env.as_deref());

                if let Some(env_var) = api_key_env {
                    Ok(Box::new(InfracostProvider::with_api_key_env(env_var)))
                } else {
                    Ok(Box::new(InfracostProvider::new()))
                }
            }
            _ => anyhow::bail!("Unsupported cost provider: {}", provider_name),
        }
    }

    fn check_provider_installed(
        ctx: &crate::context::Context,
        provider: &dyn CostProvider,
    ) -> Result<()> {
        ctx.output.dimmed(&format!(
            "Checking if {} is installed...",
            provider.get_name()
        ));

        if !provider.check_installed()? {
            anyhow::bail!(
                "{} is not installed. Install from: https://www.infracost.io/docs/",
                provider.get_name()
            );
        }

        ctx.output.status_check(provider.get_name(), true);
        Ok(())
    }

    fn display_estimate(
        ctx: &crate::context::Context,
        estimate: &CostEstimate,
        cost_config: Option<&CostConfig>,
    ) -> Result<()> {
        ctx.output.subsection("Monthly Cost Breakdown");

        ctx.output.key_value(
            "Currency",
            &estimate.breakdown.currency,
        );

        ctx.output.key_value_highlight(
            "Total Monthly Cost",
            &format!("${:.2}", estimate.breakdown.monthly_cost),
        );

        if let Some(hourly) = estimate.breakdown.hourly_cost {
            ctx.output.key_value(
                "Total Hourly Cost",
                &format!("${:.4}", hourly),
            );
        }

        Self::check_thresholds(ctx, estimate.breakdown.monthly_cost, cost_config)?;

        if !estimate.breakdown.resources.is_empty() {
            ctx.output.blank();
            ctx.output.subsection("Resources");

            for resource in &estimate.breakdown.resources {
                if resource.monthly_cost > 0.0 {
                    ctx.output.key_value(
                        &format!("{} ({})", resource.name, resource.resource_type),
                        &format!("${:.2}/mo", resource.monthly_cost),
                    );
                }
            }
        }

        if !estimate.warnings.is_empty() {
            ctx.output.blank();
            ctx.output.subsection("Warnings");

            for warning in &estimate.warnings {
                ctx.output.warning(warning);
            }
        }

        Ok(())
    }

    /// Check thresholds and display warnings/errors
    pub fn check_thresholds(
        ctx: &crate::context::Context,
        monthly_cost: f64,
        cost_config: Option<&CostConfig>,
    ) -> Result<()> {
        if let Some(config) = cost_config {
            if let Some(ref thresholds) = config.thresholds {
                if let Some(warn) = thresholds.warn {
                    if monthly_cost > warn {
                        ctx.output.blank();
                        ctx.output.warning(&format!(
                            "Monthly cost (${:.2}) exceeds warning threshold (${:.2})",
                            monthly_cost, warn
                        ));
                    }
                }

                if let Some(block) = thresholds.block {
                    if monthly_cost > block {
                        ctx.output.blank();
                        ctx.output.error(&format!(
                            "Monthly cost (${:.2}) exceeds blocking threshold (${:.2})",
                            monthly_cost, block
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    fn display_diff(
        ctx: &crate::context::Context,
        diff: &CostDiff,
        cost_config: Option<&CostConfig>,
    ) -> Result<()> {
        ctx.output.subsection("Cost Changes");

        ctx.output.key_value(
            "Current Monthly",
            &format!("${:.2}", diff.current_monthly),
        );

        ctx.output.key_value(
            "Planned Monthly",
            &format!("${:.2}", diff.planned_monthly),
        );

        let sign = if diff.diff_monthly >= 0.0 { "+" } else { "" };
        let diff_color = if diff.diff_monthly > 0.0 {
            "increase"
        } else if diff.diff_monthly < 0.0 {
            "decrease"
        } else {
            "no change"
        };

        ctx.output.key_value_highlight(
            "Difference",
            &format!(
                "{}${:.2} ({:.1}%) - {}",
                sign,
                diff.diff_monthly.abs(),
                diff.diff_percentage.abs(),
                diff_color
            ),
        );

        Self::check_thresholds(ctx, diff.planned_monthly, cost_config)?;

        if !diff.resources_added.is_empty() {
            ctx.output.blank();
            ctx.output.subsection("Resources Added");

            for resource in &diff.resources_added {
                ctx.output.key_value(
                    &format!("+ {}", resource.name),
                    &format!("${:.2}/mo", resource.monthly_cost),
                );
            }
        }

        if !diff.resources_removed.is_empty() {
            ctx.output.blank();
            ctx.output.subsection("Resources Removed");

            for resource in &diff.resources_removed {
                ctx.output.key_value(
                    &format!("- {}", resource.name),
                    &format!("-${:.2}/mo", resource.monthly_cost),
                );
            }
        }

        if !diff.resources_changed.is_empty() {
            ctx.output.blank();
            ctx.output.subsection("Resources Changed");

            for change in &diff.resources_changed {
                let change_sign = if change.diff_monthly >= 0.0 { "+" } else { "" };
                ctx.output.key_value(
                    &format!("~ {}", change.name),
                    &format!(
                        "${:.2} -> ${:.2} ({}${:.2})",
                        change.previous_monthly,
                        change.new_monthly,
                        change_sign,
                        change.diff_monthly
                    ),
                );
            }
        }

        Ok(())
    }

    /// Detect context and select project/environment
    fn detect_and_select_environment(
        ctx: &crate::context::Context,
        work_dir: &Path,
    ) -> Result<(PathBuf, String, String)> {
        if let Some(env_info) = Self::check_in_environment(ctx, work_dir)? {
            return Ok(env_info);
        }

        if let Some((project_path, project_name)) = Self::check_in_project(ctx, work_dir)? {
            let env_name = Self::select_environment(ctx, &project_path)?;
            let env_path = project_path.join("environments").join(&env_name);
            return Ok((env_path, project_name, env_name));
        }

        Self::select_project_and_environment(ctx)
    }

    fn check_in_environment(
        ctx: &crate::context::Context,
        dir: &Path,
    ) -> Result<Option<(PathBuf, String, String)>> {
        let env_file = dir.join(".pmp.environment.yaml");

        if ctx.fs.exists(&env_file) {
            let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)?;
            let env_name = resource.metadata.environment_name.clone();
            let project_name = resource.metadata.name.clone();

            return Ok(Some((dir.to_path_buf(), project_name, env_name)));
        }

        Ok(None)
    }

    fn check_in_project(
        ctx: &crate::context::Context,
        dir: &Path,
    ) -> Result<Option<(PathBuf, String)>> {
        let project_file = dir.join(".pmp.project.yaml");

        if ctx.fs.exists(&project_file) {
            let resource = ProjectResource::from_file(&*ctx.fs, &project_file)?;
            return Ok(Some((dir.to_path_buf(), resource.metadata.name.clone())));
        }

        Ok(None)
    }

    fn select_environment(ctx: &crate::context::Context, project_path: &Path) -> Result<String> {
        let environments = CollectionDiscovery::discover_environments(&*ctx.fs, project_path)
            .context("Failed to discover environments")?;

        if environments.is_empty() {
            anyhow::bail!("No environments found in project: {:?}", project_path);
        }

        if environments.len() == 1 {
            ctx.output.environment_badge(&environments[0]);
            return Ok(environments[0].clone());
        }

        let selected = ctx
            .input
            .select("Select an environment:", environments, None)
            .context("Failed to select environment")?;

        Ok(selected)
    }

    fn select_project_and_environment(
        ctx: &crate::context::Context,
    ) -> Result<(PathBuf, String, String)> {
        let manager = CollectionManager::load(ctx).context("Failed to load collection")?;

        let all_projects = manager.get_all_projects();

        if all_projects.is_empty() {
            anyhow::bail!("No projects found in collection");
        }

        let mut sorted_projects: Vec<_> = all_projects.iter().collect();
        sorted_projects.sort_by(|a, b| a.name.cmp(&b.name));

        let project_options: Vec<String> = sorted_projects
            .iter()
            .map(|p| format!("{} ({})", p.name, p.kind))
            .collect();

        let selected_project_display = ctx
            .input
            .select("Select a project:", project_options.clone(), None)
            .context("Failed to select project")?;

        let project_index = project_options
            .iter()
            .position(|opt| opt == &selected_project_display)
            .context("Project not found")?;

        let selected_project = sorted_projects[project_index];
        let project_path = manager.get_project_path(selected_project);

        let env_name = Self::select_environment(ctx, &project_path)?;
        let env_path = project_path.join("environments").join(&env_name);

        Ok((env_path, selected_project.name.clone(), env_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_working_dir_with_path() {
        let result = CostCommand::resolve_working_dir(Some("/some/path")).unwrap();
        assert_eq!(result, PathBuf::from("/some/path"));
    }

    #[test]
    fn test_resolve_working_dir_without_path() {
        let result = CostCommand::resolve_working_dir(None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_provider_default() {
        let provider = CostCommand::create_provider(None).unwrap();
        assert_eq!(provider.get_name(), "infracost");
    }

    #[test]
    fn test_create_provider_with_config() {
        let config = CostConfig {
            provider: "infracost".to_string(),
            api_key_env: Some("MY_API_KEY".to_string()),
            thresholds: None,
            ci: None,
        };

        let provider = CostCommand::create_provider(Some(&config)).unwrap();
        assert_eq!(provider.get_name(), "infracost");
    }

    #[test]
    fn test_create_provider_unsupported() {
        let config = CostConfig {
            provider: "unknown_provider".to_string(),
            api_key_env: None,
            thresholds: None,
            ci: None,
        };

        let result = CostCommand::create_provider(Some(&config));
        assert!(result.is_err());
    }
}
