# Performance Tier 2: Request Pipelining Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.
> If modifying client libraries: verify imports work and run `just test-clients`.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

**Goal:** Add a `Pipeline` gRPC RPC that batches N independent operations in a single round-trip, executing them concurrently on the sidecar with per-operation error reporting.

**Architecture:** A new unary RPC accepts a list of operations (each a `oneof` of existing request types), fans them out concurrently via `tokio::join_all` with a semaphore-based concurrency limit, collects results (including per-op errors), and returns them indexed by position. MCP tool uses all-or-nothing safety validation. No new operations module needed — dispatches through existing `Operations` methods.

**Tech Stack:** tonic (gRPC), tokio (concurrency + semaphore), prost (proto), serde_json (MCP)

---

## File Structure

| File | Responsibility |
|------|---------------|
| `proto/mongocore/v1/mongocore.proto` | `Pipeline` RPC definition, `PipelineRequest`, `PipelineOperation`, `PipelineResponse`, `PipelineResult`, `PipelineError` messages |
| `src/defaults.rs` | `DEFAULT_PIPELINE_MAX_OPS`, `DEFAULT_PIPELINE_TIMEOUT_SECS`, `DEFAULT_PIPELINE_MAX_CONCURRENCY` constants |
| `src/config.rs` | `pipeline_timeout_secs`, `pipeline_max_concurrency` CLI args and config fields |
| `src/grpc/service.rs` | `pipeline()` handler, `execute_pipeline_op()` dispatch helper |
| `src/mcp/tools.rs` | `pipeline` tool definition and executor |
| `src/mcp/safety.rs` | `check_pipeline_allowed()` for all-or-nothing validation |
| `src/mcp/handler.rs` | No changes needed (dispatches via `execute_tool` which already routes by name) |
| `src/analytics/mod.rs` | Add `OperationKind::Pipeline` variant (for the parent operation record) |
| `tests/integration/pipeline_test.rs` | Integration tests for pipeline RPC |
| `clients/python/src/mongocore/client.py` | `pipeline()` method + `ops` module |
| `clients/go/client.go` | `Pipeline()` method + `ops` package |
| `clients/typescript/src/client.ts` | `pipeline()` method + `ops` module |
| `clients/java/src/main/java/com/mongocore/client/MongoClient.java` | `pipeline()` method + `Ops` class |

---

### Task 1: Proto Definition

**Files:**
- Modify: `proto/mongocore/v1/mongocore.proto`

- [ ] **Step 1: Add Pipeline RPC to service definition**

In `proto/mongocore/v1/mongocore.proto`, add to the `MongoCore` service block (after the last existing RPC):

```protobuf
  // Pipeline
  rpc Pipeline(PipelineRequest) returns (PipelineResponse);
```

- [ ] **Step 2: Add Pipeline messages**

At the end of the file (after existing messages), add:

```protobuf
// --- Pipeline ---

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

- [ ] **Step 3: Verify proto compiles**

Run: `cargo build 2>&1 | head -20`
Expected: Build succeeds (tonic-build auto-generates Rust stubs from proto)

- [ ] **Step 4: Commit**

```bash
git add proto/mongocore/v1/mongocore.proto
git commit -m "feat(grpc): add Pipeline RPC proto definition"
```

---

### Task 2: Config & Defaults

**Files:**
- Modify: `src/defaults.rs`
- Modify: `src/config.rs`

- [ ] **Step 1: Add default constants**

In `src/defaults.rs`, add after the existing streaming constants:

```rust
pub const DEFAULT_PIPELINE_MAX_OPS: usize = 100;
pub const DEFAULT_PIPELINE_TIMEOUT_SECS: u64 = 30;
pub const DEFAULT_PIPELINE_MAX_CONCURRENCY: usize = 20;
```

- [ ] **Step 2: Add CLI args**

In `src/config.rs`, add to `CliArgs` struct (after the streaming args):

```rust
    #[arg(long, env = "MONGOCORE_PIPELINE_TIMEOUT_SECS")]
    pub pipeline_timeout_secs: Option<u64>,

    #[arg(long, env = "MONGOCORE_PIPELINE_MAX_CONCURRENCY")]
    pub pipeline_max_concurrency: Option<usize>,
```

- [ ] **Step 3: Add to FileConfig**

In `src/config.rs`, add to the `FileConfig` struct:

```rust
    pub pipeline_timeout_secs: Option<u64>,
    pub pipeline_max_concurrency: Option<usize>,
```

- [ ] **Step 4: Add to Config struct and resolution**

In `src/config.rs`, add fields to `Config`:

```rust
    pub pipeline_timeout_secs: u64,
    pub pipeline_max_concurrency: usize,
```

In `Config::load()`, add resolution (after stream_idle_timeout_secs):

```rust
        let pipeline_timeout_secs = cli
            .pipeline_timeout_secs
            .or(file_config.pipeline_timeout_secs)
            .unwrap_or(DEFAULT_PIPELINE_TIMEOUT_SECS);

        let pipeline_max_concurrency = cli
            .pipeline_max_concurrency
            .or(file_config.pipeline_max_concurrency)
            .unwrap_or(DEFAULT_PIPELINE_MAX_CONCURRENCY);
