# Performance Tier 1: UDS + Streaming Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.
> If modifying client libraries: verify imports work and run `just test-clients`.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

**Goal:** Reduce MongoCore's IPC overhead by switching to Unix Domain Sockets, adding streaming RPCs for bulk operations, raising gRPC message limits, and enabling optional compression.

**Architecture:** gRPC server runs dual listeners (TCP + UDS) on the same tonic service. New server-streaming RPCs (`FindStream`, `AggregateStream`) and bidirectional streaming (`InsertManyBidi`) handle large payloads without message size constraints. Existing unary RPCs remain unchanged for backwards compatibility.

**Tech Stack:** tonic 0.12 (UDS via `tokio::net::UnixListener`), tokio, prost, async-stream, protobuf streaming RPCs

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/defaults.rs` | New constants: `DEFAULT_GRPC_MAX_MESSAGE_SIZE`, `DEFAULT_STREAM_BATCH_SIZE`, `DEFAULT_STREAM_IDLE_TIMEOUT_SECS` |
| `src/config.rs` | New fields: `socket_path`, `socket_permissions`, `grpc_max_message_size`, `stream_batch_size`, `stream_idle_timeout_secs`, `grpc_compression` |
| `src/grpc/server.rs` | Dual listener (TCP + UDS), message size limits, compression config |
| `src/grpc/service.rs` | New streaming RPC handlers: `find_stream`, `aggregate_stream`, `insert_many_stream`, `insert_many_bidi` |
| `src/grpc/mod.rs` | Export updated `start_grpc_server` signature |
| `src/main.rs` | Pass new config fields to `start_grpc_server` |
| `proto/mongocore/v1/mongocore.proto` | New streaming RPC definitions and messages |
| `tests/harness/mod.rs` | UDS test helper |
| `tests/integration/streaming_test.rs` | Integration tests for streaming RPCs |
| `tests/integration/uds_test.rs` | Integration tests for UDS transport |
| `Cargo.toml` | Add `tokio-stream` features if needed, `tower` service layer for UDS |

---

### Task 1: Raise gRPC Message Limits

**Files:**
- Modify: `src/defaults.rs`
- Modify: `src/config.rs`
- Modify: `src/grpc/server.rs`
- Modify: `src/main.rs`
- Modify: `tests/harness/mod.rs` (Config struct literal)

- [ ] **Step 1: Add default constant**

In `src/defaults.rs`, add:

```rust
/// Default max gRPC message size (64 MB).
pub const DEFAULT_GRPC_MAX_MESSAGE_SIZE: usize = 64 * 1024 * 1024;
```

- [ ] **Step 2: Add config fields**

In `src/config.rs`, add to `CliArgs`:

```rust
/// Maximum gRPC message size in bytes
#[arg(long, env = "MONGOCORE_GRPC_MAX_MESSAGE_SIZE")]
pub grpc_max_message_size: Option<usize>,
```

Add to `FileConfig`:

```rust
pub grpc_max_message_size: Option<usize>,
```

Add to `Config`:

```rust
pub grpc_max_message_size: usize,
```

Add resolution in `Config::load()`:

```rust
let grpc_max_message_size = cli
    .grpc_max_message_size
    .or(file_config.grpc_max_message_size)
    .unwrap_or(DEFAULT_GRPC_MAX_MESSAGE_SIZE);
