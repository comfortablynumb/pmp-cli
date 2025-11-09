# AWS Template Pack

Comprehensive AWS infrastructure management templates using OpenTofu/Terraform.

## Overview

This template pack provides solutions for managing AWS resources with best practices for security, cost optimization, and operational excellence.

## Prerequisites

### AWS Authentication

You need AWS credentials configured. The provider can be authenticated in several ways:

1. **Environment Variables** (recommended):
   ```bash
   export AWS_ACCESS_KEY_ID="your-access-key"
   export AWS_SECRET_ACCESS_KEY="your-secret-key"
   export AWS_DEFAULT_REGION="us-east-1"
   ```

2. **AWS CLI Configuration**:
   ```bash
   aws configure
   ```

3. **IAM Role** (when running on AWS infrastructure like EC2, ECS, Lambda)

### Required AWS Permissions

Your AWS credentials need appropriate permissions based on the template you're using. For ECR, you'll need:

- `ecr:CreateRepository`
- `ecr:DescribeRepositories`
- `ecr:DeleteRepository`
- `ecr:PutImageScanningConfiguration`
- `ecr:PutImageTagMutability`
- `ecr:SetRepositoryPolicy`
- `ecr:PutLifecyclePolicy`
- `kms:CreateKey` (if using KMS encryption)
- `kms:DescribeKey`
- `kms:EnableKeyRotation`

## Templates

### ECR Template (Elastic Container Registry)

Create and manage AWS ECR repositories for storing Docker container images.

**Resource Kind:** `ContainerRegistry`

**Use When:** You need a standalone ECR repository with full control over all configuration options.

**Key Features:**

- **Repository Management**: Create private container registries
- **Encryption**: Support for both AWS-managed (AES256) and customer-managed (KMS) encryption
- **Image Scanning**: Automatic vulnerability scanning on push
- **Lifecycle Policies**: Automated image cleanup to reduce storage costs
- **Tag Mutability**: Control whether tags can be overwritten (MUTABLE/IMMUTABLE)
- **Cross-Account Access**: Configure repository policies for multi-account access
- **Security Best Practices**: Scanning enabled, encryption by default

## Plugins

### ECR Plugin

Add an AWS ECR repository to your application project with opinionated defaults.

**Role:** `container-registry`

**Use When:** You want to add container image storage to an existing project (like a Kubernetes workload or ECS service).

**Key Features:**

- **Convention over Configuration**: Automatically uses project name for repository
- **Simplified Setup**: Fewer configuration options, sensible defaults for apps
- **Security-First**: Scanning enabled, encryption by default
- **Auto-Cleanup**: Lifecycle policies pre-configured for app images
- **Quick Integration**: Minimal prompts for faster setup

**Key Differences from Template:**

| Feature | Template | Plugin |
|---------|----------|--------|
| Repository Name | Manually specified | Auto-uses project name (override available) |
| Configuration Options | 18 options | 8 simplified options |
| Use Case | Standalone repository | Attached to application project |
| Repository Policies | Configurable cross-account access | Disabled (app-focused) |
| Lifecycle Rules | Fully customizable | Opinionated for app containers (v*, prod*, staging*, dev*) |

**Example Plugin Configuration:**

```yaml
# Minimal configuration - uses project name
image_tag_mutability: IMMUTABLE
encryption_type: KMS
scan_on_push: true
keep_image_count: 20
```

## Usage Example

```bash
# Set AWS credentials
export AWS_ACCESS_KEY_ID="your-access-key"
export AWS_SECRET_ACCESS_KEY="your-secret-key"
export AWS_DEFAULT_REGION="us-east-1"

# Create a new ECR repository
pmp create

# Follow prompts to:
# 1. Select "aws" template pack
# 2. Select "ecr" template
# 3. Choose environment
# 4. Configure repository options
```

## ECR Configuration Examples

### Simple ECR Repository

```yaml
repository_name: my-app
scan_on_push: true
image_tag_mutability: MUTABLE
encryption_type: AES256
```

### Production ECR Repository with KMS Encryption

```yaml
repository_name: production-api
image_tag_mutability: IMMUTABLE
encryption_type: KMS
enable_kms_key_rotation: true
scan_on_push: true
enable_lifecycle_policy: true
keep_image_count: 20
```

### ECR with Cross-Account Access

```yaml
repository_name: shared-images
enable_repository_policy: true
allowed_aws_account_ids: "123456789012,987654321098"
scan_on_push: true
```

## Image Tag Mutability

**MUTABLE** (default):
- Tags can be reassigned to different images
- Useful for development environments
- Allows overwriting tags like `latest`

**IMMUTABLE**:
- Once a tag is assigned, it cannot be changed
- Recommended for production environments
- Prevents accidental tag overwrites
- Ensures reproducible deployments

## Lifecycle Policies

Lifecycle policies help manage costs by automatically removing old or unused images:

- **Tagged Images**: Keep only the N most recent images with specific tag prefixes
- **Untagged Images**: Remove untagged images after N days
- **Custom Rules**: Combine multiple rules for fine-grained control

## Encryption Options

**AES256** (AWS-managed):
- Default encryption at rest
- No additional configuration required
- AWS manages encryption keys

**KMS** (Customer-managed):
- Use your own KMS keys
- Enhanced control and auditing
- Supports key rotation
- Required for compliance scenarios

## Docker Commands

After creating an ECR repository, use these commands:

```bash
# Get login password and authenticate Docker
aws ecr get-login-password --region us-east-1 | \
  docker login --username AWS --password-stdin <account_id>.dkr.ecr.us-east-1.amazonaws.com

# Tag your image
docker tag my-app:latest <account_id>.dkr.ecr.us-east-1.amazonaws.com/my-app:latest

# Push image to ECR
docker push <account_id>.dkr.ecr.us-east-1.amazonaws.com/my-app:latest

# Pull image from ECR
docker pull <account_id>.dkr.ecr.us-east-1.amazonaws.com/my-app:latest
```

## Security Best Practices

1. **Enable Image Scanning**: Detect vulnerabilities before deployment
2. **Use Immutable Tags**: Prevent tag overwrites in production
3. **Enable KMS Encryption**: Use customer-managed keys for sensitive workloads
4. **Implement Lifecycle Policies**: Reduce attack surface by removing old images
5. **Restrict Access**: Use repository policies to limit access to authorized accounts
6. **Enable Key Rotation**: Automatically rotate KMS keys annually
7. **Tag Resources**: Apply consistent tagging for cost allocation and governance

## Cost Optimization

- Enable lifecycle policies to remove unused images
- Set appropriate retention periods based on deployment frequency
- Monitor storage usage with CloudWatch metrics
- Consider using ECR Public for open-source images

## Provider Version

This template uses the AWS provider version `~> 5.0`. The provider is actively maintained and regularly updated.

## Outputs

After creating an ECR repository, the template provides:

- Repository URL and ARN
- Docker login commands
- Push/pull examples
- AWS CLI commands for repository management

## Notes

- Repository names must be unique within an AWS account and region
- Encryption type cannot be changed after repository creation
- KMS keys must exist in the same region as the repository
- Cross-region replication requires additional configuration
- Maximum repository name length is 256 characters
- Repository names can include lowercase letters, numbers, hyphens, underscores, and forward slashes

## References

- [AWS ECR Documentation](https://docs.aws.amazon.com/ecr/)
- [AWS Provider Documentation](https://registry.terraform.io/providers/hashicorp/aws/latest/docs)
- [ECR Best Practices](https://docs.aws.amazon.com/AmazonECR/latest/userguide/best-practices.html)
- [OpenTofu Documentation](https://opentofu.org/docs/)
