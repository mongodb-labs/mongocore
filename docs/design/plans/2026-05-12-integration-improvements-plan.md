# Integration Improvements — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add driver metadata to MongoDB handshakes, enable URL-based ingestion via Polars cloud support, and add OpenTelemetry tracing with both MongoCore-level and driver-level spans.

**Architecture:** Three independent subsystems. Driver metadata sets `DriverInfo` at connection time and appends per-interface info on first request. URL ingestion enables Polars `cloud` feature — no code branching needed. OpenTelemetry adds an optional feature-gated tracing pipeline that exports spans via OTLP when configured.

**Tech Stack:** Rust, mongodb driver v3 (`DriverInfo`, `append_metadata`, `tracing-unstable`), Polars (`cloud` feature), opentelemetry/tracing-opentelemetry/opentelemetry-otlp crates.

---

## File Structure

### Subsystem 1: Driver Metadata

| Action | Path | Responsibility |
|--------|------|----------------|
| Modify | `src/connection/pool.rs` | Set `DriverInfo` on startup, expose `append_metadata` wrapper |
| Modify | `src/grpc/service.rs` | Extract `x-client-language` header, call append on first request |
| Modify | `src/mcp/handler.rs` | Call append with "mcp" on first request |
| Modify | `clients/python/src/mongocore/client.py` | Add `x-client-language: python` metadata |
| Modify | `clients/typescript/src/client.ts` | Add `x-client-language: typescript` metadata |
| Modify | `clients/go/mongocore/client.go` | Add `x-client-language: go` metadata |
| Modify | `clients/java/src/main/java/com/mongocore/MongoClient.java` | Add `x-client-language: java` metadata |

### Subsystem 2: URL Ingestion

| Action | Path | Responsibility |
|--------|------|----------------|
| Modify | `Cargo.toml` | Add `"cloud"` to polars features |
| Modify | `src/ingestion/reader.rs` | Accept `&str` source, handle URL format detection |

### Subsystem 3: OpenTelemetry

| Action | Path | Responsibility |
|--------|------|----------------|
| Modify | `Cargo.toml` | Add otel deps (feature-gated), enable `tracing-unstable` on mongodb |
| Modify | `src/config.rs` | Add OTel config fields |
| Modify | `src/defaults.rs` | Add OTel defaults |
| Modify | `src/main.rs` | Initialize OTel tracing pipeline when enabled |
| Modify | `src/grpc/service.rs` | Add `#[tracing::instrument]` to RPC handlers |
| Modify | `src/mcp/handler.rs` | Add `#[tracing::instrument]` to request handler |
| Modify | `src/ingestion/engine.rs` | Add `#[tracing::instrument]` to ingest method |
| Modify | `config.test.toml.example` | Add OTel config (commented out) |
| Create | `docs/opentelemetry.md` | OTel documentation |
| Modify | `README.md` | Add OTel to documentation table |

---

## Subsystem 1: Driver Metadata

### Task 1.1: Set DriverInfo at Connection Time

**Files:**
- Modify: `src/connection/pool.rs`

- [ ] **Step 1: Write failing test for driver_info being set**

Add to the `tests` module in `src/connection/pool.rs`:

```rust
#[tokio::test]
async fn test_client_options_driver_info_set() {
    let config = Config {
        connection_uri: "mongodb://localhost:27017".to_string(),
        grpc_port: 50051,
        mcp_port: 3000,
        llm_provider: None,
        llm_api_key_env: None,
        voyage_api_key_env: None,
        compiled_cache_sync: true,
        log_level: "info".to_string(),
        multi_tenant_enabled: false,
        tenants: vec![],
        analytics_enabled: true,
        analytics_buffer_size: 10000,
        analytics_flush_interval_secs: 300,
        ingestion: crate::config::ResolvedIngestionConfig::default(),
    };

    let options = ConnectionPool::build_client_options(&config).await.unwrap();

    let driver_info = options.driver_info.expect("driver_info should be set");
    assert_eq!(driver_info.name, "mongocore");
    assert_eq!(driver_info.version.as_deref(), Some(env!("CARGO_PKG_VERSION")));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib test_client_options_driver_info_set`
Expected: FAIL — `driver_info` is `None`

