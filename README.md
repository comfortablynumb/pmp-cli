# PMP - Poor Man's Platform

A CLI for managing Infrastructure as Code projects using OpenTofu/Terraform with template-based generation, dependency management, and workflow automation.

> **Production Ready**: Core features are stable. See the [ROADMAP](doc/ROADMAP.md) for development status.

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Core Concepts](#core-concepts)
- [Commands Reference](#commands-reference)
- [Configuration Files](#configuration-files)
- [Import & Cloud Inspector](#import--cloud-inspector)
- [Development](#development)
- [Architecture](#architecture)

## Features

### Core Features (Stable)

| Feature | Description | Documentation |
|---------|-------------|---------------|
| **Template System** | Handlebars-based templates with 25+ input types | [Templates](doc/templates.md) |
| **Project Management** | Create, find, clone, update projects | [Projects](doc/projects.md) |
| **Multi-Executor** | OpenTofu, Terraform, None (dependency-only) | [Executors](doc/executors.md) |
| **Dependency Graph** | Manage inter-project dependencies | [Dependencies](doc/dependencies.md) |
| **Hooks System** | Pre/post execution hooks (command, confirm, set_environment) | [Hooks](doc/hooks.md) |
| **Environment Management** | Multi-environment support with diff/promote | [Environments](doc/environments.md) |
| **CI/CD Generation** | GitHub Actions, GitLab CI, Jenkins pipelines | [CI/CD](doc/cicd.md) |
| **Plugin System** | Reusable template components | [Plugins](doc/plugins.md) |

### Advanced Template Features

| Feature | Description | Documentation |
|---------|-------------|---------------|
| **Template Versioning** | Semantic versioning with directory-based versions | [Templates](doc/templates.md) |
| **Template Inheritance** | Extend base templates with merge rules | [Templates](doc/templates.md) |
| **Template Partials** | Reusable Handlebars partials (global/pack-level) | [Templates](doc/templates.md) |

### Additional Features

| Feature | Status | Description |
|---------|--------|-------------|
| **Cost Estimation** | Stable | Infracost integration with budget thresholds |
| **OPA Policy** | Stable | Native Open Policy Agent with Rego policies | [OPA Policies](doc/opa-policies.md) |
| **Template Marketplace** | Stable | Search, install packs from registries | [Marketplace](doc/marketplace.md) |
| **State Management** | Stable | List, lock/unlock, backup/restore state |
| **Drift Detection** | Stable | Detect and reconcile configuration drift |
| **Policy Validation** | Stable | Built-in policy checks (naming, security, deps) |
| **Security Scanning** | Stable | Integration with tfsec, checkov, trivy |
| **Dependency Analysis** | Stable | Impact analysis, validation, ordering |
| **Search** | Stable | Search by tags, resources, name, outputs |
| **Web UI** | Stable | Web interface for project management |
| **Project Import** | Stable | Import existing Terraform projects | [Import](doc/import.md) |
| **Infrastructure Import** | Stable | Import cloud resources via pmp-cloud-inspector | [Import](doc/import.md) |
| **Plan Diff** | Stable | Color-coded diff visualization with ASCII/HTML output |
| **Environment Time Limits** | Stable | TTL configuration and automatic purge |
| **Template Linting** | Stable | Validate templates for common issues and best practices |

## Installation

### From Source

```bash
git clone https://github.com/your-repo/pmp-cli.git
cd pmp-cli
cargo build --release

# Add to PATH
export PATH="$PATH:$(pwd)/target/release"  # Linux/macOS
# Or on Windows: add target\release to your PATH
```

### Prerequisites

- Rust 1.70+
- OpenTofu or Terraform (for IaC operations)
- Git (for template pack installation)

## Quick Start

### 1. Initialize Infrastructure

```bash
# Create a new infrastructure configuration
pmp infrastructure init

# Or with a specific output directory
pmp infrastructure init --output ./my-infra
```

This creates `.pmp.infrastructure.yaml` with environment definitions and category organization.

### 2. Install Template Packs

Template packs are installed to `~/.pmp/template-packs/`:

```bash
# Clone from Git repository
git clone https://github.com/your-org/pmp-templates ~/.pmp/template-packs/my-templates

# Or create your own
pmp template scaffold --output ~/.pmp/template-packs/my-pack
```

### 3. Create a Project

```bash
# Interactive mode
pmp project create

# With options
pmp project create --template my-pack/web-app --name my-api --environment dev

# Create and apply immediately
pmp project create --template my-pack/web-app --name my-api --environment dev --apply
```

### 4. Manage Infrastructure

```bash
# Preview changes
pmp project preview

# Preview with color-coded diff visualization
pmp project preview --diff

# Preview with side-by-side diff view
pmp project preview --diff --side-by-side

# Export diff as HTML
pmp project preview --diff --diff-format html --diff-output plan.html

# Apply changes
pmp project apply

# Destroy infrastructure
pmp project destroy

# Destroy expired environments (dry-run by default)
pmp env purge
pmp env purge --force  # Execute destruction

# Refresh state
pmp project refresh
```

### 5. Work with Dependencies

```bash
# View dependency graph
pmp project graph

# Analyze dependencies
pmp project deps analyze

# Check impact of changes
pmp project deps impact my-api
```

### 6. Import Existing Infrastructure

Import cloud resources that aren't yet managed by Terraform/OpenTofu using [pmp-cloud-inspector](https://github.com/pmp-cloud-inspector) exports:

```bash
# Import from pmp-cloud-inspector export (recommended workflow)
pmp import infrastructure from-export ./cloud-inventory.json

# Filter by provider, type, or region
pmp import infrastructure from-export ./export.json --filter 'aws:ec2:*'
pmp import infrastructure from-export ./export.json --provider aws --region us-east-1

# Manual import (when you know the resource details)
pmp import infrastructure manual aws_vpc vpc-12345 --name main-vpc

# Batch import from YAML configuration
pmp import infrastructure batch ./import-config.yaml --yes
```

**Supported Providers:** AWS, Azure, GCP, GitHub, GitLab, JFrog Artifactory, Okta, Auth0, Jira, Opsgenie

See [Provider Permissions](doc/cloud-inspector-permissions.md) for required credentials and IAM permissions.

## Core Concepts

### Resource Definitions

PMP uses Kubernetes-style resource definitions with `apiVersion`, `kind`, `metadata`, and `spec`:

```yaml
apiVersion: pmp.io/v1
kind: Template
metadata:
  name: "Web Application"
  description: "Deploy a web application"
spec:
  apiVersion: pmp.io/v1
  kind: WebApp
  executor: opentofu
  inputs:
    - name: replicas
      type:
        type: number
        min: 1
        max: 10
      default: 3
```

### File Types

| File | Kind | Purpose |
|------|------|---------|
| `.pmp.infrastructure.yaml` | Infrastructure | Root configuration defining environments and categories |
| `.pmp.template-pack.yaml` | TemplatePack | Template pack metadata |
| `.pmp.template.yaml` | Template | Template definition with inputs and configuration |
| `.pmp.plugin.yaml` | Plugin | Reusable template component |
| `.pmp.project.yaml` | Project | Project identifier (metadata only) |
| `.pmp.environment.yaml` | ProjectEnvironment | Environment-specific configuration |

### Directory Structure

```
infrastructure/
├── .pmp.infrastructure.yaml      # Infrastructure configuration
└── projects/
    └── {project-name}/
        ├── .pmp.project.yaml     # Project identifier
        └── environments/
            └── {env-name}/
                ├── .pmp.environment.yaml  # Environment config
                ├── _common.tf             # Auto-generated backend
                └── *.tf                   # Generated Terraform files

~/.pmp/template-packs/
└── {pack-name}/
    ├── .pmp.template-pack.yaml   # Pack metadata
    ├── templates/
    │   └── {template-name}/
    │       ├── .pmp.template.yaml
    │       └── *.tf.hbs          # Handlebars templates
    └── plugins/
        └── {plugin-name}/
            ├── .pmp.plugin.yaml
            └── *.tf.hbs
```

## Commands Reference

### Infrastructure Commands

```bash
pmp infrastructure init [--output DIR]    # Initialize new infrastructure
```

### Project Commands

```bash
# Lifecycle
pmp project create [options]              # Create new project
pmp project find [--name NAME] [--kind KIND]  # Find projects
pmp project clone SOURCE NAME [--environment ENV]  # Clone project
pmp project update [--path PATH]          # Update from template

# Operations
pmp project preview [--cost] [--skip-policy] [-- EXECUTOR_ARGS]  # Plan changes
pmp project apply [--cost] [--skip-policy] [-- EXECUTOR_ARGS]    # Apply changes
pmp project destroy [--yes]               # Destroy infrastructure
pmp project refresh                       # Refresh state
pmp project test                          # Run tests

# Dependency Management
pmp project graph [--format FORMAT] [--output FILE]
pmp project deps analyze
pmp project deps impact PROJECT
pmp project deps validate
pmp project deps order
pmp project deps why PROJECT

# State Management
pmp project state list [--details]
pmp project state drift [PROJECT]
pmp project state lock PROJECT
pmp project state unlock PROJECT [--force]
pmp project state backup
pmp project state restore BACKUP_ID
pmp project state migrate BACKEND_TYPE
pmp project state sync

# Drift Detection
pmp project drift detect [--format FORMAT]
pmp project drift report [--output FILE]
pmp project drift reconcile [--auto-approve]

# Environment Management
pmp project env diff SOURCE TARGET
pmp project env promote SOURCE TARGET [--project NAME]
pmp project env sync
pmp project env variables [--environment ENV]

# Policy & Security
pmp project policy validate [--policy FILTER]
pmp project policy scan [--scanner SCANNER]
```

### OPA Policy Commands

```bash
pmp policy opa validate [--path PATH] [--policy FILTER]  # Validate against Rego policies
pmp policy opa test [--path PATH]                        # Run policy tests
pmp policy opa list                                      # List discovered policies
pmp policy opa report [--format FORMAT] [--output FILE]  # Generate compliance report (json/markdown/html)
```

Configure OPA policies in `.pmp.infrastructure.yaml`:

```yaml
spec:
  policy:
    enabled: true
    fail_on_violation: true
    opa:
      paths:
        - ./custom-policies        # Additional policy directories
      entrypoint: data.pmp         # Rego entrypoint (default)
      thresholds:
        block_on_error: true       # Block on deny violations
        max_warnings: 10           # Maximum warnings allowed
```

Policies are discovered from (in priority order):
1. `./policies/` - Project-local policies
2. `~/.pmp/policies/` - Global policies
3. Custom paths from configuration

**Automatic Validation**: When `spec.policy.enabled: true`, policies are automatically validated during:
- `pmp project preview` - Shows violations after plan (doesn't block)
- `pmp project apply` - Blocks apply if violations found

Use `--skip-policy` to bypass validation.

See **[OPA Policies Guide](doc/opa-policies.md)** for policy examples and best practices.

### Template Commands

```bash
pmp template scaffold [--output DIR]      # Create new template pack
pmp template lint [OPTIONS]               # Lint template packs for issues
```

#### Template Linting

Validate template packs for common issues:

```bash
# Lint all discovered template packs
pmp template lint

# Lint a specific pack
pmp template lint --pack my-pack

# Output as JSON
pmp template lint --format json

# Include info-level suggestions
pmp template lint --include-info

# Skip unused input detection (faster)
pmp template lint --skip-unused-inputs

# Use custom template pack paths
pmp template lint --template-packs-paths /path/to/packs:/other/path
```

**Checks performed:**
- Missing required fields (apiVersion, kind, metadata.name, spec.apiVersion, spec.kind, spec.executor)
- Unused inputs (defined but not used in templates)
- Invalid input configurations (min > max, empty options, etc.)
- Handlebars syntax errors and unclosed blocks
- Circular inheritance detection
- Best practices (missing descriptions, many inputs without defaults)

### CI/CD Commands

```bash
pmp ci generate TYPE [--output FILE]      # Generate pipeline
pmp ci detect-changes --base REF --head REF  # Detect changed projects
```

### Cost Commands

```bash
pmp cost estimate [--path PATH] [--format FORMAT]  # Estimate monthly costs
pmp cost diff [--path PATH]               # Compare current vs planned costs
pmp cost report [--format FORMAT] [--output FILE]  # Generate cost report
```

Requires [Infracost](https://www.infracost.io/) to be installed. Configure in `.pmp.infrastructure.yaml`:

```yaml
spec:
  cost:
    provider: infracost
    api_key_env: INFRACOST_API_KEY   # Optional: environment variable for API key
    thresholds:
      warn: 1000    # Warn if monthly cost > $1000
      block: 5000   # Block if monthly cost > $5000 (blocks apply with --cost flag)
    ci:
      enabled: true              # Enable cost estimation in CI pipelines
      comment_on_pr: true        # Post cost breakdown as PR comment (GitHub Actions)
      fail_on_threshold: true    # Fail CI if cost exceeds block threshold
```

Use `--cost` flag with preview/apply to see cost estimation:

```bash
pmp project preview --cost    # Show cost diff after plan
pmp project apply --cost      # Check costs before apply (blocks if threshold exceeded)
```

### Import Commands

```bash
# Import existing Terraform projects
pmp import project PATH                   # Import existing Terraform project
pmp import state PATH                     # Import from state file
pmp import resource ADDRESS               # Import specific resource
pmp import bulk CONFIG.yaml               # Bulk import from config

# Import cloud infrastructure (via pmp-cloud-inspector)
pmp import infrastructure from-export FILE [--filter PATTERN] [--provider PROVIDER]
pmp import infrastructure manual TYPE ID [--name NAME]
pmp import infrastructure batch CONFIG.yaml [--yes]
```

### Marketplace Commands

```bash
# Search and discover template packs
pmp marketplace search QUERY [--registry NAME]  # Search for packs
pmp marketplace list [--registry NAME]          # List available packs
pmp marketplace info PACK_NAME                  # Get pack details

# Install and update packs
pmp marketplace install PACK_NAME [--version VERSION]  # Install a pack
pmp marketplace update [PACK_NAME] [--all]             # Update installed packs

# Registry management
pmp marketplace registry add NAME --url URL   # Add URL-based registry
pmp marketplace registry add NAME --path PATH # Add filesystem registry
pmp marketplace registry list                 # List configured registries
pmp marketplace registry remove NAME          # Remove a registry

# Generate registry index (for hosting your own registry)
pmp marketplace generate-index [--output DIR] [--name NAME] [--description DESC]
```

Template packs are installed to `~/.pmp/template-packs/`. See **[Marketplace Guide](doc/marketplace.md)** for hosting your own registry with GitHub Pages.

### Other Commands

```bash
pmp generate [--template-pack PACK] [--template TEMPLATE]  # Generate without project
pmp search by-tags TAG=VALUE...           # Search by tags
pmp search by-resources TYPE              # Search by resource type
pmp search by-name PATTERN                # Search by name
pmp search by-output NAME                 # Search by output
pmp ui [--port PORT] [--host HOST]        # Start web UI
```

## Configuration Files

### Infrastructure Configuration

```yaml
apiVersion: pmp.io/v1
kind: Infrastructure
metadata:
  name: "My Infrastructure"
  description: "Production infrastructure"

spec:
  # Define available environments
  environments:
    dev:
      name: Development
    staging:
      name: Staging
    prod:
      name: Production

  # Organize templates into categories
  categories:
    - id: databases
      name: Databases
      templates:
        - template_pack: postgres
          template: rds

    - id: applications
      name: Applications
      subcategories:
        - id: apis
          name: API Services
          templates:
            - template_pack: kubernetes
              template: api-service

  # Template pack defaults (optional)
  template_packs:
    kubernetes:
      templates:
        api-service:
          defaults:
            inputs:
              replicas:
                value: 3
                show_as_default: true  # User can override

  # Backend configuration (optional)
  executor:
    name: opentofu
    config:
      backend:
        type: s3
        bucket: terraform-state
        region: us-west-2
        encrypt: true
        dynamodb_table: terraform-locks

  # Infrastructure-level hooks (optional)
  hooks:
    pre_apply:
      - type: confirm
        config:
          question: "Apply to production?"
          exit_on_cancel: true
```

### Project Naming Rules

- Allowed characters: lowercase letters (a-z), numbers (0-9), hyphens (-)
- Cannot start or end with a hyphen
- Cannot start with a number
- Must be unique within the infrastructure

**Examples**: `my-api`, `api-v2`, `database-primary`

## Import & Cloud Inspector

PMP can import existing cloud infrastructure into OpenTofu/Terraform management. The recommended workflow uses [pmp-cloud-inspector](https://github.com/pmp-cloud-inspector) to discover resources.

### Import Workflow

1. **Discover resources** with pmp-cloud-inspector:
   ```bash
   pmp-cloud-inspector scan --provider aws --region us-east-1 -o inventory.json
   ```

2. **Import into PMP**:
   ```bash
   pmp import infrastructure from-export inventory.json --target-project my-infra --target-environment prod
   ```

3. **Review generated files**:
   - `_imports.tf` - Import blocks for each resource
   - `_providers.tf` - Required providers with version constraints
   - `generated_resources.tf` - Resource configurations (after `tofu plan`)

4. **Apply the import**:
   ```bash
   cd projects/my-infra/environments/prod
   tofu apply
   ```

### Supported Providers

| Provider | Resources | Terraform Provider |
|----------|-----------|-------------------|
| AWS | EC2, VPC, S3, RDS, Lambda, ECS, EKS, IAM, etc. | `hashicorp/aws` |
| Azure | VMs, VNets, Storage, App Service, Key Vault, etc. | `hashicorp/azurerm` |
| GCP | Compute, Networks, Storage, Cloud Functions, Cloud Run | `hashicorp/google` |
| GitHub | Repositories, Teams, Memberships, Org Settings | `integrations/github` |
| GitLab | Projects, Groups, Users | `gitlabhq/gitlab` |
| JFrog | Repositories, Users, Groups, Permissions | `jfrog/artifactory` |
| Okta | Users, Groups, Applications, Auth Servers | `okta/okta` |
| Auth0 | Clients, Connections, Users, Roles, APIs | `auth0/auth0` |
| Opsgenie | Alert Policies | `opsgenie/opsgenie` |
| Jira | Projects | No stable provider |

### Provider Permissions

Each provider requires specific permissions for resource discovery. See **[Provider Permissions Guide](doc/cloud-inspector-permissions.md)** for:

- Required IAM policies (AWS)
- RBAC roles (Azure)
- Service account permissions (GCP)
- API token scopes (GitHub, GitLab, Okta, Auth0, etc.)
- Credential setup instructions
- Security best practices

## Development

### Build Commands

```bash
cargo build                    # Debug build
cargo build --release          # Release build
cargo test                     # Run tests
cargo clippy                   # Run linter
cargo fmt                      # Format code
```

### Running from Source

```bash
cargo run -- project create
cargo run -- project preview
cargo run -- project apply
```

### Project Structure

```
src/
├── main.rs              # CLI entry point and command definitions
├── commands/            # Command implementations
│   ├── create.rs        # Project creation
│   ├── apply.rs         # Apply changes
│   ├── preview.rs       # Preview changes
│   ├── graph.rs         # Dependency visualization
│   ├── deps.rs          # Dependency analysis
│   ├── state.rs         # State management
│   ├── ci.rs            # CI/CD generation
│   ├── import.rs        # Import commands
│   ├── ui.rs            # Web UI server
│   └── ...
├── collection/          # Project collection and discovery
├── executor/            # Executor implementations (OpenTofu, None)
├── hooks/               # Hook system implementation
├── import/              # Terraform project import
├── infrastructure/      # Cloud infrastructure import
│   ├── cloud_inspector.rs   # pmp-cloud-inspector integration
│   ├── resource_mapper.rs   # Resource type mapping
│   ├── config_generator.rs  # Import block generation
│   └── providers/           # Provider-specific logic
├── opa/                 # Native OPA policy integration
│   ├── provider.rs          # OpaProvider trait
│   ├── regorus.rs           # Regorus-based implementation
│   └── discovery.rs         # Policy discovery
├── template/            # Template discovery and rendering
│   ├── inheritance.rs       # Template inheritance
│   ├── partials.rs          # Handlebars partials
│   └── ...
└── traits/              # Shared interfaces

doc/
├── cloud-inspector-permissions.md  # Provider permissions guide
├── opa-policies.md                 # OPA policy guide and examples
├── ROADMAP.md                      # Development roadmap
└── ...                             # Feature documentation
```

## Architecture

### Design Principles

1. **Kubernetes-style Resources**: All configuration uses `apiVersion`, `kind`, `metadata`, `spec`
2. **Template-based Generation**: Handlebars templates with extensive input validation
3. **Dependency-first**: Projects can declare dependencies on other projects
4. **Multi-environment**: First-class support for dev/staging/prod workflows
5. **Extensible Executors**: Support for multiple IaC tools (OpenTofu, Terraform)

### Supported Backends

| Backend | Type | Configuration |
|---------|------|---------------|
| S3 | `s3` | bucket, region, encrypt, dynamodb_table |
| Azure Storage | `azurerm` | storage_account_name, container_name |
| Google Cloud Storage | `gcs` | bucket, prefix |
| Kubernetes | `kubernetes` | secret_suffix, namespace |
| PostgreSQL | `pg` | conn_str, schema_name |
| Consul | `consul` | path, address |
| HTTP | `http` | address, lock_address |
| Local | `local` | path |

See [CLAUDE.md](CLAUDE.md) for detailed architecture documentation.

## License

MIT License
