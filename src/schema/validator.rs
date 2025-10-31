use anyhow::Result;

/// Validates and collects user input
pub struct SchemaValidator;

impl SchemaValidator {
    /// Public method to prompt for project name
    pub fn prompt_for_project_name(ctx: &crate::context::Context) -> Result<String> {
        loop {
            let input = ctx.input.text(
                "Project name (lowercase letters, numbers, and underscores; cannot start with number):",
                None
            )?;

            // Check if empty
            if input.is_empty() {
                ctx.output.warning("Project name is required and cannot be empty");
                continue;
            }

            // Check if starts with a number
            if input.chars().next().map_or(false, |c| c.is_ascii_digit()) {
                ctx.output.warning("Project name must not start with a number");
                continue;
            }

            // Check if contains only lowercase alphanumeric characters and underscores
            if !input
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
            {
                ctx.output.warning("Project name must contain only lowercase letters, numbers, or underscores");
                continue;
            }

            return Ok(input);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to validate project names without user interaction
    fn validate_project_name(name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("Project name is required and cannot be empty".to_string());
        }

        if name.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            return Err("Project name must not start with a number".to_string());
        }

        if !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            return Err("Project name must contain only lowercase letters, numbers, or underscores".to_string());
        }

        Ok(())
    }

    #[test]
    fn test_valid_project_names() {
        assert!(validate_project_name("my_project").is_ok());
        assert!(validate_project_name("project").is_ok());
        assert!(validate_project_name("project_123").is_ok());
        assert!(validate_project_name("abc_def_ghi").is_ok());
        assert!(validate_project_name("my_cool_project_v2").is_ok());
        assert!(validate_project_name("a").is_ok());
        assert!(validate_project_name("project_with_many_underscores").is_ok());
    }

    #[test]
    fn test_invalid_project_name_starts_with_number() {
        let result = validate_project_name("1_project");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must not start with a number"));

        assert!(validate_project_name("2project").is_err());
        assert!(validate_project_name("9_test").is_err());
    }

    #[test]
    fn test_invalid_project_name_with_hyphen() {
        let result = validate_project_name("my-project");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("lowercase letters, numbers, or underscores"));

        assert!(validate_project_name("test-case").is_err());
        assert!(validate_project_name("my-cool-project").is_err());
    }

    #[test]
    fn test_invalid_project_name_with_uppercase() {
        let result = validate_project_name("MyProject");
        assert!(result.is_err());

        assert!(validate_project_name("PROJECT").is_err());
        assert!(validate_project_name("myProject").is_err());
        assert!(validate_project_name("My_Project").is_err());
    }

    #[test]
    fn test_invalid_project_name_empty() {
        let result = validate_project_name("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("required"));
    }

    #[test]
    fn test_invalid_project_name_special_chars() {
        assert!(validate_project_name("my@project").is_err());
        assert!(validate_project_name("my.project").is_err());
        assert!(validate_project_name("my project").is_err());
        assert!(validate_project_name("my/project").is_err());
        assert!(validate_project_name("my\\project").is_err());
        assert!(validate_project_name("my!project").is_err());
    }
}
