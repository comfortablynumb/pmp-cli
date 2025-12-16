use std::fmt;

/// Error types for infrastructure import operations
#[derive(Debug)]
#[allow(dead_code)]
pub enum ImportError {
    /// Invalid or missing credentials for provider
    Authentication(String),

    /// Resource not found in provider
    ResourceNotFound {
        resource_type: String,
        resource_id: String,
    },

    /// Failed to resolve dependencies
    DependencyResolution(String),

    /// OpenTofu command failed
    ExecutorFailed {
        command: String,
        message: String,
        exit_code: Option<i32>,
    },

    /// Some resources failed during batch import
    PartialImport {
        succeeded: Vec<String>,
        failed: Vec<(String, String)>,
    },

    /// Configuration file parsing error
    ConfigParse(String),

    /// Invalid resource type for provider
    UnsupportedResourceType {
        provider: String,
        resource_type: String,
    },

    /// Provider API error
    ProviderApi(String),

    /// File system operation failed
    FileSystem(String),

    /// Invalid input or parameter
    InvalidInput(String),

    /// Project not found
    ProjectNotFound(String),

    /// General I/O error
    Io(std::io::Error),

    /// Serialization error
    Serialization(String),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ImportError::Authentication(msg) => {
                write!(f, "Authentication failed: {}", msg)
            }
            ImportError::ResourceNotFound {
                resource_type,
                resource_id,
            } => {
                write!(
                    f,
                    "Resource not found: {} with ID '{}'",
                    resource_type, resource_id
                )
            }
            ImportError::DependencyResolution(msg) => {
                write!(f, "Failed to resolve dependencies: {}", msg)
            }
            ImportError::ExecutorFailed {
                command,
                message,
                exit_code,
            } => {
                write!(f, "Executor command '{}' failed", command)?;

                if let Some(code) = exit_code {
                    write!(f, " (exit code {})", code)?;
                }

                write!(f, ": {}", message)
            }
            ImportError::PartialImport { succeeded, failed } => {
                write!(
                    f,
                    "Partial import: {} succeeded, {} failed",
                    succeeded.len(),
                    failed.len()
                )
            }
            ImportError::ConfigParse(msg) => {
                write!(f, "Failed to parse configuration: {}", msg)
            }
            ImportError::UnsupportedResourceType {
                provider,
                resource_type,
            } => {
                write!(
                    f,
                    "Unsupported resource type '{}' for provider '{}'",
                    resource_type, provider
                )
            }
            ImportError::ProviderApi(msg) => {
                write!(f, "Provider API error: {}", msg)
            }
            ImportError::FileSystem(msg) => {
                write!(f, "File system error: {}", msg)
            }
            ImportError::InvalidInput(msg) => {
                write!(f, "Invalid input: {}", msg)
            }
            ImportError::ProjectNotFound(name) => {
                write!(f, "Project not found: {}", name)
            }
            ImportError::Io(err) => {
                write!(f, "I/O error: {}", err)
            }
            ImportError::Serialization(msg) => {
                write!(f, "Serialization error: {}", msg)
            }
        }
    }
}

impl std::error::Error for ImportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ImportError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ImportError {
    fn from(err: std::io::Error) -> Self {
        ImportError::Io(err)
    }
}

impl From<serde_yaml::Error> for ImportError {
    fn from(err: serde_yaml::Error) -> Self {
        ImportError::ConfigParse(err.to_string())
    }
}

impl From<serde_json::Error> for ImportError {
    fn from(err: serde_json::Error) -> Self {
        ImportError::Serialization(err.to_string())
    }
}

/// Result type for import operations
pub type ImportResult<T> = Result<T, ImportError>;