- [ ] **Step 3: Implement DriverInfo in build_client_options**

In `src/connection/pool.rs`, add the import:

```rust
use mongodb::options::{ClientOptions, DriverInfo, SelectionCriteria};
```

Then add at the end of `build_client_options()`, before `Ok(options)`:

```rust
options.driver_info = Some(
    DriverInfo::builder()
        .name("mongocore")
        .version(env!("CARGO_PKG_VERSION").to_string())
        .build(),
);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib test_client_options_driver_info_set`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/connection/pool.rs
git commit -m "feat(connection): set mongocore DriverInfo in MongoDB handshake"
```

---

### Task 1.2: Expose append_metadata Wrapper on ConnectionPool

**Files:**
- Modify: `src/connection/pool.rs`

- [ ] **Step 1: Add append_interface_metadata method**

Add to the `impl ConnectionPool` block in `src/connection/pool.rs`:

```rust
use std::collections::HashSet;
use std::sync::Mutex;
```

Change the `ConnectionPool` struct to include a tracker:

```rust
pub struct ConnectionPool {
    client: Client,
    capabilities: Capabilities,
    host: String,
    appended_interfaces: Mutex<HashSet<String>>,
}
```

Update the constructor in `connect()` where `Self` is built:

```rust
let pool = Self {
    client,
    capabilities,
    host,
    appended_interfaces: Mutex::new(HashSet::new()),
};
```

Add the method:

```rust
/// Append interface metadata to the MongoDB handshake (e.g., "python", "mcp").
/// Each interface is appended at most once.
pub fn append_interface_metadata(&self, interface: &str) {
    let mut appended = self.appended_interfaces.lock().unwrap();
    if appended.contains(interface) {
        return;
    }
    let driver_info = DriverInfo::builder()
        .name(interface.to_string())
        .build();
    if self.client.append_metadata(driver_info).is_ok() {
        appended.insert(interface.to_string());
        tracing::debug!("Appended driver metadata for interface: {}", interface);
    }
}
```

- [ ] **Step 2: Fix the Clone derive**

`Mutex<HashSet<String>>` doesn't derive `Clone`. Remove `#[derive(Clone)]` from `ConnectionPool` and implement it manually:

```rust
impl Clone for ConnectionPool {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            capabilities: self.capabilities.clone(),
            host: self.host.clone(),
            appended_interfaces: Mutex::new(HashSet::new()),
        }
    }
}
```

Change the derive to `#[derive(Debug)]` only.

- [ ] **Step 3: Build to verify compilation**

Run: `cargo build`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/connection/pool.rs
git commit -m "feat(connection): add append_interface_metadata for per-caller handshake info"
```

---

### Task 1.3: Append "mcp" on First MCP Request

**Files:**
- Modify: `src/mcp/handler.rs`

- [ ] **Step 1: Add pool field access and append call**

The `McpHandler` already has a `pool: ConnectionPool` field. Add an `std::sync::Once` or a simple `AtomicBool` to track whether we've appended:

Add to imports in `src/mcp/handler.rs`:

```rust
use std::sync::atomic::{AtomicBool, Ordering};
```

Add field to `McpHandler`:

```rust
pub struct McpHandler {
    operations: Operations,
    pool: ConnectionPool,
    safety: SafetyConfig,
    analytics: Option<Arc<AnalyticsCollector>>,
    ingestion: Option<Arc<IngestionEngine>>,
    watcher: Option<Arc<DirectoryWatcher>>,
    mcp_metadata_appended: AtomicBool,
}
```

Initialize in `McpHandler::new()`:

```rust
mcp_metadata_appended: AtomicBool::new(false),
```

Add to the top of `handle_request()`:

```rust
if !self.mcp_metadata_appended.load(Ordering::Relaxed) {
    self.pool.append_interface_metadata("mcp");
    self.mcp_metadata_appended.store(true, Ordering::Relaxed);
}
```

- [ ] **Step 2: Build to verify compilation**

Run: `cargo build`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/mcp/handler.rs
git commit -m "feat(mcp): append 'mcp' to driver metadata on first request"
```

---

### Task 1.4: Append Client Language from gRPC Metadata

**Files:**
- Modify: `src/grpc/service.rs`

