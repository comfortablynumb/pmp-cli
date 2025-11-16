use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::output;
use crate::template::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub struct DevExCommand;

impl DevExCommand {
    /// Launch interactive shell for exploring projects
    pub fn execute_shell(ctx: &Context) -> Result<()> {
        ctx.output.section("PMP Interactive Shell");
        ctx.output
            .dimmed("Type 'help' for available commands, 'exit' to quit");
        output::blank();

        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output
            .key_value("Infrastructure", &infrastructure.metadata.name);
        output::blank();

        // Interactive REPL loop
        loop {
            print!("pmp> ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            match input {
                "exit" | "quit" => {
                    ctx.output.dimmed("Goodbye!");
                    break;
                }
                "help" => {
                    Self::show_shell_help(ctx);
                }
                "list" | "ls" => {
                    Self::shell_list_projects(ctx, &infrastructure_root)?;
                }
                "pwd" => {
                    let cwd = std::env::current_dir()?;
                    ctx.output.info(&cwd.display().to_string());
                }
                cmd if cmd.starts_with("cd ") => {
                    let path = cmd.strip_prefix("cd ").unwrap().trim();
                    if let Err(e) = std::env::set_current_dir(path) {
                        ctx.output
                            .error(&format!("Failed to change directory: {}", e));
                    } else {
                        let cwd = std::env::current_dir()?;
                        ctx.output.dimmed(&cwd.display().to_string());
                    }
                }
                cmd if cmd.starts_with("show ") => {
                    let project = cmd.strip_prefix("show ").unwrap().trim();
                    Self::shell_show_project(ctx, &infrastructure_root, project)?;
                }
                cmd if cmd.starts_with("inspect ") => {
                    let path = cmd.strip_prefix("inspect ").unwrap().trim();
                    Self::shell_inspect_environment(ctx, path)?;
                }
                _ => {
                    ctx.output.error(&format!("Unknown command: {}", input));
                    ctx.output.dimmed("Type 'help' for available commands");
                }
            }

            output::blank();
        }

        Ok(())
    }

    /// Generate documentation from infrastructure
    pub fn execute_docs(
        ctx: &Context,
        path: Option<&str>,
        output_file: Option<&str>,
        format: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Documentation Generation");

        let _current_path = if let Some(p) = path {
            PathBuf::from(p)
        } else {
            std::env::current_dir()?
        };

        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output
            .key_value("Infrastructure", &infrastructure.metadata.name);
        output::blank();

        // Generate documentation
        let docs = Self::generate_documentation(ctx, &infrastructure_root, &infrastructure)?;

        // Render documentation
        let content = Self::render_documentation(ctx, &docs, format.unwrap_or("markdown"))?;

        if let Some(file) = output_file {
            ctx.fs.write(&PathBuf::from(file), &content)?;
            ctx.output
                .success(&format!("Documentation written to: {}", file));
        } else {
            ctx.output.info(&content);
        }

        Ok(())
    }

    /// Visualize dependency graphs
    pub fn execute_graph_viz(
        ctx: &Context,
        output_file: Option<&str>,
        format: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Dependency Graph Visualization");

        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output
            .key_value("Infrastructure", &infrastructure.metadata.name);
        output::blank();

        // Build dependency graph
        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &infrastructure_root)?;

        if projects.is_empty() {
            ctx.output.dimmed("No projects found.");
            return Ok(());
        }

        // Generate graph
        let graph_format = format.unwrap_or("mermaid");
        let graph =
            Self::generate_dependency_graph(ctx, &projects, &infrastructure_root, graph_format)?;

        if let Some(file) = output_file {
            ctx.fs.write(&PathBuf::from(file), &graph)?;
            ctx.output
                .success(&format!("Dependency graph written to: {}", file));
        } else {
            ctx.output.info(&graph);
        }

