# Brainstorm: Transactional Pipeline (Future)

Context from Performance Tier 2 design discussions (2026-05-13).

## Concept

A separate `TransactionPipeline` RPC that supports **dependent operations** — where step N can reference the result of step 0..N-1. Operations execute sequentially, optionally wrapped in a MongoDB transaction.

## Motivating Example

```python
results = await client.transaction_pipeline(
    ops.find_one("mydb", "users", {"name": "bob"}),           # step 0
    ops.update("mydb", "users",
        filter={"_id": "$[0]._id"},                           # references step 0
        update={"$set": {"active": False}}
    ),                                                         # step 1
    ops.insert("mydb", "audit", {
        "user_id": "$[0]._id",                                # references step 0
        "action": "deactivated",
        "previous_status": "$[0].status"                      # references step 0
    }),                                                         # step 2
)
```

## Key Design Questions

### 1. Reference Syntax

Options discussed:
- `$[0].field` — positional reference to step index + JSON path
- `{{step0.field}}` — template syntax (aligns with existing template system)
- Named steps: `{"name": "get_user", ...}` then `$[get_user].field`

### 2. Execution Model

- Always sequential (dependencies require ordering)
- Auto-wrapped in MongoDB transaction (consistent state)
- Rollback on failure (all-or-nothing)

### 3. Error Semantics

- If step 0 returns no document, what happens to step 1 that references `$[0]._id`?
  - Option A: Null propagation (field becomes null)
  - Option B: Fail the pipeline at that step
  - Option C: Skip dependent steps, execute independent ones

### 4. Validation

- Static analysis before execution: detect cycles, out-of-range references
- Type checking where possible (referencing `.documents[0]` from a find vs `.inserted_id` from an insert)
- Max depth/chain length to prevent pathological cases

## Why Separate from Pipeline

The independent `Pipeline` RPC (v0.9) is always-concurrent and has simple error semantics. Adding dependency support to it would:
- Force sequential execution when dependencies exist (breaking the performance benefit)
- Complicate error handling (partial execution with rollback?)
- Make the proto much more complex (reference expressions in every field)

Keeping them as separate RPCs:
| RPC | Execution | Dependencies | Transaction |
|-----|-----------|--------------|-------------|
| `Pipeline` | Concurrent | None | No |
| `TransactionPipeline` | Sequential | Yes | Yes (auto) |

## Relationship to Existing Template System

The v0.6 template registry already supports parameterized queries with variable substitution. TransactionPipeline could reuse the same parameter resolution engine but with step results as the parameter source instead of user-provided values.

## Ordered Flag Decision (from Pipeline design)

During Pipeline design, we evaluated an `ordered=true` flag for sequential execution. We dropped it because sequential execution without dependency support is a half-measure — it saves round-trips but doesn't enable result forwarding. TransactionPipeline is the proper solution for ordered, dependent operations.

## Open Questions

- Should non-dependent ops within a TransactionPipeline be parallelized? (DAG scheduling)
- What's the maximum reference depth before we need a proper expression language?
- Should this auto-detect whether a transaction is needed (only if writes are present)?
- How does this interact with multi-tenant isolation?

## Next Steps

When we're ready to implement this:
1. Design the reference/expression syntax
2. Decide on error propagation semantics
3. Define proto messages (extending PipelineOperation with optional `depends_on` and reference fields)
4. Prototype with a simple linear chain before supporting full DAG
