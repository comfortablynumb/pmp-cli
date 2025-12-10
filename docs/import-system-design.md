# PMP Import System Design

## Overview

The import system allows users to bring existing infrastructure under PMP management. This is critical for adoption as most users have existing infrastructure they want to manage.

## Use Cases

1. **Migrate from standalone Terraform/OpenTofu**
   - User has existing .tf files and .tfstate
   - Want to organize into PMP project structure
   - Need to preserve state and resources

2. **Import existing cloud resources**
   - Resources created manually or via other tools
   - Want to manage them with Terraform + PMP
   - Need to generate Terraform code

3. **Bulk migration**
   - Multiple projects to import
   - Batch operation
   - Preserve relationships between projects

4. **Team onboarding**
   - New team member has existing infra
   - Need to standardize on PMP
   - Minimal disruption to running systems

## Command Structure

```bash
# Import existing Terraform/OpenTofu project
pmp import project <path-to-terraform-dir> [options]

# Import from existing state file
pmp import state <path-to-state-file> [options]

# Import specific resources
pmp import resource <resource-address> [options]

# Discover and import cloud resources
pmp import discover [options]

# Bulk import multiple projects
pmp import bulk <config-file>
```

## Import Workflows

### Workflow 1: Import Existing Terraform Project

```bash
pmp import project ./my-terraform-infra

# Interactive prompts:
# 1. Detected resources: [list of resources from state]
# 2. Match to template? [Yes/No/Create Custom]
# 3. If Yes: Select template from available templates
# 4. If No: Create custom template (extract inputs from variables.tf)
# 5. Project name: [auto-suggest from directory name]
# 6. Environment: [select from infrastructure.yaml]
# 7. Copy or move files? [copy/move/symlink]
# 8. Import state? [yes/no]
# 9. Preview changes [show file structure that will be created]
# 10. Confirm import
```

**Result:**
```
collection/
└── projects/
    └── my-terraform-infra/
        ├── .pmp.project.yaml          # Created
        └── environments/
            └── production/
                ├── .pmp.environment.yaml   # Created
                ├── main.tf                 # Copied/moved
                ├── variables.tf            # Copied/moved
                ├── outputs.tf              # Copied/moved
                ├── terraform.tfstate       # Imported
                └── _common.tf              # Generated (backend config)
```

### Workflow 2: Import from State File

```bash
pmp import state ./terraform.tfstate --project-name my-app

# Interactive prompts:
# 1. Analyze state file
# 2. Detected resources: [list]
# 3. Source Terraform files location: [path or skip]
# 4. Match to template or create custom
# 5. Environment selection
# 6. Import configuration
```

### Workflow 3: Import Specific Resources

```bash
# Import single resource
pmp import resource aws_s3_bucket.my_bucket \
  --project my-storage \
  --environment production

# Import multiple resources
pmp import resource aws_s3_bucket.my_bucket,aws_s3_bucket_policy.my_policy \
  --project my-storage \
  --environment production
```

### Workflow 4: Bulk Import

Create import configuration file:

```yaml
# import-config.yaml
apiVersion: pmp.io/v1
kind: ImportConfig
metadata:
  name: bulk-import-2024

spec:
  source_type: terraform_directories  # or: state_files, cloud_provider

  projects:
    - name: vpc-infrastructure
      source_path: ./existing-infra/vpc
      environment: production
      template:
        match_strategy: auto  # auto, manual, create_custom
        template_pack: aws
        template_name: vpc
      import_strategy:
        files: copy  # copy, move, symlink
        state: import  # import, skip

    - name: database-layer
      source_path: ./existing-infra/rds
      environment: production
      template:
        match_strategy: create_custom

    - name: application-servers
      source_path: ./existing-infra/ec2
      environment: production
      dependencies:
        - project: vpc-infrastructure
          dependency_name: vpc
```

```bash
pmp import bulk ./import-config.yaml
```

## Import Strategies

### 1. Template Matching

**Auto-match:**
- Analyze resources in state
- Compare against available templates
- Score similarity
- Suggest best matches

**Manual selection:**
- User chooses from available templates
- System validates compatibility
- Warns about missing/extra resources

**Create custom:**
- Extract inputs from variables.tf
- Generate .pmp.template.yaml
- Store in custom template pack location

### 2. File Handling

**Copy:**
- Copy .tf files to project directory
- Original files unchanged
- Safe, but duplicates files

**Move:**
- Move .tf files to project directory
- Original directory can be deleted
- Clean migration

**Symlink:**
- Symlink to original files
- Useful for gradual migration
- Files remain in original location

**Template conversion:**
- Convert to .tf.hbs templates
- Extract variables
- Most PMP-native approach

### 3. State Import

**Direct import:**
- Copy state file to project directory
- Update backend configuration
- Immediate management

**Remote state:**
- Configure remote backend
- Migrate state to remote
- Production-recommended

