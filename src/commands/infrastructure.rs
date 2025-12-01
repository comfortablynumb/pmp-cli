use crate::context::Context;
use crate::output;
use crate::template::metadata::{
    Category, CategoryTemplate, Environment, ExecutorCollectionConfig, InfrastructureMetadata,
    InfrastructureResource, InfrastructureSpec, TemplatePackConfig,
};
use crate::template::{InfrastructureTemplateInfo, TemplateDiscovery, TemplatePackInfo};
use anyhow::{Context as _, Result};
use std::collections::HashMap;
use std::path::PathBuf;

// Type alias for template selection result
type TemplateSelectionResult = (
    Vec<String>,
    HashMap<String, (String, String, String, String)>,
);

pub struct InfrastructureCommand;

impl InfrastructureCommand {
    /// Initialize a new infrastructure from an infrastructure template
    pub fn execute_init(
        ctx: &Context,
        output_dir: Option<&str>,
        template_packs_paths: Option<&str>,
    ) -> Result<()> {
        // Determine output directory
        let output_path = if let Some(dir) = output_dir {
            PathBuf::from(dir)
        } else {
            std::env::current_dir()?
        };

        // Step 1: Check if infrastructure already exists
        let infrastructure_file = output_path.join(".pmp.infrastructure.yaml");
        if ctx.fs.exists(&infrastructure_file) {
            anyhow::bail!(
                "Infrastructure already exists in this directory: {:?}",
                infrastructure_file
            );
        }

        output::section("Creating Infrastructure from Template");
        output::blank();

        // Step 2: Parse custom template pack paths
        let custom_paths_vec: Vec<String> = if let Some(paths_str) = template_packs_paths {
            crate::template::discovery::parse_colon_separated_paths(paths_str)
        } else {
            vec![]
        };
        let custom_paths: Vec<&str> = custom_paths_vec.iter().map(|s| s.as_str()).collect();

        // Step 3: Discover infrastructure templates
        output::subsection("Discovering Infrastructure Templates");
        let template_packs = TemplateDiscovery::discover_template_packs_with_custom_paths(
            &*ctx.fs,
            &*ctx.output,
            &custom_paths,
        )?;

        // Collect all infrastructure templates from all packs
        let mut all_infra_templates: Vec<(String, InfrastructureTemplateInfo)> = Vec::new();
        for pack in &template_packs {
            let infra_templates = TemplateDiscovery::discover_infrastructure_templates_in_pack(
                &*ctx.fs,
                &*ctx.output,
                &pack.path,
            )?;

            for infra_template in infra_templates {
                all_infra_templates.push((pack.resource.metadata.name.clone(), infra_template));
            }
        }

        if all_infra_templates.is_empty() {
            anyhow::bail!(
                "No infrastructure templates found. Please install a template pack with infrastructure templates."
            );
        }

        output::success(&format!(
            "Found {} infrastructure template(s)",
            all_infra_templates.len()
        ));
        output::blank();

        // Step 4: Select infrastructure template
        let selected_template = if all_infra_templates.len() == 1 {
            let (pack_name, template) = &all_infra_templates[0];
            output::info(&format!(
                "Auto-selecting infrastructure template: {} / {}",
                pack_name, template.resource.metadata.name
            ));
            template
        } else {
            output::subsection("Select Infrastructure Template");
            let template_options: Vec<String> = all_infra_templates
                .iter()
                .map(|(pack_name, template)| {
                    format!("{} / {}", pack_name, template.resource.metadata.name)
                })
                .collect();

            let selected_option = ctx
                .input
                .select("Select infrastructure template:", template_options.clone())?;

            // Find the index
            let selected_idx = template_options
                .iter()
                .position(|o| o == &selected_option)
                .unwrap();

            &all_infra_templates[selected_idx].1
        };

        output::blank();

        // Step 5: Collect template inputs (if any)
        // TODO: Implement input collection for infrastructure templates if needed

        // Step 6: Collect basic metadata
        output::subsection("Infrastructure Metadata");
        let collection_name = ctx
            .input
            .text("Infrastructure name (optional):", Some("My Infrastructure"))
            .context("Failed to get infrastructure name")?;

        let collection_description_input = ctx
            .input
            .text("Description (optional):", None)
            .context("Failed to get description")?;

        let collection_description = if collection_description_input.is_empty() {
            None
        } else {
            Some(collection_description_input)
        };

        output::blank();

        // Step 7: Configure environments (optional)
        let mut environments: HashMap<String, Environment> = HashMap::new();
        let configure_envs = ctx
            .input
            .confirm("Configure environments?", false)
            .context("Failed to get confirmation")?;

        if configure_envs {
            output::subsection("Environments");
            environments = Self::collect_environments(ctx)?;
            output::blank();
        }

        // Step 8: Select allowed templates
        output::subsection("Select Allowed Templates");
        output::dimmed("Select which project templates will be available in this infrastructure.");
        output::blank();

        let (selected_templates, template_map) =
            Self::select_allowed_templates(ctx, &template_packs)?;
        output::blank();

        // Step 9: Configure categories (optional)
        let configure_categories = ctx
            .input
            .confirm("Create categories for organizing templates?", false)
            .context("Failed to get confirmation")?;

        let (categories, template_packs_config) = if configure_categories {
            output::subsection("Configure Categories");
            let mut available_templates = selected_templates.clone();
            let categories =
                Self::collect_categories(ctx, &mut available_templates, &template_map)?;

            // Build template_packs_config from categories
            let template_packs_config = Self::build_template_packs_config(&categories);
            output::blank();
            (categories, template_packs_config)
        } else {
            // Use simple auto-generated categories
            Self::build_categories_from_selections(&selected_templates, &template_map)
        };

        // Step 10: Configure executor backend (optional)
        let mut executor_config: Option<ExecutorCollectionConfig> = None;
        let configure_executor = ctx
            .input
            .confirm("Configure executor backend?", false)
            .context("Failed to get confirmation")?;

        if configure_executor {
            output::subsection("Executor Backend Configuration");
            executor_config = Some(Self::collect_executor_config(ctx)?);
            output::blank();
        }

        // Step 11: Build infrastructure resource
        let infrastructure = InfrastructureResource {
            api_version: "pmp.io/v1".to_string(),
            kind: "Infrastructure".to_string(),
            metadata: InfrastructureMetadata {
                name: collection_name.clone(),
                description: collection_description,
            },
            spec: InfrastructureSpec {
                categories,
                template_packs: template_packs_config,
                resource_kinds: vec![], // Deprecated field
                environments,
                hooks: None,
                executor: executor_config,
            },
        };

        // Step 12: Render infrastructure template (if it has template files)
        let src_dir = selected_template.path.join("src");
        if ctx.fs.exists(&src_dir) {
            output::subsection("Rendering Infrastructure Template");
            let renderer = crate::template::TemplateRenderer::new();
            let template_input_values: HashMap<String, serde_json::Value> = HashMap::new();
            renderer.render_template(
                ctx,
                &src_dir,
                &output_path,
                &template_input_values,
                None, // No plugin context
            )?;
            output::success("Template files rendered successfully");
            output::blank();
        }

        // Step 13: Save infrastructure file
        infrastructure
            .save(&*ctx.fs, &infrastructure_file)
            .context("Failed to save .pmp.infrastructure.yaml")?;

        // Step 14: Create projects directory
        let projects_dir = output_path.join("projects");
        if !ctx.fs.exists(&projects_dir) {
            ctx.fs.create_dir_all(&projects_dir)?;
        }

        output::section("Infrastructure Created Successfully");
        output::key_value("Name", &collection_name);
        output::key_value("Location", &format!("{:?}", infrastructure_file));
        output::key_value(
            "Environments",
            &format!("{}", infrastructure.spec.environments.len()),
        );
        output::key_value(
            "Categories",
            &format!("{}", infrastructure.spec.categories.len()),
        );
        output::blank();

        // Step 15: Ask if user wants to generate CI job files
        let generate_ci = ctx
            .input
            .confirm("Generate CI/CD pipeline files?", false)
            .context("Failed to get CI generation confirmation")?;

        if generate_ci {
            output::blank();
            output::subsection("CI/CD Pipeline Generation");

            // Ask for CI provider
            let ci_providers = vec![
                "github-actions".to_string(),
                "gitlab-ci".to_string(),
                "jenkins".to_string(),
            ];

            let ci_provider = ctx.input.select("Select CI provider:", ci_providers)?;

            output::blank();
            output::info(&format!("Generating {} pipeline...", ci_provider));

            // Call CiCommand::execute_generate
            let ci_result = crate::commands::CiCommand::execute_generate(
                ctx,
                &ci_provider,
                None,  // output_file - will use defaults
                None,  // environment - will include all environments
                false, // static_mode - use dynamic by default
            );

            match ci_result {
                Ok(()) => {
                    output::success("CI/CD pipeline files generated successfully");
                    output::blank();
                }
                Err(e) => {
                    output::warning(&format!("Failed to generate CI pipeline: {}", e));
                    output::blank();
                }
            }
        }

        output::info("Next steps:");
        output::info("  1. Review the generated .pmp.infrastructure.yaml file");
        if generate_ci {
            output::info("  2. Review the generated CI/CD pipeline files");
            output::info("  3. Run 'pmp create' to create your first project");
        } else {
            output::info("  2. Run 'pmp create' to create your first project");
        }
        output::blank();

        Ok(())
    }

