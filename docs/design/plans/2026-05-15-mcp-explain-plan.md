# MCP Explain Feature Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.
> If modifying client libraries: verify imports work and run `just test-clients`.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

**Goal:** Add response enrichment (`_context` on all MCP responses), session recording, and `explain_last`/`explain_session` tools that generate parameterized MongoCore client code.

**Architecture:** Session recorder module → response enrichment helper → modify all tool functions to include `_context` → two new explain tools → extended codegen modules (CRUD, ingest, search, session stitching) → documentation.

**Tech Stack:** Rust, serde_json, chrono, Arc/Mutex, Tera templates (existing), MongoCore client libraries (Python/TS/Go/Java).

---

## Client Library Audit Results

Before implementation, a subagent validated client library coverage. Findings:

| Operation | Python | TypeScript | Go | Java |
|-----------|--------|-----------|-----|------|
| find, find_one, insert, insert_many, update, update_many, delete, delete_many | ✓ | ✓ | ✓ | ✓ |
| aggregate, create_index, create_collection, list_collections, list_databases | ✓ | ✓ | ✓ | ✓ |
| run_command, ingest, watch_directory, pipeline | ✓ | ✓ | ✓ | ✓ |
| transaction_pipeline | ✓ | ✓ | ✗ Go | ✗ Java (stub) |
| count_documents | ✗ | ✗ | ✗ | ✗ |
| drop_collection | ✗ | ✗ | ✗ | ✗ |
| embed_and_store, semantic_search | ✗ | ✗ | ✗ | ✗ |

**Strategy:** Implement the missing operations in all client libraries (Tasks 13-16) so codegen produces first-class client code for every operation.

---

## File Structure

| File | Responsibility |
|------|---------------|
| `src/mcp/session.rs` (new) | `SessionRecorder` + `OperationRecord` structs, append/retrieve/clear logic |
| `src/mcp/context.rs` (new) | `build_context()` helper for each tool type, truncation logic |
| `src/mcp/tools.rs` | Modify all tool functions to include `_context` in responses + record to session |
| `src/mcp/handler.rs` | Add `SessionRecorder` to `McpHandler`, thread through to tool execution |
| `src/mcp/mod.rs` | Declare new `session` and `context` modules |
| `src/mcp/codegen/crud_gen.rs` (new) | Insert, update, delete code generation |
| `src/mcp/codegen/ingest_gen.rs` (new) | Ingest operation code generation |
| `src/mcp/codegen/search_gen.rs` (new) | embed_and_store, semantic_search code generation |
| `src/mcp/codegen/session_gen.rs` (new) | Full session stitching (imports, client init, main) |
| `src/mcp/codegen/mod.rs` | Register new codegen modules |
| `tests/integration/mcp_test.rs` | Update tool count assertion, add explain integration tests |
| `docs/explain.md` (new) | User-facing documentation |
| `clients/python/src/mongocore/{collection,client}.py` | Add count_documents, drop_collection, embed_and_store, semantic_search |
| `clients/typescript/src/{collection,client,database}.ts` | Add countDocuments, dropCollection, embedAndStore, semanticSearch |
| `clients/go/mongocore/{collection,client,database}.go` | Add CountDocuments, DropCollection, EmbedAndStore, SemanticSearch, TransactionPipeline |
| `clients/java/src/main/java/com/mongocore/{MongoCollection,MongoClient,MongoDatabase}.java` | Add countDocuments, dropCollection, embedAndStore, semanticSearch, transactionPipeline |

---

## Task 1: Session Recorder Module

**Files:**
- Create: `src/mcp/session.rs`
- Modify: `src/mcp/mod.rs`

- [ ] **Step 1: Write unit tests for SessionRecorder**

Add to `src/mcp/session.rs`:

```rust
use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct OperationRecord {
    pub index: usize,
    pub tool_name: String,
    pub params: Value,
    pub context: Value,
    pub success: bool,
    pub error_message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Default)]
pub struct SessionRecorder {
    operations: Vec<OperationRecord>,
}

/// Tools excluded from recording (meta/diagnostic)
const EXCLUDED_TOOLS: &[&str] = &[
    "explain_last",
    "explain_session",
    "get_analytics",
    "collection_schema",
];

impl SessionRecorder {
    pub fn new() -> Self {
        Self { operations: Vec::new() }
    }

    pub fn should_record(tool_name: &str) -> bool {
        !EXCLUDED_TOOLS.contains(&tool_name)
    }

    pub fn record(
        &mut self,
        tool_name: String,
        params: Value,
        context: Value,
        success: bool,
        error_message: Option<String>,
    ) {
        let index = self.operations.len();
        self.operations.push(OperationRecord {
            index,
            tool_name,
            params,
            context,
            success,
            error_message,
            timestamp: Utc::now(),
        });
    }

    pub fn get_last(&self, offset: usize) -> Option<&OperationRecord> {
        if offset >= self.operations.len() {
            return None;
        }
        self.operations.get(self.operations.len() - 1 - offset)
    }

    pub fn get_all(&self) -> &[OperationRecord] {
        &self.operations
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    pub fn len(&self) -> usize {
        self.operations.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_record_and_retrieve_last() {
        let mut recorder = SessionRecorder::new();
        recorder.record(
            "insert_many".to_string(),
            json!({"database": "mydb", "collection": "users", "documents": []}),
            json!({"operation": "insert_many", "database": "mydb", "collection": "users", "document_count": 0}),
            true,
            None,
        );
        recorder.record(
            "find".to_string(),
            json!({"database": "mydb", "collection": "users", "filter": {}}),
            json!({"operation": "find", "database": "mydb", "collection": "users", "filter": {}}),
            true,
            None,
        );

        let last = recorder.get_last(0).unwrap();
        assert_eq!(last.tool_name, "find");
        assert_eq!(last.index, 1);

        let prev = recorder.get_last(1).unwrap();
        assert_eq!(prev.tool_name, "insert_many");
        assert_eq!(prev.index, 0);

        assert!(recorder.get_last(2).is_none());
    }

    #[test]
    fn test_excluded_tools() {
        assert!(!SessionRecorder::should_record("explain_last"));
        assert!(!SessionRecorder::should_record("explain_session"));
        assert!(!SessionRecorder::should_record("get_analytics"));
        assert!(!SessionRecorder::should_record("collection_schema"));
        assert!(SessionRecorder::should_record("find"));
        assert!(SessionRecorder::should_record("insert_many"));
        assert!(SessionRecorder::should_record("list_collections"));
    }

    #[test]
    fn test_error_recording() {
        let mut recorder = SessionRecorder::new();
        recorder.record(
            "delete_many".to_string(),
            json!({"database": "mydb", "collection": "users", "filter": {"status": "inactive"}}),
            json!({"operation": "delete_many", "database": "mydb", "collection": "users", "filter": {"status": "inactive"}}),
            false,
            Some("permission denied".to_string()),
        );

        let last = recorder.get_last(0).unwrap();
        assert!(!last.success);
        assert_eq!(last.error_message.as_deref(), Some("permission denied"));
    }

    #[test]
    fn test_empty_session() {
        let recorder = SessionRecorder::new();
        assert!(recorder.is_empty());
        assert_eq!(recorder.len(), 0);
        assert!(recorder.get_last(0).is_none());
    }

    #[test]
    fn test_get_all() {
        let mut recorder = SessionRecorder::new();
        recorder.record("find".to_string(), json!({}), json!({}), true, None);
        recorder.record("insert".to_string(), json!({}), json!({}), true, None);
        recorder.record("update".to_string(), json!({}), json!({}), true, None);

        let all = recorder.get_all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].tool_name, "find");
        assert_eq!(all[1].tool_name, "insert");
        assert_eq!(all[2].tool_name, "update");
    }
}
```

- [ ] **Step 2: Register the module in mod.rs**

Add to `src/mcp/mod.rs`:

```rust
pub mod session;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib session`
Expected: All 5 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/mcp/session.rs src/mcp/mod.rs
git commit -m "feat(mcp): add SessionRecorder module for operation history"
```

---

## Task 2: Context Builder Module

**Files:**
- Create: `src/mcp/context.rs`
- Modify: `src/mcp/mod.rs`

- [ ] **Step 1: Write the context builder with truncation logic**

Create `src/mcp/context.rs`:

```rust
use serde_json::{json, Value};

const MAX_SERIALIZED_SIZE: usize = 1024;
const MAX_PIPELINE_STAGES: usize = 5;

/// Build a `_context` object for a tool response.
/// Only includes fields relevant to the operation.
pub fn build_context(tool_name: &str, args: &Value) -> Value {
    match tool_name {
        "find" | "find_one" => build_find_context(tool_name, args),
        "insert" | "insert_many" => build_insert_context(tool_name, args),
        "update" | "update_many" => build_update_context(tool_name, args),
        "delete" | "delete_many" => build_delete_context(tool_name, args),
        "aggregate" => build_aggregate_context(args),
        "count_documents" => build_count_context(args),
        "ask" => build_ask_context(args),
        "ingest" => build_ingest_context(args),
        "create_index" => build_create_index_context(args),
        "drop_collection" => build_drop_collection_context(args),
        "create_collection" => build_create_collection_context(args),
        "list_collections" => build_list_collections_context(args),
        "list_databases" => json!({"operation": "list_databases"}),
        "run_command" => build_run_command_context(args),
        "embed_and_store" => build_embed_context(args),
        "semantic_search" => build_semantic_search_context(args),
        "watch_directory" => build_watch_context(args),
        "pipeline" => build_pipeline_context(args),
        "transaction_pipeline" => build_transaction_pipeline_context(args),
        _ => json!({"operation": tool_name}),
    }
}

fn build_find_context(tool_name: &str, args: &Value) -> Value {
    let mut ctx = json!({
        "operation": tool_name,
        "database": args.get("database").unwrap_or(&Value::Null),
        "collection": args.get("collection").unwrap_or(&Value::Null),
    });
    let obj = ctx.as_object_mut().unwrap();
    if let Some(filter) = args.get("filter") {
        obj.insert("filter".to_string(), truncate_value(filter));
    }
    if let Some(projection) = args.get("projection") {
        obj.insert("projection".to_string(), projection.clone());
    }
    if let Some(sort) = args.get("sort") {
        obj.insert("sort".to_string(), sort.clone());
    }
    if let Some(limit) = args.get("limit") {
        obj.insert("limit".to_string(), limit.clone());
    }
    if let Some(skip) = args.get("skip") {
        obj.insert("skip".to_string(), skip.clone());
    }
    ctx
}

fn build_insert_context(tool_name: &str, args: &Value) -> Value {
    let mut ctx = json!({
        "operation": tool_name,
        "database": args.get("database").unwrap_or(&Value::Null),
        "collection": args.get("collection").unwrap_or(&Value::Null),
    });
    let obj = ctx.as_object_mut().unwrap();

    // Document count
    if let Some(docs) = args.get("documents").and_then(|d| d.as_array()) {
        obj.insert("document_count".to_string(), json!(docs.len()));
        // Document schema from first document
        if let Some(first) = docs.first().and_then(|d| d.as_object()) {
            let schema: serde_json::Map<String, Value> = first
                .iter()
                .map(|(k, v)| (k.clone(), json!(json_type_name(v))))
                .collect();
            obj.insert("document_schema".to_string(), Value::Object(schema));
        }
    } else if args.get("document").is_some() {
        obj.insert("document_count".to_string(), json!(1));
        if let Some(doc) = args.get("document").and_then(|d| d.as_object()) {
            let schema: serde_json::Map<String, Value> = doc
                .iter()
                .map(|(k, v)| (k.clone(), json!(json_type_name(v))))
                .collect();
            obj.insert("document_schema".to_string(), Value::Object(schema));
        }
    }
    ctx
}

fn build_update_context(tool_name: &str, args: &Value) -> Value {
    let mut ctx = json!({
        "operation": tool_name,
        "database": args.get("database").unwrap_or(&Value::Null),
        "collection": args.get("collection").unwrap_or(&Value::Null),
    });
    let obj = ctx.as_object_mut().unwrap();
    if let Some(filter) = args.get("filter") {
        obj.insert("filter".to_string(), truncate_value(filter));
    }
    if let Some(update) = args.get("update") {
        obj.insert("update".to_string(), truncate_value(update));
    }
    ctx
}