```

Add field to the `Ok(Config { ... })` block and import the new default.

- [ ] **Step 3: Update all Config struct literals**

Update `tests/harness/mod.rs` `get_test_pool()` Config literal to add:

```rust
grpc_max_message_size: 64 * 1024 * 1024,
```

Update all test Config literals in `src/config.rs` tests — search for `Config {` and add the new field to every instance. The `default_cli()` function already returns `CliArgs` with `None` for the new field.

Add the field to `CliArgs` struct literals in tests:

```rust
grpc_max_message_size: None,
```

- [ ] **Step 4: Update `start_grpc_server` to accept and apply message size**

In `src/grpc/server.rs`, change the function signature:

```rust
pub fn start_grpc_server(
    pool: ConnectionPool,
    port: u16,
    voyage_api_key: Option<&str>,
    analytics: Option<Arc<AnalyticsCollector>>,
    ingestion_engine: Option<Arc<IngestionEngine>>,
    directory_watcher: Option<Arc<DirectoryWatcher>>,
    grpc_max_message_size: usize,
) -> JoinHandle<Result<(), tonic::transport::Error>> {
```

Apply the limit to the server builder:

```rust
tokio::spawn(async move {
    Server::builder()
        .add_service(
            MongoCoreServer::new(service)
                .max_decoding_message_size(grpc_max_message_size)
                .max_encoding_message_size(grpc_max_message_size)
        )
        .serve(addr)
        .await
})
```

- [ ] **Step 5: Update `main.rs` to pass the new argument**

```rust
let grpc_handle = start_grpc_server(
    pool.clone(),
    config.grpc_port,
    voyage_api_key.as_deref(),
    analytics.clone(),
    ingestion_engine.clone(),
    directory_watcher.clone(),
    config.grpc_max_message_size,
);
```

- [ ] **Step 6: Update integration test `start_test_server`**

In `tests/integration/grpc_test.rs`, update the `start_grpc_server` call:

```rust
let _handle = start_grpc_server(pool, port, None, None, None, None, 64 * 1024 * 1024);
```

Search for ALL other `start_grpc_server` calls in `tests/integration/` and update them similarly.

- [ ] **Step 7: Build and test**

Run: `cargo build 2>&1 | grep "warning:"` — expect no output.
Run: `cargo test --lib` — expect all tests pass.
Run: `cargo test --test integration --no-run` — expect compilation success.

- [ ] **Step 8: Commit**

```bash
git add src/defaults.rs src/config.rs src/grpc/server.rs src/main.rs tests/
git commit -m "feat(grpc): raise max message size to 64MB (configurable)"
```

---

### Task 2: Unix Domain Socket Transport — Server Side

**Files:**
- Modify: `src/defaults.rs`
- Modify: `src/config.rs`
- Modify: `src/grpc/server.rs`
- Modify: `src/main.rs`
- Modify: `Cargo.toml` (if `hyper-util` or `tower` features needed)
- Modify: `tests/harness/mod.rs`

- [ ] **Step 1: Add UDS config defaults**

In `src/defaults.rs`:

```rust
/// Default Unix domain socket permissions (owner-only).
pub const DEFAULT_SOCKET_PERMISSIONS: u32 = 0o600;
```

- [ ] **Step 2: Add UDS config fields**

In `src/config.rs`, add to `CliArgs`:

```rust
/// Unix domain socket path (enables UDS transport when set)
#[arg(long, env = "MONGOCORE_SOCKET_PATH")]
pub socket_path: Option<String>,

/// Unix domain socket file permissions (octal, e.g. 0600)
#[arg(long, env = "MONGOCORE_SOCKET_PERMISSIONS")]
pub socket_permissions: Option<u32>,
```

Add to `FileConfig`:

```rust
pub socket_path: Option<String>,
pub socket_permissions: Option<u32>,
```

Add to `Config`:

```rust
pub socket_path: Option<String>,
pub socket_permissions: u32,
```

Add resolution in `Config::load()`:

```rust
let socket_path = cli
    .socket_path
    .clone()
    .or(file_config.socket_path);

let socket_permissions = cli
    .socket_permissions
    .or(file_config.socket_permissions)
    .unwrap_or(DEFAULT_SOCKET_PERMISSIONS);
```

Add to the `Ok(Config { ... })` block.

- [ ] **Step 3: Update all Config struct literals**

Add `socket_path: None, socket_permissions: 0o600,` to:
- `tests/harness/mod.rs` Config literal
- All test Config literals in `src/config.rs`

Add `socket_path: None, socket_permissions: None,` to all `CliArgs` struct literals in `src/config.rs` tests.

- [ ] **Step 4: Implement UDS listener in `start_grpc_server`**

Rewrite `src/grpc/server.rs` to support dual transport. The function now accepts an optional socket path:

```rust
use std::sync::Arc;
use tokio::task::JoinHandle;
use tonic::transport::Server;
use tracing::{info, warn};

use crate::analytics::AnalyticsCollector;
use crate::connection::pool::ConnectionPool;
use crate::ingestion::{DirectoryWatcher, IngestionEngine};

use super::proto::mongo_core_server::MongoCoreServer;
use super::service::MongoCoreService;

pub struct GrpcServerConfig {
    pub port: u16,
    pub socket_path: Option<String>,
    pub socket_permissions: u32,
    pub max_message_size: usize,
}

pub fn start_grpc_server(
    pool: ConnectionPool,
    config: GrpcServerConfig,
    voyage_api_key: Option<&str>,
    analytics: Option<Arc<AnalyticsCollector>>,
    ingestion_engine: Option<Arc<IngestionEngine>>,
    directory_watcher: Option<Arc<DirectoryWatcher>>,
) -> JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>> {
    let service = match voyage_api_key {
        Some(key) => MongoCoreService::with_voyage(pool.clone(), key, analytics, None, None),
        None => MongoCoreService::new(pool.clone(), analytics, None, None),
    };

    let service = if let (Some(engine), Some(watcher)) = (ingestion_engine, directory_watcher) {
        service.with_ingestion(engine, watcher, pool.client().clone())
    } else {
        service
    };

    let grpc_service = MongoCoreServer::new(service)
        .max_decoding_message_size(config.max_message_size)
        .max_encoding_message_size(config.max_message_size);

    let socket_path = config.socket_path.clone();
    let socket_permissions = config.socket_permissions;
    let port = config.port;

    tokio::spawn(async move {
        let addr = format!("[::]:{}", port).parse().expect("Invalid address");
        info!("gRPC server listening on {}", addr);

        if let Some(ref path) = socket_path {
            // Remove stale socket file
            if std::path::Path::new(path).exists() {
                warn!("Removing stale socket file: {}", path);
                let _ = std::fs::remove_file(path);
            }

            let uds = tokio::net::UnixListener::bind(path)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;

            // Set permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let perms = std::fs::Permissions::from_mode(socket_permissions);
                std::fs::set_permissions(path, perms)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
            }

            info!("gRPC server also listening on UDS: {}", path);

            let uds_stream = tokio_stream::wrappers::UnixListenerStream::new(uds);

            // Run both TCP and UDS concurrently
            let tcp_server = Server::builder()
                .add_service(grpc_service.clone())
                .serve(addr);

            let uds_server = Server::builder()
                .add_service(grpc_service)
                .serve_with_incoming(uds_stream);

            tokio::select! {
                r = tcp_server => r.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
                r = uds_server => r.map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
            }
        } else {
            Server::builder()
                .add_service(grpc_service)
                .serve(addr)
                .await
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
        }
    })
}
```

Note: `MongoCoreServer` wraps the service in an `Arc` internally, so `.clone()` works on the wrapped server type.

- [ ] **Step 5: Update `main.rs`**

Update the `start_grpc_server` call to use the new config struct:

```rust
use mongocore::grpc::server::GrpcServerConfig;

let grpc_handle = start_grpc_server(
    pool.clone(),
    GrpcServerConfig {
        port: config.grpc_port,
        socket_path: config.socket_path.clone(),
        socket_permissions: config.socket_permissions,
        max_message_size: config.grpc_max_message_size,
    },
    voyage_api_key.as_deref(),
    analytics.clone(),
    ingestion_engine.clone(),
    directory_watcher.clone(),
);
```

Update the `tokio::select!` match arm since the return type changed:

```rust
tokio::select! {
    result = grpc_handle => {
        match result {
            Ok(Ok(())) => info!("gRPC server shut down"),
            Ok(Err(e)) => error!("gRPC server error: {e}"),
            Err(e) => error!("gRPC server task panicked: {e}"),
        }
    }
    // ... mcp_handle unchanged
}
```

Also update `print_banner` to show socket path:

```rust
fn print_banner(config: &Config) {
    // ... existing banner ...
    if let Some(ref path) = config.socket_path {
        println!("  UDS path:  {}", path);
    }
    println!();
}
```

- [ ] **Step 6: Update `src/grpc/mod.rs` exports**

```rust
pub mod server;
pub mod service;

pub mod proto {
    tonic::include_proto!("mongocore.v1");
}

pub use server::{start_grpc_server, GrpcServerConfig};
pub use service::MongoCoreService;
```

- [ ] **Step 7: Update all integration test `start_grpc_server` calls**

Search for all calls to `start_grpc_server` in `tests/integration/` and update to use the new signature:

```rust
use mongocore::grpc::server::GrpcServerConfig;

let _handle = start_grpc_server(
    pool,
    GrpcServerConfig {
        port,
        socket_path: None,
        socket_permissions: 0o600,
        max_message_size: 64 * 1024 * 1024,
    },
    None,
    None,
    None,
    None,
);
```

- [ ] **Step 8: Add `tokio-stream` `UnixListenerStream` dependency check**

Verify `tokio-stream` has the `net` feature. In `Cargo.toml`, update:

```toml
tokio-stream = { version = "0.1", features = ["net"] }
```

- [ ] **Step 9: Build and test**

Run: `cargo build 2>&1 | grep "warning:"` — expect no output.
Run: `cargo test --lib` — expect all tests pass.
Run: `cargo test --test integration --no-run` — expect compilation success.

- [ ] **Step 10: Commit**

```bash
git add src/ tests/ Cargo.toml Cargo.lock
git commit -m "feat(grpc): add Unix Domain Socket transport support"
```

---

### Task 3: UDS Integration Test

**Files:**
- Create: `tests/integration/uds_test.rs`

- [ ] **Step 1: Write UDS integration test**

Create `tests/integration/uds_test.rs`:

```rust
use bson::doc;
use uuid::Uuid;

use mongocore::grpc::proto::mongo_core_client::MongoCoreClient;
use mongocore::grpc::proto::{Document, Filter, FindOneRequest, InsertRequest};
use mongocore::grpc::server::GrpcServerConfig;
use mongocore::grpc::start_grpc_server;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;

#[path = "../harness/mod.rs"]
mod harness;

const TEST_DB: &str = harness::TEST_DB;

fn unique_collection() -> String {
    format!("test_uds_{}", Uuid::new_v4().to_string().replace('-', ""))
}

fn encode_doc(doc: &bson::Document) -> Vec<u8> {
    let mut buf = Vec::new();
    doc.to_writer(&mut buf).unwrap();
    buf
}

async fn start_uds_server() -> (MongoCoreClient<Channel>, String) {
    let pool = harness::get_test_pool().await;
    let socket_path = format!("/tmp/mongocore_test_{}.sock", Uuid::new_v4());

    let _handle = start_grpc_server(
        pool,
        GrpcServerConfig {
            port: 0, // TCP port not needed for this test but required
            socket_path: Some(socket_path.clone()),
            socket_permissions: 0o600,
            max_message_size: 64 * 1024 * 1024,
        },
        None,
        None,
        None,
        None,
    );

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Connect via UDS
    let path = socket_path.clone();
    let channel = Endpoint::try_from("http://[::]:50051")
        .unwrap()
        .connect_with_connector(service_fn(move |_: Uri| {
            let path = path.clone();
            async move { tokio::net::UnixStream::connect(path).await }
        }))
        .await
        .unwrap();

    let client = MongoCoreClient::new(channel);
    (client, socket_path)
}

#[tokio::test]
async fn test_find_one_over_uds() {
    let (mut client, socket_path) = start_uds_server().await;
    let coll = unique_collection();

    // Insert a document
    let doc = doc! { "name": "uds_test", "value": 42 };
    let request = InsertRequest {
        database: TEST_DB.to_string(),
        collection: coll.clone(),
        document: Some(Document { data: encode_doc(&doc) }),
        transaction_id: None,
    };
    client.insert(request).await.unwrap();

    // Find it back over UDS
    let filter = doc! { "name": "uds_test" };
    let request = FindOneRequest {
        database: TEST_DB.to_string(),
        collection: coll,
        filter: Some(Filter { data: encode_doc(&filter) }),
        options: None,
        transaction_id: None,
    };
    let response = client.find_one(request).await.unwrap();
    let found = response.into_inner().document.unwrap();
    let found_doc = bson::Document::from_reader(&found.data[..]).unwrap();
    assert_eq!(found_doc.get_str("name").unwrap(), "uds_test");
    assert_eq!(found_doc.get_i32("value").unwrap(), 42);

    // Cleanup
    let _ = std::fs::remove_file(&socket_path);
}

#[tokio::test]
async fn test_socket_cleanup_on_restart() {
    let socket_path = format!("/tmp/mongocore_test_{}.sock", Uuid::new_v4());

    // Create a stale socket file
    std::fs::write(&socket_path, "stale").unwrap();
    assert!(std::path::Path::new(&socket_path).exists());

    let pool = harness::get_test_pool().await;

    // Server should remove stale socket and start successfully
    let _handle = start_grpc_server(
        pool,
        GrpcServerConfig {
            port: 0,
            socket_path: Some(socket_path.clone()),
            socket_permissions: 0o600,
            max_message_size: 64 * 1024 * 1024,
        },
        None,
        None,
        None,
        None,
    );

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Verify we can connect
    let path = socket_path.clone();
    let channel = Endpoint::try_from("http://[::]:50051")
        .unwrap()
        .connect_with_connector(service_fn(move |_: Uri| {
            let path = path.clone();
            async move { tokio::net::UnixStream::connect(path).await }
        }))
        .await
        .unwrap();

    let mut client = MongoCoreClient::new(channel);
    let response = client
        .list_databases(mongocore::grpc::proto::ListDatabasesRequest {})
        .await;
    assert!(response.is_ok());

    let _ = std::fs::remove_file(&socket_path);
}
```

- [ ] **Step 2: Verify test compiles**

Run: `cargo test --test integration --no-run` — expect compilation success.

- [ ] **Step 3: Run the UDS test (requires Docker MongoDB)**

Run: `cargo test --test integration uds_test -- --nocapture`

Expected: Both tests pass.

- [ ] **Step 4: Commit**

```bash
git add tests/integration/uds_test.rs
git commit -m "test(grpc): add UDS transport integration tests"
```

---

### Task 4: Streaming Proto Definitions

**Files:**
- Modify: `proto/mongocore/v1/mongocore.proto`
- Modify: `proto/mongocore/v1/types.proto`

- [ ] **Step 1: Add streaming messages to `types.proto`**

Add to the end of `proto/mongocore/v1/types.proto`:

```protobuf
// Streaming batch of documents (used by FindStream, AggregateStream)
message DocumentBatch {
  repeated Document documents = 1;
  uint32 batch_index = 2;
  bool has_more = 3;
}

// Streaming insert batch (client-to-server)
message InsertBatch {
  string database = 1;
  string collection = 2;
  repeated Document documents = 3;
}

// Per-batch acknowledgment for bidirectional insert
message InsertBatchAck {
  uint32 batch_index = 1;
  uint32 inserted_count = 2;
  repeated InsertError errors = 3;
}

message InsertError {
  uint32 index = 1;
  string message = 2;
  int32 code = 3;
}

// Response for unidirectional streaming insert
message InsertManyStreamResponse {
  uint64 total_inserted = 1;
  repeated InsertError errors = 2;
}
```

- [ ] **Step 2: Add streaming RPCs to `mongocore.proto`**

Add these RPCs inside the `service MongoCore { ... }` block, after the existing `Watch` RPC:

```protobuf
  // Streaming bulk operations
  rpc FindStream(FindStreamRequest) returns (stream DocumentBatch);
  rpc AggregateStream(AggregateStreamRequest) returns (stream DocumentBatch);
  rpc InsertManyStream(stream InsertBatch) returns (InsertManyStreamResponse);
  rpc InsertManyBidi(stream InsertBatch) returns (stream InsertBatchAck);
```

Add the request messages after the existing `WatchEvent` message (before `RunCommandRequest`):

```protobuf
// ==================== Streaming Bulk ====================

message FindStreamRequest {
  string database = 1;
  string collection = 2;
  Filter filter = 3;
  FindOptions options = 4;
  optional string transaction_id = 5;
  uint32 batch_size = 6;
}

message AggregateStreamRequest {
  string database = 1;
  string collection = 2;
  Pipeline pipeline = 3;
  optional string transaction_id = 4;
  uint32 batch_size = 5;
}
```

- [ ] **Step 3: Verify proto compiles**

Run: `cargo build` — this triggers tonic-build to regenerate Rust stubs.

Expected: Build succeeds (there will be warnings about unimplemented trait methods — that's expected and we'll fix in the next task).

- [ ] **Step 4: Commit**

```bash
git add proto/
git commit -m "feat(proto): add streaming RPCs for bulk operations"
```

---

### Task 5: Implement Streaming RPC Handlers

**Files:**
- Modify: `src/defaults.rs`
- Modify: `src/config.rs`
- Modify: `src/grpc/service.rs`
- Modify: `tests/harness/mod.rs`

- [ ] **Step 1: Add streaming config defaults**

In `src/defaults.rs`:

```rust
/// Default streaming batch size (documents per frame).
pub const DEFAULT_STREAM_BATCH_SIZE: u32 = 1000;

/// Default stream idle timeout in seconds.
pub const DEFAULT_STREAM_IDLE_TIMEOUT_SECS: u64 = 60;

/// Minimum allowed batch size.
pub const MIN_STREAM_BATCH_SIZE: u32 = 1;

/// Maximum allowed batch size.
pub const MAX_STREAM_BATCH_SIZE: u32 = 10000;
```

- [ ] **Step 2: Add streaming config to Config**

In `src/config.rs`, add to `CliArgs`:

```rust
/// Default streaming batch size
#[arg(long, env = "MONGOCORE_STREAM_BATCH_SIZE")]
pub stream_batch_size: Option<u32>,

/// Stream idle timeout in seconds
#[arg(long, env = "MONGOCORE_STREAM_IDLE_TIMEOUT_SECS")]
pub stream_idle_timeout_secs: Option<u64>,
```

Add to `FileConfig`:

```rust
pub stream_batch_size: Option<u32>,
pub stream_idle_timeout_secs: Option<u64>,
```

Add to `Config`:

```rust
pub stream_batch_size: u32,
pub stream_idle_timeout_secs: u64,
```

Add resolution in `Config::load()`:

```rust
let stream_batch_size = cli
    .stream_batch_size
    .or(file_config.stream_batch_size)
    .unwrap_or(DEFAULT_STREAM_BATCH_SIZE);

let stream_idle_timeout_secs = cli
    .stream_idle_timeout_secs
    .or(file_config.stream_idle_timeout_secs)
    .unwrap_or(DEFAULT_STREAM_IDLE_TIMEOUT_SECS);
```

Import the new defaults and add fields to the `Ok(Config { ... })` return block.

Update ALL Config struct literals in tests and harness:

```rust
stream_batch_size: 1000,
stream_idle_timeout_secs: 60,
```

Add to `CliArgs` test literals:

```rust
stream_batch_size: None,
stream_idle_timeout_secs: None,
```

- [ ] **Step 3: Implement `find_stream` handler**

In `src/grpc/service.rs`, add the stream type aliases and implementations inside the `impl MongoCore for MongoCoreService` block.

First, add the type alias (near the existing `type WatchStream`):

```rust
type FindStreamStream = Pin<
    Box<dyn tokio_stream::Stream<Item = Result<proto::DocumentBatch, Status>> + Send + 'static>,
>;
```

Then implement the handler:

```rust
async fn find_stream(
    &self,
    request: Request<proto::FindStreamRequest>,
) -> Result<Response<Self::FindStreamStream>, Status> {
    self.append_client_language(request.metadata());
    self.check_tenant_quota(request.metadata())?;
    let req = request.into_inner();
    let filter = proto_filter_to_bson(&req.filter)?;
    let options = convert_find_options(&req.options)?;

    let batch_size = req.batch_size.max(MIN_STREAM_BATCH_SIZE).min(MAX_STREAM_BATCH_SIZE);

    let cursor = if let Some(ref txn_id) = req.transaction_id {
        let mut txn = self
            .transactions
            .get_mut(txn_id)
            .ok_or_else(|| Status::not_found(format!("Transaction not found: {}", txn_id)))?;
        txn.find_cursor(&req.database, &req.collection, filter).await
    } else {
        self.operations
            .find_cursor(&req.database, &req.collection, filter, options)
            .await
    }.map_err(to_status)?;

    let stream = async_stream::stream! {
        let mut cursor = cursor;
        let mut batch_index: u32 = 0;
        let mut batch: Vec<proto::Document> = Vec::with_capacity(batch_size as usize);

        while let Some(result) = cursor.next().await {
            match result {
                Ok(doc) => {
                    let bytes = bson::to_vec(&doc).unwrap_or_default();
                    batch.push(proto::Document { data: bytes });

                    if batch.len() >= batch_size as usize {
                        yield Ok(proto::DocumentBatch {
                            documents: std::mem::take(&mut batch),
                            batch_index,
                            has_more: true,
                        });
                        batch_index += 1;
                        batch = Vec::with_capacity(batch_size as usize);
                    }
                }
                Err(e) => {
                    yield Err(Status::internal(format!("Cursor error: {}", e)));
                    return;
                }
            }
        }

        // Final batch
        yield Ok(proto::DocumentBatch {
            documents: batch,
            batch_index,
            has_more: false,
        });
    };

    Ok(Response::new(Box::pin(stream)))
}
```

Note: This requires adding a `find_cursor` method to `Operations` (see Step 5).

- [ ] **Step 4: Implement `aggregate_stream` handler**

Add the type alias:

```rust
type AggregateStreamStream = Pin<
    Box<dyn tokio_stream::Stream<Item = Result<proto::DocumentBatch, Status>> + Send + 'static>,
>;
```

Then the handler:

```rust
async fn aggregate_stream(
    &self,
    request: Request<proto::AggregateStreamRequest>,
) -> Result<Response<Self::AggregateStreamStream>, Status> {
    self.append_client_language(request.metadata());
    self.check_tenant_quota(request.metadata())?;
    let req = request.into_inner();

    let pipeline: Vec<bson::Document> = match req.pipeline {
        Some(p) => p
            .stages
            .iter()
            .map(|s| bson::from_slice(s))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| Status::invalid_argument(format!("Invalid pipeline: {}", e)))?,
        None => vec![],
    };

    let batch_size = req.batch_size.max(MIN_STREAM_BATCH_SIZE).min(MAX_STREAM_BATCH_SIZE);

    let cursor = self
        .operations
        .aggregate_cursor(&req.database, &req.collection, pipeline)
        .await
        .map_err(to_status)?;

    let stream = async_stream::stream! {
        let mut cursor = cursor;
        let mut batch_index: u32 = 0;
        let mut batch: Vec<proto::Document> = Vec::with_capacity(batch_size as usize);

        while let Some(result) = cursor.next().await {
            match result {
                Ok(doc) => {
                    let bytes = bson::to_vec(&doc).unwrap_or_default();
                    batch.push(proto::Document { data: bytes });

                    if batch.len() >= batch_size as usize {
                        yield Ok(proto::DocumentBatch {
                            documents: std::mem::take(&mut batch),
                            batch_index,
                            has_more: true,
                        });
                        batch_index += 1;
                        batch = Vec::with_capacity(batch_size as usize);
                    }
                }
                Err(e) => {
                    yield Err(Status::internal(format!("Cursor error: {}", e)));
                    return;
                }
            }
        }

        yield Ok(proto::DocumentBatch {
            documents: batch,
            batch_index,
            has_more: false,
        });
    };

    Ok(Response::new(Box::pin(stream)))
}
```

- [ ] **Step 5: Add `find_cursor` and `aggregate_cursor` methods to Operations**

In `src/operations/crud.rs` (or wherever `find` is implemented), add a method that returns the raw cursor instead of collecting results:

```rust
pub async fn find_cursor(
    &self,
    database: &str,
    collection: &str,
    filter: bson::Document,
    options: Option<mongodb::options::FindOptions>,
) -> Result<mongodb::Cursor<bson::Document>, MongoCoreError> {
    let coll = self.pool.collection(database, collection);
    let mut find = coll.find(filter);
    if let Some(opts) = options {
        find = find.with_options(opts);
    }
    let cursor = find.await?;
    Ok(cursor)
}
```

In `src/operations/aggregation.rs`, add:

```rust
pub async fn aggregate_cursor(
    &self,
    database: &str,
    collection: &str,
    pipeline: Vec<bson::Document>,
) -> Result<mongodb::Cursor<bson::Document>, MongoCoreError> {
    let coll = self.pool.collection(database, collection);
    let cursor = coll.aggregate(pipeline).await?;
    Ok(cursor)
}
```

- [ ] **Step 6: Implement `insert_many_stream` handler (client-streaming)**

Add the handler:

```rust
async fn insert_many_stream(
    &self,
    request: Request<tonic::Streaming<proto::InsertBatch>>,
) -> Result<Response<proto::InsertManyStreamResponse>, Status> {
    self.append_client_language(request.metadata());
    let mut stream = request.into_inner();
    let mut total_inserted: u64 = 0;
    let mut errors: Vec<proto::InsertError> = Vec::new();
    let mut database = String::new();
    let mut collection = String::new();
    let mut global_index: u32 = 0;

    while let Some(batch) = stream.message().await.map_err(|e| {
        Status::internal(format!("Stream receive error: {}", e))
    })? {
        // Capture db/collection from first batch
        if database.is_empty() {
            database = batch.database;
            collection = batch.collection;
        }

        let docs: Result<Vec<bson::Document>, Status> = batch
            .documents
            .iter()
            .map(|d| proto_doc_to_bson(d))
            .collect();
        let docs = docs?;
        let batch_len = docs.len() as u32;

        match self.operations.insert_many(&database, &collection, docs).await {
            Ok(result) => {
                total_inserted += result.inserted_ids.len() as u64;
            }
            Err(e) => {
                errors.push(proto::InsertError {
                    index: global_index,
                    message: e.to_string(),
                    code: 0,
                });
            }
        }
        global_index += batch_len;
    }

    Ok(Response::new(proto::InsertManyStreamResponse {
        total_inserted,
        errors,
    }))
}
```

- [ ] **Step 7: Implement `insert_many_bidi` handler (bidirectional streaming)**

Add the type alias:

```rust
type InsertManyBidiStream = Pin<
    Box<dyn tokio_stream::Stream<Item = Result<proto::InsertBatchAck, Status>> + Send + 'static>,
