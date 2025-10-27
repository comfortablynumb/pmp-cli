use super::metadata::{TemplateResource, TemplatePackResource, PluginResource};
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Discovers and loads templates from the filesystem
pub struct TemplateDiscovery;

impl TemplateDiscovery {
    /// Find all template packs in standard locations
    /// Checks:
    /// 1. Current directory's .pmp/template-packs
    /// 2. User's home directory ~/.pmp/template-packs
    #[allow(dead_code)]
    pub fn discover_template_packs() -> Result<Vec<TemplatePackInfo>> {
        Self::discover_template_packs_with_custom_paths(&[])
    }

    /// Find all template packs in standard locations plus additional custom paths
    /// Checks:
    /// 1. Current directory's .pmp/template-packs
    /// 2. User's home directory ~/.pmp/template-packs
    /// 3. Custom paths provided
    pub fn discover_template_packs_with_custom_paths(custom_paths: &[&str]) -> Result<Vec<TemplatePackInfo>> {
        let mut template_packs = Vec::new();

        // Check current directory's .pmp/template-packs
        let current_templates_path = std::env::current_dir()?.join(".pmp").join("template-packs");

        if current_templates_path.exists() {
            template_packs.extend(Self::load_template_packs_from_dir(&current_templates_path)?);
        }

        // Check ~/.pmp/template-packs
        if let Some(home_dir) = dirs::home_dir() {
            let home_templates_path = home_dir.join(".pmp").join("template-packs");

            if home_templates_path.exists() {
                template_packs.extend(Self::load_template_packs_from_dir(&home_templates_path)?);
            }
        }

        // Check custom paths
        for custom_path in custom_paths {
            let custom_path_buf = PathBuf::from(custom_path);

            if custom_path_buf.exists() {
                template_packs.extend(Self::load_template_packs_from_dir(&custom_path_buf)?);
            }
        }

        Ok(template_packs)
    }

    /// Load template packs from a specific directory
    fn load_template_packs_from_dir(base_path: &Path) -> Result<Vec<TemplatePackInfo>> {
        let mut template_packs = Vec::new();

        // Walk through subdirectories looking for .pmp.template-pack.yaml files
        for entry in WalkDir::new(base_path)
            .min_depth(1)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if path.is_file() && path.file_name() == Some(std::ffi::OsStr::new(".pmp.template-pack.yaml"))
                && let Some(pack_dir) = path.parent() {
                    match TemplatePackResource::from_file(path) {
                        Ok(resource) => {
                            template_packs.push(TemplatePackInfo {
                                resource,
                                path: pack_dir.to_path_buf(),
                            });
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to load template pack from {:?}: {}", path, e);
                        }
                    }
                }
        }

        Ok(template_packs)
    }

    /// Discover templates within a template pack
    pub fn discover_templates_in_pack(pack_path: &Path) -> Result<Vec<TemplateInfo>> {
        let mut templates = Vec::new();
        let templates_dir = pack_path.join("templates");

        if !templates_dir.exists() {
            return Ok(templates);
        }

        // Walk through template subdirectories looking for .pmp.template.yaml files
        for entry in WalkDir::new(&templates_dir)
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

    /// Discover plugins within a template pack
    #[allow(dead_code)]
    pub fn discover_plugins_in_pack(pack_path: &Path) -> Result<Vec<PluginInfo>> {
        let mut plugins = Vec::new();
        let plugins_dir = pack_path.join("plugins");

        if !plugins_dir.exists() {
            return Ok(plugins);
        }

        // Walk through plugin subdirectories looking for .pmp.plugin.yaml files
        for entry in WalkDir::new(&plugins_dir)
            .min_depth(1)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if path.is_file() && path.file_name() == Some(std::ffi::OsStr::new(".pmp.plugin.yaml"))
                && let Some(plugin_dir) = path.parent() {
                    match PluginResource::from_file(path) {
                        Ok(resource) => {
                            plugins.push(PluginInfo {
                                resource,
                                path: plugin_dir.to_path_buf(),
                            });
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to load plugin from {:?}: {}", path, e);
                        }
                    }
                }
        }

        Ok(plugins)
    }

    /// Find all templates in standard locations (DEPRECATED - use discover_template_packs instead)
    /// Checks:
    /// 1. Current directory's .pmp/template-packs
    /// 2. User's home directory ~/.pmp/template-packs
    #[allow(dead_code)]
    pub fn discover_templates() -> Result<Vec<TemplateInfo>> {
        Self::discover_templates_with_custom_paths(&[])
    }

    /// Find all templates in standard locations plus additional custom paths
    /// Checks:
    /// 1. Current directory's .pmp/template-packs
    /// 2. User's home directory ~/.pmp/template-packs
    /// 3. Custom paths provided
    pub fn discover_templates_with_custom_paths(custom_paths: &[&str]) -> Result<Vec<TemplateInfo>> {
        let mut templates = Vec::new();

        // Check current directory's .pmp/template-packs
        let current_templates_path = std::env::current_dir()?.join(".pmp").join("template-packs");

        if current_templates_path.exists() {
            templates.extend(Self::load_templates_from_dir(&current_templates_path)?);
        }

        // Check ~/.pmp/template-packs
        if let Some(home_dir) = dirs::home_dir() {
            let home_templates_path = home_dir.join(".pmp").join("template-packs");

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
    #[allow(dead_code)]
    pub fn group_by_resource_kind(templates: &[TemplateInfo]) -> HashMap<String, Vec<&TemplateInfo>> {
        let mut grouped: HashMap<String, Vec<&TemplateInfo>> = HashMap::new();

        for template in templates {
            let key = format!("{}/{}", template.resource.spec.api_version, template.resource.spec.kind);
            grouped
                .entry(key)
                .or_default()
                .push(template);
        }

        grouped
    }
}

/// Information about a discovered template pack
#[derive(Debug, Clone)]
pub struct TemplatePackInfo {
    /// Template pack resource from .pmp.template-pack.yaml
    pub resource: TemplatePackResource,
    /// Path to the template pack directory
    pub path: PathBuf,
}

impl TemplatePackInfo {
    /// Get the path to the templates directory
    #[allow(dead_code)]
    pub fn templates_dir(&self) -> PathBuf {
        self.resource.templates_dir(&self.path)
    }
}

/// Information about a discovered template
#[derive(Debug, Clone)]
pub struct TemplateInfo {
    /// Template resource from .pmp.template.yaml
    pub resource: TemplateResource,
    /// Path to the template directory
    pub path: PathBuf,
}

/// Information about a discovered plugin
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PluginInfo {
    /// Plugin resource from .pmp.plugin.yaml
    pub resource: PluginResource,
    /// Path to the plugin directory
    pub path: PathBuf,
}
