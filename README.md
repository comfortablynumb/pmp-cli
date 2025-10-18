# PMP - Poor Man's Platform

A CLI for managing Infrastructure as Code projects using OpenTofu/Terraform with template-based project generation.

## Features

- **Template-based project creation** with JSON Schema validation
- **Multiple IaC executors** via trait-based architecture (OpenTofu included)
- **Pre/post execution hooks** for custom workflows
- **Custom command overrides** per project
- **Category-based template organization**
- **Interactive CLI** with intuitive prompts

## Installation

```bash
cargo install --path .
```

Or build from source:

```bash
cargo build --release
```

## Quick Start

### 1. Create a Template

Templates are stored in `~/.pmp/templates` or `.pmp/templates` in your current directory.

Example template structure:

```
~/.pmp/templates/workload/
├── .pmp.yaml           # Template metadata
├── schema.json         # JSON Schema for input validation
└── src/                # Template files (*.hbs files)
    ├── main.tf.hbs
    ├── variables.tf.hbs
    └── .pmp.yaml.hbs
```

#### Template `.pmp.yaml` (Kubernetes-style Resource)

PMP uses a Kubernetes-style resource format for configuration files:

```yaml
apiVersion: pmp.io/v1
kind: Template
metadata:
  name: "EKS Workload"
  description: "Creates a Kubernetes workload on EKS"
spec:
  categories:
    - workload
    - kubernetes

  # Defines the resource kind that will be generated
  resource:
    apiVersion: pmp.io/v1
    kind: Workload

  # Optional: schema and source paths
  schema_path: schema.json  # Defaults to "schema.json"
  src_path: src             # Defaults to "src"

  # Optional: Environment-specific configurations
  environments:
    development:
      description: "Development environment"
      overrides:
        replicas:
          default: 1
        instance_type:
          default: "t3.micro"
          enum_values: ["t3.micro", "t3.small"]

    production:
      description: "Production environment"
      overrides:
        replicas:
          default: 3
        instance_type:
          default: "t3.large"
          enum_values: ["t3.large", "t3.xlarge"]
```

**Key Fields:**
- `apiVersion`: Namespace for resource kinds (e.g., `pmp.io/v1`)
- `kind`: Type of resource (always `Template` for templates)
- `metadata`: Template name and description
- `spec.resource`: Defines the generated project's `apiVersion` and `kind`
- `spec.categories`: Categories for template discovery
- `spec.environments`: Environment-specific overrides (optional)

