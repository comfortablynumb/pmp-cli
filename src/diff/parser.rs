//! Plan output parser for OpenTofu/Terraform
//!
//! This module parses the text output from `tofu plan` or `terraform plan`
//! commands to extract structured resource and attribute changes.

use anyhow::Result;
use regex::Regex;

use super::types::{
    AttributeChange, AttributeChangeType, DiffChangeType, ParsedPlan, PlanSummary, ResourceChange,
};

/// Parser for OpenTofu/Terraform plan output
pub struct PlanParser {
    resource_pattern: Regex,
    attribute_pattern: Regex,
    arrow_pattern: Regex,
    summary_pattern: Regex,
    sensitive_pattern: Regex,
    computed_pattern: Regex,
    forces_replacement_pattern: Regex,
}

impl Default for PlanParser {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanParser {
    /// Create a new plan parser with compiled regex patterns
    pub fn new() -> Self {
        Self {
            // Match resource declarations like:
            // # aws_instance.example will be created
            // # module.vpc.aws_subnet.main must be replaced
            // Note: Allow leading whitespace before #
            resource_pattern: Regex::new(
                r"^\s*#\s+(.+?)\s+(will be|must be)\s+(created|updated|destroyed|replaced|read)",
            )
            .expect("Invalid resource pattern regex"),

            // Match attribute change lines:
            // + ami = "ami-12345678"
            // ~ instance_type = "t2.micro" -> "t3.micro"
            // - old_attr = "value"
            attribute_pattern: Regex::new(r"^\s*([+~-])\s+(.+?)\s+=\s+(.+)$")
                .expect("Invalid attribute pattern regex"),

            // Match arrow notation for modifications: "old" -> "new"
            arrow_pattern: Regex::new(r#""(.+?)"\s+->\s+"(.+?)""#)
                .expect("Invalid arrow pattern regex"),

            // Match summary line: Plan: 3 to add, 2 to change, 1 to destroy.
            summary_pattern: Regex::new(
                r"Plan:\s*(\d+)\s*to add,\s*(\d+)\s*to change,\s*(\d+)\s*to destroy",
            )
            .expect("Invalid summary pattern regex"),

            // Match sensitive value marker
            sensitive_pattern: Regex::new(r"\(sensitive(?:\s+value)?\)")
                .expect("Invalid sensitive pattern regex"),

            // Match computed value marker
            computed_pattern: Regex::new(r"\(known after apply\)")
                .expect("Invalid computed pattern regex"),

            // Match forces replacement marker
            forces_replacement_pattern: Regex::new(r"#\s*forces replacement")
                .expect("Invalid forces replacement pattern regex"),
        }
    }

    /// Parse plan output and return a structured ParsedPlan
    pub fn parse(&self, output: &str) -> Result<ParsedPlan> {
        let mut plan = ParsedPlan::new();
        plan.raw_output = output.to_string();

        let lines: Vec<&str> = output.lines().collect();
        let mut current_resource: Option<ResourceChange> = None;
        let mut parsed_summary: Option<PlanSummary> = None;

        for line in lines {
            // Check for resource declaration
            if let Some(caps) = self.resource_pattern.captures(line) {
                // Save the previous resource if any
                if let Some(resource) = current_resource.take() {
                    plan.resources.push(resource);
                }

                let address = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let action = caps.get(3).map(|m| m.as_str()).unwrap_or("");

                let change_type = self.parse_change_type(action);
                current_resource = Some(ResourceChange::new(address, change_type));
            }
            // Check for attribute changes (only if we're inside a resource)
            else if let Some(ref mut resource) = current_resource {
                if let Some(attr) = self.parse_attribute_line(line) {
                    resource.add_attribute(attr);
                }
            }

            // Check for summary line (can appear anywhere)
            if let Some(caps) = self.summary_pattern.captures(line) {
                parsed_summary = Some(self.parse_summary_from_captures(&caps));
            }
        }

        // Don't forget the last resource
        if let Some(resource) = current_resource {
            plan.resources.push(resource);
        }

        // Prefer parsed summary line, otherwise compute from resources
        plan.summary = if let Some(summary) = parsed_summary {
            summary
        } else {
            self.compute_summary_from_resources(&plan.resources)
        };

        plan.has_changes = plan.summary.has_changes();

        Ok(plan)
    }

    /// Parse from command output (stdout bytes)
    pub fn parse_output(&self, output: &std::process::Output) -> Result<ParsedPlan> {
        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse(&stdout)
    }

    /// Parse change type from action string
    fn parse_change_type(&self, action: &str) -> DiffChangeType {
        match action {
            "created" => DiffChangeType::Create,
            "updated" => DiffChangeType::Update,
            "destroyed" => DiffChangeType::Destroy,
            "replaced" => DiffChangeType::Replace,
            "read" => DiffChangeType::Read,
            _ => DiffChangeType::NoOp,
        }
    }

    /// Parse an attribute change line
    fn parse_attribute_line(&self, line: &str) -> Option<AttributeChange> {
        let caps = self.attribute_pattern.captures(line)?;

        let symbol = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        let name = caps.get(2).map(|m| m.as_str()).unwrap_or("").trim();
        let value_part = caps.get(3).map(|m| m.as_str()).unwrap_or("").trim();

        let change_type = match symbol {
            "+" => AttributeChangeType::Added,
            "-" => AttributeChangeType::Removed,
            "~" => AttributeChangeType::Modified,
            _ => return None,
        };

        let mut attr = AttributeChange::new(name, change_type.clone());

        // Check for sensitive marker
        attr.sensitive = self.sensitive_pattern.is_match(value_part);

        // Check for computed marker
        attr.computed = self.computed_pattern.is_match(value_part);

        // Check for forces replacement marker
        attr.forces_replacement = self.forces_replacement_pattern.is_match(line);

        // Parse values based on change type
        match change_type {
            AttributeChangeType::Modified => {
                if let Some(arrow_caps) = self.arrow_pattern.captures(value_part) {
                    let old = arrow_caps.get(1).map(|m| m.as_str()).unwrap_or("");
                    let new = arrow_caps.get(2).map(|m| m.as_str()).unwrap_or("");
                    attr.old_value = Some(old.to_string());
                    attr.new_value = Some(new.to_string());
                } else {
                    // Handle non-quoted or complex values
                    let (old, new) = self.parse_arrow_value(value_part);
                    attr.old_value = Some(old);
                    attr.new_value = Some(new);
                }
            }
            AttributeChangeType::Added => {
                attr.new_value = Some(self.clean_value(value_part));
            }
            AttributeChangeType::Removed => {
                attr.old_value = Some(self.clean_value(value_part));
            }
            AttributeChangeType::Unchanged => {}
        }

        Some(attr)
    }

    /// Parse non-quoted arrow values (e.g., numbers, booleans)
    fn parse_arrow_value(&self, value: &str) -> (String, String) {
        // Try to split on " -> " for unquoted values
        if let Some(pos) = value.find(" -> ") {
            let old = value[..pos].trim().trim_matches('"');
            let new = value[pos + 4..].trim().trim_matches('"');
            (old.to_string(), new.to_string())
        } else {
            ("(unknown)".to_string(), value.to_string())
        }
    }

    /// Clean a value string (remove quotes, markers)
    fn clean_value(&self, value: &str) -> String {
        let mut cleaned = value.trim().to_string();

        // Remove surrounding quotes
        if cleaned.starts_with('"') && cleaned.ends_with('"') && cleaned.len() > 1 {
            cleaned = cleaned[1..cleaned.len() - 1].to_string();
        }

        // Keep special markers as-is for display
        if self.sensitive_pattern.is_match(&cleaned) {
            return "(sensitive)".to_string();
        }

        if self.computed_pattern.is_match(&cleaned) {
            return "(known after apply)".to_string();
        }

        cleaned
    }

    /// Parse summary from regex captures
    fn parse_summary_from_captures(&self, caps: &regex::Captures) -> PlanSummary {
        PlanSummary {
            to_add: caps
                .get(1)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0),
            to_change: caps
                .get(2)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0),
            to_destroy: caps
                .get(3)
                .and_then(|m| m.as_str().parse().ok())
                .unwrap_or(0),
            to_replace: 0,
            unchanged: 0,
        }
    }

