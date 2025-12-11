use anyhow::{Context, Result};
use inquire::{Confirm, Password, Text};
use std::process::Command;

use crate::template::metadata::{
    CommandHookConfig, ConfirmHookConfig, Hook, SetEnvironmentHookConfig,
};

/// Outcome of running hooks - determines if the command should continue
#[derive(Debug, Clone, PartialEq)]
pub enum HookOutcome {
    /// Continue with the command execution
    Continue,
    /// Cancel the command execution (not an error, just stop)
    Cancel,
}

/// Executes pre and post hooks for commands
pub struct HooksRunner;

impl HooksRunner {
    /// Execute a list of hooks in sequence
    /// Returns HookOutcome::Cancel if any hook indicates execution should stop
    /// Returns an error if any hook fails unexpectedly
    pub fn run_hooks(hooks: &[Hook], working_dir: &str, hook_type: &str) -> Result<HookOutcome> {
        if hooks.is_empty() {
            return Ok(HookOutcome::Continue);
        }

        println!("Running {} hooks...", hook_type);

        for (index, hook) in hooks.iter().enumerate() {
            match hook {
                Hook::Command(config) => {
                    let outcome = Self::run_command_hook(config, working_dir, hook_type, index)?;

                    if outcome == HookOutcome::Cancel {
                        return Ok(HookOutcome::Cancel);
                    }
                }
                Hook::Confirm(config) => {
                    let outcome = Self::run_confirm_hook(config, hook_type, index)?;

                    if outcome == HookOutcome::Cancel {
                        return Ok(HookOutcome::Cancel);
                    }
                }
                Hook::SetEnvironment(config) => {
                    let outcome = Self::run_set_environment_hook(config, hook_type, index)?;

                    if outcome == HookOutcome::Cancel {
                        return Ok(HookOutcome::Cancel);
                    }
                }
            }
        }

        println!("{} hooks completed successfully", hook_type);
        Ok(HookOutcome::Continue)
    }