- [ ] **Step 1: Add language tracking to MongoCoreService**

Add to imports:

```rust
use std::sync::Mutex;
use std::collections::HashSet;
```

Add field to `MongoCoreService`:

```rust
pub struct MongoCoreService {
    operations: Operations,
    pool: ConnectionPool,
    transactions: DashMap<String, Transaction>,
    search_engine: SearchEngine,
    analytics: Option<Arc<AnalyticsCollector>>,
    tenant_registry: Option<Arc<TenantRegistry>>,
    quota_manager: Option<Arc<QuotaManager>>,
    ingestion_engine: Option<Arc<crate::ingestion::IngestionEngine>>,
    directory_watcher: Option<Arc<crate::ingestion::DirectoryWatcher>>,
    client: Option<mongodb::Client>,
    appended_languages: Mutex<HashSet<String>>,
}
```

Initialize in `MongoCoreService::new()`:

```rust
appended_languages: Mutex::new(HashSet::new()),
```

Add helper method:

```rust
fn append_client_language(&self, request_metadata: &tonic::metadata::MetadataMap) {
    if let Some(lang) = request_metadata.get("x-client-language") {
        if let Ok(lang_str) = lang.to_str() {
            let mut seen = self.appended_languages.lock().unwrap();
            if !seen.contains(lang_str) {
                self.pool.append_interface_metadata(lang_str);
                seen.insert(lang_str.to_string());
            }
        }
    }
}
```

- [ ] **Step 2: Call from a representative RPC handler (find)**

Add at the top of the `find` handler:

```rust
self.append_client_language(request.metadata());
```

Apply the same one-liner to all other RPC handlers: `find_one`, `insert`, `insert_many`, `update`, `update_many`, `delete`, `delete_many`, `find_and_modify`, `aggregate`, `search`, `run_command`, `list_databases`, `list_collections`, `create_collection`, `create_index`, `begin_transaction`, `commit_transaction`, `abort_transaction`, `watch`, and all ingestion RPCs.

- [ ] **Step 3: Build to verify compilation**

Run: `cargo build`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/grpc/service.rs
git commit -m "feat(grpc): append client language to driver metadata from x-client-language header"
```

---

### Task 1.5: Add x-client-language Header to Client Libraries

**Files:**
- Modify: `clients/python/src/mongocore/client.py`
- Modify: `clients/typescript/src/client.ts`
- Modify: `clients/go/mongocore/client.go`
- Modify: `clients/java/src/main/java/com/mongocore/MongoClient.java`

- [ ] **Step 1: Python — add metadata to all gRPC calls**

In the Python client, find where the gRPC stub calls are made and add metadata. Typically there's a wrapper or the stub is called directly. Add to each stub call:

```python
_METADATA = [("x-client-language", "python")]
```

Then pass `metadata=_METADATA` to each stub call. For example:

```python
response = await self._stub.Find(request, metadata=_METADATA)
```

- [ ] **Step 2: TypeScript — add metadata**

In the TypeScript client, add a metadata constant and pass it with calls:

```typescript
const CLIENT_METADATA = new grpc.Metadata();
CLIENT_METADATA.set('x-client-language', 'typescript');
```

Pass to each call: `this.client.find(request, CLIENT_METADATA, callback)`

- [ ] **Step 3: Go — add metadata**

In the Go client, add metadata to the context:

```go
import "google.golang.org/grpc/metadata"

func clientContext(ctx context.Context) context.Context {
    return metadata.AppendToOutgoingContext(ctx, "x-client-language", "go")
}
```

Wrap each call's context with `clientContext(ctx)`.

- [ ] **Step 4: Java — add metadata interceptor**

In the Java client, add a `ClientInterceptor`:

```java
private static final Metadata.Key<String> CLIENT_LANG_KEY =
    Metadata.Key.of("x-client-language", Metadata.ASCII_STRING_MARSHALLER);

