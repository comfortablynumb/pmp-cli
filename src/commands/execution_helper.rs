use crate::collection::{DependencyGraph, DependencyNode};
use crate::executor::{Executor, ExecutorConfig, NoneExecutor, OpenTofuExecutor};
use crate::hooks::HooksRunner;
use crate::template::DynamicProjectEnvironmentResource;
use anyhow::{Context, Result};
use std::path::Path;

/// Helper functions for executing commands with dependency support
pub struct ExecutionHelper;

impl ExecutionHelper {
    /// Check and display dependencies before execution
    /// Returns true if execution should proceed, false otherwise
    pub fn check_and_display_dependencies(
        ctx: &crate::context::Context,
        env_path: &Path,
        project_name: &str,
        env_name: &str,
        command_name: &str,
    ) -> Result<Option<DependencyGraph>> {
        // Load environment resource to check for dependencies
        let env_file = env_path.join(".pmp.environment.yaml");
        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
            .context("Failed to load environment resource")?;

        // Check if there are dependencies
        if resource.spec.dependencies.is_empty() {
            return Ok(None);
        }

        // Build dependency graph
        ctx.output.blank();
        ctx.output.subsection("Dependencies");
        ctx.output.dimmed("Analyzing project dependencies...");

        let graph = DependencyGraph::build(&*ctx.fs, env_path, project_name, env_name)
            .context("Failed to build dependency graph")?;

        // Display dependency tree
        ctx.output.blank();
        ctx.output.info(
            "This project has dependencies. The following projects will be executed in order:",
        );
        ctx.output.blank();

        let tree = graph.format_tree();
        for line in tree.lines() {
            ctx.output.info(line);
        }

        ctx.output.blank();
        ctx.output.info(&format!(
            "Total projects to execute: {}",
            graph.node_count()
        ));

        // Ask for confirmation
        ctx.output.blank();
        let confirmation = ctx
            .input
            .confirm(
                &format!(
                    "Proceed with {} on all {} projects?",
                    command_name,
                    graph.node_count()
                ),
                true,
            )
            .context("Failed to get confirmation")?;

        if !confirmation {
            ctx.output.blank();
            ctx.output.info("Operation cancelled by user");
            return Ok(None);
        }

        Ok(Some(graph))
    }

    /// Execute a command (preview, apply, destroy) on a dependency graph
    pub fn execute_on_graph<F>(
        ctx: &crate::context::Context,
        graph: &DependencyGraph,
        command_name: &str,
        executor_fn: F,
    ) -> Result<()>
    where
        F: Fn(
            &crate::context::Context,
            &DependencyNode,
            &dyn Executor,
            &ExecutorConfig,
            &[String],
        ) -> Result<()>,
    {
        // Get execution order
        let execution_order = graph.execution_order()?;

        ctx.output.blank();
        ctx.output.section(&format!(
            "Executing {} on {} projects",
            command_name,
            execution_order.len()
        ));

        // Execute on each project in order
        for (i, node) in execution_order.iter().enumerate() {
            ctx.output.blank();
            ctx.output.subsection(&format!(
                "Step {}/{}: {} ({})",
                i + 1,
                execution_order.len(),
                node.project_name,
                node.environment_name
            ));

            // Load environment resource
            let env_file = node.environment_path.join(".pmp.environment.yaml");
            let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
                .context("Failed to load environment resource")?;

            // Get executor configuration
            let executor_config = resource.get_executor_config();

            // Get executor
            let executor = Self::get_executor(&executor_config.name)?;

            // Build executor config
            let execution_config = ExecutorConfig {
                plan_command: None,
                apply_command: None,
                destroy_command: None,
                refresh_command: None,
            };

            // Execute the command on this node
            executor_fn(ctx, node, executor.as_ref(), &execution_config, &[])?;
        }

        Ok(())
    }

