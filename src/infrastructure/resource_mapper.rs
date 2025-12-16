//! Resource Type Mapper
//!
//! Maps pmp-cloud-inspector resource types (format: `provider:service:resource`)
//! to Terraform/OpenTofu resource types (format: `provider_resource_type`).

use std::collections::HashMap;

use lazy_static::lazy_static;

use super::discovery::{DependencyType, Provider, ResourceDependency};

/// Maps cloud inspector resource type to Terraform resource type
pub fn map_resource_type(cloud_inspector_type: &str) -> Option<TerraformResourceType> {
    RESOURCE_TYPE_MAP.get(cloud_inspector_type).cloned()
}

/// Get the provider from a cloud inspector resource type
pub fn extract_provider(cloud_inspector_type: &str) -> Option<Provider> {
    let parts: Vec<&str> = cloud_inspector_type.split(':').collect();

    if parts.is_empty() {
        return None;
    }

    Provider::from_str(parts[0])
}

/// Terraform resource type information
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct TerraformResourceType {
    /// The Terraform resource type (e.g., "aws_vpc", "azurerm_virtual_network")
    pub tf_type: &'static str,
    /// The Terraform provider name
    pub provider: &'static str,
    /// Whether this resource supports import
    pub importable: bool,
    /// The ID format description for import
    pub id_format: &'static str,
}

impl TerraformResourceType {
    const fn new(
        tf_type: &'static str,
        provider: &'static str,
        importable: bool,
        id_format: &'static str,
    ) -> Self {
        Self {
            tf_type,
            provider,
            importable,
            id_format,
        }
    }
}

/// Information about a Terraform provider for generating required_providers blocks
#[derive(Debug, Clone)]
pub struct ProviderInfo {
    /// Provider source (e.g., "hashicorp/aws")
    pub source: &'static str,
    /// Default version constraint (e.g., "~> 5.0")
    pub default_version: &'static str,
    /// Terraform provider name for the provider block (e.g., "aws", "azurerm")
    pub tf_name: &'static str,
}

impl ProviderInfo {
    const fn new(
        source: &'static str,
        default_version: &'static str,
        tf_name: &'static str,
    ) -> Self {
        Self {
            source,
            default_version,
            tf_name,
        }
    }
}

/// Get provider info for generating required_providers block
///
/// Returns None for providers without known Terraform provider sources
pub fn get_provider_info(provider: &Provider) -> Option<ProviderInfo> {
    match provider {
        Provider::Aws => Some(ProviderInfo::new(
            "hashicorp/aws",
            "~> 5.0",
            "aws",
        )),
        Provider::Azure => Some(ProviderInfo::new(
            "hashicorp/azurerm",
            "~> 3.0",
            "azurerm",
        )),
        Provider::Gcp => Some(ProviderInfo::new(
            "hashicorp/google",
            "~> 5.0",
            "google",
        )),
        Provider::Github => Some(ProviderInfo::new(
            "integrations/github",
            "~> 6.0",
            "github",
        )),
        Provider::Gitlab => Some(ProviderInfo::new(
            "gitlabhq/gitlab",
            "~> 16.0",
            "gitlab",
        )),
        Provider::Jfrog => Some(ProviderInfo::new(
            "jfrog/artifactory",
            "~> 10.0",
            "artifactory",
        )),
        Provider::Okta => Some(ProviderInfo::new(
            "okta/okta",
            "~> 4.0",
            "okta",
        )),
        Provider::Auth0 => Some(ProviderInfo::new(
            "auth0/auth0",
            "~> 1.0",
            "auth0",
        )),
        Provider::Jira => None, // No stable Terraform provider
        Provider::Opsgenie => Some(ProviderInfo::new(
            "opsgenie/opsgenie",
            "~> 0.6",
            "opsgenie",
        )),
    }
}

