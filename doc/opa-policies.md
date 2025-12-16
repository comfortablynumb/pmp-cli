# OPA Policies Guide

PMP integrates native Open Policy Agent (OPA) support using the [regorus](https://github.com/microsoft/regorus) engine (pure Rust OPA implementation). This allows you to write Rego policies to validate Terraform/OpenTofu plans before applying changes.

## Table of Contents

- [Quick Start](#quick-start)
- [Policy Discovery](#policy-discovery)
- [Configuration](#configuration)
- [CLI Commands](#cli-commands)
- [Writing Policies](#writing-policies)
- [Policy Examples](#policy-examples)
- [Testing Policies](#testing-policies)
- [Best Practices](#best-practices)
- [Remediation Annotations](#remediation-annotations)
- [Compliance Reports](#compliance-reports)
- [Troubleshooting](#troubleshooting)

## Quick Start

### 1. Create a Policy Directory

```bash
mkdir -p policies
```

### 2. Write Your First Policy

Create `policies/naming.rego`:

```rego
package pmp.naming

# @description Enforce resource naming conventions

# Deny resources without proper naming prefix
deny[msg] {
    resource := input.resource_changes[_]
    resource.change.actions[_] == "create"
    name := resource.change.after.name
    not startswith(name, "pmp-")
    msg := sprintf("Resource '%s' must have 'pmp-' prefix, got: %s", [resource.address, name])
}
```

### 3. Validate Your Infrastructure

```bash
# Generate a plan and validate
pmp project preview
pmp policy opa validate

# Or validate a specific path
pmp policy opa validate --path ./projects/my-app/environments/dev
```

## Policy Discovery

PMP discovers policies from multiple directories in priority order:

| Priority | Path | Description |
|----------|------|-------------|
| 1 (highest) | `./policies/` | Project-local policies |
| 2 | `~/.pmp/policies/` | Global user policies |
| 3 (lowest) | Custom paths | Paths from configuration |

Files ending in `_test.rego` or starting with `test_` are excluded from validation (used for testing).

### List Discovered Policies

```bash
pmp policy opa list
```

Example output:

```
Discovered OPA Policies
=======================

./policies/naming.rego
  Package: data.pmp.naming
  Description: Enforce resource naming conventions
  Entrypoints: deny

./policies/tagging.rego
  Package: data.pmp.tagging
  Description: Require mandatory tags
  Entrypoints: deny, warn

~/.pmp/policies/security.rego
  Package: data.pmp.security
  Description: Security best practices
  Entrypoints: deny, warn, info

Total: 3 policies
```

## Configuration

Configure OPA in `.pmp.infrastructure.yaml`:

```yaml
apiVersion: pmp.io/v1
kind: Infrastructure
metadata:
  name: "My Infrastructure"
spec:
  policy:
    enabled: true                    # Enable policy validation (default: true)
    fail_on_violation: true          # Fail on any violation (default: true)
    opa:
      paths:                         # Additional policy directories
        - ./team-policies
        - /shared/company-policies
      entrypoint: data.pmp           # Rego entrypoint (default: data.pmp)
      data_files:                    # Additional data files to load
        - ./policy-data/exemptions.json
      thresholds:
        block_on_error: true         # Block apply on deny violations (default: true)
        max_warnings: 10             # Maximum warnings before blocking (optional)
```

## CLI Commands

### Validate

Validate current project or specific path against OPA policies:

```bash
# Validate current directory (uses terraform plan JSON)
pmp policy opa validate

# Validate specific project/environment
pmp policy opa validate --path ./projects/vpc/environments/prod

# Filter by policy package name
pmp policy opa validate --policy naming

# Output as JSON
pmp policy opa validate --format json
```

### Test

Run policy unit tests:

```bash
# Run all tests in default locations
pmp policy opa test

# Run tests in specific directory
pmp policy opa test --path ./policies
```

### List

List all discovered policies:

```bash
pmp policy opa list
```

## Writing Policies

### Policy Structure

OPA policies use the Rego language. PMP expects policies in the `pmp` package namespace:

```rego
package pmp.policy_name

# @description Brief description of this policy

# deny - blocks apply (severity: error)
deny[msg] {
    # condition
    msg := "Error message"
}

# warn - allows apply but shows warning (severity: warning)
warn[msg] {
    # condition
    msg := "Warning message"
}

# info - informational messages (severity: info)
info[msg] {
    # condition
    msg := "Info message"
}
```

### Input Structure

The input is a Terraform/OpenTofu plan in JSON format. Key fields:

```json
{
  "format_version": "1.0",
  "terraform_version": "1.5.0",
  "resource_changes": [
    {
      "address": "aws_instance.web",
      "type": "aws_instance",
      "name": "web",
      "provider_name": "registry.terraform.io/hashicorp/aws",
      "change": {
        "actions": ["create"],
        "before": null,
        "after": {
          "ami": "ami-12345",
          "instance_type": "t3.micro",
          "tags": {
            "Name": "web-server",
            "Environment": "dev"
          }
        },
        "after_unknown": {},
        "after_sensitive": {}
      }
    }
  ],
  "configuration": {
    "provider_config": {},
    "root_module": {}
  }
}
```

### Common Patterns

**Check resource type:**

```rego
resource := input.resource_changes[_]
resource.type == "aws_instance"
```

**Check action (create, update, delete):**

```rego
resource.change.actions[_] == "create"
```

**Check attribute value:**

```rego
resource.change.after.instance_type == "t3.micro"
```

**Check for missing attribute:**

```rego
not resource.change.after.tags.Environment
```

**Check attribute pattern:**

```rego
startswith(resource.change.after.name, "prod-")
```

## Policy Examples

### 1. Naming Conventions

Enforce consistent resource naming:

```rego
# policies/naming.rego
package pmp.naming

# @description Enforce resource naming conventions

import future.keywords.in

# Allowed prefixes per environment
allowed_prefixes := {
    "dev": ["dev-", "development-"],
    "staging": ["stg-", "staging-"],
    "prod": ["prod-", "production-"]
}

# Deny resources without environment prefix
deny[msg] {
    resource := input.resource_changes[_]
    resource.change.actions[_] in ["create", "update"]

    name := resource.change.after.name
    env := resource.change.after.tags.Environment

    prefixes := allowed_prefixes[env]
    not has_valid_prefix(name, prefixes)

    msg := sprintf(
        "Resource '%s' name '%s' must start with one of: %v for environment '%s'",
        [resource.address, name, prefixes, env]
    )
}

has_valid_prefix(name, prefixes) {
    prefix := prefixes[_]
    startswith(name, prefix)
}

# Warn about generic names
warn[msg] {
    resource := input.resource_changes[_]
    resource.change.actions[_] == "create"

    name := resource.change.after.name
    generic_names := ["test", "temp", "tmp", "foo", "bar", "example"]

    contains(lower(name), generic_names[_])

    msg := sprintf(
        "Resource '%s' has generic name '%s'. Consider a more descriptive name.",
        [resource.address, name]
    )
}
```

### 2. Required Tags

Enforce mandatory tagging:

```rego
# policies/tagging.rego
package pmp.tagging

# @description Enforce mandatory resource tags

import future.keywords.in

# Required tags for all resources
required_tags := ["Environment", "Owner", "Project", "CostCenter"]

# Resources that must have tags
taggable_resources := [
    "aws_instance",
    "aws_s3_bucket",
    "aws_rds_cluster",
    "aws_lambda_function",
    "aws_ecs_service",
    "azurerm_virtual_machine",
    "azurerm_storage_account",
    "google_compute_instance"
]

# Deny resources missing required tags
deny[msg] {
    resource := input.resource_changes[_]
    resource.change.actions[_] in ["create", "update"]
    resource.type in taggable_resources

    required_tag := required_tags[_]
    not resource.change.after.tags[required_tag]

    msg := sprintf(
        "Resource '%s' (type: %s) missing required tag: '%s'",
        [resource.address, resource.type, required_tag]
    )
}

# Warn about empty tag values
warn[msg] {
    resource := input.resource_changes[_]
    resource.change.actions[_] in ["create", "update"]

    tag_name := required_tags[_]
    tag_value := resource.change.after.tags[tag_name]
    tag_value == ""

    msg := sprintf(
        "Resource '%s' has empty value for tag '%s'",
        [resource.address, tag_name]
    )
}
```

### 3. Security Best Practices

Enforce security standards:

```rego
# policies/security.rego
package pmp.security

# @description Security best practices for cloud resources

import future.keywords.in

# Deny public S3 buckets
deny[msg] {
    resource := input.resource_changes[_]
    resource.type == "aws_s3_bucket"
    resource.change.actions[_] in ["create", "update"]

    acl := resource.change.after.acl
    acl in ["public-read", "public-read-write"]

    msg := sprintf(
        "S3 bucket '%s' has public ACL '%s'. Use private ACL and bucket policies instead.",
        [resource.address, acl]
    )
}

# Deny unencrypted RDS instances
deny[msg] {
    resource := input.resource_changes[_]
    resource.type == "aws_db_instance"
    resource.change.actions[_] == "create"

    not resource.change.after.storage_encrypted

    msg := sprintf(
        "RDS instance '%s' must have storage encryption enabled",
        [resource.address]
    )
}

# Deny overly permissive security group rules
deny[msg] {
    resource := input.resource_changes[_]
    resource.type == "aws_security_group_rule"
    resource.change.actions[_] in ["create", "update"]

    resource.change.after.type == "ingress"
    resource.change.after.cidr_blocks[_] == "0.0.0.0/0"
    resource.change.after.from_port == 0
    resource.change.after.to_port == 65535

    msg := sprintf(
        "Security group rule '%s' allows all traffic from anywhere. Restrict CIDR and ports.",
        [resource.address]
    )
}

# Warn about SSH from anywhere
warn[msg] {
    resource := input.resource_changes[_]
    resource.type == "aws_security_group_rule"
    resource.change.actions[_] in ["create", "update"]

    resource.change.after.type == "ingress"
    resource.change.after.cidr_blocks[_] == "0.0.0.0/0"
    resource.change.after.from_port <= 22
    resource.change.after.to_port >= 22

    msg := sprintf(
        "Security group rule '%s' allows SSH from anywhere. Consider restricting to specific IPs.",
        [resource.address]
    )
}

# Deny unencrypted EBS volumes
deny[msg] {
    resource := input.resource_changes[_]
    resource.type == "aws_ebs_volume"
    resource.change.actions[_] == "create"

    not resource.change.after.encrypted

    msg := sprintf(
        "EBS volume '%s' must be encrypted",
        [resource.address]
    )
}
```

### 4. Cost Control

Prevent expensive resources:

```rego
# policies/cost.rego
package pmp.cost

# @description Cost control policies

import future.keywords.in

# Expensive instance types that require approval
expensive_instance_types := [
    "x2idn", "x2iedn", "x2iezn",  # Memory optimized
    "p4d", "p4de", "p5",          # GPU instances
    "dl1", "dl2q",                # Deep learning
    "u-", "mac"                   # High memory / Mac
]

# Deny large instance types in non-prod
deny[msg] {
    resource := input.resource_changes[_]
    resource.type == "aws_instance"
    resource.change.actions[_] == "create"

    env := resource.change.after.tags.Environment
    not env in ["prod", "production"]

    instance_type := resource.change.after.instance_type
    is_expensive(instance_type)

    msg := sprintf(
        "Instance '%s' uses expensive type '%s' in non-production. Use smaller instances for dev/staging.",
        [resource.address, instance_type]
    )
}

is_expensive(instance_type) {
    prefix := expensive_instance_types[_]
    startswith(instance_type, prefix)
}

# Warn about large RDS instances
warn[msg] {
    resource := input.resource_changes[_]
    resource.type == "aws_db_instance"
    resource.change.actions[_] == "create"

    class := resource.change.after.instance_class
    contains(class, "xlarge")

    msg := sprintf(
        "RDS instance '%s' uses large instance class '%s'. Verify this is necessary.",
        [resource.address, class]
    )
}

# Deny provisioned IOPS in non-prod
deny[msg] {
    resource := input.resource_changes[_]
    resource.type == "aws_ebs_volume"
    resource.change.actions[_] == "create"

    env := resource.change.after.tags.Environment
    not env in ["prod", "production"]

    resource.change.after.type == "io1"

    msg := sprintf(
        "EBS volume '%s' uses provisioned IOPS (io1) in non-production. Use gp3 instead.",
        [resource.address]
    )
}
```

### 5. Compliance

Enforce compliance requirements:

```rego
# policies/compliance.rego
package pmp.compliance

# @description Compliance and governance policies

import future.keywords.in

# Required regions (data residency)
allowed_regions := ["us-east-1", "us-west-2", "eu-west-1"]

# Deny resources in non-approved regions
deny[msg] {
    resource := input.resource_changes[_]
    resource.change.actions[_] == "create"

    # Check provider region from configuration
    provider_config := input.configuration.provider_config.aws
    region := provider_config.expressions.region.constant_value

    not region in allowed_regions

    msg := sprintf(
        "Resource '%s' deployed to non-approved region '%s'. Allowed: %v",
        [resource.address, region, allowed_regions]
    )
}

# Deny resources without backup configuration
deny[msg] {
    resource := input.resource_changes[_]
    resource.type == "aws_db_instance"
    resource.change.actions[_] == "create"

    retention := resource.change.after.backup_retention_period
    retention < 7

    msg := sprintf(
        "RDS instance '%s' must have backup retention of at least 7 days, got: %d",
        [resource.address, retention]
    )
}

# Warn about deletion protection disabled
warn[msg] {
    resource := input.resource_changes[_]
    resource.type in ["aws_db_instance", "aws_rds_cluster"]
    resource.change.actions[_] == "create"

    env := resource.change.after.tags.Environment
    env in ["prod", "production"]

    not resource.change.after.deletion_protection

    msg := sprintf(
        "Production database '%s' should have deletion protection enabled",
        [resource.address]
    )
}
```

### 6. Resource Limits

Prevent resource sprawl:

```rego
# policies/limits.rego
package pmp.limits

# @description Resource limit policies

# Maximum resources per type
resource_limits := {
    "aws_instance": 50,
    "aws_lambda_function": 100,
    "aws_s3_bucket": 20,
    "aws_rds_cluster": 5
}

# Count resources being created
count_creates_by_type[resource_type] = count {
    creates := [r |
        r := input.resource_changes[_]
        r.change.actions[_] == "create"
        r.type == resource_type
    ]
    count := count(creates)
}

# Warn when approaching limits
warn[msg] {
    resource_type := resource_limits[type]
    limit := resource_limits[type]
    count := count_creates_by_type[type]

    count > limit * 0.8  # 80% threshold

    msg := sprintf(
        "Creating %d %s resources. Approaching limit of %d.",
        [count, type, limit]
    )
}

# Deny when exceeding limits
deny[msg] {
    type := object.keys(resource_limits)[_]
    limit := resource_limits[type]
    count := count_creates_by_type[type]

    count > limit

    msg := sprintf(
        "Cannot create %d %s resources. Limit is %d.",
        [count, type, limit]
    )
}
```

## Testing Policies

### Test File Structure

Create test files with `_test.rego` suffix:

```rego
# policies/naming_test.rego
package pmp.naming

test_deny_missing_prefix {
    deny[_] with input as {
        "resource_changes": [{
            "address": "aws_instance.web",
            "change": {
                "actions": ["create"],
                "after": {
                    "name": "web-server",
                    "tags": {"Environment": "dev"}
                }
            }
        }]
    }
}

test_allow_valid_prefix {
    count(deny) == 0 with input as {
        "resource_changes": [{
            "address": "aws_instance.web",
            "change": {
                "actions": ["create"],
                "after": {
                    "name": "dev-web-server",
                    "tags": {"Environment": "dev"}
                }
            }
        }]
    }
}

test_skip_delete_actions {
    count(deny) == 0 with input as {
        "resource_changes": [{
            "address": "aws_instance.web",
            "change": {
                "actions": ["delete"],
                "after": null
            }
        }]
    }
}
```

### Run Tests

```bash
# Run all tests
pmp policy opa test

# Run tests in specific directory
pmp policy opa test --path ./policies
```

## Best Practices

### 1. Organize Policies by Domain

```
policies/
├── naming.rego           # Naming conventions
├── naming_test.rego
├── tagging.rego          # Required tags
├── tagging_test.rego
├── security.rego         # Security rules
├── security_test.rego
├── cost.rego             # Cost controls
├── cost_test.rego
└── compliance.rego       # Compliance rules
```

### 2. Use Descriptive Messages

Include context in violation messages:

```rego
# Good
msg := sprintf(
    "Resource '%s' (type: %s) missing required tag '%s'. All %s resources must be tagged.",
    [resource.address, resource.type, tag, resource.type]
)

# Bad
msg := "Missing tag"
```

### 3. Use Severity Levels Appropriately

| Rule | Severity | Use Case |
|------|----------|----------|
| `deny` | Error | Security violations, compliance requirements |
| `warn` | Warning | Best practices, recommendations |
| `info` | Info | Informational notices |

### 4. Add Policy Metadata

Use comments to document policies:

```rego
package pmp.security

# @description Security policies for AWS resources
# @author Platform Team
# @version 1.2.0

# Deny unencrypted storage
deny[msg] {
    # ...
}
```

### 5. Handle Edge Cases

Check for null values and missing attributes:

```rego
deny[msg] {
    resource := input.resource_changes[_]
    resource.change.actions[_] == "create"

    # Check if tags exist before accessing
    tags := resource.change.after.tags
    tags != null
    not tags.Environment

    msg := sprintf("Resource '%s' missing Environment tag", [resource.address])
}
```

### 6. Use Data Files for Configuration

Externalize configuration into data files:

```json
// policy-data/exemptions.json
{
    "exempt_resources": [
        "aws_instance.legacy_app",
        "aws_s3_bucket.migration_temp"
    ],
    "exempt_tags": {
        "legacy": true
    }
}
```

```rego
package pmp.tagging

import data.exemptions

deny[msg] {
    resource := input.resource_changes[_]
    not resource.address in exemptions.exempt_resources
    not resource.change.after.tags.legacy
    # ... rest of policy
}
```

### 7. Test Thoroughly

Write tests for:
- Positive cases (violations detected)
- Negative cases (valid resources pass)
- Edge cases (null values, missing fields)
- Different actions (create, update, delete)

## Remediation Annotations

PMP supports special annotations in Rego policy comments for compliance reporting and remediation guidance.

### @remediation

Description of how to fix the violation:

```rego
# @remediation Add encryption to the resource configuration
```

### @remediation-code

Code example showing the fix:

```rego
# @remediation-code encrypted = true
```

### @remediation-url

Link to documentation:

```rego
# @remediation-url https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/EBSEncryption.html
```

### @remediation-auto

Whether this can be auto-fixed (true/false):

```rego
# @remediation-auto false
```

### @compliance

Reference to compliance framework controls (format: `FRAMEWORK:CONTROL_ID Description`):

```rego
# @compliance CIS:2.2.1 Ensure EBS volume encryption is enabled
# @compliance PCI-DSS:3.4 Render PAN unreadable anywhere it is stored
```

### Complete Example with Remediation

```rego
# policies/encryption.rego
package pmp.security.encryption

# @description Ensure EBS volumes are encrypted at rest
# @remediation Add 'encrypted = true' to your aws_ebs_volume resource
# @remediation-code encrypted = true
# @remediation-url https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/EBSEncryption.html
# @remediation-auto false
# @compliance CIS:2.2.1 Ensure EBS volume encryption is enabled
# @compliance PCI-DSS:3.4 Render PAN unreadable anywhere it is stored

deny[result] {
    resource := input.resource_changes[_]
    resource.type == "aws_ebs_volume"
    resource.change.actions[_] == "create"
    not resource.change.after.encrypted

    result := {
        "msg": sprintf("EBS volume '%s' is not encrypted", [resource.address]),
        "resource": resource.address
    }
}
```

## Compliance Reports

Generate compliance reports in multiple formats with remediation guidance.

### Generate Markdown Report (default)

```bash
pmp policy opa report
```

### Generate JSON Report

```bash
pmp policy opa report --format json --output compliance.json
```

### Generate HTML Report

```bash
pmp policy opa report --format html --output compliance.html
```

### Include Passing Checks

```bash
pmp policy opa report --include-passed
```

### Report for Specific Path

```bash
pmp policy opa report --path ./projects/vpc/environments/prod
```

### Example Markdown Report

```markdown
# Compliance Report

**Generated:** 2024-01-15T10:30:00Z
**Infrastructure:** my-infrastructure
**Project:** vpc
**Environment:** production

## Summary

| Metric | Value |
|--------|-------|
| Total Checks | 15 |
| Passed | 12 |
| Failed | 3 |
| Compliance Score | 80.0% |

## Violations

### [ERROR] EBS volume 'aws_ebs_volume.data' is not encrypted

- **Policy:** pmp.security.encryption
- **Resource:** aws_ebs_volume.data
- **Compliance:** CIS 2.2.1, PCI-DSS 3.4

**Remediation:**
Add 'encrypted = true' to your aws_ebs_volume resource

\`\`\`hcl
encrypted = true
\`\`\`

[Documentation](https://docs.aws.amazon.com/AWSEC2/latest/UserGuide/EBSEncryption.html)

---

## By Framework

### CIS

| Control | Status | Description |
|---------|--------|-------------|
| 2.2.1 | FAIL | Ensure EBS volume encryption is enabled |
| 4.1.1 | PASS | Ensure security groups restrict SSH |

### PCI-DSS

| Control | Status | Description |
|---------|--------|-------------|
| 3.4 | FAIL | Render PAN unreadable anywhere it is stored |
```

## Troubleshooting

### Policy Not Found

Check policy discovery:

```bash
pmp policy opa list
```

Ensure:
- File has `.rego` extension
- File is in a discovered path
- File is not named `*_test.rego`

### Validation Errors

Enable verbose output:

```bash
pmp policy opa validate --verbose
```

### Test Failures

Run specific test file:

```bash
pmp policy opa test --path ./policies/naming_test.rego
```
