# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

PMP (Poor Man's Platform) is a CLI for managing Infrastructure as Code projects using OpenTofu/Terraform with template-based project generation. It uses Kubernetes-style resource definitions (apiVersion, kind, metadata, spec) for all configuration files.

## Key Design Principles

1. **Templates use `.pmp.template.yaml`** - Template metadata must be in a file named `.pmp.template.yaml` with `apiVersion: pmp.io/v1` and `kind: Template`
2. **Projects use `.pmp.project.yaml`** - Generated project identifier is in `.pmp.project.yaml` with `apiVersion: pmp.io/v1` and `kind: Project` (metadata only, no spec)
3. **Environments use `.pmp.environment.yaml`** - Each environment has its own file with `apiVersion: pmp.io/v1` and `kind: ProjectEnvironment` containing the spec
4. **A Project has a Resource associated with its own apiVersion and kind** - Resources may define inputs by environment
5. **Projects are organized by environment** - Each project has an `environments/` directory containing one subdirectory per environment

## Development Commands

```bash
cargo build                    # Build the project
cargo build --release          # Build release
cargo test                     # Run tests
cargo run -- create            # Create project
cargo run -- find --name foo   # Find projects
```

## Architecture Overview

### Template System

**Template File**: `.pmp.template.yaml` (REQUIRED)
- Must have `apiVersion: pmp.io/v1` and `kind: Template`
- Must define `spec.resource.apiVersion` and `spec.resource.kind`
- Project name must contain only alphanumeric chars or hyphens 
- Resource kind must be alphanumeric only
- Located in: `~/.pmp/templates/<name>/`, `.pmp/templates/<name>/`, or custom paths
- **Must NOT include `.pmp.yaml` or `.pmp.yaml.hbs`** - This file is auto-generated

### ProjectCollection System

**Required**: Yes - cannot create projects without one
**File**: `.pmp.project-collection.yaml`

**Must define**:
- `spec.environments` - Available environments (keys must be lowercase alphanumeric + optional hyphens)
- `spec.resource_kinds` - Available resource kinds, which limits which templates can be used

**Environment Naming Rules**:
- Environment names (keys in `spec.environments`) MUST be lowercase alphanumeric
- May contain hyphens (-)
- No uppercase letters, underscores, or special characters allowed

### Project Creation Workflow

1. Discover templates (`.pmp.template.yaml` files)
2. Filter templates by collection's allowed resource kinds
3. User selects template
4. User selects environment (from the environments defined in the project collection)
5. User provides name, optional description, and inputs defined in the template
6. **Auto-generate `.pmp.project.yaml`** at project root (identifier only - metadata, no spec)
7. **Auto-generate `.pmp.environment.yaml`** in environment folder (with full spec)
8. Render template files from `src/` directory into environment folder
9. Generate project structure:
   - Project root: `projects/{resource_kind_snake}/{project_name}/`
   - Project identifier: `projects/{resource_kind_snake}/{project_name}/.pmp.project.yaml`
   - Environment: `projects/{resource_kind_snake}/{project_name}/environments/{environment_name}/`
   - Environment spec: `projects/{resource_kind_snake}/{project_name}/environments/{environment_name}/.pmp.environment.yaml`
   - Generated files: `projects/{resource_kind_snake}/{project_name}/environments/{environment_name}/...`

### Project Discovery

- Scans `projects/` directory **recursively at ALL levels**
- Looks for `.pmp.project.yaml` files (identifier files)
- No depth limit - finds projects anywhere under `projects/`
- Reads resource kind from first `.pmp.environment.yaml` found

### Find Command

**Supports**:
- Search by name (substring match)
- Search by kind
- Show all projects (no filter)
- **After selecting project**: prompts user to select environment
- **Displays**: project metadata + selected environment details with spec

### File Naming

- **Templates**: `.pmp.template.yaml`
- **Project Identifiers**: `.pmp.project.yaml` (metadata only, no spec)
- **Environment Specs**: `.pmp.environment.yaml` (contains full spec)
- **Collections**: `.pmp.project-collection.yaml`

### Directory Structure

```
collection/
├── .pmp.project-collection.yaml
└── projects/
    └── {resource_kind_snake}/
        └── {project_name}/
            ├── .pmp.project.yaml (identifier only)
            └── environments/
                └── {environment_name}/
                    ├── .pmp.environment.yaml (full spec)
                    └── ... (generated terraform/tofu files)
```

Example: `KubernetesWorkload` → `projects/kubernetes_workload/my-api/.pmp.project.yaml`
Environment: `projects/kubernetes_workload/my-api/environments/dev/.pmp.environment.yaml`

## Important Implementation Details

### Template Discovery (`src/template/discovery.rs`)
- Looks for `.pmp.template.yaml` files
- Max depth: 2 levels from template base directories
- Validates `apiVersion` and `kind` on load

### Project Discovery (`src/collection/discovery.rs`)
- Looks for `.pmp.project.yaml` files
- **No depth limit** - scans all subdirectories under `projects/`
- Extracts resource kind from first `.pmp.environment.yaml` found in `environments/` subdirectory
- Discovers environments by scanning `environments/` subdirectories for `.pmp.environment.yaml` files

### Preview/Apply Commands (`src/commands/preview.rs`, `src/commands/apply.rs`)
- **Smart context detection**:
  1. If in environment directory (has `.pmp.environment.yaml`): execute directly
  2. If in project directory (has `.pmp.project.yaml`): prompt to select environment
  3. If in collection or elsewhere: use find/search UI to select project + environment
- Executes in the environment directory (where generated files are located)

## Testing Considerations

- Template files must be `.pmp.template.yaml`
- Resource kinds must be alphanumeric
- Environment names must be lowercase alphanumeric + optional hyphens
- Project discovery must work at any depth
- Multiple projects per resource kind supported
- Multiple environments per project supported
