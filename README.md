<p align="center">
  <img src="assets/mongocore-header.svg" alt="MongoCore" width="100%"/>
</p>

# MongoCore

An AI-native MongoDB driver implemented as a lightweight Rust sidecar. MongoCore provides a single, fast core that serves all languages via gRPC, with native AI agent support via MCP (Model Context Protocol).

## Key Features

- **Rust sidecar architecture** - One fast core serves every language via gRPC
- **AI-native from the outset** - MCP interface for AI agents alongside gRPC for applications
- **Compiled queries** - Natural language queries translated once by an LLM, cached and reused at native speed
- **Voyage AI integration** - Embeddings, semantic vector search, and automatic fallback chain
- **Atlas Search & Vector Search** - Full-text and vector search with `readConcern:local` handled automatically
- **Change streams** - Real-time Watch with auto-close semantics in all languages
- **Polyglot clients** - Python, TypeScript, Go, and Java libraries with idiomatic APIs
- **Opinionated defaults** - Majority write/read concern, retryable operations, sensible timeouts

## Documentation

Full documentation with language-specific examples is available in the [`docs/`](./docs/) folder:

| Guide | Description |
|-------|-------------|
| [Getting Started](./docs/getting-started.md) | Installation, configuration, running MongoCore |
| [CRUD Operations](./docs/crud-operations.md) | Find, insert, update, delete in all languages |
| [Aggregation](./docs/aggregation.md) | Pipeline operations and common patterns |
| [Transactions](./docs/transactions.md) | Multi-document ACID transactions |
| [Search](./docs/search.md) | Vector search, full-text search, fallback chains |
| [Compiled Queries](./docs/compiled-queries.md) | Natural language to MQL with caching |
| [Change Streams](./docs/streaming.md) | Real-time notifications via Watch |
| [Admin Operations](./docs/admin-operations.md) | Collections, indexes, introspection |
| [MCP Server](./docs/mcp-server.md) | AI agent integration via JSON-RPC |
| [Client Libraries](./docs/client-libraries.md) | Language-specific setup and API reference |

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
- [Docker](https://docs.docker.com/get-docker/) (for integration tests)
- [just](https://github.com/casey/just) (task runner, optional)

### Build & Run

```bash
git clone https://github.com/rozza/mongocore.git
cd mongocore
cargo build --release

# Run with defaults (MongoDB on localhost:27017, gRPC :50051, MCP :3000)
cargo run

# Or with a config file
cargo run -- --config config.toml
```

### Connect from Your Language

**Python:**
```python
from mongocore import MongoClient

async with MongoClient("localhost:50051") as client:
    users = client["myapp"]["users"]
    await users.insert_one({"name": "Alice", "age": 30})
    docs = await users.find({"age": {"$gte": 25}})

    # Change streams with auto-close
    async with users.watch() as stream:
        async for event in stream:
            print(event["operation_type"], event["document"])
```

**TypeScript:**
```typescript
import { MongoClient } from '@mongocore/client';

const client = new MongoClient('localhost:50051');
await client.connect();
const users = client.db('myapp').collection('users');
await users.insertOne({ name: 'Alice', age: 30 });

// Change streams with auto-dispose
await using stream = users.watch();
for await (const event of stream) {
  console.log(event.operationType, event.document);
}
```

**Go:**
```go
client := mongocore.NewClient("localhost:50051")
client.Connect(ctx)
users := client.Database("myapp").Collection("users")
users.InsertOne(ctx, bson.D{{Key: "name", Value: "Alice"}, {Key: "age", Value: 30}})

// Change streams with io.Closer
cs, _ := users.Watch(ctx, nil)
defer cs.Close()
for {
    event, err := cs.Next()
    if err != nil { break }
    fmt.Println(event.OperationType, event.Document)
}
```

**Java:**
```java
try (MongoClient client = MongoClient.create("localhost:50051")) {
    MongoCollection users = client.getDatabase("myapp").getCollection("users");
    users.insertOne(new Document("name", "Alice").append("age", 30));

    // Change streams with AutoCloseable
    try (ChangeStream stream = users.watch()) {
        for (ChangeEvent event : stream) {
            System.out.println(event.getOperationType() + ": " + event.getDocument());
        }
    }
}
```

## Configuration

MongoCore uses layered configuration (CLI args > environment variables > TOML file > defaults):

```toml
# config.toml
connection_uri = "mongodb://localhost:27017"
grpc_port = 50051
mcp_port = 3000
log_level = "info"
compiled_cache_sync = true

# Optional: LLM provider for compiled queries
llm_provider = "anthropic"
llm_api_key_env = "ANTHROPIC_API_KEY"

# Optional: Voyage AI for embeddings
voyage_api_key_env = "VOYAGE_API_KEY"
```

See [Getting Started](./docs/getting-started.md) for full configuration reference.

## Testing

### Test Configuration

Copy the example config to create your local test configuration:

```bash
cp config.test.toml.example config.test.toml
```

Edit `config.test.toml` to enable AI features for testing:

```toml
# Uncomment and set to test compiled queries
llm_provider = "anthropic"
llm_api_key_env = "ANTHROPIC_API_KEY"

# Uncomment and set to test vector search
voyage_api_key_env = "VOYAGE_API_KEY"
```

Then run with:

```bash
cargo run -- --config config.test.toml
```

> **Note:** `config.test.toml` is gitignored since it may contain API key references. Only the `.example` template is committed.

### Unit Tests

No external dependencies required:

```bash
cargo test --lib
# or
just test-unit
```

### Integration Tests

Integration tests run against `mongodb/mongodb-atlas-local`, which provides Atlas Vector Search, Atlas Search, and replica set support locally:

```bash
# Start test MongoDB (Atlas Local with vector search, Atlas Search, replica set)
just docker-up
# or: docker compose -f docker-compose.test.yml up -d

# Run Rust integration tests (CRUD, search, transactions, admin)
just test-integration
# or: cargo test --test integration

# Stop test MongoDB
just docker-down
# or: docker compose -f docker-compose.test.yml down
```

The Rust integration suite includes:
- CRUD operations, aggregation, transactions
- Atlas Vector Search end-to-end (with pre-computed embeddings)
- Atlas Full-Text Search end-to-end
- Search fallback chain (vector → fulltext → filter)
- Change streams (Watch)
- Admin operations (indexes, collections, databases)

### Client Integration Tests

Client integration tests require the MongoCore sidecar running:

```bash
# Start the sidecar
cargo run -- --config config.test.toml &

# Run all client integration tests (Python, TypeScript, Go, Java)
just test-clients

# Or run individually
just test-python
just test-typescript
just test-go
just test-java
```

Each client has 10 integration tests covering: insert, insertMany, findOne, updateOne, deleteOne, deleteMany, aggregate, findWithLimit, watch (change streams), and listDatabases.

### Run Everything

```bash
# Rust tests (94 unit + 53 integration) + all client tests (40 total)
just test-all
```

### Docker

Build and run MongoCore as a container:

```bash
# Build
docker build -t mongocore .

# Run
docker run -p 50051:50051 -p 3000:3000 mongocore \
  --connection-uri "mongodb://host.docker.internal:27017"
```

## Opinionated Defaults

MongoCore enforces safe defaults that eliminate common footguns:

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
│   ├── main.rs              # Entry point, banner, startup
│   ├── config.rs            # Layered config (CLI + env + TOML)
│   ├── connection/          # Connection pool, capability detection
│   ├── operations/          # CRUD, aggregation, transactions, admin
│   ├── grpc/                # gRPC server (tonic) — 19 RPCs
│   ├── mcp/                 # MCP server (axum) — JSON-RPC tools & resources
│   ├── compiled/            # NL→MQL translation, 3-level cache, templates
│   ├── search/              # Vector search, full-text, fallback chain
│   └── voyage/              # Voyage AI REST client, batch embeddings
├── proto/                   # Protobuf service definitions (19 RPCs)
├── clients/
│   ├── python/              # Python client (async, BSON-native, change streams)
│   ├── typescript/          # TypeScript/Node.js client (AsyncDisposable streams)
│   ├── go/                  # Go client (io.Closer streams)
│   └── java/                # Java client (AutoCloseable, try-with-resources)
├── docs/                    # Comprehensive documentation
├── tests/                   # Integration tests (search, CRUD, transactions, watch)
├── docker-compose.test.yml  # Atlas Local for testing (vector search, Atlas Search)
├── Dockerfile               # Multi-stage production build
└── justfile                 # Task runner (test-all, test-clients, docker-up/down)
```

## v1 Feature Set

- **19 gRPC RPCs** — Find, FindOne, Insert, InsertMany, Update, UpdateMany, Delete, DeleteMany, FindAndModify, Aggregate, Search, BeginTransaction, CommitTransaction, AbortTransaction, CreateCollection, CreateIndex, ListDatabases, ListCollections, Watch
- **Change streams (Watch)** — Server-streaming RPC with auto-close in all clients (Python `async with`, TypeScript `AsyncDisposable`, Go `io.Closer`, Java `AutoCloseable`)
- **Search fallback chain** — Vector search → Atlas full-text → `$text` filter, with automatic fallthrough on empty results
- **Atlas Vector Search** — `$vectorSearch` with Voyage AI embeddings, tested end-to-end against Atlas Local
- **Atlas Full-Text Search** — `$search` with dynamic mappings, tested end-to-end against Atlas Local
- **Compiled queries** — NL→MQL with in-memory, disk, and Atlas caching
- **MCP server** — 13 tools for AI agent interaction with safety controls
- **Polyglot clients** — Python, TypeScript, Go, and Java with full CRUD + Watch
- **Unified test runner** — `just test-all` runs 94 unit + 53 integration + 40 client tests
- **Deployment infrastructure** — Dockerfile, GitHub Actions CI/CD, installer script

## Roadmap

| Version | Focus | Status |
|---------|-------|--------|
| **v1** | Core sidecar, gRPC + MCP interfaces, compiled queries, Voyage AI, change streams | **Complete** |
| **v2** | Power user features, query analytics, multi-tenant support | Planned |
| **v3** | Intelligent data ingestion (LLM-powered ETL) | Planned |
| **v4** | Migration paths, framework adapters (Mongoose, Spring Data, etc.) | Planned |
| **v5** | Self-contained AI (local NL-MQL model) | Planned |
| **v6** | WASM, browser client, plugin system | Planned |

## License

Apache-2.0 - See [LICENSE](LICENSE) for details.
