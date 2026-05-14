# Transactional Pipeline Design

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.
> If modifying client libraries: verify imports work and run `just test-clients`.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

**Date:** 2026-05-14
**Status:** Draft
**Scope:** New `TransactionPipeline` RPC for sequential, dependent operations with result forwarding

---

## Summary

A new `TransactionPipeline` RPC that executes named steps sequentially within a MongoDB transaction. Steps can reference results from prior steps using `{{step_name.field.path[0]}}` syntax. On any step failure, the transaction is aborted and all changes roll back.

This is separate from the existing concurrent `Pipeline` RPC (v0.9) which executes independent operations in parallel without dependencies or transactions.

## Motivation

Many real-world workflows require dependent operations that must succeed or fail atomically:
- Deactivate a user, then log the action with the user's details
- Find expired subscriptions, archive them, then delete originals
- Reserve inventory for pending orders, then mark orders as processing

The concurrent Pipeline RPC cannot serve these because it has no inter-step dependencies and no transactional guarantees.

## Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Reference syntax | `{{step_name.path[0].field}}` | Aligns with existing template registry; distinct from MQL `$` operators; universally recognized as placeholder syntax |
| Error model | Fail pipeline immediately | Matches transaction semantics (all-or-nothing); simple and predictable |
| Execution model | Strictly sequential | Simple implementation; independent ops can use concurrent Pipeline instead |
| Operation scope | CRUD + Aggregation | Covers 95% of transactional use cases; admin/search ops can't participate in transactions |
| Step namespace | Per-step database + collection | Unified proto; client wrappers provide collection-scoped and database-scoped convenience |
| Response | All step results + summary | Full execution trace for debugging; summary for quick inspection |
| Transient errors | Auto-retry up to 3 attempts | Matches MongoDB driver best practice for TransientTransactionError |

## Reference Syntax

References use `{{}}` delimiters consistent with the existing template registry (`src/compiled/template_registry.rs`). The syntax supports:

### Basic field access
```
{{step_name.field}}           → top-level field from step's result
{{step_name.field.subfield}}  → nested dot-path traversal
```

### Array indexing
```
{{step_name[0]}}              → first document in a multi-result step
{{step_name[0].field}}        → field from first document
{{step_name[2].address.city}} → nested field from specific document
```

### Wildcard pluck
```
{{step_name[*].field}}        → array of `field` values from all documents
```
Returns a JSON array, useful for `$in` queries or `insert_many` inputs.

### Full result passthrough
```
{{step_name}}                 → entire result (document or array of documents)
```
Useful for passing a Find result directly into an InsertMany.

### Length accessor
```
{{step_name.length}}          → number of documents in result (for Find/Aggregate)
```

### Type preservation

- If the entire string value is a single `{{ref}}`, the resolved value replaces it with its native type (number, boolean, object, array)
- If `{{ref}}` is embedded in a larger string (e.g., `"User {{find_user.name}} deactivated"`), the referenced value is stringified and interpolated

## Result Shape Per Operation Type

Each operation type exposes a defined result shape for references:

| Operation | Referenceable result |
|-----------|---------------------|
| `FindOne` | Single document (or null → triggers failure if referenced) |
| `Find` | Array of documents |
| `Aggregate` | Array of documents |
| `Insert` | `{ inserted_id }` |
| `InsertMany` | `{ inserted_ids: [...] }` |
| `Update` / `UpdateMany` | `{ matched_count, modified_count, upserted_id }` |
| `Delete` / `DeleteMany` | `{ deleted_count }` |
| `FindAndModify` | Single document (the before/after document) |

## Proto Definition

```protobuf
message TransactionStep {
  string name = 1;
  string database = 2;
  string collection = 3;
  oneof operation {
    FindOneRequest find_one = 10;
    FindRequest find = 11;
    InsertRequest insert = 12;
    InsertManyRequest insert_many = 13;
    UpdateRequest update = 14;
    UpdateManyRequest update_many = 15;
    DeleteRequest delete = 16;
    DeleteManyRequest delete_many = 17;
    FindAndModifyRequest find_and_modify = 18;
    AggregateRequest aggregate = 19;
  }
}

message TransactionPipelineOptions {
  optional string read_concern = 1;    // default: "snapshot"
  optional string write_concern = 2;   // default: "majority"
  optional uint64 max_time_ms = 3;     // default: 30000 (30s)
}

message TransactionPipelineRequest {
  repeated TransactionStep steps = 1;
  optional TransactionPipelineOptions options = 2;
}

message TransactionStepResult {
  string name = 1;
  bool success = 2;
  oneof result {
    FindResponse find_result = 10;
    FindOneResponse find_one_result = 11;
    InsertResponse insert_result = 12;
    InsertManyResponse insert_many_result = 13;
    UpdateResponse update_result = 14;
    DeleteResponse delete_result = 15;
    AggregateResponse aggregate_result = 16;
    FindAndModifyResponse find_and_modify_result = 17;
  }
}

message TransactionPipelineSummary {
  uint32 total_steps = 1;
  uint32 steps_completed = 2;
  uint64 elapsed_ms = 3;
}

message TransactionPipelineResponse {
  repeated TransactionStepResult steps = 1;
  TransactionPipelineSummary summary = 2;
}

// In the MongoCore service:
rpc TransactionPipeline(TransactionPipelineRequest) returns (TransactionPipelineResponse);
```