        Ok(())
    }

    /// Export infrastructure to other formats
    pub fn execute_export(
        ctx: &Context,
        path: Option<&str>,
        target_format: &str,
        output_file: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Infrastructure Export");

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
        ctx.output.key_value("Target Format", target_format);
        output::blank();

        // Export to target format
        let exported = Self::export_to_format(ctx, &current_path, &resource, target_format)?;

        if let Some(file) = output_file {
            ctx.fs.write(&PathBuf::from(file), &exported)?;
            ctx.output.success(&format!("Exported to: {}", file));
        } else {
            ctx.output.info(&exported);
        }

        Ok(())
    }

    /// Import existing infrastructure into PMP
    pub fn execute_import(
        ctx: &Context,
        source_path: &str,
        source_format: &str,
        project_name: &str,
        environment: &str,
    ) -> Result<()> {
        ctx.output.section("Infrastructure Import");

        ctx.output.key_value("Source", source_path);
        ctx.output.key_value("Format", source_format);
        ctx.output.key_value("Project Name", project_name);
        ctx.output.key_value("Environment", environment);
        output::blank();

        let (_infrastructure, infrastructure_root) =
            CollectionDiscovery::find_collection(&*ctx.fs)?
                .context("Infrastructure is required. Run 'pmp init' first.")?;

        // Import from source format
        ctx.output
            .info(&format!("Importing from {} format...", source_format));

        let imported = Self::import_from_format(ctx, source_path, source_format)?;

        // Create project structure
        ctx.output.info("Creating project structure...");

        Self::create_imported_project(
            ctx,
            &infrastructure_root,
            project_name,
            environment,
            &imported,
        )?;

        ctx.output.success("Import completed successfully!");

        Ok(())
    }

    // Shell helper methods

    fn show_shell_help(ctx: &Context) {
        ctx.output.subsection("Available Commands");
        output::blank();

        ctx.output
            .dimmed("  help              - Show this help message");
        ctx.output.dimmed("  list, ls          - List all projects");
        ctx.output
            .dimmed("  pwd               - Print current directory");
        ctx.output.dimmed("  cd <path>         - Change directory");
        ctx.output
            .dimmed("  show <project>    - Show project details");
        ctx.output
            .dimmed("  inspect <path>    - Inspect environment details");
        ctx.output.dimmed("  exit, quit        - Exit the shell");
    }

    fn shell_list_projects(ctx: &Context, infrastructure_root: &Path) -> Result<()> {
        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, infrastructure_root)?;

        if projects.is_empty() {
            ctx.output.dimmed("No projects found.");
            return Ok(());
        }

        ctx.output.subsection("Projects");
        output::blank();

        for project in &projects {
            ctx.output.info(&format!(
                "• {} ({}) - {}",
                project.name, project.kind, project.path
            ));
        }

        Ok(())
    }

    fn shell_show_project(
        ctx: &Context,
        infrastructure_root: &Path,
        project_name: &str,
    ) -> Result<()> {
        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, infrastructure_root)?;

        let project = projects
            .iter()
            .find(|p| p.name == project_name)
            .context("Project not found")?;

        ctx.output.subsection(&project.name);
        output::blank();

        ctx.output.key_value("Name", &project.name);
        ctx.output.key_value("Kind", &project.kind);
        ctx.output.key_value("Path", &project.path);

        // List environments
        let project_path = infrastructure_root.join(&project.path);
        let environments_dir = project_path.join("environments");

        if let Ok(env_entries) = ctx.fs.read_dir(&environments_dir) {
            output::blank();
            ctx.output.dimmed("Environments:");

            for env_path in env_entries {
                if let Some(env_name) = env_path.file_name() {
                    ctx.output
                        .dimmed(&format!("  • {}", env_name.to_string_lossy()));
                }
            }
        }

        Ok(())
    }

    fn shell_inspect_environment(ctx: &Context, path: &str) -> Result<()> {
        let env_path = PathBuf::from(path);
        let env_yaml = env_path.join(".pmp.environment.yaml");

        if !ctx.fs.exists(&env_yaml) {
            ctx.output.error("Not an environment directory");
            return Ok(());
        }

        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_yaml)?;

        ctx.output.subsection("Environment Details");
        output::blank();

        ctx.output.key_value("Project", &resource.metadata.name);
        ctx.output
            .key_value("Environment", &resource.metadata.environment_name);

        if let Some(desc) = &resource.metadata.description {
            ctx.output.key_value("Description", desc);
        }

        ctx.output.key_value("Kind", &resource.spec.resource.kind);
        ctx.output
            .key_value("Executor", &resource.spec.executor.name);

        // Show inputs
        if !resource.spec.inputs.is_empty() {
            output::blank();
            ctx.output.dimmed("Inputs:");

            for (key, value) in &resource.spec.inputs {
                let value_str = match value {
                    serde_json::Value::String(s) => s.clone(),
                    _ => value.to_string(),
                };
                ctx.output.dimmed(&format!("  {} = {}", key, value_str));
            }
        }

        Ok(())
    }

    // Documentation generation

    fn generate_documentation(
        ctx: &Context,
        infrastructure_root: &Path,
        infrastructure: &crate::template::metadata::InfrastructureResource,
    ) -> Result<String> {
        let mut docs = String::new();

        // Header
        docs.push_str(&format!(
            "# {} Documentation\\n\\n",
            infrastructure.metadata.name
        ));

        if let Some(desc) = &infrastructure.metadata.description {
            docs.push_str(&format!("{}\\n\\n", desc));
        }

        docs.push_str(&format!(
            "**Generated:** {}\\n\\n",
            chrono::Utc::now().to_rfc3339()
        ));

        // Environments
        docs.push_str("## Environments\\n\\n");

        for env_name in infrastructure.spec.environments.keys() {
            docs.push_str(&format!("- `{}`\\n", env_name));
        }

        docs.push_str("\\n");

        // Projects
        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, infrastructure_root)?;

        if !projects.is_empty() {
            docs.push_str("## Projects\\n\\n");

            for project in &projects {
                docs.push_str(&format!("### {}\\n\\n", project.name));
                docs.push_str(&format!("**Type:** {}\\n\\n", project.kind));
                docs.push_str(&format!("**Path:** `{}`\\n\\n", project.path));

                // List environments for this project
                let project_path = infrastructure_root.join(&project.path);
                let environments_dir = project_path.join("environments");

                if let Ok(env_entries) = ctx.fs.read_dir(&environments_dir) {
                    docs.push_str("**Environments:**\\n\\n");

                    for env_path in env_entries {
                        if let Some(env_name) = env_path.file_name() {
                            docs.push_str(&format!("- {}\\n", env_name.to_string_lossy()));
                        }
                    }

                    docs.push_str("\\n");
                }
            }
        }

        Ok(docs)
    }

    fn render_documentation(_ctx: &Context, docs: &str, format: &str) -> Result<String> {
        match format {
            "markdown" | "md" => Ok(docs.to_string()),
            "html" => {
                // In a real implementation, convert markdown to HTML
                // For now, just wrap in basic HTML
                Ok(format!(
                    "<!DOCTYPE html>\\n<html>\\n<head><title>PMP Documentation</title></head>\\n<body>\\n<pre>{}\\n</pre>\\n</body>\\n</html>",
                    docs
                ))
            }
            _ => anyhow::bail!("Unsupported documentation format: {}", format),
        }
    }

    // Graph visualization

    fn generate_dependency_graph(
        ctx: &Context,
        projects: &[crate::template::metadata::ProjectReference],
        infrastructure_root: &Path,
        format: &str,
    ) -> Result<String> {
        match format {
            "mermaid" => Self::generate_mermaid_graph(ctx, projects, infrastructure_root),
            "graphviz" | "dot" => Self::generate_graphviz_graph(ctx, projects, infrastructure_root),
            _ => anyhow::bail!("Unsupported graph format: {}", format),
        }
    }

    fn generate_mermaid_graph(
        ctx: &Context,
        projects: &[crate::template::metadata::ProjectReference],
        infrastructure_root: &Path,
    ) -> Result<String> {
        let mut graph = String::from("graph TD\\n");

        // Add nodes
        for project in projects {
            let node_id = project.name.replace('-', "_");
            graph.push_str(&format!("    {}[{}]\\n", node_id, project.name));
        }

        // Add edges (dependencies)
        for project in projects {
            let project_path = infrastructure_root.join(&project.path);
            let environments_dir = project_path.join("environments");

            if let Ok(env_entries) = ctx.fs.read_dir(&environments_dir) {
                for env_path in env_entries {
                    let env_file = env_path.join(".pmp.environment.yaml");

                    if ctx.fs.exists(&env_file)
                        && let Ok(resource) =
                            DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
                    {
                        for dep in &resource.spec.dependencies {
                            let from_id = project.name.replace('-', "_");
                            let to_id = dep.project.name.replace('-', "_");
                            graph.push_str(&format!("    {} --> {}\\n", from_id, to_id));
                        }
                    }
                }
            }
        }

        Ok(graph)
    }

    fn generate_graphviz_graph(
        ctx: &Context,
        projects: &[crate::template::metadata::ProjectReference],
        infrastructure_root: &Path,
    ) -> Result<String> {
        let mut graph = String::from("digraph dependencies {\\n");
        graph.push_str("    rankdir=LR;\\n");
        graph.push_str("    node [shape=box];\\n\\n");

        // Add nodes
        for project in projects {
            graph.push_str(&format!(
                "    \\\"{}\\\" [label=\\\"{}\\\"];\\n",
                project.name, project.name
            ));
        }

        graph.push_str("\\n");

        // Add edges (dependencies)
        for project in projects {
            let project_path = infrastructure_root.join(&project.path);
            let environments_dir = project_path.join("environments");

            if let Ok(env_entries) = ctx.fs.read_dir(&environments_dir) {
                for env_path in env_entries {
                    let env_file = env_path.join(".pmp.environment.yaml");

                    if ctx.fs.exists(&env_file)
                        && let Ok(resource) =
                            DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
                    {
                        for dep in &resource.spec.dependencies {
                            graph.push_str(&format!(
                                "    \\\"{}\\\" -> \\\"{}\\\";\\n",
                                project.name, dep.project.name
                            ));
                        }
                    }
                }
            }
        }

        graph.push_str("}\\n");

        Ok(graph)
    }

    // Export functionality

    fn export_to_format(
        ctx: &Context,
        env_path: &Path,
        resource: &DynamicProjectEnvironmentResource,
        target_format: &str,
    ) -> Result<String> {
        match target_format {
            "helm" => Self::export_to_helm(ctx, env_path, resource),
            "cloudformation" | "cfn" => Self::export_to_cloudformation(ctx, env_path, resource),
            "pulumi" => Self::export_to_pulumi(ctx, env_path, resource),
            _ => anyhow::bail!("Unsupported export format: {}", target_format),
        }
    }

    fn export_to_helm(
        ctx: &Context,
        _env_path: &Path,
        resource: &DynamicProjectEnvironmentResource,
    ) -> Result<String> {
        ctx.output.dimmed("Exporting to Helm chart format...");

        // In a real implementation, convert Terraform/OpenTofu to Helm
        // For now, return placeholder
        Ok(format!(
            "# Helm Chart: {}\\n# Environment: {}\\n# TODO: Implement Helm export\\n",
            resource.metadata.name, resource.metadata.environment_name
        ))
    }

    fn export_to_cloudformation(
        ctx: &Context,
        _env_path: &Path,
        resource: &DynamicProjectEnvironmentResource,
    ) -> Result<String> {
        ctx.output.dimmed("Exporting to CloudFormation format...");

        // In a real implementation, convert Terraform/OpenTofu to CloudFormation
        // For now, return placeholder
        Ok(format!(
            "# CloudFormation Template: {}\\n# Environment: {}\\n# TODO: Implement CloudFormation export\\n",
            resource.metadata.name, resource.metadata.environment_name
        ))
    }

    fn export_to_pulumi(
        ctx: &Context,
        _env_path: &Path,
        resource: &DynamicProjectEnvironmentResource,
    ) -> Result<String> {
        ctx.output.dimmed("Exporting to Pulumi format...");

        // In a real implementation, convert Terraform/OpenTofu to Pulumi
        // For now, return placeholder
        Ok(format!(
            "# Pulumi Program: {}\\n# Environment: {}\\n# TODO: Implement Pulumi export\\n",
            resource.metadata.name, resource.metadata.environment_name
        ))
    }

    // Import functionality

    fn import_from_format(ctx: &Context, source_path: &str, source_format: &str) -> Result<String> {
        match source_format {
            "terraform" | "tf" => Self::import_from_terraform(ctx, source_path),
            "helm" => Self::import_from_helm(ctx, source_path),
            "cloudformation" | "cfn" => Self::import_from_cloudformation(ctx, source_path),
            _ => anyhow::bail!("Unsupported import format: {}", source_format),
        }
    }

    fn import_from_terraform(ctx: &Context, source_path: &str) -> Result<String> {
        ctx.output
            .dimmed(&format!("Importing Terraform from: {}", source_path));

        // In a real implementation, read and parse Terraform files
        // For now, return placeholder
        Ok(String::from("# Imported Terraform configuration\\n"))
    }

    fn import_from_helm(ctx: &Context, source_path: &str) -> Result<String> {
        ctx.output
            .dimmed(&format!("Importing Helm chart from: {}", source_path));

        // In a real implementation, read and parse Helm chart
        // For now, return placeholder
        Ok(String::from("# Imported Helm configuration\\n"))
    }

    fn import_from_cloudformation(ctx: &Context, source_path: &str) -> Result<String> {
        ctx.output
            .dimmed(&format!("Importing CloudFormation from: {}", source_path));

        // In a real implementation, read and parse CloudFormation template
        // For now, return placeholder
        Ok(String::from("# Imported CloudFormation configuration\\n"))
    }

    fn create_imported_project(
        ctx: &Context,
        infrastructure_root: &Path,
        project_name: &str,
        environment: &str,
        imported_content: &str,
    ) -> Result<()> {
        // In a real implementation, create proper project structure
        // For now, just create placeholder structure

        let project_dir = infrastructure_root
            .join("projects")
            .join("imported")
            .join(project_name);
        let env_dir = project_dir.join("environments").join(environment);

        ctx.fs.create_dir_all(&env_dir)?;

        // Write imported content
        let main_file = env_dir.join("main.tf");
        ctx.fs.write(&main_file, imported_content)?;

        ctx.output
            .dimmed(&format!("Created project at: {}", project_dir.display()));

        Ok(())
    }
}
