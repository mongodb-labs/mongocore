use std::sync::Arc;
use tokio::task::JoinHandle;
use tonic::transport::Server;
use tracing::{info, warn};

use crate::analytics::AnalyticsCollector;
use crate::connection::pool::ConnectionPool;
use crate::ingestion::{DirectoryWatcher, IngestionEngine};

use super::proto::mongo_core_server::MongoCoreServer;
use super::service::MongoCoreService;


/// Configuration for the gRPC server.
pub struct GrpcServerConfig {
    pub port: u16,
    pub transport: String,
    pub socket_path: String,
    pub socket_permissions: u32,
    pub max_message_size: usize,
    pub compression: String,
    pub stream_idle_timeout_secs: u64,
    pub pipeline_timeout_secs: u64,
    pub pipeline_max_concurrency: usize,
}

/// Start the gRPC server on TCP and optionally UDS.
pub fn start_grpc_server(
    pool: ConnectionPool,
    config: GrpcServerConfig,
    voyage_api_key: Option<&str>,
    analytics: Option<Arc<AnalyticsCollector>>,
    ingestion_engine: Option<Arc<IngestionEngine>>,
    directory_watcher: Option<Arc<DirectoryWatcher>>,
) -> JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
    let stream_idle_timeout = std::time::Duration::from_secs(config.stream_idle_timeout_secs);
    let pipeline_timeout = std::time::Duration::from_secs(config.pipeline_timeout_secs);
    let pipeline_max_concurrency = config.pipeline_max_concurrency;
    let service = match voyage_api_key {
        Some(key) => MongoCoreService::with_voyage(pool.clone(), key, analytics, None, None, stream_idle_timeout),
        None => MongoCoreService::new(pool.clone(), analytics, None, None, stream_idle_timeout),
    };
    let service = service.with_pipeline_config(pipeline_timeout, pipeline_max_concurrency);

    let service = if let (Some(engine), Some(watcher)) = (ingestion_engine, directory_watcher) {
        service.with_ingestion(engine, watcher, pool.client().clone())
    } else {
        service
    };

    let grpc_service = {
        let svc = MongoCoreServer::new(service)
            .max_decoding_message_size(config.max_message_size)
            .max_encoding_message_size(config.max_message_size);
        match config.compression.as_str() {
            "gzip" => svc
                .send_compressed(tonic::codec::CompressionEncoding::Gzip)
                .accept_compressed(tonic::codec::CompressionEncoding::Gzip)
                .accept_compressed(tonic::codec::CompressionEncoding::Zstd),
            "zstd" => svc
                .send_compressed(tonic::codec::CompressionEncoding::Zstd)
                .accept_compressed(tonic::codec::CompressionEncoding::Gzip)
                .accept_compressed(tonic::codec::CompressionEncoding::Zstd),
            _ => svc
                .accept_compressed(tonic::codec::CompressionEncoding::Gzip)
                .accept_compressed(tonic::codec::CompressionEncoding::Zstd),
        }
    };

    let transport = config.transport.clone();
    let socket_path = config.socket_path.clone();
    let socket_permissions = config.socket_permissions;
    let port = config.port;

    let enable_tcp = transport != "uds";
    let enable_uds = transport != "tcp";

    tokio::spawn(async move {
        let addr = format!("[::]:{}", port).parse().expect("Invalid address");

        if enable_tcp {
            info!("gRPC server listening on {}", addr);
        }

        // Try to bind UDS if enabled
        let uds_listener = if enable_uds {
            let path = &socket_path;
            if std::path::Path::new(path).exists() {
                warn!("Removing stale socket file: {}", path);
                let _ = std::fs::remove_file(path);
            }

            match tokio::net::UnixListener::bind(path) {
                Ok(uds) => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let perms = std::fs::Permissions::from_mode(socket_permissions);
                        if let Err(e) = std::fs::set_permissions(path, perms) {
                            warn!("Failed to set socket permissions: {}", e);
                        }
                    }

                    info!("gRPC server also listening on UDS: {}", path);
                    Some(uds)
                }
                Err(e) => {
                    warn!("Failed to bind UDS at {}: {}. Falling back to TCP only.", path, e);
                    None
                }
            }
        } else {
            None
        };

        if let Some(uds) = uds_listener {
            let uds_stream = tokio_stream::wrappers::UnixListenerStream::new(uds);

            if enable_tcp {
                // Both TCP and UDS
                let tcp_server = Server::builder()
                    .initial_stream_window_size(1024 * 1024)
                    .initial_connection_window_size(4 * 1024 * 1024)
                    .max_concurrent_streams(Some(200))
                    .add_service(grpc_service.clone())
                    .serve(addr);

                let uds_server = Server::builder()
                    .initial_stream_window_size(1024 * 1024)
                    .initial_connection_window_size(4 * 1024 * 1024)
                    .max_concurrent_streams(Some(200))
                    .add_service(grpc_service)
                    .serve_with_incoming(uds_stream);

                tokio::select! {
                    r = tcp_server => r.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
                    r = uds_server => r.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
                }
            } else {
                // UDS only
                Server::builder()
                    .initial_stream_window_size(1024 * 1024)
                    .initial_connection_window_size(4 * 1024 * 1024)
                    .max_concurrent_streams(Some(200))
                    .add_service(grpc_service)
                    .serve_with_incoming(uds_stream)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
            }
        } else {
            // TCP only (either by config or UDS bind failure)
            Server::builder()
                .initial_stream_window_size(1024 * 1024)
                .initial_connection_window_size(4 * 1024 * 1024)
                .max_concurrent_streams(Some(200))
                .add_service(grpc_service)
                .serve(addr)
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    })
}
