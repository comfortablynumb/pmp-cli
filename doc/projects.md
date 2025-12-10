# Projects

Projects are instances of templates deployed to specific environments. Each project generates Terraform/OpenTofu files and manages infrastructure state.

## Project Structure

```
projects/{project-name}/
├── .pmp.project.yaml              # Project identifier
└── environments/
    └── {env-name}/
        ├── .pmp.environment.yaml  # Environment configuration
        ├── _common.tf             # Auto-generated backend config
        ├── main.tf                # Generated from template
        ├── variables.tf           # Generated from template
        └── outputs.tf             # Generated from template
```

## Project File (.pmp.project.yaml)

```yaml
apiVersion: pmp.io/v1
kind: Project
metadata:
  name: my-api
  description: "API service for user management"
spec: {}
```

## Environment File (.pmp.environment.yaml)

```yaml
apiVersion: pmp.io/v1
kind: ProjectEnvironment
metadata:
  name: my-api
  description: "API service for user management"
  environment_name: dev
spec:
  template:
    pack_name: kubernetes
    template_name: api-service
    kind: KubernetesWorkload
    api_version: pmp.io/v1

  inputs:
    replicas: 3
    namespace: my-api-dev
    image: myregistry/my-api:latest

  dependencies:
    - project:
        name: postgres-db
        environments:
          - dev

  executor:
    name: opentofu
    config:
      backend:
        type: s3
        bucket: terraform-state
        key: my-api/dev/terraform.tfstate

  hooks:
    pre_apply:
      - type: confirm
        config:
          question: "Deploy to dev?"
```

## Commands

### Create Project

```bash
# Interactive mode
pmp project create

# With template
pmp project create --template kubernetes/api-service

# Full specification
pmp project create \
  --template kubernetes/api-service \
  --name my-api \
  --environment dev \
  --inputs '{"replicas": 3, "namespace": "my-api"}'

# Create and apply immediately
pmp project create --template kubernetes/api-service --apply
```

### Find Projects

```bash
# List all projects
pmp project find

# Filter by name (case-insensitive substring match)
pmp project find --name api

# Filter by kind
pmp project find --kind KubernetesWorkload

# Combined filters
pmp project find --name api --kind KubernetesWorkload
```

### Clone Project

```bash
# Clone to new name
pmp project clone my-api my-api-v2

# Clone specific environment
pmp project clone my-api my-api-v2 --environment prod
```

### Update Project

Regenerate files from the original template with updated inputs:

```bash
# Update in current directory
pmp project update

# Update specific path
pmp project update --path ./projects/my-api/environments/dev

# Update with new inputs
pmp project update --inputs '{"replicas": 5}'
```

## Operations

### Preview Changes

```bash
# Preview in current environment
pmp project preview

# Preview specific path
pmp project preview --path ./projects/my-api/environments/dev

# Pass arguments to executor
pmp project preview -- -no-color
pmp project preview -- -var="environment=prod"
```

### Apply Changes

```bash
# Apply in current environment
pmp project apply

# With auto-approve
pmp project apply -- -auto-approve

# Pass multiple arguments
pmp project apply -- -var="environment=prod" -parallelism=10
```

### Destroy Infrastructure

```bash
# Interactive (prompts for confirmation)
pmp project destroy

# Skip confirmation
pmp project destroy --yes

# Pass arguments
pmp project destroy -- -auto-approve
```

### Refresh State

Update state file with real infrastructure status:

```bash
pmp project refresh

# With arguments
pmp project refresh -- -var="environment=prod"
```

### Test Configuration

Validate configuration without creating resources:

```bash
pmp project test

# With verbose output
pmp project test -- -verbose
```

## Naming Rules

| Rule | Valid | Invalid |
|------|-------|---------|
| Lowercase only | `my-api` | `My-API` |
| Hyphens allowed | `api-v2` | `api_v2` |
| Numbers allowed (not at start) | `api-v2` | `2-api` |
| No hyphen at start/end | `my-api` | `-my-api-` |

## Context Detection

PMP automatically detects the execution context:

| Location | Behavior |
|----------|----------|
| Environment directory | Execute directly |
| Project directory | Prompt for environment selection |
| Infrastructure root | Prompt for project and environment |

```bash
# From environment directory
cd projects/my-api/environments/dev
pmp project apply  # Applies directly

# From project directory
cd projects/my-api
pmp project apply  # Prompts: Select environment

# From infrastructure root
pmp project apply  # Prompts: Select project, then environment
```

## Dependency Execution

When a project has dependencies, operations cascade through the dependency graph:

```bash
# If my-api depends on postgres-db:
pmp project apply
# 1. Applies postgres-db first
# 2. Then applies my-api
```

See [Dependencies](dependencies.md) for details.
