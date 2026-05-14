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

    #[error("Ingestion error: {0}")]
    IngestionError(String),

    #[error("Transaction pipeline error: {0}")]
    TransactionPipelineError(String),
}

impl From<mongodb::error::Error> for MongoCoreError {
    fn from(err: mongodb::error::Error) -> Self {
        use mongodb::error::ErrorKind;
        let msg = err.to_string();
        match *err.kind {
            ErrorKind::Authentication { .. } => MongoCoreError::ConnectionError(msg),
            ErrorKind::ServerSelection { .. } => MongoCoreError::ConnectionError(msg),
            ErrorKind::DnsResolve { .. } => MongoCoreError::ConnectionError(msg),
            ErrorKind::Write(_) => MongoCoreError::OperationError(msg),
            ErrorKind::Command(_) => MongoCoreError::OperationError(msg),
            ErrorKind::BulkWrite(_) => MongoCoreError::OperationError(msg),
            ErrorKind::Io(_) => MongoCoreError::TimeoutError(msg),
            _ => MongoCoreError::OperationError(msg),
        }
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