**Note:** The `database`, `collection`, and `transaction_id` fields within individual operation requests (e.g., `FindOneRequest.database`) are **ignored** when used inside a `TransactionStep`. The step-level `database` and `collection` fields take precedence, and the transaction session is managed by the pipeline executor.

## Validation (Pre-Execution)

Before starting a transaction, validate:

1. **Non-empty** — at least one step
2. **Step count cap** — maximum 50 steps
3. **Unique step names** — no duplicates
4. **Valid name format** — alphanumeric + underscore only (valid identifiers)
5. **Valid references** — every `{{step_name...}}` references a step that exists
6. **No forward references** — a step can only reference steps defined before it
7. **Required fields** — each step has a name, database, collection, and operation
8. **Find/Aggregate limit** — if `limit` is explicitly set and > 101, reject
9. **No nesting** — `TransactionPipeline` cannot appear inside a concurrent `Pipeline` RPC, and transaction operations (`BeginTransaction`, `CommitTransaction`, `AbortTransaction`) cannot appear as steps

Validation errors return immediately with no transaction started and a descriptive error message identifying the offending step/reference.

## Execution Flow

```
1. Validate all steps (static analysis)
2. Begin MongoDB transaction (readConcern: snapshot, writeConcern: majority)
3. For each step in order:
   a. Resolve {{references}} from accumulated results map
   b. Execute operation within transaction session
   c. Store result in results map keyed by step name
   d. If step fails → abort transaction, return error with context
4. Commit transaction
5. Return all step results + summary (elapsed time, steps completed)

On TransientTransactionError: retry entire pipeline (up to 3 attempts)
```

## Result Size Cap

Find and Aggregate steps are capped at **101 documents**:

- **If `limit` is specified and > 101:** Validation rejects the pipeline before execution with a clear error.
- **If no `limit` is specified:** The executor automatically sets `limit: 101` on the query.

This prevents memory exhaustion from unbounded queries within a transaction pipeline. The cap may be revisited in the future based on real-world usage.

## Error Response

On failure, the response includes:

```json
{
  "error": {
    "failed_step": "update_audit",
    "step_index": 2,
    "reason": "referenced step 'find_user' returned no result",
    "steps_completed": ["find_user", "deactivate"],
    "rolled_back": true
  }
}
```

