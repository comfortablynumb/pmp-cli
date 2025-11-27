use crate::template::DynamicProjectEnvironmentResource;
use crate::traits::FileSystem;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

/// Represents a node in the dependency graph
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DependencyNode {
    /// Name of the project
    pub project_name: String,
    /// Environment name
    pub environment_name: String,
    /// Path to the environment directory
    pub environment_path: PathBuf,
}

impl DependencyNode {
    /// Create a new dependency node
    pub fn new(project_name: String, environment_name: String, environment_path: PathBuf) -> Self {
        Self {
            project_name,
            environment_name,
            environment_path,
        }
    }

    /// Create a unique key for this node
    pub fn key(&self) -> String {
        format!("{}:{}", self.project_name, self.environment_name)
    }
}

/// Represents the dependency graph for a project and its dependencies
#[derive(Debug)]
pub struct DependencyGraph {
    /// The root node (the project we're executing)
    pub root: DependencyNode,
    /// All nodes in the dependency graph (includes root)
    pub nodes: Vec<DependencyNode>,
    /// Adjacency list (node -> list of nodes it depends on)
    pub dependencies: HashMap<String, Vec<DependencyNode>>,
}

impl DependencyGraph {
    /// Build a dependency graph starting from a root project/environment
    pub fn build(
        fs: &dyn FileSystem,
        root_path: &Path,
        project_name: &str,
        environment_name: &str,
    ) -> Result<Self> {
        let root = DependencyNode::new(
            project_name.to_string(),
            environment_name.to_string(),
            root_path.to_path_buf(),
        );

        let mut graph = DependencyGraph {
            root: root.clone(),
            nodes: Vec::new(),
            dependencies: HashMap::new(),
        };

        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();

        // Start with the root node
        queue.push_back(root.clone());

        while let Some(current_node) = queue.pop_front() {
            let node_key = current_node.key();

            if visited.contains(&node_key) {
                continue;
            }

            visited.insert(node_key.clone());
            graph.nodes.push(current_node.clone());

            // Load environment resource to get dependencies
            let env_file = current_node.environment_path.join(".pmp.environment.yaml");
            if !fs.exists(&env_file) {
                anyhow::bail!(
                    "Environment file not found for {}: {:?}",
                    node_key,
                    env_file
                );
            }

            let resource = DynamicProjectEnvironmentResource::from_file(fs, &env_file)
                .with_context(|| format!("Failed to load environment resource: {:?}", env_file))?;

            // Process dependencies
            let deps = &resource.spec.dependencies;
            if !deps.is_empty() {
                let mut dep_nodes = Vec::new();

                for dep in deps {
                    // For each dependency, we need to resolve all its environments
                    for dep_env in &dep.project.environments {
                        // Find the dependency project
                        // If create: true and project doesn't exist, skip it (will be created later)
                        match Self::find_dependency_project(
                            fs,
                            &dep.project.name,
                            dep_env,
                            dep.project.create,
                        )? {
                            Some(dep_node) => {
                                dep_nodes.push(dep_node.clone());
                                queue.push_back(dep_node);
                            }
                            None => {
                                // Dependency with create: true doesn't exist yet, skip it
                                // It will be created by ProjectGroupHandler
                            }
                        }
                    }
                }

                if !dep_nodes.is_empty() {
                    graph.dependencies.insert(node_key, dep_nodes);
                }
            }
        }

        Ok(graph)
    }

    /// Find a dependency project by name and environment
    /// If create_if_missing is true and the project doesn't exist, returns None instead of an error
    fn find_dependency_project(
        fs: &dyn FileSystem,
        project_name: &str,
        environment_name: &str,
        create_if_missing: bool,
    ) -> Result<Option<DependencyNode>> {
        // Search for the project in the projects directory
        // Try both relative and absolute paths (for tests)
        let projects_dirs = [PathBuf::from("projects"), PathBuf::from("/projects")];

        let projects_dir = projects_dirs.iter().find(|dir| fs.exists(dir));

        let Some(projects_dir) = projects_dir else {
            if create_if_missing {
                // Project directory doesn't exist, but that's OK if create: true
                return Ok(None);
            }
            anyhow::bail!(
                "Projects directory not found. Cannot resolve dependency: {} (env: {})",
                project_name,
                environment_name
            );
        };

        // Recursively search for the project
        let project_path = match Self::search_project(fs, projects_dir, project_name) {
            Ok(path) => path,
            Err(_) if create_if_missing => {
                // Project doesn't exist, but that's OK if create: true
                return Ok(None);
            }
            Err(e) => return Err(e),
        };

        let env_path = project_path.join("environments").join(environment_name);

        if !fs.exists(&env_path) {
            if create_if_missing {
                // Environment doesn't exist, but that's OK if create: true
                return Ok(None);
            }
            anyhow::bail!(
                "Environment '{}' not found for project '{}' at {:?}",
                environment_name,
                project_name,
                env_path
            );
        }

        Ok(Some(DependencyNode::new(
            project_name.to_string(),
            environment_name.to_string(),
            env_path,
        )))
    }

