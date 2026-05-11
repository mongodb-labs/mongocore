use std::sync::Arc;
use tokio::task::JoinHandle;
use tonic::transport::Server;
use tracing::info;

use crate::analytics::AnalyticsCollector;
use crate::connection::pool::ConnectionPool;
use crate::ingestion::{DirectoryWatcher, IngestionEngine};

use super::proto::mongo_core_server::MongoCoreServer;
use super::service::MongoCoreService;

/// Start the gRPC server on the specified port.
///
/// Returns a `JoinHandle` so the server can be spawned alongside other tasks.
pub fn start_grpc_server(
    pool: ConnectionPool,
    port: u16,
    voyage_api_key: Option<&str>,
    analytics: Option<Arc<AnalyticsCollector>>,
    ingestion_engine: Option<Arc<IngestionEngine>>,
    directory_watcher: Option<Arc<DirectoryWatcher>>,
) -> JoinHandle<Result<(), tonic::transport::Error>> {
    let addr = format!("[::]:{}", port).parse().expect("Invalid address");
    let service = match voyage_api_key {
        Some(key) => MongoCoreService::with_voyage(pool.clone(), key, analytics, None, None),
        None => MongoCoreService::new(pool.clone(), analytics, None, None),
    };

    let service = if let (Some(engine), Some(watcher)) = (ingestion_engine, directory_watcher) {
        service.with_ingestion(engine, watcher, pool.client().clone())
    } else {
        service
    };

    info!("gRPC server listening on {}", addr);

    tokio::spawn(async move {
        Server::builder()
            .add_service(MongoCoreServer::new(service))
            .serve(addr)
            .await
    })
}
