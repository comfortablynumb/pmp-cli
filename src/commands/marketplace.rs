use anyhow::{Result, bail};
use std::path::PathBuf;

use crate::context::Context as AppContext;
use crate::marketplace::{
    PackInfo, RegistryIndex, RegistryManager, RegistryResource, RegistrySourceConfig,
    RegistrySource,
};
use crate::marketplace::filesystem_source::FilesystemSource;
use crate::marketplace::html_generator::write_index_files;
use crate::marketplace::url_source::UrlSource;
use crate::template::discovery::TemplateDiscovery;
use crate::traits::Output;

pub struct MarketplaceCommand;

impl MarketplaceCommand {
    /// Search for template packs across all registries
    pub fn execute_search(ctx: &AppContext, query: &str, registry: Option<&str>) -> Result<()> {
        ctx.output.section("Searching template packs");
        ctx.output.blank();

        let manager = RegistryManager::new(&*ctx.fs);
        let registries = get_registries(&manager, registry)?;

        if registries.is_empty() {
            ctx.output.warning("No registries configured. Add one with:");
            ctx.output.lavender("  pmp marketplace registry add <name> --url <url>");
            return Ok(());
        }

        let mut results: Vec<(String, PackInfo)> = Vec::new();

        for reg in &registries {
            let source = create_source(&reg.metadata.name, &reg.spec.source, &*ctx.fs)?;

            match source.search(query) {
                Ok(packs) => {
                    for pack in packs {
                        results.push((reg.metadata.name.clone(), pack));
                    }
                }
                Err(e) => {
                    ctx.output.warning(&format!(
                        "Failed to search registry '{}': {}",
                        reg.metadata.name, e
                    ));
                }
            }
        }

        if results.is_empty() {
            ctx.output.dimmed(&format!("No packs found matching '{}'", query));
            return Ok(());
        }

        ctx.output.success(&format!("Found {} pack(s)", results.len()));
        ctx.output.blank();

        for (registry_name, pack) in results {
            print_pack_summary(&pack, &registry_name, &*ctx.output);
        }

        Ok(())
    }

    /// List all template packs from registries
    pub fn execute_list(ctx: &AppContext, registry: Option<&str>) -> Result<()> {
        ctx.output.section("Available template packs");
        ctx.output.blank();

        let manager = RegistryManager::new(&*ctx.fs);
        let registries = get_registries(&manager, registry)?;

        if registries.is_empty() {
            ctx.output.warning("No registries configured. Add one with:");
            ctx.output.lavender("  pmp marketplace registry add <name> --url <url>");
            return Ok(());
        }

        let mut total = 0;

        for reg in &registries {
            let source = create_source(&reg.metadata.name, &reg.spec.source, &*ctx.fs)?;

            ctx.output.bright_white(&format!("Registry: {}", reg.metadata.name));

            if let Some(desc) = &reg.metadata.description {
                ctx.output.dimmed(&format!("  {}", desc));
            }

            ctx.output.blank();

            match source.list_packs() {
                Ok(packs) => {
                    if packs.is_empty() {
                        ctx.output.dimmed("  No packs available");
                    } else {
                        for pack in &packs {
                            print_pack_summary(pack, &reg.metadata.name, &*ctx.output);
                            total += 1;
                        }
                    }
                }
                Err(e) => {
                    ctx.output.warning(&format!("  Failed to list packs: {}", e));
                }
            }

            ctx.output.blank();
        }

        ctx.output.success(&format!("Total: {} pack(s)", total));

        Ok(())
    }

    /// Get info about a specific pack
    pub fn execute_info(ctx: &AppContext, pack_name: &str) -> Result<()> {
        let manager = RegistryManager::new(&*ctx.fs);
        let registries = manager.get_enabled_registries()?;

        for reg in &registries {
            let source = create_source(&reg.metadata.name, &reg.spec.source, &*ctx.fs)?;

            if let Ok(Some(pack)) = source.get_pack_info(pack_name) {
                print_pack_details(&pack, &reg.metadata.name, &*ctx.output);
                return Ok(());
            }
        }

        bail!("Pack '{}' not found in any registry", pack_name);
    }

