use crate::opa::provider::{
    OpaSeverity, OpaViolation, OpaProvider, PolicyEvaluation, PolicyInfo, PolicyTestResult,
    ValidationParams, ValidationSummary,
};
use anyhow::{Context, Result};
use regorus::Engine;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, RwLock};

/// OPA provider implementation using the regorus crate
pub struct RegorusProvider {
    engine: RwLock<Engine>,
    policies: RwLock<HashMap<String, PolicyInfo>>,
}

impl RegorusProvider {
    pub fn new() -> Self {
        Self {
            engine: RwLock::new(Engine::new()),
            policies: RwLock::new(HashMap::new()),
        }
    }

    /// Evaluate deny rules and collect violations
    fn evaluate_deny_rules(
        &self,
        engine: &mut Engine,
        entrypoint: &str,
    ) -> Result<Vec<OpaViolation>> {
        let deny_path = format!("{}.deny", entrypoint);
        self.evaluate_rule_set(engine, &deny_path, OpaSeverity::Error)
    }

    /// Evaluate warn rules and collect violations
    fn evaluate_warn_rules(
        &self,
        engine: &mut Engine,
        entrypoint: &str,
    ) -> Result<Vec<OpaViolation>> {
        let warn_path = format!("{}.warn", entrypoint);
        self.evaluate_rule_set(engine, &warn_path, OpaSeverity::Warning)
    }

    /// Evaluate info rules and collect violations
    fn evaluate_info_rules(
        &self,
        engine: &mut Engine,
        entrypoint: &str,
    ) -> Result<Vec<OpaViolation>> {
        let info_path = format!("{}.info", entrypoint);
        self.evaluate_rule_set(engine, &info_path, OpaSeverity::Info)
    }

    /// Evaluate a specific rule set and parse results
    fn evaluate_rule_set(
        &self,
        engine: &mut Engine,
        rule_path: &str,
        severity: OpaSeverity,
    ) -> Result<Vec<OpaViolation>> {
        let mut violations = Vec::new();
        let result = engine.eval_rule(rule_path.to_string());

        if let Ok(value) = result {
            violations.extend(self.parse_rule_results(&value, rule_path, severity)?);
        }

        Ok(violations)
    }

    /// Parse OPA rule results into violations
    fn parse_rule_results(
        &self,
        value: &regorus::Value,
        rule_path: &str,
        severity: OpaSeverity,
    ) -> Result<Vec<OpaViolation>> {
        let mut violations = Vec::new();

        if let regorus::Value::Set(set) = value {
            for item in set.iter() {
                let violation = self.parse_single_result(item, rule_path, &severity)?;
                violations.push(violation);
            }
        }

        Ok(violations)
    }

    /// Parse a single OPA result into a violation
    fn parse_single_result(
        &self,
        item: &regorus::Value,
        rule_path: &str,
        severity: &OpaSeverity,
    ) -> Result<OpaViolation> {
        match item {
            regorus::Value::String(msg) => Ok(OpaViolation {
                rule: rule_path.to_string(),
                message: msg.to_string(),
                severity: severity.clone(),
                resource: None,
                details: None,
                remediation: None,
                compliance: Vec::new(),
            }),
            regorus::Value::Object(obj) => self.parse_object_result(obj, rule_path, severity),
            _ => Ok(OpaViolation {
                rule: rule_path.to_string(),
                message: format!("{}", item),
                severity: severity.clone(),
                resource: None,
                details: None,
                remediation: None,
                compliance: Vec::new(),
            }),
        }
    }

    /// Parse an object result into a violation with optional fields
    fn parse_object_result(
        &self,
        obj: &Arc<BTreeMap<regorus::Value, regorus::Value>>,
        rule_path: &str,
        severity: &OpaSeverity,
    ) -> Result<OpaViolation> {
        let msg_key = regorus::Value::String("msg".into());
        let resource_key = regorus::Value::String("resource".into());
        let details_key = regorus::Value::String("details".into());

        let msg = obj
            .get(&msg_key)
            .map(|v| self.value_to_string(v))
            .unwrap_or_else(|| "Policy violation".to_string());

        let resource = obj.get(&resource_key).map(|v| self.value_to_string(v));
        let details = obj.get(&details_key).map(|v| self.regorus_to_json(v));

        Ok(OpaViolation {
            rule: rule_path.to_string(),
            message: msg,
            severity: severity.clone(),
            resource,
            details,
            remediation: None,
            compliance: Vec::new(),
        })
    }

    /// Convert regorus Value to string
    fn value_to_string(&self, value: &regorus::Value) -> String {
        match value {
            regorus::Value::String(s) => s.to_string(),
            _ => format!("{}", value),
        }
    }

