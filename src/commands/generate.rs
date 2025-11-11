use crate::output;
use crate::schema::SchemaValidator;
use crate::template::{TemplateDiscovery, TemplateRenderer};
use anyhow::{Context, Result};

/// Handles the 'generate' command - generates files from templates without creating a project
pub struct GenerateCommand;

impl GenerateCommand {
    /// Execute the generate command
    pub fn execute(
        ctx: &crate::context::Context,
        template_pack: Option<&str>,
        template_name: Option<&str>,
        output_dir: Option<&str>,
        template_packs_paths: Option<&str>,
    ) -> Result<()> {
        ctx.output.section("Generate from Template");
        ctx.output
            .dimmed("Generate files from a template without creating a project structure.");

        // Step 1: Discover template packs (NO infrastructure filtering)
        // Parse flag paths (colon-separated)
        let flag_paths: Vec<String> = if let Some(paths) = template_packs_paths {
            crate::template::discovery::parse_colon_separated_paths(paths)
        } else {
            vec![]
        };

        // Parse environment variable paths (colon-separated)
        let env_paths: Vec<String> = std::env::var("PMP_TEMPLATE_PACKS_PATHS")
            .ok()
            .map(|p| crate::template::discovery::parse_colon_separated_paths(&p))
            .unwrap_or_default();

        // Combine paths: flag paths have priority over env paths
        let mut all_paths = flag_paths;
        all_paths.extend(env_paths);

        // Convert to Vec<&str> for the discovery function
        let custom_paths: Vec<&str> = all_paths.iter().map(|s| s.as_str()).collect();

        let all_template_packs = TemplateDiscovery::discover_template_packs_with_custom_paths(
            &*ctx.fs,
            &*ctx.output,
            &custom_paths,
        )
        .context("Failed to discover template packs")?;

        if all_template_packs.is_empty() {
            anyhow::bail!(
                "No template packs found. Please create template packs in ~/.pmp/template-packs or .pmp/template-packs"
            );
        }

        ctx.output.blank();
        ctx.output.info(&format!(
            "Found {} template pack(s)",
            all_template_packs.len()
        ));

        // Step 2: Select template pack (with optional CLI flag)
        let selected_pack = if let Some(pack_name) = template_pack {
            // Find pack by name from CLI flag
            all_template_packs
                .into_iter()
                .find(|p| p.resource.metadata.name == pack_name)
                .ok_or_else(|| anyhow::anyhow!("Template pack '{}' not found", pack_name))?
        } else if all_template_packs.len() == 1 {
            // Only one pack, use it automatically
            let pack = all_template_packs.into_iter().next().unwrap();
            ctx.output.subsection("Template Pack");
            ctx.output
                .key_value_highlight("Pack", &pack.resource.metadata.name);
            if let Some(desc) = &pack.resource.metadata.description {
                ctx.output.key_value("Description", desc);
            }
            pack
        } else {
            // Multiple packs, let user choose
            let mut sorted_packs = all_template_packs;
            sorted_packs.sort_by(|a, b| a.resource.metadata.name.cmp(&b.resource.metadata.name));

            let pack_options: Vec<String> = sorted_packs
                .iter()
                .map(|pack| {
                    let desc = pack.resource.metadata.description.as_deref().unwrap_or("");
                    if desc.is_empty() {
                        pack.resource.metadata.name.clone()
                    } else {
                        format!("{} - {}", pack.resource.metadata.name, desc)
                    }
                })
                .collect();

            let selected_pack_display = ctx
                .input
                .select("Select a template pack:", pack_options.clone())
                .context("Failed to select template pack")?;

            let pack_index = pack_options
                .iter()
                .position(|opt| opt == &selected_pack_display)
                .context("Template pack not found")?;

            let pack = sorted_packs.into_iter().nth(pack_index).unwrap();

            ctx.output.subsection("Selected Template Pack");
            ctx.output
                .key_value_highlight("Pack", &pack.resource.metadata.name);
            if let Some(desc) = &pack.resource.metadata.description {
                ctx.output.key_value("Description", desc);
            }

            pack
        };

        // Step 3: Discover templates within the selected pack (NO filtering)
        let available_templates = TemplateDiscovery::discover_templates_in_pack(
            &*ctx.fs,
            &*ctx.output,
            &selected_pack.path,
        )
        .context("Failed to discover templates in pack")?;

        if available_templates.is_empty() {
            anyhow::bail!(
                "No templates found in template pack '{}'",
                selected_pack.resource.metadata.name
            );
        }

        // Step 4: Select template (with optional CLI flag)
        let selected_template = if let Some(tmpl_name) = template_name {
            // Find template by name from CLI flag
            available_templates
                .into_iter()
                .find(|t| t.resource.metadata.name == tmpl_name)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Template '{}' not found in pack '{}'",
                        tmpl_name,
                        selected_pack.resource.metadata.name
                    )
                })?
        } else if available_templates.len() == 1 {
            // Only one template, use it automatically
            let template = available_templates.into_iter().next().unwrap();
            ctx.output.subsection("Template");
            ctx.output
                .key_value_highlight("Template", &template.resource.metadata.name);
            if let Some(desc) = &template.resource.metadata.description {
                ctx.output.key_value("Description", desc);
            }
            template
        } else {
            // Multiple templates, let user choose
            ctx.output.subsection("Select a template");

            let mut sorted_templates = available_templates;
            sorted_templates
                .sort_by(|a, b| a.resource.metadata.name.cmp(&b.resource.metadata.name));

            let template_options: Vec<String> = sorted_templates
                .iter()
                .map(|t| {
                    let desc = t.resource.metadata.description.as_deref().unwrap_or("");
                    if desc.is_empty() {
                        t.resource.metadata.name.clone()
                    } else {
                        format!("{} - {}", t.resource.metadata.name, desc)
                    }
                })
                .collect();

            let selected_template_display = ctx
                .input
                .select("Template:", template_options.clone())
                .context("Failed to select template")?;

            let template_index = template_options
                .iter()
                .position(|opt| opt == &selected_template_display)
                .context("Template not found")?;

            let template = sorted_templates.into_iter().nth(template_index).unwrap();

            ctx.output.subsection("Selected Template");
            ctx.output
                .key_value_highlight("Template", &template.resource.metadata.name);
            if let Some(desc) = &template.resource.metadata.description {
                ctx.output.key_value("Description", desc);
            }

            template
        };

        // Step 5: Handle environment selection if template has environment-specific inputs
        let selected_environment = if !selected_template.resource.spec.environments.is_empty() {
            ctx.output.subsection("Select an environment context");
            ctx.output
                .dimmed("This template has environment-specific configurations.");

            // Get environment keys and sort them
            let mut env_keys: Vec<String> = selected_template
                .resource
                .spec
                .environments
                .keys()
                .cloned()
                .collect();
            env_keys.sort();

            if env_keys.len() == 1 {
                // Only one environment, use it automatically
                let env = env_keys[0].clone();
                ctx.output.environment_badge(&env);
                Some(env)
            } else {
                // Multiple environments, let user choose
                let selected_env = ctx
                    .input
                    .select("Environment context:", env_keys.clone())
                    .context("Failed to select environment")?;

                ctx.output.environment_badge(&selected_env);
                Some(selected_env)
            }
        } else {
            None
        };

        // Step 6: Prompt for a name (used as project identifier in templates)
        ctx.output.subsection("Generation Configuration");
        let name = SchemaValidator::prompt_for_project_name(ctx).context("Failed to get name")?;

        // Step 7: Collect inputs based on template's input definitions
        ctx.output.subsection("Template Inputs");
        ctx.output
            .dimmed("Please provide the following information:");

        // Start with base inputs from template spec
        let mut merged_inputs = selected_template.resource.spec.inputs.clone();

        // Override with environment-specific inputs if an environment was selected
        if let Some(ref env) = selected_environment
            && let Some(env_overrides) = selected_template.resource.spec.environments.get(env)
        {
            for env_input in &env_overrides.overrides.inputs {
                // Remove any existing input with the same name
                merged_inputs.retain(|input_def| input_def.name != env_input.name);
                // Add the environment-specific input
                merged_inputs.push(env_input.clone());
            }
        }

        // Collect inputs from user (no infrastructure overrides in generate mode)
        let mut inputs = Self::collect_template_inputs(ctx, &merged_inputs, &name)
            .context("Failed to collect inputs")?;

        // Step 8: Add internal fields for template rendering
        if let Some(ref env) = selected_environment {
            inputs.insert(
                "environment".to_string(),
                serde_json::Value::String(env.clone()),
            );
        }
        inputs.insert(
            "resource_api_version".to_string(),
            serde_json::Value::String(selected_template.resource.spec.api_version.clone()),
        );
        inputs.insert(
            "resource_kind".to_string(),
            serde_json::Value::String(selected_template.resource.spec.kind.clone()),
        );

        // Step 9: Determine output directory
        let output_path = if let Some(path) = output_dir {
            std::path::PathBuf::from(path)
        } else {
            std::env::current_dir().context("Failed to get current directory")?
        };

        // Create output directory if it doesn't exist
        if !ctx.fs.exists(&output_path) {
            ctx.fs.create_dir_all(&output_path).context(format!(
                "Failed to create output directory: {}",
                output_path.display()
            ))?;
        }

        // Step 10: Render template into output directory
        ctx.output.subsection("Generating Files");
        ctx.output.dimmed("Rendering template...");
        let renderer = TemplateRenderer::new();
        let template_src = &selected_template.path;

        if !ctx.fs.exists(template_src) {
            anyhow::bail!("Template directory not found: {}", template_src.display());
        }

        let _generated_files = renderer
            .render_template(ctx, template_src, output_path.as_path(), &inputs, None)
            .context("Failed to render template")?;

        ctx.output.blank();
        ctx.output.success("Files generated successfully!");

        ctx.output.subsection("Generation Details");
        ctx.output
            .key_value("Template Pack", &selected_pack.resource.metadata.name);
        ctx.output
            .key_value_highlight("Template", &selected_template.resource.metadata.name);
        ctx.output.key_value("Name", &name);
        if let Some(env) = selected_environment {
            ctx.output.environment_badge(&env);
        }
        ctx.output
            .key_value("Output Directory", &output_path.display().to_string());

        let next_steps_list = vec![
            format!("Review the generated files in {}", output_path.display()),
            "Customize the files as needed for your use case".to_string(),
        ];
        output::next_steps(&next_steps_list);

        Ok(())
    }

    /// Collect inputs from user based on template input specifications (simplified version without infrastructure overrides)
    fn collect_template_inputs(
        ctx: &crate::context::Context,
        inputs_spec: &[crate::template::metadata::InputDefinition],
        name: &str,
    ) -> Result<std::collections::HashMap<String, serde_json::Value>> {
        let mut inputs = std::collections::HashMap::new();

        // Always add automatic variables
        inputs.insert(
            "_name".to_string(),
            serde_json::Value::String(name.to_string()),
        );
        inputs.insert(
            "name".to_string(),
            serde_json::Value::String(name.to_string()),
        );

        // Collect each input defined in the template
        for input_def in inputs_spec {
            // Skip automatic variables
            if input_def.name == "_name" || input_def.name == "name" {
                continue;
            }

            let value = Self::prompt_for_input(ctx, &input_def.name, &input_def.to_input_spec())?;
            inputs.insert(input_def.name.clone(), value);
        }

        Ok(inputs)
    }

    /// Prompt for a single input based on its specification
    fn prompt_for_input(
        ctx: &crate::context::Context,
        input_name: &str,
        input_spec: &crate::template::metadata::InputSpec,
    ) -> Result<serde_json::Value> {
        let description = input_spec.description.as_deref().unwrap_or(input_name);
        let default_value = input_spec.default.as_ref();

        if let Some(enum_values) = &input_spec.enum_values {
            // This is a select input
            let mut sorted_enum_values = enum_values.clone();
            sorted_enum_values.sort();

            let default_str = default_value
                .and_then(|v| v.as_str())
                .or_else(|| sorted_enum_values.first().map(|s| s.as_str()));

            let selected = if let Some(default) = default_str {
                let starting_cursor = sorted_enum_values
                    .iter()
                    .position(|v| v == default)
                    .unwrap_or(0);
                let _ = starting_cursor; // Suppress unused warning
                ctx.input
                    .select(description, sorted_enum_values.clone())
                    .context("Failed to get input")?
            } else {
                ctx.input
                    .select(description, sorted_enum_values)
                    .context("Failed to get input")?
            };

            Ok(serde_json::Value::String(selected))
        } else if let Some(default) = default_value {
            // Determine type from default value
            match default {
                serde_json::Value::Bool(b) => {
                    let answer = ctx
                        .input
                        .confirm(description, *b)
                        .context("Failed to get input")?;
                    Ok(serde_json::Value::Bool(answer))
                }
                serde_json::Value::Number(n) => {
                    let prompt_text = format!("{} (default: {})", description, n);
                    let answer = ctx
                        .input
                        .text(&prompt_text, Some(&n.to_string()))
                        .context("Failed to get input")?;

                    // Try to parse as number
                    if let Ok(num) = answer.parse::<i64>() {
                        Ok(serde_json::Value::Number(num.into()))
                    } else if let Ok(num) = answer.parse::<f64>() {
                        Ok(serde_json::Value::Number(
                            serde_json::Number::from_f64(num).unwrap(),
                        ))
                    } else {
                        Ok(serde_json::Value::String(answer))
                    }
                }
                serde_json::Value::String(s) => {
                    let prompt_text = format!("{} (default: {})", description, s);
                    let answer = ctx
                        .input
                        .text(&prompt_text, Some(s))
                        .context("Failed to get input")?;
                    Ok(serde_json::Value::String(answer))
                }
                _ => {
                    // Fallback to string input
                    let answer = ctx
                        .input
                        .text(description, None)
                        .context("Failed to get input")?;
                    Ok(serde_json::Value::String(answer))
                }
            }
        } else {
            // No default, prompt for string
            let answer = ctx
                .input
                .text(description, None)
                .context("Failed to get input")?;
            Ok(serde_json::Value::String(answer))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::Context;
    use crate::executor::registry::DefaultExecutorRegistry;
    use crate::traits::user_input::MockResponse;
    use crate::traits::{
        FileSystem, MockCommandExecutor, MockFileSystem, MockOutput, MockUserInput,
    };
    use std::path::PathBuf;
    use std::sync::Arc;

    /// Helper to create a test context with mocks
    fn create_test_context(fs: Arc<MockFileSystem>, input: MockUserInput) -> Context {
        Context {
            fs,
            input: Arc::new(input),
            output: Arc::new(MockOutput::new()),
            command: Arc::new(MockCommandExecutor::new()),
            executor_registry: Arc::new(DefaultExecutorRegistry::with_defaults()),
        }
    }

    /// Helper to set up a basic template pack in the mock filesystem
    fn setup_template_pack(
        fs: &MockFileSystem,
        pack_name: &str,
        template_name: &str,
        resource_kind: &str,
        inputs: &str,
    ) -> PathBuf {
        // Use actual current directory for template pack discovery to work
        let current_dir = std::env::current_dir().unwrap();
        let pack_path = current_dir.join(".pmp/template-packs").join(pack_name);

        // Create template pack file
        let pack_yaml = format!(
            r#"apiVersion: pmp.io/v1
kind: TemplatePack
metadata:
  name: {}
  description: Test template pack
spec: {{}}"#,
            pack_name
        );
        fs.write(&pack_path.join(".pmp.template-pack.yaml"), &pack_yaml)
            .unwrap();

        // Create template directory
        let template_dir = pack_path.join("templates").join(template_name);

        // Create template file
        let template_yaml = format!(
            r#"apiVersion: pmp.io/v1
kind: Template
metadata:
  name: {}
  description: Test template
spec:
  apiVersion: pmp.io/v1
  kind: {}
  executor: opentofu
  inputs:
{}"#,
            template_name, resource_kind, inputs
        );
        fs.write(&template_dir.join(".pmp.template.yaml"), &template_yaml)
            .unwrap();

        // Create src/ subdirectory with a simple template file
        fs.write(&template_dir.join("src/main.tf.hbs"), "# Test template")
            .unwrap();

        pack_path
    }

    #[test]
    fn test_generate_command_basic() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Set up template pack with a template
        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r#"    setting:
      default: "value"
      description: Test setting"#,
        );

        // Set up mock user input
        let input = MockUserInput::new();
        input.add_response(MockResponse::Text("test_generation".to_string())); // name
        input.add_response(MockResponse::Text("custom".to_string())); // setting

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run generate command
        let result = GenerateCommand::execute(
            &ctx,
            Some("test-pack"),     // template pack
            Some("test-template"), // template
            None,                  // output dir (current dir)
            None,                  // template packs paths
        );

        // Verify command succeeded
        assert!(
            result.is_ok(),
            "Generate command should succeed: {:?}",
            result
        );
    }

    #[test]
    fn test_generate_command_with_environment() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Use actual current directory for template pack discovery to work
        let current_dir = std::env::current_dir().unwrap();
        let pack_path = current_dir.join(".pmp/template-packs/test-pack");
        let template_dir = pack_path.join("templates/env-template");

        // Create template pack file
        let pack_yaml = r#"apiVersion: pmp.io/v1
