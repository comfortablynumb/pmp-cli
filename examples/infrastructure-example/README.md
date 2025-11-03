# Infrastructure Example with PostgreSQL Backend

This example demonstrates how to configure PMP to use PostgreSQL as a remote state backend for OpenTofu/Terraform. This setup enables team collaboration with proper state locking and persistence.

## Prerequisites

- Docker and Docker Compose installed
- PMP CLI built and available
- OpenTofu or Terraform installed

## Quick Start

### 1. Start PostgreSQL Backend

From the project root directory, start the PostgreSQL container:

```bash
docker-compose up -d
```

This will:
- Start PostgreSQL 16 on port 5432
- Create a database named `terraform_backend`
- Initialize the `terraform_remote_state` schema
- Configure health checks to ensure the database is ready

Verify the container is running and healthy:

```bash
docker-compose ps
```

You should see the `pmp-postgres-backend` container with a "healthy" status.

### 2. Verify Database Setup

You can connect to the database to verify the setup:

```bash
docker-compose exec postgres psql -U terraform -d terraform_backend -c "\dn"
```

You should see the `terraform_remote_state` schema listed.

### 3. Use the Project Collection

Navigate to this example directory:

```bash
cd examples/infrastructure-example
```

Create a new project using PMP:

```bash
cargo run -- create
```

PMP will:
1. Detect the `.pmp.infrastructure.yaml` configuration
2. Show available template packs and templates
3. Prompt for environment selection (development, staging, or production)
4. Generate project files with PostgreSQL backend configuration

### 4. Generated Backend Configuration

When you create a project, PMP automatically generates a `_common.tf` file in the environment directory with the backend configuration:

```hcl
terraform {
  backend "pg" {
    conn_str    = "postgres://terraform:terraform@localhost:5432/terraform_backend?sslmode=disable"
    schema_name = "terraform_remote_state"
    table_name  = "tf_a1b2c3d4e5f6..."  # Auto-generated unique table name
  }
}
```

**Table Name Isolation**: Each project gets its own PostgreSQL table for state storage. The table name is automatically generated using a SHA1 hash of the project's unique identifier:

- **Format**: `tf_{sha1_hex_lowercase}`
- **Input**: `{apiVersion}_{kind}__{environment}__{projectName}`
- **Example**: For a Database project named "my-db" in development environment:
  - Input: `pmp.io/v1_Database__development__my-db`
  - Table: `tf_a7f8e4c9b2d1...` (43 characters total)

This ensures:
- **Isolation**: Each project has its own state table
- **No Collisions**: SHA1 hash guarantees uniqueness
- **PostgreSQL Compliance**: Table names never exceed 63 characters
- **Deterministic**: Same project always gets the same table name

### 5. Initialize and Use OpenTofu/Terraform

Navigate to your generated project's environment directory:

```bash
cd projects/{resource_kind}/{project_name}/environments/{environment_name}
```

Initialize OpenTofu/Terraform:

```bash
tofu init
# or
terraform init
```

The state will now be stored in PostgreSQL instead of local files.

## Configuration Details

### Infrastructure Settings

- **Allowed Resource Kinds**: KubernetesWorkload, Database
- **Environments**: development, staging, production
- **Backend Type**: PostgreSQL (pg)
- **Connection**: localhost:5432/terraform_backend

### PostgreSQL Backend Features

- **Table Isolation**: Each project gets its own table (auto-generated SHA1-based name)
- **State Locking**: Automatic locking prevents concurrent modifications
- **Persistence**: State survives container restarts
- **Schema Isolation**: Uses dedicated `terraform_remote_state` schema
- **Security**: Configurable credentials (default: terraform/terraform)
- **No Collisions**: Unique table names prevent state conflicts between projects

## Customization

### Change Database Credentials

Edit `docker-compose.yaml`:

```yaml
environment:
  POSTGRES_USER: your_user
  POSTGRES_PASSWORD: your_password
  POSTGRES_DB: your_database
```

Update the connection string in `.pmp.infrastructure.yaml`:

```yaml
spec:
  executor:
    config:
      backend:
        conn_str: postgres://your_user:your_password@localhost:5432/your_database?sslmode=disable
```

### Add Additional Environments

Edit `.pmp.infrastructure.yaml` and add new environment definitions:

```yaml
spec:
  environments:
    development:
      name: Development
      description: Development environment
    qa:
      name: QA
      description: Quality assurance environment
    # ... add more as needed
```

### Use Different Resource Kinds

Add more resource kinds to allow different template types:

```yaml
spec:
  resource_kinds:
    - apiVersion: pmp.io/v1
      kind: KubernetesWorkload
    - apiVersion: pmp.io/v1
      kind: Database
    - apiVersion: pmp.io/v1
      kind: YourCustomKind
```

## Troubleshooting

### Container Not Starting

Check container logs:

```bash
docker-compose logs postgres
```

### Connection Issues

Verify PostgreSQL is accepting connections:

```bash
docker-compose exec postgres pg_isready -U terraform -d terraform_backend
```

### Reset Database

To start fresh, stop and remove the container:

```bash
docker-compose down -v
docker-compose up -d
```

**Warning**: This will delete all stored Terraform/OpenTofu state!

## Cleanup

Stop and remove the PostgreSQL container:

```bash
docker-compose down
```

To also remove the volume and all data:

```bash
docker-compose down -v
```

## Production Considerations

This example uses simple credentials for demonstration. For production use:

1. **Use strong passwords**: Don't use default credentials
2. **Enable SSL**: Set `sslmode=require` in the connection string
3. **Network security**: Don't expose port 5432 publicly
4. **Backup strategy**: Implement regular backups of the PostgreSQL database
5. **High availability**: Consider using managed PostgreSQL services (AWS RDS, Azure Database, etc.)
6. **Access control**: Use separate credentials per team/environment
7. **Monitoring**: Add monitoring for database health and state operations

## Additional Resources

- [OpenTofu PostgreSQL Backend Documentation](https://opentofu.org/docs/language/settings/backends/pg/)
- [Terraform PostgreSQL Backend Documentation](https://www.terraform.io/docs/language/settings/backends/pg.html)
- [PMP Documentation](../../CLAUDE.md)
