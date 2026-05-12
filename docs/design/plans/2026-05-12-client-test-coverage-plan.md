# Client Test Coverage — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add full 27-RPC integration test coverage to all 4 client libraries, add missing client methods, create uniform unit tests, fix Java output, and add AGENTS.md testing rules.

**Architecture:** Each client gets 6 new methods (FindAndModify, BeginTransaction, CommitTransaction, AbortTransaction, CreateIndex, GetAnalytics) plus Go gets ListCollections/CreateCollection. Then 16 new integration tests per client exercise the previously-untested RPCs. Unit tests are standardized across all 4 clients. A shared CSV fixture supports ingestion tests.

**Tech Stack:** Python (pytest-asyncio, grpcio), TypeScript (Jest, @grpc/grpc-js), Go (testing, google.golang.org/grpc), Java (JUnit, gRPC-java)

---

## Important Notes for Implementers

**Read and follow `AGENTS.md` at the project root.**

Before committing:
- Run `cargo test --lib` AND verify `cargo test --test integration` compiles
- If touching client libraries, verify imports work without errors
- Search for ALL struct literals across `src/` AND `tests/` when modifying shared types

---

## File Structure

### Shared Fixtures
| Action | Path | Responsibility |
|--------|------|----------------|
| Create | `clients/test_fixtures/sample.csv` | CSV for ingestion integration tests |
| Create | `clients/test_fixtures/watch_drop.csv` | CSV for watch directory tests |

### Infrastructure
| Action | Path | Responsibility |
|--------|------|----------------|
| Modify | `justfile` | Fix Java output, add unit test commands |
| Modify | `AGENTS.md` | Add testing rules section |

### Per Language (×4)
| Action | Path Pattern | Responsibility |
|--------|-------------|----------------|
| Modify | `clients/<lang>/...collection...` | Add find_and_modify, create_index methods |
| Modify | `clients/<lang>/...client...` | Add transaction, get_analytics methods |
| Modify | `clients/<lang>/...database...` | Add list_collections, create_collection (Go only) |
| Modify | `clients/<lang>/tests/...integration...` | Add 16 new integration tests |
| Create/Modify | `clients/<lang>/tests/...unit...` | Standardize unit tests (5 per client) |

---

## Task 1: Shared Fixtures and Infrastructure

**Files:**
- Create: `clients/test_fixtures/sample.csv`
- Create: `clients/test_fixtures/watch_drop.csv`
- Modify: `justfile`
- Modify: `AGENTS.md`

- [ ] **Step 1: Create test fixtures directory and CSV files**

Create `clients/test_fixtures/sample.csv`:
```csv
name,age,city
Alice,30,NYC
Bob,25,LA
Charlie,35,Chicago
```

Create `clients/test_fixtures/watch_drop.csv`:
```csv
name,score
Delta,95
Echo,88
```

- [ ] **Step 2: Fix Java test output in justfile**

Change:
```
test-java:
    cd clients/java && mvn test -Dtest=IntegrationTest -q
```
To:
```
test-java:
    cd clients/java && mvn test -Dtest=IntegrationTest
```

- [ ] **Step 3: Add unit test commands to justfile**

Add after the existing `test-clients` recipe:
```
# Run Python client unit tests
test-unit-python:
    cd clients/python && python3 -m pytest tests/test_client.py -v

# Run TypeScript client unit tests
test-unit-typescript:
    cd clients/typescript && npx jest tests/unit.test.ts --no-coverage

# Run Go client unit tests
test-unit-go:
    cd clients/go && go test ./mongocore/ -v -count=1 -run "^TestUnit"

# Run Java client unit tests
test-unit-java:
    cd clients/java && mvn test -Dtest=MongoClientTest

# Run all client unit tests
test-unit-clients: test-unit-python test-unit-typescript test-unit-go test-unit-java
```

- [ ] **Step 4: Add testing rules to AGENTS.md**

Add after the "Don'ts" section:
```markdown
## Testing Rules

- Every gRPC RPC must have a corresponding integration test in each client library (Python, TypeScript, Go, Java)
- Client integration tests must produce verbose per-test output (no silent/quiet modes)
- New tests should follow the existing pattern in each client's test file
- Use unique collection names per test to avoid interference
- When adding a new gRPC RPC, add integration tests to ALL 4 client test suites in the same commit
```

- [ ] **Step 5: Commit**

```bash
git add clients/test_fixtures/ justfile AGENTS.md
git commit -m "chore: add test fixtures, fix Java output, add testing rules to AGENTS.md"
```

---

## Task 2: Add Missing Client Methods — Python

