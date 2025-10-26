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

### Template Pack System

**Template Pack File**: `.pmp.template-pack.yaml` (REQUIRED)
- Must have `apiVersion: pmp.io/v1` and `kind: TemplatePack`
- Contains metadata (name, description)
- Has an empty spec: `spec: {}`
- Located in: `~/.pmp/templates/<pack-name>/`, `.pmp/templates/<pack-name>/`, or custom paths

**Template File**: `.pmp.template.yaml` (inside `templates/` subdirectory)
- Must have `apiVersion: pmp.io/v1` and `kind: Template`
- Contains metadata (name, description) plus spec with resource definition
- Spec contains: `apiVersion`, `kind`, `executor`, `inputs`, `environments`
- Resource kind must be alphanumeric only
- Located in: `<pack-dir>/templates/<template-name>/.pmp.template.yaml`

**Plugin File**: `.pmp.plugin.yaml` (inside `templates/` subdirectory - OPTIONAL)
- Must have `apiVersion: pmp.io/v1` and `kind: Plugin`
- Contains metadata (name, description) plus spec with `role` and `inputs`
- Plugins are reusable components that can be referenced by templates
- Located in: `<pack-dir>/templates/<plugin-name>/.pmp.plugin.yaml`

**Template Pack Structure**:
```
~/.pmp/templates/<pack-name>/
├── .pmp.template-pack.yaml (kind: TemplatePack)
└── templates/
    ├── <template-name>/
    │   ├── .pmp.template.yaml (kind: Template)
    │   ├── main.tf.hbs
    │   └── ... (other template files)
    └── <plugin-name>/
        ├── .pmp.plugin.yaml (kind: Plugin)
        └── ... (plugin files)
```

**Note**: `.pmp.yaml` or `.pmp.yaml.hbs` files are auto-generated and must NOT be included in templates

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

**Executor Configuration** (Optional):
- `spec.executor` - Collection-level executor configuration
- Applies to all projects in the collection (unless overridden at project level)
- For OpenTofu executor, generates `_common.tf` file with backend configuration
- Structure:
  ```yaml
  spec:
    executor:
      name: opentofu
      config:
        backend:
          type: s3  # or azurerm, gcs, http, kubernetes, pg, consul, etc.
          # ... backend-specific parameters
  ```

**Backend Configuration Examples**:

1. **S3 Backend** (AWS):
   ```yaml
   spec:
     executor:
       name: opentofu
       config:
         backend:
           type: s3
           bucket: my-terraform-state
           key: project/terraform.tfstate
           region: us-west-2
           encrypt: true
           dynamodb_table: terraform-locks
   ```

2. **AzureRM Backend** (Azure):
   ```yaml
   spec:
     executor:
       name: opentofu
       config:
         backend:
           type: azurerm
           storage_account_name: mystorageaccount
           container_name: tfstate
           key: prod.terraform.tfstate
           resource_group_name: my-resource-group
   ```

3. **GCS Backend** (Google Cloud):
   ```yaml
   spec:
     executor:
       name: opentofu
       config:
         backend:
           type: gcs
           bucket: my-terraform-state
           prefix: terraform/state
   ```

4. **Kubernetes Backend**:
   ```yaml
   spec:
     executor:
       name: opentofu
       config:
         backend:
           type: kubernetes
           secret_suffix: state
           namespace: terraform
   ```

**Supported Backend Types**:
- `local` - Local file system (default)
- `s3` - AWS S3 with DynamoDB locking
- `azurerm` - Azure Blob Storage
- `gcs` - Google Cloud Storage
- `http` - Generic HTTP backend
- `kubernetes` - Kubernetes secrets
- `pg` - PostgreSQL
- `consul` - HashiCorp Consul
- `cos` - Tencent Cloud Object Storage
- `oss` - Alibaba Cloud OSS
- `remote` - Terraform Cloud/Enterprise

### Project Creation Workflow

1. Discover template packs (`.pmp.template-pack.yaml` files)
2. Filter template packs by checking their templates against collection's allowed resource kinds
3. User selects template pack (auto-selected if only one available)
4. Discover templates within selected pack
5. User selects template (auto-selected if only one available)
6. Plugins are NOT shown to users - they are reusable components only
7. User selects environment (from the environments defined in the project collection)
8. User provides name, optional description, and inputs defined in the template
9. Render template files from template directory into environment folder
10. **Auto-generate `_common.tf`** (if executor config with backend is present in collection)
11. **Auto-generate `.pmp.project.yaml`** at project root (identifier only - metadata, no spec)
12. **Auto-generate `.pmp.environment.yaml`** in environment folder (with full spec)
13. Generate project structure:
    - Project root: `projects/{resource_kind_snake}/{project_name}/`
    - Project identifier: `projects/{resource_kind_snake}/{project_name}/.pmp.project.yaml`
    - Environment: `projects/{resource_kind_snake}/{project_name}/environments/{environment_name}/`
    - Environment spec: `projects/{resource_kind_snake}/{project_name}/environments/{environment_name}/.pmp.environment.yaml`
    - Backend config: `projects/{resource_kind_snake}/{project_name}/environments/{environment_name}/_common.tf` (if executor config present)
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

- **Template Packs**: `.pmp.template-pack.yaml`
- **Templates**: `.pmp.template.yaml` (inside `templates/` subdirectory of pack)
- **Plugins**: `.pmp.plugin.yaml` (inside `templates/` subdirectory of pack)
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

### Template Pack Discovery (`src/template/discovery.rs`)
- Looks for `.pmp.template-pack.yaml` files
- Max depth: 2 levels from template base directories
- Validates `apiVersion` and `kind` on load
- Discovers templates within pack by scanning `templates/` subdirectory for `.pmp.template.yaml` files
- Discovers plugins within pack by scanning `templates/` subdirectory for `.pmp.plugin.yaml` files
- Template packs are selected first, then templates within the pack are shown to the user
- If only one template in a pack, it's auto-selected

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

- Template pack files must be `.pmp.template-pack.yaml`
- Template files must be `.pmp.template.yaml` (inside `templates/` subdirectory)
- Plugin files must be `.pmp.plugin.yaml` (inside `templates/` subdirectory)
- Resource kinds must be alphanumeric
- Environment names must be lowercase alphanumeric + optional hyphens
- Project discovery must work at any depth
- Multiple projects per resource kind supported
- Multiple environments per project supported
- Auto-selection works when only one template pack or template is available
