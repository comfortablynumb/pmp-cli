# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

PMP (Poor Man's Platform) is a CLI for managing Infrastructure as Code projects using OpenTofu/Terraform with template-based project generation. It uses Kubernetes-style resource definitions (apiVersion, kind, metadata, spec) for all configuration files.

## Key Design Principles

1. **Templates use `.pmp.template.yaml`** - Template metadata must be in a file named `.pmp.template.yaml` with `apiVersion: pmp.io/v1` and `kind: Template`
2. **Projects use `.pmp.project.yaml`** - Generated project identifier is in `.pmp.project.yaml` with `apiVersion: pmp.io/v1` and `kind: Project` (metadata only, no spec)
3. **Environments use `.pmp.environment.yaml`** - Each environment has its own file with `apiVersion: pmp.io/v1` and `kind: ProjectEnvironment` containing the spec
4. **A Project has a Resource associated with its own apiVersion and kind** - Resources may define inputs by environment
5. **Projects are organized by environment** - Each project has an `environments/` directory containing one subdirectory per environment

## Development Commands

```bash
cargo build                              # Build the project
cargo build --release                    # Build release
cargo test                               # Run tests
cargo run -- create                      # Create project
cargo run -- find --name foo             # Find projects
cargo run -- graph                       # Visualize dependency graph
cargo run -- deps analyze                # Analyze dependencies
cargo run -- deps impact my-api          # Show impact of changes
cargo run -- state list                  # List state across projects
cargo run -- state drift                 # Detect configuration drift
cargo run -- state lock my-project       # Lock project state
cargo run -- ci generate github          # Generate GitHub Actions workflow
cargo run -- template scaffold           # Create new template pack interactively
cargo run -- clone my-api new-api        # Clone existing project
cargo run -- env diff dev staging        # Compare environments
cargo run -- env promote dev staging     # Promote configs between environments
cargo run -- env variables --environment production  # View environment variables
```

## Architecture Overview

### Template Pack System

**Template Pack File**: `.pmp.template-pack.yaml` (REQUIRED)
- Must have `apiVersion: pmp.io/v1` and `kind: TemplatePack`
- Contains metadata (name, description)
- Has an empty spec: `spec: {}`
- Located in: `~/.pmp/template-packs/<pack-name>/`, `.pmp/template-packs/<pack-name>/`, or custom paths

**Template File**: `.pmp.template.yaml` (inside `templates/` subdirectory)
- Must have `apiVersion: pmp.io/v1` and `kind: Template`
- Contains metadata (name, description) plus spec with resource definition
- Spec contains: `apiVersion`, `kind`, `executor`, `inputs`, `environments`
- Resource kind must be alphanumeric only
- Located in: `<pack-dir>/templates/<template-name>/.pmp.template.yaml`

**Plugin File**: `.pmp.plugin.yaml` (inside `plugins/` subdirectory - OPTIONAL)
- Must have `apiVersion: pmp.io/v1` and `kind: Plugin`
- Contains metadata (name, description) plus spec with `role` and `inputs`
- Plugins are reusable components that can be referenced by templates
- Located in: `<pack-dir>/plugins/<plugin-name>/.pmp.plugin.yaml`

**Template Pack Structure**:
```
~/.pmp/template-packs/<pack-name>/
├── .pmp.template-pack.yaml (kind: TemplatePack)
├── templates/
│   └── <template-name>/
│       ├── .pmp.template.yaml (kind: Template)
│       ├── main.tf.hbs
│       └── ... (other template files)
└── plugins/
    └── <plugin-name>/
        ├── .pmp.plugin.yaml (kind: Plugin)
        └── ... (plugin files)
```

**Note**: `.pmp.yaml` or `.pmp.yaml.hbs` files are auto-generated and must NOT be included in templates

### Plugin Allowlist in Templates

Templates can declare which plugins they allow to be used by other templates. This is configured in the template's `.pmp.template.yaml` file.

**Configuration Location**: `spec.plugins.allowed` (array of allowed plugin configurations)

**Allowed Plugin Structure**:
```yaml
spec:
  plugins:
    allowed:
      - template:
          apiVersion: pmp.io/v1  # API version of template providing the plugin
          kind: KubernetesWorkload  # Kind of template providing the plugin
        plugin: access  # Plugin name
        inputs:  # Optional input constraints/defaults
          database_name:
            default: ""
            description: Database to grant access to
```

**Example: PostgreSQL Access Plugin**

The PostgreSQL template includes an "access" plugin for creating database credentials:

```yaml
# In .pmp/template-packs/postgres/templates/postgres/.pmp.template.yaml
spec:
  plugins:
    allowed:
      - template:
          apiVersion: pmp.io/v1
          kind: KubernetesWorkload
        plugin: access
        inputs:
          database_name:
            default: ""
            description: Database to grant access to (inherits from parent if empty)
          k8s_secret_namespace:
            default: ""
            description: Kubernetes namespace for credential secret
```

**Access Plugin Features**:
- Creates PostgreSQL users/roles with configurable privileges
- Supports granular permissions: SELECT, INSERT, UPDATE, DELETE, TRUNCATE, REFERENCES, TRIGGER
- Generates secure random passwords if not provided
- Stores credentials in Kubernetes secrets
- Configurable connection limits and password expiration
- Schema-level permissions and role attributes (SUPERUSER, CREATEDB, CREATEROLE)

**Plugin Inputs** (for access plugin):
- `role_name` - Username to create (required)
- `role_password` - Password (auto-generated if empty)
- `database_name` - Database to grant access to
- `privileges` - Array of privileges (default: ["SELECT"])
- `grant_option` - Allow user to grant privileges to others (default: false)
- `connection_limit` - Max concurrent connections (default: -1, unlimited)
- `valid_until` - Password expiration date
- `schema_name` - Schema for privileges (default: "public")
- `create_schema` - Allow schema creation (default: false)
- `superuser` - Grant superuser privileges (default: false)
- `create_role` - Allow role creation (default: false)
- `create_db` - Allow database creation (default: false)
- `create_k8s_secret` - Store credentials in K8s secret (default: true)

**Plugin Files**:
- `.pmp.plugin.yaml` - Plugin metadata and input definitions
- `plugin.tf.hbs` - Terraform/OpenTofu code for credential management
- `variables.tf.hbs` - Variable definitions
- `outputs.tf.hbs` - Output definitions (connection info, secrets, usage instructions)

### Installed Plugins in Templates

Templates can define plugins that are automatically installed during project creation. These plugins cannot be removed and use the same structure as `spec.plugins.allowed`.

**Configuration Location**: `spec.plugins.installed` (array of installed plugin configurations)

**Installed Plugin Structure**:
```yaml
spec:
  plugins:
    installed:
      - template_pack_name: github
        plugin_name: repository
        inputs:  # Optional input overrides/defaults
          visibility:
            default: private
          enable_branch_protection:
            default: "true"
      - template_pack_name: aws
        plugin_name: ecr
```

**Behavior During Project Creation**:
1. User creates project with template
2. Template inputs collected
3. Installed plugins are processed automatically:
   - User sees: "Installing plugin: {template_pack_name}/{plugin_name}"
   - If plugin requires reference project: user selects from compatible projects
   - User prompted: "Customize inputs for this plugin? (y/N)"
   - If yes: collect all inputs (similar to update command)
   - If no: use defaults from plugin spec + installed config overrides
4. Plugins are rendered before template rendering
5. Template can reference plugins via `{{#if plugins.added}}` blocks
6. Plugins appear in `.pmp.environment.yaml` under `spec.plugins.added`

**Use Cases**:
- Auto-provision source control (GitHub repository) for applications
- Auto-create container registries (ECR) for containerized workloads
- Enforce organizational standards (e.g., all workloads must have monitoring)
- Simplify onboarding by reducing manual plugin selection

**Example: Kubernetes Workload with Auto-provisioned Resources**:
```yaml
spec:
  apiVersion: pmp.io/v1
  kind: KubernetesWorkload
  executor: opentofu

  plugins:
    # Users can manually add these plugins
    allowed:
      - template_pack_name: postgres
        plugin_name: access

    # These plugins are automatically installed
    installed:
      - template_pack_name: github
        plugin_name: repository
        inputs:
          gitignore_template:
            default: Terraform
          license_template:
            default: apache-2.0
      - template_pack_name: aws
        plugin_name: ecr
        inputs:
          image_tag_mutability:
            default: IMMUTABLE
```

### Infrastructure System

**Required**: Yes - cannot create projects without one
**File**: `.pmp.infrastructure.yaml`

**Must define**:
- `spec.environments` - Available environments (keys must be lowercase alphanumeric + optional underscores)
- `spec.categories` - Hierarchical category tree for organizing templates

**Environment Naming Rules**:
- Environment names (keys in `spec.environments`) MUST be lowercase alphanumeric
- May contain underscores (_)
- Cannot start with a number
- No uppercase letters, hyphens, or other special characters allowed

**Category Structure** (Required):
- `spec.categories` - Hierarchical tree organizing templates by category
- Templates must be listed in categories to be usable
- Categories can have subcategories for nested organization
- Structure:
  ```yaml
  spec:
    categories:
      - id: category_id               # Unique identifier (alphanumeric, underscores)
        name: Category Display Name   # Human-readable name
        description: Optional description
        subcategories: []             # Nested categories (optional)
        templates:                    # Templates in this category
          - template_pack: pack-name
            template: template-name
  ```

**Template Pack Configuration** (Optional):
- `spec.template_packs` - Template pack configurations for input defaults and overrides
- Separate from categories - handles input customization
- Structure:
  ```yaml
  spec:
    template_packs:
      <pack-name>:
        templates:
          <template-name>:
            defaults:
              inputs:
                <input-name>:
                  value: <value>
                  show_as_default: true  # true: change default; false: skip user prompt
  ```

**Template Organization Features**:
- **Filtering**: Only templates listed in categories are accessible
- **Multi-Category**: Templates can appear in multiple categories
- **Hierarchical**: Use subcategories for nested organization
- **Input Overrides**:
  - `show_as_default: true` - Changes the default value shown to user (user can still override)
  - `show_as_default: false` - Uses the value directly without prompting the user
- **Input Precedence**: Template base → Environment overrides → Template pack overrides → User input
- **Backward Compatible**: Old `resource_kinds` format auto-migrates to categories on load

**Complete Infrastructure Example**:
```yaml
spec:
  # Category tree organizing templates
  categories:
    - id: pmp_io_v1_kubernetesworkload
      name: KubernetesWorkload (pmp.io/v1)
      description: Kubernetes workload templates
      subcategories: []
      templates:
        - template_pack: microservices
          template: api-service
        - template_pack: microservices
          template: worker-service

    - id: databases
      name: Databases
      description: Database templates
      subcategories:
        - id: relational
          name: Relational Databases
          templates:
            - template_pack: postgres
              template: postgres
      templates: []

  # Template pack configuration for input overrides
  template_packs:
    microservices:
      templates:
        api-service:
          defaults:
            inputs:
              replicas:
                value: 3
                show_as_default: true     # User can change from 3
              environment:
                value: "production"
                show_as_default: false    # Always use "production", no prompt
        worker-service:
          defaults: {}
    postgres:
      templates:
        postgres:
          defaults: {}
```

**Executor Configuration** (Optional):
- `spec.executor` - Collection-level executor configuration
- Applies to all projects in the collection (unless overridden at project level)
- For OpenTofu executor, generates `_common.tf` file with backend configuration
- Structure:
  ```yaml
  spec:
    executor:
      name: opentofu
      config:
        backend:
          type: s3  # or azurerm, gcs, http, kubernetes, pg, consul, etc.
          # ... backend-specific parameters
  ```

**Backend Configuration Examples**:

1. **S3 Backend** (AWS):
   ```yaml
   spec:
     executor:
       name: opentofu
       config:
         backend:
           type: s3
           bucket: my-terraform-state
           key: project/terraform.tfstate
           region: us-west-2
           encrypt: true
           dynamodb_table: terraform-locks
   ```

2. **AzureRM Backend** (Azure):
   ```yaml
   spec:
     executor:
       name: opentofu
       config:
         backend:
           type: azurerm
           storage_account_name: mystorageaccount
           container_name: tfstate
           key: prod.terraform.tfstate
           resource_group_name: my-resource-group
   ```

3. **GCS Backend** (Google Cloud):
   ```yaml
   spec:
     executor:
       name: opentofu
       config:
         backend:
           type: gcs
           bucket: my-terraform-state
           prefix: terraform/state
   ```

4. **Kubernetes Backend**:
   ```yaml
   spec:
     executor:
       name: opentofu
       config:
         backend:
           type: kubernetes
           secret_suffix: state
           namespace: terraform
   ```

**Supported Backend Types**:
- `local` - Local file system (default)
- `s3` - AWS S3 with DynamoDB locking
- `azurerm` - Azure Blob Storage
- `gcs` - Google Cloud Storage
- `http` - Generic HTTP backend
- `kubernetes` - Kubernetes secrets
- `pg` - PostgreSQL
- `consul` - HashiCorp Consul
- `cos` - Tencent Cloud Object Storage
- `oss` - Alibaba Cloud OSS
- `remote` - Terraform Cloud/Enterprise

### Dependency-Only Projects

PMP supports creating projects that don't manage their own infrastructure but only define dependencies on other projects. These projects use the `none` executor.

**None Executor**:
- Special executor type that performs no operations
- Used for dependency-only projects that group other projects
- Always returns success for all operations (init, plan, apply, destroy, refresh)
- Does not support backends or generate infrastructure files
- When executing commands on a dependency graph, projects with `none` executor are skipped with a message

**Use Cases**:
1. **Environment-wide deployments**: Group all services for an environment
2. **Feature deployments**: Bundle all microservices for a feature
3. **Staged rollouts**: Define deployment order via dependency chains
4. **Integration testing**: Group services needed for end-to-end tests

**Example Template**:
```yaml
apiVersion: pmp.io/v1
kind: Template
metadata:
  name: "Project Group"
  description: "A dependency-only project for grouping other projects"
spec:
  resource:
    apiVersion: pmp.io/v1
    kind: ProjectGroup
  executor: none  # Use none executor for dependency-only projects
  inputs: []      # No inputs needed
```

**Workflow**:
1. Create a project using a template with `executor: none`
2. Edit the `.pmp.environment.yaml` file to add dependencies
3. Run `pmp apply`, `pmp preview`, or `pmp destroy` on the group project
4. PMP will execute the command on all dependencies in topological order
5. The group project itself is skipped (displays "Skipping... - dependency-only project")

**Example**:
See `examples/template-packs/grouping/` for a complete example of a dependency-only template pack.

### Project Creation Workflow

1. Discover template packs (`.pmp.template-pack.yaml` files)
2. Filter template packs by checking their templates against collection's allowed resource kinds
3. User selects template pack (auto-selected if only one available)
4. Discover templates within selected pack
5. User selects template (auto-selected if only one available)
6. Plugins are NOT shown to users - they are reusable components only
7. User selects environment (from the environments defined in the infrastructure)
8. User provides name, optional description, and inputs defined in the template
9. Render template files from template directory into environment folder
10. **Auto-generate `_common.tf`** (if executor config with backend is present in collection)
11. **Auto-generate `.pmp.project.yaml`** at project root (identifier only - metadata, no spec)
12. **Auto-generate `.pmp.environment.yaml`** in environment folder (with full spec)
13. Generate project structure:
    - Project root: `projects/{project_name}/`
    - Project identifier: `projects/{project_name}/.pmp.project.yaml`
    - Environment: `projects/{project_name}/environments/{environment_name}/`
    - Environment spec: `projects/{project_name}/environments/{environment_name}/.pmp.environment.yaml`
    - Backend config: `projects/{project_name}/environments/{environment_name}/_common.tf` (if executor config present)
    - Generated files: `projects/{project_name}/environments/{environment_name}/...`

### Project Discovery

- Scans `projects/` directory **recursively at ALL levels**
- Looks for `.pmp.project.yaml` files (identifier files)
- No depth limit - finds projects anywhere under `projects/`
- Reads resource kind from first `.pmp.environment.yaml` found

### Find Command

**Supports**:
- Search by name (substring match)
- Search by kind
- Show all projects (no filter)
- **After selecting project**: prompts user to select environment
- **Displays**: project metadata + selected environment details with spec

### File Naming

- **Template Packs**: `.pmp.template-pack.yaml`
- **Templates**: `.pmp.template.yaml` (inside `templates/` subdirectory of pack)
- **Plugins**: `.pmp.plugin.yaml` (inside `templates/` subdirectory of pack)
- **Project Identifiers**: `.pmp.project.yaml` (metadata only, no spec)
- **Environment Specs**: `.pmp.environment.yaml` (contains full spec)
- **Infrastructure**: `.pmp.infrastructure.yaml`

### Directory Structure

```
collection/
├── .pmp.infrastructure.yaml
└── projects/
    └── {project_name}/
        ├── .pmp.project.yaml (identifier only)
        └── environments/
            └── {environment_name}/
                ├── .pmp.environment.yaml (full spec)
                └── ... (generated terraform/tofu files)
```

Example: `projects/my-api/.pmp.project.yaml`
Environment: `projects/my-api/environments/dev/.pmp.environment.yaml`

### Project Naming Rules

- **Allowed characters**: lowercase letters (a-z), numbers (0-9), and hyphens (-)
- **Cannot start with**: a number or hyphen
- **Cannot end with**: a hyphen
- **Must be unique**: across the entire infrastructure
- **Internal variables available**:
  - `_name` - The project name as entered (e.g., `my-api`)
  - `_project_name_underscores` - Project name with hyphens converted to underscores (e.g., `my_api`)
  - `_project_name_hyphens` - Legacy variable (same as `_name` for hyphenated names)

## Important Implementation Details

### Template Pack Discovery (`src/template/discovery.rs`)
- Looks for `.pmp.template-pack.yaml` files
- Max depth: 2 levels from template base directories
- Validates `apiVersion` and `kind` on load
- Discovers templates within pack by scanning `templates/` subdirectory for `.pmp.template.yaml` files
- Discovers plugins within pack by scanning `templates/` subdirectory for `.pmp.plugin.yaml` files
- Template packs are selected first, then templates within the pack are shown to the user
- If only one template in a pack, it's auto-selected

### Project Discovery (`src/collection/discovery.rs`)
- Looks for `.pmp.project.yaml` files
- **No depth limit** - scans all subdirectories under `projects/`
- Extracts resource kind from first `.pmp.environment.yaml` found in `environments/` subdirectory
- Discovers environments by scanning `environments/` subdirectories for `.pmp.environment.yaml` files

### Preview/Apply Commands (`src/commands/preview.rs`, `src/commands/apply.rs`)
- **Smart context detection**:
  1. If in environment directory (has `.pmp.environment.yaml`): execute directly
  2. If in project directory (has `.pmp.project.yaml`): prompt to select environment
  3. If in collection or elsewhere: use find/search UI to select project + environment
- Executes in the environment directory (where generated files are located)

### Executors (`src/executor/`)
- **OpenTofu Executor** (`opentofu.rs`): Default executor for OpenTofu/Terraform projects
  - Supports backends (S3, Azure, GCS, Kubernetes, PostgreSQL, etc.)
  - Generates `_common.tf` files with backend and module configuration
  - Runs `tofu init`, `tofu plan`, `tofu apply`, `tofu destroy`

- **None Executor** (`none.rs`): Special executor for dependency-only projects
  - All operations are no-ops (always succeed)
  - Does not support backends
  - Used with templates that specify `executor: none`
  - Projects are skipped during dependency graph execution

- **Executor Registry** (`registry.rs`): Available for future extensibility
  - Can register custom executors
  - Currently supports `opentofu` and `none` executors

### Execution Helper (`src/commands/execution_helper.rs`)
- Handles dependency graph execution for preview, apply, and destroy commands
- Automatically detects and skips projects with `none` executor
- Displays "Skipping {project} ({env}) - dependency-only project" for none executor
- Runs hooks (pre/post) for all executors except `none`

### Graph Command (`src/commands/graph.rs`)
- **Purpose**: Visualize project dependency graphs
- **Usage**:
  - `pmp graph` - Show dependency graph for current project
  - `pmp graph --all` - Show all projects in infrastructure
  - `pmp graph --format mermaid --output graph.mmd` - Export to Mermaid format
  - `pmp graph --format dot --output graph.dot` - Export to GraphViz DOT format

- **Features**:
  - **ASCII Format**: Terminal-friendly tree visualization (default)
  - **Mermaid Format**: Generate Mermaid.js diagrams for documentation
  - **DOT Format**: Generate GraphViz DOT files for rendering
  - **Context-Aware**: Detects current location (environment, project, or infrastructure)
  - **Multi-Project View**: Visualize all projects and their relationships

- **Output Examples**:
  ```
  # ASCII (default)
  project-a:dev
  └─ project-b:dev
      └─ project-c:dev

  # Mermaid
  graph TD
      project_a_dev["project-a\n(dev)"]
      project_b_dev["project-b\n(dev)"]
      project_a_dev --> project_b_dev

  # DOT
  digraph dependencies {
      rankdir=LR;
      node [shape=box, style=rounded];
      project_a_dev [label="project-a\n(dev)"];
      project_b_dev [label="project-b\n(dev)"];
      project_a_dev -> project_b_dev;
  }
  ```

### Deps Command (`src/commands/deps.rs`)
- **Purpose**: Analyze and manage project dependencies
- **Subcommands**:
  - `pmp deps analyze` - Comprehensive dependency analysis
  - `pmp deps impact <project>` - Show projects affected by changes

- **Analyze Features**:
  - **Health Checks**:
    - Circular dependency detection
    - Missing dependency validation
    - Orphaned project identification
  - **Statistics**:
    - Total projects count
    - Projects with dependencies
    - Standalone projects
  - **Bottleneck Detection**: Find projects that many others depend on
  - **Standalone Projects**: List projects with no dependencies

- **Impact Analysis**:
  - Shows all projects (direct and indirect) that depend on a target project
  - Helps assess blast radius of changes
  - Useful for planning deployments and understanding dependencies

- **Analysis Output Example**:
  ```
  Summary:
  Total Projects: 15
  Projects with Dependencies: 8
  Standalone Projects: 7

  Health Checks:
  ✓ No circular dependencies detected
  ✓ No missing dependencies

  Dependency Bottlenecks:
  postgres-db:production ← 5 project(s) depend on this
  api-gateway:production ← 3 project(s) depend on this

  Orphaned Projects:
  • monitoring:dev
  • logging:dev
  ```

### State Command (`src/commands/state.rs`)
- **Purpose**: Manage infrastructure state and detect drift
- **Subcommands**:
  - `pmp state list` - Show state across all projects
  - `pmp state drift` - Detect configuration drift
  - `pmp state lock` - Lock state for a project
  - `pmp state unlock` - Unlock state for a project
  - `pmp state sync` - Sync remote state

- **List Features**:
  - **Overview**: Shows state information for all project environments
  - **Details Mode**: Use `--details` flag for additional information
  - **Information Displayed**:
    - Project name and environment
    - Resource count (if state file exists)
    - Last modified timestamp
    - Lock status and lock holder information
  - **Example**:
    ```
    Project: my-api (dev)
    Resources: 15
    Last Modified: 2025-01-15 10:30:00 UTC
    Locked: No

    Project: postgres-db (production)
    Resources: 8
    Last Modified: 2025-01-15 09:15:00 UTC
    Locked: Yes (by user@hostname)
    ```

- **Drift Detection**:
  - **Purpose**: Detect differences between desired and actual state
  - **Usage**: `pmp state drift` or `pmp state drift <project>`
  - **Mechanism**: Uses OpenTofu's `plan -detailed-exitcode`
    - Exit code 0: No drift
    - Exit code 2: Drift detected
  - **Output**: Shows which projects have drift and the changes detected
  - **Example**:
    ```
    Checking drift for my-api (dev)...
    ⚠ Drift detected!
    Changes:
      ~ aws_instance.web: ami changed
      + aws_s3_bucket.logs: will be created
    ```

- **State Locking**:
  - **Lock**: `pmp state lock <project>`
  - **Unlock**: `pmp state unlock <project>` or `pmp state unlock <project> --force`
  - **Purpose**: Prevent concurrent modifications
  - **Lock Information**:
    - Lock ID (UUID)
    - Operation type
    - User and hostname
    - Timestamp
  - **Ownership**: Unlock checks lock ownership unless `--force` is used
  - **Storage**: Locks stored in `.terraform/terraform.tfstate.lock.info`

- **State Sync**:
  - **Purpose**: Sync state with remote backend
  - **Usage**: `pmp state sync`
  - **Mechanism**: Runs `tofu refresh` on all project environments
  - **Use Cases**:
    - Update local state after manual changes
    - Refresh state before planning changes
    - Sync after backend configuration changes

### CI Command (`src/commands/ci.rs`)
- **Purpose**: Generate CI/CD pipeline configurations
- **Usage**: `pmp ci generate <type> [--output FILE] [--environment ENV]`
- **Supported Pipeline Types**:
  - `github-actions`, `github` - GitHub Actions workflow
  - `gitlab-ci`, `gitlab` - GitLab CI configuration
  - `jenkins` - Jenkinsfile

- **Features**:
  - **Dependency-Aware**: Automatically organizes projects into stages based on dependencies
  - **Parallel Execution**: Projects without dependencies run in parallel within stages
  - **Topological Sorting**: Uses `group_by_dependency_level()` to determine execution order
  - **Environment Filtering**: Generate pipelines for specific environments only
  - **File Output**: Save to file with `--output` or print to stdout

- **GitHub Actions Output**:
  ```yaml
  name: PMP Infrastructure Deployment

  on:
    push:
      branches:
        - main
    pull_request:
      branches:
        - main
    workflow_dispatch:

  env:
    TOFU_VERSION: "1.6.0"

  jobs:
    stage_0:
      name: Deploy Stage 0
      runs-on: ubuntu-latest
      strategy:
        matrix:
          project:
            - name: "postgres-db"
              env: "production"
              path: "projects/postgres_db/postgres_db/environments/production"
      steps:
        - name: Checkout
          uses: actions/checkout@v4
        - name: Setup OpenTofu
          uses: opentofu/setup-opentofu@v1
          with:
            tofu_version: ${{ env.TOFU_VERSION }}
        - name: Tofu Init
          working-directory: ${{ matrix.project.path }}
          run: tofu init
        - name: Tofu Plan
          working-directory: ${{ matrix.project.path }}
          run: tofu plan -no-color
        - name: Tofu Apply
          if: github.ref == 'refs/heads/main' && github.event_name == 'push'
          working-directory: ${{ matrix.project.path }}
          run: tofu apply -auto-approve
  ```

- **GitLab CI Output**:
  ```yaml
  # GitLab CI/CD Pipeline for PMP Infrastructure

  stages:
    - stage_0
    - stage_1

  variables:
    TOFU_VERSION: "1.6.0"

  default:
    image: alpine:latest
    before_script:
      - apk add --no-cache curl
      - curl -Lo /usr/local/bin/tofu https://github.com/opentofu/opentofu/...
      - chmod +x /usr/local/bin/tofu

  postgres_db_production:
    stage: stage_0
    script:
      - cd projects/postgres_db/postgres_db/environments/production
      - tofu init
      - tofu validate
      - tofu plan -no-color
      - |
        if [ "$CI_COMMIT_BRANCH" == "main" ]; then
          tofu apply -auto-approve
        fi
    rules:
      - if: $CI_PIPELINE_SOURCE == "merge_request_event"
      - if: $CI_COMMIT_BRANCH == "main"
  ```

- **Jenkins Output**:
  ```groovy
  // Jenkinsfile for PMP Infrastructure

  pipeline {
      agent any

      environment {
          TOFU_VERSION = '1.6.0'
      }

      stages {
          stage('Stage 0') {
              parallel {
                  stage('postgres-db:production') {
                      steps {
                          dir('projects/postgres_db/postgres_db/environments/production') {
                              sh 'tofu init'
                              sh 'tofu validate'
                              sh 'tofu plan -no-color'
                              script {
                                  if (env.BRANCH_NAME == 'main') {
                                      sh 'tofu apply -auto-approve'
                                  }
                              }
                          }
                      }
                  }
              }
          }
      }

      post {
          success {
              echo 'Deployment successful!'
          }
          failure {
              echo 'Deployment failed!'
          }
      }
  }
  ```

- **Dependency Grouping Algorithm**:
  - Projects are grouped into "stages" (levels)
  - Stage 0: Projects with no dependencies
  - Stage N: Projects whose dependencies are all satisfied in stages 0 through N-1
  - Within each stage, projects execute in parallel
  - If circular dependencies exist, all remaining projects are added to break the deadlock

- **Usage Examples**:
  ```bash
  # Generate GitHub Actions workflow
  pmp ci generate github-actions --output .github/workflows/deploy.yml

  # Generate GitLab CI for production only
  pmp ci generate gitlab-ci --environment production --output .gitlab-ci.yml

  # Generate Jenkins pipeline and print to stdout
  pmp ci generate jenkins

  # Generate GitHub Actions for dev environment
  pmp ci generate github --environment dev --output .github/workflows/deploy-dev.yml
  ```

### Template Command (`src/commands/template.rs`)
- **Purpose**: Create and scaffold new template packs interactively
- **Usage**: `pmp template scaffold [--output DIR]`

- **Features**:
  - **Interactive Creation**: Guided prompts for all template pack metadata
  - **Auto-generation**: Automatically creates directory structure and starter files
  - **Complete Package**: Generates README, template files, and variable definitions
  - **Flexible Executors**: Support for OpenTofu, Terraform, or none (dependency-only)

- **Interactive Prompts**:
  - Template pack name and description
  - Template name and description
  - Resource kind (alphanumeric validation)
  - Executor selection (opentofu, terraform, none)
  - Input definitions (name, type, description, defaults)

- **Generated Files**:
  - `.pmp.template-pack.yaml` - Pack metadata
  - `templates/<name>/.pmp.template.yaml` - Template definition
  - `main.tf.hbs` - Main infrastructure code (if executor != none)
  - `variables.tf.hbs` - Variable definitions
  - `outputs.tf.hbs` - Output definitions
  - `README.md` - Documentation with usage examples

- **Example Output Structure**:
  ```
  my-pack/
  ├── .pmp.template-pack.yaml
  ├── README.md
  └── templates/
      └── my-template/
          ├── .pmp.template.yaml
          ├── main.tf.hbs
          ├── variables.tf.hbs
          └── outputs.tf.hbs
  ```

- **Usage Examples**:
  ```bash
  # Scaffold in current directory
  pmp template scaffold

  # Scaffold in custom location
  pmp template scaffold --output ./custom-templates

  # After scaffolding, use with:
  pmp create --template-packs-paths ./my-pack
  ```

### Clone Command (`src/commands/clone.rs`)
- **Purpose**: Clone existing projects with new names
- **Usage**: `pmp clone [SOURCE] <NAME> [--environment ENV]`

- **Features**:
  - **Project Selection**: Interactive or direct project selection
  - **Environment Filtering**: Clone specific environments or all
  - **Metadata Update**: Automatically updates project name in configurations
  - **Directory Structure**: Maintains original project structure

- **Cloning Process**:
  1. Select source project (interactive or by name)
  2. Select environments to clone (multi-select or specific)
  3. Confirm cloning operation
  4. Create new project directory
  5. Copy all files from source environments
  6. Update `.pmp.project.yaml` and `.pmp.environment.yaml` with new name
  7. Preserve all configuration and infrastructure code

- **Use Cases**:
  - Create similar projects with different configurations
  - Replicate project structure for new services
  - Environment-specific cloning (e.g., clone only production)
  - Rapid prototyping from existing projects

- **Usage Examples**:
  ```bash
  # Interactive selection
  pmp clone new-project-name

  # Clone specific project
  pmp clone my-api new-api

  # Clone only specific environment
  pmp clone my-api new-api --environment production

  # After cloning
  cd projects/resource_kind/new-api/environments/production
  tofu init
  ```

### Env Command (`src/commands/env.rs`)
- **Purpose**: Manage and compare environments across projects
- **Subcommands**:
  - `pmp env diff <source> <target>` - Compare configurations
  - `pmp env promote <source> <target> [--project FILTER]` - Promote configs
  - `pmp env sync [--project FILTER]` - Find common settings
  - `pmp env variables [--environment ENV] [--project FILTER]` - View variables

- **Diff Features**:
  - **Comparison**: Shows differences between two environments
  - **Input Analysis**: Identifies inputs only in source, only in target, or with different values
  - **Project Scope**: Compares all projects that have both environments
  - **Output**:
    ```
    Project: my-api
      Only in dev: debug_mode
      Only in staging: monitoring_enabled
      replicas: 1 → 3
    ```

- **Promote Features**:
  - **Configuration Promotion**: Copy input values from source to target environment
  - **Backup Creation**: Automatically creates `.yaml.backup` before changes
  - **Dependency Preservation**: Keeps environment-specific dependencies intact
  - **Safety**: Requires confirmation before overwriting
  - **Project Filtering**: Optionally promote only matching projects

- **Sync Features**:
  - **Common Settings**: Identifies inputs with identical values across all environments
  - **Consistency Check**: Helps find settings that should be synchronized
  - **Multi-Environment**: Analyzes projects with 2+ environments

- **Variables Features**:
  - **Centralized View**: Display all environment variables in one place
  - **Environment Filter**: Show variables for specific environment
  - **Project Filter**: Show variables for specific project(s)
  - **Grouped Display**: Variables grouped by environment and then by name

- **Usage Examples**:
  ```bash
  # Compare dev and staging environments
  pmp env diff dev staging

  # Promote dev configs to staging (with confirmation)
  pmp env promote dev staging

  # Promote only for specific project
  pmp env promote dev staging --project my-api

  # Find common settings across environments
  pmp env sync

  # View all production variables
  pmp env variables --environment production

  # View variables for specific project across all environments
  pmp env variables --project my-api

  # View all variables for all projects
  pmp env variables
  ```

- **Promote Output Example**:
  ```
  Environment Promotion
  Infrastructure: My Infrastructure
  Source Environment: dev
  Target Environment: staging

  Found 5 project(s) to promote

  Promote dev → staging? This will overwrite target configurations. (y/N)

  ✓ Promoted my-api (backup: .pmp.environment.yaml.backup)
  ✓ Promoted my-worker (backup: .pmp.environment.yaml.backup)
  ...

  ✓ Promoted 5 project(s) from dev to staging
  ```

## Testing Considerations

- Template pack files must be `.pmp.template-pack.yaml`
- Template files must be `.pmp.template.yaml` (inside `templates/` subdirectory)
- Plugin files must be `.pmp.plugin.yaml` (inside `templates/` subdirectory)
- Resource kinds must be alphanumeric
- Environment names must be lowercase alphanumeric + optional underscores (cannot start with number)
- Project discovery must work at any depth
- Multiple projects per resource kind supported
- Multiple environments per project supported
- Auto-selection works when only one template pack or template is available
