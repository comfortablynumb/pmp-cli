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

**Advanced Types**: All input types are documented in detail below.

### Complete Input Types Reference

PMP supports 25+ input types for flexible and powerful template configuration:

#### Basic Input Types

**String** - Text input
```yaml
- name: app_name
  type:
    type: string
  default: "my-app"
```

**Number** - Numeric input with validation
```yaml
- name: replicas
  type:
    type: number
    min: 1
    max: 10
  default: 3
```

**Boolean** - True/false toggle
```yaml
- name: enable_monitoring
  type:
    type: boolean
  default: true
```

**Password** - Sensitive text (hidden input)
```yaml
- name: admin_password
  type:
    type: password
  default: ""
```

**Email** - Email validation
```yaml
- name: contact_email
  type:
    type: email
  default: "admin@example.com"
```

**URL** - URL validation
```yaml
- name: webhook_url
  type:
    type: url
  default: "https://example.com/hook"
```

**IP** - IP address validation
```yaml
- name: server_ip
  type:
    type: ip
  default: "192.168.1.1"
```

**CIDR** - CIDR block validation
```yaml
- name: vpc_cidr
  type:
    type: cidr
  default: "10.0.0.0/16"
```

**Path** - File/directory path
```yaml
- name: config_path
  type:
    type: path
  default: "/etc/app/config.yaml"
```

**Port** - Network port number
```yaml
- name: service_port
  type:
    type: port
  default: 8080
```

**JSON** - JSON format validation
```yaml
- name: custom_config
  type:
    type: json
  default: "{}"
```

**YAML** - YAML format validation
```yaml
- name: config_yaml
  type:
    type: yaml
  default: ""
```

**ARN** - AWS ARN validation
```yaml
- name: role_arn
  type:
    type: arn
  default: ""
```

**DockerImage** - Docker image reference
```yaml
- name: container_image
  type:
    type: docker_image
  default: "nginx:latest"
```

**Region** - Cloud region selection
```yaml
- name: aws_region
  type:
    type: region
  default: "us-east-1"
```

#### Selection Input Types

**Select** - Single choice from options
```yaml
- name: environment_type
  type:
    type: select
    options:
      - label: "Development"
        value: "dev"
      - label: "Production"
        value: "prod"
  default: "dev"
```

**MultiSelect** - Multiple choices from options
```yaml
- name: enabled_features
  type:
    type: multiselect
    options:
      - label: "Monitoring"
        value: "monitoring"
      - label: "Logging"
        value: "logging"
      - label: "Tracing"
        value: "tracing"
  default: ["monitoring"]
```

#### List and Object Input Types

**List** - Comma-separated values
```yaml
- name: allowed_ips
  type:
    type: list
  default: "10.0.0.1,10.0.0.2"
```

**Object** - Single structured object with named fields
```yaml
- name: database_config
  type:
    type: object
    fields:
      - name: host
        type:
          type: string
        description: "Database host"
        default: "localhost"
      - name: port
        type:
          type: number
        description: "Database port"
        default: 5432
      - name: ssl_enabled
        type:
          type: boolean
        description: "Enable SSL"
        default: true
  description: "Database configuration"
```

**RepeatableObject** - Array of structured objects with repeatable prompts
```yaml
- name: team_members
  type:
    type: repeatable_object
    min: 0
    max: 50
    add_another_prompt: "Add another team member?"
    fields:
      - name: username
        type:
          type: string
        description: "GitHub username"
      - name: role
        type:
          type: select
          options:
            - label: "Member"
              value: "member"
            - label: "Maintainer"
              value: "maintainer"
        description: "Member role"
        default: "member"
  description: "Team members with roles"
```

**Interactive flow:**
```
Team members with roles:
  Add another team member? yes

  Team member #1:
    GitHub username: alice
    Member role: maintainer

  Add another team member? yes

  Team member #2:
    GitHub username: bob
    Member role: member

  Add another team member? no
```

**Template usage:**
```handlebars
{{#each team_members}}
resource "github_team_membership" "member_{{@index}}" {
  username = "{{username}}"
  role     = "{{role}}"
}
{{/each}}
```

#### Specialized Input Types

**Color** - Hex color with validation
```yaml
- name: brand_color
  type:
    type: color
    allow_alpha: true
  description: "Brand color"
  default: "#3B82F6"
```
- Validates: `#RRGGBB` or `#RRGGBBAA` (with alpha)
- Returns: String (e.g., "#3B82F6")

**Duration** - Time duration parsing
```yaml
- name: cache_ttl
  type:
    type: duration
    min_seconds: 60
    max_seconds: 86400
  description: "Cache time-to-live"
  default: "1h"
```
- Accepts: "30s", "5m", "1h30m", "2d", "1w"
- Units: s (seconds), m (minutes), h (hours), d (days), w (weeks)
- Returns: Number (seconds)

