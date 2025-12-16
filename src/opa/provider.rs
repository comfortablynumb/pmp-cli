use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Severity levels for OPA policy violations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpaSeverity {
    Error,
    Warning,
    Info,
}

/// Remediation information for a policy violation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RemediationInfo {
    pub description: String,
    pub code_example: Option<String>,
    pub documentation_url: Option<String>,
    pub auto_fixable: bool,
}

/// Reference to a compliance framework control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceRef {
    pub framework: String,
    pub control_id: String,
    pub description: Option<String>,
}

impl std::fmt::Display for OpaSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OpaSeverity::Error => write!(f, "error"),
            OpaSeverity::Warning => write!(f, "warning"),
            OpaSeverity::Info => write!(f, "info"),
        }
    }
}

/// A single policy violation from OPA evaluation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpaViolation {
    pub rule: String,
    pub message: String,
    pub severity: OpaSeverity,
    pub resource: Option<String>,
    pub details: Option<serde_json::Value>,
    pub remediation: Option<RemediationInfo>,
    pub compliance: Vec<ComplianceRef>,
}

/// Result from evaluating a single policy file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyEvaluation {
    pub policy_path: String,
    pub policy_name: String,
    pub package_name: String,
    pub passed: bool,
    pub violations: Vec<OpaViolation>,
    pub warnings: Vec<String>,
}

/// Summary of policy validation across all policies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub total_policies: usize,
    pub passed_policies: usize,
    pub failed_policies: usize,
    pub total_violations: usize,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub evaluations: Vec<PolicyEvaluation>,
}

impl ValidationSummary {
    pub fn new() -> Self {
        Self {
            total_policies: 0,
            passed_policies: 0,
            failed_policies: 0,
            total_violations: 0,
            errors: 0,
            warnings: 0,
            infos: 0,
            evaluations: Vec::new(),
        }
    }

    pub fn add_evaluation(&mut self, eval: PolicyEvaluation) {
        self.total_policies += 1;

        if eval.passed {
            self.passed_policies += 1;
        } else {
            self.failed_policies += 1;
        }

        for violation in &eval.violations {
            self.total_violations += 1;
            match violation.severity {
                OpaSeverity::Error => self.errors += 1,
                OpaSeverity::Warning => self.warnings += 1,
                OpaSeverity::Info => self.infos += 1,
            }
        }

        self.evaluations.push(eval);
    }
}

impl Default for ValidationSummary {
    fn default() -> Self {
        Self::new()
    }
}

/// Test result for a single policy test file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyTestResult {
    pub test_file: String,
    pub passed: usize,
    pub failed: usize,
    pub test_cases: Vec<TestCaseResult>,
}

/// Result of a single test case
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseResult {
    pub name: String,
    pub passed: bool,
    pub message: Option<String>,
}

/// Metadata parsed from Rego policy comments
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PolicyMetadata {
    pub description: Option<String>,
    pub remediation: Option<RemediationInfo>,
    pub compliance: Vec<ComplianceRef>,
}

/// Policy metadata discovered from Rego files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyInfo {
    pub path: String,
    pub package_name: String,
    pub description: Option<String>,
    pub severity: Option<OpaSeverity>,
    pub entrypoints: Vec<String>,
    pub remediation: Option<RemediationInfo>,
    pub compliance: Vec<ComplianceRef>,
}

/// Parameters for validation (groups params to stay under 4 limit)
pub struct ValidationParams<'a> {
    pub input: &'a serde_json::Value,
    pub policy_filter: Option<&'a str>,
    pub entrypoint: &'a str,
}

/// Trait for OPA policy providers (enables testing and future alternatives)
pub trait OpaProvider: Send + Sync {
    /// Get the name of this provider
    fn get_name(&self) -> &str;

    /// Validate input against loaded policies
    fn validate(&self, params: &ValidationParams) -> Result<ValidationSummary>;

    /// Run policy tests with fixtures
    fn test_policies(&self, test_dir: &Path) -> Result<Vec<PolicyTestResult>>;

    /// List all loaded policies
    fn list_policies(&self) -> Vec<PolicyInfo>;

    /// Load policies from a directory path
    fn load_policies(&mut self, path: &Path) -> Result<usize>;

    /// Load a single policy from string content
    fn load_policy_from_string(&mut self, name: &str, content: &str) -> Result<()>;

    /// Set data document for evaluation
    fn set_data(&mut self, data: serde_json::Value) -> Result<()>;

    /// Clear all loaded policies
    fn clear(&mut self);
}

/// Mock OPA provider for testing
#[cfg(test)]
pub struct MockOpaProvider {
    policies: Vec<PolicyInfo>,
    validation_result: Option<ValidationSummary>,
    test_results: Vec<PolicyTestResult>,
}

#[cfg(test)]
impl MockOpaProvider {
    pub fn new() -> Self {
        Self {
            policies: Vec::new(),
            validation_result: None,
            test_results: Vec::new(),
        }
    }

    pub fn with_policies(policies: Vec<PolicyInfo>) -> Self {
        Self {
            policies,
            validation_result: None,
            test_results: Vec::new(),
        }
    }