private final ClientInterceptor langInterceptor = new ClientInterceptor() {
    @Override
    public <ReqT, RespT> ClientCall<ReqT, RespT> interceptCall(
            MethodDescriptor<ReqT, RespT> method, CallOptions options, Channel next) {
        return new ForwardingClientCall.SimpleForwardingClientCall<>(next.newCall(method, options)) {
            @Override
            public void start(Listener<RespT> listener, Metadata headers) {
                headers.put(CLIENT_LANG_KEY, "java");
                super.start(listener, headers);
            }
        };
    }
};
```

Apply the interceptor when building the channel.

- [ ] **Step 5: Commit**

```bash
git add clients/
git commit -m "feat(clients): add x-client-language gRPC metadata header to all client libraries"
```

---

## Subsystem 2: URL Ingestion

### Task 2.1: Enable Polars Cloud Feature

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add "cloud" to polars features**

Change the polars dependency in `Cargo.toml` from:

```toml
polars = { version = "0.46", features = ["lazy", "csv", "json", "parquet", "dtype-struct"] }
```

To:

```toml
polars = { version = "0.46", features = ["lazy", "csv", "json", "parquet", "dtype-struct", "cloud"] }
```

- [ ] **Step 2: Build to verify it compiles with cloud feature**

Run: `cargo build`
Expected: PASS (may take longer due to new deps like object_store)

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat(ingestion): enable Polars cloud feature for URL-based ingestion"
```

---

### Task 2.2: Update Reader to Accept String Sources

**Files:**
- Modify: `src/ingestion/reader.rs`

- [ ] **Step 1: Write test for URL format detection**

Add to the `tests` module in `src/ingestion/reader.rs`:

```rust
#[test]
fn test_detect_format_from_url() {
    let path = Path::new("https://example.com/data/restaurants.csv");
    assert_eq!(detect_format(path).unwrap(), FileFormat::Csv);
}

#[test]
fn test_detect_format_from_s3_url() {
    let path = Path::new("s3://my-bucket/exports/data.parquet");
    assert_eq!(detect_format(path).unwrap(), FileFormat::Parquet);
}

#[test]
fn test_detect_format_from_url_with_query_params() {
    // Path::new strips query params from extension detection
    let path = Path::new("https://example.com/data.json");
    assert_eq!(detect_format(path).unwrap(), FileFormat::Json);
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test --lib test_detect_format_from_url`
Expected: PASS — `Path::new` on URLs preserves the extension, and `detect_format` already uses `path.extension()` which handles this.

- [ ] **Step 3: Add a convenience function that accepts &str**

Add to `src/ingestion/reader.rs`:

```rust
/// Create a LazyFrame from a source string (local path or URL).
/// Polars handles local files and cloud URLs (http://, s3://, gs://, az://) identically.
pub fn read_lazy_from_source(
    source: &str,
    format: FileFormat,
    csv_options: &CsvOptions,
) -> Result<LazyFrame, MongoCoreError> {
    read_lazy(Path::new(source), format, csv_options)
}

/// Count rows from a source string (local path or URL).
pub fn count_rows_from_source(
    source: &str,
    format: FileFormat,
    csv_options: &CsvOptions,
) -> Result<u64, MongoCoreError> {
    count_rows(Path::new(source), format, csv_options)
}
```

- [ ] **Step 4: Write test for read_lazy_from_source with local file**

```rust
#[test]
fn test_read_lazy_from_source_local_file() {
    let mut file = NamedTempFile::with_suffix(".csv").unwrap();
    writeln!(file, "name,value").unwrap();
    writeln!(file, "test,42").unwrap();
    file.flush().unwrap();

    let opts = CsvOptions::default();
    let lf = read_lazy_from_source(
        file.path().to_str().unwrap(),
        FileFormat::Csv,
        &opts,
    ).unwrap();
    let df = lf.collect().unwrap();
    assert_eq!(df.height(), 1);
}
```

- [ ] **Step 5: Run all reader tests**

Run: `cargo test --lib ingestion::reader`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/ingestion/reader.rs
git commit -m "feat(ingestion): add read_lazy_from_source for URL and local path ingestion"
```

---

### Task 2.3: Wire Source String Through Ingestion Engine

**Files:**
- Modify: `src/ingestion/engine.rs`

- [ ] **Step 1: Update engine to use read_lazy_from_source**

Find where the ingestion engine calls `read_lazy()` with a `Path` and update it to call `read_lazy_from_source()` with the source string instead. The `IngestOptions` or `IngestJob` likely has a `source` or `path` field — change it to accept a `String` and pass it through.

Look for the pattern:
```rust
let lf = reader::read_lazy(&path, format, &csv_options)?;
```

Replace with:
```rust
let lf = reader::read_lazy_from_source(&source, format, &csv_options)?;
```

- [ ] **Step 2: Build to verify compilation**

Run: `cargo build`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/ingestion/engine.rs
git commit -m "feat(ingestion): use read_lazy_from_source in engine for URL support"
```