    /// Install a template pack
    pub fn execute_install(
        ctx: &AppContext,
        pack_name: &str,
        version: Option<&str>,
    ) -> Result<()> {
        ctx.output.section(&format!("Installing template pack: {}", pack_name));
        ctx.output.blank();

        let manager = RegistryManager::new(&*ctx.fs);
        let registries = manager.get_enabled_registries()?;

        if registries.is_empty() {
            bail!("No registries configured. Add one with: pmp marketplace registry add <name> --url <url>");
        }

        // Find pack in registries
        for reg in &registries {
            let source = create_source(&reg.metadata.name, &reg.spec.source, &*ctx.fs)?;

            if let Ok(Some(_pack)) = source.get_pack_info(pack_name) {
                ctx.output.dimmed(&format!("Found in registry: {}", reg.metadata.name));

                if let Some(v) = version {
                    ctx.output.dimmed(&format!("Version: {}", v));
                }

                ctx.output.blank();

                // Install to ~/.pmp/template-packs/
                let dest = get_install_destination()?;
                let result = source.install(pack_name, version, &dest)?;

                ctx.output.success(&format!(
                    "Installed {} v{} to {}",
                    result.pack_name,
                    result.version,
                    result.install_path.display()
                ));

                return Ok(());
            }
        }

        bail!("Pack '{}' not found in any registry", pack_name);
    }

    /// Update installed template packs
    pub fn execute_update(ctx: &AppContext, pack_name: Option<&str>, all: bool) -> Result<()> {
        ctx.output.section("Updating template packs");
        ctx.output.blank();

        if pack_name.is_none() && !all {
            ctx.output.warning("Specify a pack name or use --all to update all packs");
            return Ok(());
        }

        // TODO: Implement update logic
        // 1. List installed packs
        // 2. Check for newer versions in registries
        // 3. Update each pack

        ctx.output.warning("Update functionality not yet implemented");

        Ok(())
    }

    /// Add a new registry
    pub fn execute_registry_add(
        ctx: &AppContext,
        name: &str,
        url: Option<&str>,
        path: Option<&str>,
    ) -> Result<()> {
        let source = if let Some(u) = url {
            RegistrySourceConfig::Url { url: u.to_string() }
        } else if let Some(p) = path {
            RegistrySourceConfig::Filesystem { path: p.to_string() }
        } else {
            bail!("Either --url or --path must be specified");
        };

        let registry = RegistryResource::new(name, source);
        let manager = RegistryManager::new(&*ctx.fs);

        manager.add_registry(registry)?;

        ctx.output.success(&format!("Added registry '{}'", name));

        Ok(())
    }

    /// List configured registries
    pub fn execute_registry_list(ctx: &AppContext) -> Result<()> {
        ctx.output.section("Configured registries");
        ctx.output.blank();

        let manager = RegistryManager::new(&*ctx.fs);
        let registries = manager.load_registries()?;

        if registries.is_empty() {
            ctx.output.dimmed("No registries configured");
            ctx.output.blank();
            ctx.output.dimmed("Add one with:");
            ctx.output.lavender("  pmp marketplace registry add <name> --url <url>");
            return Ok(());
        }

        for reg in registries {
            let status = if reg.spec.enabled { "✓" } else { "○" };
            let source_info = match &reg.spec.source {
                RegistrySourceConfig::Url { url } => format!("URL: {}", url),
                RegistrySourceConfig::Filesystem { path } => format!("Path: {}", path),
            };

            ctx.output.bright_white(&format!(
                "{} {} (priority: {})",
                status, reg.metadata.name, reg.spec.priority
            ));

            ctx.output.dimmed(&format!("  {}", source_info));

            if let Some(desc) = &reg.metadata.description {
                ctx.output.dimmed(&format!("  {}", desc));
            }

            ctx.output.blank();
        }

        Ok(())
    }

    /// Remove a registry
    pub fn execute_registry_remove(ctx: &AppContext, name: &str) -> Result<()> {
        let manager = RegistryManager::new(&*ctx.fs);
        manager.remove_registry(name)?;

        ctx.output.success(&format!("Removed registry '{}'", name));

        Ok(())
    }

    /// Generate registry index from local template packs
    pub fn execute_generate_index(
        ctx: &AppContext,
        output_dir: Option<&str>,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Generating registry index");
        ctx.output.blank();

        let out_path = output_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("./dist"));

        // Discover local template packs
        let current_dir = ctx.fs.current_dir()?;

        ctx.output.dimmed("Scanning for template packs...");

        // Use the current directory as a custom path to scan
        let current_dir_str = current_dir.to_string_lossy().to_string();
        let packs = TemplateDiscovery::discover_template_packs_with_custom_paths(
            &*ctx.fs,
            &*ctx.output,
            &[&current_dir_str],
        )?;

        if packs.is_empty() {
            ctx.output.warning("No template packs found in current directory");
            return Ok(());
        }

        ctx.output.success(&format!("Found {} template pack(s)", packs.len()));

