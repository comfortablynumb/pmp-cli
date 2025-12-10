# Templates

Templates are the foundation of PMP's project generation system. They define the structure, inputs, and configuration for infrastructure resources.

## Template Pack Structure

```
~/.pmp/template-packs/{pack-name}/
├── .pmp.template-pack.yaml       # Pack metadata
├── templates/
│   └── {template-name}/
│       ├── .pmp.template.yaml    # Template definition
│       ├── main.tf.hbs           # Handlebars templates
│       ├── variables.tf.hbs
│       └── outputs.tf.hbs
└── plugins/
    └── {plugin-name}/
        ├── .pmp.plugin.yaml
        └── *.tf.hbs
```

## Template Pack Definition

```yaml
apiVersion: pmp.io/v1
kind: TemplatePack
metadata:
  name: "kubernetes"
  description: "Kubernetes infrastructure templates"
spec: {}
```

## Template Definition

```yaml
apiVersion: pmp.io/v1
kind: Template
metadata:
  name: "API Service"
  description: "Deploy a Kubernetes API service"
  labels:
    tier: backend
    category: applications

spec:
  apiVersion: pmp.io/v1
  kind: KubernetesWorkload  # Must be alphanumeric only
  executor: opentofu        # opentofu | none

  inputs:
    - name: replicas
      description: "Number of pod replicas"
      default: 3
      type:
        type: number
        min: 1
        max: 10

  # Environment-specific overrides
  environments:
    prod:
      overrides:
        inputs:
          - name: replicas
            default: 5

  # Dependencies on other projects
  dependencies:
    - project:
        apiVersion: pmp.io/v1
        kind: PostgresDatabase
        description: "Database to connect to"

  # Plugins configuration
  plugins:
    allowed:
      - template_pack_name: postgres
        plugin_name: access
    installed:
      - template_pack_name: monitoring
        plugin_name: prometheus

  # Hooks (copied to generated projects)
  hooks:
    pre_destroy:
      - type: confirm
        config:
          question: "Destroy this service?"
          exit_on_cancel: true
```

## Input Types

PMP supports 25+ input types for collecting user configuration.

### Basic Types

| Type | Description | Returns |
|------|-------------|---------|
| `string` | Text input | String |
| `number` | Numeric with optional min/max | Number |
| `boolean` | Yes/no toggle | Boolean |
| `password` | Hidden text input | String |
| `email` | Email validation | String |
| `url` | URL validation | String |
| `ip` | IP address validation | String |
| `cidr` | CIDR block validation | String |
| `path` | File/directory path | String |
| `port` | Network port (1-65535) | Number |

### Selection Types

| Type | Description | Returns |
|------|-------------|---------|
| `select` | Single choice from options | String |
| `multiselect` | Multiple choices from options | Array |

### Complex Types

| Type | Description | Returns |
|------|-------------|---------|
| `list` | Comma-separated values | Array |
| `json` | JSON format validation | JSON |
| `yaml` | YAML format validation | String |
| `object` | Structured object with fields | Object |
| `repeatable_object` | Array of objects (add/remove) | Array |
| `keyvalue` | Key-value pairs | Object |

### Specialized Types

