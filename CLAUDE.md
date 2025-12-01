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
cargo run -- clone source target         # Clone existing project
cargo run -- env diff dev staging        # Compare environments
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
```

**Supported Backends**: local, s3, azurerm, gcs, http, kubernetes, pg, consul, cos, oss, remote

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
