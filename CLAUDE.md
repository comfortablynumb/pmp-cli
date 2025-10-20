# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

PMP (Poor Man's Platform) is a CLI for managing Infrastructure as Code projects using OpenTofu/Terraform with template-based project generation. It uses Kubernetes-style resource definitions (apiVersion, kind, metadata, spec) for all configuration files.

## Key Design Principles

1. **Templates use `.pmp.template.yaml`** - Template metadata must be in a file named `.pmp.template.yaml` with `apiVersion: pmp.io/v1` and `kind: Template`
2. **Projects use `.pmp.yaml`** - Generated project metadata is in `.pmp.yaml` with `apiVersion: pmp.io/v1` and `kind: Project` - Projects are generated from Templates
3. **A Project has a Resource associated with its own apiVersion and kind** - Resources may define inputs by environment
4. **A Project has inputs that include both template-specific inputs and shared inputs** - Example: name, description, and custom template inputs

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
- `spec.environments` - Available environments
- `spec.resource_kinds` - Available resource kinds, which limits which templates can be used

### Project Creation Workflow

1. Discover templates (`.pmp.template.yaml` files)
2. Filter templates by collection's allowed resource kinds
3. User selects template
4. User selects environment (from the environments defined in the project collection)
5. User provides name, optional description, and inputs defined in the template
6. Render template files from `src/` directory
7. **Auto-generate `.pmp.yaml`** with template metadata + user inputs
8. Generate project in `projects/{resource_kind_snake}/{project_name}/`

### Project Discovery

- Scans `projects/` directory **recursively at ALL levels**
- Looks for `.pmp.yaml` files
- No depth limit - finds projects anywhere under `projects/`

### Find Command

**Supports**:
- Search by name (substring match)
- Search by kind
- Show all projects (no filter)

### File Naming

- **Templates**: `.pmp.template.yaml`
- **Projects**: `.pmp.yaml`  
- **Collections**: `.pmp.project-collection.yaml`

### Directory Structure

```
collection/
├── .pmp.project-collection.yaml
└── projects/
    └── {resource_kind_snake}/
        └── {project_name}/
            └── .pmp.yaml
```

Example: `KubernetesWorkload` → `projects/kubernetes_workload/my-api/.pmp.yaml`

## Important Implementation Details

### Template Discovery (`src/template/discovery.rs`)
- Looks for `.pmp.template.yaml` files
- Max depth: 2 levels from template base directories
- Validates `apiVersion` and `kind` on load

### Project Discovery (`src/collection/discovery.rs`)
- Looks for `.pmp.yaml` files
- **No depth limit** - scans all subdirectories under `projects/`
- Extracts resource kind from `.pmp.yaml` file content

## Testing Considerations

- Template files must be `.pmp.template.yaml`
- Resource kinds must be alphanumeric
- Project discovery must work at any depth
- Multiple projects per resource kind supported