    /// Recursively search for a project by name
    fn search_project(
        fs: &dyn FileSystem,
        search_dir: &Path,
        project_name: &str,
    ) -> Result<PathBuf> {
        // Check if this directory contains a .pmp.project.yaml
        let project_file = search_dir.join(".pmp.project.yaml");

        if fs.exists(&project_file) {
            // Load the project and check its name
            let resource = crate::template::ProjectResource::from_file(fs, &project_file)
                .with_context(|| format!("Failed to load project file: {:?}", project_file))?;

            if resource.metadata.name == project_name {
                return Ok(search_dir.to_path_buf());
            }
        }

        // Search subdirectories
        let entries = fs.read_dir(search_dir)?;

        for entry in entries {
            let path = entry;

            if fs.is_dir(&path)
                && let Ok(found_path) = Self::search_project(fs, &path, project_name)
            {
                return Ok(found_path);
            }
        }

        anyhow::bail!("Project '{}' not found", project_name)
    }

    /// Get the execution order (topologically sorted)
    /// Returns nodes in the order they should be executed (dependencies first)
    pub fn execution_order(&self) -> Result<Vec<DependencyNode>> {
        let mut order = Vec::new();
        let mut visited = HashSet::new();
        let mut visiting = HashSet::new();

        self.topological_sort(&self.root, &mut order, &mut visited, &mut visiting)?;

        Ok(order)
    }

    /// Perform topological sort using DFS
    fn topological_sort(
        &self,
        node: &DependencyNode,
        order: &mut Vec<DependencyNode>,
        visited: &mut HashSet<String>,
        visiting: &mut HashSet<String>,
    ) -> Result<()> {
        let node_key = node.key();

        if visited.contains(&node_key) {
            return Ok(());
        }

        if visiting.contains(&node_key) {
            anyhow::bail!("Circular dependency detected involving: {}", node_key);
        }

        visiting.insert(node_key.clone());

        // Visit dependencies first
        if let Some(deps) = self.dependencies.get(&node_key) {
            for dep in deps {
                self.topological_sort(dep, order, visited, visiting)?;
            }
        }

        visiting.remove(&node_key);
        visited.insert(node_key);
        order.push(node.clone());

        Ok(())
    }

    /// Check if there are any dependencies
    #[allow(dead_code)]
    pub fn has_dependencies(&self) -> bool {
        !self.dependencies.is_empty()
    }

    /// Get the total count of nodes (including root)
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Format the dependency tree as a string for display
    pub fn format_tree(&self) -> String {
        let mut output = String::new();
        let mut visited = HashSet::new();

        self.format_tree_node(&self.root, &mut output, "", true, &mut visited);

        output
    }