```

And include them in the Config struct literal.

- [ ] **Step 5: Update all Config struct literals in tests**

Search for `Config {` in `src/` and `tests/` — add `pipeline_timeout_secs: DEFAULT_PIPELINE_TIMEOUT_SECS` and `pipeline_max_concurrency: DEFAULT_PIPELINE_MAX_CONCURRENCY` to every struct literal.

Run: `grep -rn "Config {" src/ tests/`

- [ ] **Step 6: Verify build**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output (zero warnings)

- [ ] **Step 7: Run unit tests**

Run: `cargo test --lib`
Expected: All tests pass

- [ ] **Step 8: Commit**

```bash
git add src/defaults.rs src/config.rs tests/
git commit -m "feat(config): add pipeline timeout and concurrency config"
```

---

### Task 3: Pipeline gRPC Handler

**Files:**
- Modify: `src/grpc/service.rs`
- Modify: `src/analytics/mod.rs` (if `OperationKind` enum exists there)

- [ ] **Step 1: Add OperationKind::Pipeline variant**

Find the `OperationKind` enum (likely in `src/analytics/mod.rs` or `src/analytics/collector.rs`) and add:

```rust
    Pipeline,
```

- [ ] **Step 2: Add pipeline_timeout and pipeline_max_concurrency to MongoCoreService**

In `src/grpc/service.rs`, add fields to the `MongoCoreService` struct:

```rust
    pipeline_timeout: Duration,
    pipeline_semaphore: Arc<tokio::sync::Semaphore>,
```

Update the constructor (`new()` or wherever MongoCoreService is built) to accept these from config:

```rust
    pipeline_timeout: Duration::from_secs(config.pipeline_timeout_secs),
    pipeline_semaphore: Arc::new(tokio::sync::Semaphore::new(config.pipeline_max_concurrency)),
```

- [ ] **Step 3: Implement the pipeline handler**

In `src/grpc/service.rs`, add the `pipeline` method to the `MongoCore` tonic trait implementation:

```rust
    #[tracing::instrument(skip(self, request))]
    async fn pipeline(
        &self,
        request: Request<proto::PipelineRequest>,
    ) -> Result<Response<proto::PipelineResponse>, Status> {
        self.append_client_language(request.metadata());
        self.check_tenant_quota(request.metadata())?;

        let start = std::time::Instant::now();
        let req = request.into_inner();

        if req.operations.is_empty() {
            return Err(Status::invalid_argument("Pipeline must contain at least one operation"));
        }

        if req.operations.len() > DEFAULT_PIPELINE_MAX_OPS {
            return Err(Status::invalid_argument(format!(
                "Pipeline exceeds maximum of {} operations",
                DEFAULT_PIPELINE_MAX_OPS
            )));
        }

        let semaphore = self.pipeline_semaphore.clone();
        let timeout_duration = self.pipeline_timeout;

        let futures: Vec<_> = req.operations.into_iter().enumerate().map(|(i, op)| {
            let sem = semaphore.clone();
            async move {
                let _permit = sem.acquire().await.unwrap();
                self.execute_pipeline_op(i as u32, op).await
            }
        }).collect();

        let results = match tokio::time::timeout(
            timeout_duration,
            futures::future::join_all(futures),
        ).await {
            Ok(results) => results,
            Err(_) => {
                return Err(Status::deadline_exceeded("Pipeline timeout exceeded"));
            }
        };

        let succeeded = results.iter().filter(|r| !r.is_error()).count() as u32;
        let failed = results.iter().filter(|r| r.is_error()).count() as u32;

        self.record_analytics(
            OperationKind::Pipeline,
            "",
            "",
            start.elapsed(),
            failed == 0,
        );

        Ok(Response::new(proto::PipelineResponse {
            results,
            succeeded,
            failed,
        }))
    }
```

- [ ] **Step 4: Implement execute_pipeline_op dispatch**

Add a helper method to `MongoCoreService`:

```rust
    async fn execute_pipeline_op(
        &self,
        index: u32,
        op: proto::PipelineOperation,
    ) -> proto::PipelineResult {
        use proto::pipeline_operation::Operation;

        let result = match op.operation {
            Some(Operation::Find(req)) => self.execute_pipeline_find(req).await,
            Some(Operation::FindOne(req)) => self.execute_pipeline_find_one(req).await,
            Some(Operation::Insert(req)) => self.execute_pipeline_insert(req).await,
            Some(Operation::InsertMany(req)) => self.execute_pipeline_insert_many(req).await,
            Some(Operation::Update(req)) => self.execute_pipeline_update(req).await,
            Some(Operation::UpdateMany(req)) => self.execute_pipeline_update_many(req).await,
            Some(Operation::Delete(req)) => self.execute_pipeline_delete(req).await,
            Some(Operation::DeleteMany(req)) => self.execute_pipeline_delete_many(req).await,
            Some(Operation::Aggregate(req)) => self.execute_pipeline_aggregate(req).await,
            Some(Operation::FindAndModify(req)) => self.execute_pipeline_find_and_modify(req).await,
            Some(Operation::RunCommand(req)) => self.execute_pipeline_run_command(req).await,
            Some(Operation::Search(req)) => self.execute_pipeline_search(req).await,
            Some(Operation::CreateCollection(req)) => self.execute_pipeline_create_collection(req).await,
            Some(Operation::CreateIndex(req)) => self.execute_pipeline_create_index(req).await,
            Some(Operation::ListDatabases(req)) => self.execute_pipeline_list_databases(req).await,
            Some(Operation::ListCollections(req)) => self.execute_pipeline_list_collections(req).await,
            Some(Operation::BeginTransaction(req)) => self.execute_pipeline_begin_transaction(req).await,
            Some(Operation::CommitTransaction(req)) => self.execute_pipeline_commit_transaction(req).await,
            Some(Operation::AbortTransaction(req)) => self.execute_pipeline_abort_transaction(req).await,
            Some(Operation::GetAnalytics(req)) => self.execute_pipeline_get_analytics(req).await,
            None => proto::pipeline_result::Result::Error(proto::PipelineError {
                code: 3, // INVALID_ARGUMENT
                message: "Operation is empty".to_string(),
            }),
        };

        proto::PipelineResult {
            index,
            result: Some(result),
        }
    }
```

- [ ] **Step 5: Implement per-operation dispatch helpers**

Each helper reuses the existing operation logic. Example for `find`:

```rust
    async fn execute_pipeline_find(&self, req: proto::FindRequest) -> proto::pipeline_result::Result {
        let filter = match proto_filter_to_bson(&req.filter) {
            Ok(f) => f,
            Err(e) => return proto::pipeline_result::Result::Error(proto::PipelineError {
                code: 3,
                message: e.message().to_string(),
            }),
        };
        let options = match convert_find_options(&req.options) {
            Ok(o) => o,
            Err(e) => return proto::pipeline_result::Result::Error(proto::PipelineError {
                code: 3,
                message: e.message().to_string(),
            }),
        };

        let result = if let Some(ref txn_id) = req.transaction_id {
            match self.transactions.get_mut(txn_id) {
                Some(mut txn) => txn.find(&req.database, &req.collection, filter).await,
                None => return proto::pipeline_result::Result::Error(proto::PipelineError {
                    code: 5,
                    message: format!("Transaction not found: {}", txn_id),
                }),
            }
        } else {
            self.operations.find(&req.database, &req.collection, filter, options).await
        };

        match result {
            Ok(docs) => {
                let documents: Result<Vec<proto::Document>, _> = docs.iter().map(bson_to_proto_doc).collect();
                match documents {
                    Ok(d) => proto::pipeline_result::Result::Find(proto::FindResponse {
                        documents: d,
                        metadata: Some(proto::ResponseMetadata { search_method: String::new() }),
                    }),
                    Err(e) => proto::pipeline_result::Result::Error(proto::PipelineError {
                        code: 13,
                        message: e.message().to_string(),
                    }),
                }
            }
            Err(e) => proto::pipeline_result::Result::Error(proto::PipelineError {
                code: 13,
                message: e.to_string(),
            }),
        }
    }
```

Implement the same pattern for each operation variant: `execute_pipeline_find_one`, `execute_pipeline_insert`, `execute_pipeline_insert_many`, `execute_pipeline_update`, `execute_pipeline_update_many`, `execute_pipeline_delete`, `execute_pipeline_delete_many`, `execute_pipeline_aggregate`, `execute_pipeline_find_and_modify`, `execute_pipeline_run_command`, `execute_pipeline_search`, `execute_pipeline_create_collection`, `execute_pipeline_create_index`, `execute_pipeline_list_databases`, `execute_pipeline_list_collections`, `execute_pipeline_begin_transaction`, `execute_pipeline_commit_transaction`, `execute_pipeline_abort_transaction`, `execute_pipeline_get_analytics`.

Each follows the same structure:
1. Parse/convert the request fields (return `PipelineError` on parse failure)
2. Dispatch to the existing operation on `self.operations` or `self.transactions`
3. Wrap the success response in the appropriate `pipeline_result::Result` variant
4. Wrap errors in `PipelineError`

- [ ] **Step 6: Add is_error helper to PipelineResult**

In `src/grpc/service.rs` (or a helper module), add:

```rust
impl proto::PipelineResult {
    fn is_error(&self) -> bool {
        matches!(self.result, Some(proto::pipeline_result::Result::Error(_)))
    }
}
```

- [ ] **Step 7: Verify build compiles with zero warnings**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output

- [ ] **Step 8: Run unit tests**

Run: `cargo test --lib`
Expected: All pass

- [ ] **Step 9: Commit**

```bash
git add src/grpc/service.rs src/analytics/ src/main.rs
git commit -m "feat(grpc): implement Pipeline RPC handler with concurrent dispatch"
```

---

### Task 4: MCP Pipeline Tool

**Files:**
- Modify: `src/mcp/tools.rs`
- Modify: `src/mcp/safety.rs`

- [ ] **Step 1: Add pipeline tool definition**

In `src/mcp/tools.rs`, add to the `tool_definitions()` vec:

```rust
        McpToolDefinition {
            name: "pipeline".to_string(),
            description: "Execute multiple independent operations concurrently in a single round-trip. All operations are validated before execution — if any violates safety rules, the entire pipeline is rejected.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operations": {
                        "type": "array",
                        "description": "List of operations to execute concurrently",
                        "items": {
                            "type": "object",
                            "properties": {
                                "op": {
                                    "type": "string",
                                    "enum": ["find", "find_one", "insert", "insert_many", "update", "update_many", "delete", "delete_many", "aggregate", "find_and_modify", "run_command", "search", "create_collection", "create_index", "list_databases", "list_collections", "begin_transaction", "commit_transaction", "abort_transaction", "get_analytics"],
                                    "description": "Operation type"
                                },
                                "database": { "type": "string", "description": "Database name" },
                                "collection": { "type": "string", "description": "Collection name" },
                                "filter": { "type": "object", "description": "Query filter" },
                                "document": { "type": "object", "description": "Document to insert" },
                                "documents": { "type": "array", "description": "Documents for insert_many" },
                                "pipeline": { "type": "array", "description": "Aggregation pipeline stages" },
                                "update": { "type": "object", "description": "Update specification" },
                                "command": { "type": "object", "description": "Raw command document" },
                                "options": { "type": "object", "description": "Operation-specific options (limit, skip, sort, projection, upsert)" }
                            },
                            "required": ["op"]
                        },
                        "maxItems": 100
                    }
                },
                "required": ["operations"]
            }),
        },