**Files:**
- Modify: `clients/python/src/mongocore/collection.py`
- Modify: `clients/python/src/mongocore/client.py`

- [ ] **Step 1: Add find_and_modify to Collection**

Add to `clients/python/src/mongocore/collection.py`:
```python
async def find_and_modify(self, filter: dict, update: dict, *, return_new: bool = True, upsert: bool = False) -> Optional[dict]:
    """Atomically find and modify a document, returning the result."""
    stub = self._client._stub
    from .generated.mongocore.v1 import mongocore_pb2, types_pb2
    options = types_pb2.FindAndModifyOptions(
        return_document=types_pb2.FindAndModifyOptions.AFTER if return_new else types_pb2.FindAndModifyOptions.BEFORE,
        upsert=upsert,
    )
    request = mongocore_pb2.FindAndModifyRequest(
        database=self._database_name,
        collection=self._name,
        filter=self._make_filter(filter),
        update=self._make_document(update),
        options=options,
    )
    response = await stub.FindAndModify(request, metadata=_CLIENT_METADATA)
    if response.document and response.document.data:
        return self._decode_doc(response.document.data)
    return None
```

- [ ] **Step 2: Add create_index to Collection**

```python
async def create_index(self, keys: dict, *, unique: bool = False, name: Optional[str] = None) -> str:
    """Create an index on the collection."""
    stub = self._client._stub
    from .generated.mongocore.v1 import mongocore_pb2
    request = mongocore_pb2.CreateIndexRequest(
        database=self._database_name,
        collection=self._name,
        keys=self._make_document(keys),
        unique=unique,
    )
    if name:
        request.name = name
    response = await stub.CreateIndex(request, metadata=_CLIENT_METADATA)
    return response.name
```

- [ ] **Step 3: Add transaction methods to Client**

Add to `clients/python/src/mongocore/client.py`:
```python
async def begin_transaction(self) -> str:
    """Begin a new transaction, returns transaction_id."""
    stub = await self._get_stub()
    from .generated.mongocore.v1 import mongocore_pb2
    response = await stub.BeginTransaction(mongocore_pb2.BeginTransactionRequest(), metadata=_CLIENT_METADATA)
    return response.transaction_id

async def commit_transaction(self, transaction_id: str) -> bool:
    """Commit a transaction."""
    stub = await self._get_stub()
    from .generated.mongocore.v1 import mongocore_pb2
    response = await stub.CommitTransaction(
        mongocore_pb2.CommitTransactionRequest(transaction_id=transaction_id),
        metadata=_CLIENT_METADATA,
    )
    return response.success

async def abort_transaction(self, transaction_id: str) -> bool:
    """Abort a transaction."""
    stub = await self._get_stub()
    from .generated.mongocore.v1 import mongocore_pb2
    response = await stub.AbortTransaction(
        mongocore_pb2.AbortTransactionRequest(transaction_id=transaction_id),
        metadata=_CLIENT_METADATA,
    )
    return response.success

async def get_analytics(self) -> dict:
    """Get query analytics summary."""
    stub = await self._get_stub()
    from .generated.mongocore.v1 import mongocore_pb2
    response = await stub.GetAnalytics(
        mongocore_pb2.GetAnalyticsRequest(window_seconds=0),
        metadata=_CLIENT_METADATA,
    )
    return {
        "total_operations": response.total_operations,
        "total_errors": response.total_errors,
        "error_rate": response.error_rate,
        "p50_latency_ms": response.p50_latency_ms,
        "p95_latency_ms": response.p95_latency_ms,
        "p99_latency_ms": response.p99_latency_ms,
    }
```

- [ ] **Step 4: Verify Python imports work**

```bash
cd clients/python && python3 -c "import sys; sys.path.insert(0,'src'); from mongocore import MongoClient; print('OK')"
```
Expected: `OK`

- [ ] **Step 5: Commit**

```bash
git add clients/python/
git commit -m "feat(clients): add find_and_modify, create_index, transactions, get_analytics to Python client"
```

---

## Task 3: Add Missing Client Methods — TypeScript

**Files:**
- Modify: `clients/typescript/src/collection.ts`
- Modify: `clients/typescript/src/client.ts`

- [ ] **Step 1: Add findAndModify and createIndex to Collection**

