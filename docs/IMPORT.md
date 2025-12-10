# PMP Import System

## Overview

The PMP import system allows you to bring existing Terraform/OpenTofu infrastructure under PMP management. This enables teams to adopt PMP gradually without disrupting existing deployments.

## Why Import?

- **Gradual Adoption**: Migrate existing infrastructure to PMP without rebuilding
- **Team Onboarding**: Bring existing projects into standardized management
- **Infrastructure Consolidation**: Unify disparate Terraform projects under PMP
- **State Preservation**: Maintain existing state and avoid resource recreation

## Quick Start

### Import an Existing Terraform Project

```bash
# Basic import (interactive)
pmp import project ./my-terraform-infra

# With options
pmp import project ./my-terraform-infra \
  --name my-app \
  --environment production \
  --file-strategy copy

# Dry-run to preview
pmp import project ./my-terraform-infra --dry-run
```

## Command Reference

### `pmp import project <path>`

Import an entire Terraform/OpenTofu project directory.

**Arguments:**
- `<path>` - Path to the Terraform project directory

**Options:**
- `--name <name>` - Project name (defaults to directory name)
- `--environment <env>` - Environment name
- `--template-pack <pack>` - Template pack to use
- `--template-name <name>` - Template name to use
- `--file-strategy <strategy>` - How to handle files: `copy`, `move`, `symlink`, `template_convert`
- `--import-state` - Import state file (default: true)
- `--dry-run` - Preview without making changes
- `--yes` - Skip confirmations and use defaults

**Example:**
```bash
# Import with custom settings
pmp import project ./existing-vpc \
  --name production-vpc \
  --environment prod \
  --file-strategy copy \
  --yes
```

### `pmp import state <state-file>`

Import from an existing state file.

**Arguments:**
- `<state-file>` - Path to the Terraform state file

**Options:**
- `--name <name>` - Project name (required)
- `--environment <env>` - Environment name
- `--source-dir <path>` - Directory containing Terraform files
- `--dry-run` - Preview without making changes

**Example:**
```bash
pmp import state ./terraform.tfstate \
  --name my-app \
  --environment production \
  --source-dir ./terraform-files
```

**Status:** Not yet implemented (planned for Phase 2)

### `pmp import resource <addresses>`

Import specific resources by their Terraform addresses.

**Arguments:**
- `<addresses>` - Comma-separated list of resource addresses

**Options:**
- `--project <name>` - Project name (required)
- `--environment <env>` - Environment name (required)
- `--id <id>` - Cloud provider resource ID

**Example:**
```bash
# Import single resource
pmp import resource aws_s3_bucket.my_bucket \
  --project storage \
  --environment production

# Import multiple resources
pmp import resource aws_s3_bucket.my_bucket,aws_s3_bucket_policy.policy \
  --project storage \
  --environment production
```

**Status:** Not yet implemented (planned for Phase 4)

### `pmp import bulk <config-file>`

Bulk import multiple projects from a configuration file.

**Arguments:**
- `<config-file>` - Path to import configuration YAML

**Options:**
- `--dry-run` - Preview without making changes
- `--parallel <n>` - Number of concurrent imports (default: 1)

**Example:**
```bash
# Sequential import
pmp import bulk ./import-config.yaml

# Parallel import
pmp import bulk ./import-config.yaml --parallel 4
```

**Status:** Not yet implemented (planned for Phase 6)

## File Handling Strategies

When importing, you can choose how to handle source Terraform files:

### Copy (Default)
- **What it does**: Copies files to PMP project directory
- **Original files**: Remain unchanged
- **Use when**: You want to keep the original files as backup
- **Pros**: Safe, reversible
- **Cons**: Duplicates files

```bash
pmp import project ./infra --file-strategy copy
```

### Move
- **What it does**: Moves files to PMP project directory
- **Original files**: Deleted from source
- **Use when**: Clean migration, no need for originals
- **Pros**: Clean, no duplicates
- **Cons**: Irreversible (without backup)

```bash
pmp import project ./infra --file-strategy move
```

### Symlink
- **What it does**: Creates symbolic links to original files
- **Original files**: Remain in original location
- **Use when**: Gradual migration, files used by other tools
- **Pros**: No duplication, files stay in place
- **Cons**: Depends on original file locations

```bash
pmp import project ./infra --file-strategy symlink
```

### Template Convert (Future)
- **What it does**: Converts .tf files to .tf.hbs templates
- **Original files**: Converted to Handlebars templates
- **Use when**: Full PMP integration desired
- **Pros**: Most PMP-native approach
- **Cons**: More complex, requires variable extraction

**Status:** Planned for Phase 3

## Import Workflow

### Interactive Mode (Recommended)

```bash
$ pmp import project ./my-terraform-infra

🔍 Analyzing Terraform project...
   ✓ Found 15 .tf files
   ✓ Found terraform.tfstate
   ✓ Detected 23 resources across 2 providers

📊 Analysis Summary:
   Providers: aws (v5.0), random (v3.5)
   Resources:
     - aws_vpc (1)
     - aws_subnet (6)
     - aws_security_group (3)
     - aws_instance (10)
     - random_password (3)

? Project name: my-terraform-infra
? Environment: production
? File handling strategy:
  ❯ Copy files (safe, creates duplicate)
    Move files (clean, deletes original)
    Symlink files (gradual migration)

? Import state file? (Y/n) Y

📝 Preview:
   Will create:
   ✓ collection/projects/my-terraform-infra/.pmp.project.yaml
   ✓ collection/projects/my-terraform-infra/environments/production/.pmp.environment.yaml
   ✓ collection/projects/my-terraform-infra/environments/production/*.tf (15 files)
   ✓ collection/projects/my-terraform-infra/environments/production/terraform.tfstate

? Proceed with import? (Y/n) Y

✅ Import completed successfully!

   Next steps:
   1. Review generated files in: collection/projects/my-terraform-infra
   2. Run 'pmp preview' to verify state
   3. Run 'pmp apply' to manage with PMP
```

