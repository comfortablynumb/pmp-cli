//! Integration tests for PMP CLI
//!
//! These tests verify CLI commands work correctly end-to-end.

use std::process::Command;

/// Get the path to the pmp binary
fn pmp_binary() -> std::path::PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop(); // Remove test executable name
    path.pop(); // Remove deps directory

    // In debug mode, binary is at target/debug/pmp
    path.push("pmp");

    if cfg!(windows) {
        path.set_extension("exe");
    }

    path
}

/// Run pmp command and return output
fn run_pmp(args: &[&str]) -> std::process::Output {
    Command::new(pmp_binary())
        .args(args)
        .output()
        .expect("Failed to execute pmp")
}

#[test]
fn test_pmp_version() {
    let output = run_pmp(&["--version"]);

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("pmp"));
}

#[test]
fn test_pmp_help() {
    let output = run_pmp(&["--help"]);

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("Commands:"));
}

#[test]
fn test_pmp_ci_help() {
    let output = run_pmp(&["ci", "--help"]);

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ci"));
    assert!(stdout.contains("generate"));
}

#[test]
fn test_pmp_generate_help() {
    let output = run_pmp(&["generate", "--help"]);

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("generate"));
}

#[test]
fn test_pmp_import_help() {
    let output = run_pmp(&["import", "--help"]);

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("import"));
    assert!(stdout.contains("from-export"));
    assert!(stdout.contains("manual"));
    assert!(stdout.contains("batch"));
}

#[test]
fn test_pmp_import_from_export_help() {
    let output = run_pmp(&["import", "from-export", "--help"]);

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("from-export"));
}

#[test]
fn test_pmp_import_manual_help() {
    let output = run_pmp(&["import", "manual", "--help"]);

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("manual"));
}

#[test]
fn test_pmp_import_batch_help() {
    let output = run_pmp(&["import", "batch", "--help"]);

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("batch"));
}

#[test]
fn test_pmp_infrastructure_help() {
    let output = run_pmp(&["infrastructure", "--help"]);

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("infrastructure"));
}

#[test]
fn test_pmp_project_help() {
    let output = run_pmp(&["project", "--help"]);

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("project"));
}

#[test]
fn test_pmp_search_help() {
    let output = run_pmp(&["search", "--help"]);

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("search"));
}

#[test]
fn test_pmp_template_help() {
    let output = run_pmp(&["template", "--help"]);

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("template"));
}

#[test]
fn test_pmp_ui_help() {
    let output = run_pmp(&["ui", "--help"]);

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ui"));
}

#[test]
fn test_pmp_invalid_command() {
    let output = run_pmp(&["invalid-command-that-does-not-exist"]);

    // Should fail with non-zero exit code
    assert!(!output.status.success());
}

#[test]
fn test_pmp_ci_generate_github_help() {
    let output = run_pmp(&["ci", "generate", "github", "--help"]);

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("github"));
}

#[test]
fn test_pmp_ci_generate_gitlab_help() {
    let output = run_pmp(&["ci", "generate", "gitlab", "--help"]);

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("gitlab"));
}

// ============================================================================
// End-to-end workflow tests with temp directories
// ============================================================================

mod workflow_tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper to verify no panic occurred in command output
    fn assert_no_panic(output: &std::process::Output, context: &str) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            !stderr.contains("panic") && !stderr.contains("RUST_BACKTRACE"),
            "{} panicked.\nstderr: {}",
            context,
            stderr
        );
    }

    #[test]
    fn test_no_infrastructure_yaml() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let output = Command::new(pmp_binary())
            .args(["project", "find"])
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute pmp");

        // Should fail gracefully without infrastructure.yaml (not panic)
        assert_no_panic(&output, "project find without infrastructure");

        // Should return error (not success) when no infrastructure found
        assert!(
            !output.status.success(),
            "Should fail when no infrastructure found"
        );
    }

    #[test]
    fn test_search_without_infrastructure() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let output = Command::new(pmp_binary())
            .args(["search", "--kind", "Test"])
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute pmp");

        assert_no_panic(&output, "search without infrastructure");
    }

    #[test]
    fn test_ci_generate_without_infrastructure() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let output = Command::new(pmp_binary())
            .args(["ci", "generate", "github", "--environment", "dev"])
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute pmp");

        assert_no_panic(&output, "ci generate without infrastructure");
    }

    #[test]
    fn test_generate_without_infrastructure() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let output = Command::new(pmp_binary())
            .args(["generate", "--project", "test", "--environment", "dev"])
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute pmp");

        assert_no_panic(&output, "generate without infrastructure");
    }

    #[test]
    fn test_import_help_works() {
        let output = run_pmp(&["import", "--help"]);
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("from-export"));
        assert!(stdout.contains("manual"));
        assert!(stdout.contains("batch"));
    }

    #[test]
    fn test_import_from_export_missing_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let output = Command::new(pmp_binary())
            .args(["import", "from-export", "nonexistent.json"])
            .current_dir(temp_dir.path())
            .output()
            .expect("Failed to execute pmp");

        assert_no_panic(&output, "import from-export with missing file");

        // Should fail because file doesn't exist
        assert!(!output.status.success());
    }

    #[test]
    fn test_import_manual_help() {
        let output = run_pmp(&["import", "manual", "--help"]);
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("RESOURCE_TYPE"));
    }

    #[test]
    fn test_template_scaffold_help() {
        let output = run_pmp(&["template", "scaffold", "--help"]);
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("scaffold"));
    }

    #[test]
    fn test_infrastructure_init_help() {
        let output = run_pmp(&["infrastructure", "init", "--help"]);
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("init"));
    }

    #[test]
    fn test_project_create_help() {
        let output = run_pmp(&["project", "create", "--help"]);
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("create"));
    }

    #[test]
    fn test_project_clone_help() {
        let output = run_pmp(&["project", "clone", "--help"]);
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("clone"));
    }

    #[test]
    fn test_project_apply_help() {
        let output = run_pmp(&["project", "apply", "--help"]);
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("apply"));
    }

    #[test]
    fn test_project_destroy_help() {
        let output = run_pmp(&["project", "destroy", "--help"]);
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("destroy"));
    }

    #[test]
    fn test_project_preview_help() {
        let output = run_pmp(&["project", "preview", "--help"]);
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("preview"));
    }

    #[test]
    fn test_project_graph_help() {
        let output = run_pmp(&["project", "graph", "--help"]);
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("graph"));
    }

    #[test]
    fn test_project_deps_help() {
        let output = run_pmp(&["project", "deps", "--help"]);
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("deps"));
    }

    #[test]
    fn test_project_state_help() {
        let output = run_pmp(&["project", "state", "--help"]);
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("state"));
    }

    #[test]
    fn test_project_env_help() {
        let output = run_pmp(&["project", "env", "--help"]);
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("env"));
    }

    #[test]
    fn test_project_drift_help() {
        let output = run_pmp(&["project", "drift", "--help"]);
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("drift"));
    }

    #[test]
    fn test_project_policy_help() {
        let output = run_pmp(&["project", "policy", "--help"]);
        assert!(output.status.success());

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("policy"));
    }
}