fn build_delete_context(tool_name: &str, args: &Value) -> Value {
    let mut ctx = json!({
        "operation": tool_name,
        "database": args.get("database").unwrap_or(&Value::Null),
        "collection": args.get("collection").unwrap_or(&Value::Null),
    });
    let obj = ctx.as_object_mut().unwrap();
    if let Some(filter) = args.get("filter") {
        obj.insert("filter".to_string(), truncate_value(filter));
    }
    ctx
}

fn build_aggregate_context(args: &Value) -> Value {
    let mut ctx = json!({
        "operation": "aggregate",
        "database": args.get("database").unwrap_or(&Value::Null),
        "collection": args.get("collection").unwrap_or(&Value::Null),
    });
    let obj = ctx.as_object_mut().unwrap();
    if let Some(pipeline) = args.get("pipeline") {
        obj.insert("pipeline".to_string(), truncate_pipeline(pipeline));
    }
    ctx
}

fn build_count_context(args: &Value) -> Value {
    let mut ctx = json!({
        "operation": "count_documents",
        "database": args.get("database").unwrap_or(&Value::Null),
        "collection": args.get("collection").unwrap_or(&Value::Null),
    });
    let obj = ctx.as_object_mut().unwrap();
    if let Some(filter) = args.get("filter") {
        obj.insert("filter".to_string(), truncate_value(filter));
    }
    ctx
}

fn build_ask_context(args: &Value) -> Value {
    json!({
        "operation": "ask",
        "database": args.get("database").unwrap_or(&Value::Null),
        "collection": args.get("collection").unwrap_or(&Value::Null),
        "question": args.get("question").unwrap_or(&Value::Null),
    })
}

fn build_ingest_context(args: &Value) -> Value {
    let mut ctx = json!({
        "operation": "ingest",
        "database": args.get("database").unwrap_or(&Value::Null),
        "collection": args.get("collection").unwrap_or(&Value::Null),
        "file_path": args.get("file_path").unwrap_or(&Value::Null),
    });
    let obj = ctx.as_object_mut().unwrap();
    if let Some(format) = args.get("format") {
        obj.insert("format".to_string(), format.clone());
    }
    if let Some(transforms) = args.get("transforms") {
        obj.insert("transforms".to_string(), transforms.clone());
    }
    if let Some(dedup_key) = args.get("dedup_key") {
        obj.insert("dedup_key".to_string(), dedup_key.clone());
    }
    if let Some(conflict_strategy) = args.get("conflict_strategy") {
        obj.insert("conflict_strategy".to_string(), conflict_strategy.clone());
    }
    ctx
}

fn build_create_index_context(args: &Value) -> Value {
    let mut ctx = json!({
        "operation": "create_index",
        "database": args.get("database").unwrap_or(&Value::Null),
        "collection": args.get("collection").unwrap_or(&Value::Null),
        "keys": args.get("keys").unwrap_or(&Value::Null),
    });
    let obj = ctx.as_object_mut().unwrap();
    if let Some(options) = args.get("options") {
        obj.insert("options".to_string(), options.clone());
    }
    ctx
}

fn build_drop_collection_context(args: &Value) -> Value {
    json!({
        "operation": "drop_collection",
        "database": args.get("database").unwrap_or(&Value::Null),
        "collection": args.get("collection").unwrap_or(&Value::Null),
    })
}

fn build_create_collection_context(args: &Value) -> Value {
    json!({
        "operation": "create_collection",
        "database": args.get("database").unwrap_or(&Value::Null),
        "collection": args.get("collection").unwrap_or(&Value::Null),
    })
}

fn build_list_collections_context(args: &Value) -> Value {
    json!({
        "operation": "list_collections",
        "database": args.get("database").unwrap_or(&Value::Null),
    })
}

fn build_run_command_context(args: &Value) -> Value {
    let mut ctx = json!({
        "operation": "run_command",
        "database": args.get("database").unwrap_or(&Value::Null),
    });
    let obj = ctx.as_object_mut().unwrap();
    // Include only the top-level command name (first key)
    if let Some(command) = args.get("command").and_then(|c| c.as_object()) {
        if let Some(first_key) = command.keys().next() {
            obj.insert("command_name".to_string(), json!(first_key));
        }
    }
    ctx
}

fn build_embed_context(args: &Value) -> Value {
    let mut ctx = json!({
        "operation": "embed_and_store",
        "database": args.get("database").unwrap_or(&Value::Null),
        "collection": args.get("collection").unwrap_or(&Value::Null),
        "embed_field": args.get("embed_field").unwrap_or(&Value::Null),
    });
    let obj = ctx.as_object_mut().unwrap();
    if let Some(docs) = args.get("documents").and_then(|d| d.as_array()) {
        obj.insert("document_count".to_string(), json!(docs.len()));
    }
    ctx
}

fn build_semantic_search_context(args: &Value) -> Value {
    let mut ctx = json!({
        "operation": "semantic_search",
        "database": args.get("database").unwrap_or(&Value::Null),
        "collection": args.get("collection").unwrap_or(&Value::Null),
        "query": args.get("query").unwrap_or(&Value::Null),
    });
    let obj = ctx.as_object_mut().unwrap();
    if let Some(index_name) = args.get("index_name") {
        obj.insert("index_name".to_string(), index_name.clone());
    }
    if let Some(limit) = args.get("limit") {
        obj.insert("limit".to_string(), limit.clone());
    }
    ctx
}

fn build_watch_context(args: &Value) -> Value {
    let mut ctx = json!({
        "operation": "watch_directory",
        "database": args.get("database").unwrap_or(&Value::Null),
        "collection": args.get("collection").unwrap_or(&Value::Null),
        "path": args.get("path").unwrap_or(&Value::Null),
    });
    let obj = ctx.as_object_mut().unwrap();
    if let Some(pattern) = args.get("pattern") {
        obj.insert("pattern".to_string(), pattern.clone());
    }
    if let Some(strategy) = args.get("conflict_strategy") {
        obj.insert("conflict_strategy".to_string(), strategy.clone());
    }
    ctx
}

fn build_pipeline_context(args: &Value) -> Value {
    let mut ctx = json!({"operation": "pipeline"});
    let obj = ctx.as_object_mut().unwrap();
    if let Some(steps) = args.get("steps").and_then(|s| s.as_array()) {
        let summary: Vec<Value> = steps
            .iter()
            .map(|step| {
                json!({
                    "name": step.get("name").unwrap_or(&Value::Null),
                    "tool_name": step.get("tool").or_else(|| step.get("operation")).unwrap_or(&Value::Null),
                })
            })
            .collect();
        obj.insert("steps".to_string(), json!(summary));
    }
    ctx
}

fn build_transaction_pipeline_context(args: &Value) -> Value {
    let mut ctx = json!({"operation": "transaction_pipeline"});
    let obj = ctx.as_object_mut().unwrap();
    if let Some(steps) = args.get("steps").and_then(|s| s.as_array()) {
        let summary: Vec<Value> = steps
            .iter()
            .map(|step| {
                json!({
                    "name": step.get("name").unwrap_or(&Value::Null),
                    "tool_name": step.get("tool").or_else(|| step.get("operation")).unwrap_or(&Value::Null),
                })
            })
            .collect();
        obj.insert("steps".to_string(), json!(summary));
    }
    ctx
}

/// Truncate a JSON value if its serialized form exceeds MAX_SERIALIZED_SIZE.
/// For objects over the limit, return only the top-level keys.
fn truncate_value(value: &Value) -> Value {
    let serialized = serde_json::to_string(value).unwrap_or_default();
    if serialized.len() <= MAX_SERIALIZED_SIZE {
        return value.clone();
    }
    // Return top-level keys only
    match value {
        Value::Object(map) => {
            let keys: Vec<String> = map.keys().cloned().collect();
            json!({"_truncated": true, "_keys": keys})
        }
        Value::Array(arr) => {
            json!({"_truncated": true, "_length": arr.len()})
        }
        _ => value.clone(),
    }
}

/// Truncate pipeline: if >5 stages, show first 3 + summary + last.
fn truncate_pipeline(pipeline: &Value) -> Value {
    match pipeline.as_array() {
        Some(stages) if stages.len() > MAX_PIPELINE_STAGES => {
            let mut result = Vec::new();
            result.extend_from_slice(&stages[..3]);
            result.push(json!(format!("... ({} more stages)", stages.len() - 4)));
            result.push(stages.last().unwrap().clone());
            Value::Array(result)
        }
        _ => pipeline.clone(),
    }
}

/// Return a human-readable type name for a JSON value.
fn json_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(n) => {
            if n.is_f64() { "float" } else { "int" }
        }
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_build_find_context() {
        let args = json!({
            "database": "mydb",
            "collection": "users",
            "filter": {"status": "active"},
            "limit": 10
        });
        let ctx = build_context("find", &args);
        assert_eq!(ctx["operation"], "find");
        assert_eq!(ctx["database"], "mydb");
        assert_eq!(ctx["collection"], "users");
        assert_eq!(ctx["filter"]["status"], "active");
        assert_eq!(ctx["limit"], 10);
        assert!(ctx.get("skip").is_none());
    }

    #[test]
    fn test_build_insert_context_schema() {
        let args = json!({
            "database": "mydb",
            "collection": "users",
            "documents": [
                {"name": "Alice", "age": 30, "tags": ["admin"]},
                {"name": "Bob", "age": 25, "tags": []}
            ]
        });
        let ctx = build_context("insert_many", &args);
        assert_eq!(ctx["document_count"], 2);
        assert_eq!(ctx["document_schema"]["name"], "string");
        assert_eq!(ctx["document_schema"]["age"], "int");
        assert_eq!(ctx["document_schema"]["tags"], "array");
    }

    #[test]
    fn test_truncate_large_filter() {
        // Create a filter that exceeds 1KB
        let mut big_filter = serde_json::Map::new();
        for i in 0..100 {
            big_filter.insert(format!("field_{}", i), json!("some_value_that_takes_space"));
        }
        let args = json!({
            "database": "mydb",
            "collection": "users",
            "filter": big_filter
        });
        let ctx = build_context("find", &args);
        assert!(ctx["filter"]["_truncated"].as_bool().unwrap_or(false));
        assert!(ctx["filter"]["_keys"].as_array().is_some());
    }

    #[test]
    fn test_truncate_pipeline() {
        let stages: Vec<Value> = (0..8)
            .map(|i| json!({"$match": {"field": i}}))
            .collect();
        let args = json!({
            "database": "mydb",
            "collection": "orders",
            "pipeline": stages
        });
        let ctx = build_context("aggregate", &args);
        let pipeline = ctx["pipeline"].as_array().unwrap();
        assert_eq!(pipeline.len(), 5); // 3 + summary + last
        assert!(pipeline[3].as_str().unwrap().contains("more stages"));
    }

    #[test]
    fn test_pipeline_steps_summary() {
        let args = json!({
            "steps": [
                {"name": "create_users", "tool": "insert_many", "arguments": {}},
                {"name": "index_email", "tool": "create_index", "arguments": {}}
            ]
        });
        let ctx = build_context("pipeline", &args);
        let steps = ctx["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0]["name"], "create_users");
        assert_eq!(steps[0]["tool_name"], "insert_many");
    }

    #[test]
    fn test_run_command_only_shows_command_name() {
        let args = json!({
            "database": "admin",
            "command": {"replSetGetStatus": 1, "someOtherField": "value"}
        });
        let ctx = build_context("run_command", &args);
        assert_eq!(ctx["command_name"], "replSetGetStatus");
        assert!(ctx.get("command").is_none());
    }

    #[test]
    fn test_json_type_name() {
        assert_eq!(json_type_name(&json!(null)), "null");
        assert_eq!(json_type_name(&json!(true)), "bool");
        assert_eq!(json_type_name(&json!(42)), "int");
        assert_eq!(json_type_name(&json!(3.14)), "float");
        assert_eq!(json_type_name(&json!("hello")), "string");
        assert_eq!(json_type_name(&json!([1, 2])), "array");
        assert_eq!(json_type_name(&json!({"a": 1})), "object");
    }
}
```

- [ ] **Step 2: Register module in mod.rs**

Add to `src/mcp/mod.rs`:

```rust
pub mod context;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --lib context`
Expected: All 7 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/mcp/context.rs src/mcp/mod.rs
git commit -m "feat(mcp): add context builder for response enrichment"
```

