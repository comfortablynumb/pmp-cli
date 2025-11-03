# PMP - Poor Man's Platform

A CLI for managing Infrastructure as Code projects using OpenTofu/Terraform with template-based project generation.

## Features

- **Infrastructure-based organization** - All projects live in a collection with defined environments and resource kinds
- **Template-based project creation** with custom input definitions (no JSON Schema)
- **Multiple IaC executors** via trait-based architecture (OpenTofu included)
- **Pre/post execution hooks** for custom workflows
- **Custom command overrides** per project
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

### 1. Create a Infrastructure

A Infrastructure is **required** before creating any projects. It defines the workspace for your infrastructure projects.

Create a `.pmp.infrastructure.yaml` file:

```yaml
apiVersion: pmp.io/v1
kind: Infrastructure
metadata:
  name: "My Infrastructure"
  description: "Company infrastructure projects"
spec:
  # Define available environments
  environments:
    dev:
      name: "Development"
      description: "Development environment"
    prod:
      name: "Production"
      description: "Production environment"

  # Define allowed resource kinds
  resource_kinds:
    - apiVersion: pmp.io/v1
      kind: KubernetesWorkload
    - apiVersion: pmp.io/v1
      kind: Infrastructure
```

### 2. Create a Template

Templates use `.pmp.template.yaml` in their root directory:

```
~/.pmp/templates/kubernetes-workload/
├── .pmp.template.yaml
└── src/
    └── deployment.yaml.hbs
```

**Note**: `.pmp.yaml` is auto-generated and should NOT be in templates.

See full documentation below for template examples.

### 3. Create a Project

```bash
cd /path/to/my-collection
pmp create
```

### 4. Find Projects

```bash
pmp find --name my-api
pmp find --kind KubernetesWorkload
```

For full documentation, see the complete README sections below.

## Template Structure

Templates must have a `.pmp.template.yaml` file with `apiVersion: pmp.io/v1` and `kind: Template`.

Example `.pmp.template.yaml`:

```yaml
apiVersion: pmp.io/v1
kind: Template
metadata:
  name: "Kubernetes Workload"
  description: "Creates a Kubernetes deployment"

spec:
  # REQUIRED: Resource kind this template generates
  resource:
    apiVersion: pmp.io/v1
    kind: KubernetesWorkload  # Must be alphanumeric only

  # Input definitions (no JSON Schema)
  inputs:
    - name: namespace
      type: text
      label: "Kubernetes namespace"
      required: true
      validation:
        pattern: "^[a-z0-9-]+$"

    - name: replicas
      type: select
      label: "Number of replicas"
      options:
        - value: "1"
          label: "1 replica"
        - value: "3"
          label: "3 replicas (HA)"

    - name: enable_monitoring
      type: boolean
      label: "Enable monitoring"
      default: true

  src_path: src
```

**Input Types**: text, password, boolean, select, multiselect

**Template Variables**: `{{name}}`, `{{environment}}`, `{{resource_kind}}`, plus all custom inputs

**Handlebars Helpers**: `{{#if (eq var "value")}}`, `{{#if (contains array "value")}}`

## Project Structure

Projects are organized as: `projects/{resource-kind}/{project-name}/`

Example:
```
projects/
└── kubernetes_workload/
    ├── api-service/
    │   ├── .pmp.yaml          # Auto-generated from template + inputs
    │   └── deployment.yaml
    └── worker/
        └── .pmp.yaml          # Auto-generated from template + inputs
```

**Important**: The `.pmp.yaml` file is automatically generated when you create a project. It contains:
- Template resource definition (apiVersion, kind)
- All user inputs collected during creation
- Custom fields from the template

## Commands

- `pmp create` - Create new project (requires Infrastructure)
- `pmp preview` - Preview changes (plan)
- `pmp apply` - Apply changes
- `pmp find` - Search projects by name or kind

## Development

```bash
cargo build
cargo test
cargo run -- create
```

## License

MIT License
