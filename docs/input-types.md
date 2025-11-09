# Input Types and Variable Interpolation

This document describes the input types available for templates and plugins, and how to use variable interpolation in input configurations.

## Input Types

PMP supports explicit input types for better user experience and validation. You can specify the type using the `type` field in the input specification.

### String Input

The default input type for text values.

```yaml
spec:
  inputs:
    app_name:
      type: string
      description: "Application name"
      default: "myapp"
```

### Boolean Input (Yes/No)

For boolean values, displayed as a confirmation prompt.

```yaml
spec:
  inputs:
    enable_monitoring:
      type: boolean
      description: "Enable monitoring"
      default: true

    debug_mode:
      type: boolean
      description: "Enable debug mode"
      default: false
```

**User Experience:**
```
✓ Enable monitoring? (Y/n)
✓ Enable debug mode? (y/N)
```

### Number Input with Ranges

For numeric values with optional min/max validation.

```yaml
spec:
  inputs:
    replica_count:
      type: number
      min: 1
      max: 10
      description: "Number of replicas"
      default: 3

    port:
      type: number
      min: 1024
      max: 65535
      description: "Application port"
      default: 8080

    timeout_seconds:
      type: number
      description: "Timeout in seconds (no limits)"
      default: 30
```

**User Experience:**
```
? Number of replicas (min: 1, max: 10, default: 3): 5
? Application port (min: 1024, max: 65535, default: 8080): 3000
? Timeout in seconds (default: 30): 60
```

**Validation:**
- Values outside the specified range are rejected with a warning
- User is prompted again until a valid value is entered
- Only integer values are supported (i64)

### Select Input with Display Labels

For selecting from predefined options with friendly display names.

```yaml
spec:
  inputs:
    environment_type:
      type: select
      options:
        - label: "Development"
          value: "dev"
        - label: "Staging / QA"
          value: "staging"
        - label: "Production"
          value: "prod"
      description: "Environment type"
      default: "dev"

    instance_size:
      type: select
      options:
        - label: "Small (2 CPU, 4GB RAM)"
          value: "t3.small"
        - label: "Medium (4 CPU, 8GB RAM)"
          value: "t3.medium"
        - label: "Large (8 CPU, 16GB RAM)"
          value: "t3.large"
      description: "Instance size"
      default: "t3.small"
```

**User Experience:**
```
? Environment type:
  > Development
    Staging / QA
    Production
```

**Template Access:**
The `value` field is what gets used in templates:
```hbs
environment_type: {{environment_type}}
# Results in: environment_type: dev
```

## Variable Interpolation

Input default values support variable interpolation using the `${var:variable_name}` syntax. This allows you to reference other variables (including automatic variables) when defining defaults.

### Available Variables for Interpolation

1. **Automatic Variables:**
   - `${var:_name}` - Project name
   - `${var:_environment}` - Environment name
   - `${var:_resource_api_version}` - Resource API version
   - `${var:_resource_kind}` - Resource kind

2. **Previously Collected Inputs:**
   - Any input that was collected before the current one
   - Progressive interpolation: inputs are processed in order

### Examples

#### Example 1: Using Project Name in Defaults

```yaml
spec:
  inputs:
    database_name:
      type: string
      description: "Database name"
      default: "${var:_name}_db"

    namespace:
      type: string
      description: "Kubernetes namespace"
      default: "${var:_name}-${var:_environment}"
```

**Result:**
- If project name is `myapp` and environment is `dev`:
  - `database_name` default: `myapp_db`
  - `namespace` default: `myapp-dev`

#### Example 2: Referencing Other Inputs

```yaml
spec:
  inputs:
    app_name:
      type: string
      description: "Application name"
      default: "myapp"

    service_name:
      type: string
      description: "Service name"
      default: "${var:app_name}-service"

    ingress_host:
      type: string
      description: "Ingress hostname"
      default: "${var:app_name}.${var:_environment}.example.com"
```

**Processing Order:**
1. `app_name` is collected first (e.g., user enters "api")
2. `service_name` default becomes "api-service"
3. `ingress_host` default becomes "api.dev.example.com"

#### Example 3: Complex Interpolation

