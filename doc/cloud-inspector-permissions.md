# Cloud Inspector Provider Permissions

This document describes the minimum permissions required for pmp-cloud-inspector to discover resources from each supported provider.

---

## AWS

### Required IAM Permissions

Create an IAM policy with read-only access to the services you want to inspect:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "CloudInspectorReadOnly",
      "Effect": "Allow",
      "Action": [
        "ec2:Describe*",
        "s3:GetBucket*",
        "s3:ListBucket*",
        "s3:ListAllMyBuckets",
        "rds:Describe*",
        "lambda:List*",
        "lambda:GetFunction",
        "iam:List*",
        "iam:GetRole",
        "iam:GetUser",
        "iam:GetPolicy",
        "ecs:Describe*",
        "ecs:List*",
        "eks:Describe*",
        "eks:List*",
        "dynamodb:Describe*",
        "dynamodb:List*",
        "sns:List*",
        "sns:GetTopicAttributes",
        "sqs:List*",
        "sqs:GetQueueAttributes",
        "secretsmanager:List*",
        "secretsmanager:DescribeSecret",
        "logs:Describe*",
        "cloudwatch:Describe*",
        "states:List*",
        "states:DescribeStateMachine",
        "elasticache:Describe*",
        "cloudfront:List*",
        "cloudfront:GetDistribution",
        "apigateway:GET",
        "elasticloadbalancing:Describe*",
        "ecr:Describe*",
        "ecr:List*",
        "tag:GetResources",
        "pricing:GetProducts"
      ],
      "Resource": "*"
    }
  ]
}
```

### Credential Setup

Configure AWS credentials using one of:

1. **Environment variables:**
   ```bash
   export AWS_ACCESS_KEY_ID="your-access-key"
   export AWS_SECRET_ACCESS_KEY="your-secret-key"
   export AWS_REGION="us-east-1"
   ```

2. **AWS credentials file** (`~/.aws/credentials`):
   ```ini
   [default]
   aws_access_key_id = your-access-key
   aws_secret_access_key = your-secret-key
   ```

3. **IAM Instance Profile** (recommended for EC2/ECS)

4. **AWS SSO:**
   ```bash
   aws sso login --profile your-profile
   export AWS_PROFILE="your-profile"
   ```

### Minimum Permissions by Resource Type

| Resource Type | Required Actions |
|--------------|------------------|
| EC2 Instances | `ec2:DescribeInstances` |
| VPCs | `ec2:DescribeVpcs` |
| Subnets | `ec2:DescribeSubnets` |
| Security Groups | `ec2:DescribeSecurityGroups` |
| S3 Buckets | `s3:ListAllMyBuckets`, `s3:GetBucketLocation`, `s3:GetBucketTagging` |
| RDS Instances | `rds:DescribeDBInstances` |
| RDS Clusters | `rds:DescribeDBClusters` |
| Lambda Functions | `lambda:ListFunctions`, `lambda:GetFunction` |
| IAM Users | `iam:ListUsers`, `iam:GetUser` |
| IAM Roles | `iam:ListRoles`, `iam:GetRole` |
| ECS Clusters | `ecs:ListClusters`, `ecs:DescribeClusters` |
| ECS Services | `ecs:ListServices`, `ecs:DescribeServices` |
| EKS Clusters | `eks:ListClusters`, `eks:DescribeCluster` |
| DynamoDB Tables | `dynamodb:ListTables`, `dynamodb:DescribeTable` |
| SNS Topics | `sns:ListTopics`, `sns:GetTopicAttributes` |
| SQS Queues | `sqs:ListQueues`, `sqs:GetQueueAttributes` |
| Secrets Manager | `secretsmanager:ListSecrets`, `secretsmanager:DescribeSecret` |
| CloudWatch Alarms | `cloudwatch:DescribeAlarms` |
| CloudWatch Logs | `logs:DescribeLogGroups` |
| Step Functions | `states:ListStateMachines`, `states:DescribeStateMachine` |
| ElastiCache | `elasticache:DescribeCacheClusters` |
| CloudFront | `cloudfront:ListDistributions`, `cloudfront:GetDistribution` |
| API Gateway | `apigateway:GET` |
| ELB/ALB/NLB | `elasticloadbalancing:DescribeLoadBalancers` |
| ECR Repositories | `ecr:DescribeRepositories` |

---

## Azure

### Required RBAC Role

Assign the **Reader** role at the subscription level, or create a custom role:

```json
{
  "Name": "Cloud Inspector Reader",
  "IsCustom": true,
  "Description": "Read-only access for cloud resource inspection",
  "Actions": [
    "Microsoft.Resources/subscriptions/resourceGroups/read",
    "Microsoft.Compute/virtualMachines/read",
    "Microsoft.Network/virtualNetworks/read",
    "Microsoft.Network/virtualNetworks/subnets/read",
    "Microsoft.Storage/storageAccounts/read",
    "Microsoft.Storage/storageAccounts/listKeys/action",
    "Microsoft.Web/sites/read",
    "Microsoft.Sql/servers/read",
    "Microsoft.Sql/servers/databases/read",
    "Microsoft.KeyVault/vaults/read",
    "Microsoft.ContainerService/managedClusters/read",
    "Microsoft.ContainerRegistry/registries/read"
  ],
  "NotActions": [],
  "AssignableScopes": [
    "/subscriptions/{subscription-id}"
  ]
}
```

### Credential Setup

1. **Service Principal (recommended for automation):**
   ```bash
   # Create service principal
   az ad sp create-for-rbac --name "cloud-inspector" --role "Reader" \
     --scopes /subscriptions/{subscription-id}

   # Set environment variables
   export AZURE_CLIENT_ID="app-id"
   export AZURE_CLIENT_SECRET="password"
   export AZURE_TENANT_ID="tenant-id"
   export AZURE_SUBSCRIPTION_ID="subscription-id"
   ```

2. **Azure CLI login:**
   ```bash
   az login
   az account set --subscription "subscription-id"
   ```

3. **Managed Identity** (recommended for Azure VMs/Functions)

### Minimum Permissions by Resource Type

| Resource Type | Required Actions |
|--------------|------------------|
| Resource Groups | `Microsoft.Resources/subscriptions/resourceGroups/read` |
| Virtual Machines | `Microsoft.Compute/virtualMachines/read` |
| Virtual Networks | `Microsoft.Network/virtualNetworks/read` |
| Subnets | `Microsoft.Network/virtualNetworks/subnets/read` |
| Storage Accounts | `Microsoft.Storage/storageAccounts/read` |
| App Services | `Microsoft.Web/sites/read` |
| SQL Databases | `Microsoft.Sql/servers/databases/read` |
| Key Vaults | `Microsoft.KeyVault/vaults/read` |
| AKS Clusters | `Microsoft.ContainerService/managedClusters/read` |
| Container Registries | `Microsoft.ContainerRegistry/registries/read` |

---

## GCP

### Required IAM Roles

Assign the **Viewer** role at project level, or use these predefined roles:

- `roles/viewer` (basic read access to all resources)
- Or use specific roles:
  - `roles/compute.viewer`
  - `roles/storage.objectViewer`
  - `roles/cloudfunctions.viewer`
  - `roles/run.viewer`
  - `roles/iam.securityReviewer`

### Custom Role Permissions

```yaml
title: "Cloud Inspector Reader"
description: "Read-only access for cloud resource inspection"
includedPermissions:
  - compute.instances.list
  - compute.instances.get
  - compute.networks.list
  - compute.networks.get
  - compute.subnetworks.list
  - compute.subnetworks.get
  - storage.buckets.list
  - storage.buckets.get
  - cloudfunctions.functions.list
  - cloudfunctions.functions.get
  - run.services.list
  - run.services.get
  - iam.serviceAccounts.list
  - resourcemanager.projects.get