```

- [ ] **Step 2: Add all-or-nothing safety check**

In `src/mcp/safety.rs`, add a method to `SafetyConfig`:

```rust
    pub fn check_pipeline_allowed(&self, operations: &[Value]) -> Result<(), String> {
        if !self.read_only {
            return Ok(());
        }

        let mut violations = Vec::new();
        for (i, op) in operations.iter().enumerate() {
            if let Some(op_type) = op.get("op").and_then(|v| v.as_str()) {
                const WRITE_OPS: &[&str] = &[
                    "insert", "insert_many", "update", "update_many",
                    "delete", "delete_many", "create_collection",
                    "create_index", "run_command", "find_and_modify",
                ];
                if WRITE_OPS.contains(&op_type) {
                    violations.push(format!("operation[{}]: '{}' is a write operation", i, op_type));
                }
            }
        }

        if violations.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Pipeline rejected: server is in read-only mode. Violations:\n{}",
                violations.join("\n")
            ))
        }
    }
```

- [ ] **Step 3: Add pipeline executor**

In `src/mcp/tools.rs`, add the executor function and wire it into `execute_tool`:

Add to the `match name` block in `execute_tool`:
```rust
        "pipeline" => execute_pipeline(operations, arguments, safety).await,
```

Note: `execute_tool` needs to accept `safety: &SafetyConfig` as a parameter (or pass it from the handler). Check the existing signature and add it if not already present.

The executor:

```rust
async fn execute_pipeline(
    operations: &Operations,
    args: &Value,
    safety: &SafetyConfig,
) -> McpToolCallResult {
    let ops = match args.get("operations").and_then(|v| v.as_array()) {
        Some(ops) => ops,
        None => return error_result("Missing required field: operations".to_string()),
    };

    if ops.is_empty() {
        return error_result("Pipeline must contain at least one operation".to_string());
    }

    if ops.len() > DEFAULT_PIPELINE_MAX_OPS {
        return error_result(format!("Pipeline exceeds maximum of {} operations", DEFAULT_PIPELINE_MAX_OPS));
    }

    // All-or-nothing safety check
    if let Err(reason) = safety.check_pipeline_allowed(ops) {
        return McpToolCallResult {
            content: vec![McpContent { type_: "text".to_string(), text: reason }],
            is_error: true,
        };
    }

    // Execute each operation concurrently
    let futures: Vec<_> = ops.iter().enumerate().map(|(i, op)| {
        execute_single_mcp_op(operations, i, op)
    }).collect();

    let results = futures::future::join_all(futures).await;

    let succeeded = results.iter().filter(|r| r.1).count();
    let failed = results.len() - succeeded;

    let output = json!({
        "results": results.iter().map(|(val, _)| val.clone()).collect::<Vec<_>>(),
        "succeeded": succeeded,
        "failed": failed,
    });

    success_result(serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string()))
}

async fn execute_single_mcp_op(
    operations: &Operations,
    index: usize,
    op: &Value,
) -> (Value, bool) {
    let op_type = match op.get("op").and_then(|v| v.as_str()) {
        Some(t) => t,
        None => return (json!({"index": index, "error": "Missing 'op' field"}), false),
    };

    // Dispatch to existing tool executors by constructing the expected args
    let result = match op_type {
        "find" => execute_find(operations, op).await,
        "find_one" => execute_find_one(operations, op).await,
        "insert" => execute_insert(operations, op).await,
        "insert_many" => execute_insert_many(operations, op).await,
        "update" => execute_update(operations, op).await,
        "update_many" => execute_update_many(operations, op).await,
        "delete" => execute_delete(operations, op).await,
        "delete_many" => execute_delete_many(operations, op).await,
        "aggregate" => execute_aggregate(operations, op).await,
        "find_and_modify" => execute_find_and_modify(operations, op).await,
        "run_command" => execute_run_command(operations, op).await,
        "list_databases" => execute_list_databases(operations, op).await,
        "list_collections" => execute_list_collections(operations, op).await,
        _ => error_result(format!("Unsupported pipeline operation: {}", op_type)),
    };

    let success = !result.is_error;
    let value = json!({
        "index": index,
        "op": op_type,
        "success": success,
        "content": result.content.first().map(|c| &c.text).unwrap_or(&String::new()).clone(),
    });
    (value, success)
}
```

- [ ] **Step 4: Update MCP tool count assertion**

In `tests/integration/mcp_test.rs`, find the tool count assertion and increment it by 1.

- [ ] **Step 5: Verify build**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output

- [ ] **Step 6: Run unit tests**

Run: `cargo test --lib`
Expected: All pass

- [ ] **Step 7: Commit**

```bash
git add src/mcp/tools.rs src/mcp/safety.rs tests/integration/mcp_test.rs
git commit -m "feat(mcp): add pipeline tool with all-or-nothing safety enforcement"
```

---

### Task 5: Integration Tests

**Files:**
- Create: `tests/integration/pipeline_test.rs`
- Modify: `tests/integration/mod.rs` (if it exists, to include the new module)

- [ ] **Step 1: Create pipeline integration test file**

Create `tests/integration/pipeline_test.rs`:

```rust
use bson::doc;
use uuid::Uuid;

