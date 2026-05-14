# Client Streaming Cursors — find() and aggregate() via Streaming RPCs

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.
> If modifying client libraries: verify imports work and run `just test-clients`.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

## Overview

Convert `find()` and `aggregate()` in all 4 client libraries (Python, TypeScript, Go, Java) from unary RPCs to their streaming counterparts (`FindStream`, `AggregateStream`). Instead of returning a flat list of all documents, these methods now return async cursors that yield documents one at a time, with batching handled internally as a transport detail.

This eliminates the 64MB message size ceiling for query results, reduces memory usage for large result sets, and improves time-to-first-result.

## Motivation

Current `find()` and `aggregate()` collect all results into a single gRPC unary response. This fails for result sets exceeding the message size limit (64MB), forces clients to hold all documents in memory, and delays processing until the entire result is available.

The server already implements `FindStream` and `AggregateStream` RPCs that return documents in batches. Clients just need to consume these streams and present them as cursors.

## Design

### Behavioral Contract

1. `find()` returns a **Cursor** — an async iterable that yields individual documents
2. `aggregate()` returns a **Cursor** — same type as find
3. `find_one()` remains **unchanged** — unary RPC, returns a single document or null
4. Cursors are lazy — the streaming RPC is not called until iteration begins
5. Breaking out of iteration early cancels the underlying gRPC stream (server cleans up MongoDB cursor)
6. Errors mid-stream surface as exceptions/errors on the next iteration call
7. A `to_list()` / `toArray()` / `collect()` helper collects all documents for simple cases

### Configuration

| Parameter | Default | Description |
|-----------|---------|-------------|
| `batch_size` | 1000 | Documents per streaming batch (server clamps to [1, 10000]) |

Batch size is passed through to the `FindStreamRequest.batch_size` / `AggregateStreamRequest.batch_size` field. It is an optional parameter on `find()` and `aggregate()`.

### Options Passthrough

All existing find options (`limit`, `skip`, `sort`, `projection`) pass through via `FindStreamRequest.options` (same `FindOptions` proto type used by the unary `FindRequest`). The server respects `limit` globally — it stops after `limit` total documents, not per-batch.

Transaction support: `transaction_id` passes through the streaming request when find/aggregate is called within a transaction context.

### Cursor Lifecycle

```
find(filter) called
  → Cursor object created (lazy, no RPC yet)
  → First iteration (__anext__ / Next() / hasNext())
    → Opens FindStream gRPC call
    → Receives first DocumentBatch
    → Buffers batch internally
    → Yields first document from buffer
  → Subsequent iterations
    → Yields from buffer
    → When buffer empty + has_more=true: pulls next batch
    → When buffer empty + has_more=false: iteration ends
  → Early termination (break / close)
    → Cancels gRPC stream
    → Server detects cancellation, closes MongoDB cursor
```

### Error Handling

- If the gRPC stream returns an error status, the cursor raises/returns that error on the next `next()` call
- If the connection drops mid-stream, the cursor raises a connection error
- Cursor timeout (server sends `UNAVAILABLE` after idle timeout): raises with a descriptive message

## Per-Language API

### Python

```python
class Cursor:
    """Async iterator over streaming query results."""

    def __aiter__(self) -> "Cursor":
        return self

    async def __anext__(self) -> dict:
        """Yield the next document, or raise StopAsyncIteration."""
        ...

    async def to_list(self) -> list[dict]:
        """Collect all documents into a list."""
        return [doc async for doc in self]
```

Usage:

```python
# Iterate documents
async for doc in coll.find({"status": "active"}, limit=100):
    process(doc)

# Collect all (equivalent to old behavior)
docs = await coll.find({"status": "active"}).to_list()

# Aggregate
async for doc in coll.aggregate([{"$group": {"_id": "$category", "count": {"$sum": 1}}}]):
    print(doc)

# With batch_size tuning
async for doc in coll.find({}, batch_size=5000):
    process(doc)

# Early break cancels the stream
async for doc in coll.find({}):
    if doc["score"] > threshold:
        break  # gRPC stream cancelled, server cursor closed
```

Signature changes:

```python
# Before
async def find(self, filter=None, *, limit=0, skip=0) -> list[dict]
async def aggregate(self, pipeline: list[dict]) -> list[dict]

# After
def find(self, filter=None, *, limit=0, skip=0, batch_size=1000) -> Cursor
def aggregate(self, pipeline: list[dict], *, batch_size=1000) -> Cursor
```

Note: `find()` and `aggregate()` become synchronous methods that return a Cursor (lazy). The RPC happens on first iteration. `to_list()` is the async operation that drives iteration to completion.

### TypeScript

```typescript
class Cursor implements AsyncIterable<Document> {
  [Symbol.asyncIterator](): AsyncIterator<Document>;
  async toArray(): Promise<Document[]>;
}
```

Usage:

```typescript
// Iterate
for await (const doc of coll.find({ status: 'active' })) {
  process(doc);
}

// Collect all
const docs = await coll.find({ status: 'active' }).toArray();

// Aggregate
for await (const doc of coll.aggregate([{ $group: { _id: '$category' } }])) {
  process(doc);
}
```

Signature changes:

```typescript
// Before
async find(filter?, options?): Promise<Document[]>
async aggregate(pipeline): Promise<Document[]>

// After
find(filter?, options?): Cursor    // options gains batchSize field
aggregate(pipeline, options?): Cursor
```

### Go

```go
type Cursor struct { ... }

func (c *Cursor) Next(ctx context.Context) bool
func (c *Cursor) Doc() bson.D
func (c *Cursor) Err() error
func (c *Cursor) Close() error
func (c *Cursor) All(ctx context.Context) ([]bson.D, error)
```

Usage:

```go
// Iterate
cursor := coll.Find(ctx, bson.D{{"status", "active"}}, nil)
defer cursor.Close()
for cursor.Next(ctx) {
    doc := cursor.Doc()
    process(doc)
}
if err := cursor.Err(); err != nil {
    return err
}

// Collect all
docs, err := coll.Find(ctx, bson.D{}, nil).All(ctx)

// Aggregate
cursor = coll.Aggregate(ctx, pipeline, nil)
defer cursor.Close()
for cursor.Next(ctx) { ... }
```

Signature changes:

```go
// Before
func (c *Collection) Find(ctx, filter, opts) ([]bson.D, error)
func (c *Collection) Aggregate(ctx, pipeline) ([]bson.D, error)

// After
func (c *Collection) Find(ctx, filter, opts) *Cursor    // opts gains BatchSize field
func (c *Collection) Aggregate(ctx, pipeline, opts) *Cursor
```

Note: `Find` and `Aggregate` no longer return errors directly. Errors surface via `cursor.Err()` after `Next()` returns false, or from `All()`.

### Java

```java
public class MongoCursor implements AutoCloseable, Iterator<Document> {
    public boolean hasNext();
    public Document next();
    public List<Document> toList();
    public void close();
}
```

Usage:

```java
// Iterate (try-with-resources for auto-close)
try (MongoCursor cursor = coll.find(filter)) {
    while (cursor.hasNext()) {
        Document doc = cursor.next();
        process(doc);
    }
}

// Collect all
List<Document> docs = coll.find(filter).toList();

// Aggregate
try (MongoCursor cursor = coll.aggregate(pipeline)) {
    while (cursor.hasNext()) { ... }
}
```

Signature changes:

```java
// Before
public List<Document> find(Document filter)
public List<Document> find(Document filter, FindOptions options)
public List<Document> aggregate(List<Document> pipeline)

// After
public MongoCursor find(Document filter)
public MongoCursor find(Document filter, FindOptions options)  // options gains batchSize
public MongoCursor aggregate(List<Document> pipeline)
```

## Backwards Compatibility

This is a **breaking change** for all 4 clients:

| Language | Before return type | After return type | Migration |
|----------|-------------------|-------------------|-----------|
| Python | `list[dict]` | `Cursor` | Add `.to_list()` or use `async for` |
| TypeScript | `Promise<Document[]>` | `Cursor` | Add `.toArray()` or use `for await` |
| Go | `([]bson.D, error)` | `*Cursor` | Use `.All(ctx)` or loop with `.Next()` |
| Java | `List<Document>` | `MongoCursor` | Use `.toList()` or loop with `.hasNext()` |

Since MongoCore is pre-1.0, this is acceptable. The migration path is mechanical: append `.to_list()` / `.toArray()` / `.All()` / `.toList()` to get the old behavior.

## Implementation Notes

### gRPC Stream Consumption

Each language's gRPC library handles server-streaming differently:

- **Python (grpc.aio):** `stub.FindStream(request)` returns an async iterator of `DocumentBatch`
- **TypeScript (@grpc/grpc-js):** `client.findStream(request)` returns a readable stream, use `on('data')` or async iterator
- **Go (google.golang.org/grpc):** `client.FindStream(ctx, request)` returns a `stream` with `Recv()` method
- **Java (io.grpc):** Use `MongoCoreGrpc.newStub()` (async) with `StreamObserver`, or blocking stub's iterator

### Cancellation

When the cursor is closed/GC'd before the stream is exhausted:

- **Python:** Call `stream.cancel()` on the gRPC call
- **TypeScript:** Call `stream.cancel()` on the client readable stream
- **Go:** Cancel the context passed to the stream call
- **Java:** Call `onCompleted()` or `cancel()` on the `ClientCall`

### Thread Safety

Cursors are NOT thread-safe. Each cursor should be consumed by a single task/goroutine/thread. This matches the behavior of pymongo's Cursor, the MongoDB Node driver's Cursor, etc.

## Testing

Each client needs integration tests for:

1. Basic find with cursor iteration
2. find with `to_list()` / collect equivalent
3. find with limit/skip options
4. aggregate with cursor iteration
5. Early break/close cancels the stream (verify no resource leak)
6. Empty result set (cursor yields nothing)
7. Large result set (> 1 batch, verifies multi-batch iteration)
8. Error mid-stream (server returns error status)
9. Transaction support (find within transaction context)

## Scope Exclusions

- Write streaming operations (`InsertManyStream`, `InsertManyBidi`) — deferred to future work
- `find_one()` — stays unary, no cursor needed
- Cursor re-use / re-iteration — cursors are single-use, not restartable
- Server-side cursor timeout recovery — if cursor times out, error is raised (no automatic retry)
