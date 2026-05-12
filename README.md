<p align="center">
  <img src="assets/mongocore-header.svg" alt="MongoCore" width="100%"/>
</p>

# MongoCore

An AI-native MongoDB driver implemented as a lightweight Rust sidecar. MongoCore provides a single, fast core that serves all languages via gRPC, with native AI agent support via MCP (Model Context Protocol).

## Key Features

- **Rust sidecar architecture** — One fast core serves every language via gRPC
- **AI-native from the outset** — MCP interface for AI agents alongside gRPC for applications
- **Compiled queries** — Natural language queries translated once by an LLM, cached and reused at native speed
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

See [Quick Start](./docs/quick-start.md) for all languages (Python, TypeScript, Go, Java), configuration, and testing setup.

## Documentation

| Guide | Description |
|-------|-------------|
| [Quick Start](./docs/quick-start.md) | All language examples, configuration, testing |
| [Getting Started](./docs/getting-started.md) | Installation, full configuration reference |
| [CRUD Operations](./docs/crud-operations.md) | Find, insert, update, delete in all languages |
| [Aggregation](./docs/aggregation.md) | Pipeline operations and common patterns |
| [Transactions](./docs/transactions.md) | Multi-document ACID transactions |
| [Search](./docs/search.md) | Vector search, full-text search, fallback chains |
| [Compiled Queries](./docs/compiled-queries.md) | Natural language to MQL with caching |
| [Change Streams](./docs/streaming.md) | Real-time notifications via Watch |
| [Data Ingestion](./docs/ingestion.md) | CSV/JSON/Parquet ingestion, transforms, dedup |
| [Admin Operations](./docs/admin-operations.md) | Collections, indexes, introspection |
| [MCP Server](./docs/mcp-server.md) | AI agent integration via JSON-RPC |
| [Client Libraries](./docs/client-libraries.md) | Language-specific setup and API reference |
| [Raw Passthrough](./docs/raw-passthrough.md) | Arbitrary MongoDB commands for power users |
| [Analytics](./docs/analytics.md) | Query performance insights and operation tracking |
| [Multi-Tenant](./docs/multi-tenant.md) | Shared sidecar with per-tenant isolation |
| [OpenTelemetry](./docs/opentelemetry.md) | Distributed tracing setup and configuration |
| [Design & Plans](./docs/design/) | Architecture specs and implementation plans |

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
│   ├── compiled/            # NL→MQL translation, 3-level cache, templates
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

## Feature Sets

### v0.1 — Core

- **25 gRPC RPCs** — Full CRUD, aggregation, transactions, search, admin, watch, ingestion
- **Change streams (Watch)** — Server-streaming RPC with auto-close in all clients (Python `async with`, TypeScript `AsyncDisposable`, Go `io.Closer`, Java `AutoCloseable`)
- **Search fallback chain** — Vector search → Atlas full-text → compiled query, with automatic fallthrough
- **Atlas Vector Search** — `$vectorSearch` with Voyage AI embeddings, tested end-to-end against Atlas Local
- **Atlas Full-Text Search** — `$search` with dynamic mappings, tested end-to-end
- **Compiled queries** — NL→MQL with 3-level cache (memory → disk → Atlas collection)
- **MCP server** — 21 tools for AI agent interaction with safety controls (read-only mode, command blocklist)
- **Polyglot clients** — Python, TypeScript, Go, and Java with full CRUD, Watch, and ingestion support
- **Opinionated defaults** — Majority concerns, retryable ops, sensible timeouts, auto `readConcern:local` for search

### v0.2 — Power Users & Operations

- **Raw wire protocol passthrough** — `RunCommand` RPC for arbitrary MongoDB commands with safety validation
- **Command blocklist** — Dangerous commands (`dropDatabase`, `shutdown`, etc.) blocked by default, explicit opt-in override
- **Query analytics** — Real-time event collection with ring buffer, latency percentiles (p50/p95/p99), error rates, top-N operations
- **Analytics persistence** — Optional background flush to `__mongocore.analytics` collection
- **`GetAnalytics` RPC + MCP tool** — Surface insights via both interfaces
- **Multi-tenant support** — `x-tenant-id` header partitions caches and enforces per-tenant quotas
- **Per-tenant rate limiting** — Configurable ops/sec with `RESOURCE_EXHAUSTED` on breach
- **Tenant registry** — TOML `[[tenants]]` config with per-tenant connection URI override

### v0.3 — Intelligent Data Ingestion

