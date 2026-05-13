use std::collections::HashSet;
use std::sync::Mutex;

use bson::{doc, Document};
use mongodb::options::{ClientOptions, DriverInfo, SelectionCriteria};
use mongodb::{Client, Collection, Database};
use tracing::info;

use crate::config::Config;
use crate::defaults::{
    default_read_concern, default_read_preference, default_write_concern, DEFAULT_RETRYABLE_READS,
    DEFAULT_RETRYABLE_WRITES,
};
use crate::error::MongoCoreError;

/// Detected MongoDB server capabilities.
#[derive(Debug, Clone)]
pub struct Capabilities {
    /// MongoDB server version string (e.g., "7.0.4").
    pub server_version: String,
    /// Whether Atlas Vector Search is available.
    pub atlas_vector_search: bool,
    /// Whether Atlas Search is available.
    pub atlas_search: bool,
}

/// MongoCore connection pool wrapping a `mongodb::Client` with opinionated defaults.
#[derive(Debug)]
pub struct ConnectionPool {
    client: Client,
    capabilities: Capabilities,
    host: String,
    appended_interfaces: Mutex<HashSet<String>>,
}

impl ConnectionPool {
    /// Create a new connection pool from the given config, apply opinionated defaults,
    /// verify connectivity via ping, and detect server capabilities.
    pub async fn connect(config: &Config) -> Result<Self, MongoCoreError> {
        let client_options = Self::build_client_options(config).await?;

        // Extract host info for logging before moving options into Client
        let host = client_options
            .hosts
            .first()
            .map(|h| h.to_string())
            .unwrap_or_else(|| config.connection_uri.clone());

        let client = Client::with_options(client_options)?;

        // Verify connectivity with a ping
        client
            .database("admin")
            .run_command(doc! { "ping": 1 })
            .await?;

        // Detect capabilities
        let capabilities = Self::detect_capabilities(&client).await?;

        let pool = Self {
            client,
            capabilities,
            host,
            appended_interfaces: Mutex::new(HashSet::new()),
        };

        pool.log_startup_banner();

        Ok(pool)
    }

    /// Build `ClientOptions` with MongoCore's opinionated defaults applied.
    pub async fn build_client_options(config: &Config) -> Result<ClientOptions, MongoCoreError> {
        let mut options = ClientOptions::parse(&config.connection_uri).await?;

        options.write_concern = Some(default_write_concern());
        options.read_concern = Some(default_read_concern());
        options.selection_criteria =
            Some(SelectionCriteria::ReadPreference(default_read_preference()));
        options.retry_writes = Some(DEFAULT_RETRYABLE_WRITES);
        options.retry_reads = Some(DEFAULT_RETRYABLE_READS);

        options.driver_info = Some(
            DriverInfo::builder()
                .name("mongocore".to_string())
                .version(env!("CARGO_PKG_VERSION").to_string())
                .build(),
        );

        Ok(options)
    }

    /// Detect server capabilities by running `buildInfo` and checking for Atlas features.
    async fn detect_capabilities(client: &Client) -> Result<Capabilities, MongoCoreError> {
        let admin_db = client.database("admin");

        // Get server version from buildInfo
        let build_info: Document = admin_db.run_command(doc! { "buildInfo": 1 }).await?;
        let server_version = build_info
            .get_str("version")
            .unwrap_or("unknown")
            .to_string();

        // Detect Atlas features by attempting to list search indexes on a non-existent collection.
        // On Atlas, the command succeeds (returns empty); on non-Atlas, it returns an error.
        let atlas_search = Self::detect_atlas_search(client).await;
        let atlas_vector_search = atlas_search; // Vector search availability implies Atlas Search and vice versa

        Ok(Capabilities {
            server_version,
            atlas_vector_search,
            atlas_search,
        })
    }

    /// Detect whether Atlas Search is available by running a `$listSearchIndexes` aggregation.
    async fn detect_atlas_search(client: &Client) -> bool {
        let db = client.database("admin");
        // Use listSearchIndexes command; this only works on Atlas.
        let result = db
            .run_command(doc! { "listSearchIndexes": "__mongocore_probe__" })
            .await;

        match result {
            Ok(_) => true,
            Err(e) => {
                let msg = e.to_string();
                // On Atlas without the collection, we get "ns not found" which still means Atlas is available.
                // On non-Atlas, we get "no such command" or "unrecognized" errors.
                msg.contains("ns not found")
                    || msg.contains("NamespaceNotFound")
                    || msg.contains("IndexNotFound")
            }
        }
    }