    /// Convert regorus Value to serde_json Value
    fn regorus_to_json(&self, value: &regorus::Value) -> serde_json::Value {
        match value {
            regorus::Value::Null => serde_json::Value::Null,
            regorus::Value::Bool(b) => serde_json::Value::Bool(*b),
            regorus::Value::String(s) => serde_json::Value::String(s.to_string()),
            regorus::Value::Number(n) => {
                let f = n.as_f64().unwrap_or(0.0);
                serde_json::Value::Number(
                    serde_json::Number::from_f64(f).unwrap_or_else(|| serde_json::Number::from(0)),
                )
            }
            regorus::Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(|v| self.regorus_to_json(v)).collect())
            }
            regorus::Value::Set(set) => {
                serde_json::Value::Array(set.iter().map(|v| self.regorus_to_json(v)).collect())
            }
            regorus::Value::Object(obj) => {
                let map: serde_json::Map<String, serde_json::Value> = obj
                    .iter()
                    .map(|(k, v)| (self.value_to_string(k), self.regorus_to_json(v)))
                    .collect();
                serde_json::Value::Object(map)
            }
            regorus::Value::Undefined => serde_json::Value::Null,
        }
    }

    /// Convert serde_json Value to regorus Value
    fn json_to_regorus(&self, value: &serde_json::Value) -> regorus::Value {
        match value {
            serde_json::Value::Null => regorus::Value::Null,
            serde_json::Value::Bool(b) => regorus::Value::Bool(*b),
            serde_json::Value::Number(n) => {
                regorus::Value::from(n.as_f64().unwrap_or(0.0))
            }
            serde_json::Value::String(s) => regorus::Value::String(s.clone().into()),
            serde_json::Value::Array(arr) => {
                let vec: Vec<regorus::Value> =
                    arr.iter().map(|v| self.json_to_regorus(v)).collect();
                regorus::Value::from(vec)
            }
            serde_json::Value::Object(obj) => {
                let map: BTreeMap<regorus::Value, regorus::Value> = obj
                    .iter()
                    .map(|(k, v)| {
                        (
                            regorus::Value::String(k.clone().into()),
                            self.json_to_regorus(v),
                        )
                    })
                    .collect();
                regorus::Value::from(map)
            }
        }
    }

    /// Parse policy metadata from Rego file content
    fn parse_policy_metadata(&self, path: &str, content: &str) -> PolicyInfo {
        use crate::opa::discovery::PolicyDiscovery;
        use std::path::Path;

        PolicyDiscovery::parse_policy_info(Path::new(path), content)
    }

    /// Run tests from a test file
    fn run_test_file(&self, test_path: &Path) -> Result<PolicyTestResult> {
        let content = std::fs::read_to_string(test_path)
            .with_context(|| format!("Failed to read test file: {:?}", test_path))?;

        let mut engine = Engine::new();
        engine
            .add_policy(test_path.to_string_lossy().to_string(), content)
            .with_context(|| format!("Failed to load test file: {:?}", test_path))?;

        let test_cases = Vec::new();
        let passed = 0;
        let failed = 0;

        // Note: regorus doesn't expose rule enumeration directly
        // Tests would need explicit registration or parsing

        Ok(PolicyTestResult {
            test_file: test_path.to_string_lossy().to_string(),
            passed,
            failed,
            test_cases,
        })
    }
}

impl Default for RegorusProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl OpaProvider for RegorusProvider {
    fn get_name(&self) -> &str {
        "regorus"
    }

    fn validate(&self, params: &ValidationParams) -> Result<ValidationSummary> {
        let mut engine = self.engine.write().map_err(|e| anyhow::anyhow!("{}", e))?;
        let policies = self.policies.read().map_err(|e| anyhow::anyhow!("{}", e))?;

        // Set input
        let input_value = self.json_to_regorus(params.input);
        engine.set_input(input_value);

        let mut summary = ValidationSummary::new();

        // Evaluate each policy
        for (name, info) in policies.iter() {
            if let Some(filter) = params.policy_filter {
                if !name.contains(filter) && !info.package_name.contains(filter) {
                    continue;
                }
            }

            let mut violations = Vec::new();

            // Evaluate deny, warn, and info rules
            violations.extend(self.evaluate_deny_rules(&mut engine, params.entrypoint)?);
            violations.extend(self.evaluate_warn_rules(&mut engine, params.entrypoint)?);
            violations.extend(self.evaluate_info_rules(&mut engine, params.entrypoint)?);

            let passed = !violations.iter().any(|v| v.severity == OpaSeverity::Error);

            let eval = PolicyEvaluation {
                policy_path: info.path.clone(),
                policy_name: name.clone(),
                package_name: info.package_name.clone(),
                passed,
                violations,
                warnings: Vec::new(),
            };

            summary.add_evaluation(eval);
        }

        Ok(summary)
    }

    fn test_policies(&self, test_dir: &Path) -> Result<Vec<PolicyTestResult>> {
        let mut results = Vec::new();

        if !test_dir.exists() {
            return Ok(results);
        }

        for entry in std::fs::read_dir(test_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "rego").unwrap_or(false) {
                let file_name = path.file_name().unwrap_or_default().to_string_lossy();

                if file_name.contains("_test") || file_name.starts_with("test_") {
                    results.push(self.run_test_file(&path)?);
                }
            }
        }

