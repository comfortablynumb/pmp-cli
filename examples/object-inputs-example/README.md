# Object and RepeatableObject Input Types Example

This example demonstrates how to use the **Object** and **RepeatableObject** input types in PMP templates.

## Input Types Overview

### Object Type

The `object` input type groups multiple related inputs into a single structured object. This is useful for:
- Configuration groups (database config, API settings, etc.)
- Nested data structures
- Organizing related settings together

**Features:**
- Supports nested objects (fields can also be of type `object`)
- Each field can use any supported input type (string, number, boolean, select, etc.)
- Returns a JSON object with field names as keys

**Example:**
```yaml
- name: database_config
  type: object
  description: Database configuration
  fields:
    - name: host
      type: string
      description: Database host
      default: "localhost"
    - name: port
      type: number
      description: Database port
      default: 5432
```

### RepeatableObject Type

The `repeatable_object` input type creates an array of structured objects with interactive add/remove functionality. This is useful for:
- Lists of users, services, or resources
- Dynamic collections where the number of items varies
- Configurable arrays of complex data

**Features:**
- Interactive workflow: Add new items, Remove existing items, or Done
- Shows current item count after each operation
- When removing, displays a list with summaries for easy selection
- Respects `min` and `max` constraints
- Returns an array of objects as JSON

**Example:**
```yaml
- name: team_members
  type: repeatable_object
  description: Team members
  min: 1
  max: 50
  add_another_prompt: "Add another team member?"
  fields:
    - name: username
      type: string
      description: GitHub username
    - name: role
      type: select
      description: Member role
      options:
        - label: "Admin"
          value: "admin"
        - label: "Member"
          value: "member"
```

## Interactive Workflow

When using `repeatable_object`, users will see:

1. **Current item count** (if any items exist)
2. **Action menu** with options:
   - "Add new [item]" - Add a new item to the list
   - "Remove [item]" - Remove an existing item (only if count > min)
   - "Done" - Finish editing the list

3. When **adding**, user is prompted for each field
4. When **removing**, user sees a list like:
   ```
   Select item to remove:
   > #1 - username: john_doe, role: admin
     #2 - username: jane_smith, role: member
     #3 - username: bob_wilson, role: developer
     Cancel
   ```

## Using in Templates

Access object fields using dot notation in Handlebars templates:

```handlebars
# Simple object access
{{database_config.host}}
{{database_config.port}}
{{bool database_config.ssl_enabled}}

# Nested object access
{{application_config.resource_limits.cpu}}
{{application_config.resource_limits.memory}}

# Iterating over repeatable objects
{{#each users}}
resource "user_{{@index}}" {
  username = "{{username}}"
  email    = "{{email}}"
  role     = "{{role}}"
  active   = {{bool active}}
}
{{/each}}
```

## Handlebars Helpers

- `{{bool variable}}` - Convert boolean to HCL format (true/false)
- `{{json variable}}` - JSON stringify an object
- `{{#each array}}...{{/each}}` - Iterate over arrays
- `{{@index}}` - Current index in iteration (0-based)

## Example Template Structure

See `.pmp.template.yaml` for the complete input definitions and `main.tf.hbs` for how to use these inputs in your infrastructure code.

## Testing the Example

1. Create a new project using this template:
   ```bash
   pmp create
   ```

2. Follow the interactive prompts to:
   - Configure database settings (Object input)
   - Configure application settings with nested objects
   - Add/remove users (RepeatableObject with add/remove)
   - Add/remove services (RepeatableObject with various field types)

3. Review the generated `main.tf` file to see how your inputs were rendered

## Best Practices

1. **Use Object for related settings**: Group logically related inputs together
2. **Set reasonable min/max for RepeatableObject**: Prevent too few or too many items
3. **Provide clear descriptions**: Help users understand what each field does
4. **Use meaningful field names**: They become object keys in the generated code
5. **Leverage nested objects**: For complex hierarchical configurations
6. **Customize add_another_prompt**: Make it specific to your use case (e.g., "Add another service?" instead of "Add another item?")

## Common Use Cases

### Object Type
- Database configurations
- API credentials and settings
- Resource limits (CPU, memory)
- Network settings
- Application configurations

### RepeatableObject Type
- User lists with roles and permissions
- Microservices configurations
- Environment variables
- Network rules or firewall rules
- Team members or collaborators
- Feature flags
- Deployment stages
