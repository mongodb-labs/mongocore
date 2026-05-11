# MongoCore v2: Power Users & Operations — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add power-user escape hatches (raw wire protocol passthrough), query analytics (usage tracking, performance insights), and multi-tenant support (shared sidecar, isolated caches) to the existing MongoCore v1 sidecar.

**Architecture:** Three independent subsystems layered onto the existing Rust sidecar. Raw passthrough adds a new gRPC RPC that forwards arbitrary OP_MSG commands. Query analytics adds an event-driven collector that records every operation's metadata into an in-memory ring buffer with optional persistence. Multi-tenant adds a tenant-id context propagator that partitions connection pools, compiled query caches, and resource limits per tenant.

**Tech Stack:** Rust (existing tokio/tonic/axum stack), Protocol Buffers (new RPCs), DashMap/parking_lot (analytics ring buffer), MongoDB (analytics persistence + tenant config).

---

## Scope

The v2 design spec calls for three features:

1. **Raw wire protocol escape hatch** — power users bypass the opinionated layer and send arbitrary MongoDB commands
2. **Query analytics dashboard** — track compiled query usage, operation latency, and surface insights
3. **Multi-tenant support** — shared sidecar serving multiple tenants with isolated caches and quotas

These are independent subsystems. Each produces working, testable software on its own. The plan builds them in order of dependency (raw passthrough has no deps, analytics uses the operation layer, multi-tenant wraps everything).

---

## File Structure

### Subsystem 1: Raw Wire Protocol Passthrough

| Action | Path | Responsibility |
|--------|------|----------------|
| Create | `src/operations/raw.rs` | Execute arbitrary MongoDB commands via `runCommand` |
| Modify | `src/operations/mod.rs` | Export the new `raw` module |
| Modify | `proto/mongocore/v1/mongocore.proto` | Add `RunCommand` RPC |
| Modify | `src/grpc/service.rs` | Implement `RunCommand` handler |
| Modify | `src/mcp/tools.rs` | Add `run_command` MCP tool |
| Modify | `src/mcp/safety.rs` | Block `run_command` in read-only mode, add command allowlist |
| Create | `src/operations/raw_validator.rs` | Validate/block dangerous raw commands |
| Create | `tests/integration/raw_command_test.rs` | Integration tests for raw passthrough |

### Subsystem 2: Query Analytics

| Action | Path | Responsibility |
|--------|------|----------------|
| Create | `src/analytics/mod.rs` | Module entry, public API |
| Create | `src/analytics/collector.rs` | Event collector: records operation metadata |
| Create | `src/analytics/ring_buffer.rs` | Fixed-size ring buffer for recent events |
| Create | `src/analytics/aggregator.rs` | Computes top-N queries, latency percentiles, error rates |
| Create | `src/analytics/persistence.rs` | Optional flush to MongoDB collection |
| Create | `src/analytics/types.rs` | Event types, query fingerprints |
| Modify | `src/grpc/service.rs` | Instrument each RPC to emit analytics events |
| Modify | `src/mcp/handler.rs` | Instrument MCP tool calls to emit analytics events |
| Modify | `proto/mongocore/v1/mongocore.proto` | Add `GetAnalytics` RPC |
| Modify | `src/grpc/service.rs` | Implement `GetAnalytics` handler |
| Modify | `src/mcp/tools.rs` | Add `get_analytics` MCP tool |
| Modify | `src/config.rs` | Add analytics config fields |
| Modify | `src/main.rs` | Initialize analytics collector on startup |
| Create | `tests/integration/analytics_test.rs` | Integration tests for analytics |

### Subsystem 3: Multi-Tenant Support

| Action | Path | Responsibility |
|--------|------|----------------|
| Create | `src/tenant/mod.rs` | Module entry, tenant context |
| Create | `src/tenant/context.rs` | Tenant ID extraction from gRPC metadata / MCP headers |
| Create | `src/tenant/registry.rs` | Tenant registry: config, limits, pool mapping |
| Create | `src/tenant/isolation.rs` | Cache partitioning, connection pool isolation |
| Create | `src/tenant/quota.rs` | Rate limiting, max connections, cache size per tenant |
| Modify | `src/config.rs` | Add multi-tenant config (enabled, tenant list, defaults) |
| Modify | `src/connection/pool.rs` | Support per-tenant pool or shared pool with tenant tagging |
| Modify | `src/compiled/cache/mod.rs` | Partition compiled query cache by tenant |
| Modify | `src/grpc/service.rs` | Extract tenant from request metadata, pass to operations |
| Modify | `src/mcp/handler.rs` | Extract tenant from headers, pass to operations |
| Modify | `proto/mongocore/v1/types.proto` | Add `tenant_id` field to request messages (or use metadata) |
| Modify | `src/main.rs` | Initialize tenant registry on startup |
| Create | `tests/integration/tenant_test.rs` | Integration tests for multi-tenant isolation |

---

## Subsystem 1: Raw Wire Protocol Passthrough

### Task 1.1: Raw Command Validator

**Files:**
- Create: `src/operations/raw_validator.rs`
- Test: `tests/integration/raw_command_test.rs` (validator unit tests inline)

- [ ] **Step 1: Write failing test for command validation**

```rust
// src/operations/raw_validator.rs
#[cfg(test)]
mod tests {
    use super::*;
    use bson::doc;

    #[test]
    fn test_safe_command_allowed() {
        let cmd = doc! { "ping": 1 };
        assert!(validate_command(&cmd, &ValidationMode::BlockDangerous).is_ok());
    }

    #[test]
    fn test_drop_database_blocked() {
        let cmd = doc! { "dropDatabase": 1 };
        assert!(validate_command(&cmd, &ValidationMode::BlockDangerous).is_err());
    }

    #[test]
    fn test_shutdown_blocked() {
        let cmd = doc! { "shutdown": 1 };
        assert!(validate_command(&cmd, &ValidationMode::BlockDangerous).is_err());
    }

    #[test]
    fn test_allowall_mode_permits_everything() {
        let cmd = doc! { "dropDatabase": 1 };
        assert!(validate_command(&cmd, &ValidationMode::AllowAll).is_ok());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib raw_validator`
Expected: FAIL — module doesn't exist

- [ ] **Step 3: Implement the validator**

```rust
// src/operations/raw_validator.rs
use bson::Document;
use crate::error::MongoCoreError;

const BLOCKED_COMMANDS: &[&str] = &[
    "dropDatabase",
    "dropAllUsersFromDatabase",
    "dropAllRolesFromDatabase",
    "shutdown",
    "replSetReconfig",
    "replSetStepDown",
    "setFeatureCompatibilityVersion",
    "fsync",
    "cleanupOrphaned",
    "compact",
];

#[derive(Debug, Clone, PartialEq)]
pub enum ValidationMode {
    BlockDangerous,
    AllowAll,
}

pub fn validate_command(
    cmd: &Document,
    mode: &ValidationMode,
) -> Result<(), MongoCoreError> {
    if *mode == ValidationMode::AllowAll {
        return Ok(());
    }

    if let Some(first_key) = cmd.keys().next() {
        if BLOCKED_COMMANDS.contains(&first_key.as_str()) {
            return Err(MongoCoreError::ValidationError(format!(
                "Command '{}' is blocked in safe mode. Use --raw-allow-all to override.",
                first_key
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    // ... tests from step 1 ...
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib raw_validator`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/operations/raw_validator.rs
git commit -m "feat(v2): add raw command validator with blocked command list"
```

---

### Task 1.2: Raw Command Execution

**Files:**
- Create: `src/operations/raw.rs`
- Modify: `src/operations/mod.rs`

- [ ] **Step 1: Write failing test for raw command execution**

```rust
// src/operations/raw.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run_command_ping() {
        // This will be an integration test — for now, verify the function signature compiles
        let _fn_exists: fn(&Operations, &str, bson::Document) -> _ = Operations::run_command;
    }
}
```

- [ ] **Step 2: Implement raw command execution**

```rust
// src/operations/raw.rs
use bson::Document;
use crate::connection::pool::ConnectionPool;
use crate::error::MongoCoreError;
use crate::operations::raw_validator::{validate_command, ValidationMode};