---

## Subsystem 3: OpenTelemetry

### Task 3.1: Add OTel Dependencies (Feature-Gated)

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add dependencies and feature flag**

Add to `[dependencies]` in `Cargo.toml`:

```toml
opentelemetry = { version = "0.28", optional = true }
opentelemetry_sdk = { version = "0.28", features = ["rt-tokio"], optional = true }
opentelemetry-otlp = { version = "0.28", features = ["grpc-tonic"], optional = true }
tracing-opentelemetry = { version = "0.29", optional = true }
```

Add a `[features]` section (or append to existing):

```toml
[features]
default = []
otel = ["opentelemetry", "opentelemetry_sdk", "opentelemetry-otlp", "tracing-opentelemetry"]
```

Enable mongodb driver tracing — change:
```toml
mongodb = "3"
```
To:
```toml
mongodb = { version = "3", features = ["tracing-unstable"] }
```

- [ ] **Step 2: Build without otel feature (default)**

Run: `cargo build`
Expected: PASS

- [ ] **Step 3: Build with otel feature**

Run: `cargo build --features otel`
Expected: PASS (or identify version conflicts to resolve)

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add OpenTelemetry dependencies (feature-gated) and enable driver tracing"
```

---

### Task 3.2: Add OTel Configuration

**Files:**
- Modify: `src/config.rs`
- Modify: `src/defaults.rs`

- [ ] **Step 1: Add defaults**

Add to `src/defaults.rs`:

```rust
/// Default OpenTelemetry OTLP endpoint.
pub const DEFAULT_OTEL_ENDPOINT: &str = "http://localhost:4317";

/// Default OpenTelemetry service name.
pub const DEFAULT_OTEL_SERVICE_NAME: &str = "mongocore";
```

- [ ] **Step 2: Add OTel CLI args**

Add to `CliArgs` in `src/config.rs`:

```rust
/// Enable OpenTelemetry tracing export
#[arg(long, env = "MONGOCORE_OTEL_ENABLED")]
pub otel_enabled: Option<bool>,

/// OpenTelemetry OTLP endpoint (gRPC)
#[arg(long, env = "MONGOCORE_OTEL_ENDPOINT")]
pub otel_endpoint: Option<String>,

/// OpenTelemetry service name
#[arg(long, env = "MONGOCORE_OTEL_SERVICE_NAME")]
pub otel_service_name: Option<String>,
```

- [ ] **Step 3: Add to FileConfig**

Add to `FileConfig`:

```rust
pub otel_enabled: Option<bool>,
pub otel_endpoint: Option<String>,
pub otel_service_name: Option<String>,
```

- [ ] **Step 4: Add to Config struct**

Add to `Config`:

```rust
pub otel_enabled: bool,
pub otel_endpoint: String,
pub otel_service_name: String,
```

- [ ] **Step 5: Add resolution logic in Config::load()**

Add before the `Ok(Config { ... })` block:

```rust
let otel_enabled = cli
    .otel_enabled
    .or(file_config.otel_enabled)
    .unwrap_or(false);
let otel_endpoint = cli
    .otel_endpoint
    .clone()
    .or(file_config.otel_endpoint)
    .unwrap_or_else(|| DEFAULT_OTEL_ENDPOINT.to_string());
let otel_service_name = cli
    .otel_service_name
    .clone()
    .or(file_config.otel_service_name)
    .unwrap_or_else(|| DEFAULT_OTEL_SERVICE_NAME.to_string());
