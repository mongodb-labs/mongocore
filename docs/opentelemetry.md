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
