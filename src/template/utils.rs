use anyhow::Result;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;

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

/// Interpolate variables in a JSON value
///
/// Recursively interpolates variables in string values within the JSON structure
pub fn interpolate_value(value: &Value, variables: &HashMap<String, Value>) -> Result<Value> {
    match value {
        Value::String(s) => {
            Ok(Value::String(interpolate_variables(s, variables)?))
        }
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
        assert!(result.unwrap_err().to_string().contains("Variable '_name' not found"));
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
}
