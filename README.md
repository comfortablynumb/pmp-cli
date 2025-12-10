# PMP - Poor Man's Platform

A CLI for managing Infrastructure as Code projects using OpenTofu/Terraform with template-based generation, dependency management, and workflow automation.

> **Work in Progress**: This project is under active development. Some features are incomplete or experimental. Features marked with `[WIP]` are not fully implemented.

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Core Concepts](#core-concepts)
- [Commands Reference](#commands-reference)
- [Configuration Files](#configuration-files)
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

### Additional Features

| Feature | Status | Description |
|---------|--------|-------------|
| **State Management** | Stable | List, lock/unlock, backup/restore state |
| **Drift Detection** | Stable | Detect and reconcile configuration drift |
| **Policy Validation** | Stable | Built-in policy checks (naming, security, deps) |
| **Security Scanning** | Stable | Integration with tfsec, checkov, trivy |
| **Dependency Analysis** | Stable | Impact analysis, validation, ordering |
| **Search** | Stable | Search by tags, resources, name, outputs |
| **Web UI** | `[WIP]` | Web interface for project management |
| **Import** | `[WIP]` | Import existing Terraform projects |

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

# Apply changes
pmp project apply

# Destroy infrastructure
pmp project destroy

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
pmp project preview [-- EXECUTOR_ARGS]    # Plan changes
pmp project apply [-- EXECUTOR_ARGS]      # Apply changes
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

### Template Commands

```bash
pmp template scaffold [--output DIR]      # Create new template pack
```

### CI/CD Commands

```bash
pmp ci generate TYPE [--output FILE]      # Generate pipeline
pmp ci detect-changes --base REF --head REF  # Detect changed projects
```

### Other Commands

```bash
pmp generate [--template-pack PACK] [--template TEMPLATE]  # Generate without project
pmp search by-tags TAG=VALUE...           # Search by tags
pmp search by-resources TYPE              # Search by resource type
pmp search by-name PATTERN                # Search by name
pmp search by-output NAME                 # Search by output
pmp ui [--port PORT] [--host HOST]        # Start web UI [WIP]
pmp import project PATH                   # Import existing project [WIP]
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
│   └── ...
├── collection/          # Project collection and discovery
├── executor/            # Executor implementations (OpenTofu, None)
├── hooks/               # Hook system implementation
├── template/            # Template discovery and rendering
└── traits/              # Shared interfaces
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