/// Convert cloud inspector relationships to PMP dependencies
pub fn convert_relationships(
    relationships: &[super::cloud_inspector::ResourceRelationship],
) -> Vec<ResourceDependency> {
    relationships
        .iter()
        .filter_map(|rel| {
            let tf_type = map_resource_type(&rel.target_type)?;
            let dependency_type = match rel.relationship_type {
                super::cloud_inspector::RelationType::BelongsTo => DependencyType::Parent,
                super::cloud_inspector::RelationType::Contains => DependencyType::Reference,
                super::cloud_inspector::RelationType::AttachedTo => DependencyType::Reference,
                super::cloud_inspector::RelationType::Assumes => DependencyType::Reference,
                super::cloud_inspector::RelationType::HasAccess => DependencyType::Association,
                super::cloud_inspector::RelationType::References => DependencyType::Reference,
                super::cloud_inspector::RelationType::DependsOn => DependencyType::Reference,
            };

            Some(ResourceDependency {
                resource_type: tf_type.tf_type.to_string(),
                resource_id: rel.target_id.clone(),
                relationship: dependency_type,
                description: None,
            })
        })
        .collect()
}

/// Build the resource type mapping
fn build_resource_type_map() -> HashMap<&'static str, TerraformResourceType> {
    let mut m = HashMap::new();

    // AWS EC2
    m.insert("aws:ec2:instance", TerraformResourceType::new("aws_instance", "aws", true, "Instance ID"));
    m.insert("aws:ec2:vpc", TerraformResourceType::new("aws_vpc", "aws", true, "VPC ID"));
    m.insert("aws:ec2:subnet", TerraformResourceType::new("aws_subnet", "aws", true, "Subnet ID"));
    m.insert("aws:ec2:security-group", TerraformResourceType::new("aws_security_group", "aws", true, "Security Group ID"));

    // AWS S3
    m.insert("aws:s3:bucket", TerraformResourceType::new("aws_s3_bucket", "aws", true, "Bucket name"));

    // AWS RDS
    m.insert("aws:rds:instance", TerraformResourceType::new("aws_db_instance", "aws", true, "DB Instance Identifier"));
    m.insert("aws:rds:cluster", TerraformResourceType::new("aws_rds_cluster", "aws", true, "Cluster Identifier"));

    // AWS Lambda
    m.insert("aws:lambda:function", TerraformResourceType::new("aws_lambda_function", "aws", true, "Function name"));

    // AWS IAM
    m.insert("aws:iam:user", TerraformResourceType::new("aws_iam_user", "aws", true, "User name"));
    m.insert("aws:iam:role", TerraformResourceType::new("aws_iam_role", "aws", true, "Role name"));

    // AWS ECS
    m.insert("aws:ecs:cluster", TerraformResourceType::new("aws_ecs_cluster", "aws", true, "Cluster ARN"));
    m.insert("aws:ecs:service", TerraformResourceType::new("aws_ecs_service", "aws", true, "cluster/service"));
    m.insert("aws:ecs:taskdefinition", TerraformResourceType::new("aws_ecs_task_definition", "aws", true, "Task Definition ARN"));

    // AWS EKS
    m.insert("aws:eks:cluster", TerraformResourceType::new("aws_eks_cluster", "aws", true, "Cluster name"));

    // AWS DynamoDB
    m.insert("aws:dynamodb:table", TerraformResourceType::new("aws_dynamodb_table", "aws", true, "Table name"));

    // AWS SNS/SQS
    m.insert("aws:sns:topic", TerraformResourceType::new("aws_sns_topic", "aws", true, "Topic ARN"));
    m.insert("aws:sqs:queue", TerraformResourceType::new("aws_sqs_queue", "aws", true, "Queue URL"));

    // AWS Secrets Manager
    m.insert("aws:secretsmanager:secret", TerraformResourceType::new("aws_secretsmanager_secret", "aws", true, "Secret ARN"));

    // AWS CloudWatch
    m.insert("aws:cloudwatch:alarm", TerraformResourceType::new("aws_cloudwatch_metric_alarm", "aws", true, "Alarm name"));
    m.insert("aws:logs:loggroup", TerraformResourceType::new("aws_cloudwatch_log_group", "aws", true, "Log group name"));

    // AWS Step Functions
    m.insert("aws:states:statemachine", TerraformResourceType::new("aws_sfn_state_machine", "aws", true, "State Machine ARN"));

    // AWS ElastiCache / MemoryDB
    m.insert("aws:elasticache:cluster", TerraformResourceType::new("aws_elasticache_cluster", "aws", true, "Cluster ID"));
    m.insert("aws:memorydb:cluster", TerraformResourceType::new("aws_memorydb_cluster", "aws", true, "Cluster name"));

    // AWS CloudFront
    m.insert("aws:cloudfront:distribution", TerraformResourceType::new("aws_cloudfront_distribution", "aws", true, "Distribution ID"));

    // AWS API Gateway
    m.insert("aws:apigateway:api", TerraformResourceType::new("aws_api_gateway_rest_api", "aws", true, "API ID"));

    // AWS ELB
    m.insert("aws:elb:classic", TerraformResourceType::new("aws_elb", "aws", true, "ELB name"));
    m.insert("aws:elb:application", TerraformResourceType::new("aws_lb", "aws", true, "ALB ARN"));
    m.insert("aws:elb:network", TerraformResourceType::new("aws_lb", "aws", true, "NLB ARN"));

    // AWS ECR
    m.insert("aws:ecr:repository", TerraformResourceType::new("aws_ecr_repository", "aws", true, "Repository name"));

    // Azure
    m.insert("azure:resourcegroup", TerraformResourceType::new("azurerm_resource_group", "azurerm", true, "Resource Group ID"));
    m.insert("azure:compute:vm", TerraformResourceType::new("azurerm_virtual_machine", "azurerm", true, "VM ID"));
    m.insert("azure:network:vnet", TerraformResourceType::new("azurerm_virtual_network", "azurerm", true, "VNet ID"));
    m.insert("azure:network:subnet", TerraformResourceType::new("azurerm_subnet", "azurerm", true, "Subnet ID"));
    m.insert("azure:storage:account", TerraformResourceType::new("azurerm_storage_account", "azurerm", true, "Storage Account ID"));
    m.insert("azure:web:appservice", TerraformResourceType::new("azurerm_app_service", "azurerm", true, "App Service ID"));
    m.insert("azure:sql:database", TerraformResourceType::new("azurerm_sql_database", "azurerm", true, "Database ID"));
    m.insert("azure:keyvault:vault", TerraformResourceType::new("azurerm_key_vault", "azurerm", true, "Key Vault ID"));

    // GCP
    m.insert("gcp:compute:instance", TerraformResourceType::new("google_compute_instance", "google", true, "Instance self_link"));
    m.insert("gcp:compute:network", TerraformResourceType::new("google_compute_network", "google", true, "Network self_link"));
    m.insert("gcp:compute:subnetwork", TerraformResourceType::new("google_compute_subnetwork", "google", true, "Subnetwork self_link"));
    m.insert("gcp:storage:bucket", TerraformResourceType::new("google_storage_bucket", "google", true, "Bucket name"));
    m.insert("gcp:cloudfunctions:function", TerraformResourceType::new("google_cloudfunctions_function", "google", true, "Function name"));
    m.insert("gcp:run:service", TerraformResourceType::new("google_cloud_run_service", "google", true, "Service name"));

    // GitHub
    m.insert("github:organization", TerraformResourceType::new("github_organization_settings", "github", true, "Organization name"));
    m.insert("github:repository", TerraformResourceType::new("github_repository", "github", true, "Repository name"));
    m.insert("github:team", TerraformResourceType::new("github_team", "github", true, "Team ID"));
    m.insert("github:user", TerraformResourceType::new("github_membership", "github", true, "org:username"));

    // GitLab
    m.insert("gitlab:project", TerraformResourceType::new("gitlab_project", "gitlab", true, "Project ID or path"));
    m.insert("gitlab:group", TerraformResourceType::new("gitlab_group", "gitlab", true, "Group ID or path"));
    m.insert("gitlab:user", TerraformResourceType::new("gitlab_user", "gitlab", true, "User ID"));

    // JFrog Artifactory
    m.insert("jfrog:repository", TerraformResourceType::new("artifactory_local_repository", "artifactory", true, "Repository key"));
    m.insert("jfrog:user", TerraformResourceType::new("artifactory_user", "artifactory", true, "Username"));
    m.insert("jfrog:group", TerraformResourceType::new("artifactory_group", "artifactory", true, "Group name"));
    m.insert("jfrog:permission", TerraformResourceType::new("artifactory_permission_target", "artifactory", true, "Permission target name"));

    // Okta
    m.insert("okta:user", TerraformResourceType::new("okta_user", "okta", true, "User ID"));
    m.insert("okta:group", TerraformResourceType::new("okta_group", "okta", true, "Group ID"));
    m.insert("okta:application", TerraformResourceType::new("okta_app_oauth", "okta", true, "App ID"));
    m.insert("okta:authorizationserver", TerraformResourceType::new("okta_auth_server", "okta", true, "Auth Server ID"));

    // Auth0
    m.insert("auth0:user", TerraformResourceType::new("auth0_user", "auth0", true, "User ID"));
    m.insert("auth0:role", TerraformResourceType::new("auth0_role", "auth0", true, "Role ID"));
    m.insert("auth0:client", TerraformResourceType::new("auth0_client", "auth0", true, "Client ID"));
    m.insert("auth0:resourceserver", TerraformResourceType::new("auth0_resource_server", "auth0", true, "Resource Server ID"));
    m.insert("auth0:connection", TerraformResourceType::new("auth0_connection", "auth0", true, "Connection ID"));

    // Jira
    m.insert("jira:project", TerraformResourceType::new("jira_project", "jira", true, "Project key"));

    // Opsgenie
    m.insert("opsgenie:alert", TerraformResourceType::new("opsgenie_alert_policy", "opsgenie", true, "Policy ID"));

    m
}

