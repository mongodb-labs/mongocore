# MongoCore Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build an AI-native MongoDB driver as a Rust sidecar with dual gRPC/MCP interfaces, compiled natural-language queries, and Voyage AI integration.

**Architecture:** A single Rust binary (the sidecar) handles all MongoDB wire protocol communication, exposes a gRPC server for application clients and an MCP server for AI agents. Both interfaces share a compiled query cache (local + Atlas-synced) and Voyage AI embedding orchestration. Language clients are thin auto-generated gRPC stubs.

**Tech Stack:** Rust (tokio async runtime), tonic (gRPC), MongoDB wire protocol (OP_MSG), JSON-RPC/HTTP (MCP), Protocol Buffers, BSON, Voyage AI REST API, pluggable LLM providers (Claude/OpenAI APIs).

---

## Subsystem Overview

The implementation is decomposed into 7 subsystems, built in dependency order. Each subsystem produces working, testable software independently.

```
Subsystem 1: Rust Sidecar Core (foundation)
    ↓
Subsystem 2: gRPC Data Plane (app interface)
    ↓
Subsystem 3: MCP Agent Plane (AI interface)
    ↓
Subsystem 4: Compiled Query System (NL→MQL + caching)
    ↓
Subsystem 5: Voyage AI & Vector Search (embeddings + semantic search)
    ↓
Subsystem 6: Language Client Libraries (polyglot stubs + dev-mode management)
    ↓
Subsystem 7: Deployment & Packaging (binaries, containers, auto-download)
```

---

## Subsystem 1: Rust Sidecar Core

**Goal:** A Rust binary that connects to MongoDB via the wire protocol, manages connection pools, handles BSON serialization, and provides the foundation for all other subsystems.

**Files to create:**
- `Cargo.toml` — workspace root with dependencies
- `src/main.rs` — entry point, config loading, startup logging
- `src/config.rs` — configuration parsing (TOML + env vars + CLI args)
- `src/connection/mod.rs` — connection pool manager
- `src/connection/pool.rs` — pool implementation (min/max connections, health checks)
- `src/connection/wire.rs` — MongoDB OP_MSG wire protocol encoding/decoding
- `src/bson/mod.rs` — BSON serialization helpers (wrapping the `bson` crate with MongoCore conventions)
- `src/operations/mod.rs` — operation dispatching (CRUD, admin, aggregation)
- `src/operations/crud.rs` — find, insert, update, delete implementations
- `src/operations/admin.rs` — createCollection, createDatabase, createUser, createIndex
- `src/operations/aggregation.rs` — aggregate pipeline execution
- `src/operations/transaction.rs` — beginTransaction, commit, abort
- `src/operations/find_and_modify.rs` — atomic find+modify
- `src/defaults.rs` — opinionated defaults (majority write/read concern, retries, timeouts)
- `src/error.rs` — error types and mapping
- `src/health.rs` — health check endpoint, capability detection, startup logging
- `tests/integration/connection_test.rs` — connection pool tests against real MongoDB
- `tests/integration/crud_test.rs` — CRUD operation tests
- `tests/integration/transaction_test.rs` — transaction tests

