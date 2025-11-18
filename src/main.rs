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
    ApplyCommand, AuditCommand, BackupCommand, CiCommand, CiDetectChangesCommand, CloneCommand,
    CreateCommand, DepsCommand, DestroyCommand, DevExCommand, DisasterRecoveryCommand,
    DriftCommand, EnvCommand, FindCommand, GenerateCommand, GraphCommand, InfrastructureCommand,
    LockCommand, MonitorCommand, PolicyCommand, PreviewCommand, ProviderCommand, RefreshCommand,
    ReviewCommand, SearchCommand, StateCommand, TagsCommand, TemplateCommand,
    TemplateMgmtCommand, TestCommand, UiCommand, UpdateCommand, WorkspaceCommand,
};

#[derive(Parser)]
#[command(name = "pmp")]
#[command(about = "Poor Man's Platform - A CLI for managing Infrastructure as Code projects", long_about = None)]
#[command(version)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum InfrastructureSubcommands {
    /// Initialize a new infrastructure from template
    #[command(
        long_about = "Initialize a new infrastructure from template\n\nExamples:\n  pmp infrastructure init\n  pmp infrastructure init --output ./my-infra\n  pmp infrastructure init --template-packs-paths /custom/packs"
    )]
    Init {
        /// Output directory (optional, defaults to current directory)
        #[arg(short, long)]
        output: Option<String>,

        /// Additional template packs directories to search (colon-separated)
        #[arg(long)]
        template_packs_paths: Option<String>,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum ProjectSubcommands {
    /// Create a new project
    #[command(
        long_about = "Create a new project from a template\n\nExamples:\n  pmp project create\n  pmp project create --output ./my-project\n  pmp project create --template-packs-paths /custom/packs"
    )]
    Create {
        /// Output directory for the project (optional)
        #[arg(short, long)]
        output: Option<String>,

        /// Additional template packs directories to search (colon-separated)
        #[arg(long)]
        template_packs_paths: Option<String>,
    },

    /// Find projects in an Infrastructure
    #[command(
        long_about = "Find projects in an Infrastructure\n\nExamples:\n  pmp project find\n  pmp project find --name my-api\n  pmp project find --kind KubernetesWorkload"
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
        long_about = "Update an existing project environment by regenerating files from the original template\n\nExamples:\n  pmp project update\n  pmp project update --path ./my-project\n  pmp project update --template-packs-paths /custom/packs1:/custom/packs2"
    )]
    Update {
        /// Path to the project directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Additional template packs directories to search (colon-separated)
        #[arg(short, long)]
        template_packs_paths: Option<String>,
    },

    /// Clone an existing project
    #[command(
        long_about = "Clone an existing project with a new name\n\nExamples:\n  pmp project clone my-api new-api\n  pmp project clone --source my-api --name new-api\n  pmp project clone my-api new-api --environment dev"
    )]
    Clone {
        /// Source project name (optional, prompts if not specified)
        source: Option<String>,

        /// New project name
        name: String,

        /// Environment to clone (optional, prompts if not specified)
        #[arg(short, long)]
        environment: Option<String>,
    },

    /// Preview changes (run IaC plan)
    #[command(
        long_about = "Preview changes (run IaC plan)\n\nYou can pass additional executor options after --:\n\nExamples:\n  pmp project preview\n  pmp project preview --path ./my-project\n  pmp project preview -- -no-color\n  pmp project preview -- -var=environment=prod"
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
        long_about = "Apply changes (run IaC apply)\n\nYou can pass additional executor options after --:\n\nExamples:\n  pmp project apply\n  pmp project apply --path ./my-project\n  pmp project apply -- -auto-approve\n  pmp project apply -- -var=environment=prod -auto-approve"
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
        long_about = "Destroy infrastructure (run IaC destroy)\n\nWARNING: This will destroy all resources managed by the project!\nYou will be prompted for confirmation unless --yes is specified.\n\nYou can pass additional executor options after --:\n\nExamples:\n  pmp project destroy\n  pmp project destroy --yes\n  pmp project destroy --path ./my-project\n  pmp project destroy -- -auto-approve\n  pmp project destroy --yes -- -var=environment=prod"
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
        long_about = "Refresh state (run IaC refresh)\n\nUpdates the state file with the real infrastructure status without modifying resources.\n\nYou can pass additional executor options after --:\n\nExamples:\n  pmp project refresh\n  pmp project refresh --path ./my-project\n  pmp project refresh -- -var=environment=prod"
    )]
    Refresh {
        /// Path to the project directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Additional arguments to pass to the executor (after --)
        #[arg(last = true)]
        executor_args: Vec<String>,
    },

    /// Visualize dependency graph
    #[command(
        long_about = "Visualize project dependency graphs\n\nSupports multiple output formats:\n- ASCII: Terminal-friendly tree visualization\n- Mermaid: Mermaid.js diagram format\n- DOT: GraphViz DOT format\n\nExamples:\n  pmp project graph\n  pmp project graph --all\n  pmp project graph --format mermaid --output graph.mmd\n  pmp project graph --format dot --output graph.dot"
    )]
    Graph {
        /// Path to the project directory (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Output format (ascii, mermaid, dot)
        #[arg(short, long)]
        format: Option<String>,

        /// Output file (defaults to stdout)
        #[arg(short, long)]
        output: Option<String>,

        /// Show all projects in the infrastructure
        #[arg(short, long)]
        all: bool,
    },

    /// Dependency analysis and management
    #[command(
        long_about = "Analyze and manage project dependencies\n\nSubcommands:\n- analyze: Comprehensive dependency analysis\n- impact: Show projects affected by changes\n- validate: Validate dependency chains\n- order: Show optimal deployment order\n- why: Explain dependency relationships\n\nExamples:\n  pmp project deps analyze\n  pmp project deps impact my-api\n  pmp project deps validate\n  pmp project deps order\n  pmp project deps why my-api"
    )]
    Deps {
        #[command(subcommand)]
        command: DepsSubcommands,
    },

    /// Drift detection and reconciliation
    #[command(
        long_about = "Detect and reconcile infrastructure drift\n\nSubcommands:\n- detect: Detect drift in infrastructure\n- report: Generate drift report\n- reconcile: Reconcile drift by applying changes\n\nExamples:\n  pmp project drift detect\n  pmp project drift detect --path ./my-project/environments/dev\n  pmp project drift report --format json --output drift-report.json\n  pmp project drift reconcile --auto-approve"
    )]
    Drift {
        #[command(subcommand)]
        command: DriftSubcommands,
    },

    /// Policy validation and security scanning
    #[command(
        long_about = "Validate policies and scan for security issues\n\nSubcommands:\n- validate: Validate against organizational policies\n- scan: Run security scanning tools\n\nExamples:\n  pmp project policy validate\n  pmp project policy validate --policy naming\n  pmp project policy scan --scanner tfsec\n  pmp project policy scan --scanner checkov"
    )]
    Policy {
        #[command(subcommand)]
        command: PolicySubcommands,
    },

    /// State management and drift detection
    #[command(
        long_about = "Manage infrastructure state and detect drift\n\nSubcommands:\n- list: Show state across all projects\n- drift: Detect configuration drift\n- lock: Lock state for a project\n- unlock: Unlock state for a project\n- sync: Sync remote state\n\nExamples:\n  pmp project state list\n  pmp project state drift\n  pmp project state lock my-project\n  pmp project state unlock my-project --force"
    )]
    State {
        #[command(subcommand)]
        command: StateSubcommands,
    },

    /// Environment management
    #[command(
        long_about = "Manage and compare environments\n\nSubcommands:\n- diff: Compare two environments\n- promote: Promote configuration between environments\n- sync: Synchronize common settings\n- variables: Manage environment variables\n\nExamples:\n  pmp project env diff dev staging\n  pmp project env promote dev staging\n  pmp project env sync\n  pmp project env variables --environment production"
    )]
    Env {
        #[command(subcommand)]
        command: EnvSubcommands,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort commands alphabetically
enum Commands {
    /// Infrastructure management commands
    #[command(
        long_about = "Initialize infrastructure from template\n\nExamples:\n  pmp infrastructure init\n  pmp infrastructure init --output ./my-infra\n  pmp infrastructure init --template-packs-paths /custom/packs"
    )]
    Infrastructure {
        #[command(subcommand)]
        command: InfrastructureSubcommands,
    },

    /// Project management commands
    #[command(
        long_about = "Manage projects within an infrastructure\n\nSubcommands:\n- create: Create a new project\n- find: Find existing projects\n- update: Update project configuration\n- clone: Clone an existing project\n- apply: Apply infrastructure changes\n- preview: Preview infrastructure changes\n- destroy: Destroy infrastructure\n- refresh: Refresh infrastructure state\n- graph: Visualize dependency graph\n- deps: Manage dependencies\n- state: Manage state\n- env: Manage environments\n\nExamples:\n  pmp project create\n  pmp project find --name my-api\n  pmp project apply"
    )]
    Project {
        #[command(subcommand)]
        command: ProjectSubcommands,
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

    /// CI/CD pipeline generation
    #[command(
        long_about = "Generate CI/CD pipeline configurations\n\nSupports:\n- GitHub Actions\n- GitLab CI\n- Jenkins\n\nExamples:\n  pmp ci generate github-actions\n  pmp ci generate gitlab-ci --output .gitlab-ci.yml\n  pmp ci generate jenkins --output Jenkinsfile"
    )]
    Ci {
        #[command(subcommand)]
        command: CiSubcommands,
    },

    /// Template management and scaffolding
    #[command(
        long_about = "Create and manage template packs\n\nExamples:\n  pmp template scaffold\n  pmp template scaffold --output ./my-templates"
    )]
    Template {
        #[command(subcommand)]
        command: TemplateSubcommands,
    },

    /// Testing and validation
    #[command(
        long_about = "Run tests and validate infrastructure\n\nSubcommands:\n- test: Run integration tests\n- validate-plan: Validate plan syntax and semantics\n- dry-run: Simulate apply without changes\n- cost-estimate: Estimate infrastructure costs\n- compliance-report: Generate compliance reports\n\nExamples:\n  pmp test\n  pmp test validate-plan\n  pmp test dry-run\n  pmp test cost-estimate --format json\n  pmp test compliance-report soc2"
    )]
    Test {
        #[command(subcommand)]
        command: TestSubcommands,
    },

    /// Developer experience tools
    #[command(
        long_about = "Tools for exploring and working with infrastructure\n\nSubcommands:\n- shell: Interactive shell\n- docs: Generate documentation\n- graph-viz: Visualize dependency graphs\n- export: Export to other formats\n- import: Import existing infrastructure\n\nExamples:\n  pmp devex shell\n  pmp devex docs --format markdown --output README.md\n  pmp devex graph-viz --format mermaid\n  pmp devex export helm --output chart.yaml\n  pmp devex import terraform ./existing-infra"
    )]
    DevEx {
        #[command(subcommand)]
        command: DevExSubcommands,
    },

    /// Template management
    #[command(
        long_about = "Manage and develop templates\n\nSubcommands:\n- validate: Validate template definitions\n- test: Test template rendering\n- publish: Publish template to registry\n- clone: Clone and customize templates\n- plugin-develop: Develop new plugins\n\nExamples:\n  pmp template-mgmt validate my-pack my-template\n  pmp template-mgmt test my-pack my-template\n  pmp template-mgmt publish my-pack --version 1.0.0\n  pmp template-mgmt clone source-pack source-template target-pack target-template\n  pmp template-mgmt plugin-develop my-pack my-plugin"
    )]
    TemplateMgmt {
        #[command(subcommand)]
        command: TemplateMgmtSubcommands,
    },

    /// Multi-cloud provider extensions
    #[command(
        long_about = "Manage cloud providers and plugins\n\nSubcommands:\n- install: Install provider plugin\n- connect: Configure cloud credentials\n- secrets: Manage secrets\n- cost-optimize: Analyze cost optimization\n\nExamples:\n  pmp provider install aws vpc\n  pmp provider connect aws --profile default\n  pmp provider secrets list\n  pmp provider cost-optimize --format json"
    )]
    Provider {
        #[command(subcommand)]
        command: ProviderSubcommands,
    },

    /// Monitor infrastructure and projects
    #[command(
        long_about = "Monitor infrastructure health, metrics, and alerts\n\nSubcommands:\n- dashboard: Display monitoring dashboard\n- metrics: Show infrastructure metrics\n- alerts: Manage alerts and notifications\n\nExamples:\n  pmp monitor dashboard\n  pmp monitor metrics --project my-api\n  pmp monitor alerts --environment production"
    )]
    Monitor {
        #[command(subcommand)]
        command: MonitorSubcommands,
    },

    /// Audit infrastructure changes and operations
    #[command(
        long_about = "Audit infrastructure changes and track operations\n\nSubcommands:\n- log: View audit logs\n- diff: Compare infrastructure states\n\nExamples:\n  pmp audit log\n  pmp audit log --project my-api --limit 50\n  pmp audit diff --from 2025-01-01 --to 2025-01-15"
    )]
    Audit {
        #[command(subcommand)]
        command: AuditSubcommands,
    },

    /// Lock management for infrastructure operations
    #[command(
        long_about = "Manage locks for infrastructure operations\n\nSubcommands:\n- acquire: Acquire a lock\n- release: Release a lock\n- status: Check lock status\n\nExamples:\n  pmp lock acquire my-api\n  pmp lock release my-api\n  pmp lock status"
    )]
    Lock {
        #[command(subcommand)]
        command: LockSubcommands,
    },

    /// Review and approval workflow
    #[command(
        long_about = "Manage review and approval workflows\n\nSubcommands:\n- request: Request review for changes\n- approve: Approve pending changes\n- list: List pending reviews\n\nExamples:\n  pmp review request\n  pmp review approve review-123\n  pmp review list --status pending"
    )]
    Review {
        #[command(subcommand)]
        command: ReviewSubcommands,
    },

    /// Workspace management
    #[command(
        long_about = "Manage workspaces for isolation\n\nSubcommands:\n- list: List available workspaces\n- new: Create a new workspace\n- select: Switch to a workspace\n- delete: Delete a workspace\n- show: Show current workspace\n\nExamples:\n  pmp workspace list\n  pmp workspace new feature-branch\n  pmp workspace select production\n  pmp workspace delete old-workspace\n  pmp workspace show"
    )]
    Workspace {
        #[command(subcommand)]
        command: WorkspaceSubcommands,
    },

    /// Backup and restore operations
    #[command(
        long_about = "Backup and restore infrastructure state\n\nSubcommands:\n- create: Create a backup\n- restore: Restore from backup\n- list: List available backups\n- delete: Delete a backup\n\nExamples:\n  pmp backup create\n  pmp backup create --project my-api --environment production\n  pmp backup restore backup-20250116-143000\n  pmp backup list\n  pmp backup delete backup-20250116-143000"
    )]
    Backup {
        #[command(subcommand)]
        command: BackupSubcommands,
    },

    /// Disaster recovery planning and testing
    #[command(
        long_about = "Plan and test disaster recovery procedures\n\nSubcommands:\n- plan: Create disaster recovery plan\n- test: Test disaster recovery procedures\n- list: List disaster recovery plans\n\nExamples:\n  pmp disaster-recovery plan\n  pmp disaster-recovery test dr-plan-1\n  pmp disaster-recovery list"
    )]
    DisasterRecovery {
        #[command(subcommand)]
        command: DisasterRecoverySubcommands,
    },

    /// Tag management for resources
    #[command(
        long_about = "Manage tags for infrastructure resources\n\nSubcommands:\n- add: Add tags to resources\n- remove: Remove tags from resources\n- list: List tags\n- audit: Audit tag compliance\n- report: Generate tag report\n\nExamples:\n  pmp tags add environment=production cost-center=engineering\n  pmp tags remove environment\n  pmp tags list --project my-api\n  pmp tags audit\n  pmp tags report --format json"
    )]
    Tags {
        #[command(subcommand)]
        command: TagsSubcommands,
    },

    /// Search infrastructure and resources
    #[command(
        long_about = "Search infrastructure projects and resources\n\nSubcommands:\n- by-tags: Search by tags\n- by-resources: Search by resource type\n- by-name: Search by name pattern\n- by-output: Search by output values\n\nExamples:\n  pmp search by-tags environment=production\n  pmp search by-resources aws_instance\n  pmp search by-name api\n  pmp search by-output vpc_id=vpc-123"
    )]
    Search {
        #[command(subcommand)]
        command: SearchSubcommands,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum DepsSubcommands {
    /// Analyze dependencies across all projects
    #[command(
        long_about = "Analyze dependencies across all projects\n\nFinds:\n- Circular dependencies\n- Missing dependencies\n- Orphaned projects\n- Dependency bottlenecks\n- Standalone projects\n\nExample:\n  pmp deps analyze"
    )]
    Analyze,

    /// Show impact of changes to a project
    #[command(
        long_about = "Show which projects would be impacted by changes to a specific project\n\nExample:\n  pmp deps impact my-api"
    )]
    Impact {
        /// Project name to analyze
        project: String,
    },

    /// Validate dependency chains
    #[command(
        long_about = "Validate all dependency chains for errors\n\nChecks for:\n- Missing dependencies\n- Circular dependencies\n- Invalid dependency references\n\nExample:\n  pmp deps validate"
    )]
    Validate,

    /// Show optimal deployment order
    #[command(
        long_about = "Calculate optimal deployment order using topological sort\n\nShows projects grouped by deployment level (can deploy in parallel)\n\nExample:\n  pmp deps order"
    )]
    Order,

    /// Explain why a project is needed
    #[command(
        long_about = "Explain dependency relationships for a project\n\nShows:\n- What the project depends on\n- What depends on the project\n- Full dependency chain\n\nExample:\n  pmp deps why my-api"
    )]
    Why {
        /// Project name to explain
        project: String,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum StateSubcommands {
    /// List state across all projects
    #[command(
        long_about = "Show state information for all projects\n\nExample:\n  pmp state list\n  pmp state list --details"
    )]
    List {
        /// Show detailed information
        #[arg(short, long)]
        details: bool,
    },

    /// Detect configuration drift
    #[command(
        long_about = "Detect drift between desired and actual state\n\nExample:\n  pmp state drift\n  pmp state drift my-project"
    )]
    Drift {
        /// Project name (optional, checks all if not specified)
        project: Option<String>,
    },

    /// Lock state for a project
    #[command(
        long_about = "Lock state to prevent concurrent modifications\n\nExample:\n  pmp state lock my-project\n  pmp state lock my-project --environment production"
    )]
    Lock {
        /// Project name
        project: String,

        /// Environment name (optional, prompts if not specified)
        #[arg(short, long)]
        environment: Option<String>,
    },

    /// Unlock state for a project
    #[command(
        long_about = "Unlock state\n\nExample:\n  pmp state unlock my-project\n  pmp state unlock my-project --force"
    )]
    Unlock {
        /// Project name
        project: String,

        /// Environment name (optional, prompts if not specified)
        #[arg(short, long)]
        environment: Option<String>,

        /// Force unlock even if locked by another user
        #[arg(short, long)]
        force: bool,
    },

    /// Sync remote state
    #[command(long_about = "Sync state with remote backend\n\nExample:\n  pmp state sync")]
    Sync,

    /// Create a manual backup of state
    #[command(
        long_about = "Create a backup of the current state\n\nExample:\n  pmp state backup\n  pmp state backup --path ./my-project/environments/dev"
    )]
    Backup {
        /// Path to the project environment (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Restore state from a backup
    #[command(
        long_about = "Restore state from a previous backup\n\nExample:\n  pmp state restore 20250116_143000\n  pmp state restore 20250116_143000 --force"
    )]
    Restore {
        /// Backup ID to restore
        backup_id: String,

        /// Path to the project environment (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },

    /// Migrate state between backends
    #[command(
        long_about = "Migrate state to a different backend\n\nSupported backends:\n- s3\n- azurerm\n- gcs\n- local\n\nExample:\n  pmp state migrate s3\n  pmp state migrate azurerm --path ./my-project/environments/prod"
    )]
    Migrate {
        /// Target backend type
        backend_type: String,

        /// Path to the project environment (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum DriftSubcommands {
    /// Detect drift in infrastructure
    #[command(
        long_about = "Compare actual infrastructure state vs declared configuration\n\nExample:\n  pmp drift detect\n  pmp drift detect --path ./my-project/environments/dev"
    )]
    Detect {
        /// Path to check (defaults to current directory or all projects)
        #[arg(short, long)]
        path: Option<String>,

        /// Output format (text, json)
        #[arg(short, long)]
        format: Option<String>,
    },

    /// Generate drift report
    #[command(
        long_about = "Generate a detailed drift report with visualization\n\nExample:\n  pmp drift report\n  pmp drift report --format json --output drift-report.json"
    )]
    Report {
        /// Path to check (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Output file
        #[arg(short, long)]
        output: Option<String>,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },

    /// Reconcile drift by applying changes
    #[command(
        long_about = "Auto-fix configuration drift by applying changes\n\nExample:\n  pmp drift reconcile\n  pmp drift reconcile --auto-approve"
    )]
    Reconcile {
        /// Path to reconcile (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Skip confirmation prompt
        #[arg(long)]
        auto_approve: bool,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum PolicySubcommands {
    /// Validate against organizational policies
    #[command(
        long_about = "Check projects against organizational policies\n\nBuilt-in policies:\n- naming: Naming conventions\n- tagging: Required tags\n- security: Security best practices\n- dependencies: Dependency validation\n\nExample:\n  pmp policy validate\n  pmp policy validate --policy naming\n  pmp policy validate --path ./my-project/environments/dev"
    )]
    Validate {
        /// Path to validate (defaults to current directory or all projects)
        #[arg(short, long)]
        path: Option<String>,

        /// Filter policies by ID or category
        #[arg(long)]
        policy: Option<String>,
    },

    /// Run security scanning
    #[command(
        long_about = "Run security scanning tools on infrastructure code\n\nSupported scanners:\n- tfsec: Terraform security scanner\n- checkov: Policy-as-code scanner\n- trivy: Comprehensive security scanner\n\nExample:\n  pmp policy scan\n  pmp policy scan --scanner tfsec\n  pmp policy scan --scanner checkov --path ./my-project/environments/prod"
    )]
    Scan {
        /// Path to scan (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Scanner to use (tfsec, checkov, trivy)
        #[arg(short, long)]
        scanner: Option<String>,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum CiSubcommands {
    /// Generate CI/CD pipeline configuration
    #[command(
        long_about = "Generate CI/CD pipeline configuration\n\nSupported types:\n- github-actions, github\n- gitlab-ci, gitlab\n- jenkins\n\nBy default, generates dynamic pipelines that only run changed projects.\nUse --static to generate pipelines that run all projects.\n\nExample:\n  pmp ci generate github-actions\n  pmp ci generate gitlab-ci --output .gitlab-ci.yml\n  pmp ci generate github-actions --static"
    )]
    Generate {
        /// Pipeline type (github-actions, gitlab-ci, jenkins)
        pipeline_type: String,

        /// Output file (optional, prints to stdout if not specified)
        #[arg(short, long)]
        output: Option<String>,

        /// Environment filter (optional, includes all if not specified)
        #[arg(short, long)]
        environment: Option<String>,

        /// Generate static pipeline (run all projects, disable change detection)
        #[arg(long)]
        static_mode: bool,
    },

    /// Detect changed projects based on git diff
    #[command(
        long_about = "Detect which projects have changed files based on git diff\n\nThis command is used internally by generated CI pipelines to determine\nwhich projects need to be previewed or applied.\n\nExit codes:\n- 0: Success, changed projects found\n- 1: No projects changed\n- 2: Infrastructure file changed (skip project CI)\n\nExample:\n  pmp ci detect-changes --base origin/main --head HEAD\n  pmp ci detect-changes --base $CI_MERGE_REQUEST_TARGET_BRANCH_NAME --head $CI_COMMIT_SHA\n  pmp ci detect-changes --base main --head feature-branch --environment production"
    )]
    DetectChanges {
        /// Base git reference for comparison (e.g., origin/main, main)
        #[arg(long)]
        base: String,

        /// Head git reference for comparison (e.g., HEAD, commit SHA)
        #[arg(long)]
        head: String,

        /// Filter by environment (optional)
        #[arg(short, long)]
        environment: Option<String>,

        /// Output format (json, yaml)
        #[arg(short = 'f', long, default_value = "json")]
        output_format: String,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum TemplateSubcommands {
    /// Scaffold a new template pack interactively
    #[command(
        long_about = "Create a new template pack with interactive prompts\n\nExample:\n  pmp template scaffold\n  pmp template scaffold --output ./custom-templates"
    )]
    Scaffold {
        /// Output directory (defaults to current directory)
        #[arg(short, long)]
        output: Option<String>,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum EnvSubcommands {
    /// Compare two environments
    #[command(
        long_about = "Compare configurations between two environments\n\nExample:\n  pmp env diff dev staging\n  pmp env diff production staging"
    )]
    Diff {
        /// Source environment name
        source: String,

        /// Target environment name
        target: String,
    },

    /// Promote configuration between environments
    #[command(
        long_about = "Promote configuration from one environment to another\n\nExample:\n  pmp env promote dev staging\n  pmp env promote dev staging --project my-api"
    )]
    Promote {
        /// Source environment name
        source: String,

        /// Target environment name
        target: String,

        /// Project filter (optional)
        #[arg(short, long)]
        project: Option<String>,
    },

    /// Synchronize common settings across environments
    #[command(
        long_about = "Find and display common settings across environments\n\nExample:\n  pmp env sync\n  pmp env sync --project my-api"
    )]
    Sync {
        /// Project filter (optional)
        #[arg(short, long)]
        project: Option<String>,
    },

    /// Manage environment variables
    #[command(
        long_about = "Display environment variables across projects\n\nExample:\n  pmp env variables\n  pmp env variables --environment production\n  pmp env variables --project my-api"
    )]
    Variables {
        /// Environment filter (optional)
        #[arg(short, long)]
        environment: Option<String>,

        /// Project filter (optional)
        #[arg(short, long)]
        project: Option<String>,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum TestSubcommands {
    /// Run integration tests
    #[command(
        long_about = "Run integration tests for infrastructure\n\nExample:\n  pmp test test\n  pmp test test --path ./my-project/environments/dev"
    )]
    Test {
        /// Path to test (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Test pattern filter
        #[arg(short = 't', long)]
        test_pattern: Option<String>,
    },

    /// Validate plan
    #[command(long_about = "Validate plan without executing\n\nExample:\n  pmp test validate-plan")]
    ValidatePlan {
        /// Path to validate (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Dry run
    #[command(long_about = "Simulate apply without making changes\n\nExample:\n  pmp test dry-run")]
    DryRun {
        /// Path to run (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Cost estimate
    #[command(
        long_about = "Generate cost estimate with breakdown\n\nExample:\n  pmp test cost-estimate --format json"
    )]
    CostEstimate {
        /// Path to estimate (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Output file
        #[arg(short, long)]
        output: Option<String>,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },

    /// Compliance report
    #[command(
        long_about = "Generate compliance report\n\nExample:\n  pmp test compliance-report soc2"
    )]
    ComplianceReport {
        /// Compliance framework (soc2, hipaa, pci-dss)
        framework: String,

        /// Path to check (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Output file
        #[arg(short, long)]
        output: Option<String>,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum DevExSubcommands {
    /// Interactive shell
    #[command(
        long_about = "Launch interactive shell for exploring projects\n\nExample:\n  pmp devex shell"
    )]
    Shell,

    /// Generate documentation
    #[command(
        long_about = "Generate documentation from infrastructure\n\nExample:\n  pmp devex docs --format markdown --output README.md"
    )]
    Docs {
        /// Path to document (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Output file
        #[arg(short, long)]
        output: Option<String>,

        /// Output format (markdown, html)
        #[arg(short, long)]
        format: Option<String>,
    },

    /// Visualize dependency graph
    #[command(
        long_about = "Visualize dependency graphs\n\nExample:\n  pmp devex graph-viz --format mermaid"
    )]
    GraphViz {
        /// Output file
        #[arg(short, long)]
        output: Option<String>,

        /// Output format (mermaid, graphviz, dot)
        #[arg(short, long)]
        format: Option<String>,
    },

    /// Export infrastructure
    #[command(
        long_about = "Export infrastructure to other formats\n\nExample:\n  pmp devex export helm --output chart.yaml"
    )]
    Export {
        /// Target format (helm, cloudformation, pulumi)
        target_format: String,

        /// Path to export (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Output file
        #[arg(short, long)]
        output: Option<String>,
    },

    /// Import existing infrastructure
    #[command(
        long_about = "Import existing infrastructure into PMP\n\nExample:\n  pmp devex import terraform ./existing-infra"
    )]
    Import {
        /// Source format (terraform, helm, cloudformation)
        source_format: String,

        /// Source path
        source_path: String,

        /// Project name
        #[arg(short, long)]
        project_name: String,

        /// Environment
        #[arg(short, long)]
        environment: String,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum TemplateMgmtSubcommands {
    /// Validate template
    #[command(
        long_about = "Validate template definitions\n\nExample:\n  pmp template-mgmt validate my-pack my-template"
    )]
    Validate {
        /// Template pack name
        template_pack: String,

        /// Template name
        template_name: String,
    },

    /// Test template rendering
    #[command(
        long_about = "Test template rendering with sample data\n\nExample:\n  pmp template-mgmt test my-pack my-template"
    )]
    Test {
        /// Template pack name
        template_pack: String,

        /// Template name
        template_name: String,

        /// Test data file (JSON)
        #[arg(short, long)]
        test_data: Option<String>,
    },

    /// Publish template
    #[command(
        long_about = "Publish template to registry\n\nExample:\n  pmp template-mgmt publish my-pack --version 1.0.0"
    )]
    Publish {
        /// Template pack name
        template_pack: String,

        /// Registry URL
        #[arg(short, long)]
        registry_url: Option<String>,

        /// Version
        #[arg(short, long)]
        version: Option<String>,
    },

    /// Clone template
    #[command(
        long_about = "Clone and customize existing template\n\nExample:\n  pmp template-mgmt clone source-pack source-template target-pack target-template"
    )]
    Clone {
        /// Source template pack
        source_pack: String,

        /// Source template
        source_template: String,

        /// Target template pack
        target_pack: String,

        /// Target template
        target_template: String,
    },

    /// Develop plugin
    #[command(
        long_about = "Helper for developing new plugins\n\nExample:\n  pmp template-mgmt plugin-develop my-pack my-plugin"
    )]
    PluginDevelop {
        /// Template pack name
        template_pack: String,

        /// Plugin name
        plugin_name: String,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum ProviderSubcommands {
    /// Install provider plugin
    #[command(
        long_about = "Install provider-specific plugin\n\nExample:\n  pmp provider install aws vpc"
    )]
    Install {
        /// Provider name (aws, azure, gcp)
        provider: String,

        /// Plugin name
        plugin: String,
    },

    /// Connect to cloud provider
    #[command(
        long_about = "Configure cloud provider credentials\n\nExample:\n  pmp provider connect aws --profile default"
    )]
    Connect {
        /// Provider name (aws, azure, gcp, kubernetes)
        provider: String,

        /// Profile or context name
        #[arg(short, long)]
        profile: Option<String>,
    },

    /// Manage secrets
    #[command(
        long_about = "Manage secrets across environments\n\nExample:\n  pmp provider secrets list"
    )]
    Secrets {
        /// Command (list, set, get, delete, rotate)
        command: String,

        /// Path to environment (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Cost optimization
    #[command(
        long_about = "Suggest cost optimization opportunities\n\nExample:\n  pmp provider cost-optimize --format json"
    )]
    CostOptimize {
        /// Path to analyze (defaults to current directory)
        #[arg(short, long)]
        path: Option<String>,

        /// Output file
        #[arg(short, long)]
        output: Option<String>,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum MonitorSubcommands {
    /// Display monitoring dashboard
    #[command(
        long_about = "Display real-time monitoring dashboard\n\nExample:\n  pmp monitor dashboard\n  pmp monitor dashboard --refresh 30"
    )]
    Dashboard {
        /// Refresh interval in seconds
        #[arg(short, long)]
        refresh: Option<u64>,

        /// Path to project (optional, monitors all if not specified)
        #[arg(short, long)]
        path: Option<String>,
    },

    /// Show infrastructure metrics
    #[command(
        long_about = "Display infrastructure metrics\n\nExample:\n  pmp monitor metrics\n  pmp monitor metrics --project my-api\n  pmp monitor metrics --metric cpu --format json"
    )]
    Metrics {
        /// Project to monitor
        #[arg(short, long)]
        project: Option<String>,

        /// Environment to monitor
        #[arg(short, long)]
        environment: Option<String>,

        /// Specific metric to display
        #[arg(short, long)]
        metric: Option<String>,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },

    /// Manage alerts and notifications
    #[command(
        long_about = "Manage alerts and notifications\n\nExample:\n  pmp monitor alerts\n  pmp monitor alerts --environment production\n  pmp monitor alerts --severity high"
    )]
    Alerts {
        /// Environment filter
        #[arg(short, long)]
        environment: Option<String>,

        /// Alert severity filter (low, medium, high, critical)
        #[arg(short, long)]
        severity: Option<String>,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum AuditSubcommands {
    /// View audit logs
    #[command(
        long_about = "View audit logs of infrastructure changes\n\nExample:\n  pmp audit log\n  pmp audit log --project my-api --limit 50\n  pmp audit log --from 2025-01-01"
    )]
    Log {
        /// Project filter
        #[arg(short, long)]
        project: Option<String>,

        /// Environment filter
        #[arg(short, long)]
        environment: Option<String>,

        /// Start date (YYYY-MM-DD)
        #[arg(long)]
        from: Option<String>,

        /// End date (YYYY-MM-DD)
        #[arg(long)]
        to: Option<String>,

        /// Maximum number of entries
        #[arg(short, long)]
        limit: Option<usize>,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },

    /// Compare infrastructure states
    #[command(
        long_about = "Compare infrastructure states between dates or versions\n\nExample:\n  pmp audit diff --from 2025-01-01 --to 2025-01-15\n  pmp audit diff --project my-api"
    )]
    Diff {
        /// Project filter
        #[arg(short, long)]
        project: Option<String>,

        /// Environment filter
        #[arg(short, long)]
        environment: Option<String>,

        /// Start date or version
        #[arg(long)]
        from: Option<String>,

        /// End date or version
        #[arg(long)]
        to: Option<String>,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum LockSubcommands {
    /// Acquire a lock
    #[command(
        long_about = "Acquire a lock for infrastructure operations\n\nExample:\n  pmp lock acquire my-api\n  pmp lock acquire my-api --environment production"
    )]
    Acquire {
        /// Project name
        project: String,

        /// Environment name
        #[arg(short, long)]
        environment: Option<String>,

        /// Lock reason/description
        #[arg(short, long)]
        reason: Option<String>,

        /// Lock timeout in minutes
        #[arg(short, long)]
        timeout: Option<u64>,
    },

    /// Release a lock
    #[command(
        long_about = "Release a previously acquired lock\n\nExample:\n  pmp lock release my-api\n  pmp lock release my-api --force"
    )]
    Release {
        /// Project name
        project: String,

        /// Environment name
        #[arg(short, long)]
        environment: Option<String>,

        /// Force release even if locked by another user
        #[arg(short, long)]
        force: bool,
    },

    /// Check lock status
    #[command(
        long_about = "Check status of locks across infrastructure\n\nExample:\n  pmp lock status\n  pmp lock status --project my-api"
    )]
    Status {
        /// Project filter
        #[arg(short, long)]
        project: Option<String>,

        /// Environment filter
        #[arg(short, long)]
        environment: Option<String>,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum ReviewSubcommands {
    /// Request review for changes
    #[command(
        long_about = "Create a review request for infrastructure changes\n\nExample:\n  pmp review request\n  pmp review request --project my-api --reviewer alice@example.com"
    )]
    Request {
        /// Project name (optional, prompts if not specified)
        #[arg(short, long)]
        project: Option<String>,

        /// Environment name (optional, prompts if not specified)
        #[arg(short, long)]
        environment: Option<String>,

        /// Reviewer email or username
        #[arg(short, long)]
        reviewer: Option<String>,

        /// Review description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Approve pending changes
    #[command(
        long_about = "Approve a pending review request\n\nExample:\n  pmp review approve review-123\n  pmp review approve review-123 --comment 'LGTM'"
    )]
    Approve {
        /// Review ID
        review_id: String,

        /// Approval comment
        #[arg(short, long)]
        comment: Option<String>,
    },

    /// List pending reviews
    #[command(
        long_about = "List review requests\n\nExample:\n  pmp review list\n  pmp review list --status pending\n  pmp review list --reviewer alice@example.com"
    )]
    List {
        /// Status filter (pending, approved, rejected)
        #[arg(short, long)]
        status: Option<String>,

        /// Reviewer filter
        #[arg(short, long)]
        reviewer: Option<String>,

        /// Project filter
        #[arg(short, long)]
        project: Option<String>,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum WorkspaceSubcommands {
    /// List available workspaces
    #[command(long_about = "List all available workspaces\n\nExample:\n  pmp workspace list")]
    List {
        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },

    /// Create a new workspace
    #[command(
        long_about = "Create a new workspace\n\nExample:\n  pmp workspace new feature-branch\n  pmp workspace new feature-branch --description 'Feature development workspace'"
    )]
    New {
        /// Workspace name
        name: String,

        /// Workspace description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Switch to a workspace
    #[command(
        long_about = "Switch to an existing workspace\n\nExample:\n  pmp workspace select production\n  pmp workspace select feature-branch"
    )]
    Select {
        /// Workspace name
        name: String,
    },

    /// Delete a workspace
    #[command(
        long_about = "Delete a workspace\n\nExample:\n  pmp workspace delete old-workspace\n  pmp workspace delete old-workspace --force"
    )]
    Delete {
        /// Workspace name
        name: String,

        /// Force deletion without confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Show current workspace
    #[command(
        long_about = "Display current workspace information\n\nExample:\n  pmp workspace show"
    )]
    Show,
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum BackupSubcommands {
    /// Create a backup
    #[command(
        long_about = "Create a backup of infrastructure state\n\nExample:\n  pmp backup create\n  pmp backup create --project my-api --environment production\n  pmp backup create --description 'Pre-deployment backup'"
    )]
    Create {
        /// Project to backup (optional, backs up all if not specified)
        #[arg(short, long)]
        project: Option<String>,

        /// Environment to backup
        #[arg(short, long)]
        environment: Option<String>,

        /// Backup description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Restore from backup
    #[command(
        long_about = "Restore infrastructure state from a backup\n\nExample:\n  pmp backup restore backup-20250116-143000\n  pmp backup restore backup-20250116-143000 --force"
    )]
    Restore {
        /// Backup ID
        backup_id: String,

        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },

    /// List available backups
    #[command(
        long_about = "List all available backups\n\nExample:\n  pmp backup list\n  pmp backup list --project my-api\n  pmp backup list --format json"
    )]
    List {
        /// Project filter
        #[arg(short, long)]
        project: Option<String>,

        /// Environment filter
        #[arg(short, long)]
        environment: Option<String>,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },

    /// Delete a backup
    #[command(
        long_about = "Delete a backup\n\nExample:\n  pmp backup delete backup-20250116-143000\n  pmp backup delete backup-20250116-143000 --force"
    )]
    Delete {
        /// Backup ID
        backup_id: String,

        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum DisasterRecoverySubcommands {
    /// Create disaster recovery plan
    #[command(
        long_about = "Create a disaster recovery plan\n\nExample:\n  pmp disaster-recovery plan\n  pmp disaster-recovery plan --project my-api --output dr-plan.yaml"
    )]
    Plan {
        /// Project to plan for (optional, plans for all if not specified)
        #[arg(short, long)]
        project: Option<String>,

        /// Environment to plan for
        #[arg(short, long)]
        environment: Option<String>,

        /// Output file
        #[arg(short, long)]
        output: Option<String>,

        /// Output format (yaml, json)
        #[arg(short, long)]
        format: Option<String>,
    },

    /// Test disaster recovery procedures
    #[command(
        long_about = "Test disaster recovery procedures\n\nExample:\n  pmp disaster-recovery test dr-plan-1\n  pmp disaster-recovery test dr-plan-1 --dry-run"
    )]
    Test {
        /// DR plan ID or name
        plan_id: String,

        /// Perform dry run without actual changes
        #[arg(long)]
        dry_run: bool,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },

    /// List disaster recovery plans
    #[command(
        long_about = "List all disaster recovery plans\n\nExample:\n  pmp disaster-recovery list\n  pmp disaster-recovery list --format json"
    )]
    List {
        /// Project filter
        #[arg(short, long)]
        project: Option<String>,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
enum TagsSubcommands {
    /// Add tags to resources
    #[command(
        long_about = "Add tags to infrastructure resources\n\nExample:\n  pmp tags add environment=production cost-center=engineering\n  pmp tags add owner=team-a --project my-api"
    )]
    Add {
        /// Tags to add (key=value format)
        tags: Vec<String>,

        /// Project to tag (optional, tags current if not specified)
        #[arg(short, long)]
        project: Option<String>,

        /// Environment to tag
        #[arg(short, long)]
        environment: Option<String>,
    },

    /// Remove tags from resources
    #[command(
        long_about = "Remove tags from infrastructure resources\n\nExample:\n  pmp tags remove environment\n  pmp tags remove environment cost-center --project my-api"
    )]
    Remove {
        /// Tag keys to remove
        tags: Vec<String>,

        /// Project to remove tags from
        #[arg(short, long)]
        project: Option<String>,

        /// Environment to remove tags from
        #[arg(short, long)]
        environment: Option<String>,
    },

    /// List tags
    #[command(
        long_about = "List tags on infrastructure resources\n\nExample:\n  pmp tags list\n  pmp tags list --project my-api\n  pmp tags list --format json"
    )]
    List {
        /// Project filter
        #[arg(short, long)]
        project: Option<String>,

        /// Environment filter
        #[arg(short, long)]
        environment: Option<String>,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },

    /// Audit tag compliance
    #[command(
        long_about = "Audit tag compliance against organizational policies\n\nExample:\n  pmp tags audit\n  pmp tags audit --project my-api\n  pmp tags audit --format json"
    )]
    Audit {
        /// Project filter
        #[arg(short, long)]
        project: Option<String>,

        /// Environment filter
        #[arg(short, long)]
        environment: Option<String>,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },

    /// Generate tag report
    #[command(
        long_about = "Generate comprehensive tag report\n\nExample:\n  pmp tags report\n  pmp tags report --output tags-report.json --format json"
    )]
    Report {
        /// Output file
        #[arg(short, long)]
        output: Option<String>,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },
}