#[path = "../harness/mod.rs"]
mod harness;

fn unique_collection() -> String {
    format!("test_pipeline_{}", Uuid::new_v4().to_string().replace('-', ""))
}

#[tokio::test]
async fn test_pipeline_mixed_operations() {
    let pool = harness::get_test_pool().await;
    let ops = mongocore::operations::crud::Operations::new(pool.clone());
    let coll = unique_collection();

    // Seed data
    ops.insert(harness::TEST_DB, &coll, doc! { "name": "Alice", "age": 30 })
        .await
        .unwrap();
    ops.insert(harness::TEST_DB, &coll, doc! { "name": "Bob", "age": 25 })
        .await
        .unwrap();

    // Test via gRPC client
    let mut client = harness::grpc_client().await;

    use mongocore::proto::pipeline_operation::Operation;
    let request = tonic::Request::new(mongocore::proto::PipelineRequest {
        operations: vec![
            mongocore::proto::PipelineOperation {
                operation: Some(Operation::Find(mongocore::proto::FindRequest {
                    database: harness::TEST_DB.to_string(),
                    collection: coll.clone(),
                    filter: Some(mongocore::proto::Filter {
                        data: bson::to_vec(&doc! { "age": { "$gte": 25 } }).unwrap(),
                    }),
                    options: None,
                    transaction_id: None,
                })),
            },
            mongocore::proto::PipelineOperation {
                operation: Some(Operation::Insert(mongocore::proto::InsertRequest {
                    database: harness::TEST_DB.to_string(),
                    collection: coll.clone(),
                    document: Some(mongocore::proto::Document {
                        data: bson::to_vec(&doc! { "name": "Charlie", "age": 35 }).unwrap(),
                    }),
                    transaction_id: None,
                })),
            },
        ],
    });

    let response = client.pipeline(request).await.unwrap().into_inner();

    assert_eq!(response.results.len(), 2);
    assert_eq!(response.succeeded, 2);
    assert_eq!(response.failed, 0);

    // Verify find returned both docs
    let find_result = &response.results[0];
    assert_eq!(find_result.index, 0);
    assert!(matches!(
        find_result.result,
        Some(mongocore::proto::pipeline_result::Result::Find(_))
    ));

    // Verify insert succeeded
    let insert_result = &response.results[1];
    assert_eq!(insert_result.index, 1);
    assert!(matches!(
        insert_result.result,
        Some(mongocore::proto::pipeline_result::Result::Insert(_))
    ));
}

#[tokio::test]
async fn test_pipeline_empty_rejected() {
    let mut client = harness::grpc_client().await;

    let request = tonic::Request::new(mongocore::proto::PipelineRequest {
        operations: vec![],
    });

    let err = client.pipeline(request).await.unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
    assert!(err.message().contains("at least one operation"));
}

#[tokio::test]
async fn test_pipeline_partial_failure() {
    let mut client = harness::grpc_client().await;
    let coll = unique_collection();

    use mongocore::proto::pipeline_operation::Operation;
    let request = tonic::Request::new(mongocore::proto::PipelineRequest {
        operations: vec![
            // Valid: find on non-existent collection (returns empty, not error)
            mongocore::proto::PipelineOperation {
                operation: Some(Operation::Find(mongocore::proto::FindRequest {
                    database: harness::TEST_DB.to_string(),
                    collection: coll.clone(),
                    filter: None,
                    options: None,
                    transaction_id: None,
                })),
            },
            // Invalid: reference a non-existent transaction
            mongocore::proto::PipelineOperation {
                operation: Some(Operation::Find(mongocore::proto::FindRequest {
                    database: harness::TEST_DB.to_string(),
                    collection: coll.clone(),
                    filter: None,
                    options: None,
                    transaction_id: Some("nonexistent_txn".to_string()),
                })),
            },
        ],
    });

    let response = client.pipeline(request).await.unwrap().into_inner();

    assert_eq!(response.succeeded, 1);
    assert_eq!(response.failed, 1);

    // First op succeeded
    assert!(matches!(
        response.results[0].result,
        Some(mongocore::proto::pipeline_result::Result::Find(_))
    ));

    // Second op failed with error
    assert!(matches!(
        response.results[1].result,
        Some(mongocore::proto::pipeline_result::Result::Error(_))
    ));
}

#[tokio::test]
async fn test_pipeline_exceeds_max_ops() {
    let mut client = harness::grpc_client().await;

    use mongocore::proto::pipeline_operation::Operation;
    let operations: Vec<_> = (0..101)
        .map(|_| mongocore::proto::PipelineOperation {
            operation: Some(Operation::ListDatabases(mongocore::proto::ListDatabasesRequest {})),
        })
        .collect();

    let request = tonic::Request::new(mongocore::proto::PipelineRequest { operations });

    let err = client.pipeline(request).await.unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
    assert!(err.message().contains("100"));
}

#[tokio::test]
async fn test_pipeline_concurrent_execution() {
    let mut client = harness::grpc_client().await;
    let coll = unique_collection();

    // Insert some data first
    let pool = harness::get_test_pool().await;
    let ops = mongocore::operations::crud::Operations::new(pool);
    for i in 0..10 {
        ops.insert(harness::TEST_DB, &coll, doc! { "i": i }).await.unwrap();
    }

    use mongocore::proto::pipeline_operation::Operation;
    // Run 5 concurrent finds — should all return the same data
    let operations: Vec<_> = (0..5)
        .map(|_| mongocore::proto::PipelineOperation {
            operation: Some(Operation::Find(mongocore::proto::FindRequest {
                database: harness::TEST_DB.to_string(),
                collection: coll.clone(),
                filter: None,
                options: None,
                transaction_id: None,
            })),
        })
        .collect();

    let request = tonic::Request::new(mongocore::proto::PipelineRequest { operations });
    let response = client.pipeline(request).await.unwrap().into_inner();

    assert_eq!(response.succeeded, 5);
    assert_eq!(response.failed, 0);

    // All 5 should return 10 documents each
    for result in &response.results {
        if let Some(mongocore::proto::pipeline_result::Result::Find(ref find)) = result.result {
            assert_eq!(find.documents.len(), 10);
        } else {
            panic!("Expected Find result");
        }
    }
}
```

- [ ] **Step 2: Verify integration tests compile**

Run: `cargo test --test integration --no-run`
Expected: Compiles successfully

- [ ] **Step 3: Run integration tests (requires Docker MongoDB)**

Run: `cargo test --test integration pipeline -- --nocapture`
Expected: All 5 tests pass

- [ ] **Step 4: Commit**

```bash
git add tests/integration/pipeline_test.rs
git commit -m "test(grpc): add pipeline RPC integration tests"
```

---

### Task 6: Python Client

**Files:**
- Modify: `clients/python/src/mongocore/client.py`
- Create: `clients/python/src/mongocore/ops.py`
- Modify: `clients/python/src/mongocore/__init__.py`
- Modify: `clients/python/tests/test_client.py`

- [ ] **Step 1: Create ops module**

Create `clients/python/src/mongocore/ops.py`:

```python
"""Operation builders for the Pipeline API."""

