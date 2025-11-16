use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::output;
use crate::template::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use std::collections::{HashMap, HashSet};

pub struct DepsCommand;

#[derive(Debug)]
struct DependencyAnalysis {
    total_projects: usize,
    projects_with_dependencies: usize,
    standalone_projects: Vec<String>,
    circular_dependencies: Vec<Vec<String>>,
    orphaned_projects: Vec<String>,
    bottlenecks: Vec<(String, usize)>, // Projects that are dependencies of many others
    missing_dependencies: Vec<(String, String)>, // (project, missing_dep)
}

impl DepsCommand {
    /// Execute the deps analyze command
    pub fn execute_analyze(ctx: &Context) -> Result<()> {
        ctx.output.section("Dependency Analysis");

        // Find the infrastructure root
        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output
            .key_value("Infrastructure", &infrastructure.metadata.name);
        output::blank();

        // Discover all projects
        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &infrastructure_root)?;

        if projects.is_empty() {
            ctx.output
                .dimmed("No projects found in this infrastructure.");
            return Ok(());
        }

        // Build comprehensive dependency data
        let mut all_dependencies: HashMap<String, Vec<String>> = HashMap::new();
        let mut all_projects_set: HashSet<String> = HashSet::new();
        let mut reverse_dependencies: HashMap<String, Vec<String>> = HashMap::new();
        let mut dependency_errors: Vec<(String, String)> = Vec::new();

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
                                    deps.push(dep_key.clone());

                                    // Build reverse dependency map
                                    reverse_dependencies
                                        .entry(dep_key.clone())
                                        .or_default()
                                        .push(node_key.clone());