    // ============================================================================
    // Helper Methods
    // ============================================================================

    /// Collect environments from user
    fn collect_environments(ctx: &Context) -> Result<HashMap<String, Environment>> {
        let mut environments: HashMap<String, Environment> = HashMap::new();

        loop {
            // Prompt for environment key
            let env_key = loop {
                let key = ctx.input.text(
                    "Environment key (lowercase, alphanumeric, underscores; cannot start with number):",
                    None,
                )?;

                // Validate environment key
                if !InfrastructureResource::is_valid_environment_name(&key) {
                    output::warning(
                        "Invalid environment key. Must be lowercase alphanumeric with underscores, and cannot start with a number.",
                    );
                    continue;
                }

                // Check for duplicate
                if environments.contains_key(&key) {
                    output::warning(&format!(
                        "Environment '{}' already exists. Please use a different key.",
                        key
                    ));
                    continue;
                }

                break key;
            };

            // Prompt for display name
            let default_name = Self::capitalize_first(&env_key);
            let env_name = ctx
                .input
                .text("Environment display name:", Some(&default_name))?;

            // Prompt for optional description
            let env_description = ctx
                .input
                .text("Environment description (optional):", None)?;

            environments.insert(
                env_key.clone(),
                Environment {
                    name: env_name,
                    description: if env_description.is_empty() {
                        None
                    } else {
                        Some(env_description)
                    },
                },
            );

            output::success(&format!("Environment '{}' added", env_key));
            output::blank();

            // Ask if they want to add another environment
            let add_another = ctx.input.confirm("Add another environment?", false)?;

            if !add_another {
                break;
            }
        }

        Ok(environments)
    }