from dataclasses import dataclass, field
from typing import Any, Optional


@dataclass
class PipelineOp:
    """Base class for pipeline operations."""
    op_type: str
    database: str = ""
    collection: str = ""


@dataclass
class FindOp(PipelineOp):
    filter: Optional[dict] = None
    options: Optional[dict] = None
    transaction_id: Optional[str] = None

    def __init__(self, database: str, collection: str, filter: Optional[dict] = None, **kwargs):
        self.op_type = "find"
        self.database = database
        self.collection = collection
        self.filter = filter
        self.options = kwargs.get("options")
        self.transaction_id = kwargs.get("transaction_id")


@dataclass
class FindOneOp(PipelineOp):
    filter: Optional[dict] = None
    transaction_id: Optional[str] = None

    def __init__(self, database: str, collection: str, filter: Optional[dict] = None, **kwargs):
        self.op_type = "find_one"
        self.database = database
        self.collection = collection
        self.filter = filter
        self.transaction_id = kwargs.get("transaction_id")


@dataclass
class InsertOp(PipelineOp):
    document: Optional[dict] = None
    transaction_id: Optional[str] = None

    def __init__(self, database: str, collection: str, document: dict, **kwargs):
        self.op_type = "insert"
        self.database = database
        self.collection = collection
        self.document = document
        self.transaction_id = kwargs.get("transaction_id")


@dataclass
class InsertManyOp(PipelineOp):
    documents: list = field(default_factory=list)
    transaction_id: Optional[str] = None

    def __init__(self, database: str, collection: str, documents: list, **kwargs):
        self.op_type = "insert_many"
        self.database = database
        self.collection = collection
        self.documents = documents
        self.transaction_id = kwargs.get("transaction_id")


@dataclass
class UpdateOp(PipelineOp):
    filter: Optional[dict] = None
    update: Optional[dict] = None
    upsert: bool = False
    transaction_id: Optional[str] = None

    def __init__(self, database: str, collection: str, filter: dict, update: dict, **kwargs):
        self.op_type = "update"
        self.database = database
        self.collection = collection
        self.filter = filter
        self.update = update
        self.upsert = kwargs.get("upsert", False)
        self.transaction_id = kwargs.get("transaction_id")


@dataclass
class UpdateManyOp(PipelineOp):
    filter: Optional[dict] = None
    update: Optional[dict] = None
    upsert: bool = False
    transaction_id: Optional[str] = None

    def __init__(self, database: str, collection: str, filter: dict, update: dict, **kwargs):
        self.op_type = "update_many"
        self.database = database
        self.collection = collection
        self.filter = filter
        self.update = update
        self.upsert = kwargs.get("upsert", False)
        self.transaction_id = kwargs.get("transaction_id")


@dataclass
class DeleteOp(PipelineOp):
    filter: Optional[dict] = None
    transaction_id: Optional[str] = None

    def __init__(self, database: str, collection: str, filter: dict, **kwargs):
        self.op_type = "delete"
        self.database = database
        self.collection = collection
        self.filter = filter
        self.transaction_id = kwargs.get("transaction_id")


@dataclass
class DeleteManyOp(PipelineOp):
    filter: Optional[dict] = None
    transaction_id: Optional[str] = None

    def __init__(self, database: str, collection: str, filter: dict, **kwargs):
        self.op_type = "delete_many"
        self.database = database
        self.collection = collection
        self.filter = filter
        self.transaction_id = kwargs.get("transaction_id")


@dataclass
class AggregateOp(PipelineOp):
    pipeline: list = field(default_factory=list)
    transaction_id: Optional[str] = None

    def __init__(self, database: str, collection: str, pipeline: list, **kwargs):
        self.op_type = "aggregate"
        self.database = database
        self.collection = collection
        self.pipeline = pipeline
        self.transaction_id = kwargs.get("transaction_id")


@dataclass
class RunCommandOp(PipelineOp):
    command: Optional[dict] = None

    def __init__(self, database: str, command: dict):
        self.op_type = "run_command"
        self.database = database
        self.collection = ""
        self.command = command


@dataclass
class ListDatabasesOp(PipelineOp):
    def __init__(self):
        self.op_type = "list_databases"
        self.database = ""
        self.collection = ""


@dataclass
class ListCollectionsOp(PipelineOp):
    def __init__(self, database: str):
        self.op_type = "list_collections"
        self.database = database
        self.collection = ""


# Convenience functions
def find(database: str, collection: str, filter: Optional[dict] = None, **kwargs) -> FindOp:
    return FindOp(database, collection, filter, **kwargs)

def find_one(database: str, collection: str, filter: Optional[dict] = None, **kwargs) -> FindOneOp:
    return FindOneOp(database, collection, filter, **kwargs)

def insert(database: str, collection: str, document: dict, **kwargs) -> InsertOp:
    return InsertOp(database, collection, document, **kwargs)

def insert_many(database: str, collection: str, documents: list, **kwargs) -> InsertManyOp:
    return InsertManyOp(database, collection, documents, **kwargs)

def update(database: str, collection: str, filter: dict, update: dict, **kwargs) -> UpdateOp:
    return UpdateOp(database, collection, filter, update, **kwargs)

def update_many(database: str, collection: str, filter: dict, update: dict, **kwargs) -> UpdateManyOp:
    return UpdateManyOp(database, collection, filter, update, **kwargs)

def delete(database: str, collection: str, filter: dict, **kwargs) -> DeleteOp:
    return DeleteOp(database, collection, filter, **kwargs)

def delete_many(database: str, collection: str, filter: dict, **kwargs) -> DeleteManyOp:
    return DeleteManyOp(database, collection, filter, **kwargs)

def aggregate(database: str, collection: str, pipeline: list, **kwargs) -> AggregateOp:
    return AggregateOp(database, collection, pipeline, **kwargs)

def run_command(database: str, command: dict) -> RunCommandOp:
    return RunCommandOp(database, command)

def list_databases() -> ListDatabasesOp:
    return ListDatabasesOp()

def list_collections(database: str) -> ListCollectionsOp:
    return ListCollectionsOp(database)
```

- [ ] **Step 2: Add PipelineResult class and pipeline method to client**

In `clients/python/src/mongocore/client.py`, add the result class and method:

```python
@dataclass
class PipelineResult:
    """Result of a single operation within a pipeline."""
    index: int
    success: bool
    error: Optional[str] = None
    _raw: Any = None

    @property
    def documents(self) -> list:
        """For find/aggregate results."""
        if hasattr(self._raw, 'documents'):
            return [bson_to_dict(d) for d in self._raw.documents]
        return []

    @property
    def document(self) -> Optional[dict]:
        """For find_one results."""
        if hasattr(self._raw, 'document') and self._raw.document:
            return bson_to_dict(self._raw.document)
        return None

    @property
    def inserted_id(self) -> Optional[str]:
        """For insert results."""
        if hasattr(self._raw, 'inserted_id'):
            return self._raw.inserted_id
        return None