Error scenarios:
- **Step returns no result** (FindOne with no match) and a later step references it
- **Write failure** (duplicate key, schema validation) → abort with MongoDB error
- **Reference resolution failure** (path doesn't exist in result) → abort with invalid path detail
- **Result cap exceeded** → abort before storing
- **Timeout** → abort after max_time_ms elapsed
- **Transient error after 3 retries** → abort with final error

"No result" semantics: Only fail if the referenced **path** doesn't resolve. `modified_count` being `0` is a valid value. But accessing `.upserted_id` when no upsert occurred would fail.

## Transaction Options

| Option | Default | Notes |
|--------|---------|-------|
| `read_concern` | `"snapshot"` | Required for multi-document transaction consistency |
| `write_concern` | `"majority"` | Ensures durability before acknowledging |
| `max_time_ms` | `30000` (30s) | Pipeline-level timeout; prevents long-held locks |

## Client Wrapper APIs

### Python

```python
# Collection-scoped (all steps same collection)
result = await collection.transaction_pipeline([
    Step("find_user", find_one({"email": "alice@example.com"})),
    Step("deactivate", update_one(
        {"_id": "{{find_user._id}}"},
        {"$set": {"active": False}}
    )),
])

# Database-scoped (cross-collection)
result = await db.transaction_pipeline([
    Step("find_user", "users", find_one({"email": "alice@example.com"})),
    Step("audit", "audit_logs", insert_one({
        "user_id": "{{find_user._id}}",
        "action": "deactivated",
    })),
])

# Access results
result.summary.total_steps        # 2
result.summary.elapsed_ms         # 15
result["find_user"].document      # {...}
result["deactivate"].modified_count  # 1
```

### TypeScript

```typescript
// Collection-scoped
const result = await collection.transactionPipeline([
  step("findUser", findOne({ email: "alice@example.com" })),
  step("deactivate", updateOne(
    { _id: "{{findUser._id}}" },
    { $set: { active: false } }
  )),
]);

// Database-scoped
const result = await db.transactionPipeline([
  step("findUser", "users", findOne({ email: "alice@example.com" })),
  step("audit", "audit_logs", insertOne({
    userId: "{{findUser._id}}",
    action: "deactivated",
  })),
]);
```

### Go

```go
// Database-scoped
result, err := db.TransactionPipeline(ctx, []Step{
    NewStep("findUser", "users", FindOne(bson.M{"email": "alice@example.com"})),
    NewStep("audit", "audit_logs", InsertOne(bson.M{
        "user_id": "{{findUser._id}}",
        "action":  "deactivated",
    })),
})
```

### Java

```java
// Database-scoped
var result = db.transactionPipeline(List.of(
    Step.of("findUser", "users", findOne(eq("email", "alice@example.com"))),
    Step.of("audit", "audit_logs", insertOne(new Document()
        .append("user_id", "{{findUser._id}}")
        .append("action", "deactivated")))
));
```

## MCP Tool

**Tool name:** `transaction_pipeline`
**Safety:** Write operation (requires confirmation in safe mode)

```json
{
  "name": "transaction_pipeline",
  "description": "Execute multiple dependent operations atomically in a transaction",
  "inputSchema": {
    "type": "object",
    "properties": {
      "steps": {
        "type": "array",
        "items": {
          "type": "object",
          "properties": {
            "name": { "type": "string" },
            "database": { "type": "string" },
            "collection": { "type": "string" },
            "operation": { "type": "string", "enum": ["find_one", "find", "insert", "insert_many", "update", "update_many", "delete", "delete_many", "find_and_modify", "aggregate"] },
            "params": { "type": "object" }
          },
          "required": ["name", "database", "collection", "operation", "params"]
        }
      },
      "options": {
        "type": "object",
        "properties": {
          "read_concern": { "type": "string" },
          "write_concern": { "type": "string" },
          "max_time_ms": { "type": "integer" }
        }
      }
    },
    "required": ["steps"]
  }
}
```

## Observability

When the `otel` feature is enabled, the executor emits OpenTelemetry spans:

- **Parent span:** `transaction_pipeline` — covers the entire pipeline execution (including retries)
- **Child spans:** One per step, named `transaction_pipeline.step.{name}` — includes operation type, collection, and duration
- **Attributes:** `pipeline.steps_total`, `pipeline.steps_completed`, `pipeline.retry_count`

This allows tracing slow pipelines and identifying which step is the bottleneck.

## Constraints

- **No nesting:** `TransactionPipeline` cannot be used as an operation inside a concurrent `Pipeline` RPC.
- **Replica set required:** MongoDB transactions require a replica set (or sharded cluster 4.2+). Standalone deployments will receive a server error at transaction begin — no special topology check is needed.
- **Read-after-write within session:** Steps can read documents inserted/modified by earlier steps in the same pipeline. MongoDB's snapshot isolation guarantees visibility within the transaction session. This enables patterns like insert → find_one → update based on computed values.

## Implementation Components

| Component | Location | Responsibility |
|-----------|----------|---------------|
| Proto messages | `proto/mongocore/v1/mongocore.proto` | API contract |
| Reference parser | `src/operations/pipeline_refs.rs` (new) | Parse `{{}}` syntax, resolve paths |
| Validator | `src/operations/transaction_pipeline.rs` (new) | Static analysis of steps |
| Executor | `src/operations/transaction_pipeline.rs` (new) | Sequential execution + retry |
| gRPC handler | `src/grpc/service.rs` | Wire up RPC |
| MCP tool | `src/mcp/tools.rs` + `src/mcp/handler.rs` | Tool definition + handler |
| MCP safety | `src/mcp/safety.rs` | Write operation rules |
| Client wrappers | `clients/{python,typescript,go,java}/` | Ergonomic APIs |
| Integration tests | `tests/integration/transaction_pipeline_test.rs` (new) | End-to-end tests |
| Documentation | `docs/transactional-pipelines.md` (new) | Comprehensive examples and use cases |

## Examples

### Deactivate user with audit trail

```python
result = await db.transaction_pipeline([
    Step("find_user", "users", find_one({"email": "alice@example.com"})),
    Step("deactivate", "users", update_one(
        {"_id": "{{find_user._id}}"},
        {"$set": {"active": False, "deactivated_at": datetime.utcnow()}}
    )),
    Step("audit", "audit_logs", insert_one({
        "action": "user_deactivated",
        "user_id": "{{find_user._id}}",
        "username": "{{find_user.username}}",
        "address_city": "{{find_user.address.city}}",
    })),
])
```

### Archive expired subscriptions

```python
result = await db.transaction_pipeline([
    Step("find_expired", "subscriptions", find(
        {"expires_at": {"$lt": "2024-01-01"}, "status": "active"}
    )),
    Step("archive", "subscriptions_archive", insert_many("{{find_expired}}")),
    Step("cleanup", "subscriptions", delete_many(
        {"_id": {"$in": "{{find_expired[*]._id}}"}}
    )),
])
```

### Reserve inventory for pending orders

```python
result = await db.transaction_pipeline([
    Step("pending_orders", "orders", find({"status": "pending", "priority": "high"})),
    Step("reserve_stock", "inventory", update_many(
        {"sku": {"$in": "{{pending_orders[*].sku}}"}},
        {"$inc": {"reserved": 1}}
    )),
    Step("mark_processing", "orders", update_many(
        {"_id": {"$in": "{{pending_orders[*]._id}}"}},
        {"$set": {"status": "processing"}}
    )),
    Step("log", "activity", insert_one({
        "action": "batch_reserve",
        "order_count": "{{pending_orders.length}}",
    })),
])
```