    /// Log the startup capability banner.
    fn log_startup_banner(&self) {
        let version = env!("CARGO_PKG_VERSION");
        let check = "\u{2713}"; // ✓
        let cross = "\u{2717}"; // ✗

        let atlas_vs = if self.capabilities.atlas_vector_search {
            check
        } else {
            cross
        };
        let atlas_s = if self.capabilities.atlas_search {
            check
        } else {
            cross
        };

        info!("MongoCore v{} connected to {}", version, self.host);
        info!(
            "  {} Wire protocol (MongoDB {})",
            check, self.capabilities.server_version
        );
        info!("  {} Atlas Vector Search", atlas_vs);
        info!("  {} Atlas Search", atlas_s);
    }

    /// Perform a health check by pinging MongoDB.
    pub async fn health_check(&self) -> Result<(), MongoCoreError> {
        self.client
            .database("admin")
            .run_command(doc! { "ping": 1 })
            .await?;
        Ok(())
    }

    /// Get a database handle by name.
    pub fn database(&self, name: &str) -> Database {
        self.client.database(name)
    }

    /// Get a collection handle for the given database and collection name.
    pub fn collection(&self, database: &str, collection: &str) -> Collection<Document> {
        self.client.database(database).collection(collection)
    }

    /// Get a reference to the underlying `mongodb::Client`.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Get the detected capabilities.
    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// Append interface metadata to the client's driver info.
    /// This is idempotent per interface - subsequent calls with the same interface name are no-ops.
    pub fn append_interface_metadata(&self, interface: &str) {
        let mut appended = self.appended_interfaces.lock().unwrap();
        if appended.contains(interface) {
            return;
        }
        let driver_info = DriverInfo::builder()
            .name(interface.to_string())
            .build();
        if self.client.append_metadata(driver_info).is_ok() {
            appended.insert(interface.to_string());
            tracing::debug!("Appended driver metadata for interface: {}", interface);
        }
    }
}