    /// Execute preview on a single node
    pub fn execute_preview_on_node(
        ctx: &crate::context::Context,
        node: &DependencyNode,
        executor: &dyn Executor,
        execution_config: &ExecutorConfig,
        extra_args: &[String],
    ) -> Result<()> {
        // Skip execution for none executor (dependency-only projects)
        if executor.get_name() == "none" {
            ctx.output.dimmed(&format!(
                "Skipping {} ({}) - dependency-only project",
                node.project_name, node.environment_name
            ));
            return Ok(());
        }

        let env_dir_str = node
            .environment_path
            .to_str()
            .context("Failed to convert environment path to string")?;

        // Load collection to get hooks
        let (collection, _) = crate::collection::CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required to run commands")?;

        let hooks = collection.get_hooks();

        // Run pre-preview hooks
        if !hooks.pre_preview.is_empty() {
            HooksRunner::run_hooks(&hooks.pre_preview, env_dir_str, "pre-preview")?;
        }

        // Initialize executor
        ctx.output
            .dimmed(&format!("Initializing {}...", executor.get_name()));
        let init_output = executor.init(env_dir_str)?;

        if !init_output.status.success() {
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

        // Run plan
        ctx.output
            .dimmed(&format!("Executing {} plan...", executor.get_name()));
        executor.plan(execution_config, env_dir_str, extra_args)?;

        // Run post-preview hooks
        if !hooks.post_preview.is_empty() {
            HooksRunner::run_hooks(&hooks.post_preview, env_dir_str, "post-preview")?;
        }

        ctx.output.success(&format!(
            "Preview completed for {} ({})",
            node.project_name, node.environment_name
        ));

        Ok(())
    }

    /// Execute apply on a single node
    pub fn execute_apply_on_node(
        ctx: &crate::context::Context,
        node: &DependencyNode,
        executor: &dyn Executor,
        execution_config: &ExecutorConfig,
        extra_args: &[String],
    ) -> Result<()> {
        // Skip execution for none executor (dependency-only projects)
        if executor.get_name() == "none" {
            ctx.output.dimmed(&format!(
                "Skipping {} ({}) - dependency-only project",
                node.project_name, node.environment_name
            ));
            return Ok(());
        }

        let env_dir_str = node
            .environment_path
            .to_str()
            .context("Failed to convert environment path to string")?;

        // Load collection to get hooks
        let (collection, _) = crate::collection::CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required to run commands")?;

        let hooks = collection.get_hooks();

        // Run pre-apply hooks
        if !hooks.pre_apply.is_empty() {
            HooksRunner::run_hooks(&hooks.pre_apply, env_dir_str, "pre-apply")?;
        }

        // Initialize executor
        ctx.output
            .dimmed(&format!("Initializing {}...", executor.get_name()));
        let init_output = executor.init(env_dir_str)?;

        if !init_output.status.success() {
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

        // Run apply
        ctx.output
            .dimmed(&format!("Executing {} apply...", executor.get_name()));
        executor.apply(execution_config, env_dir_str, extra_args)?;

        // Run post-apply hooks
        if !hooks.post_apply.is_empty() {
            HooksRunner::run_hooks(&hooks.post_apply, env_dir_str, "post-apply")?;
        }

        ctx.output.success(&format!(
            "Apply completed for {} ({})",
            node.project_name, node.environment_name
        ));

        Ok(())
    }

    /// Execute destroy on a single node
    pub fn execute_destroy_on_node(
        ctx: &crate::context::Context,
        node: &DependencyNode,
        executor: &dyn Executor,
        execution_config: &ExecutorConfig,
        extra_args: &[String],
    ) -> Result<()> {
        // Skip execution for none executor (dependency-only projects)
        if executor.get_name() == "none" {
            ctx.output.dimmed(&format!(
                "Skipping {} ({}) - dependency-only project",
                node.project_name, node.environment_name
            ));
            return Ok(());
        }

        let env_dir_str = node
            .environment_path
            .to_str()
            .context("Failed to convert environment path to string")?;

        // Load collection to get hooks
        let (collection, _) = crate::collection::CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required to run commands")?;

        let hooks = collection.get_hooks();

        // Run pre-destroy hooks
        if !hooks.pre_destroy.is_empty() {
            HooksRunner::run_hooks(&hooks.pre_destroy, env_dir_str, "pre-destroy")?;
        }

        // Initialize executor
        ctx.output
            .dimmed(&format!("Initializing {}...", executor.get_name()));
        let init_output = executor.init(env_dir_str)?;

        if !init_output.status.success() {
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

        // Run destroy
        ctx.output
            .dimmed(&format!("Executing {} destroy...", executor.get_name()));
        executor.destroy(execution_config, env_dir_str, extra_args)?;

        // Run post-destroy hooks
        if !hooks.post_destroy.is_empty() {
            HooksRunner::run_hooks(&hooks.post_destroy, env_dir_str, "post-destroy")?;
        }

        ctx.output.success(&format!(
            "Destroy completed for {} ({})",
            node.project_name, node.environment_name
        ));

        Ok(())
    }

    /// Get the appropriate executor based on name
    fn get_executor(name: &str) -> Result<Box<dyn Executor>> {
        match name {
            "opentofu" => Ok(Box::new(OpenTofuExecutor::new())),
            "none" => Ok(Box::new(NoneExecutor::new())),
            _ => anyhow::bail!("Unknown executor: {}", name),
        }
    }
}
