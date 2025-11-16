use super::executor::{Executor, ExecutorConfig};
use anyhow::Result;
use std::process::Output;

/// None executor implementation for dependency-only projects
/// This executor does nothing - it's used for projects that only define dependencies
/// and don't have their own infrastructure to manage
pub struct NoneExecutor;

impl NoneExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Executor for NoneExecutor {
    fn check_installed(&self) -> Result<bool> {
        // None executor is always "installed" since it does nothing
        Ok(true)
    }

    fn init(&self, _working_dir: &str) -> Result<Output> {
        // Return a successful "no-op" output
        Ok(Output {
            status: std::process::ExitStatus::default(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }

    fn plan(
        &self,
        _config: &ExecutorConfig,
        _working_dir: &str,
        _extra_args: &[String],
    ) -> Result<()> {
        // No-op for plan
        Ok(())
    }

    fn apply(
        &self,
        _config: &ExecutorConfig,
        _working_dir: &str,
        _extra_args: &[String],
    ) -> Result<()> {
        // No-op for apply
        Ok(())
    }

    fn destroy(
        &self,
        _config: &ExecutorConfig,
        _working_dir: &str,
        _extra_args: &[String],
    ) -> Result<()> {
        // No-op for destroy
        Ok(())
    }

    fn refresh(
        &self,
        _config: &ExecutorConfig,
        _working_dir: &str,
        _extra_args: &[String],
    ) -> Result<()> {
        // No-op for refresh
        Ok(())
    }

    fn get_name(&self) -> &str {
        "none"
    }

    fn default_plan_command(&self) -> &str {
        ""
    }

    fn default_apply_command(&self) -> &str {
        ""
    }

    fn default_destroy_command(&self) -> &str {
        ""
    }

    fn default_refresh_command(&self) -> &str {
        ""
    }

    fn supports_backend(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_none_executor_check_installed() {
        let executor = NoneExecutor::new();
        assert!(executor.check_installed().unwrap());
    }

    #[test]
    fn test_none_executor_init() {
        let executor = NoneExecutor::new();
        let result = executor.init("/tmp");
        assert!(result.is_ok());

        let output = result.unwrap();
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn test_none_executor_plan() {
        let executor = NoneExecutor::new();
        let config = ExecutorConfig {
            plan_command: None,
            apply_command: None,
            destroy_command: None,
            refresh_command: None,
        };
        assert!(executor.plan(&config, "/tmp", &[]).is_ok());
    }

    #[test]
    fn test_none_executor_apply() {
        let executor = NoneExecutor::new();
        let config = ExecutorConfig {
            plan_command: None,
            apply_command: None,
            destroy_command: None,
            refresh_command: None,
        };
        assert!(executor.apply(&config, "/tmp", &[]).is_ok());
    }

    #[test]
    fn test_none_executor_destroy() {
        let executor = NoneExecutor::new();
        let config = ExecutorConfig {
            plan_command: None,
            apply_command: None,
            destroy_command: None,
            refresh_command: None,
        };
        assert!(executor.destroy(&config, "/tmp", &[]).is_ok());
    }

    #[test]
    fn test_none_executor_refresh() {
        let executor = NoneExecutor::new();
        let config = ExecutorConfig {
            plan_command: None,
            apply_command: None,
            destroy_command: None,
            refresh_command: None,
        };
        assert!(executor.refresh(&config, "/tmp", &[]).is_ok());
    }

    #[test]
    fn test_none_executor_get_name() {
        let executor = NoneExecutor::new();
        assert_eq!(executor.get_name(), "none");
    }

    #[test]
    fn test_none_executor_default_commands() {
        let executor = NoneExecutor::new();
        assert_eq!(executor.default_plan_command(), "");
        assert_eq!(executor.default_apply_command(), "");
        assert_eq!(executor.default_destroy_command(), "");
        assert_eq!(executor.default_refresh_command(), "");
    }

    #[test]
    fn test_none_executor_does_not_support_backend() {
        let executor = NoneExecutor::new();
        assert!(!executor.supports_backend());
    }
}