**Terraform import:**
- For resources not in state
- Use `terraform import` commands
- Generate import blocks (TF 1.5+)

## Technical Implementation

### Command Structure

```rust
// src/commands/import.rs
pub struct ImportCommand {
    // Main import command
}

pub enum ImportSubcommand {
    Project(ProjectImportArgs),
    State(StateImportArgs),
    Resource(ResourceImportArgs),
    Discover(DiscoverArgs),
    Bulk(BulkImportArgs),
}

// src/commands/import/project.rs
pub struct ProjectImporter {
    ctx: Context,
    source_path: PathBuf,
    project_name: String,
    environment: String,
    template_match: TemplateMatch,
    import_strategy: ImportStrategy,
}

impl ProjectImporter {
    pub fn analyze_source(&self) -> Result<ProjectAnalysis> {
        // Scan directory for .tf files
        // Parse state file if exists
        // Extract variables
        // Detect resources
    }

    pub fn match_template(&self, analysis: &ProjectAnalysis) -> Result<TemplateMatch> {
        // Score against available templates
        // Return best matches with confidence scores
    }

    pub fn create_custom_template(&self, analysis: &ProjectAnalysis) -> Result<Template> {
        // Generate .pmp.template.yaml
        // Extract inputs from variables.tf
        // Create template structure
    }

    pub fn import_project(&self) -> Result<()> {
        // Create project structure
        // Copy/move/symlink files
        // Generate .pmp.project.yaml
        // Generate .pmp.environment.yaml
        // Import state if requested
    }
}
```

### State File Analysis

```rust
// src/import/state_analyzer.rs
pub struct StateAnalyzer;

impl StateAnalyzer {
    pub fn analyze(state_path: &Path) -> Result<StateAnalysis> {
        // Parse JSON state file
        // Extract resources
        // Detect providers
        // Identify resource types
    }

    pub fn extract_resources(&self, state: &Value) -> Vec<ResourceInfo> {
        // Parse resources from state
        // Get addresses, types, attributes
    }
}

pub struct StateAnalysis {
    pub terraform_version: String,
    pub resources: Vec<ResourceInfo>,
    pub providers: Vec<ProviderInfo>,
    pub outputs: HashMap<String, Value>,
}

pub struct ResourceInfo {
    pub address: String,
    pub resource_type: String,
    pub provider: String,
    pub attributes: HashMap<String, Value>,
}
```

### Template Matching

```rust
// src/import/template_matcher.rs
pub struct TemplateMatcher {
    templates: Vec<Template>,
}

impl TemplateMatcher {
    pub fn find_matches(&self, analysis: &StateAnalysis) -> Vec<TemplateMatch> {
        // Compare resources against template specs
        // Score similarity
        // Return ranked matches
    }

    fn calculate_similarity(&self, template: &Template, resources: &[ResourceInfo]) -> f64 {
        // Calculate similarity score (0.0 - 1.0)
        // Based on resource types, counts, attributes
    }
}

pub struct TemplateMatch {
    pub template_pack: String,
    pub template_name: String,
    pub confidence: f64,  // 0.0 - 1.0
    pub matching_resources: Vec<String>,
    pub missing_resources: Vec<String>,
    pub extra_resources: Vec<String>,
}
```

### Terraform File Parser

```rust
// src/import/tf_parser.rs
pub struct TerraformParser;

impl TerraformParser {
    pub fn parse_directory(&self, path: &Path) -> Result<TerraformProject> {
        // Parse .tf files using HCL parser
        // Extract variables, resources, outputs
    }

    pub fn extract_variables(&self, files: &[HclFile]) -> Vec<VariableDefinition> {
        // Find variable blocks
        // Extract name, type, description, default
    }

    pub fn generate_pmp_inputs(&self, variables: &[VariableDefinition]) -> Vec<InputDefinition> {
        // Convert Terraform variables to PMP inputs
        // Map types: string, number, bool, list, map
    }
}
```

## Import Configuration File Schema

```yaml
apiVersion: pmp.io/v1
kind: ImportConfig
metadata:
  name: import-name
  description: Optional description

spec:
  # Source type
  source_type: terraform_directories  # terraform_directories, state_files, cloud_provider

  # Default settings applied to all projects
  defaults:
    import_strategy:
      files: copy  # copy, move, symlink, template_convert
      state: import  # import, skip, remote_migrate
    template:
      match_strategy: auto  # auto, manual, create_custom
      create_custom_pack: imported-templates  # Template pack for custom templates

  # Projects to import
  projects:
    - name: project-name
      source_path: ./path/to/terraform
      environment: production

      # Template matching (optional, uses defaults if not specified)
      template:
        match_strategy: manual
        template_pack: aws
        template_name: vpc

      # Import strategy (optional, uses defaults if not specified)
      import_strategy:
        files: copy
        state: import

      # Dependencies (optional)
      dependencies:
        - project: other-project
          dependency_name: vpc

      # Custom inputs (optional)
      inputs:
        input_name: value

      # Metadata (optional)
      metadata:
        description: Custom description
        labels:
          team: platform
          env: prod

  # Cloud provider discovery (alternative to projects list)
  cloud_discovery:
    provider: aws  # aws, azure, gcp
    region: us-east-1
    filters:
      tags:
        managed-by: terraform
    resource_types:
      - aws_vpc
      - aws_subnet
      - aws_security_group
```

