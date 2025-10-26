#[allow(clippy::module_inception)]
pub mod executor;
pub mod opentofu;
pub mod backend;

pub use executor::{Executor, ExecutorConfig};
pub use opentofu::OpenTofuExecutor;
pub use backend::generate_backend_config;