```

Add the fields to the `Config` struct literal in the `Ok(...)` return.

- [ ] **Step 6: Update test structs**

Update `Config` literals in tests to include the new fields:

```rust
otel_enabled: false,
otel_endpoint: "http://localhost:4317".to_string(),
otel_service_name: "mongocore".to_string(),
```

Also update the `Config` literal in `src/connection/pool.rs` tests.

- [ ] **Step 7: Add import for new default**

Add to the `use crate::defaults` import in `src/config.rs`:

```rust
use crate::defaults::{
    DEFAULT_COMPILED_CACHE_SYNC, DEFAULT_CONNECTION_URI, DEFAULT_GRPC_PORT, DEFAULT_LOG_LEVEL,
    DEFAULT_MCP_PORT, DEFAULT_OTEL_ENDPOINT, DEFAULT_OTEL_SERVICE_NAME,
};
```

- [ ] **Step 8: Run tests**

Run: `cargo test --lib config`
Expected: PASS

- [ ] **Step 9: Commit**

```bash
git add src/config.rs src/defaults.rs
git commit -m "feat(config): add OpenTelemetry configuration fields"
```

---

### Task 3.3: Initialize OTel Tracing Pipeline

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Conditionally initialize OTel tracing**

Replace the tracing initialization block in `main()`:

```rust
// Initialize tracing/logging
tracing_subscriber::fmt()
    .with_env_filter(
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log_level)),
    )
    .init();
```

With:

```rust
// Initialize tracing/logging
let filter = EnvFilter::try_from_default_env()
    .unwrap_or_else(|_| EnvFilter::new(&config.log_level));

#[cfg(feature = "otel")]
{
    if config.otel_enabled {
        use opentelemetry::KeyValue;
        use opentelemetry_sdk::Resource;
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;

        let tracer = opentelemetry_otlp::new_pipeline()
            .tracing()
            .with_exporter(
                opentelemetry_otlp::new_exporter()
                    .tonic()
                    .with_endpoint(&config.otel_endpoint),
            )
            .with_trace_config(
                opentelemetry_sdk::trace::Config::default().with_resource(
                    Resource::new(vec![KeyValue::new(
                        "service.name",
                        config.otel_service_name.clone(),
                    )]),
                ),
            )
            .install_batch(opentelemetry_sdk::runtime::Tokio)
            .expect("Failed to initialize OpenTelemetry tracer");

        let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
        let fmt_layer = tracing_subscriber::fmt::layer();

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt_layer)
            .with(otel_layer)
            .init();

        info!("OpenTelemetry tracing enabled, exporting to {}", config.otel_endpoint);
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }
}

#[cfg(not(feature = "otel"))]
{
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
```

- [ ] **Step 2: Add graceful shutdown**

Before the `tokio::select!` block, or in a shutdown handler, add:

```rust
// At the end of main, after tokio::select! completes:
#[cfg(feature = "otel")]
{
    if config.otel_enabled {
        opentelemetry::global::shutdown_tracer_provider();
    }
}
```

- [ ] **Step 3: Add required imports**

Add to the top of `main.rs`:

```rust
use tracing_subscriber::EnvFilter;
```

Remove the existing `use tracing_subscriber::EnvFilter;` if it's already imported differently.

- [ ] **Step 4: Build with otel feature**

Run: `cargo build --features otel`
Expected: PASS

- [ ] **Step 5: Build without otel feature**

Run: `cargo build`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/main.rs
git commit -m "feat: initialize OpenTelemetry tracing pipeline when otel feature enabled"
```

---

### Task 3.4: Add Tracing Instrumentation to gRPC Handlers

**Files:**
- Modify: `src/grpc/service.rs`

- [ ] **Step 1: Add #[tracing::instrument] to RPC handlers**

Add `#[tracing::instrument(skip(self, request))]` above each async RPC handler method. For example:

```rust
#[tracing::instrument(skip(self, request), fields(db, collection))]
async fn find(
    &self,
    request: Request<proto::FindRequest>,
) -> Result<Response<proto::FindResponse>, Status> {
    let req = request.into_inner();
    tracing::Span::current().record("db", &req.database.as_str());
    tracing::Span::current().record("collection", &req.collection.as_str());
    // ... existing implementation
}
```

Apply to all RPC handlers. For handlers without db/collection (like `list_databases`), use simpler instrumentation:

```rust
#[tracing::instrument(skip(self, request))]
async fn list_databases(
    &self,
    request: Request<proto::ListDatabasesRequest>,
) -> Result<Response<proto::ListDatabasesResponse>, Status> {
    // ... existing implementation
}
```

- [ ] **Step 2: Build**

Run: `cargo build`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/grpc/service.rs
git commit -m "feat(grpc): add tracing instrumentation to all RPC handlers"
```

---

### Task 3.5: Add Tracing Instrumentation to MCP and Ingestion

**Files:**
- Modify: `src/mcp/handler.rs`
- Modify: `src/ingestion/engine.rs`

- [ ] **Step 1: Instrument MCP handler**

Add to `handle_request` in `src/mcp/handler.rs`:

```rust
#[tracing::instrument(skip(self), fields(method = %request.method))]
pub async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
    // ... existing implementation
}
```

Add to `handle_tools_call`:

```rust
#[tracing::instrument(skip(self, params), fields(tool))]
async fn handle_tools_call(&self, id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
    // At the start, after extracting tool name:
    tracing::Span::current().record("tool", &tool_name.as_str());
    // ... existing implementation
}
```

- [ ] **Step 2: Instrument ingestion engine**

Add to the main `ingest` method in `src/ingestion/engine.rs`:

```rust
#[tracing::instrument(skip(self, options), fields(source = %options.source, database = %options.database, collection = %options.collection))]
pub async fn ingest(&self, options: IngestOptions) -> Result<IngestResult, MongoCoreError> {
    // ... existing implementation
}
```

- [ ] **Step 3: Build**

Run: `cargo build`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/mcp/handler.rs src/ingestion/engine.rs
git commit -m "feat: add tracing instrumentation to MCP handler and ingestion engine"
```

