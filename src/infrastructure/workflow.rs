use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::infrastructure::config_generator::{
    ConfigGenerator, FileOrganization, GeneratorConfig,
};
use crate::infrastructure::discovery::{
    DiscoveredResource, ImportDestination, ImportStatus, ResourceDependency,
    ResourceImportResult,
};
use crate::infrastructure::error::{ImportError, ImportResult};
use crate::traits::Output;

/// Options for the import workflow
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ImportWorkflowOptions {
    /// Whether to automatically detect and import dependencies
    pub auto_detect_dependencies: bool,
    /// Whether to run in non-interactive mode
    pub non_interactive: bool,
    /// Whether to generate config files
    pub generate_config: bool,
    /// How to organize generated files
    pub file_organization: FileOrganization,
    /// Whether to continue on errors (for batch imports)
    pub continue_on_error: bool,
    /// Whether to run tofu init
    pub run_init: bool,
    /// Whether to run tofu plan with config generation
    pub run_plan: bool,
    /// Whether to run tofu apply
    pub run_apply: bool,
}

impl Default for ImportWorkflowOptions {
    fn default() -> Self {
        Self {
            auto_detect_dependencies: true,
            non_interactive: false,
            generate_config: true,
            file_organization: FileOrganization::SingleFile,
            continue_on_error: false,
            run_init: true,
            run_plan: true,
            run_apply: false,
        }
    }
}

/// Result of the import workflow
#[derive(Debug)]
pub struct ImportWorkflowResult {
    /// Results for each resource
    pub resource_results: Vec<ResourceImportResult>,
    /// Files that were generated
    pub generated_files: Vec<String>,
    /// Path to the generated config (if any)
    pub generated_config_path: Option<PathBuf>,
    /// Overall success status
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

#[allow(dead_code)]
impl ImportWorkflowResult {
    /// Count successful imports
    pub fn succeeded_count(&self) -> usize {
        self.resource_results
            .iter()
            .filter(|r| r.status == ImportStatus::Succeeded)
            .count()
    }

    /// Count failed imports
    pub fn failed_count(&self) -> usize {
        self.resource_results
            .iter()
            .filter(|r| r.status == ImportStatus::Failed)
            .count()
    }
}

/// Orchestrates the infrastructure import workflow
pub struct ImportWorkflow<'a> {
    options: ImportWorkflowOptions,
    output: &'a dyn Output,
}

impl<'a> ImportWorkflow<'a> {
    /// Create a new import workflow
    pub fn new(options: ImportWorkflowOptions, output: &'a dyn Output) -> Self {
        Self { options, output }
    }

    /// Execute the import workflow
    pub fn execute(
        &self,
        resources: Vec<DiscoveredResource>,
        _destination: &ImportDestination,
        project_path: &Path,
    ) -> ImportResult<ImportWorkflowResult> {
        self.output.section("Import Workflow");

        // Step 1: Order resources by dependencies
        self.output.info("Ordering resources by dependencies...");
        let ordered_resources = self.order_by_dependencies(&resources)?;
        self.output.success(&format!(
            "Ordered {} resources for import",
            ordered_resources.len()
        ));

        // Step 2: Generate import blocks
        let mut result = ImportWorkflowResult {
            resource_results: Vec::new(),
            generated_files: Vec::new(),
            generated_config_path: None,
            success: false,
            error: None,
        };

        if self.options.generate_config {
            self.output.info("Generating import blocks...");
            let generator = self.create_generator();
            let files = generator.write_files(&ordered_resources, project_path)?;
            result.generated_files = files.clone();

            for file in &files {
                self.output.success(&format!("Generated: {}", file));
            }
        }

        // Step 3: Run tofu init (if enabled)
        if self.options.run_init {
            self.output.info("Running tofu init...");
            self.run_tofu_init(project_path)?;
            self.output.success("Terraform/OpenTofu initialized");
        }

        // Step 4: Run tofu plan with config generation (if enabled)
        if self.options.run_plan && self.options.generate_config {
            self.output.info("Running tofu plan -generate-config-out...");
            let config_path = self.run_tofu_plan_generate(project_path)?;
            result.generated_config_path = Some(config_path.clone());
            self.output.success(&format!(
                "Generated resource config: {}",
                config_path.display()
            ));
        }

        // Step 5: Run tofu apply (if enabled)
        if self.options.run_apply {
            self.output
                .info("Running tofu apply to import resources...");

            match self.run_tofu_apply(project_path) {
                Ok(_) => {
                    self.output.success("Resources imported successfully");

                    for resource in &ordered_resources {
                        result.resource_results.push(ResourceImportResult {
                            resource: resource.clone(),
                            status: ImportStatus::Succeeded,
                            error: None,
                            tf_name: Some(resource.suggested_tf_name()),
                        });
                    }

                    // Move imports file to .completed
                    self.archive_imports(project_path)?;
                }
                Err(e) => {
                    self.output.error(&format!("Apply failed: {}", e));

                    for resource in &ordered_resources {
                        result.resource_results.push(ResourceImportResult {
                            resource: resource.clone(),
                            status: ImportStatus::Failed,
                            error: Some(e.to_string()),
                            tf_name: Some(resource.suggested_tf_name()),
                        });
                    }

                    if !self.options.continue_on_error {
                        result.error = Some(e.to_string());
                        return Ok(result);
                    }
                }
            }
        } else {
            // Mark as pending since apply wasn't run
            for resource in &ordered_resources {
                result.resource_results.push(ResourceImportResult {
                    resource: resource.clone(),
                    status: ImportStatus::Pending,
                    error: None,
                    tf_name: Some(resource.suggested_tf_name()),
                });
            }
        }

        result.success = result.failed_count() == 0;
        Ok(result)
    }

