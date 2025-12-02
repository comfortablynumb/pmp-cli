use crate::context::Context;
use crate::output;
use anyhow::{Context as AnyhowContext, Result};
use std::path::PathBuf;

pub struct TemplateCommand;

impl TemplateCommand {
    /// Execute the template scaffold command
    pub fn execute_scaffold(ctx: &Context, output_dir: Option<&str>) -> Result<()> {
        ctx.output.section("Template Scaffolding");
        ctx.output
            .dimmed("Create a new template pack interactively");
        output::blank();

        // Determine output directory
        let base_dir = if let Some(dir) = output_dir {
            PathBuf::from(dir)
        } else {
            std::env::current_dir().context("Failed to get current directory")?
        };

        // Collect template pack metadata
        let pack_name = ctx.input.text("Template pack name:", Some("my-pack"))?;

        let pack_description = ctx.input.text(
            "Template pack description:",
            Some("My custom template pack"),
        )?;

        // Collect template metadata
        let template_name = ctx.input.text("Template name:", Some("my-template"))?;

        let template_description = ctx
            .input
            .text("Template description:", Some("My custom template"))?;

        // Collect resource definition
        let resource_kind = ctx
            .input
            .text("Resource kind (alphanumeric only):", Some("CustomResource"))?;

        // Validate resource kind is alphanumeric
        if !resource_kind.chars().all(|c| c.is_alphanumeric()) {
            anyhow::bail!("Resource kind must be alphanumeric only");
        }

        let executor = ctx.input.select(
            "Executor:",
            vec![
                "opentofu".to_string(),
                "terraform".to_string(),
                "none".to_string(),
            ],
            None,
        )?;

        // Ask about inputs
        let add_inputs = ctx.input.confirm("Add input definitions?", true)?;

        let mut inputs = Vec::new();
        if add_inputs {
            loop {
                let input_name = ctx.input.text("Input name (or empty to finish):", None)?;

                if input_name.is_empty() {
                    break;
                }

                let input_type = ctx.input.select(
                    "Input type:",
                    vec![
                        "string".to_string(),
                        "number".to_string(),
                        "boolean".to_string(),
                    ],
                    None,
                )?;

                let input_description = ctx.input.text("Input description:", None)?;

                let input_default = ctx.input.text("Default value (optional):", None)?;

                inputs.push((input_name, input_type, input_description, input_default));

                let add_more = ctx.input.confirm("Add another input?", true)?;
                if !add_more {
                    break;
                }
            }
        }

        // Create directory structure
        let pack_dir = base_dir.join(&pack_name);
        let template_dir = pack_dir.join("templates").join(&template_name);

        ctx.output.subsection("Creating template pack structure");

        ctx.fs.create_dir_all(&template_dir)?;

        // Generate template pack file
        let pack_yaml = format!(
            "apiVersion: pmp.io/v1\nkind: TemplatePack\nmetadata:\n  name: {}\n  description: {}\nspec: {{}}\n",
            pack_name, pack_description
        );

        ctx.fs
            .write(&pack_dir.join(".pmp.template-pack.yaml"), &pack_yaml)?;

        ctx.output
            .success(&format!("Created template pack: {}", pack_name));

        // Generate template file
        let mut template_yaml = format!(
            "apiVersion: pmp.io/v1\nkind: Template\nmetadata:\n  name: \"{}\"\n  description: \"{}\"\nspec:\n  resource:\n    apiVersion: pmp.io/v1\n    kind: {}\n  executor: {}\n",
            template_name, template_description, resource_kind, executor
        );

        if !inputs.is_empty() {
            template_yaml.push_str("  inputs:\n");
            for (name, input_type, description, default) in &inputs {
                template_yaml.push_str(&format!("    {}:\n", name));
                template_yaml.push_str(&format!("      type: {}\n", input_type));
                template_yaml.push_str(&format!("      description: \"{}\"\n", description));
                if !default.is_empty() {
                    template_yaml.push_str(&format!("      default: \"{}\"\n", default));
                }
            }
        } else {
            template_yaml.push_str("  inputs: []\n");
        }

        template_yaml.push_str("  environments: {}\n");

        ctx.fs
            .write(&template_dir.join(".pmp.template.yaml"), &template_yaml)?;

        ctx.output
            .success(&format!("Created template: {}", template_name));

        // Generate sample template files based on executor
        if executor != "none" {
            // Create main.tf.hbs
            let mut main_tf = String::from("# Main infrastructure configuration\n\n");
            main_tf.push_str("# Project: {{ project.name }}\n");
            main_tf.push_str("# Environment: {{ environment }}\n\n");

            if !inputs.is_empty() {
                main_tf.push_str("# Inputs:\n");
                for (name, _, _, _) in &inputs {
                    main_tf.push_str(&format!("# - {}: {{{{ inputs.{} }}}}\n", name, name));
                }
                main_tf.push('\n');
            }

            main_tf.push_str("# Add your infrastructure code here\n");

            ctx.fs.write(&template_dir.join("main.tf.hbs"), &main_tf)?;
            ctx.output.success("Created main.tf.hbs");

            // Create variables.tf.hbs
            let mut variables_tf = String::from("# Variable definitions\n\n");

            for (name, input_type, description, _) in &inputs {
                variables_tf.push_str(&format!("variable \"{}\" {{\n", name));
                variables_tf.push_str(&format!("  description = \"{}\"\n", description));

                let tf_type = match input_type.as_str() {
                    "string" => "string",
                    "number" => "number",
                    "boolean" => "bool",
                    _ => "string",
                };
                variables_tf.push_str(&format!("  type        = {}\n", tf_type));
                variables_tf.push_str("}\n\n");
            }

            ctx.fs
                .write(&template_dir.join("variables.tf.hbs"), &variables_tf)?;
            ctx.output.success("Created variables.tf.hbs");

            // Create outputs.tf.hbs
            let outputs_tf = String::from(
                "# Output definitions\n\n# Add your outputs here\n# Example:\n# output \"endpoint\" {\n#   value = resource.example.endpoint\n# }\n",
            );

            ctx.fs
                .write(&template_dir.join("outputs.tf.hbs"), &outputs_tf)?;
            ctx.output.success("Created outputs.tf.hbs");
        }

        // Create README
        let readme = format!(
            "# {} - {}\n\n{}\n\n## Usage\n\nUse this template pack with PMP:\n\n```bash\npmp create --template-packs-paths {}\n```\n\n## Template: {}\n\n{}\n\n### Inputs\n\n{}\n",
            pack_name,
            template_name,
            pack_description,
            pack_dir.display(),
            template_name,
            template_description,
            if inputs.is_empty() {
                "No inputs defined.".to_string()
            } else {
                let mut input_docs = String::from("| Name | Type | Description | Default |\n");
                input_docs.push_str("|------|------|-------------|----------|\n");
                for (name, input_type, description, default) in &inputs {
                    input_docs.push_str(&format!(
                        "| {} | {} | {} | {} |\n",
                        name,
                        input_type,
                        description,
                        if default.is_empty() { "-" } else { default }
                    ));
                }
                input_docs
            }
        );

        ctx.fs.write(&pack_dir.join("README.md"), &readme)?;
        ctx.output.success("Created README.md");

        output::blank();
        ctx.output.success("Template pack scaffolding complete!");
        ctx.output
            .key_value("Location", &pack_dir.display().to_string());

        output::blank();
        ctx.output.info("Next steps:");
        ctx.output
            .dimmed("1. Customize the template files in the templates directory");
        ctx.output
            .dimmed("2. Add any additional templates to the pack");
        ctx.output.dimmed(&format!(
            "3. Use with: pmp create --template-packs-paths {}",
            pack_dir.display()
        ));

        Ok(())
    }
}
