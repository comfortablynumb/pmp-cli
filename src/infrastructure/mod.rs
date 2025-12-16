//! Infrastructure Import Module
//!
//! This module provides functionality to import existing infrastructure resources
//! into OpenTofu/Terraform management. It supports:
//!
//! - Discovering resources from cloud provider APIs (AWS, Azure, GCP)
//! - Manual resource entry when API access is not available
//! - Generating import blocks for OpenTofu
//! - Auto-detecting resource dependencies
//! - Batch imports from configuration files
//!
//! # Usage
//!
//! ```bash
//! # Discover AWS resources interactively
//! pmp import infrastructure discover --provider aws --region us-east-1
//!
//! # Filter by resource type and tags
//! pmp import infrastructure discover --provider aws -t aws_vpc,aws_subnet --tags Environment=prod
//!
//! # Manual import
//! pmp import infrastructure manual aws_vpc vpc-12345 --name main_vpc --generate-config
//!
//! # Batch import from config
//! pmp import infrastructure batch ./import-config.yaml --yes
//! ```
//!
//! # OpenTofu Import Workflow
//!
//! This module uses OpenTofu's import blocks approach:
//!
//! 1. Generate import blocks in `_imports.tf`:
//!    ```hcl
//!    import {
//!      to = aws_vpc.main
//!      id = "vpc-12345"
//!    }
//!    ```
//!
//! 2. Run `tofu plan -generate-config-out=generated.tf` to generate resource config
//!
//! 3. Run `tofu apply` to import resources into state and apply configuration
//!
//! After successful import, import blocks are moved to `_imports.tf.completed`.

pub mod cloud_inspector;
pub mod config_generator;
pub mod discovery;
pub mod error;
pub mod providers;
pub mod registry;
pub mod resource_mapper;
pub mod rollback;
pub mod validation;
pub mod workflow;

// Re-export commonly used types
pub use cloud_inspector::{validate_schema_version, SchemaVersionStatus};
pub use config_generator::{
    generate_required_providers_from_resources, ConfigGenerator, FileOrganization,
};
pub use discovery::{DiscoveredResource, ImportDestination, Provider};
pub use providers::ManualDiscovery;
pub use rollback::RollbackManager;
pub use validation::{validate_import, ValidationReport};
pub use workflow::{ImportWorkflow, ImportWorkflowOptions};
