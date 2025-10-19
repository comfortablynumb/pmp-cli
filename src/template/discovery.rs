use super::metadata::TemplateResource;
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
    #[allow(dead_code)]
    pub fn discover_templates() -> Result<Vec<TemplateInfo>> {
        Self::discover_templates_with_custom_paths(&[])
    }

    /// Find all templates in standard locations plus additional custom paths
    /// Checks:
    /// 1. Current directory's .pmp/templates
    /// 2. User's home directory ~/.pmp/templates
    /// 3. Custom paths provided
    pub fn discover_templates_with_custom_paths(custom_paths: &[&str]) -> Result<Vec<TemplateInfo>> {
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

        // Check custom paths
        for custom_path in custom_paths {
            let custom_path_buf = PathBuf::from(custom_path);

            if custom_path_buf.exists() {
                templates.extend(Self::load_templates_from_dir(&custom_path_buf)?);
            }
        }

        Ok(templates)
    }

    /// Load templates from a specific directory
    fn load_templates_from_dir(base_path: &Path) -> Result<Vec<TemplateInfo>> {
        let mut templates = Vec::new();

        // Walk through subdirectories looking for .pmp.template.yaml files
        for entry in WalkDir::new(base_path)
            .min_depth(1)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if path.is_file() && path.file_name() == Some(std::ffi::OsStr::new(".pmp.template.yaml"))
                && let Some(template_dir) = path.parent() {
                    match TemplateResource::from_file(path) {
                        Ok(resource) => {
                            templates.push(TemplateInfo {
                                resource,
                                path: template_dir.to_path_buf(),
                            });
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to load template from {:?}: {}", path, e);
                        }
                    }
                }
        }

        Ok(templates)
    }

    /// Group templates by resource kind
    pub fn group_by_resource_kind(templates: &[TemplateInfo]) -> HashMap<String, Vec<&TemplateInfo>> {
        let mut grouped: HashMap<String, Vec<&TemplateInfo>> = HashMap::new();

        for template in templates {
            let key = format!("{}/{}", template.resource.spec.resource.api_version, template.resource.spec.resource.kind);
            grouped
                .entry(key)
                .or_default()
                .push(template);
        }

        grouped
    }
}

/// Information about a discovered template
#[derive(Debug, Clone)]
pub struct TemplateInfo {
    /// Template resource from .pmp.yaml
    pub resource: TemplateResource,
    /// Path to the template directory
    pub path: PathBuf,
}

impl TemplateInfo {
    /// Get the full path to the src directory
    pub fn src_path(&self) -> PathBuf {
        self.resource.src_path(&self.path)
    }
}
