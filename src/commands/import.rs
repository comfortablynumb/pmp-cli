use anyhow::Result;
use clap::{Args, Subcommand};
use std::path::PathBuf;

use crate::context::Context;
use crate::import::analyzer::ProjectAnalysis;

/// Import existing infrastructure into PMP
#[derive(Debug, Args)]
pub struct ImportCommand {
    #[command(subcommand)]
    subcommand: ImportSubcommand,
}

#[derive(Debug, Subcommand)]
enum ImportSubcommand {
    /// Import an existing Terraform/OpenTofu project directory
    Project(ProjectImportArgs),

    /// Import from an existing state file
    State(StateImportArgs),

    /// Import specific resources by address
    Resource(ResourceImportArgs),

    /// Bulk import multiple projects from configuration file
    Bulk(BulkImportArgs),
}

#[derive(Debug, Args)]
struct ProjectImportArgs {
    /// Path to the Terraform/OpenTofu project directory
    source_path: PathBuf,

    /// Project name (defaults to directory name)
    #[arg(short, long)]
    name: Option<String>,

    /// Environment name
    #[arg(short, long)]
    environment: Option<String>,

    /// Template pack to use (optional)
    #[arg(long)]
    template_pack: Option<String>,

    /// Template name to use (optional)
    #[arg(long)]
    template_name: Option<String>,

    /// File handling strategy: copy, move, symlink, template_convert
    #[arg(long, default_value = "copy")]
    file_strategy: String,

    /// Whether to import state file
    #[arg(long, default_value = "true")]
    import_state: bool,

    /// Dry-run mode (show what would happen)
    #[arg(long)]
    dry_run: bool,

    /// Skip interactive prompts and use defaults
    #[arg(short, long)]
    yes: bool,
}

#[derive(Debug, Args)]
struct StateImportArgs {
    /// Path to the state file
    state_path: PathBuf,

    /// Project name
    #[arg(short, long)]
    name: String,

    /// Environment name
    #[arg(short, long)]
    environment: Option<String>,

    /// Source Terraform files directory (optional)
    #[arg(long)]
    source_dir: Option<PathBuf>,

    /// Dry-run mode
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Args)]
struct ResourceImportArgs {
    /// Resource address(es) to import (comma-separated)
    addresses: String,

    /// Project name
    #[arg(short, long)]
    project: String,

    /// Environment name
    #[arg(short, long)]
    environment: String,

    /// Resource ID in cloud provider
    #[arg(long)]
    id: Option<String>,
}

#[derive(Debug, Args)]
struct BulkImportArgs {
    /// Path to import configuration file
    config_path: PathBuf,

    /// Dry-run mode
    #[arg(long)]
    dry_run: bool,

    /// Parallel import (number of concurrent imports)
    #[arg(long, default_value = "1")]
    parallel: usize,
}

impl ImportCommand {
    pub fn execute(self, ctx: &Context) -> Result<()> {
        match self.subcommand {
            ImportSubcommand::Project(args) => Self::import_project(ctx, args),
            ImportSubcommand::State(args) => Self::import_state(ctx, args),
            ImportSubcommand::Resource(args) => Self::import_resource(ctx, args),
            ImportSubcommand::Bulk(args) => Self::import_bulk(ctx, args),
        }
    }

    fn import_project(ctx: &Context, args: ProjectImportArgs) -> Result<()> {
        ctx.output.info("🔍 Analyzing Terraform project...");

        // Validate source path exists
        if !args.source_path.exists() {
            return Err(anyhow::anyhow!(
                "Source path does not exist: {}",
                args.source_path.display()
            ));
        }

        // Create project importer
        let importer = ProjectImporter::new(ctx, args)?;

        // Analyze source project
        let analysis = importer.analyze_source()?;

        // Display analysis summary
        importer.display_analysis(&analysis)?;

        // Match to template (interactive or auto)
        let template_match = importer.match_template(&analysis)?;

        // Get project configuration
        let project_config = importer.get_project_config(&template_match)?;

        // Preview import
        importer.preview_import(&project_config)?;

        // Confirm import
        if !importer.confirm_import()? {
            ctx.output.info("Import cancelled");
            return Ok(());
        }

        // Execute import
        importer.execute_import(&project_config)?;

        ctx.output.success("✅ Import completed successfully!");

        // Display next steps
        importer.display_next_steps(&project_config)?;

        Ok(())
    }

    fn import_state(_ctx: &Context, _args: StateImportArgs) -> Result<()> {
        // TODO: Implement state import
        Err(anyhow::anyhow!("State import not yet implemented"))
    }

    fn import_resource(_ctx: &Context, _args: ResourceImportArgs) -> Result<()> {
        // TODO: Implement resource import
        Err(anyhow::anyhow!("Resource import not yet implemented"))
    }

    fn import_bulk(_ctx: &Context, _args: BulkImportArgs) -> Result<()> {
        // TODO: Implement bulk import
        Err(anyhow::anyhow!("Bulk import not yet implemented"))
    }
}

/// Project importer implementation
struct ProjectImporter<'a> {
    ctx: &'a Context,
    args: ProjectImportArgs,
}

impl<'a> ProjectImporter<'a> {
    fn new(ctx: &'a Context, args: ProjectImportArgs) -> Result<Self> {
        Ok(Self { ctx, args })
    }

