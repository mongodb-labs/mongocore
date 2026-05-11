use tokio::task::JoinHandle;
use tonic::transport::Server;
use tracing::info;

use crate::connection::pool::ConnectionPool;

use super::proto::mongo_core_server::MongoCoreServer;
use super::service::MongoCoreService;

/// Start the gRPC server on the specified port.
///
/// Returns a `JoinHandle` so the server can be spawned alongside other tasks.
pub fn start_grpc_server(pool: ConnectionPool, port: u16) -> JoinHandle<Result<(), tonic::transport::Error>> {
    let addr = format!("[::]:{}", port).parse().expect("Invalid address");
    let service = MongoCoreService::new(pool);

    info!("gRPC server listening on {}", addr);

    tokio::spawn(async move {
        Server::builder()
            .add_service(MongoCoreServer::new(service))
            .serve(addr)
            .await
    })
}
