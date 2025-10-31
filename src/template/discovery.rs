use super::metadata::{TemplateResource, TemplatePackResource, PluginResource};
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Discovers and loads templates from the filesystem
pub struct TemplateDiscovery;

impl TemplateDiscovery {
    /// Find all template packs in standard locations
    /// Checks:
    /// 1. Current directory's .pmp/template-packs
    /// 2. User's home directory ~/.pmp/template-packs
    #[allow(dead_code)]
    pub fn discover_template_packs(fs: &dyn crate::traits::FileSystem, output: &dyn crate::traits::Output) -> Result<Vec<TemplatePackInfo>> {
        Self::discover_template_packs_with_custom_paths(fs, output, &[])
    }

    /// Find all template packs in standard locations plus additional custom paths
    /// Checks:
    /// 1. Current directory's .pmp/template-packs
    /// 2. User's home directory ~/.pmp/template-packs
    /// 3. Custom paths provided
    pub fn discover_template_packs_with_custom_paths(fs: &dyn crate::traits::FileSystem, output: &dyn crate::traits::Output, custom_paths: &[&str]) -> Result<Vec<TemplatePackInfo>> {
        let mut template_packs = Vec::new();

        // Check current directory's .pmp/template-packs
        let current_templates_path = std::env::current_dir()?.join(".pmp").join("template-packs");

        if fs.exists(&current_templates_path) {
            template_packs.extend(Self::load_template_packs_from_dir(fs, output, &current_templates_path)?);
        }

        // Check ~/.pmp/template-packs
        if let Some(home_dir) = dirs::home_dir() {
            let home_templates_path = home_dir.join(".pmp").join("template-packs");

            if fs.exists(&home_templates_path) {
                template_packs.extend(Self::load_template_packs_from_dir(fs, output, &home_templates_path)?);
            }
        }

        // Check custom paths
        for custom_path in custom_paths {
            let custom_path_buf = PathBuf::from(custom_path);

            if fs.exists(&custom_path_buf) {
                template_packs.extend(Self::load_template_packs_from_dir(fs, output, &custom_path_buf)?);
            }
        }

        Ok(template_packs)
    }

    /// Load template packs from a specific directory
    fn load_template_packs_from_dir(fs: &dyn crate::traits::FileSystem, output: &dyn crate::traits::Output, base_path: &Path) -> Result<Vec<TemplatePackInfo>> {
        let mut template_packs = Vec::new();

        // Walk through subdirectories looking for .pmp.template-pack.yaml files
        let entries = fs.walk_dir(base_path, 2)?;

        for entry_path in entries {
            if fs.is_file(&entry_path) && entry_path.file_name() == Some(std::ffi::OsStr::new(".pmp.template-pack.yaml"))
                && let Some(pack_dir) = entry_path.parent() {
                    match TemplatePackResource::from_file(fs, &entry_path) {
                        Ok(resource) => {
                            template_packs.push(TemplatePackInfo {
                                resource,
                                path: pack_dir.to_path_buf(),
                            });
                        }
                        Err(e) => {
                            output.warning(&format!("Failed to load template pack from {:?}: {}", entry_path, e));
                        }
                    }
                }
        }

        Ok(template_packs)
    }

    /// Discover templates within a template pack
    pub fn discover_templates_in_pack(fs: &dyn crate::traits::FileSystem, output: &dyn crate::traits::Output, pack_path: &Path) -> Result<Vec<TemplateInfo>> {
        let mut templates = Vec::new();
        let templates_dir = pack_path.join("templates");

        if !fs.exists(&templates_dir) {
            return Ok(templates);
        }

        // Walk through template subdirectories looking for .pmp.template.yaml files
        let entries = fs.walk_dir(&templates_dir, 2)?;

        for entry_path in entries {
            if fs.is_file(&entry_path) && entry_path.file_name() == Some(std::ffi::OsStr::new(".pmp.template.yaml"))
                && let Some(template_dir) = entry_path.parent() {
                    match TemplateResource::from_file(fs, &entry_path) {
                        Ok(resource) => {
                            templates.push(TemplateInfo {
                                resource,
                                path: template_dir.to_path_buf(),
                            });
                        }
                        Err(e) => {
                            output.warning(&format!("Failed to load template from {:?}: {}", entry_path, e));
                        }
                    }
                }
        }

        Ok(templates)
    }

