use anyhow::Result;

/// Validates and collects user input
pub struct SchemaValidator;

impl SchemaValidator {
    /// Validate a project name without user interaction
    pub fn validate_project_name(name: &str) -> Result<(), String> {
        if name.is_empty() {
            return Err("Project name is required and cannot be empty".to_string());
        }

        if name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return Err("Project name must not start with a number".to_string());
        }

        if name.starts_with('-') {
            return Err("Project name must not start with a hyphen".to_string());
        }

        if name.ends_with('-') {
            return Err("Project name must not end with a hyphen".to_string());
        }

        if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(
                "Project name must contain only lowercase letters, numbers, or hyphens".to_string(),
            );
        }

        Ok(())
    }

    /// Public method to prompt for project name
    pub fn prompt_for_project_name(ctx: &crate::context::Context) -> Result<String> {
        loop {
            let input = ctx.input.text(
                "Project name (lowercase letters, numbers, and hyphens; cannot start with number or hyphen):",
                None,
            )?;

            // Validate using the shared validation function
            if let Err(err) = Self::validate_project_name(&input) {
                ctx.output.warning(&err);
                continue;
            }

            return Ok(input);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_project_names() {
        assert!(SchemaValidator::validate_project_name("my-project").is_ok());
        assert!(SchemaValidator::validate_project_name("project").is_ok());
        assert!(SchemaValidator::validate_project_name("project-123").is_ok());
        assert!(SchemaValidator::validate_project_name("abc-def-ghi").is_ok());
        assert!(SchemaValidator::validate_project_name("my-cool-project-v2").is_ok());
        assert!(SchemaValidator::validate_project_name("a").is_ok());
        assert!(SchemaValidator::validate_project_name("project-with-many-hyphens").is_ok());
    }

    #[test]
    fn test_invalid_project_name_starts_with_number() {
        let result = SchemaValidator::validate_project_name("1-project");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must not start with a number"));

        assert!(SchemaValidator::validate_project_name("2project").is_err());
        assert!(SchemaValidator::validate_project_name("9-test").is_err());
    }

    #[test]
    fn test_invalid_project_name_starts_with_hyphen() {
        let result = SchemaValidator::validate_project_name("-project");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must not start with a hyphen"));

        assert!(SchemaValidator::validate_project_name("-test").is_err());
        assert!(SchemaValidator::validate_project_name("-my-project").is_err());
    }

    #[test]
    fn test_invalid_project_name_ends_with_hyphen() {
        let result = SchemaValidator::validate_project_name("project-");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must not end with a hyphen"));

        assert!(SchemaValidator::validate_project_name("test-").is_err());
        assert!(SchemaValidator::validate_project_name("my-project-").is_err());
    }

    #[test]
    fn test_invalid_project_name_with_underscore() {
        let result = SchemaValidator::validate_project_name("my_project");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("lowercase letters, numbers, or hyphens")
        );

        assert!(SchemaValidator::validate_project_name("test_case").is_err());
        assert!(SchemaValidator::validate_project_name("my_cool_project").is_err());
    }

    #[test]
    fn test_invalid_project_name_with_uppercase() {
        let result = SchemaValidator::validate_project_name("MyProject");
        assert!(result.is_err());

        assert!(SchemaValidator::validate_project_name("PROJECT").is_err());
        assert!(SchemaValidator::validate_project_name("myProject").is_err());
        assert!(SchemaValidator::validate_project_name("My-Project").is_err());
    }

    #[test]
    fn test_invalid_project_name_empty() {
        let result = SchemaValidator::validate_project_name("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("required"));
    }

    #[test]
    fn test_invalid_project_name_special_chars() {
        assert!(SchemaValidator::validate_project_name("my@project").is_err());
        assert!(SchemaValidator::validate_project_name("my.project").is_err());
        assert!(SchemaValidator::validate_project_name("my project").is_err());
        assert!(SchemaValidator::validate_project_name("my/project").is_err());
        assert!(SchemaValidator::validate_project_name("my\\project").is_err());
        assert!(SchemaValidator::validate_project_name("my!project").is_err());
    }
}