```

Add the `pipeline` method to `MongoClient`:

```python
    async def pipeline(self, *operations: "PipelineOp") -> list[PipelineResult]:
        """Execute multiple independent operations concurrently in a single round-trip."""
        from .ops import PipelineOp
        from .generated import mongocore_pb2

        proto_ops = []
        for op in operations:
            proto_op = self._build_pipeline_op(op)
            proto_ops.append(proto_op)

        request = mongocore_pb2.PipelineRequest(operations=proto_ops)
        response = await self._stub.Pipeline(request)

        results = []
        for r in response.results:
            result_field = r.WhichOneof("result")
            if result_field == "error":
                results.append(PipelineResult(
                    index=r.index,
                    success=False,
                    error=r.error.message,
                ))
            else:
                results.append(PipelineResult(
                    index=r.index,
                    success=True,
                    _raw=getattr(r, result_field),
                ))
        return results
```

- [ ] **Step 3: Export ops module from __init__.py**

In `clients/python/src/mongocore/__init__.py`, add:

```python
from . import ops
```

- [ ] **Step 4: Regenerate Python proto stubs**

```bash
cd clients/python && python -m grpc_tools.protoc -I../../proto \
  --python_out=src/mongocore/generated --grpc_python_out=src/mongocore/generated \
  ../../proto/mongocore/v1/mongocore.proto ../../proto/mongocore/v1/types.proto \
  ../../proto/mongocore/v1/ingestion.proto
```

- [ ] **Step 5: Add pipeline test to Python client tests**

In `clients/python/tests/test_client.py`, add:

```python
async def test_pipeline():
    """Test pipeline with mixed operations."""
    from mongocore import ops

    client = await get_client()
    coll = unique_collection()

    # Seed data
    await client.insert("test", coll, {"name": "Alice", "age": 30})

    results = await client.pipeline(
        ops.find("test", coll, {"name": "Alice"}),
        ops.insert("test", coll, {"name": "Bob", "age": 25}),
        ops.list_databases(),
    )

    assert len(results) == 3
    assert results[0].success
    assert len(results[0].documents) == 1
    assert results[1].success
    assert results[2].success
    print("  ✓ test_pipeline passed")
```

- [ ] **Step 6: Commit**

```bash
git add clients/python/
git commit -m "feat(clients): add Python pipeline client with ops module"
```

---

### Task 7: Go Client

**Files:**
- Modify: `clients/go/client.go`
- Create: `clients/go/ops/ops.go`
- Modify: `clients/go/client_test.go`

- [ ] **Step 1: Create ops package**

Create `clients/go/ops/ops.go`:

```go
package ops

import (
    pb "github.com/mongocore/clients/go/proto"
)

func Find(database, collection string, filter map[string]interface{}) *pb.PipelineOperation {
    return &pb.PipelineOperation{
        Operation: &pb.PipelineOperation_Find{
            Find: &pb.FindRequest{
                Database:   database,
                Collection: collection,
                Filter:     marshalFilter(filter),
            },
        },
    }
}

func FindOne(database, collection string, filter map[string]interface{}) *pb.PipelineOperation {
    return &pb.PipelineOperation{
        Operation: &pb.PipelineOperation_FindOne{
            FindOne: &pb.FindOneRequest{
                Database:   database,
                Collection: collection,
                Filter:     marshalFilter(filter),
            },
        },
    }
}

func Insert(database, collection string, document map[string]interface{}) *pb.PipelineOperation {
    return &pb.PipelineOperation{
        Operation: &pb.PipelineOperation_Insert{
            Insert: &pb.InsertRequest{
                Database:   database,
                Collection: collection,
                Document:   marshalDocument(document),
            },
        },
    }
}

func Aggregate(database, collection string, pipeline []map[string]interface{}) *pb.PipelineOperation {
    return &pb.PipelineOperation{
        Operation: &pb.PipelineOperation_Aggregate{
            Aggregate: &pb.AggregateRequest{
                Database:   database,
                Collection: collection,
                Pipeline:   marshalPipeline(pipeline),
            },
        },
    }
}

func Delete(database, collection string, filter map[string]interface{}) *pb.PipelineOperation {
    return &pb.PipelineOperation{
        Operation: &pb.PipelineOperation_Delete{
            Delete: &pb.DeleteRequest{
                Database:   database,
                Collection: collection,
                Filter:     marshalFilter(filter),
            },
        },
    }
}

func ListDatabases() *pb.PipelineOperation {
    return &pb.PipelineOperation{
        Operation: &pb.PipelineOperation_ListDatabases{
            ListDatabases: &pb.ListDatabasesRequest{},
        },
    }
}
```

- [ ] **Step 2: Add Pipeline method to Go client**

In `clients/go/client.go`, add:

```go
type PipelineResult struct {
    Index   uint32
    Success bool
    Error   string
    Raw     interface{}
}

func (r *PipelineResult) AsFind() ([]*Document, error) {
    if !r.Success {
        return nil, fmt.Errorf("operation failed: %s", r.Error)
    }
    if find, ok := r.Raw.(*pb.FindResponse); ok {
        return unmarshalDocuments(find.Documents), nil
    }
    return nil, fmt.Errorf("result is not a Find response")
}

func (r *PipelineResult) AsFindOne() (*Document, error) {
    if !r.Success {
        return nil, fmt.Errorf("operation failed: %s", r.Error)
    }
    if find, ok := r.Raw.(*pb.FindOneResponse); ok {
        if find.Document != nil {
            return unmarshalDocument(find.Document), nil
        }
        return nil, nil
    }
    return nil, fmt.Errorf("result is not a FindOne response")
}

func (r *PipelineResult) AsInsert() (string, error) {
    if !r.Success {
        return "", fmt.Errorf("operation failed: %s", r.Error)
    }
    if ins, ok := r.Raw.(*pb.InsertResponse); ok {
        return ins.InsertedId, nil
    }
    return "", fmt.Errorf("result is not an Insert response")
}

func (c *Client) Pipeline(ctx context.Context, operations ...*pb.PipelineOperation) ([]*PipelineResult, error) {
    resp, err := c.client.Pipeline(ctx, &pb.PipelineRequest{
        Operations: operations,
    })
    if err != nil {
        return nil, err
    }

    results := make([]*PipelineResult, len(resp.Results))
    for i, r := range resp.Results {
        result := &PipelineResult{Index: r.Index}
        switch v := r.Result.(type) {
        case *pb.PipelineResult_Error:
            result.Success = false
            result.Error = v.Error.Message
        case *pb.PipelineResult_Find:
            result.Success = true
            result.Raw = v.Find
        case *pb.PipelineResult_FindOne:
            result.Success = true
            result.Raw = v.FindOne
        case *pb.PipelineResult_Insert:
            result.Success = true
            result.Raw = v.Insert
        default:
            result.Success = true
            result.Raw = r.Result
        }
        results[i] = result
    }
    return results, nil
}
```

- [ ] **Step 3: Regenerate Go proto stubs**

```bash
cd clients/go && protoc --go_out=./proto --go-grpc_out=./proto -I../../proto \
  ../../proto/mongocore/v1/mongocore.proto ../../proto/mongocore/v1/types.proto \
  ../../proto/mongocore/v1/ingestion.proto