## User Experience

### Import Wizard (Interactive Mode)

```
$ pmp import project ./my-terraform-infra

🔍 Analyzing Terraform project...
   ✓ Found 15 .tf files
   ✓ Found terraform.tfstate
   ✓ Detected 23 resources across 3 providers

📊 Analysis Summary:
   Providers: aws (v5.0), random (v3.5)
   Resources:
     - aws_vpc (1)
     - aws_subnet (6)
     - aws_security_group (3)
     - aws_instance (10)
     - random_password (3)

🎯 Template Matching:
   Found 2 potential matches:

   1. aws/ec2-with-vpc (85% match)
      ✓ Matching: 18 resources
      ⚠ Missing: 2 resources (aws_route_table)
      ⚠ Extra: 3 resources (random_password)

   2. aws/full-stack (72% match)
      ✓ Matching: 15 resources
      ⚠ Missing: 5 resources
      ⚠ Extra: 3 resources

? Select template matching strategy:
  ❯ Use template: aws/ec2-with-vpc (recommended)
    Create custom template
    Skip template matching (import as-is)

? Project name: my-terraform-infra
? Environment: production
? File handling strategy:
  ❯ Copy files (safe, creates duplicate)
    Move files (clean, deletes original)
    Symlink files (gradual migration)
    Convert to templates (most PMP-native)

? Import state file? (Y/n) Y

📝 Preview:
   Will create:
   ✓ collection/projects/my-terraform-infra/.pmp.project.yaml
   ✓ collection/projects/my-terraform-infra/environments/production/.pmp.environment.yaml
   ✓ collection/projects/my-terraform-infra/environments/production/*.tf (15 files)
   ✓ collection/projects/my-terraform-infra/environments/production/terraform.tfstate
   ✓ collection/projects/my-terraform-infra/environments/production/_common.tf

? Proceed with import? (Y/n) Y

✅ Import completed successfully!

   Next steps:
   1. Review generated files in: collection/projects/my-terraform-infra
   2. Run 'pmp preview' to verify state
   3. Run 'pmp apply' to manage with PMP

   ⚠️  Note: Original files remain at ./my-terraform-infra
```

### Dry-run Mode

```bash
pmp import project ./my-infra --dry-run

# Shows what would happen without making changes
# Useful for validation before actual import
```

## Error Handling

### Common Issues

1. **State file version mismatch**
   - Detect Terraform version in state
   - Warn if incompatible with OpenTofu
   - Offer migration options

2. **Missing provider configuration**
   - Detect required providers
   - Prompt for provider configuration
   - Generate provider blocks

3. **Resource conflicts**
   - Check if resources already exist in PMP
   - Offer merge or skip options

4. **Invalid template match**
   - Template doesn't support all resources
   - Offer custom template creation
   - Allow partial import

## Implementation Phases

### Phase 1: Basic Project Import
- Import existing Terraform directory
- Copy files to PMP structure
- Generate project metadata
- Import state file

### Phase 2: Template Matching
- Analyze resources in state
- Match against available templates
- Score similarity
- Suggest best matches

### Phase 3: Custom Template Generation
- Parse variables.tf
- Generate .pmp.template.yaml
- Extract inputs automatically
- Create custom template pack

### Phase 4: Resource Import
- Import specific resources via addresses
- Generate Terraform import commands
- Support TF 1.5+ import blocks

### Phase 5: Cloud Discovery
- Scan cloud providers for resources
- Detect unmanaged resources
- Generate Terraform code
- Import into PMP

### Phase 6: Bulk Import
- Parse import configuration file
- Import multiple projects
- Handle dependencies
- Batch operations

## Testing Strategy

### Unit Tests
- State file parsing
- Template matching algorithm
- Variable extraction
- File operations

### Integration Tests
- Import sample Terraform projects
- Verify generated structure
- Test state import
- Validate dependencies

### E2E Tests
- Complete import workflows
- Multi-project imports
- Template matching accuracy
- Error scenarios

## Documentation

### User Guide
- When to use import
- Import workflows
- Best practices
- Troubleshooting

### API Reference
- Command options
- Import config schema
- Examples
- Migration guides

### Migration Guides
- From standalone Terraform
- From other IaC tools
- Team onboarding
- Bulk migration strategies
