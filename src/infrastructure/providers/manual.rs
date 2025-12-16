use crate::infrastructure::discovery::{
    DependencyType, DiscoveredResource, InfrastructureDiscovery, Provider,
    ResourceDependency,
};
use crate::infrastructure::error::{ImportError, ImportResult};

/// Manual discovery provider for entering resources without API access
///
/// This provider allows users to manually specify resource type and ID
/// without connecting to cloud provider APIs. Useful for:
/// - Resources in accounts without API access configured
/// - Quick imports when you know the exact resource details
/// - Testing and development
pub struct ManualDiscovery {
    provider: Provider,
}

#[allow(dead_code)]
impl ManualDiscovery {
    /// Create a new manual discovery instance for a provider
    pub fn new(provider: Provider) -> Self {
        Self { provider }
    }

    /// Create a discovered resource from manual input
    pub fn create_resource(
        &self,
        resource_type: &str,
        resource_id: &str,
        name: Option<String>,
    ) -> DiscoveredResource {
        let mut resource = DiscoveredResource::new(
            self.provider,
            resource_type.to_string(),
            resource_id.to_string(),
        );

        if let Some(n) = name {
            resource = resource.with_name(n);
        }

        resource
    }
}

impl InfrastructureDiscovery for ManualDiscovery {
    fn provider(&self) -> Provider {
        self.provider
    }

    fn supported_resource_types(&self) -> Vec<&'static str> {
        match self.provider {
            Provider::Aws => AWS_RESOURCE_TYPES.to_vec(),
            Provider::Azure => AZURE_RESOURCE_TYPES.to_vec(),
            Provider::Gcp => GCP_RESOURCE_TYPES.to_vec(),
            Provider::Github => GITHUB_RESOURCE_TYPES.to_vec(),
            Provider::Gitlab => GITLAB_RESOURCE_TYPES.to_vec(),
            Provider::Jfrog => JFROG_RESOURCE_TYPES.to_vec(),
            Provider::Okta => OKTA_RESOURCE_TYPES.to_vec(),
            Provider::Auth0 => AUTH0_RESOURCE_TYPES.to_vec(),
            Provider::Jira => JIRA_RESOURCE_TYPES.to_vec(),
            Provider::Opsgenie => OPSGENIE_RESOURCE_TYPES.to_vec(),
        }
    }
}

#[allow(dead_code)]
impl ManualDiscovery {
    /// Get details for a specific resource
    pub fn get_resource_details(
        &self,
        resource_type: &str,
        resource_id: &str,
    ) -> ImportResult<Option<DiscoveredResource>> {
        let supported = self.supported_resource_types();

        if !supported.contains(&resource_type) {
            return Err(ImportError::UnsupportedResourceType {
                provider: self.provider.as_str().to_string(),
                resource_type: resource_type.to_string(),
            });
        }

        Ok(Some(self.create_resource(resource_type, resource_id, None)))
    }

    /// Detect dependencies for a resource
    pub fn detect_dependencies(
        &self,
        resource: &DiscoveredResource,
    ) -> ImportResult<Vec<ResourceDependency>> {
        Ok(infer_dependencies_from_type(&resource.resource_type))
    }

    /// Always returns true for manual discovery
    pub fn validate_connection(&self) -> ImportResult<bool> {
        Ok(true)
    }

    /// List available regions for this provider
    pub fn list_regions(&self) -> ImportResult<Vec<String>> {
        Ok(match self.provider {
            Provider::Aws => vec![
                "us-east-1".to_string(),
                "us-east-2".to_string(),
                "us-west-1".to_string(),
                "us-west-2".to_string(),
                "eu-west-1".to_string(),
                "eu-west-2".to_string(),
                "eu-central-1".to_string(),
                "ap-northeast-1".to_string(),
                "ap-southeast-1".to_string(),
                "ap-southeast-2".to_string(),
            ],
            Provider::Azure => vec![
                "eastus".to_string(),
                "eastus2".to_string(),
                "westus".to_string(),
                "westus2".to_string(),
                "northeurope".to_string(),
                "westeurope".to_string(),
                "southeastasia".to_string(),
            ],
            Provider::Gcp => vec![
                "us-central1".to_string(),
                "us-east1".to_string(),
                "us-west1".to_string(),
                "europe-west1".to_string(),
                "asia-east1".to_string(),
            ],
            // Non-infrastructure providers don't have regions
            Provider::Github
            | Provider::Gitlab
            | Provider::Jfrog
            | Provider::Okta
            | Provider::Auth0
            | Provider::Jira
            | Provider::Opsgenie => vec!["global".to_string()],
        })
    }
}

