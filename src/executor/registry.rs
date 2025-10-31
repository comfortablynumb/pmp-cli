use super::Executor;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Trait for executor registry that manages available executors
#[allow(dead_code)]
pub trait ExecutorRegistry: Send + Sync {
    /// Register an executor with the given name
    fn register(&mut self, name: String, executor: Box<dyn Executor>);

    /// Get an executor by name
    fn get(&self, name: &str) -> Result<Arc<dyn Executor>>;

    /// Check if an executor is registered
    fn has(&self, name: &str) -> bool;

    /// List all registered executor names
    fn list(&self) -> Vec<String>;
}

/// Default implementation of executor registry using a HashMap
#[allow(dead_code)]
pub struct DefaultExecutorRegistry {
    executors: RwLock<HashMap<String, Arc<dyn Executor>>>,
}

impl DefaultExecutorRegistry {
    /// Create a new empty registry
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            executors: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new registry with default executors (OpenTofu)
    #[allow(dead_code)]
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();
        registry.register(
            "opentofu".to_string(),
            Box::new(crate::executor::OpenTofuExecutor::new()),
        );
        registry
    }
}

impl ExecutorRegistry for DefaultExecutorRegistry {
    fn register(&mut self, name: String, executor: Box<dyn Executor>) {
        let mut executors = self.executors.write().unwrap();
        executors.insert(name, Arc::from(executor));
    }

    fn get(&self, name: &str) -> Result<Arc<dyn Executor>> {
        let executors = self.executors.read().unwrap();
        executors
            .get(name)
            .cloned()
            .with_context(|| format!("Unknown executor: {}", name))
    }

    fn has(&self, name: &str) -> bool {
        let executors = self.executors.read().unwrap();
        executors.contains_key(name)
    }

    fn list(&self) -> Vec<String> {
        let executors = self.executors.read().unwrap();
        executors.keys().cloned().collect()
    }
}

impl Default for DefaultExecutorRegistry {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::ExecutorConfig;
    use std::process::Output;

    // Mock executor for testing
    struct MockExecutor {
        name: String,
    }

    impl MockExecutor {
        fn new(name: impl Into<String>) -> Self {
            Self {
                name: name.into(),
            }
        }
    }

    impl Executor for MockExecutor {
        fn check_installed(&self) -> Result<bool> {
            Ok(true)
        }

        fn init(&self, working_dir: &str) -> Result<Output> {
            // Use a real command to get a proper ExitStatus for testing
            std::process::Command::new("echo")
                .arg("test")
                .current_dir(working_dir)
                .output()
                .context("Failed to execute test command")
        }

        fn plan(&self, _config: &ExecutorConfig, _working_dir: &str) -> Result<()> {
            Ok(())
        }

        fn apply(&self, _config: &ExecutorConfig, _working_dir: &str) -> Result<()> {
            Ok(())
        }

        fn get_name(&self) -> &str {
            &self.name
        }

        fn default_plan_command(&self) -> &str {
            "mock plan"
        }

        fn default_apply_command(&self) -> &str {
            "mock apply"
        }
    }

    #[test]
    fn test_register_and_get_executor() {
        let mut registry = DefaultExecutorRegistry::new();
        registry.register("test".to_string(), Box::new(MockExecutor::new("test")));

        let executor = registry.get("test").unwrap();
        assert_eq!(executor.get_name(), "test");
    }

    #[test]
    fn test_get_unknown_executor() {
        let registry = DefaultExecutorRegistry::new();
        let result = registry.get("unknown");
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(e.to_string().contains("Unknown executor"));
        }
    }

    #[test]
    fn test_has_executor() {
        let mut registry = DefaultExecutorRegistry::new();
        assert!(!registry.has("test"));

        registry.register("test".to_string(), Box::new(MockExecutor::new("test")));
        assert!(registry.has("test"));
    }

    #[test]
    fn test_list_executors() {
        let mut registry = DefaultExecutorRegistry::new();
        registry.register("test1".to_string(), Box::new(MockExecutor::new("test1")));
        registry.register("test2".to_string(), Box::new(MockExecutor::new("test2")));

        let mut names = registry.list();
        names.sort();
        assert_eq!(names, vec!["test1", "test2"]);
    }

    #[test]
    fn test_with_defaults_includes_opentofu() {
        let registry = DefaultExecutorRegistry::with_defaults();
        assert!(registry.has("opentofu"));

        let executor = registry.get("opentofu").unwrap();
        assert_eq!(executor.get_name(), "opentofu");
    }

    #[test]
    fn test_default_includes_opentofu() {
        let registry = DefaultExecutorRegistry::default();
        assert!(registry.has("opentofu"));
    }
}
