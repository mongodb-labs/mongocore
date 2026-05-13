use clap::Parser;
use serde::Deserialize;
use std::path::PathBuf;

use crate::defaults::{
    DEFAULT_COMPILED_CACHE_SYNC, DEFAULT_CONNECTION_URI, DEFAULT_GRPC_COMPRESSION,
    DEFAULT_GRPC_MAX_MESSAGE_SIZE, DEFAULT_GRPC_PORT, DEFAULT_LOG_LEVEL, DEFAULT_MCP_PORT,
    DEFAULT_OTEL_ENDPOINT, DEFAULT_OTEL_SERVICE_NAME, DEFAULT_PIPELINE_MAX_CONCURRENCY,
    DEFAULT_PIPELINE_TIMEOUT_SECS, DEFAULT_SOCKET_PATH, DEFAULT_SOCKET_PERMISSIONS,
    DEFAULT_STREAM_BATCH_SIZE, DEFAULT_STREAM_IDLE_TIMEOUT_SECS, DEFAULT_TRANSPORT,
};
use crate::error::MongoCoreError;

/// Command-line arguments for MongoCore.
#[derive(Parser, Debug)]
#[command(name = "mongocore", about = "AI-native MongoDB driver sidecar")]
pub struct CliArgs {
    /// Path to TOML configuration file
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// MongoDB connection URI
    #[arg(long, env = "MONGOCORE_CONNECTION_URI")]
    pub connection_uri: Option<String>,

    /// gRPC server port
    #[arg(long, env = "MONGOCORE_GRPC_PORT")]
    pub grpc_port: Option<u16>,

    /// MCP server port
    #[arg(long, env = "MONGOCORE_MCP_PORT")]
    pub mcp_port: Option<u16>,

    /// Anthropic API key for compiled queries
    #[arg(long, env = "ANTHROPIC_API_KEY")]
    pub anthropic_api_key: Option<String>,

    /// OpenAI API key for compiled queries
    #[arg(long, env = "OPENAI_API_KEY")]
    pub openai_api_key: Option<String>,

    /// Voyage AI API key for embeddings
    #[arg(long, env = "VOYAGE_API_KEY")]
    pub voyage_api_key: Option<String>,

    /// Custom LLM gateway base URL (overrides direct API keys)
    #[arg(long, env = "LLM_BASE_URL")]
    pub llm_base_url: Option<String>,

    /// API key for custom LLM gateway
    #[arg(long, env = "LLM_API_KEY")]
    pub llm_gateway_key: Option<String>,

    /// Auth header name for custom LLM gateway
    #[arg(long, env = "LLM_AUTH_HEADER")]
    pub llm_auth_header: Option<String>,

    /// Model name for custom LLM gateway
    #[arg(long, env = "LLM_MODEL")]
    pub llm_model: Option<String>,

    /// Provider type for custom LLM gateway (anthropic or openai)
    #[arg(long, env = "LLM_PROVIDER_TYPE")]
    pub llm_provider_type: Option<String>,