/// Supported AWS resource types for import
const AWS_RESOURCE_TYPES: &[&str] = &[
    // VPC and Networking
    "aws_vpc",
    "aws_subnet",
    "aws_security_group",
    "aws_security_group_rule",
    "aws_route_table",
    "aws_route",
    "aws_internet_gateway",
    "aws_nat_gateway",
    "aws_eip",
    "aws_network_acl",
    "aws_vpc_peering_connection",
    // EC2
    "aws_instance",
    "aws_ami",
    "aws_key_pair",
    "aws_launch_template",
    "aws_placement_group",
    // Load Balancing
    "aws_lb",
    "aws_lb_listener",
    "aws_lb_target_group",
    "aws_lb_target_group_attachment",
    // Auto Scaling
    "aws_autoscaling_group",
    "aws_autoscaling_policy",
    // S3
    "aws_s3_bucket",
    "aws_s3_bucket_policy",
    "aws_s3_bucket_versioning",
    "aws_s3_bucket_lifecycle_configuration",
    // RDS
    "aws_db_instance",
    "aws_db_subnet_group",
    "aws_db_parameter_group",
    "aws_rds_cluster",
    "aws_rds_cluster_instance",
    // IAM
    "aws_iam_role",
    "aws_iam_policy",
    "aws_iam_role_policy_attachment",
    "aws_iam_user",
    "aws_iam_group",
    "aws_iam_instance_profile",
    // Lambda
    "aws_lambda_function",
    "aws_lambda_layer_version",
    "aws_lambda_permission",
    // ECS
    "aws_ecs_cluster",
    "aws_ecs_service",
    "aws_ecs_task_definition",
    // EKS
    "aws_eks_cluster",
    "aws_eks_node_group",
    "aws_eks_addon",
    // DynamoDB
    "aws_dynamodb_table",
    // SQS/SNS
    "aws_sqs_queue",
    "aws_sns_topic",
    // CloudWatch
    "aws_cloudwatch_log_group",
    "aws_cloudwatch_metric_alarm",
    // KMS
    "aws_kms_key",
    "aws_kms_alias",
    // Route53
    "aws_route53_zone",
    "aws_route53_record",
    // ACM
    "aws_acm_certificate",
    // Secrets Manager
    "aws_secretsmanager_secret",
];

/// Supported Azure resource types for import
const AZURE_RESOURCE_TYPES: &[&str] = &[
    // Resource Group
    "azurerm_resource_group",
    // Networking
    "azurerm_virtual_network",
    "azurerm_subnet",
    "azurerm_network_security_group",
    "azurerm_network_security_rule",
    "azurerm_public_ip",
    "azurerm_network_interface",
    "azurerm_route_table",
    "azurerm_route",
    "azurerm_nat_gateway",
    "azurerm_application_gateway",
    "azurerm_lb",
    // Compute
    "azurerm_linux_virtual_machine",
    "azurerm_windows_virtual_machine",
    "azurerm_virtual_machine_scale_set",
    "azurerm_availability_set",
    // Storage
    "azurerm_storage_account",
    "azurerm_storage_container",
    "azurerm_storage_blob",
    // Database
    "azurerm_sql_server",
    "azurerm_sql_database",
    "azurerm_postgresql_server",
    "azurerm_mysql_server",
    "azurerm_cosmosdb_account",
    // AKS
    "azurerm_kubernetes_cluster",
    "azurerm_kubernetes_cluster_node_pool",
    // App Service
    "azurerm_app_service_plan",
    "azurerm_app_service",
    "azurerm_function_app",
    // Key Vault
    "azurerm_key_vault",
    "azurerm_key_vault_secret",
    // Identity
    "azurerm_user_assigned_identity",
    // Container Registry
    "azurerm_container_registry",
    // DNS
    "azurerm_dns_zone",
    "azurerm_dns_a_record",
    // Log Analytics
    "azurerm_log_analytics_workspace",
];