```yaml
spec:
  inputs:
    project_prefix:
      type: string
      description: "Project prefix"
      default: "${var:_name}"

    database_host:
      type: string
      description: "Database hostname"
      default: "${var:project_prefix}-db.${var:_environment}.internal"

    cache_host:
      type: string
      description: "Cache hostname"
      default: "${var:project_prefix}-redis.${var:_environment}.internal"
```

### Interpolation Rules

1. **Syntax:** `${var:variable_name}`
   - Variable name must start with a letter or underscore
   - Can contain letters, numbers, and underscores
   - Case-sensitive

2. **Value Types:**
   - String variables: Inserted as-is
   - Number variables: Converted to string
   - Boolean variables: Converted to "true" or "false"
   - Other types: Error

3. **Error Handling:**
   - If a variable is not found: Error with clear message
   - If a variable has an unsupported type: Error with type information

4. **Nested Interpolation:**
   - Not supported: `${var:${var:foo}}` will not work
   - Workaround: Use progressive collection with multiple inputs

## Backward Compatibility

### Legacy enum_values

The old `enum_values` field is still supported but deprecated:

```yaml
# Old style (still works)
spec:
  inputs:
    size:
      enum_values:
        - small
        - medium
        - large
      default: medium
```

**Recommendation:** Migrate to the new `select` type with `options` for better UX:

```yaml
# New style (recommended)
spec:
  inputs:
    size:
      type: select
      options:
        - label: "Small Instance"
          value: "small"
        - label: "Medium Instance"
          value: "medium"
        - label: "Large Instance"
          value: "large"
      default: "medium"
```

### Type Inference

If no explicit `type` is specified, the type is inferred from the `default` value:

```yaml
spec:
  inputs:
    # Inferred as string
    name:
      default: "myapp"

    # Inferred as number
    count:
      default: 3

    # Inferred as boolean
    enabled:
      default: true
```

## Complete Example

Here's a complete template showing all input types and variable interpolation:

```yaml
apiVersion: pmp.io/v1
kind: Template
metadata:
  name: web-application
  description: "Web application with database"
spec:
  apiVersion: pmp.io/v1
  kind: KubernetesWorkload
  executor: opentofu

  inputs:
    # String with interpolation
    app_name:
      type: string
      description: "Application name"
      default: "${var:_name}"

    # Number with range
    replica_count:
      type: number
      min: 1
      max: 10
      description: "Number of replicas"
      default: 3

    # Boolean
    enable_autoscaling:
      type: boolean
      description: "Enable horizontal pod autoscaling"
      default: false

    # Select with labels
    instance_type:
      type: select
      options:
        - label: "Development (Burstable)"
          value: "t3.micro"
        - label: "Production (General Purpose)"
          value: "t3.medium"
        - label: "High Performance"
          value: "c5.large"
      description: "Instance type"
      default: "t3.micro"

    # String with complex interpolation
    database_url:
      type: string
      description: "Database connection URL"
      default: "postgresql://${var:app_name}-db.${var:_environment}.svc.cluster.local:5432/${var:app_name}"

    # Number without range
    max_connections:
      type: number
      description: "Maximum database connections"
      default: 100

    # Boolean for feature flags
    enable_metrics:
      type: boolean
      description: "Enable Prometheus metrics"
      default: true

    enable_tracing:
      type: boolean
      description: "Enable distributed tracing"
      default: false
```

## Tips and Best Practices

1. **Use Descriptive Labels:**
   ```yaml
   # Good
   options:
     - label: "Small (1 CPU, 2GB RAM) - $0.05/hour"
       value: "small"

   # Less helpful
   options:
     - label: "Small"
       value: "small"
   ```

2. **Set Sensible Defaults:**
   - Use variable interpolation to auto-fill common patterns
   - Set safe production-ready defaults for numbers
   - Default booleans to the most secure option

3. **Validate Numbers:**
   - Always set `min` for values that must be positive
   - Set `max` for resource limits (CPUs, memory, etc.)

4. **Order Inputs Logically:**
   - Put inputs that are referenced by others first
   - Group related inputs together
   - Put less frequently changed inputs last

5. **Provide Clear Descriptions:**
   - Explain what the value is used for
   - Mention any constraints or implications
   - Include examples if the format is complex
