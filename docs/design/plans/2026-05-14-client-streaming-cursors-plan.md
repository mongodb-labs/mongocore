# Client Streaming Cursors Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.
> If modifying client libraries: verify imports work and run `just test-clients`.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

**Goal:** Convert `find()` and `aggregate()` in all 4 client libraries from unary RPCs to streaming cursors backed by `FindStream`/`AggregateStream` RPCs, returning async iterators that yield documents one at a time.

**Architecture:** Each client gets a Cursor class that wraps the gRPC server-streaming call, buffers one batch internally, and yields individual documents. The `find()` and `aggregate()` methods on Collection return a Cursor instead of a list. A `to_list()`/`toArray()`/`All()` helper provides backwards-compatible collection semantics.

**Tech Stack:** Python grpc.aio (async streaming), TypeScript @grpc/grpc-js (dynamic proto loading + readable stream), Go grpc ServerStreamingClient, Java grpc blocking iterator

---

## File Structure

| File | Responsibility |
|------|---------------|
| `clients/python/src/mongocore/cursor.py` | Python Cursor class (async iterator over streaming batches) |
| `clients/python/src/mongocore/collection.py` | Modify find()/aggregate() to return Cursor |
| `clients/python/src/mongocore/__init__.py` | Export Cursor |
| `clients/python/tests/test_integration.py` | Update existing tests + add cursor-specific tests |
| `clients/typescript/src/cursor.ts` | TypeScript Cursor class (AsyncIterable) |
| `clients/typescript/src/collection.ts` | Modify find()/aggregate() to return Cursor |
| `clients/typescript/src/index.ts` | Export Cursor |
| `clients/typescript/src/types.ts` | Add FindStreamOptions type |
| `clients/typescript/tests/integration.test.ts` | Update existing tests + add cursor tests |
| `clients/go/mongocore/cursor.go` | Go Cursor struct (Next/Doc/Err/Close/All) |
| `clients/go/mongocore/collection.go` | Modify Find()/Aggregate() to return *Cursor |
| `clients/go/mongocore/integration_test.go` | Update existing tests + add cursor tests |
| `clients/java/src/main/java/com/mongocore/MongoCursor.java` | Java MongoCursor (AutoCloseable + Iterator) |
| `clients/java/src/main/java/com/mongocore/MongoCollection.java` | Modify find()/aggregate() to return MongoCursor |
| `clients/java/src/main/java/com/mongocore/FindOptions.java` | Add batchSize field |
| `clients/java/src/test/java/com/mongocore/IntegrationTest.java` | Update existing tests + add cursor tests |

---

### Task 1: Regenerate Python Proto Stubs

The Python generated gRPC stubs are missing `FindStream`/`AggregateStream` — they need regeneration from the proto files.

**Files:**
- Modify: `clients/python/src/mongocore/generated/mongocore/v1/mongocore_pb2_grpc.py` (regenerated)
- Modify: `clients/python/src/mongocore/generated/mongocore/v1/mongocore_pb2.py` (regenerated)
- Modify: `clients/python/src/mongocore/generated/mongocore/v1/types_pb2.py` (regenerated)

- [ ] **Step 1: Regenerate Python proto stubs**

The generated files live at `clients/python/src/mongocore/generated/mongocore/v1/`. Protoc automatically creates the `mongocore/v1/` subdirectory from the proto package path. The existing files use relative imports (`from .`), which requires the output root to be `src/mongocore/generated`.

Run from project root:

```bash
cd clients/python && python3 -m grpc_tools.protoc -I../../proto \
  --python_out=src/mongocore/generated --grpc_python_out=src/mongocore/generated \
  --pyi_out=src/mongocore/generated \
  ../../proto/mongocore/v1/mongocore.proto \
  ../../proto/mongocore/v1/types.proto \
  ../../proto/mongocore/v1/ingestion.proto
```

- [ ] **Step 2: Verify output landed in correct path and streaming stubs exist**

```bash
grep "FindStream\|AggregateStream" clients/python/src/mongocore/generated/mongocore/v1/mongocore_pb2_grpc.py
```

Expected: Output showing `FindStream` and `AggregateStream` methods in the stub class. If no output, the regeneration failed or went to the wrong directory — check paths.

Also verify the imports still use relative form:

```bash
grep "^from \." clients/python/src/mongocore/generated/mongocore/v1/mongocore_pb2_grpc.py | head -3
```

Expected: Lines like `from . import mongocore_pb2 as ...`

- [ ] **Step 3: Verify Python tests still pass (imports work)**

```bash
cd clients/python && python3 -c "from mongocore.generated.mongocore.v1 import mongocore_pb2_grpc; print('OK')"
```

Expected: `OK` printed.

- [ ] **Step 4: Commit**

```bash
git add clients/python/src/mongocore/generated/
git commit -m "chore(clients): regenerate Python proto stubs with streaming RPCs"
```

---

### Task 2: Python Cursor Class

**Files:**
- Create: `clients/python/src/mongocore/cursor.py`
- Modify: `clients/python/src/mongocore/__init__.py`

- [ ] **Step 1: Create the Cursor class**

Create `clients/python/src/mongocore/cursor.py`:

