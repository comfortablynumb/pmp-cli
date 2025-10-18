use anyhow::{Context, Result};
use handlebars::Handlebars;
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
        Self {
            handlebars: Handlebars::new(),
        }
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