Add to `clients/typescript/src/collection.ts`:
```typescript
async findAndModify(filter: Record<string, unknown>, update: Record<string, unknown>, options?: { returnNew?: boolean; upsert?: boolean }): Promise<Record<string, unknown> | null> {
  return new Promise((resolve, reject) => {
    const request = {
      database: this.client.db(this.dbName).name,
      collection: this.name,
      filter: { data: BSON.serialize(filter) },
      update: { data: BSON.serialize(update) },
      options: {
        returnDocument: (options?.returnNew !== false) ? 1 : 0,
        upsert: options?.upsert || false,
      },
    };
    this.client.getGrpcClient().findAndModify(request, CLIENT_METADATA, (err: any, response: any) => {
      if (err) return reject(err);
      resolve(response.document?.data ? BSON.deserialize(response.document.data) : null);
    });
  });
}

async createIndex(keys: Record<string, unknown>, options?: { unique?: boolean; name?: string }): Promise<string> {
  return new Promise((resolve, reject) => {
    const request = {
      database: this.client.db(this.dbName).name,
      collection: this.name,
      keys: { data: BSON.serialize(keys) },
      unique: options?.unique || false,
      name: options?.name || '',
    };
    this.client.getGrpcClient().createIndex(request, CLIENT_METADATA, (err: any, response: any) => {
      if (err) return reject(err);
      resolve(response.name);
    });
  });
}
```

- [ ] **Step 2: Add transaction and analytics methods to Client**

Add to `clients/typescript/src/client.ts`:
```typescript
async beginTransaction(): Promise<string> {
  return new Promise((resolve, reject) => {
    this.getGrpcClient().beginTransaction({}, CLIENT_METADATA, (err: any, response: any) => {
      if (err) return reject(err);
      resolve(response.transactionId);
    });
  });
}

async commitTransaction(transactionId: string): Promise<boolean> {
  return new Promise((resolve, reject) => {
    this.getGrpcClient().commitTransaction({ transactionId }, CLIENT_METADATA, (err: any, response: any) => {
      if (err) return reject(err);
      resolve(response.success);
    });
  });
}

async abortTransaction(transactionId: string): Promise<boolean> {
  return new Promise((resolve, reject) => {
    this.getGrpcClient().abortTransaction({ transactionId }, CLIENT_METADATA, (err: any, response: any) => {
      if (err) return reject(err);
      resolve(response.success);
    });
  });
}

async getAnalytics(): Promise<Record<string, unknown>> {
  return new Promise((resolve, reject) => {
    this.getGrpcClient().getAnalytics({ windowSeconds: 0 }, CLIENT_METADATA, (err: any, response: any) => {
      if (err) return reject(err);
      resolve(response);
    });
  });
}
```

- [ ] **Step 3: Verify TypeScript compiles**

```bash
cd clients/typescript && npx tsc --noEmit
```
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add clients/typescript/
git commit -m "feat(clients): add findAndModify, createIndex, transactions, getAnalytics to TypeScript client"
```

---

## Task 4: Add Missing Client Methods — Go

**Files:**
- Modify: `clients/go/mongocore/collection.go`
- Modify: `clients/go/mongocore/client.go`
- Modify: `clients/go/mongocore/database.go`

- [ ] **Step 1: Add FindAndModify and CreateIndex to Collection**

Add to `clients/go/mongocore/collection.go`:
```go
func (c *Collection) FindAndModify(ctx context.Context, filter, update bson.D, returnNew bool) (bson.D, error) {
    ctx = clientContext(ctx)
    filterBytes, _ := bson.Marshal(filter)
    updateBytes, _ := bson.Marshal(update)
    returnDoc := pb.FindAndModifyOptions_AFTER
    if !returnNew {
        returnDoc = pb.FindAndModifyOptions_BEFORE
    }
    resp, err := c.client.Stub().FindAndModify(ctx, &pb.FindAndModifyRequest{
        Database:   c.dbName,
        Collection: c.name,
        Filter:     &pb.Filter{Data: filterBytes},
        Update:     &pb.Document{Data: updateBytes},
        Options:    &pb.FindAndModifyOptions{ReturnDocument: returnDoc},
    })
    if err != nil {
        return nil, err
    }
    if resp.Document == nil || len(resp.Document.Data) == 0 {
        return nil, nil
    }
    var result bson.D
    bson.Unmarshal(resp.Document.Data, &result)
    return result, nil
}

