# Roadmap & Version History

## Completed Features

### Core

- **25 gRPC RPCs** — Full CRUD, aggregation, transactions, search, admin, watch, ingestion
- **Change streams (Watch)** — Server-streaming RPC with auto-close in all clients (Python `async with`, TypeScript `AsyncDisposable`, Go `io.Closer`, Java `AutoCloseable`)
- **Search fallback chain** — Vector search → Atlas full-text → compiled query, with automatic fallthrough
- **Atlas Vector Search** — `$vectorSearch` with Voyage AI embeddings, tested end-to-end against Atlas Local
- **Atlas Full-Text Search** — `$search` with dynamic mappings, tested end-to-end
- **Compiled queries** — NL→MQL with 3-level cache (memory → disk → Atlas collection)
- **MCP server** — 21 tools for AI agent interaction with safety controls (read-only mode, command blocklist)
- **Polyglot clients** — Python, TypeScript, Go, and Java with full CRUD, Watch, and ingestion support
- **Opinionated defaults** — Majority concerns, retryable ops, sensible timeouts, auto `readConcern:local` for search

### Power Users & Operations

- **Raw wire protocol passthrough** — `RunCommand` RPC for arbitrary MongoDB commands with safety validation
- **Command blocklist** — Dangerous commands (`dropDatabase`, `shutdown`, etc.) blocked by default, explicit opt-in override
- **Query analytics** — Real-time event collection with ring buffer, latency percentiles (p50/p95/p99), error rates, top-N operations
- **Analytics persistence** — Optional background flush to `__mongocore.analytics` collection
- **`GetAnalytics` RPC + MCP tool** — Surface insights via both interfaces
- **Multi-tenant support** — `x-tenant-id` header partitions caches and enforces per-tenant quotas
- **Per-tenant rate limiting** — Configurable ops/sec with `RESOURCE_EXHAUSTED` on breach
- **Tenant registry** — TOML `[[tenants]]` config with per-tenant connection URI override

### Intelligent Data Ingestion

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

### Integration & Testing

- **Driver metadata** — MongoCore identifies itself in MongoDB handshakes (`mongocore/0.1.0`), per-client-language tagging via `x-client-language` header
- **URL-based ingestion** — Ingest from HTTP/HTTPS, S3, GCS, Azure Blob URLs via Polars cloud feature (no download step)
- **OpenTelemetry** — Optional distributed tracing (`--features otel`) with MongoCore-level and driver-level spans, OTLP export
- **Client methods** — Added FindAndModify, CreateIndex, BeginTransaction, CommitTransaction, AbortTransaction, GetAnalytics to all 4 clients
- **Full test coverage** — All 27 gRPC RPCs tested in every client library (Python, TypeScript, Go, Java)
- **Standardized unit tests** — 5+ unit tests per client library (no server required)
- **AGENTS.md / CLAUDE.md** — Universal AI agent development guide and Claude Code specialization
- **Testing rules** — Enforced test parity across all clients, verbose output, proto regeneration workflow documented

### LLM Gateway & Security

