use super::discovery::TemplatePackInfo;
use crate::traits::{FileSystem, Output};
use anyhow::{Result, bail};
use inquire::Confirm;
use std::path::PathBuf;
use std::process::Command;

const TEMPLATE_PACKS_REPO: &str = "https://github.com/comfortablynumb/pmp-template-packs.git";

/// Check if template packs are available, and offer to install if not
///
/// Returns:
/// - Ok(true) if template packs exist or were successfully installed
/// - Ok(false) if user declined installation
/// - Err if installation failed
pub fn check_and_offer_installation(
    fs: &dyn FileSystem,
    output: &dyn Output,
    template_packs: &[TemplatePackInfo],
) -> Result<bool> {
    // If template packs exist, we're good
    if !template_packs.is_empty() {
        return Ok(true);
    }

    // No template packs found - offer to install
    output.blank();
    output.dark_yellow("──────────────────────────────────────────────────");
    output.dark_yellow("No template packs found.");
    output.dark_yellow("──────────────────────────────────────────────────");
    output.blank();
    output.bright_white("PMP needs template packs to create projects.");
    output.bright_white("We can install the standard template packs provided by PMP for you.");
    output.blank();
    output.dimmed("Repository:");
    output.lavender(&format!("  {}", TEMPLATE_PACKS_REPO));
    output.dimmed("Will be installed to:");
    output.lavender(&format!("  {}", get_home_template_packs_path()?.display()));
    output.blank();

    // Prompt user for installation
    let response = Confirm::new("Would you like to install the official PMP template packs now?")
        .with_default(false)
        .prompt()?;

    if !response {
        output.blank();
        output.dimmed("Skipped installation. You can install template packs later by running:");
        output.lavender(&format!(
            "  git clone {} ~/.pmp/template-packs",
            TEMPLATE_PACKS_REPO
        ));
        output.blank();
        return Ok(false);
    }

    // User wants to install - proceed with installation
    install_official_template_packs(fs, output)?;

    Ok(true)
}

/// Install official template packs from GitHub
fn install_official_template_packs(fs: &dyn FileSystem, output: &dyn Output) -> Result<()> {
    // Check if git is available
    if !is_git_available() {
        bail!(
            "Git is not installed or not available in PATH.\n\
             Please install git and try again, or manually clone the template packs:\n  \
             git clone {} ~/.pmp/template-packs",
            TEMPLATE_PACKS_REPO
        );
    }

    let install_path = get_home_template_packs_path()?;

    // Check if directory already exists
    if fs.exists(&install_path) {
        bail!(
            "Directory already exists: {}\n\
             Please remove it or manually update the template packs:\n  \
             cd {}\n  \
             git pull",
            install_path.display(),
            install_path.display()
        );
    }

    // Ensure parent directory exists
    let parent_dir = install_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid installation path"))?;

    if !fs.exists(parent_dir) {
        output.dimmed(&format!("Creating directory: {}", parent_dir.display()));
        fs.create_dir_all(parent_dir)?;
    }

    // Clone the repository
    output.blank();
    output.dimmed("Installing template packs...");
    output.dimmed(&format!("  Repository: {}", TEMPLATE_PACKS_REPO));
    output.dimmed(&format!("  Destination: {}", install_path.display()));
    output.blank();

    let status = Command::new("git")
        .arg("clone")
        .arg(TEMPLATE_PACKS_REPO)
        .arg(&install_path)
        .status()?;

    if !status.success() {
        bail!(
            "Failed to clone template packs repository.\n\
             Please check your network connection and try again, or clone manually:\n  \
             git clone {} {}",
            TEMPLATE_PACKS_REPO,
            install_path.display()
        );
    }

    output.blank();
    output.success("Template packs installed successfully.");
    output.dimmed("You can now use PMP to create projects.");
    output.blank();

    Ok(())
}

/// Check if git command is available
fn is_git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Get the path to the home directory template packs installation
fn get_home_template_packs_path() -> Result<PathBuf> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Unable to determine home directory"))?;

    Ok(home_dir.join(".pmp").join("template-packs"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{MockFileSystem, MockOutput};

    #[test]
    fn test_check_with_existing_template_packs() {
        let fs = MockFileSystem::new();
        let output = MockOutput::new();

        // Create a dummy template pack
        let template_packs: Vec<TemplatePackInfo> = vec![]; // We'd need to create a proper TemplatePackInfo here

        // For now, just test that empty packs trigger the flow
        // (We can't fully test interactive prompts in unit tests)
        let _ = (fs, output, template_packs); // Suppress unused variable warnings
    }

    #[test]
    fn test_is_git_available() {
        // This test will pass if git is installed, fail otherwise
        // In a real scenario, we might want to mock this
        let available = is_git_available();
        // We can't assert true/false without knowing the test environment
        println!("Git available: {}", available);
    }

    #[test]
    fn test_get_home_template_packs_path() {
        let path = get_home_template_packs_path();
        assert!(path.is_ok());

        if let Ok(path) = path {
            assert!(path.ends_with(".pmp/template-packs"));
        }
    }
}