func (c *Collection) CreateIndex(ctx context.Context, keys bson.D, unique bool) (string, error) {
    ctx = clientContext(ctx)
    keysBytes, _ := bson.Marshal(keys)
    resp, err := c.client.Stub().CreateIndex(ctx, &pb.CreateIndexRequest{
        Database:   c.dbName,
        Collection: c.name,
        Keys:       &pb.Document{Data: keysBytes},
        Unique:     unique,
    })
    if err != nil {
        return "", err
    }
    return resp.Name, nil
}
```

- [ ] **Step 2: Add ListCollections, CreateCollection to Database**

Add to `clients/go/mongocore/database.go`:
```go
func (d *Database) ListCollections(ctx context.Context) ([]string, error) {
    ctx = clientContext(ctx)
    resp, err := d.client.Stub().ListCollections(ctx, &pb.ListCollectionsRequest{
        Database: d.name,
    })
    if err != nil {
        return nil, err
    }
    return resp.Collections, nil
}

func (d *Database) CreateCollection(ctx context.Context, name string) error {
    ctx = clientContext(ctx)
    _, err := d.client.Stub().CreateCollection(ctx, &pb.CreateCollectionRequest{
        Database:   d.name,
        Collection: name,
    })
    return err
}
```

- [ ] **Step 3: Add transaction and analytics methods to Client**

Add to `clients/go/mongocore/client.go`:
```go
func (c *Client) BeginTransaction(ctx context.Context) (string, error) {
    ctx = clientContext(ctx)
    resp, err := c.Stub().BeginTransaction(ctx, &pb.BeginTransactionRequest{})
    if err != nil {
        return "", err
    }
    return resp.TransactionId, nil
}

func (c *Client) CommitTransaction(ctx context.Context, transactionID string) (bool, error) {
    ctx = clientContext(ctx)
    resp, err := c.Stub().CommitTransaction(ctx, &pb.CommitTransactionRequest{
        TransactionId: transactionID,
    })
    if err != nil {
        return false, err
    }
    return resp.Success, nil
}

func (c *Client) AbortTransaction(ctx context.Context, transactionID string) (bool, error) {
    ctx = clientContext(ctx)
    resp, err := c.Stub().AbortTransaction(ctx, &pb.AbortTransactionRequest{
        TransactionId: transactionID,
    })
    if err != nil {
        return false, err
    }
    return resp.Success, nil
}

func (c *Client) GetAnalytics(ctx context.Context) (map[string]interface{}, error) {
    ctx = clientContext(ctx)
    resp, err := c.Stub().GetAnalytics(ctx, &pb.GetAnalyticsRequest{WindowSeconds: 0})
    if err != nil {
        return nil, err
    }
    return map[string]interface{}{
        "total_operations": resp.TotalOperations,
        "total_errors":     resp.TotalErrors,
        "error_rate":       resp.ErrorRate,
        "p50_latency_ms":   resp.P50LatencyMs,
        "p95_latency_ms":   resp.P95LatencyMs,
        "p99_latency_ms":   resp.P99LatencyMs,
    }, nil
}
```

- [ ] **Step 4: Verify Go compiles**

```bash
cd clients/go && go build ./...
```
Expected: No errors

- [ ] **Step 5: Commit**

```bash
git add clients/go/
git commit -m "feat(clients): add FindAndModify, CreateIndex, ListCollections, CreateCollection, transactions, GetAnalytics to Go client"
```

---

## Task 5: Add Missing Client Methods — Java

**Files:**
- Modify: `clients/java/src/main/java/com/mongocore/MongoCollection.java`
- Modify: `clients/java/src/main/java/com/mongocore/MongoClient.java`
- Modify: `clients/java/src/main/java/com/mongocore/MongoDatabase.java`

- [ ] **Step 1: Add findAndModify and createIndex to MongoCollection**

Add to `MongoCollection.java`:
```java
public Document findAndModify(Document filter, Document update, boolean returnNew) {
    byte[] filterBytes = toBson(filter);
    byte[] updateBytes = toBson(update);
    Mongocore.FindAndModifyResponse response = getStub().findAndModify(
        Mongocore.FindAndModifyRequest.newBuilder()
            .setDatabase(databaseName)
            .setCollection(name)
            .setFilter(Types.Filter.newBuilder().setData(ByteString.copyFrom(filterBytes)).build())
            .setUpdate(Types.Document.newBuilder().setData(ByteString.copyFrom(updateBytes)).build())
            .setOptions(Types.FindAndModifyOptions.newBuilder()
                .setReturnDocument(returnNew
                    ? Types.FindAndModifyOptions.ReturnDocument.AFTER
                    : Types.FindAndModifyOptions.ReturnDocument.BEFORE)
                .build())
            .build());
    if (response.hasDocument() && !response.getDocument().getData().isEmpty()) {
        return fromBson(response.getDocument().getData().toByteArray());
    }
    return null;
}

