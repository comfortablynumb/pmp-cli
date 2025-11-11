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
use commands::{
    ApplyCommand, CreateCommand, DestroyCommand, FindCommand, GenerateCommand, InitCommand,
    PreviewCommand, RefreshCommand, UiCommand, UpdateCommand,
};

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
    #[command(
        long_about = "Initialize a new Infrastructure in the current directory\n\nExamples:\n  pmp init\n  pmp init --name \"My Infrastructure\"\n  pmp init --name \"Dev Projects\" --description \"Development infrastructure\""
    )]
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
    #[command(
        long_about = "Create a new project from a template\n\nExamples:\n  pmp create\n  pmp create --output ./my-project\n  pmp create --template-packs-paths /custom/packs1:/custom/packs2"
    )]
    Create {
        /// Output directory for the new project (defaults to current directory)
        #[arg(short, long)]
        output: Option<String>,

        /// Additional template packs directories to search (colon-separated)
        #[arg(short, long)]
        template_packs_paths: Option<String>,
    },

    /// Generate files from a template without creating a project structure
    #[command(
        long_about = "Generate files from a template without creating a project structure or requiring an infrastructure\n\nThis command allows you to quickly generate files from any template without the need for an infrastructure configuration.\nAll templates are available without filtering, and files are generated directly to the specified output directory.\n\nExamples:\n  pmp generate\n  pmp generate --template-pack my-pack --template my-template\n  pmp generate --output-dir ./output\n  pmp generate -p my-pack -t my-template -o ./output\n  pmp generate --template-packs-paths /custom/packs1:/custom/packs2"
    )]
    Generate {
        /// Template pack name (optional, will prompt if not specified)
        #[arg(short = 'p', long)]
        template_pack: Option<String>,

        /// Template name (optional, will prompt if not specified)
        #[arg(short = 't', long)]
        template: Option<String>,

        /// Output directory (defaults to current directory)
        #[arg(short = 'o', long)]
        output_dir: Option<String>,

        /// Additional template packs directories to search (colon-separated)
        #[arg(long)]
        template_packs_paths: Option<String>,
    },

    /// Preview changes (run IaC plan)
    #[command(
        long_about = "Preview changes (run IaC plan)\n\nYou can pass additional executor options after --:\n\nExamples:\n  pmp preview\n  pmp preview --path ./my-project\n  pmp preview -- -no-color\n  pmp preview -- -var=environment=prod"
    )]
    Preview {
        /// Path to the project directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Additional arguments to pass to the executor (after --)
        #[arg(last = true)]
        executor_args: Vec<String>,
    },

    /// Apply changes (run IaC apply)
    #[command(
        long_about = "Apply changes (run IaC apply)\n\nYou can pass additional executor options after --:\n\nExamples:\n  pmp apply\n  pmp apply --path ./my-project\n  pmp apply -- -auto-approve\n  pmp apply -- -var=environment=prod -auto-approve"
    )]
    Apply {
        /// Path to the project directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Additional arguments to pass to the executor (after --)
        #[arg(last = true)]
        executor_args: Vec<String>,
    },

    /// Destroy infrastructure (run IaC destroy)
    #[command(
        long_about = "Destroy infrastructure (run IaC destroy)\n\nWARNING: This will destroy all resources managed by the project!\nYou will be prompted for confirmation unless --yes is specified.\n\nYou can pass additional executor options after --:\n\nExamples:\n  pmp destroy\n  pmp destroy --yes\n  pmp destroy --path ./my-project\n  pmp destroy -- -auto-approve\n  pmp destroy --yes -- -var=environment=prod"
    )]
    Destroy {
        /// Path to the project directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,

        /// Additional arguments to pass to the executor (after --)
        #[arg(last = true)]
        executor_args: Vec<String>,
    },

    /// Refresh state (run IaC refresh)
    #[command(
        long_about = "Refresh state (run IaC refresh)\n\nUpdates the state file with the real infrastructure status without modifying resources.\n\nYou can pass additional executor options after --:\n\nExamples:\n  pmp refresh\n  pmp refresh --path ./my-project\n  pmp refresh -- -var=environment=prod"
    )]
    Refresh {
        /// Path to the project directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Additional arguments to pass to the executor (after --)
        #[arg(last = true)]
        executor_args: Vec<String>,
    },

    /// Find projects in an Infrastructure
    #[command(
        long_about = "Find projects in an Infrastructure\n\nExamples:\n  pmp find\n  pmp find --name my-api\n  pmp find --kind KubernetesWorkload"
    )]
    Find {
        /// Filter by project name (case-insensitive substring match)
        #[arg(short, long)]
        name: Option<String>,

        /// Filter by kind
        #[arg(short, long)]
        kind: Option<String>,
    },

    /// Update an existing project environment by regenerating files from the original template
    #[command(
        long_about = "Update an existing project environment by regenerating files from the original template\n\nExamples:\n  pmp update\n  pmp update --path ./my-project\n  pmp update --template-packs-paths /custom/packs1:/custom/packs2"
    )]
    Update {
        /// Path to the project directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Additional template packs directories to search (colon-separated)
        #[arg(short, long)]
        template_packs_paths: Option<String>,
    },

    /// Start the web UI server
    #[command(
        long_about = "Start the web UI server\n\nProvides a web-based interface for managing PMP projects, templates, and infrastructure.\nThe UI exposes all CLI functionality through an intuitive web interface.\n\nExamples:\n  pmp ui\n  pmp ui --port 3000\n  pmp ui --host 0.0.0.0 --port 8080"
    )]
    Ui {
        /// Port to bind the server to (defaults to 8080)
        #[arg(short, long)]
        port: Option<u16>,

        /// Host to bind the server to (defaults to 127.0.0.1)
        #[arg(long)]
        host: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let ctx = context::Context::new();

    match cli.command {
        Commands::Init {
            name,
            description,
            template_packs_paths,
        } => {
            InitCommand::execute(
                &ctx,
                name.as_deref(),
                description.as_deref(),
                template_packs_paths.as_deref(),
            )?;
        }
        Commands::Create {
            output,
            template_packs_paths,
        } => {
            CreateCommand::execute(&ctx, output.as_deref(), template_packs_paths.as_deref())?;
        }
        Commands::Generate {
            template_pack,
            template,
            output_dir,
            template_packs_paths,
        } => {
            GenerateCommand::execute(
                &ctx,
                template_pack.as_deref(),
                template.as_deref(),
                output_dir.as_deref(),
                template_packs_paths.as_deref(),
            )?;
        }
        Commands::Preview {
            path,
            executor_args,
        } => {
            PreviewCommand::execute(&ctx, path.as_deref(), &executor_args)?;
        }
        Commands::Apply {
            path,
            executor_args,
        } => {
            ApplyCommand::execute(&ctx, path.as_deref(), &executor_args)?;
        }
        Commands::Destroy {
            path,
            yes,
            executor_args,
        } => {
            DestroyCommand::execute(&ctx, path.as_deref(), yes, &executor_args)?;
        }
        Commands::Refresh {
            path,
            executor_args,
        } => {
            RefreshCommand::execute(&ctx, path.as_deref(), &executor_args)?;
        }
        Commands::Find { name, kind } => {
            FindCommand::execute(&ctx, name.as_deref(), kind.as_deref())?;
        }
        Commands::Update {
            path,
            template_packs_paths,
        } => {
            UpdateCommand::execute(&ctx, path.as_deref(), template_packs_paths.as_deref())?;
        }
        Commands::Ui { port, host } => {
            UiCommand::execute(&ctx, port, host)?;
        }
    }

    Ok(())
}
