# Hooks

Hooks execute custom workflows before and after PMP operations. They enable validation, confirmation, credential collection, and automation.

## Hook Levels

Hooks can be defined at three levels, executed in order:

1. **Infrastructure** (`.pmp.infrastructure.yaml`) - All projects
2. **Template** (`.pmp.template.yaml`) - Copied to generated projects
3. **Environment** (`.pmp.environment.yaml`) - Project-specific

## Hook Phases

| Phase | Description |
|-------|-------------|
| `pre_preview` | Before plan operation |
| `post_preview` | After plan operation |
| `pre_apply` | Before apply operation |
| `post_apply` | After apply operation |
| `pre_destroy` | Before destroy operation |
| `post_destroy` | After destroy operation |
| `pre_refresh` | Before refresh operation |
| `post_refresh` | After refresh operation |
| `pre_test` | Before test operation |
| `post_test` | After test operation |

## Hook Types

### Confirm Hook

Prompt for yes/no confirmation:

```yaml
hooks:
  pre_destroy:
    - type: confirm
      config:
        question: "Destroy database? All data will be LOST!"
        exit_on_cancel: true   # Stop if No (default: true)
        exit_on_confirm: false # Stop if Yes (default: false)
```

**Behavior:**
- Default answer: No
- `exit_on_cancel: true` - Stop execution when user answers No
- `exit_on_confirm: true` - Stop execution when user answers Yes (rare use case)

**Use cases:**
- Prevent accidental destruction
- Double-confirmation for critical operations
- Pre-flight checks

### Set Environment Hook

Collect input and set environment variables:

```yaml
hooks:
  pre_apply:
    - type: set_environment
      config:
        name: AWS_ACCESS_KEY_ID
        prompt: "AWS Access Key:"
        sensitive: false  # Show input (default)

    - type: set_environment
      config:
        name: AWS_SECRET_ACCESS_KEY
        prompt: "AWS Secret Key:"
        sensitive: true  # Hide input
```

**Features:**
- **Smart defaults**: Uses existing env var value as default (non-sensitive only)
- **Security**: Sensitive inputs never show defaults
- **Convenience**: Press Enter to keep current value

```bash
# If AWS_REGION is already set:
$ export AWS_REGION=us-west-2
$ pmp project apply
# Prompt shows: AWS region: [us-west-2]
```

**Use cases:**
- Cloud credentials (AWS, Azure, GCP)
- Terraform variables (`TF_VAR_*`)
- API keys and tokens
- Database passwords

### Command Hook

Execute shell commands:

```yaml
hooks:
  pre_apply:
    - type: command
      config:
        command: "aws sts get-caller-identity"

  post_apply:
    - type: command
      config:
        command: "curl -X POST https://webhook.example.com/deploy"
```

**Execution:**
- Commands run in the environment directory
- Exit code 0 = success, non-zero = failure (stops execution)
- stdout/stderr displayed to user

**Use cases:**
- Validation scripts
- Security scanning
- Notifications (Slack, email)
- Logging deployments
- Pre-flight checks

## Examples

### Production Safety

```yaml
hooks:
  pre_apply:
    # 1. Confirm deployment
    - type: confirm
      config:
        question: "Deploy to production?"
        exit_on_cancel: true

    # 2. Collect credentials
    - type: set_environment
      config:
        name: AWS_ACCESS_KEY_ID
        prompt: "AWS Access Key:"

    - type: set_environment
      config:
        name: AWS_SECRET_ACCESS_KEY
        prompt: "AWS Secret Key:"
        sensitive: true

    # 3. Validate credentials
    - type: command
      config:
        command: "aws sts get-caller-identity"

    # 4. Run security scan
    - type: command
      config:
        command: "tfsec ."

  post_apply:
    # Notify on completion
    - type: command
      config:
        command: "curl -X POST $WEBHOOK_URL -d '{\"status\": \"deployed\"}'"
```

### Database Protection

```yaml
hooks:
  pre_destroy:
    # Double confirmation for database destruction
    - type: confirm
      config:
        question: "⚠️  DANGER: Destroy database? ALL DATA WILL BE LOST!"
        exit_on_cancel: true

    - type: confirm
      config:
        question: "Are you ABSOLUTELY sure? Type 'yes' to confirm:"
        exit_on_cancel: true

    # Create backup before destroy
    - type: command
      config:
        command: "pg_dump -h $DB_HOST -U $DB_USER $DB_NAME > backup_$(date +%Y%m%d_%H%M%S).sql"
```

### Template Hooks

Define hooks in templates to enforce patterns across all projects:

```yaml
# .pmp.template.yaml
spec:
  hooks:
    pre_apply:
      - type: set_environment
        config:
          name: DB_PASSWORD
          prompt: "Database password:"
          sensitive: true

    pre_destroy:
      - type: confirm
        config:
          question: "Destroy this resource?"
          exit_on_cancel: true
```

When a project is created from this template, hooks are copied to `.pmp.environment.yaml`.

### Infrastructure-Level Hooks

Apply hooks to all projects in an infrastructure:

```yaml
# .pmp.infrastructure.yaml
spec:
  hooks:
    pre_apply:
      - type: confirm
        config:
          question: "Apply changes?"
          exit_on_cancel: true

    post_apply:
      - type: command
        config:
          command: "echo 'Deployment completed at $(date)' >> /var/log/deployments.log"
```

## Hook Merging

When hooks are defined at multiple levels, they are merged in execution order:

1. Infrastructure hooks (first)
2. Template hooks (second)
3. Environment hooks (third)

Within each level, hooks execute in array order.

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Confirm cancelled | Execution stops (if `exit_on_cancel: true`) |
| Command fails (exit != 0) | Execution stops |
| Environment var empty | Prompt repeats |

## Variables in Hooks

Command hooks can use environment variables:

```yaml
hooks:
  post_apply:
    - type: command
      config:
        command: "curl -X POST $SLACK_WEBHOOK -d '{\"text\": \"Deployed $PROJECT_NAME\"}'"
```

Available variables:
- All system environment variables
- Variables set by `set_environment` hooks
- Terraform variables (`TF_VAR_*`)
