# Environments

PMP supports multiple environments (dev, staging, prod) with tools for comparison, promotion, and synchronization.

## Defining Environments

Environments are defined in the infrastructure configuration:

```yaml
# .pmp.infrastructure.yaml
spec:
  environments:
    dev:
      name: Development
    staging:
      name: Staging
    prod:
      name: Production
```

### Naming Rules

- Lowercase letters (a-z)
- Numbers (0-9)
- Underscores (_)
- Cannot start with a number

**Valid**: `dev`, `staging`, `prod`, `us_west_2`, `feature_123`
**Invalid**: `Dev`, `prod-us`, `1_dev`

## Environment-Specific Overrides

Templates can define different defaults per environment:

```yaml
# .pmp.template.yaml
spec:
  inputs:
    - name: replicas
      default: 1

  environments:
    prod:
      overrides:
        inputs:
          - name: replicas
            default: 5

    staging:
      overrides:
        inputs:
          - name: replicas
            default: 3
```

## Commands

### Compare Environments

```bash
pmp project env diff dev staging
```

**Output:**
```
Environment Comparison: dev → staging
=====================================

Project: my-api

  replicas:
    dev:     1
    staging: 3

  namespace:
    dev:     my-api-dev
    staging: my-api-staging

Project: postgres-db

  instance_class:
    dev:     db.t3.micro
    staging: db.t3.medium
```

### Promote Configuration

Copy configuration from one environment to another:

```bash
# Promote all projects
pmp project env promote dev staging

# Promote specific project
pmp project env promote dev staging --project my-api
```

**Process:**
1. Creates backup of target environment
2. Copies input values
3. Updates environment-specific values (namespace, etc.)
4. Does NOT copy state or backend configuration

**Safety:**
- Requires confirmation
- Creates automatic backup
- Does not affect infrastructure until `apply`

### Synchronize Settings

Find common settings across environments:

```bash
pmp project env sync

# For specific project
pmp project env sync --project my-api
```

**Output:**
```
Common Settings Across Environments
===================================

Project: my-api

  Identical in all environments:
    - image: myregistry/my-api:latest
    - port: 8080
    - health_check_path: /health

  Different:
    - replicas: dev=1, staging=3, prod=5
    - namespace: varies by environment
```

### View Variables

Display environment variables across projects:

```bash
# All environments
pmp project env variables

# Specific environment
pmp project env variables --environment prod

# Specific project
pmp project env variables --project my-api
```

**Output:**
```
Environment Variables
=====================

Environment: prod

  my-api:
    replicas: 5
    namespace: my-api-prod
    image: myregistry/my-api:v1.2.3

  postgres-db:
    instance_class: db.t3.large
    storage: 100
```

## Directory Structure

Each project has an `environments` directory with subdirectories per environment:

```
projects/my-api/
├── .pmp.project.yaml
└── environments/
    ├── dev/
    │   ├── .pmp.environment.yaml
    │   ├── _common.tf
    │   └── main.tf
    ├── staging/
    │   ├── .pmp.environment.yaml
    │   ├── _common.tf
    │   └── main.tf
    └── prod/
        ├── .pmp.environment.yaml
        ├── _common.tf
        └── main.tf
```

## Environment Selection

### Interactive

When running commands from project root, PMP prompts for environment:

```bash
cd projects/my-api
pmp project apply

# Output:
# Select an environment:
# > dev
#   staging
#   prod
```

### Command Line

Specify environment during project creation:

```bash
pmp project create --name my-api --environment dev
```

### Clone to Environment

Clone a project to a new environment:

```bash
pmp project clone my-api my-api-staging --environment staging
```

## Environment Variables in Templates

Use `_environment` variable in templates:

```handlebars
locals {
  env_prefix = "{{_environment}}"
  full_name  = "{{_name}}-{{_environment}}"
}

resource "kubernetes_namespace" "app" {
  metadata {
    name = "{{_name}}-{{_environment}}"
  }
}
```

## Best Practices

1. **Keep environments similar** - Minimize differences between dev/staging/prod
2. **Use environment overrides** - Define sensible defaults per environment
3. **Promote regularly** - Keep environments in sync
4. **Review before apply** - Always preview changes after promotion
5. **Version control** - Commit all environment configurations