**Cron** - Cron expression validation
```yaml
- name: backup_schedule
  type:
    type: cron
  description: "Backup schedule"
  default: "0 2 * * *"
```
- Validates: 5 or 6 field cron expressions
- Format: `minute hour day month weekday [year]`
- Returns: String

**KeyValue** - Key-value pairs
```yaml
- name: labels
  type:
    type: keyvalue
    key_value_separator: "="
    pair_separator: ","
    min: 0
    max: 20
  description: "Resource labels"
  default: ""
```
- Input: `env=prod,team=platform,version=1.0`
- Returns: JSON object `{"env": "prod", "team": "platform", "version": "1.0"}`

**Semver** - Semantic version validation
```yaml
- name: app_version
  type:
    type: semver
    allow_prerelease: true
    allow_build: true
  description: "Application version"
  default: "1.0.0"
```
- Validates: `MAJOR.MINOR.PATCH[-PRERELEASE][+BUILD]`
- Examples: "1.0.0", "2.1.3-beta.1", "1.0.0+20230615"
- Returns: String

#### Project Reference Types

**ProjectSelect** - Single project reference
```yaml
- name: vpc_project
  type:
    type: project_select
    filter:
      apiVersion: pmp.io/v1
      kind: VPC
  description: "VPC project to use"
```

**MultiProjectSelect** - Multiple project references
```yaml
- name: dependent_services
  type:
    type: multi_project_select
    filter:
      apiVersion: pmp.io/v1
      kind: Service
  description: "Dependent services"
```

#### Conditional Inputs

Show/hide inputs based on other values:

```yaml
inputs:
  - name: enable_ssl
    type:
      type: boolean
    default: false

  - name: ssl_certificate_path
    type:
      type: path
    description: "SSL certificate path"
    show_if:
      - field: enable_ssl
        condition: equals
        value: true

  - name: ssl_key_path
    type:
      type: path
    description: "SSL key path"
    show_if:
      - field: enable_ssl
        condition: equals
        value: true
```

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

## Available Template Packs

### GitHub Template Pack

Manage GitHub resources including repositories and teams.

**Templates:**
- `repository` - Create and manage GitHub repositories

**Plugins:**
- `team` - Create GitHub teams with repeatable member management

**Team Plugin Example:**

The team plugin uses the `repeatable_object` input type to provide an intuitive interface for adding team members:

```yaml
apiVersion: pmp.io/v1
kind: Plugin
metadata:
  name: team

spec:
  role: access-management

  inputs:
    - name: team_name
      type: string
      description: Name of the GitHub team
      default: "${var:_project_name_hyphens}-team"

    - name: privacy
      type: select
      options:
        - label: "Secret (Only visible to organization owners and team members)"
          value: "secret"
        - label: "Closed (Visible to all organization members)"
          value: "closed"
      default: "secret"

    - name: team_members
      type: repeatable_object
      description: Team members with roles
      min: 0
      max: 100
      add_another_prompt: "Add another team member?"
      fields:
        - name: username
          type: string
          description: GitHub username
        - name: role
          type: select
          description: Member role
          options:
            - label: "Member (Regular team member)"
              value: "member"
            - label: "Maintainer (Team admin)"
              value: "maintainer"
          default: "member"
```

**Usage:**
```bash
pmp create
# Select: github template pack → team plugin
# Interactive prompts for each team member
```

### Argo CD Template Pack

Deploy and configure Argo CD with comprehensive SSO support.

**Templates:**
- `argo-cd` - Deploy Argo CD with optional SSO and RBAC configuration

**Features:**
- **Multiple SSO Providers:**
  - External OIDC (Azure AD, Google, Okta, Keycloak, Auth0)
  - Dex connectors (GitHub, GitLab, SAML, LDAP)

- **Optional RBAC Configuration:**
  - Pre-defined roles: Admin, Developer, Readonly
  - SSO group mappings
  - Custom policy CSV support

- **Automatic Configuration:**
  - Generates redirect URLs for SSO providers
  - Configures Helm chart with proper secrets
  - Provides detailed setup instructions in outputs

**SSO Configuration Example:**

