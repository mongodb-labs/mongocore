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

/// Default MongoDB connection URI.
pub const DEFAULT_CONNECTION_URI: &str = "mongodb://localhost:27017";

/// Default OpenTelemetry OTLP endpoint.
pub const DEFAULT_OTEL_ENDPOINT: &str = "http://localhost:4317";

/// Default OpenTelemetry service name.
pub const DEFAULT_OTEL_SERVICE_NAME: &str = "mongocore";

/// Default max gRPC message size (64 MB).
pub const DEFAULT_GRPC_MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;

/// Default gRPC compression algorithm.
pub const DEFAULT_GRPC_COMPRESSION: &str = "none";

/// Default transport mode.
pub const DEFAULT_TRANSPORT: &str = "both";

/// Default Unix domain socket path.
pub const DEFAULT_SOCKET_PATH: &str = "/tmp/mongocore.sock";

pub const DEFAULT_SOCKET_PERMISSIONS: u32 = 0o600;

/// Default streaming batch size (documents per frame).
pub const DEFAULT_STREAM_BATCH_SIZE: u32 = 1000;

/// Default stream idle timeout in seconds.
pub const DEFAULT_STREAM_IDLE_TIMEOUT_SECS: u64 = 60;

/// Default maximum operations in a pipeline.
pub const DEFAULT_PIPELINE_MAX_OPS: usize = 100;

/// Default pipeline timeout in seconds.
pub const DEFAULT_PIPELINE_TIMEOUT_SECS: u64 = 30;

/// Default maximum concurrent operations within a pipeline.
pub const DEFAULT_PIPELINE_MAX_CONCURRENCY: usize = 20;

/// Minimum allowed batch size.
pub const MIN_STREAM_BATCH_SIZE: u32 = 1;

/// Maximum allowed batch size.
pub const MAX_STREAM_BATCH_SIZE: u32 = 10000;

/// Returns the default write concern (majority).
pub fn default_write_concern() -> mongodb::options::WriteConcern {
    mongodb::options::WriteConcern::majority()
}

/// Returns the default read concern (majority).
pub fn default_read_concern() -> mongodb::options::ReadConcern {
    mongodb::options::ReadConcern::majority()
}

/// Maximum steps allowed in a transactional pipeline.
pub const DEFAULT_TRANSACTION_PIPELINE_MAX_STEPS: usize = 50;

/// Maximum documents stored from a Find/Aggregate step for referencing.
pub const DEFAULT_TRANSACTION_PIPELINE_MAX_DOCS: usize = 101;

/// Default transaction pipeline timeout in milliseconds.
pub const DEFAULT_TRANSACTION_PIPELINE_TIMEOUT_MS: u64 = 30_000;

/// Maximum retries on transient transaction errors.
pub const DEFAULT_TRANSACTION_PIPELINE_MAX_RETRIES: u32 = 3;

/// Returns the default read preference (PrimaryPreferred).
pub fn default_read_preference() -> mongodb::options::ReadPreference {
    mongodb::options::ReadPreference::PrimaryPreferred {
        options: Default::default(),
    }
}
