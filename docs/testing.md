# Testing

## Test Configuration

Copy the example config to create your local test configuration:

```bash
cp config.test.toml.example config.test.toml
```

Edit `config.test.toml` with your settings:

```toml
connection_uri = "mongodb://localhost:27017"
grpc_port = 50051
mcp_port = 3000
log_level = "debug"
compiled_cache_sync = false

# API Keys for LLM providers (pick one)
# ANTHROPIC_API_KEY = "your-api-key-here"
# OPENAI_API_KEY = "your-api-key-here"

# Voyage AI for embeddings
# VOYAGE_API_KEY = "your-api-key-here"

# Custom LLM gateway (optional — overrides direct API keys)
# Use this for corporate AI gateways or self-hosted endpoints
# LLM_BASE_URL = "https://my-ai-gateway.example.com/anthropic/v1/messages"
# LLM_API_KEY = "your-gateway-api-key"
# LLM_AUTH_HEADER = "api-key"
# LLM_MODEL = "claude-sonnet-4-6"
# LLM_PROVIDER_TYPE = "anthropic"

# Enable LLM integration tests (requires LLM provider configured above)
# TEST_LLM_INTEGRATION = "true"

# OpenTelemetry tracing (requires --features otel)
# otel_enabled = true
# otel_endpoint = "http://localhost:4317"
# otel_service_name = "mongocore"
```

> **Note:** `config.test.toml` is gitignored since it may contain API keys. Only the `.example` template is committed.

## Running Tests

### Quick Reference

| Command | What it does | Dependencies |
|---------|-------------|--------------|
| `just test-unit` | Rust unit tests (~233) | None |
| `just test-integration` | Rust integration tests (~97) | Docker MongoDB |
| `just test-rust` | All Rust tests (unit + integration) | Docker MongoDB |
| `just test-clients` | All client tests (Python, TS, Go, Java) | Docker MongoDB (sidecar auto-managed) |
| `just test-unit-clients` | Client unit tests only (no server needed) | None |
| `just test-llm` | Compiled query LLM tests (23) | Docker MongoDB + LLM configured |
| `just test-all` | Everything (Rust + clients) | Docker MongoDB |

### Cargo Commands

| Command | What it does |
|---------|-------------|
| `cargo test --lib` | Run all unit tests (no external dependencies) |
| `cargo test --test integration` | Run integration tests (needs Docker MongoDB) |
| `cargo test --test integration compiled_llm -- --nocapture` | Run LLM tests with output |
| `cargo build` | Build (also regenerates proto stubs) |
| `cargo build --release` | Build optimized binary |
| `cargo build --features otel` | Build with OpenTelemetry support |

### Just Commands (Full List)

| Command | Description |
|---------|-------------|
| `just test-unit` | Run Rust unit tests only — fast, no dependencies |
| `just test-integration` | Run Rust integration tests — needs Docker MongoDB running |
| `just test-rust` | Run all Rust tests (unit + integration) |
| `just test-python` | Run Python client tests (unit + integration) |
| `just test-typescript` | Run TypeScript client tests (unit + integration) |
| `just test-go` | Run Go client tests (unit + integration) |
| `just test-java` | Run Java client tests (unit + integration) |
| `just test-clients` | Run all client tests — auto-starts/stops the MongoCore sidecar |
| `just test-unit-python` | Python unit tests only (no server needed) |
| `just test-unit-typescript` | TypeScript unit tests only (no server needed) |
| `just test-unit-go` | Go unit tests only (no server needed) |
| `just test-unit-java` | Java unit tests only (no server needed) |
| `just test-unit-clients` | All client unit tests (no server needed) |
| `just test-all` | Everything — Rust tests + client tests with auto-managed sidecar |
| `just test-llm` | LLM integration tests — sets TEST_LLM_INTEGRATION=true automatically |
| `just docker-up` | Start MongoDB Atlas Local (loads sample data on first start) |
| `just docker-down` | Stop MongoDB container |
| `just docker-build` | Build MongoCore Docker image |
| `just docker-run` | Run MongoCore as a Docker container |
| `just release-local` | Build optimized release binary |

## Docker Setup

```bash
# Start MongoDB (loads Atlas sample datasets on first start, ~30-60s)
just docker-up

# Stop
just docker-down
```

The Docker container uses `mongodb/mongodb-atlas-local` which provides:
- Atlas Vector Search
- Atlas Search (full-text)
- Replica set support (for transactions and change streams)
- Sample datasets (sample_restaurants, sample_mflix, sample_supplies, etc.)

Sample data loads automatically on first container start. Subsequent starts are fast (data persists in the container volume).

## Test Types Explained

### Unit Tests (`just test-unit`)

Fast, isolated tests with no external dependencies. Tests individual functions, parsers, validators, cache logic, etc. Always run these before committing.

### Integration Tests (`just test-integration`)

Tests that connect to a real MongoDB instance. Cover CRUD operations, aggregation, transactions, search, ingestion, analytics, and more. Require Docker MongoDB running.

### Client Tests (`just test-clients`)

End-to-end tests for all 4 client libraries (Python, TypeScript, Go, Java). The command automatically:
1. Builds the release binary
2. Starts the MongoCore sidecar in background
3. Waits for gRPC port 50051 to be ready
4. Runs all client test suites with verbose output
5. Kills the sidecar on exit (even if tests fail)

Each client runs both unit tests (no server) and integration tests (via sidecar).

### LLM Integration Tests (`just test-llm`)

Tests the compiled query NL→MQL system with a real LLM provider. Requires:
1. An LLM configured in `config.test.toml` (direct API key OR gateway)
2. Docker MongoDB running with sample data loaded

The command automatically sets `TEST_LLM_INTEGRATION=true`. Tests skip gracefully when not configured — they won't cause failures in CI or when run without LLM access.

Tests cover:
- Multi-database queries (sample_restaurants, sample_mflix, sample_supplies, sample_training)
- Method routing verification (filter vs aggregate)
- Template registry reuse
- Cache behavior (different phrasing, cross-collection isolation, parameterized numbers)
- Injection safety ($where, $out, prompt override, SQL injection, special characters)

## Pre-Commit Checklist

Before every commit, ensure:

```bash
# 1. Zero compiler warnings
cargo build 2>&1 | grep "warning:"
# Must produce NO output

# 2. All unit tests pass
cargo test --lib

# 3. Integration tests compile (if you changed shared types)
cargo test --test integration --no-run
```

Before merging/pushing:

```bash
# Full test suite
just test-all
```

## Docker Container (Production)

Build and run MongoCore as a container:

```bash
# Build
just docker-build
# or: docker build -t mongocore:dev .

# Run
just docker-run
# or: docker run --rm -p 50051:50051 -p 3000:3000 mongocore:dev

# With custom connection
docker run -p 50051:50051 -p 3000:3000 mongocore:dev \
  --connection-uri "mongodb://host.docker.internal:27017"
```
