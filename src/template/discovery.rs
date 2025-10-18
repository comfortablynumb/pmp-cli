use super::metadata::TemplateMetadata;
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Discovers and loads templates from the filesystem
pub struct TemplateDiscovery;

impl TemplateDiscovery {
    /// Find all templates in standard locations
    /// Checks:
    /// 1. Current directory's .pmp/templates
    /// 2. User's home directory ~/.pmp/templates
    pub fn discover_templates() -> Result<Vec<TemplateInfo>> {
        let mut templates = Vec::new();

        // Check current directory's .pmp/templates
        let current_templates_path = std::env::current_dir()?.join(".pmp").join("templates");

        if current_templates_path.exists() {
            templates.extend(Self::load_templates_from_dir(&current_templates_path)?);
        }

        // Check ~/.pmp/templates
        if let Some(home_dir) = dirs::home_dir() {
            let home_templates_path = home_dir.join(".pmp").join("templates");

            if home_templates_path.exists() {
                templates.extend(Self::load_templates_from_dir(&home_templates_path)?);
            }
        }

        Ok(templates)
    }

    /// Load templates from a specific directory
    fn load_templates_from_dir(base_path: &Path) -> Result<Vec<TemplateInfo>> {
        let mut templates = Vec::new();

        // Walk through subdirectories looking for .pmp.yaml files
        for entry in WalkDir::new(base_path)
            .min_depth(1)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if path.is_file() && path.file_name() == Some(std::ffi::OsStr::new(".pmp.yaml")) {
                if let Some(template_dir) = path.parent() {
                    match TemplateMetadata::from_file(path) {
                        Ok(metadata) => {
                            templates.push(TemplateInfo {
                                metadata,
                                path: template_dir.to_path_buf(),
                            });
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to load template from {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        Ok(templates)
    }

    /// Group templates by category
    pub fn group_by_category(templates: &[TemplateInfo]) -> HashMap<String, Vec<&TemplateInfo>> {
        let mut grouped: HashMap<String, Vec<&TemplateInfo>> = HashMap::new();

        for template in templates {
            for category in &template.metadata.categories {
                grouped
                    .entry(category.clone())
                    .or_default()
                    .push(template);
            }
        }

        grouped
    }
}

/// Information about a discovered template
#[derive(Debug, Clone)]
pub struct TemplateInfo {
    /// Template metadata from .pmp.yaml
    pub metadata: TemplateMetadata,
    /// Path to the template directory
    pub path: PathBuf,
}

impl TemplateInfo {
    /// Get the full path to the schema.json file
    pub fn schema_path(&self) -> PathBuf {
        self.path.join(&self.metadata.schema_path)
    }

    /// Get the full path to the src directory
    pub fn src_path(&self) -> PathBuf {
        self.path.join(&self.metadata.src_path)
    }
}
