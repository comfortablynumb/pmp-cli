use anyhow::Result;
use std::path::Path;
use std::process::{Command, Output, Stdio};

/// Trait for executing system commands, allowing for mocking in tests
pub trait CommandExecutor: Send + Sync {
    /// Execute a command with arguments and return output
    #[allow(dead_code)]
    fn execute(&self, command: &str, args: &[&str], working_dir: &Path) -> Result<Output>;

    /// Execute a command interactively (inherits stdin/stdout/stderr)
    #[allow(dead_code)]
    fn execute_interactive(&self, command: &str, args: &[&str], working_dir: &Path) -> Result<i32>;

    /// Execute a shell command (uses cmd on Windows, sh on Unix)
    #[allow(dead_code)]
    fn execute_shell(&self, command: &str, working_dir: &Path) -> Result<Output>;
}

/// Real command executor using std::process::Command
pub struct RealCommandExecutor;

impl RealCommandExecutor {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RealCommandExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandExecutor for RealCommandExecutor {
    fn execute(&self, command: &str, args: &[&str], working_dir: &Path) -> Result<Output> {
        let output = Command::new(command)
            .args(args)
            .current_dir(working_dir)
            .output()?;

        Ok(output)
    }

    fn execute_interactive(&self, command: &str, args: &[&str], working_dir: &Path) -> Result<i32> {
        let mut child = Command::new(command)
            .args(args)
            .current_dir(working_dir)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;

        let status = child.wait()?;
        Ok(status.code().unwrap_or(-1))
    }

    fn execute_shell(&self, command: &str, working_dir: &Path) -> Result<Output> {
        #[cfg(target_os = "windows")]
        let output = Command::new("cmd")
            .args(&["/C", command])
            .current_dir(working_dir)
            .output()?;

        #[cfg(not(target_os = "windows"))]
        let output = Command::new("sh")
            .args(&["-c", command])
            .current_dir(working_dir)
            .output()?;

        Ok(output)
    }
}

/// Mock command executor for testing
#[cfg(test)]
pub struct MockCommandExecutor {
    /// Pre-configured outputs for commands
    outputs: std::sync::Mutex<Vec<MockCommandResult>>,
}

#[cfg(test)]
#[derive(Clone, Debug)]
pub struct MockCommandResult {
    pub command: String,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[cfg(test)]
impl MockCommandExecutor {
    pub fn new() -> Self {
        Self {
            outputs: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn with_outputs(outputs: Vec<MockCommandResult>) -> Self {
        Self {
            outputs: std::sync::Mutex::new(outputs),
        }
    }

    pub fn add_output(&self, output: MockCommandResult) {
        let mut outputs = self.outputs.lock().unwrap();
        outputs.push(output);
    }
}

#[cfg(test)]
impl Default for MockCommandExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl CommandExecutor for MockCommandExecutor {
    fn execute(&self, command: &str, _args: &[&str], _working_dir: &Path) -> Result<Output> {
        let mut outputs = self.outputs.lock().unwrap();

        if let Some(result) = outputs.iter().position(|r| r.command == command) {
            let mock_result = outputs.remove(result);
            return Ok(Output {
                status: create_exit_status(mock_result.exit_code),
                stdout: mock_result.stdout.into_bytes(),
                stderr: mock_result.stderr.into_bytes(),
            });
        }

        // Default: successful empty output
        Ok(Output {
            status: create_exit_status(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }

    fn execute_interactive(&self, command: &str, _args: &[&str], _working_dir: &Path) -> Result<i32> {
        let mut outputs = self.outputs.lock().unwrap();

        if let Some(result) = outputs.iter().position(|r| r.command == command) {
            let mock_result = outputs.remove(result);
            return Ok(mock_result.exit_code);
        }

        // Default: success
        Ok(0)
    }

    fn execute_shell(&self, command: &str, _working_dir: &Path) -> Result<Output> {
        let mut outputs = self.outputs.lock().unwrap();

        if let Some(result) = outputs.iter().position(|r| r.command == command) {
            let mock_result = outputs.remove(result);
            return Ok(Output {
                status: create_exit_status(mock_result.exit_code),
                stdout: mock_result.stdout.into_bytes(),
                stderr: mock_result.stderr.into_bytes(),
            });
        }

        // Default: successful empty output
        Ok(Output {
            status: create_exit_status(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
}

#[cfg(test)]
fn create_exit_status(code: i32) -> std::process::ExitStatus {
    // Create a dummy process to get an ExitStatus
    // This is a workaround since ExitStatus can't be constructed directly
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code)
    }

    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(code as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_mock_executor_returns_configured_output() {
        let executor = MockCommandExecutor::with_outputs(vec![
            MockCommandResult {
                command: "test".to_string(),
                exit_code: 0,
                stdout: "success".to_string(),
                stderr: String::new(),
            },
        ]);

        let output = executor.execute("test", &[], &PathBuf::from(".")).unwrap();
        assert_eq!(String::from_utf8_lossy(&output.stdout), "success");
    }

    #[test]
    fn test_mock_executor_default_success() {
        let executor = MockCommandExecutor::new();
        let output = executor.execute("unknown", &[], &PathBuf::from(".")).unwrap();
        assert!(output.status.success());
    }

    #[test]
    fn test_mock_executor_interactive() {
        let executor = MockCommandExecutor::with_outputs(vec![
            MockCommandResult {
                command: "test".to_string(),
                exit_code: 42,
                stdout: String::new(),
                stderr: String::new(),
            },
        ]);

        let code = executor.execute_interactive("test", &[], &PathBuf::from(".")).unwrap();
        assert_eq!(code, 42);
    }
}
