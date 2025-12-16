use crate::opa::provider::{ComplianceRef, PolicyInfo, PolicyMetadata, RemediationInfo};
use crate::traits::FileSystem;
use anyhow::Result;
use std::path::{Path, PathBuf};

/// Discovers OPA policies from multiple paths
pub struct PolicyDiscovery;

impl PolicyDiscovery {
    /// Discover all policy directories in order of priority
    /// Returns paths in order: local ./policies, global ~/.pmp/policies, custom paths
    pub fn discover_policy_paths(custom_paths: &[String]) -> Vec<PathBuf> {
        let mut paths = Vec::new();

        // Local policies (highest priority)
        let local_policies = PathBuf::from("./policies");
        if local_policies.exists() {
            paths.push(local_policies);
        }

        // Global policies
        if let Some(home) = dirs::home_dir() {
            let global_policies = home.join(".pmp").join("policies");
            if global_policies.exists() {
                paths.push(global_policies);
            }
        }

        // Custom paths from config (lowest priority)
        for custom in custom_paths {
            let path = PathBuf::from(custom);
            if path.exists() && !paths.contains(&path) {
                paths.push(path);
            }
        }

        paths
    }

    /// Discover all .rego files in a directory
    pub fn discover_rego_files(fs: &dyn FileSystem, dir: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        if !fs.exists(dir) || !fs.is_dir(dir) {
            return Ok(files);
        }

        let entries = fs.read_dir(dir)?;

        for entry in entries {
            if fs.is_file(&entry) && Self::is_rego_file(&entry) {
                files.push(entry);
            } else if fs.is_dir(&entry) {
                // Recurse into subdirectories
                let subfiles = Self::discover_rego_files(fs, &entry)?;
                files.extend(subfiles);
            }
        }

        Ok(files)
    }

    /// Check if a path is a .rego file
    fn is_rego_file(path: &Path) -> bool {
        path.extension()
            .map(|ext| ext == "rego")
            .unwrap_or(false)
    }

    /// Check if a file is a test file
    pub fn is_test_file(path: &Path) -> bool {
        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        file_name.contains("_test.rego") || file_name.starts_with("test_")
    }

    /// Parse policy metadata from file content (remediation, compliance annotations)
    pub fn parse_policy_metadata(content: &str) -> PolicyMetadata {
        let mut metadata = PolicyMetadata::default();
        let mut remediation_desc = None;
        let mut remediation_code = None;
        let mut remediation_url = None;
        let mut remediation_auto = false;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("# @description") {
                metadata.description = Some(
                    trimmed
                        .trim_start_matches("# @description")
                        .trim()
                        .to_string(),
                );
            } else if trimmed.starts_with("# @remediation-code") {
                remediation_code = Some(
                    trimmed
                        .trim_start_matches("# @remediation-code")
                        .trim()
                        .to_string(),
                );
            } else if trimmed.starts_with("# @remediation-url") {
                remediation_url = Some(
                    trimmed
                        .trim_start_matches("# @remediation-url")
                        .trim()
                        .to_string(),
                );
            } else if trimmed.starts_with("# @remediation-auto") {
                let value = trimmed
                    .trim_start_matches("# @remediation-auto")
                    .trim()
                    .to_lowercase();
                remediation_auto = value == "true" || value == "yes" || value == "1";
            } else if trimmed.starts_with("# @remediation") {
                remediation_desc = Some(
                    trimmed
                        .trim_start_matches("# @remediation")
                        .trim()
                        .to_string(),
                );
            } else if trimmed.starts_with("# @compliance") {
                if let Some(compliance_ref) = Self::parse_compliance_annotation(trimmed) {
                    metadata.compliance.push(compliance_ref);
                }
            }
        }

        if remediation_desc.is_some() || remediation_code.is_some() || remediation_url.is_some() {
            metadata.remediation = Some(RemediationInfo {
                description: remediation_desc.unwrap_or_default(),
                code_example: remediation_code,
                documentation_url: remediation_url,
                auto_fixable: remediation_auto,
            });
        }