---

## Task 3: Wire SessionRecorder into McpHandler

**Files:**
- Modify: `src/mcp/handler.rs`
- Modify: `src/mcp/tools.rs` (signature change only)

- [ ] **Step 1: Add SessionRecorder to McpHandler**

In `src/mcp/handler.rs`, add the import and field:

```rust
use crate::mcp::session::SessionRecorder;
use std::sync::Mutex;
```

Add field to `McpHandler` struct:

```rust
pub struct McpHandler {
    // ... existing fields ...
    session: Arc<Mutex<SessionRecorder>>,
}
```

Update the constructor (`McpHandler::new` or wherever it's built) to initialize:

```rust
session: Arc::new(Mutex::new(SessionRecorder::new())),
```

- [ ] **Step 2: Modify handle_tools_call to record operations**

In the `handle_tools_call` method, after `tools::execute_tool(...)` returns, add recording logic:

```rust
let result = tools::execute_tool(
    &self.operations,
    &self.pool,
    self.analytics.as_ref(),
    self.ingestion.as_ref(),
    self.watcher.as_ref(),
    &self.safety,
    self.translator.as_ref(),
    self.voyage.as_ref(),
    &self.skills,
    &tool_name,
    &arguments,
).await;

// Record to session (if not excluded)
if SessionRecorder::should_record(&tool_name) {
    let context = crate::mcp::context::build_context(&tool_name, &arguments);
    if let Ok(mut session) = self.session.lock() {
        session.record(
            tool_name.clone(),
            arguments.clone(),
            context,
            !result.is_error,
            if result.is_error {
                result.content.first().map(|c| c.text.clone())
            } else {
                None
            },
        );
    }
}
```

- [ ] **Step 3: Add session Arc to execute_tool signature**

Modify `execute_tool` in `src/mcp/tools.rs` to accept the session recorder (needed for explain tools later):

```rust
pub async fn execute_tool(
    operations: &Operations,
    pool: &ConnectionPool,
    analytics: Option<&Arc<AnalyticsCollector>>,
    ingestion: Option<&Arc<IngestionEngine>>,
    watcher: Option<&Arc<DirectoryWatcher>>,
    safety: &SafetyConfig,
    translator: Option<&Arc<CompiledQueryTranslator>>,
    voyage: Option<&Arc<VoyageClient>>,
    skills: &SkillRegistry,
    session: &Arc<Mutex<SessionRecorder>>,
    name: &str,
    arguments: &Value,
) -> McpToolCallResult {
```

Update the call site in `handler.rs` accordingly.

- [ ] **Step 4: Verify build compiles**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No warnings (you may need to add `#[allow(unused)]` on `session` param temporarily until explain tools use it)

- [ ] **Step 5: Run tests**

Run: `cargo test --lib`
Expected: All tests pass

- [ ] **Step 6: Commit**

```bash
git add src/mcp/handler.rs src/mcp/tools.rs
git commit -m "feat(mcp): wire SessionRecorder into McpHandler and tool execution"
```

---

## Task 4: Enrich Tool Responses with _context

**Files:**
- Modify: `src/mcp/tools.rs`

This is the largest task — every tool's success (and error) response needs `_context` injected. The pattern is the same for each: take the existing response JSON, add a `_context` field built from `build_context()`.

- [ ] **Step 1: Create a helper function to inject context into responses**

Add to `src/mcp/tools.rs`:

```rust
use crate::mcp::context::build_context;

/// Wrap a tool result with _context metadata.
/// For text responses that are valid JSON objects, inserts _context as a field.
/// For non-JSON or array responses, wraps in {"result": ..., "_context": ...}.
fn enrich_result(result: McpToolCallResult, tool_name: &str, args: &Value) -> McpToolCallResult {
    let context = build_context(tool_name, args);

    let enriched_content: Vec<McpContent> = result
        .content
        .into_iter()
        .map(|content| {
            // Try to parse as JSON and inject _context
            if let Ok(mut parsed) = serde_json::from_str::<Value>(&content.text) {
                if let Some(obj) = parsed.as_object_mut() {
                    obj.insert("_context".to_string(), context.clone());
                    McpContent {
                        type_: content.type_,
                        text: serde_json::to_string_pretty(&parsed).unwrap_or(content.text),
                    }
                } else {
                    // Array or scalar — wrap
                    let wrapped = json!({
                        "result": parsed,
                        "_context": context
                    });
                    McpContent {
                        type_: content.type_,
                        text: serde_json::to_string_pretty(&wrapped).unwrap_or(content.text),
                    }
                }
            } else {
                // Non-JSON text — wrap
                let wrapped = json!({
                    "result": content.text,
                    "_context": context
                });
                McpContent {
                    type_: "text".to_string(),
                    text: serde_json::to_string_pretty(&wrapped).unwrap_or_default(),
                }
            }
        })
        .collect();

    McpToolCallResult {
        content: enriched_content,
        is_error: result.is_error,
    }
}
```

- [ ] **Step 2: Apply enrichment in execute_tool_inner**

Instead of modifying each individual tool function, apply enrichment at the dispatch level. In `execute_tool_inner` (or `execute_tool`), wrap the result:

```rust
pub async fn execute_tool(
    // ... all params including session ...
    name: &str,
    arguments: &Value,
) -> McpToolCallResult {
    let result = execute_tool_inner(
        operations, pool, analytics, ingestion, watcher,
        safety, translator, voyage, skills, session, name, arguments,
    ).await;

    // Enrich all responses with _context (both success and error)
    enrich_result(result, name, arguments)
}
```

- [ ] **Step 3: Special case for `ask` tool — add compiled_query to context**

The `ask` tool has access to the compiled MQL (filter/pipeline) at execution time, which is not in the original `args`. After the translation succeeds in the `ask` handler, inject the compiled query into a modified args value before context is built. 

In the `ask` tool implementation, after `compiled` is available, store it in args for context building:

```rust
// Inside the ask tool handler, after successful translation:
let mut enriched_args = arguments.clone();
if let Some(obj) = enriched_args.as_object_mut() {
    match &compiled.mql {
        CompiledMql::Find { filter, .. } => {
            obj.insert("compiled_query".to_string(), json!({"method": "find", "filter": filter}));
        }
        CompiledMql::Aggregate { pipeline } => {
            obj.insert("compiled_query".to_string(), json!({"method": "aggregate", "pipeline": pipeline}));
        }
    }
}
```

Note: This requires `ask` to return the enriched_args alongside the result, or to build context locally. The simplest approach is to handle `ask` enrichment inside the tool itself and skip the generic enrichment for it. Add `ask` to a list of tools that handle their own context.

- [ ] **Step 4: Run tests and verify build**

Run: `cargo build 2>&1 | grep "warning:"` then `cargo test --lib`
Expected: Zero warnings, all tests pass

- [ ] **Step 5: Commit**

```bash
git add src/mcp/tools.rs
git commit -m "feat(mcp): enrich all tool responses with _context metadata"
```

---

## Task 5: CRUD Code Generation Module

**Files:**
- Create: `src/mcp/codegen/crud_gen.rs`
- Modify: `src/mcp/codegen/mod.rs`

- [ ] **Step 1: Write unit tests for CRUD codegen**

Create `src/mcp/codegen/crud_gen.rs` with tests first:

```rust
use serde_json::Value;
use super::Language;

/// Generate MongoCore client code for a CRUD operation.
pub fn generate_crud_code(
    language: Language,
    tool_name: &str,
    params: &Value,
) -> Result<String, String> {
    match tool_name {
        "find" | "find_one" => generate_find(language, tool_name, params),
        "insert" => generate_insert_one(language, params),
        "insert_many" => generate_insert_many(language, params),
        "update" | "update_many" => generate_update(language, tool_name, params),
        "delete" | "delete_many" => generate_delete(language, tool_name, params),
        _ => Err(format!("Unsupported CRUD operation: {}", tool_name)),
    }
}

fn get_str_param<'a>(params: &'a Value, key: &str) -> &'a str {
    params.get(key).and_then(|v| v.as_str()).unwrap_or("unknown")
}

fn format_json_value(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
}

fn generate_find(language: Language, tool_name: &str, params: &Value) -> Result<String, String> {
    let db = get_str_param(params, "database");
    let coll = get_str_param(params, "collection");
    let filter = params.get("filter").unwrap_or(&Value::Null);
    let method = if tool_name == "find_one" { "find_one" } else { "find" };

    match language {
        Language::Python => {
            let mut code = format!(
                "async def {method}_{coll}(\n    client,\n    db_name: str = \"{db}\",\n    collection_name: str = \"{coll}\",\n    filter_doc: dict = {filter},\n",
                method = method,
                coll = coll,
                db = db,
                filter = format_json_value(filter),
            );
            if let Some(limit) = params.get("limit") {
                code.push_str(&format!("    limit: int = {},\n", limit));
            }
            if let Some(skip) = params.get("skip") {
                code.push_str(&format!("    skip: int = {},\n", skip));
            }
            code.push_str(") -> dict:\n");
            code.push_str(&format!("    db = client.database(db_name)\n"));
            code.push_str(&format!("    collection = db.collection(collection_name)\n"));
            code.push_str(&format!("    return await collection.{method}(filter_doc",
                method = method));
            if params.get("limit").is_some() {
                code.push_str(", limit=limit");
            }
            if params.get("skip").is_some() {
                code.push_str(", skip=skip");
            }
            code.push_str(")\n");
            Ok(code)
        }
        Language::TypeScript => {
            let mut code = format!(
                "async function {method}_{coll}(\n  client: MongoCore,\n  dbName = \"{db}\",\n  collectionName = \"{coll}\",\n  filterDoc = {filter},\n",
                method = method,
                coll = coll,
                db = db,
                filter = format_json_value(filter),
            );
            if let Some(limit) = params.get("limit") {
                code.push_str(&format!("  limit = {},\n", limit));
            }
            code.push_str(") {{\n");
            code.push_str("  const db = client.database(dbName);\n");
            code.push_str("  const collection = db.collection(collectionName);\n");
            code.push_str(&format!("  return await collection.{method}(filterDoc",
                method = method));
            if params.get("limit").is_some() {
                code.push_str(", {{ limit }}");
            }
            code.push_str(");\n}}\n");
            Ok(code)
        }
        Language::Go => {
            let mut code = format!(
                "func {method}_{coll}(client *mongocore.Client, dbName string, collectionName string, filter interface{{}}) ([]bson.M, error) {{\n",
                method = method,
                coll = coll,
            );
            code.push_str("    db := client.Database(dbName)\n");
            code.push_str("    collection := db.Collection(collectionName)\n");
            code.push_str(&format!("    return collection.{}(filter)\n}}\n", capitalize(method)));
            Ok(code)
        }
        Language::Java => {
            let method_name = to_camel_case(&format!("{}_{}", method, coll));
            let mut code = format!(
                "public List<Document> {}(MongoClient client, String dbName, String collectionName, Document filter) {{\n",
                method_name,
            );
            code.push_str("    MongoDatabase db = client.database(dbName);\n");
            code.push_str("    MongoCollection collection = db.collection(collectionName);\n");
            code.push_str(&format!("    return collection.{}(filter);\n}}\n", method));
            Ok(code)
        }
    }
}

fn generate_insert_one(language: Language, params: &Value) -> Result<String, String> {
    let db = get_str_param(params, "database");
    let coll = get_str_param(params, "collection");

    match language {
        Language::Python => Ok(format!(
            "async def insert_{coll}(\n    client,\n    db_name: str = \"{db}\",\n    collection_name: str = \"{coll}\",\n    document: dict = None,\n) -> str:\n    db = client.database(db_name)\n    collection = db.collection(collection_name)\n    result = await collection.insert(document or {{}})\n    return result[\"insertedId\"]\n",
            coll = coll, db = db,
        )),
        Language::TypeScript => Ok(format!(
            "async function insert_{coll}(\n  client: MongoCore,\n  dbName = \"{db}\",\n  collectionName = \"{coll}\",\n  document: Record<string, unknown> = {{}},\n) {{\n  const db = client.database(dbName);\n  const collection = db.collection(collectionName);\n  const result = await collection.insert(document);\n  return result.insertedId;\n}}\n",
            coll = coll, db = db,
        )),
        Language::Go => Ok(format!(
            "func insert_{coll}(client *mongocore.Client, dbName string, collectionName string, document interface{{}}) (string, error) {{\n    db := client.Database(dbName)\n    collection := db.Collection(collectionName)\n    return collection.Insert(document)\n}}\n",
            coll = coll,
        )),
        Language::Java => Ok(format!(
            "public String insert{coll_cap}(MongoClient client, String dbName, String collectionName, Document document) {{\n    MongoDatabase db = client.database(dbName);\n    MongoCollection collection = db.collection(collectionName);\n    return collection.insert(document).getInsertedId();\n}}\n",
            coll_cap = capitalize(coll),
        )),
    }
}

fn generate_insert_many(language: Language, params: &Value) -> Result<String, String> {
    let db = get_str_param(params, "database");
    let coll = get_str_param(params, "collection");

    match language {
        Language::Python => Ok(format!(
            "async def insert_many_{coll}(\n    client,\n    db_name: str = \"{db}\",\n    collection_name: str = \"{coll}\",\n    documents: list[dict] = None,\n) -> dict:\n    db = client.database(db_name)\n    collection = db.collection(collection_name)\n    return await collection.insert_many(documents or [])\n",
            coll = coll, db = db,
        )),
        Language::TypeScript => Ok(format!(
            "async function insertMany_{coll}(\n  client: MongoCore,\n  dbName = \"{db}\",\n  collectionName = \"{coll}\",\n  documents: Record<string, unknown>[] = [],\n) {{\n  const db = client.database(dbName);\n  const collection = db.collection(collectionName);\n  return await collection.insertMany(documents);\n}}\n",
            coll = coll, db = db,
        )),
        Language::Go => Ok(format!(
            "func insertMany_{coll}(client *mongocore.Client, dbName string, collectionName string, documents []interface{{}}) (*InsertManyResult, error) {{\n    db := client.Database(dbName)\n    collection := db.Collection(collectionName)\n    return collection.InsertMany(documents)\n}}\n",
            coll = coll,
        )),
        Language::Java => Ok(format!(
            "public InsertManyResult insertMany{coll_cap}(MongoClient client, String dbName, String collectionName, List<Document> documents) {{\n    MongoDatabase db = client.database(dbName);\n    MongoCollection collection = db.collection(collectionName);\n    return collection.insertMany(documents);\n}}\n",
            coll_cap = capitalize(coll),
        )),
    }
}

fn generate_update(language: Language, tool_name: &str, params: &Value) -> Result<String, String> {
    let db = get_str_param(params, "database");
    let coll = get_str_param(params, "collection");
    let filter = params.get("filter").unwrap_or(&Value::Null);
    let update = params.get("update").unwrap_or(&Value::Null);
    let method = tool_name;

    match language {
        Language::Python => Ok(format!(
            "async def {method}_{coll}(\n    client,\n    db_name: str = \"{db}\",\n    collection_name: str = \"{coll}\",\n    filter_doc: dict = {filter},\n    update_doc: dict = {update},\n) -> dict:\n    db = client.database(db_name)\n    collection = db.collection(collection_name)\n    return await collection.{method}(filter_doc, update_doc)\n",
            method = method, coll = coll, db = db,
            filter = format_json_value(filter),
            update = format_json_value(update),
        )),
        Language::TypeScript => Ok(format!(
            "async function {method}_{coll}(\n  client: MongoCore,\n  dbName = \"{db}\",\n  collectionName = \"{coll}\",\n  filterDoc = {filter},\n  updateDoc = {update},\n) {{\n  const db = client.database(dbName);\n  const collection = db.collection(collectionName);\n  return await collection.{method}(filterDoc, updateDoc);\n}}\n",
            method = to_camel_case(method), coll = coll, db = db,
            filter = format_json_value(filter),
            update = format_json_value(update),
        )),
        Language::Go => Ok(format!(
            "func {method}_{coll}(client *mongocore.Client, dbName string, collectionName string, filter interface{{}}, update interface{{}}) (*UpdateResult, error) {{\n    db := client.Database(dbName)\n    collection := db.Collection(collectionName)\n    return collection.{}(filter, update)\n}}\n",
            capitalize(method), method = method, coll = coll,
        )),
        Language::Java => Ok(format!(
            "public UpdateResult {method_camel}{coll_cap}(MongoClient client, String dbName, String collectionName, Document filter, Document update) {{\n    MongoDatabase db = client.database(dbName);\n    MongoCollection collection = db.collection(collectionName);\n    return collection.{method_camel}(filter, update);\n}}\n",
            method_camel = to_camel_case(method), coll_cap = capitalize(coll),
        )),
    }
}

fn generate_delete(language: Language, tool_name: &str, params: &Value) -> Result<String, String> {
    let db = get_str_param(params, "database");
    let coll = get_str_param(params, "collection");
    let filter = params.get("filter").unwrap_or(&Value::Null);
    let method = tool_name;

    match language {
        Language::Python => Ok(format!(
            "async def {method}_{coll}(\n    client,\n    db_name: str = \"{db}\",\n    collection_name: str = \"{coll}\",\n    filter_doc: dict = {filter},\n) -> dict:\n    db = client.database(db_name)\n    collection = db.collection(collection_name)\n    return await collection.{method}(filter_doc)\n",
            method = method, coll = coll, db = db,
            filter = format_json_value(filter),
        )),
        Language::TypeScript => Ok(format!(
            "async function {method}_{coll}(\n  client: MongoCore,\n  dbName = \"{db}\",\n  collectionName = \"{coll}\",\n  filterDoc = {filter},\n) {{\n  const db = client.database(dbName);\n  const collection = db.collection(collectionName);\n  return await collection.{method}(filterDoc);\n}}\n",
            method = to_camel_case(method), coll = coll, db = db,
            filter = format_json_value(filter),
        )),
        Language::Go => Ok(format!(
            "func {method}_{coll}(client *mongocore.Client, dbName string, collectionName string, filter interface{{}}) (*DeleteResult, error) {{\n    db := client.Database(dbName)\n    collection := db.Collection(collectionName)\n    return collection.{}(filter)\n}}\n",
            capitalize(method), method = method, coll = coll,
        )),
        Language::Java => Ok(format!(
            "public DeleteResult {method_camel}{coll_cap}(MongoClient client, String dbName, String collectionName, Document filter) {{\n    MongoDatabase db = client.database(dbName);\n    MongoCollection collection = db.collection(collectionName);\n    return collection.{method_camel}(filter);\n}}\n",
            method_camel = to_camel_case(method), coll_cap = capitalize(coll),
        )),
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

fn to_camel_case(s: &str) -> String {
    let parts: Vec<&str> = s.split('_').collect();
    if parts.is_empty() {
        return String::new();
    }
    let mut result = parts[0].to_string();
    for part in &parts[1..] {
        result.push_str(&capitalize(part));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_generate_find_python() {
        let params = json!({
            "database": "mydb",
            "collection": "users",
            "filter": {"status": "active"},
            "limit": 10
        });
        let code = generate_crud_code(Language::Python, "find", &params).unwrap();
        assert!(code.contains("async def find_users("));
        assert!(code.contains("db_name: str = \"mydb\""));
        assert!(code.contains("collection_name: str = \"users\""));
        assert!(code.contains("limit: int = 10"));
        assert!(code.contains("collection.find(filter_doc"));
    }

    #[test]
    fn test_generate_insert_many_typescript() {
        let params = json!({
            "database": "testdb",
            "collection": "orders",
            "documents": [{"item": "widget"}]
        });
        let code = generate_crud_code(Language::TypeScript, "insert_many", &params).unwrap();
        assert!(code.contains("async function insertMany_orders("));
        assert!(code.contains("dbName = \"testdb\""));
        assert!(code.contains("collection.insertMany(documents)"));
    }

    #[test]
    fn test_generate_update_many_go() {
        let params = json!({
            "database": "mydb",
            "collection": "users",
            "filter": {"active": false},
            "update": {"$set": {"archived": true}}
        });
        let code = generate_crud_code(Language::Go, "update_many", &params).unwrap();
        assert!(code.contains("func update_many_users("));
        assert!(code.contains("collection.UpdateMany(filter, update)"));
    }

    #[test]
    fn test_generate_delete_java() {
        let params = json!({
            "database": "mydb",
            "collection": "sessions",
            "filter": {"expired": true}
        });
        let code = generate_crud_code(Language::Java, "delete_many", &params).unwrap();
        assert!(code.contains("public DeleteResult deleteManySessions("));
        assert!(code.contains("collection.deleteMany(filter)"));
    }

    #[test]
    fn test_unsupported_operation() {
        let params = json!({});
        let result = generate_crud_code(Language::Python, "unknown_op", &params);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Register in codegen mod.rs**

Add to `src/mcp/codegen/mod.rs`:

```rust
pub mod crud_gen;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib crud_gen`
Expected: All 5 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/mcp/codegen/crud_gen.rs src/mcp/codegen/mod.rs
git commit -m "feat(mcp): add CRUD code generation module"
```

---

## Task 6: Ingest Code Generation Module

**Files:**
- Create: `src/mcp/codegen/ingest_gen.rs`
- Modify: `src/mcp/codegen/mod.rs`

- [ ] **Step 1: Write ingest codegen with tests**

Create `src/mcp/codegen/ingest_gen.rs`:

```rust
use serde_json::Value;
use super::Language;

/// Generate MongoCore client code for ingestion operations.
pub fn generate_ingest_code(
    language: Language,
    tool_name: &str,
    params: &Value,
) -> Result<String, String> {
    match tool_name {
        "ingest" => generate_ingest(language, params),
        "watch_directory" => generate_watch(language, params),
        _ => Err(format!("Unsupported ingest operation: {}", tool_name)),
    }
}

fn get_str_param<'a>(params: &'a Value, key: &str) -> &'a str {
    params.get(key).and_then(|v| v.as_str()).unwrap_or("unknown")
}

fn generate_ingest(language: Language, params: &Value) -> Result<String, String> {
    let db = get_str_param(params, "database");
    let coll = get_str_param(params, "collection");
    let file_path = get_str_param(params, "file_path");

    match language {
        Language::Python => {
            let mut code = format!(
                "async def ingest_{coll}(\n    client,\n    db_name: str = \"{db}\",\n    collection_name: str = \"{coll}\",\n    file_path: str = \"{file_path}\",\n",
                coll = coll, db = db, file_path = file_path,
            );
            if let Some(format) = params.get("format").and_then(|v| v.as_str()) {
                code.push_str(&format!("    format: str = \"{}\",\n", format));
            }
            if let Some(transforms) = params.get("transforms") {
                code.push_str(&format!("    transforms = {},\n", serde_json::to_string(transforms).unwrap_or_default()));
            }
            if let Some(dedup_key) = params.get("dedup_key").and_then(|v| v.as_str()) {
                code.push_str(&format!("    dedup_key: str = \"{}\",\n", dedup_key));
            }
            if let Some(strategy) = params.get("conflict_strategy").and_then(|v| v.as_str()) {
                code.push_str(&format!("    conflict_strategy: str = \"{}\",\n", strategy));
            }
            code.push_str(") -> dict:\n");
            code.push_str("    return await client.ingest(\n");
            code.push_str("        file_path=file_path,\n");
            code.push_str("        database=db_name,\n");
            code.push_str("        collection=collection_name,\n");
            if params.get("format").is_some() {
                code.push_str("        format=format,\n");
            }
            if params.get("transforms").is_some() {
                code.push_str("        transforms=transforms,\n");
            }
            if params.get("dedup_key").is_some() {
                code.push_str("        dedup_key=dedup_key,\n");
            }
            if params.get("conflict_strategy").is_some() {
                code.push_str("        conflict_strategy=conflict_strategy,\n");
            }
            code.push_str("    )\n");
            Ok(code)
        }
        Language::TypeScript => {
            let mut code = format!(
                "async function ingest_{coll}(\n  client: MongoCore,\n  dbName = \"{db}\",\n  collectionName = \"{coll}\",\n  filePath = \"{file_path}\",\n",
                coll = coll, db = db, file_path = file_path,
            );
            if let Some(format) = params.get("format").and_then(|v| v.as_str()) {
                code.push_str(&format!("  format = \"{}\",\n", format));
            }
            if let Some(dedup_key) = params.get("dedup_key").and_then(|v| v.as_str()) {
                code.push_str(&format!("  dedupKey = \"{}\",\n", dedup_key));
            }
            code.push_str(") {\n");
            code.push_str("  return await client.ingest({\n");
            code.push_str("    filePath,\n");
            code.push_str("    database: dbName,\n");
            code.push_str("    collection: collectionName,\n");
            if params.get("format").is_some() {
                code.push_str("    format,\n");
            }
            if params.get("dedup_key").is_some() {
                code.push_str("    dedupKey,\n");
            }
            code.push_str("  });\n}\n");
            Ok(code)
        }
        Language::Go => {
            let mut code = format!(
                "func ingest_{coll}(client *mongocore.Client, dbName string, collectionName string, filePath string) (*IngestResult, error) {{\n",
                coll = coll,
            );
            code.push_str("    return client.Ingest(&IngestOptions{\n");
            code.push_str("        FilePath:   filePath,\n");
            code.push_str("        Database:   dbName,\n");
            code.push_str("        Collection: collectionName,\n");
            if let Some(format) = params.get("format").and_then(|v| v.as_str()) {
                code.push_str(&format!("        Format:     \"{}\",\n", format));
            }
            if let Some(dedup_key) = params.get("dedup_key").and_then(|v| v.as_str()) {
                code.push_str(&format!("        DedupKey:   \"{}\",\n", dedup_key));
            }
            code.push_str("    })\n}\n");
            Ok(code)
        }
        Language::Java => {
            let mut code = format!(
                "public IngestResult ingest{coll_cap}(MongoClient client, String dbName, String collectionName, String filePath) {{\n",
                coll_cap = super::crud_gen::capitalize(coll),
            );
            code.push_str("    return client.ingest(IngestOptions.builder()\n");
            code.push_str("        .filePath(filePath)\n");
            code.push_str("        .database(dbName)\n");
            code.push_str("        .collection(collectionName)\n");
            if let Some(format) = params.get("format").and_then(|v| v.as_str()) {
                code.push_str(&format!("        .format(\"{}\")\n", format));
            }
            if let Some(dedup_key) = params.get("dedup_key").and_then(|v| v.as_str()) {
                code.push_str(&format!("        .dedupKey(\"{}\")\n", dedup_key));
            }
            code.push_str("        .build());\n}\n");
            Ok(code)
        }
    }
}

fn generate_watch(language: Language, params: &Value) -> Result<String, String> {
    let db = get_str_param(params, "database");
    let coll = get_str_param(params, "collection");
    let path = get_str_param(params, "path");

    match language {
        Language::Python => {
            let mut code = format!(
                "async def watch_{coll}(\n    client,\n    db_name: str = \"{db}\",\n    collection_name: str = \"{coll}\",\n    path: str = \"{path}\",\n",
                coll = coll, db = db, path = path,
            );
            if let Some(pattern) = params.get("pattern").and_then(|v| v.as_str()) {
                code.push_str(&format!("    pattern: str = \"{}\",\n", pattern));
            }
            code.push_str(") -> dict:\n");
            code.push_str("    return await client.watch_directory(\n");
            code.push_str("        path=path,\n");
            code.push_str("        database=db_name,\n");
            code.push_str("        collection=collection_name,\n");
            if params.get("pattern").is_some() {
                code.push_str("        pattern=pattern,\n");
            }
            code.push_str("    )\n");
            Ok(code)
        }
        Language::TypeScript => Ok(format!(
            "async function watch_{coll}(\n  client: MongoCore,\n  dbName = \"{db}\",\n  collectionName = \"{coll}\",\n  path = \"{path}\",\n) {{\n  return await client.watchDirectory({{ path, database: dbName, collection: collectionName }});\n}}\n",
            coll = coll, db = db, path = path,
        )),
        Language::Go => Ok(format!(
            "func watch_{coll}(client *mongocore.Client, dbName string, collectionName string, path string) (*WatchResult, error) {{\n    return client.WatchDirectory(&WatchOptions{{\n        Path:       path,\n        Database:   dbName,\n        Collection: collectionName,\n    }})\n}}\n",
            coll = coll,
        )),
        Language::Java => Ok(format!(
            "public WatchResult watch{coll_cap}(MongoClient client, String dbName, String collectionName, String path) {{\n    return client.watchDirectory(WatchOptions.builder()\n        .path(path)\n        .database(dbName)\n        .collection(collectionName)\n        .build());\n}}\n",
            coll_cap = super::crud_gen::capitalize(coll),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_ingest_python_with_options() {
        let params = json!({
            "database": "mydb",
            "collection": "contacts",
            "file_path": "https://example.com/data.csv",
            "format": "csv",
            "dedup_key": "email",
            "conflict_strategy": "upsert"
        });
        let code = generate_ingest_code(Language::Python, "ingest", &params).unwrap();
        assert!(code.contains("async def ingest_contacts("));
        assert!(code.contains("file_path: str = \"https://example.com/data.csv\""));
        assert!(code.contains("dedup_key: str = \"email\""));
        assert!(code.contains("conflict_strategy: str = \"upsert\""));
        assert!(code.contains("client.ingest("));
    }

    #[test]
    fn test_ingest_typescript() {
        let params = json!({
            "database": "mydb",
            "collection": "products",
            "file_path": "/data/products.json",
            "format": "json"
        });
        let code = generate_ingest_code(Language::TypeScript, "ingest", &params).unwrap();
        assert!(code.contains("async function ingest_products("));
        assert!(code.contains("client.ingest("));
    }

    #[test]
    fn test_watch_go() {
        let params = json!({
            "database": "mydb",
            "collection": "logs",
            "path": "/var/data/incoming",
            "pattern": "*.csv"
        });
        let code = generate_ingest_code(Language::Go, "watch_directory", &params).unwrap();
        assert!(code.contains("func watch_logs("));
        assert!(code.contains("client.WatchDirectory("));
    }
}
```

- [ ] **Step 2: Register in codegen mod.rs**

Add to `src/mcp/codegen/mod.rs`:

```rust
pub mod ingest_gen;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib ingest_gen`
Expected: All 3 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/mcp/codegen/ingest_gen.rs src/mcp/codegen/mod.rs
git commit -m "feat(mcp): add ingest code generation module"
```

---

## Task 7: Search Code Generation Module

**Files:**
- Create: `src/mcp/codegen/search_gen.rs`
- Modify: `src/mcp/codegen/mod.rs`

- [ ] **Step 1: Write search codegen with tests**

Create `src/mcp/codegen/search_gen.rs`:

```rust
use serde_json::Value;
use super::Language;

/// Generate MongoCore client code for search/embedding operations.
/// Note: embed_and_store and semantic_search are currently MCP-only
/// (not available in client libraries), so generated code includes
/// a comment noting this limitation.
pub fn generate_search_code(
    language: Language,
    tool_name: &str,
    params: &Value,
) -> Result<String, String> {
    match tool_name {
        "embed_and_store" => generate_embed(language, params),
        "semantic_search" => generate_semantic_search(language, params),
        _ => Err(format!("Unsupported search operation: {}", tool_name)),
    }
}

fn get_str_param<'a>(params: &'a Value, key: &str) -> &'a str {
    params.get(key).and_then(|v| v.as_str()).unwrap_or("unknown")
}

