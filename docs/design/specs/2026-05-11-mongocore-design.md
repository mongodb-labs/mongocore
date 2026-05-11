# MongoCore: AI-Native MongoDB Driver

## Overview

MongoCore is an intelligent Rust sidecar that provides a paradigm-shifting MongoDB experience: a single fast core serving all languages via gRPC, with native AI agent support via MCP, compiled natural-language queries, and seamless Voyage AI/Atlas Vector Search integration.

The goal is to be the simplest, fastest, and most correct way to consume MongoDB — for both humans and AI agents.

## Core Architecture

```
┌──────────────────┐      gRPC       ┌──────────────────────────────┐      Wire Protocol
│  App (any lang)  │────────────────▶│                              │────────────────────▶ MongoDB
└──────────────────┘                 │       MongoCore Sidecar      │
                                     │          (Rust)              │      REST API
┌──────────────────┐      MCP        │                              │────────────────────▶ Voyage AI
│   AI Agents      │────────────────▶│                              │
└──────────────────┘                 └──────────────┬───────────────┘
                                                    │
                                                    │ Pluggable LLM (NL→MQL)
                                                    ▼
                                     ┌──────────────────────────────┐
                                     │  Claude / OpenAI / Local     │
                                     └──────────────────────────────┘
```

### Design Principles

- **Opinionated defaults, escape hatch for power users** — the default path is the correct path
- **Schema is opt-in, not a gate** — connect and go instantly, add schema when you want type safety
- **AI-native from the outset** — agents are first-class consumers, not an afterthought
- **Single Rust core serves all languages** — thin gRPC stubs per language, no reimplementation
- **Compiled queries eliminate repeated AI costs** — pay once, run at native speed forever

## Dual Interface

### gRPC (Data Plane — for applications)

- Auto-generated stubs from `.proto` definitions per language
- Bidirectional streaming for change streams
- High throughput, binary serialization (protobuf)
- Connection multiplexing over HTTP/2
- Ultra-thin language clients (~100-200 lines of idiomatic wrapper)

### MCP (Agent Plane — for AI)

- JSON-RPC over Streamable HTTP
- Tool discovery (`tools/list`) — agents learn what's available at runtime
- Same operations as gRPC, optimized for discoverability and safety
- Read-only mode available, confirmation-required for writes
- Schema/collection context exposed as MCP resources
- Bidirectional: agents subscribe to change streams, receive proactive notifications

Both interfaces hit the same Rust engine. Compiled queries are shared across both planes.

## Compiled Query System

The novel core innovation. Natural language queries are translated once and cached for native-speed repeat execution.

### First Execution (Cold Path)

1. User writes: `db.products.search("wireless headphones under $50")`
2. Sidecar hashes the intent string + collection context
3. Cache miss → sends to configured LLM provider with collection schema/sample data as context
4. LLM returns MQL (aggregation pipeline with `$vectorSearch` + filters)
5. Sidecar validates the MQL is syntactically correct and safe
6. Executes against MongoDB, returns results
7. Stores compiled form: `{hash → validated MQL template}` in local cache

### Subsequent Executions (Hot Path)

1. Same intent string → hash match → execute cached MQL directly
2. Zero LLM cost, native speed
3. Parameterized templates handle variable values (e.g., "$50" → "$100")

### Cache Hierarchy

- **L1**: In-memory in the sidecar (fastest, ephemeral)
- **L2**: Local disk (survives sidecar restart)
- **L3**: Atlas collection (shared fleet-wide, new instances warm instantly)

### Cache Invalidation

- Schema change detected → invalidate affected compiled queries
- Manual flush via API/MCP tool
- TTL-based expiry (configurable, default long-lived)

### Safety

- All LLM-generated MQL passes through a validation layer before execution
- Whitelist of allowed operations (no `$out` to unexpected collections, no `dropDatabase`)
- Compiled queries are immutable once cached

## API Surface

### Operations