    /// Enable compiled cache sync
    #[arg(long, env = "MONGOCORE_COMPILED_CACHE_SYNC")]
    pub compiled_cache_sync: Option<bool>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, env = "MONGOCORE_LOG_LEVEL")]
    pub log_level: Option<String>,

    /// Maximum gRPC message size in bytes
    #[arg(long, env = "MONGOCORE_GRPC_MAX_MESSAGE_SIZE")]
    pub grpc_max_message_size: Option<usize>,

    /// Transport mode: both, uds, tcp (default: both)
    #[arg(long, env = "MONGOCORE_TRANSPORT")]
    pub transport: Option<String>,

    /// Unix domain socket path (default: /tmp/mongocore.sock)
    #[arg(long, env = "MONGOCORE_SOCKET_PATH")]
    pub socket_path: Option<String>,

    /// Unix domain socket file permissions (octal, e.g. 0600)
    #[arg(long, env = "MONGOCORE_SOCKET_PERMISSIONS")]
    pub socket_permissions: Option<u32>,

    /// Enable OpenTelemetry tracing export
    #[arg(long, env = "MONGOCORE_OTEL_ENABLED")]
    pub otel_enabled: Option<bool>,

    /// OpenTelemetry OTLP endpoint (gRPC)
    #[arg(long, env = "MONGOCORE_OTEL_ENDPOINT")]
    pub otel_endpoint: Option<String>,

    /// OpenTelemetry service name
    #[arg(long, env = "MONGOCORE_OTEL_SERVICE_NAME")]
    pub otel_service_name: Option<String>,

    /// Default streaming batch size
    #[arg(long, env = "MONGOCORE_STREAM_BATCH_SIZE")]
    pub stream_batch_size: Option<u32>,

    /// Stream idle timeout in seconds
    #[arg(long, env = "MONGOCORE_STREAM_IDLE_TIMEOUT_SECS")]
    pub stream_idle_timeout_secs: Option<u64>,

    /// gRPC compression algorithm (none, gzip, zstd)
    #[arg(long, env = "MONGOCORE_GRPC_COMPRESSION")]
    pub grpc_compression: Option<String>,

    /// Pipeline timeout in seconds
    #[arg(long, env = "MONGOCORE_PIPELINE_TIMEOUT_SECS")]
    pub pipeline_timeout_secs: Option<u64>,

    /// Pipeline maximum concurrent operations
    #[arg(long, env = "MONGOCORE_PIPELINE_MAX_CONCURRENCY")]
    pub pipeline_max_concurrency: Option<usize>,

    /// Run in MCP stdio mode (stdin/stdout JSON-RPC, no gRPC server)
    #[arg(long, env = "MONGOCORE_STDIO")]
    pub stdio: bool,
}

/// Per-tenant configuration structure.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct TenantFileConfig {
    pub tenant_id: Option<String>,
    pub max_connections: Option<usize>,
    pub max_cache_entries: Option<usize>,
    pub rate_limit_ops_per_sec: Option<u64>,
    pub connection_uri: Option<String>,
}

/// Ingestion watch configuration from TOML.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct WatchFileConfig {
    pub enabled: Option<bool>,
    pub path: Option<String>,
    pub file_pattern: Option<String>,
    pub database: Option<String>,
    pub collection: Option<String>,
    pub conflict_strategy: Option<String>,
}

/// Ingestion configuration from TOML.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct IngestionFileConfig {
    pub enabled: Option<bool>,
    pub sample_size: Option<u32>,
    pub default_batch_size: Option<u32>,
    pub default_concurrency: Option<u32>,
    pub max_file_size_mb: Option<u64>,
    pub llm_expressions: Option<bool>,
    pub max_llm_concurrency: Option<u32>,
    pub watch_debounce_ms: Option<u64>,
    pub watch: Option<WatchFileConfig>,
}

/// Resolved ingestion configuration with defaults applied.
#[derive(Debug, Clone)]
pub struct ResolvedIngestionConfig {
    pub enabled: bool,
    pub sample_size: u32,
    pub default_batch_size: u32,
    pub default_concurrency: u32,
    pub max_file_size_mb: u64,
    pub llm_expressions: bool,
    pub max_llm_concurrency: u32,
    pub watch_debounce_ms: u64,
    pub watch: Option<WatchFileConfig>,
}

impl Default for ResolvedIngestionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sample_size: 1000,
            default_batch_size: 1000,
            default_concurrency: 4,
            max_file_size_mb: 10240,
            llm_expressions: false,
            max_llm_concurrency: 4,
            watch_debounce_ms: 2000,
            watch: None,
        }
    }
}

/// Configuration for a custom LLM gateway endpoint.
#[derive(Debug, Clone)]
pub struct LlmGatewayConfig {
    pub base_url: String,
    pub api_key: String,
    pub auth_header: String,
    pub model: String,
    pub provider_type: String,
}

