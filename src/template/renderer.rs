use anyhow::{Context, Result};
use handlebars::{Handlebars, Helper, HelperResult, Output, RenderContext};
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;

use super::partials::{PartialDiscovery, PartialInfo};

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
        handlebars.register_helper("k8s_name", Box::new(k8s_name_helper));
        handlebars.register_helper("bool", Box::new(bool_helper));
        handlebars.register_helper("secret", Box::new(secret_helper));

        Self { handlebars }
    }

    /// Create a new template renderer with partials loaded
    /// Discovers partials from:
    /// 1. Pack partials (highest priority): {pack_path}/partials/*.hbs
    /// 2. Global partials: ~/.pmp/partials/*.hbs
    pub fn new_with_partials(
        fs: &dyn crate::traits::FileSystem,
        pack_path: Option<&Path>,
    ) -> Result<Self> {
        let mut renderer = Self::new();

        // Discover and register partials
        let partials = PartialDiscovery::discover_all(fs, pack_path)?;
        renderer.register_partials(&partials)?;

        Ok(renderer)
    }

    /// Register Handlebars partials
    /// Partials can be used in templates with {{> partial_name}} syntax
    pub fn register_partials(&mut self, partials: &[PartialInfo]) -> Result<()> {
        for partial in partials {
            self.handlebars
                .register_partial(&partial.name, &partial.content)
                .with_context(|| {
                    format!(
                        "Failed to register partial '{}' from {:?}",
                        partial.name, partial.source
                    )
                })?;
        }

        Ok(())
    }

    /// Render all template files from src directory to output directory
    ///
    /// # Arguments
    /// * `ctx` - Application context with filesystem and output traits
    /// * `template_src_dir` - Base directory of the template (e.g., `.pmp/template-packs/postgres/templates/postgres/`)
    /// * `output_dir` - Output directory for rendered files
    /// * `variables` - Variables to use in template rendering
    /// * `plugin_context` - Optional tuple of (template_pack_name, plugin_name) for plugin rendering
    ///
    /// # Returns
    /// List of relative paths of generated files
    pub fn render_template(
        &self,
        ctx: &crate::context::Context,
        template_src_dir: &Path,
        output_dir: &Path,
        variables: &HashMap<String, Value>,
        _plugin_context: Option<(&str, &str)>,
    ) -> Result<Vec<String>> {
        // Create output directory if it doesn't exist
        ctx.fs
            .create_dir_all(output_dir)
            .context("Failed to create output directory")?;

        let mut generated_files = Vec::new();

        // Walk through all files in the template/plugin src directory
        let src_dir = template_src_dir.join("src");

        // Check if src/ exists - if not, skip rendering (for plugin-only templates)
        if !ctx.fs.exists(&src_dir) {
            ctx.output.dimmed(&format!(
                "  No src/ directory found in {} - skipping file generation (plugin-only template)",
                template_src_dir.display()
            ));
            return Ok(Vec::new()); // Return empty file list
        }

        let entries = ctx.fs.walk_dir(&src_dir, 100)?;

        for path in entries {
            if ctx.fs.is_file(&path)
                && let Some(relative_path) =
                    self.render_file(ctx, &path, &src_dir, output_dir, variables)?
            {
                generated_files.push(relative_path);
            }
        }

        Ok(generated_files)
    }

    /// Render a single file
    /// Returns the relative path of the generated file, or None if the file was skipped
    fn render_file(
        &self,
        ctx: &crate::context::Context,
        file_path: &Path,
        template_base_dir: &Path,
        output_base_dir: &Path,
        variables: &HashMap<String, Value>,
    ) -> Result<Option<String>> {
        // Calculate relative path
        let relative_path = file_path
            .strip_prefix(template_base_dir)
            .context("Failed to calculate relative path")?;

        // Determine output path (remove .hbs extension if present)
        let output_path = if let Some(file_name) = relative_path.file_name() {
            let file_name_str = file_name.to_string_lossy();

            // Skip .pmp.* files - these are auto-generated or metadata
            if file_name_str == ".pmp.yaml.hbs"
                || file_name_str == ".pmp.yaml"
                || file_name_str == ".pmp.project.yaml.hbs"
                || file_name_str == ".pmp.project.yaml"
                || file_name_str == ".pmp.environment.yaml.hbs"
                || file_name_str == ".pmp.environment.yaml"
                || file_name_str == ".pmp.template.yaml"
                || file_name_str == ".pmp.plugin.yaml"
            {
                ctx.output.info(&format!(
                    "  Skipped: {} (metadata/auto-generated)",
                    file_name_str
                ));
                return Ok(None);
            }

            // Process filename
            let final_name = if file_name_str.ends_with(".hbs") {
                file_name_str.trim_end_matches(".hbs").to_string()
            } else {
                file_name_str.to_string()
            };

            // Plugin files no longer need SHA1 prefix since they're in separate module directories
            let parent = relative_path.parent().unwrap_or_else(|| Path::new(""));
            output_base_dir.join(parent).join(final_name)
        } else {
            output_base_dir.join(relative_path)
        };

        // Create parent directories if needed
        if let Some(parent) = output_path.parent() {
            ctx.fs
                .create_dir_all(parent)
                .context("Failed to create parent directories")?;
        }

        // Read template content
        let template_content = ctx
            .fs
            .read_to_string(file_path)
            .with_context(|| format!("Failed to read template file: {:?}", file_path))?;

        // Render template with Handlebars (handles {{variable}} syntax)
        let rendered = self
            .handlebars
            .render_template(&template_content, variables)
            .with_context(|| format!("Failed to render template: {:?}", file_path))?;

        // Post-process for ${var:...} and ${env:...} interpolation patterns
        let final_content = crate::template::utils::interpolate_all(&rendered, variables)
            .with_context(|| format!("Failed to interpolate variables in: {:?}", file_path))?;

        // Write rendered content
        ctx.fs
            .write(&output_path, &final_content)
            .with_context(|| format!("Failed to write output file: {:?}", output_path))?;

        ctx.output
            .info(&format!("  Created: {}", output_path.display()));

        // Return relative path from output_base_dir
        let relative_output = output_path
            .strip_prefix(output_base_dir)
            .context("Failed to calculate relative output path")?
            .to_string_lossy()
            .to_string();

        Ok(Some(relative_output))
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

    if let (Some(p1), Some(p2)) = (param1, param2)
        && p1 == p2
    {
        out.write("true")?;
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
            if let Some(item_str) = item.as_str()
                && item_str == search
            {
                out.write("true")?;
                return Ok(());
            }
        }
    }

    Ok(())
}

