use crate::infrastructure::discovery::{InfrastructureDiscovery, Provider};

/// GCP infrastructure discovery using Google Cloud SDK
///
/// Discovers resources from GCP projects using the Google Cloud SDK.
/// Requires valid GCP credentials configured via:
/// - Environment variable (GOOGLE_APPLICATION_CREDENTIALS)
/// - gcloud CLI authentication (gcloud auth application-default login)
/// - Service account key file
/// - Workload Identity (for GKE)
#[allow(dead_code)]
pub struct GcpDiscovery {
    project_id: String,
}

#[allow(dead_code)]
impl GcpDiscovery {
    /// Create a new GCP discovery instance for a project
    pub fn new(project_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
        }
    }

    /// Get the configured project ID
    pub fn project_id(&self) -> &str {
        &self.project_id
    }
}

impl InfrastructureDiscovery for GcpDiscovery {
    fn provider(&self) -> Provider {
        Provider::Gcp
    }

    fn supported_resource_types(&self) -> Vec<&'static str> {
        vec![
            // Compute
            "google_compute_network",
            "google_compute_subnetwork",
            "google_compute_firewall",
            "google_compute_instance",
            "google_compute_disk",
            "google_compute_address",
            // Load Balancing
            "google_compute_forwarding_rule",
            "google_compute_target_pool",
            "google_compute_health_check",
            // Storage
            "google_storage_bucket",
            // SQL
            "google_sql_database_instance",
            "google_sql_database",
            // GKE
            "google_container_cluster",
            "google_container_node_pool",
            // IAM
            "google_service_account",
            // Cloud Functions
            "google_cloudfunctions_function",
            // Pub/Sub
            "google_pubsub_topic",
            "google_pubsub_subscription",
            // Cloud Run
            "google_cloud_run_service",
        ]
    }
}

#[allow(dead_code)]
impl GcpDiscovery {
    /// List available GCP regions
    pub fn list_regions(&self) -> Vec<String> {
        vec![
            "us-central1".to_string(),
            "us-east1".to_string(),
            "us-east4".to_string(),
            "us-west1".to_string(),
            "us-west2".to_string(),
            "us-west3".to_string(),
            "us-west4".to_string(),
            "europe-west1".to_string(),
            "europe-west2".to_string(),
            "europe-west3".to_string(),
            "europe-west4".to_string(),
            "europe-west6".to_string(),
            "europe-north1".to_string(),
            "asia-east1".to_string(),
            "asia-east2".to_string(),
            "asia-northeast1".to_string(),
            "asia-northeast2".to_string(),
            "asia-northeast3".to_string(),
            "asia-south1".to_string(),
            "asia-southeast1".to_string(),
            "asia-southeast2".to_string(),
            "australia-southeast1".to_string(),
            "southamerica-east1".to_string(),
            "northamerica-northeast1".to_string(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcp_discovery_provider() {
        let discovery = GcpDiscovery::new("my-project-123");
        assert_eq!(discovery.provider(), Provider::Gcp);
        assert_eq!(discovery.project_id(), "my-project-123");
    }

    #[test]
    fn test_gcp_supported_types() {
        let discovery = GcpDiscovery::new("my-project");
        let types = discovery.supported_resource_types();

        assert!(types.contains(&"google_compute_network"));
        assert!(types.contains(&"google_compute_instance"));
        assert!(types.contains(&"google_storage_bucket"));
    }
}