- **Polars-based ingestion** — CSV, JSON, NDJSON, Parquet with parallel processing via Polars LazyFrames
- **Schema inference** — Spark-connector-inspired multi-row sampling with Polars→BSON type mapping and type widening
- **Transform engine** — User-provided Polars expressions (rename, filter, cast, drop, select, derive)
- **LLM expressions (optional)** — `llm_classify`, `llm_extract`, `llm_normalize`, `llm_embed` when API key configured
- **Bulk writer** — Chunked parallel writes with DataFrame→BSON document conversion
- **Deduplication** — Key-based dedup with skip/overwrite/merge conflict resolution strategies
- **Dead letter queue** — Failed documents routed to `__mongocore.dead_letter` for inspection and retry
- **Progress tracking** — Real-time job status persisted to `__mongocore.ingestion_jobs`, resumable on crash
- **Directory watching** — Filesystem monitor with debounce, auto-triggers ingestion on new files
- **6 gRPC RPCs** — Ingest, GetIngestStatus, ListIngestJobs, CancelIngest, WatchDirectory, StopWatch
- **6 MCP tools** — Full AI agent support for data ingestion workflows

### v0.4 — Integration & Testing

- **Driver metadata** — MongoCore identifies itself in MongoDB handshakes (`mongocore/0.1.0`), per-client-language tagging via `x-client-language` header
- **URL-based ingestion** — Ingest from HTTP/HTTPS, S3, GCS, Azure Blob URLs via Polars cloud feature (no download step)
- **OpenTelemetry** — Optional distributed tracing (`--features otel`) with MongoCore-level and driver-level spans, OTLP export
- **Client methods** — Added FindAndModify, CreateIndex, BeginTransaction, CommitTransaction, AbortTransaction, GetAnalytics to all 4 clients
- **Full test coverage** — All 27 gRPC RPCs tested in every client library (Python, TypeScript, Go, Java)
- **Standardized unit tests** — 5+ unit tests per client library (no server required)
- **AGENTS.md / CLAUDE.md** — Universal AI agent development guide and Claude Code specialization
- **Testing rules** — Enforced test parity across all clients, verbose output, proto regeneration workflow documented

### v0.5 — LLM Gateway & Security

- **Custom LLM gateway** — Configurable base URL, auth header, and model for corporate AI gateways/proxies (supports both Anthropic and OpenAI request formats)
- **Simplified LLM config** — Direct API keys in TOML (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`) with env var fallback, auto-detect provider
- **MQL validator hardening** — Blocks `$function` and `$accumulator` operators (code execution risk), recursive pipeline content checking
- **LLM integration tests** — 20 tests across 4 sample databases with cache behavior validation and injection safety testing
- **Safety documentation** — Full documentation of validator protections, blocked operators, and attack vectors defended against
- **Auto-managed test sidecar** — `just test-clients` automatically builds, starts, and stops the sidecar
- **Sample data in Docker** — `just docker-up` loads Atlas sample datasets for LLM testing
- **TEST_LLM_INTEGRATION flag** — Opt-in flag for LLM tests, reads config from `config.test.toml`

## Roadmap

| Version | Focus | Status |
|---------|-------|--------|
| **v0.1** | Core sidecar, gRPC + MCP, compiled queries, Voyage AI, search, change streams | **Complete** |
| **v0.2** | Raw passthrough, query analytics, multi-tenant support | **Complete** |
| **v0.3** | Intelligent data ingestion (Polars-powered ETL) | **Complete** |
| **v0.4** | Integration (driver metadata, URL ingestion, OpenTelemetry), full client test coverage | **Complete** |
| **v0.5** | Custom LLM gateway, simplified config, validator hardening, LLM integration tests | **Complete** |

### Future Roadmap

| Area | Description |
|------|-------------|
| Demo | Stdio MCP transport for Claude Code integration, curated restaurant dataset, scripted demo flow |
| LLM-Provided Templates | Ask LLM to return parameterized templates for smarter cache reuse across semantic variants |
| MCP + Claude Integration | Auto-detect API key from Claude's environment when launched as MCP server, zero-config NL→MQL |
| Packaging & Deployment | Pre-built binaries (GitHub Releases, Homebrew), Docker images (GHCR), client libs on PyPI/npm/Maven Central, Helm chart |
| Performance | Benchmarking suite comparing MongoCore vs native drivers for common workloads |
| Visualizations | Configurable web UI for analytics, query flow, and ingestion progress |
| Migration & Ecosystem | Framework adapters (Mongoose, Spring Data, etc.), migration paths from existing drivers |
| Self-Contained AI | Local NL→MQL model, no external LLM dependency required |
| WASM & Extensibility | Browser client, WASM compilation target, plugin system |

## License

Apache-2.0 — See [LICENSE](LICENSE) for details.