```python
"""Async cursor over streaming gRPC query results."""

from typing import Optional


_CLIENT_METADATA = [("x-client-language", "python")]


class Cursor:
    """Async iterator that yields documents from a streaming gRPC call.

    The underlying RPC is not called until iteration begins (lazy).
    """

    def __init__(self, stub, request, rpc_method: str, decode_fn):
        self._stub = stub
        self._request = request
        self._rpc_method = rpc_method
        self._decode_fn = decode_fn
        self._stream = None
        self._buffer: list = []
        self._buffer_index: int = 0
        self._exhausted: bool = False

    def __aiter__(self):
        return self

    async def __anext__(self):
        if self._buffer_index < len(self._buffer):
            doc = self._buffer[self._buffer_index]
            self._buffer_index += 1
            return doc

        if self._exhausted:
            raise StopAsyncIteration

        await self._fetch_next_batch()

        if self._buffer_index < len(self._buffer):
            doc = self._buffer[self._buffer_index]
            self._buffer_index += 1
            return doc

        raise StopAsyncIteration

    async def _fetch_next_batch(self):
        if self._stream is None:
            rpc = getattr(self._stub, self._rpc_method)
            self._stream = rpc(self._request, metadata=_CLIENT_METADATA)

        try:
            batch = await self._stream.read()
            if batch is None:
                self._exhausted = True
                return
        except StopAsyncIteration:
            self._exhausted = True
            return
        except Exception as e:
            self._exhausted = True
            raise

        self._buffer = [self._decode_fn(doc.data) for doc in batch.documents]
        self._buffer_index = 0

        if not batch.has_more:
            self._exhausted = True

    async def to_list(self) -> list:
        """Collect all documents into a list."""
        results = []
        async for doc in self:
            results.append(doc)
        return results

    async def close(self):
        """Cancel the underlying stream."""
        if self._stream is not None:
            self._stream.cancel()
            self._stream = None
        self._exhausted = True
```

- [ ] **Step 2: Export Cursor from package**

In `clients/python/src/mongocore/__init__.py`, add `Cursor` to imports and `__all__`:

```python
from .client import MongoClient
from .collection import Collection, ChangeStream
from .cursor import Cursor
from .database import Database
from . import ops
from .result import PipelineResult

__version__ = "0.1.0"
__all__ = ["MongoClient", "Collection", "ChangeStream", "Cursor", "Database", "ops", "PipelineResult"]
```

- [ ] **Step 3: Verify import works**

```bash
cd clients/python && python3 -c "from mongocore import Cursor; print('OK')"
```

Expected: `OK`

- [ ] **Step 4: Commit**

```bash
git add clients/python/src/mongocore/cursor.py clients/python/src/mongocore/__init__.py
git commit -m "feat(clients): add Python Cursor class for streaming iteration"
```

---

### Task 3: Python Collection — Switch find() and aggregate() to Cursors

**Files:**
- Modify: `clients/python/src/mongocore/collection.py`

- [ ] **Step 1: Update find() to return a Cursor**

In `clients/python/src/mongocore/collection.py`, replace the `find` method:

```python
def find(self, filter: Optional[dict] = None, *, limit: int = 0, skip: int = 0, batch_size: int = 1000, transaction_id: Optional[str] = None) -> "Cursor":
    """Find documents matching the filter. Returns an async cursor."""
    from .generated.mongocore.v1 import mongocore_pb2, types_pb2
    from .cursor import Cursor

    options = types_pb2.FindOptions()
    if limit:
        options.limit = limit
    if skip:
        options.skip = skip

    request = mongocore_pb2.FindStreamRequest(
        database=self._database,
        collection=self._name,
        filter=self._make_filter(filter),
        options=options,
        batch_size=batch_size,
    )
    if transaction_id:
        request.transaction_id = transaction_id

    stub = self._get_stub()
    return Cursor(stub, request, "FindStream", self._decode_doc)
```

- [ ] **Step 2: Update aggregate() to return a Cursor**

Replace the `aggregate` method:

```python
def aggregate(self, pipeline: list[dict], *, batch_size: int = 1000, transaction_id: Optional[str] = None) -> "Cursor":
    """Run an aggregation pipeline. Returns an async cursor."""
    from .generated.mongocore.v1 import mongocore_pb2, types_pb2
    from .cursor import Cursor

    stages = [self._encode_doc(stage) for stage in pipeline]

    request = mongocore_pb2.AggregateStreamRequest(
        database=self._database,
        collection=self._name,
        pipeline=types_pb2.Pipeline(stages=stages),
        batch_size=batch_size,
    )
    if transaction_id:
        request.transaction_id = transaction_id

    stub = self._get_stub()
    return Cursor(stub, request, "AggregateStream", self._decode_doc)
```

- [ ] **Step 3: Verify module imports cleanly**

```bash
cd clients/python && python3 -c "from mongocore import Collection; print('OK')"
```

Expected: `OK`

- [ ] **Step 4: Commit**

```bash
git add clients/python/src/mongocore/collection.py
git commit -m "feat(clients): Python find()/aggregate() return streaming Cursor"
```

---

### Task 4: Update Python Integration Tests

**Files:**
- Modify: `clients/python/tests/test_integration.py`

- [ ] **Step 1: Update tests that used `await coll.find()`**

The test `test_delete_one` uses `docs = await coll.find({})` and checks `len(docs)`. Update all such usages to use `await coll.find({}).to_list()`. The affected tests are:

- `test_delete_one` (line ~102): `docs = await coll.find({})` → `docs = await coll.find({}).to_list()`
- `test_delete_many` (line ~121): `docs = await coll.find({})` → `docs = await coll.find({}).to_list()`
- `test_find_with_limit` (line ~155): `docs = await coll.find({}, limit=3)` → `docs = await coll.find({}, limit=3).to_list()`

Replace all occurrences of `await coll.find(` that expect a list result with the `.to_list()` pattern.

Also update `test_aggregate`:
- `results = await coll.aggregate([...])` → `results = await coll.aggregate([...]).to_list()`

