<p align="center">
  <img src="assets/mongocore-header.svg" alt="MongoCore" width="100%"/>
</p>

# MongoCore

An AI-native MongoDB driver implemented as a lightweight Rust sidecar. MongoCore provides a single, fast core that serves all languages via gRPC, with native AI agent support via MCP (Model Context Protocol).

## Key Features

- **Rust sidecar architecture** - One fast core serves every language via gRPC
- **AI-native from the outset** - MCP interface for AI agents alongside gRPC for applications
- **Compiled queries** - Natural language queries translated once by an LLM, cached and reused at native speed
- **Voyage AI integration** - Auto-embed on write, semantic vector search, reranking
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
```

**TypeScript:**
```typescript
import { MongoClient } from '@mongocore/client';

const client = new MongoClient('localhost:50051');
await client.connect();
const users = client.db('myapp').collection('users');
await users.insertOne({ name: 'Alice', age: 30 });
```

**Go:**
```go
client := mongocore.NewClient("localhost:50051")
client.Connect(ctx)
users := client.Database("myapp").Collection("users")
users.InsertOne(ctx, bson.D{{Key: "name", Value: "Alice"}, {Key: "age", Value: 30}})
```

**Java:**
```java
try (MongoClient client = MongoClient.create("localhost:50051")) {
    MongoCollection users = client.getDatabase("myapp").getCollection("users");
    users.insertOne(new Document("name", "Alice").append("age", 30));
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
# Start test MongoDB
just docker-up
# or: docker compose -f docker-compose.test.yml up -d

# Run integration tests
just test-integration
# or: cargo test --test integration

# Run all tests (unit + integration)
just test-all
# or: cargo test

# Stop test MongoDB
just docker-down
# or: docker compose -f docker-compose.test.yml down
```

### Client Library Tests

```bash
# Python
cd clients/python && pip install -e ".[dev]" && pytest

# TypeScript
cd clients/typescript && npm install && npm test

# Go
cd clients/go && go test ./...

# Java
cd clients/java && mvn test
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

## Project Structure

```
mongocore/
├── src/
│   ├── main.rs              # Entry point, banner, startup
│   ├── config.rs            # Layered config (CLI + env + TOML)
│   ├── connection/          # Connection pool, capability detection
│   ├── operations/          # CRUD, aggregation, transactions, admin
│   ├── grpc/                # gRPC server (tonic) — 18 RPCs
│   ├── mcp/                 # MCP server (axum) — JSON-RPC tools & resources
│   ├── compiled/            # NL→MQL translation, 3-level cache, templates
│   ├── search/              # Vector search, full-text, fallback chain
│   └── voyage/              # Voyage AI REST client, batch embeddings
├── proto/                   # Protobuf service definitions
├── clients/
│   ├── python/              # Python client (async, BSON-native)
│   ├── typescript/          # TypeScript/Node.js client
│   ├── go/                  # Go client
│   └── java/                # Java client (AutoCloseable, builder pattern)
├── docs/                    # Comprehensive documentation
├── tests/                   # Integration tests + harness
├── docker-compose.test.yml  # Atlas Local for testing
├── Dockerfile               # Multi-stage production build
└── justfile                 # Task runner commands
```

## Recent Changes

- **Polyglot client libraries** — Python, TypeScript, Go, and Java wrappers with `MongoClient` API
- **Comprehensive documentation** — Full docs with examples in all four languages
- **MCP server** — 13 tools for AI agent interaction with safety controls
- **Compiled queries** — NL→MQL with in-memory, disk, and Atlas caching
- **Voyage AI integration** — Embeddings + vector search with automatic fallback
- **Change streams** — Real-time Watch via server-streaming gRPC
- **Deployment infrastructure** — Dockerfile, GitHub Actions CI/CD, installer script

## Roadmap

| Version | Focus |
|---------|-------|
| **v1** | Core sidecar, gRPC + MCP interfaces, compiled queries, Voyage AI integration |
| **v2** | Power user features, query analytics, multi-tenant support |
| **v3** | Intelligent data ingestion (LLM-powered ETL) |
| **v4** | Migration paths, framework adapters (Mongoose, Spring Data, etc.) |
| **v5** | Self-contained AI (local NL-MQL model) |
| **v6** | WASM, browser client, plugin system |

## License

Apache-2.0 - See [LICENSE](LICENSE) for details.