/// Helper function to convert strings to Kubernetes-compatible DNS subdomain names (RFC 1123)
/// Rules: lowercase alphanumeric, '-', '.'; must start/end with alphanumeric; max 253 chars
fn k8s_name_helper(
    h: &Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let value = h.param(0).and_then(|v| v.value().as_str()).ok_or_else(|| {
        handlebars::RenderError::from(handlebars::RenderErrorReason::Other(
            "k8s_name requires a string parameter".to_string(),
        ))
    })?;

    // Sanitize: keep only lowercase alphanumeric, '-', and '.'
    // Replace underscores with hyphens, convert to lowercase, remove invalid chars
    let mut sanitized = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            'a'..='z' | '0'..='9' | '-' | '.' => sanitized.push(ch),
            'A'..='Z' => sanitized.push(ch.to_ascii_lowercase()),
            '_' => sanitized.push('-'),
            _ => {} // Skip invalid characters
        }
    }

    // Trim non-alphanumeric from start and end
    let trimmed = sanitized.trim_matches(|c: char| !c.is_alphanumeric());

    // Truncate to 253 characters (Kubernetes DNS subdomain limit)
    let result = if trimmed.len() > 253 {
        trimmed[..253].trim_end_matches(|c: char| !c.is_alphanumeric())
    } else {
        trimmed
    };

    out.write(result)?;
    Ok(())
}