| Category | Methods |
|----------|---------|
| **CRUD** | `find`, `findOne`, `insert`, `insertMany`, `update`, `updateMany`, `delete`, `deleteMany` |
| **Atomic** | `findAndModify` (find+update+return in one round trip) |
| **Aggregation** | `aggregate` (pipeline), `search` (NL→compiled query) |
| **Transactions** | `beginTransaction`, `commit`, `abort` (multi-doc ACID) |
| **Vector** | `vectorSearch`, `embed` (explicit embedding via Voyage AI) |
| **Admin** | `createCollection`, `createDatabase`, `createUser`, `createIndex` |
| **Streaming** | `watch` (change streams, bidirectional via gRPC streaming) |

### Opinionated Defaults (always-on unless explicitly overridden)

- Write concern: `majority`
- Read concern: `majority`
- Retryable writes: `on`
- Retryable reads: `on`
- Read preference: `primaryPreferred`
- Automatic client-side timeout: sensible per-operation (30s queries, 60s aggregations)

### Deliberately Excluded from v1

- `mapReduce` (deprecated, aggregation covers it)
- GridFS (separate concern)
- Unconcerned writes (`w:0`)
- Raw wire protocol passthrough (v2 escape hatch)

## Voyage AI & Vector Search Integration

### Auto-Embed on Write (opt-in per collection)

```python
client.products.configure(auto_embed={"fields": ["name", "description"], "model": "voyage-3-large"})

client.products.insert({"name": "Sony WH-1000XM5", "description": "Noise cancelling headphones"})
# → Sidecar batches embedding requests to Voyage AI
# → Stores document + vector atomically
```

### Semantic Search (first-class)

```python
# Intent-based (compiled query system + Voyage AI for query embedding)
results = client.products.search("comfortable headphones for long flights")

# Explicit vector search (typed, validated)
results = client.products.vector_search(
    query="noise cancelling",
    filter={"price": {"$lt": 100}},
    limit=10
)
```

### Reranking

- After vector search returns candidates, Voyage AI reranks by relevance
- On by default for `search()`, configurable for `vector_search()`

### Batching & Efficiency

- Embedding requests batched in the sidecar (not one HTTP call per document)
- Cached embeddings for repeated query strings
- Async — writes don't block on embedding if configured for eventual consistency

### Search Fallback Chain

For `search()` (NL intent-based):
1. **Voyage AI + Atlas Vector Search available** → Full semantic search (embed → `$vectorSearch` → rerank)
2. **Voyage AI unavailable** → Falls back to Atlas full-text search (`$search`) with keyword extraction
3. **No Atlas Search indexes** → Compiled query system translates intent to traditional `$match`/text query via LLM
4. **No LLM + no search indexes** → Clear error, no silent degradation

For `vector_search()` (explicit):
- Requires either Voyage AI (to embed the query string) OR a pre-computed query vector passed directly
- If neither available → clear error explaining what's needed

Response metadata includes `search_method` ("vector" | "fulltext" | "compiled_query" | "error") for transparency.

### Startup Capability Logging

```
MongoCore v1.0 connected to cluster0.abc.mongodb.net
  ✓ Wire protocol (MongoDB 7.0)
  ✓ Voyage AI (voyage-3-large)
  ✓ Atlas Vector Search (3 indexed collections)
  ✓ LLM provider (Claude, compiled queries enabled)
  ✗ Atlas Stream Processing (not configured)
```

## AI Agent Experience (MCP)

### Tools Exposed

- `find`, `aggregate`, `insert`, `update`, `delete` — standard CRUD
- `search` — semantic/NL search (uses compiled query system)
- `vector_search` — explicit vector similarity
- `watch` — subscribe to change streams
- `list_databases`, `list_collections`, `collection_schema`
- `explain` — query execution plan
- `create_index`, `create_collection`

### Resources Exposed

- Collection schemas (inferred or declared) — agent understands data shape
- Available indexes — agent writes efficient queries
- Compiled query library — agent browses previously compiled intents
- Capability report — what's available (vector, full-text, streaming)

### Safety Controls