    /// Discover plugins within a template pack
    #[allow(dead_code)]
    pub fn discover_plugins_in_pack(fs: &dyn crate::traits::FileSystem, output: &dyn crate::traits::Output, pack_path: &Path, template_pack_name: &str) -> Result<Vec<PluginInfo>> {
        let mut plugins = Vec::new();
        let plugins_dir = pack_path.join("plugins");

        if !fs.exists(&plugins_dir) {
            return Ok(plugins);
        }

        // Walk through plugin subdirectories looking for .pmp.plugin.yaml files
        let entries = fs.walk_dir(&plugins_dir, 2)?;

        for entry_path in entries {
            if fs.is_file(&entry_path) && entry_path.file_name() == Some(std::ffi::OsStr::new(".pmp.plugin.yaml"))
                && let Some(plugin_dir) = entry_path.parent() {
                    match PluginResource::from_file(fs, &entry_path) {
                        Ok(resource) => {
                            plugins.push(PluginInfo {
                                resource,
                                path: plugin_dir.to_path_buf(),
                                template_pack_name: template_pack_name.to_string(),
                            });
                        }
                        Err(e) => {
                            output.warning(&format!("Failed to load plugin from {:?}: {}", entry_path, e));
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
    pub fn discover_templates(fs: &dyn crate::traits::FileSystem, output: &dyn crate::traits::Output) -> Result<Vec<TemplateInfo>> {
        Self::discover_templates_with_custom_paths(fs, output, &[])
    }

    /// Find all templates in standard locations plus additional custom paths
    /// Checks:
    /// 1. Current directory's .pmp/template-packs
    /// 2. User's home directory ~/.pmp/template-packs
    /// 3. Custom paths provided
    pub fn discover_templates_with_custom_paths(fs: &dyn crate::traits::FileSystem, output: &dyn crate::traits::Output, custom_paths: &[&str]) -> Result<Vec<TemplateInfo>> {
        let mut templates = Vec::new();

        // Check current directory's .pmp/template-packs
        let current_templates_path = std::env::current_dir()?.join(".pmp").join("template-packs");

        if fs.exists(&current_templates_path) {
            templates.extend(Self::load_templates_from_dir(fs, output, &current_templates_path)?);
        }

        // Check ~/.pmp/template-packs
        if let Some(home_dir) = dirs::home_dir() {
            let home_templates_path = home_dir.join(".pmp").join("template-packs");

            if fs.exists(&home_templates_path) {
                templates.extend(Self::load_templates_from_dir(fs, output, &home_templates_path)?);
            }
        }

        // Check custom paths
        for custom_path in custom_paths {
            let custom_path_buf = PathBuf::from(custom_path);

            if fs.exists(&custom_path_buf) {
                templates.extend(Self::load_templates_from_dir(fs, output, &custom_path_buf)?);
            }
        }

        Ok(templates)
    }

    /// Load templates from a specific directory
    fn load_templates_from_dir(fs: &dyn crate::traits::FileSystem, output: &dyn crate::traits::Output, base_path: &Path) -> Result<Vec<TemplateInfo>> {
        let mut templates = Vec::new();

        // Walk through subdirectories looking for .pmp.template.yaml files
        let entries = fs.walk_dir(base_path, 2)?;

        for entry_path in entries {
            if fs.is_file(&entry_path) && entry_path.file_name() == Some(std::ffi::OsStr::new(".pmp.template.yaml"))
                && let Some(template_dir) = entry_path.parent() {
                    match TemplateResource::from_file(fs, &entry_path) {
                        Ok(resource) => {
                            templates.push(TemplateInfo {
                                resource,
                                path: template_dir.to_path_buf(),
                            });
                        }
                        Err(e) => {
                            output.warning(&format!("Failed to load template from {:?}: {}", entry_path, e));
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
    /// Name of the template pack containing this plugin
    pub template_pack_name: String,
}
