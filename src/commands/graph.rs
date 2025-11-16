use crate::collection::{CollectionDiscovery, DependencyGraph};
use crate::context::Context;
use crate::output;
use crate::template::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

pub struct GraphCommand;

impl GraphCommand {
    /// Execute the graph command
    pub fn execute(
        ctx: &Context,
        path: Option<&str>,
        format: Option<&str>,
        output_file: Option<&str>,
        show_all: bool,
    ) -> Result<()> {
        ctx.output.section("Dependency Graph");

        // Find the infrastructure root
        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output
            .key_value("Infrastructure", &infrastructure.metadata.name);
        output::blank();

        // Determine what to graph
        let current_path = if let Some(p) = path {
            PathBuf::from(p)
        } else {
            std::env::current_dir()?
        };

        // Check if we're in a project/environment context
        let env_yaml = current_path.join(".pmp.environment.yaml");
        let project_yaml = current_path.join(".pmp.project.yaml");

        if show_all {
            // Graph all projects in the infrastructure
            Self::graph_all_projects(ctx, &infrastructure_root, format, output_file)?;
        } else if ctx.fs.exists(&env_yaml) {
            // We're in an environment directory
            Self::graph_single_project(ctx, &current_path, format, output_file)?;
        } else if ctx.fs.exists(&project_yaml) {
            // We're in a project directory - prompt for environment
            Self::graph_project_with_env_selection(ctx, &current_path, format, output_file)?;
        } else {
            // Not in a project context - show all projects
            ctx.output.dimmed(
                "Not in a project context. Use --all to graph all projects, or navigate to a project directory."
            );
            output::blank();
            Self::graph_all_projects(ctx, &infrastructure_root, format, output_file)?;
        }

        Ok(())
    }

    /// Graph a single project's dependencies
    fn graph_single_project(
        ctx: &Context,
        env_path: &Path,
        format: Option<&str>,
        output_file: Option<&str>,
    ) -> Result<()> {
        // Load environment resource
        let env_file = env_path.join(".pmp.environment.yaml");
        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_file)
            .context("Failed to load environment resource")?;

        let project_name = &resource.metadata.name;
        let env_name = &resource.metadata.environment_name;

        ctx.output
            .subsection(&format!("Project: {} ({})", project_name, env_name));
        output::blank();

        // Check if project has dependencies
        if resource.spec.dependencies.is_empty() {
            ctx.output.dimmed("This project has no dependencies.");
            return Ok(());
        }

        // Build dependency graph
        let graph = DependencyGraph::build(&*ctx.fs, env_path, project_name, env_name)
            .context("Failed to build dependency graph")?;

        // Render based on format
        Self::render_graph(ctx, &graph, format, output_file)?;