    /// Select allowed templates from all template packs
    fn select_allowed_templates(
        ctx: &Context,
        template_packs: &[TemplatePackInfo],
    ) -> Result<TemplateSelectionResult> {
        // Discover all templates across all packs
        let mut all_templates: Vec<(String, crate::template::TemplateInfo)> = Vec::new();
        for pack in template_packs {
            let templates =
                TemplateDiscovery::discover_templates_in_pack(&*ctx.fs, &*ctx.output, &pack.path)?;

            for template in templates {
                all_templates.push((pack.resource.metadata.name.clone(), template));
            }
        }

        if all_templates.is_empty() {
            anyhow::bail!("No project templates found in template packs.");
        }

        // Build template options and map
        let mut template_options: Vec<String> = Vec::new();
        let mut template_map: HashMap<String, (String, String, String, String)> = HashMap::new();

        for (pack_name, template) in &all_templates {
            let option = format!(
                "{} / {} ({})",
                pack_name, template.resource.metadata.name, template.resource.spec.kind
            );
            template_options.push(option.clone());
            template_map.insert(
                option,
                (
                    pack_name.clone(),
                    template.resource.metadata.name.clone(),
                    template.resource.spec.api_version.clone(),
                    template.resource.spec.kind.clone(),
                ),
            );
        }

        // Multi-select templates (loop until at least one is selected)
        let selected_templates = loop {
            let templates = ctx.input.multi_select(
                "Select templates to allow in this infrastructure:",
                template_options.clone(),
                None,
            )?;

            if templates.is_empty() {
                output::warning("At least one template must be selected. Please try again.");
                output::blank();
                continue;
            }

            break templates;
        };

        output::success(&format!(
            "Selected {} template(s)",
            selected_templates.len()
        ));

        Ok((selected_templates, template_map))
    }

