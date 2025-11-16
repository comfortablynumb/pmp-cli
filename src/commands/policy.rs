use crate::collection::CollectionDiscovery;
use crate::context::Context;
use crate::output;
use crate::template::DynamicProjectEnvironmentResource;
use anyhow::{Context as AnyhowContext, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub struct PolicyCommand;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Policy {
    pub id: String,
    pub name: String,
    pub description: String,
    pub severity: PolicySeverity,
    pub category: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum PolicySeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug)]
pub struct PolicyViolation {
    pub policy: Policy,
    pub project: String,
    pub environment: String,
    pub message: String,
    pub details: Option<String>,
}

#[derive(Debug)]
pub struct ValidationReport {
    pub total_checks: usize,
    pub violations: Vec<PolicyViolation>,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
}

impl PolicyCommand {
    /// Execute the policy scan command for security scanning
    pub fn execute_scan(ctx: &Context, path: Option<&str>, scanner: Option<&str>) -> Result<()> {
        ctx.output.section("Security Scanning");

        let current_path = if let Some(p) = path {
            std::path::PathBuf::from(p)
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
        output::blank();

        let scanner_type = scanner.unwrap_or("tfsec");

        match scanner_type {
            "tfsec" => Self::run_tfsec(ctx, &current_path)?,
            "checkov" => Self::run_checkov(ctx, &current_path)?,
            "trivy" => Self::run_trivy(ctx, &current_path)?,
            _ => anyhow::bail!(
                "Unsupported scanner: {}. Use: tfsec, checkov, or trivy",
                scanner_type
            ),
        }

        Ok(())
    }

    /// Run tfsec scanner
    fn run_tfsec(ctx: &Context, env_path: &Path) -> Result<()> {
        ctx.output.info("Running tfsec security scanner...");
        output::blank();

        // Check if tfsec is installed
        let check = std::process::Command::new("tfsec")
            .arg("--version")
            .output();

        if check.is_err() {
            ctx.output.error("tfsec not found. Install it first:");
            ctx.output.dimmed("  brew install tfsec  (macOS)");
            ctx.output
                .dimmed("  or visit: https://github.com/aquasecurity/tfsec");
            anyhow::bail!("tfsec not installed");
        }

        // Run tfsec
        let result = std::process::Command::new("tfsec")
            .arg(env_path)
            .arg("--format")
            .arg("default")
            .output()?;

        let output_str = String::from_utf8_lossy(&result.stdout);

        if result.status.success() {
            ctx.output.success("✓ No security issues found");
        } else {
            ctx.output.error("Security issues detected:");
            output::blank();
            ctx.output.info(&output_str);
        }

        Ok(())
    }

    /// Run checkov scanner
    fn run_checkov(ctx: &Context, env_path: &Path) -> Result<()> {
        ctx.output.info("Running Checkov security scanner...");
        output::blank();

        // Check if checkov is installed
        let check = std::process::Command::new("checkov")
            .arg("--version")
            .output();

        if check.is_err() {
            ctx.output.error("checkov not found. Install it first:");
            ctx.output.dimmed("  pip install checkov");
            ctx.output.dimmed("  or visit: https://www.checkov.io/");
            anyhow::bail!("checkov not installed");
        }

        // Run checkov
        let result = std::process::Command::new("checkov")
            .arg("-d")
            .arg(env_path)
            .arg("--framework")
            .arg("terraform")
            .output()?;

        let output_str = String::from_utf8_lossy(&result.stdout);

        if result.status.success() {
            ctx.output.success("✓ No security issues found");
        } else {
            ctx.output.error("Security issues detected:");
            output::blank();
            ctx.output.info(&output_str);
        }

        Ok(())
    }

    /// Run trivy scanner
    fn run_trivy(ctx: &Context, env_path: &Path) -> Result<()> {
        ctx.output.info("Running Trivy security scanner...");
        output::blank();

        // Check if trivy is installed
        let check = std::process::Command::new("trivy")
            .arg("--version")
            .output();

        if check.is_err() {
            ctx.output.error("trivy not found. Install it first:");
            ctx.output.dimmed("  brew install trivy  (macOS)");
            ctx.output.dimmed("  or visit: https://trivy.dev/");
            anyhow::bail!("trivy not installed");
        }

        // Run trivy
        let result = std::process::Command::new("trivy")
            .arg("config")
            .arg(env_path)
            .output()?;

        let output_str = String::from_utf8_lossy(&result.stdout);

        if result.status.success() {
            ctx.output.success("✓ No security issues found");
        } else {
            ctx.output.error("Security issues detected:");
            output::blank();
            ctx.output.info(&output_str);
        }

        Ok(())
    }

    /// Execute the policy validate command
    pub fn execute_validate(
        ctx: &Context,
        path: Option<&str>,
        policy_filter: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Policy Validation");

        // Find infrastructure
        let (infrastructure, infrastructure_root) = CollectionDiscovery::find_collection(&*ctx.fs)?
            .context("Infrastructure is required. Run 'pmp init' first.")?;

        ctx.output
            .key_value("Infrastructure", &infrastructure.metadata.name);
        output::blank();

        // Load policies
        let policies = Self::load_builtin_policies();
        let active_policies: Vec<Policy> = if let Some(filter) = policy_filter {
            policies
                .into_iter()
                .filter(|p| p.id.contains(filter) || p.category.contains(filter))
                .collect()
        } else {
            policies
        };

        ctx.output.info(&format!(
            "Running {} policy check(s)",
            active_policies.len()
        ));
        output::blank();

        // Determine what to validate
        let current_path = if let Some(p) = path {
            std::path::PathBuf::from(p)
        } else {
            std::env::current_dir()?
        };

        let env_yaml = current_path.join(".pmp.environment.yaml");

        let report = if ctx.fs.exists(&env_yaml) {
            // Validate single environment
            Self::validate_single_environment(ctx, &current_path, &active_policies)?
        } else {
            // Validate all projects
            Self::validate_all_projects(ctx, &infrastructure_root, &active_policies)?
        };

        // Display results
        Self::display_validation_report(ctx, &report)?;

        // Exit with error if there are policy errors
        if report.errors > 0 {
            anyhow::bail!("Policy validation failed with {} error(s)", report.errors);
        }

        Ok(())
    }

    /// Load built-in policies
    fn load_builtin_policies() -> Vec<Policy> {
        vec![
            // Naming convention policies
            Policy {
                id: "naming-001".to_string(),
                name: "Project Name Format".to_string(),
                description: "Project names must be lowercase alphanumeric with underscores"
                    .to_string(),
                severity: PolicySeverity::Error,
                category: "naming".to_string(),
            },
            Policy {
                id: "naming-002".to_string(),
                name: "Environment Name Format".to_string(),
                description: "Environment names must be lowercase alphanumeric".to_string(),
                severity: PolicySeverity::Error,
                category: "naming".to_string(),
            },
            // Tagging policies
            Policy {
                id: "tagging-001".to_string(),
                name: "Required Tags".to_string(),
                description: "Projects must have required tags: owner, cost-center".to_string(),
                severity: PolicySeverity::Warning,
                category: "tagging".to_string(),
            },
            // Security policies
            Policy {
                id: "security-001".to_string(),
                name: "No Hardcoded Secrets".to_string(),
                description: "Projects must not contain hardcoded secrets or API keys".to_string(),
                severity: PolicySeverity::Error,
                category: "security".to_string(),
            },
            Policy {
                id: "security-002".to_string(),
                name: "Encryption at Rest".to_string(),
                description: "State backend must use encryption".to_string(),
                severity: PolicySeverity::Error,
                category: "security".to_string(),
            },
            // Dependency policies
            Policy {
                id: "deps-001".to_string(),
                name: "No Circular Dependencies".to_string(),
                description: "Projects must not have circular dependencies".to_string(),
                severity: PolicySeverity::Error,
                category: "dependencies".to_string(),
            },
            Policy {
                id: "deps-002".to_string(),
                name: "Valid Dependencies".to_string(),
                description: "All dependencies must reference existing projects".to_string(),
                severity: PolicySeverity::Error,
                category: "dependencies".to_string(),
            },
            // Best practices
            Policy {
                id: "best-practice-001".to_string(),
                name: "Resource Documentation".to_string(),
                description: "Projects should have a README.md file".to_string(),
                severity: PolicySeverity::Info,
                category: "best-practice".to_string(),
            },
        ]
    }

    /// Validate a single environment
    fn validate_single_environment(
        ctx: &Context,
        env_path: &Path,
        policies: &[Policy],
    ) -> Result<ValidationReport> {
        let env_yaml = env_path.join(".pmp.environment.yaml");
        let resource = DynamicProjectEnvironmentResource::from_file(&*ctx.fs, &env_yaml)?;

        let mut violations = Vec::new();

        for policy in policies {
            if let Some(violation) = Self::check_policy(ctx, &resource, env_path, policy)? {
                violations.push(violation);
            }
        }

        let errors = violations
            .iter()
            .filter(|v| v.policy.severity == PolicySeverity::Error)
            .count();
        let warnings = violations
            .iter()
            .filter(|v| v.policy.severity == PolicySeverity::Warning)
            .count();
        let infos = violations
            .iter()
            .filter(|v| v.policy.severity == PolicySeverity::Info)
            .count();

        Ok(ValidationReport {
            total_checks: policies.len(),
            violations,
            errors,
            warnings,
            infos,
        })
    }

    /// Validate all projects
    fn validate_all_projects(
        ctx: &Context,
        infrastructure_root: &Path,
        policies: &[Policy],
    ) -> Result<ValidationReport> {
        let projects =
            CollectionDiscovery::discover_projects(&*ctx.fs, &*ctx.output, infrastructure_root)?;

        let mut all_violations = Vec::new();

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
                        for policy in policies {
                            if let Ok(Some(violation)) =
                                Self::check_policy(ctx, &resource, &env_path, policy)
                            {
                                all_violations.push(violation);
                            }
                        }
                    }
                }
            }
        }

        let errors = all_violations
            .iter()
            .filter(|v| v.policy.severity == PolicySeverity::Error)
            .count();
        let warnings = all_violations
            .iter()
            .filter(|v| v.policy.severity == PolicySeverity::Warning)
            .count();
        let infos = all_violations
            .iter()
            .filter(|v| v.policy.severity == PolicySeverity::Info)
            .count();

        Ok(ValidationReport {
            total_checks: policies.len() * projects.len(),
            violations: all_violations,
            errors,
            warnings,
            infos,
        })
    }

    /// Check a single policy against a resource
    fn check_policy(
        ctx: &Context,
        resource: &DynamicProjectEnvironmentResource,
        env_path: &Path,
        policy: &Policy,
    ) -> Result<Option<PolicyViolation>> {
        match policy.id.as_str() {
            "naming-001" => Self::check_project_name_format(resource, policy),
            "naming-002" => Self::check_environment_name_format(resource, policy),
            "tagging-001" => Self::check_required_tags(resource, policy),
            "security-001" => Self::check_no_hardcoded_secrets(ctx, env_path, resource, policy),
            "security-002" => Self::check_encryption_at_rest(ctx, env_path, resource, policy),
            "deps-001" => Self::check_no_circular_deps(resource, policy),
            "deps-002" => Self::check_valid_dependencies(resource, policy),
            "best-practice-001" => Self::check_documentation(ctx, env_path, resource, policy),
            _ => Ok(None),
        }
    }

    /// Check project name format
    fn check_project_name_format(
        resource: &DynamicProjectEnvironmentResource,
        policy: &Policy,
    ) -> Result<Option<PolicyViolation>> {
        let name = &resource.metadata.name;
        let valid = name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');

        if !valid {
            Ok(Some(PolicyViolation {
                policy: policy.clone(),
                project: name.clone(),
                environment: resource.metadata.environment_name.clone(),
                message: format!("Project name '{}' does not follow naming convention", name),
                details: Some(
                    "Use lowercase alphanumeric characters and underscores only".to_string(),
                ),
            }))
        } else {
            Ok(None)
        }
    }

    /// Check environment name format
    fn check_environment_name_format(
        resource: &DynamicProjectEnvironmentResource,
        policy: &Policy,
    ) -> Result<Option<PolicyViolation>> {
        let env_name = &resource.metadata.environment_name;
        let valid = env_name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');

        if !valid {
            Ok(Some(PolicyViolation {
                policy: policy.clone(),
                project: resource.metadata.name.clone(),
                environment: env_name.clone(),
                message: format!(
                    "Environment name '{}' does not follow naming convention",
                    env_name
                ),
                details: Some(
                    "Use lowercase alphanumeric characters and underscores only".to_string(),
                ),
            }))
        } else {
            Ok(None)
        }
    }

    /// Check required tags
    fn check_required_tags(
        resource: &DynamicProjectEnvironmentResource,
        policy: &Policy,
    ) -> Result<Option<PolicyViolation>> {
        let required_tags = vec!["owner", "cost_center"];
        let mut missing_tags = Vec::new();

        for tag in required_tags {
            if !resource.spec.inputs.contains_key(tag) {
                missing_tags.push(tag);
            }
        }

        if !missing_tags.is_empty() {
            Ok(Some(PolicyViolation {
                policy: policy.clone(),
                project: resource.metadata.name.clone(),
                environment: resource.metadata.environment_name.clone(),
                message: format!("Missing required tags: {}", missing_tags.join(", ")),
                details: Some("Add these tags to the project inputs".to_string()),
            }))
        } else {
            Ok(None)
        }
    }

    /// Check for hardcoded secrets
    fn check_no_hardcoded_secrets(
        ctx: &Context,
        env_path: &Path,
        resource: &DynamicProjectEnvironmentResource,
        policy: &Policy,
    ) -> Result<Option<PolicyViolation>> {
        // Scan files for common secret patterns
        let secret_patterns = vec![
            "password",
            "secret",
            "api_key",
            "apikey",
            "token",
            "private_key",
        ];

        let mut found_secrets = Vec::new();

        // Read all .tf files in the environment
        if let Ok(entries) = ctx.fs.read_dir(env_path) {
            for entry in entries {
                if let Some(ext) = entry.extension()
                    && ext == "tf"
                    && let Ok(content) = ctx.fs.read_to_string(&entry)
                {
                    let content_lower = content.to_lowercase();
                    for pattern in &secret_patterns {
                        if content_lower.contains(&format!("\"{}\"", pattern))
                            || content_lower.contains(&format!("{} =", pattern))
                        {
                            found_secrets.push(pattern.to_string());
                        }
                    }
                }
            }
        }

        if !found_secrets.is_empty() {
            Ok(Some(PolicyViolation {
                policy: policy.clone(),
                project: resource.metadata.name.clone(),
                environment: resource.metadata.environment_name.clone(),
                message: "Potential hardcoded secrets detected".to_string(),
                details: Some(format!(
                    "Found patterns: {}. Use variables or secret management instead.",
                    found_secrets.join(", ")
                )),
            }))
        } else {
            Ok(None)
        }
    }

    /// Check encryption at rest
    fn check_encryption_at_rest(
        ctx: &Context,
        env_path: &Path,
        resource: &DynamicProjectEnvironmentResource,
        policy: &Policy,
    ) -> Result<Option<PolicyViolation>> {
        // Check if backend configuration has encryption enabled
        let common_tf = env_path.join("_common.tf");

        if ctx.fs.exists(&common_tf)
            && let Ok(content) = ctx.fs.read_to_string(&common_tf)
        {
            let has_encryption = content.contains("encrypt")
                || content.contains("encryption")
                || content.contains("kms_key");

            if !has_encryption {
                return Ok(Some(PolicyViolation {
                    policy: policy.clone(),
                    project: resource.metadata.name.clone(),
                    environment: resource.metadata.environment_name.clone(),
                    message: "State backend encryption not configured".to_string(),
                    details: Some("Enable encryption in backend configuration".to_string()),
                }));
            }
        }

        Ok(None)
    }

    /// Check for circular dependencies (simplified)
    fn check_no_circular_deps(
        resource: &DynamicProjectEnvironmentResource,
        policy: &Policy,
    ) -> Result<Option<PolicyViolation>> {
        // This is a simplified check - real implementation would need full graph
        // Just check if project depends on itself
        for dep in &resource.spec.dependencies {
            if dep.project.name == resource.metadata.name {
                return Ok(Some(PolicyViolation {
                    policy: policy.clone(),
                    project: resource.metadata.name.clone(),
                    environment: resource.metadata.environment_name.clone(),
                    message: "Project depends on itself".to_string(),
                    details: Some("Remove self-reference from dependencies".to_string()),
                }));
            }
        }

        Ok(None)
    }

    /// Check valid dependencies
    fn check_valid_dependencies(
        resource: &DynamicProjectEnvironmentResource,
        policy: &Policy,
    ) -> Result<Option<PolicyViolation>> {
        // This is a placeholder - real implementation would check against actual projects
        // For now, just check that dependencies have valid structure
        for dep in &resource.spec.dependencies {
            if dep.project.name.is_empty() || dep.project.environments.is_empty() {
                return Ok(Some(PolicyViolation {
                    policy: policy.clone(),
                    project: resource.metadata.name.clone(),
                    environment: resource.metadata.environment_name.clone(),
                    message: "Invalid dependency structure".to_string(),
                    details: Some(
                        "Dependencies must have valid project name and environments".to_string(),
                    ),
                }));
            }
        }

        Ok(None)
    }

    /// Check for documentation
    fn check_documentation(
        ctx: &Context,
        env_path: &Path,
        resource: &DynamicProjectEnvironmentResource,
        policy: &Policy,
    ) -> Result<Option<PolicyViolation>> {
        let project_root = env_path.parent().and_then(|p| p.parent());

        if let Some(root) = project_root {
            let readme = root.join("README.md");
            if !ctx.fs.exists(&readme) {
                return Ok(Some(PolicyViolation {
                    policy: policy.clone(),
                    project: resource.metadata.name.clone(),
                    environment: resource.metadata.environment_name.clone(),
                    message: "Missing README.md documentation".to_string(),
                    details: Some("Add a README.md file to document the project".to_string()),
                }));
            }
        }

        Ok(None)
    }

    /// Display validation report
    fn display_validation_report(ctx: &Context, report: &ValidationReport) -> Result<()> {
        ctx.output.subsection("Validation Summary");
        ctx.output
            .key_value("Total Checks", &report.total_checks.to_string());
        ctx.output
            .key_value("Violations", &report.violations.len().to_string());
        ctx.output.key_value("Errors", &report.errors.to_string());
        ctx.output
            .key_value("Warnings", &report.warnings.to_string());
        ctx.output.key_value("Info", &report.infos.to_string());
        output::blank();

        if report.violations.is_empty() {
            ctx.output
                .success("✓ All policy checks passed. No violations found.");
            return Ok(());
        }

        // Group violations by severity
        let errors: Vec<_> = report
            .violations
            .iter()
            .filter(|v| v.policy.severity == PolicySeverity::Error)
            .collect();

        let warnings: Vec<_> = report
            .violations
            .iter()
            .filter(|v| v.policy.severity == PolicySeverity::Warning)
            .collect();

        let infos: Vec<_> = report
            .violations
            .iter()
            .filter(|v| v.policy.severity == PolicySeverity::Info)
            .collect();

        // Display errors
        if !errors.is_empty() {
            ctx.output.subsection("Errors");
            for violation in errors {
                Self::display_violation(ctx, violation);
            }
            output::blank();
        }

        // Display warnings
        if !warnings.is_empty() {
            ctx.output.subsection("Warnings");
            for violation in warnings {
                Self::display_violation(ctx, violation);
            }
            output::blank();
        }

        // Display info
        if !infos.is_empty() {
            ctx.output.subsection("Info");
            for violation in infos {
                Self::display_violation(ctx, violation);
            }
            output::blank();
        }

        Ok(())
    }

    /// Display a single violation
    fn display_violation(ctx: &Context, violation: &PolicyViolation) {
        let severity_symbol = match violation.policy.severity {
            PolicySeverity::Error => "✗",
            PolicySeverity::Warning => "⚠",
            PolicySeverity::Info => "ℹ",
        };

        ctx.output.dimmed(&format!(
            "{} [{}] {}:{}",
            severity_symbol, violation.policy.id, violation.project, violation.environment
        ));
        ctx.output.dimmed(&format!("  {}", violation.message));
        if let Some(details) = &violation.details {
            ctx.output.dimmed(&format!("  → {}", details));
        }
        output::blank();
    }
}
