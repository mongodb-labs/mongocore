# MongoCore: Integration Improvements

## Overview

Three independent improvements that make MongoCore a better citizen in production environments: driver metadata in MongoDB handshakes, URL-based data ingestion via Polars cloud support, and OpenTelemetry tracing for end-to-end observability.

## Motivation

- **Driver metadata:** Ops teams need to see what's connecting to their clusters. MongoCore should identify itself in MongoDB server logs, `$currentOp`, and Atlas monitoring.
- **URL ingestion:** Users shouldn't need to download files locally before ingesting. Polars natively supports HTTP, S3, GCS, and Azure sources — we just need to enable it.
- **OpenTelemetry:** Production deployments need distributed tracing. MongoCore should emit spans for its own operations and surface the underlying MongoDB driver spans.

## Design

### 1. Driver Metadata (Handshake)

#### Startup

Set `DriverInfo` on `ClientOptions` in `ConnectionPool::build_client_options()`:

```rust
options.driver_info = Some(DriverInfo::builder()
    .name("mongocore")
    .version(env!("CARGO_PKG_VERSION"))
    .build());
```

This makes MongoDB server logs show `mongocore/0.1.0` alongside the underlying Rust driver info.

#### Per-Interface Append

When MongoCore first receives a request from a new interface type, call `client.append_metadata()`:

- **MCP requests:** On first MCP request, append `DriverInfo { name: "mcp" }`
- **gRPC requests:** Client libraries send an `x-client-language` gRPC metadata header. On first request from each language, append `DriverInfo { name: "<language>" }`

Tracked with a `HashSet<String>` on `ConnectionPool` (or `MongoCoreService`) to ensure each interface is appended at most once.

Result in MongoDB handshake metadata: `mongocore|mcp`, `mongocore|python`, `mongocore|java`, etc.

#### Client Library Changes

Each client library adds the `x-client-language` metadata header to every gRPC call:

- **Python:** `metadata=[("x-client-language", "python")]`
- **TypeScript:** `metadata: { "x-client-language": "typescript" }`
- **Go:** `metadata.Pairs("x-client-language", "go")`
- **Java:** `Metadata.Key.of("x-client-language")` → `"java"`

One-line addition per client in the stub wrapper.

### 2. URL Source for Ingestion

#### Polars Cloud Feature

Add `"cloud"` to Polars features in `Cargo.toml`:

```toml
polars = { version = "0.46", features = ["lazy", "csv", "json", "parquet", "dtype-struct", "cloud"] }
```

This enables Polars to read directly from:
- `http://` / `https://` — public web URLs
- `s3://` — Amazon S3 (credentials via AWS env vars)
- `gs://` — Google Cloud Storage (credentials via GOOGLE_APPLICATION_CREDENTIALS)
- `az://` / `abfss://` — Azure Blob Storage (credentials via Azure env vars)

#### Reader Changes

Update `src/ingestion/reader.rs`:

No branching needed. Polars internally uses `is_cloud_url()` (matching `^(s3a?|gs|gcs|file|abfss?|azure|az|adl|https?|hf)://`) to detect cloud URLs and routes through the `object_store` crate transparently. Local file paths and URLs use the same API.

The existing `read_lazy()` function already passes the source to `LazyCsvReader::new(path)` etc. — it just needs to accept a `&str` source instead of only `&Path`, since URL strings like `https://...` work as paths in Polars:

```rust
pub fn read_lazy(
    source: &str,
    format: FileFormat,
    csv_options: &CsvOptions,
) -> Result<LazyFrame, MongoCoreError> {
    let path = Path::new(source);
    let format = if format == FileFormat::Auto {
        detect_format(path)?
    } else {
        format
    };
    // LazyCsvReader::new(), LazyJsonLineReader::new(), scan_parquet()
    // all handle local paths and cloud URLs identically
    match format {
        FileFormat::Csv => { /* existing code, unchanged */ }
        // ...
    }
}
```

No separate URL handling, no download step, no new functions.

#### No Proto/MCP Changes

The `Ingest` RPC and MCP `ingest` tool already accept a `source` string field. URLs work transparently — users just pass a URL where they previously passed a file path.

#### Cloud Credentials

S3/GCS/Azure credentials are picked up automatically by Polars (via the `object_store` crate) from standard environment variables:
- AWS: `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION`
- GCS: `GOOGLE_APPLICATION_CREDENTIALS`
- Azure: `AZURE_STORAGE_ACCOUNT_NAME`, `AZURE_STORAGE_ACCOUNT_KEY`

No MongoCore-specific config needed for credentials.

### 3. OpenTelemetry Support

#### Dependencies

Add to `Cargo.toml`:
```toml
opentelemetry = { version = "0.28", optional = true }
opentelemetry_sdk = { version = "0.28", features = ["rt-tokio"], optional = true }
opentelemetry-otlp = { version = "0.28", features = ["grpc-tonic"], optional = true }
tracing-opentelemetry = { version = "0.29", optional = true }

[features]
otel = ["opentelemetry", "opentelemetry_sdk", "opentelemetry-otlp", "tracing-opentelemetry"]
```