fn generate_embed(language: Language, params: &Value) -> Result<String, String> {
    let db = get_str_param(params, "database");
    let coll = get_str_param(params, "collection");
    let embed_field = get_str_param(params, "embed_field");

    let comment = "NOTE: embed_and_store is currently MCP-only. Client library support pending.";

    match language {
        Language::Python => Ok(format!(
            "# {comment}\n# To use via MCP, call the embed_and_store tool with:\n#   database=\"{db}\", collection=\"{coll}\", embed_field=\"{embed_field}\"\n#   documents=[...]\n",
            comment = comment, db = db, coll = coll, embed_field = embed_field,
        )),
        Language::TypeScript => Ok(format!(
            "// {comment}\n// To use via MCP, call the embed_and_store tool with:\n//   database: \"{db}\", collection: \"{coll}\", embed_field: \"{embed_field}\"\n//   documents: [...]\n",
            comment = comment, db = db, coll = coll, embed_field = embed_field,
        )),
        Language::Go => Ok(format!(
            "// {comment}\n// To use via MCP, call the embed_and_store tool with:\n//   database=\"{db}\", collection=\"{coll}\", embed_field=\"{embed_field}\"\n//   documents=[...]\n",
            comment = comment, db = db, coll = coll, embed_field = embed_field,
        )),
        Language::Java => Ok(format!(
            "// {comment}\n// To use via MCP, call the embed_and_store tool with:\n//   database=\"{db}\", collection=\"{coll}\", embed_field=\"{embed_field}\"\n//   documents=[...]\n",
            comment = comment, db = db, coll = coll, embed_field = embed_field,
        )),
    }
}

