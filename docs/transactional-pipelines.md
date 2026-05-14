# Transactional Pipelines

Execute a sequence of dependent database operations atomically within a single transaction, with automatic result forwarding between steps.

## When to Use

- **Dependent operations** — the output of one operation feeds the next (e.g., find a user, then update their orders)
- **Atomic multi-step workflows** — all steps succeed or all roll back (e.g., transfer funds + write audit log)
- **Cross-collection transactions** — operations spanning multiple collections that must be consistent
- **Read-after-write patterns** — insert a document, then immediately query for related data

If your operations are independent and can run concurrently, use [Request Pipelining](./request-pipelining.md) instead. If you need a simple transaction without result forwarding, use [Transactions](./transactions.md).

---

## Quick Start

### Python

```python
from mongocore import MongoClient
from mongocore.ops import TransactionStep, step_find_one, step_update

async with MongoClient() as client:
    db = client["myapp"]
    result = await db.transaction_pipeline([
        TransactionStep("find_user", step_find_one({"email": "alice@example.com"}), collection="users"),
        TransactionStep("deactivate", step_update(
            {"_id": "{{find_user._id}}"},
            {"$set": {"active": False}}
        ), collection="users"),
    ])
    print(f"Completed {result.summary.steps_completed} steps in {result.summary.elapsed_ms}ms")
```

### TypeScript

```typescript
import { MongoClient } from '@mongocore/client';
import { step, stepFindOne, stepUpdate } from '@mongocore/client/ops';

const client = new MongoClient();
await client.connect();
const db = client.db('myapp');

const result = await db.transactionPipeline([
  step('find_user', 'users', stepFindOne({ email: 'alice@example.com' })),
  step('deactivate', 'users', stepUpdate(
    { _id: '{{find_user._id}}' },
    { $set: { active: false } }
  )),
]);
console.log(`Completed ${result.summary.stepsCompleted} steps`);
```

### Go

```go
findStep, _ := mongocore.NewFindOneStep("find_user", "myapp", "users",
    bson.D{{Key: "email", Value: "alice@example.com"}})

updateStep, _ := mongocore.NewUpdateStep("deactivate", "myapp", "users",
    bson.D{{Key: "_id", Value: "{{find_user._id}}"}},
    bson.D{{Key: "$set", Value: bson.D{{Key: "active", Value: false}}}})

result, err := client.TransactionPipeline(ctx,
    []*mongocore.TransactionStep{findStep, updateStep}, nil)
if err != nil {
    log.Fatal(err)
}
fmt.Printf("Completed %d steps in %dms\n", result.StepsCompleted, result.ElapsedMs)
```

### Java

```java
import com.mongocore.TransactionPipelineStep;

var findUser = TransactionPipelineStep.findOne("find_user", "myapp", "users",
    new Document("email", "alice@example.com"));

var deactivate = TransactionPipelineStep.update("deactivate", "myapp", "users",
    new Document("_id", "{{find_user._id}}"),
    new Document("$set", new Document("active", false)));

var result = client.transactionPipeline(findUser, deactivate);
System.out.printf("Completed %d steps in %dms%n",
    result.stepsCompleted(), result.elapsedMs());
```

---

## Reference Syntax

Steps can reference results from earlier steps using `{{step_name.path}}` syntax. References are resolved at execution time, before each step runs.

| Pattern | Description | Example | Resolves To |
|---------|-------------|---------|-------------|
| `{{step.field}}` | Top-level field | `{{find_user._id}}` | `"abc123"` |
| `{{step.field.sub}}` | Nested field | `{{find_user.address.city}}` | `"Portland"` |
| `{{step[0].field}}` | Array index | `{{find_users[0].email}}` | `"alice@example.com"` |
| `{{step[*].field}}` | Wildcard pluck | `{{find_users[*]._id}}` | `["id1", "id2", "id3"]` |
| `{{step}}` | Passthrough (entire result) | `{{find_expired}}` | `[{...}, {...}]` |
| `{{step.length}}` | Array length | `{{find_users.length}}` | `3` |

### Rules

- References can only point **backward** — a step cannot reference a later step
- Step names must be unique, start with a letter or underscore, and contain only alphanumeric characters and underscores
- References to unknown steps or missing fields produce an error and abort the transaction

---

## Type Preservation

When a reference is the **entire value** of a JSON field, the resolved type is preserved:

```json
{"user_id": "{{find_user._id}}"}
```

If `find_user._id` is an ObjectId string `"507f1f77bcf86cd799439011"`, the result is a string. If it resolves to a number (like `modified_count`), it stays a number.

When a reference appears **inline** with other text, it is interpolated as a string:

```json
{"message": "User {{find_user.name}} was deactivated"}
```

This always produces a string value, regardless of the original type.

---

## Collection-Scoped API

When all steps target the same collection, use the collection-scoped API to avoid repeating the collection name:

### Python