/// Supported GCP resource types for import
const GCP_RESOURCE_TYPES: &[&str] = &[
    // Compute
    "google_compute_network",
    "google_compute_subnetwork",
    "google_compute_firewall",
    "google_compute_instance",
    "google_compute_instance_template",
    "google_compute_instance_group",
    "google_compute_instance_group_manager",
    "google_compute_disk",
    "google_compute_address",
    "google_compute_global_address",
    // Load Balancing
    "google_compute_forwarding_rule",
    "google_compute_target_pool",
    "google_compute_health_check",
    "google_compute_backend_service",
    "google_compute_url_map",
    // Storage
    "google_storage_bucket",
    "google_storage_bucket_iam_binding",
    // SQL
    "google_sql_database_instance",
    "google_sql_database",
    "google_sql_user",
    // GKE
    "google_container_cluster",
    "google_container_node_pool",
    // IAM
    "google_service_account",
    "google_project_iam_binding",
    "google_project_iam_member",
    // Cloud Functions
    "google_cloudfunctions_function",
    // Pub/Sub
    "google_pubsub_topic",
    "google_pubsub_subscription",
    // Cloud Run
    "google_cloud_run_service",
    // BigQuery
    "google_bigquery_dataset",
    "google_bigquery_table",
    // Secret Manager
    "google_secret_manager_secret",
    // DNS
    "google_dns_managed_zone",
    "google_dns_record_set",
    // VPC
    "google_compute_router",
    "google_compute_router_nat",
];

/// Supported GitHub resource types for import
const GITHUB_RESOURCE_TYPES: &[&str] = &[
    "github_repository",
    "github_team",
    "github_team_membership",
    "github_membership",
    "github_organization_settings",
    "github_branch_protection",
    "github_actions_secret",
];

/// Supported GitLab resource types for import
const GITLAB_RESOURCE_TYPES: &[&str] = &[
    "gitlab_project",
    "gitlab_group",
    "gitlab_user",
    "gitlab_project_variable",
    "gitlab_group_variable",
    "gitlab_branch_protection",
];

/// Supported JFrog Artifactory resource types for import
const JFROG_RESOURCE_TYPES: &[&str] = &[
    "artifactory_local_repository",
    "artifactory_remote_repository",
    "artifactory_virtual_repository",
    "artifactory_user",
    "artifactory_group",
    "artifactory_permission_target",
];

/// Supported Okta resource types for import
const OKTA_RESOURCE_TYPES: &[&str] = &[
    "okta_user",
    "okta_group",
    "okta_app_oauth",
    "okta_app_saml",
    "okta_auth_server",
    "okta_policy_rule_signon",
];

/// Supported Auth0 resource types for import
const AUTH0_RESOURCE_TYPES: &[&str] = &[
    "auth0_client",
    "auth0_connection",
    "auth0_user",
    "auth0_role",
    "auth0_resource_server",
    "auth0_tenant",
];

/// Supported Jira resource types for import
const JIRA_RESOURCE_TYPES: &[&str] = &[
    "jira_project",
    "jira_issue_type",
    "jira_group",
    "jira_user",
];

/// Supported Opsgenie resource types for import
const OPSGENIE_RESOURCE_TYPES: &[&str] = &[
    "opsgenie_team",
    "opsgenie_user",
    "opsgenie_escalation",
    "opsgenie_schedule",
    "opsgenie_alert_policy",
];

