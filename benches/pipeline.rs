use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

use mongocore::grpc::proto::mongo_core_client::MongoCoreClient;
use mongocore::grpc::proto::pipeline_operation::Operation;
use mongocore::grpc::proto::{ListDatabasesRequest, PipelineOperation, PipelineRequest};
use mongocore::grpc::{start_grpc_server, GrpcServerConfig};

#[path = "../tests/harness/mod.rs"]
mod harness;

/// Try to start a test server on a random port.
/// Returns None if MongoDB is not available (allows the benchmark to gracefully skip).
async fn try_start_test_server() -> Option<MongoCoreClient<tonic::transport::Channel>> {
    // Try to get MongoDB pool — if it fails, MongoDB isn't running
    let pool = match tokio::time::timeout(
        std::time::Duration::from_secs(2),
        harness::get_test_pool(),
    )
    .await
    {
        Ok(pool) => pool,
        Err(_) => return None,
    };

    // Find a free port by binding to port 0
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.ok()?;
    let port = listener.local_addr().ok()?.port();
    drop(listener);

    // Start the gRPC server
    let _handle = start_grpc_server(
        pool,
        GrpcServerConfig {
            port,
            transport: "tcp".to_string(),
            socket_path: "/tmp/mongocore.sock".to_string(),
            socket_permissions: 0o600,
            max_message_size: 64 * 1024 * 1024,
            compression: "none".to_string(),
            stream_idle_timeout_secs: 60,
            pipeline_timeout_secs: 30,
            pipeline_max_concurrency: 20,
        },
        None,
        None,
        None,
        None,
    );

    // Give the server time to bind
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Connect client
    MongoCoreClient::connect(format!("http://127.0.0.1:{}", port))
        .await
        .ok()
}

fn bench_pipeline_vs_individual(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // Try to start server — skip gracefully if MongoDB isn't running
    let client = match rt.block_on(try_start_test_server()) {
        Some(c) => c,
        None => {
            eprintln!("Skipping pipeline benchmark: MongoDB not running on localhost:27017");
            eprintln!("Start MongoDB with: just docker-up");
            return;
        }
    };

    let mut group = c.benchmark_group("pipeline_latency");
    group.sample_size(20); // Lower sample size since these are network calls

    for n_ops in [3, 5, 10, 20] {
        group.bench_with_input(
            BenchmarkId::new("individual_rpcs", n_ops),
            &n_ops,
            |b, &n| {
                b.to_async(&rt).iter(|| {
                    let mut client = client.clone();
                    async move {
                        for _ in 0..n {
                            // Use ListDatabases as a lightweight operation
                            client
                                .list_databases(ListDatabasesRequest {})
                                .await
                                .unwrap();
                        }
                    }
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("pipeline_rpc", n_ops),
            &n_ops,
            |b, &n| {
                b.to_async(&rt).iter(|| {
                    let mut client = client.clone();
                    async move {
                        let ops = (0..n)
                            .map(|_| PipelineOperation {
                                operation: Some(Operation::ListDatabases(ListDatabasesRequest {})),
                            })
                            .collect();
                        client
                            .pipeline(PipelineRequest { operations: ops })
                            .await
                            .unwrap();
                    }
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_pipeline_vs_individual);
criterion_main!(benches);