    /// Compute summary from parsed resources
    fn compute_summary_from_resources(&self, resources: &[ResourceChange]) -> PlanSummary {
        let mut summary = PlanSummary::default();

        for resource in resources {
            match resource.change_type {
                DiffChangeType::Create => summary.to_add += 1,
                DiffChangeType::Update => summary.to_change += 1,
                DiffChangeType::Destroy => summary.to_destroy += 1,
                DiffChangeType::Replace => summary.to_replace += 1,
                DiffChangeType::NoOp => summary.unchanged += 1,
                DiffChangeType::Read => {}
            }
        }

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_plan_output() -> &'static str {
        r#"
Terraform will perform the following actions:

  # aws_instance.web_server will be created
  + resource "aws_instance" "web_server" {
      + ami                          = "ami-12345678"
      + instance_type                = "t3.micro"
      + id                           = (known after apply)
      + tags                         = {
          + "Name" = "web-server"
        }
    }

  # aws_security_group.main will be updated in-place
  ~ resource "aws_security_group" "main" {
      ~ ingress.0.from_port = "80" -> "443"
        name                = "main-sg"
    }

  # aws_instance.old_server will be destroyed
  - resource "aws_instance" "old_server" {
      - ami           = "ami-old12345"
      - instance_type = "t2.micro"
    }

Plan: 1 to add, 1 to change, 1 to destroy.
"#
    }

