#[allow(clippy::module_inception)]
pub mod executor;
pub mod none;
pub mod opentofu;
pub mod registry;

pub use executor::{Executor, ExecutorConfig, ProjectMetadata};
pub use none::NoneExecutor;
pub use opentofu::OpenTofuExecutor;

// Registry types available for future use (Phase 3: dependency injection)
#[allow(unused_imports)]
pub use registry::{DefaultExecutorRegistry, ExecutorRegistry};