    /// Order resources by their dependencies (topological sort)
    fn order_by_dependencies(
        &self,
        resources: &[DiscoveredResource],
    ) -> ImportResult<Vec<DiscoveredResource>> {
        let resource_map: HashMap<String, &DiscoveredResource> = resources
            .iter()
            .map(|r| (format!("{}:{}", r.resource_type, r.resource_id), r))
            .collect();

        // Build adjacency list
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut dependents: HashMap<String, Vec<String>> = HashMap::new();

        for resource in resources {
            let key = format!("{}:{}", resource.resource_type, resource.resource_id);
            in_degree.entry(key.clone()).or_insert(0);

            for dep in &resource.dependencies {
                let dep_key = format!("{}:{}", dep.resource_type, dep.resource_id);

                if resource_map.contains_key(&dep_key) {
                    *in_degree.entry(key.clone()).or_insert(0) += 1;
                    dependents
                        .entry(dep_key)
                        .or_default()
                        .push(key.clone());
                }
            }
        }

        // Topological sort using Kahn's algorithm
        let mut queue: Vec<String> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(k, _)| k.clone())
            .collect();

        let mut result = Vec::new();

        while let Some(key) = queue.pop() {
            if let Some(resource) = resource_map.get(&key) {
                result.push((*resource).clone());
            }

            if let Some(deps) = dependents.get(&key) {
                for dep_key in deps {
                    if let Some(deg) = in_degree.get_mut(dep_key) {
                        *deg -= 1;

                        if *deg == 0 {
                            queue.push(dep_key.clone());
                        }
                    }
                }
            }
        }

        // Check for cycles
        if result.len() != resources.len() {
            return Err(ImportError::DependencyResolution(
                "Circular dependency detected in resources".to_string(),
            ));
        }

        Ok(result)
    }

    /// Create a config generator with current options
    fn create_generator(&self) -> ConfigGenerator {
        let config = GeneratorConfig {
            file_organization: self.options.file_organization.clone(),
            include_comments: true,
            name_overrides: HashMap::new(),
        };
        ConfigGenerator::new(config)
    }

    /// Run tofu init
    fn run_tofu_init(&self, project_path: &Path) -> ImportResult<()> {
        let output = Command::new("tofu")
            .arg("init")
            .current_dir(project_path)
            .output()
            .map_err(|e| {
                ImportError::ExecutorFailed {
                    command: "tofu init".to_string(),
                    message: e.to_string(),
                    exit_code: None,
                }
            })?;

        if !output.status.success() {
            return Err(ImportError::ExecutorFailed {
                command: "tofu init".to_string(),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code(),
            });
        }

        Ok(())
    }

    /// Run tofu plan with config generation
    fn run_tofu_plan_generate(&self, project_path: &Path) -> ImportResult<PathBuf> {
        let config_file = "generated_resources.tf";
        let output = Command::new("tofu")
            .args([
                "plan",
                &format!("-generate-config-out={}", config_file),
            ])
            .current_dir(project_path)
            .output()
            .map_err(|e| {
                ImportError::ExecutorFailed {
                    command: "tofu plan".to_string(),
                    message: e.to_string(),
                    exit_code: None,
                }
            })?;

        if !output.status.success() {
            return Err(ImportError::ExecutorFailed {
                command: "tofu plan -generate-config-out".to_string(),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code(),
            });
        }

        Ok(project_path.join(config_file))
    }

    /// Run tofu apply
    fn run_tofu_apply(&self, project_path: &Path) -> ImportResult<()> {
        let mut args = vec!["apply"];

        if self.options.non_interactive {
            args.push("-auto-approve");
        }

        let output = Command::new("tofu")
            .args(&args)
            .current_dir(project_path)
            .output()
            .map_err(|e| {
                ImportError::ExecutorFailed {
                    command: "tofu apply".to_string(),
                    message: e.to_string(),
                    exit_code: None,
                }
            })?;

        if !output.status.success() {
            return Err(ImportError::ExecutorFailed {
                command: "tofu apply".to_string(),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code(),
            });
        }

        Ok(())
    }

    /// Archive import blocks after successful apply
    fn archive_imports(&self, project_path: &Path) -> ImportResult<()> {
        let imports_file = project_path.join("_imports.tf");
        let completed_file = project_path.join("_imports.tf.completed");

        if imports_file.exists() {
            std::fs::rename(&imports_file, &completed_file).map_err(|e| {
                ImportError::FileSystem(format!(
                    "Failed to archive imports file: {}",
                    e
                ))
            })?;
        }

        Ok(())
    }
}

