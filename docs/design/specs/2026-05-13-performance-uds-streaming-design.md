# Performance Tier 1: gRPC over UDS + Streaming Bulk Responses

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.
> If modifying client libraries: verify imports work and run `just test-clients`.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

## Overview

MongoCore's gRPC-over-TCP transport adds ~0.15ms per call and imposes a 4MB message limit. This design eliminates both bottlenecks through three changes:

1. **Unix Domain Socket transport** — ~36% latency reduction for all same-machine operations
2. **Server-streaming RPCs for bulk responses** — unlimited result sizes, lower memory, faster time-to-first-result
3. **Raised gRPC message limits** — 64MB safety net for unary RPCs

These changes are additive and backwards-compatible. Existing TCP+unary clients continue to work unchanged.

## Motivation

Current benchmark results show significant overhead for small operations:

| Operation | pymongo (native) | MongoCore+Python | Overhead |
|-----------|-----------------|------------------|----------|
| find_one | 3,891 ops/s | 2,343 ops/s | -40% |
| insert_one_small | 3,608 ops/s | 878 ops/s | -76% |
| bulk_insert (10K) | 152K ops/s | 100K ops/s | -34% |

The overhead comes from: TCP loopback latency, HTTP/2 framing, protobuf envelope encode/decode, and the 4MB message ceiling forcing smaller batches.

## Design

### 1. Unix Domain Socket Transport

#### Server Side

Tonic supports UDS natively via `tokio::net::UnixListener`. MongoCore listens on both TCP and UDS simultaneously:

- TCP listener: existing behavior, bound to `--host`/`--port` (default `127.0.0.1:50051`)
- UDS listener: new, bound to `--socket-path` (default: disabled; suggested convention: `/tmp/mongocore.sock`)

When `--socket-path` is provided, both listeners run concurrently on the same tonic service. UDS is the preferred path for same-machine clients; TCP remains for remote clients and backwards compatibility.

#### Socket Lifecycle

- **Startup:** If the socket file already exists, unlink it before binding (handles prior crash). Log a warning when doing so.
- **Shutdown:** Remove the socket file on graceful shutdown (SIGTERM/SIGINT handler).
- **Permissions:** Set socket file to `0600` (owner-only read/write). Configurable via `--socket-permissions` for multi-user scenarios.
- **Crash recovery:** Stale socket files are detected by attempting a connect — if it fails with "connection refused", safe to unlink and rebind.

#### Configuration

```toml
[server]
host = "127.0.0.1"
port = 50051
socket_path = "/tmp/mongocore.sock"    # Optional, enables UDS
socket_permissions = "0600"            # Default: owner-only
```

CLI args: `--socket-path`, `--socket-permissions`
Env vars: `MONGOCORE_SOCKET_PATH`, `MONGOCORE_SOCKET_PERMISSIONS`

#### Client Discovery

Clients determine the transport in priority order:

1. Explicit socket path passed to constructor (`MongoCore(socket_path="/tmp/mongocore.sock")`)
2. `MONGOCORE_SOCKET_PATH` environment variable
3. Well-known path check: if `/tmp/mongocore.sock` exists and is connectable, use it
4. Fall back to TCP (`host:port`)

This means for the default local development case, UDS is automatic once the server is configured with a socket path.

### 2. Streaming Bulk Responses

#### New RPCs

Add server-streaming variants for operations that can return large result sets:

```protobuf
// Streaming find — returns documents in configurable batches
rpc FindStream(FindStreamRequest) returns (stream DocumentBatch);

// Streaming aggregation — pipeline results in batches
rpc AggregateStream(AggregateStreamRequest) returns (stream DocumentBatch);

// Streaming insert — client streams batches, server acknowledges at end
rpc InsertManyStream(stream InsertBatch) returns (InsertManyStreamResponse);

// Bidirectional streaming insert — per-batch acknowledgments
rpc InsertManyBidi(stream InsertBatch) returns (stream InsertBatchAck);
```

#### Message Definitions

