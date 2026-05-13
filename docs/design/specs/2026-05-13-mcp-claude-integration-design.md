# MCP + Claude Integration — Intelligent Data Companion

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.
> If modifying client libraries: verify imports work and run `just test-clients`.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

## Summary

Transform MongoCore's MCP server from a basic CRUD tool into an **intelligent data companion** for Claude. Users point Claude at their MongoDB instance via MongoCore, and Claude can: ask natural language questions about their data, generate ready-to-run application code in the user's language, explore schemas intelligently, and perform semantic search via an integrated embedding pipeline.

The key differentiator: **zero LLM configuration for Claude users**. MongoCore uses MCP sampling to leverage the host LLM (Claude itself) for NL→MQL compilation when no API key is configured, while the template cache ensures subsequent similar queries need no LLM call at all.

## Goals

1. Claude can answer questions about a user's MongoDB data using natural language
2. Claude can generate typed, runnable MongoCore client code in the user's project language
3. Claude can ingest documents, embed them, and perform semantic search in one flow
4. Zero-config experience for Claude users (no separate LLM API key needed)
5. Pre-built binary distribution (Homebrew, GitHub Releases) for instant setup
6. Graceful degradation when components are unavailable

## Non-Goals

- Atlas cloud management (cluster ops, scaling) — separate concern
- Change stream subscriptions via MCP — future work
- Schema migration planning — future work
- Replacing the gRPC sidecar mode — MCP and gRPC coexist

## Architecture

```
Claude (host LLM)
    │
    ├── MCP stdio/UDS transport
    │
    ▼
MongoCore MCP Server
    ├── Tool Dispatch Layer
    │   ├── Data Exploration (existing, enhanced)
    │   ├── Ask Your Data (new — NL→MQL→execute→answer)
    │   ├── Code Generation (new — query→client code)
    │   ├── Embedding Pipeline (new — ingest→embed→search)
    │   └── Insights (enhanced — index suggestions, slow queries)
    │
    ├── Compiled Query Engine (existing v0.6)
    │   ├── Template Registry + Cache
    │   ├── Intent Router (find/aggregate/vector/fulltext/geo)
    │   └── LLM Provider (configured key OR MCP sampling fallback)
    │
    ├── Polars Ingestion Engine (existing v0.3)
    ├── Voyage AI Embedding Client (existing v0.1)
    ├── Analytics Collector (existing v0.2)
    └── MongoDB Connection Pool
```

### LLM Strategy: Hybrid

1. **If LLM API key configured** → use it directly (current behavior, for standalone use)
2. **If no key + running as MCP server** → use MCP sampling to request the host LLM (Claude) to generate MQL
3. **Template cache** works the same either way — after first compilation, no LLM needed for repeat patterns

This means Claude users get full NL→MQL capability with zero configuration.

## Tool Surface

### Group 1: Data Exploration (existing + enhanced)

| Tool | Status | Purpose |
|------|--------|---------|
| `list_databases` | Existing | List available databases |
| `list_collections` | Existing | List collections in a database with document counts |
| `find` | Existing | Query with filter, projection, sort, limit, skip |
| `find_one` | Existing | Single document lookup |
| `aggregate` | Existing | Run aggregation pipeline |
| `collection_schema` | **New** | Sample N documents, infer field names/types/cardinality, report schema |

**`collection_schema` details:**
- Input: `database`, `collection`, `sample_size` (default 100)
- Output: field paths, BSON types per field, % documents containing field, example values
- Uses existing sampling logic from ingestion schema inference

### Group 2: Ask Your Data (new)

| Tool | Purpose |
|------|---------|
| `ask` | NL question → compile to MQL → execute → return answer + query + confidence |
| `explain_query` | NL question → compile to MQL → return query plan + index usage (no execution) |