fn generate_semantic_search(language: Language, params: &Value) -> Result<String, String> {
    let db = get_str_param(params, "database");
    let coll = get_str_param(params, "collection");
    let query = get_str_param(params, "query");

    let comment = "NOTE: semantic_search is currently MCP-only. Client library support pending.";

    match language {
        Language::Python => Ok(format!(
            "# {comment}\n# To use via MCP, call the semantic_search tool with:\n#   database=\"{db}\", collection=\"{coll}\", query=\"{query}\"\n",
            comment = comment, db = db, coll = coll, query = query,
        )),
        Language::TypeScript => Ok(format!(
            "// {comment}\n// To use via MCP, call the semantic_search tool with:\n//   database: \"{db}\", collection: \"{coll}\", query: \"{query}\"\n",
            comment = comment, db = db, coll = coll, query = query,
        )),
        Language::Go => Ok(format!(
            "// {comment}\n// To use via MCP, call the semantic_search tool with:\n//   database=\"{db}\", collection=\"{coll}\", query=\"{query}\"\n",
            comment = comment, db = db, coll = coll, query = query,
        )),
        Language::Java => Ok(format!(
            "// {comment}\n// To use via MCP, call the semantic_search tool with:\n//   database=\"{db}\", collection=\"{coll}\", query=\"{query}\"\n",
            comment = comment, db = db, coll = coll, query = query,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_embed_generates_comment() {
        let params = json!({
            "database": "mydb",
            "collection": "articles",
            "embed_field": "content",
            "documents": [{"content": "hello"}]
        });
        let code = generate_search_code(Language::Python, "embed_and_store", &params).unwrap();
        assert!(code.contains("MCP-only"));
        assert!(code.contains("embed_field=\"content\""));
    }

    #[test]
    fn test_semantic_search_generates_comment() {
        let params = json!({
            "database": "mydb",
            "collection": "articles",
            "query": "machine learning papers"
        });
        let code = generate_search_code(Language::TypeScript, "semantic_search", &params).unwrap();
        assert!(code.contains("MCP-only"));
        assert!(code.contains("query: \"machine learning papers\""));
    }
}
```

- [ ] **Step 2: Register in codegen mod.rs**

Add to `src/mcp/codegen/mod.rs`:

```rust
pub mod search_gen;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib search_gen`
Expected: All 2 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/mcp/codegen/search_gen.rs src/mcp/codegen/mod.rs
git commit -m "feat(mcp): add search code generation module (MCP-only comment stubs)"
```

---

## Task 8: Session Code Generation Module

**Files:**
- Create: `src/mcp/codegen/session_gen.rs`
- Modify: `src/mcp/codegen/mod.rs`

- [ ] **Step 1: Write session stitching codegen with tests**

Create `src/mcp/codegen/session_gen.rs`:

```rust
use serde_json::Value;
use super::Language;
use crate::mcp::session::OperationRecord;
use std::collections::HashMap;

/// Generate a complete session script from a list of operation records.
pub fn generate_session_script(
    language: Language,
    operations: &[OperationRecord],
) -> Result<String, String> {
    if operations.is_empty() {
        return Err("No operations recorded in session".to_string());
    }

    let mut functions = Vec::new();
    let mut function_names = Vec::new();
    let mut name_counts: HashMap<String, usize> = HashMap::new();

    for op in operations {
        let base_name = derive_function_name(&op.tool_name, &op.params);
        let count = name_counts.entry(base_name.clone()).or_insert(0);
        *count += 1;
        let name = if *count > 1 {
            format!("{}_{}", base_name, count)
        } else {
            base_name
        };

        let docstring = build_docstring(&op.tool_name, &op.context);

        if op.success {
            let code = generate_single_operation(language, &name, &docstring, &op.tool_name, &op.params)?;
            function_names.push(name);
            functions.push(code);
        } else {
            let commented = generate_failed_operation(language, &name, &docstring, &op.tool_name, &op.params, op.error_message.as_deref())?;
            functions.push(commented);
            // Don't add to function_names (not called in main)
        }
    }

    let script = assemble_script(language, &functions, &function_names)?;
    Ok(script)
}

/// Generate code for a single operation (used by explain_last).
pub fn generate_single_operation_code(
    language: Language,
    tool_name: &str,
    params: &Value,
) -> Result<String, String> {
    let name = derive_function_name(tool_name, params);
    let docstring = ""; // No docstring for explain_last
    generate_single_operation(language, &name, docstring, tool_name, params)
}

fn derive_function_name(tool_name: &str, params: &Value) -> String {
    let collection = params
        .get("collection")
        .and_then(|v| v.as_str())
        .unwrap_or("data");
    format!("{}_{}", tool_name, collection)
}

fn build_docstring(tool_name: &str, context: &Value) -> String {
    let db = context.get("database").and_then(|v| v.as_str()).unwrap_or("unknown");
    let coll = context.get("collection").and_then(|v| v.as_str());
    match coll {
        Some(c) => format!("{} on {} in {}", tool_name, c, db),
        None => format!("{} in {}", tool_name, db),
    }
}

fn generate_single_operation(
    language: Language,
    name: &str,
    docstring: &str,
    tool_name: &str,
    params: &Value,
) -> Result<String, String> {
    // Route to appropriate codegen module
    match tool_name {
        "find" | "find_one" | "insert" | "insert_many" | "update" | "update_many" | "delete" | "delete_many" => {
            super::crud_gen::generate_crud_code(language, tool_name, params)
        }
        "ingest" | "watch_directory" => {
            super::ingest_gen::generate_ingest_code(language, tool_name, params)
        }
        "embed_and_store" | "semantic_search" => {
            super::search_gen::generate_search_code(language, tool_name, params)
        }
        "aggregate" => {
            // Use existing query_gen for aggregation
            let db = params.get("database").and_then(|v| v.as_str()).unwrap_or("unknown");
            let coll = params.get("collection").and_then(|v| v.as_str()).unwrap_or("unknown");
            let mql = serde_json::json!({"pipeline": params.get("pipeline").unwrap_or(&Value::Null)});
            super::generate_query_code(language, db, coll, "aggregate", &mql, "localhost:27017")
        }
        "run_command" => generate_run_command(language, params),
        "list_collections" => generate_list_collections(language, params),
        "list_databases" => generate_list_databases(language, params),
        "create_index" => {
            let db = params.get("database").and_then(|v| v.as_str()).unwrap_or("unknown");
            let coll = params.get("collection").and_then(|v| v.as_str()).unwrap_or("unknown");
            let keys = params.get("keys").unwrap_or(&Value::Null);
            let suggestion = super::suggest_index(language, db, coll, keys);
            Ok(suggestion.code)
        }
        "pipeline" | "transaction_pipeline" => generate_pipeline_code(language, tool_name, params),
        _ => {
            // Fallback: generate a comment with the tool name and params
            let comment_char = match language {
                Language::Python => "#",
                _ => "//",
            };
            Ok(format!(
                "{} TODO: Code generation not yet supported for '{}'\n{} Params: {}\n",
                comment_char, tool_name, comment_char,
                serde_json::to_string_pretty(params).unwrap_or_default()
            ))
        }
    }
}

fn generate_failed_operation(
    language: Language,
    name: &str,
    _docstring: &str,
    tool_name: &str,
    params: &Value,
    error: Option<&str>,
) -> Result<String, String> {
    let error_msg = error.unwrap_or("unknown error");
    let comment_char = match language {
        Language::Python => "#",
        _ => "//",
    };
    let mut output = format!("{} FAILED: {}\n", comment_char, error_msg);
    // Generate the code but comment it out
    let code = generate_single_operation(language, name, "", tool_name, params)
        .unwrap_or_else(|_| format!("{} (could not generate code)\n", comment_char));
    for line in code.lines() {
        output.push_str(&format!("{} {}\n", comment_char, line));
    }
    output.push('\n');
    Ok(output)
}

fn generate_run_command(language: Language, params: &Value) -> Result<String, String> {
    let db = params.get("database").and_then(|v| v.as_str()).unwrap_or("admin");
    let command = params.get("command").unwrap_or(&Value::Null);
    let command_json = serde_json::to_string(command).unwrap_or_else(|_| "{}".to_string());

    match language {
        Language::Python => Ok(format!(
            "async def run_command(\n    client,\n    db_name: str = \"{db}\",\n    command: dict = {cmd},\n) -> dict:\n    return await client.run_command(db_name, command)\n",
            db = db, cmd = command_json,
        )),
        Language::TypeScript => Ok(format!(
            "async function runCommand(\n  client: MongoCore,\n  dbName = \"{db}\",\n  command = {cmd},\n) {{\n  return await client.runCommand(dbName, command);\n}}\n",
            db = db, cmd = command_json,
        )),
        Language::Go => Ok(format!(
            "func runCommand(client *mongocore.Client, dbName string, command interface{{}}) (bson.M, error) {{\n    return client.RunCommand(dbName, command)\n}}\n",
        )),
        Language::Java => Ok(format!(
            "public Document runCommand(MongoClient client, String dbName, Document command) {{\n    return client.runCommand(dbName, command);\n}}\n",
        )),
    }
}

fn generate_list_collections(language: Language, params: &Value) -> Result<String, String> {
    let db = params.get("database").and_then(|v| v.as_str()).unwrap_or("unknown");

    match language {
        Language::Python => Ok(format!(
            "async def list_collections(\n    client,\n    db_name: str = \"{db}\",\n) -> list:\n    db = client.database(db_name)\n    return await db.list_collections()\n",
            db = db,
        )),
        Language::TypeScript => Ok(format!(
            "async function listCollections(\n  client: MongoCore,\n  dbName = \"{db}\",\n) {{\n  const db = client.database(dbName);\n  return await db.listCollections();\n}}\n",
            db = db,
        )),
        Language::Go => Ok(format!(
            "func listCollections(client *mongocore.Client, dbName string) ([]string, error) {{\n    db := client.Database(dbName)\n    return db.ListCollections()\n}}\n",
        )),
        Language::Java => Ok(format!(
            "public List<String> listCollections(MongoClient client, String dbName) {{\n    MongoDatabase db = client.database(dbName);\n    return db.listCollections();\n}}\n",
        )),
    }
}

fn generate_list_databases(language: Language, _params: &Value) -> Result<String, String> {
    match language {
        Language::Python => Ok(
            "async def list_databases(\n    client,\n) -> list:\n    return await client.list_databases()\n".to_string(),
        ),
        Language::TypeScript => Ok(
            "async function listDatabases(\n  client: MongoCore,\n) {\n  return await client.listDatabases();\n}\n".to_string(),
        ),
        Language::Go => Ok(
            "func listDatabases(client *mongocore.Client) ([]string, error) {\n    return client.ListDatabases()\n}\n".to_string(),
        ),
        Language::Java => Ok(
            "public List<String> listDatabases(MongoClient client) {\n    return client.listDatabases();\n}\n".to_string(),
        ),
    }
}

fn generate_pipeline_code(language: Language, tool_name: &str, params: &Value) -> Result<String, String> {
    let steps = params.get("steps").and_then(|s| s.as_array());
    let steps_json = serde_json::to_string_pretty(params.get("steps").unwrap_or(&Value::Null))
        .unwrap_or_else(|_| "[]".to_string());
    let method = if tool_name == "transaction_pipeline" { "transaction_pipeline" } else { "pipeline" };

    match language {
        Language::Python => Ok(format!(
            "async def run_{method}(\n    client,\n    steps = {steps},\n) -> dict:\n    return await client.{method}(steps)\n",
            method = method, steps = steps_json,
        )),
        Language::TypeScript => Ok(format!(
            "async function run{method_cap}(\n  client: MongoCore,\n  steps = {steps},\n) {{\n  return await client.{method}(steps);\n}}\n",
            method_cap = super::crud_gen::capitalize(method), method = method, steps = steps_json,
        )),
        Language::Go => Ok(format!(
            "func run{method_cap}(client *mongocore.Client, steps []Step) (*PipelineResult, error) {{\n    return client.{}(steps)\n}}\n",
            super::crud_gen::capitalize(method), method_cap = super::crud_gen::capitalize(method),
        )),
        Language::Java => Ok(format!(
            "public PipelineResult run{method_cap}(MongoClient client, List<Step> steps) {{\n    return client.{method}(steps);\n}}\n",
            method_cap = super::crud_gen::capitalize(method), method = method,
        )),
    }
}

fn assemble_script(
    language: Language,
    functions: &[String],
    called_function_names: &[String],
) -> Result<String, String> {
    let mut script = String::new();

    match language {
        Language::Python => {
            script.push_str("from mongocore import MongoCore\n\n\n");
            for func in functions {
                script.push_str(func);
                script.push_str("\n\n");
            }
            script.push_str("async def main():\n");
            script.push_str("    client = MongoCore()\n");
            for name in called_function_names {
                script.push_str(&format!("    await {}(client)\n", name));
            }
            script.push_str("\n\nif __name__ == \"__main__\":\n");
            script.push_str("    import asyncio\n");
            script.push_str("    asyncio.run(main())\n");
        }
        Language::TypeScript => {
            script.push_str("import { MongoCore } from 'mongocore';\n\n\n");
            for func in functions {
                script.push_str(func);
                script.push_str("\n\n");
            }
            script.push_str("async function main() {\n");
            script.push_str("  const client = new MongoCore();\n");
            for name in called_function_names {
                script.push_str(&format!("  await {}(client);\n", name));
            }
            script.push_str("}\n\nmain();\n");
        }
        Language::Go => {
            script.push_str("package main\n\nimport (\n    \"github.com/mongocore/mongocore-go/mongocore\"\n)\n\n");
            for func in functions {
                script.push_str(func);
                script.push_str("\n\n");
            }
            script.push_str("func main() {\n");
            script.push_str("    client, _ := mongocore.NewClient()\n");
            for name in called_function_names {
                script.push_str(&format!("    {}, _ := {}(client)\n", "_", name));
            }
            script.push_str("}\n");
        }
        Language::Java => {
            script.push_str("import com.mongocore.MongoClient;\nimport com.mongocore.MongoDatabase;\nimport com.mongocore.MongoCollection;\n\n");
            script.push_str("public class SessionScript {\n\n");
            for func in functions {
                // Indent Java methods
                for line in func.lines() {
                    script.push_str(&format!("    {}\n", line));
                }
                script.push('\n');
            }
            script.push_str("    public static void main(String[] args) {\n");
            script.push_str("        MongoClient client = new MongoClient();\n");
            script.push_str("        SessionScript script = new SessionScript();\n");
            for name in called_function_names {
                script.push_str(&format!("        script.{}(client);\n", name));
            }
            script.push_str("    }\n}\n");
        }
    }

    Ok(script)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use chrono::Utc;

    fn make_record(tool_name: &str, params: Value, success: bool, error: Option<&str>) -> OperationRecord {
        OperationRecord {
            index: 0,
            tool_name: tool_name.to_string(),
            params: params.clone(),
            context: json!({"operation": tool_name}),
            success,
            error_message: error.map(|s| s.to_string()),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_session_script_python() {
        let ops = vec![
            make_record("insert_many", json!({"database": "mydb", "collection": "users", "documents": []}), true, None),
            make_record("find", json!({"database": "mydb", "collection": "users", "filter": {"active": true}}), true, None),
        ];
        let script = generate_session_script(Language::Python, &ops).unwrap();
        assert!(script.contains("from mongocore import MongoCore"));
        assert!(script.contains("async def insert_many_users("));
        assert!(script.contains("async def find_users("));
        assert!(script.contains("async def main():"));
        assert!(script.contains("await insert_many_users(client)"));
        assert!(script.contains("await find_users(client)"));
        assert!(script.contains("asyncio.run(main())"));
    }

    #[test]
    fn test_failed_operation_commented_out() {
        let ops = vec![
            make_record("insert_many", json!({"database": "mydb", "collection": "users", "documents": []}), true, None),
            make_record("delete_many", json!({"database": "mydb", "collection": "users", "filter": {}}), false, Some("permission denied")),
        ];
        let script = generate_session_script(Language::Python, &ops).unwrap();
        assert!(script.contains("# FAILED: permission denied"));
        // Failed op should NOT be in main()
        assert!(!script.contains("await delete_many_users(client)"));
        // But successful one should be
        assert!(script.contains("await insert_many_users(client)"));
    }

    #[test]
    fn test_empty_session_error() {
        let result = generate_session_script(Language::Python, &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No operations"));
    }

    #[test]
    fn test_duplicate_names_get_suffix() {
        let ops = vec![
            make_record("find", json!({"database": "mydb", "collection": "users", "filter": {"a": 1}}), true, None),
            make_record("find", json!({"database": "mydb", "collection": "users", "filter": {"b": 2}}), true, None),
        ];
        let script = generate_session_script(Language::Python, &ops).unwrap();
        assert!(script.contains("find_users"));
        assert!(script.contains("find_users_2"));
    }

    #[test]
    fn test_typescript_structure() {
        let ops = vec![
            make_record("insert_many", json!({"database": "db", "collection": "items", "documents": []}), true, None),
        ];
        let script = generate_session_script(Language::TypeScript, &ops).unwrap();
        assert!(script.contains("import { MongoCore } from 'mongocore'"));
        assert!(script.contains("async function main()"));
        assert!(script.contains("const client = new MongoCore()"));
        assert!(script.contains("main();"));
    }
}
```

- [ ] **Step 2: Register in codegen mod.rs**

Add to `src/mcp/codegen/mod.rs`:

```rust
pub mod session_gen;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib session_gen`
Expected: All 5 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/mcp/codegen/session_gen.rs src/mcp/codegen/mod.rs
git commit -m "feat(mcp): add session code generation module for explain tools"
```

---

## Task 9: Implement explain_last and explain_session Tools

**Files:**
- Modify: `src/mcp/tools.rs`

- [ ] **Step 1: Add tool definitions for explain_last and explain_session**

In the `tools_list` function (where all tool definitions are declared), add:

```rust
json!({
    "name": "explain_last",
    "description": "Generate reusable MongoCore client code for a recent operation. Produces a parameterized function in the specified language.",
    "inputSchema": {
        "type": "object",
        "properties": {
            "language": {
                "type": "string",
                "enum": ["python", "typescript", "go", "java"],
                "description": "Target programming language"
            },
            "offset": {
                "type": "integer",
                "description": "How many operations back (0 = most recent)",
                "default": 0
            }
        },
        "required": ["language"]
    }
}),
json!({
    "name": "explain_session",
    "description": "Generate a complete MongoCore client script reproducing all operations performed in this session. Produces parameterized functions with a main entry point.",
    "inputSchema": {
        "type": "object",
        "properties": {
            "language": {
                "type": "string",
                "enum": ["python", "typescript", "go", "java"],
                "description": "Target programming language"
            }
        },
        "required": ["language"]
    }
}),
```

- [ ] **Step 2: Implement explain_last handler**

Add the handler function:

```rust
use crate::mcp::session::SessionRecorder;
use crate::mcp::codegen::{Language, session_gen};
use std::sync::Mutex;

fn parse_language(args: &Value) -> Result<Language, McpToolCallResult> {
    match args.get("language").and_then(|v| v.as_str()) {
        Some("python") => Ok(Language::Python),
        Some("typescript") => Ok(Language::TypeScript),
        Some("go") => Ok(Language::Go),
        Some("java") => Ok(Language::Java),
        Some(other) => Err(error_result(format!("Unsupported language: {}. Use python, typescript, go, or java.", other))),
        None => Err(error_result("Missing required field: language".to_string())),
    }
}

fn execute_explain_last(session: &Arc<Mutex<SessionRecorder>>, args: &Value) -> McpToolCallResult {
    let language = match parse_language(args) {
        Ok(l) => l,
        Err(e) => return e,
    };
    let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

    let session_guard = match session.lock() {
        Ok(s) => s,
        Err(_) => return error_result("Failed to access session state".to_string()),
    };

    if session_guard.is_empty() {
        return error_result("No operations recorded in this session yet.".to_string());
    }

    let record = match session_guard.get_last(offset) {
        Some(r) => r,
        None => return error_result(format!(
            "Offset {} is out of bounds. Session has {} operations.",
            offset, session_guard.len()
        )),
    };

    match session_gen::generate_single_operation_code(language, &record.tool_name, &record.params) {
        Ok(code) => {
            let result = json!({
                "code": code,
                "language": args.get("language").unwrap_or(&Value::Null),
                "operation": record.tool_name,
            });
            success_result(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
        Err(e) => error_result(format!("Code generation failed: {}", e)),
    }
}
```

- [ ] **Step 3: Implement explain_session handler**

```rust
fn execute_explain_session(session: &Arc<Mutex<SessionRecorder>>, args: &Value) -> McpToolCallResult {
    let language = match parse_language(args) {
        Ok(l) => l,
        Err(e) => return e,
    };

    let session_guard = match session.lock() {
        Ok(s) => s,
        Err(_) => return error_result("Failed to access session state".to_string()),
    };

    if session_guard.is_empty() {
        return error_result("No operations recorded in this session yet.".to_string());
    }

    let operations = session_guard.get_all();

    match session_gen::generate_session_script(language, operations) {
        Ok(code) => {
            let op_names: Vec<&str> = operations.iter().map(|o| o.tool_name.as_str()).collect();
            let result = json!({
                "code": code,
                "language": args.get("language").unwrap_or(&Value::Null),
                "operation_count": operations.len(),
                "operations": op_names,
            });
            success_result(serde_json::to_string_pretty(&result).unwrap_or_default())
        }
        Err(e) => error_result(format!("Code generation failed: {}", e)),
    }
}
```

- [ ] **Step 4: Wire into the tool dispatch**

In `execute_tool_inner`, add the match arms:

```rust
"explain_last" => execute_explain_last(session, arguments),
"explain_session" => execute_explain_session(session, arguments),
```

- [ ] **Step 5: Verify build and tests**

Run: `cargo build 2>&1 | grep "warning:"` then `cargo test --lib`
Expected: Zero warnings, all tests pass

- [ ] **Step 6: Commit**

```bash
git add src/mcp/tools.rs
git commit -m "feat(mcp): implement explain_last and explain_session tools"
```

---

## Task 10: Update MCP Tool Count and Integration Tests

**Files:**
- Modify: `tests/integration/mcp_test.rs`

- [ ] **Step 1: Update the tool count assertion**

Find the assertion that checks the number of MCP tools and increment it by 2 (for `explain_last` and `explain_session`).

- [ ] **Step 2: Add integration test for explain_last**

```rust
#[tokio::test]
async fn test_explain_last() {
    // 1. Execute an insert_many operation
    let insert_result = call_tool("insert_many", json!({
        "database": "test_explain",
        "collection": "users",
        "documents": [{"name": "Alice", "age": 30}]
    })).await;
    assert!(!insert_result.is_error);

    // 2. Call explain_last
    let explain_result = call_tool("explain_last", json!({
        "language": "python"
    })).await;
    assert!(!explain_result.is_error);

    let response: Value = serde_json::from_str(&explain_result.content[0].text).unwrap();
    assert_eq!(response["language"], "python");
    assert_eq!(response["operation"], "insert_many");
    let code = response["code"].as_str().unwrap();
    assert!(code.contains("async def"));
    assert!(code.contains("insert_many"));
    assert!(code.contains("users"));
}
```

- [ ] **Step 3: Add integration test for explain_session**

```rust
#[tokio::test]
async fn test_explain_session() {
    // 1. Execute a few operations
    call_tool("insert_many", json!({
        "database": "test_explain_session",
        "collection": "items",
        "documents": [{"name": "Widget"}]
    })).await;

    call_tool("find", json!({
        "database": "test_explain_session",
        "collection": "items",
        "filter": {"name": "Widget"}
    })).await;

    // 2. Call explain_session
    let result = call_tool("explain_session", json!({
        "language": "typescript"
    })).await;
    assert!(!result.is_error);

    let response: Value = serde_json::from_str(&result.content[0].text).unwrap();
    assert_eq!(response["language"], "typescript");
    assert!(response["operation_count"].as_u64().unwrap() >= 2);
    let code = response["code"].as_str().unwrap();
    assert!(code.contains("import { MongoCore }"));
    assert!(code.contains("async function main()"));
}
```

- [ ] **Step 4: Add integration test for _context in responses**

```rust
#[tokio::test]
async fn test_response_contains_context() {
    let result = call_tool("find", json!({
        "database": "test_context",
        "collection": "docs",
        "filter": {"status": "active"},
        "limit": 5
    })).await;
    assert!(!result.is_error);

    let response: Value = serde_json::from_str(&result.content[0].text).unwrap();
    let context = &response["_context"];
    assert_eq!(context["operation"], "find");
    assert_eq!(context["database"], "test_context");
    assert_eq!(context["collection"], "docs");
    assert_eq!(context["filter"]["status"], "active");
    assert_eq!(context["limit"], 5);
}
```

- [ ] **Step 5: Verify integration tests compile**

Run: `cargo test --test integration --no-run`
Expected: Compiles without errors

- [ ] **Step 6: Commit**

```bash
git add tests/integration/mcp_test.rs
git commit -m "test(mcp): add integration tests for explain tools and _context"
```

---

## Task 11: Documentation

**Files:**
- Create: `docs/explain.md`
- Modify: `docs/README.md` (add entry for explain.md)

- [ ] **Step 1: Write user-facing documentation**

Create `docs/explain.md`:

```markdown
# Operation Explain

MongoCore's MCP interface includes built-in operation explanation — every response tells you what happened, and two tools generate reusable client code from your session.

## Response Context

Every MCP tool response includes a `_context` field with the key parameters of the operation:

```json
{
  "insertedCount": 500,
  "_context": {
    "operation": "insert_many",
    "database": "analytics",
    "collection": "events",
    "document_count": 500,
    "document_schema": {"event_type": "string", "timestamp": "string", "payload": "object"}
  }
}
```

This makes responses self-contained — you can understand what happened without scrolling back to the request.

## explain_last

Generate MongoCore client code for the most recent operation (or Nth most recent).

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| language | string | Yes | `python`, `typescript`, `go`, or `java` |
| offset | integer | No | How many operations back (default: 0 = most recent) |

**Example:**

```json
{"tool": "explain_last", "arguments": {"language": "python"}}
```

**Response:**

```json
{
  "code": "async def insert_many_events(\n    client,\n    db_name: str = \"analytics\",\n    ...\n) -> dict:\n    ...",
  "language": "python",
  "operation": "insert_many"
}
```

## explain_session

Generate a complete script reproducing all operations from the current session.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| language | string | Yes | `python`, `typescript`, `go`, or `java` |

**Example:**

```json
{"tool": "explain_session", "arguments": {"language": "python"}}
```

**Response:**

```json
{
  "code": "from mongocore import MongoCore\n\nasync def ingest_contacts(...):\n    ...\n\nasync def find_contacts(...):\n    ...\n\nasync def main():\n    client = MongoCore()\n    await ingest_contacts(client)\n    await find_contacts(client)\n\nif __name__ == '__main__':\n    import asyncio\n    asyncio.run(main())\n",
  "language": "python",
  "operation_count": 2,
  "operations": ["ingest", "find"]
}
```

## Supported Languages

| Language | Import | Client Init |
|----------|--------|-------------|
| Python | `from mongocore import MongoCore` | `MongoCore()` |
| TypeScript | `import { MongoCore } from 'mongocore'` | `new MongoCore()` |
| Go | `"github.com/mongocore/mongocore-go/mongocore"` | `mongocore.NewClient()` |
| Java | `import com.mongocore.MongoClient` | `new MongoClient()` |

## Session Scope

- Operations are recorded per MCP connection (stdio or SSE session)
- History is in-memory only — cleared when the connection drops
- Diagnostic tools (`get_analytics`, `collection_schema`) are not recorded
- Failed operations appear as commented-out code with the error reason

## Notes

- Generated code produces parameterized functions with defaults matching the actual values used
- Functions are named `{operation}_{collection}` (e.g., `insert_many_users`)
- `embed_and_store` and `semantic_search` generate comments noting they are MCP-only (no client library support yet)
```

- [ ] **Step 2: Add to docs README**

Add an entry to the documentation index in `docs/README.md`.

- [ ] **Step 3: Commit**

```bash
git add docs/explain.md docs/README.md
git commit -m "docs: add user-facing documentation for explain feature"
```

---

## Task 12: Add Missing gRPC RPCs to Proto

The MCP tools `count_documents`, `drop_collection`, `embed_and_store`, and `semantic_search` currently exist only in the MCP layer (no gRPC RPC). Client libraries need gRPC RPCs to call these operations.

**Files:**
- Modify: `proto/mongocore/v1/mongocore.proto`
- Modify: `src/grpc/service.rs`
- Modify: `src/operations/mod.rs` (if operations don't already exist)

- [ ] **Step 1: Add proto messages and RPCs**

Add to `proto/mongocore/v1/mongocore.proto`:

```protobuf
// Count Documents
message CountDocumentsRequest {
  string database = 1;
  string collection = 2;
  string filter = 3; // JSON filter
}

message CountDocumentsResponse {
  int64 count = 1;
}

// Drop Collection
message DropCollectionRequest {
  string database = 1;
  string collection = 2;
}

message DropCollectionResponse {
  bool ok = 1;
}

// Embed and Store
message EmbedAndStoreRequest {
  string database = 1;
  string collection = 2;
  string documents = 3; // JSON array
  string embed_field = 4;
  string embedding_field = 5;
}

message EmbedAndStoreResponse {
  int64 documents_stored = 1;
  int64 embeddings_generated = 2;
  int32 embedding_dimensions = 3;
}

// Semantic Search
message SemanticSearchRequest {
  string database = 1;
  string collection = 2;
  string query = 3;
  string index_name = 4;
  int32 limit = 5;
}

message SemanticSearchResponse {
  string results = 1; // JSON array
  int64 count = 2;
}
```

Add RPCs to the service:

```protobuf
rpc CountDocuments(CountDocumentsRequest) returns (CountDocumentsResponse);
rpc DropCollection(DropCollectionRequest) returns (DropCollectionResponse);
rpc EmbedAndStore(EmbedAndStoreRequest) returns (EmbedAndStoreResponse);
rpc SemanticSearch(SemanticSearchRequest) returns (SemanticSearchResponse);
```

- [ ] **Step 2: cargo build (regenerates Rust stubs)**
- [ ] **Step 3: Implement gRPC handlers in service.rs**
- [ ] **Step 4: Regenerate client stubs (`just proto-gen`)**
- [ ] **Step 5: Verify build**

Run: `cargo build 2>&1 | grep "warning:"`

- [ ] **Step 6: Commit**

```bash
git add proto/ src/grpc/service.rs
git commit -m "feat(grpc): add CountDocuments, DropCollection, EmbedAndStore, SemanticSearch RPCs"
```

---

## Task 13: Add Missing Client Library Operations (Parallel Subagents)

These tasks add `count_documents`, `drop_collection`, `embed_and_store`, `semantic_search`, and `transaction_pipeline` (where missing) to all client libraries. Each language can be implemented by an independent subagent in parallel.

**Prerequisite:** Task 12 (proto RPCs) must be complete and stubs regenerated.

### Task 13a: Add Missing Operations to Python Client

**Files:**
- Modify: `clients/python/src/mongocore/collection.py`
- Modify: `clients/python/src/mongocore/client.py`
- Modify: `clients/python/tests/test_integration.py`

- [ ] **Step 1: Add count_documents to Collection**

```python
async def count_documents(self, filter: dict = None) -> int:
    response = self._stub.CountDocuments(
        CountDocumentsRequest(
            database=self._database_name,
            collection=self._name,
            filter=json.dumps(filter or {}),
        )
    )
    return response.count
```

- [ ] **Step 2: Add drop_collection to Database**

```python
async def drop_collection(self, name: str) -> None:
    self._stub.DropCollection(
        DropCollectionRequest(database=self._name, collection=name)
    )
```

- [ ] **Step 3: Add embed_and_store to Client**

```python
async def embed_and_store(
    self, database: str, collection: str, documents: list[dict],
    embed_field: str, embedding_field: str = "embedding",
) -> dict:
    response = self._stub.EmbedAndStore(
        EmbedAndStoreRequest(
            database=database,
            collection=collection,
            documents=json.dumps(documents),
            embed_field=embed_field,
            embedding_field=embedding_field,
        )
    )
    return {"documents_stored": response.documents_stored, "embeddings_generated": response.embeddings_generated}
```

- [ ] **Step 4: Add semantic_search to Client**

```python
async def semantic_search(
    self, database: str, collection: str, query: str,
    index_name: str = None, limit: int = 10,
) -> list[dict]:
    response = self._stub.SemanticSearch(
        SemanticSearchRequest(
            database=database,
            collection=collection,
            query=query,
            index_name=index_name or "",
            limit=limit,
        )
    )
    return json.loads(response.results)
```

- [ ] **Step 5: Add integration tests for new operations**
- [ ] **Step 6: Verify with `just test-clients`**
- [ ] **Step 7: Commit**

```bash
git add clients/python/
git commit -m "feat(clients): add count_documents, drop_collection, embed_and_store, semantic_search to Python client"
```

### Task 13b: Add Missing Operations to TypeScript Client

**Files:**
- Modify: `clients/typescript/src/collection.ts`
- Modify: `clients/typescript/src/client.ts`
- Modify: `clients/typescript/src/database.ts`
- Modify: `clients/typescript/tests/integration.test.ts`

Follow the same pattern as Python but with TypeScript idioms (async/await, proper typing).

- [ ] **Step 1: Add count_documents to Collection class**
- [ ] **Step 2: Add drop_collection (via dropCollection) to Database class**
- [ ] **Step 3: Add embedAndStore to Client class**
- [ ] **Step 4: Add semanticSearch to Client class**
- [ ] **Step 5: Add integration tests**
- [ ] **Step 6: Verify with `just test-clients`**
- [ ] **Step 7: Commit**

```bash
git add clients/typescript/
git commit -m "feat(clients): add countDocuments, dropCollection, embedAndStore, semanticSearch to TypeScript client"
```

### Task 13c: Add Missing Operations to Go Client

**Files:**
- Modify: `clients/go/mongocore/collection.go`
- Modify: `clients/go/mongocore/client.go`
- Modify: `clients/go/mongocore/database.go`
- Create: `clients/go/mongocore/transaction_pipeline.go` (if not already present)
- Modify: `clients/go/tests/integration_test.go`

Follow Go conventions (exported methods, error returns, no exceptions).

- [ ] **Step 1: Add CountDocuments to Collection**
- [ ] **Step 2: Add DropCollection to Database**
- [ ] **Step 3: Add EmbedAndStore to Client**
- [ ] **Step 4: Add SemanticSearch to Client**
- [ ] **Step 5: Add TransactionPipeline to Client**
- [ ] **Step 6: Add integration tests**
- [ ] **Step 7: Verify with `just test-clients`**
- [ ] **Step 8: Commit**

```bash
git add clients/go/
git commit -m "feat(clients): add CountDocuments, DropCollection, EmbedAndStore, SemanticSearch, TransactionPipeline to Go client"
```

### Task 13d: Add Missing Operations to Java Client

**Files:**
- Modify: `clients/java/src/main/java/com/mongocore/MongoCollection.java`
- Modify: `clients/java/src/main/java/com/mongocore/MongoClient.java`
- Modify: `clients/java/src/main/java/com/mongocore/MongoDatabase.java`
- Modify: `clients/java/src/test/java/com/mongocore/IntegrationTest.java`

Follow Java conventions (method overloading, builders where appropriate).

- [ ] **Step 1: Add countDocuments to MongoCollection**
- [ ] **Step 2: Add dropCollection to MongoDatabase**
- [ ] **Step 3: Add embedAndStore to MongoClient**
- [ ] **Step 4: Add semanticSearch to MongoClient**
- [ ] **Step 5: Implement transactionPipeline (currently throws UnsupportedOperationException)**
- [ ] **Step 6: Add integration tests**
- [ ] **Step 7: Verify with `just test-clients`**
- [ ] **Step 8: Commit**

```bash
git add clients/java/
git commit -m "feat(clients): add countDocuments, dropCollection, embedAndStore, semanticSearch, transactionPipeline to Java client"
```

---

## Task 14: Update Search Codegen to Generate Real Client Code

**Files:**
- Modify: `src/mcp/codegen/search_gen.rs`

After Task 12 completes and client libraries have the methods, update search_gen to generate actual client method calls instead of MCP-only comments.

- [ ] **Step 1: Replace comment stubs with real client code**

```rust
fn generate_embed(language: Language, params: &Value) -> Result<String, String> {
    let db = get_str_param(params, "database");
    let coll = get_str_param(params, "collection");
    let embed_field = get_str_param(params, "embed_field");

    match language {
        Language::Python => Ok(format!(
            "async def embed_and_store_{coll}(\n    client,\n    db_name: str = \"{db}\",\n    collection_name: str = \"{coll}\",\n    documents: list[dict] = None,\n    embed_field: str = \"{embed_field}\",\n) -> dict:\n    return await client.embed_and_store(\n        database=db_name,\n        collection=collection_name,\n        documents=documents or [],\n        embed_field=embed_field,\n    )\n",
            coll = coll, db = db, embed_field = embed_field,
        )),
        Language::TypeScript => Ok(format!(
            "async function embedAndStore_{coll}(\n  client: MongoCore,\n  dbName = \"{db}\",\n  collectionName = \"{coll}\",\n  documents: Record<string, unknown>[] = [],\n  embedField = \"{embed_field}\",\n) {{\n  return await client.embedAndStore({{ database: dbName, collection: collectionName, documents, embedField }});\n}}\n",
            coll = coll, db = db, embed_field = embed_field,
        )),
        Language::Go => Ok(format!(
            "func embedAndStore_{coll}(client *mongocore.Client, dbName string, collectionName string, documents []interface{{}}, embedField string) (*EmbedResult, error) {{\n    return client.EmbedAndStore(&EmbedOptions{{\n        Database:   dbName,\n        Collection: collectionName,\n        Documents:  documents,\n        EmbedField: embedField,\n    }})\n}}\n",
            coll = coll,
        )),
        Language::Java => Ok(format!(
            "public EmbedResult embedAndStore{coll_cap}(MongoClient client, String dbName, String collectionName, List<Document> documents, String embedField) {{\n    return client.embedAndStore(EmbedOptions.builder()\n        .database(dbName)\n        .collection(collectionName)\n        .documents(documents)\n        .embedField(embedField)\n        .build());\n}}\n",
            coll_cap = super::crud_gen::capitalize(coll),
        )),
    }
}
```

- [ ] **Step 2: Update semantic_search similarly**
- [ ] **Step 3: Update tests to assert real code instead of comments**
- [ ] **Step 4: Run tests**

Run: `cargo test --lib search_gen`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add src/mcp/codegen/search_gen.rs
git commit -m "feat(mcp): update search codegen to generate real client method calls"
```

---

## Task 15: Final Verification

- [ ] **Step 1: Verify all client tests pass**

Run: `just test-clients`
Expected: All client integration tests pass (Python, TypeScript, Go, Java)

- [ ] **Step 2: Full build check**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output (zero warnings)

- [ ] **Step 2: All unit tests**

Run: `cargo test --lib`
Expected: All tests pass (existing + new)

- [ ] **Step 3: Integration tests compile**

Run: `cargo test --test integration --no-run`
Expected: Compiles successfully

- [ ] **Step 4: Integration tests pass (requires Docker MongoDB)**

Run: `just docker-up && cargo test --test integration`
Expected: All integration tests pass including the new explain tests

- [ ] **Step 5: Commit any final fixes**

If any issues were found and fixed:

```bash
git add -A
git commit -m "fix(mcp): address issues found during final verification"
```
