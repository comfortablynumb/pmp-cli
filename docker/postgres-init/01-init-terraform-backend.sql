-- Initialize the terraform_backend database with required schema
-- This script runs automatically when the PostgreSQL container starts for the first time

-- Create schema for terraform state
CREATE SCHEMA IF NOT EXISTS terraform_remote_state;

-- Grant all privileges on the schema to the terraform user
GRANT ALL PRIVILEGES ON SCHEMA terraform_remote_state TO terraform;

-- Set default privileges for future tables in the schema
-- This allows OpenTofu/Terraform to create tables dynamically for each project
ALTER DEFAULT PRIVILEGES IN SCHEMA terraform_remote_state GRANT ALL ON TABLES TO terraform;
ALTER DEFAULT PRIVILEGES IN SCHEMA terraform_remote_state GRANT ALL ON SEQUENCES TO terraform;

-- Note: Individual state tables are created automatically by OpenTofu/Terraform
-- Each project gets its own table with a unique name (format: tf_{sha1_hash})
-- This provides isolation between projects and prevents state conflicts

-- Output confirmation
\echo 'Terraform backend schema initialized successfully'
\echo 'State tables will be created automatically per project'