>;
```

Then the handler:

```rust
async fn insert_many_bidi(
    &self,
    request: Request<tonic::Streaming<proto::InsertBatch>>,
) -> Result<Response<Self::InsertManyBidiStream>, Status> {
    self.append_client_language(request.metadata());
    let mut inbound = request.into_inner();
    let operations = self.operations.clone();

    let stream = async_stream::stream! {
        let mut database = String::new();
        let mut collection = String::new();
        let mut batch_index: u32 = 0;

        while let Some(result) = inbound.message().await.transpose() {
            match result {
                Ok(batch) => {
                    if database.is_empty() {
                        database = batch.database;
                        collection = batch.collection;
                    }

                    let docs: Result<Vec<bson::Document>, Status> = batch
                        .documents
                        .iter()
                        .map(|d| proto_doc_to_bson(d))
                        .collect();

                    match docs {
                        Ok(docs) => {
                            match operations.insert_many(&database, &collection, docs).await {
                                Ok(result) => {
                                    yield Ok(proto::InsertBatchAck {
                                        batch_index,
                                        inserted_count: result.inserted_ids.len() as u32,
                                        errors: vec![],
                                    });
                                }
                                Err(e) => {
                                    yield Ok(proto::InsertBatchAck {
                                        batch_index,
                                        inserted_count: 0,
                                        errors: vec![proto::InsertError {
                                            index: 0,
                                            message: e.to_string(),
                                            code: 0,
                                        }],
                                    });
                                }
                            }
                        }
                        Err(e) => {
                            yield Err(e);
                            return;
                        }
                    }
                    batch_index += 1;
                }
                Err(e) => {
                    yield Err(Status::internal(format!("Stream error: {}", e)));
                    return;
                }
            }
        }
    };

    Ok(Response::new(Box::pin(stream)))
}
```

- [ ] **Step 8: Add required imports to `service.rs`**

At the top of `src/grpc/service.rs`, ensure these imports exist:

```rust
use futures::StreamExt;
use crate::defaults::{MIN_STREAM_BATCH_SIZE, MAX_STREAM_BATCH_SIZE};
```

- [ ] **Step 9: Make `Operations` cloneable**

The `insert_many_bidi` handler needs to move `operations` into the async stream. Verify `Operations` derives or implements `Clone`. If not, add `#[derive(Clone)]` to `Operations` in `src/operations/mod.rs`.