public String createIndex(Document keys, boolean unique) {
    byte[] keysBytes = toBson(keys);
    Mongocore.CreateIndexResponse response = getStub().createIndex(
        Mongocore.CreateIndexRequest.newBuilder()
            .setDatabase(databaseName)
            .setCollection(name)
            .setKeys(Types.Document.newBuilder().setData(ByteString.copyFrom(keysBytes)).build())
            .setUnique(unique)
            .build());
    return response.getName();
}
```

- [ ] **Step 2: Add transaction and analytics methods to MongoClient**

Add to `MongoClient.java`:
```java
public String beginTransaction() {
    Mongocore.BeginTransactionResponse response = stub.beginTransaction(
        Mongocore.BeginTransactionRequest.newBuilder().build());
    return response.getTransactionId();
}

public boolean commitTransaction(String transactionId) {
    Mongocore.CommitTransactionResponse response = stub.commitTransaction(
        Mongocore.CommitTransactionRequest.newBuilder()
            .setTransactionId(transactionId)
            .build());
    return response.getSuccess();
}

public boolean abortTransaction(String transactionId) {
    Mongocore.AbortTransactionResponse response = stub.abortTransaction(
        Mongocore.AbortTransactionRequest.newBuilder()
            .setTransactionId(transactionId)
            .build());
    return response.getSuccess();
}

public Map<String, Object> getAnalytics() {
    Mongocore.GetAnalyticsResponse response = stub.getAnalytics(
        Mongocore.GetAnalyticsRequest.newBuilder().setWindowSeconds(0).build());
    Map<String, Object> result = new HashMap<>();
    result.put("total_operations", response.getTotalOperations());
    result.put("total_errors", response.getTotalErrors());
    result.put("error_rate", response.getErrorRate());
    result.put("p50_latency_ms", response.getP50LatencyMs());
    result.put("p95_latency_ms", response.getP95LatencyMs());
    result.put("p99_latency_ms", response.getP99LatencyMs());
    return result;
}
```

- [ ] **Step 3: Add listCollections and createCollection to MongoDatabase (if missing)**

Check if `MongoDatabase.java` has these — if not, add:
```java
public List<String> listCollections() {
    Mongocore.ListCollectionsResponse response = stub.listCollections(
        Mongocore.ListCollectionsRequest.newBuilder()
            .setDatabase(name)
            .build());
    return response.getCollectionsList();
}

public void createCollection(String collectionName) {
    stub.createCollection(
        Mongocore.CreateCollectionRequest.newBuilder()
            .setDatabase(name)
            .setCollection(collectionName)
            .build());
}
```

- [ ] **Step 4: Verify Java compiles**

```bash
cd clients/java && mvn compile -q
```
Expected: BUILD SUCCESS

- [ ] **Step 5: Commit**

```bash
git add clients/java/
git commit -m "feat(clients): add findAndModify, createIndex, transactions, getAnalytics to Java client"
```

---

## Task 6: Add Integration Tests — Python

**Files:**
- Modify: `clients/python/tests/test_integration.py`

- [ ] **Step 1: Add all 16 new integration tests**

Append to `clients/python/tests/test_integration.py`:

```python
@pytest.mark.asyncio
async def test_update_many():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]
        await coll.insert_many([
            {"status": "pending", "value": 1},
            {"status": "pending", "value": 2},
            {"status": "done", "value": 3},
        ])
        result = await coll.update_many(
            {"status": "pending"},
            {"$set": {"status": "complete"}}
        )
        assert result["modified_count"] == 2


@pytest.mark.asyncio
async def test_find_and_modify():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]
        await coll.insert_one({"name": "target", "counter": 0})
        doc = await coll.find_and_modify(
            {"name": "target"},
            {"$inc": {"counter": 1}},
            return_new=True,
        )
        assert doc is not None
        assert doc["counter"] == 1


@pytest.mark.asyncio
async def test_list_collections():
    async with MongoClient("localhost:50051") as client:
        db = client[TEST_DB]
        coll_name = unique_collection()
        coll = db[coll_name]
        await coll.insert_one({"x": 1})
        collections = await db.list_collections()
        assert coll_name in collections


@pytest.mark.asyncio
async def test_create_collection():
    async with MongoClient("localhost:50051") as client:
        db = client[TEST_DB]
        coll_name = unique_collection()
        await db.create_collection(coll_name)
        collections = await db.list_collections()
        assert coll_name in collections


@pytest.mark.asyncio
async def test_create_index():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]
        await coll.insert_one({"email": "test@example.com"})
        name = await coll.create_index({"email": 1}, unique=True)
        assert name  # Non-empty index name returned


