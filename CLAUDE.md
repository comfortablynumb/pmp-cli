# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

PMP (Poor Man's Platform) is a CLI for managing Infrastructure as Code projects using OpenTofu/Terraform with template-based project generation. It uses Kubernetes-style resource definitions (apiVersion, kind, metadata, spec) for all configuration files.

## Key Design Principles

1. **Templates use `.pmp.template.yaml`** - Template metadata with `apiVersion: pmp.io/v1` and `kind: Template`
2. **Projects use `.pmp.project.yaml`** - Project identifier with `apiVersion: pmp.io/v1` and `kind: Project` (metadata only, no spec)
3. **Environments use `.pmp.environment.yaml`** - Environment spec with `apiVersion: pmp.io/v1` and `kind: ProjectEnvironment`
4. **Resources have apiVersion and kind** - Resources may define inputs by environment
5. **Projects organized by environment** - Each project has `environments/` directory with subdirectories per environment

## Development Commands

```bash
cargo build                              # Build the project
cargo test                               # Run tests
cargo run -- create                      # Create project
cargo run -- find --name foo             # Find projects
cargo run -- graph                       # Visualize dependency graph
cargo run -- deps analyze                # Analyze dependencies
cargo run -- state list                  # List state across projects
cargo run -- state drift                 # Detect configuration drift
cargo run -- ci generate github          # Generate CI/CD workflows
cargo run -- template scaffold           # Create new template pack
cargo run -- template lint               # Lint template packs
cargo run -- clone source target         # Clone existing project
cargo run -- env diff dev staging        # Compare environments
cargo run -- project preview --diff      # Preview with color-coded diff
cargo run -- env purge                   # Destroy expired environments
```

## Architecture

### File Structure

```
~/.pmp/template-packs/<pack-name>/
├── .pmp.template-pack.yaml (kind: TemplatePack)
├── templates/
│   └── <template-name>/
│       ├── .pmp.template.yaml (kind: Template)
│       └── ... (template files)
└── plugins/
    └── <plugin-name>/
        ├── .pmp.plugin.yaml (kind: Plugin)
        └── ... (plugin files)

collection/
├── .pmp.infrastructure.yaml
└── projects/
    └── {project_name}/
        ├── .pmp.project.yaml
        └── environments/
            └── {environment_name}/
                ├── .pmp.environment.yaml
                └── ... (generated files)
```

### Template Pack System

**Template Pack** (`.pmp.template-pack.yaml`):
- `apiVersion: pmp.io/v1`, `kind: TemplatePack`
- Located in: `~/.pmp/template-packs/<pack-name>/` or `.pmp/template-packs/<pack-name>/`
- Contains metadata, empty spec

**Template** (`.pmp.template.yaml`):
- `apiVersion: pmp.io/v1`, `kind: Template`
- Located in: `<pack-dir>/templates/<template-name>/`
- Spec contains: `apiVersion`, `kind`, `executor`, `inputs`, `environments`
- Resource kind must be alphanumeric only

**Plugin** (`.pmp.plugin.yaml`):
- `apiVersion: pmp.io/v1`, `kind: Plugin`
- Located in: `<pack-dir>/plugins/<plugin-name>/`
- Reusable components referenced by templates
- Contains metadata and spec with `role` and `inputs`

### Input Types System

PMP supports 25+ input types for collecting user input during project creation. All input types can be used in both templates (`.pmp.template.yaml`) and plugins (`.pmp.plugin.yaml`).

#### Basic Input Types

**String**
```yaml
- name: project_name
  type: string
  description: Name of the project
  default: "my-project"
```

**Number**
```yaml
- name: replica_count
  type: number
  description: Number of replicas
  default: 3
  min: 1
  max: 10
```

**Boolean**
```yaml
- name: enable_monitoring
  type: boolean
  description: Enable monitoring
  default: true
```

**Password**
```yaml
- name: admin_password
  type: password
  description: Administrator password
  default: ""
```

**Email**
```yaml
- name: contact_email
  type: email
  description: Contact email address
  default: ""
```

**URL**
```yaml
- name: webhook_url
  type: url
  description: Webhook URL
  default: ""
```

**IP**
```yaml
- name: server_ip
  type: ip
  description: Server IP address
  default: ""
```

**CIDR**
```yaml
- name: vpc_cidr
  type: cidr
  description: VPC CIDR block
  default: "10.0.0.0/16"
```

**JSON**
```yaml
- name: custom_config
  type: json
  description: Custom configuration in JSON format
  default: "{}"
```

**YAML**
```yaml
- name: config_yaml
  type: yaml
  description: Configuration in YAML format
  default: ""
```

#### Selection Input Types

**Select** (Single choice from options)
```yaml
- name: environment_type
  type: select
  description: Type of environment
  options:
    - label: "Development"
      value: "dev"
    - label: "Production"
      value: "prod"
  default: "dev"
```

**MultiSelect** (Multiple choices from options)
```yaml
- name: enabled_features
  type: multiselect
  description: Features to enable
  options:
    - label: "Monitoring"
      value: "monitoring"
    - label: "Logging"
      value: "logging"
    - label: "Tracing"
      value: "tracing"
  default: ["monitoring"]
```

#### List and Array Input Types

**List** (Comma-separated values)
```yaml
- name: allowed_ips
  type: list
  description: Allowed IP addresses (comma-separated)
  default: ""
```

**Object** (Single structured object with named fields)
```yaml
- name: database_config
  type: object
  description: Database configuration
  fields:
    - name: host
      type: string
      description: Database host
      default: "localhost"
    - name: port
      type: number
      description: Database port
      default: 5432
    - name: ssl_enabled
      type: boolean
      description: Enable SSL
      default: true
```
- Groups multiple related inputs into a single structured object
- Supports nested objects (fields can also be of type `object`)
- Each field can use any supported input type
- Returns JSON object with field names as keys

**RepeatableObject** (Array of structured objects with add/remove functionality)
```yaml
- name: team_members
  type: repeatable_object
  description: Team members with roles
  min: 0
  max: 50
  add_another_prompt: "Add another team member?"
  fields:
    - name: username
      type: string
      description: GitHub username
    - name: role
      type: select
      description: Member role
      options:
        - label: "Member"
          value: "member"
        - label: "Maintainer"
          value: "maintainer"
      default: "member"
```
- Interactive workflow: User can **Add**, **Remove**, or mark **Done**
- Shows current item count after each operation
- When removing, displays a list of existing items with summaries for easy selection
- Respects `min` and `max` constraints during add/remove operations
- Returns array of objects as JSON

#### Project Reference Input Types

**ProjectSelect** (Single project reference)
```yaml
- name: vpc_project
  type: project_select
  description: VPC project to use
  filter:
    apiVersion: pmp.io/v1
    kind: VPC
```

**MultiProjectSelect** (Multiple project references)
```yaml
- name: dependent_services
  type: multi_project_select
  description: Dependent services
  filter:
    apiVersion: pmp.io/v1
    kind: Service
```

#### Specialized Input Types

**Color** (Hex color with validation)
```yaml
- name: brand_color
  type: color
  description: Brand color
  allow_alpha: true
  default: "#3B82F6"
```
- Validates hex color format: `#RRGGBB` or `#RRGGBBAA` (with alpha)
- Returns string value (e.g., "#3B82F6" or "#3B82F6FF")

**Duration** (Time duration parsing)
```yaml
- name: cache_ttl
  type: duration
  description: Cache time-to-live
  min_seconds: 60
  max_seconds: 86400
  default: "1h"
```
- Accepts formats: "1h30m", "5d", "2w", "30s"
- Units: s (seconds), m (minutes), h (hours), d (days), w (weeks)
- Returns number (seconds)

**Cron** (Cron expression validation)
```yaml
- name: backup_schedule
  type: cron
  description: Backup schedule (cron expression)
  default: "0 2 * * *"
```
- Validates cron expressions (5 or 6 fields)
- Format: `minute hour day month weekday [year]`
- Returns string value

**KeyValue** (Key-value pairs)
```yaml
- name: labels
  type: keyvalue
  description: Resource labels
  key_value_separator: "="
  pair_separator: ","
  min: 0
  max: 20
  default: ""
```
- Input format: `key1=value1,key2=value2`
- Returns JSON object: `{"key1": "value1", "key2": "value2"}`

**Semver** (Semantic version validation)
```yaml
- name: app_version
  type: semver
  description: Application version
  allow_prerelease: true
  allow_build: true
  default: "1.0.0"
```
- Validates semantic versioning: `MAJOR.MINOR.PATCH[-PRERELEASE][+BUILD]`
- Examples: "1.0.0", "2.1.3-beta.1", "1.0.0+20230615"
- Returns string value

**Region** (Cloud region selection)
```yaml
- name: aws_region
  type: region
  description: AWS region
  default: "us-east-1"
```

**Path** (File/directory path)
```yaml
- name: config_path
  type: path
  description: Configuration file path
  default: "/etc/app/config.yaml"
```

**Port** (Network port number)
```yaml
- name: service_port
  type: port
  description: Service port
  default: 8080
```

**ARN** (AWS ARN validation)
```yaml
- name: role_arn
  type: arn
  description: IAM role ARN
  default: ""
```

**DockerImage** (Docker image reference)
```yaml
- name: container_image
  type: docker_image
  description: Container image
  default: "nginx:latest"
```

#### Input Type Features

**Conditional Inputs** - Show/hide inputs based on other values:
```yaml
- name: enable_ssl
  type: boolean
  default: false

- name: ssl_certificate
  type: string
  description: SSL certificate path
  show_if:
    - field: enable_ssl
      condition: equals
      value: true
```

**Default Value Interpolation** - Use variables in defaults:
```yaml
- name: namespace
  type: string
  default: "${var:_project_name_hyphens}-ns"
```

**Template Rendering** - Access input values in templates:
```hcl
# Handlebars template (.tf.hbs)
resource "kubernetes_namespace" "app" {
  metadata {
    name = "{{namespace}}"
  }
}

{{#if enable_monitoring}}
resource "kubernetes_service_monitor" "app" {
  # ... monitoring config
}
{{/if}}

{{#each team_members}}
resource "github_team_membership" "member_{{@index}}" {
  username = "{{username}}"
  role     = "{{role}}"
}
{{/each}}
```

**Handlebars Helpers**:
- `{{bool variable_name}}` - Boolean to HCL (true/false)
- `{{json variable_name}}` - JSON stringify
- `{{secret input_name}}` - Secret reference (`local.secret_<input_name>`)
- `{{#if variable}}...{{/if}}` - Conditional rendering
- `{{#each array}}...{{/each}}` - Array iteration
- `{{#eq a b}}...{{/eq}}` - Equality comparison

### Plugin System

**Allowed Plugins** (`spec.plugins.allowed`):
- Templates declare which plugins can be used
- Structure: template reference (apiVersion, kind), plugin name, optional input constraints
- Plugins selected manually by users during updates

**Installed Plugins** (`spec.plugins.installed`):
- Plugins automatically installed during project creation
- Cannot be removed
- User can customize inputs or use defaults
- Set `disable_user_input_override: true` to skip user prompt and use defaults/configured values
- Processed before template rendering

**Plugin Configuration in Project Groups**:

Projects in `spec.projects.list` can pre-configure plugins (both installed and allowed):

```yaml
spec:
  projects:
    list:
      - name: my-project
        template_pack: pack-name
        template: template-name
        plugins:
          <plugin-name>:
            reference_projects:
              - name: reference-project-name
                environment: env-name  # Optional, defaults to project group environment
                dependency_name: dep-name  # Optional, matches by apiVersion/kind if not specified
            inputs:
              <input-name>:
                value: <value>
                # OR
                use_default: true
```

**Behavior:**
- Pre-configured dependencies skip interactive prompts
- Partial configuration: prompts only for unconfigured dependencies
- Applies to both `spec.plugins.installed` and `spec.plugins.allowed`
- Input precedence: pre-configured value > template default
- Dependency matching: by dependency_name (if specified) or by apiVersion+kind (fallback)

### Infrastructure System

**File**: `.pmp.infrastructure.yaml` (Required)

**Must define**:
- `spec.environments` - Available environments (lowercase alphanumeric + underscores)
- `spec.categories` - Hierarchical tree organizing templates

**Categories Structure**:
```yaml
spec:
  categories:
    - id: category_id
      name: Display Name
      description: Optional
      subcategories: []
      templates:
        - template_pack: pack-name
          template: template-name
```

**Template Pack Configuration** (Optional):
```yaml
spec:
  template_packs:
    <pack-name>:
      templates:
        <template-name>:
          defaults:
            inputs:
              <input-name>:
                value: <value>
                show_as_default: true  # true: user can override; false: skip prompt
```

**Executor Configuration** (Optional):
```yaml
spec:
  executor:
    name: opentofu
    config:
      backend:
        type: s3  # or azurerm, gcs, kubernetes, pg, consul, etc.
        # ... backend-specific parameters
    parallel:
      max: 4  # Max concurrent projects (default: 1 = sequential)
      on_failure: continue  # stop, continue (default), or finish_level
```

**Supported Backends**: local, s3, azurerm, gcs, http, kubernetes, pg, consul, cos, oss, remote

### Parallel Execution

Execute multiple projects at the same dependency level concurrently:

**Configuration** (`.pmp.infrastructure.yaml`):
```yaml
spec:
  executor:
    parallel:
      max: 4                   # Maximum concurrent executions
      on_failure: continue     # Behavior on failure
```

**CLI Override**:
```bash
pmp project preview --parallel 4   # Override config
pmp project apply --parallel 4     # Execute up to 4 projects in parallel
pmp project destroy --parallel 4   # Destroy with parallelism
pmp project test --parallel 4      # Test with parallelism
```

**Failure Behaviors**:
- `stop`: Stop execution immediately on any failure
- `continue` (default): Continue executing remaining projects
- `finish_level`: Finish current level, then stop

**How it works**:
- Projects grouped by dependency level (level 0 = no dependencies)
- All projects in a level execute concurrently (up to `max`)
- Respects dependency order (level N completes before level N+1)
- For destroy, levels are reversed (dependents destroyed first)

**Secrets Configuration** (Optional):
```yaml
spec:
  secrets:
    managers:
      # Static configuration
      - name: dev-vault
        type: vault
        config:
          address: https://vault-dev.example.com
          namespace: dev

      # Dynamic configuration from PMP project outputs
      - name: production-vault
        type: vault
        project:
          name: vault-cluster
          environment: production  # Optional
          outputs:
            address: vault_url
            namespace: vault_namespace

      # AWS Secrets Manager
      - name: aws-secrets
        type: aws_secrets_manager
        config:
          region: us-east-1
```

### Secrets Integration

**Secret-enabled inputs** (`spec.inputs`):
```yaml
- name: database_password
  type: password
  description: Database password
  secret_manager:
    enabled: true
```

**Environment file** (`.pmp.environment.yaml`):
```yaml
spec:
  secrets:
    database_password:
      manager: production-vault
      secret_id: secret/data/myapp/db
      data_source_name: secret_database_password
      secret_key: value  # Optional, for JSON secrets
```

**Generated Terraform** (`_common.tf`):
- Vault provider with remote state reference (for project-based config)
- Data sources: `vault_generic_secret`, `aws_secretsmanager_secret_version`
- Locals: `local.secret_<input_name>` for easy access

**Template helper**: `{{secret input_name}}` outputs `local.secret_<input_name>`

### Environment Time Limits

Environments can be configured with expiration time limits. When expired, they can be destroyed using `pmp env purge`.

**Time Limit Configuration** (`.pmp.environment.yaml`):
```yaml
metadata:
  name: my-project
  environment_name: dev
  created_at: "2024-12-15T10:30:00Z"  # Auto-set on creation
spec:
  time_limit:
    # Option 1: Fixed expiration date (ISO 8601)
    expires_at: "2025-03-01T00:00:00Z"
    # OR Option 2: TTL from creation
    ttl: "7d"  # Supports: s, m, h, d, w (e.g., "1d12h", "2w")
```

**Commands**:
- `pmp env purge` - Dry-run: show expired environments
- `pmp env purge --force` - Destroy expired environments
- `pmp env purge --environment dev` - Filter by environment
- `pmp env purge --force --yes` - Skip confirmation

### Dependencies

**Template Dependencies** (`spec.dependencies`):
```yaml
spec:
  dependencies:
    - dependency_name: optional_name  # Optional unique identifier
      project:
        apiVersion: pmp.io/v1
        kind: ResourceKind
        label_selector: {}  # Optional
        description: ""     # Optional
        remote_state:
          data_source_name: datasource_name
```

**Named Dependencies**:
- Reference dependencies by name in project groups
- Falls back to matching by apiVersion/kind if name not specified

### Dependency-Only Projects

**None Executor**:
- Special executor for grouping projects without managing infrastructure
- All operations are no-ops (always succeed)
- Projects skipped during dependency graph execution
- Use cases: environment-wide deployments, feature bundles, staged rollouts

**Pre-defined Project Groups** (`spec.projects`):
```yaml
spec:
  projects:
    list:
      - name: project-name
        template_pack: pack-name
        template: template-name
        inputs:
          input_name:
            value: value
        reference_projects:
          - name: ref-project
            dependency_name: optional_dep_name
    shared_config:
      use_all_defaults: true
      executor:
        config: {}
```

### Project Creation Workflow

1. Discover template packs
2. Filter by allowed resource kinds
3. User selects template pack and template (auto-select if only one)
4. User selects environment
5. User provides name, description, inputs
6. Render template files
7. Auto-generate `.pmp.project.yaml`, `.pmp.environment.yaml`, `_common.tf` (if executor config present)

### Project Naming Rules

- Allowed: lowercase letters (a-z), numbers (0-9), hyphens (-)
- Cannot start/end with: number or hyphen
- Must be unique
- Variables: `_name`, `_project_name_underscores`, `_project_name_hyphens`

## Key Commands

### Graph (`src/commands/graph.rs`)
- Visualize dependency graphs
- Formats: ASCII (default), Mermaid, DOT
- Usage: `pmp graph [--all] [--format FORMAT] [--output FILE]`

### Deps (`src/commands/deps.rs`)
- `pmp deps analyze` - Dependency analysis with health checks, statistics, bottlenecks
- `pmp deps impact <project>` - Show projects affected by changes

### State (`src/commands/state.rs`)
- `pmp state list` - Show state across all projects
- `pmp state drift` - Detect configuration drift
- `pmp state lock/unlock` - Lock/unlock project state
- `pmp state sync` - Sync remote state

### CI (`src/commands/ci.rs`)
- Generate CI/CD pipelines: GitHub Actions, GitLab CI, Jenkins
- Dependency-aware with topological sorting into stages
- Usage: `pmp ci generate <type> [--output FILE] [--environment ENV]`

### Template (`src/commands/template.rs`)
- Create new template packs interactively
- Usage: `pmp template scaffold [--output DIR]`

### Clone (`src/commands/clone.rs`)
- Clone existing projects with new names
- Usage: `pmp clone [SOURCE] <NAME> [--environment ENV]`

### Env (`src/commands/env.rs`)
- `pmp env diff <source> <target>` - Compare environments
- `pmp env promote <source> <target>` - Promote configs (with backup)
- `pmp env sync` - Find common settings
- `pmp env variables` - View environment variables
- `pmp env purge` - Destroy all expired environments (dry-run by default, use `--force` to execute)

## Implementation Details

### Discovery (`src/template/discovery.rs`, `src/collection/discovery.rs`)

**Template Packs**:
- Max depth: 2 levels from base directories
- Validates apiVersion and kind
- Auto-selects if only one available

**Projects**:
- Scans `projects/` recursively (no depth limit)
- Looks for `.pmp.project.yaml` files
- Extracts resource kind from first `.pmp.environment.yaml`

### Commands (`src/commands/`)

**Preview/Apply** (`preview.rs`, `apply.rs`):
- Smart context detection: environment dir → project dir → collection
- Executes in environment directory

**Execution Helper** (`execution_helper.rs`):
- Handles dependency graph execution
- Skips projects with `none` executor
- Runs hooks (pre/post) except for `none` executor

### Executors (`src/executor/`)

**OpenTofu** (`opentofu.rs`):
- Supports all backend types
- Generates `_common.tf` with backend config
- Runs tofu commands (init, plan, apply, destroy, refresh)

**None** (`none.rs`):
- All operations are no-ops
- Used for dependency-only projects
- No backend support

**Registry** (`registry.rs`):
- Extensible executor system
- Currently supports: opentofu, none

## Testing Considerations

- Template pack files: `.pmp.template-pack.yaml`
- Template files: `.pmp.template.yaml` (in `templates/` subdirectory)
- Plugin files: `.pmp.plugin.yaml` (in `plugins/` subdirectory)
- Resource kinds: alphanumeric only
- Environment names: lowercase alphanumeric + underscores (cannot start with number)
- Project discovery: works at any depth
- Auto-selection: when only one template pack or template available
