use anyhow::{Context, Result};
use handlebars::{Handlebars, Helper, HelperResult, Output, RenderContext};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

/// Renders templates using Handlebars
pub struct TemplateRenderer {
    handlebars: Handlebars<'static>,
}

impl TemplateRenderer {
    /// Create a new template renderer
    pub fn new() -> Self {
        let mut handlebars = Handlebars::new();

        // Register custom helpers
        handlebars.register_helper("eq", Box::new(eq_helper));
        handlebars.register_helper("contains", Box::new(contains_helper));

        Self { handlebars }
    }

    /// Render all template files from src directory to output directory
    pub fn render_template(
        &self,
        template_src_dir: &Path,
        output_dir: &Path,
        variables: &HashMap<String, Value>,
    ) -> Result<()> {
        // Create output directory if it doesn't exist
        fs::create_dir_all(output_dir)
            .context("Failed to create output directory")?;

        // Walk through all files in the template src directory
        for entry in WalkDir::new(template_src_dir)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            if path.is_file() {
                self.render_file(path, template_src_dir, output_dir, variables)?;
            }
        }

        Ok(())
    }

    /// Render a single file
    fn render_file(
        &self,
        file_path: &Path,
        template_base_dir: &Path,
        output_base_dir: &Path,
        variables: &HashMap<String, Value>,
    ) -> Result<()> {
        // Calculate relative path
        let relative_path = file_path
            .strip_prefix(template_base_dir)
            .context("Failed to calculate relative path")?;

        // Determine output path (remove .hbs extension if present)
        let output_path = if let Some(file_name) = relative_path.file_name() {
            let file_name_str = file_name.to_string_lossy();

            // Skip .pmp.yaml files - these are auto-generated
            if file_name_str == ".pmp.yaml.hbs" || file_name_str == ".pmp.yaml" {
                println!("  Skipped: {} (auto-generated)", file_name_str);
                return Ok(());
            }

            if file_name_str.ends_with(".hbs") {
                let new_name = file_name_str.trim_end_matches(".hbs");
                let parent = relative_path.parent().unwrap_or_else(|| Path::new(""));
                output_base_dir.join(parent).join(new_name)
            } else {
                output_base_dir.join(relative_path)
            }
        } else {
            output_base_dir.join(relative_path)
        };

        // Create parent directories if needed
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create parent directories")?;
        }

        // Read template content
        let template_content = fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read template file: {:?}", file_path))?;

        // Render template
        let rendered = self
            .handlebars
            .render_template(&template_content, variables)
            .with_context(|| format!("Failed to render template: {:?}", file_path))?;

        // Write rendered content
        fs::write(&output_path, rendered)
            .with_context(|| format!("Failed to write output file: {:?}", output_path))?;

        println!("  Created: {}", output_path.display());

        Ok(())
    }
}

impl Default for TemplateRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function for equality comparison
fn eq_helper(
    h: &Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param1 = h.param(0).and_then(|v| v.value().as_str());
    let param2 = h.param(1).and_then(|v| v.value().as_str());

    if let (Some(p1), Some(p2)) = (param1, param2) {
        if p1 == p2 {
            out.write("true")?;
        }
    }

    Ok(())
}

/// Helper function to check if an array contains a value
fn contains_helper(
    h: &Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let array = h.param(0).and_then(|v| v.value().as_array());
    let search_value = h.param(1).and_then(|v| v.value().as_str());

    if let (Some(arr), Some(search)) = (array, search_value) {
        for item in arr {
            if let Some(item_str) = item.as_str() {
                if item_str == search {
                    out.write("true")?;
                    return Ok(());
                }
            }
        }
    }

    Ok(())
}
