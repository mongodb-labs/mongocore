<p align="center">
  <img src="assets/mongocore-header.svg" alt="MongoCore" width="100%"/>
</p>

# MongoCore

An AI-native MongoDB driver implemented as a lightweight Rust sidecar. MongoCore provides a single, fast core that serves all languages via gRPC, with native AI agent support via MCP (Model Context Protocol).

## Key Features

- **Rust sidecar architecture** — One fast core serves every language via gRPC
- **AI-native from the outset** — MCP interface for AI agents alongside gRPC for applications
- **Compiled queries** — NL→MQL with intelligent routing (filter/aggregate/vector/fulltext/geo), parameterized template reuse, and multi-level caching
- **Voyage AI integration** — Auto-embed on write, semantic vector search, reranking, batch embedding
- **Atlas Search & Vector Search** — Full-text and vector search with automatic `readConcern:local` handling
- **Search fallback chain** — Vector → full-text → compiled query → clear error (no silent degradation)
- **Change streams** — Real-time Watch with auto-close semantics in all languages
- **Data ingestion** — Polars-powered CSV/JSON/Parquet/URL/S3/GCS ingestion with schema inference and transforms
- **Query analytics** — Real-time latency percentiles, error rates, and operation insights
- **Multi-tenant support** — Shared sidecar with isolated caches, rate limiting, per-tenant pools
- **Raw passthrough** — Escape hatch for arbitrary MongoDB commands with safety validation
- **Polyglot clients** — Python, TypeScript, Go, and Java with idiomatic APIs
- **OpenTelemetry** — Optional distributed tracing with driver-level and MongoCore-level spans
- **Driver metadata** — MongoCore identifies itself in MongoDB handshakes, per-client-language tagging
- **Opinionated defaults** — Majority write/read concern, retryable operations, sensible timeouts

## Architecture

```
┌──────────────┐         ┌──────────────────────────┐         ┌──────────┐
│ App (gRPC)   │────────▶│                          │────────▶│ MongoDB  │
├──────────────┤         │   MongoCore Sidecar      │         └──────────┘
│ AI Agent     │──MCP───▶│        (Rust)            │────────▶ Voyage AI
└──────────────┘         └──────────────────────────┘────────▶ LLM Provider
```

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (1.85+)
- [Protobuf](https://grpc.io/docs/protoc-installation/) (`protoc` compiler, for client stub regeneration)
- [Docker](https://docs.docker.com/get-docker/) (for MongoDB)
- [just](https://github.com/casey/just) (task runner, optional)

### Build & Run

```bash
git clone https://github.com/rozza/mongocore.git
cd mongocore
cargo build --release

# Start MongoDB
just docker-up

# Run with defaults (gRPC :50051, MCP :3000)
cargo run

# Or with a config file
cargo run -- --config config.toml
```

### Connect (Python example)

```python
from mongocore import MongoClient

async with MongoClient("localhost:50051") as client:
    users = client["myapp"]["users"]
    await users.insert_one({"name": "Alice", "age": 30})
    results = await users.find({"age": {"$gte": 25}})
```

See [Quick Start](./docs/quick-start.md) for all languages (Python, TypeScript, Go, Java) and configuration.

## Documentation

Full documentation is available in the [`docs/`](./docs/) folder — see the [Documentation Index](./docs/README.md) for all guides.

## Opinionated Defaults

| Setting | Default | Why |
|---------|---------|-----|
| Write concern | `majority` | Prevents silent data loss |
| Read concern | `majority` | Prevents dirty reads |
| Retryable writes | `true` | Handles transient failures |
| Retryable reads | `true` | Handles transient failures |
| Read preference | `primaryPreferred` | Balances freshness and availability |
| Query timeout | 30s | Prevents runaway queries |
| Aggregation timeout | 60s | Allows complex pipelines |
| Search read concern | `local` | Required by `$search`/`$vectorSearch` (auto-detected) |

## Project Structure

```
mongocore/
├── src/
│   ├── main.rs              # Entry point, startup orchestration
│   ├── config.rs            # Layered config (CLI + env + TOML)
│   ├── connection/          # Connection pool, capability detection
│   ├── operations/          # CRUD, aggregation, transactions, admin, raw passthrough
│   ├── ingestion/           # Polars-based data ingestion, transforms, dedup, DLQ, watch
│   ├── grpc/                # gRPC server (tonic) — 25 RPCs
│   ├── mcp/                 # MCP server (axum) — JSON-RPC tools & resources
│   ├── compiled/            # NL→MQL translation, routing, template registry, 3-level cache
│   ├── search/              # Vector search, full-text, fallback chain
│   ├── analytics/           # Query analytics, ring buffer, aggregator, persistence
│   ├── tenant/              # Multi-tenant context, registry, isolation, quota
│   └── voyage/              # Voyage AI REST client, batch embeddings
├── proto/                   # Protobuf service definitions (25 RPCs)
├── clients/
│   ├── python/              # Python async client (BSON-native, change streams)
│   ├── typescript/          # TypeScript/Node.js client (AsyncDisposable streams)
│   ├── go/                  # Go client (io.Closer streams)
│   └── java/                # Java client (AutoCloseable, try-with-resources)
├── docs/                    # User-facing documentation
│   └── design/              # Design specs and implementation plans
├── tests/                   # Integration tests (one file per subsystem)
├── docker-compose.test.yml  # Atlas Local for testing
├── Dockerfile               # Multi-stage production build
└── justfile                 # Task runner
```

## Roadmap

See [Roadmap & Version History](./docs/roadmap.md) for detailed feature sets per version and the full future roadmap.

**Current version: v0.6** — Intelligent NL→MQL routing with LLM-provided templates

## License

Apache-2.0 — See [LICENSE](LICENSE) for details.