/// TOML file configuration structure.
#[derive(Debug, Deserialize, Default)]
pub struct FileConfig {
    pub connection_uri: Option<String>,
    pub grpc_port: Option<u16>,
    pub mcp_port: Option<u16>,
    #[serde(rename = "ANTHROPIC_API_KEY")]
    pub anthropic_api_key: Option<String>,
    #[serde(rename = "OPENAI_API_KEY")]
    pub openai_api_key: Option<String>,
    #[serde(rename = "VOYAGE_API_KEY")]
    pub voyage_api_key: Option<String>,
    #[serde(rename = "LLM_BASE_URL")]
    pub llm_base_url: Option<String>,
    #[serde(rename = "LLM_API_KEY")]
    pub llm_gateway_key: Option<String>,
    #[serde(rename = "LLM_AUTH_HEADER")]
    pub llm_auth_header: Option<String>,
    #[serde(rename = "LLM_MODEL")]
    pub llm_model: Option<String>,
    #[serde(rename = "LLM_PROVIDER_TYPE")]
    pub llm_provider_type: Option<String>,
    pub compiled_cache_sync: Option<bool>,
    pub log_level: Option<String>,
    pub multi_tenant_enabled: Option<bool>,
    pub tenants: Option<Vec<TenantFileConfig>>,
    pub analytics_enabled: Option<bool>,
    pub analytics_buffer_size: Option<usize>,
    pub analytics_flush_interval_secs: Option<u64>,
    pub ingestion: Option<IngestionFileConfig>,
    pub grpc_max_message_size: Option<usize>,
    pub transport: Option<String>,
    pub socket_path: Option<String>,
    pub socket_permissions: Option<u32>,
    pub otel_enabled: Option<bool>,
    pub otel_endpoint: Option<String>,
    pub otel_service_name: Option<String>,
    pub stream_batch_size: Option<u32>,
    pub stream_idle_timeout_secs: Option<u64>,
    pub grpc_compression: Option<String>,
    pub pipeline_timeout_secs: Option<u64>,
    pub pipeline_max_concurrency: Option<usize>,
}

/// Resolved configuration for MongoCore.
#[derive(Debug, Clone)]
pub struct Config {
    pub connection_uri: String,
    pub grpc_port: u16,
    pub mcp_port: u16,
    pub llm_api_key: Option<String>,
    pub llm_provider_name: Option<String>,
    pub voyage_api_key: Option<String>,
    pub llm_gateway: Option<LlmGatewayConfig>,
    pub compiled_cache_sync: bool,
    pub log_level: String,
    pub multi_tenant_enabled: bool,
    pub tenants: Vec<TenantFileConfig>,
    pub analytics_enabled: bool,
    pub analytics_buffer_size: usize,
    pub analytics_flush_interval_secs: u64,
    pub ingestion: ResolvedIngestionConfig,
    pub grpc_max_message_size: usize,
    pub transport: String,
    pub socket_path: String,
    pub socket_permissions: u32,
    pub otel_enabled: bool,
    pub otel_endpoint: String,
    pub otel_service_name: String,
    pub stream_batch_size: u32,
    pub stream_idle_timeout_secs: u64,
    pub grpc_compression: String,
    pub pipeline_timeout_secs: u64,
    pub pipeline_max_concurrency: usize,
}

