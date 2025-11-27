# PMP - Poor Man's Platform

A CLI for managing Infrastructure as Code projects using OpenTofu/Terraform with template-based generation, dependency management, and workflow automation.

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Infrastructure Configuration](#infrastructure-configuration)
- [Hooks System](#hooks-system)
- [Templates](#templates)
- [Projects](#projects)
- [Dependencies](#dependencies)
- [State & Drift](#state--drift)
- [Environments](#environments)
- [CI/CD](#cicd)
- [Commands](#commands)

## Features

**Core**
- Template-based project creation with Handlebars
- Multi-executor support (OpenTofu, Terraform, None)
- Dependency graph management and visualization
- Pre/post execution hooks (command, confirm, set_environment)
- Environment comparison and promotion
- State management with drift detection
- CI/CD pipeline generation (GitHub Actions, GitLab, Jenkins)

**Advanced**
- Plugin system for templates
- Backend configuration (S3, Azure, GCS, Kubernetes, PostgreSQL)
- Project cloning and updates
- Helm integration with auto-repo-update

## Installation

```bash
cargo build --release
export PATH="$PATH:$(pwd)/target/release"  # or add to ~/.bashrc
```

## Quick Start

### 1. Initialize Infrastructure

```bash
# Create .pmp.infrastructure.yaml
cat > .pmp.infrastructure.yaml << 'EOF'
apiVersion: pmp.io/v1
kind: Infrastructure
metadata:
  name: "My Infrastructure"
spec:
  categories:
    - id: apps
      name: Applications
      templates:
        - template_pack: kubernetes
          template: web-app

  environments:
    dev:
      name: Development
    prod:
      name: Production
EOF
```

### 2. Create Template Pack

```bash
pmp template scaffold --output ./templates
```

### 3. Create Project

```bash
pmp project create
# Select template, environment, provide inputs
# Files generated in projects/{kind}/{name}/environments/{env}/
```

### 4. Manage Projects

```bash
pmp project preview   # Plan changes
pmp project apply     # Apply changes
pmp project destroy   # Destroy infrastructure
```

## Infrastructure Configuration

### Complete Example

```yaml
apiVersion: pmp.io/v1
kind: Infrastructure
metadata:
  name: "Production Infrastructure"

spec:
  # Organize templates by category
  categories:
    - id: databases
      name: Databases
      templates:
        - template_pack: postgres
          template: postgres

    - id: apps
      name: Applications
      subcategories:
        - id: apis
          name: API Services
          templates:
            - template_pack: kubernetes
              template: api-service

  # Define environments (lowercase alphanumeric + underscores)
  environments:
    dev:
      name: Development
    staging:
      name: Staging
    prod:
      name: Production

  # Template input defaults/overrides
  template_packs:
    postgres:
      templates:
        postgres:
          defaults:
            inputs:
              instance_class:
                value: "db.t3.medium"
                show_as_default: true  # Show as default, user can override

  # Infrastructure-level hooks (apply to all projects)
  hooks:
    pre_apply:
      - type: confirm
        config:
          question: "Apply to production?"
          exit_on_cancel: true

  # Backend configuration
  executor:
    name: opentofu
    config:
      backend:
        type: s3
        bucket: my-terraform-state
        region: us-west-2
        encrypt: true
        dynamodb_table: terraform-locks
```

**Supported Backends**: s3, azurerm, gcs, kubernetes, pg, consul, http, local

## Hooks System

Hooks run custom workflows before/after operations. Define at **three levels**:

1. **Infrastructure** (`.pmp.infrastructure.yaml`) - All projects
2. **Template** (`.pmp.template.yaml`) - Copied to projects created from template
3. **Environment** (`.pmp.environment.yaml`) - Project-specific

**Execution order**: Infrastructure → Template → Environment

### Hook Types

#### 1. Confirm Hook

Prompt for Yes/No confirmation:

```yaml
hooks:
  pre_destroy:
    - type: confirm
      config:
        question: "Destroy database? All data will be LOST!"
        exit_on_cancel: true   # Stop if No (default: true)
        exit_on_confirm: false # Stop if Yes (default: false)
```

**Behavior**:
- Default answer: No
- `exit_on_cancel: true` → Stop when user says No
- `exit_on_confirm: true` → Stop when user says Yes (rare)

**Use cases**: Prevent accidental destruction, double confirmations, pre-flight checks

#### 2. Set Environment Hook

Collect input and set environment variables:

```yaml
hooks:
  pre_apply:
    - type: set_environment
      config:
        name: AWS_ACCESS_KEY_ID
        prompt: "AWS Access Key:"
        sensitive: false  # Show input (default)

    - type: set_environment
      config:
        name: AWS_SECRET_ACCESS_KEY
        prompt: "AWS Secret:"
        sensitive: true  # Hide input
```

**Features**:
- **Smart defaults**: Uses existing env var value as default (non-sensitive only)
- **Security**: Sensitive inputs never show defaults
- **Convenience**: Press Enter to keep current value

```bash
# If already set in shell:
$ export AWS_REGION=us-west-2
$ pmp project apply
# Prompt shows: AWS region: [us-west-2]
```

**Use cases**: Cloud credentials, Terraform variables (`TF_VAR_*`), API keys, database passwords

#### 3. Command Hook

Execute shell commands:

```yaml
hooks:
  pre_apply:
    - type: command
      config:
        command: "aws sts get-caller-identity"

  post_apply:
    - type: command
      config:
        command: "echo Deployed at $(date) >> deployments.log"
```

**Use cases**: Validation, notifications, logging, security scans

### Hook Phases

Available for all hook types:
- `pre_preview` / `post_preview`
- `pre_apply` / `post_apply`
- `pre_destroy` / `post_destroy`
- `pre_refresh` / `post_refresh`

### Template Hooks

Define hooks in templates - automatically added to generated projects:

```yaml
# In .pmp.template.yaml
spec:
  apiVersion: pmp.io/v1
  kind: PostgresDatabase
  executor: opentofu

  inputs:
    - name: instance_type
      type:
        type: string

  # Hooks copied to .pmp.environment.yaml on project creation
  hooks:
    pre_destroy:
      - type: confirm
        config:
          question: "Destroy database? Data will be LOST!"
          exit_on_cancel: true

      - type: confirm
        config:
          question: "Type 'yes' again:"
          exit_on_cancel: true

    pre_apply:
      - type: set_environment
        config:
          name: DB_PASSWORD
          prompt: "Master password:"
          sensitive: true
```

**Benefits**:
- Templates embed safety measures
- Projects inherit best practices automatically
- Users can customize in environment files

### Complete Hooks Example

```yaml
hooks:
  pre_apply:
    # 1. Confirmation
    - type: confirm
      config:
        question: "Deploy to production?"
        exit_on_cancel: true

    # 2. Credentials
    - type: set_environment
      config:
        name: AWS_ACCESS_KEY_ID
        prompt: "AWS Key:"
        sensitive: false

    - type: set_environment
      config:
        name: AWS_SECRET_ACCESS_KEY
        prompt: "AWS Secret:"
        sensitive: true

    # 3. Validation
    - type: command
      config:
        command: "aws sts get-caller-identity"

    # 4. Security scan
    - type: command
      config:
        command: "tfsec ."

  post_apply:
    - type: command
      config:
        command: "curl -X POST $WEBHOOK -d 'Deployed'"
```

## Templates

### Template Pack Structure

```
~/.pmp/template-packs/my-pack/
├── .pmp.template-pack.yaml
├── templates/
│   └── my-template/
│       ├── .pmp.template.yaml
│       ├── main.tf.hbs
│       ├── variables.tf.hbs
│       └── outputs.tf.hbs
└── plugins/
    └── my-plugin/
        ├── .pmp.plugin.yaml
        └── plugin.tf.hbs
```

### Template File

```yaml
apiVersion: pmp.io/v1
kind: Template
metadata:
  name: "API Service"
  labels:
    tier: backend

spec:
  apiVersion: pmp.io/v1
  kind: KubernetesWorkload
  executor: opentofu

  inputs:
    - name: replicas
      description: "Number of replicas"
      default: 3
      type:
        type: number
        min: 1
        max: 10

    - name: environment
      description: "Environment type"
      type:
        type: select
        options:
          - label: "Development"
            value: "dev"
          - label: "Production"
            value: "prod"

  # Environment-specific overrides
  environments:
    prod:
      overrides:
        inputs:
          - name: replicas
            default: 5

  # Plugin configuration
  plugins:
    allowed:
      - template_pack_name: postgres
        plugin_name: access
    installed:
      - template_pack_name: github
        plugin_name: repository

  # Dependencies
  dependencies:
    - project:
        apiVersion: pmp.io/v1
        kind: PostgresDatabase
        description: "Select database"

  # Template hooks (copied to projects)
  hooks:
    pre_apply:
      - type: confirm
        config:
          question: "Deploy to production?"
          exit_on_cancel: true
```

### Template Variables

**System Variables** (auto-provided):
- `{{_name}}` - Project name (`my-api`)
- `{{_project_name_underscores}}` - Name with underscores (`my_api`)
- `{{_environment}}` - Environment name (`dev`, `prod`)
- `{{_resource_api_version}}` - Resource API version (`pmp.io/v1`)
- `{{_resource_kind}}` - Resource kind (`KubernetesWorkload`)

**User Inputs**: All template inputs are available by name: `{{replicas}}`, `{{namespace}}`, etc.

**Plugin Variables** (when plugins are installed):
- `{{_plugins.added}}` - Array of installed plugins
- `{{_reference_project_name}}` - Reference project name (in plugins)

**Handlebars Helpers**:
- `eq` - Equality: `{{#if (eq env "prod")}}...{{/if}}`
- `contains` - Array contains: `{{#if (contains privileges "SELECT")}}...{{/if}}`
- `k8s_name` - Sanitize for Kubernetes: `{{k8s_name _name}}`
- `bool` - Boolean conversion: `{{bool enable_feature}}`

**Examples**:
```handlebars
# Conditional logic
{{#if (eq _environment "production")}}
  replicas = 5
{{else}}
  replicas = 1
{{/if}}

# Plugin integration
{{#if _plugins.added}}
{{#each _plugins.added}}
module "{{template_pack_name}}_{{name}}" {
  source = "./modules/{{template_pack_name}}/{{name}}"
}
{{/each}}
{{/if}}

# Loops
{{#each databases}}
  db_{{@index}}: {{this}}
{{/each}}
```

### Input Types

**String**:
```yaml
inputs:
  - name: app_name
    type:
      type: string
    default: "myapp"
```

**Number** (with optional min/max):
```yaml
inputs:
  - name: replicas
    type:
      type: number
      min: 1
      max: 10
    default: 3
```

**Boolean** (yes/no):
```yaml
inputs:
  - name: enable_monitoring
    type:
      type: boolean
    default: true
```

**Select** (dropdown with labels):
```yaml
inputs:
  - name: instance_size
    type:
      type: select
      options:
        - label: "Small (2 CPU, 4GB RAM)"
          value: "t3.small"
        - label: "Large (8 CPU, 16GB RAM)"
          value: "t3.large"
    default: "t3.small"
```

**Variable Interpolation** - Reference other inputs or system variables:
```yaml
inputs:
  - name: app_name
    type:
      type: string
    default: "${var:_name}"  # Use project name

  - name: namespace
    type:
      type: string
    default: "${var:app_name}-${var:_environment}"  # Combine variables

  - name: database_url
    type:
      type: string
    default: "postgresql://${var:app_name}-db:5432/${var:app_name}"
```

**Advanced Types**: password, multiselect, project_select, multiproject_select, path, url, date, datetime, json, yaml, list, email, ip, cidr, port

### Create Template

```bash
pmp template scaffold --output ./my-templates
```

## Projects

### Structure

```
projects/{kind}/{name}/
├── .pmp.project.yaml          # Project identifier
└── environments/{env}/
    ├── .pmp.environment.yaml  # Env spec + inputs + hooks
    ├── _common.tf             # Auto-generated backend
    ├── main.tf                # From template
    └── variables.tf           # From template
```

### Lifecycle

```bash
# Create
pmp project create

# Find
pmp project find --name my-api
pmp project find --kind KubernetesWorkload

# Clone
pmp project clone my-api new-api
pmp project clone my-api new-api --environment prod

# Update
pmp project update

# Operations
pmp project preview
pmp project apply
pmp project refresh
pmp project destroy --yes
```

### Naming Rules

- **Allowed**: lowercase, numbers, hyphens
- **Cannot**: start/end with hyphen, start with number, use underscores
- **Examples**: ✅ `my-api` ✅ `api-v2` ❌ `My-API` ❌ `_api`

## Dependencies

### Define

```yaml
# In .pmp.environment.yaml
spec:
  dependencies:
    - project:
        name: postgres-db
        environments:
          - dev
          - prod
```

### Commands

```bash
pmp project deps analyze    # Full analysis
pmp project deps impact my-api  # What depends on this?
pmp project graph          # ASCII tree
pmp project graph --format mermaid  # Mermaid.js
pmp project graph --format dot      # GraphViz
```

### Dependency-Only Projects

```yaml
# Template with executor: none
spec:
  executor: none  # No infrastructure operations
  dependencies: []
```

**Use cases**: Group microservices, environment-wide deployments, staged rollouts

## State & Drift

```bash
# State
pmp project state list
pmp project state drift
pmp project state lock my-project
pmp project state unlock my-project --force
pmp project state backup
pmp project state restore 20250116_143000

# Drift
pmp project drift detect
pmp project drift report --format json
pmp project drift reconcile --auto-approve
```

## Environments

```bash
# Compare
pmp project env diff dev staging

# Promote (with backup)
pmp project env promote dev staging
pmp project env promote dev staging --project my-api

# View variables
pmp project env variables --environment prod
pmp project env variables --project my-api
```

## CI/CD

```bash
# Generate pipelines
pmp ci generate github-actions --output .github/workflows/deploy.yml
pmp ci generate gitlab-ci --output .gitlab-ci.yml
pmp ci generate jenkins --output Jenkinsfile

# Change detection
pmp ci detect-changes --base origin/main --head HEAD
```

**Pipeline features**:
- Dependency-aware execution order
- Parallel execution for independent projects
- Change detection (only deploy modified)
- Multi-environment support

## Commands

### Infrastructure

```bash
pmp infrastructure init [--output DIR]
```

### Projects

```bash
# Lifecycle
pmp project create
pmp project find --name <name> | --kind <kind>
pmp project clone <source> <new-name> [--environment ENV]
pmp project update

# Operations
pmp project preview [-- ARGS]
pmp project apply [-- ARGS]
pmp project destroy [--yes]
pmp project refresh

# Dependencies
pmp project deps analyze | impact <name> | validate | order | why <name>
pmp project graph [--all] [--format mermaid|dot] [--output FILE]

# State
pmp project state list | drift | lock <name> | unlock <name> [--force]
pmp project state backup | restore <id> | migrate <backend> | sync

# Drift
pmp project drift detect | report [--format json] | reconcile [--auto-approve]

# Environments
pmp project env diff <src> <target>
pmp project env promote <src> <target> [--project NAME]
pmp project env sync [--project NAME]
pmp project env variables [--environment ENV] [--project NAME]
```

### Templates

```bash
pmp template scaffold [--output DIR]
```

### CI/CD

```bash
pmp ci generate <type> [--output FILE] [--environment ENV] [--static]
pmp ci detect-changes --base <ref> --head <ref> [--environment ENV]
```

### Utilities

```bash
pmp generate        # Template generation (no infra)
pmp ui [--port N]   # Web UI
pmp search by-tags | by-resources | by-name
```

## Development

```bash
# Build
cargo build --release

# Test
cargo test --all

# Run
cargo run -- project create
```

**Project structure**:
```
src/
├── collection/   # Discovery, dependency graph
├── commands/     # CLI implementation
├── executor/     # OpenTofu/Terraform/None
├── hooks/        # Hook system
├── template/     # Discovery, rendering
└── main.rs       # Entry point
```

## License

MIT License

## Architecture

See [CLAUDE.md](CLAUDE.md) for design principles and implementation details.

## Roadmap

**Implemented** ✅
- Infrastructure organization
- Template system with hooks
- Multi-executor support
- Dependency management
- Environment management
- State management & drift
- CI/CD generation

**Planned** 🚧
- Policy framework
- Security scanning
- Cost estimation
- Template marketplace