    pub fn with_validation_result(mut self, result: ValidationSummary) -> Self {
        self.validation_result = Some(result);
        self
    }

    pub fn with_test_results(mut self, results: Vec<PolicyTestResult>) -> Self {
        self.test_results = results;
        self
    }
}

#[cfg(test)]
impl Default for MockOpaProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl OpaProvider for MockOpaProvider {
    fn get_name(&self) -> &str {
        "mock"
    }

    fn validate(&self, _params: &ValidationParams) -> Result<ValidationSummary> {
        Ok(self
            .validation_result
            .clone()
            .unwrap_or_else(ValidationSummary::new))
    }

    fn test_policies(&self, _test_dir: &Path) -> Result<Vec<PolicyTestResult>> {
        Ok(self.test_results.clone())
    }

    fn list_policies(&self) -> Vec<PolicyInfo> {
        self.policies.clone()
    }

    fn load_policies(&mut self, _path: &Path) -> Result<usize> {
        Ok(self.policies.len())
    }

    fn load_policy_from_string(&mut self, name: &str, _content: &str) -> Result<()> {
        self.policies.push(PolicyInfo {
            path: name.to_string(),
            package_name: format!("data.{}", name),
            description: None,
            severity: None,
            entrypoints: vec!["deny".to_string()],
            remediation: None,
            compliance: Vec::new(),
        });
        Ok(())
    }

    fn set_data(&mut self, _data: serde_json::Value) -> Result<()> {
        Ok(())
    }

    fn clear(&mut self) {
        self.policies.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_summary_new() {
        let summary = ValidationSummary::new();
        assert_eq!(summary.total_policies, 0);
        assert_eq!(summary.passed_policies, 0);
        assert_eq!(summary.failed_policies, 0);
        assert_eq!(summary.total_violations, 0);
    }

    #[test]
    fn test_validation_summary_add_passed_evaluation() {
        let mut summary = ValidationSummary::new();
        let eval = PolicyEvaluation {
            policy_path: "test.rego".to_string(),
            policy_name: "test".to_string(),
            package_name: "data.test".to_string(),
            passed: true,
            violations: Vec::new(),
            warnings: Vec::new(),
        };

        summary.add_evaluation(eval);

        assert_eq!(summary.total_policies, 1);
        assert_eq!(summary.passed_policies, 1);
        assert_eq!(summary.failed_policies, 0);
    }

    #[test]
    fn test_validation_summary_add_failed_evaluation() {
        let mut summary = ValidationSummary::new();
        let eval = PolicyEvaluation {
            policy_path: "test.rego".to_string(),
            policy_name: "test".to_string(),
            package_name: "data.test".to_string(),
            passed: false,
            violations: vec![
                OpaViolation {
                    rule: "deny".to_string(),
                    message: "error 1".to_string(),
                    severity: OpaSeverity::Error,
                    resource: None,
                    details: None,
                    remediation: None,
                    compliance: Vec::new(),
                },
                OpaViolation {
                    rule: "warn".to_string(),
                    message: "warning 1".to_string(),
                    severity: OpaSeverity::Warning,
                    resource: None,
                    details: None,
                    remediation: None,
                    compliance: Vec::new(),
                },
            ],
            warnings: Vec::new(),
        };

        summary.add_evaluation(eval);

        assert_eq!(summary.total_policies, 1);
        assert_eq!(summary.passed_policies, 0);
        assert_eq!(summary.failed_policies, 1);
        assert_eq!(summary.total_violations, 2);
        assert_eq!(summary.errors, 1);
        assert_eq!(summary.warnings, 1);
    }

    #[test]
    fn test_opa_severity_display() {
        assert_eq!(format!("{}", OpaSeverity::Error), "error");
        assert_eq!(format!("{}", OpaSeverity::Warning), "warning");
        assert_eq!(format!("{}", OpaSeverity::Info), "info");
    }

    #[test]
    fn test_mock_provider_validate() {
        let provider = MockOpaProvider::new();
        let input = serde_json::json!({});
        let params = ValidationParams {
            input: &input,
            policy_filter: None,
            entrypoint: "data.pmp",
        };

        let result = provider.validate(&params).unwrap();
        assert_eq!(result.total_policies, 0);
    }

    #[test]
    fn test_mock_provider_with_validation_result() {
        let mut expected = ValidationSummary::new();
        expected.total_policies = 5;
        expected.errors = 2;

        let provider = MockOpaProvider::new().with_validation_result(expected);
        let input = serde_json::json!({});
        let params = ValidationParams {
            input: &input,
            policy_filter: None,
            entrypoint: "data.pmp",
        };

        let result = provider.validate(&params).unwrap();
        assert_eq!(result.total_policies, 5);
        assert_eq!(result.errors, 2);
    }

    #[test]
    fn test_mock_provider_load_policy_from_string() {
        let mut provider = MockOpaProvider::new();
        provider
            .load_policy_from_string("naming", "package naming")
            .unwrap();

        let policies = provider.list_policies();
        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].path, "naming");
    }
}