impl Config {
    /// Load configuration by merging TOML file, environment variables, and CLI args.
    /// Priority (highest wins): CLI args > env vars > TOML file > defaults.
    /// Note: clap handles env var parsing for CLI args, so env vars and CLI share priority.
    pub fn load(cli: &CliArgs) -> Result<Self, MongoCoreError> {
        let file_config = if let Some(ref path) = cli.config {
            let content = std::fs::read_to_string(path)?;
            toml::from_str::<FileConfig>(&content)?
        } else {
            FileConfig::default()
        };

        let connection_uri = cli
            .connection_uri
            .clone()
            .or(file_config.connection_uri)
            .unwrap_or_else(|| DEFAULT_CONNECTION_URI.to_string());

        let grpc_port = cli
            .grpc_port
            .or(file_config.grpc_port)
            .unwrap_or(DEFAULT_GRPC_PORT);

        let mcp_port = cli
            .mcp_port
            .or(file_config.mcp_port)
            .unwrap_or(DEFAULT_MCP_PORT);

        // Check for custom LLM gateway first (takes precedence over direct keys)
        let llm_gateway = if let Some(base_url) = cli.llm_base_url.clone()
            .or(file_config.llm_base_url)
            .or_else(|| std::env::var("LLM_BASE_URL").ok())
        {
            let api_key = cli.llm_gateway_key.clone()
                .or(file_config.llm_gateway_key)
                .or_else(|| std::env::var("LLM_API_KEY").ok())
                .unwrap_or_default();
            let auth_header = cli.llm_auth_header.clone()
                .or(file_config.llm_auth_header)
                .unwrap_or_else(|| "api-key".to_string());
            let model = cli.llm_model.clone()
                .or(file_config.llm_model)
                .or_else(|| std::env::var("LLM_MODEL").ok())
                .unwrap_or_else(|| "claude-sonnet-4-6".to_string());
            let provider_type = cli.llm_provider_type.clone()
                .or(file_config.llm_provider_type)
                .unwrap_or_else(|| "anthropic".to_string());
            Some(LlmGatewayConfig { base_url, api_key, auth_header, model, provider_type })
        } else {
            None
        };

        // Resolve LLM API key: CLI/env > TOML > env var fallback
        let anthropic_key = cli.anthropic_api_key
            .clone()
            .or(file_config.anthropic_api_key)
            .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok());

        let openai_key = cli.openai_api_key
            .clone()
            .or(file_config.openai_api_key)
            .or_else(|| std::env::var("OPENAI_API_KEY").ok());

        let (llm_api_key, llm_provider_name) = if let Some(key) = anthropic_key {
            (Some(key), Some("anthropic".to_string()))
        } else if let Some(key) = openai_key {
            (Some(key), Some("openai".to_string()))
        } else {
            (None, None)
        };

        // Resolve Voyage API key: CLI/env > TOML > env var fallback
        let voyage_api_key = cli.voyage_api_key
            .clone()
            .or(file_config.voyage_api_key)
            .or_else(|| std::env::var("VOYAGE_API_KEY").ok());

        let compiled_cache_sync = cli
            .compiled_cache_sync
            .or(file_config.compiled_cache_sync)
            .unwrap_or(DEFAULT_COMPILED_CACHE_SYNC);

        let log_level = cli
            .log_level
            .clone()
            .or(file_config.log_level)
            .unwrap_or_else(|| DEFAULT_LOG_LEVEL.to_string());

        let multi_tenant_enabled = file_config.multi_tenant_enabled.unwrap_or(false);
        let tenants = file_config.tenants.unwrap_or_default();

        let analytics_enabled = file_config.analytics_enabled.unwrap_or(true);
        let analytics_buffer_size = file_config.analytics_buffer_size.unwrap_or(10000);
        let analytics_flush_interval_secs = file_config.analytics_flush_interval_secs.unwrap_or(300);

        let ingestion_file = file_config.ingestion.unwrap_or_default();
        let ingestion = ResolvedIngestionConfig {
            enabled: ingestion_file.enabled.unwrap_or(true),
            sample_size: ingestion_file.sample_size.unwrap_or(1000),
            default_batch_size: ingestion_file.default_batch_size.unwrap_or(1000),
            default_concurrency: ingestion_file.default_concurrency.unwrap_or(4),
            max_file_size_mb: ingestion_file.max_file_size_mb.unwrap_or(10240),
            llm_expressions: ingestion_file.llm_expressions.unwrap_or(false),
            max_llm_concurrency: ingestion_file.max_llm_concurrency.unwrap_or(4),
            watch_debounce_ms: ingestion_file.watch_debounce_ms.unwrap_or(2000),
            watch: ingestion_file.watch,
        };

        let grpc_max_message_size = cli
            .grpc_max_message_size
            .or(file_config.grpc_max_message_size)
            .unwrap_or(DEFAULT_GRPC_MAX_MESSAGE_SIZE);