**`ask` tool:**
- Input: `question` (string), `database` (string), `collection` (optional — auto-detect if omitted)
- Output:
  ```json
  {
    "answer": "42 restaurants in Brooklyn have grade A",
    "documents": [...],
    "query": { "method": "find", "filter": {...}, "options": {...} },
    "confidence": 0.95,
    "execution_time_ms": 12,
    "from_cache": true
  }
  ```
- Flow: check template cache → hit: substitute params and execute → miss: compile via LLM (configured or MCP sampling) → cache template → execute
- If collection omitted: list collections, include names in LLM prompt for routing

**`explain_query` tool:**
- Input: `question` (string), `database` (string), `collection` (optional)
- Output: compiled MQL + MongoDB explain plan + index recommendations
- Does NOT execute the query — safe for expensive operations

### Group 3: Code Generation (new)

| Tool | Purpose |
|------|---------|
| `generate_code` | Given a query (NL or MQL) + language → produce ready-to-run MongoCore client code |
| `generate_model` | Given a collection → produce typed data model in target language |
| `generate_index` | Analyze a query pattern → suggest and generate index creation code |

**`generate_code` details:**
- Input: `query` (NL string or MQL object), `database`, `collection`, `language` (optional — auto-detect)
- Output:
  ```json
  {
    "code": "...",
    "language": "python",
    "dependencies": ["mongocore-client>=0.1.0"],
    "query_used": { "method": "find", "filter": {...} },
    "explanation": "Finds restaurants matching the criteria and prints results"
  }
  ```
- Language auto-detection checks workspace for:
  - `pyproject.toml` / `requirements.txt` / `setup.py` → Python
  - `package.json` / `tsconfig.json` → TypeScript
  - `go.mod` → Go
  - `pom.xml` / `build.gradle` / `build.gradle.kts` → Java
- Code generation is **template-based** — MongoCore has Handlebars/Tera templates per language that map MQL operation types (find, aggregate, vector_search, etc.) to the corresponding MongoCore client API calls. No LLM needed for code generation itself.

**`generate_model` details:**
- Input: `database`, `collection`, `language` (optional), `sample_size` (default 100)
- Output: typed model definition:
  - Python → Pydantic `BaseModel` or `TypedDict`
  - TypeScript → `interface`
  - Go → `struct` with BSON tags
  - Java → `record` or class with Jackson annotations
- Uses schema inference from `collection_schema` internally

**`generate_index` details:**
- Input: `query` (NL or MQL), `database`, `collection`, `language` (optional)
- Output: recommended index + code to create it + explanation of why

### Group 4: Embedding Pipeline (new)

| Tool | Purpose |
|------|---------|
| `embed_and_store` | Take text/documents → embed via Voyage AI → store with vectors |
| `semantic_search` | NL query → embed → $vectorSearch → return ranked results |
| `ingest_and_embed` | File → parse (Polars) → embed specified field → store with vectors |

**`embed_and_store` details:**
- Input: `documents` (array of objects), `database`, `collection`, `embed_field` (which field to embed), `model` (Voyage model, default "voyage-2")
- Batch embeds the specified field using Voyage AI
- Stores documents with `_embedding` vector field appended
- Returns: count stored, embedding dimensions

**`semantic_search` details:**
- Input: `query` (string), `database`, `collection`, `limit` (default 10), `filter` (optional pre-filter)
- Embeds query via Voyage AI → runs `$vectorSearch` → returns ranked documents with scores
- Requires: Voyage AI key configured, vector search index exists on collection

**`ingest_and_embed` details:**
- Input: `file_path`, `database`, `collection`, `embed_field`, `format` (optional — auto-detect), `transforms` (optional Polars expressions)
- Combines: Polars file parsing → batch embedding → bulk storage
- Returns: documents ingested, embeddings generated, time elapsed

### Group 5: Insights (enhanced)

| Tool | Status | Purpose |
|------|--------|---------|
| `get_analytics` | Existing | Operation counts, error rates, p50/p95/p99 latencies |
| `suggest_indexes` | **New** | Analyze recent query patterns, recommend missing indexes |
| `slow_queries` | **New** | Surface queries above p95 latency with optimization suggestions |