Enable MongoDB driver tracing:
```toml
mongodb = { version = "3", features = ["tracing-unstable"] }
```

#### Configuration

Add to `CliArgs`, `FileConfig`, and `Config`:

```rust
// CliArgs
#[arg(long, env = "MONGOCORE_OTEL_ENABLED")]
pub otel_enabled: Option<bool>,

#[arg(long, env = "MONGOCORE_OTEL_ENDPOINT")]
pub otel_endpoint: Option<String>,

#[arg(long, env = "MONGOCORE_OTEL_SERVICE_NAME")]
pub otel_service_name: Option<String>,
```

Defaults:
- `otel_enabled`: `false`
- `otel_endpoint`: `"http://localhost:4317"` (standard OTLP gRPC port)
- `otel_service_name`: `"mongocore"`

#### Tracing Initialization

In `main.rs`, when OTel is enabled:

```rust
if config.otel_enabled {
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(opentelemetry_otlp::new_exporter().tonic()
            .with_endpoint(&config.otel_endpoint))
        .with_trace_config(
            opentelemetry_sdk::trace::Config::default()
                .with_resource(Resource::new(vec![
                    KeyValue::new("service.name", config.otel_service_name.clone()),
                ]))
        )
        .install_batch(opentelemetry_sdk::runtime::Tokio)?;

    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .with(otel_layer)
        .init();
} else {
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();
}
```

#### MongoCore Spans

Add `#[tracing::instrument]` to key entry points:

- **gRPC:** Each RPC handler in `src/grpc/service.rs` (e.g. `find`, `insert`, `aggregate`)
- **MCP:** `McpHandler::handle_request()` and the tool dispatch in `handle_tools_call()`
- **Ingestion:** `IngestionEngine::ingest()` (job-level), chunk write operations
- **Operations:** Key methods in `src/operations/` modules

Span attributes include: operation type, database, collection, tenant_id (if multi-tenant).

#### Driver Spans

With `tracing-unstable` enabled on the `mongodb` crate, the driver automatically emits spans for:
- Connection checkout/checkin
- Command execution (with command name, database)
- Server selection

These appear as child spans under MongoCore's operation spans, giving full end-to-end visibility.

#### Graceful Shutdown

On sidecar shutdown, flush pending spans:
```rust
opentelemetry::global::shutdown_tracer_provider();
```

#### Documentation

Add `docs/opentelemetry.md` covering:
- How to enable OTel (config fields, env vars)
- What spans are emitted (MongoCore layer + driver layer)
- Example setup with Jaeger or OTel Collector
- Example config.toml snippet

Update `config.test.toml.example` with OTel fields (commented out):
```toml
# OpenTelemetry (optional)
# otel_enabled = true
# otel_endpoint = "http://localhost:4317"
# otel_service_name = "mongocore"
```

Update README documentation table to include the OTel guide.

## Implementation Scope

| Component | Files |
|-----------|-------|
| Driver metadata (startup) | `src/connection/pool.rs` |
| Driver metadata (per-interface append) | `src/grpc/service.rs`, `src/mcp/handler.rs`, `src/connection/pool.rs` |
| Client language headers | `clients/{python,typescript,go,java}/` |
| URL ingestion | `Cargo.toml` (polars features), `src/ingestion/reader.rs` |
| OTel dependencies | `Cargo.toml` |
| OTel config | `src/config.rs`, `src/defaults.rs` |
| OTel initialization | `src/main.rs` |
| MongoCore spans | `src/grpc/service.rs`, `src/mcp/handler.rs`, `src/ingestion/engine.rs`, `src/operations/*.rs` |
| Documentation | `docs/opentelemetry.md`, `config.test.toml.example`, `README.md` |
| Tests | `tests/integration/` (metadata verification, URL ingestion) |

## Won't Build

- Custom span exporters (use standard OTLP)
- Metrics (just tracing/spans for now)
- Per-request connection pools for language isolation
- MongoCore-specific cloud credential management (rely on env vars)

## Testing

- **Driver metadata:** Integration test that connects and verifies `$currentOp` or `serverStatus.connections` shows MongoCore metadata
- **URL ingestion:** Integration test that ingests from an HTTP URL (can use a local HTTP server in test harness)
- **OTel:** Unit test that verifies spans are created; integration test with in-memory exporter to verify span hierarchy

## Success Criteria

- [ ] MongoDB server logs show `mongocore/0.1.0` in connection metadata
- [ ] Per-interface metadata appended on first request (`mongocore|python`, `mongocore|mcp`, etc.)
- [ ] `ingest` tool accepts HTTP/HTTPS URLs and ingests data without local download
- [ ] `ingest` tool accepts S3/GCS/Azure URLs when credentials are configured
- [ ] OTel spans exported when `otel_enabled = true` and endpoint is configured
- [ ] Both MongoCore-level and driver-level spans visible in trace viewer
- [ ] Zero overhead when OTel is disabled (feature-gated)
- [ ] `docs/opentelemetry.md` documents setup, spans, and example configurations
- [ ] `config.test.toml.example` includes OTel parameters
