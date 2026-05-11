use clap::Parser;
use serde::Deserialize;
use std::path::PathBuf;

use crate::defaults::{
    DEFAULT_COMPILED_CACHE_SYNC, DEFAULT_CONNECTION_URI, DEFAULT_GRPC_PORT, DEFAULT_LOG_LEVEL,
    DEFAULT_MCP_PORT,
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

    /// LLM provider name
    #[arg(long, env = "MONGOCORE_LLM_PROVIDER")]
    pub llm_provider: Option<String>,

    /// Environment variable name containing the LLM API key
    #[arg(long, env = "MONGOCORE_LLM_API_KEY_ENV")]
    pub llm_api_key_env: Option<String>,

    /// Environment variable name containing the Voyage API key
    #[arg(long, env = "MONGOCORE_VOYAGE_API_KEY_ENV")]
    pub voyage_api_key_env: Option<String>,

    /// Enable compiled cache sync
    #[arg(long, env = "MONGOCORE_COMPILED_CACHE_SYNC")]
    pub compiled_cache_sync: Option<bool>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, env = "MONGOCORE_LOG_LEVEL")]
    pub log_level: Option<String>,
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

/// TOML file configuration structure.
#[derive(Debug, Deserialize, Default)]
pub struct FileConfig {
    pub connection_uri: Option<String>,
    pub grpc_port: Option<u16>,
    pub mcp_port: Option<u16>,
    pub llm_provider: Option<String>,
    pub llm_api_key_env: Option<String>,
    pub voyage_api_key_env: Option<String>,
    pub compiled_cache_sync: Option<bool>,
    pub log_level: Option<String>,
    pub multi_tenant_enabled: Option<bool>,
    pub tenants: Option<Vec<TenantFileConfig>>,
    pub analytics_enabled: Option<bool>,
    pub analytics_buffer_size: Option<usize>,
    pub analytics_flush_interval_secs: Option<u64>,
}

/// Resolved configuration for MongoCore.
#[derive(Debug, Clone)]
pub struct Config {
    pub connection_uri: String,
    pub grpc_port: u16,
    pub mcp_port: u16,
    pub llm_provider: Option<String>,
    pub llm_api_key_env: Option<String>,
    pub voyage_api_key_env: Option<String>,
    pub compiled_cache_sync: bool,
    pub log_level: String,
    pub multi_tenant_enabled: bool,
    pub tenants: Vec<TenantFileConfig>,
    pub analytics_enabled: bool,
    pub analytics_buffer_size: usize,
    pub analytics_flush_interval_secs: u64,
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

        let llm_provider = cli.llm_provider.clone().or(file_config.llm_provider);

        let llm_api_key_env = cli.llm_api_key_env.clone().or(file_config.llm_api_key_env);

        let voyage_api_key_env = cli
            .voyage_api_key_env
            .clone()
            .or(file_config.voyage_api_key_env);

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

        Ok(Config {
            connection_uri,
            grpc_port,
            mcp_port,
            llm_provider,
            llm_api_key_env,
            voyage_api_key_env,
            compiled_cache_sync,
            log_level,
            multi_tenant_enabled,
            tenants,
            analytics_enabled,
            analytics_buffer_size,
            analytics_flush_interval_secs,
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
            llm_provider: None,
            llm_api_key_env: None,
            voyage_api_key_env: None,
            compiled_cache_sync: None,
            log_level: None,
        };

        let config = Config::load(&cli).unwrap();
        assert_eq!(config.connection_uri, "mongodb://localhost:27017");
        assert_eq!(config.grpc_port, 50051);
        assert_eq!(config.mcp_port, 3000);
        assert_eq!(config.log_level, "info");
        assert!(config.compiled_cache_sync);
        assert!(config.llm_provider.is_none());
        assert!(config.llm_api_key_env.is_none());
        assert!(config.voyage_api_key_env.is_none());
    }

    #[test]
    fn test_toml_parsing() {
        let toml_content = r#"
connection_uri = "mongodb://myhost:27018"
grpc_port = 9090
mcp_port = 4000
llm_provider = "anthropic"
llm_api_key_env = "ANTHROPIC_API_KEY"
voyage_api_key_env = "VOYAGE_KEY"
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
            llm_provider: None,
            llm_api_key_env: None,
            voyage_api_key_env: None,
            compiled_cache_sync: None,
            log_level: None,
        };

        let config = Config::load(&cli).unwrap();
        assert_eq!(config.connection_uri, "mongodb://myhost:27018");
        assert_eq!(config.grpc_port, 9090);
        assert_eq!(config.mcp_port, 4000);
        assert_eq!(config.llm_provider.as_deref(), Some("anthropic"));
        assert_eq!(config.llm_api_key_env.as_deref(), Some("ANTHROPIC_API_KEY"));
        assert_eq!(config.voyage_api_key_env.as_deref(), Some("VOYAGE_KEY"));
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
            llm_provider: None,
            llm_api_key_env: None,
            voyage_api_key_env: None,
            compiled_cache_sync: None,
            log_level: Some("warn".to_string()),
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
            llm_provider: None,
            llm_api_key_env: None,
            voyage_api_key_env: None,
            compiled_cache_sync: None,
            log_level: None,
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
            llm_provider: None,
            llm_api_key_env: None,
            voyage_api_key_env: None,
            compiled_cache_sync: None,
            log_level: None,
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
}
