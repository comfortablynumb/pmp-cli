mod collection;
mod commands;
mod context;
mod executor;
mod hooks;
mod output;
mod schema;
mod template;
mod traits;

use anyhow::Result;
use clap::{Parser, Subcommand};
use commands::{ApplyCommand, CreateCommand, FindCommand, InitCommand, PreviewCommand, UpdateCommand};

#[derive(Parser)]
#[command(name = "pmp")]
#[command(about = "Poor Man's Platform - A CLI for managing Infrastructure as Code projects", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new Infrastructure
    #[command(long_about = "Initialize a new Infrastructure in the current directory\n\nExamples:\n  pmp init\n  pmp init --name \"My Infrastructure\"\n  pmp init --name \"Dev Projects\" --description \"Development infrastructure\"")]
    Init {
        /// Name of the infrastructure (defaults to "My Infrastructure")
        #[arg(short, long)]
        name: Option<String>,

        /// Description of the infrastructure
        #[arg(short, long)]
        description: Option<String>,

        /// Additional template packs directories to search (colon-separated)
        #[arg(short, long)]
        template_packs_paths: Option<String>,
    },

    /// Create a new project from a template
    #[command(long_about = "Create a new project from a template\n\nExamples:\n  pmp create\n  pmp create --output ./my-project\n  pmp create --template-packs-paths /custom/packs1:/custom/packs2")]
    Create {
        /// Output directory for the new project (defaults to current directory)
        #[arg(short, long)]
        output: Option<String>,

        /// Additional template packs directories to search (colon-separated)
        #[arg(short, long)]
        template_packs_paths: Option<String>,
    },

    /// Preview changes (run IaC plan)
    #[command(long_about = "Preview changes (run IaC plan)\n\nExamples:\n  pmp preview\n  pmp preview --path ./my-project")]
    Preview {
        /// Path to the project directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Apply changes (run IaC apply)
    #[command(long_about = "Apply changes (run IaC apply)\n\nExamples:\n  pmp apply\n  pmp apply --path ./my-project")]
    Apply {
        /// Path to the project directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Find projects in an Infrastructure
    #[command(long_about = "Find projects in an Infrastructure\n\nExamples:\n  pmp find\n  pmp find --name my-api\n  pmp find --kind KubernetesWorkload")]
    Find {
        /// Filter by project name (case-insensitive substring match)
        #[arg(short, long)]
        name: Option<String>,

        /// Filter by kind
        #[arg(short, long)]
        kind: Option<String>,
    },

    /// Update an existing project environment by regenerating files from the original template
    #[command(long_about = "Update an existing project environment by regenerating files from the original template\n\nExamples:\n  pmp update\n  pmp update --path ./my-project\n  pmp update --template-packs-paths /custom/packs1:/custom/packs2")]
    Update {
        /// Path to the project directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Additional template packs directories to search (colon-separated)
        #[arg(short, long)]
        template_packs_paths: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let ctx = context::Context::new();

    match cli.command {
        Commands::Init { name, description, template_packs_paths } => {
            InitCommand::execute(&ctx, name.as_deref(), description.as_deref(), template_packs_paths.as_deref())?;
        }
        Commands::Create { output, template_packs_paths } => {
            CreateCommand::execute(&ctx, output.as_deref(), template_packs_paths.as_deref())?;
        }
        Commands::Preview { path } => {
            PreviewCommand::execute(&ctx, path.as_deref())?;
        }
        Commands::Apply { path } => {
            ApplyCommand::execute(&ctx, path.as_deref())?;
        }
        Commands::Find { name, kind } => {
            FindCommand::execute(
                &ctx,
                name.as_deref(),
                kind.as_deref(),
            )?;
        }
        Commands::Update { path, template_packs_paths } => {
            UpdateCommand::execute(&ctx, path.as_deref(), template_packs_paths.as_deref())?;
        }
    }

    Ok(())
}
