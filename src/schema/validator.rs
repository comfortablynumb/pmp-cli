use anyhow::{Context, Result};
use inquire::Text;

/// Validates and collects user input
pub struct SchemaValidator;

impl SchemaValidator {
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