impl Clone for ConnectionPool {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            capabilities: self.capabilities.clone(),
            host: self.host.clone(),
            appended_interfaces: Mutex::new(HashSet::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_options_defaults_applied() {
        let config = Config {
            connection_uri: "mongodb://localhost:27017".to_string(),
            grpc_port: 50051,
            mcp_port: 3000,
            llm_api_key: None,
            llm_provider_name: None,
            voyage_api_key: None,
            llm_gateway: None,
            compiled_cache_sync: true,
            log_level: "info".to_string(),
            multi_tenant_enabled: false,
            tenants: vec![],
            analytics_enabled: true,
            analytics_buffer_size: 10000,
            analytics_flush_interval_secs: 300,
            ingestion: crate::config::ResolvedIngestionConfig::default(),
            grpc_max_message_size: 64 * 1024 * 1024,
            transport: "both".to_string(),
            socket_path: "/tmp/mongocore.sock".to_string(),
            socket_permissions: 0o600,
            otel_enabled: false,
            otel_endpoint: "http://localhost:4317".to_string(),
            otel_service_name: "mongocore".to_string(),
            stream_batch_size: 1000,
            stream_idle_timeout_secs: 60,
            grpc_compression: "none".to_string(),
            pipeline_timeout_secs: 30,
            pipeline_max_concurrency: 20,
        };

        let options = ConnectionPool::build_client_options(&config).await.unwrap();

        // Verify write concern is majority
        let wc = options.write_concern.expect("write_concern should be set");
        assert_eq!(wc, default_write_concern());

        // Verify read concern is majority
        let rc = options.read_concern.expect("read_concern should be set");
        assert_eq!(rc, default_read_concern());

        // Verify read preference is PrimaryPreferred
        let sel = options
            .selection_criteria
            .expect("selection_criteria should be set");
        match sel {
            SelectionCriteria::ReadPreference(rp) => {
                assert!(matches!(
                    rp,
                    mongodb::options::ReadPreference::PrimaryPreferred { .. }
                ));
            }
            _ => panic!("Expected ReadPreference selection criteria"),
        }

        // Verify retryable writes/reads
        assert_eq!(options.retry_writes, Some(true));
        assert_eq!(options.retry_reads, Some(true));
    }

    #[tokio::test]
    async fn test_client_options_custom_uri() {
        let config = Config {
            connection_uri: "mongodb://customhost:12345".to_string(),
            grpc_port: 50051,
            mcp_port: 3000,
            llm_api_key: None,
            llm_provider_name: None,
            voyage_api_key: None,
            llm_gateway: None,
            compiled_cache_sync: true,
            log_level: "info".to_string(),
            multi_tenant_enabled: false,
            tenants: vec![],
            analytics_enabled: true,
            analytics_buffer_size: 10000,
            analytics_flush_interval_secs: 300,
            ingestion: crate::config::ResolvedIngestionConfig::default(),
            grpc_max_message_size: 64 * 1024 * 1024,
            transport: "both".to_string(),
            socket_path: "/tmp/mongocore.sock".to_string(),
            socket_permissions: 0o600,
            otel_enabled: false,
            otel_endpoint: "http://localhost:4317".to_string(),
            otel_service_name: "mongocore".to_string(),
            stream_batch_size: 1000,
            stream_idle_timeout_secs: 60,
            grpc_compression: "none".to_string(),
            pipeline_timeout_secs: 30,
            pipeline_max_concurrency: 20,
        };

        let options = ConnectionPool::build_client_options(&config).await.unwrap();

        // Should have parsed the custom host
        let host_str = options.hosts.first().unwrap().to_string();
        assert!(host_str.contains("customhost"));
        assert!(host_str.contains("12345"));
    }

    #[tokio::test]
    async fn test_client_options_retryable_settings() {
        let config = Config {
            connection_uri: "mongodb://localhost:27017".to_string(),
            grpc_port: 50051,
            mcp_port: 3000,
            llm_api_key: None,
            llm_provider_name: None,
            voyage_api_key: None,
            llm_gateway: None,
            compiled_cache_sync: true,
            log_level: "info".to_string(),
            multi_tenant_enabled: false,
            tenants: vec![],
            analytics_enabled: true,
            analytics_buffer_size: 10000,
            analytics_flush_interval_secs: 300,
            ingestion: crate::config::ResolvedIngestionConfig::default(),
            grpc_max_message_size: 64 * 1024 * 1024,
            transport: "both".to_string(),
            socket_path: "/tmp/mongocore.sock".to_string(),
            socket_permissions: 0o600,
            otel_enabled: false,
            otel_endpoint: "http://localhost:4317".to_string(),
            otel_service_name: "mongocore".to_string(),
            stream_batch_size: 1000,
            stream_idle_timeout_secs: 60,
            grpc_compression: "none".to_string(),
            pipeline_timeout_secs: 30,
            pipeline_max_concurrency: 20,
        };

        let options = ConnectionPool::build_client_options(&config).await.unwrap();

        assert_eq!(options.retry_writes, Some(DEFAULT_RETRYABLE_WRITES));
        assert_eq!(options.retry_reads, Some(DEFAULT_RETRYABLE_READS));
    }

    #[tokio::test]
    async fn test_client_options_driver_info_set() {
        let config = Config {
            connection_uri: "mongodb://localhost:27017".to_string(),
            grpc_port: 50051,
            mcp_port: 3000,
            llm_api_key: None,
            llm_provider_name: None,
            voyage_api_key: None,
            llm_gateway: None,
            compiled_cache_sync: true,
            log_level: "info".to_string(),
            multi_tenant_enabled: false,
            tenants: vec![],
            analytics_enabled: true,
            analytics_buffer_size: 10000,
            analytics_flush_interval_secs: 300,
            ingestion: crate::config::ResolvedIngestionConfig::default(),
            grpc_max_message_size: 64 * 1024 * 1024,
            transport: "both".to_string(),
            socket_path: "/tmp/mongocore.sock".to_string(),
            socket_permissions: 0o600,
            otel_enabled: false,
            otel_endpoint: "http://localhost:4317".to_string(),
            otel_service_name: "mongocore".to_string(),
            stream_batch_size: 1000,
            stream_idle_timeout_secs: 60,
            grpc_compression: "none".to_string(),
            pipeline_timeout_secs: 30,
            pipeline_max_concurrency: 20,
        };

        let options = ConnectionPool::build_client_options(&config).await.unwrap();
        let driver_info = options.driver_info.expect("driver_info should be set");
        assert_eq!(driver_info.name, "mongocore");
        assert_eq!(driver_info.version.as_deref(), Some(env!("CARGO_PKG_VERSION")));
    }
}
