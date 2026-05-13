# Performance Tier 2: Request Pipelining

Batch N independent operations in a single gRPC round-trip for reduced latency and increased throughput.

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.
> If modifying client libraries: verify imports work and run `just test-clients`.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

## Motivation

Today every gRPC RPC is independent: N operations require N round-trips between client and sidecar. For AI agents gathering context from multiple collections, or applications issuing mixed reads and writes, this adds unnecessary latency — especially over TCP where each round-trip costs ~1ms.

**Target use cases:**
- AI agents (MCP tools) that fan out 3-10 operations in a single reasoning step
- Application code batching heterogeneous operations for throughput
- Dashboard/UI loads issuing many parallel queries

**Expected gains:**
| Scenario | Today | With Pipeline |
|----------|-------|---------------|
| AI agent reads 5 collections | 5 round-trips (~5ms TCP) | 1 round-trip (~1ms) |
| Dashboard loading 8 queries | 8 RPCs | 1 RPC |
| Mixed read+write batch (10 ops) | 10 RPCs | 1 RPC |

## Design Decisions

### Always concurrent, no ordered flag

All operations in a pipeline execute concurrently via `tokio::join_all`. There is no `ordered` flag.

**Rationale:** Sequential execution without dependency support is a half-measure — it saves round-trips but doesn't enable "use result of step 0 in step 1." When we add the `TransactionPipeline` RPC (future), that will provide sequential execution with result forwarding. Clean separation:

| RPC | Execution | Dependencies |
|-----|-----------|--------------|
| `Pipeline` (v0.9) | Concurrent | None |
| `TransactionPipeline` (future) | Sequential | Yes, with result forwarding |

### All non-streaming operations supported

The pipeline accepts any operation that has unary (request/response) semantics. Streaming RPCs (Watch, FindStream, AggregateStream, InsertManyStream, InsertManyBidi) are excluded since they require ongoing connections.

### All-or-nothing MCP safety enforcement

When invoked via the MCP `pipeline` tool, **all operations are validated before any execute**. If any operation violates safety rules (writes in read-only mode, blocked commands), the entire pipeline is rejected with an error listing which operations failed validation and why.

This prevents:
- Partial state changes that confuse AI agents
- Silent failures where an agent assumes all ops succeeded
- Inconsistency with existing per-tool safety behavior

## Proto Definition

```protobuf
// New RPC on MongoCore service
rpc Pipeline(PipelineRequest) returns (PipelineResponse);

message PipelineRequest {
  repeated PipelineOperation operations = 1;
}

message PipelineOperation {
  oneof operation {
    FindRequest find = 1;
    FindOneRequest find_one = 2;
    InsertRequest insert = 3;
    InsertManyRequest insert_many = 4;
    UpdateRequest update = 5;
    UpdateManyRequest update_many = 6;
    DeleteRequest delete = 7;
    DeleteManyRequest delete_many = 8;
    AggregateRequest aggregate = 9;
    FindAndModifyRequest find_and_modify = 10;
    RunCommandRequest run_command = 11;
    SearchRequest search = 12;
    CreateCollectionRequest create_collection = 13;
    CreateIndexRequest create_index = 14;
    ListDatabasesRequest list_databases = 15;
    ListCollectionsRequest list_collections = 16;
    BeginTransactionRequest begin_transaction = 17;
    CommitTransactionRequest commit_transaction = 18;
    AbortTransactionRequest abort_transaction = 19;
    GetAnalyticsRequest get_analytics = 20;
  }
}

message PipelineResponse {
  repeated PipelineResult results = 1;
  uint32 succeeded = 2;
  uint32 failed = 3;
}

message PipelineResult {
  uint32 index = 1;
  oneof result {
    FindResponse find = 2;
    FindOneResponse find_one = 3;
    InsertResponse insert = 4;
    InsertManyResponse insert_many = 5;
    UpdateResponse update = 6;
    UpdateManyResponse update_many = 7;
    DeleteResponse delete = 8;
    DeleteManyResponse delete_many = 9;
    AggregateResponse aggregate = 10;
    FindAndModifyResponse find_and_modify = 11;
    RunCommandResponse run_command = 12;
    SearchResponse search = 13;
    CreateCollectionResponse create_collection = 14;
    CreateIndexResponse create_index = 15;
    ListDatabasesResponse list_databases = 16;
    ListCollectionsResponse list_collections = 17;
    BeginTransactionResponse begin_transaction = 18;
    CommitTransactionResponse commit_transaction = 19;
    AbortTransactionResponse abort_transaction = 20;
    GetAnalyticsResponse get_analytics = 21;
    PipelineError error = 22;
  }
}

message PipelineError {
  int32 code = 1;
  string message = 2;
}
```

## Execution Model

### Server-side (Rust)

