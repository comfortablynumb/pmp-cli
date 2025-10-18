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
        let schema = Self::load_schema(schema_path)?;

        // Extract properties from schema
        let properties = schema
            .get("properties")
            .and_then(|p| p.as_object())
            .context("Schema must have 'properties' object")?;

        let required_fields = schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(String::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let mut inputs = HashMap::new();

        // Collect inputs for each property
        for (key, property_schema) in properties {
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

        let prompt_message = if description.is_empty() {
            name.to_string()
        } else {
            format!("{} ({})", name, description)
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
                    let mut text_prompt = Text::new(&prompt_message);

                    if !required {
                        text_prompt = text_prompt.with_help_message("Optional - press Enter to skip");
                    }

                    // Set default if provided
                    if let Some(default) = schema.get("default").and_then(|d| d.as_str()) {
                        text_prompt = text_prompt.with_default(default);
                    }

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
}