    /// Recursively format a node and its dependencies
    fn format_tree_node(
        &self,
        node: &DependencyNode,
        output: &mut String,
        prefix: &str,
        is_last: bool,
        visited: &mut HashSet<String>,
    ) {
        let node_key = node.key();

        // Print the current node
        let connector = if is_last { "└── " } else { "├── " };
        output.push_str(&format!(
            "{}{}{} ({})\n",
            prefix, connector, node.project_name, node.environment_name
        ));

        // Mark as visited
        visited.insert(node_key.clone());

        // Get dependencies
        if let Some(deps) = self.dependencies.get(&node_key) {
            let dep_count = deps.len();

            for (i, dep) in deps.iter().enumerate() {
                let is_last_dep = i == dep_count - 1;
                let new_prefix = if is_last {
                    format!("{}    ", prefix)
                } else {
                    format!("{}│   ", prefix)
                };

                // Check if we've already visited this dependency (to avoid infinite loops in display)
                let dep_key = dep.key();
                if visited.contains(&dep_key) {
                    // Show it's already visited
                    let dep_connector = if is_last_dep {
                        "└── "
                    } else {
                        "├── "
                    };
                    output.push_str(&format!(
                        "{}{}{} ({}) [already shown]\n",
                        new_prefix, dep_connector, dep.project_name, dep.environment_name
                    ));
                } else {
                    self.format_tree_node(dep, output, &new_prefix, is_last_dep, visited);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::MockFileSystem;
    use std::sync::Arc;

    #[test]
    fn test_dependency_node_key() {
        let node = DependencyNode::new(
            "test-project".to_string(),
            "dev".to_string(),
            PathBuf::from("/path/to/env"),
        );

        assert_eq!(node.key(), "test-project:dev");
    }

    #[test]
    fn test_dependency_node_equality() {
        let node1 = DependencyNode::new(
            "test-project".to_string(),
            "dev".to_string(),
            PathBuf::from("/path/to/env"),
        );

        let node2 = DependencyNode::new(
            "test-project".to_string(),
            "dev".to_string(),
            PathBuf::from("/path/to/env"),
        );

        let node3 = DependencyNode::new(
            "other-project".to_string(),
            "dev".to_string(),
            PathBuf::from("/path/to/env"),
        );

        // Should be equal when all fields match
        assert_eq!(node1, node2);
        // Should not be equal when project name differs
        assert_ne!(node1, node3);
    }

    #[test]
    fn test_empty_dependency_graph() {
        let fs = Arc::new(MockFileSystem::new());

        // Create a simple project with no dependencies
        let env_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: test-project
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies: []
"#;

        let env_path = PathBuf::from("/projects/test_resource/test-project/environments/dev");
        fs.create_dir_all(&env_path).unwrap();
        fs.write(&env_path.join(".pmp.environment.yaml"), env_yaml)
            .unwrap();

        let graph = DependencyGraph::build(&*fs, &env_path, "test-project", "dev").unwrap();

        assert_eq!(graph.node_count(), 1);
        assert!(!graph.has_dependencies());
        assert_eq!(graph.root.project_name, "test-project");
        assert_eq!(graph.root.environment_name, "dev");
    }

    #[test]
    fn test_simple_dependency_chain() {
        let fs = Arc::new(MockFileSystem::new());

        // Create projects directory
        fs.create_dir_all(&PathBuf::from("/projects")).unwrap();

        // Create project-a (root) that depends on project-b
        let project_a_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: project-a
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies:
    - project:
        name: project-b
        environments:
          - dev
"#;

        // Create project-b (dependency)
        let project_b_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: project-b
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies: []
"#;

        // Create project directory structure
        let project_a_path = PathBuf::from("/projects/test_resource/project-a");
        let project_a_env_path = project_a_path.join("environments/dev");
        fs.create_dir_all(&project_a_env_path).unwrap();
        fs.write(
            &project_a_path.join(".pmp.project.yaml"),
            r#"
apiVersion: pmp.io/v1
kind: Project
metadata:
  name: project-a
"#,
        )
        .unwrap();
        fs.write(
            &project_a_env_path.join(".pmp.environment.yaml"),
            project_a_yaml,
        )
        .unwrap();

        let project_b_path = PathBuf::from("/projects/test_resource/project-b");
        let project_b_env_path = project_b_path.join("environments/dev");
        fs.create_dir_all(&project_b_env_path).unwrap();
        fs.write(
            &project_b_path.join(".pmp.project.yaml"),
            r#"
apiVersion: pmp.io/v1
kind: Project
metadata:
  name: project-b
"#,
        )
        .unwrap();
        fs.write(
            &project_b_env_path.join(".pmp.environment.yaml"),
            project_b_yaml,
        )
        .unwrap();

        let graph = DependencyGraph::build(&*fs, &project_a_env_path, "project-a", "dev").unwrap();

        assert_eq!(graph.node_count(), 2);
        assert!(graph.has_dependencies());
        assert_eq!(graph.root.project_name, "project-a");

        // Check execution order (dependencies first)
        let order = graph.execution_order().unwrap();
        assert_eq!(order.len(), 2);
        assert_eq!(order[0].project_name, "project-b");
        assert_eq!(order[1].project_name, "project-a");
    }

    #[test]
    fn test_multi_level_dependency_chain() {
        let fs = Arc::new(MockFileSystem::new());

        // Create projects directory
        fs.create_dir_all(&PathBuf::from("/projects")).unwrap();

        // Create project-a (root) -> project-b -> project-c
        let project_a_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: project-a
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies:
    - project:
        name: project-b
        environments:
          - dev
"#;

        let project_b_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: project-b
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies:
    - project:
        name: project-c
        environments:
          - dev
"#;

        let project_c_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: project-c
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies: []
"#;

        // Create projects
        for (name, yaml) in [
            ("project-a", project_a_yaml),
            ("project-b", project_b_yaml),
            ("project-c", project_c_yaml),
        ] {
            let project_path = PathBuf::from(format!("/projects/test_resource/{}", name));
            let env_path = project_path.join("environments/dev");
            fs.create_dir_all(&env_path).unwrap();
            let project_yaml = format!(
                r#"
apiVersion: pmp.io/v1
kind: Project
metadata:
  name: {}
"#,
                name
            );
            fs.write(&project_path.join(".pmp.project.yaml"), &project_yaml)
                .unwrap();
            fs.write(&env_path.join(".pmp.environment.yaml"), yaml)
                .unwrap();
        }

        let project_a_env_path =
            PathBuf::from("/projects/test_resource/project-a/environments/dev");
        let graph = DependencyGraph::build(&*fs, &project_a_env_path, "project-a", "dev").unwrap();

        assert_eq!(graph.node_count(), 3);

        // Check execution order (deepest dependency first)
        let order = graph.execution_order().unwrap();
        assert_eq!(order.len(), 3);
        assert_eq!(order[0].project_name, "project-c");
        assert_eq!(order[1].project_name, "project-b");
        assert_eq!(order[2].project_name, "project-a");
    }

    #[test]
    fn test_multiple_dependencies_same_level() {
        let fs = Arc::new(MockFileSystem::new());

        // Create projects directory
        fs.create_dir_all(&PathBuf::from("/projects")).unwrap();

        // Create project-a (root) -> [project-b, project-c]
        let project_a_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: project-a
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies:
    - project:
        name: project-b
        environments:
          - dev
    - project:
        name: project-c
        environments:
          - dev
"#;

        let project_b_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: project-b
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies: []
"#;

        let project_c_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: project-c
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies: []
"#;

        // Create projects
        for (name, yaml) in [
            ("project-a", project_a_yaml),
            ("project-b", project_b_yaml),
            ("project-c", project_c_yaml),
        ] {
            let project_path = PathBuf::from(format!("/projects/test_resource/{}", name));
            let env_path = project_path.join("environments/dev");
            fs.create_dir_all(&env_path).unwrap();
            let project_yaml = format!(
                r#"
apiVersion: pmp.io/v1
kind: Project
metadata:
  name: {}
"#,
                name
            );
            fs.write(&project_path.join(".pmp.project.yaml"), &project_yaml)
                .unwrap();
            fs.write(&env_path.join(".pmp.environment.yaml"), yaml)
                .unwrap();
        }

        let project_a_env_path =
            PathBuf::from("/projects/test_resource/project-a/environments/dev");
        let graph = DependencyGraph::build(&*fs, &project_a_env_path, "project-a", "dev").unwrap();

        assert_eq!(graph.node_count(), 3);

        // Check execution order (both dependencies before root)
        let order = graph.execution_order().unwrap();
        assert_eq!(order.len(), 3);
        // project-b and project-c should come before project-a
        assert!(
            order
                .iter()
                .position(|n| n.project_name == "project-b")
                .unwrap()
                < 2
        );
        assert!(
            order
                .iter()
                .position(|n| n.project_name == "project-c")
                .unwrap()
                < 2
        );
        assert_eq!(order[2].project_name, "project-a");
    }

    #[test]
    fn test_circular_dependency_detection() {
        let fs = Arc::new(MockFileSystem::new());

        // Create projects directory
        fs.create_dir_all(&PathBuf::from("/projects")).unwrap();

        // Create circular dependency: project-a -> project-b -> project-a
        let project_a_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: project-a
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies:
    - project:
        name: project-b
        environments:
          - dev
"#;

        let project_b_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: project-b
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies:
    - project:
        name: project-a
        environments:
          - dev
"#;

        // Create projects
        for (name, yaml) in [("project-a", project_a_yaml), ("project-b", project_b_yaml)] {
            let project_path = PathBuf::from(format!("/projects/test_resource/{}", name));
            let env_path = project_path.join("environments/dev");
            fs.create_dir_all(&env_path).unwrap();
            let project_yaml = format!(
                r#"
apiVersion: pmp.io/v1
kind: Project
metadata:
  name: {}
"#,
                name
            );
            fs.write(&project_path.join(".pmp.project.yaml"), &project_yaml)
                .unwrap();
            fs.write(&env_path.join(".pmp.environment.yaml"), yaml)
                .unwrap();
        }

        let project_a_env_path =
            PathBuf::from("/projects/test_resource/project-a/environments/dev");
        let graph = DependencyGraph::build(&*fs, &project_a_env_path, "project-a", "dev").unwrap();

        // Building the graph should succeed
        assert_eq!(graph.node_count(), 2);

        // But getting execution order should fail with circular dependency error
        let result = graph.execution_order();
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Circular dependency")
        );
    }

    #[test]
    fn test_multiple_environments() {
        let fs = Arc::new(MockFileSystem::new());

        // Create projects directory
        fs.create_dir_all(&PathBuf::from("/projects")).unwrap();

        // Create project-a (dev) that depends on project-b (dev and staging)
        let project_a_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: project-a
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies:
    - project:
        name: project-b
        environments:
          - dev
          - staging
"#;

        let project_b_dev_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: project-b
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies: []
"#;

        let project_b_staging_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: project-b
  environment_name: staging
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies: []
"#;

        // Create project-a
        let project_a_path = PathBuf::from("/projects/test_resource/project-a");
        let project_a_env_path = project_a_path.join("environments/dev");
        fs.create_dir_all(&project_a_env_path).unwrap();
        fs.write(
            &project_a_path.join(".pmp.project.yaml"),
            r#"
apiVersion: pmp.io/v1
kind: Project
metadata:
  name: project-a
"#,
        )
        .unwrap();
        fs.write(
            &project_a_env_path.join(".pmp.environment.yaml"),
            project_a_yaml,
        )
        .unwrap();

        // Create project-b with two environments
        let project_b_path = PathBuf::from("/projects/test_resource/project-b");
        fs.create_dir_all(&project_b_path.join("environments/dev"))
            .unwrap();
        fs.create_dir_all(&project_b_path.join("environments/staging"))
            .unwrap();
        fs.write(
            &project_b_path.join(".pmp.project.yaml"),
            r#"
apiVersion: pmp.io/v1
kind: Project
metadata:
  name: project-b
"#,
        )
        .unwrap();
        fs.write(
            &project_b_path.join("environments/dev/.pmp.environment.yaml"),
            project_b_dev_yaml,
        )
        .unwrap();
        fs.write(
            &project_b_path.join("environments/staging/.pmp.environment.yaml"),
            project_b_staging_yaml,
        )
        .unwrap();

        let graph = DependencyGraph::build(&*fs, &project_a_env_path, "project-a", "dev").unwrap();

        // Should include project-a:dev, project-b:dev, and project-b:staging
        assert_eq!(graph.node_count(), 3);

        let order = graph.execution_order().unwrap();
        assert_eq!(order.len(), 3);

        // Both project-b environments should come before project-a
        let project_a_pos = order
            .iter()
            .position(|n| n.project_name == "project-a")
            .unwrap();
        let project_b_dev_pos = order
            .iter()
            .position(|n| n.project_name == "project-b" && n.environment_name == "dev")
            .unwrap();
        let project_b_staging_pos = order
            .iter()
            .position(|n| n.project_name == "project-b" && n.environment_name == "staging")
            .unwrap();

        assert!(project_b_dev_pos < project_a_pos);
        assert!(project_b_staging_pos < project_a_pos);
    }

    #[test]
    fn test_dependency_tree_formatting() {
        let fs = Arc::new(MockFileSystem::new());

        // Create projects directory
        fs.create_dir_all(&PathBuf::from("/projects")).unwrap();

        // Create simple dependency: project-a -> project-b
        let project_a_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: project-a
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies:
    - project:
        name: project-b
        environments:
          - dev
"#;

        let project_b_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: project-b
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies: []
"#;

        // Create projects
        for (name, yaml) in [("project-a", project_a_yaml), ("project-b", project_b_yaml)] {
            let project_path = PathBuf::from(format!("/projects/test_resource/{}", name));
            let env_path = project_path.join("environments/dev");
            fs.create_dir_all(&env_path).unwrap();
            let project_yaml = format!(
                r#"
apiVersion: pmp.io/v1
kind: Project
metadata:
  name: {}
"#,
                name
            );
            fs.write(&project_path.join(".pmp.project.yaml"), &project_yaml)
                .unwrap();
            fs.write(&env_path.join(".pmp.environment.yaml"), yaml)
                .unwrap();
        }

        let project_a_env_path =
            PathBuf::from("/projects/test_resource/project-a/environments/dev");
        let graph = DependencyGraph::build(&*fs, &project_a_env_path, "project-a", "dev").unwrap();

        let tree = graph.format_tree();

        // Tree should contain both projects
        assert!(tree.contains("project-a (dev)"));
        assert!(tree.contains("project-b (dev)"));

        // Tree should use box drawing characters
        assert!(tree.contains("└──") || tree.contains("├──"));
    }

    #[test]
    fn test_missing_dependency_project() {
        let fs = Arc::new(MockFileSystem::new());

        // Create project-a that depends on non-existent project-b
        let project_a_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: project-a
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies:
    - project:
        name: project-b
        environments:
          - dev
"#;

        let project_a_path = PathBuf::from("/projects/test_resource/project-a");
        let project_a_env_path = project_a_path.join("environments/dev");
        fs.create_dir_all(&project_a_env_path).unwrap();
        fs.write(
            &project_a_path.join(".pmp.project.yaml"),
            r#"
apiVersion: pmp.io/v1
kind: Project
metadata:
  name: project-a
"#,
        )
        .unwrap();
        fs.write(
            &project_a_env_path.join(".pmp.environment.yaml"),
            project_a_yaml,
        )
        .unwrap();

        let result = DependencyGraph::build(&*fs, &project_a_env_path, "project-a", "dev");

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("project-b"));
    }

    #[test]
    fn test_missing_dependency_environment() {
        let fs = Arc::new(MockFileSystem::new());

        // Create project-a that depends on project-b:staging (which doesn't exist)
        let project_a_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: project-a
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies:
    - project:
        name: project-b
        environments:
          - staging
"#;

        let project_b_yaml = r#"
apiVersion: pmp.io/v1
kind: TestResource
metadata:
  name: project-b
  environment_name: dev
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: TestResource
  executor:
    name: opentofu
  inputs: {}
  dependencies: []
"#;

        // Create project-a
        let project_a_path = PathBuf::from("/projects/test_resource/project-a");
        let project_a_env_path = project_a_path.join("environments/dev");
        fs.create_dir_all(&project_a_env_path).unwrap();
        fs.write(
            &project_a_path.join(".pmp.project.yaml"),
            r#"
apiVersion: pmp.io/v1
kind: Project
metadata:
  name: project-a
"#,
        )
        .unwrap();
        fs.write(
            &project_a_env_path.join(".pmp.environment.yaml"),
            project_a_yaml,
        )
        .unwrap();

        // Create project-b with only dev environment
        let project_b_path = PathBuf::from("/projects/test_resource/project-b");
        let project_b_env_path = project_b_path.join("environments/dev");
        fs.create_dir_all(&project_b_env_path).unwrap();
        fs.write(
            &project_b_path.join(".pmp.project.yaml"),
            r#"
apiVersion: pmp.io/v1
kind: Project
metadata:
  name: project-b
"#,
        )
        .unwrap();
        fs.write(
            &project_b_env_path.join(".pmp.environment.yaml"),
            project_b_yaml,
        )
        .unwrap();

        let result = DependencyGraph::build(&*fs, &project_a_env_path, "project-a", "dev");

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("staging"));
    }
}