        let transport = cli.transport.clone()
            .or(file_config.transport)
            .unwrap_or_else(|| DEFAULT_TRANSPORT.to_string());

        let socket_path = cli.socket_path.clone()
            .or(file_config.socket_path)
            .unwrap_or_else(|| DEFAULT_SOCKET_PATH.to_string());

        let socket_permissions = cli
            .socket_permissions
            .or(file_config.socket_permissions)
            .unwrap_or(DEFAULT_SOCKET_PERMISSIONS);

        let stream_batch_size = cli
            .stream_batch_size
            .or(file_config.stream_batch_size)
            .unwrap_or(DEFAULT_STREAM_BATCH_SIZE);
        let stream_idle_timeout_secs = cli
            .stream_idle_timeout_secs
            .or(file_config.stream_idle_timeout_secs)
            .unwrap_or(DEFAULT_STREAM_IDLE_TIMEOUT_SECS);

        let grpc_compression = cli
            .grpc_compression
            .clone()
            .or(file_config.grpc_compression)
            .unwrap_or_else(|| DEFAULT_GRPC_COMPRESSION.to_string());

        let pipeline_timeout_secs = cli
            .pipeline_timeout_secs
            .or(file_config.pipeline_timeout_secs)
            .unwrap_or(DEFAULT_PIPELINE_TIMEOUT_SECS);

        let pipeline_max_concurrency = cli
            .pipeline_max_concurrency
            .or(file_config.pipeline_max_concurrency)
            .unwrap_or(DEFAULT_PIPELINE_MAX_CONCURRENCY);

        let otel_enabled = cli
            .otel_enabled
            .or(file_config.otel_enabled)
            .unwrap_or(false);
        let otel_endpoint = cli
            .otel_endpoint
            .clone()
            .or(file_config.otel_endpoint)
            .unwrap_or_else(|| DEFAULT_OTEL_ENDPOINT.to_string());
        let otel_service_name = cli
            .otel_service_name
            .clone()
            .or(file_config.otel_service_name)
            .unwrap_or_else(|| DEFAULT_OTEL_SERVICE_NAME.to_string());