**`suggest_indexes` details:**
- Input: `database`, `collection` (optional — all collections if omitted)
- Analyzes analytics ring buffer for repeated filter patterns without supporting indexes
- Returns: recommended indexes with impact estimate and creation code

**`slow_queries` details:**
- Input: `database` (optional), `threshold_ms` (optional, default p95)
- Returns: slowest queries with their MQL, frequency, avg latency, and optimization suggestions

## MCP Resources (enhanced)

| URI | Purpose |
|-----|---------|
| `mongocore://capabilities` | Existing — server features |
| `mongocore://databases` | Existing — database list |
| `mongocore://collections/{database}` | Existing — collection list |
| `mongocore://schema/{database}/{collection}` | **New** — cached schema for collection |

## MCP Sampling Integration

When MongoCore detects no LLM API key is configured and it's running as an MCP server:

1. During `initialize`, it advertises `sampling` in its `clientCapabilities` requirements
2. When `ask` or `explain_query` need NL→MQL compilation (cache miss):
   - MongoCore sends a `sampling/createMessage` request to the host
   - Prompt includes: schema context, available methods, few-shot examples
   - Host (Claude) returns MQL + template
   - MongoCore caches the template as usual
3. Subsequent similar queries use the cached template — no sampling needed

### Prompt sent via MCP sampling

```
Given this MongoDB collection schema:
{database}.{collection}: {schema_summary}

Translate this natural language query to MQL:
"{user_question}"

Respond with JSON: { "method": "find|aggregate|...", "query": {...}, "template": {...} }
```

## Degraded Mode Behavior

| Condition | Available | Unavailable | User-facing message |
|-----------|-----------|-------------|---------------------|
| Full (MongoDB + LLM/sampling + Voyage) | All tools | — | — |
| No LLM + no cache | Exploration, codegen, embedding, insights | `ask`, `explain_query` return suggestion to use `find`/`aggregate` directly | "NL queries require an LLM. Configure ANTHROPIC_API_KEY or use within Claude." |
| No Voyage AI key | All except embedding | `embed_and_store`, `semantic_search`, `ingest_and_embed` | "Embedding requires VOYAGE_API_KEY configuration" |
| MongoDB unreachable | `generate_code`, `generate_model` (from last cached schema) | All data tools | "Cannot connect to MongoDB at {uri}" |

Error responses include a `suggestion` field so Claude can recover:
```json
{
  "isError": true,
  "error_type": "llm_unavailable",
  "message": "No LLM configured and MCP sampling not available",
  "suggestion": "Use 'find' or 'aggregate' tools directly, or configure ANTHROPIC_API_KEY",
  "recoverable": true
}
```

## Packaging & Distribution

### Installation

| Method | Command | Target |
|--------|---------|--------|
| Homebrew | `brew install mongocore` | macOS users |
| GitHub Releases | Download binary | All platforms |
| Cargo | `cargo install mongocore` | Rust developers |
| Source | `cargo build --release` | Contributors |

### Binary variants

- `mongocore-darwin-arm64` (Apple Silicon)
- `mongocore-darwin-x86_64` (Intel Mac)
- `mongocore-linux-x86_64` (Linux)
- `mongocore-linux-arm64` (Linux ARM)

### MCP Configuration (for Claude Desktop / Claude Code)

Minimal (local MongoDB, no config):
```json
{
  "mcpServers": {
    "mongocore": {
      "command": "mongocore",
      "args": ["--stdio"]
    }
  }
}
```

With connection string:
```json
{
  "mcpServers": {
    "mongocore": {
      "command": "mongocore",
      "args": ["--stdio", "--connection-uri", "mongodb+srv://user:pass@cluster.mongodb.net/mydb"]
    }
  }
}
```