    fn analyze_source(&self) -> Result<ProjectAnalysis> {
        use crate::import::analyzer::ProjectAnalyzer;

        let analyzer = ProjectAnalyzer::new(&self.args.source_path);
        analyzer.analyze()
    }

    fn display_analysis(&self, analysis: &ProjectAnalysis) -> Result<()> {
        self.ctx.output.info(&format!(
            "   ✓ Found {} .tf files",
            analysis.terraform_files.len()
        ));

        if analysis.has_state {
            self.ctx.output.info("   ✓ Found terraform.tfstate");

            if let Some(resources) = &analysis.resources {
                self.ctx.output.info(&format!(
                    "   ✓ Detected {} resources across {} providers",
                    resources.len(),
                    analysis.providers.len()
                ));
            }
        }

        // Display resource summary
        if let Some(resources) = &analysis.resources {
            self.ctx.output.info("\n📊 Analysis Summary:");

            if !analysis.providers.is_empty() {
                let provider_list: Vec<String> = analysis
                    .providers
                    .iter()
                    .map(|p| format!("{} ({})", p.name, p.version.as_deref().unwrap_or("unknown")))
                    .collect();
                self.ctx
                    .output
                    .info(&format!("   Providers: {}", provider_list.join(", ")));
            }

            // Group resources by type
            let mut type_counts = std::collections::HashMap::new();
            for resource in resources {
                *type_counts.entry(&resource.resource_type).or_insert(0) += 1;
            }

            if !type_counts.is_empty() {
                self.ctx.output.info("   Resources:");

                for (resource_type, count) in type_counts {
                    self.ctx
                        .output
                        .info(&format!("     - {} ({})", resource_type, count));
                }
            }
        }

        Ok(())
    }

    fn match_template(&self, _analysis: &ProjectAnalysis) -> Result<TemplateMatch> {
        // For now, return a simple match
        // TODO: Implement template matching logic
        Ok(TemplateMatch {
            template_pack: self.args.template_pack.clone(),
            template_name: self.args.template_name.clone(),
            confidence: 1.0,
            matching_resources: vec![],
            missing_resources: vec![],
            extra_resources: vec![],
        })
    }

    fn get_project_config(&self, _template_match: &TemplateMatch) -> Result<ProjectConfig> {
        // Get project name
        let project_name = if let Some(name) = &self.args.name {
            name.clone()
        } else {
            // Use directory name as default
            self.args
                .source_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("imported-project")
                .to_string()
        };

        // Get environment
        let environment = if let Some(env) = &self.args.environment {
            env.clone()
        } else {
            // Prompt for environment
            "production".to_string()
        };

        Ok(ProjectConfig {
            name: project_name,
            environment,
            source_path: self.args.source_path.clone(),
            file_strategy: self.args.file_strategy.clone(),
            import_state: self.args.import_state,
        })
    }

    fn preview_import(&self, config: &ProjectConfig) -> Result<()> {
        self.ctx.output.info("\n📝 Preview:");
        self.ctx.output.info("   Will create:");

        let base_path = format!(
            "collection/projects/{}/environments/{}",
            config.name, config.environment
        );

        self.ctx.output.info(&format!(
            "   ✓ collection/projects/{}/.pmp.project.yaml",
            config.name
        ));
        self.ctx
            .output
            .info(&format!("   ✓ {}/.pmp.environment.yaml", base_path));
        self.ctx
            .output
            .info(&format!("   ✓ {}/*.tf (terraform files)", base_path));

        if config.import_state {
            self.ctx
                .output
                .info(&format!("   ✓ {}/terraform.tfstate", base_path));
        }

        self.ctx
            .output
            .info(&format!("   ✓ {}/_common.tf", base_path));

        Ok(())
    }

    fn confirm_import(&self) -> Result<bool> {
        if self.args.yes || self.args.dry_run {
            return Ok(!self.args.dry_run);
        }

        self.ctx
            .input
            .confirm("? Proceed with import?", Some(true))
    }

    fn execute_import(&self, _config: &ProjectConfig) -> Result<()> {
        if self.args.dry_run {
            self.ctx.output.info("Dry-run mode: no changes made");
            return Ok(());
        }

        // TODO: Implement actual import logic
        self.ctx.output.info("Importing project...");

        Ok(())
    }

    fn display_next_steps(&self, config: &ProjectConfig) -> Result<()> {
        self.ctx.output.info("\n   Next steps:");
        self.ctx.output.info(&format!(
            "   1. Review generated files in: collection/projects/{}",
            config.name
        ));
        self.ctx.output.info("   2. Run 'pmp preview' to verify state");
        self.ctx
            .output
            .info("   3. Run 'pmp apply' to manage with PMP");

        if self.args.file_strategy == "copy" {
            self.ctx.output.info(&format!(
                "\n   ⚠️  Note: Original files remain at {}",
                self.args.source_path.display()
            ));
        }

        Ok(())
    }
}

#[allow(dead_code)]
struct TemplateMatch {
    template_pack: Option<String>,
    template_name: Option<String>,
    confidence: f64,
    matching_resources: Vec<String>,
    missing_resources: Vec<String>,
    extra_resources: Vec<String>,
}

#[allow(dead_code)]
struct ProjectConfig {
    name: String,
    environment: String,
    source_path: PathBuf,
    file_strategy: String,
    import_state: bool,
}
