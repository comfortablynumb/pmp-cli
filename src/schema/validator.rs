use crate::template::metadata::EnvironmentConfig;
use anyhow::{Context, Result};
use inquire::{CustomType, Select, Text};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::Path;

/// Validates and collects user input based on JSON Schema
pub struct SchemaValidator;

impl SchemaValidator {
    /// Load JSON Schema from file
    pub fn load_schema(schema_path: &Path) -> Result<Value> {
        let content = std::fs::read_to_string(schema_path)
            .context("Failed to read schema.json")?;
        let schema: Value = serde_json::from_str(&content)
            .context("Failed to parse schema.json")?;
        Ok(schema)
    }

    /// Prompt user for inputs based on JSON Schema and validate
    pub fn collect_and_validate_inputs(schema_path: &Path) -> Result<HashMap<String, Value>> {
        Self::collect_and_validate_inputs_with_env(schema_path, None)
    }

    /// Prompt user for inputs based on JSON Schema with optional environment overrides
    pub fn collect_and_validate_inputs_with_env(
        schema_path: &Path,
        env_config: Option<&EnvironmentConfig>,
    ) -> Result<HashMap<String, Value>> {
        let mut schema = Self::load_schema(schema_path)?;

        // Apply environment overrides to the schema if provided
        if let Some(env) = env_config {
            Self::apply_environment_overrides(&mut schema, env)?;
        }

        // Extract properties from schema
        let properties = schema
            .get("properties")
            .and_then(|p| p.as_object())
            .context("Schema must have 'properties' object")?;

        let mut required_fields = schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // Ensure "name" is always required
        if properties.contains_key("name") && !required_fields.contains(&"name".to_string()) {
            required_fields.push("name".to_string());
        }

        let mut inputs = HashMap::new();

        // Always prompt for "name" first if it exists in the schema
        if let Some(name_schema) = properties.get("name") {
            let value = Self::prompt_for_name_property(name_schema)?;
            inputs.insert("name".to_string(), value);
        }

        // Collect inputs for remaining properties (excluding "name")
        for (key, property_schema) in properties {
            if key == "name" {
                continue; // Already handled
            }
            let is_required = required_fields.contains(key);
            let value = Self::prompt_for_property(key, property_schema, is_required)?;
            inputs.insert(key.clone(), value);
        }

        // Validate collected inputs against schema
        let compiled_schema = jsonschema::validator_for(&schema)
            .context("Failed to compile JSON Schema")?;

        let input_json = json!(inputs);

        if let Err(error) = compiled_schema.validate(&input_json) {
            anyhow::bail!("Validation error: {}", error);
        }

        Ok(inputs)
    }

    /// Prompt user for the "name" property with special validation
    fn prompt_for_name_property(schema: &Value) -> Result<Value> {
        let description = schema
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("Project name");

        let prompt_message = description.to_string();

        let text_prompt = Text::new(&prompt_message)
            .with_help_message("Only alphanumeric characters and hyphens allowed")
            .with_validator(|input: &str| {
                // Check if empty
                if input.is_empty() {
                    return Ok(inquire::validator::Validation::Invalid(
                        "Project name is required and cannot be empty".into()
                    ));
                }

                // Check if contains only alphanumeric characters and hyphens
                if !input.chars().all(|c| c.is_alphanumeric() || c == '-') {
                    return Ok(inquire::validator::Validation::Invalid(
                        "Project name must contain only alphanumeric characters or hyphens".into()
                    ));
                }

                Ok(inquire::validator::Validation::Valid)
            });

        let input = text_prompt
            .prompt()
            .context("Failed to get project name")?;

        Ok(Value::String(input))
    }

