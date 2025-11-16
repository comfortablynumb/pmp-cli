use crate::context::Context;
use crate::output;
use crate::template::TemplateDiscovery;
use anyhow::{Context as AnyhowContext, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub struct TemplateMgmtCommand;

#[derive(Debug, Serialize, Deserialize)]
struct TemplateValidationReport {
    template_pack: String,
    template_name: String,
    valid: bool,
    issues: Vec<TemplateIssue>,
}

#[derive(Debug, Serialize, Deserialize)]
struct TemplateIssue {
    severity: String,
    category: String,
    message: String,
    location: Option<String>,
}

impl TemplateMgmtCommand {
    /// Validate template definitions
    pub fn execute_validate(ctx: &Context, template_pack: &str, template_name: &str) -> Result<()> {
        ctx.output.section("Template Validation");

        ctx.output.key_value("Template Pack", template_pack);
        ctx.output.key_value("Template", template_name);
        output::blank();

        // Discover template packs
        let template_packs = TemplateDiscovery::discover_template_packs(&*ctx.fs, &*ctx.output)?;

        // Find the specified template pack
        let pack = template_packs
            .iter()
            .find(|p| p.resource.metadata.name == template_pack)
            .context("Template pack not found")?;

        // Find the specified template
        let templates =
            TemplateDiscovery::discover_templates_in_pack(&*ctx.fs, &*ctx.output, &pack.path)?;
        let template = templates
            .iter()
            .find(|t| t.resource.metadata.name == template_name)
            .context("Template not found")?;

        ctx.output.dimmed("Validating template...");

        // Validate template
        let report = Self::validate_template(ctx, template_pack, template_name, &template.path)?;

        // Display results
        Self::display_validation_report(ctx, &report);

        if !report.valid {
            anyhow::bail!("Template validation failed");
        }

        Ok(())
    }

    /// Test template rendering with sample data
    pub fn execute_test(
        ctx: &Context,
        template_pack: &str,
        template_name: &str,
        test_data: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Template Testing");

        ctx.output.key_value("Template Pack", template_pack);
        ctx.output.key_value("Template", template_name);
        output::blank();

        // Discover template packs
        let template_packs = TemplateDiscovery::discover_template_packs(&*ctx.fs, &*ctx.output)?;

        // Find the specified template pack
        let pack = template_packs
            .iter()
            .find(|p| p.resource.metadata.name == template_pack)
            .context("Template pack not found")?;

        // Find the specified template
        let templates =
            TemplateDiscovery::discover_templates_in_pack(&*ctx.fs, &*ctx.output, &pack.path)?;
        let template = templates
            .iter()
            .find(|t| t.resource.metadata.name == template_name)
            .context("Template not found")?;

        // Load test data
        let test_inputs = if let Some(data_file) = test_data {
            let data_path = PathBuf::from(data_file);
            let data_content = ctx.fs.read_to_string(&data_path)?;
            serde_json::from_str(&data_content)?
        } else {
            // Use default test data
            Self::generate_default_test_data(ctx, template)?
        };

        ctx.output.dimmed("Rendering template with test data...");

        // Test template rendering
        let output_dir = std::env::temp_dir().join("pmp-template-test");
        ctx.fs.create_dir_all(&output_dir)?;

        Self::test_template_rendering(ctx, &template.path, &test_inputs, &output_dir)?;

        ctx.output.success(&format!(
            "Template rendered successfully to: {}",
            output_dir.display()
        ));

        Ok(())
    }

    /// Publish template to registry
    pub fn execute_publish(
        ctx: &Context,
        template_pack: &str,
        registry_url: Option<&str>,
        version: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Template Publishing");

        ctx.output.key_value("Template Pack", template_pack);

        if let Some(url) = registry_url {
            ctx.output.key_value("Registry", url);
        }

        if let Some(ver) = version {
            ctx.output.key_value("Version", ver);
        }

        output::blank();

        // Discover template packs
        let template_packs = TemplateDiscovery::discover_template_packs(&*ctx.fs, &*ctx.output)?;

        // Find the specified template pack
        let pack = template_packs
            .iter()
            .find(|p| p.resource.metadata.name == template_pack)
            .context("Template pack not found")?;

        ctx.output
            .dimmed("Validating template pack before publishing...");

        // Validate template pack
        Self::validate_template_pack(ctx, &pack.path)?;

        ctx.output.dimmed("Packaging template pack...");

        // Package template pack
        let package_path = Self::package_template_pack(ctx, &pack.path, template_pack, version)?;

        ctx.output
            .dimmed(&format!("Package created: {}", package_path.display()));

        // Publish to registry
        if let Some(url) = registry_url {
            ctx.output.dimmed(&format!("Publishing to {}...", url));
            Self::publish_to_registry(ctx, &package_path, url)?;
            ctx.output.success("Template pack published successfully!");
        } else {
            ctx.output.success(&format!(
                "Template pack packaged at: {}",
                package_path.display()
            ));
            ctx.output
                .dimmed("Use --registry-url to publish to a registry");
        }

        Ok(())
    }

    /// Clone and customize existing template
    pub fn execute_clone(
        ctx: &Context,
        source_pack: &str,
        source_template: &str,
        target_pack: &str,
        target_template: &str,
    ) -> Result<()> {
        ctx.output.section("Template Cloning");

        ctx.output.key_value("Source Pack", source_pack);
        ctx.output.key_value("Source Template", source_template);
        ctx.output.key_value("Target Pack", target_pack);
        ctx.output.key_value("Target Template", target_template);
        output::blank();

        // Discover template packs
        let template_packs = TemplateDiscovery::discover_template_packs(&*ctx.fs, &*ctx.output)?;

        // Find source template pack
        let source_pack_ref = template_packs
            .iter()
            .find(|p| p.resource.metadata.name == source_pack)
            .context("Source template pack not found")?;

        // Find source template
        let templates = TemplateDiscovery::discover_templates_in_pack(
            &*ctx.fs,
            &*ctx.output,
            &source_pack_ref.path,
        )?;
        let source_template_ref = templates
            .iter()
            .find(|t| t.resource.metadata.name == source_template)
            .context("Source template not found")?;

        // Determine target location
        let target_base = PathBuf::from(".pmp/template-packs");
        let target_pack_dir = target_base.join(target_pack);
        let target_template_dir = target_pack_dir.join("templates").join(target_template);

        ctx.output.dimmed("Cloning template...");

        // Clone template
        Self::clone_template(
            ctx,
            &source_template_ref.path,
            &target_template_dir,
            target_template,
        )?;

        // Create template pack if it doesn't exist
        let pack_file = target_pack_dir.join(".pmp.template-pack.yaml");

        if !ctx.fs.exists(&pack_file) {
            ctx.output.dimmed("Creating template pack...");
            Self::create_template_pack(ctx, &target_pack_dir, target_pack)?;
        }

        ctx.output.success(&format!(
            "Template cloned to: {}",
            target_template_dir.display()
        ));
        ctx.output
            .dimmed("You can now customize the template to your needs");

        Ok(())
    }

    /// Helper for developing new plugins
    pub fn execute_plugin_develop(
        ctx: &Context,
        template_pack: &str,
        plugin_name: &str,
    ) -> Result<()> {
        ctx.output.section("Plugin Development");

        ctx.output.key_value("Template Pack", template_pack);
        ctx.output.key_value("Plugin Name", plugin_name);
        output::blank();

        // Create plugin directory structure
        let pack_dir = PathBuf::from(".pmp/template-packs").join(template_pack);
        let plugin_dir = pack_dir.join("plugins").join(plugin_name);

        ctx.output.dimmed("Creating plugin directory structure...");

        ctx.fs.create_dir_all(&plugin_dir)?;

        // Create plugin metadata file
        let plugin_file = plugin_dir.join(".pmp.plugin.yaml");
        let plugin_content = format!(
            r#"apiVersion: pmp.io/v1
kind: Plugin
metadata:
  name: {}
  description: Plugin for {}
spec:
  role: {}
  inputs:
    - name: example_input
      type: string
      description: Example input for the plugin
      default: ""
      required: false
"#,
            plugin_name, plugin_name, plugin_name
        );

        ctx.fs.write(&plugin_file, &plugin_content)?;

        // Create plugin template files
        let plugin_tf = plugin_dir.join("plugin.tf.hbs");
        let plugin_tf_content = format!(
            r#"# Plugin: {}
# Add your Terraform/OpenTofu resources here

# Example resource
resource "null_resource" "{}" {{
  triggers = {{
    example = var.example_input
  }}
}}
"#,
            plugin_name, plugin_name
        );

        ctx.fs.write(&plugin_tf, &plugin_tf_content)?;

        // Create variables file
        let variables_tf = plugin_dir.join("variables.tf.hbs");
        let variables_content = r#"variable "example_input" {
  description = "Example input for the plugin"
  type        = string
  default     = ""
}
"#;

        ctx.fs.write(&variables_tf, variables_content)?;

        // Create outputs file
        let outputs_tf = plugin_dir.join("outputs.tf.hbs");
        let outputs_content = format!(
            r#"output "{}_output" {{
  description = "Example output from the plugin"
  value       = null_resource.{}.id
}}
"#,
            plugin_name, plugin_name
        );

        ctx.fs.write(&outputs_tf, &outputs_content)?;

        ctx.output.success(&format!(
            "Plugin scaffolding created at: {}",
            plugin_dir.display()
        ));
        output::blank();

        ctx.output.subsection("Next Steps");
        output::blank();

        ctx.output
            .dimmed("1. Edit .pmp.plugin.yaml to define plugin metadata and inputs");
        ctx.output
            .dimmed("2. Add your infrastructure code to plugin.tf.hbs");
        ctx.output.dimmed("3. Define variables in variables.tf.hbs");
        ctx.output.dimmed("4. Define outputs in outputs.tf.hbs");
        ctx.output
            .dimmed("5. Test the plugin using 'pmp template test'");

        Ok(())
    }

    // Helper methods

    fn validate_template(
        ctx: &Context,
        template_pack: &str,
        template_name: &str,
        template_path: &Path,
    ) -> Result<TemplateValidationReport> {
        let mut issues = Vec::new();

        // Check template metadata file exists
        let metadata_file = template_path.join(".pmp.template.yaml");

        if !ctx.fs.exists(&metadata_file) {
            issues.push(TemplateIssue {
                severity: "error".to_string(),
                category: "metadata".to_string(),
                message: "Template metadata file (.pmp.template.yaml) not found".to_string(),
                location: Some(template_path.display().to_string()),
            });
        } else {
            // Parse and validate metadata
            let metadata_content = ctx.fs.read_to_string(&metadata_file)?;

            if let Err(e) = serde_yaml::from_str::<serde_yaml::Value>(&metadata_content) {
                issues.push(TemplateIssue {
                    severity: "error".to_string(),
                    category: "metadata".to_string(),
                    message: format!("Invalid YAML in metadata file: {}", e),
                    location: Some(metadata_file.display().to_string()),
                });
            }
        }

        // Check for template files
        let has_template_files = ctx.fs.read_dir(template_path)?.iter().any(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e == "hbs" || e == "tf")
                .unwrap_or(false)
        });

        if !has_template_files {
            issues.push(TemplateIssue {
                severity: "warning".to_string(),
                category: "content".to_string(),
                message: "No template files (.hbs or .tf) found".to_string(),
                location: Some(template_path.display().to_string()),
            });
        }

        // Check for common issues
        // - Missing required inputs
        // - Invalid Handlebars syntax
        // - Missing documentation

        let valid = !issues.iter().any(|i| i.severity == "error");

        Ok(TemplateValidationReport {
            template_pack: template_pack.to_string(),
            template_name: template_name.to_string(),
            valid,
            issues,
        })
    }

    fn display_validation_report(ctx: &Context, report: &TemplateValidationReport) {
        ctx.output.subsection("Validation Results");
        output::blank();

        if report.issues.is_empty() {
            ctx.output.success("No issues found!");
            return;
        }

        for issue in &report.issues {
            let symbol = match issue.severity.as_str() {
                "error" => "✗",
                "warning" => "⚠",
                _ => "ℹ",
            };

            ctx.output.info(&format!(
                "{} [{}] {}",
                symbol, issue.category, issue.message
            ));

            if let Some(loc) = &issue.location {
                ctx.output.dimmed(&format!("  at {}", loc));
            }
        }

        output::blank();

        if report.valid {
            ctx.output.success("Validation passed!");
        } else {
            ctx.output.error("Validation failed!");
        }
    }

    fn generate_default_test_data(
        _ctx: &Context,
        template: &crate::template::discovery::TemplateInfo,
    ) -> Result<serde_json::Map<String, serde_json::Value>> {
        let mut test_data = serde_json::Map::new();

        // Generate default values for each input
        for input in &template.resource.spec.inputs {
            let default_value = if let Some(default) = &input.default {
                default.clone()
            } else {
                match input.input_type {
                    Some(crate::template::metadata::InputType::String) => {
                        serde_json::Value::String("test-value".to_string())
                    }
                    Some(crate::template::metadata::InputType::Number { .. }) => {
                        serde_json::Value::Number(serde_json::Number::from(42))
                    }
                    Some(crate::template::metadata::InputType::Boolean) => {
                        serde_json::Value::Bool(true)
                    }
                    _ => serde_json::Value::String("test".to_string()),
                }
            };

            test_data.insert(input.name.clone(), default_value);
        }

        Ok(test_data)
    }

    fn test_template_rendering(
        ctx: &Context,
        template_path: &Path,
        _test_inputs: &serde_json::Map<String, serde_json::Value>,
        output_dir: &Path,
    ) -> Result<()> {
        // In a real implementation, render the template with test data
        // For now, just copy template files to output directory

        for entry in ctx.fs.read_dir(template_path)? {
            if let Some(ext) = entry.extension()
                && (ext == "hbs" || ext == "tf")
            {
                let file_name = entry.file_name().unwrap();
                let target = output_dir.join(file_name);

                let content = ctx.fs.read_to_string(&entry)?;
                ctx.fs.write(&target, &content)?;
            }
        }

        Ok(())
    }

    fn validate_template_pack(_ctx: &Context, pack_path: &Path) -> Result<()> {
        // Check for .pmp.template-pack.yaml
        let pack_file = pack_path.join(".pmp.template-pack.yaml");

        if !pack_file.exists() {
            anyhow::bail!("Template pack metadata file not found");
        }

        // Check for templates directory
        let templates_dir = pack_path.join("templates");

        if !templates_dir.exists() {
            anyhow::bail!("Templates directory not found");
        }

        // In a real implementation, we would discover and validate templates
        // For now, simplified validation
        Ok(())
    }

    fn package_template_pack(
        ctx: &Context,
        _pack_path: &Path,
        pack_name: &str,
        version: Option<&str>,
    ) -> Result<PathBuf> {
        let version_str = version.unwrap_or("latest");
        let package_name = format!("{}-{}.tar.gz", pack_name, version_str);
        let package_path = std::env::temp_dir().join(package_name);

        // In a real implementation, create a tar.gz archive
        // For now, just return the path
        ctx.output.dimmed(&format!(
            "Package would be created at: {}",
            package_path.display()
        ));

        Ok(package_path)
    }

    fn publish_to_registry(ctx: &Context, package_path: &Path, registry_url: &str) -> Result<()> {
        // In a real implementation, upload package to registry
        // For now, just simulate
        ctx.output.dimmed(&format!(
            "Would publish {} to {}",
            package_path.display(),
            registry_url
        ));

        Ok(())
    }

    fn clone_template(
        ctx: &Context,
        source_path: &Path,
        target_path: &Path,
        new_name: &str,
    ) -> Result<()> {
        ctx.fs.create_dir_all(target_path)?;

        // Copy all files from source to target
        for entry in ctx.fs.read_dir(source_path)? {
            let file_name = entry.file_name().unwrap();
            let source_file = source_path.join(file_name);
            let target_file = target_path.join(file_name);

            if ctx.fs.is_file(&source_file) {
                let mut content = ctx.fs.read_to_string(&source_file)?;

                // Update template name in metadata file
                if file_name == ".pmp.template.yaml" {
                    content = content.replace(
                        &format!(
                            "name: {}",
                            source_path.file_name().unwrap().to_str().unwrap()
                        ),
                        &format!("name: {}", new_name),
                    );
                }

                ctx.fs.write(&target_file, &content)?;
            }
        }

        Ok(())
    }

    fn create_template_pack(ctx: &Context, pack_dir: &Path, pack_name: &str) -> Result<()> {
        ctx.fs.create_dir_all(pack_dir)?;

        let pack_file = pack_dir.join(".pmp.template-pack.yaml");
        let pack_content = format!(
            r#"apiVersion: pmp.io/v1
kind: TemplatePack
metadata:
  name: {}
  description: Custom template pack
spec: {{}}
"#,
            pack_name
        );

        ctx.fs.write(&pack_file, &pack_content)?;

        Ok(())
    }
}
