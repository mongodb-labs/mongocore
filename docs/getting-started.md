# Getting Started

MongoCore is an AI-native MongoDB driver implemented as a Rust sidecar process. Your application communicates with MongoCore over gRPC, and MongoCore handles connections, operations, and AI features (vector search, compiled queries, embeddings) on your behalf.

## Architecture

```
┌──────────────────┐   gRPC (TCP/UDS)  ┌───────────────┐       Wire Protocol       ┌───────────┐
│  Your App        │ ─────────────────── │   MongoCore   │ ────────────────────────── │  MongoDB  │
│  (any language)  │                    │   (sidecar)   │                            │  Server   │
└──────────────────┘                   └───────────────┘                            └───────────┘
                                              │
                                              │  HTTP/JSON-RPC
                                              ▼
                                       ┌───────────────┐
                                       │   MCP Server  │
                                       │  :3000/mcp    │
                                       └───────────────┘
```

- **gRPC interface** — TCP on port 50051 + UDS at `/tmp/mongocore.sock` (configurable)
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
# Defaults: TCP + UDS, connects to localhost:27017
mongocore

# TCP only (no UDS socket)
mongocore --transport tcp

# UDS only (no TCP listener)
mongocore --transport uds

# Custom socket path
mongocore --socket-path /var/run/mongocore.sock

# Custom connection
mongocore --connection-uri "mongodb+srv://user:pass@cluster.mongodb.net"

# Custom ports
mongocore --grpc-port 9090 --mcp-port 4000

# With AI features (requires API keys in config or environment)
export ANTHROPIC_API_KEY="your-api-key-here"
export VOYAGE_API_KEY="your-api-key-here"
mongocore
```

### Configuration File

Create a `mongocore.toml`:

```toml
connection_uri = "mongodb+srv://user:pass@cluster.mongodb.net"
grpc_port = 50051
mcp_port = 3000
transport = "both"
socket_path = "/tmp/mongocore.sock"
ANTHROPIC_API_KEY = "your-api-key-here"
VOYAGE_API_KEY = "your-api-key-here"
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
| `ANTHROPIC_API_KEY` | Anthropic API key for Claude | — |
| `OPENAI_API_KEY` | OpenAI API key | — |
| `VOYAGE_API_KEY` | Voyage AI API key for embeddings | — |
| `MONGOCORE_COMPILED_CACHE_SYNC` | Sync compiled queries to Atlas | `true` |
| `MONGOCORE_LOG_LEVEL` | Log level (trace/debug/info/warn/error) | `info` |
| `MONGOCORE_TRANSPORT` | Transport mode (both/uds/tcp) | `both` |
| `MONGOCORE_SOCKET_PATH` | UDS socket file path | `/tmp/mongocore.sock` |
| `MONGOCORE_GRPC_MAX_MESSAGE_SIZE` | Max gRPC message size (bytes) | `67108864` (64MB) |
| `MONGOCORE_GRPC_COMPRESSION` | Compression algorithm | `none` |
| `MONGOCORE_STREAM_BATCH_SIZE` | Streaming batch size | `1000` |
| `MONGOCORE_STREAM_IDLE_TIMEOUT_SECS` | Stream idle timeout | `60` |

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