        // Convert to PackInfo
        let pack_infos: Vec<PackInfo> = packs
            .into_iter()
            .map(|p| {
                let mut info = PackInfo::new(
                    &p.resource.metadata.name,
                    &format!("https://github.com/OWNER/{}", p.resource.metadata.name),
                );

                if let Some(desc) = &p.resource.metadata.description {
                    info = info.with_description(desc);
                }

                info
            })
            .collect();

        // Create index
        let registry_name = name.unwrap_or("pmp-registry");
        let index = RegistryIndex::new(registry_name, description)
            .with_packs(pack_infos);

        // Write files
        ctx.output.blank();
        ctx.output.dimmed(&format!("Writing to {}...", out_path.display()));

        write_index_files(&*ctx.fs, &out_path, &index)?;

        ctx.output.blank();
        ctx.output.success("Generated:");
        ctx.output.lavender(&format!("  {}/index.json", out_path.display()));
        ctx.output.lavender(&format!("  {}/index.html", out_path.display()));

        Ok(())
    }
}

/// Get registries to use (filtered by name if specified)
fn get_registries(
    manager: &RegistryManager,
    filter: Option<&str>,
) -> Result<Vec<RegistryResource>> {
    let registries = manager.get_enabled_registries()?;

    if let Some(name) = filter {
        let filtered: Vec<_> = registries
            .into_iter()
            .filter(|r| r.metadata.name == name)
            .collect();

        if filtered.is_empty() {
            bail!("Registry '{}' not found", name);
        }

        Ok(filtered)
    } else {
        Ok(registries)
    }
}

/// Create a registry source from configuration
fn create_source<'a>(
    name: &str,
    config: &RegistrySourceConfig,
    fs: &'a dyn crate::traits::FileSystem,
) -> Result<Box<dyn RegistrySource + 'a>> {
    match config {
        RegistrySourceConfig::Url { url } => {
            Ok(Box::new(UrlSource::new(name, url, fs)))
        }
        RegistrySourceConfig::Filesystem { path } => {
            let expanded = RegistryManager::expand_path(path);
            Ok(Box::new(FilesystemSource::new(name, expanded, fs)))
        }
    }
}

/// Get installation destination directory
fn get_install_destination() -> Result<PathBuf> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Unable to determine home directory"))?;

    Ok(home_dir.join(".pmp").join("template-packs"))
}

/// Print pack summary
fn print_pack_summary(pack: &PackInfo, registry: &str, output: &dyn Output) {
    let version = pack
        .latest_version()
        .map(|v| v.version.as_str())
        .unwrap_or("latest");

    output.bright_white(&format!("  {} (v{})", pack.name, version));

    if let Some(desc) = &pack.description {
        output.dimmed(&format!("    {}", desc));
    }

    output.dimmed(&format!("    Registry: {}", registry));

    if !pack.tags.is_empty() {
        output.dimmed(&format!("    Tags: {}", pack.tags.join(", ")));
    }

    output.blank();
}

/// Print detailed pack information
fn print_pack_details(pack: &PackInfo, registry: &str, output: &dyn Output) {
    output.section(&format!("Pack: {}", pack.name));
    output.blank();

    if let Some(desc) = &pack.description {
        output.bright_white("Description:");
        output.dimmed(&format!("  {}", desc));
        output.blank();
    }

    output.bright_white("Registry:");
    output.dimmed(&format!("  {}", registry));
    output.blank();

    output.bright_white("Repository:");
    output.lavender(&format!("  {}", pack.repository));
    output.blank();

    if let Some(author) = &pack.author {
        output.bright_white("Author:");
        output.dimmed(&format!("  {}", author));
        output.blank();
    }

    if let Some(license) = &pack.license {
        output.bright_white("License:");
        output.dimmed(&format!("  {}", license));
        output.blank();
    }

    if !pack.tags.is_empty() {
        output.bright_white("Tags:");
        output.dimmed(&format!("  {}", pack.tags.join(", ")));
        output.blank();
    }

    if !pack.versions.is_empty() {
        output.bright_white("Versions:");

        for v in &pack.versions {
            let released = v
                .released_at
                .map(|dt| format!(" ({})", dt.format("%Y-%m-%d")))
                .unwrap_or_default();

            output.dimmed(&format!("  {} {}", v.version, released));
        }

        output.blank();
    }

    output.bright_white("Install:");
    output.lavender(&format!("  pmp marketplace install {}", pack.name));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_install_destination() {
        let dest = get_install_destination();
        assert!(dest.is_ok());

        let path = dest.unwrap();
        assert!(path.ends_with("template-packs"));
    }
}