- [ ] **Step 10: Build and test**

Run: `cargo build 2>&1 | grep "warning:"` — expect no output.
Run: `cargo test --lib` — expect all tests pass.

- [ ] **Step 11: Commit**

```bash
git add src/ proto/
git commit -m "feat(grpc): implement streaming RPCs (FindStream, AggregateStream, InsertManyStream, InsertManyBidi)"
```

---

### Task 6: Streaming Integration Tests

**Files:**
- Create: `tests/integration/streaming_test.rs`

- [ ] **Step 1: Write streaming integration tests**

Create `tests/integration/streaming_test.rs`:

```rust
use bson::doc;
use futures::StreamExt;
use uuid::Uuid;

use mongocore::grpc::proto::mongo_core_client::MongoCoreClient;
use mongocore::grpc::proto::{
    AggregateStreamRequest, Document, DocumentBatch, Filter, FindStreamRequest,
    InsertBatch, InsertManyRequest, InsertRequest, Pipeline,
};
use mongocore::grpc::server::GrpcServerConfig;
use mongocore::grpc::start_grpc_server;

#[path = "../harness/mod.rs"]
mod harness;

const TEST_DB: &str = harness::TEST_DB;

fn unique_collection() -> String {
    format!("test_stream_{}", Uuid::new_v4().to_string().replace('-', ""))
}

fn encode_doc(doc: &bson::Document) -> Vec<u8> {
    let mut buf = Vec::new();
    doc.to_writer(&mut buf).unwrap();
    buf
}

fn make_doc(doc: &bson::Document) -> Document {
    Document { data: encode_doc(doc) }
}

async fn start_test_server() -> MongoCoreClient<tonic::transport::Channel> {
    let pool = harness::get_test_pool().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let _handle = start_grpc_server(
        pool,
        GrpcServerConfig {
            port,
            socket_path: None,
            socket_permissions: 0o600,
            max_message_size: 64 * 1024 * 1024,
        },
        None,
        None,
        None,
        None,
    );

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    MongoCoreClient::connect(format!("http://127.0.0.1:{}", port))
        .await
        .expect("Failed to connect to test server")
}

#[tokio::test]
async fn test_find_stream_basic() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Insert 250 documents
    let docs: Vec<Document> = (0..250)
        .map(|i| make_doc(&doc! { "idx": i, "data": format!("item_{}", i) }))
        .collect();

    client
        .insert_many(InsertManyRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            documents: docs,
            transaction_id: None,
        })
        .await
        .unwrap();

    // Stream with batch_size=100
    let response = client
        .find_stream(FindStreamRequest {
            database: TEST_DB.to_string(),
            collection: coll,
            filter: Some(Filter { data: encode_doc(&doc! {}) }),
            options: None,
            transaction_id: None,
            batch_size: 100,
        })
        .await
        .unwrap();

    let mut stream = response.into_inner();
    let mut total_docs = 0;
    let mut batch_count = 0;

    while let Some(batch) = stream.next().await {
        let batch = batch.unwrap();
        total_docs += batch.documents.len();
        batch_count += 1;

        if !batch.has_more {
            break;
        }
    }

    assert_eq!(total_docs, 250);
    assert!(batch_count >= 3); // 100 + 100 + 50
}

#[tokio::test]
async fn test_find_stream_empty_result() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    let response = client
        .find_stream(FindStreamRequest {
            database: TEST_DB.to_string(),
            collection: coll,
            filter: Some(Filter { data: encode_doc(&doc! { "nonexistent": true }) }),
            options: None,
            transaction_id: None,
            batch_size: 100,
        })
        .await
        .unwrap();

    let mut stream = response.into_inner();
    let batch = stream.next().await.unwrap().unwrap();
    assert_eq!(batch.documents.len(), 0);
    assert!(!batch.has_more);
}

#[tokio::test]
async fn test_aggregate_stream() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Insert documents with categories
    let docs: Vec<Document> = (0..100)
        .map(|i| make_doc(&doc! { "category": format!("cat_{}", i % 5), "value": i }))
        .collect();

    client
        .insert_many(InsertManyRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            documents: docs,
            transaction_id: None,
        })
        .await
        .unwrap();

    // Aggregate with $group
    let group_stage = doc! { "$group": { "_id": "$category", "total": { "$sum": "$value" } } };
    let pipeline = Pipeline {
        stages: vec![encode_doc(&group_stage)],
    };

    let response = client
        .aggregate_stream(AggregateStreamRequest {
            database: TEST_DB.to_string(),
            collection: coll,
            pipeline: Some(pipeline),
            transaction_id: None,
            batch_size: 10,
        })
        .await
        .unwrap();

    let mut stream = response.into_inner();
    let mut total_docs = 0;

    while let Some(batch) = stream.next().await {
        let batch = batch.unwrap();
        total_docs += batch.documents.len();
        if !batch.has_more {
            break;
        }
    }

    assert_eq!(total_docs, 5); // 5 categories
}

#[tokio::test]
async fn test_insert_many_bidi() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Create 3 batches of 50 docs each
    let batches: Vec<InsertBatch> = (0..3)
        .map(|batch_idx| {
            let docs: Vec<Document> = (0..50)
                .map(|i| make_doc(&doc! { "batch": batch_idx, "idx": i }))
                .collect();
            InsertBatch {
                database: if batch_idx == 0 { TEST_DB.to_string() } else { String::new() },
                collection: if batch_idx == 0 { coll.clone() } else { String::new() },
                documents: docs,
            }
        })
        .collect();

    let inbound = tokio_stream::iter(batches);
    let response = client.insert_many_bidi(inbound).await.unwrap();
    let mut stream = response.into_inner();

    let mut total_inserted = 0;
    while let Some(ack) = stream.next().await {
        let ack = ack.unwrap();
        assert_eq!(ack.inserted_count, 50);
        assert!(ack.errors.is_empty());
        total_inserted += ack.inserted_count;
    }

    assert_eq!(total_inserted, 150);
}
```