lazy_static! {
    static ref RESOURCE_TYPE_MAP: HashMap<&'static str, TerraformResourceType> = build_resource_type_map();
}

/// Get all supported cloud inspector resource types
#[allow(dead_code)]
pub fn supported_types() -> Vec<&'static str> {
    RESOURCE_TYPE_MAP.keys().copied().collect()
}

/// Check if a cloud inspector type is supported
#[allow(dead_code)]
pub fn is_supported(cloud_inspector_type: &str) -> bool {
    RESOURCE_TYPE_MAP.contains_key(cloud_inspector_type)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_aws_resource_types() {
        let vpc = map_resource_type("aws:ec2:vpc").unwrap();
        assert_eq!(vpc.tf_type, "aws_vpc");
        assert_eq!(vpc.provider, "aws");
        assert!(vpc.importable);

        let instance = map_resource_type("aws:ec2:instance").unwrap();
        assert_eq!(instance.tf_type, "aws_instance");

        let bucket = map_resource_type("aws:s3:bucket").unwrap();
        assert_eq!(bucket.tf_type, "aws_s3_bucket");
    }

    #[test]
    fn test_map_azure_resource_types() {
        let rg = map_resource_type("azure:resourcegroup").unwrap();
        assert_eq!(rg.tf_type, "azurerm_resource_group");
        assert_eq!(rg.provider, "azurerm");

        let vnet = map_resource_type("azure:network:vnet").unwrap();
        assert_eq!(vnet.tf_type, "azurerm_virtual_network");
    }

    #[test]
    fn test_map_gcp_resource_types() {
        let instance = map_resource_type("gcp:compute:instance").unwrap();
        assert_eq!(instance.tf_type, "google_compute_instance");
        assert_eq!(instance.provider, "google");
    }

    #[test]
    fn test_map_github_resource_types() {
        let repo = map_resource_type("github:repository").unwrap();
        assert_eq!(repo.tf_type, "github_repository");
        assert_eq!(repo.provider, "github");
    }

    #[test]
    fn test_extract_provider() {
        assert_eq!(extract_provider("aws:ec2:instance"), Some(Provider::Aws));
        assert_eq!(extract_provider("azure:compute:vm"), Some(Provider::Azure));
        assert_eq!(extract_provider("gcp:compute:instance"), Some(Provider::Gcp));
        assert_eq!(extract_provider("github:repository"), Some(Provider::Github));
        assert_eq!(extract_provider("unknown:resource"), None);
    }

    #[test]
    fn test_unsupported_type() {
        assert!(map_resource_type("unsupported:type:here").is_none());
    }

    #[test]
    fn test_get_provider_info() {
        let aws = get_provider_info(&Provider::Aws).unwrap();
        assert_eq!(aws.source, "hashicorp/aws");
        assert_eq!(aws.tf_name, "aws");

        let azure = get_provider_info(&Provider::Azure).unwrap();
        assert_eq!(azure.source, "hashicorp/azurerm");
        assert_eq!(azure.tf_name, "azurerm");

        let github = get_provider_info(&Provider::Github).unwrap();
        assert_eq!(github.source, "integrations/github");

        // Jira has no stable provider
        assert!(get_provider_info(&Provider::Jira).is_none());
    }
}