| Type | Description | Returns |
|------|-------------|---------|
| `color` | Hex color (#RRGGBB or #RRGGBBAA) | String |
| `duration` | Time duration (1h30m, 5d, 2w) | Number (seconds) |
| `cron` | Cron expression | String |
| `semver` | Semantic version | String |
| `region` | Cloud region | String |
| `arn` | AWS ARN validation | String |
| `docker_image` | Docker image reference | String |

### Project Reference Types

| Type | Description | Returns |
|------|-------------|---------|
| `project_select` | Single project reference | Object |
| `multi_project_select` | Multiple project references | Array |

## Input Examples

### String with Default

```yaml
- name: app_name
  description: "Application name"
  default: "my-app"
  type:
    type: string
```

### Number with Constraints

```yaml
- name: replicas
  description: "Number of replicas"
  default: 3
  type:
    type: number
    min: 1
    max: 100
```

### Select with Options

```yaml
- name: instance_size
  description: "Instance size"
  default: "small"
  type:
    type: select
    options:
      - label: "Small (2 CPU, 4GB)"
        value: "small"
      - label: "Medium (4 CPU, 8GB)"
        value: "medium"
      - label: "Large (8 CPU, 16GB)"
        value: "large"
```

### Object with Nested Fields

```yaml
- name: database_config
  description: "Database configuration"
  type:
    type: object
    fields:
      - name: host
        type:
          type: string
        default: "localhost"
      - name: port
        type:
          type: number
        default: 5432
      - name: ssl_enabled
        type:
          type: boolean
        default: true
```

### Repeatable Object

```yaml
- name: team_members
  description: "Team members with roles"
  type:
    type: repeatable_object
    min: 0
    max: 50
    add_another_prompt: "Add another member?"
    fields:
      - name: username
        type:
          type: string
        description: "Username"
      - name: role
        type:
          type: select
          options:
            - label: "Member"
              value: "member"
            - label: "Admin"
              value: "admin"
        default: "member"
```

### Conditional Inputs

```yaml
- name: enable_ssl
  type:
    type: boolean
  default: false

- name: ssl_certificate
  description: "SSL certificate path"
  type:
    type: path
  show_if:
    - field: enable_ssl
      condition: equals
      value: true
```

### Variable Interpolation

```yaml
- name: namespace
  type:
    type: string
  default: "${var:_project_name_hyphens}-ns"

- name: database_url
  type:
    type: string
  default: "postgresql://${var:app_name}-db:5432/${var:app_name}"
```

## Template Variables

### System Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `_name` | Project name | `my-api` |
| `_project_name_underscores` | Name with underscores | `my_api` |
| `_project_name_hyphens` | Name with hyphens | `my-api` |
| `_environment` | Environment name | `dev`, `prod` |
| `_resource_api_version` | Resource API version | `pmp.io/v1` |
| `_resource_kind` | Resource kind | `KubernetesWorkload` |

### Plugin Variables

| Variable | Description |
|----------|-------------|
| `_plugins.added` | Array of installed plugins |
| `_reference_project_name` | Referenced project name (in plugins) |

## Handlebars Templates

Templates use Handlebars syntax with custom helpers.

### Basic Syntax

```handlebars
resource "kubernetes_deployment" "{{_name}}" {
  metadata {
    name      = "{{_name}}"
    namespace = "{{namespace}}"
  }

  spec {
    replicas = {{replicas}}
  }
}
```

### Conditionals

```handlebars
{{#if enable_monitoring}}
resource "kubernetes_service_monitor" "{{_name}}" {
  # ...
}
{{/if}}
```

### Loops

```handlebars
{{#each team_members}}
resource "github_team_membership" "member_{{@index}}" {
  username = "{{username}}"
  role     = "{{role}}"
}
{{/each}}
```

### Custom Helpers

| Helper | Description | Example |
|--------|-------------|---------|
| `eq` | Equality check | `{{#if (eq env "prod")}}...{{/if}}` |
| `contains` | Array contains | `{{#if (contains features "monitoring")}}...{{/if}}` |
| `k8s_name` | Kubernetes name sanitization | `{{k8s_name _name}}` |
| `bool` | Boolean to HCL | `{{bool enable_feature}}` |
| `json` | JSON stringify | `{{json config}}` |

## Creating a Template Pack

```bash
pmp template scaffold --output ~/.pmp/template-packs/my-pack
```

This creates an interactive wizard to define:
- Pack name and description
- Initial template with inputs
- Sample Handlebars files

## Discovery

Template packs are discovered from:
1. `~/.pmp/template-packs/` (global)
2. `.pmp/template-packs/` (local to infrastructure)
3. Custom paths via `--template-packs-paths` or `PMP_TEMPLATE_PACKS_PATHS` environment variable