/// Recursively collect dependencies for a set of resources
#[allow(dead_code)]
pub fn collect_all_dependencies(
    resources: &[DiscoveredResource],
) -> HashSet<(String, String)> {
    let mut deps = HashSet::new();

    for resource in resources {
        for dep in &resource.dependencies {
            if !dep.resource_id.is_empty() {
                deps.insert((dep.resource_type.clone(), dep.resource_id.clone()));
            }
        }
    }

    deps
}

/// Check if dependencies are satisfied within a resource set
#[allow(dead_code)]
pub fn validate_dependencies(
    resources: &[DiscoveredResource],
) -> Vec<(String, Vec<ResourceDependency>)> {
    let resource_set: HashSet<(String, String)> = resources
        .iter()
        .map(|r| (r.resource_type.clone(), r.resource_id.clone()))
        .collect();

    let mut missing = Vec::new();

    for resource in resources {
        let missing_deps: Vec<ResourceDependency> = resource
            .dependencies
            .iter()
            .filter(|dep| {
                !dep.resource_id.is_empty()
                    && !resource_set.contains(&(
                        dep.resource_type.clone(),
                        dep.resource_id.clone(),
                    ))
            })
            .cloned()
            .collect();

        if !missing_deps.is_empty() {
            missing.push((resource.display_string(), missing_deps));
        }
    }

    missing
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::discovery::{DependencyType, Provider};
    use crate::traits::MockOutput;

    fn sample_vpc() -> DiscoveredResource {
        DiscoveredResource::new(
            Provider::Aws,
            "aws_vpc".to_string(),
            "vpc-12345".to_string(),
        )
        .with_name("main")
    }

    fn sample_subnet() -> DiscoveredResource {
        DiscoveredResource::new(
            Provider::Aws,
            "aws_subnet".to_string(),
            "subnet-67890".to_string(),
        )
        .with_name("private")
        .with_dependency(ResourceDependency {
            resource_type: "aws_vpc".to_string(),
            resource_id: "vpc-12345".to_string(),
            relationship: DependencyType::Parent,
            description: None,
        })
    }

    #[test]
    fn test_order_by_dependencies() {
        let output = MockOutput::new();
        let workflow = ImportWorkflow::new(ImportWorkflowOptions::default(), &output);

        let resources = vec![sample_subnet(), sample_vpc()];
        let ordered = workflow.order_by_dependencies(&resources).unwrap();

        assert_eq!(ordered.len(), 2);
        assert_eq!(ordered[0].resource_type, "aws_vpc");
        assert_eq!(ordered[1].resource_type, "aws_subnet");
    }

    #[test]
    fn test_validate_dependencies_satisfied() {
        let resources = vec![sample_vpc(), sample_subnet()];
        let missing = validate_dependencies(&resources);

        assert!(missing.is_empty());
    }

    #[test]
    fn test_validate_dependencies_missing() {
        let resources = vec![sample_subnet()];
        let missing = validate_dependencies(&resources);

        assert_eq!(missing.len(), 1);
        assert!(missing[0].1[0].resource_type == "aws_vpc");
    }

    #[test]
    fn test_collect_all_dependencies() {
        let resources = vec![sample_subnet()];
        let deps = collect_all_dependencies(&resources);

        assert!(deps.contains(&("aws_vpc".to_string(), "vpc-12345".to_string())));
    }
}
