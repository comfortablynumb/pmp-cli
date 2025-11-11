use anyhow::Result;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;

/// Interpolate environment variables in a string value
///
/// Replaces patterns like ${env:ENV_VAR_NAME} with environment variable values
///
/// # Arguments
/// * `input` - The input string that may contain environment variable references
///
/// # Examples
/// ```
/// std::env::set_var("DOCKER_USERNAME", "myuser");
/// let result = interpolate_env_variables("namespace-${env:DOCKER_USERNAME}")?;
/// assert_eq!(result, "namespace-myuser");
/// ```
pub fn interpolate_env_variables(input: &str) -> Result<String> {
    let re = Regex::new(r"\$\{env:([a-zA-Z_][a-zA-Z0-9_]*)\}").unwrap();

    let mut result = input.to_string();
    let mut errors = Vec::new();

    for cap in re.captures_iter(input) {
        let full_match = cap.get(0).unwrap().as_str();
        let env_var_name = cap.get(1).unwrap().as_str();

        match std::env::var(env_var_name) {
            Ok(value) => {
                result = result.replace(full_match, &value);
            }
            Err(_) => {
                errors.push(format!("Environment variable '{}' not found", env_var_name));
            }
        }
    }

    if !errors.is_empty() {
        anyhow::bail!(
            "Environment variable interpolation errors: {}",
            errors.join(", ")
        );
    }

    Ok(result)
}

/// Interpolate variables in a string value
///
/// Replaces patterns like ${var:variable_name} with actual variable values
///
/// # Arguments
/// * `input` - The input string that may contain variable references
/// * `variables` - Map of available variables
///
/// # Examples
/// ```
/// let vars = HashMap::from([("_name".to_string(), Value::String("myapp".to_string()))]);
/// let result = interpolate_variables("project-${var:_name}", &vars)?;
/// assert_eq!(result, "project-myapp");
/// ```
pub fn interpolate_variables(input: &str, variables: &HashMap<String, Value>) -> Result<String> {
    let re = Regex::new(r"\$\{var:([a-zA-Z_][a-zA-Z0-9_]*)\}").unwrap();

    let mut result = input.to_string();
    let mut errors = Vec::new();

    for cap in re.captures_iter(input) {
        let full_match = cap.get(0).unwrap().as_str();
        let var_name = cap.get(1).unwrap().as_str();

        match variables.get(var_name) {
            Some(Value::String(s)) => {
                result = result.replace(full_match, s);
            }
            Some(Value::Number(n)) => {
                result = result.replace(full_match, &n.to_string());
            }
            Some(Value::Bool(b)) => {
                result = result.replace(full_match, &b.to_string());
            }
            Some(_) => {
                errors.push(format!("Variable '{}' has unsupported type for interpolation (must be string, number, or boolean)", var_name));
            }
            None => {
                errors.push(format!("Variable '{}' not found", var_name));
            }
        }
    }

    if !errors.is_empty() {
        anyhow::bail!("Variable interpolation errors: {}", errors.join(", "));
    }

    Ok(result)
}

/// Interpolate both environment variables and project variables in a string value
///
/// First resolves ${env:...} patterns, then ${var:...} patterns
///
/// # Arguments
/// * `input` - The input string that may contain variable references
/// * `variables` - Map of available project variables
///
/// # Examples
/// ```
/// std::env::set_var("DOCKER_USERNAME", "myuser");
/// let vars = HashMap::from([("_name".to_string(), Value::String("myapp".to_string()))]);
/// let result = interpolate_all("${env:DOCKER_USERNAME}/${var:_name}", &vars)?;
/// assert_eq!(result, "myuser/myapp");
/// ```
pub fn interpolate_all(input: &str, variables: &HashMap<String, Value>) -> Result<String> {
    // First interpolate environment variables
    let after_env = interpolate_env_variables(input)?;

    // Then interpolate project variables
    let after_var = interpolate_variables(&after_env, variables)?;

    Ok(after_var)
}

/// Interpolate variables in a JSON value
///
/// Recursively interpolates variables in string values within the JSON structure
#[allow(dead_code)]
pub fn interpolate_value(value: &Value, variables: &HashMap<String, Value>) -> Result<Value> {
    match value {
        Value::String(s) => Ok(Value::String(interpolate_variables(s, variables)?)),
        Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for (key, val) in map {
                result.insert(key.clone(), interpolate_value(val, variables)?);
            }
            Ok(Value::Object(result))
        }
        Value::Array(arr) => {
            let mut result = Vec::new();
            for item in arr {
                result.push(interpolate_value(item, variables)?);
            }
            Ok(Value::Array(result))
        }
        other => Ok(other.clone()),
    }
}