        Ok(results)
    }

    fn list_policies(&self) -> Vec<PolicyInfo> {
        self.policies
            .read()
            .map(|p| p.values().cloned().collect())
            .unwrap_or_default()
    }

    fn load_policies(&mut self, path: &Path) -> Result<usize> {
        let mut count = 0;

        if !path.exists() {
            return Ok(0);
        }

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();

            if file_path.extension().map(|e| e == "rego").unwrap_or(false) {
                let content = std::fs::read_to_string(&file_path)?;
                let name = file_path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                self.load_policy_from_string(&name, &content)?;
                count += 1;
            }
        }

        Ok(count)
    }

    fn load_policy_from_string(&mut self, name: &str, content: &str) -> Result<()> {
        let mut engine = self.engine.write().map_err(|e| anyhow::anyhow!("{}", e))?;
        let mut policies = self.policies.write().map_err(|e| anyhow::anyhow!("{}", e))?;

        engine
            .add_policy(name.to_string(), content.to_string())
            .with_context(|| format!("Failed to add policy: {}", name))?;

        let info = self.parse_policy_metadata(name, content);
        policies.insert(name.to_string(), info);

        Ok(())
    }

    fn set_data(&mut self, data: serde_json::Value) -> Result<()> {
        let mut engine = self.engine.write().map_err(|e| anyhow::anyhow!("{}", e))?;
        let data_value = self.json_to_regorus(&data);

        engine.add_data(data_value).context("Failed to set data")?;
        Ok(())
    }

    fn clear(&mut self) {
        if let Ok(mut engine) = self.engine.write() {
            *engine = Engine::new();
        }

        if let Ok(mut policies) = self.policies.write() {
            policies.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regorus_provider_new() {
        let provider = RegorusProvider::new();
        assert_eq!(provider.get_name(), "regorus");
    }

    #[test]
    fn test_regorus_provider_load_simple_policy() {
        let mut provider = RegorusProvider::new();

        let policy = r#"
            package pmp.test

            deny[msg] {
                msg := "test violation"
            }
        "#;

        provider
            .load_policy_from_string("test", policy)
            .expect("Failed to load policy");

        let policies = provider.list_policies();
        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].package_name, "data.pmp.test");
    }

    #[test]
    fn test_regorus_provider_validate_no_violations() {
        let mut provider = RegorusProvider::new();

        let policy = r#"
            package pmp.test

            deny[msg] {
                input.should_fail == true
                msg := "Should not see this"
            }
        "#;

        provider
            .load_policy_from_string("test", policy)
            .expect("Failed to load policy");

        let input = serde_json::json!({
            "should_fail": false
        });

        let params = ValidationParams {
            input: &input,
            policy_filter: None,
            entrypoint: "data.pmp.test",
        };

        let summary = provider.validate(&params).expect("Validation failed");
        assert_eq!(summary.errors, 0);
    }

    #[test]
    fn test_regorus_provider_validate_with_violations() {
        let mut provider = RegorusProvider::new();

        let policy = r#"
            package pmp.test

            deny[msg] {
                input.should_fail == true
                msg := "This should fail"
            }
        "#;

        provider
            .load_policy_from_string("test", policy)
            .expect("Failed to load policy");

        let input = serde_json::json!({
            "should_fail": true
        });

        let params = ValidationParams {
            input: &input,
            policy_filter: None,
            entrypoint: "data.pmp.test",
        };

        let summary = provider.validate(&params).expect("Validation failed");
        assert_eq!(summary.errors, 1);
    }

    #[test]
    fn test_parse_policy_metadata() {
        let provider = RegorusProvider::new();

        let content = r#"
            package pmp.naming

            # @description Enforce naming conventions

            deny[msg] {
                msg := "violation"
            }

            warn[msg] {
                msg := "warning"
            }
        "#;

        let info = provider.parse_policy_metadata("naming.rego", content);

        assert_eq!(info.package_name, "data.pmp.naming");
        assert_eq!(
            info.description,
            Some("Enforce naming conventions".to_string())
        );
        assert!(info.entrypoints.contains(&"deny".to_string()));
        assert!(info.entrypoints.contains(&"warn".to_string()));
    }

    #[test]
    fn test_json_to_regorus_conversion() {
        let provider = RegorusProvider::new();

        // Use floats since regorus numbers are f64
        let json = serde_json::json!({
            "string": "hello",
            "number": 42.0,
            "bool": true,
            "null": null,
            "array": [1.0, 2.0, 3.0],
            "object": {"nested": "value"}
        });

        let regorus_value = provider.json_to_regorus(&json);

        // Convert back and verify
        let back_to_json = provider.regorus_to_json(&regorus_value);
        assert_eq!(json, back_to_json);
    }

    #[test]
    fn test_regorus_provider_clear() {
        let mut provider = RegorusProvider::new();

        let policy = "package test\ndeny[msg] { msg := \"test\" }";
        provider
            .load_policy_from_string("test", policy)
            .expect("Failed to load");

        assert_eq!(provider.list_policies().len(), 1);

        provider.clear();

        assert_eq!(provider.list_policies().len(), 0);
    }
}
