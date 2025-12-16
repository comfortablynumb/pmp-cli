pub mod aws;
pub mod azure;
pub mod gcp;
pub mod manual;

#[allow(unused_imports)]
pub use aws::AwsDiscovery;
#[allow(unused_imports)]
pub use azure::AzureDiscovery;
#[allow(unused_imports)]
pub use gcp::GcpDiscovery;
pub use manual::ManualDiscovery;
