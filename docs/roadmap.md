# Roadmap & Version History

## Version History

| Version | Focus | Status |
|---------|-------|--------|
| **v0.1** | Core sidecar, gRPC + MCP, compiled queries, Voyage AI, search, change streams | **Complete** |
| **v0.2** | Raw passthrough, query analytics, multi-tenant support | **Complete** |
| **v0.3** | Intelligent data ingestion (Polars-powered ETL) | **Complete** |
| **v0.4** | Integration (driver metadata, URL ingestion, OpenTelemetry), full client test coverage | **Complete** |
| **v0.5** | Custom LLM gateway, simplified config, validator hardening, LLM integration tests | **Complete** |
| **v0.6** | Intelligent NL→MQL routing, LLM-provided templates, template registry | **Complete** |
| **v0.7** | Cross-language performance benchmarking suite | **Complete** |
| **v0.8** | MCP + Claude Integration: Intelligent Data Companion | **Complete** |
| **v0.8.1** | Performance Tier 1 (transport, streaming, compression) | **Complete** |
| **v0.9** | Request pipelining (Performance Tier 2) | **Complete** |

## v0.1 — Core

- **25 gRPC RPCs** — Full CRUD, aggregation, transactions, search, admin, watch, ingestion
- **Change streams (Watch)** — Server-streaming RPC with auto-close in all clients (Python `async with`, TypeScript `AsyncDisposable`, Go `io.Closer`, Java `AutoCloseable`)
- **Search fallback chain** — Vector search → Atlas full-text → compiled query, with automatic fallthrough
- **Atlas Vector Search** — `$vectorSearch` with Voyage AI embeddings, tested end-to-end against Atlas Local
- **Atlas Full-Text Search** — `$search` with dynamic mappings, tested end-to-end
- **Compiled queries** — NL→MQL with 3-level cache (memory → disk → Atlas collection)
- **MCP server** — 21 tools for AI agent interaction with safety controls (read-only mode, command blocklist)
- **Polyglot clients** — Python, TypeScript, Go, and Java with full CRUD, Watch, and ingestion support
- **Opinionated defaults** — Majority concerns, retryable ops, sensible timeouts, auto `readConcern:local` for search

## v0.2 — Power Users & Operations

- **Raw wire protocol passthrough** — `RunCommand` RPC for arbitrary MongoDB commands with safety validation
- **Command blocklist** — Dangerous commands (`dropDatabase`, `shutdown`, etc.) blocked by default, explicit opt-in override
- **Query analytics** — Real-time event collection with ring buffer, latency percentiles (p50/p95/p99), error rates, top-N operations
- **Analytics persistence** — Optional background flush to `__mongocore.analytics` collection
- **`GetAnalytics` RPC + MCP tool** — Surface insights via both interfaces
- **Multi-tenant support** — `x-tenant-id` header partitions caches and enforces per-tenant quotas
- **Per-tenant rate limiting** — Configurable ops/sec with `RESOURCE_EXHAUSTED` on breach
- **Tenant registry** — TOML `[[tenants]]` config with per-tenant connection URI override

## v0.3 — Intelligent Data Ingestion

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

## v0.4 — Integration & Testing

- **Driver metadata** — MongoCore identifies itself in MongoDB handshakes (`mongocore/0.1.0`), per-client-language tagging via `x-client-language` header
- **URL-based ingestion** — Ingest from HTTP/HTTPS, S3, GCS, Azure Blob URLs via Polars cloud feature (no download step)
- **OpenTelemetry** — Optional distributed tracing (`--features otel`) with MongoCore-level and driver-level spans, OTLP export
- **Client methods** — Added FindAndModify, CreateIndex, BeginTransaction, CommitTransaction, AbortTransaction, GetAnalytics to all 4 clients
- **Full test coverage** — All 27 gRPC RPCs tested in every client library (Python, TypeScript, Go, Java)
- **Standardized unit tests** — 5+ unit tests per client library (no server required)
- **AGENTS.md / CLAUDE.md** — Universal AI agent development guide and Claude Code specialization
- **Testing rules** — Enforced test parity across all clients, verbose output, proto regeneration workflow documented

## v0.5 — LLM Gateway & Security

