# Plugins

Plugins are reusable template components that can be added to projects. They enable modular infrastructure composition.

## Plugin Structure

```
~/.pmp/template-packs/{pack-name}/
└── plugins/
    └── {plugin-name}/
        ├── .pmp.plugin.yaml   # Plugin definition
        └── *.tf.hbs           # Handlebars templates
```

## Plugin Definition

```yaml
# .pmp.plugin.yaml
apiVersion: pmp.io/v1
kind: Plugin
metadata:
  name: database-access
  description: "Configure database access credentials"

spec:
  role: access-management

  inputs:
    - name: username
      description: "Database username"
      type:
        type: string
      default: "${var:_project_name_underscores}"

    - name: privileges
      description: "Database privileges"
      type:
        type: multiselect
        options:
          - label: "SELECT"
            value: "SELECT"
          - label: "INSERT"
            value: "INSERT"
          - label: "UPDATE"
            value: "UPDATE"
          - label: "DELETE"
            value: "DELETE"
      default: ["SELECT"]
```

## Plugin Types

### Allowed Plugins

Users can optionally add these plugins during project update:

```yaml
# .pmp.template.yaml
spec:
  plugins:
    allowed:
      - template_pack_name: postgres
        plugin_name: access
        input_constraints:
          privileges:
            allowed_values: ["SELECT", "INSERT"]  # Restrict options
```

### Installed Plugins

Automatically added during project creation:

```yaml
# .pmp.template.yaml
spec:
  plugins:
    installed:
      - template_pack_name: monitoring
        plugin_name: prometheus
        disable_user_input_override: true  # Use defaults, skip prompts
```

## Using Plugins

### During Project Creation

Installed plugins are processed automatically:

```bash
pmp project create --template kubernetes/api-service

# Output:
# Creating project: my-api
# Installing plugin: prometheus
#   > Scrape interval: [30s]
#   > Enable alerts: [yes]
```

### During Project Update

Add allowed plugins when updating:

```bash
pmp project update

# Output:
# Available plugins:
#   [x] database-access (postgres)
#   [ ] redis-cache (redis)
# Select plugins to add: ...
```

## Plugin Templates

Plugin templates have access to:

- All template variables (`_name`, `_environment`, etc.)
- Plugin input values
- Reference project information (when declared)

```handlebars
# access.tf.hbs
resource "postgresql_role" "{{_project_name_underscores}}" {
  name     = "{{username}}"
  login    = true
  password = var.db_password

  {{#each privileges}}
  # Privilege: {{this}}
  {{/each}}
}

{{#if _reference_project_name}}
# Depends on: {{_reference_project_name}}
data "terraform_remote_state" "database" {
  backend = "s3"
  config = {
    bucket = "terraform-state"
    key    = "{{_reference_project_name}}/{{_environment}}/terraform.tfstate"
  }
}
{{/if}}
```

## Plugin with Dependencies

Plugins can reference other projects:

```yaml
# .pmp.plugin.yaml
spec:
  dependencies:
    - project:
        apiVersion: pmp.io/v1
        kind: PostgresDatabase
        description: "Database to configure access for"
```

When users add this plugin, they select which database project to reference.

## Pre-configured Plugins

Configure plugins in infrastructure or project groups:

```yaml
# .pmp.infrastructure.yaml
spec:
  projects:
    list:
      - name: my-api
        template_pack: kubernetes
        template: api-service
        plugins:
          database-access:
            reference_projects:
              - name: postgres-main
                environment: dev
            inputs:
              username:
                value: "my_api_user"
              privileges:
                value: ["SELECT", "INSERT"]
```

**Behavior:**
- Pre-configured plugins skip interactive prompts
- Partial configuration: only unconfigured values prompt
- Input precedence: pre-configured > template default

## Plugin Variables

| Variable | Description |
|----------|-------------|
| `_plugins.added` | Array of all installed plugins |
| `_reference_project_name` | Name of referenced project |
| `_plugin_name` | Current plugin name |
| `_plugin_template_pack` | Template pack containing the plugin |

## Example: Team Management Plugin

```yaml
# .pmp.plugin.yaml
apiVersion: pmp.io/v1
kind: Plugin
metadata:
  name: team
  description: "Configure team access"

spec:
  role: access-management

  inputs:
    - name: team_name
      type:
        type: string
      default: "${var:_project_name_hyphens}-team"

    - name: team_members
      description: "Team members with roles"
      type:
        type: repeatable_object
        min: 0
        max: 100
        add_another_prompt: "Add another member?"
        fields:
          - name: username
            type:
              type: string
            description: "GitHub username"
          - name: role
            type:
              type: select
              options:
                - label: "Member"
                  value: "member"
                - label: "Maintainer"
                  value: "maintainer"
            default: "member"
```

```handlebars
# team.tf.hbs
resource "github_team" "{{k8s_name team_name}}" {
  name        = "{{team_name}}"
  privacy     = "closed"
}

{{#each team_members}}
resource "github_team_membership" "{{../team_name}}_{{username}}" {
  team_id  = github_team.{{k8s_name ../team_name}}.id
  username = "{{username}}"
  role     = "{{role}}"
}
{{/each}}
```

## Best Practices

1. **Single responsibility** - Each plugin does one thing well
2. **Sensible defaults** - Minimize required configuration
3. **Clear descriptions** - Document what the plugin does
4. **Input validation** - Use appropriate input types with constraints
5. **Version compatibility** - Document required template versions