- [ ] **Step 2: Verify test compiles**

Run: `cargo test --test integration --no-run`

- [ ] **Step 3: Run streaming tests**

Run: `cargo test --test integration streaming_test -- --nocapture`

Expected: All 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add tests/integration/streaming_test.rs
git commit -m "test(grpc): add streaming RPC integration tests"
```

---

### Task 7: gRPC Compression Support

**Files:**
- Modify: `src/defaults.rs`
- Modify: `src/config.rs`
- Modify: `src/grpc/server.rs`
- Modify: `tests/harness/mod.rs`

- [ ] **Step 1: Add compression config defaults**

In `src/defaults.rs`:

```rust
/// Default gRPC compression algorithm.
pub const DEFAULT_GRPC_COMPRESSION: &str = "none";
```

- [ ] **Step 2: Add compression config fields**

In `src/config.rs`, add to `CliArgs`:

```rust
/// gRPC compression algorithm (none, gzip, zstd)
#[arg(long, env = "MONGOCORE_GRPC_COMPRESSION")]
pub grpc_compression: Option<String>,
```

Add to `FileConfig`:

```rust
pub grpc_compression: Option<String>,
```

Add to `Config`:

```rust
pub grpc_compression: String,
```

Add resolution in `Config::load()`:

```rust
let grpc_compression = cli
    .grpc_compression
    .clone()
    .or(file_config.grpc_compression)
    .unwrap_or_else(|| DEFAULT_GRPC_COMPRESSION.to_string());
