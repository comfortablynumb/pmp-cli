use crate::opa::provider::{ComplianceRef, OpaSeverity, RemediationInfo, ValidationSummary};
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Summary statistics for a compliance report
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ComplianceSummary {
    pub total_checks: usize,
    pub passed: usize,
    pub failed: usize,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub compliance_score: f64,
}

/// A single violation in the compliance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceViolation {
    pub policy: String,
    pub rule: String,
    pub severity: OpaSeverity,
    pub message: String,
    pub resource: Option<String>,
    pub remediation: Option<RemediationInfo>,
    pub compliance: Vec<ComplianceRef>,
}

/// Status of a compliance control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlStatus {
    pub control_id: String,
    pub description: Option<String>,
    pub passed: bool,
    pub violations: Vec<String>,
}

/// Summary for a compliance framework
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FrameworkSummary {
    pub framework: String,
    pub total_controls: usize,
    pub passed: usize,
    pub failed: usize,
    pub controls: Vec<ControlStatus>,
}

/// Context information for report generation
pub struct ReportContext {
    pub infrastructure: String,
    pub project: Option<String>,
    pub environment: Option<String>,
}

/// Complete compliance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceReport {
    pub timestamp: String,
    pub infrastructure: String,
    pub project: Option<String>,
    pub environment: Option<String>,
    pub summary: ComplianceSummary,
    pub violations: Vec<ComplianceViolation>,
    pub by_framework: HashMap<String, FrameworkSummary>,
}

/// Compliance report generator
pub struct ComplianceReporter;

impl ComplianceReporter {
    /// Generate a compliance report from validation summary
    pub fn generate_report(
        summary: &ValidationSummary,
        context: &ReportContext,
    ) -> Result<ComplianceReport> {
        let violations = Self::extract_violations(summary);
        let compliance_summary = Self::build_summary(summary, &violations);
        let by_framework = Self::group_by_framework(&violations);

        Ok(ComplianceReport {
            timestamp: Utc::now().to_rfc3339(),
            infrastructure: context.infrastructure.clone(),
            project: context.project.clone(),
            environment: context.environment.clone(),
            summary: compliance_summary,
            violations,
            by_framework,
        })
    }

    /// Extract violations from validation summary
    fn extract_violations(summary: &ValidationSummary) -> Vec<ComplianceViolation> {
        let mut violations = Vec::new();

        for eval in &summary.evaluations {
            for v in &eval.violations {
                violations.push(ComplianceViolation {
                    policy: eval.package_name.clone(),
                    rule: v.rule.clone(),
                    severity: v.severity.clone(),
                    message: v.message.clone(),
                    resource: v.resource.clone(),
                    remediation: v.remediation.clone(),
                    compliance: v.compliance.clone(),
                });
            }
        }

        violations
    }

    /// Build compliance summary from validation data
    fn build_summary(
        summary: &ValidationSummary,
        violations: &[ComplianceViolation],
    ) -> ComplianceSummary {
        let total_checks = summary.total_policies;
        let passed = summary.passed_policies;
        let failed = summary.failed_policies;

        let mut errors = 0;
        let mut warnings = 0;
        let mut infos = 0;

        for v in violations {
            match v.severity {
                OpaSeverity::Error => errors += 1,
                OpaSeverity::Warning => warnings += 1,
                OpaSeverity::Info => infos += 1,
            }
        }

        let compliance_score = Self::calculate_score(passed, total_checks);

        ComplianceSummary {
            total_checks,
            passed,
            failed,
            errors,
            warnings,
            infos,
            compliance_score,
        }
    }

    /// Calculate compliance score as percentage
    fn calculate_score(passed: usize, total: usize) -> f64 {
        if total == 0 {
            return 100.0;
        }

        (passed as f64 / total as f64) * 100.0
    }

    /// Group violations by compliance framework
    fn group_by_framework(
        violations: &[ComplianceViolation],
    ) -> HashMap<String, FrameworkSummary> {
        let mut frameworks: HashMap<String, HashMap<String, ControlStatus>> = HashMap::new();

        for violation in violations {
            for comp_ref in &violation.compliance {
                let framework_controls = frameworks
                    .entry(comp_ref.framework.clone())
                    .or_default();

                let control = framework_controls
                    .entry(comp_ref.control_id.clone())
                    .or_insert_with(|| ControlStatus {
                        control_id: comp_ref.control_id.clone(),
                        description: comp_ref.description.clone(),
                        passed: true,
                        violations: Vec::new(),
                    });

                control.passed = false;
                control.violations.push(violation.message.clone());
            }
        }

        frameworks
            .into_iter()
            .map(|(name, controls)| {
                let controls_vec: Vec<ControlStatus> = controls.into_values().collect();
                let passed = controls_vec.iter().filter(|c| c.passed).count();
                let failed = controls_vec.len() - passed;

                let summary = FrameworkSummary {
                    framework: name.clone(),
                    total_controls: controls_vec.len(),
                    passed,
                    failed,
                    controls: controls_vec,
                };

                (name, summary)
            })
            .collect()
    }