    #[test]
    fn test_parse_create_resource() {
        let parser = PlanParser::new();
        let plan = parser.parse(sample_plan_output()).unwrap();

        let create_resource = plan
            .resources
            .iter()
            .find(|r| r.change_type == DiffChangeType::Create);

        assert!(create_resource.is_some());
        let resource = create_resource.unwrap();
        assert_eq!(resource.address, "aws_instance.web_server");
        assert_eq!(resource.resource_type, "aws_instance");
        assert_eq!(resource.resource_name, "web_server");
    }

    #[test]
    fn test_parse_update_resource() {
        let parser = PlanParser::new();
        let plan = parser.parse(sample_plan_output()).unwrap();

        let update_resource = plan
            .resources
            .iter()
            .find(|r| r.change_type == DiffChangeType::Update);

        assert!(update_resource.is_some());
        let resource = update_resource.unwrap();
        assert_eq!(resource.address, "aws_security_group.main");
    }

    #[test]
    fn test_parse_destroy_resource() {
        let parser = PlanParser::new();
        let plan = parser.parse(sample_plan_output()).unwrap();

        let destroy_resource = plan
            .resources
            .iter()
            .find(|r| r.change_type == DiffChangeType::Destroy);

        assert!(destroy_resource.is_some());
        let resource = destroy_resource.unwrap();
        assert_eq!(resource.address, "aws_instance.old_server");
    }

    #[test]
    fn test_parse_summary() {
        let parser = PlanParser::new();
        let plan = parser.parse(sample_plan_output()).unwrap();

        assert_eq!(plan.summary.to_add, 1);
        assert_eq!(plan.summary.to_change, 1);
        assert_eq!(plan.summary.to_destroy, 1);
        assert!(plan.has_changes);
    }

    #[test]
    fn test_parse_attribute_added() {
        let parser = PlanParser::new();
        let plan = parser.parse(sample_plan_output()).unwrap();

        let create_resource = plan
            .resources
            .iter()
            .find(|r| r.change_type == DiffChangeType::Create)
            .unwrap();

        let ami_attr = create_resource
            .attributes
            .iter()
            .find(|a| a.name == "ami")
            .unwrap();

        assert_eq!(ami_attr.change_type, AttributeChangeType::Added);
        assert_eq!(ami_attr.new_value, Some("ami-12345678".to_string()));
    }

