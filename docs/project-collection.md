# Project Collection

## Overview

A **ProjectCollection** is a new concept in PMP that allows you to manage multiple projects in a centralized way. When you define a `.pmp.yaml` file with `kind: ProjectCollection`, PMP recognizes that directory as a collection of projects and provides additional features for managing them.

## Features

### 1. Centralized Project Registry
- Maintains a registry of all projects created within the collection
- Prevents duplicate projects (same name and kind)
- Tracks project metadata (name, kind, path, category)

### 2. Project Discovery with `pmp find`
Search for projects within the collection using various criteria:
- **By name**: `pmp find --name my-api`
- **By category**: `pmp find --category workload`
- **By kind**: `pmp find --kind Infrastructure`
- **All projects**: `pmp find`

### 3. Automatic Project Registration
When you create a new project inside a ProjectCollection, PMP automatically:
- Detects the collection
- Validates that no duplicate exists
- Registers the project in the collection's `.pmp.yaml`
- Updates the collection metadata

### 4. Organized Folder Structure
Enable `organize_by_category: true` to automatically structure projects by category:
```
my-platform/
├── .pmp.yaml (kind: ProjectCollection)
├── workload/
│   ├── api-gateway/
│   │   └── .pmp.yaml
│   └── auth-service/
│       └── .pmp.yaml
└── infrastructure/
    ├── vpc-network/
    │   └── .pmp.yaml
    └── database-cluster/
        └── .pmp.yaml
```

## Configuration

### Basic ProjectCollection `.pmp.yaml`

```yaml
apiVersion: pmp.io/v1
kind: ProjectCollection
metadata:
  name: my-platform
  description: A collection of infrastructure and application projects
spec:
  # List of projects in this collection
  projects: []

  # Optional: Organize projects in folders by category
  organize_by_category: false
```

### Full Example

```yaml
apiVersion: pmp.io/v1
kind: ProjectCollection
metadata:
  name: my-platform
  description: A complete platform with workloads and infrastructure
spec:
  projects:
    - name: api-gateway
      kind: Workload
      path: workload/api-gateway
      category: workload

    - name: vpc-network
      kind: Infrastructure
      path: infrastructure/vpc-network
      category: infrastructure

  organize_by_category: true
```

## Usage

### Creating a ProjectCollection

1. Create a directory for your collection:
   ```bash
   mkdir my-platform
   cd my-platform
   ```

2. Create a `.pmp.yaml` file with `kind: ProjectCollection`:
   ```bash
   cat > .pmp.yaml << EOF
   apiVersion: pmp.io/v1
   kind: ProjectCollection
   metadata:
     name: my-platform
     description: My platform projects
   spec:
     projects: []
     organize_by_category: true
   EOF
   ```

### Creating Projects in a Collection

Once inside a ProjectCollection directory, simply use `pmp create` as usual:

```bash
cd my-platform
pmp create
```

PMP will:
1. Detect the ProjectCollection
2. Prompt you to select a template
3. Create the project
4. Automatically register it in the collection

If `organize_by_category: true`, projects are created in category-specific folders:
- `workload/my-api/`
- `infrastructure/my-vpc/`

### Finding Projects

Use the `pmp find` command to search for projects:

```bash
# Find all projects
pmp find

# Find by name (case-insensitive substring match)
pmp find --name api

# Find by category
pmp find --category workload

# Find by kind
pmp find --kind Infrastructure
```

**Example output:**
```
Found 2 project(s) in collection 'my-platform':

  Name:     api-gateway
  Kind:     Workload
  Path:     workload/api-gateway
  Category: workload
  Full path: /home/user/my-platform/workload/api-gateway

  Name:     auth-service
  Kind:     Workload
  Path:     workload/auth-service
  Category: workload
  Full path: /home/user/my-platform/workload/auth-service
```

## Project Reference Schema

Each project in the collection's `spec.projects` array has the following structure:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Name of the project |
| `kind` | string | Yes | Kind of project (e.g., "Workload", "Infrastructure") |
| `path` | string | Yes | Relative path from collection root to the project |
| `category` | string | No | Category of the project (e.g., "workload", "infrastructure") |

## Validation

PMP enforces the following validations for ProjectCollections:

1. **Kind Validation**: The `.pmp.yaml` must have `kind: ProjectCollection`
2. **Duplicate Prevention**: No two projects can have the same `name` and `kind` combination
3. **Path Constraints**: Projects must be inside the collection directory

## Benefits

### 1. Better Organization
- Centralized view of all projects
- Consistent naming and structure
- Clear categorization

### 2. Improved Discoverability
- Quickly find projects by name, category, or kind
- No need to manually navigate directories
- See all projects at a glance

### 3. Consistency Enforcement
- Prevent naming conflicts
- Maintain metadata standards
- Automated registration

### 4. Scalability
- Manage dozens or hundreds of projects
- Hierarchical organization by category
- Easy to add new projects

## Best Practices

### 1. Enable Category Organization
For collections with many projects, set `organize_by_category: true` to keep things tidy:

```yaml
spec:
  organize_by_category: true
```

### 2. Use Descriptive Names
Choose clear, descriptive names for your collection:

```yaml
metadata:
  name: ecommerce-platform
  description: All infrastructure and services for our e-commerce platform
```

### 3. Consistent Categories
Use consistent category names across your templates:
- `workload` for applications
- `infrastructure` for infrastructure resources
- `database` for database resources
- etc.

### 4. Regular Maintenance
Periodically review your collection to:
- Remove projects that are no longer in use
- Update descriptions
- Verify paths are correct

## Advanced Use Cases

### Multi-Team Platform
```yaml
apiVersion: pmp.io/v1
kind: ProjectCollection
metadata:
  name: company-platform
  description: All teams' infrastructure projects
spec:
  organize_by_category: true
  projects:
    # Team A projects
    - name: team-a-api
      kind: Workload
      category: team-a
      path: team-a/api

    # Team B projects
    - name: team-b-service
      kind: Workload
      category: team-b
      path: team-b/service

    # Shared infrastructure
    - name: shared-vpc
      kind: Infrastructure
      category: shared
      path: shared/vpc
```

### Environment-Based Organization
```yaml
spec:
  organize_by_category: true
  projects:
    - name: prod-api
      kind: Workload
      category: production
      path: production/api

    - name: staging-api
      kind: Workload
      category: staging
      path: staging/api

    - name: dev-api
      kind: Workload
      category: development
      path: development/api
```

## Troubleshooting

### Project Not Registered Automatically

**Cause**: The project was created outside the collection directory, or the collection couldn't be found.

**Solution**: Make sure you're running `pmp create` from within the ProjectCollection directory or a subdirectory.

### Duplicate Project Error

**Cause**: A project with the same name and kind already exists in the collection.

**Solution**: Either choose a different name, or use a different kind for the project.

### Path Not Inside Collection

**Cause**: Trying to register a project that's outside the collection directory.

**Solution**: Create projects inside the collection directory, or manually update the collection's `.pmp.yaml`.