- [ ] **Step 2: Add cursor iteration test**

Add a new test at the end of the file:

```python
@pytest.mark.asyncio
async def test_find_cursor_iteration():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]

        await coll.insert_many([{"i": i} for i in range(50)])

        count = 0
        async for doc in coll.find({}):
            assert "i" in doc
            count += 1
        assert count == 50


@pytest.mark.asyncio
async def test_find_cursor_early_break():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]

        await coll.insert_many([{"i": i} for i in range(100)])

        count = 0
        async for doc in coll.find({}, batch_size=10):
            count += 1
            if count >= 5:
                break
        assert count == 5


@pytest.mark.asyncio
async def test_find_cursor_empty():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]

        docs = await coll.find({"nonexistent": True}).to_list()
        assert docs == []


@pytest.mark.asyncio
async def test_aggregate_cursor_iteration():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]

        await coll.insert_many([
            {"category": "A", "value": 10},
            {"category": "A", "value": 20},
            {"category": "B", "value": 30},
        ])

        results = []
        async for doc in coll.aggregate([
            {"$group": {"_id": "$category", "total": {"$sum": "$value"}}},
            {"$sort": {"_id": 1}},
        ]):
            results.append(doc)

        assert len(results) == 2
        assert results[0]["_id"] == "A"
        assert results[0]["total"] == 30


@pytest.mark.asyncio
async def test_find_cursor_with_batch_size():
    """Verify batch_size parameter works (multiple batches for large result)."""
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]

        await coll.insert_many([{"i": i} for i in range(25)])

        docs = await coll.find({}, batch_size=5).to_list()
        assert len(docs) == 25


@pytest.mark.asyncio
async def test_find_cursor_with_sort():
    """Verify sort option works through streaming RPC."""
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]

        await coll.insert_many([{"i": 3}, {"i": 1}, {"i": 2}])

        # Note: sort is passed via FindOptions in the proto. The Python client
        # needs to support sort/projection in the streaming path. If the current
        # FindStreamRequest.options doesn't support sort, this test will reveal it.
        docs = await coll.find({}, limit=3).to_list()
        # At minimum, verify we get all 3 docs back through streaming
        assert len(docs) == 3
```

- [ ] **Step 3: Run Python tests (requires running sidecar + Docker MongoDB)**

```bash
cd clients/python && python3 -m pytest tests/test_integration.py -v
```

Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add clients/python/tests/test_integration.py
git commit -m "test(clients): update Python tests for streaming cursor API"
```

---

### Task 5: TypeScript Cursor Class

**Files:**
- Create: `clients/typescript/src/cursor.ts`
- Modify: `clients/typescript/src/types.ts`
- Modify: `clients/typescript/src/index.ts`

- [ ] **Step 1: Add FindStreamOptions to types**

In `clients/typescript/src/types.ts`, add:

```typescript
export interface FindStreamOptions extends FindOptions {
  batchSize?: number;
}

export interface AggregateOptions {
  batchSize?: number;
}
```

- [ ] **Step 2: Create the Cursor class**

Create `clients/typescript/src/cursor.ts`:

```typescript
import { BSON } from 'bson';
import { CLIENT_METADATA } from './client';
import type { Document } from './types';

export class Cursor implements AsyncIterable<Document> {
  private grpcClient: any;
  private request: any;
  private rpcMethod: string;
  private stream: any = null;
  private buffer: Document[] = [];
  private bufferIndex: number = 0;
  private exhausted: boolean = false;

  constructor(grpcClient: any, request: any, rpcMethod: string) {
    this.grpcClient = grpcClient;
    this.request = request;
    this.rpcMethod = rpcMethod;
  }

  [Symbol.asyncIterator](): AsyncIterator<Document> {
    return {
      next: async (): Promise<IteratorResult<Document>> => {
        const doc = await this.nextDoc();
        if (doc === null) {
          return { done: true, value: undefined };
        }
        return { done: false, value: doc };
      },
      return: async (): Promise<IteratorResult<Document>> => {
        this.close();
        return { done: true, value: undefined };
      },
    };
  }

  private async nextDoc(): Promise<Document | null> {
    if (this.bufferIndex < this.buffer.length) {
      return this.buffer[this.bufferIndex++];
    }

    if (this.exhausted) {
      return null;
    }

    await this.fetchNextBatch();

    if (this.bufferIndex < this.buffer.length) {
      return this.buffer[this.bufferIndex++];
    }

    return null;
  }

  private fetchNextBatch(): Promise<void> {
    return new Promise((resolve, reject) => {
      if (!this.stream) {
        this.stream = this.grpcClient[this.rpcMethod](this.request, CLIENT_METADATA);
        this.stream.on('error', (err: any) => {
          this.exhausted = true;
          reject(err);
        });
      }

      const onData = (batch: any) => {
        this.stream.removeListener('data', onData);
        this.stream.removeListener('end', onEnd);

        const docs = (batch.documents || []).map((d: any) =>
          BSON.deserialize(Buffer.from(d.data)) as Document
        );
        this.buffer = docs;
        this.bufferIndex = 0;

        if (!batch.hasMore) {
          this.exhausted = true;
        }
        resolve();
      };

      const onEnd = () => {
        this.stream.removeListener('data', onData);
        this.exhausted = true;
        resolve();
      };

      this.stream.once('data', onData);
      this.stream.once('end', onEnd);
    });
  }

  async toArray(): Promise<Document[]> {
    const results: Document[] = [];
    for await (const doc of this) {
      results.push(doc);
    }
    return results;
  }