```yaml
# Template inputs (30+ SSO-related inputs available)

# Core SSO
- name: enable_sso
  type: boolean
  default: false

- name: sso_provider_type
  type: select
  options:
    - label: "External OIDC (Okta, Auth0, Azure AD, Google, Keycloak)"
      value: "oidc"
    - label: "Dex (for GitHub, GitLab, SAML, LDAP)"
      value: "dex"
  default: "oidc"

# OIDC Configuration (when using OIDC provider)
- name: oidc_issuer
  type: url
  description: "OIDC issuer URL (e.g., https://accounts.google.com)"

- name: oidc_client_id
  type: string
  description: "OIDC client ID from your provider"

- name: oidc_client_secret
  type: password
  description: "OIDC client secret"

# Dex Configuration (when using Dex provider)
- name: dex_connector_type
  type: select
  options:
    - label: "GitHub"
      value: "github"
    - label: "GitLab"
      value: "gitlab"
    - label: "SAML"
      value: "saml"
    - label: "LDAP"
      value: "ldap"

# RBAC Configuration (optional)
- name: enable_rbac_config
  type: boolean
  default: false

- name: rbac_admin_group
  type: string
  description: "SSO group/email to map to admin role"

- name: rbac_developer_group
  type: string
  description: "SSO group/email to map to developer role"

- name: rbac_readonly_group
  type: string
  description: "SSO group/email to map to readonly role"
```

**Deployment Example:**

```bash
# Deploy Argo CD with Google OIDC SSO
pmp create

# Select: argo-cd template pack → argo-cd template

# Configuration prompts:
# - Namespace: argocd
# - Enable SSO: yes
# - SSO Provider: oidc
# - OIDC Issuer: https://accounts.google.com
# - OIDC Client ID: [your-client-id]
# - OIDC Client Secret: [your-client-secret]
# - Enable RBAC: yes
# - Admin Group: admin@company.com
# - Developer Group: developers@company.com

# Outputs will include:
# - SSO redirect URL to configure in Google
# - RBAC configuration summary
# - Access instructions
```

**Generated Outputs:**

```hcl
# SSO redirect URL for provider configuration
output "sso_redirect_url" {
  value = "https://argocd.example.com/auth/callback"
}

# Configuration summary
output "sso_configuration_summary" {
  value = <<-EOT
    SSO Configuration:
    - Provider Type: oidc
    - OIDC Issuer: https://accounts.google.com
    - Admin User: ENABLED (disable after confirming SSO works)
    - RBAC: ENABLED
      - Admin Group: admin@company.com
      - Developer Group: developers@company.com
      - Readonly Group: readonly@company.com
  EOT
}
```

## Real-World Examples

### Example 1: Create GitHub Team

```bash
pmp create

# Select GitHub template pack → team plugin
# Provide inputs:

Team Name: platform-team
Description: Platform Engineering Team
Privacy: secret

Team members:
  Add another team member? yes

  Team member #1:
    GitHub username: alice
    Member role: maintainer

  Add another team member? yes

  Team member #2:
    GitHub username: bob
    Member role: member

  Add another team member? yes

  Team member #3:
    GitHub username: charlie
    Member role: member

  Add another team member? no

# Generated Terraform creates:
# - GitHub team "platform-team"
# - 3 team memberships (alice as maintainer, bob and charlie as members)
```

### Example 2: Deploy Argo CD with Azure AD SSO

```bash
pmp create

# Select: argo-cd → argo-cd

# Core Configuration:
Namespace: argocd
Chart Version: 5.51.0
Create Namespace: yes
Server Host: argocd.company.com
Enable Ingress: yes
Ingress Class: nginx

# SSO Configuration:
Enable SSO: yes
SSO Provider Type: oidc
OIDC Name: Azure AD
OIDC Issuer: https://login.microsoftonline.com/{tenant-id}/v2.0
OIDC Client ID: {azure-app-client-id}
OIDC Client Secret: {azure-app-client-secret}
OIDC Requested Scopes: openid,profile,email,groups

# RBAC Configuration:
Enable RBAC: yes
RBAC Admin Group: ArgoCD-Admins
RBAC Developer Group: ArgoCD-Developers
RBAC Readonly Group: ArgoCD-Viewers
Default Policy: role:readonly

# Apply configuration:
cd collection/projects/argocd/argocd/environments/prod
pmp apply

# Configure Azure AD:
# - Add redirect URL: https://argocd.company.com/auth/callback
# - Ensure groups claim is included in token
# - Create Azure AD groups: ArgoCD-Admins, ArgoCD-Developers, ArgoCD-Viewers
```

## Roadmap

**Implemented** ✅
- Infrastructure organization
- Template system with hooks
- Multi-executor support
- Dependency management
- Environment management
- State management & drift
- CI/CD generation
- 25+ input types including Object and RepeatableObject
- GitHub team plugin with member management
- Argo CD SSO configuration (OIDC, Dex, RBAC)

**Planned** 🚧
- Policy framework
- Security scanning
- Cost estimation
- Template marketplace
- Additional template packs (AWS, Azure, GCP, Kubernetes)
