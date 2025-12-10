use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Analyzes a Terraform/OpenTofu project directory
pub struct ProjectAnalyzer {
    source_path: PathBuf,
}

impl ProjectAnalyzer {
    pub fn new(source_path: &Path) -> Self {
        Self {
            source_path: source_path.to_path_buf(),
        }
    }

    /// Analyze the project directory
    pub fn analyze(&self) -> Result<ProjectAnalysis> {
        let mut analysis = ProjectAnalysis {
            terraform_files: Vec::new(),
            has_state: false,
            resources: None,
            providers: Vec::new(),
            variables: Vec::new(),
        };

        // Find all .tf files
        analysis.terraform_files = self.find_terraform_files()?;

        // Check for state file
        let state_path = self.source_path.join("terraform.tfstate");

        if state_path.exists() {
            analysis.has_state = true;

            // Parse state file
            if let Ok(state_analysis) = self.analyze_state(&state_path) {
                analysis.resources = Some(state_analysis.resources);
                analysis.providers = state_analysis.providers;
            }
        }

        // Parse Terraform files for variables
        analysis.variables = self.extract_variables(&analysis.terraform_files)?;

        Ok(analysis)
    }

    /// Find all .tf files in the directory
    fn find_terraform_files(&self) -> Result<Vec<PathBuf>> {
        let mut tf_files = Vec::new();

        for entry in fs::read_dir(&self.source_path)
            .with_context(|| format!("Failed to read directory: {}", self.source_path.display()))?
        {
            let entry = entry?;
            let path = entry.path();

            if path.is_file()
                && let Some(ext) = path.extension()
                && ext == "tf"
            {
                tf_files.push(path);
            }
        }

        Ok(tf_files)
    }

    /// Analyze state file
    fn analyze_state(&self, state_path: &Path) -> Result<StateAnalysis> {
        use crate::import::state_parser::StateParser;

        let parser = StateParser::new(state_path);
        parser.parse()
    }

    /// Extract variables from Terraform files
    fn extract_variables(&self, _tf_files: &[PathBuf]) -> Result<Vec<VariableInfo>> {
        // For now, return empty
        // TODO: Implement HCL parsing to extract variables
        Ok(Vec::new())
    }
}

/// Project analysis result
pub struct ProjectAnalysis {
    pub terraform_files: Vec<PathBuf>,
    pub has_state: bool,
    pub resources: Option<Vec<ResourceInfo>>,
    pub providers: Vec<ProviderInfo>,
    pub variables: Vec<VariableInfo>,
}

/// State analysis result
pub struct StateAnalysis {
    pub resources: Vec<ResourceInfo>,
    pub providers: Vec<ProviderInfo>,
    pub outputs: HashMap<String, Value>,
}

/// Resource information from state
#[derive(Debug, Clone)]
pub struct ResourceInfo {
    pub address: String,
    pub resource_type: String,
    pub provider: String,
    pub attributes: HashMap<String, Value>,
}

/// Provider information
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub name: String,
    pub version: Option<String>,
}

/// Variable information
#[derive(Debug, Clone)]
pub struct VariableInfo {
    pub name: String,
    pub var_type: Option<String>,
    pub description: Option<String>,
    pub default: Option<String>,
}