    #[test]
    fn test_parse_attribute_modified() {
        let parser = PlanParser::new();
        let plan = parser.parse(sample_plan_output()).unwrap();

        let update_resource = plan
            .resources
            .iter()
            .find(|r| r.change_type == DiffChangeType::Update)
            .unwrap();

        let port_attr = update_resource
            .attributes
            .iter()
            .find(|a| a.name == "ingress.0.from_port")
            .unwrap();

        assert_eq!(port_attr.change_type, AttributeChangeType::Modified);
        assert_eq!(port_attr.old_value, Some("80".to_string()));
        assert_eq!(port_attr.new_value, Some("443".to_string()));
    }

    #[test]
    fn test_parse_computed_attribute() {
        let parser = PlanParser::new();
        let plan = parser.parse(sample_plan_output()).unwrap();

        let create_resource = plan
            .resources
            .iter()
            .find(|r| r.change_type == DiffChangeType::Create)
            .unwrap();

        let id_attr = create_resource.attributes.iter().find(|a| a.name == "id");

        assert!(id_attr.is_some());
        let attr = id_attr.unwrap();
        assert!(attr.computed);
    }

    #[test]
    fn test_parse_module_address() {
        let parser = PlanParser::new();
        let output = r#"
  # module.vpc.aws_subnet.public will be created
  + resource "aws_subnet" "public" {
      + cidr_block = "10.0.1.0/24"
    }

Plan: 1 to add, 0 to change, 0 to destroy.
"#;

        let plan = parser.parse(output).unwrap();
        let resource = &plan.resources[0];

        assert_eq!(resource.address, "module.vpc.aws_subnet.public");
        assert_eq!(resource.module_path, Some("module.vpc".to_string()));
        assert_eq!(resource.resource_type, "aws_subnet");
        assert_eq!(resource.resource_name, "public");
    }

    #[test]
    fn test_parse_replace_resource() {
        let parser = PlanParser::new();
        let output = r#"
  # aws_instance.web must be replaced
  -/+ resource "aws_instance" "web" {
      ~ ami = "ami-old" -> "ami-new" # forces replacement
    }

Plan: 0 to add, 0 to change, 0 to destroy.
"#;

        let plan = parser.parse(output).unwrap();
        let resource = &plan.resources[0];

        assert_eq!(resource.change_type, DiffChangeType::Replace);
        assert_eq!(resource.address, "aws_instance.web");
    }

    #[test]
    fn test_parse_sensitive_value() {
        let parser = PlanParser::new();
        let output = r#"
  # aws_db_instance.main will be created
  + resource "aws_db_instance" "main" {
      + password = (sensitive value)
    }

Plan: 1 to add, 0 to change, 0 to destroy.
"#;

        let plan = parser.parse(output).unwrap();
        let resource = &plan.resources[0];
        let password_attr = resource
            .attributes
            .iter()
            .find(|a| a.name == "password")
            .unwrap();

        assert!(password_attr.sensitive);
    }

    #[test]
    fn test_no_changes_plan() {
        let parser = PlanParser::new();
        let output = r#"
No changes. Your infrastructure matches the configuration.

Terraform has compared your real infrastructure against your configuration
and found no differences, so no changes are needed.
"#;

        let plan = parser.parse(output).unwrap();

        assert!(plan.resources.is_empty());
        assert!(!plan.has_changes);
        assert_eq!(plan.summary.total_changes(), 0);
    }

    #[test]
    fn test_parse_forces_replacement() {
        let parser = PlanParser::new();
        let output = r#"
  # aws_instance.web will be updated in-place
  ~ resource "aws_instance" "web" {
      ~ ami = "ami-old" -> "ami-new" # forces replacement
    }

Plan: 0 to add, 1 to change, 0 to destroy.
"#;

        let plan = parser.parse(output).unwrap();
        let resource = &plan.resources[0];
        let ami_attr = resource
            .attributes
            .iter()
            .find(|a| a.name == "ami")
            .unwrap();

        assert!(ami_attr.forces_replacement);
        assert!(!resource.forces_replacement.is_empty());
    }
}