        Ok(Config {
            connection_uri,
            grpc_port,
            mcp_port,
            llm_api_key,
            llm_provider_name,
            voyage_api_key,
            llm_gateway,
            compiled_cache_sync,
            log_level,
            multi_tenant_enabled,
            tenants,
            analytics_enabled,
            analytics_buffer_size,
            analytics_flush_interval_secs,
            ingestion,
            grpc_max_message_size,
            transport,
            socket_path,
            socket_permissions,
            otel_enabled,
            otel_endpoint,
            otel_service_name,
            stream_batch_size,
            stream_idle_timeout_secs,
            grpc_compression,
            pipeline_timeout_secs,
            pipeline_max_concurrency,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_default_values() {
        let cli = CliArgs {
            config: None,
            connection_uri: None,
            grpc_port: None,
            mcp_port: None,
            anthropic_api_key: None,
            openai_api_key: None,
            voyage_api_key: None,
            llm_base_url: None,
            llm_gateway_key: None,
            llm_auth_header: None,
            llm_model: None,
            llm_provider_type: None,
            compiled_cache_sync: None,
            log_level: None,
            grpc_max_message_size: None,
            socket_path: None,
            transport: None,
            socket_permissions: None,
            otel_enabled: None,
            otel_endpoint: None,
            otel_service_name: None,
            stream_batch_size: None,
            stream_idle_timeout_secs: None,
            grpc_compression: None,
            pipeline_timeout_secs: None,
            pipeline_max_concurrency: None,
            stdio: false,
        };

        let config = Config::load(&cli).unwrap();
        assert_eq!(config.connection_uri, "mongodb://localhost:27017");
        assert_eq!(config.grpc_port, 50051);
        assert_eq!(config.mcp_port, 3000);
        assert_eq!(config.log_level, "info");
        assert!(config.compiled_cache_sync);
        assert!(config.llm_api_key.is_none());
        assert!(config.llm_provider_name.is_none());
        assert!(config.voyage_api_key.is_none());
    }

    #[test]
    fn test_toml_parsing() {
        let toml_content = r#"
connection_uri = "mongodb://myhost:27018"
grpc_port = 9090
mcp_port = 4000
ANTHROPIC_API_KEY = "sk-ant-test-key"
VOYAGE_API_KEY = "voyage-test-key"
compiled_cache_sync = false
log_level = "debug"
"#;

        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(toml_content.as_bytes()).unwrap();

        let cli = CliArgs {
            config: Some(tmp.path().to_path_buf()),
            connection_uri: None,
            grpc_port: None,
            mcp_port: None,
            anthropic_api_key: None,
            openai_api_key: None,
            voyage_api_key: None,
            llm_base_url: None,
            llm_gateway_key: None,
            llm_auth_header: None,
            llm_model: None,
            llm_provider_type: None,
            compiled_cache_sync: None,
            log_level: None,
            grpc_max_message_size: None,
            socket_path: None,
            transport: None,
            socket_permissions: None,
            otel_enabled: None,
            otel_endpoint: None,
            otel_service_name: None,
            stream_batch_size: None,
            stream_idle_timeout_secs: None,
            grpc_compression: None,
            pipeline_timeout_secs: None,
            pipeline_max_concurrency: None,
            stdio: false,
        };

        let config = Config::load(&cli).unwrap();
        assert_eq!(config.connection_uri, "mongodb://myhost:27018");
        assert_eq!(config.grpc_port, 9090);
        assert_eq!(config.mcp_port, 4000);
        assert_eq!(config.llm_provider_name.as_deref(), Some("anthropic"));
        assert_eq!(config.llm_api_key.as_deref(), Some("sk-ant-test-key"));
        assert_eq!(config.voyage_api_key.as_deref(), Some("voyage-test-key"));
        assert!(!config.compiled_cache_sync);
        assert_eq!(config.log_level, "debug");
    }

    #[test]
    fn test_cli_overrides_toml() {
        let toml_content = r#"
connection_uri = "mongodb://myhost:27018"
grpc_port = 9090
log_level = "debug"
"#;

        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(toml_content.as_bytes()).unwrap();

        let cli = CliArgs {
            config: Some(tmp.path().to_path_buf()),
            connection_uri: Some("mongodb://override:27019".to_string()),
            grpc_port: Some(7070),
            mcp_port: None,
            anthropic_api_key: None,
            openai_api_key: None,
            voyage_api_key: None,
            llm_base_url: None,
            llm_gateway_key: None,
            llm_auth_header: None,
            llm_model: None,
            llm_provider_type: None,
            compiled_cache_sync: None,
            log_level: Some("warn".to_string()),
            grpc_max_message_size: None,
            socket_path: None,
            transport: None,
            socket_permissions: None,
            otel_enabled: None,
            otel_endpoint: None,
            otel_service_name: None,
            stream_batch_size: None,
            stream_idle_timeout_secs: None,
            grpc_compression: None,
            pipeline_timeout_secs: None,
            pipeline_max_concurrency: None,
            stdio: false,
        };

        let config = Config::load(&cli).unwrap();
        assert_eq!(config.connection_uri, "mongodb://override:27019");
        assert_eq!(config.grpc_port, 7070);
        assert_eq!(config.mcp_port, 3000); // default since not in CLI or TOML
        assert_eq!(config.log_level, "warn");
    }

    #[test]
    fn test_invalid_toml_returns_error() {
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(b"this is not valid toml [[[").unwrap();

        let cli = CliArgs {
            config: Some(tmp.path().to_path_buf()),
            connection_uri: None,
            grpc_port: None,
            mcp_port: None,
            anthropic_api_key: None,
            openai_api_key: None,
            voyage_api_key: None,
            llm_base_url: None,
            llm_gateway_key: None,
            llm_auth_header: None,
            llm_model: None,
            llm_provider_type: None,
            compiled_cache_sync: None,
            log_level: None,
            grpc_max_message_size: None,
            socket_path: None,
            transport: None,
            socket_permissions: None,
            otel_enabled: None,
            otel_endpoint: None,
            otel_service_name: None,
            stream_batch_size: None,
            stream_idle_timeout_secs: None,
            grpc_compression: None,
            pipeline_timeout_secs: None,
            pipeline_max_concurrency: None,
            stdio: false,
        };

        let result = Config::load(&cli);
        assert!(result.is_err());
    }

    #[test]
    fn test_tenant_config_parsing() {
        let toml_content = r#"
connection_uri = "mongodb://localhost:27017"
multi_tenant_enabled = true

[[tenants]]
tenant_id = "acme"
max_connections = 20
rate_limit_ops_per_sec = 500

[[tenants]]
tenant_id = "beta"
max_connections = 5
connection_uri = "mongodb://other:27017"
"#;

        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(toml_content.as_bytes()).unwrap();

        let cli = CliArgs {
            config: Some(tmp.path().to_path_buf()),
            connection_uri: None,
            grpc_port: None,
            mcp_port: None,
            anthropic_api_key: None,
            openai_api_key: None,
            voyage_api_key: None,
            llm_base_url: None,
            llm_gateway_key: None,
            llm_auth_header: None,
            llm_model: None,
            llm_provider_type: None,
            compiled_cache_sync: None,
            log_level: None,
            grpc_max_message_size: None,
            socket_path: None,
            transport: None,
            socket_permissions: None,
            otel_enabled: None,
            otel_endpoint: None,
            otel_service_name: None,
            stream_batch_size: None,
            stream_idle_timeout_secs: None,
            grpc_compression: None,
            pipeline_timeout_secs: None,
            pipeline_max_concurrency: None,
            stdio: false,
        };

        let config = Config::load(&cli).unwrap();
        assert!(config.multi_tenant_enabled);
        assert_eq!(config.tenants.len(), 2);

        // Check first tenant
        assert_eq!(config.tenants[0].tenant_id.as_deref(), Some("acme"));
        assert_eq!(config.tenants[0].max_connections, Some(20));
        assert_eq!(config.tenants[0].rate_limit_ops_per_sec, Some(500));
        assert!(config.tenants[0].connection_uri.is_none());
        assert!(config.tenants[0].max_cache_entries.is_none());

        // Check second tenant
        assert_eq!(config.tenants[1].tenant_id.as_deref(), Some("beta"));
        assert_eq!(config.tenants[1].max_connections, Some(5));
        assert_eq!(
            config.tenants[1].connection_uri.as_deref(),
            Some("mongodb://other:27017")
        );
        assert!(config.tenants[1].rate_limit_ops_per_sec.is_none());
        assert!(config.tenants[1].max_cache_entries.is_none());
    }

    fn default_cli() -> CliArgs {
        CliArgs {
            config: None,
            connection_uri: None,
            grpc_port: None,
            mcp_port: None,
            anthropic_api_key: None,
            openai_api_key: None,
            voyage_api_key: None,
            llm_base_url: None,
            llm_gateway_key: None,
            llm_auth_header: None,
            llm_model: None,
            llm_provider_type: None,
            compiled_cache_sync: None,
            log_level: None,
            grpc_max_message_size: None,
            socket_path: None,
            transport: None,
            socket_permissions: None,
            otel_enabled: None,
            otel_endpoint: None,
            otel_service_name: None,
            stream_batch_size: None,
            stream_idle_timeout_secs: None,
            grpc_compression: None,
            pipeline_timeout_secs: None,
            pipeline_max_concurrency: None,
            stdio: false,
        }
    }

    #[test]
    fn test_ingestion_config_defaults() {
        let cli = default_cli();
        let config = Config::load(&cli).unwrap();
        assert!(config.ingestion.enabled);
        assert_eq!(config.ingestion.sample_size, 1000);
        assert_eq!(config.ingestion.default_batch_size, 1000);
        assert_eq!(config.ingestion.default_concurrency, 4);
        assert!(!config.ingestion.llm_expressions);
    }

    #[test]
    fn test_ingestion_config_from_toml() {
        let toml_content = r#"
connection_uri = "mongodb://localhost:27017"

[ingestion]
enabled = true
sample_size = 2000
default_batch_size = 500
default_concurrency = 8
max_file_size_mb = 5120
llm_expressions = false
max_llm_concurrency = 2
watch_debounce_ms = 3000

[ingestion.watch]
enabled = true
path = "/data/incoming"
file_pattern = "*.csv"
database = "imports"
collection = "data"
conflict_strategy = "merge"
"#;
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(toml_content.as_bytes()).unwrap();
        let cli = CliArgs {
            config: Some(tmp.path().to_path_buf()),
            connection_uri: None,
            grpc_port: None,
            mcp_port: None,
            anthropic_api_key: None,
            openai_api_key: None,
            voyage_api_key: None,
            llm_base_url: None,
            llm_gateway_key: None,
            llm_auth_header: None,
            llm_model: None,
            llm_provider_type: None,
            compiled_cache_sync: None,
            log_level: None,
            grpc_max_message_size: None,
            socket_path: None,
            transport: None,
            socket_permissions: None,
            otel_enabled: None,
            otel_endpoint: None,
            otel_service_name: None,
            stream_batch_size: None,
            stream_idle_timeout_secs: None,
            grpc_compression: None,
            pipeline_timeout_secs: None,
            pipeline_max_concurrency: None,
            stdio: false,
        };
        let config = Config::load(&cli).unwrap();
        assert_eq!(config.ingestion.sample_size, 2000);
        assert_eq!(config.ingestion.default_batch_size, 500);
        assert_eq!(config.ingestion.watch_debounce_ms, 3000);
        let watch = config.ingestion.watch.unwrap();
        assert_eq!(watch.enabled, Some(true));
        assert_eq!(watch.path.as_deref(), Some("/data/incoming"));
    }

    #[test]
    fn test_gateway_config_from_toml() {
        let toml_content = r#"
connection_uri = "mongodb://localhost:27017"
LLM_BASE_URL = "https://gateway.example.com/anthropic/v1/messages"
LLM_API_KEY = "gw-key-123"
LLM_AUTH_HEADER = "x-custom-key"
LLM_MODEL = "claude-sonnet-4-6"
LLM_PROVIDER_TYPE = "anthropic"
"#;
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(toml_content.as_bytes()).unwrap();
        let mut cli = default_cli();
        cli.config = Some(tmp.path().to_path_buf());

        let config = Config::load(&cli).unwrap();
        let gw = config.llm_gateway.expect("gateway should be configured");
        assert_eq!(gw.base_url, "https://gateway.example.com/anthropic/v1/messages");
        assert_eq!(gw.api_key, "gw-key-123");
        assert_eq!(gw.auth_header, "x-custom-key");
        assert_eq!(gw.model, "claude-sonnet-4-6");
        assert_eq!(gw.provider_type, "anthropic");
    }

    #[test]
    fn test_gateway_takes_precedence_over_direct_keys() {
        let toml_content = r#"
connection_uri = "mongodb://localhost:27017"
ANTHROPIC_API_KEY = "direct-key"
LLM_BASE_URL = "https://gateway.example.com/v1/messages"
LLM_API_KEY = "gw-key"
"#;
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(toml_content.as_bytes()).unwrap();
        let mut cli = default_cli();
        cli.config = Some(tmp.path().to_path_buf());

        let config = Config::load(&cli).unwrap();
        assert!(config.llm_gateway.is_some(), "Gateway should be configured");
        // Direct key is still resolved (for non-gateway uses) but gateway takes priority
        assert!(config.llm_api_key.is_some());
    }

    #[test]
    fn test_stdio_flag_default_false() {
        let cli = default_cli();
        assert!(!cli.stdio);
    }
}
