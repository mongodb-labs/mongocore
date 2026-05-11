use std::time::Duration;

/// Default gRPC port for MongoCore.
pub const DEFAULT_GRPC_PORT: u16 = 50051;

/// Default MCP port for MongoCore.
pub const DEFAULT_MCP_PORT: u16 = 3000;

/// Default log level.
pub const DEFAULT_LOG_LEVEL: &str = "info";

/// Default query timeout (30 seconds).
pub const DEFAULT_QUERY_TIMEOUT: Duration = Duration::from_secs(30);

/// Default aggregation timeout (60 seconds).
pub const DEFAULT_AGGREGATION_TIMEOUT: Duration = Duration::from_secs(60);

/// Whether retryable writes are enabled by default.
pub const DEFAULT_RETRYABLE_WRITES: bool = true;

/// Whether retryable reads are enabled by default.
pub const DEFAULT_RETRYABLE_READS: bool = true;

/// Default compiled cache sync setting.
pub const DEFAULT_COMPILED_CACHE_SYNC: bool = true;

/// Returns the default write concern (majority).
pub fn default_write_concern() -> mongodb::options::WriteConcern {
    mongodb::options::WriteConcern::majority()
}

/// Returns the default read concern (majority).
pub fn default_read_concern() -> mongodb::options::ReadConcern {
    mongodb::options::ReadConcern::majority()
}

/// Returns the default read preference (PrimaryPreferred).
pub fn default_read_preference() -> mongodb::options::ReadPreference {
    mongodb::options::ReadPreference::PrimaryPreferred {
        options: Default::default(),
    }
}
