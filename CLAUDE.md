# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

PMP (Poor Man's Platform) is a CLI for managing Infrastructure as Code projects using OpenTofu/Terraform with template-based project generation. It uses Kubernetes-style resource definitions (apiVersion, kind, metadata, spec) for all configuration files.

## Key Design Principles

1. **Templates use `.pmp.template.yaml`** - Template metadata must be in a file named `.pmp.template.yaml` with `apiVersion: pmp.io/v1` and `kind: Template`
2. **Projects use `.pmp.yaml`** - Generated project metadata is in `.pmp.yaml`
3. **Categories define resource kinds** - Categories (in ProjectCollection) specify which resource kinds they allow
4. **Templates have NO categories** - Templates only define what they generate, not where they belong
5. **Category-first workflow** - User selects category first, then templates are filtered by category's resource kinds

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
- Resource kind must be alphanumeric only
- NO categories field (categories are in ProjectCollection)
- Located in: `~/.pmp/templates/<name>/`, `.pmp/templates/<name>/`, or custom paths

### ProjectCollection System

**Required**: Yes - cannot create projects without one
**File**: `.pmp.project-collection.yaml`

**Must define**:
- `spec.environments` - Available environments
- `spec.categories` - Categories with `resource_kinds` arrays

**Category Structure**:
```yaml
categories:
  workload:
    name: "Workloads"
    resource_kinds:              # Defines which templates can be used
      - apiVersion: pmp.io/v1
        kind: KubernetesWorkload
    children:                    # Optional nested categories
      critical:
        resource_kinds: [...]
```

### Project Creation Workflow

1. Select category from ProjectCollection
2. Get resource kinds from category (including parent categories)
3. Discover templates (`.pmp.template.yaml` files)
4. Filter templates by category's resource kinds
5. User selects template, environment, provides name and inputs
6. Generate project in `projects/{resource_kind_snake}/{project_name}/`

### Project Discovery

- Scans `projects/` directory **recursively at ALL levels**
- Looks for `.pmp.yaml` files (NOT `.pmp.template.yaml`)
- No depth limit - finds projects anywhere under `projects/`

### Find Command

**Only supports**:
- Search by name (substring match)
- Search by category
- Show all projects

**Removed**: Search by kind, search by tags

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
- **No depth limit** - scans all subdirectories
- Extracts category from path (first level after `projects/`)

### Create Command (`src/commands/create.rs`)
- Requires ProjectCollection with categories
- Category selection happens FIRST
- Templates filtered by `get_category_resource_kinds()`
- Resource kind validated (alphanumeric only)
- Converted to snake_case for directory names

### Find Command (`src/commands/find.rs`)
- Only: name, category, or show all
- Removed: kind and search_categories options

## Testing Considerations

- Template files must be `.pmp.template.yaml`
- Resource kinds must be alphanumeric
- Project discovery must work at any depth
- Category resource kinds filter templates correctly
- Multiple projects per resource kind supported
