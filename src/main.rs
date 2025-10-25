mod collection;
mod commands;
mod executor;
mod hooks;
mod output;
mod schema;
mod template;

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
    /// Initialize a new ProjectCollection
    #[command(long_about = "Initialize a new ProjectCollection in the current directory\n\nExamples:\n  pmp init\n  pmp init --name \"My Infrastructure\"\n  pmp init --name \"Dev Projects\" --description \"Development infrastructure\"")]
    Init {
        /// Name of the project collection (defaults to "My Infrastructure")
        #[arg(short, long)]
        name: Option<String>,

        /// Description of the project collection
        #[arg(short, long)]
        description: Option<String>,

        /// Additional templates directory to search
        #[arg(short, long)]
        templates_path: Option<String>,
    },

    /// Create a new project from a template
    #[command(long_about = "Create a new project from a template\n\nExamples:\n  pmp create\n  pmp create --output ./my-project\n  pmp create --templates-path /custom/templates")]
    Create {
        /// Output directory for the new project (defaults to current directory)
        #[arg(short, long)]
        output: Option<String>,

        /// Additional templates directory to search
        #[arg(short, long)]
        templates_path: Option<String>,
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

    /// Find projects in a ProjectCollection
    #[command(long_about = "Find projects in a ProjectCollection\n\nExamples:\n  pmp find\n  pmp find --name my-api\n  pmp find --kind KubernetesWorkload")]
    Find {
        /// Filter by project name (case-insensitive substring match)
        #[arg(short, long)]
        name: Option<String>,

        /// Filter by kind
        #[arg(short, long)]
        kind: Option<String>,
    },

    /// Update an existing project environment by regenerating files from the original template
    #[command(long_about = "Update an existing project environment by regenerating files from the original template\n\nExamples:\n  pmp update\n  pmp update --path ./my-project\n  pmp update --templates-path /custom/templates")]
    Update {
        /// Path to the project directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Additional templates directory to search
        #[arg(short, long)]
        templates_path: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { name, description, templates_path } => {
            InitCommand::execute(name.as_deref(), description.as_deref(), templates_path.as_deref())?;
        }
        Commands::Create { output, templates_path } => {
            CreateCommand::execute(output.as_deref(), templates_path.as_deref())?;
        }
        Commands::Preview { path } => {
            PreviewCommand::execute(path.as_deref())?;
        }
        Commands::Apply { path } => {
            ApplyCommand::execute(path.as_deref())?;
        }
        Commands::Find { name, kind } => {
            FindCommand::execute(
                name.as_deref(),
                kind.as_deref(),
            )?;
        }
        Commands::Update { path, templates_path } => {
            UpdateCommand::execute(path.as_deref(), templates_path.as_deref())?;
        }
    }

    Ok(())
}