#### `schema.json`

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "name": {
      "type": "string",
      "description": "Name of the workload"
    },
    "namespace": {
      "type": "string",
      "description": "Kubernetes namespace",
      "default": "default"
    },
    "replicas": {
      "type": "integer",
      "description": "Number of replicas",
      "default": 1
    },
    "environment": {
      "type": "string",
      "description": "Environment",
      "enum": ["dev", "staging", "prod"]
    }
  },
  "required": ["name", "environment"]
}
```

#### Template Files (`src/*.hbs`)

```hbs
# main.tf.hbs
resource "kubernetes_deployment" "{{name}}" {
  metadata {
    name      = "{{name}}"
    namespace = "{{namespace}}"
  }

  spec {
    replicas = {{replicas}}

    selector {
      match_labels = {
        app = "{{name}}"
      }
    }

    template {
      metadata {
        labels = {
          app = "{{name}}"
          env = "{{environment}}"
        }
      }
    }
  }
}
```

### 2. Create a Project

```bash
pmp create
```

This will:
1. Show available categories
2. Show templates in the selected category
3. Prompt for environment selection (if template supports multiple environments)
4. Prompt for inputs based on the template's JSON Schema (with environment-specific overrides applied)
5. Validate inputs
6. Render the template files to the output directory

You can specify an output directory:

```bash
pmp create --output ./my-project
```

You can also use custom template directories:

```bash
pmp create --templates-path /path/to/custom/templates
```

**Note:** Input prompts will display only the field description (from the JSON Schema) to keep the interface clean and user-friendly.

### 3. Preview Changes

```bash
pmp preview
```

Or for a specific project:

```bash
pmp preview --path ./my-project
```

### 4. Apply Changes

```bash
pmp apply
```

Or for a specific project:

```bash
pmp apply --path ./my-project
```

## Environment-Specific Configurations

Templates can define environment-specific configurations that override default values in the JSON Schema. This is useful for having different settings for development, staging, and production environments.

### How It Works

1. **Define environments in template's `.pmp.yaml`:**

```yaml
environments:
  development:
    description: "Development environment with minimal resources"
    overrides:
      instance_type:
        default: "t3.micro"
        enum_values: ["t3.micro", "t3.small"]
        description: "EC2 instance type (development options)"
      replicas:
        default: 1
      enable_monitoring:
        default: false

  production:
    description: "Production environment with high availability"
    overrides:
      instance_type:
        default: "t3.large"
        enum_values: ["t3.large", "t3.xlarge", "t3.2xlarge"]
        description: "EC2 instance type (production options)"
      replicas:
        default: 3
      enable_monitoring:
        default: true
```

2. **During `pmp create`, users select an environment** and the overrides are automatically applied to the schema
3. **The selected environment name is available** as `{{environment}}` in your templates

### Override Options

Each property can override:
- `default`: Override the default value
- `enum_values`: Override enum options (for string enums)
- `description`: Override the field description

## Project Configuration

Each generated project has a `.pmp.yaml` file in its root:

```yaml
apiVersion: pmp.io/v1
kind: Workload  # Defined by template's spec.resource.kind
metadata:
  name: "my-api"
  description: "This is an API that allows you to manage users"
spec:
  # Optional: IaC executor configuration
  iac:
    executor: opentofu  # Default: opentofu

    # Optional: Override default commands
    commands:
      plan: "tofu plan -out=tfplan"
      apply: "tofu apply tfplan"

  # Optional: Hooks
  hooks:
    pre_preview:
      - "echo 'Running pre-preview checks'"
      - "./scripts/validate-env.sh"
    post_preview:
      - "echo 'Preview completed'"
    pre_apply:
      - "./scripts/notify-slack.sh 'Starting deployment...'"
    post_apply:
      - "./scripts/notify-slack.sh 'Deployment complete!'"
      - "./scripts/run-tests.sh"
```

**Generated Project Structure:**
- `apiVersion` and `kind`: Automatically populated from template's `spec.resource` definition
- `metadata.name`: Typically comes from template inputs
- `spec.iac`: IaC executor configuration (optional)
- `spec.hooks`: Pre/post execution hooks (optional)

## Architecture

### IaC Executor Trait

The `IacExecutor` trait allows for multiple IaC tool implementations:

```rust
pub trait IacExecutor {
    fn check_installed(&self) -> Result<bool>;
    fn plan(&self, config: &IacConfig, working_dir: &str) -> Result<Output>;
    fn apply(&self, config: &IacConfig, working_dir: &str) -> Result<Output>;
    fn get_name(&self) -> &str;
    fn default_plan_command(&self) -> &str;
    fn default_apply_command(&self) -> &str;
}
```

Currently implemented:
- **OpenTofu** (`opentofu`)

To add support for another IaC tool, implement the `IacExecutor` trait and register it in the commands.

### Template Discovery

Templates are discovered in this order:
1. `.pmp/templates` in the current directory
2. `~/.pmp/templates` in the user's home directory
3. Custom paths specified via `--templates-path` flag

You can use the `--templates-path` flag to add additional template directories:

```bash
pmp create --templates-path /company/shared/templates
pmp create --templates-path ~/my-custom-templates
```

### Hooks System

Hooks are shell commands executed at specific points:
- `pre_preview`: Before running plan
- `post_preview`: After running plan
- `pre_apply`: Before running apply
- `post_apply`: After running apply

Hooks run in sequence and must succeed for the command to continue.

## Example Templates

### Basic Workload Template

```
~/.pmp/templates/basic-workload/
├── .pmp.yaml
├── schema.json
└── src/
    ├── main.tf.hbs
    ├── variables.tf.hbs
    ├── outputs.tf.hbs
    └── .pmp.yaml.hbs
```

### ECR Repository Template

```yaml
# .pmp.yaml
name: "ECR Repository"
description: "Creates an AWS ECR repository"
categories:
  - aws
  - registry
```

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "type": "object",
  "properties": {
    "repository_name": {
      "type": "string",
      "description": "Name of the ECR repository"
    },
    "image_tag_mutability": {
      "type": "string",
      "description": "Image tag mutability",
      "enum": ["MUTABLE", "IMMUTABLE"],
      "default": "MUTABLE"
    }
  },
  "required": ["repository_name"]
}
```

## Development

Build the project:

```bash
cargo build
```

Run tests:

```bash
cargo test
```

Run in development:

```bash
cargo run -- create
cargo run -- preview
cargo run -- apply
```

## Contributing

This is an open-source project. Contributions are welcome!

## License

MIT License - see LICENSE file for details
