pub mod handlers;
pub mod server;

use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::analytics::AnalyticsCollector;
use crate::compiled::translator::CompiledQueryTranslator;
use crate::config::Config;
use crate::connection::pool::ConnectionPool;
use crate::ingestion::engine::IngestionEngine;
use crate::ingestion::watch::DirectoryWatcher;

pub struct WebUiState {
    pub analytics: Option<Arc<AnalyticsCollector>>,
    pub pool: ConnectionPool,
    pub config: Config,
    pub translator: Option<Arc<CompiledQueryTranslator>>,
    pub ingestion_engine: Option<Arc<IngestionEngine>>,
    pub directory_watcher: Option<Arc<DirectoryWatcher>>,
    pub start_time: std::time::Instant,
}

pub fn start_web_ui_server(
    config: &Config,
    pool: ConnectionPool,
    analytics: Option<Arc<AnalyticsCollector>>,
    translator: Option<Arc<CompiledQueryTranslator>>,
    ingestion_engine: Option<Arc<IngestionEngine>>,
    directory_watcher: Option<Arc<DirectoryWatcher>>,
) -> Option<JoinHandle<()>> {
    if !config.web_ui_enabled {
        return None;
    }

    let state = Arc::new(WebUiState {
        analytics,
        pool,
        config: config.clone(),
        translator,
        ingestion_engine,
        directory_watcher,
        start_time: std::time::Instant::now(),
    });

    let port = config.web_ui_port;

    let handle = tokio::spawn(async move {
        let app = server::create_router(state);
        let addr = format!("127.0.0.1:{}", port);
        match tokio::net::TcpListener::bind(&addr).await {
            Ok(listener) => {
                info!("Web UI available at http://{}", addr);
                if let Err(e) = axum::serve(listener, app).await {
                    warn!("Web UI server error: {}", e);
                }
            }
            Err(e) => {
                warn!(
                    "Web UI: failed to bind port {} ({}), continuing without dashboard",
                    port, e
                );
            }
        }
    });

    Some(handle)
}
