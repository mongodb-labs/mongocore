use thiserror::Error;

/// Core error type for MongoCore operations.
#[derive(Debug, Error)]
pub enum MongoCoreError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Connection error: {0}")]
    ConnectionError(String),

    #[error("Operation error: {0}")]
    OperationError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Timeout error: {0}")]
    TimeoutError(String),
}

impl From<mongodb::error::Error> for MongoCoreError {
    fn from(err: mongodb::error::Error) -> Self {
        MongoCoreError::ConnectionError(err.to_string())
    }
}

impl From<toml::de::Error> for MongoCoreError {
    fn from(err: toml::de::Error) -> Self {
        MongoCoreError::ConfigError(err.to_string())
    }
}

impl From<std::io::Error> for MongoCoreError {
    fn from(err: std::io::Error) -> Self {
        MongoCoreError::ConfigError(format!("IO error: {err}"))
    }
}