- **Custom LLM gateway** — Configurable base URL, auth header, and model for corporate AI gateways/proxies (supports both Anthropic and OpenAI request formats)
- **Simplified LLM config** — Direct API keys in TOML (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`) with env var fallback, auto-detect provider
- **MQL validator hardening** — Blocks `$function` and `$accumulator` operators (code execution risk), recursive pipeline content checking
- **LLM integration tests** — 20 tests across 4 sample databases with cache behavior validation and injection safety testing
- **Safety documentation** — Full documentation of validator protections, blocked operators, and attack vectors defended against
- **Auto-managed test sidecar** — `just test-clients` automatically builds, starts, and stops the sidecar
- **Sample data in Docker** — `just docker-up` loads Atlas sample datasets for LLM testing
- **TEST_LLM_INTEGRATION flag** — Opt-in flag for LLM tests, reads config from `config.test.toml`

## v0.6 — Intelligent NL→MQL Routing & Templates

- **LLM-provided templates** — LLM returns parameterized templates alongside MQL, enabling cache reuse across semantic variants ("Italian restaurants" → "Chinese restaurants" without LLM call)
- **Intelligent method routing** — LLM classifies query intent and routes to optimal execution: filter, aggregate, vector_search, fulltext, or geo
- **Template registry** — Regex-based matching of new queries against cached intent patterns with automatic parameter substitution
- **CompiledMql extended** — 5 execution variants: Find, Aggregate, VectorSearch, Fulltext, Geo
- **Markdown fence stripping** — Robust LLM response parsing handles ` ```json ``` ` wrapped output
- **23 LLM integration tests** — Routing verification, template reuse, multi-database, injection safety
- **Zero warnings policy** — Enforced in AGENTS.md/CLAUDE.md, all commits must produce zero compiler warnings

## v0.7 — Performance Benchmarking

- **Cross-language benchmark suite** — Python, TypeScript, Go, Java comparing native drivers vs MongoCore sidecar
- **MongoDB Driver Benchmarking Spec** — Batched iterations (10K ops/iter), warmup, percentile reporting (p10-p99), before_task cleanup
- **Consistent methodology** — All 8 benchmarks use identical harness structure, batch sizes, and cleanup for fair comparison
- **Polars ingestion benchmarks** — Native pymongo bulk insert vs MongoCore Polars pipeline at 1MB, 10MB, 100MB
- **Rust criterion microbenchmarks** — Cache lookup, MQL validation, template matching (sidecar internals)
- **Auto-generated results** — Jinja2 templates produce per-run README with throughput tables, latency percentiles, SVG charts
- **Composable justfile** — `bench-setup`/`bench-teardown` lifecycle, per-language and per-variant tasks
- **Regression detection** — `bench-check-regression` compares runs and exits non-zero on >10% slowdown
- **Timestamped results** — Each run stored in `results/<timestamp>/` with `latest` symlink, history committed to git
- **Honest caveats** — Documented limitations (uncontrolled environment, no tuning, localhost, gRPC limits, single client)

## v0.8 — MCP + Claude Integration

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

## v0.8.1 — Performance Tier 1

- **Unix Domain Socket transport** — `--transport` flag (both/uds/tcp), default both. Automatic socket at `/tmp/mongocore.sock`
- **64MB message limit** — Raised from 4MB default, configurable via `--grpc-max-message-size`
- **Streaming bulk RPCs** — `FindStream`, `AggregateStream`, `InsertManyStream`, `InsertManyBidi` for unlimited result sizes
- **gRPC compression** — Optional gzip/zstd compression via `--grpc-compression`
- **Stream idle timeout** — Configurable server-side timeout for streaming cursors (default 60s)
- **Client auto-discovery** — Python and Go clients automatically find UDS socket, fall back to TCP
- **Socket lifecycle** — Automatic cleanup on shutdown, stale file removal on startup, graceful fallback on bind failure

## v0.9 — Request Pipelining

- **Pipeline RPC** — Batch N independent operations in a single gRPC round-trip with concurrent execution
- **All non-streaming operations** — Find, FindOne, Insert, InsertMany, Update, UpdateMany, Delete, DeleteMany, Aggregate, FindAndModify, RunCommand, Search, CreateCollection, CreateIndex, ListDatabases, ListCollections, transactions, GetAnalytics
- **Concurrent execution** — Operations fan out via tokio with semaphore-based concurrency limit (default 20)
- **Per-operation errors** — Individual failures don't abort the pipeline; results indexed by position
- **Pipeline timeout** — Configurable deadline (default 30s) with cancellation of incomplete ops
- **MCP pipeline tool** — All-or-nothing safety validation (rejects entire pipeline if any op violates read-only mode)
- **Typed client builders** — `ops` modules in Python, TypeScript, Go, Java with typed result accessors

## Backlog

| Area | Description |
|------|-------------|
| Client UDS Support | Add Unix Domain Socket auto-discovery to Java and TypeScript clients (Python and Go already supported) |
| Pipeline Benchmarks | Add pipeline equivalents to the cross-language benchmark suite (Python, TypeScript, Go, Java) — compare N individual driver calls vs single pipeline RPC at various batch sizes |
| Search RPC Integration | Wire compiled query translator into search handler as intelligent router |
| Query Explanation | Show generated MQL, confidence scores, and alternative interpretations to users |
| Hybrid Search (RRF) | Vector + fulltext with reciprocal rank fusion scoring — industry standard for RAG |
| Window Functions | Moving averages, running totals, rankings via $setWindowFields |
| Graph Queries | $graphLookup support with safety constraints for recursive hierarchy traversal |
| Enterprise Compliance | Audit trail, multi-tenant auto-scoping, role-based field redaction, query governance |
| Demo | Curated restaurant dataset, scripted demo flow |
| MCP Code Quality | Extract shared schema inference helpers (FieldInfo, collect_fields) into `src/mcp/schema.rs`; split `tools.rs` into submodules; add document count limit to `embed_and_store`; implement full `ingest_and_embed` pipeline |
| Packaging & Deployment | Pre-built binaries (GitHub Releases, Homebrew), Docker images (GHCR), Helm chart |
| Performance Tier 1 | gRPC over Unix Domain Sockets + streaming bulk responses + raised message limits | **Complete (v0.8.1)** |
| Performance Tier 2 | Request pipelining — batch N independent operations in a single round-trip | **Complete (v0.9)** |
| Performance Tier 2b | Transactional pipeline — sequential ops with result forwarding between steps (dependent operations) |
| Performance Tier 3 | Native embedding via FFI (PyO3, Neon, cgo) for zero-IPC overhead |
| BulkWrite Operations | Collection-level and client-level bulkWrite with mixed insert/update/delete |
| Database/Collection Management | Drop database, drop collection, rename collection, list indexes, compact |
| Visualizations | Configurable web UI for analytics, query flow, and ingestion progress |
| Migration & Ecosystem | Framework adapters (Mongoose, Spring Data, etc.), migration paths |
| Self-Contained AI | Local NL→MQL model, no external LLM dependency required |
| WASM & Extensibility | Browser client, WASM compilation target, plugin system |