        metadata
    }

    /// Parse a compliance annotation line
    /// Format: # @compliance FRAMEWORK:CONTROL_ID [Description]
    fn parse_compliance_annotation(line: &str) -> Option<ComplianceRef> {
        let content = line.trim_start_matches("# @compliance").trim();

        if content.is_empty() {
            return None;
        }

        let (framework_control, description) = Self::split_compliance_parts(content);
        let parts: Vec<&str> = framework_control.splitn(2, ':').collect();

        if parts.len() < 2 {
            return None;
        }

        Some(ComplianceRef {
            framework: parts[0].trim().to_string(),
            control_id: parts[1].trim().to_string(),
            description,
        })
    }

    /// Split compliance annotation into framework:control and description
    fn split_compliance_parts(content: &str) -> (&str, Option<String>) {
        if let Some(space_idx) = content.find(' ') {
            let framework_control = &content[..space_idx];

            if framework_control.contains(':') {
                let description = content[space_idx..].trim();
                let desc = if description.is_empty() {
                    None
                } else {
                    Some(description.to_string())
                };
                return (framework_control, desc);
            }
        }

        (content, None)
    }

    /// Parse policy info from file content
    pub fn parse_policy_info(path: &Path, content: &str) -> PolicyInfo {
        let mut package_name = String::new();
        let mut entrypoints = Vec::new();
        let metadata = Self::parse_policy_metadata(content);

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("package ") {
                package_name = format!("data.{}", trimmed.trim_start_matches("package ").trim());
            }

            Self::check_entrypoint(trimmed, "deny", &mut entrypoints);
            Self::check_entrypoint(trimmed, "warn", &mut entrypoints);
            Self::check_entrypoint(trimmed, "violation", &mut entrypoints);
            Self::check_entrypoint(trimmed, "info", &mut entrypoints);
        }

        PolicyInfo {
            path: path.to_string_lossy().to_string(),
            package_name,
            description: metadata.description,
            severity: None,
            entrypoints,
            remediation: metadata.remediation,
            compliance: metadata.compliance,
        }
    }

    /// Check and add entrypoint if line matches rule pattern
    fn check_entrypoint(line: &str, rule: &str, entrypoints: &mut Vec<String>) {
        let bracket_pattern = format!("{}[", rule);
        let equals_pattern = format!("{} =", rule);
        let assign_pattern = format!("{} :=", rule);

        if line.starts_with(&bracket_pattern)
            || line.starts_with(&equals_pattern)
            || line.starts_with(&assign_pattern)
        {
            if !entrypoints.contains(&rule.to_string()) {
                entrypoints.push(rule.to_string());
            }
        }
    }

    /// Load all policies from discovered paths into a provider
    pub fn load_all_policies<P: crate::opa::OpaProvider>(
        fs: &dyn FileSystem,
        provider: &mut P,
        custom_paths: &[String],
    ) -> Result<usize> {
        let paths = Self::discover_policy_paths(custom_paths);
        let mut total_loaded = 0;

        for dir in paths {
            let files = Self::discover_rego_files(fs, &dir)?;

            for file in files {
                if Self::is_test_file(&file) {
                    continue;
                }

                let content = fs.read_to_string(&file)?;
                let name = file
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                provider.load_policy_from_string(&name, &content)?;
                total_loaded += 1;
            }
        }

        Ok(total_loaded)
    }

    /// Get all policy info from discovered paths
    pub fn get_all_policy_info(
        fs: &dyn FileSystem,
        custom_paths: &[String],
    ) -> Result<Vec<PolicyInfo>> {
        let paths = Self::discover_policy_paths(custom_paths);
        let mut all_info = Vec::new();

        for dir in paths {
            let files = Self::discover_rego_files(fs, &dir)?;

            for file in files {
                if Self::is_test_file(&file) {
                    continue;
                }

                let content = fs.read_to_string(&file)?;
                let info = Self::parse_policy_info(&file, &content);
                all_info.push(info);
            }
        }

        Ok(all_info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::MockFileSystem;
    use std::sync::Arc;

    #[test]
    fn test_is_rego_file() {
        assert!(PolicyDiscovery::is_rego_file(Path::new("policy.rego")));
        assert!(PolicyDiscovery::is_rego_file(Path::new("/path/to/naming.rego")));
        assert!(!PolicyDiscovery::is_rego_file(Path::new("policy.json")));
        assert!(!PolicyDiscovery::is_rego_file(Path::new("policy")));
    }

    #[test]
    fn test_is_test_file() {
        assert!(PolicyDiscovery::is_test_file(Path::new("naming_test.rego")));
        assert!(PolicyDiscovery::is_test_file(Path::new("test_naming.rego")));
        assert!(!PolicyDiscovery::is_test_file(Path::new("naming.rego")));
    }

    #[test]
    fn test_parse_policy_info() {
        let content = r#"
            package pmp.naming

            # @description Enforce resource naming conventions

            deny[msg] {
                msg := "violation"
            }

            warn[msg] {
                msg := "warning"
            }
        "#;

        let info = PolicyDiscovery::parse_policy_info(Path::new("naming.rego"), content);

        assert_eq!(info.package_name, "data.pmp.naming");
        assert_eq!(
            info.description,
            Some("Enforce resource naming conventions".to_string())
        );
        assert!(info.entrypoints.contains(&"deny".to_string()));
        assert!(info.entrypoints.contains(&"warn".to_string()));
    }

    #[test]
    fn test_parse_policy_info_with_assign() {
        let content = r#"
            package pmp.tags

            deny := result {
                result := "must have tags"
            }
        "#;

        let info = PolicyDiscovery::parse_policy_info(Path::new("tags.rego"), content);
        assert!(info.entrypoints.contains(&"deny".to_string()));
    }

    #[test]
    fn test_discover_rego_files_with_mock_fs() {
        let fs = Arc::new(MockFileSystem::new());

        // Create mock directory structure
        fs.write(
            Path::new("/policies/naming.rego"),
            "package pmp.naming\ndeny[msg] { msg := \"test\" }",
        )
        .unwrap();
        fs.write(
            Path::new("/policies/tagging.rego"),
            "package pmp.tagging\nwarn[msg] { msg := \"test\" }",
        )
        .unwrap();
        fs.write(
            Path::new("/policies/naming_test.rego"),
            "package pmp.naming_test\ntest_something { true }",
        )
        .unwrap();

        let files = PolicyDiscovery::discover_rego_files(&*fs, Path::new("/policies")).unwrap();

        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_check_entrypoint() {
        let mut entrypoints = Vec::new();

        PolicyDiscovery::check_entrypoint("deny[msg] {", "deny", &mut entrypoints);
        assert!(entrypoints.contains(&"deny".to_string()));

        PolicyDiscovery::check_entrypoint("warn = true", "warn", &mut entrypoints);
        assert!(entrypoints.contains(&"warn".to_string()));

        PolicyDiscovery::check_entrypoint("violation := result", "violation", &mut entrypoints);
        assert!(entrypoints.contains(&"violation".to_string()));

        // Should not add duplicates
        PolicyDiscovery::check_entrypoint("deny[other] {", "deny", &mut entrypoints);
        assert_eq!(entrypoints.iter().filter(|&e| e == "deny").count(), 1);
    }

    #[test]
    fn test_get_all_policy_info_with_mock_fs() {
        let fs = Arc::new(MockFileSystem::new());

        // Create policy file in mock filesystem
        fs.write(
            Path::new("/test-policies/naming.rego"),
            "package pmp.naming\n# @description Naming policy\ndeny[msg] { msg := \"test\" }",
        )
        .unwrap();

        // Get policy info from a specific path that exists in mock fs
        let files = PolicyDiscovery::discover_rego_files(&*fs, Path::new("/test-policies")).unwrap();
        assert_eq!(files.len(), 1);

        let content = fs.read_to_string(&files[0]).unwrap();
        let info = PolicyDiscovery::parse_policy_info(&files[0], &content);

        assert_eq!(info.package_name, "data.pmp.naming");
        assert_eq!(info.description, Some("Naming policy".to_string()));
    }

    #[test]
    fn test_parse_remediation_annotation() {
        let content = r#"
            package pmp.test
            # @description Test policy
            # @remediation Fix by adding encryption
            # @remediation-code encrypted = true
            # @remediation-url https://docs.example.com/encryption
            # @remediation-auto true
            deny[msg] { msg := "test" }
        "#;

        let metadata = PolicyDiscovery::parse_policy_metadata(content);

        assert!(metadata.remediation.is_some());
        let remediation = metadata.remediation.unwrap();
        assert_eq!(remediation.description, "Fix by adding encryption");
        assert_eq!(remediation.code_example, Some("encrypted = true".to_string()));
        assert_eq!(
            remediation.documentation_url,
            Some("https://docs.example.com/encryption".to_string())
        );
        assert!(remediation.auto_fixable);
    }

    #[test]
    fn test_parse_remediation_partial() {
        let content = r#"
            package pmp.test
            # @remediation Add the required tag
            deny[msg] { msg := "test" }
        "#;

        let metadata = PolicyDiscovery::parse_policy_metadata(content);

        assert!(metadata.remediation.is_some());
        let remediation = metadata.remediation.unwrap();
        assert_eq!(remediation.description, "Add the required tag");
        assert!(remediation.code_example.is_none());
        assert!(remediation.documentation_url.is_none());
        assert!(!remediation.auto_fixable);
    }

    #[test]
    fn test_parse_compliance_annotation() {
        let content = r#"
            package pmp.test
            # @compliance CIS:2.2.1 Ensure EBS volume encryption is enabled
            # @compliance PCI-DSS:3.4 Render PAN unreadable anywhere it is stored
            deny[msg] { msg := "test" }
        "#;

        let metadata = PolicyDiscovery::parse_policy_metadata(content);

        assert_eq!(metadata.compliance.len(), 2);

        assert_eq!(metadata.compliance[0].framework, "CIS");
        assert_eq!(metadata.compliance[0].control_id, "2.2.1");
        assert_eq!(
            metadata.compliance[0].description,
            Some("Ensure EBS volume encryption is enabled".to_string())
        );

        assert_eq!(metadata.compliance[1].framework, "PCI-DSS");
        assert_eq!(metadata.compliance[1].control_id, "3.4");
        assert_eq!(
            metadata.compliance[1].description,
            Some("Render PAN unreadable anywhere it is stored".to_string())
        );
    }

    #[test]
    fn test_parse_compliance_without_description() {
        let content = r#"
            package pmp.test
            # @compliance HIPAA:164.312
            deny[msg] { msg := "test" }
        "#;

        let metadata = PolicyDiscovery::parse_policy_metadata(content);

        assert_eq!(metadata.compliance.len(), 1);
        assert_eq!(metadata.compliance[0].framework, "HIPAA");
        assert_eq!(metadata.compliance[0].control_id, "164.312");
        assert!(metadata.compliance[0].description.is_none());
    }

    #[test]
    fn test_parse_policy_info_with_remediation_and_compliance() {
        let content = r#"
            package pmp.security.encryption
            # @description Ensure EBS volumes are encrypted at rest
            # @remediation Add 'encrypted = true' to your aws_ebs_volume resource
            # @remediation-code encrypted = true
            # @compliance CIS:2.2.1 Ensure EBS volume encryption
            deny[msg] { msg := "not encrypted" }
        "#;

        let info = PolicyDiscovery::parse_policy_info(Path::new("encryption.rego"), content);

        assert_eq!(info.package_name, "data.pmp.security.encryption");
        assert_eq!(
            info.description,
            Some("Ensure EBS volumes are encrypted at rest".to_string())
        );
        assert!(info.remediation.is_some());
        assert_eq!(info.compliance.len(), 1);
        assert!(info.entrypoints.contains(&"deny".to_string()));
    }

    #[test]
    fn test_parse_compliance_annotation_invalid() {
        // Missing colon separator
        let result = PolicyDiscovery::parse_compliance_annotation("# @compliance CIS");
        assert!(result.is_none());

        // Empty content
        let result = PolicyDiscovery::parse_compliance_annotation("# @compliance");
        assert!(result.is_none());
    }
}