    /// Format report as JSON
    pub fn format_json(report: &ComplianceReport) -> Result<String> {
        serde_json::to_string_pretty(report).map_err(|e| anyhow::anyhow!("JSON error: {}", e))
    }

    /// Format report as Markdown
    pub fn format_markdown(report: &ComplianceReport) -> Result<String> {
        let mut md = String::new();

        md.push_str("# Compliance Report\n\n");
        md.push_str(&format!("**Generated:** {}\n", report.timestamp));
        md.push_str(&format!("**Infrastructure:** {}\n", report.infrastructure));

        if let Some(project) = &report.project {
            md.push_str(&format!("**Project:** {}\n", project));
        }

        if let Some(env) = &report.environment {
            md.push_str(&format!("**Environment:** {}\n", env));
        }

        md.push_str("\n## Summary\n\n");
        md.push_str("| Metric | Value |\n");
        md.push_str("|--------|-------|\n");
        md.push_str(&format!("| Total Checks | {} |\n", report.summary.total_checks));
        md.push_str(&format!("| Passed | {} |\n", report.summary.passed));
        md.push_str(&format!("| Failed | {} |\n", report.summary.failed));
        md.push_str(&format!("| Errors | {} |\n", report.summary.errors));
        md.push_str(&format!("| Warnings | {} |\n", report.summary.warnings));
        md.push_str(&format!(
            "| Compliance Score | {:.1}% |\n",
            report.summary.compliance_score
        ));

        if !report.violations.is_empty() {
            md.push_str("\n## Violations\n\n");

            for violation in &report.violations {
                md.push_str(&Self::format_violation_markdown(violation));
            }
        }

        if !report.by_framework.is_empty() {
            md.push_str("\n## By Framework\n\n");

            for (name, framework) in &report.by_framework {
                md.push_str(&format!("### {}\n\n", name));
                md.push_str(&format!(
                    "**Score:** {:.1}% ({}/{} controls passed)\n\n",
                    Self::calculate_score(framework.passed, framework.total_controls),
                    framework.passed,
                    framework.total_controls
                ));

                md.push_str("| Control | Status | Description |\n");
                md.push_str("|---------|--------|-------------|\n");

                for control in &framework.controls {
                    let status = if control.passed { "PASS" } else { "FAIL" };
                    let desc = control.description.as_deref().unwrap_or("-");
                    md.push_str(&format!("| {} | {} | {} |\n", control.control_id, status, desc));
                }

                md.push('\n');
            }
        }

        Ok(md)
    }

    /// Format a single violation as Markdown
    fn format_violation_markdown(violation: &ComplianceViolation) -> String {
        let mut md = String::new();
        let severity_label = match violation.severity {
            OpaSeverity::Error => "[ERROR]",
            OpaSeverity::Warning => "[WARNING]",
            OpaSeverity::Info => "[INFO]",
        };

        md.push_str(&format!("### {} {}\n\n", severity_label, violation.message));
        md.push_str(&format!("- **Policy:** {}\n", violation.policy));
        md.push_str(&format!("- **Rule:** {}\n", violation.rule));

        if let Some(resource) = &violation.resource {
            md.push_str(&format!("- **Resource:** {}\n", resource));
        }

        if !violation.compliance.is_empty() {
            let refs: Vec<String> = violation
                .compliance
                .iter()
                .map(|c| format!("{} {}", c.framework, c.control_id))
                .collect();
            md.push_str(&format!("- **Compliance:** {}\n", refs.join(", ")));
        }

        if let Some(remediation) = &violation.remediation {
            md.push_str("\n**Remediation:**\n");
            md.push_str(&format!("{}\n", remediation.description));

            if let Some(code) = &remediation.code_example {
                md.push_str("\n```hcl\n");
                md.push_str(code);
                md.push_str("\n```\n");
            }

            if let Some(url) = &remediation.documentation_url {
                md.push_str(&format!("\n[Documentation]({})\n", url));
            }
        }

        md.push_str("\n---\n\n");
        md
    }