pub struct RawCommandOptions {
    pub validation_mode: ValidationMode,
}

impl Default for RawCommandOptions {
    fn default() -> Self {
        Self {
            validation_mode: ValidationMode::BlockDangerous,
        }
    }
}

pub async fn run_command(
    pool: &ConnectionPool,
    database: &str,
    command: Document,
    options: &RawCommandOptions,
) -> Result<Document, MongoCoreError> {
    validate_command(&command, &options.validation_mode)?;

    let db = pool.database(database);
    let result = db
        .run_command(command)
        .await
        .map_err(|e| MongoCoreError::OperationError(format!("Raw command failed: {}", e)))?;

    Ok(result)
}
```

- [ ] **Step 3: Export from operations module**

Add to `src/operations/mod.rs`:
```rust
pub mod raw;
pub mod raw_validator;
```

- [ ] **Step 4: Run unit tests**

Run: `cargo test --lib`
Expected: PASS (compilation succeeds)

- [ ] **Step 5: Commit**

```bash
git add src/operations/raw.rs src/operations/raw_validator.rs src/operations/mod.rs
git commit -m "feat(v2): add raw command execution with validation"
```

---

### Task 1.3: RunCommand gRPC RPC

**Files:**
- Modify: `proto/mongocore/v1/mongocore.proto`
- Modify: `src/grpc/service.rs`

- [ ] **Step 1: Add RunCommand to proto definition**

Add to the `service MongoCore` block in `proto/mongocore/v1/mongocore.proto`:
```protobuf
  // Raw passthrough
  rpc RunCommand(RunCommandRequest) returns (RunCommandResponse);
```

Add message definitions:
```protobuf
// ==================== Raw Passthrough ====================

message RunCommandRequest {
  string database = 1;
  Document command = 2;  // Arbitrary MongoDB command as BSON
  bool allow_all = 3;    // If true, bypasses command blocklist
}

message RunCommandResponse {
  Document result = 1;   // Raw command result as BSON
}
```

- [ ] **Step 2: Regenerate proto stubs**

Run: `cargo build`
Expected: Build succeeds, new proto types available

- [ ] **Step 3: Implement RunCommand in gRPC service**

Add to `src/grpc/service.rs` in the `impl MongoCore for MongoCoreService` block:
```rust
    async fn run_command(
        &self,
        request: Request<proto::RunCommandRequest>,
    ) -> Result<Response<proto::RunCommandResponse>, Status> {
        let req = request.into_inner();
        let command_doc = req
            .command
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("Missing command document"))?;
        let command = proto_doc_to_bson(command_doc)?;

        let options = crate::operations::raw::RawCommandOptions {
            validation_mode: if req.allow_all {
                crate::operations::raw_validator::ValidationMode::AllowAll
            } else {
                crate::operations::raw_validator::ValidationMode::BlockDangerous
            },
        };

        let result = crate::operations::raw::run_command(
            &self.pool,
            &req.database,
            command,
            &options,
        )
        .await
        .map_err(to_status)?;

        Ok(Response::new(proto::RunCommandResponse {
            result: Some(bson_to_proto_doc(&result)?),
        }))
    }
```

- [ ] **Step 4: Build and verify compilation**

Run: `cargo build`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add proto/mongocore/v1/mongocore.proto src/grpc/service.rs
git commit -m "feat(v2): add RunCommand gRPC RPC for raw wire protocol passthrough"
```

---

### Task 1.4: RunCommand MCP Tool

**Files:**
- Modify: `src/mcp/tools.rs`
- Modify: `src/mcp/safety.rs`

- [ ] **Step 1: Add run_command tool definition**

Add to the tool list in `src/mcp/tools.rs`:
```rust
Tool {
    name: "run_command".to_string(),
    description: "Execute an arbitrary MongoDB command. Dangerous commands are blocked by default.".to_string(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "database": { "type": "string", "description": "Target database name" },
            "command": { "type": "object", "description": "MongoDB command document" },
            "allow_all": { "type": "boolean", "description": "Bypass command blocklist (requires explicit opt-in)", "default": false }
        },
        "required": ["database", "command"]
    }),
}
```

- [ ] **Step 2: Add safety check for run_command**

In `src/mcp/safety.rs`, add `run_command` to the write-operations list that gets blocked in read-only mode. Additionally, always block `allow_all: true` unless the server was started with `--raw-allow-all` flag.

- [ ] **Step 3: Implement the tool handler**

Add the handler case in the MCP tool dispatch:
```rust
"run_command" => {
    let database = params["database"].as_str()
        .ok_or_else(|| "Missing 'database' parameter")?;
    let command_value = &params["command"];
    let command: bson::Document = bson::to_document(
        &serde_json::from_value::<serde_json::Value>(command_value.clone())?
    )?;
    let allow_all = params.get("allow_all").and_then(|v| v.as_bool()).unwrap_or(false);

    let options = crate::operations::raw::RawCommandOptions {
        validation_mode: if allow_all {
            crate::operations::raw_validator::ValidationMode::AllowAll
        } else {
            crate::operations::raw_validator::ValidationMode::BlockDangerous
        },
    };

    let result = crate::operations::raw::run_command(pool, database, command, &options).await?;
    serde_json::to_value(&bson::to_document(&result)?)?
}
```

- [ ] **Step 4: Build and verify**

Run: `cargo build`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/mcp/tools.rs src/mcp/safety.rs
git commit -m "feat(v2): add run_command MCP tool with safety controls"
```

---

### Task 1.5: Raw Command Integration Tests

**Files:**
- Create: `tests/integration/raw_command_test.rs`
- Modify: `tests/integration.rs`

- [ ] **Step 1: Write integration tests**

```rust
// tests/integration/raw_command_test.rs
use crate::harness;
use bson::doc;

#[tokio::test]
async fn test_raw_ping() {
    let pool = harness::setup_pool().await;
    let cmd = doc! { "ping": 1 };
    let options = mongocore::operations::raw::RawCommandOptions::default();

    let result = mongocore::operations::raw::run_command(&pool, "admin", cmd, &options)
        .await
        .unwrap();

    assert_eq!(result.get_f64("ok").unwrap(), 1.0);
}

#[tokio::test]
async fn test_raw_server_status() {
    let pool = harness::setup_pool().await;
    let cmd = doc! { "serverStatus": 1 };
    let options = mongocore::operations::raw::RawCommandOptions::default();

    let result = mongocore::operations::raw::run_command(&pool, "admin", cmd, &options)
        .await
        .unwrap();

    assert!(result.contains_key("version"));
    assert!(result.contains_key("uptime"));
}