    /// Collect categories interactively (recursive for subcategories)
    fn collect_categories(
        ctx: &Context,
        available_templates: &mut Vec<String>,
        template_map: &HashMap<String, (String, String, String, String)>,
    ) -> Result<Vec<Category>> {
        Self::collect_categories_recursive(ctx, available_templates, template_map, 0)
    }

    /// Recursive helper for collecting categories
    fn collect_categories_recursive(
        ctx: &Context,
        available_templates: &mut Vec<String>,
        template_map: &HashMap<String, (String, String, String, String)>,
        depth: usize,
    ) -> Result<Vec<Category>> {
        let mut categories: Vec<Category> = Vec::new();

        if available_templates.is_empty() {
            output::dimmed("All templates have been assigned to categories.");
            return Ok(categories);
        }

        loop {
            let indent = "  ".repeat(depth);
            output::dimmed(&format!("{}Creating category (depth {})", indent, depth));

            // Category name
            let category_name = ctx.input.text(&format!("{}Category name:", indent), None)?;

            // Category description
            let category_desc_input = ctx
                .input
                .text(&format!("{}Category description (optional):", indent), None)?;
            let category_desc = if category_desc_input.is_empty() {
                None
            } else {
                Some(category_desc_input)
            };

            // Category ID (slug from name as default)
            let default_id = Self::slugify(&category_name);
            let category_id = ctx
                .input
                .text(&format!("{}Category ID:", indent), Some(&default_id))?;

            // Select templates for this category
            if available_templates.is_empty() {
                output::warning(&format!("{}No more templates available to assign.", indent));
                continue;
            }

            let selected_for_category = ctx.input.multi_select(
                &format!("{}Select templates for this category:", indent),
                available_templates.clone(),
                None,
            )?;

            // Build template references
            let mut category_templates: Vec<CategoryTemplate> = Vec::new();
            for option in &selected_for_category {
                if let Some((pack_name, template_name, _, _)) = template_map.get(option) {
                    category_templates.push(CategoryTemplate {
                        template_pack: pack_name.clone(),
                        template: template_name.clone(),
                    });
                }
            }

            // Remove assigned templates from available list
            available_templates.retain(|t| !selected_for_category.contains(t));

            // Ask about subcategories
            let create_subcategories = ctx.input.confirm(
                &format!("{}Create subcategories for this category?", indent),
                false,
            )?;

            let subcategories = if create_subcategories && !available_templates.is_empty() {
                Self::collect_categories_recursive(
                    ctx,
                    available_templates,
                    template_map,
                    depth + 1,
                )?
            } else {
                vec![]
            };

            // Add category
            categories.push(Category {
                id: category_id.clone(),
                name: category_name.clone(),
                description: category_desc,
                subcategories,
                templates: category_templates,
            });

            output::success(&format!("{}Category '{}' created", indent, category_name));
            output::blank();

            if available_templates.is_empty() {
                output::dimmed("All templates have been assigned.");
                break;
            }

            // Ask if they want to add another category at this level
            let add_another = ctx.input.confirm(
                &format!("{}Add another category at this level?", indent),
                false,
            )?;

            if !add_another {
                break;
            }
        }

        Ok(categories)
    }