- **Custom LLM gateway** — Configurable base URL, auth header, and model for corporate AI gateways/proxies (supports both Anthropic and OpenAI request formats)
- **Simplified LLM config** — Direct API keys in TOML (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`) with env var fallback, auto-detect provider
- **MQL validator hardening** — Blocks `$function` and `$accumulator` operators (code execution risk), recursive pipeline content checking
- **LLM integration tests** — 20 tests across 4 sample databases with cache behavior validation and injection safety testing
- **Safety documentation** — Full documentation of validator protections, blocked operators, and attack vectors defended against
- **Auto-managed test sidecar** — `just test-clients` automatically builds, starts, and stops the sidecar
- **Sample data in Docker** — `just docker-up` loads Atlas sample datasets for LLM testing
- **TEST_LLM_INTEGRATION flag** — Opt-in flag for LLM tests, reads config from `config.test.toml`

### Intelligent NL→MQL Routing & Templates

- **LLM-provided templates** — LLM returns parameterized templates alongside MQL, enabling cache reuse across semantic variants ("Italian restaurants" → "Chinese restaurants" without LLM call)
- **Intelligent method routing** — LLM classifies query intent and routes to optimal execution: filter, aggregate, vector_search, fulltext, or geo
- **Template registry** — Regex-based matching of new queries against cached intent patterns with automatic parameter substitution
- **CompiledMql extended** — 5 execution variants: Find, Aggregate, VectorSearch, Fulltext, Geo
- **Markdown fence stripping** — Robust LLM response parsing handles ` ```json ``` ` wrapped output
- **23 LLM integration tests** — Routing verification, template reuse, multi-database, injection safety
- **Zero warnings policy** — Enforced in AGENTS.md/CLAUDE.md, all commits must produce zero compiler warnings

### Performance Benchmarking

- **Cross-language benchmark suite** — Python, TypeScript, Go, Java comparing native drivers vs MongoCore sidecar
- **MongoDB Driver Benchmarking Spec** — Batched iterations (10K ops/iter), warmup, percentile reporting (p10-p99), before_task cleanup
- **Consistent methodology** — All 8 benchmarks use identical harness structure, batch sizes, and cleanup for fair comparison
- **Polars ingestion benchmarks** — Native pymongo bulk insert vs MongoCore Polars pipeline at 1MB, 10MB, 100MB
- **Rust criterion microbenchmarks** — Cache lookup, MQL validation, template matching (sidecar internals)
- **Auto-generated results** — Jinja2 templates produce per-run README with throughput tables, latency percentiles, SVG charts
- **Composable justfile** — `bench-setup`/`bench-teardown` lifecycle, per-language and per-variant tasks
- **Incremental runs** — Benchmarks skip automatically when results exist; delete a file to rerun just that benchmark
- **Honest caveats** — Documented limitations (uncontrolled environment, no tuning, localhost, gRPC limits, single client)

### MCP + Claude Integration

- **Stdio MCP transport** — `--stdio` flag for Claude Desktop/Code integration, JSON-RPC over stdin/stdout
- **`ask` tool** — Natural language questions → MQL → execute → return answer with generated query and confidence
- **`explain_query` tool** — NL → MQL translation with execution plan, no execution (safe for expensive queries)
- **`collection_schema` tool** — Sample documents and infer schema (field types, cardinality, examples)
- **MCP sampling** — Zero-config LLM: uses Claude itself via MCP sampling protocol when no API key configured
- **Code generation** — `generate_code`, `generate_model`, `generate_index` tools with Tera templates for Python, TypeScript, Go, Java
- **Language/framework detection** — Auto-detect from workspace (pyproject.toml, package.json, go.mod, pom.xml/build.gradle.kts)
- **Composable skill recommendations** — Detects framework (FastAPI, Express, Spring, etc.) and recommends combining with framework-specific skills
- **Embedding pipeline** — `embed_and_store`, `semantic_search`, `ingest_and_embed` tools wiring Voyage AI + Polars + $vectorSearch
- **Skills system** — 13 guided workflows (MCP Prompts protocol + `list_skills`/`get_skill` tool fallback)
- **Insights tools** — `suggest_indexes` and `slow_queries` analyzing analytics ring buffer
- **Schema resource** — `mongocore://schema/{database}/{collection}` MCP resource
- **35 MCP tools total** — up from 21 in v0.7

### Web UI & Diagnostics

- **Web UI Dashboard** — Built-in diagnostic dashboard with real-time analytics
- **Real-time Charts** — Operations/sec and latency charts powered by analytics ring buffer
- **Operation Insights** — Operation breakdown, query insights, pipeline and transaction stats
- **Error Tracking** — Recent errors with context
- **Expandable Statistics** — LLM usage, ingestion progress, cache statistics
- **Localhost-only binding** — Secure by default, binds to 127.0.0.1:27999
- **Configuration** — Enable/disable via `--web-ui` flag, port configurable via `--web-ui-port`

### Performance Tier 1

- **Unix Domain Socket transport** — `--transport` flag (both/uds/tcp), default both. Automatic socket at `/tmp/mongocore.sock`
- **64MB message limit** — Raised from 4MB default, configurable via `--grpc-max-message-size`
- **Streaming bulk RPCs** — `FindStream`, `AggregateStream`, `InsertManyStream`, `InsertManyBidi` for unlimited result sizes
- **gRPC compression** — Optional gzip/zstd compression via `--grpc-compression`
- **Stream idle timeout** — Configurable server-side timeout for streaming cursors (default 60s)
- **Client auto-discovery** — Python and Go clients automatically find UDS socket, fall back to TCP
- **Socket lifecycle** — Automatic cleanup on shutdown, stale file removal on startup, graceful fallback on bind failure

### Request Pipelining

- **Pipeline RPC** — Batch N independent operations in a single gRPC round-trip with concurrent execution
- **All non-streaming operations** — Find, FindOne, Insert, InsertMany, Update, UpdateMany, Delete, DeleteMany, Aggregate, FindAndModify, RunCommand, Search, CreateCollection, CreateIndex, ListDatabases, ListCollections, transactions, GetAnalytics
- **Concurrent execution** — Operations fan out via tokio with semaphore-based concurrency limit (default 20)
- **Per-operation errors** — Individual failures don't abort the pipeline; results indexed by position
- **Pipeline timeout** — Configurable deadline (default 30s) with cancellation of incomplete ops
- **MCP pipeline tool** — All-or-nothing safety validation (rejects entire pipeline if any op violates read-only mode)
- **Typed client builders** — `ops` modules in Python, TypeScript, Go, Java with typed result accessors

### Transactional Pipelines

- **TransactionPipeline RPC** — Execute sequential dependent operations atomically within a single MongoDB transaction
- **Result forwarding** — `{{step_name.field}}` reference syntax resolves prior step results into subsequent operations
- **Reference patterns** — Top-level, nested, array index, wildcard pluck, passthrough, and length accessors
- **Type preservation** — Standalone references preserve original types (numbers, arrays, objects); inline references interpolate as strings
- **Validation** — Step name uniqueness, no forward references, no nested transactions, max 50 steps, max 101 docs per find/aggregate
- **Transient error retry** — Automatic retry (up to 3 attempts) on `TransientTransactionError`
- **Transaction options** — Configurable read_concern, write_concern, max_time_ms
- **Collection-scoped API** — Simpler form when all steps target the same collection
- **MCP tool** — `transaction_pipeline` tool for AI agents with full safety validation
- **All 4 client libraries** — Python, TypeScript, Go, Java with typed step builders

## Backlog

Items ordered by recommended implementation sequence.

| Area | Description |
|------|-------------|
| Search RPC Integration | Wire compiled query translator into search fallback chain as intelligent router |
| MCP Code Quality | Extract schema helpers into `src/mcp/schema.rs`; split `tools.rs` into submodules; add document count limit to `embed_and_store`; complete `ingest_and_embed` pipeline |
| Pipeline Benchmarks | Add pipeline equivalents to cross-language benchmark suite — compare N individual driver calls vs single pipeline RPC |
| Demo | Curated restaurant dataset, scripted demo flow |
| Query Explanation (Enhanced) | Add confidence scores, alternative interpretations, and cost estimates to `explain_query` |
| Self-Contained AI | Local NL→MQL model, no external LLM dependency required |
| Native Embedding (FFI) | PyO3, Neon, cgo embedding for zero-IPC overhead |
| Driver API parity | Add all the standard driver level database / client apis |
| Migration & Ecosystem | Framework adapters (Mongoose, Spring Data, etc.), migration paths |
| WASM & Extensibility | Browser client, WASM compilation target, plugin system |
| Packaging & Deployment | Pre-built binaries (GitHub Releases, Homebrew), Docker images (GHCR), Helm chart |
