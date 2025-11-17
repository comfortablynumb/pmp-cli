pub mod discovery;
pub mod installer;
pub mod metadata;
pub mod renderer;
pub mod utils;

pub use discovery::{InfrastructureTemplateInfo, PluginInfo, TemplateDiscovery, TemplateInfo, TemplatePackInfo};
pub use installer::check_and_offer_installation;
pub use metadata::{DynamicProjectEnvironmentResource, ProjectReference, ProjectResource};
pub use renderer::TemplateRenderer;