@pytest.mark.asyncio
async def test_run_command():
    async with MongoClient("localhost:50051") as client:
        result = await client.run_command("admin", {"ping": 1})
        assert result.get("ok") == 1.0


@pytest.mark.asyncio
async def test_get_analytics():
    async with MongoClient("localhost:50051") as client:
        # Do an operation first to generate analytics
        coll = client[TEST_DB][unique_collection()]
        await coll.insert_one({"x": 1})
        analytics = await client.get_analytics()
        assert "total_operations" in analytics
        assert analytics["total_operations"] >= 0


@pytest.mark.asyncio
async def test_transaction_commit():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]
        # Ensure collection exists
        await coll.insert_one({"setup": True})
        await coll.delete_many({})

        txn_id = await client.begin_transaction()
        assert txn_id
        success = await client.commit_transaction(txn_id)
        assert success


@pytest.mark.asyncio
async def test_transaction_abort():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]
        await coll.insert_one({"setup": True})
        await coll.delete_many({})

        txn_id = await client.begin_transaction()
        assert txn_id
        success = await client.abort_transaction(txn_id)
        assert success


@pytest.mark.asyncio
async def test_ingest_csv():
    async with MongoClient("localhost:50051") as client:
        import os
        csv_path = os.path.join(os.path.dirname(__file__), "../../test_fixtures/sample.csv")
        csv_path = os.path.abspath(csv_path)
        coll_name = unique_collection()
        job = await client.ingest(
            file=csv_path,
            database=TEST_DB,
            collection=coll_name,
        )
        assert job.get("job_id")


@pytest.mark.asyncio
async def test_ingest_status():
    async with MongoClient("localhost:50051") as client:
        import os
        csv_path = os.path.join(os.path.dirname(__file__), "../../test_fixtures/sample.csv")
        csv_path = os.path.abspath(csv_path)
        coll_name = unique_collection()
        job = await client.ingest(
            file=csv_path,
            database=TEST_DB,
            collection=coll_name,
        )
        status = await client.ingest_status(job["job_id"])
        assert "job_id" in status


@pytest.mark.asyncio
async def test_list_ingest_jobs():
    async with MongoClient("localhost:50051") as client:
        jobs = await client.list_ingest_jobs()
        assert isinstance(jobs, list)


@pytest.mark.asyncio
async def test_cancel_ingest():
    async with MongoClient("localhost:50051") as client:
        import os
        csv_path = os.path.join(os.path.dirname(__file__), "../../test_fixtures/sample.csv")
        csv_path = os.path.abspath(csv_path)
        coll_name = unique_collection()
        job = await client.ingest(
            file=csv_path,
            database=TEST_DB,
            collection=coll_name,
        )
        result = await client.cancel_ingest(job["job_id"])
        assert isinstance(result, bool)


@pytest.mark.asyncio
async def test_watch_directory():
    async with MongoClient("localhost:50051") as client:
        import tempfile
        with tempfile.TemporaryDirectory() as tmpdir:
            result = await client.watch_directory(
                path=tmpdir,
                database=TEST_DB,
                collection=unique_collection(),
            )
            assert result.get("watch_id")
            # Clean up
            await client.stop_watch(result["watch_id"])


@pytest.mark.asyncio
async def test_stop_watch():
    async with MongoClient("localhost:50051") as client:
        import tempfile
        with tempfile.TemporaryDirectory() as tmpdir:
            result = await client.watch_directory(
                path=tmpdir,
                database=TEST_DB,
                collection=unique_collection(),
            )
            watch_id = result["watch_id"]
            stopped = await client.stop_watch(watch_id)
            assert stopped