---

### Task 3.6: Update Config Example and Documentation

**Files:**
- Modify: `config.test.toml.example`
- Create: `docs/opentelemetry.md`
- Modify: `README.md`

- [ ] **Step 1: Update config.test.toml.example**

Add to the end of `config.test.toml.example`:

```toml

# OpenTelemetry tracing (requires --features otel)
# otel_enabled = true
# otel_endpoint = "http://localhost:4317"
# otel_service_name = "mongocore"
```

- [ ] **Step 2: Create docs/opentelemetry.md**

```markdown
# OpenTelemetry Support

MongoCore supports distributed tracing via OpenTelemetry, exporting spans to any OTLP-compatible collector (Jaeger, Zipkin, Grafana Tempo, Datadog, etc.).

## Enabling OpenTelemetry

OpenTelemetry is an optional feature. Build with it enabled:

```bash
cargo build --release --features otel
```

Then configure in your `config.toml`:

```toml
otel_enabled = true
otel_endpoint = "http://localhost:4317"   # OTLP gRPC endpoint
otel_service_name = "mongocore"           # Service name in traces
```

Or via environment variables:

```bash
export MONGOCORE_OTEL_ENABLED=true
export MONGOCORE_OTEL_ENDPOINT=http://localhost:4317
export MONGOCORE_OTEL_SERVICE_NAME=mongocore
```

## What Gets Traced

### MongoCore Spans

- **gRPC handlers** — Each RPC call (find, insert, aggregate, etc.) with database/collection attributes
- **MCP handler** — Each JSON-RPC request with method and tool name
- **Ingestion jobs** — Each ingest operation with source, database, and collection

### MongoDB Driver Spans

With `tracing-unstable` enabled on the MongoDB driver, you also get:
- Connection checkout/checkin
- Command execution (command name, database, duration)
- Server selection

These appear as child spans under MongoCore's operation spans, giving full end-to-end visibility from request to database.

## Example Setup with Jaeger

```bash
# Start Jaeger with OTLP receiver
docker run -d --name jaeger \
  -p 4317:4317 \
  -p 16686:16686 \
  jaegertracing/all-in-one:latest

# Start MongoCore with OTel
cargo run --features otel -- --config config.toml

