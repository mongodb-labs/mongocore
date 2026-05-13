pub mod server;
pub mod service;

pub mod proto {
    tonic::include_proto!("mongocore.v1");
}

pub use server::{start_grpc_server, GrpcServerConfig};
pub use service::MongoCoreService;