    /// Collect executor backend configuration
    fn collect_executor_config(ctx: &Context) -> Result<ExecutorCollectionConfig> {
        // Select backend type
        let backend_types = vec![
            "s3".to_string(),
            "azurerm".to_string(),
            "gcs".to_string(),
            "kubernetes".to_string(),
            "pg".to_string(),
            "consul".to_string(),
            "http".to_string(),
            "local".to_string(),
        ];

        let backend_type = ctx.input.select("Backend type:", backend_types.clone())?;

        output::blank();
        output::dimmed(&format!("Configuring {} backend...", backend_type));
        output::blank();

        let mut backend_config: HashMap<String, serde_json::Value> = HashMap::new();

        // Collect backend-specific configuration
        match backend_type.as_str() {
            "s3" => {
                let bucket = ctx.input.text("S3 bucket name:", None)?;
                let key = ctx
                    .input
                    .text("State file key:", Some("terraform.tfstate"))?;
                let region = ctx.input.text("AWS region:", Some("us-east-1"))?;
                let encrypt = ctx.input.confirm("Enable encryption?", true)?;
                let dynamodb_table = ctx
                    .input
                    .text("DynamoDB table for locking (optional):", None)?;

                backend_config.insert("bucket".to_string(), serde_json::json!(bucket));
                backend_config.insert("key".to_string(), serde_json::json!(key));
                backend_config.insert("region".to_string(), serde_json::json!(region));
                backend_config.insert("encrypt".to_string(), serde_json::json!(encrypt));
                if !dynamodb_table.is_empty() {
                    backend_config.insert(
                        "dynamodb_table".to_string(),
                        serde_json::json!(dynamodb_table),
                    );
                }
            }
            "azurerm" => {
                let storage_account = ctx.input.text("Storage account name:", None)?;
                let container = ctx.input.text("Container name:", Some("tfstate"))?;
                let key = ctx
                    .input
                    .text("State file key:", Some("terraform.tfstate"))?;
                let resource_group = ctx.input.text("Resource group name:", None)?;

                backend_config.insert(
                    "storage_account_name".to_string(),
                    serde_json::json!(storage_account),
                );
                backend_config.insert("container_name".to_string(), serde_json::json!(container));
                backend_config.insert("key".to_string(), serde_json::json!(key));
                backend_config.insert(
                    "resource_group_name".to_string(),
                    serde_json::json!(resource_group),
                );
            }
            "gcs" => {
                let bucket = ctx.input.text("GCS bucket name:", None)?;
                let prefix = ctx
                    .input
                    .text("State file prefix:", Some("terraform/state"))?;

                backend_config.insert("bucket".to_string(), serde_json::json!(bucket));
                backend_config.insert("prefix".to_string(), serde_json::json!(prefix));
            }
            "kubernetes" => {
                let secret_suffix = ctx.input.text("Secret suffix:", Some("state"))?;
                let namespace = ctx.input.text("Namespace:", Some("default"))?;

                backend_config.insert(
                    "secret_suffix".to_string(),
                    serde_json::json!(secret_suffix),
                );
                backend_config.insert("namespace".to_string(), serde_json::json!(namespace));
            }
            "pg" => {
                let conn_str = ctx.input.text("PostgreSQL connection string:", None)?;
                let schema_name = ctx
                    .input
                    .text("Schema name:", Some("terraform_remote_state"))?;

                backend_config.insert("conn_str".to_string(), serde_json::json!(conn_str));
                backend_config.insert("schema_name".to_string(), serde_json::json!(schema_name));
            }
            "consul" => {
                let address = ctx.input.text("Consul address:", Some("127.0.0.1:8500"))?;
                let path = ctx.input.text("State path:", Some("terraform/state"))?;

                backend_config.insert("address".to_string(), serde_json::json!(address));
                backend_config.insert("path".to_string(), serde_json::json!(path));
            }
            "http" => {
                let address = ctx.input.text("HTTP endpoint address:", None)?;
                let lock_address = ctx.input.text("Lock endpoint address (optional):", None)?;
                let unlock_address = ctx
                    .input
                    .text("Unlock endpoint address (optional):", None)?;

                backend_config.insert("address".to_string(), serde_json::json!(address));
                if !lock_address.is_empty() {
                    backend_config
                        .insert("lock_address".to_string(), serde_json::json!(lock_address));
                }
                if !unlock_address.is_empty() {
                    backend_config.insert(
                        "unlock_address".to_string(),
                        serde_json::json!(unlock_address),
                    );
                }
            }
            "local" => {
                let path = ctx
                    .input
                    .text("Local state file path:", Some("terraform.tfstate"))?;
                backend_config.insert("path".to_string(), serde_json::json!(path));
            }
            _ => {}
        }

        // Build backend config JSON
        let mut backend_json = serde_json::Map::new();
        backend_json.insert("type".to_string(), serde_json::json!(backend_type));
        for (key, value) in backend_config {
            backend_json.insert(key, value);
        }

        // Build final config HashMap
        let mut config_map: HashMap<String, serde_json::Value> = HashMap::new();
        config_map.insert(
            "backend".to_string(),
            serde_json::Value::Object(backend_json),
        );

        Ok(ExecutorCollectionConfig {
            name: "opentofu".to_string(),
            config: config_map,
        })
    }

