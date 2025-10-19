# PMP - Poor Man's Platform

A CLI for managing Infrastructure as Code projects using OpenTofu/Terraform with template-based project generation.

## Features

- **ProjectCollection-based organization** - All projects live in a collection with defined categories and environments
- **Category-driven template selection** - Categories define which resource kinds are allowed
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

### 1. Create a ProjectCollection

A ProjectCollection is **required** before creating any projects. It defines the workspace for your infrastructure projects.

Create a `.pmp.project-collection.yaml` file:

```yaml
apiVersion: pmp.io/v1
kind: ProjectCollection
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

  # Define categories with allowed resource kinds
  categories:
    workload:
      name: "Workloads"
      description: "Application workloads"
      resource_kinds:
        - apiVersion: pmp.io/v1
          kind: KubernetesWorkload

  # Optional: Organize by category
  organize_by_category: false
```

### 2. Create a Template

Templates use `.pmp.template.yaml` in their root directory:

```
~/.pmp/templates/kubernetes-workload/
├── .pmp.template.yaml
└── src/
    ├── deployment.yaml.hbs
    └── .pmp.yaml.hbs
```

See full documentation below for template examples.

### 3. Create a Project

```bash
cd /path/to/my-collection
pmp create
```

### 4. Find Projects

```bash
pmp find --name my-api
pmp find --category workload
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

## ProjectCollection Categories

Categories define which resource kinds are allowed. Templates are filtered by category selection.

```yaml
categories:
  workload:
    name: "Workloads"
    resource_kinds:
      - apiVersion: pmp.io/v1
        kind: KubernetesWorkload
    children:
      critical:
        name: "Critical"
        resource_kinds:
          - apiVersion: pmp.io/v1
            kind: KubernetesWorkload
```

## Project Structure

Projects are organized as: `projects/{resource-kind}/{project-name}/`

Example:
```
projects/
└── kubernetes_workload/
    ├── api-service/
    │   ├── .pmp.yaml
    │   └── deployment.yaml
    └── worker/
        └── .pmp.yaml
```

## Commands

- `pmp create` - Create new project (requires ProjectCollection)
- `pmp preview` - Preview changes (plan)
- `pmp apply` - Apply changes
- `pmp find` - Search projects by name or category

## Development

```bash
cargo build
cargo test
cargo run -- create
```

## License

MIT License