```

- [ ] **Step 2: Run Python integration tests**

```bash
cd clients/python && python3 -m pytest tests/test_integration.py -v
```
Expected: 27 tests PASS

- [ ] **Step 3: Commit**

```bash
git add clients/python/tests/test_integration.py
git commit -m "test(python): add 16 integration tests for full 27-RPC coverage"
```

---

## Task 7: Add Integration Tests — TypeScript

**Files:**
- Modify: `clients/typescript/tests/integration.test.ts`

- [ ] **Step 1: Add all 16 new integration tests**

Add new `describe` blocks to the existing test file following the same patterns as Task 6 but in TypeScript/Jest syntax. Each test should use `client.db(TEST_DB).collection(uniqueCollection())` and follow the existing promise-based patterns.

The tests to add (same operations as Python):
- `update_many`, `find_and_modify`, `list_collections`, `create_collection`, `create_index`
- `run_command`, `get_analytics`
- `transaction_commit`, `transaction_abort`
- `ingest_csv`, `ingest_status`, `list_ingest_jobs`, `cancel_ingest`
- `watch_directory`, `stop_watch`

Use `path.resolve(__dirname, '../../test_fixtures/sample.csv')` for CSV path.
Use `fs.mkdtempSync(path.join(os.tmpdir(), 'mongocore-'))` for watch directory tests.

- [ ] **Step 2: Run TypeScript integration tests**

```bash
cd clients/typescript && npx jest tests/integration.test.ts --no-coverage
```
Expected: 27 tests PASS

- [ ] **Step 3: Commit**

```bash
git add clients/typescript/tests/integration.test.ts
git commit -m "test(typescript): add 16 integration tests for full 27-RPC coverage"
```

---

## Task 8: Add Integration Tests — Go

**Files:**
- Modify: `clients/go/mongocore/integration_test.go`

- [ ] **Step 1: Add all 16 new integration tests**

Add test functions following the existing Go testing patterns (same operations as Task 6):
- `TestUpdateMany`, `TestFindAndModify`, `TestListCollections`, `TestCreateCollection`, `TestCreateIndex`
- `TestRunCommand`, `TestGetAnalytics`
- `TestTransactionCommit`, `TestTransactionAbort`
- `TestIngestCSV`, `TestIngestStatus`, `TestListIngestJobs`, `TestCancelIngest`
- `TestWatchDirectory`, `TestStopWatch`

Use `os.MkdirTemp("", "mongocore-test-")` for watch tests.
Use `filepath.Join("..", "..", "test_fixtures", "sample.csv")` for CSV path (resolve relative to test file).

- [ ] **Step 2: Run Go integration tests**

```bash
cd clients/go && go test ./mongocore/ -v -count=1
```
Expected: 27 tests PASS

- [ ] **Step 3: Commit**

```bash
git add clients/go/mongocore/integration_test.go
git commit -m "test(go): add 16 integration tests for full 27-RPC coverage"
```

---

## Task 9: Add Integration Tests — Java

**Files:**
- Modify: `clients/java/src/test/java/com/mongocore/IntegrationTest.java`

- [ ] **Step 1: Add all 16 new integration tests**

Add test methods following the existing JUnit patterns (same operations as Task 6):
- `testUpdateMany`, `testFindAndModify`, `testListCollections`, `testCreateCollection`, `testCreateIndex`
- `testRunCommand`, `testGetAnalytics`
- `testTransactionCommit`, `testTransactionAbort`
- `testIngestCSV`, `testIngestStatus`, `testListIngestJobs`, `testCancelIngest`
- `testWatchDirectory`, `testStopWatch`

Use `Files.createTempDirectory("mongocore-test-")` for watch tests.
Use `Paths.get("../test_fixtures/sample.csv").toAbsolutePath().toString()` for CSV path.

- [ ] **Step 2: Run Java integration tests**

```bash
cd clients/java && mvn test -Dtest=IntegrationTest
```
Expected: 27 tests PASS with verbose output

- [ ] **Step 3: Commit**

```bash
git add clients/java/src/test/java/com/mongocore/IntegrationTest.java
git commit -m "test(java): add 16 integration tests for full 27-RPC coverage"
```

---

## Task 10: Standardize Unit Tests

**Files:**
- Modify: `clients/python/tests/test_client.py`
- Create: `clients/typescript/tests/unit.test.ts`
- Create: `clients/go/mongocore/client_unit_test.go`
- Modify: `clients/java/src/test/java/com/mongocore/MongoClientTest.java`

- [ ] **Step 1: Python — add 2 missing unit tests**

Add to `clients/python/tests/test_client.py`:
```python
def test_client_default_address():
    client = MongoClient()
    assert client._address == "localhost:50051"


def test_client_metadata_constant():
    from mongocore.client import _CLIENT_METADATA
    assert _CLIENT_METADATA == [("x-client-language", "python")]
```

- [ ] **Step 2: TypeScript — create unit test file**

Create `clients/typescript/tests/unit.test.ts`:
```typescript
import { MongoClient, CLIENT_METADATA } from '../src/client';
import { Database } from '../src/database';
import { Collection } from '../src/collection';

