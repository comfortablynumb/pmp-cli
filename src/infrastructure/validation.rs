//! Import validation module
//!
//! Provides pre-import validation to catch issues before writing files.

use std::collections::HashSet;
use std::path::Path;

use super::discovery::DiscoveredResource;

/// Validation report containing errors and warnings
#[derive(Debug, Default)]
pub struct ValidationReport {
    /// Errors that prevent the import from proceeding
    pub errors: Vec<ValidationError>,
    /// Warnings that don't block the import but should be noted
    pub warnings: Vec<ValidationWarning>,
}

/// Validation errors that prevent import
#[derive(Debug)]
#[allow(dead_code)]
pub enum ValidationError {
    /// Multiple resources have the same Terraform name
    DuplicateResourceName {
        name: String,
        resource_ids: Vec<String>,
    },
    /// Resource type is not recognized
    InvalidResourceType {
        resource_type: String,
        resource_id: String,
    },
    /// No resources to import
    EmptyResourceList,
}

/// Validation warnings that don't block import
#[derive(Debug)]
pub enum ValidationWarning {
    /// State file already exists at destination
    ExistingStateFile { path: String },
    /// A resource dependency is not included in the import
    MissingDependency {
        resource: String,
        dependency_type: String,
        dependency_id: String,
    },
    /// Directory already exists
    ExistingDirectory { path: String },
}

#[allow(dead_code)]
impl ValidationReport {
    /// Check if the validation passed (no errors)
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    /// Get the total number of issues (errors + warnings)
    pub fn issue_count(&self) -> usize {
        self.errors.len() + self.warnings.len()
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::DuplicateResourceName { name, resource_ids } => {
                write!(
                    f,
                    "Duplicate resource name '{}' for resources: {}",
                    name,
                    resource_ids.join(", ")
                )
            }
            ValidationError::InvalidResourceType {
                resource_type,
                resource_id,
            } => {
                write!(
                    f,
                    "Invalid resource type '{}' for resource '{}'",
                    resource_type, resource_id
                )
            }
            ValidationError::EmptyResourceList => {
                write!(f, "No resources to import")
            }
        }
    }
}

impl std::fmt::Display for ValidationWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationWarning::ExistingStateFile { path } => {
                write!(f, "State file already exists: {}", path)
            }
            ValidationWarning::MissingDependency {
                resource,
                dependency_type,
                dependency_id,
            } => {
                write!(
                    f,
                    "Resource '{}' depends on {} '{}' which is not in the import list",
                    resource, dependency_type, dependency_id
                )
            }
            ValidationWarning::ExistingDirectory { path } => {
                write!(f, "Directory already exists: {}", path)
            }
        }
    }
}

/// Validate resources before import
///
/// Checks for:
/// - Duplicate resource names
/// - Empty resource list
/// - Missing dependencies (warning)
/// - Existing state files (warning)
pub fn validate_import(
    resources: &[DiscoveredResource],
    project_path: &Path,
) -> ValidationReport {
    let mut report = ValidationReport::default();

    // Check for empty resource list
    if resources.is_empty() {
        report.errors.push(ValidationError::EmptyResourceList);
        return report;
    }

    // Check for duplicate Terraform names
    let mut names_to_ids: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    for resource in resources {
        let tf_name = resource.suggested_tf_name();
        names_to_ids
            .entry(tf_name)
            .or_default()
            .push(resource.resource_id.clone());
    }

    for (name, ids) in names_to_ids {
        if ids.len() > 1 {
            report.errors.push(ValidationError::DuplicateResourceName {
                name,
                resource_ids: ids,
            });
        }
    }

    // Collect all resource IDs for dependency checking
    let resource_ids: HashSet<&str> = resources
        .iter()
        .map(|r| r.resource_id.as_str())
        .collect();

    // Check for missing dependencies
    for resource in resources {
        for dep in &resource.dependencies {
            if !resource_ids.contains(dep.resource_id.as_str()) {
                report.warnings.push(ValidationWarning::MissingDependency {
                    resource: resource.resource_id.clone(),
                    dependency_type: dep.resource_type.clone(),
                    dependency_id: dep.resource_id.clone(),
                });
            }
        }
    }

    // Check for existing state file
    let state_file = project_path.join("terraform.tfstate");
    if state_file.exists() {
        report.warnings.push(ValidationWarning::ExistingStateFile {
            path: state_file.display().to_string(),
        });
    }

    // Check for existing directory (for new projects)
    if project_path.exists() {
        report.warnings.push(ValidationWarning::ExistingDirectory {
            path: project_path.display().to_string(),
        });
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::discovery::{DependencyType, Provider, ResourceDependency};
    use std::path::Path;

    // Use a path that doesn't exist for testing
    fn test_path() -> &'static Path {
        Path::new("/nonexistent/test/path/that/should/not/exist")
    }

    #[test]
    fn test_validate_empty_resources() {
        let report = validate_import(&[], test_path());

        assert!(!report.is_valid());
        assert_eq!(report.errors.len(), 1);
        assert!(matches!(
            report.errors[0],
            ValidationError::EmptyResourceList
        ));
    }

    #[test]
    fn test_validate_duplicate_names() {
        let resources = vec![
            DiscoveredResource::new(Provider::Aws, "aws_vpc".to_string(), "vpc-123".to_string())
                .with_name("main"),
            DiscoveredResource::new(Provider::Aws, "aws_vpc".to_string(), "vpc-456".to_string())
                .with_name("main"),
        ];

        let report = validate_import(&resources, test_path());

        assert!(!report.is_valid());
        assert_eq!(report.errors.len(), 1);
        assert!(matches!(
            &report.errors[0],
            ValidationError::DuplicateResourceName { name, .. } if name == "main"
        ));
    }

    #[test]
    fn test_validate_missing_dependency() {
        let resources = vec![DiscoveredResource::new(
            Provider::Aws,
            "aws_subnet".to_string(),
            "subnet-123".to_string(),
        )
        .with_dependency(ResourceDependency {
            resource_type: "aws_vpc".to_string(),
            resource_id: "vpc-missing".to_string(),
            relationship: DependencyType::Parent,
            description: None,
        })];

        let report = validate_import(&resources, test_path());

        // Should be valid but with warnings
        assert!(report.is_valid());
        // Only 1 warning for missing dependency (path doesn't exist)
        assert_eq!(report.warnings.len(), 1);
        assert!(report.warnings.iter().any(|w| matches!(
            w,
            ValidationWarning::MissingDependency { dependency_id, .. }
            if dependency_id == "vpc-missing"
        )));
    }

    #[test]
    fn test_validate_success() {
        let resources = vec![
            DiscoveredResource::new(
                Provider::Aws,
                "aws_vpc".to_string(),
                "vpc-123".to_string(),
            )
            .with_name("main-vpc"),
            DiscoveredResource::new(
                Provider::Aws,
                "aws_subnet".to_string(),
                "subnet-456".to_string(),
            )
            .with_name("main-subnet"),
        ];

        let report = validate_import(&resources, test_path());

        assert!(report.is_valid());
        assert!(report.warnings.is_empty());
    }
}
