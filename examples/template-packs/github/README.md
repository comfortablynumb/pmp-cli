# GitHub Template Pack

Comprehensive GitHub repository management using OpenTofu/Terraform.

## Overview

This template pack provides a complete solution for managing GitHub repositories with all common configuration options including:

- Repository creation and configuration
- Branch protection rules
- Team and collaborator access management
- Security settings
- Merge strategies and branch management

## Prerequisites

### GitHub Authentication

You need a GitHub Personal Access Token (PAT) with appropriate permissions. The provider can be authenticated in several ways:

1. **Environment Variable** (recommended):
   ```bash
   export GITHUB_TOKEN="ghp_your_token_here"
   export GITHUB_OWNER="your-org-or-username"
   ```

2. **Provider Configuration**:
   The token can also be configured directly in the provider block (not recommended for security reasons).

### Required Token Permissions

Your GitHub token needs the following scopes:

- `repo` - Full control of private repositories
- `admin:org` - Full control of orgs and teams (if managing organization repositories)
- `delete_repo` - Delete repositories (if you plan to destroy resources)

## Templates

### Repository

Create and manage a GitHub repository with comprehensive configuration options.

**Resource Kind:** `CodeRepository`

**Key Features:**

- **Basic Settings**: Name, description, visibility (public/private/internal)
- **Repository Features**: Issues, Wiki, Projects, Discussions, Downloads
- **Initialization**: Auto-init with README, .gitignore, and license templates
- **Merge Configuration**: Control merge strategies and auto-merge behavior
- **Branch Protection**: Require reviews, status checks, enforce admins
- **Access Management**: Team and collaborator permissions
- **Security**: Vulnerability alerts and security features
- **Topics**: Repository categorization with topics/tags

## Usage Example

```bash
# Set authentication
export GITHUB_TOKEN="ghp_your_token_here"
export GITHUB_OWNER="your-organization"

# Create a new repository project
pmp create

# Follow prompts to:
# 1. Select "github" template pack
# 2. Select "repository" template
# 3. Choose environment
# 4. Configure repository options
```

## Permission Levels

When configuring team or collaborator access, use these permission levels:

- `pull` - Read-only access (clone, pull)
- `triage` - Read access + manage issues and PRs
- `push` - Read/write access (push to repository)
- `maintain` - Push access + manage repository settings (no admin access)
- `admin` - Full administrative access

## Configuration Examples

### Simple Public Repository

```yaml
repository_name: my-awesome-project
description: An awesome open source project
visibility: public
has_issues: true
has_wiki: true
auto_init: true
license_template: mit
gitignore_template: Node
```

### Private Repository with Branch Protection

```yaml
repository_name: production-app
visibility: private
enable_branch_protection: true
require_pull_request_reviews: true
required_approving_review_count: 2
require_code_owner_reviews: true
enforce_admins: true
delete_branch_on_merge: true
```

### Repository with Team Access

```yaml
repository_name: team-project
enable_team_access: true
teams: "backend-team:push,frontend-team:push,security-team:admin"
```

## Provider Version

This template uses the GitHub provider version `~> 6.0`. The provider is actively maintained and regularly updated.

## Outputs

After creating a repository, the template provides:

- Repository URLs (HTTPS and SSH)
- Clone commands
- Settings summary
- Useful `gh` CLI commands for management

## Notes

- Repository names must be unique within your organization/account
- Some features (like internal visibility) require GitHub Enterprise
- Branch protection rules apply to the default branch
- Teams can only be used with organization repositories
- Topics must be lowercase and use hyphens instead of spaces

## References

- [GitHub Provider Documentation](https://registry.terraform.io/providers/integrations/github/latest/docs)
- [GitHub API Documentation](https://docs.github.com/en/rest)
- [OpenTofu Documentation](https://opentofu.org/docs/)