                                    // Check if dependency exists
                                    if !all_projects_set.contains(&dep_key) {
                                        // We'll check again at the end since we're still discovering
                                        dependency_errors.push((node_key.clone(), dep_key));
                                    }
                                }
                            }

                            if !deps.is_empty() {
                                all_dependencies.insert(node_key, deps);
                            }
                        }
                }
            }
        }

        // Check for missing dependencies (after all projects discovered)
        let missing_dependencies: Vec<(String, String)> = dependency_errors
            .into_iter()
            .filter(|(_, dep)| !all_projects_set.contains(dep))
            .collect();

        // Perform analysis
        let analysis = Self::analyze_dependencies(
            &all_projects_set,
            &all_dependencies,
            &reverse_dependencies,
            &missing_dependencies,
        )?;

        // Display analysis results
        Self::display_analysis(ctx, &analysis)?;

        Ok(())
    }

    /// Execute the deps impact command
    pub fn execute_impact(ctx: &Context, project_name: &str) -> Result<()> {
        ctx.output
            .section(&format!("Impact Analysis: {}", project_name));

        // Find the infrastructure root
        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output
            .key_value("Infrastructure", &infrastructure.metadata.name);
        output::blank();

        // Discover all projects
        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, &infrastructure_root)?;

        if projects.is_empty() {
            ctx.output
                .dimmed("No projects found in this infrastructure.");
            return Ok(());
        }

        // Build dependency map
        let mut all_dependencies: HashMap<String, Vec<String>> = HashMap::new();
        let mut all_projects_set: HashSet<String> = HashSet::new();
        let mut reverse_dependencies: HashMap<String, Vec<String>> = HashMap::new();

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
                                    deps.push(dep_key.clone());

                                    // Build reverse dependency map
                                    reverse_dependencies
                                        .entry(dep_key.clone())
                                        .or_default()
                                        .push(node_key.clone());
                                }
                            }

                            if !deps.is_empty() {
                                all_dependencies.insert(node_key, deps);
                            }
                        }
                }
            }
        }

        // Find all environments of the target project
        let target_projects: Vec<String> = all_projects_set
            .iter()
            .filter(|p| p.starts_with(&format!("{}:", project_name)))
            .cloned()
            .collect();

        if target_projects.is_empty() {
            anyhow::bail!("Project '{}' not found", project_name);
        }

        // For each environment, find impacted projects
        for target_project in &target_projects {
            ctx.output.subsection(target_project);
            output::blank();

            // Find all projects that depend on this one (directly or indirectly)
            let impacted = Self::find_impacted_projects(
                target_project,
                &all_dependencies,
                &reverse_dependencies,
            );

            if impacted.is_empty() {
                ctx.output.dimmed("No projects depend on this project.");
            } else {
                ctx.output.info(&format!(
                    "Projects that would be impacted by changes: {}",
                    impacted.len()
                ));
                output::blank();

                let mut sorted_impacted: Vec<_> = impacted.iter().collect();
                sorted_impacted.sort();

                for (i, project) in sorted_impacted.iter().enumerate() {
                    ctx.output.info(&format!("{}. {}", i + 1, project));
                }
            }

            output::blank();
        }

        Ok(())
    }

    /// Analyze dependencies and find issues
    fn analyze_dependencies(
        all_projects: &HashSet<String>,
        dependencies: &HashMap<String, Vec<String>>,
        reverse_dependencies: &HashMap<String, Vec<String>>,
        missing_dependencies: &[(String, String)],
    ) -> Result<DependencyAnalysis> {
        let total_projects = all_projects.len();
        let projects_with_dependencies = dependencies.len();

        // Find standalone projects
        let mut standalone_projects: Vec<String> = all_projects
            .iter()
            .filter(|p| !dependencies.contains_key(*p))
            .cloned()
            .collect();
        standalone_projects.sort();

        // Find orphaned projects (not depended on by anyone)
        let mut orphaned_projects: Vec<String> = all_projects
            .iter()
            .filter(|p| !reverse_dependencies.contains_key(*p))
            .cloned()
            .collect();
        orphaned_projects.sort();

        // Find bottlenecks (projects that many others depend on)
        let mut bottlenecks: Vec<(String, usize)> = reverse_dependencies
            .iter()
            .map(|(k, v)| (k.clone(), v.len()))
            .collect();
        bottlenecks.sort_by(|a, b| b.1.cmp(&a.1));
        bottlenecks.truncate(10); // Top 10 bottlenecks

        // Detect circular dependencies
        let circular_dependencies = Self::detect_circular_dependencies(dependencies);

        Ok(DependencyAnalysis {
            total_projects,
            projects_with_dependencies,
            standalone_projects,
            circular_dependencies,
            orphaned_projects,
            bottlenecks,
            missing_dependencies: missing_dependencies.to_vec(),
        })
    }

    /// Detect circular dependencies
    fn detect_circular_dependencies(
        dependencies: &HashMap<String, Vec<String>>,
    ) -> Vec<Vec<String>> {
        let mut cycles = Vec::new();
        let mut visited = HashSet::new();
        let mut rec_stack = HashSet::new();

        for node in dependencies.keys() {
            if !visited.contains(node) {
                Self::detect_cycle_dfs(
                    node,
                    dependencies,
                    &mut visited,
                    &mut rec_stack,
                    &mut Vec::new(),
                    &mut cycles,
                );
            }
        }

        cycles
    }

    /// DFS helper for cycle detection
    fn detect_cycle_dfs(
        node: &str,
        dependencies: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        rec_stack: &mut HashSet<String>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(node.to_string());
        rec_stack.insert(node.to_string());
        path.push(node.to_string());

        if let Some(deps) = dependencies.get(node) {
            for dep in deps {
                if !visited.contains(dep) {
                    Self::detect_cycle_dfs(dep, dependencies, visited, rec_stack, path, cycles);
                } else if rec_stack.contains(dep) {
                    // Found a cycle
                    if let Some(cycle_start) = path.iter().position(|p| p == dep) {
                        let cycle = path[cycle_start..].to_vec();
                        cycles.push(cycle);
                    }
                }
            }
        }

        path.pop();
        rec_stack.remove(node);
    }

    /// Find all projects impacted by changes to a target project
    fn find_impacted_projects(
        target: &str,
        _dependencies: &HashMap<String, Vec<String>>,
        reverse_dependencies: &HashMap<String, Vec<String>>,
    ) -> HashSet<String> {
        let mut impacted = HashSet::new();
        let mut queue = vec![target.to_string()];
        let mut visited = HashSet::new();

        while let Some(current) = queue.pop() {
            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());

            if let Some(dependents) = reverse_dependencies.get(&current) {
                for dependent in dependents {
                    if !impacted.contains(dependent) {
                        impacted.insert(dependent.clone());
                        queue.push(dependent.clone());
                    }
                }
            }
        }

        // Remove the target itself from the impacted set
        impacted.remove(target);
        impacted
    }

    /// Display analysis results
    fn display_analysis(ctx: &Context, analysis: &DependencyAnalysis) -> Result<()> {
        // Summary
        ctx.output.subsection("Summary");
        ctx.output
            .key_value("Total Projects", &analysis.total_projects.to_string());
        ctx.output.key_value(
            "Projects with Dependencies",
            &analysis.projects_with_dependencies.to_string(),
        );
        ctx.output.key_value(
            "Standalone Projects",
            &analysis.standalone_projects.len().to_string(),
        );
        output::blank();

        // Health checks
        ctx.output.subsection("Health Checks");

        // Circular dependencies
        if analysis.circular_dependencies.is_empty() {
            ctx.output.success("✓ No circular dependencies detected");
        } else {
            ctx.output.error(&format!(
                "✗ Circular dependencies detected: {}",
                analysis.circular_dependencies.len()
            ));
            for (i, cycle) in analysis.circular_dependencies.iter().enumerate() {
                ctx.output
                    .dimmed(&format!("  Cycle {}: {}", i + 1, cycle.join(" → ")));
            }
        }

        // Missing dependencies
        if analysis.missing_dependencies.is_empty() {
            ctx.output
                .success("✓ No missing dependencies (all references are valid)");
        } else {
            ctx.output.error(&format!(
                "✗ Missing dependencies: {}",
                analysis.missing_dependencies.len()
            ));
            for (project, missing) in &analysis.missing_dependencies {
                ctx.output.dimmed(&format!(
                    "  {} references missing project: {}",
                    project, missing
                ));
            }
        }

        output::blank();

        // Orphaned projects
        if !analysis.orphaned_projects.is_empty() {
            ctx.output.subsection("Orphaned Projects");
            ctx.output
                .dimmed("Projects that no other projects depend on:");
            output::blank();
            for project in &analysis.orphaned_projects {
                ctx.output.info(&format!("• {}", project));
            }
            output::blank();
        }

        // Bottlenecks
        if !analysis.bottlenecks.is_empty() {
            ctx.output.subsection("Dependency Bottlenecks");
            ctx.output
                .dimmed("Projects that many others depend on (top 10):");
            output::blank();
            for (project, count) in &analysis.bottlenecks {
                ctx.output.info(&format!(
                    "{} ← {} project(s) depend on this",
                    project, count
                ));
            }
            output::blank();
        }

        // Standalone projects
        if !analysis.standalone_projects.is_empty() {
            ctx.output.subsection("Standalone Projects");
            ctx.output
                .dimmed("Projects with no dependencies (can be deployed independently):");
            output::blank();
            for project in &analysis.standalone_projects {
                ctx.output.info(&format!("• {}", project));
            }
            output::blank();
        }

        Ok(())
    }
}