/// Interpolate both environment variables and project variables in a JSON value
///
/// Recursively interpolates both ${env:...} and ${var:...} patterns in string values
pub fn interpolate_value_all(value: &Value, variables: &HashMap<String, Value>) -> Result<Value> {
    match value {
        Value::String(s) => Ok(Value::String(interpolate_all(s, variables)?)),
        Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for (key, val) in map {
                result.insert(key.clone(), interpolate_value_all(val, variables)?);
            }
            Ok(Value::Object(result))
        }
        Value::Array(arr) => {
            let mut result = Vec::new();
            for item in arr {
                result.push(interpolate_value_all(item, variables)?);
            }
            Ok(Value::Array(result))
        }
        other => Ok(other.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_simple_variable() {
        let mut vars = HashMap::new();
        vars.insert("_name".to_string(), Value::String("myapp".to_string()));

        let result = interpolate_variables("project-${var:_name}", &vars).unwrap();
        assert_eq!(result, "project-myapp");
    }

    #[test]
    fn test_interpolate_multiple_variables() {
        let mut vars = HashMap::new();
        vars.insert("_name".to_string(), Value::String("myapp".to_string()));
        vars.insert("_environment".to_string(), Value::String("dev".to_string()));

        let result = interpolate_variables("${var:_name}-${var:_environment}", &vars).unwrap();
        assert_eq!(result, "myapp-dev");
    }

    #[test]
    fn test_interpolate_number() {
        let mut vars = HashMap::new();
        vars.insert("port".to_string(), Value::Number(8080.into()));

        let result = interpolate_variables("Port is ${var:port}", &vars).unwrap();
        assert_eq!(result, "Port is 8080");
    }

    #[test]
    fn test_interpolate_missing_variable() {
        let vars = HashMap::new();

        let result = interpolate_variables("project-${var:_name}", &vars);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Variable '_name' not found")
        );
    }

    #[test]
    fn test_interpolate_no_variables() {
        let vars = HashMap::new();

        let result = interpolate_variables("simple-string", &vars).unwrap();
        assert_eq!(result, "simple-string");
    }

    #[test]
    fn test_interpolate_value_object() {
        let mut vars = HashMap::new();
        vars.insert("_name".to_string(), Value::String("myapp".to_string()));

        let input = serde_json::json!({
            "name": "${var:_name}",
            "nested": {
                "field": "value-${var:_name}"
            }
        });

        let result = interpolate_value(&input, &vars).unwrap();
        assert_eq!(result["name"], "myapp");
        assert_eq!(result["nested"]["field"], "value-myapp");
    }

    #[test]
    fn test_interpolate_env_variable() {
        unsafe {
            std::env::set_var("TEST_USERNAME", "testuser");
        }

        let result = interpolate_env_variables("namespace-${env:TEST_USERNAME}").unwrap();
        assert_eq!(result, "namespace-testuser");

        unsafe {
            std::env::remove_var("TEST_USERNAME");
        }
    }

    #[test]
    fn test_interpolate_env_multiple_variables() {
        unsafe {
            std::env::set_var("TEST_NAMESPACE", "myorg");
            std::env::set_var("TEST_REPO", "myrepo");
        }

        let result = interpolate_env_variables("${env:TEST_NAMESPACE}/${env:TEST_REPO}").unwrap();
        assert_eq!(result, "myorg/myrepo");

        unsafe {
            std::env::remove_var("TEST_NAMESPACE");
            std::env::remove_var("TEST_REPO");
        }
    }

    #[test]
    fn test_interpolate_env_missing_variable() {
        let result = interpolate_env_variables("namespace-${env:NONEXISTENT_VAR}");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Environment variable 'NONEXISTENT_VAR' not found")
        );
    }

    #[test]
    fn test_interpolate_env_no_variables() {
        let result = interpolate_env_variables("simple-string").unwrap();
        assert_eq!(result, "simple-string");
    }

    #[test]
    fn test_interpolate_all_combined() {
        unsafe {
            std::env::set_var("TEST_DOCKER_USERNAME", "dockeruser");
        }
        let mut vars = HashMap::new();
        vars.insert("_name".to_string(), Value::String("myapp".to_string()));

        let result = interpolate_all("${env:TEST_DOCKER_USERNAME}/${var:_name}", &vars).unwrap();
        assert_eq!(result, "dockeruser/myapp");

        unsafe {
            std::env::remove_var("TEST_DOCKER_USERNAME");
        }
    }

    #[test]
    fn test_interpolate_all_env_only() {
        unsafe {
            std::env::set_var("TEST_NAMESPACE_ONLY", "testns");
        }
        let vars = HashMap::new();

        let result = interpolate_all("namespace-${env:TEST_NAMESPACE_ONLY}", &vars).unwrap();
        assert_eq!(result, "namespace-testns");

        unsafe {
            std::env::remove_var("TEST_NAMESPACE_ONLY");
        }
    }

    #[test]
    fn test_interpolate_all_var_only() {
        let mut vars = HashMap::new();
        vars.insert("_name".to_string(), Value::String("myapp".to_string()));

        let result = interpolate_all("project-${var:_name}", &vars).unwrap();
        assert_eq!(result, "project-myapp");
    }

    #[test]
    fn test_interpolate_value_all_combined() {
        unsafe {
            std::env::set_var("TEST_ENV_VAR", "envvalue");
        }
        let mut vars = HashMap::new();
        vars.insert("_name".to_string(), Value::String("myapp".to_string()));

        let input = serde_json::json!({
            "namespace": "${env:TEST_ENV_VAR}",
            "name": "${var:_name}",
            "full": "${env:TEST_ENV_VAR}/${var:_name}"
        });

        let result = interpolate_value_all(&input, &vars).unwrap();
        assert_eq!(result["namespace"], "envvalue");
        assert_eq!(result["name"], "myapp");
        assert_eq!(result["full"], "envvalue/myapp");

        unsafe {
            std::env::remove_var("TEST_ENV_VAR");
        }
    }
}