/// Infer common dependencies based on resource type
fn infer_dependencies_from_type(resource_type: &str) -> Vec<ResourceDependency> {
    match resource_type {
        // AWS dependencies
        "aws_subnet" => vec![ResourceDependency {
            resource_type: "aws_vpc".to_string(),
            resource_id: "".to_string(),
            relationship: DependencyType::Parent,
            description: Some("Subnet belongs to a VPC".to_string()),
        }],
        "aws_security_group" => vec![ResourceDependency {
            resource_type: "aws_vpc".to_string(),
            resource_id: "".to_string(),
            relationship: DependencyType::Parent,
            description: Some("Security group belongs to a VPC".to_string()),
        }],
        "aws_instance" => vec![
            ResourceDependency {
                resource_type: "aws_subnet".to_string(),
                resource_id: "".to_string(),
                relationship: DependencyType::Parent,
                description: Some("Instance runs in a subnet".to_string()),
            },
            ResourceDependency {
                resource_type: "aws_security_group".to_string(),
                resource_id: "".to_string(),
                relationship: DependencyType::Reference,
                description: Some("Instance uses security groups".to_string()),
            },
        ],
        "aws_nat_gateway" => vec![
            ResourceDependency {
                resource_type: "aws_subnet".to_string(),
                resource_id: "".to_string(),
                relationship: DependencyType::Parent,
                description: Some("NAT gateway in a subnet".to_string()),
            },
            ResourceDependency {
                resource_type: "aws_eip".to_string(),
                resource_id: "".to_string(),
                relationship: DependencyType::Reference,
                description: Some("NAT gateway uses elastic IP".to_string()),
            },
        ],
        "aws_db_instance" => vec![
            ResourceDependency {
                resource_type: "aws_db_subnet_group".to_string(),
                resource_id: "".to_string(),
                relationship: DependencyType::Reference,
                description: Some("RDS uses subnet group".to_string()),
            },
            ResourceDependency {
                resource_type: "aws_security_group".to_string(),
                resource_id: "".to_string(),
                relationship: DependencyType::Reference,
                description: Some("RDS uses security groups".to_string()),
            },
        ],

        // Azure dependencies
        "azurerm_subnet" => vec![ResourceDependency {
            resource_type: "azurerm_virtual_network".to_string(),
            resource_id: "".to_string(),
            relationship: DependencyType::Parent,
            description: Some("Subnet belongs to a virtual network".to_string()),
        }],
        "azurerm_network_interface" => vec![ResourceDependency {
            resource_type: "azurerm_subnet".to_string(),
            resource_id: "".to_string(),
            relationship: DependencyType::Reference,
            description: Some("NIC attached to subnet".to_string()),
        }],
        "azurerm_linux_virtual_machine" | "azurerm_windows_virtual_machine" => {
            vec![ResourceDependency {
                resource_type: "azurerm_network_interface".to_string(),
                resource_id: "".to_string(),
                relationship: DependencyType::Reference,
                description: Some("VM uses network interface".to_string()),
            }]
        }

        // GCP dependencies
        "google_compute_subnetwork" => vec![ResourceDependency {
            resource_type: "google_compute_network".to_string(),
            resource_id: "".to_string(),
            relationship: DependencyType::Parent,
            description: Some("Subnetwork belongs to a network".to_string()),
        }],
        "google_compute_instance" => vec![ResourceDependency {
            resource_type: "google_compute_subnetwork".to_string(),
            resource_id: "".to_string(),
            relationship: DependencyType::Reference,
            description: Some("Instance uses subnetwork".to_string()),
        }],

        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manual_discovery_create_resource() {
        let discovery = ManualDiscovery::new(Provider::Aws);
        let resource =
            discovery.create_resource("aws_vpc", "vpc-12345", Some("main".to_string()));

        assert_eq!(resource.provider, Provider::Aws);
        assert_eq!(resource.resource_type, "aws_vpc");
        assert_eq!(resource.resource_id, "vpc-12345");
        assert_eq!(resource.name, Some("main".to_string()));
    }

    #[test]
    fn test_manual_discovery_get_details() {
        let discovery = ManualDiscovery::new(Provider::Aws);
        let result = discovery.get_resource_details("aws_vpc", "vpc-12345");

        assert!(result.is_ok());
        let resource = result.unwrap().unwrap();
        assert_eq!(resource.resource_type, "aws_vpc");
        assert_eq!(resource.resource_id, "vpc-12345");
    }

    #[test]
    fn test_manual_discovery_unsupported_type() {
        let discovery = ManualDiscovery::new(Provider::Aws);
        let result = discovery.get_resource_details("invalid_type", "123");

        assert!(matches!(
            result,
            Err(ImportError::UnsupportedResourceType { .. })
        ));
    }

    #[test]
    fn test_manual_discovery_validates() {
        let discovery = ManualDiscovery::new(Provider::Aws);
        let result = discovery.validate_connection();

        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[test]
    fn test_infer_dependencies() {
        let discovery = ManualDiscovery::new(Provider::Aws);
        let resource = DiscoveredResource::new(
            Provider::Aws,
            "aws_subnet".to_string(),
            "subnet-12345".to_string(),
        );

        let deps = discovery.detect_dependencies(&resource).unwrap();

        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].resource_type, "aws_vpc");
        assert_eq!(deps[0].relationship, DependencyType::Parent);
    }
}