```rust
async fn pipeline(&self, request: Request<PipelineRequest>) -> Result<Response<PipelineResponse>, Status> {
    let tenant_ctx = self.extract_tenant_context(&request)?;
    let req = request.into_inner();

    // Validate limits
    if req.operations.len() > MAX_PIPELINE_OPS {
        return Err(Status::invalid_argument("too many operations"));
    }

    // Fan out all operations concurrently
    let futures: Vec<_> = req.operations.iter().enumerate().map(|(i, op)| {
        self.execute_pipeline_op(i as u32, op, &tenant_ctx)
    }).collect();

    let results = futures::future::join_all(futures).await;

    let succeeded = results.iter().filter(|r| !r.is_error()).count() as u32;
    let failed = results.iter().filter(|r| r.is_error()).count() as u32;

    Ok(Response::new(PipelineResponse { results, succeeded, failed }))
}
```

Each `execute_pipeline_op` dispatches through the same `Operations` module that individual RPCs use — no logic duplication.

### Error semantics

- Individual op failures → captured in `PipelineResult.error` (the RPC succeeds)
- Request-level failures (auth, quota, malformed) → gRPC Status error (RPC fails)
- Client checks `response.failed > 0` to detect partial failures

### Analytics & observability

- Each sub-operation is recorded individually in the analytics collector
- A parent span wraps the pipeline; child spans per operation (when OTel enabled)
- Tenant quota is counted per-operation (a 10-op pipeline counts as 10 toward rate limit)

## Client API (Typed Builder)

### Python

```python
from mongocore import ops

results = await client.pipeline(
    ops.find_one("mydb", "users", {"name": "bob"}),
    ops.find("mydb", "orders", {"user_id": "123"}),
    ops.aggregate("mydb", "metrics", [{"$group": {"_id": "$type", "count": {"$sum": 1}}}]),
    ops.insert("mydb", "audit", {"action": "pipeline_test"}),
)

user = results[0].document          # Optional[dict]
orders = results[1].documents       # list[dict]
metrics = results[2].documents      # list[dict]
insert = results[3].inserted_id     # str

if results[1].error:
    print(f"Orders query failed: {results[1].error.message}")
```

### Go

```go
results, err := client.Pipeline(ctx,
    ops.FindOne("mydb", "users", bson.M{"name": "bob"}),
    ops.Find("mydb", "orders", bson.M{"user_id": "123"}),
    ops.Insert("mydb", "audit", bson.M{"action": "test"}),
)

user, _ := results[0].AsFindOne()
orders, _ := results[1].AsFind()
insert, _ := results[2].AsInsert()
```

### TypeScript

```typescript
import { ops } from "mongocore";

const results = await client.pipeline(
    ops.findOne("mydb", "users", { name: "bob" }),
    ops.find("mydb", "orders", { userId: "123" }),
    ops.insert("mydb", "audit", { action: "test" }),
);

const user = results[0].asFindOne();    // Document | null
const orders = results[1].asFind();     // Document[]
const insert = results[2].asInsert();   // { insertedId: string }
```

### Java

```java
List<PipelineResult> results = client.pipeline(
    Ops.findOne("mydb", "users", Map.of("name", "bob")),
    Ops.find("mydb", "orders", Map.of("userId", "123")),
    Ops.insert("mydb", "audit", Map.of("action", "test"))
);

Document user = results.get(0).asFindOne();
List<Document> orders = results.get(1).asFind();
InsertResult insert = results.get(2).asInsert();
```

### MCP Tool

```json
{
  "name": "pipeline",
  "description": "Execute multiple independent operations concurrently in a single round-trip. All operations are validated before execution — if any operation violates safety rules, the entire pipeline is rejected.",
  "inputSchema": {
    "type": "object",
    "properties": {
      "operations": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "op": { "type": "string", "enum": ["find", "find_one", "insert", "insert_many", "update", "update_many", "delete", "delete_many", "aggregate", "find_and_modify", "run_command", "search", "create_collection", "create_index", "list_databases", "list_collections", "begin_transaction", "commit_transaction", "abort_transaction", "get_analytics"] },
            "database": { "type": "string" },
            "collection": { "type": "string" },
            "filter": { "type": "object" },
            "document": { "type": "object" },
            "documents": { "type": "array" },
            "pipeline": { "type": "array" },
            "update": { "type": "object" },
            "command": { "type": "object" }
          },
          "required": ["op"]
        },
        "maxItems": 100
      }
    },
    "required": ["operations"]
  }
}
```

## Limits & Safety

| Constraint | Value | Rationale |
|-----------|-------|-----------|
| Max operations per pipeline | 100 | Prevents resource exhaustion from unbounded batches |
| Max total response size | 64MB | Existing gRPC message size limit (configurable) |
| Quota accounting | Per-operation | Each op counts toward tenant rate limit individually |
| Blocked commands | Per-op validation | RunCommand ops validated against blocklist |
| MCP safety mode | All-or-nothing | All ops validated before execution; reject entire pipeline if any violate |

## Testing Strategy

- **Unit tests:** Pipeline dispatch logic, error collection, limit enforcement
- **Integration tests:** Mixed operation pipelines, partial failure scenarios, quota interaction
- **Client tests:** All 4 languages exercise pipeline with typed results
- **MCP tests:** Safety enforcement (read-only mode rejects writes in pipeline), tool invocation
- **Benchmark:** Pipeline of N ops vs N individual RPCs — measure latency reduction

## Future: TransactionPipeline (out of scope for v0.9)

See `docs/design/brainstorm/transactional-pipeline.md` for the dependent-operations variant that will support result forwarding between steps.