```

### Credential Setup

1. **Service Account (recommended):**
   ```bash
   # Create service account
   gcloud iam service-accounts create cloud-inspector \
     --display-name="Cloud Inspector"

   # Grant viewer role
   gcloud projects add-iam-policy-binding PROJECT_ID \
     --member="serviceAccount:cloud-inspector@PROJECT_ID.iam.gserviceaccount.com" \
     --role="roles/viewer"

   # Create key file
   gcloud iam service-accounts keys create key.json \
     --iam-account=cloud-inspector@PROJECT_ID.iam.gserviceaccount.com

   # Set environment variable
   export GOOGLE_APPLICATION_CREDENTIALS="/path/to/key.json"
   ```

2. **User credentials:**
   ```bash
   gcloud auth application-default login
   ```

3. **Workload Identity** (recommended for GKE)

### Minimum Permissions by Resource Type

| Resource Type | Required Permissions |
|--------------|---------------------|
| Compute Instances | `compute.instances.list`, `compute.instances.get` |
| VPC Networks | `compute.networks.list`, `compute.networks.get` |
| Subnetworks | `compute.subnetworks.list`, `compute.subnetworks.get` |
| Cloud Storage | `storage.buckets.list`, `storage.buckets.get` |
| Cloud Functions | `cloudfunctions.functions.list`, `cloudfunctions.functions.get` |
| Cloud Run | `run.services.list`, `run.services.get` |

---

## GitHub

### Required Scopes

Create a Personal Access Token (PAT) or GitHub App with these scopes:

| Scope | Purpose |
|-------|---------|
| `repo` | Access private repositories (if needed) |
| `read:org` | Read organization membership and teams |
| `admin:org` (read only) | Read organization settings |
| `read:user` | Read user profile information |

### Token Setup

1. **Personal Access Token (Classic):**
   - Go to GitHub → Settings → Developer settings → Personal access tokens → Tokens (classic)
   - Generate new token with required scopes
   ```bash
   export GITHUB_TOKEN="ghp_xxxxxxxxxxxx"
   ```

2. **Fine-grained Personal Access Token (recommended):**
   - Go to GitHub → Settings → Developer settings → Personal access tokens → Fine-grained tokens
   - Select organization/repositories
   - Grant read-only permissions:
     - Repository permissions: Contents (Read), Metadata (Read)
     - Organization permissions: Members (Read), Administration (Read)

3. **GitHub App:**
   - Create a GitHub App with required permissions
   - Install on organization
   - Generate installation access token

### Minimum Permissions by Resource Type

| Resource Type | Required Scope |
|--------------|---------------|
| Repositories | `repo` or `public_repo` |
| Teams | `read:org` |
| Organization Members | `read:org` |
| Organization Settings | `admin:org` (read) |

---

## GitLab

### Required Scopes

Create a Personal Access Token or Group/Project Access Token with:

| Scope | Purpose |
|-------|---------|
| `read_api` | Read access to the API |
| `read_user` | Read user information |
| `read_repository` | Read repository contents |

### Token Setup

1. **Personal Access Token:**
   - Go to GitLab → User Settings → Access Tokens
   - Create token with required scopes
   ```bash
   export GITLAB_TOKEN="glpat-xxxxxxxxxxxx"
   export GITLAB_URL="https://gitlab.com"  # or your self-hosted URL
   ```

2. **Group Access Token** (for group-level access):
   - Go to Group → Settings → Access Tokens
   - Create token with `read_api` scope

3. **Project Access Token** (for single project):
   - Go to Project → Settings → Access Tokens
   - Create token with `read_api` scope

### Minimum Permissions by Resource Type

| Resource Type | Required Scope |
|--------------|---------------|
| Projects | `read_api` |
| Groups | `read_api` |
| Users | `read_user`, `read_api` |

---

## JFrog Artifactory

### Required Permissions

Create a user or access token with read permissions:

| Permission | Purpose |
|-----------|---------|
| Read Repositories | List and read repository configurations |
| Read Users | List users (admin only) |
| Read Groups | List groups (admin only) |
| Read Permissions | List permission targets (admin only) |

### Token Setup

1. **Access Token:**
   ```bash
   export ARTIFACTORY_URL="https://your-instance.jfrog.io"
   export ARTIFACTORY_ACCESS_TOKEN="your-access-token"
   ```

2. **API Key (legacy):**
   ```bash
   export ARTIFACTORY_URL="https://your-instance.jfrog.io"
   export ARTIFACTORY_API_KEY="your-api-key"
   ```

3. **Username/Password:**
   ```bash
   export ARTIFACTORY_URL="https://your-instance.jfrog.io"
   export ARTIFACTORY_USERNAME="your-username"
   export ARTIFACTORY_PASSWORD="your-password"
   ```

### Minimum Permissions by Resource Type

| Resource Type | Required Permission |
|--------------|-------------------|
| Repositories | Read on `Any Repository` |
| Users | Admin: Manage Users |
| Groups | Admin: Manage Groups |
| Permission Targets | Admin: Manage Permissions |

---

## Okta

### Required Scopes

Create an API token or OAuth 2.0 application with:

| Scope | Purpose |
|-------|---------|
| `okta.users.read` | Read user information |
| `okta.groups.read` | Read group information |
| `okta.apps.read` | Read application configurations |
| `okta.authorizationServers.read` | Read authorization servers |

### Token Setup

1. **API Token:**
   - Go to Okta Admin → Security → API → Tokens
   - Create new token
   ```bash
   export OKTA_ORG_URL="https://your-org.okta.com"
   export OKTA_API_TOKEN="your-api-token"
   ```

2. **OAuth 2.0 Application:**
   - Create an OAuth 2.0 app with required scopes
   - Use client credentials flow
   ```bash
   export OKTA_ORG_URL="https://your-org.okta.com"
   export OKTA_CLIENT_ID="your-client-id"
   export OKTA_CLIENT_SECRET="your-client-secret"
   ```

### Minimum Permissions by Resource Type

| Resource Type | Required Scope |
|--------------|---------------|
| Users | `okta.users.read` |
| Groups | `okta.groups.read` |
| Applications | `okta.apps.read` |
| Authorization Servers | `okta.authorizationServers.read` |

---

## Auth0

### Required Scopes

Create a Machine-to-Machine application with these scopes:

| Scope | Purpose |
|-------|---------|
| `read:users` | Read user information |
| `read:roles` | Read role information |
| `read:clients` | Read application/client information |
| `read:connections` | Read connection configurations |
| `read:resource_servers` | Read API/resource server configurations |

### Token Setup

1. **Machine-to-Machine Application:**
   - Go to Auth0 Dashboard → Applications → Create Application
   - Select "Machine to Machine Applications"
   - Authorize for Auth0 Management API with required scopes
   ```bash
   export AUTH0_DOMAIN="your-tenant.auth0.com"
   export AUTH0_CLIENT_ID="your-client-id"
   export AUTH0_CLIENT_SECRET="your-client-secret"
   ```

### Minimum Permissions by Resource Type

| Resource Type | Required Scope |
|--------------|---------------|
| Users | `read:users` |
| Roles | `read:roles` |
| Clients (Applications) | `read:clients` |
| Connections | `read:connections` |
| Resource Servers (APIs) | `read:resource_servers` |

---

## Jira

### Required Permissions

Use an API token with project read access:

| Permission | Purpose |
|-----------|---------|
| Browse Projects | View project details |
| View Read-Only Workflow | View project workflows |

### Token Setup

1. **API Token:**
   - Go to Atlassian Account → Security → API tokens
   - Create new token
   ```bash
   export JIRA_URL="https://your-domain.atlassian.net"
   export JIRA_EMAIL="your-email@example.com"
   export JIRA_API_TOKEN="your-api-token"
   ```

### Minimum Permissions by Resource Type

| Resource Type | Required Permission |
|--------------|-------------------|
| Projects | Browse Projects |

**Note:** Jira does not have a stable Terraform provider. Resources discovered from Jira will generate warnings during import.

---

## Opsgenie

### Required Permissions

Create an API key with read access:

| Permission | Purpose |
|-----------|---------|
| Read | Read alert policies and configurations |
| Configuration Access | Read team and integration configurations |

### Token Setup

1. **API Key:**
   - Go to Opsgenie → Settings → API key management
   - Create new API key with read access
   ```bash
   export OPSGENIE_API_KEY="your-api-key"
   # For EU region:
   export OPSGENIE_API_URL="https://api.eu.opsgenie.com"
   ```

### Minimum Permissions by Resource Type

| Resource Type | Required Permission |
|--------------|-------------------|
| Alert Policies | Read, Configuration Access |

---

## Security Best Practices

1. **Use read-only permissions** - Never grant write/modify permissions for inspection
2. **Use service accounts** - Avoid using personal credentials
3. **Rotate credentials regularly** - Set up credential rotation policies
4. **Use short-lived tokens** - Prefer temporary credentials over long-lived keys
5. **Restrict scope** - Only grant access to the specific resources needed
6. **Audit access** - Enable logging to track API usage
7. **Use secrets management** - Store credentials in a secrets manager, not in plain text