#[tokio::test]
async fn test_raw_blocked_command_rejected() {
    let pool = harness::setup_pool().await;
    let cmd = doc! { "dropDatabase": 1 };
    let options = mongocore::operations::raw::RawCommandOptions::default();

    let result = mongocore::operations::raw::run_command(&pool, "test", cmd, &options).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_raw_blocked_command_allowed_with_override() {
    let pool = harness::setup_pool().await;
    let cmd = doc! { "ping": 1 };  // Use ping as safe stand-in for AllowAll mode
    let options = mongocore::operations::raw::RawCommandOptions {
        validation_mode: mongocore::operations::raw_validator::ValidationMode::AllowAll,
    };

    let result = mongocore::operations::raw::run_command(&pool, "admin", cmd, &options)
        .await
        .unwrap();

    assert_eq!(result.get_f64("ok").unwrap(), 1.0);
}

#[tokio::test]
async fn test_raw_custom_aggregation_command() {
    let pool = harness::setup_pool().await;
    let cmd = doc! {
        "aggregate": "test_raw_coll",
        "pipeline": [{ "$limit": 1 }],
        "cursor": {}
    };
    let options = mongocore::operations::raw::RawCommandOptions::default();

    let result = mongocore::operations::raw::run_command(&pool, "test", cmd, &options)
        .await
        .unwrap();

    assert!(result.contains_key("cursor"));
}
```

- [ ] **Step 2: Add module to integration test root**

Add to `tests/integration.rs`:
```rust
mod integration {
    // ... existing modules ...
    mod raw_command_test;
}
```

- [ ] **Step 3: Run integration tests**

Run: `just docker-up && cargo test --test integration raw_command -- --nocapture`
Expected: All 5 tests PASS

- [ ] **Step 4: Commit**

```bash
git add tests/integration/raw_command_test.rs tests/integration.rs
git commit -m "test(v2): add raw command passthrough integration tests"
```

---

### Task 1.6: Update Client Libraries for RunCommand

**Files:**
- Modify: `clients/python/src/mongocore/client.py`
- Modify: `clients/typescript/src/client.ts`
- Modify: `clients/go/mongocore/client.go`
- Modify: `clients/java/src/main/java/com/mongocore/MongoClient.java`

- [ ] **Step 1: Add run_command to Python client**

```python
async def run_command(self, database: str, command: dict, allow_all: bool = False) -> dict:
    """Execute an arbitrary MongoDB command via raw passthrough."""
    request = mongocore_pb2.RunCommandRequest(
        database=database,
        command=mongocore_pb2.Document(data=bson.encode(command)),
        allow_all=allow_all,
    )
    response = await self._stub.RunCommand(request)
    return bson.decode(response.result.data) if response.result else {}
```

- [ ] **Step 2: Add run_command to TypeScript client**

```typescript
async runCommand(database: string, command: Record<string, unknown>, allowAll = false): Promise<Record<string, unknown>> {
  const request = {
    database,
    command: { data: BSON.serialize(command) },
    allowAll,
  };
  const response = await this.client.runCommand(request);
  return response.result ? BSON.deserialize(response.result.data) : {};
}
```

- [ ] **Step 3: Add run_command to Go client**

```go
func (c *Client) RunCommand(ctx context.Context, database string, command bson.D, allowAll bool) (bson.M, error) {
    cmdBytes, err := bson.Marshal(command)
    if err != nil {
        return nil, err
    }
    resp, err := c.stub.RunCommand(ctx, &proto.RunCommandRequest{
        Database: database,
        Command:  &proto.Document{Data: cmdBytes},
        AllowAll: allowAll,
    })
    if err != nil {
        return nil, err
    }
    var result bson.M
    if resp.Result != nil {
        err = bson.Unmarshal(resp.Result.Data, &result)
    }
    return result, err
}
```

- [ ] **Step 4: Add run_command to Java client**

```java
public Document runCommand(String database, Document command, boolean allowAll) {
    byte[] cmdBytes = toBson(command);
    RunCommandResponse response = stub.runCommand(RunCommandRequest.newBuilder()
        .setDatabase(database)
        .setCommand(MongoCore.Document.newBuilder().setData(ByteString.copyFrom(cmdBytes)).build())
        .setAllowAll(allowAll)
        .build());
    return response.hasResult() ? fromBson(response.getResult().getData().toByteArray()) : new Document();
}
```

- [ ] **Step 5: Regenerate gRPC stubs for all languages**

Run:
```bash
cd clients/python && python -m grpc_tools.protoc -I../../proto --python_out=src/mongocore/generated --grpc_python_out=src/mongocore/generated ../../proto/mongocore/v1/mongocore.proto ../../proto/mongocore/v1/types.proto
cd clients/typescript && npx grpc_tools_node_protoc --ts_out=src/generated --grpc_out=src/generated -I../../proto ../../proto/mongocore/v1/mongocore.proto ../../proto/mongocore/v1/types.proto
cd clients/go && protoc --go_out=./proto --go-grpc_out=./proto -I../../proto ../../proto/mongocore/v1/mongocore.proto ../../proto/mongocore/v1/types.proto
cd clients/java && protoc --java_out=src/main/java --grpc-java_out=src/main/java -I../../proto ../../proto/mongocore/v1/mongocore.proto ../../proto/mongocore/v1/types.proto
```

- [ ] **Step 6: Commit**

```bash
git add clients/
git commit -m "feat(v2): add run_command to all client libraries"
```

---

## Subsystem 2: Query Analytics

### Task 2.1: Analytics Event Types

**Files:**
- Create: `src/analytics/types.rs`
- Create: `src/analytics/mod.rs`

- [ ] **Step 1: Write failing test for event types**

```rust
// src/analytics/types.rs
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_event_creation() {
        let event = AnalyticsEvent::new(
            OperationKind::Find,
            "mydb".to_string(),
            "users".to_string(),
            Duration::from_millis(5),
            true,
        );
        assert_eq!(event.operation, OperationKind::Find);
        assert_eq!(event.database, "mydb");
        assert_eq!(event.collection, "users");
        assert!(event.success);
    }

    #[test]
    fn test_query_fingerprint() {
        let fp1 = QueryFingerprint::from_filter(&bson::doc! { "age": { "$gt": 25 } });
        let fp2 = QueryFingerprint::from_filter(&bson::doc! { "age": { "$gt": 50 } });
        // Same shape, different values → same fingerprint
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_different_shape_different_fingerprint() {
        let fp1 = QueryFingerprint::from_filter(&bson::doc! { "age": { "$gt": 25 } });
        let fp2 = QueryFingerprint::from_filter(&bson::doc! { "name": "Alice" });
        assert_ne!(fp1, fp2);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib analytics`
Expected: FAIL — module doesn't exist

- [ ] **Step 3: Implement event types**

```rust
// src/analytics/types.rs
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OperationKind {
    Find,
    FindOne,
    Insert,
    InsertMany,
    Update,
    UpdateMany,
    Delete,
    DeleteMany,
    FindAndModify,
    Aggregate,
    Search,
    Watch,
    RunCommand,
    BeginTransaction,
    CommitTransaction,
    AbortTransaction,
    CreateCollection,
    CreateIndex,
    ListDatabases,
    ListCollections,
}

#[derive(Debug, Clone)]
pub struct AnalyticsEvent {
    pub operation: OperationKind,
    pub database: String,
    pub collection: String,
    pub latency: Duration,
    pub success: bool,
    pub timestamp: Instant,
    pub fingerprint: Option<QueryFingerprint>,
    pub tenant_id: Option<String>,
    pub document_count: Option<u64>,
}

impl AnalyticsEvent {
    pub fn new(
        operation: OperationKind,
        database: String,
        collection: String,
        latency: Duration,
        success: bool,
    ) -> Self {
        Self {
            operation,
            database,
            collection,
            latency,
            success,
            timestamp: Instant::now(),
            fingerprint: None,
            tenant_id: None,
            document_count: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QueryFingerprint(String);

impl QueryFingerprint {
    pub fn from_filter(filter: &bson::Document) -> Self {
        Self(Self::extract_shape(filter))
    }

    fn extract_shape(doc: &bson::Document) -> String {
        let mut parts: Vec<String> = doc
            .keys()
            .map(|k| {
                match doc.get(k) {
                    Some(bson::Bson::Document(inner)) => {
                        format!("{}:{{{}}}", k, Self::extract_shape(inner))
                    }
                    _ => k.to_string(),
                }
            })
            .collect();
        parts.sort();
        parts.join(",")
    }
}

#[cfg(test)]
mod tests {
    // ... tests from step 1 ...
}
```

- [ ] **Step 4: Create module entry**

```rust
// src/analytics/mod.rs
pub mod types;
pub mod collector;
pub mod ring_buffer;
pub mod aggregator;
pub mod persistence;

pub use collector::AnalyticsCollector;
pub use types::{AnalyticsEvent, OperationKind, QueryFingerprint};
```

- [ ] **Step 5: Run tests**

Run: `cargo test --lib analytics::types`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/analytics/types.rs src/analytics/mod.rs
git commit -m "feat(v2): add analytics event types and query fingerprinting"
```

---

### Task 2.2: Ring Buffer

**Files:**
- Create: `src/analytics/ring_buffer.rs`

- [ ] **Step 1: Write failing tests**

```rust
// src/analytics/ring_buffer.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::types::*;
    use std::time::Duration;

    fn make_event(op: OperationKind) -> AnalyticsEvent {
        AnalyticsEvent::new(op, "db".into(), "coll".into(), Duration::from_millis(1), true)
    }

    #[test]
    fn test_push_and_len() {
        let buf = RingBuffer::new(100);
        buf.push(make_event(OperationKind::Find));
        buf.push(make_event(OperationKind::Insert));
        assert_eq!(buf.len(), 2);
    }

    #[test]
    fn test_overflow_evicts_oldest() {
        let buf = RingBuffer::new(2);
        buf.push(make_event(OperationKind::Find));
        buf.push(make_event(OperationKind::Insert));
        buf.push(make_event(OperationKind::Delete));
        assert_eq!(buf.len(), 2);
    }

    #[test]
    fn test_snapshot_returns_copy() {
        let buf = RingBuffer::new(10);
        buf.push(make_event(OperationKind::Find));
        let snapshot = buf.snapshot();
        assert_eq!(snapshot.len(), 1);
    }
}
```

- [ ] **Step 2: Implement ring buffer**

```rust
// src/analytics/ring_buffer.rs
use std::sync::Mutex;
use std::collections::VecDeque;
use crate::analytics::types::AnalyticsEvent;

pub struct RingBuffer {
    capacity: usize,
    buffer: Mutex<VecDeque<AnalyticsEvent>>,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            buffer: Mutex::new(VecDeque::with_capacity(capacity)),
        }
    }

    pub fn push(&self, event: AnalyticsEvent) {
        let mut buf = self.buffer.lock().unwrap();
        if buf.len() >= self.capacity {
            buf.pop_front();
        }
        buf.push_back(event);
    }

    pub fn len(&self) -> usize {
        self.buffer.lock().unwrap().len()
    }

    pub fn snapshot(&self) -> Vec<AnalyticsEvent> {
        self.buffer.lock().unwrap().iter().cloned().collect()
    }

    pub fn drain(&self) -> Vec<AnalyticsEvent> {
        self.buffer.lock().unwrap().drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    // ... tests from step 1 ...
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib analytics::ring_buffer`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/analytics/ring_buffer.rs
git commit -m "feat(v2): add thread-safe ring buffer for analytics events"
```

---

### Task 2.3: Analytics Collector

**Files:**
- Create: `src/analytics/collector.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::types::*;
    use std::time::Duration;

    #[test]
    fn test_collector_records_event() {
        let collector = AnalyticsCollector::new(1000);
        collector.record(AnalyticsEvent::new(
            OperationKind::Find, "db".into(), "coll".into(),
            Duration::from_millis(5), true,
        ));
        assert_eq!(collector.event_count(), 1);
    }

    #[test]
    fn test_collector_total_ops() {
        let collector = AnalyticsCollector::new(1000);
        for _ in 0..10 {
            collector.record(AnalyticsEvent::new(
                OperationKind::Find, "db".into(), "coll".into(),
                Duration::from_millis(5), true,
            ));
        }
        assert_eq!(collector.total_operations(), 10);
    }
}
```

- [ ] **Step 2: Implement collector**

```rust
// src/analytics/collector.rs
use std::sync::atomic::{AtomicU64, Ordering};
use crate::analytics::ring_buffer::RingBuffer;
use crate::analytics::types::AnalyticsEvent;

pub struct AnalyticsCollector {
    buffer: RingBuffer,
    total_ops: AtomicU64,
    total_errors: AtomicU64,
}

impl AnalyticsCollector {
    pub fn new(buffer_capacity: usize) -> Self {
        Self {
            buffer: RingBuffer::new(buffer_capacity),
            total_ops: AtomicU64::new(0),
            total_errors: AtomicU64::new(0),
        }
    }

    pub fn record(&self, event: AnalyticsEvent) {
        self.total_ops.fetch_add(1, Ordering::Relaxed);
        if !event.success {
            self.total_errors.fetch_add(1, Ordering::Relaxed);
        }
        self.buffer.push(event);
    }

    pub fn event_count(&self) -> usize {
        self.buffer.len()
    }

    pub fn total_operations(&self) -> u64 {
        self.total_ops.load(Ordering::Relaxed)
    }

    pub fn total_errors(&self) -> u64 {
        self.total_errors.load(Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> Vec<AnalyticsEvent> {
        self.buffer.snapshot()
    }
}

#[cfg(test)]
mod tests {
    // ... tests from step 1 ...
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib analytics::collector`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/analytics/collector.rs
git commit -m "feat(v2): add analytics collector with atomic counters"
```

---

### Task 2.4: Analytics Aggregator

**Files:**
- Create: `src/analytics/aggregator.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::types::*;
    use std::time::Duration;

    fn events() -> Vec<AnalyticsEvent> {
        vec![
            AnalyticsEvent::new(OperationKind::Find, "db".into(), "users".into(), Duration::from_millis(5), true),
            AnalyticsEvent::new(OperationKind::Find, "db".into(), "users".into(), Duration::from_millis(10), true),
            AnalyticsEvent::new(OperationKind::Insert, "db".into(), "users".into(), Duration::from_millis(3), true),
            AnalyticsEvent::new(OperationKind::Find, "db".into(), "orders".into(), Duration::from_millis(50), false),
        ]
    }

    #[test]
    fn test_top_operations() {
        let summary = aggregate(&events());
        let top = &summary.top_operations;
        assert_eq!(top[0].0, OperationKind::Find);
        assert_eq!(top[0].1, 3);
    }

    #[test]
    fn test_latency_percentiles() {
        let summary = aggregate(&events());
        assert!(summary.p50_latency_ms > 0.0);
        assert!(summary.p99_latency_ms >= summary.p50_latency_ms);
    }

    #[test]
    fn test_error_rate() {
        let summary = aggregate(&events());
        assert!((summary.error_rate - 0.25).abs() < 0.01);
    }
}
```

- [ ] **Step 2: Implement aggregator**

```rust
// src/analytics/aggregator.rs
use std::collections::HashMap;
use crate::analytics::types::{AnalyticsEvent, OperationKind};

#[derive(Debug, Clone)]
pub struct AnalyticsSummary {
    pub total_operations: usize,
    pub total_errors: usize,
    pub error_rate: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub top_operations: Vec<(OperationKind, usize)>,
    pub top_collections: Vec<(String, usize)>,
}

pub fn aggregate(events: &[AnalyticsEvent]) -> AnalyticsSummary {
    if events.is_empty() {
        return AnalyticsSummary {
            total_operations: 0,
            total_errors: 0,
            error_rate: 0.0,
            p50_latency_ms: 0.0,
            p95_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            top_operations: vec![],
            top_collections: vec![],
        };
    }

    let total = events.len();
    let errors = events.iter().filter(|e| !e.success).count();

    let mut latencies: Vec<f64> = events.iter().map(|e| e.latency.as_secs_f64() * 1000.0).collect();
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let p50 = percentile(&latencies, 50.0);
    let p95 = percentile(&latencies, 95.0);
    let p99 = percentile(&latencies, 99.0);

    let mut op_counts: HashMap<OperationKind, usize> = HashMap::new();
    let mut coll_counts: HashMap<String, usize> = HashMap::new();

    for event in events {
        *op_counts.entry(event.operation.clone()).or_default() += 1;
        let key = format!("{}.{}", event.database, event.collection);
        *coll_counts.entry(key).or_default() += 1;
    }

    let mut top_ops: Vec<_> = op_counts.into_iter().collect();
    top_ops.sort_by(|a, b| b.1.cmp(&a.1));
    top_ops.truncate(10);

    let mut top_colls: Vec<_> = coll_counts.into_iter().collect();
    top_colls.sort_by(|a, b| b.1.cmp(&a.1));
    top_colls.truncate(10);

    AnalyticsSummary {
        total_operations: total,
        total_errors: errors,
        error_rate: errors as f64 / total as f64,
        p50_latency_ms: p50,
        p95_latency_ms: p95,
        p99_latency_ms: p99,
        top_operations: top_ops,
        top_collections: top_colls,
    }
}

fn percentile(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((pct / 100.0) * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

#[cfg(test)]
mod tests {
    // ... tests from step 1 ...
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib analytics::aggregator`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/analytics/aggregator.rs
git commit -m "feat(v2): add analytics aggregator with percentiles and top-N"
```

---

### Task 2.5: Analytics Persistence

**Files:**
- Create: `src/analytics/persistence.rs`

- [ ] **Step 1: Write the persistence module**

```rust
// src/analytics/persistence.rs
use crate::analytics::aggregator::AnalyticsSummary;
use crate::analytics::collector::AnalyticsCollector;
use crate::connection::pool::ConnectionPool;
use crate::error::MongoCoreError;
use std::sync::Arc;
use std::time::Duration;

pub struct AnalyticsPersistence {
    pool: ConnectionPool,
    collector: Arc<AnalyticsCollector>,
    flush_interval: Duration,
    database: String,
    collection: String,
}

impl AnalyticsPersistence {
    pub fn new(
        pool: ConnectionPool,
        collector: Arc<AnalyticsCollector>,
        flush_interval: Duration,
    ) -> Self {
        Self {
            pool,
            collector,
            flush_interval,
            database: "__mongocore".to_string(),
            collection: "analytics".to_string(),
        }
    }

    pub async fn flush_snapshot(&self) -> Result<(), MongoCoreError> {
        let events = self.collector.snapshot();
        if events.is_empty() {
            return Ok(());
        }

        let summary = crate::analytics::aggregator::aggregate(&events);

        let doc = bson::doc! {
            "timestamp": bson::DateTime::now(),
            "total_operations": summary.total_operations as i64,
            "total_errors": summary.total_errors as i64,
            "error_rate": summary.error_rate,
            "p50_latency_ms": summary.p50_latency_ms,
            "p95_latency_ms": summary.p95_latency_ms,
            "p99_latency_ms": summary.p99_latency_ms,
        };

        let coll = self.pool.collection(&self.database, &self.collection);
        coll.insert_one(doc)
            .await
            .map_err(|e| MongoCoreError::OperationError(format!("Analytics flush failed: {}", e)))?;

        Ok(())
    }

    pub fn start_background_flush(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        let interval = self.flush_interval;
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(interval);
            loop {
                tick.tick().await;
                if let Err(e) = self.flush_snapshot().await {
                    tracing::warn!("Analytics flush failed: {}", e);
                }
            }
        })
    }
}
```

- [ ] **Step 2: Build to verify compilation**

Run: `cargo build`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/analytics/persistence.rs
git commit -m "feat(v2): add background analytics persistence to MongoDB"
```

---

### Task 2.6: Instrument gRPC Service with Analytics

**Files:**
- Modify: `src/grpc/service.rs`
- Modify: `src/main.rs`
- Modify: `src/config.rs`

- [ ] **Step 1: Add analytics config fields**

Add to `FileConfig` and `Config` in `src/config.rs`:
```rust
// In FileConfig:
pub analytics_enabled: Option<bool>,
pub analytics_buffer_size: Option<usize>,
pub analytics_flush_interval_secs: Option<u64>,

// In Config:
pub analytics_enabled: bool,
pub analytics_buffer_size: usize,
pub analytics_flush_interval_secs: u64,
```

Add resolution logic with defaults:
```rust
let analytics_enabled = cli.analytics_enabled
    .or(file_config.analytics_enabled)
    .unwrap_or(true);
let analytics_buffer_size = cli.analytics_buffer_size
    .or(file_config.analytics_buffer_size)
    .unwrap_or(10_000);
let analytics_flush_interval_secs = cli.analytics_flush_interval_secs
    .or(file_config.analytics_flush_interval_secs)
    .unwrap_or(300);
```

- [ ] **Step 2: Add collector to MongoCoreService**

Modify `MongoCoreService` struct:
```rust
pub struct MongoCoreService {
    operations: Operations,
    pool: ConnectionPool,
    transactions: DashMap<String, Transaction>,
    search_engine: SearchEngine,
    analytics: Option<Arc<AnalyticsCollector>>,
}
```

Add helper method:
```rust
fn record_analytics(&self, op: OperationKind, db: &str, coll: &str, latency: Duration, success: bool) {
    if let Some(ref analytics) = self.analytics {
        analytics.record(AnalyticsEvent::new(op, db.to_string(), coll.to_string(), latency, success));
    }
}
```

- [ ] **Step 3: Instrument the Find RPC as example pattern**

Wrap the existing `find` implementation with timing:
```rust
async fn find(&self, request: Request<proto::FindRequest>) -> Result<Response<proto::FindResponse>, Status> {
    let start = std::time::Instant::now();
    let req = request.into_inner();
    let filter = proto_filter_to_bson(&req.filter)?;
    let options = convert_find_options(&req.options)?;

    let result = if let Some(ref txn_id) = req.transaction_id {
        // ... existing transaction logic ...
    } else {
        self.operations.find(&req.database, &req.collection, filter, options).await
    };

    let latency = start.elapsed();
    let success = result.is_ok();
    self.record_analytics(OperationKind::Find, &req.database, &req.collection, latency, success);

    let docs = result.map_err(to_status)?;
    // ... rest of response building ...
}
```

Apply the same pattern to all other RPCs (Insert, Update, Delete, Aggregate, Search, etc.).

- [ ] **Step 4: Initialize collector in main.rs**

```rust
let analytics = if config.analytics_enabled {
    Some(Arc::new(AnalyticsCollector::new(config.analytics_buffer_size)))
} else {
    None
};
```

Pass to `MongoCoreService::new()`.

- [ ] **Step 5: Build and run unit tests**

Run: `cargo build && cargo test --lib`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/grpc/service.rs src/main.rs src/config.rs
git commit -m "feat(v2): instrument gRPC service with analytics collection"
```

---

### Task 2.7: GetAnalytics RPC and MCP Tool

**Files:**
- Modify: `proto/mongocore/v1/mongocore.proto`
- Modify: `src/grpc/service.rs`
- Modify: `src/mcp/tools.rs`

- [ ] **Step 1: Add GetAnalytics to proto**

```protobuf
  // Analytics
  rpc GetAnalytics(GetAnalyticsRequest) returns (GetAnalyticsResponse);
```

```protobuf
// ==================== Analytics ====================

message GetAnalyticsRequest {
  int64 window_seconds = 1;  // How far back to look (0 = all available)
}

message GetAnalyticsResponse {
  int64 total_operations = 1;
  int64 total_errors = 2;
  double error_rate = 3;
  double p50_latency_ms = 4;
  double p95_latency_ms = 5;
  double p99_latency_ms = 6;
  repeated OperationCount top_operations = 7;
  repeated CollectionCount top_collections = 8;
}

message OperationCount {
  string operation = 1;
  int64 count = 2;
}

message CollectionCount {
  string collection = 1;
  int64 count = 2;
}
```

- [ ] **Step 2: Implement GetAnalytics handler**

```rust
async fn get_analytics(
    &self,
    _request: Request<proto::GetAnalyticsRequest>,
) -> Result<Response<proto::GetAnalyticsResponse>, Status> {
    let analytics = self.analytics.as_ref()
        .ok_or_else(|| Status::unavailable("Analytics not enabled"))?;

    let events = analytics.snapshot();
    let summary = crate::analytics::aggregator::aggregate(&events);

    let top_operations = summary.top_operations.iter().map(|(op, count)| {
        proto::OperationCount {
            operation: format!("{:?}", op),
            count: *count as i64,
        }
    }).collect();

    let top_collections = summary.top_collections.iter().map(|(coll, count)| {
        proto::CollectionCount {
            collection: coll.clone(),
            count: *count as i64,
        }
    }).collect();

    Ok(Response::new(proto::GetAnalyticsResponse {
        total_operations: summary.total_operations as i64,
        total_errors: summary.total_errors as i64,
        error_rate: summary.error_rate,
        p50_latency_ms: summary.p50_latency_ms,
        p95_latency_ms: summary.p95_latency_ms,
        p99_latency_ms: summary.p99_latency_ms,
        top_operations,
        top_collections,
    }))
}
```

- [ ] **Step 3: Add get_analytics MCP tool**

```rust
Tool {
    name: "get_analytics".to_string(),
    description: "Get query analytics summary: top operations, latency percentiles, error rates.".to_string(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "window_seconds": { "type": "integer", "description": "Time window to analyze (0 = all)", "default": 0 }
        }
    }),
}
```

- [ ] **Step 4: Build**

Run: `cargo build`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add proto/mongocore/v1/mongocore.proto src/grpc/service.rs src/mcp/tools.rs
git commit -m "feat(v2): add GetAnalytics RPC and MCP tool"
```

---

### Task 2.8: Analytics Integration Tests

**Files:**
- Create: `tests/integration/analytics_test.rs`
- Modify: `tests/integration.rs`

- [ ] **Step 1: Write integration tests**

```rust
// tests/integration/analytics_test.rs
use crate::harness;
use std::sync::Arc;

#[tokio::test]
async fn test_analytics_records_operations() {
    let (pool, service) = harness::setup_service_with_analytics().await;

    // Perform some operations
    service.operations.insert("test", "analytics_coll", bson::doc! { "x": 1 }).await.unwrap();
    service.operations.find("test", "analytics_coll", bson::doc! {}, None).await.unwrap();
    service.operations.find("test", "analytics_coll", bson::doc! {}, None).await.unwrap();

    // Wait briefly for async recording
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let analytics = service.analytics.as_ref().unwrap();
    assert!(analytics.total_operations() >= 3);
}

#[tokio::test]
async fn test_analytics_summary_via_grpc() {
    let (client, _service) = harness::setup_grpc_client_with_analytics().await;

    // Perform operations via gRPC
    // ...

    let response = client.get_analytics(proto::GetAnalyticsRequest { window_seconds: 0 }).await.unwrap();
    let analytics = response.into_inner();
    assert!(analytics.total_operations > 0);
    assert!(analytics.p50_latency_ms > 0.0);
}
```

- [ ] **Step 2: Add to integration test module**

Add `mod analytics_test;` to `tests/integration.rs`.

- [ ] **Step 3: Run tests**

Run: `just docker-up && cargo test --test integration analytics -- --nocapture`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add tests/integration/analytics_test.rs tests/integration.rs
git commit -m "test(v2): add analytics integration tests"
```

---

## Subsystem 3: Multi-Tenant Support

### Task 3.1: Tenant Context Extraction

**Files:**
- Create: `src/tenant/mod.rs`
- Create: `src/tenant/context.rs`

- [ ] **Step 1: Write failing tests**

```rust
// src/tenant/context.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tenant_from_metadata() {
        let mut map = tonic::metadata::MetadataMap::new();
        map.insert("x-tenant-id", "tenant-abc".parse().unwrap());
        let tenant = TenantContext::from_grpc_metadata(&map);
        assert_eq!(tenant.tenant_id(), Some("tenant-abc"));
    }

    #[test]
    fn test_no_tenant_returns_default() {
        let map = tonic::metadata::MetadataMap::new();
        let tenant = TenantContext::from_grpc_metadata(&map);
        assert_eq!(tenant.tenant_id(), None);
    }

    #[test]
    fn test_extract_tenant_from_headers() {
        let mut headers = http::HeaderMap::new();
        headers.insert("x-tenant-id", "tenant-xyz".parse().unwrap());
        let tenant = TenantContext::from_http_headers(&headers);
        assert_eq!(tenant.tenant_id(), Some("tenant-xyz"));
    }
}
```

- [ ] **Step 2: Implement tenant context**

```rust
// src/tenant/context.rs
use tonic::metadata::MetadataMap;
use http::HeaderMap;

const TENANT_HEADER: &str = "x-tenant-id";

#[derive(Debug, Clone)]
pub struct TenantContext {
    tenant_id: Option<String>,
}

impl TenantContext {
    pub fn new(tenant_id: Option<String>) -> Self {
        Self { tenant_id }
    }

    pub fn default_tenant() -> Self {
        Self { tenant_id: None }
    }

    pub fn tenant_id(&self) -> Option<&str> {
        self.tenant_id.as_deref()
    }

    pub fn from_grpc_metadata(metadata: &MetadataMap) -> Self {
        let tenant_id = metadata
            .get(TENANT_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        Self { tenant_id }
    }

    pub fn from_http_headers(headers: &HeaderMap) -> Self {
        let tenant_id = headers
            .get(TENANT_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        Self { tenant_id }
    }
}

#[cfg(test)]
mod tests {
    // ... tests from step 1 ...
}
```

```rust
// src/tenant/mod.rs
pub mod context;
pub mod registry;
pub mod isolation;
pub mod quota;

pub use context::TenantContext;
pub use registry::TenantRegistry;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib tenant::context`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/tenant/mod.rs src/tenant/context.rs
git commit -m "feat(v2): add tenant context extraction from gRPC/HTTP headers"
```

---

### Task 3.2: Tenant Registry

**Files:**
- Create: `src/tenant/registry.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_tenant() {
        let registry = TenantRegistry::new();
        let config = TenantConfig {
            tenant_id: "acme".to_string(),
            max_connections: 10,
            max_cache_entries: 1000,
            rate_limit_ops_per_sec: 100,
            connection_uri_override: None,
        };
        registry.register(config);
        assert!(registry.get("acme").is_some());
    }

    #[test]
    fn test_unknown_tenant_returns_none() {
        let registry = TenantRegistry::new();
        assert!(registry.get("unknown").is_none());
    }

    #[test]
    fn test_remove_tenant() {
        let registry = TenantRegistry::new();
        registry.register(TenantConfig {
            tenant_id: "acme".to_string(),
            max_connections: 10,
            max_cache_entries: 1000,
            rate_limit_ops_per_sec: 100,
            connection_uri_override: None,
        });
        registry.remove("acme");
        assert!(registry.get("acme").is_none());
    }
}
```

- [ ] **Step 2: Implement registry**

```rust
// src/tenant/registry.rs
use dashmap::DashMap;

#[derive(Debug, Clone)]
pub struct TenantConfig {
    pub tenant_id: String,
    pub max_connections: usize,
    pub max_cache_entries: usize,
    pub rate_limit_ops_per_sec: u64,
    pub connection_uri_override: Option<String>,
}

pub struct TenantRegistry {
    tenants: DashMap<String, TenantConfig>,
}

impl TenantRegistry {
    pub fn new() -> Self {
        Self {
            tenants: DashMap::new(),
        }
    }

    pub fn register(&self, config: TenantConfig) {
        self.tenants.insert(config.tenant_id.clone(), config);
    }

    pub fn get(&self, tenant_id: &str) -> Option<TenantConfig> {
        self.tenants.get(tenant_id).map(|r| r.value().clone())
    }

    pub fn remove(&self, tenant_id: &str) {
        self.tenants.remove(tenant_id);
    }

    pub fn list(&self) -> Vec<String> {
        self.tenants.iter().map(|r| r.key().clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    // ... tests from step 1 ...
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib tenant::registry`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/tenant/registry.rs
git commit -m "feat(v2): add tenant registry with DashMap storage"
```

---

### Task 3.3: Cache Isolation

**Files:**
- Create: `src/tenant/isolation.rs`
- Modify: `src/compiled/cache/mod.rs`

- [ ] **Step 1: Write failing tests**

```rust
// src/tenant/isolation.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tenant_cache_key_includes_tenant() {
        let key1 = TenantCacheKey::new(Some("tenant-a"), "hash123");
        let key2 = TenantCacheKey::new(Some("tenant-b"), "hash123");
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_no_tenant_uses_default_partition() {
        let key1 = TenantCacheKey::new(None, "hash123");
        let key2 = TenantCacheKey::new(None, "hash123");
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_partitioned_cache_isolates_tenants() {
        let cache = PartitionedCache::new(100);
        cache.insert("tenant-a", "key1", "value-a".to_string());
        cache.insert("tenant-b", "key1", "value-b".to_string());

        assert_eq!(cache.get("tenant-a", "key1"), Some("value-a".to_string()));
        assert_eq!(cache.get("tenant-b", "key1"), Some("value-b".to_string()));
    }
}
```

- [ ] **Step 2: Implement cache isolation**

```rust
// src/tenant/isolation.rs
use dashmap::DashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantCacheKey {
    partition: String,
    key: String,
}

impl TenantCacheKey {
    pub fn new(tenant_id: Option<&str>, key: &str) -> Self {
        Self {
            partition: tenant_id.unwrap_or("__default__").to_string(),
            key: key.to_string(),
        }
    }
}

pub struct PartitionedCache {
    max_per_tenant: usize,
    entries: DashMap<TenantCacheKey, String>,
    counts: DashMap<String, usize>,
}

impl PartitionedCache {
    pub fn new(max_per_tenant: usize) -> Self {
        Self {
            max_per_tenant,
            entries: DashMap::new(),
            counts: DashMap::new(),
        }
    }

    pub fn insert(&self, tenant_id: &str, key: &str, value: String) -> bool {
        let count = self.counts.entry(tenant_id.to_string()).or_insert(0);
        if *count >= self.max_per_tenant {
            return false;
        }
        let cache_key = TenantCacheKey::new(Some(tenant_id), key);
        if self.entries.insert(cache_key, value).is_none() {
            *count += 1;
        }
        true
    }

    pub fn get(&self, tenant_id: &str, key: &str) -> Option<String> {
        let cache_key = TenantCacheKey::new(Some(tenant_id), key);
        self.entries.get(&cache_key).map(|r| r.value().clone())
    }

    pub fn remove_tenant(&self, tenant_id: &str) {
        self.entries.retain(|k, _| k.partition != tenant_id);
        self.counts.remove(tenant_id);
    }
}

#[cfg(test)]
mod tests {
    // ... tests from step 1 ...
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib tenant::isolation`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/tenant/isolation.rs
git commit -m "feat(v2): add partitioned cache for tenant isolation"
```

---

### Task 3.4: Tenant Rate Limiting

**Files:**
- Create: `src/tenant/quota.rs`

- [ ] **Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limiter_allows_within_limit() {
        let limiter = RateLimiter::new(10); // 10 ops/sec
        for _ in 0..10 {
            assert!(limiter.try_acquire());
        }
    }

    #[test]
    fn test_rate_limiter_blocks_over_limit() {
        let limiter = RateLimiter::new(2);
        assert!(limiter.try_acquire());
        assert!(limiter.try_acquire());
        assert!(!limiter.try_acquire());
    }

    #[test]
    fn test_tenant_quota_manager() {
        let mgr = QuotaManager::new();
        mgr.set_limit("tenant-a", 5);
        assert!(mgr.try_acquire("tenant-a"));
        assert!(mgr.try_acquire("unknown")); // unknown tenants have no limit
    }
}
```

- [ ] **Step 2: Implement rate limiting**

```rust
// src/tenant/quota.rs
use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

pub struct RateLimiter {
    max_per_second: u64,
    count: AtomicU64,
    window_start: std::sync::Mutex<Instant>,
}

impl RateLimiter {
    pub fn new(max_per_second: u64) -> Self {
        Self {
            max_per_second,
            count: AtomicU64::new(0),
            window_start: std::sync::Mutex::new(Instant::now()),
        }
    }

    pub fn try_acquire(&self) -> bool {
        let mut start = self.window_start.lock().unwrap();
        if start.elapsed().as_secs() >= 1 {
            *start = Instant::now();
            self.count.store(0, Ordering::Relaxed);
        }
        drop(start);

        let current = self.count.fetch_add(1, Ordering::Relaxed);
        if current >= self.max_per_second {
            self.count.fetch_sub(1, Ordering::Relaxed);
            return false;
        }
        true
    }
}

pub struct QuotaManager {
    limiters: DashMap<String, RateLimiter>,
}

impl QuotaManager {
    pub fn new() -> Self {
        Self {
            limiters: DashMap::new(),
        }
    }

    pub fn set_limit(&self, tenant_id: &str, max_per_second: u64) {
        self.limiters.insert(tenant_id.to_string(), RateLimiter::new(max_per_second));
    }

    pub fn try_acquire(&self, tenant_id: &str) -> bool {
        match self.limiters.get(tenant_id) {
            Some(limiter) => limiter.try_acquire(),
            None => true, // No limit configured
        }
    }

    pub fn remove(&self, tenant_id: &str) {
        self.limiters.remove(tenant_id);
    }
}

#[cfg(test)]
mod tests {
    // ... tests from step 1 ...
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib tenant::quota`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/tenant/quota.rs
git commit -m "feat(v2): add per-tenant rate limiting"
```

---

### Task 3.5: Multi-Tenant Configuration

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Add multi-tenant config structure**

```rust
#[derive(Debug, Deserialize, Clone, Default)]
pub struct TenantFileConfig {
    pub tenant_id: String,
    pub max_connections: Option<usize>,
    pub max_cache_entries: Option<usize>,
    pub rate_limit_ops_per_sec: Option<u64>,
    pub connection_uri: Option<String>,
}

// Add to FileConfig:
pub multi_tenant_enabled: Option<bool>,
pub tenants: Option<Vec<TenantFileConfig>>,

// Add to Config:
pub multi_tenant_enabled: bool,
pub tenants: Vec<TenantFileConfig>,
```

- [ ] **Step 2: Add test for tenant config parsing**

```rust
#[test]
fn test_tenant_config_parsing() {
    let toml_content = r#"
connection_uri = "mongodb://localhost:27017"
multi_tenant_enabled = true

[[tenants]]
tenant_id = "acme"
max_connections = 20
rate_limit_ops_per_sec = 500

[[tenants]]
tenant_id = "beta"
max_connections = 5
connection_uri = "mongodb://other:27017"
"#;

    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(toml_content.as_bytes()).unwrap();

    let cli = CliArgs {
        config: Some(tmp.path().to_path_buf()),
        // ... all None fields ...
    };

    let config = Config::load(&cli).unwrap();
    assert!(config.multi_tenant_enabled);
    assert_eq!(config.tenants.len(), 2);
    assert_eq!(config.tenants[0].tenant_id, "acme");
    assert_eq!(config.tenants[0].rate_limit_ops_per_sec, Some(500));
}
```

- [ ] **Step 3: Implement config loading for tenants**

Add to `Config::load()`:
```rust
let multi_tenant_enabled = file_config.multi_tenant_enabled.unwrap_or(false);
let tenants = file_config.tenants.unwrap_or_default();
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib config`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat(v2): add multi-tenant configuration support"
```

---

### Task 3.6: Wire Tenant Context into gRPC Service

**Files:**
- Modify: `src/grpc/service.rs`

- [ ] **Step 1: Extract tenant from gRPC metadata**

Add a helper to `MongoCoreService`:
```rust
fn extract_tenant(request_metadata: &tonic::metadata::MetadataMap) -> TenantContext {
    TenantContext::from_grpc_metadata(request_metadata)
}
```

- [ ] **Step 2: Add tenant registry and quota to service**

```rust
pub struct MongoCoreService {
    operations: Operations,
    pool: ConnectionPool,
    transactions: DashMap<String, Transaction>,
    search_engine: SearchEngine,
    analytics: Option<Arc<AnalyticsCollector>>,
    tenant_registry: Option<Arc<TenantRegistry>>,
    quota_manager: Option<Arc<QuotaManager>>,
}
```

- [ ] **Step 3: Add quota check to RPC handlers**

Add before each operation:
```rust
if let Some(ref quota) = self.quota_manager {
    let tenant = Self::extract_tenant(request.metadata());
    if let Some(tid) = tenant.tenant_id() {
        if !quota.try_acquire(tid) {
            return Err(Status::resource_exhausted(
                format!("Rate limit exceeded for tenant '{}'", tid)
            ));
        }
    }
}
```

- [ ] **Step 4: Build and run tests**

Run: `cargo build && cargo test --lib`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/grpc/service.rs
git commit -m "feat(v2): wire tenant context and quota enforcement into gRPC"
```

---

### Task 3.7: Multi-Tenant Integration Tests

**Files:**
- Create: `tests/integration/tenant_test.rs`
- Modify: `tests/integration.rs`

- [ ] **Step 1: Write integration tests**

```rust
// tests/integration/tenant_test.rs
use crate::harness;
use tonic::metadata::MetadataValue;

#[tokio::test]
async fn test_tenant_operations_isolated() {
    let (mut client_a, mut client_b) = harness::setup_multi_tenant_clients().await;

    // Client A inserts
    let mut req = tonic::Request::new(proto::InsertRequest {
        database: "tenant_test".into(),
        collection: "data".into(),
        document: Some(harness::make_doc(bson::doc! { "tenant": "a", "value": 1 })),
        transaction_id: None,
    });
    req.metadata_mut().insert("x-tenant-id", MetadataValue::from_static("tenant-a"));
    client_a.insert(req).await.unwrap();

    // Client B inserts
    let mut req = tonic::Request::new(proto::InsertRequest {
        database: "tenant_test".into(),
        collection: "data".into(),
        document: Some(harness::make_doc(bson::doc! { "tenant": "b", "value": 2 })),
        transaction_id: None,
    });
    req.metadata_mut().insert("x-tenant-id", MetadataValue::from_static("tenant-b"));
    client_b.insert(req).await.unwrap();

    // Both succeed — basic connectivity
}

#[tokio::test]
async fn test_rate_limit_enforcement() {
    let mut client = harness::setup_rate_limited_client("limited-tenant", 2).await;

    // First two requests should succeed
    for _ in 0..2 {
        let mut req = tonic::Request::new(proto::ListDatabasesRequest {});
        req.metadata_mut().insert("x-tenant-id", MetadataValue::from_static("limited-tenant"));
        assert!(client.list_databases(req).await.is_ok());
    }

    // Third should be rate-limited
    let mut req = tonic::Request::new(proto::ListDatabasesRequest {});
    req.metadata_mut().insert("x-tenant-id", MetadataValue::from_static("limited-tenant"));
    let result = client.list_databases(req).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().code(), tonic::Code::ResourceExhausted);
}

#[tokio::test]
async fn test_no_tenant_header_uses_default() {
    let mut client = harness::setup_grpc_client().await;

    // No tenant header — should work with default context
    let req = tonic::Request::new(proto::ListDatabasesRequest {});
    let result = client.list_databases(req).await;
    assert!(result.is_ok());
}
```

- [ ] **Step 2: Add to integration module**

Add `mod tenant_test;` to `tests/integration.rs`.

- [ ] **Step 3: Run tests**

Run: `just docker-up && cargo test --test integration tenant -- --nocapture`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add tests/integration/tenant_test.rs tests/integration.rs
git commit -m "test(v2): add multi-tenant integration tests"
```

---

### Task 3.8: Update Documentation

**Files:**
- Create: `docs/raw-passthrough.md`
- Create: `docs/analytics.md`
- Create: `docs/multi-tenant.md`
- Modify: `docs/README.md`
- Modify: `README.md`

- [ ] **Step 1: Write raw passthrough docs**

```markdown
# Raw Wire Protocol Passthrough

MongoCore v2 adds a raw command escape hatch for power users who need to execute
arbitrary MongoDB commands not covered by the standard API.

## Usage

### Python
```python
result = await client.run_command("admin", {"ping": 1})
```

### TypeScript
```typescript
const result = await client.runCommand('admin', { ping: 1 });
```

### Safety

By default, dangerous commands (dropDatabase, shutdown, etc.) are blocked.
To allow all commands, pass `allow_all=True` — this requires the sidecar to
be started with `--raw-allow-all`.

### Blocked Commands

- `dropDatabase`
- `shutdown`
- `replSetReconfig`
- `replSetStepDown`
- `setFeatureCompatibilityVersion`
- `fsync`
- `compact`
```

- [ ] **Step 2: Write analytics docs**

```markdown
# Query Analytics

MongoCore v2 tracks operation metrics automatically and exposes them
via both gRPC (`GetAnalytics`) and MCP (`get_analytics` tool).

## Configuration

```toml
analytics_enabled = true          # default: true
analytics_buffer_size = 10000     # events in memory
analytics_flush_interval_secs = 300  # persist to MongoDB every 5 min
```

## Available Metrics

- Total operations and error count
- Latency percentiles (p50, p95, p99)
- Top operations by count
- Top collections by activity
- Error rate

## Persistence

Analytics are optionally flushed to `__mongocore.analytics` collection
for historical analysis.
```

- [ ] **Step 3: Write multi-tenant docs**

```markdown
# Multi-Tenant Support

MongoCore v2 supports serving multiple tenants from a shared sidecar
with isolated caches, quotas, and optional per-tenant connection URIs.

## Configuration

```toml
multi_tenant_enabled = true

[[tenants]]
tenant_id = "acme"
max_connections = 20
max_cache_entries = 1000
rate_limit_ops_per_sec = 500

[[tenants]]
tenant_id = "beta"
max_connections = 5
rate_limit_ops_per_sec = 100
connection_uri = "mongodb://separate-cluster:27017"
```

## Tenant Identification

Pass `x-tenant-id` header in gRPC metadata or HTTP headers:

```python
# Python — tenant passed automatically if configured on client
client = MongoClient("localhost:50051", tenant_id="acme")
```

## Isolation Guarantees

- **Compiled query cache**: Partitioned by tenant — one tenant's compiled queries are invisible to others
- **Rate limiting**: Per-tenant ops/sec limits, returns RESOURCE_EXHAUSTED when exceeded
- **Connection pools**: Optional per-tenant URI for full network isolation
```

- [ ] **Step 4: Update docs/README.md with new guides**

Add rows to the table:
```markdown
| [Raw Passthrough](./raw-passthrough.md) | Arbitrary MongoDB commands for power users |
| [Analytics](./analytics.md) | Query performance insights and operation tracking |
| [Multi-Tenant](./multi-tenant.md) | Shared sidecar with per-tenant isolation |
```

- [ ] **Step 5: Update README.md roadmap**

Change v0.2 status from "Planned" to "Complete" (or "In Progress" during development).

- [ ] **Step 6: Commit**

```bash
git add docs/raw-passthrough.md docs/analytics.md docs/multi-tenant.md docs/README.md README.md
git commit -m "docs(v2): add documentation for raw passthrough, analytics, and multi-tenant"
```

---

## Implementation Order & Dependencies

```
Phase 1 (Independent, no v1 changes):
  Task 1.1–1.2: Raw validator + execution module
  Task 2.1–2.4: Analytics types, buffer, collector, aggregator

Phase 2 (Proto + gRPC changes):
  Task 1.3: RunCommand RPC
  Task 2.6–2.7: Instrument service + GetAnalytics RPC
  Task 3.1–3.5: Tenant context, registry, isolation, quota, config

Phase 3 (Integration):
  Task 1.4: RunCommand MCP tool
  Task 1.6: Client libraries update
  Task 3.6: Wire tenant into gRPC

Phase 4 (Validation):
  Task 1.5: Raw command integration tests
  Task 2.5, 2.8: Analytics persistence + integration tests
  Task 3.7: Multi-tenant integration tests
  Task 3.8: Documentation
```

Tasks within the same phase are parallelizable.

---

## Definition of Done (v2)

- [ ] `RunCommand` RPC executes arbitrary MongoDB commands with safety validation
- [ ] Dangerous commands blocked by default, bypass requires explicit opt-in
- [ ] All four client libraries expose `run_command`
- [ ] Analytics collector tracks every operation with latency and success/failure
- [ ] `GetAnalytics` RPC returns top operations, percentiles, and error rates
- [ ] Analytics optionally persist to `__mongocore.analytics` collection
- [ ] Multi-tenant: `x-tenant-id` header partitions cache and enforces quotas
- [ ] Per-tenant rate limiting returns `RESOURCE_EXHAUSTED` when exceeded
- [ ] Tenant config loaded from TOML `[[tenants]]` array
- [ ] All integration tests pass against `mongodb/atlas-local` container
- [ ] Documentation covers all three features with examples
