# PMP Roadmap

This document outlines the development plan for PMP (Poor Man's Platform).

## Current Status

PMP is under active development. Core features are stable while advanced features are being refined.

## Implemented Features

### Core (Stable)

- [x] **Template System**
  - [x] Handlebars template rendering
  - [x] 25+ input types (string, number, boolean, select, object, etc.)
  - [x] Conditional inputs (`show_if`)
  - [x] Variable interpolation in defaults
  - [x] Environment-specific overrides
  - [x] Template scaffolding command

- [x] **Project Management**
  - [x] Interactive project creation
  - [x] Project discovery and listing
  - [x] Project cloning
  - [x] Project updates from templates
  - [x] Naming validation

- [x] **Executors**
  - [x] OpenTofu/Terraform executor
  - [x] None executor (dependency-only projects)
  - [x] Backend configuration (S3, Azure, GCS, K8s, PG, Consul, HTTP, Local)
  - [x] Command options configuration

- [x] **Operations**
  - [x] Preview (plan)
  - [x] Apply
  - [x] Destroy
  - [x] Refresh
  - [x] Test

- [x] **Dependency System**
  - [x] Dependency declaration in templates
  - [x] Dependency graph building
  - [x] Topological execution order
  - [x] Graph visualization (ASCII, Mermaid, DOT)
  - [x] Impact analysis
  - [x] Dependency validation

- [x] **Hooks System**
  - [x] Confirm hooks
  - [x] Set environment hooks
  - [x] Command hooks
  - [x] Pre/post phases for all operations
  - [x] Multi-level hooks (infrastructure, template, environment)

- [x] **Environment Management**
  - [x] Environment comparison (diff)
  - [x] Environment promotion
  - [x] Variable synchronization
  - [x] Environment-specific backends

- [x] **Plugin System**
  - [x] Plugin discovery
  - [x] Allowed plugins (user-selected)
  - [x] Installed plugins (auto-installed)
  - [x] Plugin input configuration
  - [x] Plugin dependencies

### CI/CD (Stable)

- [x] **Pipeline Generation**
  - [x] GitHub Actions
  - [x] GitLab CI
  - [x] Jenkins
  - [x] Dynamic pipelines (change detection)
  - [x] Static pipelines (deploy all)

- [x] **Change Detection**
  - [x] Git-based change detection
  - [x] Environment filtering
  - [x] JSON/YAML output formats

### State & Drift (Stable)

- [x] **State Management**
  - [x] State listing
  - [x] State locking/unlocking
  - [x] State backup
  - [x] State restore
  - [x] State migration
  - [x] State sync

- [x] **Drift Detection**
  - [x] Drift detection
  - [x] Drift reporting
  - [x] Drift reconciliation

### Policy & Security (Stable)

- [x] **Policy Validation**
  - [x] Naming convention checks
  - [x] Tag validation
  - [x] Dependency validation
  - [x] Secret detection

- [x] **Security Scanning**
  - [x] tfsec integration
  - [x] checkov integration
  - [x] trivy integration

### Search (Stable)

- [x] Search by tags
- [x] Search by resource type
- [x] Search by name pattern
- [x] Search by outputs

## Work in Progress

### Web UI `[WIP]`

- [x] Basic HTTP server
- [x] Template pack listing API
- [x] Project listing API
- [x] Infrastructure loading API
- [x] Preview/Apply/Destroy APIs
- [ ] Project creation via API (partial)
- [ ] Interactive forms for inputs
- [ ] Real-time operation output
- [ ] Dependency graph visualization
- [ ] Dashboard with project status

### Import `[WIP]`

- [x] Project import framework
- [x] Terraform project analysis
- [x] Provider detection
- [x] Resource inventory
- [ ] State file import
- [ ] Resource-level import
- [ ] Bulk import from config
- [ ] Template matching

## Planned Features

### Short-term

1. **Complete Web UI**
   - Interactive project creation wizard
   - Real-time operation streaming
   - Visual dependency graph

2. **Complete Import**
   - State file migration
   - Automatic template matching
   - Bulk import configuration

3. **Enhanced Templates**
   - Template versioning
   - Template inheritance
   - Template marketplace integration

### Medium-term

1. **Cost Estimation**
   - Pre-apply cost estimates
   - Cost tracking over time
   - Budget alerts

2. **Advanced Policy**
   - Custom policy definitions (OPA/Rego)
   - Policy-as-code integration
   - Compliance reporting

3. **Multi-Cloud**
   - Cross-provider templates
   - Unified resource naming
   - Multi-region deployments

### Long-term

1. **Template Marketplace**
   - Public template registry
   - Template sharing
   - Version management
   - Documentation generation

2. **GitOps Integration**
   - ArgoCD integration
   - Flux integration
   - Automatic drift reconciliation

3. **Advanced Monitoring**
   - Resource health tracking
   - Deployment history
   - Rollback capabilities

## Contributing

Contributions are welcome. Priority areas:

1. Web UI completion
2. Import functionality
3. Template packs for common scenarios
4. Documentation improvements
5. Test coverage

See the repository issues for specific tasks.

## Version History

| Version | Status | Key Features |
|---------|--------|--------------|
| 0.1.x | Current | Core features, CLI, basic plugins |
| 0.2.x | Planned | Web UI, Import, Cost estimation |
| 0.3.x | Planned | Policy framework, Template marketplace |
| 1.0.0 | Future | Production-ready release |