        Ok(())
    }

    /// Graph a project with environment selection
    fn graph_project_with_env_selection(
        ctx: &Context,
        project_path: &Path,
        format: Option<&str>,
        output_file: Option<&str>,
    ) -> Result<()> {
        // Discover environments
        let environments = CollectionDiscovery::discover_environments(&*ctx.fs, project_path)?;

        if environments.is_empty() {
            anyhow::bail!("No environments found in this project");
        }

        let selected_env = if environments.len() == 1 {
            environments[0].clone()
        } else {
            ctx.input
                .select("Select environment:", environments.clone())
                .context("Failed to select environment")?
        };

        let env_path = project_path.join("environments").join(&selected_env);
        Self::graph_single_project(ctx, &env_path, format, output_file)
    }

    /// Graph all projects in the infrastructure
    fn graph_all_projects(
        ctx: &Context,
        infrastructure_root: &Path,
        format: Option<&str>,
        output_file: Option<&str>,
    ) -> Result<()> {
        ctx.output.subsection("All Projects");
        output::blank();

        // Discover all projects
        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, infrastructure_root)?;

        if projects.is_empty() {
            ctx.output
                .dimmed("No projects found in this infrastructure.");
            return Ok(());
        }

        // Build a comprehensive dependency map
        let mut all_dependencies: HashMap<String, Vec<String>> = HashMap::new();
        let mut all_projects_set: HashSet<String> = HashSet::new();

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
                        let node_key = format!(
                            "{}:{}",
                            resource.metadata.name, resource.metadata.environment_name
                        );
                        all_projects_set.insert(node_key.clone());

                        let mut deps = Vec::new();
                        for dep in &resource.spec.dependencies {
                            for env in &dep.project.environments {
                                let dep_key = format!("{}:{}", dep.project.name, env);
                                deps.push(dep_key);
                            }
                        }

                        if !deps.is_empty() {
                            all_dependencies.insert(node_key, deps);
                        }
                    }
                }
            }
        }

        // Render based on format
        Self::render_all_projects_graph(
            ctx,
            &all_projects_set,
            &all_dependencies,
            format,
            output_file,
        )?;

        Ok(())
    }

    /// Render a single project's dependency graph
    fn render_graph(
        ctx: &Context,
        graph: &DependencyGraph,
        format: Option<&str>,
        output_file: Option<&str>,
    ) -> Result<()> {
        let format_type = format.unwrap_or("ascii");

        match format_type {
            "ascii" => {
                // Use existing format_tree method
                let tree = graph.format_tree();
                for line in tree.lines() {
                    ctx.output.info(line);
                }

                output::blank();
                ctx.output
                    .info(&format!("Total nodes: {}", graph.node_count()));
            }
            "mermaid" => {
                let mermaid = Self::generate_mermaid(graph)?;
                if let Some(file) = output_file {
                    ctx.fs
                        .write(&PathBuf::from(file), &mermaid)
                        .context("Failed to write Mermaid output")?;
                    ctx.output
                        .success(&format!("Mermaid graph written to: {}", file));
                } else {
                    output::blank();
                    ctx.output.info("Mermaid Diagram:");
                    ctx.output.info("```mermaid");
                    ctx.output.info(&mermaid);
                    ctx.output.info("```");
                }
            }
            "dot" | "graphviz" => {
                let dot = Self::generate_dot(graph)?;
                if let Some(file) = output_file {
                    ctx.fs
                        .write(&PathBuf::from(file), &dot)
                        .context("Failed to write DOT output")?;
                    ctx.output
                        .success(&format!("GraphViz DOT file written to: {}", file));
                } else {
                    output::blank();
                    ctx.output.info("GraphViz DOT:");
                    ctx.output.info(&dot);
                }
            }
            _ => anyhow::bail!("Unsupported format: {}", format_type),
        }

        Ok(())
    }

    /// Render all projects graph
    fn render_all_projects_graph(
        ctx: &Context,
        all_projects: &HashSet<String>,
        dependencies: &HashMap<String, Vec<String>>,
        format: Option<&str>,
        output_file: Option<&str>,
    ) -> Result<()> {
        let format_type = format.unwrap_or("ascii");

        match format_type {
            "ascii" => {
                Self::render_ascii_all_projects(ctx, all_projects, dependencies)?;
            }
            "mermaid" => {
                let mermaid = Self::generate_mermaid_all_projects(all_projects, dependencies)?;
                if let Some(file) = output_file {
                    ctx.fs
                        .write(&PathBuf::from(file), &mermaid)
                        .context("Failed to write Mermaid output")?;
                    ctx.output
                        .success(&format!("Mermaid graph written to: {}", file));
                } else {
                    output::blank();
                    ctx.output.info("Mermaid Diagram:");
                    ctx.output.info("```mermaid");
                    ctx.output.info(&mermaid);
                    ctx.output.info("```");
                }
            }
            "dot" | "graphviz" => {
                let dot = Self::generate_dot_all_projects(all_projects, dependencies)?;
                if let Some(file) = output_file {
                    ctx.fs
                        .write(&PathBuf::from(file), &dot)
                        .context("Failed to write DOT output")?;
                    ctx.output
                        .success(&format!("GraphViz DOT file written to: {}", file));
                } else {
                    output::blank();
                    ctx.output.info("GraphViz DOT:");
                    ctx.output.info(&dot);
                }
            }
            _ => anyhow::bail!("Unsupported format: {}", format_type),
        }

        Ok(())
    }

    /// Render ASCII visualization for all projects
    fn render_ascii_all_projects(
        ctx: &Context,
        all_projects: &HashSet<String>,
        dependencies: &HashMap<String, Vec<String>>,
    ) -> Result<()> {
        // Group projects by whether they have dependencies or not
        let mut with_deps: Vec<String> = Vec::new();
        let mut without_deps: Vec<String> = Vec::new();

        for project in all_projects {
            if dependencies.contains_key(project) {
                with_deps.push(project.clone());
            } else {
                without_deps.push(project.clone());
            }
        }

        with_deps.sort();
        without_deps.sort();

        // Display projects with dependencies
        if !with_deps.is_empty() {
            ctx.output.info("Projects with dependencies:");
            output::blank();

            for project in &with_deps {
                ctx.output.info(&format!("├─ {}", project));
                if let Some(deps) = dependencies.get(project) {
                    for (i, dep) in deps.iter().enumerate() {
                        let is_last = i == deps.len() - 1;
                        let prefix = if is_last { "   └─" } else { "   ├─" };
                        ctx.output.info(&format!("{}  {}", prefix, dep));
                    }
                }
                output::blank();
            }
        }

        // Display standalone projects
        if !without_deps.is_empty() {
            ctx.output.info("Standalone projects (no dependencies):");
            output::blank();
            for project in &without_deps {
                ctx.output.info(&format!("• {}", project));
            }
            output::blank();
        }

        ctx.output
            .info(&format!("Total projects: {}", all_projects.len()));
        ctx.output
            .info(&format!("Projects with dependencies: {}", with_deps.len()));

        Ok(())
    }

    /// Generate Mermaid diagram from dependency graph
    fn generate_mermaid(graph: &DependencyGraph) -> Result<String> {
        let mut output = String::new();
        output.push_str("graph TD\n");

        // Add all nodes
        for node in &graph.nodes {
            let node_id = Self::sanitize_id(&node.key());
            let label = format!("{}\\n({})", node.project_name, node.environment_name);
            output.push_str(&format!("    {}[\"{}\"]\n", node_id, label));
        }

        output.push('\n');

        // Add edges
        for (parent_key, deps) in &graph.dependencies {
            let parent_id = Self::sanitize_id(parent_key);
            for dep in deps {
                let dep_id = Self::sanitize_id(&dep.key());
                output.push_str(&format!("    {} --> {}\n", parent_id, dep_id));
            }
        }

        Ok(output)
    }

    /// Generate Mermaid diagram for all projects
    fn generate_mermaid_all_projects(
        all_projects: &HashSet<String>,
        dependencies: &HashMap<String, Vec<String>>,
    ) -> Result<String> {
        let mut output = String::new();
        output.push_str("graph TD\n");

        // Add all nodes
        let mut sorted_projects: Vec<_> = all_projects.iter().collect();
        sorted_projects.sort();

        for project in sorted_projects {
            let node_id = Self::sanitize_id(project);
            let parts: Vec<&str> = project.split(':').collect();
            let label = if parts.len() == 2 {
                format!("{}\\n({})", parts[0], parts[1])
            } else {
                project.clone()
            };
            output.push_str(&format!("    {}[\"{}\"]\n", node_id, label));
        }

        output.push('\n');

        // Add edges
        for (parent_key, deps) in dependencies {
            let parent_id = Self::sanitize_id(parent_key);
            for dep in deps {
                let dep_id = Self::sanitize_id(dep);
                output.push_str(&format!("    {} --> {}\n", parent_id, dep_id));
            }
        }

        Ok(output)
    }

    /// Generate GraphViz DOT format from dependency graph
    fn generate_dot(graph: &DependencyGraph) -> Result<String> {
        let mut output = String::new();
        output.push_str("digraph dependencies {\n");
        output.push_str("    rankdir=LR;\n");
        output.push_str("    node [shape=box, style=rounded];\n\n");

        // Add all nodes with labels
        for node in &graph.nodes {
            let node_id = Self::sanitize_id(&node.key());
            let label = format!("{}\\n({})", node.project_name, node.environment_name);
            output.push_str(&format!("    {} [label=\"{}\"];\n", node_id, label));
        }

        output.push('\n');

        // Add edges
        for (parent_key, deps) in &graph.dependencies {
            let parent_id = Self::sanitize_id(parent_key);
            for dep in deps {
                let dep_id = Self::sanitize_id(&dep.key());
                output.push_str(&format!("    {} -> {};\n", parent_id, dep_id));
            }
        }

        output.push_str("}\n");

        Ok(output)
    }

    /// Generate GraphViz DOT format for all projects
    fn generate_dot_all_projects(
        all_projects: &HashSet<String>,
        dependencies: &HashMap<String, Vec<String>>,
    ) -> Result<String> {
        let mut output = String::new();
        output.push_str("digraph dependencies {\n");
        output.push_str("    rankdir=LR;\n");
        output.push_str("    node [shape=box, style=rounded];\n\n");

        // Add all nodes with labels
        let mut sorted_projects: Vec<_> = all_projects.iter().collect();
        sorted_projects.sort();

        for project in sorted_projects {
            let node_id = Self::sanitize_id(project);
            let parts: Vec<&str> = project.split(':').collect();
            let label = if parts.len() == 2 {
                format!("{}\\n({})", parts[0], parts[1])
            } else {
                project.clone()
            };
            output.push_str(&format!("    {} [label=\"{}\"];\n", node_id, label));
        }

        output.push('\n');

        // Add edges
        for (parent_key, deps) in dependencies {
            let parent_id = Self::sanitize_id(parent_key);
            for dep in deps {
                let dep_id = Self::sanitize_id(dep);
                output.push_str(&format!("    {} -> {};\n", parent_id, dep_id));
            }
        }

        output.push_str("}\n");

        Ok(output)
    }

    /// Sanitize node IDs for Mermaid and DOT formats
    fn sanitize_id(s: &str) -> String {
        s.replace([':', '-', ' ', '.'], "_")
    }
}