```python
users = client["myapp"]["users"]
result = await users.transaction_pipeline([
    TransactionStep("find_user", step_find_one({"email": "alice@example.com"})),
    TransactionStep("deactivate", step_update(
        {"_id": "{{find_user._id}}"},
        {"$set": {"active": False}}
    )),
])
```

### TypeScript

```typescript
const users = client.db('myapp').collection('users');
const result = await users.transactionPipeline([
  step('find_user', stepFindOne({ email: 'alice@example.com' })),
  step('deactivate', stepUpdate({ _id: '{{find_user._id}}' }, { $set: { active: false } })),
]);
```

In the collection-scoped form, omit the `collection` parameter from each step — it is automatically set to the collection you called the method on.

---

## Result Shapes

Each operation type produces a specific result shape for referencing:

| Operation | Result Shape | Referencing Examples |
|-----------|-------------|---------------------|
| `find_one` | `{"_id": ..., "field": ...}` or `null` | `{{step._id}}`, `{{step.name}}` |
| `find` | `[{"_id": ..., ...}, ...]` | `{{step[0]._id}}`, `{{step[*].email}}`, `{{step.length}}` |
| `insert` | `{"inserted_id": ...}` | `{{step.inserted_id}}` |
| `insert_many` | `{"inserted_ids": [...], "inserted_count": N}` | `{{step.inserted_ids}}`, `{{step.inserted_count}}` |
| `update` | `{"matched_count": N, "modified_count": N, "upserted_id": ...}` | `{{step.modified_count}}` |
| `update_many` | `{"matched_count": N, "modified_count": N, "upserted_id": ...}` | `{{step.matched_count}}` |
| `delete` | `{"deleted_count": N}` | `{{step.deleted_count}}` |
| `delete_many` | `{"deleted_count": N}` | `{{step.deleted_count}}` |
| `find_and_modify` | `{"_id": ..., "field": ...}` or `null` | `{{step._id}}`, `{{step.status}}` |
| `aggregate` | `[{...}, ...]` | `{{step[0].total}}`, `{{step[*].name}}` |

---

## Error Handling

If any step fails, the entire transaction is aborted and all previous steps are rolled back. The error response indicates which step failed and why.

### Error Response Structure

```json
{
  "failed_step": "update_balance",
  "step_index": 2,
  "reason": "Cannot access field 'balance' on non-object value in reference 'find_account.balance'",
  "steps_completed": ["find_user", "find_account"],
  "rolled_back": true
}
```

| Field | Description |
|-------|-------------|
| `failed_step` | Name of the step that caused the failure |
| `step_index` | Zero-based index of the failed step |
| `reason` | Human-readable error message |
| `steps_completed` | Names of steps that executed before the failure |
| `rolled_back` | Whether the transaction was aborted (always `true` on failure) |

### Common Failure Causes

- **Reference resolution** — referenced field does not exist in the prior step's result
- **MongoDB errors** — duplicate key, validation failure, write concern timeout
- **Pipeline timeout** — execution exceeded `max_time_ms`
- **Transient errors** — retried automatically up to 3 times before failing

---

## Transaction Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `read_concern` | string | `"majority"` | Read concern level for the transaction |
| `write_concern` | string | `"majority"` | Write concern for the transaction |
| `max_time_ms` | integer | `30000` | Maximum execution time in milliseconds |

### Python

```python
result = await db.transaction_pipeline(steps, options={
    "read_concern": "snapshot",
    "write_concern": "majority",
    "max_time_ms": 10000,
})
```

### Go

```go
result, err := client.TransactionPipeline(ctx, steps, &mongocore.TransactionPipelineOptions{
    ReadConcern:  "snapshot",
    WriteConcern: "majority",
    MaxTimeMs:    10000,
})
```

---

## Limits

| Limit | Value | Description |
|-------|-------|-------------|
| Max steps | 50 | Maximum operations in a single pipeline |
| Max documents per step | 101 | Maximum documents returned by `find` or `aggregate` for referencing |
| Timeout | 30s | Default max execution time (configurable via `max_time_ms`) |
| Retries | 3 | Automatic retries on transient transaction errors |

---

## Use Cases

### User Deactivation with Audit Trail

Find a user, deactivate them, and write an audit log — all atomically.

```python
from mongocore.ops import TransactionStep, step_find_one, step_update, step_insert
from datetime import datetime

result = await db.transaction_pipeline([
    TransactionStep("find_user", step_find_one({"email": "alice@example.com"}), collection="users"),
    TransactionStep("deactivate", step_update(
        {"_id": "{{find_user._id}}"},
        {"$set": {"active": False, "deactivated_at": datetime.utcnow().isoformat()}}
    ), collection="users"),
    TransactionStep("audit", step_insert({
        "action": "user_deactivated",
        "user_id": "{{find_user._id}}",
        "user_email": "{{find_user.email}}",
        "timestamp": datetime.utcnow().isoformat(),
    }), collection="audit_log"),
])
```

### Archive Expired Records

