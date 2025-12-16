# PMP Roadmap

This document outlines the development plan for PMP (Poor Man's Platform).

## Current Status

**Version:** 0.4.x (Active Development)
**Overall Completion:** ~90% (core features complete, developer experience features in progress)
**Test Coverage:** 487 tests (451 unit + 36 integration) across 20+ modules

PMP core features are stable and production-ready. v0.3.x completed: template versioning/inheritance/partials, cost estimation (Infracost), OPA policy integration, secrets management, and parallel execution. Current focus (v0.4.x): template linting, template diff, webhook notifications, and documentation generation.

---

## Implemented Features

### Core CLI (100% Complete)

#### Template System
- [x] Handlebars template rendering with custom helpers (`eq`, `contains`, `k8s_name`, `bool`)
- [x] 25+ input types:
  - Basic: string, number, boolean, password, email, url, ip, cidr, json, yaml
  - Selection: select, multiselect, list
  - Complex: object, repeatable_object
  - References: project_select, multi_project_select
  - Specialized: color, duration, cron, keyvalue, semver, region, path, port, arn, docker_image
- [x] Conditional inputs (`show_if` with equals/exists conditions)
- [x] Variable interpolation (`${var:name}`, `${env:VAR}`, `${env:VAR:default}`)
- [x] Environment-specific input overrides
- [x] Template scaffolding command (`pmp template scaffold`)
- [x] Template pack installation from GitHub
- [x] **Template Versioning** - Directory-based versions (`templates/{name}/versions/{semver}/`), version selection UI, legacy support (v0.0.1)
- [x] **Template Inheritance** - `extends` field for base templates, merge rules (child wins), circular detection
- [x] **Template Partials** - Handlebars partials (`{{> partial_name}}`), global and pack-level priority

#### Project Management
- [x] Interactive project creation with plugin support
- [x] Project discovery (recursive, no depth limit)
- [x] Project cloning with name transformation
- [x] Project updates from templates (preserves user modifications)
- [x] Project groups for batch creation
- [x] Naming validation (lowercase, hyphens, no leading/trailing numbers)

#### Executors
- [x] OpenTofu/Terraform executor with full lifecycle support
- [x] None executor for dependency-only projects
- [x] 11 backend types: local, s3, azurerm, gcs, http, kubernetes, pg, consul, cos, oss, remote
- [x] PostgreSQL auto-table-naming (SHA1 hash)
- [x] Command options configuration
- [x] Executor registry with factory pattern

#### Operations
- [x] Preview (plan) - with dependency ordering
- [x] Apply - with confirmation and hooks
- [x] Destroy - with confirmation and dependency ordering
- [x] Refresh - state synchronization
- [x] Test - validation without resource creation
- [x] Generate - template rendering without project structure

#### Dependency System
- [x] Dependency declaration in templates with remote state
- [x] Dependency graph building (BFS traversal)
- [x] Topological sort with cycle detection
- [x] Graph visualization (ASCII, Mermaid, DOT formats)
- [x] Impact analysis (`pmp deps impact`)
- [x] Dependency validation and health checks
- [x] Multi-environment dependency resolution

#### Hooks System
- [x] Command hooks (shell execution, platform-aware)
- [x] Confirm hooks (user confirmation with defaults)
- [x] Set environment hooks (interactive input, sensitive support)
- [x] Pre/post phases for all operations
- [x] Multi-level hooks (infrastructure, template, environment)
- [x] Hook outcomes (Continue/Cancel)

#### Environment Management
- [x] Environment comparison (`pmp env diff`)
- [x] Environment promotion (`pmp env promote`)
- [x] Variable synchronization (`pmp env sync`)
- [x] Environment-specific backends
- [x] Environment variables display
- [x] Environment time limits (`spec.time_limit` with `expires_at` or `ttl`)
- [x] Environment purge (`pmp env purge` - destroy expired environments)

#### Plugin System
- [x] Plugin discovery within template packs
- [x] Allowed plugins (user-selected during updates)
- [x] Installed plugins (auto-installed during creation)
- [x] Plugin input configuration with defaults
- [x] Plugin dependencies with reference resolution
- [x] Disable user input override option

### CI/CD (100% Complete)

#### Pipeline Generation
- [x] GitHub Actions workflows
- [x] GitLab CI pipelines
- [x] Jenkins pipelines
- [x] Dynamic pipelines (change detection based)
- [x] Static pipelines (deploy all projects)
- [x] Topological stage ordering

#### Change Detection
- [x] Git-based change detection
- [x] Environment filtering
- [x] JSON/YAML output formats
- [x] Exit codes for CI integration

### State & Drift (100% Complete)

#### State Management
- [x] State listing across projects
- [x] State locking/unlocking
- [x] State backup and restore
- [x] State migration between backends
- [x] State sync with remote

#### Drift Detection
- [x] Drift detection (`pmp drift detect`)
- [x] Drift reporting (JSON/text formats)
- [x] Drift reconciliation

### Policy & Security (100% Complete)

#### Policy Validation
- [x] Naming convention checks
- [x] Tag validation
- [x] Dependency validation
- [x] Secret detection

#### Security Scanning
- [x] tfsec integration
- [x] checkov integration
- [x] trivy integration

### Search (100% Complete)
- [x] Search by tags
- [x] Search by resource type (kind)
- [x] Search by name pattern
- [x] Search by outputs

---

## Work in Progress

### Web UI (100% Complete - Auth Excluded)

**File:** `src/commands/ui.rs` (2,000+ lines)

#### Implemented
- [x] Axum HTTP server with CORS support
- [x] Template pack listing API (`GET /api/template-packs`)
- [x] Template details API (`GET /api/template-packs/:pack/templates/:template`)
- [x] Project listing API (`GET /api/projects`)
- [x] Infrastructure loading API (`POST /api/infrastructure/load`)
- [x] Directory browsing API (`POST /api/browse`)
- [x] Drive enumeration (Windows/Unix aware)
- [x] Execute APIs: generate, preview, apply, destroy, refresh
- [x] Template pack installation (git and local)
- [x] **Project creation API** (`POST /api/projects/create`) - Full validation and non-interactive mode
- [x] **Dependency graph API** (`GET /api/graph`) - Mermaid diagram, nodes, edges
- [x] **Embedded UI files** (`src/ui/index.html`, `src/ui/app.js`, `src/ui/tailwind.css`, `src/ui/jquery.js`)
- [x] **Input type conversion** - All 24+ input types converted to API-friendly format
- [x] **Project creation form** - Dynamic input rendering with type-aware fields
- [x] **Conditional input visibility** - `show_if` conditions exposed via API
- [x] **WebSocket streaming** (`/ws/execute`) - Real-time operation output for preview, apply, destroy, refresh
- [x] **Dashboard API** (`GET /api/dashboard`) - Project stats, distribution by kind/environment, recent operations
- [x] **Operations tracking** (`GET /api/operations`) - List running and completed operations with status
- [x] **Multi-view UI** - Dashboard, Projects, and Graph views with navigation
- [x] **HTTP fallback** - Automatic fallback to HTTP when WebSocket unavailable

#### Not Implemented (By Design)
- [ ] Authentication/authorization (not required for local development tool)

### Import (100% Complete)

**Files:** `src/commands/import.rs`, `src/infrastructure/`

Import **existing cloud infrastructure** into OpenTofu/Terraform management using pmp-cloud-inspector exports.

**Commands:**
```bash
# Import from pmp-cloud-inspector export (recommended)
pmp import from-export ./cloud-inventory.json
pmp import from-export ./cloud-inventory.yaml --filter 'aws:ec2:*'
pmp import from-export ./export.json --provider aws --region us-east-1

# Manual import (no API access required)
pmp import manual aws_vpc vpc-12345 --name main_vpc

# Batch import from YAML config
pmp import batch ./import-config.yaml --yes
```

**Supported Providers:** (see [cloud-inspector-permissions.md](cloud-inspector-permissions.md) for required permissions)
- AWS (30+ resource types)
- Azure (20+ resource types)
- GCP (20+ resource types)
- GitHub (repository, team, membership, org settings)
- GitLab (project, group, user)
- JFrog Artifactory (repository, user, group, permission)
- Okta (user, group, application, auth server)
- Auth0 (client, connection, user, role, resource server)
- Jira (project) - no stable Terraform provider
- Opsgenie (alert policy)

**Features:**
- [x] Core types and traits (`src/infrastructure/discovery.rs`)
- [x] **Cloud Inspector integration** (`src/infrastructure/cloud_inspector.rs`)
- [x] **Resource type mapping** (`src/infrastructure/resource_mapper.rs`)
- [x] Import from pmp-cloud-inspector JSON/YAML exports
- [x] **Schema version validation** - Validates pmp-cloud-inspector export format compatibility
- [x] **Auto-generate required_providers** - Best-effort provider block generation with version constraints
- [x] Resource filtering by provider, type, region, tags
- [x] Cost summary display from exports
- [x] Manual resource entry (no provider SDK required)
- [x] Import block generation (OpenTofu format)
- [x] **Auto-generate _providers.tf** - Creates provider configuration templates
- [x] Dependency detection (VPC → Subnet → Instance ordering)
- [x] Workflow orchestration (init, plan, apply)
- [x] Batch import from YAML configuration
- [x] Provider registry architecture
- [x] Validation and rollback support

**Batch Config Example:**
```yaml
apiVersion: pmp.io/v1
kind: InfrastructureImport
metadata:
  name: production-import
spec:
  provider: aws
  destination:
    type: new_project
    project_name: imported-infra
    environment: production
  resources:
    - type: aws_vpc
      id: vpc-12345678
      name: main_vpc
    - type: aws_subnet
      id: subnet-abcdef
      name: private_subnet
  options:
    generate_config: true
    auto_detect_dependencies: true
```

---

## Code Quality & Technical Debt

### Test Coverage

| Component | Test Status | Notes |
|-----------|-------------|-------|
| Dependency Graph | 11 tests | Excellent coverage |
| Template System | Multiple tests | Good coverage |
| Executors | Tests present | Good coverage |
| Hooks | Tests present | Good coverage |
| Commands | 11/25+ modules | graph, search, env, import, clone, create, generate, ci_detect_changes, project_group, ui |
| Infrastructure | 51 tests | Complete coverage |
| UI/HTTP API | 19 tests | API serialization/deserialization |
| OPA Policy | 28 tests | Provider, discovery, regorus integration, compliance reports |
| Integration | 36 tests | CLI help commands, error handling, temp directory workflows |

**Total:** 370 tests (running count from `cargo test`)

### Known TODOs in Codebase

All major TODOs have been resolved:
- Label selector matching in create.rs - **Completed**
- CI binary installation steps - **Completed** (uses install.sh from pmp-project/pmp-cli)
- Import validation and rollback - **Completed**
- Compiler warnings - **All fixed**

Remaining minor TODOs (low priority):
- None - all major features complete

---

## Planned Features

### Short-term (v0.2.x)

#### 1. Complete Web UI (DONE)
- [x] Project creation wizard with form validation
- [x] Real-time operation streaming via WebSocket
- [x] Visual dependency graph API (Mermaid format)
- [x] Dashboard with project status overview
- [x] Embedded static UI files
- [x] Operation tracking and history

#### 2. Complete Import Feature (DONE)
- [x] State file migration with conflict resolution
- [x] Bulk import configuration file support
- [x] Import validation and rollback

#### 3. Test Coverage Expansion (DONE)
- [x] Integration test suite (`tests/` directory)
- [x] Command module unit tests (graph, search, env, import, clone, ui)
- [x] HTTP API endpoint tests (serialization/deserialization)
- [x] End-to-end workflow tests with temp directories

#### 4. CI/CD Installation (DONE)
- [x] PMP binary distribution (GitHub releases workflow)
- [x] CI pipeline integration (install.sh references in generated workflows)
- [x] install.sh script creation
- [x] Docker image for CI runners (Dockerfile)

### Medium-term (v0.3.x) - COMPLETED

#### 1. Enhanced Templates (DONE)
- [x] **Template versioning** - Directory-based versions (`templates/{name}/versions/{semver}/`), automatic version selection, legacy v0.0.1 fallback
- [x] **Template inheritance** - Base templates with `extends` field, merge rules (child wins), circular detection, inheritance chain tracking
- [x] **Template partials** - Handlebars partials (`{{> partial_name}}`), pack-level (`{pack}/partials/`) and global (`~/.pmp/partials/`) discovery with priority

#### 2. Cost Estimation (DONE)
- [x] **Infracost integration** - `pmp cost estimate`, `pmp cost diff`, `pmp cost report` commands
- [x] **Cost provider trait** - Extensible architecture for future cost providers
- [x] **Budget thresholds** - Configurable warn/block thresholds in `.pmp.infrastructure.yaml`
- [x] **Infrastructure configuration** - `spec.cost` section with provider, API key, thresholds, CI settings
- [x] **Pre-apply cost integration** - `--cost` flag on `pmp project preview` and `pmp project apply`
- [x] **CI/CD pipeline cost steps** - Infracost integration in GitHub Actions, GitLab CI, and Jenkins

#### 3. OPA Policy Integration (DONE)
- [x] **Native OPA integration** - Uses `regorus` crate (pure Rust OPA implementation)
- [x] **Policy provider trait** - Extensible architecture for future policy providers
- [x] **Policy discovery** - Multi-path discovery (./policies, ~/.pmp/policies, custom paths)
- [x] **CLI commands**: `pmp policy opa validate|test|list|report`
- [x] **Pre-apply/preview policy validation** - Automatic validation with `--skip-policy` flag to bypass
- [x] **Compliance reporting with remediation** - JSON, Markdown, HTML formats

#### 4. Template Marketplace (DONE)
- [x] **URL-based registries** - Fetch JSON index from any URL with 1-hour caching
- [x] **Filesystem registries** - Scan local directories for template packs
- [x] **Registry management** - Add, list, remove registries with priority ordering
- [x] **Pack operations** - Search, list, info, install (git clone)
- [x] **Index generation** - `pmp marketplace generate-index` creates `index.json` and `index.html`

#### 5. Secrets Integration (DONE)
- [x] **Secret manager configuration** - Define available secret managers in `.pmp.infrastructure.yaml`
- [x] **Secret-backed inputs** - Template/plugin inputs with `secret_manager` configuration
- [x] **Secret manager providers** - HashiCorp Vault, AWS Secrets Manager
- [x] **Terraform generation** - Generates provider blocks, remote state data sources, secret data sources
- [x] **Template helper** - `{{secret input_name}}` outputs `local.secret_<input_name>`

#### 6. Developer Experience (DONE)
- [x] **Plan diff visualization** - `pmp project preview --diff` for color-coded diff output
  - Side-by-side view, color-coded changes, summary statistics
  - ASCII and HTML output formats with `--diff-format`

#### 7. Environment Operations (DONE)
- [x] **Environment time limits** - TTL configuration (`spec.time_limit` with `expires_at` or `ttl`)
- [x] **Environment purge** - `pmp env purge` destroy all expired environments

#### 8. Parallel Execution (DONE)
- [x] **Parallel operations** - Execute independent projects concurrently
- [x] **Graph-aware scheduling** - Topological sort with parallel execution within each level
- [x] **Failure behavior configuration** - `spec.executor.parallel.on_failure: stop|continue|finish_level`

---

### v0.4.x - Developer Experience (Current Focus)

#### 1. Template Validation & Linting
- [ ] **`pmp template lint`** - Validate template packs for common issues
  - Missing required fields in `.pmp.template.yaml`
  - Unused inputs (defined but not used in templates)
  - Invalid input type configurations
  - Circular inheritance detection at pack level
  - Handlebars syntax validation
  - Best practices warnings (e.g., missing descriptions)
  - Auto-fix capability for common issues
  ```bash
  pmp template lint [--pack PACK] [--fix] [--format json|text]
  ```

#### 2. Template Diff
- [ ] **`pmp template diff`** - Compare template versions or packs
  ```bash
  pmp template diff v1.0.0 v2.0.0 --pack my-pack
  pmp template diff pack-a pack-b --template web-app
  ```

#### 3. Webhook Notifications (Generic)
- [ ] **Generic webhook hook type** - Covers Slack, Teams, Discord, etc.
  ```yaml
  hooks:
    post_apply:
      - type: webhook
        config:
          url: ${env:WEBHOOK_URL}
          method: POST
          headers:
            Content-Type: application/json
          body: |
            {"text": "Deployed {{_project_name}} to {{_environment}}"}
  ```
- [ ] **Template variables in messages** - Use project/environment context in notification text
- [ ] **Hook outcome reporting** - Success/failure status in notifications

#### 4. Documentation Generation
- [ ] **`pmp docs generate`** - Auto-generate markdown from templates
  - Template inputs with descriptions and defaults
  - Output variables documentation
  - Dependency diagrams (Mermaid)
  - Usage examples

#### 5. Dry-Run Mode Enhancement
- [ ] **Enhanced `--dry-run`** for all commands
  - Show what would happen without executing
  - Works for create, update, destroy
  - JSON output for scripting

---

### v0.5.x - Operations & Governance

#### 1. Project Health Dashboard
- [ ] **`pmp project health`** - Aggregate health status across all projects
  - Drift status (in-sync, drifted, unknown)
  - Last apply timestamp
  - State lock status
  - Policy compliance score
  - Cost trend (if Infracost enabled)
  ```bash
  pmp project health [--format json|table|html]
  ```

#### 2. Environment Cloning
- [ ] **`pmp env clone <source> <target>`** - Clone environments
  - Clone all projects from one environment to another
  - Optional `--to-region REGION` for multi-region deployments
  - Input value mapping/transformation

#### 3. Environment Blueprints
- [ ] **Pre-defined environment configurations**
  ```yaml
  spec:
    blueprints:
      production:
        time_limit: null
        require_approval: true
        allowed_templates: [core/*]
      sandbox:
        time_limit: { ttl: "7d" }
        auto_destroy: true
        max_projects: 5
  ```

#### 4. Audit Logging
- [ ] **Track all PMP operations**
  - Who ran what command, when
  - Success/failure status
  - Input values (sanitized)
  - Output to file, syslog, or webhook

#### 5. Resource Tagging Automation
- [ ] **Auto-inject common tags**
  - `pmp:project`, `pmp:environment`, `pmp:template`
  - Custom tag templates in infrastructure config
  - Tag compliance checking

---

### v0.6.x - Advanced Templates

#### 1. Provider Abstractions
- [ ] **Provider-agnostic input types** with provider mapping
  ```yaml
  inputs:
    - name: compute_size
      type: compute_tier
      provider_map:
        aws: { t3.micro: small, t3.small: medium, t3.large: large }
        azure: { Standard_B1s: small, Standard_B2s: medium, Standard_D2s_v3: large }
        gcp: { e2-micro: small, e2-small: medium, e2-standard-2: large }
  ```
  - Templates work across providers
  - User selects "small/medium/large", system maps to provider-specific value
  - Extensible via configuration

#### 2. Project Migration
- [ ] **`pmp project migrate`** - Migrate projects between templates
  ```bash
  pmp project migrate --from old-template --to new-template
  ```
  - Auto-map compatible inputs
  - Generate migration plan
  - Preserve state where possible

---

### v1.0.0 - Production Ready

#### 1. Workspace Profiles
- [ ] **Multiple infrastructure configurations**
  - `pmp profile list/switch/create`
  - Support for different teams/projects
  - Profile-specific template packs

#### 2. Git Integration Improvements
- [ ] **`pmp git status`** - Show changed projects since last commit
- [ ] **Auto-commit after apply** (optional)
- [ ] **Branch-per-environment support**
- [ ] **PR integration** (create PR for changes)

#### 3. Remote State Browser
- [ ] **`pmp state browse`** - Interactive TUI for exploring remote state
  - Tree view of resources
  - Resource details with attributes
  - Search/filter capabilities
  - Export to JSON

#### 4. Cost Alerts (Simplified)
- [ ] **Threshold-based alerts**
  ```yaml
  spec:
    cost:
      alerts:
        - threshold: 100
          action: warn
        - threshold: 500
          action: block
  ```

---

## Contributing

Contributions are welcome. Priority areas:

### High Priority (v0.4.x)
1. **Template linting** - `pmp template lint` command
2. **Template diff** - `pmp template diff` command
3. **Webhook notifications** - Generic webhook hook type

### Medium Priority
4. **Documentation generation** - Auto-generate docs from templates
5. **Environment cloning** - Clone environments across regions
6. **Template packs** - Create packs for common scenarios (AWS, Azure, GCP)

### Recently Completed (v0.3.x)
- **Plan diff visualization** - `pmp project preview --diff` with color-coded output
- **Environment time limits** - TTL configuration and purge command
- **Secrets integration** - HashiCorp Vault, AWS Secrets Manager
- **Parallel execution** - Concurrent project operations
- **OPA policy integration** - Native Rego policy validation
- **Cost estimation** - Infracost integration

### Getting Started
1. Clone the repository
2. Run `cargo build` to build
3. Run `cargo test` to run tests
4. Check the Planned Features section for specific tasks

---

## Version History

| Version | Status | Key Features |
|---------|--------|--------------|
| 0.1.x | Completed | Core CLI, templates, plugins, hooks, CI/CD generation |
| 0.2.x | Completed | Web UI completion, Import feature, Test expansion |
| 0.3.x | Completed | Template versioning, inheritance, partials, cost estimation, OPA policy, secrets integration, parallel execution |
| 0.4.x | **Current** | Template linting, template diff, webhook notifications, documentation generation |
| 0.5.x | Planned | Project health dashboard, environment cloning, blueprints, audit logging |
| 0.6.x | Planned | Provider abstractions, project migration |
| 1.0.0 | Future | Workspace profiles, git integration, state browser, cost alerts |

---

## Architecture Overview

```
pmp-cli/
├── src/
│   ├── commands/           # CLI command implementations (25+ commands)
│   ├── collection/         # Project/infrastructure discovery
│   ├── diff/               # Plan diff visualization
│   │   ├── types.rs            # Data structures (DiffChangeType, ResourceChange, etc.)
│   │   ├── parser.rs           # Plan output parser
│   │   └── renderer.rs         # ASCII and HTML renderers
│   ├── executor/           # OpenTofu, None executors
│   ├── hooks/              # Pre/post execution hooks
│   ├── infrastructure/     # Cloud infrastructure import
│   │   ├── cloud_inspector.rs  # pmp-cloud-inspector export parsing + schema validation
│   │   ├── discovery.rs        # Core types and traits
│   │   ├── providers/          # AWS, Azure, GCP, GitHub, GitLab, etc.
│   │   ├── resource_mapper.rs  # Cloud inspector → Terraform type mapping
│   │   ├── config_generator.rs # Import block + provider generation
│   │   ├── validation.rs       # Pre-import validation
│   │   ├── rollback.rs         # Transaction-style rollback on failure
│   │   └── workflow.rs         # Import orchestration
│   ├── opa/                # Native OPA policy integration
│   │   ├── mod.rs              # Module exports
│   │   ├── provider.rs         # OpaProvider trait + data structures
│   │   ├── regorus.rs          # Regorus-based provider implementation
│   │   ├── discovery.rs        # Multi-path policy discovery
│   │   └── compliance.rs       # Compliance report generation
│   ├── schema/             # YAML/JSON validation
│   ├── template/           # Template discovery, rendering, metadata
│   │   ├── discovery.rs        # Template pack and template discovery
│   │   ├── inheritance.rs      # Template inheritance resolution
│   │   ├── metadata.rs         # YAML metadata structures
│   │   ├── partials.rs         # Handlebars partials discovery
│   │   └── renderer.rs         # Handlebars rendering with helpers
│   └── traits/             # Abstractions for testing (filesystem, output, input)
├── doc/                    # Documentation
├── examples/               # Example infrastructures and template packs
└── tests/                  # (Planned) Integration tests
```

**Lines of Code:** ~25,000+ (Rust)
**Dependencies:** clap, serde, handlebars, axum, tokio, anyhow, regorus