# View traces at http://localhost:16686
```

## Zero Overhead When Disabled

When `otel_enabled = false` (the default) or when built without the `otel` feature, no tracing pipeline is initialized and no spans are exported. The `#[tracing::instrument]` annotations remain but produce no overhead without a subscriber consuming them.
```

- [ ] **Step 3: Update README documentation table**

Add a row to the documentation table in `README.md`:

```markdown
| [OpenTelemetry](./docs/opentelemetry.md) | Distributed tracing setup and configuration |
```

Add after the "Multi-Tenant" row.

- [ ] **Step 4: Commit**

```bash
git add config.test.toml.example docs/opentelemetry.md README.md
git commit -m "docs: add OpenTelemetry documentation and update config example"
```

---

## Task 4: Regression Testing

### Task 4.1: Full Test Suite Verification

**Files:**
- None (verification only)

- [ ] **Step 1: Run all unit tests**

Run: `cargo test --lib`
Expected: ALL PASS — no regressions from driver metadata changes, polars cloud feature, or tracing instrumentation.

- [ ] **Step 2: Run all unit tests with otel feature**

Run: `cargo test --lib --features otel`
Expected: ALL PASS — OTel feature doesn't break any existing tests.

- [ ] **Step 3: Build all client libraries**

Verify each client still compiles/installs after adding the `x-client-language` metadata:

```bash
cd clients/python && pip install -e . 2>&1 | tail -1
cd clients/typescript && npm install && npx tsc --noEmit 2>&1 | tail -1
cd clients/go && go build ./... 2>&1 | tail -1
cd clients/java && mvn compile -q 2>&1 | tail -1
```

Expected: All succeed without errors.

- [ ] **Step 4: Run integration tests (requires Docker MongoDB)**

```bash
just docker-up
cargo test --test integration
```

Expected: ALL PASS — driver metadata, polars cloud, and tracing instrument annotations don't break existing integration tests (CRUD, search, transactions, ingestion, MCP, etc.)

- [ ] **Step 5: Run integration tests with otel feature**

```bash
cargo test --test integration --features otel
```

Expected: ALL PASS — OTel initialization (with `otel_enabled = false` default) doesn't interfere with integration test behavior.

- [ ] **Step 6: Verify ingestion still works with local files**

Run the existing ingestion integration tests specifically:

```bash
cargo test --test integration ingestion -- --nocapture
```

Expected: ALL PASS — adding `"cloud"` to polars features and the `read_lazy_from_source` function doesn't regress local file ingestion.

- [ ] **Step 7: Commit regression test run confirmation**

If all tests pass, no commit needed. If any test required a fix, commit the fix:

```bash
git add -A
git commit -m "fix: resolve regression from integration improvements"
```

---

## Implementation Order & Dependencies

```
Phase 1 (Independent, parallel):
  Task 1.1–1.2: Driver metadata setup (connection pool)
  Task 2.1: Enable Polars cloud feature
  Task 3.1: Add OTel dependencies

Phase 2 (Depends on Phase 1):
  Task 1.3: MCP metadata append
  Task 1.4: gRPC language metadata
  Task 2.2–2.3: Reader update and engine wiring
  Task 3.2: OTel config fields

Phase 3 (Depends on Phase 2):
  Task 1.5: Client library headers
  Task 3.3: OTel initialization in main.rs
  Task 3.4–3.5: Instrumentation

Phase 4:
  Task 3.6: Documentation
```

---

## Definition of Done

- [ ] `cargo build` succeeds with `DriverInfo` set in client options
- [ ] `append_interface_metadata("mcp")` called on first MCP request
- [ ] `append_interface_metadata("<language>")` called on first gRPC request per language
- [ ] All 4 client libraries send `x-client-language` gRPC metadata
- [ ] `cargo build` succeeds with `"cloud"` polars feature
- [ ] `read_lazy_from_source("https://...")` works for URL sources
- [ ] `cargo build --features otel` succeeds with OTel pipeline
- [ ] `config.test.toml.example` contains OTel params (commented out)
- [ ] `docs/opentelemetry.md` exists with setup instructions
- [ ] `#[tracing::instrument]` on all gRPC handlers, MCP handler, and ingestion engine
- [ ] All unit tests pass: `cargo test --lib` and `cargo test --lib --features otel`
- [ ] All integration tests pass: `cargo test --test integration` and `cargo test --test integration --features otel`
- [ ] All client libraries compile successfully with `x-client-language` header addition
- [ ] Local file ingestion is not regressed by polars cloud feature
