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

#### Template `.pmp.yaml`

```yaml
name: "EKS Workload"
description: "Creates a Kubernetes workload on EKS"
categories:
  - workload
  - kubernetes
schema_path: schema.json  # Optional, defaults to "schema.json"
src_path: src             # Optional, defaults to "src"
```

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
3. Prompt for inputs based on the template's JSON Schema
4. Validate inputs
5. Render the template files to the output directory

You can specify an output directory:

```bash
pmp create --output ./my-project
```

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

## Project Configuration

Each generated project has a `.pmp.yaml` file in its root:

```yaml
resource_type: workload
description: This is an API that allows you to manage users

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
