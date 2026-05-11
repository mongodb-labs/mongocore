# Getting Started

MongoCore is an AI-native MongoDB driver implemented as a Rust sidecar process. Your application communicates with MongoCore over gRPC, and MongoCore handles connections, operations, and AI features (vector search, compiled queries, embeddings) on your behalf.

## Architecture

```
┌──────────────────┐       gRPC        ┌───────────────┐       Wire Protocol       ┌───────────┐
│  Your App        │ ───────────────── │   MongoCore   │ ────────────────────────── │  MongoDB  │
│  (any language)  │   localhost:50051  │   (sidecar)   │                            │  Server   │
└──────────────────┘                   └───────────────┘                            └───────────┘
                                              │
                                              │  HTTP/JSON-RPC
                                              ▼
                                       ┌───────────────┐
                                       │   MCP Server  │
                                       │  :3000/mcp    │
                                       └───────────────┘
```

- **gRPC interface** (port 50051) — High-performance binary protocol for application code
- **MCP interface** (port 3000) — JSON-RPC for AI agents (Claude, GPT, etc.)

## Installation

### From Source

```bash
git clone https://github.com/rozza/mongocore.git
cd mongocore
cargo build --release
```

The binary is at `target/release/mongocore`.

### Docker

```bash
docker build -t mongocore .
docker run -p 50051:50051 -p 3000:3000 mongocore \
  --connection-uri "mongodb://host.docker.internal:27017"
```

## Running MongoCore

```bash
# Defaults: connects to localhost:27017, gRPC on 50051, MCP on 3000
mongocore

# Custom connection
mongocore --connection-uri "mongodb+srv://user:pass@cluster.mongodb.net"

# Custom ports
mongocore --grpc-port 9090 --mcp-port 4000

# With AI features
mongocore \
  --llm-provider anthropic \
  --llm-api-key-env ANTHROPIC_API_KEY \
  --voyage-api-key-env VOYAGE_API_KEY
```

### Configuration File

Create a `mongocore.toml`:

```toml
connection_uri = "mongodb+srv://user:pass@cluster.mongodb.net"
grpc_port = 50051
mcp_port = 3000
llm_provider = "anthropic"
llm_api_key_env = "ANTHROPIC_API_KEY"
voyage_api_key_env = "VOYAGE_API_KEY"
compiled_cache_sync = true
log_level = "info"
```

```bash
mongocore --config mongocore.toml
```

### Environment Variables

All config options can be set via environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `MONGOCORE_CONNECTION_URI` | MongoDB connection string | `mongodb://localhost:27017` |
| `MONGOCORE_GRPC_PORT` | gRPC server port | `50051` |
| `MONGOCORE_MCP_PORT` | MCP server port | `3000` |
| `MONGOCORE_LLM_PROVIDER` | LLM provider (anthropic) | — |
| `MONGOCORE_LLM_API_KEY_ENV` | Env var holding LLM API key | — |
| `MONGOCORE_VOYAGE_API_KEY_ENV` | Env var holding Voyage AI key | — |
| `MONGOCORE_COMPILED_CACHE_SYNC` | Sync compiled queries to Atlas | `true` |
| `MONGOCORE_LOG_LEVEL` | Log level (trace/debug/info/warn/error) | `info` |

### Configuration Priority

CLI args > Environment variables > TOML file > Defaults

## Next Steps

- [CRUD Operations](./crud-operations.md) — Find, insert, update, delete
- [Aggregation](./aggregation.md) — Pipeline operations
- [Transactions](./transactions.md) — Multi-document ACID transactions
- [Search](./search.md) — Vector search, full-text search, fallback chains
- [Compiled Queries](./compiled-queries.md) — Natural language to MQL
- [MCP Server](./mcp-server.md) — AI agent integration
- [Client Libraries](./client-libraries.md) — Python, TypeScript, Go, Java