kind: TemplatePack
metadata:
  name: test-pack
  description: Test template pack
spec: {}"#;
        fs.write(&pack_path.join(".pmp.template-pack.yaml"), pack_yaml)
            .unwrap();

        // Create template with environment-specific inputs
        let template_yaml = r#"apiVersion: pmp.io/v1
kind: Template
metadata:
  name: env-template
  description: Template with environments
spec:
  apiVersion: pmp.io/v1
  kind: TestResource
  executor: opentofu
  inputs:
    replicas:
      default: 1
      description: Number of replicas
  environments:
    dev:
      overrides:
        inputs:
          replicas:
            default: 2
            description: Dev replicas
    prod:
      overrides:
        inputs:
          replicas:
            default: 5
            description: Prod replicas"#;
        fs.write(&template_dir.join(".pmp.template.yaml"), template_yaml)
            .unwrap();
        fs.write(&template_dir.join("src/main.tf.hbs"), "# Test template")
            .unwrap();

        // Set up mock user input
        let input = MockUserInput::new();
        input.add_response(MockResponse::Select("prod".to_string())); // environment
        input.add_response(MockResponse::Text("test_generation".to_string())); // name
        input.add_response(MockResponse::Text("10".to_string())); // replicas (override prod default)

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run generate command
        let result =
            GenerateCommand::execute(&ctx, Some("test-pack"), Some("env-template"), None, None);

        // Verify command succeeded
        assert!(
            result.is_ok(),
            "Generate command with environment should succeed: {:?}",
            result
        );
    }

    #[test]
    fn test_generate_command_custom_output_dir() {
        // Set up mock filesystem
        let fs = Arc::new(MockFileSystem::new());

        // Set up template pack
        setup_template_pack(
            &fs,
            "test-pack",
            "test-template",
            "TestResource",
            r#"    setting:
      default: "value"
      description: Test setting"#,
        );

        // Set up mock user input
        let input = MockUserInput::new();
        input.add_response(MockResponse::Text("test_generation".to_string())); // name
        input.add_response(MockResponse::Text("custom".to_string())); // setting

        let ctx = create_test_context(Arc::clone(&fs), input);

        // Run generate command with custom output directory
        let output_dir = "/tmp/custom-output";
        let result = GenerateCommand::execute(
            &ctx,
            Some("test-pack"),
            Some("test-template"),
            Some(output_dir),
            None,
        );

        // Verify command succeeded
        assert!(
            result.is_ok(),
            "Generate command with custom output dir should succeed: {:?}",
            result
        );
    }
}