```protobuf
message FindStreamRequest {
  string database = 1;
  string collection = 2;
  Filter filter = 3;
  FindOptions options = 4;
  optional string transaction_id = 5;
  uint32 batch_size = 6;  // docs per batch frame, default 100
}

message AggregateStreamRequest {
  string database = 1;
  string collection = 2;
  repeated bytes stages = 3;
  AggregateOptions options = 4;
  optional string transaction_id = 5;
  uint32 batch_size = 6;
}

message DocumentBatch {
  repeated Document documents = 1;
  uint32 batch_index = 2;       // 0-based sequence number
  bool has_more = 3;            // false on final batch
  optional uint64 cursor_id = 4; // for cursor management
}

message InsertBatch {
  string database = 1;          // only required on first batch
  string collection = 2;        // only required on first batch
  repeated Document documents = 3;
  InsertOptions options = 4;    // only required on first batch
}

message InsertBatchAck {
  uint32 batch_index = 1;
  uint32 inserted_count = 2;
  repeated InsertError errors = 3;
}

message InsertManyStreamResponse {
  uint64 total_inserted = 1;
  repeated InsertError errors = 2;
}

message InsertError {
  uint32 index = 1;
  string message = 2;
  int32 code = 3;
}
```

#### Cursor Lifecycle

- The server opens a MongoDB cursor and streams batches from it.
- **Client disconnect:** Tonic detects the dropped stream. The server-side handler catches this and closes the MongoDB cursor immediately (no leak).
- **Client cancellation:** gRPC native cancellation propagates through tonic's `CancellationToken`. Handler closes cursor on cancellation.
- **MongoDB cursor timeout:** Default MongoDB cursor timeout is 10 minutes of inactivity. For long-paused streams, the handler catches `CursorNotFound` and returns an appropriate gRPC status code (`UNAVAILABLE` with retry hint).
- **Idle timeout:** Configurable server-side idle timeout per stream (default: 60s). If the client stops consuming for this duration, the server closes the stream proactively.

#### Batch Size Tuning

- Client specifies `batch_size` per request (how many documents per `DocumentBatch` frame).
- Default: 1000 documents per batch.
- Server clamps to range [1, 10000] to prevent abuse.
- The batch size also aligns with the MongoDB cursor's `batchSize` to avoid buffering entire cursor results in MongoCore memory.

### 3. Raised gRPC Message Limits

For unary RPCs that don't use streaming (existing API, backwards-compatible):

- Max send message size: **64 MB** (from default 4 MB)
- Max receive message size: **64 MB**
- Configurable via `--grpc-max-message-size` / `MONGOCORE_GRPC_MAX_MESSAGE_SIZE`

This is a safety net — streaming is preferred for truly large payloads, but raising the limit prevents failures for moderately large unary responses (e.g., 1000 small documents in a single Find).

### 4. gRPC Compression

Enable optional per-message compression for streaming RPCs:

- Support `gzip` and `zstd` (tonic supports both)
- Default: disabled (overhead on small messages outweighs benefit)
- Client can request compression via gRPC metadata header (`grpc-accept-encoding: zstd`)
- Server enables compression for streaming batches when the batch payload exceeds a threshold (default: 4 KB)
- BSON is typically 30-60% compressible, so this helps throughput on bulk streams

Configuration:

```toml
[server]
compression = "none"        # "none", "gzip", "zstd"
compression_threshold = 4096  # bytes; only compress batches larger than this
```

## Security Considerations

### UDS Permissions

Unix domain sockets use filesystem permissions for access control, replacing TCP's TLS requirement for same-machine communication:

- Socket file created with `0600` by default (only the MongoCore process owner can connect)
- For multi-user deployments, `0660` with a shared group is supported
- No TLS negotiation overhead — connections are authenticated by the OS kernel via filesystem permissions
- This is standard practice (Docker, PostgreSQL, MySQL all use UDS with permission-based auth)

### Streaming Abuse Prevention

- Per-client stream limit: maximum 10 concurrent streams per connection (configurable)
- Batch size clamped server-side regardless of client request
- Idle stream timeout prevents resource leaks from abandoned streams

## Client Library Changes

Each client library gains:

1. **UDS connection support** — connect via socket path instead of host:port
2. **Streaming API** — async iterator/stream interface for bulk operations
3. **Auto-discovery** — attempt UDS first, fall back to TCP

### Python Example

