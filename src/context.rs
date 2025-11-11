use crate::executor::{DefaultExecutorRegistry, ExecutorRegistry};
use crate::traits::{
    CommandExecutor, FileSystem, InquireUserInput, Output, RealCommandExecutor, RealFileSystem,
    TerminalOutput, UserInput,
};
#[cfg(test)]
use crate::traits::{MockCommandExecutor, MockFileSystem, MockOutput, MockUserInput};
use std::sync::Arc;

/// Application context that holds all dependencies for dependency injection
pub struct Context {
    pub fs: Arc<dyn FileSystem>,
    pub input: Arc<dyn UserInput>,
    pub output: Arc<dyn Output>,
    #[allow(dead_code)]
    pub command: Arc<dyn CommandExecutor>,
    #[allow(dead_code)]
    pub executor_registry: Arc<dyn ExecutorRegistry>,
}

impl Context {
    /// Create a new context with real implementations (for production use)
    pub fn new() -> Self {
        Self {
            fs: Arc::new(RealFileSystem),
            input: Arc::new(InquireUserInput),
            output: Arc::new(TerminalOutput),
            command: Arc::new(RealCommandExecutor::new()),
            executor_registry: Arc::new(DefaultExecutorRegistry::with_defaults()),
        }
    }

    /// Create a new context with mock implementations (for testing)
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn test() -> Self {
        Self {
            fs: Arc::new(MockFileSystem::new()),
            input: Arc::new(MockUserInput::new()),
            output: Arc::new(MockOutput::new()),
            command: Arc::new(MockCommandExecutor::new()),
            executor_registry: Arc::new(DefaultExecutorRegistry::new()),
        }
    }

    /// Create a test context with specific mock implementations
    #[cfg(test)]
    #[allow(dead_code)]
    pub fn test_with(
        fs: Arc<dyn FileSystem>,
        input: Arc<dyn UserInput>,
        output: Arc<dyn Output>,
        command: Arc<dyn CommandExecutor>,
        executor_registry: Arc<dyn ExecutorRegistry>,
    ) -> Self {
        Self {
            fs,
            input,
            output,
            command,
            executor_registry,
        }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Context {
    fn clone(&self) -> Self {
        Self {
            fs: Arc::clone(&self.fs),
            input: Arc::clone(&self.input),
            output: Arc::clone(&self.output),
            command: Arc::clone(&self.command),
            executor_registry: Arc::clone(&self.executor_registry),
        }
    }
}
