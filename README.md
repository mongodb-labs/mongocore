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
- **Opinionated defaults** - Majority write/read concern, retryable operations, sensible timeouts. The correct path is the default path.
- **Schema opt-in** - Connect and go instantly. Add type-safe schemas when you want them, not before.

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
- [Docker](https://docs.docker.com/get-docker/) (for testing)
- [just](https://github.com/casey/just) (task runner, optional)

### Build

```bash
git clone https://github.com/rozza/mongocore.git
cd mongocore
cargo build
```

### Run

```bash
# With a config file
cargo run -- --config config.toml

# With environment variables
MONGOCORE_URI="mongodb+srv://..." cargo run

# With CLI args
cargo run -- --connection-uri "mongodb://localhost:27017"
```

### Configuration

MongoCore uses layered configuration (CLI args > environment variables > TOML file > defaults):

```toml
# config.toml
connection_uri = "mongodb://localhost:27017"
grpc_port = 50051
mcp_port = 3000
log_level = "info"
compiled_cache_sync = true

# Optional: LLM provider for compiled queries
llm_provider = "claude"
llm_api_key_env = "ANTHROPIC_API_KEY"

# Optional: Voyage AI for embeddings
voyage_api_key_env = "VOYAGE_API_KEY"
```

All settings have sensible defaults. The minimum required is a MongoDB connection URI.

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

These can be overridden per-operation when needed, but the default path is always correct.

## Testing

### Unit Tests

```bash
# Run unit tests (no dependencies required)
just test-unit
# or
cargo test --lib
```

### Integration Tests

Integration tests run against the `mongodb/mongodb-atlas-local` Docker image, which provides a local Atlas-like environment with vector search, Atlas Search, and replica set support.

```bash
# Start the test MongoDB instance
just docker-up

# Run integration tests
just test-integration

# Run all tests
just test-all

# Stop MongoDB when done
just docker-down
```

### Manual Docker Setup

If not using `just`:

```bash
docker compose -f docker-compose.test.yml up -d
cargo test --test integration
docker compose -f docker-compose.test.yml down
```

## Project Structure

```
mongocore/
├── src/
│   ├── main.rs              # Entry point, config loading, startup
│   ├── lib.rs               # Module exports
│   ├── config.rs            # Layered config (CLI + env + TOML)
│   ├── defaults.rs          # Opinionated MongoDB defaults
│   ├── error.rs             # Error types
│   ├── connection/
│   │   ├── mod.rs           # Connection module
│   │   └── pool.rs          # Connection pool with capability detection
│   └── operations/
│       ├── mod.rs           # Operations module
│       ├── crud.rs          # find, insert, update, delete
│       ├── aggregation.rs   # Aggregation pipeline
│       ├── find_and_modify.rs # Atomic find+modify
│       ├── admin.rs         # createCollection, createIndex, etc.
│       └── transaction.rs   # Multi-document transactions
├── tests/
│   ├── integration.rs       # Integration test entry point
│   ├── integration/         # Integration test modules
│   │   ├── crud_test.rs
│   │   ├── transaction_test.rs
│   │   └── aggregation_test.rs
│   └── harness/             # Shared test utilities
│       ├── mod.rs
│       └── mongodb.rs
├── design/
│   ├── specs/               # Design specifications
│   └── plans/               # Implementation plans
├── docker-compose.test.yml  # MongoDB Atlas Local for testing
├── justfile                 # Task runner commands
└── Cargo.toml
```

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