```python
from mongocore import MongoCore

# Auto-discovers UDS at /tmp/mongocore.sock, falls back to TCP
client = MongoCore()

# Explicit UDS
client = MongoCore(socket_path="/tmp/mongocore.sock")

# Streaming find — async iterator
async for batch in client.find_stream("mydb", "mycoll", {}, batch_size=200):
    for doc in batch.documents:
        process(doc)

# Streaming insert — sends batches, gets per-batch acks
async for ack in client.insert_many_stream("mydb", "mycoll", large_doc_list, batch_size=500):
    print(f"Batch {ack.batch_index}: {ack.inserted_count} inserted")
```

### TypeScript Example

```typescript
import { MongoCore } from 'mongocore';

// Auto-discovers UDS
const client = new MongoCore();

// Streaming find
for await (const batch of client.findStream('mydb', 'mycoll', {}, { batchSize: 200 })) {
  batch.documents.forEach(doc => process(doc));
}
```

### Go Example

```go
client, _ := mongocore.New(mongocore.WithSocketPath("/tmp/mongocore.sock"))

// Streaming find
stream, _ := client.FindStream(ctx, "mydb", "mycoll", bson.M{}, mongocore.BatchSize(200))
for stream.Next() {
    batch := stream.Batch()
    for _, doc := range batch.Documents {
        process(doc)
    }
}
```

## Performance Targets

Expected improvements based on research:

| Operation | Current Overhead | Target After | Mechanism |
|-----------|-----------------|--------------|-----------|
| find_one | -40% | -10% to -15% | UDS eliminates TCP latency |
| insert_one_small | -76% | -40% to -50% | UDS helps; remaining overhead is protobuf + HTTP/2 framing |
| insert_one_large | -10% | -5% | Already data-dominated |
| bulk_insert (10K) | -34% (+ 4MB limit) | -5% to -10% | Streaming amortizes overhead, no size limit |
| find (10K docs) | Fails at 4MB | Works, -10% to -15% | Streaming + UDS |
| find (100K docs) | Impossible | Works, streaming | Bounded memory via cursor batching |

### Remaining Overhead (future work)

After this design, the remaining per-call overhead for small ops (~40-50% on insert_one_small) comes from:
- HTTP/2 frame headers and stream management (~30μs)
- Protobuf encode/decode of the request/response envelope (~10-20μs)
- Tokio task scheduling (~5-10μs)

Addressing this residual overhead is the target for **Performance Tier 2: Request Pipelining** — batching multiple small operations into a single gRPC round-trip.

## Future Work (Roadmap)

### Performance Tier 2: Request Pipelining

Batch N independent operations into a single round-trip:

```protobuf
rpc BatchExecute(BatchRequest) returns (BatchResponse);

message BatchRequest {
  repeated Operation operations = 1;
}

message Operation {
  oneof op {
    FindOneRequest find_one = 1;
    InsertOneRequest insert_one = 2;
    UpdateOneRequest update_one = 3;
    DeleteOneRequest delete_one = 4;
  }
}
```

This amortizes per-call overhead across multiple ops, targeting chatty workloads that issue many small operations in sequence.

### Performance Tier 3: Native Embedding (FFI)

For maximum performance (eliminating IPC entirely):
- PyO3 native Python extension
- Neon native Node.js extension
- cgo native Go bindings

These embed MongoCore's Rust core directly in the language process, reducing call overhead to nanoseconds. The sidecar remains for deployment flexibility and AI/MCP use cases.

## Testing Plan

1. **Unit tests:** UDS listener setup/teardown, socket permission setting, batch size clamping
2. **Integration tests:** Full round-trip over UDS for all streaming RPCs
3. **Benchmark suite:** Re-run existing benchmarks over UDS transport; add streaming variants
4. **Client tests:** All 4 languages connecting via UDS and consuming streams
5. **Edge cases:** Client disconnect mid-stream, cursor timeout, stale socket file, permission denied
6. **Regression:** Verify TCP path still works identically (no regression)

## Implementation Order

1. Raise gRPC message limits (one-line change, immediate unblocking)
2. UDS transport support in server (tonic config + socket lifecycle)
3. Client library UDS connection support (all 4 languages)
4. Streaming proto definitions + server implementation
5. Client library streaming APIs
6. Compression support
7. Benchmark validation