### Automated Mode

For CI/CD or scripting:

```bash
pmp import project ./infra \
  --name my-app \
  --environment prod \
  --file-strategy copy \
  --yes
```

## What Gets Imported

### Files
- `*.tf` - Terraform configuration files
- `terraform.tfstate` - State file (if `--import-state` is true)
- `*.tfvars` - Variable files
- `terraform.lock.hcl` - Dependency lock file

### Generated Files
- `.pmp.project.yaml` - PMP project metadata
- `.pmp.environment.yaml` - Environment configuration
- `_common.tf` - Backend configuration (if executor config exists)

### Directory Structure

```
collection/
└── projects/
    └── my-app/
        ├── .pmp.project.yaml
        └── environments/
            └── production/
                ├── .pmp.environment.yaml
                ├── main.tf
                ├── variables.tf
                ├── outputs.tf
                ├── terraform.tfstate
                └── _common.tf
```

## Best Practices

### Before Importing

1. **Backup State**: Always backup your state file before import
2. **Review Files**: Ensure all necessary files are in the directory
3. **Check Dependencies**: Note any external dependencies or modules
4. **Test Locally**: Use `--dry-run` to preview the import

### During Import

1. **Use Descriptive Names**: Choose clear project and environment names
2. **Match Conventions**: Follow existing naming conventions
3. **Start with Copy**: Use `copy` strategy first, verify, then cleanup
4. **Document Custom Changes**: Note any manual modifications needed

### After Import

1. **Verify State**: Run `pmp preview` to check state integrity
2. **Test Apply**: Run `pmp apply` (should show no changes)
3. **Check Dependencies**: Verify project dependencies are correct
4. **Update Documentation**: Document the imported project
5. **Cleanup Original**: Only remove original files after verification

## Troubleshooting

### State Version Mismatch

**Problem**: Terraform version in state doesn't match OpenTofu

**Solution**:
```bash
# Check version in state file
jq '.terraform_version' terraform.tfstate

# Upgrade state if needed
tofu state replace-provider \
  registry.terraform.io/hashicorp/aws \
  registry.opentofu.org/hashicorp/aws
```

### Missing Provider Configuration

**Problem**: Import fails due to missing provider config

**Solution**: Ensure `provider.tf` or provider blocks exist in source directory

### Resource Conflicts

**Problem**: Resources already exist in PMP

**Solution**: Use different project name or merge manually

### State File Not Found

**Problem**: State file doesn't exist in directory

**Solution**: Either create it or use `--import-state=false`

## Implementation Status

### Phase 1: Basic Project Import ✅
- Import existing Terraform directory
- Copy files to PMP structure
- Generate project metadata
- Import state file

### Phase 2: State File Import 📅
- Import from state file only
- Match to templates
- Generate basic config

### Phase 3: Custom Template Generation 📅
- Parse variables.tf
- Generate .pmp.template.yaml
- Create custom template pack

### Phase 4: Resource Import 📅
- Import by resource address
- Generate Terraform import commands
- Support TF 1.5+ import blocks

### Phase 5: Cloud Discovery 📅
- Scan cloud providers
- Detect unmanaged resources
- Generate Terraform code

### Phase 6: Bulk Import 📅
- Multi-project import
- Configuration file support
- Parallel execution

## Examples

### Example 1: Simple VPC Import

```bash
# Import existing VPC infrastructure
pmp import project ./vpc-infra \
  --name production-vpc \
  --environment production

# Verify
cd collection/projects/production-vpc
pmp preview

# Should show no changes if import was successful
```

### Example 2: Multi-Environment Import

```bash
# Import production environment
pmp import project ./app-prod \
  --name my-app \
  --environment production

# Import staging with same config
pmp import project ./app-staging \
  --name my-app \
  --environment staging

# Both environments now under same project
```

### Example 3: Gradual Migration

```bash
# Phase 1: Symlink import (no disruption)
pmp import project ./legacy-infra \
  --name legacy-system \
  --environment prod \
  --file-strategy symlink

# Verify everything works
pmp preview
pmp apply

# Phase 2: Convert to copy when confident
# Manually copy files and update
```

## Related Documentation

- [Design Document](import-system-design.md) - Detailed architecture and implementation
- [CLI Reference](CLI.md) - Complete command reference
- [Project Structure](STRUCTURE.md) - PMP project organization
- [Migration Guide](MIGRATION.md) - Migrating from standalone Terraform

## Support

For issues or questions:
- Check troubleshooting section above
- Review [design document](import-system-design.md)
- Open an issue on GitHub
- Ask in community discussions

## Future Enhancements

- Template matching algorithm
- Cloud resource discovery
- Automated dependency detection
- State migration tools
- IDE integration for imports
- Import validation and health checks