    /// Format report as HTML
    pub fn format_html(report: &ComplianceReport) -> Result<String> {
        let mut html = String::new();

        html.push_str("<!DOCTYPE html>\n<html>\n<head>\n");
        html.push_str("<meta charset=\"utf-8\">\n");
        html.push_str("<title>Compliance Report</title>\n");
        html.push_str("<style>\n");
        html.push_str(Self::get_html_styles());
        html.push_str("</style>\n</head>\n<body>\n");

        html.push_str("<div class=\"container\">\n");
        html.push_str("<h1>Compliance Report</h1>\n");

        html.push_str("<div class=\"metadata\">\n");
        html.push_str(&format!("<p><strong>Generated:</strong> {}</p>\n", report.timestamp));
        html.push_str(&format!(
            "<p><strong>Infrastructure:</strong> {}</p>\n",
            report.infrastructure
        ));

        if let Some(project) = &report.project {
            html.push_str(&format!("<p><strong>Project:</strong> {}</p>\n", project));
        }

        if let Some(env) = &report.environment {
            html.push_str(&format!("<p><strong>Environment:</strong> {}</p>\n", env));
        }

        html.push_str("</div>\n");

        html.push_str(&Self::format_summary_html(&report.summary));

        if !report.violations.is_empty() {
            html.push_str("<h2>Violations</h2>\n");

            for violation in &report.violations {
                html.push_str(&Self::format_violation_html(violation));
            }
        }

        if !report.by_framework.is_empty() {
            html.push_str("<h2>By Framework</h2>\n");

            for (name, framework) in &report.by_framework {
                html.push_str(&Self::format_framework_html(name, framework));
            }
        }

        html.push_str("</div>\n</body>\n</html>");

        Ok(html)
    }