#[derive(Subcommand)]
#[command(next_display_order = None)]  // Sort subcommands alphabetically
#[allow(clippy::enum_variant_names)]
enum SearchSubcommands {
    /// Search by tags
    #[command(
        long_about = "Search infrastructure by tags\n\nExample:\n  pmp search by-tags environment=production\n  pmp search by-tags environment=production cost-center=engineering"
    )]
    ByTags {
        /// Tags to search for (key=value format)
        tags: Vec<String>,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },

    /// Search by resource type
    #[command(
        long_about = "Search infrastructure by resource type\n\nExample:\n  pmp search by-resources aws_instance\n  pmp search by-resources aws_s3_bucket --format json"
    )]
    ByResources {
        /// Resource type to search for
        resource_type: String,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },

    /// Search by name pattern
    #[command(
        long_about = "Search infrastructure by name pattern\n\nExample:\n  pmp search by-name api\n  pmp search by-name '*-production' --format json"
    )]
    ByName {
        /// Name pattern to search for
        pattern: String,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },

    /// Search by output values
    #[command(
        long_about = "Search infrastructure by output values\n\nExample:\n  pmp search by-output vpc_id=vpc-123\n  pmp search by-output subnet_id=subnet-456 --format json"
    )]
    ByOutput {
        /// Output values to search for (key=value format)
        outputs: Vec<String>,

        /// Output format (text, json, yaml)
        #[arg(short, long)]
        format: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let ctx = context::Context::new();

    match cli.command {
        Commands::Infrastructure { command } => match command {
            InfrastructureSubcommands::Init {
                output,
                template_packs_paths,
            } => {
                InfrastructureCommand::execute_init(
                    &ctx,
                    output.as_deref(),
                    template_packs_paths.as_deref(),
                )?;
            }
        },
        Commands::Project { command } => match command {
            ProjectSubcommands::Create {
                output,
                template_packs_paths,
            } => {
                CreateCommand::execute(&ctx, output.as_deref(), template_packs_paths.as_deref())?;
            }
            ProjectSubcommands::Find { name, kind } => {
                FindCommand::execute(&ctx, name.as_deref(), kind.as_deref())?;
            }
            ProjectSubcommands::Update {
                path,
                template_packs_paths,
            } => {
                UpdateCommand::execute(&ctx, path.as_deref(), template_packs_paths.as_deref())?;
            }
            ProjectSubcommands::Clone {
                source,
                name,
                environment,
            } => {
                CloneCommand::execute(&ctx, source.as_deref(), &name, environment.as_deref())?;
            }
            ProjectSubcommands::Preview {
                path,
                executor_args,
            } => {
                PreviewCommand::execute(&ctx, path.as_deref(), &executor_args)?;
            }
            ProjectSubcommands::Apply {
                path,
                executor_args,
            } => {
                ApplyCommand::execute(&ctx, path.as_deref(), &executor_args)?;
            }
            ProjectSubcommands::Destroy {
                path,
                yes,
                executor_args,
            } => {
                DestroyCommand::execute(&ctx, path.as_deref(), yes, &executor_args)?;
            }
            ProjectSubcommands::Refresh {
                path,
                executor_args,
            } => {
                RefreshCommand::execute(&ctx, path.as_deref(), &executor_args)?;
            }
            ProjectSubcommands::Graph {
                path,
                format,
                output,
                all,
            } => {
                GraphCommand::execute(
                    &ctx,
                    path.as_deref(),
                    format.as_deref(),
                    output.as_deref(),
                    all,
                )?;
            }
            ProjectSubcommands::Deps { command } => match command {
                DepsSubcommands::Analyze => {
                    DepsCommand::execute_analyze(&ctx)?;
                }
                DepsSubcommands::Impact { project } => {
                    DepsCommand::execute_impact(&ctx, &project)?;
                }
                DepsSubcommands::Validate => {
                    DepsCommand::execute_validate(&ctx)?;
                }
                DepsSubcommands::Order => {
                    DepsCommand::execute_order(&ctx)?;
                }
                DepsSubcommands::Why { project } => {
                    DepsCommand::execute_why(&ctx, &project)?;
                }
            },
            ProjectSubcommands::Drift { command } => match command {
                DriftSubcommands::Detect { path, format } => {
                    DriftCommand::execute_detect(&ctx, path.as_deref(), format.as_deref())?;
                }
                DriftSubcommands::Report {
                    path,
                    output,
                    format,
                } => {
                    DriftCommand::execute_report(
                        &ctx,
                        path.as_deref(),
                        output.as_deref(),
                        format.as_deref(),
                    )?;
                }
                DriftSubcommands::Reconcile { path, auto_approve } => {
                    DriftCommand::execute_reconcile(&ctx, path.as_deref(), auto_approve)?;
                }
            },
            ProjectSubcommands::Policy { command } => match command {
                PolicySubcommands::Validate { path, policy } => {
                    PolicyCommand::execute_validate(&ctx, path.as_deref(), policy.as_deref())?;
                }
                PolicySubcommands::Scan { path, scanner } => {
                    PolicyCommand::execute_scan(&ctx, path.as_deref(), scanner.as_deref())?;
                }
            },
            ProjectSubcommands::State { command } => match command {
                StateSubcommands::List { details } => {
                    StateCommand::execute_list(&ctx, details)?;
                }
                StateSubcommands::Drift { project } => {
                    StateCommand::execute_drift(&ctx, project.as_deref())?;
                }
                StateSubcommands::Lock {
                    project,
                    environment,
                } => {
                    StateCommand::execute_lock(&ctx, &project, environment.as_deref())?;
                }
                StateSubcommands::Unlock {
                    project,
                    environment,
                    force,
                } => {
                    StateCommand::execute_unlock(&ctx, &project, environment.as_deref(), force)?;
                }
                StateSubcommands::Sync => {
                    StateCommand::execute_sync(&ctx)?;
                }
                StateSubcommands::Backup { path } => {
                    StateCommand::execute_backup(&ctx, path.as_deref())?;
                }
                StateSubcommands::Restore {
                    backup_id,
                    path,
                    force,
                } => {
                    StateCommand::execute_restore(&ctx, &backup_id, path.as_deref(), force)?;
                }
                StateSubcommands::Migrate { backend_type, path } => {
                    StateCommand::execute_migrate(&ctx, &backend_type, path.as_deref())?;
                }
            },
            ProjectSubcommands::Env { command } => match command {
                EnvSubcommands::Diff { source, target } => {
                    EnvCommand::execute_diff(&ctx, &source, &target)?;
                }
                EnvSubcommands::Promote {
                    source,
                    target,
                    project,
                } => {
                    EnvCommand::execute_promote(&ctx, &source, &target, project.as_deref())?;
                }
                EnvSubcommands::Sync { project } => {
                    EnvCommand::execute_sync(&ctx, project.as_deref())?;
                }
                EnvSubcommands::Variables {
                    environment,
                    project,
                } => {
                    EnvCommand::execute_variables(
                        &ctx,
                        environment.as_deref(),
                        project.as_deref(),
                    )?;
                }
            },
        },
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
        Commands::Ui { port, host } => {
            UiCommand::execute(&ctx, port, host)?;
        }
        Commands::Ci { command } => match command {
            CiSubcommands::Generate {
                pipeline_type,
                output,
                environment,
                static_mode,
            } => {
                CiCommand::execute_generate(
                    &ctx,
                    &pipeline_type,
                    output.as_deref(),
                    environment.as_deref(),
                    static_mode,
                )?;
            }
            CiSubcommands::DetectChanges {
                base,
                head,
                environment,
                output_format,
            } => {
                CiDetectChangesCommand::execute(
                    &ctx,
                    &base,
                    &head,
                    environment.as_deref(),
                    &output_format,
                )?;
            }
        },
        Commands::Template { command } => match command {
            TemplateSubcommands::Scaffold { output } => {
                TemplateCommand::execute_scaffold(&ctx, output.as_deref())?;
            }
        },
        Commands::Test { command } => match command {
            TestSubcommands::Test { path, test_pattern } => {
                TestCommand::execute_test(&ctx, path.as_deref(), test_pattern.as_deref())?;
            }
            TestSubcommands::ValidatePlan { path } => {
                TestCommand::execute_validate_plan(&ctx, path.as_deref())?;
            }
            TestSubcommands::DryRun { path } => {
                TestCommand::execute_dry_run(&ctx, path.as_deref())?;
            }
            TestSubcommands::CostEstimate {
                path,
                output,
                format,
            } => {
                TestCommand::execute_cost_estimate(
                    &ctx,
                    path.as_deref(),
                    output.as_deref(),
                    format.as_deref(),
                )?;
            }
            TestSubcommands::ComplianceReport {
                framework,
                path,
                output,
                format,
            } => {
                TestCommand::execute_compliance_report(
                    &ctx,
                    path.as_deref(),
                    &framework,
                    output.as_deref(),
                    format.as_deref(),
                )?;
            }
        },
        Commands::DevEx { command } => match command {
            DevExSubcommands::Shell => {
                DevExCommand::execute_shell(&ctx)?;
            }
            DevExSubcommands::Docs {
                path,
                output,
                format,
            } => {
                DevExCommand::execute_docs(
                    &ctx,
                    path.as_deref(),
                    output.as_deref(),
                    format.as_deref(),
                )?;
            }
            DevExSubcommands::GraphViz { output, format } => {
                DevExCommand::execute_graph_viz(&ctx, output.as_deref(), format.as_deref())?;
            }
            DevExSubcommands::Export {
                target_format,
                path,
                output,
            } => {
                DevExCommand::execute_export(
                    &ctx,
                    path.as_deref(),
                    &target_format,
                    output.as_deref(),
                )?;
            }
            DevExSubcommands::Import {
                source_format,
                source_path,
                project_name,
                environment,
            } => {
                DevExCommand::execute_import(
                    &ctx,
                    &source_path,
                    &source_format,
                    &project_name,
                    &environment,
                )?;
            }
        },
        Commands::TemplateMgmt { command } => match command {
            TemplateMgmtSubcommands::Validate {
                template_pack,
                template_name,
            } => {
                TemplateMgmtCommand::execute_validate(&ctx, &template_pack, &template_name)?;
            }
            TemplateMgmtSubcommands::Test {
                template_pack,
                template_name,
                test_data,
            } => {
                TemplateMgmtCommand::execute_test(
                    &ctx,
                    &template_pack,
                    &template_name,
                    test_data.as_deref(),
                )?;
            }
            TemplateMgmtSubcommands::Publish {
                template_pack,
                registry_url,
                version,
            } => {
                TemplateMgmtCommand::execute_publish(
                    &ctx,
                    &template_pack,
                    registry_url.as_deref(),
                    version.as_deref(),
                )?;
            }
            TemplateMgmtSubcommands::Clone {
                source_pack,
                source_template,
                target_pack,
                target_template,
            } => {
                TemplateMgmtCommand::execute_clone(
                    &ctx,
                    &source_pack,
                    &source_template,
                    &target_pack,
                    &target_template,
                )?;
            }
            TemplateMgmtSubcommands::PluginDevelop {
                template_pack,
                plugin_name,
            } => {
                TemplateMgmtCommand::execute_plugin_develop(&ctx, &template_pack, &plugin_name)?;
            }
        },
        Commands::Provider { command } => match command {
            ProviderSubcommands::Install { provider, plugin } => {
                ProviderCommand::execute_install(&ctx, &provider, &plugin)?;
            }
            ProviderSubcommands::Connect { provider, profile } => {
                ProviderCommand::execute_connect(&ctx, &provider, profile.as_deref())?;
            }
            ProviderSubcommands::Secrets { command, path } => {
                ProviderCommand::execute_secrets(&ctx, &command, path.as_deref())?;
            }
            ProviderSubcommands::CostOptimize {
                path,
                output,
                format,
            } => {
                ProviderCommand::execute_cost_optimization(
                    &ctx,
                    path.as_deref(),
                    output.as_deref(),
                    format.as_deref(),
                )?;
            }
        },
        Commands::Monitor { command } => match command {
            MonitorSubcommands::Dashboard { refresh: _, path } => {
                MonitorCommand::execute_dashboard(&ctx, path.as_deref())?;
            }
            MonitorSubcommands::Metrics {
                project: _,
                environment: _,
                metric,
                format: _,
            } => {
                MonitorCommand::execute_metrics(&ctx, None, metric.as_deref())?;
            }
            MonitorSubcommands::Alerts {
                environment: _,
                severity: _,
                format: _,
            } => {
                MonitorCommand::execute_alerts(&ctx, "list", None, None)?;
            }
        },
        Commands::Audit { command } => match command {
            AuditSubcommands::Log {
                project: _,
                environment: _,
                from: _,
                to: _,
                limit,
                format: _,
            } => {
                AuditCommand::execute_log(&ctx, None, limit, None)?;
            }
            AuditSubcommands::Diff {
                project: _,
                environment: _,
                from,
                to,
                format,
            } => {
                AuditCommand::execute_diff(
                    &ctx,
                    None,
                    from.as_deref(),
                    to.as_deref(),
                    format.as_deref(),
                )?;
            }
        },
        Commands::Lock { command } => match command {
            LockSubcommands::Acquire {
                project: _,
                environment: _,
                reason,
                timeout: _,
            } => {
                LockCommand::execute_acquire(&ctx, None, reason.as_deref())?;
            }
            LockSubcommands::Release {
                project: _,
                environment: _,
                force,
            } => {
                LockCommand::execute_release(&ctx, None, force)?;
            }
            LockSubcommands::Status {
                project: _,
                environment: _,
                format: _,
            } => {
                LockCommand::execute_status(&ctx, None, false)?;
            }
        },
        Commands::Review { command } => match command {
            ReviewSubcommands::Request {
                project: _,
                environment: _,
                reviewer: _,
                description,
            } => {
                ReviewCommand::execute_request(&ctx, None, description.as_deref())?;
            }
            ReviewSubcommands::Approve { review_id, comment } => {
                ReviewCommand::execute_approve(
                    &ctx,
                    Some(&review_id),
                    "approve",
                    comment.as_deref(),
                )?;
            }
            ReviewSubcommands::List {
                status,
                reviewer: _,
                project: _,
                format: _,
            } => {
                ReviewCommand::execute_list(&ctx, status.as_deref())?;
            }
        },
        Commands::Workspace { command } => match command {
            WorkspaceSubcommands::List { format } => {
                WorkspaceCommand::execute_list(&ctx, format.as_deref())?;
            }
            WorkspaceSubcommands::New { name, description } => {
                WorkspaceCommand::execute_new(&ctx, &name, description.as_deref())?;
            }
            WorkspaceSubcommands::Select { name } => {
                WorkspaceCommand::execute_select(&ctx, &name)?;
            }
            WorkspaceSubcommands::Delete { name, force } => {
                WorkspaceCommand::execute_delete(&ctx, &name, force)?;
            }
            WorkspaceSubcommands::Show => {
                WorkspaceCommand::execute_show(&ctx, None)?;
            }
        },
        Commands::Backup { command } => match command {
            BackupSubcommands::Create {
                project,
                environment,
                description,
            } => {
                BackupCommand::execute_create(
                    &ctx,
                    project.as_deref(),
                    environment.as_deref(),
                    description.as_deref(),
                )?;
            }
            BackupSubcommands::Restore {
                backup_id,
                force: _,
            } => {
                BackupCommand::execute_restore(&ctx, Some(&backup_id), None)?;
            }
            BackupSubcommands::List {
                project,
                environment,
                format: _,
            } => {
                BackupCommand::execute_list(&ctx, project.as_deref(), environment.as_deref())?;
            }
            BackupSubcommands::Delete { backup_id, force } => {
                BackupCommand::execute_delete(&ctx, &backup_id, force)?;
            }
        },
        Commands::DisasterRecovery { command } => match command {
            DisasterRecoverySubcommands::Plan {
                project,
                environment: _,
                output: _,
                format: _,
            } => {
                DisasterRecoveryCommand::execute_plan(&ctx, project.as_deref())?;
            }
            DisasterRecoverySubcommands::Test {
                plan_id,
                dry_run,
                format: _,
            } => {
                DisasterRecoveryCommand::execute_test(&ctx, Some(&plan_id), dry_run)?;
            }
            DisasterRecoverySubcommands::List {
                project: _,
                format: _,
            } => {
                DisasterRecoveryCommand::execute_list(&ctx)?;
            }
        },
        Commands::Tags { command } => match command {
            TagsSubcommands::Add {
                tags,
                project,
                environment: _,
            } => {
                TagsCommand::execute_add(&ctx, project.as_deref(), tags)?;
            }
            TagsSubcommands::Remove {
                tags,
                project,
                environment: _,
            } => {
                TagsCommand::execute_remove(&ctx, project.as_deref(), tags)?;
            }
            TagsSubcommands::List {
                project,
                environment: _,
                format: _,
            } => {
                TagsCommand::execute_list(&ctx, project.as_deref())?;
            }
            TagsSubcommands::Audit {
                project: _,
                environment: _,
                format: _,
            } => {
                // Execute audit with empty required tags (will use defaults from infrastructure)
                TagsCommand::execute_audit(&ctx, Vec::new())?;
            }
            TagsSubcommands::Report { output, format } => {
                TagsCommand::execute_report(&ctx, output.as_deref(), format.as_deref())?;
            }
        },
        Commands::Search { command } => match command {
            SearchSubcommands::ByTags { tags, format: _ } => {
                SearchCommand::execute_by_tags(&ctx, tags)?;
            }
            SearchSubcommands::ByResources {
                resource_type,
                format: _,
            } => {
                SearchCommand::execute_by_resources(&ctx, Some(&resource_type), None)?;
            }
            SearchSubcommands::ByName { pattern, format: _ } => {
                SearchCommand::execute_by_name(&ctx, &pattern)?;
            }
            SearchSubcommands::ByOutput { outputs, format: _ } => {
                SearchCommand::execute_by_output(&ctx, &outputs[0])?;
            }
        },
    }

    Ok(())
}