```

Add to `Ok(Config { ... })` block.

Update ALL Config struct literals (harness and tests):

```rust
grpc_compression: "none".to_string(),
```

Add to `CliArgs` test literals:

```rust
grpc_compression: None,
```

- [ ] **Step 3: Add compression field to `GrpcServerConfig`**

In `src/grpc/server.rs`, update `GrpcServerConfig`:

```rust
pub struct GrpcServerConfig {
    pub port: u16,
    pub socket_path: Option<String>,
    pub socket_permissions: u32,
    pub max_message_size: usize,
    pub compression: String,
}
```

- [ ] **Step 4: Apply compression to server**

In `src/grpc/server.rs`, apply compression based on config. Add after the `grpc_service` construction:

```rust
let grpc_service = {
    let svc = MongoCoreServer::new(service)
        .max_decoding_message_size(config.max_message_size)
        .max_encoding_message_size(config.max_message_size);
    match config.compression.as_str() {
        "gzip" => svc
            .send_compressed(tonic::codec::CompressionEncoding::Gzip)
            .accept_compressed(tonic::codec::CompressionEncoding::Gzip),
        "zstd" => svc
            .send_compressed(tonic::codec::CompressionEncoding::Zstd)
            .accept_compressed(tonic::codec::CompressionEncoding::Zstd),
        _ => svc
            .accept_compressed(tonic::codec::CompressionEncoding::Gzip)
            .accept_compressed(tonic::codec::CompressionEncoding::Zstd),
    }
};
```

Note: Even when compression is "none", we accept compressed requests (clients can opt-in). We only disable *sending* compressed responses unless configured.

- [ ] **Step 5: Update `main.rs`**

Add `compression` to the `GrpcServerConfig`:

```rust
let grpc_handle = start_grpc_server(
    pool.clone(),
    GrpcServerConfig {
        port: config.grpc_port,
        socket_path: config.socket_path.clone(),
        socket_permissions: config.socket_permissions,
        max_message_size: config.grpc_max_message_size,
        compression: config.grpc_compression.clone(),
    },
    voyage_api_key.as_deref(),
    analytics.clone(),
    ingestion_engine.clone(),
    directory_watcher.clone(),
);
```

- [ ] **Step 6: Update all integration test `GrpcServerConfig` usages**

Add `compression: "none".to_string(),` to all `GrpcServerConfig` struct literals in tests.

- [ ] **Step 7: Build and test**

Run: `cargo build 2>&1 | grep "warning:"` — expect no output.
Run: `cargo test --lib` — expect all tests pass.
Run: `cargo test --test integration --no-run` — expect compilation success.

- [ ] **Step 8: Commit**

```bash
git add src/ tests/
git commit -m "feat(grpc): add configurable compression support (gzip, zstd)"
```

---

### Task 8: Update MCP Tool Count and Documentation

**Files:**
- Modify: `tests/integration/mcp_test.rs` (update tool count assertion if new streaming ops get MCP tools)
- Modify: `docs/roadmap.md` (mark Tier 1 as in-progress)

- [ ] **Step 1: Check if MCP tools need updating**

Per the spec, streaming RPCs are gRPC-only (MCP clients don't need streaming — they use JSON-RPC). No new MCP tools are needed. Verify the tool count assertion in `tests/integration/mcp_test.rs` is unchanged.

- [ ] **Step 2: Update roadmap**

In `docs/roadmap.md`, change the Performance Tier 1 entry to indicate it's complete:

```markdown
| Performance Tier 1 | gRPC over Unix Domain Sockets + streaming bulk responses + raised message limits | **Complete** |
```

- [ ] **Step 3: Run full test suite**

Run: `cargo build 2>&1 | grep "warning:"` — must be clean.
Run: `cargo test --lib` — all pass.
Run: `cargo test --test integration --no-run` — compiles.

- [ ] **Step 4: Commit**

```bash
git add docs/ tests/
git commit -m "docs: mark Performance Tier 1 as complete, update roadmap"
```

---

### Task 9: Final Validation and Benchmark

**Files:**
- No new files — validation only

- [ ] **Step 1: Full build with zero warnings**

Run: `cargo build 2>&1 | grep "warning:"` — expect NO output.

- [ ] **Step 2: Unit tests**

Run: `cargo test --lib` — expect all pass.

- [ ] **Step 3: Integration tests compile**

Run: `cargo test --test integration --no-run` — expect success.

- [ ] **Step 4: Integration tests pass (requires Docker MongoDB)**

Run: `just docker-up && cargo test --test integration -- --nocapture`

Verify all existing tests still pass (no regressions) and new UDS + streaming tests pass.

- [ ] **Step 5: Manual UDS verification**

Start the server with UDS:

```bash
cargo run -- --socket-path /tmp/mongocore.sock --connection-uri mongodb://localhost:27017
```

Verify the socket file exists with correct permissions:

```bash
ls -la /tmp/mongocore.sock
# Should show srw------- (socket, owner-only)
```

- [ ] **Step 6: Record results**

Add a brief entry to `docs/design/development-log.md` summarizing:
- What was implemented (UDS, streaming, message limits, compression)
- Benchmark comparison if available
- Any deviations from the spec
