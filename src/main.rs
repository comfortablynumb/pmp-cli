mod commands;
mod hooks;
mod iac;
mod schema;
mod template;

use anyhow::Result;
use clap::{Parser, Subcommand};
use commands::{ApplyCommand, CreateCommand, PreviewCommand};

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
    Create {
        /// Output directory for the new project (defaults to current directory)
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Preview changes (run IaC plan)
    Preview {
        /// Path to the project directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Apply changes (run IaC apply)
    Apply {
        /// Path to the project directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Create { output } => {
            CreateCommand::execute(output.as_deref())?;
        }
        Commands::Preview { path } => {
            PreviewCommand::execute(path.as_deref())?;
        }
        Commands::Apply { path } => {
            ApplyCommand::execute(path.as_deref())?;
        }
    }

    Ok(())
}