Find expired documents, insert them into an archive collection, then delete the originals.

```python
from mongocore.ops import TransactionStep, step_find, step_insert_many, step_delete_many

result = await db.transaction_pipeline([
    TransactionStep("find_expired", step_find(
        {"expires_at": {"$lt": "2024-01-01T00:00:00Z"}, "status": "active"},
        limit=100
    ), collection="sessions"),
    TransactionStep("archive", step_insert_many("{{find_expired}}"), collection="sessions_archive"),
    TransactionStep("cleanup", step_delete_many(
        {"_id": {"$in": "{{find_expired[*]._id}}"}}
    ), collection="sessions"),
])
print(f"Archived {result.steps[1].result_json['inserted_count']} records")
```

### Inventory Reservation

Check stock, decrement inventory, and create a reservation — atomically preventing overselling.

```python
from mongocore.ops import TransactionStep, step_find_and_modify, step_insert

result = await db.transaction_pipeline([
    TransactionStep("reserve", step_find_and_modify(
        {"sku": "WIDGET-42", "quantity": {"$gte": 5}},
        {"$inc": {"quantity": -5}}
    ), collection="inventory"),
    TransactionStep("create_reservation", step_insert({
        "sku": "WIDGET-42",
        "quantity": 5,
        "previous_stock": "{{reserve.quantity}}",
        "order_id": "order_12345",
    }), collection="reservations"),
])
```

### Read-After-Write

Insert a document, then immediately query for related data using the new ID.

```python
from mongocore.ops import TransactionStep, step_insert, step_find

result = await db.transaction_pipeline([
    TransactionStep("create_order", step_insert({
        "customer_id": "cust_789",
        "items": [{"sku": "A1", "qty": 2}],
        "status": "pending",
    }), collection="orders"),
    TransactionStep("customer_orders", step_find(
        {"customer_id": "cust_789", "status": "pending"}
    ), collection="orders"),
])
pending_count = len(result.steps[1].result_json)
```

### Transfer Between Collections

Move a document from one collection to another (e.g., promoting a draft to published).

```python
from mongocore.ops import TransactionStep, step_find_one, step_insert, step_delete

result = await db.transaction_pipeline([
    TransactionStep("get_draft", step_find_one({"_id": "article_123"}), collection="drafts"),
    TransactionStep("publish", step_insert("{{get_draft}}"), collection="published"),
    TransactionStep("remove_draft", step_delete({"_id": "{{get_draft._id}}"}), collection="drafts"),
])
```

---

## MCP Tool

AI agents can use the `transaction_pipeline` tool to execute atomic multi-step workflows:

```json
{
  "name": "transaction_pipeline",
  "arguments": {
    "steps": [
      {
        "name": "find_user",
        "database": "myapp",
        "collection": "users",
        "operation": "find_one",
        "params": {"filter": {"email": "alice@example.com"}}
      },
      {
        "name": "deactivate",
        "database": "myapp",
        "collection": "users",
        "operation": "update",
        "params": {
          "filter": {"_id": "{{find_user._id}}"},
          "update": {"$set": {"active": false}}
        }
      },
      {
        "name": "audit",
        "database": "myapp",
        "collection": "audit_log",
        "operation": "insert",
        "params": {
          "document": {
            "action": "user_deactivated",
            "user_id": "{{find_user._id}}",
            "timestamp": "2024-01-15T10:30:00Z"
          }
        }
      }
    ],
    "options": {
      "max_time_ms": 10000
    }
  }
}
```

The tool supports all 10 operation types: `find_one`, `find`, `insert`, `insert_many`, `update`, `update_many`, `delete`, `delete_many`, `find_and_modify`, `aggregate`.

In read-only mode, the tool is blocked entirely (since pipelines typically contain write operations).

---

## Requirements

- **MongoDB replica set or sharded cluster** — transactions require a replica set (standalone MongoDB does not support multi-document transactions)
- **MongoCore sidecar** running and connected to MongoDB
- References use `{{...}}` syntax in filter/update/document fields — the sidecar resolves them server-side before executing each step

---

## Supported Operations

| Operation | Builder (Python) | Description |
|-----------|-----------------|-------------|
| `find_one` | `step_find_one(filter)` | Find a single document |
| `find` | `step_find(filter, limit=N)` | Find multiple documents (max 101) |
| `insert` | `step_insert(document)` | Insert one document |
| `insert_many` | `step_insert_many(documents)` | Insert multiple documents |
| `update` | `step_update(filter, update)` | Update first matching document |
| `update_many` | `step_update_many(filter, update)` | Update all matching documents |
| `delete` | `step_delete(filter)` | Delete first matching document |
| `delete_many` | `step_delete_many(filter)` | Delete all matching documents |
| `find_and_modify` | `step_find_and_modify(filter, update)` | Atomically find and update, returns modified doc |
| `aggregate` | `step_aggregate(pipeline)` | Run an aggregation pipeline |