```

- [ ] **Step 4: Add Go client test**

In `clients/go/client_test.go`, add:

```go
func TestPipeline(t *testing.T) {
    ctx := context.Background()
    client := getTestClient(t)
    coll := uniqueCollection()

    // Seed data
    _, err := client.Insert(ctx, "test", coll, map[string]interface{}{"name": "Alice", "age": 30})
    require.NoError(t, err)

    results, err := client.Pipeline(ctx,
        ops.Find("test", coll, map[string]interface{}{"name": "Alice"}),
        ops.Insert("test", coll, map[string]interface{}{"name": "Bob", "age": 25}),
        ops.ListDatabases(),
    )
    require.NoError(t, err)
    assert.Len(t, results, 3)
    assert.True(t, results[0].Success)
    assert.True(t, results[1].Success)
    assert.True(t, results[2].Success)

    docs, err := results[0].AsFind()
    require.NoError(t, err)
    assert.Len(t, docs, 1)

    fmt.Println("  ✓ TestPipeline passed")
}
```

- [ ] **Step 5: Commit**

```bash
git add clients/go/
git commit -m "feat(clients): add Go pipeline client with ops package"
```

---

### Task 8: TypeScript Client

**Files:**
- Modify: `clients/typescript/src/client.ts`
- Create: `clients/typescript/src/ops.ts`
- Modify: `clients/typescript/src/index.ts`
- Modify: `clients/typescript/tests/client.test.ts`

- [ ] **Step 1: Create ops module**

Create `clients/typescript/src/ops.ts`:

```typescript
export interface PipelineOp {
  opType: string;
  toProto(): any;
}

export function find(database: string, collection: string, filter?: Record<string, any>): PipelineOp {
  return {
    opType: "find",
    toProto() {
      return { find: { database, collection, filter: filter ? marshalFilter(filter) : undefined } };
    },
  };
}

export function findOne(database: string, collection: string, filter?: Record<string, any>): PipelineOp {
  return {
    opType: "find_one",
    toProto() {
      return { findOne: { database, collection, filter: filter ? marshalFilter(filter) : undefined } };
    },
  };
}

export function insert(database: string, collection: string, document: Record<string, any>): PipelineOp {
  return {
    opType: "insert",
    toProto() {
      return { insert: { database, collection, document: marshalDocument(document) } };
    },
  };
}

export function aggregate(database: string, collection: string, pipeline: Record<string, any>[]): PipelineOp {
  return {
    opType: "aggregate",
    toProto() {
      return { aggregate: { database, collection, pipeline: marshalPipeline(pipeline) } };
    },
  };
}

export function deleteFn(database: string, collection: string, filter: Record<string, any>): PipelineOp {
  return {
    opType: "delete",
    toProto() {
      return { delete: { database, collection, filter: marshalFilter(filter) } };
    },
  };
}

export function listDatabases(): PipelineOp {
  return {
    opType: "list_databases",
    toProto() {
      return { listDatabases: {} };
    },
  };
}
```

- [ ] **Step 2: Add pipeline method and result types to client**

In `clients/typescript/src/client.ts`, add:

```typescript
export interface PipelineResult {
  index: number;
  success: boolean;
  error?: string;
  raw: any;
  asFind(): Document[];
  asFindOne(): Document | null;
  asInsert(): { insertedId: string };
}

async pipeline(...operations: PipelineOp[]): Promise<PipelineResult[]> {
  const protoOps = operations.map(op => op.toProto());
  const response = await this.client.pipeline({ operations: protoOps });

  return response.results.map(r => ({
    index: r.index,
    success: !r.error,
    error: r.error?.message,
    raw: r,
    asFind() { return unmarshalDocuments(r.find?.documents ?? []); },
    asFindOne() { return r.findOne?.document ? unmarshalDocument(r.findOne.document) : null; },
    asInsert() { return { insertedId: r.insert?.insertedId ?? "" }; },
  }));
}
```

- [ ] **Step 3: Export ops from index.ts**

In `clients/typescript/src/index.ts`, add:

```typescript
export * as ops from "./ops";
```

- [ ] **Step 4: Regenerate TypeScript proto stubs**

```bash
cd clients/typescript && npx grpc_tools_node_protoc \
  --ts_out=src/generated --grpc_out=src/generated -I../../proto \
  ../../proto/mongocore/v1/mongocore.proto ../../proto/mongocore/v1/types.proto \
  ../../proto/mongocore/v1/ingestion.proto
```

- [ ] **Step 5: Add TypeScript test**

In `clients/typescript/tests/client.test.ts`, add:

```typescript
test("pipeline with mixed operations", async () => {
  const coll = uniqueCollection();
  await client.insert("test", coll, { name: "Alice", age: 30 });

  const results = await client.pipeline(
    ops.find("test", coll, { name: "Alice" }),
    ops.insert("test", coll, { name: "Bob", age: 25 }),
    ops.listDatabases(),
  );

  expect(results.length).toBe(3);
  expect(results[0].success).toBe(true);
  expect(results[0].asFind().length).toBe(1);
  expect(results[1].success).toBe(true);
  expect(results[2].success).toBe(true);
  console.log("  ✓ pipeline test passed");
});
```

- [ ] **Step 6: Commit**

```bash
git add clients/typescript/
git commit -m "feat(clients): add TypeScript pipeline client with ops module"
```

---

### Task 9: Java Client

**Files:**
- Modify: `clients/java/src/main/java/com/mongocore/client/MongoClient.java`
- Create: `clients/java/src/main/java/com/mongocore/client/Ops.java`
- Create: `clients/java/src/main/java/com/mongocore/client/PipelineResult.java`
- Modify: `clients/java/src/test/java/com/mongocore/client/ClientTest.java`

- [ ] **Step 1: Create Ops builder class**

Create `clients/java/src/main/java/com/mongocore/client/Ops.java`:

```java
package com.mongocore.client;

import com.mongocore.proto.*;
import java.util.Map;
import java.util.List;

public class Ops {
    public static PipelineOperation find(String database, String collection, Map<String, Object> filter) {
        return PipelineOperation.newBuilder()
            .setFind(FindRequest.newBuilder()
                .setDatabase(database)
                .setCollection(collection)
                .setFilter(BsonUtil.marshalFilter(filter))
                .build())
            .build();
    }

    public static PipelineOperation findOne(String database, String collection, Map<String, Object> filter) {
        return PipelineOperation.newBuilder()
            .setFindOne(FindOneRequest.newBuilder()
                .setDatabase(database)
                .setCollection(collection)
                .setFilter(BsonUtil.marshalFilter(filter))
                .build())
            .build();
    }

    public static PipelineOperation insert(String database, String collection, Map<String, Object> document) {
        return PipelineOperation.newBuilder()
            .setInsert(InsertRequest.newBuilder()
                .setDatabase(database)
                .setCollection(collection)
                .setDocument(BsonUtil.marshalDocument(document))
                .build())
            .build();
    }