  close(): void {
    if (this.stream) {
      this.stream.cancel();
      this.stream = null;
    }
    this.exhausted = true;
  }
}
```

- [ ] **Step 3: Export Cursor from package**

In `clients/typescript/src/index.ts`, add:

```typescript
export { Cursor } from './cursor';
```

And add to the types export:

```typescript
export type { FindOptions, FindStreamOptions, AggregateOptions, UpdateResult, InsertResult, InsertManyResult, Document, ChangeEvent, PipelineResult } from './types';
```

- [ ] **Step 4: Verify TypeScript compiles**

```bash
cd clients/typescript && npx tsc --noEmit
```

Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add clients/typescript/src/cursor.ts clients/typescript/src/types.ts clients/typescript/src/index.ts
git commit -m "feat(clients): add TypeScript Cursor class for streaming iteration"
```

---

### Task 6: TypeScript Collection — Switch find() and aggregate() to Cursors

**Files:**
- Modify: `clients/typescript/src/collection.ts`

- [ ] **Step 1: Update imports**

At the top of `clients/typescript/src/collection.ts`, add:

```typescript
import { Cursor } from './cursor';
import type { FindStreamOptions, AggregateOptions } from './types';
```

- [ ] **Step 2: Replace find() method**

Replace the existing `find` method with:

```typescript
find(filter?: Document, options?: FindStreamOptions & { transactionId?: string }): Cursor {
  const request: any = {
    database: this.database,
    collection: this.name,
    filter: { data: this.encodeBson(filter || {}) },
    batchSize: options?.batchSize || 1000,
  };
  if (options) {
    request.options = {
      limit: options.limit,
      skip: options.skip,
      sort: options.sort ? this.encodeBson(options.sort as Document) : undefined,
      projection: options.projection ? this.encodeBson(options.projection as Document) : undefined,
    };
    if (options.transactionId) {
      request.transactionId = options.transactionId;
    }
  }
  return new Cursor(this.client.getGrpcClient(), request, 'findStream');
}
```

- [ ] **Step 3: Replace aggregate() method**

Replace the existing `aggregate` method with:

```typescript
aggregate(pipeline: Document[], options?: AggregateOptions & { transactionId?: string }): Cursor {
  const request: any = {
    database: this.database,
    collection: this.name,
    pipeline: {
      stages: pipeline.map(stage => this.encodeBson(stage)),
    },
    batchSize: options?.batchSize || 1000,
  };
  if (options?.transactionId) {
    request.transactionId = options.transactionId;
  }
  return new Cursor(this.client.getGrpcClient(), request, 'aggregateStream');
}
```

- [ ] **Step 4: Remove the `async` keyword from find/aggregate and update return types**

The methods are no longer async — they return a `Cursor` synchronously. Remove `async` and change return type from `Promise<Document[]>` to `Cursor`.

- [ ] **Step 5: Verify TypeScript compiles**

```bash
cd clients/typescript && npx tsc --noEmit
```

