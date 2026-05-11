pub mod server;
pub mod service;

pub mod proto {
    tonic::include_proto!("mongocore.v1");
}

pub use server::start_grpc_server;
pub use service::MongoCoreService;