    public static PipelineOperation aggregate(String database, String collection, List<Map<String, Object>> pipeline) {
        return PipelineOperation.newBuilder()
            .setAggregate(AggregateRequest.newBuilder()
                .setDatabase(database)
                .setCollection(collection)
                .addAllPipeline(BsonUtil.marshalPipeline(pipeline))
                .build())
            .build();
    }

    public static PipelineOperation delete(String database, String collection, Map<String, Object> filter) {
        return PipelineOperation.newBuilder()
            .setDelete(DeleteRequest.newBuilder()
                .setDatabase(database)
                .setCollection(collection)
                .setFilter(BsonUtil.marshalFilter(filter))
                .build())
            .build();
    }

    public static PipelineOperation listDatabases() {
        return PipelineOperation.newBuilder()
            .setListDatabases(ListDatabasesRequest.newBuilder().build())
            .build();
    }
}
```

- [ ] **Step 2: Create PipelineResult class**

Create `clients/java/src/main/java/com/mongocore/client/PipelineResult.java`:

```java
package com.mongocore.client;

import com.mongocore.proto.*;
import java.util.List;
import java.util.Map;
import java.util.Collections;

public class PipelineResult {
    private final int index;
    private final boolean success;
    private final String error;
    private final com.mongocore.proto.PipelineResult raw;

    public PipelineResult(com.mongocore.proto.PipelineResult raw) {
        this.raw = raw;
        this.index = raw.getIndex();
        if (raw.hasError()) {
            this.success = false;
            this.error = raw.getError().getMessage();
        } else {
            this.success = true;
            this.error = null;
        }
    }

    public int getIndex() { return index; }
    public boolean isSuccess() { return success; }
    public String getError() { return error; }

    public List<Map<String, Object>> asFind() {
        if (raw.hasFind()) {
            return BsonUtil.unmarshalDocuments(raw.getFind().getDocumentsList());
        }
        return Collections.emptyList();
    }

    public Map<String, Object> asFindOne() {
        if (raw.hasFindOne() && raw.getFindOne().hasDocument()) {
            return BsonUtil.unmarshalDocument(raw.getFindOne().getDocument());
        }
        return null;
    }

    public String asInsert() {
        if (raw.hasInsert()) {
            return raw.getInsert().getInsertedId();
        }
        return null;
    }
}
```

- [ ] **Step 3: Add pipeline method to MongoClient**

In `clients/java/src/main/java/com/mongocore/client/MongoClient.java`, add:

```java
    public List<PipelineResult> pipeline(PipelineOperation... operations) throws Exception {
        PipelineRequest request = PipelineRequest.newBuilder()
            .addAllOperations(java.util.Arrays.asList(operations))
            .build();

        PipelineResponse response = stub.pipeline(request);

        List<PipelineResult> results = new java.util.ArrayList<>();
        for (com.mongocore.proto.PipelineResult r : response.getResultsList()) {
            results.add(new PipelineResult(r));
        }
        return results;
    }
```

- [ ] **Step 4: Regenerate Java proto stubs**

```bash
cd clients/java && protoc --java_out=src/main/java --grpc-java_out=src/main/java \
  -I../../proto ../../proto/mongocore/v1/mongocore.proto \
  ../../proto/mongocore/v1/types.proto ../../proto/mongocore/v1/ingestion.proto
```

- [ ] **Step 5: Add Java test**

In `clients/java/src/test/java/com/mongocore/client/ClientTest.java`, add:

```java
@Test
public void testPipeline() throws Exception {
    String coll = uniqueCollection();
    client.insert("test", coll, Map.of("name", "Alice", "age", 30));

    List<PipelineResult> results = client.pipeline(
        Ops.find("test", coll, Map.of("name", "Alice")),
        Ops.insert("test", coll, Map.of("name", "Bob", "age", 25)),
        Ops.listDatabases()
    );

    assertEquals(3, results.size());
    assertTrue(results.get(0).isSuccess());
    assertEquals(1, results.get(0).asFind().size());
    assertTrue(results.get(1).isSuccess());
    assertTrue(results.get(2).isSuccess());
    System.out.println("  ✓ testPipeline passed");
}
```

- [ ] **Step 6: Commit**

```bash
git add clients/java/
git commit -m "feat(clients): add Java pipeline client with Ops builder"
```

---

### Task 10: Documentation & Final Verification

**Files:**
- Modify: `docs/roadmap.md`
- Modify: `docs/design/development-log.md`

- [ ] **Step 1: Update roadmap**

In `docs/roadmap.md`, add a `## v0.9 — Request Pipelining` section after v0.8:

```markdown
## v0.9 — Request Pipelining

- **Pipeline RPC** — Batch N independent operations in a single gRPC round-trip with concurrent execution
- **All non-streaming operations** — Find, FindOne, Insert, InsertMany, Update, UpdateMany, Delete, DeleteMany, Aggregate, FindAndModify, RunCommand, Search, CreateCollection, CreateIndex, ListDatabases, ListCollections, transactions, GetAnalytics
- **Concurrent execution** — Operations fan out via tokio with semaphore-based concurrency limit (default 20)
- **Per-operation errors** — Individual failures don't abort the pipeline; results indexed by position
- **Pipeline timeout** — Configurable deadline (default 30s) with cancellation of incomplete ops
- **MCP pipeline tool** — All-or-nothing safety validation (rejects entire pipeline if any op violates read-only mode)
- **Typed client builders** — `ops` modules in Python, TypeScript, Go, Java with typed result accessors
```

Move "Performance Tier 2" from the Future Roadmap table to the version history table with status **Complete**.

- [ ] **Step 2: Add development log entry**

In `docs/design/development-log.md`, add:

```markdown
## 2026-05-13: Performance Tier 2 — Request Pipelining

Implemented the Pipeline RPC to batch N independent operations in a single gRPC round-trip.

**Problem:** Every RPC was independent — an AI agent gathering context from 5 collections needed 5 round-trips (~5ms over TCP). Applications doing mixed reads+writes paid the same per-op overhead.

**Approach:** A single `Pipeline` unary RPC accepts a list of operations (oneof of all non-streaming request types), fans them out concurrently via `tokio::join_all` with a semaphore (20 concurrent ops), and returns indexed results with per-op error reporting. MCP tool uses all-or-nothing safety validation — if any op violates read-only mode, the entire pipeline is rejected before execution.

**Key decisions:**
- Always concurrent, no ordered flag (sequential + dependencies deferred to TransactionPipeline)
- Ingestion and streaming RPCs excluded (long-running, don't benefit from batching)
- Pipeline-level timeout (30s default) — prevents slow pipelines from holding resources
- Typed op builders in all 4 client languages for ergonomic API

**Learned:** The proto `oneof` for 20 operation types creates a large message definition but the implementation is mechanical — each per-op dispatch helper is ~20 lines following the same pattern.
```

- [ ] **Step 3: Run full test suite**

```bash
cargo build 2>&1 | grep "warning:"    # Must be empty
cargo test --lib                        # Unit tests
cargo test --test integration           # Integration tests (needs Docker)
just test-clients                       # Client tests (needs sidecar running)
```

Expected: All pass with zero warnings.

- [ ] **Step 4: Commit documentation**

```bash
git add docs/
git commit -m "docs: add v0.9 pipeline to roadmap and development log"
```