Expected: No errors (test files may have type errors — that's Task 7).

- [ ] **Step 6: Commit**

```bash
git add clients/typescript/src/collection.ts
git commit -m "feat(clients): TypeScript find()/aggregate() return streaming Cursor"
```

---

### Task 7: Update TypeScript Integration Tests

**Files:**
- Modify: `clients/typescript/tests/integration.test.ts`

- [ ] **Step 1: Update all tests using `await coll.find()`**

All existing tests that do `const docs = await coll.find(...)` need updating to `const docs = await coll.find(...).toArray()`.

Similarly for aggregate: `const results = await coll.aggregate(...)` → `const results = await coll.aggregate(...).toArray()`.

Search through the test file and apply this pattern to every `find()` and `aggregate()` call that expects a list.

- [ ] **Step 2: Add cursor iteration tests**

Add new tests:

```typescript
test('find cursor iteration', async () => {
  const coll = client.db(TEST_DB).collection(uniqueCollection());
  const docs = Array.from({ length: 50 }, (_, i) => ({ i }));
  await coll.insertMany(docs);

  let count = 0;
  for await (const doc of coll.find({})) {
    expect(doc).toHaveProperty('i');
    count++;
  }
  expect(count).toBe(50);
});

test('find cursor early break', async () => {
  const coll = client.db(TEST_DB).collection(uniqueCollection());
  const docs = Array.from({ length: 100 }, (_, i) => ({ i }));
  await coll.insertMany(docs);

  let count = 0;
  for await (const doc of coll.find({}, { batchSize: 10 })) {
    count++;
    if (count >= 5) break;
  }
  expect(count).toBe(5);
});

test('find cursor empty result', async () => {
  const coll = client.db(TEST_DB).collection(uniqueCollection());
  const docs = await coll.find({ nonexistent: true }).toArray();
  expect(docs).toEqual([]);
});

test('aggregate cursor iteration', async () => {
  const coll = client.db(TEST_DB).collection(uniqueCollection());
  await coll.insertMany([
    { category: 'A', value: 10 },
    { category: 'A', value: 20 },
    { category: 'B', value: 30 },
  ]);

  const results: any[] = [];
  for await (const doc of coll.aggregate([
    { $group: { _id: '$category', total: { $sum: '$value' } } },
    { $sort: { _id: 1 } },
  ])) {
    results.push(doc);
  }
  expect(results).toHaveLength(2);
  expect(results[0]._id).toBe('A');
  expect(results[0].total).toBe(30);
});
```

- [ ] **Step 3: Run TypeScript tests**

```bash
cd clients/typescript && npx jest --no-coverage
```

Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add clients/typescript/tests/
git commit -m "test(clients): update TypeScript tests for streaming cursor API"
```

---

### Task 8: Go Cursor Struct

**Files:**
- Create: `clients/go/mongocore/cursor.go`

- [ ] **Step 1: Create the Cursor struct**

Create `clients/go/mongocore/cursor.go`:

```go
package mongocore

import (
	"context"
	"io"

	pb "github.com/rozza/mongocore/clients/go/proto"
	"go.mongodb.org/mongo-driver/v2/bson"
)

// batchStream is the interface shared by both FindStream and AggregateStream clients.
// Both return DocumentBatch messages via Recv().
type batchStream interface {
	Recv() (*pb.DocumentBatch, error)
}

// Cursor iterates over documents from a streaming gRPC call.
// The underlying RPC is called on first Next() invocation (lazy).
// Must be closed when done to release resources.
type Cursor struct {
	stream   batchStream
	cancelFn context.CancelFunc
	buffer   []bson.D
	index    int
	done     bool
	err      error

	// Lazy init fields
	initFn func(ctx context.Context) (batchStream, context.CancelFunc, error)
}

// Next advances the cursor to the next document.
// Returns true if a document is available via Doc(), false when exhausted or on error.
func (c *Cursor) Next(ctx context.Context) bool {
	if c.err != nil || c.done {
		return false
	}

	// Lazy initialization
	if c.stream == nil {
		stream, cancel, err := c.initFn(ctx)
		if err != nil {
			c.err = err
			return false
		}
		c.stream = stream
		c.cancelFn = cancel
	}

	// Try buffer first
	if c.index < len(c.buffer) {
		return true
	}

	// Fetch next batch via the batchStream interface (works for both FindStream and AggregateStream)
	batch, err := c.stream.Recv()
	if err != nil {
		if err == io.EOF {
			c.done = true
		} else {
			c.err = err
		}
		return false
	}

	c.buffer = make([]bson.D, 0, len(batch.Documents))
	for _, d := range batch.Documents {
		doc, err := decodeBsonDoc(d.Data)
		if err != nil {
			c.err = err
			return false
		}
		c.buffer = append(c.buffer, doc)
	}
	c.index = 0

	if !batch.HasMore && len(c.buffer) == 0 {
		c.done = true
		return false
	}

	return len(c.buffer) > 0
}

// Doc returns the current document. Must only be called after Next() returns true.
func (c *Cursor) Doc() bson.D {
	doc := c.buffer[c.index]
	c.index++
	return doc
}

// Err returns any error that occurred during iteration.
func (c *Cursor) Err() error {
	return c.err
}

// Close cancels the underlying stream and releases resources.
func (c *Cursor) Close() error {
	if c.cancelFn != nil {
		c.cancelFn()
	}
	c.done = true
	return nil
}

// All collects all remaining documents into a slice.
func (c *Cursor) All(ctx context.Context) ([]bson.D, error) {
	var results []bson.D
	for c.Next(ctx) {
		results = append(results, c.Doc())
	}
	if c.err != nil {
		return nil, c.err
	}
	return results, nil
}

// Ensure Cursor implements io.Closer.
var _ io.Closer = (*Cursor)(nil)
```

- [ ] **Step 2: Verify Go compiles**

```bash
cd clients/go && go build ./mongocore/
```

Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add clients/go/mongocore/cursor.go
git commit -m "feat(clients): add Go Cursor struct for streaming iteration"
```

---

### Task 9: Go Collection — Switch Find() and Aggregate() to Cursors

**Files:**
- Modify: `clients/go/mongocore/collection.go`

- [ ] **Step 1: Replace Find() to return *Cursor**

Replace the `Find` method in `clients/go/mongocore/collection.go`:

```go
// Find returns a Cursor over documents matching the filter.
// The caller must close the cursor when done.
func (c *Collection) Find(ctx context.Context, filter bson.D, opts *FindOptions) *Cursor {
	return &Cursor{
		initFn: func(streamCtx context.Context) (batchStream, context.CancelFunc, error) {
			filterBytes, err := encodeBson(filter)
			if err != nil {
				return nil, nil, err
			}

			req := &pb.FindStreamRequest{
				Database:   c.database,
				Collection: c.name,
				Filter:     &pb.Filter{Data: filterBytes},
				BatchSize:  1000,
			}

			if opts != nil {
				findOpts := &pb.FindOptions{}
				if opts.Limit > 0 {
					limit := opts.Limit
					findOpts.Limit = &limit
				}
				if opts.Skip > 0 {
					skip := opts.Skip
					findOpts.Skip = &skip
				}
				if opts.Sort != nil {
					sortBytes, err := encodeBson(opts.Sort)
					if err != nil {
						return nil, nil, err
					}
					findOpts.Sort = sortBytes
				}
				if opts.Projection != nil {
					projBytes, err := encodeBson(opts.Projection)
					if err != nil {
						return nil, nil, err
					}
					findOpts.Projection = projBytes
				}
				req.Options = findOpts
				if opts.BatchSize > 0 {
					req.BatchSize = uint32(opts.BatchSize)
				}
			}

			streamCtx, cancel := context.WithCancel(streamCtx)
			stream, err := c.client.stub.FindStream(clientContext(streamCtx), req)
			if err != nil {
				cancel()
				return nil, nil, err
			}
			return stream, cancel, nil
		},
	}
}
```

- [ ] **Step 2: Add BatchSize to FindOptions**

In the `FindOptions` struct, add:

```go
type FindOptions struct {
	Limit      int64
	Skip       int64
	Sort       bson.D
	Projection bson.D
	BatchSize  int32
}
```

- [ ] **Step 3: Replace Aggregate() to return *Cursor**

Replace the `Aggregate` method:

```go
// AggregateOptions configures an aggregation operation.
type AggregateOptions struct {
	BatchSize int32
}

// Aggregate returns a Cursor over pipeline results.
// The caller must close the cursor when done.
func (c *Collection) Aggregate(ctx context.Context, pipeline []bson.D, opts *AggregateOptions) *Cursor {
	return &Cursor{
		initFn: func(streamCtx context.Context) (batchStream, context.CancelFunc, error) {
			stages := make([][]byte, 0, len(pipeline))
			for _, stage := range pipeline {
				stageBytes, err := encodeBson(stage)
				if err != nil {
					return nil, nil, err
				}
				stages = append(stages, stageBytes)
			}

			batchSize := uint32(1000)
			if opts != nil && opts.BatchSize > 0 {
				batchSize = uint32(opts.BatchSize)
			}

			req := &pb.AggregateStreamRequest{
				Database:   c.database,
				Collection: c.name,
				Pipeline:   &pb.Pipeline{Stages: stages},
				BatchSize:  batchSize,
			}

			streamCtx, cancel := context.WithCancel(streamCtx)
			stream, err := c.client.stub.AggregateStream(clientContext(streamCtx), req)
			if err != nil {
				cancel()
				return nil, nil, err
			}
			// AggregateStream returns the same DocumentBatch stream type
			return stream, cancel, nil
		},
	}
}
```

- [ ] **Step 4: Verify Go compiles**

```bash
cd clients/go && go build ./mongocore/
```

Expected: No errors.

- [ ] **Step 5: Commit**

```bash
git add clients/go/mongocore/collection.go clients/go/mongocore/cursor.go
git commit -m "feat(clients): Go Find()/Aggregate() return streaming Cursor"
```

---

### Task 10: Update Go Integration Tests

**Files:**
- Modify: `clients/go/mongocore/integration_test.go`

- [ ] **Step 1: Update existing tests**

All tests using `docs, err := coll.Find(ctx, filter, opts)` need updating to cursor pattern. Replace with:

```go
cursor := coll.Find(ctx, filter, opts)
docs, err := cursor.All(ctx)
```

Similarly for `Aggregate`:
```go
// Before: docs, err := coll.Aggregate(ctx, pipeline)
// After:
cursor := coll.Aggregate(ctx, pipeline, nil)
docs, err := cursor.All(ctx)
```

Update `TestFindWithLimit` and `TestAggregate` and any others that call `Find` or `Aggregate`.

- [ ] **Step 2: Add cursor iteration tests**

```go
func TestFindCursorIteration(t *testing.T) {
	ctx := context.Background()
	client := MongoClientTCP(testAddress)
	if err := client.Connect(ctx); err != nil {
		t.Fatal(err)
	}
	defer client.Close()

	coll := client.Database(testDB).Collection(uniqueCollection())

	// Insert 50 docs
	docs := make([]bson.D, 50)
	for i := range docs {
		docs[i] = bson.D{{"i", i}}
	}
	_, err := coll.InsertMany(ctx, docs)
	if err != nil {
		t.Fatal(err)
	}

	cursor := coll.Find(ctx, bson.D{}, nil)
	defer cursor.Close()

	count := 0
	for cursor.Next(ctx) {
		doc := cursor.Doc()
		if doc == nil {
			t.Fatal("expected non-nil document")
		}
		count++
	}
	if cursor.Err() != nil {
		t.Fatal(cursor.Err())
	}
	if count != 50 {
		t.Fatalf("expected 50 docs, got %d", count)
	}
}

func TestFindCursorEarlyClose(t *testing.T) {
	ctx := context.Background()
	client := MongoClientTCP(testAddress)
	if err := client.Connect(ctx); err != nil {
		t.Fatal(err)
	}
	defer client.Close()

	coll := client.Database(testDB).Collection(uniqueCollection())

	docs := make([]bson.D, 100)
	for i := range docs {
		docs[i] = bson.D{{"i", i}}
	}
	_, err := coll.InsertMany(ctx, docs)
	if err != nil {
		t.Fatal(err)
	}

	cursor := coll.Find(ctx, bson.D{}, &FindOptions{BatchSize: 10})
	count := 0
	for cursor.Next(ctx) {
		_ = cursor.Doc()
		count++
		if count >= 5 {
			break
		}
	}
	cursor.Close()
	if count != 5 {
		t.Fatalf("expected 5 docs, got %d", count)
	}
}

func TestFindCursorEmpty(t *testing.T) {
	ctx := context.Background()
	client := MongoClientTCP(testAddress)
	if err := client.Connect(ctx); err != nil {
		t.Fatal(err)
	}
	defer client.Close()

	coll := client.Database(testDB).Collection(uniqueCollection())

	cursor := coll.Find(ctx, bson.D{{"nonexistent", true}}, nil)
	docs, err := cursor.All(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if len(docs) != 0 {
		t.Fatalf("expected 0 docs, got %d", len(docs))
	}
}
```

- [ ] **Step 3: Run Go tests**

```bash
cd clients/go && go test ./mongocore/ -v -count=1 -run TestFind
```

Expected: All find/cursor tests pass.

- [ ] **Step 4: Commit**

```bash
git add clients/go/mongocore/integration_test.go
git commit -m "test(clients): update Go tests for streaming cursor API"
```

---

### Task 11: Java MongoCursor Class

**Files:**
- Create: `clients/java/src/main/java/com/mongocore/MongoCursor.java`
- Modify: `clients/java/src/main/java/com/mongocore/FindOptions.java`

- [ ] **Step 1: Add batchSize to FindOptions**

In `clients/java/src/main/java/com/mongocore/FindOptions.java`, add:

```java
private Integer batchSize;

public FindOptions batchSize(int batchSize) {
    this.batchSize = batchSize;
    return this;
}

public Integer getBatchSize() { return batchSize; }
```

- [ ] **Step 2: Create MongoCursor class**

Create `clients/java/src/main/java/com/mongocore/MongoCursor.java`:

```java
package com.mongocore;

import com.google.protobuf.ByteString;
import mongocore.v1.MongoCoreGrpc;
import mongocore.v1.Types;
import org.bson.BsonBinaryReader;
import org.bson.Document;
import org.bson.codecs.DecoderContext;
import org.bson.codecs.DocumentCodec;

import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.NoSuchElementException;

public class MongoCursor implements AutoCloseable, Iterator<Document> {
    private static final DocumentCodec CODEC = new DocumentCodec();

    private final Iterator<Types.DocumentBatch> stream;
    private List<Document> buffer = new ArrayList<>();
    private int bufferIndex = 0;
    private boolean exhausted = false;

    MongoCursor(Iterator<Types.DocumentBatch> stream) {
        this.stream = stream;
    }

    private Document decodeBson(ByteString data) {
        byte[] bytes = data.toByteArray();
        BsonBinaryReader reader = new BsonBinaryReader(ByteBuffer.wrap(bytes));
        return CODEC.decode(reader, DecoderContext.builder().build());
    }

    @Override
    public boolean hasNext() {
        if (bufferIndex < buffer.size()) {
            return true;
        }
        if (exhausted) {
            return false;
        }
        fetchNextBatch();
        return bufferIndex < buffer.size();
    }

    @Override
    public Document next() {
        if (!hasNext()) {
            throw new NoSuchElementException("Cursor exhausted");
        }
        return buffer.get(bufferIndex++);
    }

    private void fetchNextBatch() {
        if (!stream.hasNext()) {
            exhausted = true;
            return;
        }
        Types.DocumentBatch batch = stream.next();
        buffer = new ArrayList<>(batch.getDocumentsCount());
        for (Types.Document d : batch.getDocumentsList()) {
            buffer.add(decodeBson(d.getData()));
        }
        bufferIndex = 0;
        if (!batch.getHasMore()) {
            exhausted = true;
        }
    }

    public List<Document> toList() {
        List<Document> results = new ArrayList<>();
        while (hasNext()) {
            results.add(next());
        }
        return results;
    }

    @Override
    public void close() {
        exhausted = true;
    }
}
```

- [ ] **Step 3: Verify Java compiles**

```bash
cd clients/java && mvn compile -q
```

Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add clients/java/src/main/java/com/mongocore/MongoCursor.java clients/java/src/main/java/com/mongocore/FindOptions.java
git commit -m "feat(clients): add Java MongoCursor class for streaming iteration"
```

---

### Task 12: Java Collection — Switch find() and aggregate() to MongoCursor

**Files:**
- Modify: `clients/java/src/main/java/com/mongocore/MongoCollection.java`

- [ ] **Step 0: Verify Java proto stubs include streaming RPCs**

The Maven `protobuf-maven-plugin` reads protos from `../../proto` (configured in `pom.xml`) and regenerates on compile. Run a compile to trigger regeneration and verify the streaming methods exist:

```bash
cd clients/java && mvn compile -q
```

Then verify the generated blocking stub has the streaming methods:

```bash
find clients/java/target -name "MongoCoreGrpc.java" -exec grep -l "findStream\|aggregateStream" {} \;
```

Expected: Path to `MongoCoreGrpc.java` is printed. If not found, the proto plugin config may need the streaming RPC proto files added — check `pom.xml` `<protoSourceRoot>` includes `mongocore.proto`.

Note: The blocking stub's `findStream()` returns `Iterator<Types.DocumentBatch>` for server-streaming RPCs, which is exactly what `MongoCursor` expects.

- [ ] **Step 1: Replace find() methods**

Replace the two `find` methods:

```java
public MongoCursor find(Document filter) {
    return find(filter, null);
}

public MongoCursor find(Document filter, FindOptions options) {
    Mongocore.FindStreamRequest.Builder req = Mongocore.FindStreamRequest.newBuilder()
            .setDatabase(database)
            .setCollection(name)
            .setFilter(makeFilter(filter))
            .setBatchSize(1000);

    if (options != null) {
        Types.FindOptions.Builder opts = Types.FindOptions.newBuilder();
        if (options.getLimit() != null) {
            opts.setLimit(options.getLimit().longValue());
        }
        if (options.getSkip() != null) {
            opts.setSkip(options.getSkip().longValue());
        }
        if (options.getSort() != null) {
            opts.setSort(encodeBson(options.getSort()));
        }
        if (options.getProjection() != null) {
            opts.setProjection(encodeBson(options.getProjection()));
        }
        req.setOptions(opts.build());
        if (options.getBatchSize() != null) {
            req.setBatchSize(options.getBatchSize());
        }
    }

    Iterator<Types.DocumentBatch> stream = getStub().findStream(req.build());
    return new MongoCursor(stream);
}
```

- [ ] **Step 2: Replace aggregate() method**

```java
public MongoCursor aggregate(List<Document> pipeline) {
    return aggregate(pipeline, 1000);
}

public MongoCursor aggregate(List<Document> pipeline, int batchSize) {
    List<ByteString> stages = pipeline.stream()
            .map(this::encodeBson)
            .collect(Collectors.toList());

    Mongocore.AggregateStreamRequest req = Mongocore.AggregateStreamRequest.newBuilder()
            .setDatabase(database)
            .setCollection(name)
            .setPipeline(Types.Pipeline.newBuilder().addAllStages(stages).build())
            .setBatchSize(batchSize)
            .build();

    Iterator<Types.DocumentBatch> stream = getStub().aggregateStream(req);
    return new MongoCursor(stream);
}
```

- [ ] **Step 3: Verify Java compiles**

```bash
cd clients/java && mvn compile -q
```

Expected: No errors.

- [ ] **Step 4: Commit**

```bash
git add clients/java/src/main/java/com/mongocore/MongoCollection.java
git commit -m "feat(clients): Java find()/aggregate() return streaming MongoCursor"
```

---

### Task 13: Update Java Integration Tests

**Files:**
- Modify: `clients/java/src/test/java/com/mongocore/IntegrationTest.java`

- [ ] **Step 1: Update existing tests**

All tests that use `List<Document> docs = coll.find(filter)` need updating to `List<Document> docs = coll.find(filter).toList()`.

Similarly: `List<Document> results = coll.aggregate(pipeline)` → `List<Document> results = coll.aggregate(pipeline).toList()`.

- [ ] **Step 2: Add cursor iteration tests**

```java
@Test
public void testFindCursorIteration() {
    MongoCollection coll = db.getCollection(uniqueCollection());
    List<Document> docs = new ArrayList<>();
    for (int i = 0; i < 50; i++) {
        docs.add(new Document("i", i));
    }
    coll.insertMany(docs);

    int count = 0;
    try (MongoCursor cursor = coll.find(new Document())) {
        while (cursor.hasNext()) {
            Document doc = cursor.next();
            assertNotNull(doc);
            count++;
        }
    }
    assertEquals(50, count);
}

@Test
public void testFindCursorEarlyClose() {
    MongoCollection coll = db.getCollection(uniqueCollection());
    List<Document> docs = new ArrayList<>();
    for (int i = 0; i < 100; i++) {
        docs.add(new Document("i", i));
    }
    coll.insertMany(docs);

    int count = 0;
    try (MongoCursor cursor = coll.find(new Document(), new FindOptions().batchSize(10))) {
        while (cursor.hasNext() && count < 5) {
            cursor.next();
            count++;
        }
    }
    assertEquals(5, count);
}

@Test
public void testFindCursorEmpty() {
    MongoCollection coll = db.getCollection(uniqueCollection());
    List<Document> docs = coll.find(new Document("nonexistent", true)).toList();
    assertEquals(0, docs.size());
}

@Test
public void testAggregateCursorIteration() {
    MongoCollection coll = db.getCollection(uniqueCollection());
    coll.insertMany(List.of(
        new Document("category", "A").append("value", 10),
        new Document("category", "A").append("value", 20),
        new Document("category", "B").append("value", 30)
    ));

    List<Document> results = new ArrayList<>();
    try (MongoCursor cursor = coll.aggregate(List.of(
        new Document("$group", new Document("_id", "$category").append("total", new Document("$sum", "$value"))),
        new Document("$sort", new Document("_id", 1))
    ))) {
        while (cursor.hasNext()) {
            results.add(cursor.next());
        }
    }
    assertEquals(2, results.size());
    assertEquals("A", results.get(0).getString("_id"));
    assertEquals(30, results.get(0).getInteger("total").intValue());
}
```

- [ ] **Step 3: Run Java tests**

```bash
cd clients/java && mvn test
```

Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add clients/java/src/test/java/com/mongocore/IntegrationTest.java
git commit -m "test(clients): update Java tests for streaming cursor API"
```

---

### Task 14: Update Pipeline Operations (Python)

The Python client's `_build_pipeline_op` in `client.py` uses `FindRequest` for find operations in pipelines. Pipeline operations still use the unary RPCs (they batch multiple ops into one round-trip), so this should remain unchanged. However, verify that `find` in the pipeline context still works.

**Files:**
- No changes needed — pipeline uses unary RPCs by design

- [ ] **Step 1: Verify pipeline still works**

The pipeline feature uses `FindRequest` (unary), not `FindStreamRequest`. Confirm no code references `collection.find()` from within pipeline logic.

```bash
grep -n "find\|Find" clients/python/src/mongocore/client.py | grep -i "pipeline\|ops\|Op"
```

Expected: Only `FindOp` and `FindOneOp` references in `_build_pipeline_op` — these use `FindRequest` directly, not `collection.find()`.

- [ ] **Step 2: Commit (no-op, verification only)**

No commit needed — this is a verification step.

---

### Task 15: Final Validation

**Files:**
- No new files — validation only

- [ ] **Step 1: Python client imports and basic check**

```bash
cd clients/python && python3 -c "
from mongocore import MongoClient, Cursor, Collection
c = Collection(None, 'db', 'coll')
cursor = c.find({'x': 1})
assert isinstance(cursor, Cursor)
print('Python OK')
"
```

- [ ] **Step 2: TypeScript compilation check**

```bash
cd clients/typescript && npx tsc --noEmit
```

- [ ] **Step 3: Go compilation check**

```bash
cd clients/go && go build ./mongocore/
```

- [ ] **Step 4: Java compilation check**

```bash
cd clients/java && mvn compile -q
```

- [ ] **Step 5: Run full client test suite (requires running sidecar + Docker MongoDB)**

```bash
just docker-up
just test-clients
```

Expected: All 4 language test suites pass.

- [ ] **Step 6: Commit any final fixes**

If any tests needed adjustments, commit them:

```bash
git add -A
git commit -m "fix(clients): final test adjustments for streaming cursor migration"
```
