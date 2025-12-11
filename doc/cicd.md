# CI/CD Generation

PMP generates dependency-aware CI/CD pipelines for GitHub Actions, GitLab CI, and Jenkins.

## Supported Platforms

| Platform | Command | Output |
|----------|---------|--------|
| GitHub Actions | `github-actions` or `github` | `.github/workflows/deploy.yml` |
| GitLab CI | `gitlab-ci` or `gitlab` | `.gitlab-ci.yml` |
| Jenkins | `jenkins` | `Jenkinsfile` |

## Generate Pipeline

```bash
# GitHub Actions
pmp ci generate github-actions
pmp ci generate github-actions --output .github/workflows/deploy.yml

# GitLab CI
pmp ci generate gitlab-ci
pmp ci generate gitlab-ci --output .gitlab-ci.yml

# Jenkins
pmp ci generate jenkins
pmp ci generate jenkins --output Jenkinsfile

# For specific environment
pmp ci generate github-actions --environment prod
```

## Pipeline Modes

### Dynamic Pipeline (Default)

Only deploys projects with changed files:

```bash
pmp ci generate github-actions
```

**Features:**
- Change detection using git diff
- Deploys only modified projects
- Faster CI runs
- Respects dependency order

### Static Pipeline

Deploys all projects every time:

```bash
pmp ci generate github-actions --static
```

**Use cases:**
- Scheduled deployments
- Full environment rebuilds
- When change detection is unreliable

## Change Detection

PMP includes a change detection command for dynamic pipelines:

```bash
pmp ci detect-changes --base origin/main --head HEAD

# For specific environment
pmp ci detect-changes --base origin/main --head HEAD --environment prod

# Output format
pmp ci detect-changes --base origin/main --head HEAD --output-format json
pmp ci detect-changes --base origin/main --head HEAD --output-format yaml
```

**Exit Codes:**
- `0` - Success, changed projects found
- `1` - No projects changed
- `2` - Infrastructure file changed (skip project-level CI)

**Output (JSON):**
```json
{
  "changed_projects": [
    {
      "name": "my-api",
      "environment": "dev",
      "path": "projects/my-api/environments/dev"
    }
  ],
  "infrastructure_changed": false
}
```

## Generated Pipeline Examples

### GitHub Actions

```yaml
name: Deploy Infrastructure

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  PMP_VERSION: "latest"

jobs:
  detect-changes:
    runs-on: ubuntu-latest
    outputs:
      changed_projects: ${{ steps.detect.outputs.changed_projects }}
      has_changes: ${{ steps.detect.outputs.has_changes }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Detect changed projects
        id: detect
        run: |
          if pmp ci detect-changes --base origin/main --head HEAD > changes.json; then
            echo "changed_projects=$(cat changes.json)" >> $GITHUB_OUTPUT
            echo "has_changes=true" >> $GITHUB_OUTPUT
          else
            echo "has_changes=false" >> $GITHUB_OUTPUT
          fi

  deploy:
    needs: detect-changes
    if: needs.detect-changes.outputs.has_changes == 'true'
    runs-on: ubuntu-latest
    strategy:
      matrix:
        project: ${{ fromJson(needs.detect-changes.outputs.changed_projects) }}
    steps:
      - uses: actions/checkout@v4

      - name: Setup OpenTofu
        uses: opentofu/setup-opentofu@v1

      - name: Preview
        if: github.event_name == 'pull_request'
        run: |
          cd ${{ matrix.project.path }}
          pmp project preview

      - name: Apply
        if: github.event_name == 'push'
        run: |
          cd ${{ matrix.project.path }}
          pmp project apply -- -auto-approve
```

### GitLab CI

```yaml
stages:
  - detect
  - plan
  - apply

variables:
  PMP_VERSION: "latest"

detect-changes:
  stage: detect
  script:
    - pmp ci detect-changes --base $CI_MERGE_REQUEST_TARGET_BRANCH_NAME --head $CI_COMMIT_SHA > changes.json
  artifacts:
    paths:
      - changes.json
    expire_in: 1 hour
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH

plan:
  stage: plan
  script:
    - |
      for project in $(cat changes.json | jq -r '.[].path'); do
        cd $project
        pmp project preview
        cd -
      done
  rules:
    - if: $CI_PIPELINE_SOURCE == "merge_request_event"
  needs: [detect-changes]

apply:
  stage: apply
  script:
    - |
      for project in $(cat changes.json | jq -r '.[].path'); do
        cd $project
        pmp project apply -- -auto-approve
        cd -
      done
  rules:
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
  needs: [detect-changes]
  when: manual
```

### Jenkins

```groovy
pipeline {
    agent any

    environment {
        PMP_VERSION = 'latest'
    }

    stages {
        stage('Detect Changes') {
            steps {
                script {
                    sh 'pmp ci detect-changes --base origin/main --head HEAD > changes.json'
                    def changes = readJSON file: 'changes.json'
                    env.CHANGED_PROJECTS = changes.collect { it.path }.join(',')
                }
            }
        }

        stage('Preview') {
            when {
                changeRequest()
            }
            steps {
                script {
                    env.CHANGED_PROJECTS.split(',').each { path ->
                        dir(path) {
                            sh 'pmp project preview'
                        }
                    }
                }
            }
        }

        stage('Apply') {
            when {
                branch 'main'
            }
            steps {
                script {
                    env.CHANGED_PROJECTS.split(',').each { path ->
                        dir(path) {
                            sh 'pmp project apply -- -auto-approve'
                        }
                    }
                }
            }
        }
    }
}
```

## Dependency-Aware Execution

Generated pipelines respect dependency order:

1. Projects are grouped into deployment stages
2. Each stage contains projects that can run in parallel
3. Dependent projects wait for their dependencies

```
Stage 1: vpc, monitoring (parallel)
Stage 2: postgres-db, redis (parallel, after stage 1)
Stage 3: my-api (after stage 2)
```

## Pipeline Features

| Feature | GitHub Actions | GitLab CI | Jenkins |
|---------|----------------|-----------|---------|
| Change detection | ✓ | ✓ | ✓ |
| Parallel execution | ✓ | ✓ | ✓ |
| Dependency ordering | ✓ | ✓ | ✓ |
| PR previews | ✓ | ✓ | ✓ |
| Manual approval | ✓ | ✓ | ✓ |
| Environment secrets | ✓ | ✓ | ✓ |

## Best Practices

1. **Use dynamic pipelines** - Only deploy what changed
2. **Preview on PRs** - Catch issues before merge
3. **Manual apply for prod** - Require approval for production
4. **Store state remotely** - Use S3, Azure, or GCS backends
5. **Rotate credentials** - Use short-lived tokens when possible