    /// Execute a command hook
    fn run_command_hook(
        config: &CommandHookConfig,
        working_dir: &str,
        hook_type: &str,
        index: usize,
    ) -> Result<HookOutcome> {
        println!("  [{}] Executing: {}", index + 1, config.command);

        let output = Self::execute_command(&config.command, working_dir)
            .with_context(|| format!("Failed to execute {} hook: {}", hook_type, config.command))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "{} hook failed: {}\nError: {}",
                hook_type,
                config.command,
                stderr
            );
        }

        // Print stdout if there's any output
        let stdout = String::from_utf8_lossy(&output.stdout);

        if !stdout.trim().is_empty() {
            println!("  Output: {}", stdout.trim());
        }

        Ok(HookOutcome::Continue)
    }

    /// Execute a confirm hook
    fn run_confirm_hook(
        config: &ConfirmHookConfig,
        hook_type: &str,
        index: usize,
    ) -> Result<HookOutcome> {
        println!("  [{}] Confirmation required", index + 1);
        println!(); // Add blank line for better readability

        // Prompt user with the question from config
        let confirmed = match config.default {
            Some(value) => {
                // Default provided - allow Enter to accept it
                Confirm::new(&config.question)
                    .with_default(value)
                    .prompt()
                    .with_context(|| {
                        format!(
                            "Failed to get user confirmation for {} hook (question: '{}')",
                            hook_type, config.question
                        )
                    })?
            }
            None => {
                // No default - require explicit Y/N input
                let help_msg = "Please enter Y or N explicitly";
                Confirm::new(&config.question)
                    .with_help_message(help_msg)
                    .prompt()
                    .with_context(|| {
                        format!(
                            "Failed to get user confirmation for {} hook (question: '{}')",
                            hook_type, config.question
                        )
                    })?
            }
        };

        println!(); // Add blank line after confirmation

        if confirmed {
            // User confirmed
            if config.exit_on_confirm {
                println!("  User confirmed - cancelling command as configured");
                return Ok(HookOutcome::Cancel);
            }
            println!("  User confirmed - continuing");
            Ok(HookOutcome::Continue)
        } else {
            // User declined/cancelled
            if config.exit_on_cancel {
                println!("  User declined - cancelling command as configured");
                return Ok(HookOutcome::Cancel);
            }
            println!("  User declined - continuing anyway");
            Ok(HookOutcome::Continue)
        }
    }

    /// Execute a set_environment hook
    fn run_set_environment_hook(
        config: &SetEnvironmentHookConfig,
        hook_type: &str,
        index: usize,
    ) -> Result<HookOutcome> {
        println!(
            "  [{}] Setting environment variable: {}",
            index + 1,
            config.name
        );

        // Check if the environment variable already exists
        let current_value = std::env::var(&config.name).ok();

        let value = if config.sensitive {
            // Use password input for sensitive values
            // For security reasons, don't show the current value as default for sensitive inputs
            Password::new(&config.prompt)
                .without_confirmation()
                .prompt()
                .with_context(|| {
                    format!(
                        "Failed to get sensitive input for {} hook (env: {})",
                        hook_type, config.name
                    )
                })?
        } else {
            // Use regular text input with current value as default if it exists
            let mut text_prompt = Text::new(&config.prompt);

            if let Some(ref default_val) = current_value {
                text_prompt = text_prompt.with_default(default_val);
            }

            text_prompt.prompt().with_context(|| {
                format!(
                    "Failed to get input for {} hook (env: {})",
                    hook_type, config.name
                )
            })?
        };

        // Set the environment variable for the current process and all child processes
        // SAFETY: This is safe because we're in a single-threaded context during hook execution
        // and the environment variable is set before any child processes are spawned
        unsafe {
            std::env::set_var(&config.name, &value);
        }

        if config.sensitive {
            println!(
                "  Environment variable {} set (sensitive value hidden)",
                config.name
            );
        } else {
            println!("  Environment variable {} set to: {}", config.name, value);
        }

        Ok(HookOutcome::Continue)
    }

    /// Execute a single shell command
    fn execute_command(command: &str, working_dir: &str) -> Result<std::process::Output> {
        // Use shell to execute the command for better compatibility
        #[cfg(target_os = "windows")]
        let output = Command::new("cmd")
            .args(["/C", command])
            .current_dir(working_dir)
            .output()?;

        #[cfg(not(target_os = "windows"))]
        let output = Command::new("sh")
            .args(["-c", command])
            .current_dir(working_dir)
            .output()?;

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_outcome_enum() {
        let continue_outcome = HookOutcome::Continue;
        let cancel_outcome = HookOutcome::Cancel;

        assert_eq!(continue_outcome, HookOutcome::Continue);
        assert_eq!(cancel_outcome, HookOutcome::Cancel);
        assert_ne!(continue_outcome, cancel_outcome);
    }

    #[test]
    fn test_run_hooks_empty() {
        let hooks: Vec<Hook> = vec![];
        let result = HooksRunner::run_hooks(&hooks, ".", "test");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), HookOutcome::Continue);
    }

    #[test]
    fn test_command_hook_config_creation() {
        let config = CommandHookConfig {
            command: "echo test".to_string(),
        };
        assert_eq!(config.command, "echo test");
    }

    #[test]
    fn test_confirm_hook_config_creation() {
        let config = ConfirmHookConfig {
            question: "Continue?".to_string(),
            exit_on_cancel: true,
            default: None,
            exit_on_confirm: false,
        };
        assert_eq!(config.question, "Continue?");
        assert!(config.exit_on_cancel);
        assert!(!config.exit_on_confirm);
    }

    #[test]
    fn test_confirm_hook_config_deserialization() {
        let yaml = r#"
question: "Are you sure you want to proceed?"
exit_on_cancel: true
exit_on_confirm: false
"#;
        let config: ConfirmHookConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.question, "Are you sure you want to proceed?");
        assert!(config.exit_on_cancel);
        assert!(!config.exit_on_confirm);
    }

    #[test]
    fn test_confirm_hook_serialization() {
        let config = ConfirmHookConfig {
            question: "Do you want to continue?".to_string(),
            exit_on_cancel: true,
            default: None,
            exit_on_confirm: false,
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("question:"));
        assert!(yaml.contains("Do you want to continue?"));
        assert!(yaml.contains("exit_on_cancel: true"));
        assert!(yaml.contains("exit_on_confirm: false"));
    }

    #[test]
    fn test_hook_deserialization_with_confirm() {
        let yaml = r#"
type: confirm
config:
  question: "Deploy to production?"
  exit_on_cancel: true
  exit_on_confirm: false
"#;
        let hook: Hook = serde_yaml::from_str(yaml).unwrap();
        match hook {
            Hook::Confirm(config) => {
                assert_eq!(config.question, "Deploy to production?");
                assert!(config.exit_on_cancel);
                assert!(!config.exit_on_confirm);
            }
            _ => panic!("Expected Confirm hook"),
        }
    }

    #[test]
    fn test_set_environment_hook_config_creation() {
        let config = SetEnvironmentHookConfig {
            name: "TEST_VAR".to_string(),
            prompt: "Enter value:".to_string(),
            sensitive: false,
        };
        assert_eq!(config.name, "TEST_VAR");
        assert_eq!(config.prompt, "Enter value:");
        assert!(!config.sensitive);
    }

    #[test]
    fn test_set_environment_hook_config_sensitive() {
        let config = SetEnvironmentHookConfig {
            name: "SECRET_KEY".to_string(),
            prompt: "Enter secret:".to_string(),
            sensitive: true,
        };
        assert_eq!(config.name, "SECRET_KEY");
        assert_eq!(config.prompt, "Enter secret:");
        assert!(config.sensitive);
    }

    #[test]
    fn test_hook_enum_variants() {
        let command_hook = Hook::Command(CommandHookConfig {
            command: "echo test".to_string(),
        });
        let confirm_hook = Hook::Confirm(ConfirmHookConfig {
            question: "Continue?".to_string(),
            exit_on_cancel: true,
            default: None,
            exit_on_confirm: false,
        });
        let set_env_hook = Hook::SetEnvironment(SetEnvironmentHookConfig {
            name: "TEST_VAR".to_string(),
            prompt: "Enter value:".to_string(),
            sensitive: false,
        });

        // Verify that all hook variants can be created
        match command_hook {
            Hook::Command(_) => {}
            _ => panic!("Expected Command variant"),
        }

        match confirm_hook {
            Hook::Confirm(_) => {}
            _ => panic!("Expected Confirm variant"),
        }

        match set_env_hook {
            Hook::SetEnvironment(_) => {}
            _ => panic!("Expected SetEnvironment variant"),
        }
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_execute_command_success() {
        let output = HooksRunner::execute_command("echo test", ".");
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.status.success());
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_execute_command_success_windows() {
        let output = HooksRunner::execute_command("echo test", ".");
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(output.status.success());
    }

    #[test]
    fn test_execute_command_failure() {
        let output = HooksRunner::execute_command("nonexistent_command_12345", ".");
        // The command execution itself should succeed, but the command will fail
        assert!(output.is_ok());
        let output = output.unwrap();
        assert!(!output.status.success());
    }

    #[test]
    fn test_set_environment_hook_config_serialization() {
        let config = SetEnvironmentHookConfig {
            name: "API_KEY".to_string(),
            prompt: "Enter your API key:".to_string(),
            sensitive: true,
        };

        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("name: API_KEY"));
        assert!(yaml.contains("prompt: 'Enter your API key:'"));
        assert!(yaml.contains("sensitive: true"));
    }

    #[test]
    fn test_set_environment_hook_config_deserialization() {
        let yaml = r#"
name: DATABASE_URL
prompt: "Enter database connection string:"
sensitive: true
"#;
        let config: SetEnvironmentHookConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "DATABASE_URL");
        assert_eq!(config.prompt, "Enter database connection string:");
        assert!(config.sensitive);
    }

    #[test]
    fn test_hook_serialization_with_set_environment() {
        let hook = Hook::SetEnvironment(SetEnvironmentHookConfig {
            name: "DEPLOY_ENV".to_string(),
            prompt: "Enter deployment environment:".to_string(),
            sensitive: false,
        });

        let yaml = serde_yaml::to_string(&hook).unwrap();
        assert!(yaml.contains("type: set_environment"));
        assert!(yaml.contains("name: DEPLOY_ENV"));
        assert!(yaml.contains("sensitive: false"));
    }

    #[test]
    fn test_hook_deserialization_with_set_environment() {
        let yaml = r#"
type: set_environment
config:
  name: AWS_REGION
  prompt: "Enter AWS region:"
  sensitive: false
"#;
        let hook: Hook = serde_yaml::from_str(yaml).unwrap();
        match hook {
            Hook::SetEnvironment(config) => {
                assert_eq!(config.name, "AWS_REGION");
                assert_eq!(config.prompt, "Enter AWS region:");
                assert!(!config.sensitive);
            }
            _ => panic!("Expected SetEnvironment hook"),
        }
    }

    #[test]
    fn test_multiple_hooks_with_set_environment() {
        let hooks = [
            Hook::Command(CommandHookConfig {
                command: "echo Starting deployment".to_string(),
            }),
            Hook::SetEnvironment(SetEnvironmentHookConfig {
                name: "DEPLOY_VERSION".to_string(),
                prompt: "Enter version to deploy:".to_string(),
                sensitive: false,
            }),
            Hook::SetEnvironment(SetEnvironmentHookConfig {
                name: "DEPLOY_TOKEN".to_string(),
                prompt: "Enter deployment token:".to_string(),
                sensitive: true,
            }),
            Hook::Confirm(ConfirmHookConfig {
                question: "Ready to deploy?".to_string(),
                exit_on_cancel: true,
                default: None,
                exit_on_confirm: false,
            }),
        ];

        assert_eq!(hooks.len(), 4);

        // Verify each hook type
        match &hooks[0] {
            Hook::Command(_) => {}
            _ => panic!("Expected Command hook at index 0"),
        }

        match &hooks[1] {
            Hook::SetEnvironment(config) => {
                assert_eq!(config.name, "DEPLOY_VERSION");
                assert!(!config.sensitive);
            }
            _ => panic!("Expected SetEnvironment hook at index 1"),
        }

        match &hooks[2] {
            Hook::SetEnvironment(config) => {
                assert_eq!(config.name, "DEPLOY_TOKEN");
                assert!(config.sensitive);
            }
            _ => panic!("Expected SetEnvironment hook at index 2"),
        }

        match &hooks[3] {
            Hook::Confirm(config) => {
                assert_eq!(config.question, "Ready to deploy?");
                assert!(config.exit_on_cancel);
                assert!(!config.exit_on_confirm);
            }
            _ => panic!("Expected Confirm hook at index 3"),
        }
    }

    #[test]
    fn test_hooks_array_deserialization_full_example() {
        let yaml = r#"
- type: confirm
  config:
    question: "Are you sure you want to continue?"
    exit_on_cancel: true
    exit_on_confirm: false

- type: command
  config:
    command: "echo Starting process"

- type: set_environment
  config:
    name: API_KEY
    prompt: "Enter your API key:"
    sensitive: true
"#;
        let hooks: Vec<Hook> = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(hooks.len(), 3);

        // Verify confirm hook with question
        match &hooks[0] {
            Hook::Confirm(config) => {
                assert_eq!(config.question, "Are you sure you want to continue?");
                assert!(config.exit_on_cancel);
                assert!(!config.exit_on_confirm);
            }
            _ => panic!("Expected Confirm hook"),
        }

        // Verify command hook
        match &hooks[1] {
            Hook::Command(config) => {
                assert_eq!(config.command, "echo Starting process");
            }
            _ => panic!("Expected Command hook"),
        }

        // Verify set_environment hook
        match &hooks[2] {
            Hook::SetEnvironment(config) => {
                assert_eq!(config.name, "API_KEY");
                assert_eq!(config.prompt, "Enter your API key:");
                assert!(config.sensitive);
            }
            _ => panic!("Expected SetEnvironment hook"),
        }
    }
}