With Voyage AI for embedding:
```json
{
  "mcpServers": {
    "mongocore": {
      "command": "mongocore",
      "args": ["--stdio", "--connection-uri", "mongodb://localhost:27017"],
      "env": {
        "VOYAGE_API_KEY": "your-key"
      }
    }
  }
}
```

### Connection URI priority

1. `--connection-uri` CLI flag
2. `MONGODB_URI` environment variable
3. `connection_uri` in config TOML
4. `mongodb://localhost:27017` (default)

### Stdio mode behavior

When launched with `--stdio`:
- MCP JSON-RPC messages on stdin/stdout
- Logs to stderr only
- gRPC server NOT started (MCP-only mode)
- Advertises MCP sampling capability in `initialize` response

### Future: UDS transport

UDS (Unix Domain Socket) support is in development. When available:
```json
{
  "mcpServers": {
    "mongocore": {
      "transport": "uds",
      "socket": "/tmp/mongocore.sock"
    }
  }
}
```

The tool surface and capabilities are transport-agnostic — same tools work over stdio, UDS, or HTTP.

## Testing Strategy

### Unit tests (no dependencies)

- Template-based code generation: verify output for each language × each query type
- Schema inference → model generation pipeline
- MCP sampling request/response serialization
- Language auto-detection logic
- Error response formatting

### Integration tests (Docker MongoDB required)

- `ask` tool end-to-end: NL question → MQL → execute → answer (with mock LLM or test templates)
- `collection_schema` against sample_restaurants and sample_mflix
- `generate_model` against real collections
- `suggest_indexes` with known query patterns
- Embedding pipeline (requires Voyage AI key or mock)

### MCP protocol tests

- Stdio transport: spawn MongoCore as child process, send JSON-RPC over stdin, verify stdout responses
- Sampling flow: mock host returning MQL, verify MongoCore caches template
- Graceful degradation: verify correct errors when components unavailable
- Tool discovery: verify all new tools appear in `tools/list`
- Prompts: verify `prompts/list` returns all skills, `prompts/get` returns valid workflow

### Skill tests

- Each skill definition parses correctly from TOML
- `list_skills` returns all skills with correct categories
- `get_skill` returns structured workflow with valid step descriptions
- Skill arguments are validated (required fields, types)
- Skills reference only tools that exist in the tool registry

## Skills System (MCP Prompts + Tools)

MongoCore ships with a library of **guided workflows** (skills) — structured, multi-step processes that combine tool calls into repeatable recipes. Skills are exposed via two mechanisms:

1. **MCP Prompts** (`prompts/list`, `prompts/get`) — native MCP protocol, shown in Claude Desktop UI
2. **Skill tools** (`list_skills`, `get_skill`) — fallback for clients that don't support prompts

Each skill returns a structured prompt that guides Claude through a workflow, calling MongoCore tools at each step. The skill provides the **reasoning framework** while the tools provide the **data access**.

### Skill Library

| Category | Skill | What it guides Claude through |
|----------|-------|-------------------------------|
| **Database Workflows** | `setup_collection` | Design schema → create collection → create indexes → generate model code |
| | `build_search_pipeline` | Analyze data → choose search type (vector/fulltext/hybrid) → create indexes → test queries → generate code |
| | `debug_slow_query` | Identify slow ops → explain plans → suggest indexes → verify improvement |
| | `design_schema` | Understand use case → propose schema → consider access patterns → create with indexes |
| **Code Scaffolding** | `bootstrap_project` | Detect language → install client → configure connection → verify with ping → generate example code |
| | `add_crud_endpoint` | Inspect collection schema → generate model → generate CRUD operations → generate tests |
| | `add_vector_search` | Identify embed field → configure Voyage AI → create embeddings → create index → generate search code |
| **Data Analysis** | `explore_dataset` | List collections → sample schema → compute stats → summarize findings |
| | `find_anomalies` | Aggregate stats → identify outliers → report with examples |
| | `collection_health` | Check indexes → analyze query patterns → identify missing indexes → report |
| **Operations** | `migration_check` | Compare schemas across envs → identify drift → suggest migrations |
| | `optimize_performance` | Review analytics → identify bottlenecks → suggest indexes + query rewrites |
| | `data_ingestion_pipeline` | Analyze file → configure transforms → ingest → verify → optionally embed |

