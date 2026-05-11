use mongocore::config::Config;
use mongocore::connection::pool::ConnectionPool;

const TEST_DB: &str = "mongocore_test";

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
        llm_provider: None,
        llm_api_key_env: None,
        voyage_api_key_env: None,
        compiled_cache_sync: true,
        log_level: "info".to_string(),
    };

    ConnectionPool::connect(&config)
        .await
        .expect("Failed to connect to test MongoDB instance")
}

/// Drop the test database to ensure a clean state between test runs.
pub async fn clean_test_db(pool: &ConnectionPool) {
    pool.database(TEST_DB)
        .drop()
        .await
        .expect("Failed to drop test database");
}
