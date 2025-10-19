use crate::template::metadata::{InputDefinition, InputType};
use anyhow::{Context, Result};
use inquire::{Confirm, MultiSelect, Select, Text};
use serde_json::Value;
use std::collections::HashMap;

/// Validates and collects user input based on custom input definitions
pub struct SchemaValidator;

impl SchemaValidator {
    /// Prompt user for inputs based on input definitions
    pub fn collect_inputs(
        inputs: &[InputDefinition],
        project_name: String,
    ) -> Result<HashMap<String, Value>> {
        let mut collected = HashMap::new();

        // Add the project name field
        collected.insert("name".to_string(), Value::String(project_name));

        // Collect each input
        for input in inputs {
            let value = Self::prompt_for_input(input)?;
            collected.insert(input.name.clone(), value);
        }

        Ok(collected)
    }

    /// Prompt for a single input
    fn prompt_for_input(input: &InputDefinition) -> Result<Value> {
        match &input.input_type {
            InputType::Text => Self::prompt_text(input),
            InputType::Password => Self::prompt_password(input),
            InputType::Boolean => Self::prompt_boolean(input),
            InputType::Select => Self::prompt_select(input),
            InputType::MultiSelect => Self::prompt_multiselect(input),
        }
    }

    /// Prompt for text input
    fn prompt_text(input: &InputDefinition) -> Result<Value> {
        let prompt_msg = format!("{}:", input.label);
        let mut prompt = Text::new(&prompt_msg);

        if let Some(help) = &input.help {
            prompt = prompt.with_help_message(help);
        }

        if !input.required {
            prompt = prompt.with_help_message("Optional - press Enter to skip");
        }

        if let Some(default) = &input.default {
            if let Some(default_str) = default.as_str() {
                prompt = prompt.with_default(default_str);
            }
        }

        // Add validator if validation rules exist
        if let Some(validation) = &input.validation {
            let min_length = validation.min_length.unwrap_or(0);
            let max_length = validation.max_length;
            let pattern = validation.pattern.clone();
            let error_msg = validation.error_message.clone();
            let required = input.required;

            prompt = prompt.with_validator(move |input_str: &str| {
                // Check if empty
                if input_str.is_empty() {
                    if required {
                        return Ok(inquire::validator::Validation::Invalid(
                            "This field is required and cannot be empty".into(),
                        ));
                    }
                    return Ok(inquire::validator::Validation::Valid);
                }

                // Check minLength
                if input_str.len() < min_length {
                    return Ok(inquire::validator::Validation::Invalid(
                        format!("Minimum length is {}", min_length).into(),
                    ));
                }

                // Check maxLength
                if let Some(max) = max_length {
                    if input_str.len() > max {
                        return Ok(inquire::validator::Validation::Invalid(
                            format!("Maximum length is {}", max).into(),
                        ));
                    }
                }

                // Check pattern
                if let Some(ref pat) = pattern {
                    if let Ok(regex) = regex::Regex::new(pat) {
                        if !regex.is_match(input_str) {
                            let msg = error_msg
                                .clone()
                                .unwrap_or_else(|| format!("Input must match pattern: {}", pat));
                            return Ok(inquire::validator::Validation::Invalid(msg.into()));
                        }
                    }
                }

                Ok(inquire::validator::Validation::Valid)
            });
        }

        let result = prompt.prompt().context("Failed to get text input")?;
        Ok(Value::String(result))
    }

    /// Prompt for password input
    fn prompt_password(input: &InputDefinition) -> Result<Value> {
        let prompt_msg = format!("{}:", input.label);
        let mut prompt = inquire::Password::new(&prompt_msg);

        if let Some(help) = &input.help {
            prompt = prompt.with_help_message(help);
        }

        if !input.required {
            prompt = prompt.with_help_message("Optional - press Enter to skip");
        }

        let result = prompt.prompt().context("Failed to get password input")?;
        Ok(Value::String(result))
    }

    /// Prompt for boolean input
    fn prompt_boolean(input: &InputDefinition) -> Result<Value> {
        let default_val = input
            .default
            .as_ref()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let message = if let Some(help) = &input.help {
            format!("{} ({})", input.label, help)
        } else {
            input.label.clone()
        };

        let result = Confirm::new(&format!("{}:", message))
            .with_default(default_val)
            .prompt()
            .context("Failed to get boolean input")?;

        Ok(Value::Bool(result))
    }

    /// Prompt for select input
    fn prompt_select(input: &InputDefinition) -> Result<Value> {
        let options = input
            .options
            .as_ref()
            .context("Select input must have options")?;

        let option_labels: Vec<String> = options.iter().map(|o| o.label.clone()).collect();

        let prompt_msg = format!("{}:", input.label);
        let mut select = Select::new(&prompt_msg, option_labels.clone());

        if let Some(help) = &input.help {
            select = select.with_help_message(help);
        }

        let selected_label = select.prompt().context("Failed to get selection")?;

        // Find the corresponding value
        let selected_option = options
            .iter()
            .find(|o| o.label == selected_label)
            .context("Selected option not found")?;

        Ok(Value::String(selected_option.value.clone()))
    }

    /// Prompt for multiselect input
    fn prompt_multiselect(input: &InputDefinition) -> Result<Value> {
        let options = input
            .options
            .as_ref()
            .context("MultiSelect input must have options")?;

        let option_labels: Vec<String> = options.iter().map(|o| o.label.clone()).collect();

        let prompt_msg = format!("{}:", input.label);
        let mut multiselect = MultiSelect::new(&prompt_msg, option_labels.clone());

        if let Some(help) = &input.help {
            multiselect = multiselect.with_help_message(help);
        }

        let selected_labels = multiselect
            .prompt()
            .context("Failed to get multi-selection")?;

        // Find the corresponding values
        let selected_values: Vec<Value> = selected_labels
            .iter()
            .filter_map(|label| {
                options
                    .iter()
                    .find(|o| &o.label == label)
                    .map(|o| Value::String(o.value.clone()))
            })
            .collect();

        Ok(Value::Array(selected_values))
    }

    /// Public method to prompt for project name
    pub fn prompt_for_project_name() -> Result<String> {
        let text_prompt = Text::new("Project name:")
            .with_help_message("Only lowercase letters, numbers, and hyphens allowed")
            .with_validator(|input: &str| {
                // Check if empty
                if input.is_empty() {
                    return Ok(inquire::validator::Validation::Invalid(
                        "Project name is required and cannot be empty".into(),
                    ));
                }

                // Check if contains uppercase characters
                if input.chars().any(|c| c.is_uppercase()) {
                    return Ok(inquire::validator::Validation::Invalid(
                        "Project name must not contain uppercase characters".into(),
                    ));
                }

                // Check if contains only lowercase alphanumeric characters and hyphens
                if !input
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                {
                    return Ok(inquire::validator::Validation::Invalid(
                        "Project name must contain only lowercase letters, numbers, or hyphens"
                            .into(),
                    ));
                }

                Ok(inquire::validator::Validation::Valid)
            });

        let input = text_prompt
            .prompt()
            .context("Failed to get project name")?;

        Ok(input)
    }
}
