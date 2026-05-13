use bson;
use mongocore::config::Config;
use mongocore::connection::pool::ConnectionPool;

pub const TEST_DB: &str = "mongocore_test";

/// Get a connected ConnectionPool for integration tests.
///
/// Uses the `MONGOCORE_TEST_URI` environment variable if set,
/// otherwise defaults to `mongodb://localhost:27017`.
pub async fn get_test_pool() -> ConnectionPool {
    let uri = std::env::var("MONGOCORE_TEST_URI")
        .unwrap_or_else(|_| "mongodb://localhost:27017".to_string());

    let config = Config {
        connection_uri: uri,
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
        analytics_enabled: false,
        analytics_buffer_size: 10000,
        analytics_flush_interval_secs: 300,
        ingestion: Default::default(),
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

    ConnectionPool::connect(&config)
        .await
        .expect("Failed to connect to test MongoDB instance")
}

/// Drop a specific collection to ensure clean state for a test.
pub async fn clean_collection(pool: &ConnectionPool, collection: &str) {
    pool.database(TEST_DB)
        .collection::<bson::Document>(collection)
        .drop()
        .await
        .ok(); // Ignore errors if collection doesn't exist
}