### Skill Structure

Each skill is defined as a TOML file in the MongoCore binary (compiled in):

```toml
[skill]
name = "explore_dataset"
description = "Systematically explore a MongoDB database to understand its structure, relationships, and content"
category = "data_analysis"

[[skill.arguments]]
name = "database"
required = true
description = "Database to explore"

[[skill.arguments]]
name = "focus"
required = false
description = "Optional specific question to answer about the data"

[[skill.steps]]
description = "List all collections and their document counts"
tool = "list_collections"
maps_input = { database = "database" }

[[skill.steps]]
description = "Sample schema from each collection (top 5 by size)"
tool = "collection_schema"
repeat_for = "collections"
maps_input = { database = "database", collection = "item.name" }

[[skill.steps]]
description = "Compute key statistics (total docs, avg doc size, date ranges)"
tool = "aggregate"
dynamic = true  # Claude constructs the pipeline based on schema findings

[[skill.steps]]
description = "Identify cross-collection relationships (shared field names, ObjectId references)"
analysis = true  # Claude reasons about the schemas, no tool call needed

[[skill.steps]]
description = "Summarize: collections, key entities, relationships, size, and notable patterns"
synthesis = true  # Claude produces final summary
```

### MCP Prompt Response Format

When Claude requests a skill via `prompts/get`:

```json
{
  "description": "Systematically explore a MongoDB database",
  "messages": [
    {
      "role": "assistant",
      "content": {
        "type": "text",
        "text": "I'll explore this database systematically:\n\n**Step 1:** List all collections and their sizes\n**Step 2:** Sample schemas from each collection\n**Step 3:** Compute key statistics\n**Step 4:** Identify cross-collection relationships\n**Step 5:** Summarize findings\n\nStarting now..."
      }
    }
  ]
}
```

The prompt gives Claude a structured plan. Claude then executes each step by calling the appropriate MongoCore tools, reasoning about intermediate results, and adapting the remaining steps based on what it finds.

### Skill + Tool Synergy

Skills orchestrate tools into coherent workflows:

```
User: "Help me add vector search to my app"

Claude invokes skill: add_vector_search(database="mydb", collection="articles")

Skill guides Claude through:
  1. collection_schema("mydb", "articles") → finds 'content' text field
  2. Ask user: "Embed the 'content' field?" → yes
  3. embed_and_store(documents=sample, embed_field="content") → test embedding
  4. semantic_search(query="test query", collection="articles") → verify it works
  5. generate_code(query="semantic search for articles", language=auto) → produce app code
  6. Return: working code + explanation + next steps
```

### Composable Skill Recommendations (Framework Awareness)

MongoCore doesn't try to encode every framework's patterns. Instead, when code generation detects a framework, it **recommends combining with an appropriate framework skill** and provides the MongoDB-specific pieces (query, model, client code) for that skill to consume.

**Principle:** MongoCore is the data expert, not the framework expert. It delegates framework knowledge to either a specialized skill (if available) or the LLM's general knowledge (via MCP sampling fallback).

**Stack detection** (extends language detection):

| Signal | Detected Framework |
|--------|-------------------|
| `fastapi` in pyproject.toml/requirements.txt | FastAPI |
| `django` in requirements | Django |
| `flask` in requirements | Flask |
| `express` in package.json | Express |
| `next` in package.json | Next.js |
| `spring-boot` in pom.xml/build.gradle(.kts) | Spring Boot |
| `gin-gonic/gin` in go.mod | Gin |
| `chi` in go.mod | Chi |

**Recommendation flow:**

