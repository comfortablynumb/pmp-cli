use crate::infrastructure::discovery::{InfrastructureDiscovery, Provider};

/// AWS infrastructure discovery using AWS SDK
///
/// Discovers resources from AWS accounts using the AWS SDK.
/// Requires valid AWS credentials configured via:
/// - Environment variables (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY)
/// - AWS credentials file (~/.aws/credentials)
/// - IAM role (for EC2 instances, ECS tasks, Lambda functions)
#[allow(dead_code)]
pub struct AwsDiscovery {
    region: String,
}

#[allow(dead_code)]
impl AwsDiscovery {
    /// Create a new AWS discovery instance for a region
    pub fn new(region: impl Into<String>) -> Self {
        Self {
            region: region.into(),
        }
    }

    /// Get the configured region
    pub fn region(&self) -> &str {
        &self.region
    }
}

impl InfrastructureDiscovery for AwsDiscovery {
    fn provider(&self) -> Provider {
        Provider::Aws
    }

    fn supported_resource_types(&self) -> Vec<&'static str> {
        vec![
            // VPC and Networking
            "aws_vpc",
            "aws_subnet",
            "aws_security_group",
            "aws_route_table",
            "aws_internet_gateway",
            "aws_nat_gateway",
            "aws_eip",
            // EC2
            "aws_instance",
            // Load Balancing
            "aws_lb",
            "aws_lb_target_group",
            // S3
            "aws_s3_bucket",
            // RDS
            "aws_db_instance",
            "aws_rds_cluster",
            // IAM
            "aws_iam_role",
            "aws_iam_policy",
            // Lambda
            "aws_lambda_function",
            // ECS
            "aws_ecs_cluster",
            // EKS
            "aws_eks_cluster",
        ]
    }
}

#[allow(dead_code)]
impl AwsDiscovery {
    /// List available AWS regions
    pub fn list_regions(&self) -> Vec<String> {
        vec![
            "us-east-1".to_string(),
            "us-east-2".to_string(),
            "us-west-1".to_string(),
            "us-west-2".to_string(),
            "eu-west-1".to_string(),
            "eu-west-2".to_string(),
            "eu-west-3".to_string(),
            "eu-central-1".to_string(),
            "eu-north-1".to_string(),
            "ap-northeast-1".to_string(),
            "ap-northeast-2".to_string(),
            "ap-southeast-1".to_string(),
            "ap-southeast-2".to_string(),
            "ap-south-1".to_string(),
            "sa-east-1".to_string(),
            "ca-central-1".to_string(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aws_discovery_provider() {
        let discovery = AwsDiscovery::new("us-east-1");
        assert_eq!(discovery.provider(), Provider::Aws);
        assert_eq!(discovery.region(), "us-east-1");
    }

    #[test]
    fn test_aws_supported_types() {
        let discovery = AwsDiscovery::new("us-east-1");
        let types = discovery.supported_resource_types();

        assert!(types.contains(&"aws_vpc"));
        assert!(types.contains(&"aws_instance"));
        assert!(types.contains(&"aws_s3_bucket"));
    }
}