    /// Get HTML styles for the report
    fn get_html_styles() -> &'static str {
        r#"
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 0; padding: 20px; background: #f5f5f5; }
        .container { max-width: 1200px; margin: 0 auto; background: white; padding: 30px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }
        h1 { color: #333; border-bottom: 2px solid #007bff; padding-bottom: 10px; }
        h2 { color: #555; margin-top: 30px; }
        h3 { color: #666; }
        .metadata { background: #f8f9fa; padding: 15px; border-radius: 5px; margin-bottom: 20px; }
        .summary { display: grid; grid-template-columns: repeat(auto-fit, minmax(150px, 1fr)); gap: 15px; margin-bottom: 30px; }
        .stat { background: #f8f9fa; padding: 15px; border-radius: 5px; text-align: center; }
        .stat .value { font-size: 2em; font-weight: bold; color: #007bff; }
        .stat .label { color: #666; font-size: 0.9em; }
        .stat.score .value { color: #28a745; }
        .stat.errors .value { color: #dc3545; }
        .stat.warnings .value { color: #ffc107; }
        .violation { border: 1px solid #ddd; border-radius: 5px; padding: 15px; margin-bottom: 15px; }
        .violation.error { border-left: 4px solid #dc3545; }
        .violation.warning { border-left: 4px solid #ffc107; }
        .violation.info { border-left: 4px solid #17a2b8; }
        .severity { font-weight: bold; margin-bottom: 10px; }
        .severity.error { color: #dc3545; }
        .severity.warning { color: #ffc107; }
        .severity.info { color: #17a2b8; }
        .remediation { background: #e7f5ff; padding: 10px; border-radius: 5px; margin-top: 10px; }
        .code { background: #1e1e1e; color: #d4d4d4; padding: 10px; border-radius: 5px; font-family: monospace; overflow-x: auto; }
        table { width: 100%; border-collapse: collapse; margin-top: 10px; }
        th, td { padding: 10px; text-align: left; border-bottom: 1px solid #ddd; }
        th { background: #f8f9fa; }
        .pass { color: #28a745; font-weight: bold; }
        .fail { color: #dc3545; font-weight: bold; }
        "#
    }

    /// Format summary section as HTML
    fn format_summary_html(summary: &ComplianceSummary) -> String {
        format!(
            r#"<div class="summary">
            <div class="stat score"><div class="value">{:.1}%</div><div class="label">Compliance Score</div></div>
            <div class="stat"><div class="value">{}</div><div class="label">Total Checks</div></div>
            <div class="stat"><div class="value">{}</div><div class="label">Passed</div></div>
            <div class="stat"><div class="value">{}</div><div class="label">Failed</div></div>
            <div class="stat errors"><div class="value">{}</div><div class="label">Errors</div></div>
            <div class="stat warnings"><div class="value">{}</div><div class="label">Warnings</div></div>
            </div>"#,
            summary.compliance_score,
            summary.total_checks,
            summary.passed,
            summary.failed,
            summary.errors,
            summary.warnings
        )
    }

    /// Format a single violation as HTML
    fn format_violation_html(violation: &ComplianceViolation) -> String {
        let severity_class = match violation.severity {
            OpaSeverity::Error => "error",
            OpaSeverity::Warning => "warning",
            OpaSeverity::Info => "info",
        };

        let mut html = format!(
            "<div class=\"violation {}\">\n<div class=\"severity {}\">{}</div>\n",
            severity_class,
            severity_class,
            violation.severity.to_string().to_uppercase()
        );

        html.push_str(&format!("<h3>{}</h3>\n", violation.message));
        html.push_str(&format!("<p><strong>Policy:</strong> {}</p>\n", violation.policy));
        html.push_str(&format!("<p><strong>Rule:</strong> {}</p>\n", violation.rule));

        if let Some(resource) = &violation.resource {
            html.push_str(&format!("<p><strong>Resource:</strong> {}</p>\n", resource));
        }

        if !violation.compliance.is_empty() {
            let refs: Vec<String> = violation
                .compliance
                .iter()
                .map(|c| format!("{} {}", c.framework, c.control_id))
                .collect();
            html.push_str(&format!(
                "<p><strong>Compliance:</strong> {}</p>\n",
                refs.join(", ")
            ));
        }

        if let Some(remediation) = &violation.remediation {
            html.push_str("<div class=\"remediation\">\n");
            html.push_str("<strong>Remediation:</strong>\n");
            html.push_str(&format!("<p>{}</p>\n", remediation.description));

            if let Some(code) = &remediation.code_example {
                html.push_str(&format!("<pre class=\"code\">{}</pre>\n", code));
            }

            if let Some(url) = &remediation.documentation_url {
                html.push_str(&format!(
                    "<p><a href=\"{}\" target=\"_blank\">Documentation</a></p>\n",
                    url
                ));
            }

            html.push_str("</div>\n");
        }

        html.push_str("</div>\n");
        html
    }

    /// Format framework section as HTML
    fn format_framework_html(name: &str, framework: &FrameworkSummary) -> String {
        let mut html = format!("<h3>{}</h3>\n", name);
        let score = Self::calculate_score(framework.passed, framework.total_controls);
        html.push_str(&format!(
            "<p><strong>Score:</strong> {:.1}% ({}/{} controls passed)</p>\n",
            score, framework.passed, framework.total_controls
        ));

        html.push_str("<table>\n<tr><th>Control</th><th>Status</th><th>Description</th></tr>\n");

        for control in &framework.controls {
            let status_class = if control.passed { "pass" } else { "fail" };
            let status_text = if control.passed { "PASS" } else { "FAIL" };
            let desc = control.description.as_deref().unwrap_or("-");

            html.push_str(&format!(
                "<tr><td>{}</td><td class=\"{}\">{}</td><td>{}</td></tr>\n",
                control.control_id, status_class, status_text, desc
            ));
        }

        html.push_str("</table>\n");
        html
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opa::provider::{OpaViolation, PolicyEvaluation};

    fn create_test_violation(message: &str, severity: OpaSeverity) -> OpaViolation {
        OpaViolation {
            rule: "data.pmp.test.deny".to_string(),
            message: message.to_string(),
            severity,
            resource: Some("aws_ebs_volume.data".to_string()),
            details: None,
            remediation: Some(RemediationInfo {
                description: "Add encryption to the volume".to_string(),
                code_example: Some("encrypted = true".to_string()),
                documentation_url: Some("https://docs.example.com".to_string()),
                auto_fixable: false,
            }),
            compliance: vec![
                ComplianceRef {
                    framework: "CIS".to_string(),
                    control_id: "2.2.1".to_string(),
                    description: Some("Ensure EBS encryption".to_string()),
                },
            ],
        }
    }

    fn create_test_summary() -> ValidationSummary {
        let mut summary = ValidationSummary::new();

        summary.add_evaluation(PolicyEvaluation {
            policy_path: "encryption.rego".to_string(),
            policy_name: "encryption".to_string(),
            package_name: "data.pmp.security.encryption".to_string(),
            passed: false,
            violations: vec![
                create_test_violation("EBS volume not encrypted", OpaSeverity::Error),
            ],
            warnings: Vec::new(),
        });

        summary.add_evaluation(PolicyEvaluation {
            policy_path: "naming.rego".to_string(),
            policy_name: "naming".to_string(),
            package_name: "data.pmp.naming".to_string(),
            passed: true,
            violations: Vec::new(),
            warnings: Vec::new(),
        });

        summary
    }

    #[test]
    fn test_calculate_compliance_score() {
        assert_eq!(ComplianceReporter::calculate_score(8, 10), 80.0);
        assert_eq!(ComplianceReporter::calculate_score(0, 10), 0.0);
        assert_eq!(ComplianceReporter::calculate_score(10, 10), 100.0);
        assert_eq!(ComplianceReporter::calculate_score(0, 0), 100.0);
    }

    #[test]
    fn test_generate_report() {
        let summary = create_test_summary();
        let context = ReportContext {
            infrastructure: "test-infra".to_string(),
            project: Some("vpc".to_string()),
            environment: Some("production".to_string()),
        };

        let report = ComplianceReporter::generate_report(&summary, &context).unwrap();

        assert_eq!(report.infrastructure, "test-infra");
        assert_eq!(report.project, Some("vpc".to_string()));
        assert_eq!(report.environment, Some("production".to_string()));
        assert_eq!(report.summary.total_checks, 2);
        assert_eq!(report.summary.passed, 1);
        assert_eq!(report.summary.failed, 1);
        assert_eq!(report.summary.errors, 1);
        assert_eq!(report.violations.len(), 1);
    }

    #[test]
    fn test_group_by_framework() {
        let violations = vec![
            ComplianceViolation {
                policy: "data.pmp.test".to_string(),
                rule: "deny".to_string(),
                severity: OpaSeverity::Error,
                message: "Test violation".to_string(),
                resource: None,
                remediation: None,
                compliance: vec![
                    ComplianceRef {
                        framework: "CIS".to_string(),
                        control_id: "2.2.1".to_string(),
                        description: Some("Test control".to_string()),
                    },
                    ComplianceRef {
                        framework: "PCI-DSS".to_string(),
                        control_id: "3.4".to_string(),
                        description: None,
                    },
                ],
            },
        ];

        let by_framework = ComplianceReporter::group_by_framework(&violations);

        assert_eq!(by_framework.len(), 2);
        assert!(by_framework.contains_key("CIS"));
        assert!(by_framework.contains_key("PCI-DSS"));

        let cis = &by_framework["CIS"];
        assert_eq!(cis.total_controls, 1);
        assert_eq!(cis.failed, 1);
        assert_eq!(cis.passed, 0);
    }

    #[test]
    fn test_format_json() {
        let summary = create_test_summary();
        let context = ReportContext {
            infrastructure: "test".to_string(),
            project: None,
            environment: None,
        };

        let report = ComplianceReporter::generate_report(&summary, &context).unwrap();
        let json = ComplianceReporter::format_json(&report).unwrap();

        assert!(json.contains("\"infrastructure\": \"test\""));
        assert!(json.contains("\"total_checks\": 2"));
    }

    #[test]
    fn test_format_markdown() {
        let summary = create_test_summary();
        let context = ReportContext {
            infrastructure: "test".to_string(),
            project: Some("my-project".to_string()),
            environment: Some("prod".to_string()),
        };

        let report = ComplianceReporter::generate_report(&summary, &context).unwrap();
        let md = ComplianceReporter::format_markdown(&report).unwrap();

        assert!(md.contains("# Compliance Report"));
        assert!(md.contains("**Infrastructure:** test"));
        assert!(md.contains("**Project:** my-project"));
        assert!(md.contains("## Summary"));
        assert!(md.contains("## Violations"));
        assert!(md.contains("[ERROR]"));
        assert!(md.contains("**Remediation:**"));
    }

    #[test]
    fn test_format_html() {
        let summary = create_test_summary();
        let context = ReportContext {
            infrastructure: "test".to_string(),
            project: None,
            environment: None,
        };

        let report = ComplianceReporter::generate_report(&summary, &context).unwrap();
        let html = ComplianceReporter::format_html(&report).unwrap();

        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<h1>Compliance Report</h1>"));
        assert!(html.contains("class=\"violation error\""));
    }

    #[test]
    fn test_empty_report() {
        let summary = ValidationSummary::new();
        let context = ReportContext {
            infrastructure: "empty".to_string(),
            project: None,
            environment: None,
        };

        let report = ComplianceReporter::generate_report(&summary, &context).unwrap();

        assert_eq!(report.summary.total_checks, 0);
        assert_eq!(report.summary.compliance_score, 100.0);
        assert!(report.violations.is_empty());
        assert!(report.by_framework.is_empty());
    }
}
