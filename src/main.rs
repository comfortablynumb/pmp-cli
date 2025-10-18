mod collection;
mod commands;
mod hooks;
mod iac;
mod schema;
mod template;

use anyhow::Result;
use clap::{Parser, Subcommand};
use commands::{ApplyCommand, CreateCommand, FindCommand, PreviewCommand};

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
    #[command(long_about = "Find projects in a ProjectCollection\n\nExamples:\n  pmp find\n  pmp find --name my-api\n  pmp find --category workload\n  pmp find --kind Infrastructure")]
    Find {
        /// Filter by project name (case-insensitive substring match)
        #[arg(short, long)]
        name: Option<String>,

        /// Filter by category
        #[arg(short, long)]
        category: Option<String>,

        /// Filter by kind
        #[arg(short, long)]
        kind: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Create { output, templates_path } => {
            CreateCommand::execute(output.as_deref(), templates_path.as_deref())?;
        }
        Commands::Preview { path } => {
            PreviewCommand::execute(path.as_deref())?;
        }
        Commands::Apply { path } => {
            ApplyCommand::execute(path.as_deref())?;
        }
        Commands::Find { name, category, kind } => {
            FindCommand::execute(
                name.as_deref(),
                category.as_deref(),
                kind.as_deref(),
            )?;
        }
    }

    Ok(())
}
