//! Parallel execution support for PMP commands.
//!
//! This module provides functionality to execute multiple projects concurrently
//! within the same dependency level.

use crate::collection::DependencyNode;
use crate::template::metadata::{FailureBehavior, ParallelConfig};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Result of executing a single project
#[derive(Debug, Clone)]
pub struct ProjectResult {
    /// The node that was executed
    pub node: DependencyNode,

    /// Whether the execution succeeded
    pub success: bool,

    /// Error message if execution failed
    pub error_message: Option<String>,
}

impl ProjectResult {
    /// Create a successful result
    pub fn success(node: DependencyNode) -> Self {
        Self {
            node,
            success: true,
            error_message: None,
        }
    }

    /// Create a failed result
    pub fn failure(node: DependencyNode, error: String) -> Self {
        Self {
            node,
            success: false,
            error_message: Some(error),
        }
    }
}

/// Execute projects in parallel within a single level
///
/// # Arguments
/// * `nodes` - Projects to execute (all at the same dependency level)
/// * `config` - Parallel execution configuration
/// * `executor_fn` - Function to execute each project (must be thread-safe)
///
/// # Returns
/// Vector of results for each project
pub async fn execute_level_parallel<F>(
    nodes: Vec<DependencyNode>,
    config: &ParallelConfig,
    executor_fn: F,
) -> Vec<ProjectResult>
where
    F: Fn(DependencyNode) -> Result<(), String> + Send + Sync + 'static,
{
    let max_concurrent = config.max.max(1);
    let semaphore = Arc::new(Semaphore::new(max_concurrent));
    let executor_fn = Arc::new(executor_fn);

    let mut handles = Vec::new();

    for node in nodes {
        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let executor_fn = executor_fn.clone();
        let node_clone = node.clone();

        let handle = tokio::spawn(async move {
            // Execute the project in a blocking task (since tofu/terraform are blocking)
            let result = tokio::task::spawn_blocking(move || executor_fn(node_clone))
                .await
                .unwrap_or_else(|e| Err(format!("Task panicked: {}", e)));

            drop(permit);

            match result {
                Ok(()) => ProjectResult::success(node),
                Err(e) => ProjectResult::failure(node, e),
            }
        });

        handles.push(handle);
    }

    // Collect all results
    let mut results = Vec::new();
    for handle in handles {
        if let Ok(result) = handle.await {
            results.push(result);
        }
    }

    results
}

/// Display results for a completed level
pub fn display_level_results(ctx: &crate::context::Context, results: &[ProjectResult]) {
    for result in results {
        if result.success {
            ctx.output.success(&format!(
                "  {} ({}) - completed",
                result.node.project_name, result.node.environment_name
            ));
        } else {
            ctx.output.error(&format!(
                "  {} ({}) - FAILED: {}",
                result.node.project_name,
                result.node.environment_name,
                result.error_message.as_deref().unwrap_or("Unknown error")
            ));
        }
    }
}

/// Check if execution should continue based on failure behavior and results
pub fn should_continue_after_failures(
    failure_behavior: &FailureBehavior,
    level_failures: usize,
) -> ContinueDecision {
    if level_failures == 0 {
        return ContinueDecision::Continue;
    }

    match failure_behavior {
        FailureBehavior::Stop => ContinueDecision::StopNow,
        FailureBehavior::FinishLevel => ContinueDecision::StopAfterLevel,
        FailureBehavior::Continue => ContinueDecision::Continue,
    }
}

/// Decision on whether to continue execution
#[derive(Debug, Clone, PartialEq)]
pub enum ContinueDecision {
    /// Continue with remaining levels
    Continue,
    /// Stop execution immediately
    StopNow,
    /// Current level finished, stop before next level
    StopAfterLevel,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_result_success() {
        let node = DependencyNode {
            project_name: "test".to_string(),
            environment_name: "dev".to_string(),
            environment_path: std::path::PathBuf::from("/test"),
        };

        let result = ProjectResult::success(node.clone());
        assert!(result.success);
        assert!(result.error_message.is_none());
        assert_eq!(result.node.project_name, "test");
    }

    #[test]
    fn test_project_result_failure() {
        let node = DependencyNode {
            project_name: "test".to_string(),
            environment_name: "dev".to_string(),
            environment_path: std::path::PathBuf::from("/test"),
        };

        let result = ProjectResult::failure(node.clone(), "Something went wrong".to_string());
        assert!(!result.success);
        assert_eq!(
            result.error_message,
            Some("Something went wrong".to_string())
        );
    }

    #[test]
    fn test_should_continue_no_failures() {
        assert_eq!(
            should_continue_after_failures(&FailureBehavior::Stop, 0),
            ContinueDecision::Continue
        );
        assert_eq!(
            should_continue_after_failures(&FailureBehavior::Continue, 0),
            ContinueDecision::Continue
        );
    }

    #[test]
    fn test_should_continue_with_failures() {
        assert_eq!(
            should_continue_after_failures(&FailureBehavior::Stop, 1),
            ContinueDecision::StopNow
        );
        assert_eq!(
            should_continue_after_failures(&FailureBehavior::Continue, 1),
            ContinueDecision::Continue
        );
        assert_eq!(
            should_continue_after_failures(&FailureBehavior::FinishLevel, 1),
            ContinueDecision::StopAfterLevel
        );
    }
}