    /// Build categories from template selections (auto-generated)
    fn build_categories_from_selections(
        selected_options: &[String],
        template_map: &HashMap<String, (String, String, String, String)>,
    ) -> (Vec<Category>, HashMap<String, TemplatePackConfig>) {
        let mut categories_map: HashMap<String, Category> = HashMap::new();
        let mut template_packs_config: HashMap<String, TemplatePackConfig> = HashMap::new();

        for option in selected_options {
            if let Some((pack_name, template_name, api_version, kind)) = template_map.get(option) {
                let category_id = format!(
                    "{}_{}",
                    api_version.replace("/", "_").replace(".", "_"),
                    kind.to_lowercase()
                );

                let category = categories_map
                    .entry(category_id.clone())
                    .or_insert_with(|| Category {
                        id: category_id.clone(),
                        name: format!("{} ({})", kind, api_version),
                        description: Some(format!("Templates for {} resources", kind)),
                        subcategories: vec![],
                        templates: vec![],
                    });

                category.templates.push(CategoryTemplate {
                    template_pack: pack_name.clone(),
                    template: template_name.clone(),
                });

                let pack_config = template_packs_config.entry(pack_name.clone()).or_default();
                pack_config
                    .templates
                    .entry(template_name.clone())
                    .or_default();
            }
        }

        let categories: Vec<Category> = categories_map.into_values().collect();
        (categories, template_packs_config)
    }

    /// Build template_packs_config from categories
    fn build_template_packs_config(categories: &[Category]) -> HashMap<String, TemplatePackConfig> {
        let mut template_packs_config: HashMap<String, TemplatePackConfig> = HashMap::new();

        fn add_templates_from_category(
            category: &Category,
            config: &mut HashMap<String, TemplatePackConfig>,
        ) {
            for template_ref in &category.templates {
                let pack_config = config
                    .entry(template_ref.template_pack.clone())
                    .or_default();
                pack_config
                    .templates
                    .entry(template_ref.template.clone())
                    .or_default();
            }

            for subcategory in &category.subcategories {
                add_templates_from_category(subcategory, config);
            }
        }

        for category in categories {
            add_templates_from_category(category, &mut template_packs_config);
        }

        template_packs_config
    }

    /// Capitalize the first letter of a string
    fn capitalize_first(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }

    /// Convert a string to a slug (lowercase with underscores)
    fn slugify(s: &str) -> String {
        s.to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>()
            .split('_')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("_")
    }
}
