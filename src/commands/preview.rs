use crate::collection::{CollectionDiscovery, CollectionManager, DependencyNode};
use crate::commands::project_group::ProjectGroupHandler;
use crate::commands::{CostCommand, ExecutionHelper, PolicyCommand};
use crate::diff::{AsciiRenderer, DiffRenderer, DiffRenderOptions, HtmlRenderer, PlanParser};
use crate::executor::{Executor, ExecutorConfig, OpenTofuExecutor};
use crate::hooks::{HookOutcome, HooksRunner};
use crate::template::metadata::{FailureBehavior, ParallelConfig};
use crate::template::{DynamicProjectEnvironmentResource, ProjectResource};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Handles the 'preview' command - runs executor plan with hooks
pub struct PreviewCommand;

impl PreviewCommand {
    /// Execute the preview command
    #[allow(clippy::too_many_arguments)]
    pub fn execute(
        ctx: &crate::context::Context,
        project_path: Option<&str>,
        show_cost: bool,
        skip_policy: bool,
        parallel: Option<usize>,
        show_diff: bool,
        diff_format: &str,
        side_by_side: bool,
        diff_output: Option<&str>,
        show_unchanged: bool,
        show_sensitive: bool,
        extra_args: &[String],
    ) -> Result<()> {
        // Check for template packs before proceeding
        let env_paths: Vec<String> = std::env::var("PMP_TEMPLATE_PACKS_PATHS")
            .ok()
            .map(|p| crate::template::discovery::parse_colon_separated_paths(&p))
            .unwrap_or_default();
        let custom_paths: Vec<&str> = env_paths.iter().map(|s| s.as_str()).collect();

        let template_packs =
            crate::template::TemplateDiscovery::discover_template_packs_with_custom_paths(
                &*ctx.fs,
                &*ctx.output,
                &custom_paths,
            )
            .context("Failed to discover template packs")?;

        if !crate::template::check_and_offer_installation(&*ctx.fs, &*ctx.output, &template_packs)?
        {
            anyhow::bail!("Cannot proceed without template packs.");
        }

        // Determine working directory
        let work_dir = if let Some(path) = project_path {
            PathBuf::from(path)
        } else {
            std::env::current_dir().context("Failed to get current directory")?
        };

        // Detect context and get environment path
        let (env_path, project_name, env_name) =
            Self::detect_and_select_environment(ctx, &work_dir)?;

        // Load environment resource
        let env_file = env_path.join(".pmp.environment.yaml");
        if !ctx.fs.exists(&env_file) {
            anyhow::bail!("Environment file not found: {:?}", env_file);
        }

        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
            .context("Failed to load environment resource")?;

        ctx.output.section("Preview");
        ctx.output.key_value_highlight("Project", &project_name);
        ctx.output.environment_badge(&env_name);

        if let Some(desc) = &resource.metadata.description {
            ctx.output.key_value("Description", desc);
        }

        ctx.output.key_value("Kind", &resource.kind);

        // Get executor configuration
        let executor_config = resource.get_executor_config();

        // Check if this is a ProjectGroup with spec.projects defined
        // ProjectGroups have special handling - they execute preview on their defined projects
        if executor_config.name == "none" && !resource.spec.projects.is_empty() {
            // Load collection to get infrastructure-level hooks
            let (collection, _collection_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required to run commands")?;

            let infrastructure_hooks = collection.get_hooks();

            // Merge infrastructure hooks with environment hooks
            let hooks = crate::commands::ExecutionHelper::merge_hooks(
                &infrastructure_hooks,
                resource.spec.hooks.as_ref(),
            );
            let env_dir_str = env_path
                .to_str()
                .context("Failed to convert environment path to string")?;

            // Run pre-preview hooks
            if !hooks.pre_preview.is_empty()
                && HooksRunner::run_hooks(&hooks.pre_preview, env_dir_str, "pre-preview")?
                    == HookOutcome::Cancel
            {
                ctx.output.blank();
                ctx.output.warning("Preview cancelled by pre-preview hook");
                return Ok(());
            }

            // Show ProjectGroup info
            ctx.output.blank();
            ctx.output.subsection("Project Group Projects");
            ctx.output.dimmed(&format!(
                "This project group has {} configured project(s):",
                resource.spec.projects.projects().len()
            ));
            for project in resource.spec.projects.projects() {
                ctx.output.dimmed(&format!(
                    "  - {} (template: {}/{})",
                    project.name, project.template_pack, project.template
                ));
            }

            // Execute preview on all configured projects
            ProjectGroupHandler::execute_command_on_projects(
                ctx, &resource, &env_name, "preview", extra_args,
            )?;

            // Run post-preview hooks
            if !hooks.post_preview.is_empty()
                && HooksRunner::run_hooks(&hooks.post_preview, env_dir_str, "post-preview")?
                    == HookOutcome::Cancel
            {
                ctx.output.blank();
                ctx.output
                    .warning("Post-preview hooks cancelled further execution");
                return Ok(());
            }

            ctx.output.blank();
            ctx.output.success("Preview completed successfully");
            return Ok(());
        }

        // Check for dependencies (non-ProjectGroup projects)
        let maybe_graph = crate::commands::ExecutionHelper::check_and_display_dependencies(
            ctx,
            &env_path,
            &project_name,
            &env_name,
            "preview",
        )?;

        if let Some(graph) = maybe_graph {
            // Load collection to get parallel config
            let (collection, _) = CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required to run commands")?;

            // Build parallel config from CLI flag or infrastructure config
            let parallel_config = Self::build_parallel_config(parallel, &collection);

            // Execute preview on entire dependency graph
            let ctx_clone = ctx.clone();
            let executor_fn: Arc<
                dyn Fn(&crate::context::Context, &DependencyNode) -> Result<()> + Send + Sync,
            > = Arc::new(move |ctx, node| {
                Self::execute_preview_on_node_wrapper(ctx, node)
            });

            ExecutionHelper::execute_on_graph_parallel(
                &ctx_clone,
                &graph,
                "preview",
                &parallel_config,
                false,
                executor_fn,
            )?;

            ctx.output.blank();
            ctx.output
                .success("Preview completed successfully for all projects");
            return Ok(());
        }

        // No dependencies - proceed with single project execution

        // Load collection to get infrastructure-level hooks
        let (collection, _collection_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required to run commands")?;

        let infrastructure_hooks = collection.get_hooks();

        // Merge infrastructure hooks with environment hooks
        let hooks = crate::commands::ExecutionHelper::merge_hooks(
            &infrastructure_hooks,
            resource.spec.hooks.as_ref(),
        );

        // Get executor
        let executor = Self::get_executor(&executor_config.name)?;

        // Check if executor is installed
        ctx.output.subsection("Prerequisites");
        ctx.output.dimmed(&format!(
            "Checking if {} is installed...",
            executor.get_name()
        ));

        if !executor.check_installed()? {
            anyhow::bail!(
                "{} is not installed or not available in PATH",
                executor.get_name()
            );
        }

        ctx.output.status_check(executor.get_name(), true);

        // Convert env_path to string for executor
        let env_dir_str = env_path
            .to_str()
            .context("Failed to convert environment path to string")?;

        // Run pre-preview hooks
        if !hooks.pre_preview.is_empty()
            && HooksRunner::run_hooks(&hooks.pre_preview, env_dir_str, "pre-preview")?
                == HookOutcome::Cancel
        {
            ctx.output.blank();
            ctx.output.warning("Preview cancelled by pre-preview hook");
            return Ok(());
        }

        // Run helm repo update if configured
        crate::commands::ExecutionHelper::run_helm_repo_update_if_needed(
            ctx,
            &collection,
            executor.get_name(),
        )?;

        // Initialize executor
        ctx.output.subsection("Initialization");
        ctx.output
            .dimmed(&format!("Initializing {}...", executor.get_name()));
        let init_output = executor.init(env_dir_str)?;

        if !init_output.status.success() {
            // Display captured stdout and stderr before failing
            if !init_output.stdout.is_empty()
                && let Ok(stdout_str) = String::from_utf8(init_output.stdout.clone())
            {
                ctx.output.error(&stdout_str);
            }
            if !init_output.stderr.is_empty()
                && let Ok(stderr_str) = String::from_utf8(init_output.stderr.clone())
            {
                ctx.output.error(&stderr_str);
            }
            anyhow::bail!(
                "Initialization failed with exit code: {:?}",
                init_output.status.code()
            );
        }

        ctx.output.success("Initialization completed");

        // Build executor config
        let mut command_options = std::collections::HashMap::new();
        if let Some(config) = &executor_config.config {
            for (cmd_name, cmd_config) in &config.commands {
                command_options.insert(cmd_name.clone(), cmd_config.options.clone());
            }
        }

        let execution_config = ExecutorConfig {
            plan_command: None,
            apply_command: None,
            destroy_command: None,
            refresh_command: None,
            test_command: None,
            command_options,
        };

        // Run plan
        ctx.output.subsection("Running Plan");
        ctx.output
            .dimmed(&format!("Executing {} plan...", executor.get_name()));

        if show_diff {
            // Use plan_with_output to capture output for diff visualization
            Self::execute_plan_with_diff(
                ctx,
                executor.as_ref(),
                &execution_config,
                env_dir_str,
                extra_args,
                diff_format,
                side_by_side,
                diff_output,
                show_unchanged,
                show_sensitive,
            )?;
        } else {
            // Standard plan execution with direct output
            executor.plan(&execution_config, env_dir_str, extra_args)?;
        }

        // Show cost estimation if requested
        if show_cost {
            Self::show_cost_estimation(ctx, &env_path, &collection)?;
        }

        // Run OPA policy validation (after plan is generated)
        if !skip_policy {
            if !PolicyCommand::run_pre_operation_validation(ctx, &env_path, &collection)? {
                // Policy validation failed - show warning but don't block preview
                ctx.output.warning("Policy validation failed. Fix violations before apply.");
                ctx.output
                    .dimmed("Use --skip-policy to bypass policy validation");
            }
        }

        // Run post-preview hooks
        if !hooks.post_preview.is_empty()
            && HooksRunner::run_hooks(&hooks.post_preview, env_dir_str, "post-preview")?
                == HookOutcome::Cancel
        {
            ctx.output.blank();
            ctx.output
                .warning("Post-preview hooks cancelled further execution");
            return Ok(());
        }

        ctx.output.blank();
        ctx.output.success("Preview completed successfully");

        Ok(())
    }

    /// Show cost estimation for the environment
    fn show_cost_estimation(
        ctx: &crate::context::Context,
        env_path: &Path,
        collection: &crate::template::metadata::InfrastructureResource,
    ) -> Result<()> {
        ctx.output.blank();
        ctx.output.subsection("Cost Estimation");

        let cost_config = collection.spec.cost.as_ref();
        let provider = CostCommand::create_provider(cost_config)?;

        if !provider.check_installed()? {
            ctx.output.warning(&format!(
                "{} is not installed. Skipping cost estimation.",
                provider.get_name()
            ));
            ctx.output.dimmed("Install from: https://www.infracost.io/docs/");
            return Ok(());
        }

        ctx.output
            .dimmed(&format!("Running {} diff...", provider.get_name()));

        match provider.diff(env_path, None) {
            Ok(diff) => {
                ctx.output.key_value("Current Monthly", &format!("${:.2}", diff.current_monthly));
                ctx.output.key_value("Planned Monthly", &format!("${:.2}", diff.planned_monthly));

                let sign = if diff.diff_monthly >= 0.0 { "+" } else { "" };
                let diff_desc = if diff.diff_monthly > 0.0 {
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
                        diff_desc
                    ),
                );

                // Check thresholds
                CostCommand::check_thresholds(ctx, diff.planned_monthly, cost_config)?;
            }
            Err(e) => {
                ctx.output.warning(&format!("Cost estimation failed: {}", e));
            }
        }

        Ok(())
    }

    /// Detect context and select project/environment
    /// Returns: (environment_path, project_name, environment_name)
    fn detect_and_select_environment(
        ctx: &crate::context::Context,
        work_dir: &Path,
    ) -> Result<(PathBuf, String, String)> {
        // Check if we're in an environment directory
        if let Some(env_info) = Self::check_in_environment(ctx, work_dir)? {
            return Ok(env_info);
        }

        // Check if we're in a project directory
        if let Some((project_path, project_name)) = Self::check_in_project(ctx, work_dir)? {
            let env_name = Self::select_environment(ctx, &project_path)?;
            let env_path = project_path.join("environments").join(&env_name);
            return Ok((env_path, project_name, env_name));
        }

        // We're in the collection or elsewhere - use find/search UI
        Self::select_project_and_environment(ctx)
    }

    /// Check if we're inside an environment directory
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

    /// Check if we're inside a project directory (but not in an environment)
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

    /// Select an environment from a project
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

    /// Select project and environment using find/search UI
    fn select_project_and_environment(
        ctx: &crate::context::Context,
    ) -> Result<(PathBuf, String, String)> {
        let manager = CollectionManager::load(ctx).context("Failed to load collection")?;

        let all_projects = manager.get_all_projects();

        if all_projects.is_empty() {
            anyhow::bail!("No projects found in collection");
        }

        // Select project
        // Sort projects by name for consistent display
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

        // Select environment
        let env_name = Self::select_environment(ctx, &project_path)?;
        let env_path = project_path.join("environments").join(&env_name);

        Ok((env_path, selected_project.name.clone(), env_name))
    }

    /// Get the appropriate executor based on name
    fn get_executor(name: &str) -> Result<Box<dyn Executor>> {
        match name {
            "opentofu" => Ok(Box::new(OpenTofuExecutor::new())),
            _ => anyhow::bail!("Unknown executor: {}", name),
        }
    }

    /// Build parallel config from CLI flag or infrastructure config
    fn build_parallel_config(
        cli_parallel: Option<usize>,
        collection: &crate::template::metadata::InfrastructureResource,
    ) -> ParallelConfig {
        // CLI flag takes precedence
        if let Some(max) = cli_parallel {
            return ParallelConfig {
                max,
                on_failure: FailureBehavior::Continue,
            };
        }

        // Fall back to infrastructure config
        if let Some(executor_config) = &collection.spec.executor
            && let Some(parallel) = &executor_config.parallel
        {
            return parallel.clone();
        }

        // Default: sequential execution
        ParallelConfig {
            max: 1,
            on_failure: FailureBehavior::Continue,
        }
    }

    /// Wrapper for execute_preview_on_node that works with parallel execution
    fn execute_preview_on_node_wrapper(
        ctx: &crate::context::Context,
        node: &DependencyNode,
    ) -> Result<()> {
        // Load environment resource
        let env_file = node.environment_path.join(".pmp.environment.yaml");
        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
            .context("Failed to load environment resource")?;

        // Get executor configuration
        let executor_config = resource.get_executor_config();

        // Get executor
        let executor = ExecutionHelper::get_executor(&executor_config.name)?;

        // Build executor config
        let mut command_options = std::collections::HashMap::new();

        if let Some(config) = &executor_config.config {
            for (cmd_name, cmd_config) in &config.commands {
                command_options.insert(cmd_name.clone(), cmd_config.options.clone());
            }
        }

        let execution_config = ExecutorConfig {
            plan_command: None,
            apply_command: None,
            destroy_command: None,
            refresh_command: None,
            test_command: None,
            command_options,
        };

        // Execute preview on this node
        ExecutionHelper::execute_preview_on_node(
            ctx,
            node,
            executor.as_ref(),
            &execution_config,
            &[],
        )
    }

    /// Execute plan with diff visualization
    #[allow(clippy::too_many_arguments)]
    fn execute_plan_with_diff(
        ctx: &crate::context::Context,
        executor: &dyn Executor,
        config: &ExecutorConfig,
        working_dir: &str,
        extra_args: &[String],
        diff_format: &str,
        side_by_side: bool,
        diff_output: Option<&str>,
        show_unchanged: bool,
        show_sensitive: bool,
    ) -> Result<()> {
        // Run plan and capture output
        let output = executor.plan_with_output(working_dir, extra_args)?;

        // Check if plan succeeded
        if !output.status.success() && output.status.code() != Some(2) {
            // Exit code 2 means there are changes, which is expected
            let stderr = String::from_utf8_lossy(&output.stderr);

            if !stderr.is_empty() {
                ctx.output.error(&stderr);
            }

            anyhow::bail!(
                "Plan failed with exit code: {:?}",
                output.status.code()
            );
        }

        // Parse plan output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let parser = PlanParser::new();
        let parsed_plan = parser.parse(&stdout)?;

        // Build render options
        let terminal_width = Self::get_terminal_width();
        let options = DiffRenderOptions {
            show_unchanged,
            compact_mode: false,
            side_by_side,
            max_value_width: 60,
            show_sensitive,
            terminal_width,
        };

        // Render based on format
        let rendered = match diff_format {
            "html" => {
                let renderer = HtmlRenderer::new();
                renderer.render(&parsed_plan, &options)
            }
            _ => {
                let renderer = AsciiRenderer::new();
                renderer.render(&parsed_plan, &options)
            }
        };

        // Output result
        if let Some(output_file) = diff_output {
            ctx.fs.write(&PathBuf::from(output_file), &rendered)?;
            ctx.output.success(&format!("Diff written to: {}", output_file));
        } else {
            // Print to terminal with colors for ASCII format
            if diff_format == "ascii" || diff_format.is_empty() {
                Self::print_colored_diff(ctx, &parsed_plan, &options);
            } else {
                println!("{}", rendered);
            }
        }

        // Show summary
        ctx.output.blank();

        if parsed_plan.has_changes {
            ctx.output.info(&format!(
                "Plan: {} to add, {} to change, {} to destroy",
                parsed_plan.summary.to_add,
                parsed_plan.summary.to_change,
                parsed_plan.summary.to_destroy
            ));
        } else {
            ctx.output.success("No changes. Your infrastructure matches the configuration.");
        }

        Ok(())
    }

    /// Print colored diff to terminal using output colors
    fn print_colored_diff(
        ctx: &crate::context::Context,
        plan: &crate::diff::ParsedPlan,
        options: &DiffRenderOptions,
    ) {
        use crate::diff::{AttributeChangeType, DiffChangeType};
        use owo_colors::OwoColorize;

        // Print summary
        println!();
        ctx.output.subsection("Plan Summary");

        let mut summary_parts = Vec::new();

        if plan.summary.to_add > 0 {
            summary_parts.push(format!("+{} to add", plan.summary.to_add).green().to_string());
        }

        if plan.summary.to_change > 0 {
            summary_parts.push(format!("~{} to change", plan.summary.to_change).yellow().to_string());
        }

        if plan.summary.to_replace > 0 {
            summary_parts.push(format!("±{} to replace", plan.summary.to_replace).magenta().to_string());
        }

        if plan.summary.to_destroy > 0 {
            summary_parts.push(format!("-{} to destroy", plan.summary.to_destroy).red().to_string());
        }

        if summary_parts.is_empty() {
            println!("  No changes.");
        } else {
            println!("  {}", summary_parts.join(", "));
        }

        println!();

        // Print each resource
        for resource in &plan.resources {
            let (symbol, color) = match resource.change_type {
                DiffChangeType::Create => ("+", "green"),
                DiffChangeType::Update => ("~", "yellow"),
                DiffChangeType::Destroy => ("-", "red"),
                DiffChangeType::Replace => ("±", "magenta"),
                DiffChangeType::Read => ("≤", "blue"),
                DiffChangeType::NoOp => (" ", "white"),
            };

            // Print resource header with color
            let header = format!(
                "{} {} ({})",
                symbol,
                resource.address,
                resource.change_type.label()
            );

            match color {
                "green" => println!("{}", header.green()),
                "yellow" => println!("{}", header.yellow()),
                "red" => println!("{}", header.red()),
                "magenta" => println!("{}", header.magenta()),
                "blue" => println!("{}", header.blue()),
                _ => println!("{}", header),
            }

            // Print attributes
            for attr in &resource.attributes {
                if attr.change_type == AttributeChangeType::Unchanged && !options.show_unchanged {
                    continue;
                }

                let attr_symbol = attr.change_type.symbol();
                let mut line = format!("    {} {}", attr_symbol, attr.name);

                // Add value information
                match attr.change_type {
                    AttributeChangeType::Added => {
                        if let Some(ref value) = attr.new_value {
                            let display = Self::format_attr_value(value, attr, options);
                            line.push_str(&format!(" = {}", display));
                        }
                    }
                    AttributeChangeType::Removed => {
                        if let Some(ref value) = attr.old_value {
                            let display = Self::format_attr_value(value, attr, options);
                            line.push_str(&format!(" = {}", display));
                        }
                    }
                    AttributeChangeType::Modified => {
                        let old = attr.old_value.as_deref().unwrap_or("(unknown)");
                        let new = attr.new_value.as_deref().unwrap_or("(unknown)");
                        let old_display = Self::format_attr_value(old, attr, options);
                        let new_display = Self::format_attr_value(new, attr, options);
                        line.push_str(&format!(" = {} -> {}", old_display, new_display));
                    }
                    AttributeChangeType::Unchanged => {
                        if let Some(ref value) = attr.new_value.as_ref().or(attr.old_value.as_ref()) {
                            let display = Self::format_attr_value(value, attr, options);
                            line.push_str(&format!(" = {}", display));
                        }
                    }
                }

                if attr.forces_replacement {
                    line.push_str(" # forces replacement");
                }

                // Print attribute with color
                match attr.change_type {
                    AttributeChangeType::Added => println!("{}", line.green()),
                    AttributeChangeType::Removed => println!("{}", line.red()),
                    AttributeChangeType::Modified => println!("{}", line.yellow()),
                    AttributeChangeType::Unchanged => println!("{}", line.dimmed()),
                }
            }

            println!();
        }
    }

    /// Format attribute value for display
    fn format_attr_value(
        value: &str,
        attr: &crate::diff::AttributeChange,
        options: &DiffRenderOptions,
    ) -> String {
        if attr.sensitive && !options.show_sensitive {
            return "(sensitive)".to_string();
        }

        if attr.computed {
            return "(known after apply)".to_string();
        }

        if value.len() > options.max_value_width {
            let truncated = &value[..options.max_value_width - 3];
            return format!("\"{}...\"", truncated);
        }

        if !value.starts_with('"') && !value.starts_with('(') {
            format!("\"{}\"", value)
        } else {
            value.to_string()
        }
    }

    /// Get terminal width for formatting
    fn get_terminal_width() -> usize {
        // Try to get terminal size, default to 100
        if let Some((width, _)) = terminal_size::terminal_size() {
            width.0 as usize
        } else {
            100
        }
    }
}