    /// Prompt user for a single property based on its schema
    fn prompt_for_property(name: &str, schema: &Value, required: bool) -> Result<Value> {
        let description = schema
            .get("description")
            .and_then(|d| d.as_str())
            .unwrap_or("");

        let property_type = schema
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("string");

        // Use description as the prompt message, fallback to field name if no description
        let prompt_message = if !description.is_empty() {
            description.to_string()
        } else {
            name.to_string()
        };

        match property_type {
            "string" => {
                // Check if there's an enum for selection
                if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
                    let options: Vec<String> = enum_values
                        .iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect();

                    let selected = Select::new(&prompt_message, options)
                        .prompt()
                        .context("Failed to get user selection")?;

                    Ok(Value::String(selected))
                } else {
                    // Get validation constraints from schema (convert to owned values)
                    let min_length = schema.get("minLength").and_then(|v| v.as_u64()).unwrap_or(0);
                    let max_length = schema.get("maxLength").and_then(|v| v.as_u64());
                    let pattern = schema.get("pattern").and_then(|v| v.as_str()).map(String::from);

                    let mut text_prompt = Text::new(&prompt_message);

                    if !required {
                        text_prompt = text_prompt.with_help_message("Optional - press Enter to skip");
                    }

                    // Set default if provided
                    if let Some(default) = schema.get("default").and_then(|d| d.as_str()) {
                        text_prompt = text_prompt.with_default(default);
                    }

                    // Add validator function
                    text_prompt = text_prompt.with_validator(move |input: &str| {
                        // Check if empty
                        if input.is_empty() {
                            if required {
                                return Ok(inquire::validator::Validation::Invalid(
                                    "This field is required and cannot be empty".into()
                                ));
                            }
                            return Ok(inquire::validator::Validation::Valid);
                        }

                        // Check minLength
                        if input.len() < min_length as usize {
                            return Ok(inquire::validator::Validation::Invalid(
                                format!("Minimum length is {}", min_length).into()
                            ));
                        }

                        // Check maxLength
                        if let Some(max) = max_length
                            && input.len() > max as usize {
                                return Ok(inquire::validator::Validation::Invalid(
                                    format!("Maximum length is {}", max).into()
                                ));
                            }

                        // Check pattern
                        if let Some(ref pat) = pattern
                            && let Ok(regex) = regex::Regex::new(pat)
                            && !regex.is_match(input) {
                                return Ok(inquire::validator::Validation::Invalid(
                                    format!("Input must match pattern: {}", pat).into()
                                ));
                            }

                        Ok(inquire::validator::Validation::Valid)
                    });

                    let input = text_prompt
                        .prompt()
                        .context("Failed to get user input")?;

                    Ok(Value::String(input))
                }
            }
            "integer" => {
                let input: i64 = CustomType::new(&prompt_message)
                    .with_error_message("Please enter a valid integer")
                    .prompt()
                    .context("Failed to get integer input")?;

                Ok(Value::Number(input.into()))
            }
            "number" => {
                let input: f64 = CustomType::new(&prompt_message)
                    .with_error_message("Please enter a valid number")
                    .prompt()
                    .context("Failed to get number input")?;

                if let Some(num) = serde_json::Number::from_f64(input) {
                    Ok(Value::Number(num))
                } else {
                    anyhow::bail!("Invalid number: {}", input)
                }
            }
            "boolean" => {
                let options = vec!["true", "false"];
                let selected = Select::new(&prompt_message, options)
                    .prompt()
                    .context("Failed to get boolean selection")?;

                Ok(Value::Bool(selected == "true"))
            }
            _ => {
                // Default to string for unsupported types
                let input = Text::new(&prompt_message)
                    .prompt()
                    .context("Failed to get user input")?;

                Ok(Value::String(input))
            }
        }
    }

    /// Apply environment-specific overrides to the schema
    fn apply_environment_overrides(schema: &mut Value, env_config: &EnvironmentConfig) -> Result<()> {
        let properties = schema
            .get_mut("properties")
            .and_then(|p| p.as_object_mut())
            .context("Schema must have 'properties' object")?;

        for (property_name, override_config) in &env_config.overrides {
            if let Some(property_schema) = properties.get_mut(property_name)
                && let Some(property_obj) = property_schema.as_object_mut() {
                    // Override default value
                    if let Some(default_value) = &override_config.default {
                        property_obj.insert("default".to_string(), default_value.clone());
                    }

                    // Override enum values
                    if let Some(enum_values) = &override_config.enum_values {
                        let enum_json: Vec<Value> = enum_values
                            .iter()
                            .map(|s| Value::String(s.clone()))
                            .collect();
                        property_obj.insert("enum".to_string(), Value::Array(enum_json));
                    }

                    // Override description
                    if let Some(description) = &override_config.description {
                        property_obj.insert("description".to_string(), Value::String(description.clone()));
                    }
                }
        }

        Ok(())
    }
}