/// Helper function to explicitly render boolean values as "true" or "false" strings
/// This prevents Handlebars from treating booleans as conditional expressions
fn bool_helper(
    h: &Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let value = h
        .param(0)
        .ok_or_else(|| {
            handlebars::RenderError::from(handlebars::RenderErrorReason::Other(
                "bool helper requires a parameter".to_string(),
            ))
        })?
        .value();

    match value {
        Value::Bool(true) => out.write("true")?,
        Value::Bool(false) => out.write("false")?,
        _ => {
            // For non-boolean values, convert to string representation
            out.write(&value.to_string())?;
        }
    }

    Ok(())
}

/// Secret helper: {{secret input_name}} outputs local.secret_<input_name>
/// This references the local value generated in _common.tf for secret inputs
fn secret_helper(
    h: &Helper,
    _: &Handlebars,
    _: &handlebars::Context,
    _: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let input_name = h
        .param(0)
        .ok_or_else(|| {
            handlebars::RenderError::from(handlebars::RenderErrorReason::Other(
                "secret helper requires an input name parameter".to_string(),
            ))
        })?
        .value();

    let name_str = match input_name {
        Value::String(s) => s.clone(),
        _ => input_name.to_string().trim_matches('"').to_string(),
    };

    // Sanitize the name for use as a Terraform local variable name
    let sanitized = name_str
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect::<String>();

    out.write(&format!("local.secret_{}", sanitized))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_k8s_name_underscore_conversion() {
        let renderer = TemplateRenderer::new();
        assert_eq!(
            renderer
                .handlebars
                .render_template("{{k8s_name name}}", &json!({"name": "my_api"}))
                .unwrap(),
            "my-api"
        );
    }

    #[test]
    fn test_k8s_name_already_valid() {
        let renderer = TemplateRenderer::new();
        assert_eq!(
            renderer
                .handlebars
                .render_template("{{k8s_name name}}", &json!({"name": "my-api"}))
                .unwrap(),
            "my-api"
        );
    }

    #[test]
    fn test_k8s_name_uppercase_conversion() {
        let renderer = TemplateRenderer::new();
        assert_eq!(
            renderer
                .handlebars
                .render_template("{{k8s_name name}}", &json!({"name": "MyApp"}))
                .unwrap(),
            "myapp"
        );
    }

    #[test]
    fn test_k8s_name_sanitization() {
        let renderer = TemplateRenderer::new();
        assert_eq!(
            renderer
                .handlebars
                .render_template("{{k8s_name name}}", &json!({"name": "my@app#123"}))
                .unwrap(),
            "myapp123"
        );
    }

    #[test]
    fn test_k8s_name_truncation() {
        let renderer = TemplateRenderer::new();
        let long_name = "a".repeat(300);
        let result = renderer
            .handlebars
            .render_template("{{k8s_name name}}", &json!({"name": long_name}))
            .unwrap();
        assert_eq!(result.len(), 253);
        assert!(result.chars().last().unwrap().is_alphanumeric());
    }

    #[test]
    fn test_k8s_name_trim_non_alphanumeric() {
        let renderer = TemplateRenderer::new();
        assert_eq!(
            renderer
                .handlebars
                .render_template("{{k8s_name name}}", &json!({"name": "-my-app-"}))
                .unwrap(),
            "my-app"
        );
    }

    #[test]
    fn test_k8s_name_multiple_underscores() {
        let renderer = TemplateRenderer::new();
        assert_eq!(
            renderer
                .handlebars
                .render_template("{{k8s_name name}}", &json!({"name": "my_test_api_service"}))
                .unwrap(),
            "my-test-api-service"
        );
    }

    #[test]
    fn test_k8s_name_dots_preserved() {
        let renderer = TemplateRenderer::new();
        assert_eq!(
            renderer
                .handlebars
                .render_template("{{k8s_name name}}", &json!({"name": "api.example.com"}))
                .unwrap(),
            "api.example.com"
        );
    }
}