```
User: "Add an endpoint to search restaurants by cuisine"

MongoCore detects: FastAPI (from pyproject.toml)

generate_code response:
{
  "code": "# MongoCore query for restaurant search\nasync def search_restaurants(cuisine: str):\n    async with MongoCore('localhost:50051') as client:\n        return await client.find(\n            database='sample_restaurants',\n            collection='restaurants',\n            filter={'cuisine': cuisine}\n        )",
  "language": "python",
  "framework_detected": "fastapi",
  "recommendation": "Combine with a FastAPI skill to generate a complete route handler with request validation, error responses, and OpenAPI docs.",
  "provided_components": {
    "query": { "method": "find", "filter": {"cuisine": "$cuisine"} },
    "model": "Restaurant(name: str, cuisine: str, borough: str, ...)",
    "client_call": "client.find(database=..., collection=..., filter=...)"
  },
  "dependencies": ["mongocore-client>=0.1.0"]
}
```

**Fallback (no framework skill available):**

When no matching framework skill is in the session, MongoCore uses MCP sampling to ask Claude:

> "Generate a {framework} endpoint that uses the following MongoCore client call. Match the user's existing code style from their project."

Claude's general knowledge of FastAPI/Spring/Express/etc. handles the framework conventions. This means:
- New frameworks don't require MongoCore updates
- The recommendation is informational, not blocking
- Generated code is always usable even without the recommended skill

### `list_skills` / `get_skill` Tools

For MCP clients that don't support the prompts protocol:

**`list_skills`:**
- Input: `category` (optional filter)
- Output: array of `{ name, description, category, arguments }`

**`get_skill`:**
- Input: `name`, plus skill-specific arguments
- Output: structured workflow description (same content as MCP prompt response)

## Implementation Phases

### Phase 1: Foundation (stdio + enhanced tools)
- Add `--stdio` flag and stdio transport
- Implement `collection_schema` tool
- Implement `ask` and `explain_query` (using existing compiled query engine)
- MCP sampling integration for zero-config LLM

### Phase 2: Code Generation
- Language auto-detection
- Code generation templates for all 4 languages
- `generate_code`, `generate_model`, `generate_index` tools

### Phase 3: Embedding Pipeline
- Wire Voyage AI into MCP tools
- `embed_and_store`, `semantic_search`, `ingest_and_embed` tools
- Auto vector index creation

### Phase 4: Skills System
- Skill definition format (TOML, compiled into binary)
- MCP Prompts protocol support (`prompts/list`, `prompts/get`)
- `list_skills` and `get_skill` tools (fallback)
- Initial skill library: `explore_dataset`, `bootstrap_project`, `setup_collection`, `add_vector_search`
- Remaining skills: `debug_slow_query`, `design_schema`, `build_search_pipeline`, `add_crud_endpoint`, `find_anomalies`, `collection_health`, `optimize_performance`, `data_ingestion_pipeline`, `migration_check`

### Phase 5: Insights
- `suggest_indexes` tool (analyze analytics buffer)
- `slow_queries` tool
- Schema resource caching

### Phase 6: Packaging
- GitHub Actions for cross-platform binary builds
- Homebrew formula
- `cargo install` support
- Documentation and getting-started guide

## Success Criteria

1. A user can install MongoCore via Homebrew, add 3 lines to their Claude MCP config, and immediately ask questions about their MongoDB data
2. `ask` answers correctly for the sample_restaurants dataset without any API key configuration (uses MCP sampling)
3. `generate_code` produces compilable/runnable code that a user can paste into their project
4. The embedding pipeline takes a JSON file from ingest to semantic search in a single conversation
5. Template cache ensures repeated similar questions don't require any LLM call
6. Skills appear in Claude Desktop's prompt selector and guide Claude through multi-step workflows without user hand-holding
7. `explore_dataset` skill produces a comprehensive database summary by orchestrating 4+ tool calls automatically
