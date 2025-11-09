# Examples Template Pack

This template pack demonstrates PMP's pre-installed plugins feature with real-world use cases.

## Overview

The `examples` template pack contains templates that showcase advanced PMP features, particularly the ability to automatically provision supporting infrastructure during project creation.

## Templates

### Application

**Template**: `application`
**Resource Kind**: `KubernetesWorkload`
**Description**: Production-ready application deployment with auto-provisioned GitHub repository and ECR registry

This template demonstrates the **pre-installed plugins** feature, which automatically provisions essential infrastructure when you create a new project.

#### Pre-installed Plugins

When you create a project using this template, PMP automatically installs:

1. **GitHub Repository** (`github/repository`)
   - Creates a private GitHub repository for your application code
   - Configures branch protection (requires 2 approvals)
   - Adds a Terraform-specific `.gitignore`
   - Applies Apache 2.0 license
   - Enables issues tracking

2. **AWS ECR Registry** (`aws/ecr`)
   - Creates an AWS ECR repository for container images
   - Configures immutable image tags
   - Enables KMS encryption for images at rest
   - Enables automatic security scanning on push
   - Sets up lifecycle policy to retain 20 most recent images

#### How It Works

During project creation (`pmp create`):

1. You select the `examples` template pack and `application` template
2. You provide standard template inputs (replicas, namespace, etc.)
3. **For each pre-installed plugin**, PMP will:
   - Display: `Installing plugin: github/repository`
   - Prompt: `Customize inputs for this plugin? (y/N)`
   - If **yes**: You'll be prompted for all plugin inputs with sensible defaults shown
   - If **no**: Uses the defaults configured in the template
4. Plugins are rendered into `modules/github/repository/` and `modules/aws/ecr/`
5. The generated `.pmp.environment.yaml` includes all plugins under `spec.plugins.added`

#### Optional Plugins

Users can also manually add these plugins after project creation:

- **PostgreSQL Access** (`postgres/access`) - Database credentials and access
- **Vault Secret** (`vault/secret`) - Secret management with HashiCorp Vault

Use `pmp update --add-plugin` to add these after project creation.

## Template Inputs

The `application` template includes these basic inputs:

- `docker_image_version` - Container image tag (default: `latest`)
- `replicas` - Number of pod replicas (default: `3`)
- `namespace` - Kubernetes namespace (default: `default`)
- `container_port` - Port to expose (default: `8080`)
- `cpu_request` - CPU request (default: `100m`)
- `memory_request` - Memory request (default: `256Mi`)
- `cpu_limit` - CPU limit (default: `500m`)
- `memory_limit` - Memory limit (default: `512Mi`)

## Use Cases

This template is ideal for:

- **Greenfield applications**: Start with complete infrastructure from day one
- **Standardization**: Ensure all applications follow organizational best practices
- **Developer onboarding**: Reduce manual setup steps for new projects
- **Compliance**: Enforce security policies (encryption, scanning, branch protection)

## Example Usage

```bash
# Create a new application project
pmp create

# Select: examples/application
# Provide inputs (name, replicas, etc.)
# For each plugin, choose whether to customize inputs
# Result: Complete Kubernetes + GitHub + ECR setup ready to use
```

## Customization

To create your own template with pre-installed plugins:

1. Define `spec.plugins.installed` in your `.pmp.template.yaml`
2. Reference plugins from existing template packs
3. Configure default values for plugin inputs
4. Users can still override defaults during project creation

## Benefits of Pre-installed Plugins

- **Consistency**: All projects start with the same foundation
- **Speed**: No manual provisioning of GitHub repos or ECR registries
- **Best practices**: Enforces organizational standards by default
- **Flexibility**: Users can still customize if needed
- **Discoverability**: New users don't need to know about plugins upfront

## Related Template Packs

- **github** - Standalone GitHub repository management
- **aws** - AWS infrastructure (includes standalone ECR template)
- **kubernetes-workloads** - Various Kubernetes deployment patterns
- **postgres** - PostgreSQL database with access plugins
- **vault** - HashiCorp Vault secret management

## Learn More

See the [PMP documentation](../../docs/) for more details on:
- Creating custom template packs
- Developing plugins
- Using pre-installed vs. allowed plugins
- Plugin development best practices