- `--read-only` mode (default for untrusted agents)
- Write operations require confirmation by default
- Query cost estimation before execution
- Max document return limit (configurable, default 100)

### Bidirectional Flow

- Agents subscribe to change streams → notified of relevant data changes
- Agents register interest patterns: "Tell me when any order over $10k is placed" (filter-based, using existing change stream infrastructure)
- **v1.5 candidate**: Proactive context pushing ("new data matches your last N queries") — requires maintaining agent interest state, scoped out of initial v1

### Key Differentiator

This is not a wrapper around a driver — it IS the driver. Same compiled cache, same Voyage AI integration, same connection pool. Agents get the same performance as application code.

## Deployment

### Dev Mode (zero-friction)

```bash
pip install mongocore  # or npm/cargo/maven equivalent
```

- Client library auto-downloads correct sidecar binary for OS/arch
- First `connect()` spawns sidecar as subprocess
- Sidecar dies when process dies (or idles out after 60s)
- No Docker, no install steps, no config files

### Prod Mode (ops-controlled)

- Standalone binary: `mongocore serve --config production.toml`
- Kubernetes sidecar container (~20MB image, <50ms startup)
- Systemd service, ECS task, etc.
- Health endpoint, Prometheus metrics, structured logging
- One sidecar per pod/host (keeps L1 cache hot)

### Configuration (minimal)

```toml
# production.toml — only what you need to override
connection_uri = "mongodb+srv://..."
llm_provider = "claude"
llm_api_key_env = "ANTHROPIC_API_KEY"
voyage_api_key_env = "VOYAGE_API_KEY"
compiled_cache_sync = true
```

Everything else has safe defaults.

## Versioning & Roadmap

### v1 — Core

- Rust sidecar with gRPC data plane + MCP agent plane
- Opinionated defaults, lean API surface
- Compiled query system (pluggable LLM provider)
- Voyage AI integration (auto-embed, vector search, reranking)
- Dev mode (auto-managed) + Prod mode (standalone)
- Change stream support via gRPC streaming + MCP subscriptions
- Schema opt-in (`mongocore generate`)
- Startup capability logging
- Compiled cache: local L1/L2 + Atlas-synced L3

### v2 — Power Users & Operations

- Raw wire protocol escape hatch for power users
- Query analytics dashboard (most-used compiled queries, performance insights)
- Multi-tenant support (shared sidecar, isolated caches per tenant)

### v3 — Intelligent Data Ingestion

- LLM-powered data cleaning and transformation from external sources (CSV, JSON, APIs, etc.)
- Schema inference and mapping: LLM identifies how source data maps to target collections
- Multi-process parallel ingestion pipeline (Rust core spawns workers for throughput)
- Automatic deduplication, normalization, and validation
- Conflict resolution strategies (merge, overwrite, skip) guided by LLM understanding of data semantics
- Compiled transforms: like compiled queries, first ingestion of a source format pays the LLM cost, subsequent runs reuse the cached transform pipeline
- Progress tracking, resumable imports, dead letter queue for failures

### v4 — Migration & Ecosystem

- Language-idiomatic wrapper APIs per language (e.g., `collection.find({...})` syntax that feels native, not protobuf-generated)
- Framework adapters: thin backends for Mongoose, Spring Data MongoDB, Mongoid, Motor — use MongoCore as transport without rewriting apps
- Compatibility layer: drop-in replacement for existing driver APIs where possible
- Performance benchmarking and documentation: quantify gains from Rust core handling connection pooling, BSON serialization, and compression vs. interpreted-language drivers

### v5 — Self-Contained AI

- Local NL→MQL model (bundled, no external LLM dependency)
- Alternatively, Atlas-hosted NL→MQL service (zero client-side model management)

### v6 — WASM & Extensibility

- Browser client (Rust core compiled to WASM, sidecar as transport)
- Edge deployment (WASI on Cloudflare Workers, etc.)
- WASM plugin system (query transformers, validators, middleware)
- Hot-reloadable plugins, language-agnostic