**Key decisions:**
- Use the `mongodb` Rust crate internally as the wire protocol layer (it's the official Rust driver). MongoCore wraps it with opinionated defaults and exposes it via gRPC/MCP rather than reimplementing the wire protocol from scratch.
- Tokio async runtime for all I/O.
- Connection pool wraps the `mongodb::Client` with MongoCore's default options baked in.
- All operations enforce majority write concern, retryable writes, and sensible timeouts unless explicitly overridden.

**Testing approach:**
- Unit tests for config parsing, BSON helpers, default application.
- Integration tests require a running MongoDB instance (use `docker compose` with a replica set for transaction support).
- Each operation tested with golden-path + error cases.

**Startup capability logging:**
- On connect, detect MongoDB version, Atlas features (vector search indexes, stream processing), and log capabilities.

---

## Subsystem 2: gRPC Data Plane

**Goal:** Expose all MongoCore operations over gRPC with bidirectional streaming for change streams. Auto-generate client stubs from `.proto` definitions.

**Files to create:**
- `proto/mongocore/v1/mongocore.proto` — service definition (all operations)
- `proto/mongocore/v1/types.proto` — shared message types (Document, Filter, Pipeline, etc.)
- `proto/mongocore/v1/streaming.proto` — change stream service definition
- `src/grpc/mod.rs` — gRPC server setup (tonic)
- `src/grpc/service.rs` — MongoCore gRPC service implementation
- `src/grpc/streaming.rs` — change stream bidirectional streaming
- `src/grpc/interceptors.rs` — request validation, timeout enforcement
- `build.rs` — protobuf compilation (tonic-build)
- `tests/integration/grpc_test.rs` — gRPC endpoint tests

**Key decisions:**
- Proto definitions are the contract. Language clients auto-generate from these.
- Documents are passed as BSON bytes in protobuf `bytes` fields (avoids double-serialization penalty of converting BSON→JSON→protobuf structs).
- Streaming RPCs for `watch()` (change streams) — server-streaming from sidecar to client.
- Unary RPCs for CRUD, aggregation, admin operations.
- Client-side deadlines enforced via gRPC timeout metadata.

**Proto service shape (high-level):**
```protobuf
service MongoCore {
  // CRUD
  rpc Find(FindRequest) returns (FindResponse);
  rpc FindOne(FindOneRequest) returns (FindOneResponse);
  rpc Insert(InsertRequest) returns (InsertResponse);
  rpc InsertMany(InsertManyRequest) returns (InsertManyResponse);
  rpc Update(UpdateRequest) returns (UpdateResponse);
  rpc UpdateMany(UpdateManyRequest) returns (UpdateManyResponse);
  rpc Delete(DeleteRequest) returns (DeleteResponse);
  rpc DeleteMany(DeleteManyRequest) returns (DeleteManyResponse);
  rpc FindAndModify(FindAndModifyRequest) returns (FindAndModifyResponse);

  // Aggregation
  rpc Aggregate(AggregateRequest) returns (AggregateResponse);
  rpc Search(SearchRequest) returns (SearchResponse);
  rpc VectorSearch(VectorSearchRequest) returns (VectorSearchResponse);

  // Transactions
  rpc BeginTransaction(BeginTransactionRequest) returns (BeginTransactionResponse);
  rpc Commit(CommitRequest) returns (CommitResponse);
  rpc Abort(AbortRequest) returns (AbortResponse);

  // Admin
  rpc CreateCollection(CreateCollectionRequest) returns (CreateCollectionResponse);
  rpc CreateDatabase(CreateDatabaseRequest) returns (CreateDatabaseResponse);
  rpc CreateUser(CreateUserRequest) returns (CreateUserResponse);
  rpc CreateIndex(CreateIndexRequest) returns (CreateIndexResponse);

  // Streaming
  rpc Watch(WatchRequest) returns (stream WatchEvent);

  // Introspection
  rpc ListDatabases(ListDatabasesRequest) returns (ListDatabasesResponse);
  rpc ListCollections(ListCollectionsRequest) returns (ListCollectionsResponse);
  rpc CollectionSchema(CollectionSchemaRequest) returns (CollectionSchemaResponse);
}
```

**Testing approach:**
- Start sidecar in test, connect gRPC client, exercise each RPC against real MongoDB.
- Streaming tests verify change events arrive for inserts/updates/deletes.

---

## Subsystem 3: MCP Agent Plane

**Goal:** Expose the same operations as MCP tools over Streamable HTTP (JSON-RPC). AI agents discover tools, read resources (schemas, capabilities), and execute operations with safety controls.

**Files to create:**
- `src/mcp/mod.rs` — MCP server setup (HTTP listener, JSON-RPC dispatch)
- `src/mcp/tools.rs` — tool definitions and handlers (maps to same operations as gRPC)
- `src/mcp/resources.rs` — resource exposure (schemas, indexes, capabilities, compiled queries)
- `src/mcp/safety.rs` — read-only mode, confirmation-required writes, query cost estimation, doc limits
- `src/mcp/session.rs` — session management, interest pattern registration
- `src/mcp/notifications.rs` — change stream → MCP notification bridge
- `tests/integration/mcp_test.rs` — MCP tool call tests

**Key decisions:**
- Implement MCP over Streamable HTTP (POST endpoint returning JSON or SSE streams).
- Tools mirror gRPC operations: `find`, `aggregate`, `search`, `vector_search`, `insert`, `update`, `delete`, `watch`, `list_databases`, `list_collections`, `collection_schema`, `explain`, `create_index`, `create_collection`.
- Resources expose: collection schemas (inferred), available indexes, compiled query library, capability report.
- Safety: `--read-only` flag disables all write tools. Write operations return a confirmation prompt by default (configurable). Max 100 documents per response (configurable).
- Bidirectional: agents register interest patterns via `watch` tool, receive notifications as SSE events.

**Testing approach:**
- HTTP client tests sending JSON-RPC requests, verifying tool discovery and execution.
- Safety tests: verify read-only mode blocks writes, confirm doc limits enforced.
- Session tests: register interest pattern, insert matching doc, verify notification arrives.

---

## Subsystem 4: Compiled Query System

**Goal:** Translate natural-language intent into validated MQL, cache it, and reuse on subsequent calls. Cache hierarchy: in-memory → disk → Atlas collection.

**Files to create:**
- `src/compiled/mod.rs` — compiled query orchestration
- `src/compiled/hasher.rs` — intent string + context → deterministic hash
- `src/compiled/translator.rs` — LLM provider abstraction and NL→MQL translation
- `src/compiled/validator.rs` — MQL safety validation (whitelist operations, syntax check)
- `src/compiled/template.rs` — parameterized query templates (extract variable slots)
- `src/compiled/cache/mod.rs` — cache hierarchy coordinator
- `src/compiled/cache/memory.rs` — L1 in-memory cache (LRU)
- `src/compiled/cache/disk.rs` — L2 local disk persistence
- `src/compiled/cache/atlas.rs` — L3 Atlas collection sync (read on startup, write on new compilations)
- `src/compiled/providers/mod.rs` — LLM provider trait
- `src/compiled/providers/claude.rs` — Anthropic Claude API provider
- `src/compiled/providers/openai.rs` — OpenAI API provider
- `src/compiled/invalidation.rs` — schema-change detection, TTL, manual flush
- `tests/unit/hasher_test.rs` — hash determinism tests
- `tests/unit/validator_test.rs` — MQL validation tests (safe/unsafe queries)
- `tests/unit/template_test.rs` — parameter extraction tests
- `tests/integration/compiled_test.rs` — end-to-end compilation + caching tests

**Key decisions:**
- Hash is computed from: normalized intent string + collection name + schema fingerprint (if schema exists). Same intent on different collections produces different hashes.
- LLM prompt includes: intent string, collection schema (if available), sample documents (first 3), available indexes. This gives the LLM enough context to generate optimal MQL.
- Validator whitelist: allow `$match`, `$project`, `$sort`, `$limit`, `$skip`, `$group`, `$lookup`, `$unwind`, `$vectorSearch`, `$search`. Block: `$out`, `$merge` (to unexpected targets), `$collStats`, system commands.
- Templates support parameter slots: `"headphones under $50"` compiles to a template where `$50` is a parameter. `"headphones under $100"` reuses the same template with different parameter binding.
- Atlas sync: on startup, pull all compiled queries for this cluster from `__mongocore.compiled_queries` collection. On new compilation, write to Atlas asynchronously.
- Invalidation: watch for `collMod`, `createIndexes`, `dropIndexes` events. If schema changes, mark affected compiled queries as stale.

**Testing approach:**
- Unit tests for hashing determinism, validator pass/fail cases, template parameter extraction.
- Integration tests with a mock LLM provider (returns known MQL for known intents) to verify full flow.
- Cache hierarchy tests: verify L1→L2→L3 promotion and retrieval.

---

## Subsystem 5: Voyage AI & Vector Search

**Goal:** Integrate Voyage AI for automatic embedding generation and reranking. Provide first-class `search()` and `vector_search()` methods with graceful fallback.

**Files to create:**
- `src/voyage/mod.rs` — Voyage AI client orchestration
- `src/voyage/client.rs` — HTTP client for Voyage AI REST API (embed + rerank endpoints)
- `src/voyage/batch.rs` — request batching (group multiple embed requests into single API call)
- `src/voyage/cache.rs` — embedding cache (avoid re-embedding identical strings)
- `src/search/mod.rs` — search orchestration (fallback chain)
- `src/search/vector.rs` — `$vectorSearch` pipeline construction
- `src/search/fulltext.rs` — `$search` (Atlas full-text) fallback
- `src/search/fallback.rs` — fallback chain logic (vector → fulltext → compiled → error)
- `src/auto_embed.rs` — auto-embed on write (intercept inserts/updates, embed configured fields)
- `tests/integration/voyage_test.rs` — Voyage AI API tests (with mocked HTTP)
- `tests/integration/search_test.rs` — search fallback chain tests
- `tests/integration/auto_embed_test.rs` — auto-embed on write tests

**Key decisions:**
- Voyage AI client uses async HTTP (reqwest). Batches embedding requests — collects writes for up to 10ms or 100 documents, whichever comes first, then sends one batch API call.
- Auto-embed is opt-in per collection via `configure()`. Configuration stored in local state + `__mongocore.config` collection in Atlas.
- `search()` method: uses compiled query system for intent parsing + Voyage AI for query embedding + `$vectorSearch` for retrieval + Voyage AI rerank for result quality.
- `vector_search()` method: explicit vector search. If query is a string, embed via Voyage AI. If query is a vector array, use directly.
- Fallback chain is transparent: response metadata includes `search_method` field.
- Reranking on by default for `search()`, off by default for `vector_search()` (configurable).

**Testing approach:**
- Mock Voyage AI HTTP responses for unit/integration tests.
- Fallback tests: disable Voyage AI → verify full-text fallback. Disable search indexes → verify compiled query fallback.
- Batch tests: verify multiple concurrent embeds are batched into single API calls.

---

## Subsystem 6: Language Client Libraries

**Goal:** Thin, idiomatic gRPC client wrappers for each target language. Include dev-mode sidecar auto-management (download + spawn).

**Target languages (v1):** Python, TypeScript/JavaScript, Java, Go, Rust

**Files to create per language (example: Python):**
- `clients/python/pyproject.toml` — package definition
- `clients/python/src/mongocore/__init__.py` — public API
- `clients/python/src/mongocore/client.py` — MongoCore client (connect, spawn sidecar if needed)
- `clients/python/src/mongocore/collection.py` — collection-level operations
- `clients/python/src/mongocore/sidecar.py` — sidecar binary management (download, spawn, health check)
- `clients/python/src/mongocore/generated/` — auto-generated gRPC stubs from proto
- `clients/python/tests/test_client.py` — client tests

**Key decisions:**
- Each language client is ~200-500 lines of idiomatic wrapper around generated gRPC stubs.
- Dev-mode sidecar management: on first `connect()`, check if sidecar is running (health endpoint). If not, download binary for current platform from GitHub releases (cached), spawn as subprocess, wait for health check.
- Prod-mode: client connects to a pre-configured sidecar address. No auto-spawn.
- API shape per language should feel native (e.g., Python uses snake_case, Java uses builders, etc.) while mapping to the same gRPC calls.
- Proto files are the source of truth. CI generates stubs for all languages on proto change.

**Testing approach:**
- Each language client tested against a running sidecar + MongoDB.
- Sidecar management tests: verify download, spawn, health check, and graceful shutdown.

---

## Subsystem 7: Deployment & Packaging

**Goal:** Build infrastructure for distributing the sidecar binary and container images. CI/CD pipeline for multi-platform builds.

**Files to create:**
- `Dockerfile` — multi-stage build (Rust build → minimal runtime image, ~20MB)
- `docker-compose.yml` — local development (sidecar + MongoDB replica set)
- `.github/workflows/build.yml` — CI: test + build for linux/mac/windows × amd64/arm64
- `.github/workflows/release.yml` — release: build binaries, publish container image, update client packages
- `config/default.toml` — default configuration with comments
- `config/production.toml.example` — production config example
- `scripts/install.sh` — one-line installer for the sidecar binary

**Key decisions:**
- Binary targets: linux-amd64, linux-arm64, darwin-amd64, darwin-arm64, windows-amd64.
- Container image: `ghcr.io/rozza/mongocore:latest` (distroless base, ~20MB).
- Language client packages reference the binary download URL pattern for dev-mode auto-download.
- Health endpoint at `/health` (HTTP) for readiness/liveness probes.
- Prometheus metrics at `/metrics` for observability.
- Structured JSON logging to stdout.

**Testing approach:**
- CI runs integration tests against a containerized MongoDB replica set.
- Release workflow builds all platform binaries, runs smoke tests, then publishes.

---

## Test Harness & Strategy

### Test Infrastructure

**Files to create:**
- `tests/harness/mod.rs` — shared test harness entry point
- `tests/harness/mongodb.rs` — MongoDB test container lifecycle (start/stop replica set)
- `tests/harness/sidecar.rs` — sidecar process lifecycle for integration tests (start, wait for health, stop)
- `tests/harness/grpc_client.rs` — pre-configured gRPC test client
- `tests/harness/mcp_client.rs` — pre-configured MCP (JSON-RPC) test client
- `tests/harness/mock_llm.rs` — mock LLM server (returns deterministic MQL for known intents)
- `tests/harness/mock_voyage.rs` — mock Voyage AI server (returns deterministic embeddings)
- `tests/harness/fixtures/` — test data (sample documents, expected query results, compiled query snapshots)
- `docker-compose.test.yml` — MongoDB 7.0 replica set (3 nodes) + Atlas Search emulation
- `Makefile` or `justfile` — test runner commands (`test-unit`, `test-integration`, `test-e2e`, `test-perf`)

### Test Tiers

| Tier | What | Dependencies | Speed | When to Run |
|------|------|-------------|-------|-------------|
| **Unit** | Pure logic: config parsing, hash determinism, BSON helpers, MQL validation, template extraction, fallback logic | None | <5s total | Every commit |
| **Integration** | Each subsystem against real MongoDB: CRUD, transactions, gRPC endpoints, MCP tools, cache hierarchy | MongoDB container | <60s total | Every PR |
| **End-to-End** | Full flow: language client → gRPC → sidecar → MongoDB. MCP agent → tool call → result. NL query → LLM → compiled → cached → re-executed | MongoDB + sidecar process + mock LLM + mock Voyage | <120s total | Every PR |
| **Performance** | Latency benchmarks, throughput under load, cache hit/miss ratios, connection pool behavior | MongoDB + sidecar process | ~5min | Nightly / pre-release |

### Mock vs Real Strategy

| Component | Unit Tests | Integration Tests | E2E Tests | Performance Tests |
|-----------|-----------|-------------------|-----------|-------------------|
| MongoDB | Mock (in-memory ops) | Real (container) | Real (container) | Real (container) |
| Sidecar | N/A (testing internals) | In-process (library mode) | Separate process | Separate process |
| LLM Provider | N/A | Mock server | Mock server | Mock server |
| Voyage AI | N/A | Mock server | Mock server | Mock server (or real, optional) |
| gRPC transport | N/A | Real (localhost) | Real (localhost) | Real (localhost) |
| MCP transport | N/A | Real (HTTP localhost) | Real (HTTP localhost) | N/A |

### Subsystem Test Specifications

#### Subsystem 1: Core — Test Parameters

| Test Case | Input | Expected Output | Pass Criteria |
|-----------|-------|-----------------|---------------|
| Connect to replica set | Valid connection URI | Client connected, pool initialized | Health check returns OK, startup log shows MongoDB version |
| Connect with bad URI | Invalid URI | Error returned | Specific error type, no panic, no hang |
| Opinionated defaults applied | Default client creation | Write concern = majority, read concern = majority, retryable writes = true | Inspect client options after creation |
| CRUD: insert + find | Insert doc, find by `_id` | Document returned matching insert | Exact BSON match |
| CRUD: update + verify | Update field, re-read | Updated field value | Field value matches update |
| CRUD: delete + verify | Delete doc, try to find | No document returned | Find returns None/empty |
| InsertMany (bulk) | 1000 documents | All inserted, count = 1000 | `count_documents` returns 1000 |
| FindAndModify | Find + update atomically | Returns pre-modification doc (or post, depending on option) | Returned doc matches expected state |
| Transaction: commit | Insert in txn, commit, read outside txn | Document visible | Find outside txn returns document |
| Transaction: abort | Insert in txn, abort, read outside txn | Document NOT visible | Find outside txn returns empty |
| Transaction: conflict | Concurrent writes to same doc in two txns | One succeeds, one gets write conflict | Correct error code on conflicting txn |
| Connection pool exhaustion | Open max connections, request one more | Queued or timeout error | Predictable behavior (no crash), returns within timeout |
| Aggregation pipeline | `$match` → `$group` → `$sort` | Correct aggregated results | Result matches hand-computed expected output |
| CreateCollection | Create with validator | Collection exists with validator | `listCollections` shows correct options |
| CreateIndex | Create compound index | Index exists | `listIndexes` shows the index |

#### Subsystem 2: gRPC — Test Parameters

| Test Case | Input | Expected Output | Pass Criteria |
|-----------|-------|-----------------|---------------|
| Service discovery | gRPC reflection request | All RPCs listed | All proto-defined methods present |
| Find via gRPC | FindRequest with filter | FindResponse with matching docs | Docs match direct MongoDB query |
| Insert via gRPC | InsertRequest with BSON doc | InsertResponse with inserted ID | ID matches, doc retrievable |
| Streaming: Watch | WatchRequest on collection, then insert | WatchEvent received | Event contains inserted doc, arrives within 5s |
| Streaming: cancel | Start Watch, then cancel stream | Stream closes cleanly | No error on server, no resource leak |
| Timeout enforcement | Request with 1ms deadline | DeadlineExceeded error | gRPC status code = DEADLINE_EXCEEDED |
| Invalid request | Malformed BSON bytes | InvalidArgument error | gRPC status code = INVALID_ARGUMENT, descriptive message |
| Concurrent requests | 100 parallel Find requests | All return correct results | No cross-contamination, all succeed |
| Large result set | Find returning 10,000 docs | All docs returned | Count matches, no truncation |

#### Subsystem 3: MCP — Test Parameters

| Test Case | Input | Expected Output | Pass Criteria |
|-----------|-------|-----------------|---------------|
| Tool discovery | `tools/list` JSON-RPC call | All tools listed with schemas | Tool names match expected set, schemas are valid JSON Schema |
| Resource discovery | `resources/list` JSON-RPC call | Schema, index, capability resources listed | URIs are well-formed, descriptions present |
| Tool execution: find | `call_tool("find", {collection, filter})` | Matching documents | Results match direct query |
| Read-only mode: block write | `call_tool("insert", {...})` with `--read-only` | Error: operation not permitted | Error code indicates read-only, no data written |
| Doc limit enforcement | Find matching 500 docs, limit = 100 | Only 100 returned | Response contains exactly 100 docs + truncation indicator |
| Session lifecycle | Initialize → call tools → disconnect | Session created and cleaned up | No resource leaks (check sidecar memory) |
| Interest pattern | Register pattern, insert matching doc | Notification via SSE | Notification received within 5s, contains matching doc |
| Query cost estimation | Expensive aggregation ($lookup on large collection) | Cost estimate returned before execution | Estimate includes stage analysis |

#### Subsystem 4: Compiled Queries — Test Parameters

| Test Case | Input | Expected Output | Pass Criteria |
|-----------|-------|-----------------|---------------|
| Hash determinism | Same intent + context, called twice | Same hash | Hashes are byte-identical |
| Hash uniqueness | Same intent, different collections | Different hashes | Hashes differ |
| Cold compilation | New intent string | LLM called, MQL returned, cached | Mock LLM receives request, result is valid MQL, cache entry created |
| Hot cache hit | Previously compiled intent | MQL from cache, NO LLM call | Mock LLM NOT called, response time <1ms |
| Template parameterization | "under $50" then "under $100" | Same template, different param | Only one LLM call total, both queries execute correctly |
| Validator: safe query | `[{$match: {x: 1}}]` | Passes validation | No error |
| Validator: unsafe query | `[{$out: "hackers"}]` | Blocked by validator | Specific validation error returned |
| L1→L2 persistence | Compile query, restart sidecar, re-query | Cache hit from L2 (disk) | No LLM call after restart |
| L3 Atlas sync | Compile on instance A, start instance B | Instance B has compiled query | Instance B serves from L3 without LLM call |
| Invalidation: schema change | Compile query, add index, re-query | Recompilation triggered | LLM called again, new MQL may differ |
| Manual flush | Flush cache via API | All cached queries removed | Subsequent calls trigger recompilation |

#### Subsystem 5: Voyage AI & Vector Search — Test Parameters

| Test Case | Input | Expected Output | Pass Criteria |
|-----------|-------|-----------------|---------------|
| Embed single string | "wireless headphones" | Vector (float array) | Non-zero vector of expected dimensions (e.g., 1024) |
| Batch embedding | 50 strings submitted in 10ms window | Single batch API call | Mock Voyage receives 1 request with 50 inputs |
| Embedding cache | Same string embedded twice | Second call skips API | Mock Voyage called once only |
| Auto-embed on insert | Insert doc to configured collection | Doc stored with embedding field | Document has vector field with correct dimensions |
| vector_search: string query | Query string + filter | Matching docs ranked by similarity | Results ordered by score, filter applied |
| vector_search: raw vector | Float array + filter | Matching docs | No Voyage API call (vector used directly) |
| search() full path | NL intent string | Compiled + embedded + vector searched + reranked | Response metadata shows `search_method: "vector"` |
| Fallback: no Voyage AI | Voyage unavailable, search indexes exist | Full-text search result | `search_method: "fulltext"` |
| Fallback: no indexes | No search indexes, LLM available | Compiled traditional query | `search_method: "compiled_query"` |
| Fallback: nothing | No Voyage, no indexes, no LLM | Clear error | Error message lists what's needed |
| Reranking | search() with rerank enabled | Results reordered by relevance | Order differs from raw vector similarity order |

#### Subsystem 6: Language Clients — Test Parameters

| Test Case | Input | Expected Output | Pass Criteria |
|-----------|-------|-----------------|---------------|
| Dev-mode: auto-download | First connect, no sidecar present | Binary downloaded, sidecar spawned | Health check passes, correct platform binary |
| Dev-mode: reuse running | Second connect, sidecar already running | Connects to existing | No new process spawned |
| Dev-mode: graceful shutdown | Client disconnects, idle timeout | Sidecar shuts down | Process exited cleanly |
| Prod-mode: connect to address | Pre-running sidecar at known address | Client connects | Operations work |
| CRUD via Python client | `client.db.collection.find({"x": 1})` | Matching documents | Results match direct query |
| CRUD via TypeScript client | `client.db.collection.find({x: 1})` | Matching documents | Results match direct query |
| Error propagation | Invalid operation | Language-native error/exception | Error type, message, code accessible idiomatically |
| Streaming via client | `client.db.collection.watch()` | Change events received | Events arrive as language-native async iterators/streams |

#### Subsystem 7: Deployment — Test Parameters

| Test Case | Input | Expected Output | Pass Criteria |
|-----------|-------|-----------------|---------------|
| Container builds | `docker build .` | Image <25MB | Image size check, binary runs |
| Container health | Deploy container, hit `/health` | 200 OK with capabilities | HTTP 200, JSON body with version + capabilities |
| Multi-platform binary | Build for linux-arm64 on CI | Valid ELF binary | File type check, smoke test on emulated platform |
| Startup with config | `mongocore serve --config test.toml` | Sidecar starts with configured settings | Logs show correct URI, options |
| Startup without config | `mongocore serve` (env vars only) | Sidecar starts with env vars | `MONGOCORE_URI` env var used |
| Prometheus metrics | Hit `/metrics` after some operations | Prometheus-format metrics | Contains `mongocore_operations_total`, `mongocore_latency_seconds` |
| Graceful shutdown | Send SIGTERM | Drains connections, exits 0 | In-flight operations complete, exit code 0 |

### Performance Benchmarks (Nightly)

| Benchmark | Metric | Target | Method |
|-----------|--------|--------|--------|
| gRPC find latency (single doc) | p50 / p99 | <2ms / <10ms | 10,000 sequential finds, measure distribution |
| gRPC insert throughput | ops/sec | >10,000 ops/s | Concurrent inserts for 30s |
| Compiled query cold path | time-to-first-result | <3s (dominated by LLM) | NL query on cold cache |
| Compiled query hot path | time-to-result | <2ms | NL query on warm cache |
| Change stream latency | event delay | <50ms | Insert doc, measure time until WatchEvent arrives |
| Connection pool under load | p99 latency | <50ms | 1000 concurrent requests |
| Sidecar memory (idle) | RSS | <50MB | After startup with connection pool initialized |
| Sidecar memory (under load) | RSS | <200MB | During sustained 10k ops/s |
| Auto-embed batch efficiency | API calls saved | >80% reduction vs per-doc | 1000 inserts in 1s, count Voyage API calls |

### Running Tests

```bash
# Unit tests (no dependencies)
just test-unit        # or: cargo test --lib

# Integration tests (requires Docker)
just test-integration # starts MongoDB container, runs tests, tears down

# End-to-end tests (full stack)
just test-e2e         # starts MongoDB + sidecar + mock services, runs full flow

# Performance benchmarks
just test-perf        # starts stack, runs criterion benchmarks, outputs report

# All tests
just test-all         # unit + integration + e2e (not perf, too slow for CI)
```

### CI Pipeline

```yaml
# On every push:
- just test-unit

# On every PR:
- just test-unit
- just test-integration
- just test-e2e

# Nightly:
- just test-all
- just test-perf
- Compare perf results to baseline, alert on >10% regression
```

---

## Implementation Order & Dependencies

```
Phase 1 (Foundation):
  Subsystem 1 (Core) — no dependencies

Phase 2 (Interfaces):
  Subsystem 2 (gRPC) — depends on Subsystem 1
  Subsystem 3 (MCP) — depends on Subsystem 1

Phase 3 (Intelligence):
  Subsystem 4 (Compiled Queries) — depends on Subsystems 1, 2, 3
  Subsystem 5 (Voyage AI) — depends on Subsystems 1, 2, 3, 4

Phase 4 (Distribution):
  Subsystem 6 (Language Clients) — depends on Subsystem 2
  Subsystem 7 (Deployment) — depends on Subsystem 1
```

Phases 2's subsystems can be built in parallel. Phase 4's subsystems can be built in parallel. Within each subsystem, follow TDD: write failing test → implement → pass → commit.

---

## Definition of Done (v1)

- [ ] Sidecar connects to MongoDB, enforces opinionated defaults
- [ ] All CRUD, aggregation, transaction, findAndModify, and admin operations work via gRPC
- [ ] MCP server exposes tools and resources, with safety controls
- [ ] Compiled queries: NL intent → LLM → validated MQL → cached (L1/L2/L3)
- [ ] Voyage AI: auto-embed on write, semantic search, reranking, fallback chain
- [ ] Python and TypeScript clients ship with dev-mode sidecar management
- [ ] Container image and multi-platform binaries available
- [ ] Startup logs available capabilities
- [ ] Integration test suite passes against MongoDB 7.0+ replica set