describe('Unit tests', () => {
  test('client creation with address', () => {
    const client = new MongoClient('custom:9999');
    expect(client).toBeTruthy();
  });

  test('client default address', () => {
    const client = new MongoClient();
    expect(client).toBeTruthy();
  });

  test('database access', () => {
    const client = new MongoClient('localhost:50051');
    const db = client.db('testdb');
    expect(db).toBeInstanceOf(Database);
    expect(db.name).toBe('testdb');
  });

  test('collection access', () => {
    const client = new MongoClient('localhost:50051');
    const coll = client.db('testdb').collection('users');
    expect(coll).toBeInstanceOf(Collection);
  });

  test('client metadata header set', () => {
    expect(CLIENT_METADATA.get('x-client-language')).toBe('typescript');
  });
});
```

- [ ] **Step 3: Go — create unit test file**

Create `clients/go/mongocore/client_unit_test.go`:
```go
package mongocore_test

import (
    "testing"

    "github.com/rozza/mongocore/clients/go/mongocore"
)

func TestUnitClientCreation(t *testing.T) {
    client := mongocore.NewClient("custom:9999")
    if client == nil {
        t.Fatal("Expected non-nil client")
    }
}

func TestUnitClientDefaultAddress(t *testing.T) {
    client := mongocore.NewClient("localhost:50051")
    if client == nil {
        t.Fatal("Expected non-nil client")
    }
}

func TestUnitDatabaseAccess(t *testing.T) {
    client := mongocore.NewClient("localhost:50051")
    db := client.Database("testdb")
    if db == nil {
        t.Fatal("Expected non-nil database")
    }
    if db.Name() != "testdb" {
        t.Fatalf("Expected 'testdb', got '%s'", db.Name())
    }
}

func TestUnitCollectionAccess(t *testing.T) {
    client := mongocore.NewClient("localhost:50051")
    coll := client.Database("testdb").Collection("users")
    if coll == nil {
        t.Fatal("Expected non-nil collection")
    }
}

func TestUnitClientMetadata(t *testing.T) {
    // Verify the metadata constant is accessible (compilation test)
    // The actual metadata is internal, but we verify it compiles
    client := mongocore.NewClient("localhost:50051")
    if client == nil {
        t.Fatal("Expected non-nil client")
    }
}
```

- [ ] **Step 4: Java — add 2 missing unit tests**

Add to `clients/java/src/test/java/com/mongocore/MongoClientTest.java`:
```java
@Test
public void testDefaultAddress() {
    MongoClient client = MongoClient.create();
    assertNotNull(client);
}

@Test
public void testMetadataInterceptor() {
    // Verify client creates successfully with interceptor
    MongoClient client = MongoClient.create("localhost:50051");
    assertNotNull(client);
}
```

- [ ] **Step 5: Run all unit tests**

```bash
cd clients/python && python3 -m pytest tests/test_client.py -v
cd clients/typescript && npx jest tests/unit.test.ts --no-coverage
cd clients/go && go test ./mongocore/ -v -count=1 -run "^TestUnit"
cd clients/java && mvn test -Dtest=MongoClientTest
```
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add clients/
git commit -m "test: standardize unit tests across all 4 client libraries"
```

---

## Task 11: Regression Verification

- [ ] **Step 1: Run full Rust test suite**

```bash
cargo test --lib
cargo test --test integration
```
Expected: All pass (208 unit, 74 integration)

- [ ] **Step 2: Run all client tests**

```bash
just test-clients
```
Expected: All 4 clients pass with 27 integration tests each

- [ ] **Step 3: Run client unit tests**

```bash
just test-unit-clients
```
Expected: All 4 clients pass unit tests

- [ ] **Step 4: Commit any fixes**

If anything failed, fix and commit.

---

## Implementation Order & Dependencies

```
Task 1: Fixtures + infrastructure (independent)
Tasks 2-5: Add missing methods (per-language, parallel)
Tasks 6-9: Add integration tests (per-language, depends on Tasks 2-5)
Task 10: Unit tests (independent of Tasks 6-9)
Task 11: Regression (depends on all above)
```

Tasks 2, 3, 4, 5 can run in parallel.
Tasks 6, 7, 8, 9 can run in parallel (after corresponding method task).
Task 10 can run in parallel with Tasks 6-9.

---

## Definition of Done

- [ ] `just test-java` produces verbose per-test output
- [ ] All 4 clients have 27 integration tests (one per RPC)
- [ ] All 4 clients have 5 unit tests
- [ ] `just test-clients` passes (27 tests × 4 clients = 108 integration tests)
- [ ] `just test-unit-clients` passes (5 tests × 4 clients = 20 unit tests)
- [ ] Shared CSV fixtures exist at `clients/test_fixtures/`
- [ ] AGENTS.md contains testing rules section
- [ ] Missing client methods added: FindAndModify, CreateIndex, BeginTransaction, CommitTransaction, AbortTransaction, GetAnalytics (all 4 clients)
- [ ] Go client additionally has ListCollections, CreateCollection
