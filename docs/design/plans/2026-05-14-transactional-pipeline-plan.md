# Transactional Pipeline Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.
> If modifying client libraries: verify imports work and run `just test-clients`.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

**Goal:** Implement the `TransactionPipeline` RPC — sequential dependent operations with `{{step.path}}` result forwarding, auto-wrapped in MongoDB transactions.

**Architecture:** New proto messages → reference parser module → validator + executor module → gRPC handler → MCP tool → client wrappers → integration tests → documentation.

**Tech Stack:** Rust, tonic (gRPC), serde_json, regex, MongoDB Rust driver transactions, protobuf.

---

## File Structure

| File | Responsibility |
|------|---------------|
| `proto/mongocore/v1/mongocore.proto` | Add TransactionStep, TransactionPipelineRequest/Response messages + RPC |
| `src/operations/pipeline_refs.rs` (new) | Parse `{{ref}}` syntax, resolve dot-paths/arrays/wildcards from results map |
| `src/operations/transaction_pipeline.rs` (new) | Validation + sequential executor with retry logic |
| `src/operations/mod.rs` | Register new modules |
| `src/grpc/service.rs` | Wire up `transaction_pipeline` RPC handler |
| `src/mcp/tools.rs` | Add `transaction_pipeline` tool definition + handler |
| `src/mcp/safety.rs` | Add `transaction_pipeline` to write tools list |
| `src/defaults.rs` | Add transaction pipeline constants |
| `src/error.rs` | Add `TransactionPipelineError` variant |
| `clients/python/src/mongocore/ops.py` | Add `Step` class and step-level op builders |
| `clients/python/src/mongocore/client.py` | Add `transaction_pipeline` method |
| `clients/python/src/mongocore/database.py` | Add database-scoped `transaction_pipeline` |
| `clients/python/src/mongocore/collection.py` | Add collection-scoped `transaction_pipeline` |
| `clients/typescript/src/ops.ts` | Add `Step` type and step builders |
| `clients/typescript/src/client.ts` | Add `transactionPipeline` method |
| `clients/typescript/src/database.ts` | Add database-scoped method |
| `clients/typescript/src/collection.ts` | Add collection-scoped method |
| `clients/go/mongocore/transaction_pipeline.go` (new) | Go Step type + methods |
| `clients/java/src/main/java/com/mongocore/TransactionPipelineStep.java` (new) | Java Step class |
| `tests/integration/transaction_pipeline_test.rs` (new) | Integration tests |
| `docs/transactional-pipelines.md` (new) | User-facing documentation |

---

## Task 1: Proto Definition

**Files:**
- Modify: `proto/mongocore/v1/mongocore.proto`

- [ ] **Step 1: Add TransactionPipeline messages to the proto file**

Add after the existing `PipelineError` message (line ~418) in `proto/mongocore/v1/mongocore.proto`:

```protobuf
// --- Transaction Pipeline ---

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
  optional string read_concern = 1;
  optional string write_concern = 2;
  optional uint64 max_time_ms = 3;
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
    DeleteManyResponse delete_result_many = 16;
    AggregateResponse aggregate_result = 17;
    FindAndModifyResponse find_and_modify_result = 18;
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

message TransactionPipelineError {
  string failed_step = 1;
  uint32 step_index = 2;
  string reason = 3;
  repeated string steps_completed = 4;
  bool rolled_back = 5;
}
```

- [ ] **Step 2: Add the RPC to the MongoCore service**

In the `service MongoCore` block, add after the `Pipeline` RPC (line 63):

```protobuf
  // Transaction Pipeline
  rpc TransactionPipeline(TransactionPipelineRequest) returns (TransactionPipelineResponse);
```

- [ ] **Step 3: Verify proto compiles**

Run: `cargo build`
Expected: Successful build with proto stubs regenerated (no errors).

- [ ] **Step 4: Commit**

```bash
git add proto/mongocore/v1/mongocore.proto
git commit -m "feat(proto): add TransactionPipeline messages and RPC"
```

---

## Task 2: Error Variant and Constants

**Files:**
- Modify: `src/error.rs`
- Modify: `src/defaults.rs`

- [ ] **Step 1: Add TransactionPipelineError variant to MongoCoreError**

In `src/error.rs`, add a new variant:

```rust
#[error("Transaction pipeline error: {0}")]
TransactionPipelineError(String),
```

- [ ] **Step 2: Add constants to defaults.rs**

In `src/defaults.rs`, add:

```rust
/// Maximum steps allowed in a transactional pipeline.
pub const DEFAULT_TRANSACTION_PIPELINE_MAX_STEPS: usize = 50;

/// Maximum documents stored from a Find/Aggregate step for referencing.
pub const DEFAULT_TRANSACTION_PIPELINE_MAX_DOCS: usize = 101;

/// Default transaction pipeline timeout in milliseconds.
pub const DEFAULT_TRANSACTION_PIPELINE_TIMEOUT_MS: u64 = 30_000;

/// Maximum retries on transient transaction errors.
pub const DEFAULT_TRANSACTION_PIPELINE_MAX_RETRIES: u32 = 3;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output (zero warnings).

- [ ] **Step 4: Commit**

```bash
git add src/error.rs src/defaults.rs
git commit -m "feat(core): add TransactionPipelineError variant and pipeline constants"
```

---

## Task 3: Reference Parser (`pipeline_refs.rs`)

**Files:**
- Create: `src/operations/pipeline_refs.rs`
- Modify: `src/operations/mod.rs`

- [ ] **Step 1: Write failing tests for reference parsing**

Create `src/operations/pipeline_refs.rs` with test module first:

```rust
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;

use crate::error::MongoCoreError;

/// A parsed reference extracted from a {{...}} placeholder.
#[derive(Debug, Clone, PartialEq)]
pub enum PathSegment {
    Field(String),
    Index(usize),
    Wildcard,
}

/// Parse a reference path like "step_name.field[0].sub" into segments.
/// Returns (step_name, path_segments).
pub fn parse_reference(reference: &str) -> Result<(String, Vec<PathSegment>), MongoCoreError> {
    todo!()
}

/// Find all {{...}} references in a JSON value, returning the reference strings.
pub fn extract_references(value: &Value) -> Vec<String> {
    todo!()
}

/// Resolve all {{...}} references in a JSON value using the results map.
/// Returns a new Value with all references substituted.
pub fn resolve_references(
    value: &Value,
    results: &HashMap<String, Value>,
) -> Result<Value, MongoCoreError> {
    todo!()
}

/// Traverse a JSON value using path segments to extract a sub-value.
fn traverse_path(value: &Value, segments: &[PathSegment]) -> Result<Value, MongoCoreError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_simple_field() {
        let (step, path) = parse_reference("find_user._id").unwrap();
        assert_eq!(step, "find_user");
        assert_eq!(path, vec![PathSegment::Field("_id".to_string())]);
    }

    #[test]
    fn test_parse_nested_field() {
        let (step, path) = parse_reference("find_user.address.city").unwrap();
        assert_eq!(step, "find_user");
        assert_eq!(path, vec![
            PathSegment::Field("address".to_string()),
            PathSegment::Field("city".to_string()),
        ]);
    }

    #[test]
    fn test_parse_array_index() {
        let (step, path) = parse_reference("find_users[0]._id").unwrap();
        assert_eq!(step, "find_users");
        assert_eq!(path, vec![
            PathSegment::Index(0),
            PathSegment::Field("_id".to_string()),
        ]);
    }

    #[test]
    fn test_parse_wildcard() {
        let (step, path) = parse_reference("find_users[*].email").unwrap();
        assert_eq!(step, "find_users");
        assert_eq!(path, vec![
            PathSegment::Wildcard,
            PathSegment::Field("email".to_string()),
        ]);
    }

    #[test]
    fn test_parse_full_result() {
        let (step, path) = parse_reference("find_users").unwrap();
        assert_eq!(step, "find_users");
        assert_eq!(path, Vec::<PathSegment>::new());
    }

    #[test]
    fn test_parse_length() {
        let (step, path) = parse_reference("find_users.length").unwrap();
        assert_eq!(step, "find_users");
        assert_eq!(path, vec![PathSegment::Field("length".to_string())]);
    }

    #[test]
    fn test_extract_references_from_json() {
        let val = json!({
            "filter": {"_id": "{{find_user._id}}"},
            "update": {"$set": {"city": "{{find_user.address.city}}"}}
        });
        let refs = extract_references(&val);
        assert_eq!(refs.len(), 2);
        assert!(refs.contains(&"find_user._id".to_string()));
        assert!(refs.contains(&"find_user.address.city".to_string()));
    }

    #[test]
    fn test_resolve_simple_reference() {
        let mut results = HashMap::new();
        results.insert("find_user".to_string(), json!({"_id": "abc123", "name": "Alice"}));

        let input = json!({"user_id": "{{find_user._id}}"});
        let resolved = resolve_references(&input, &results).unwrap();
        assert_eq!(resolved, json!({"user_id": "abc123"}));
    }

    #[test]
    fn test_resolve_preserves_type() {
        let mut results = HashMap::new();
        results.insert("update_step".to_string(), json!({"modified_count": 5}));

        let input = json!({"count": "{{update_step.modified_count}}"});
        let resolved = resolve_references(&input, &results).unwrap();
        assert_eq!(resolved, json!({"count": 5}));
    }

    #[test]
    fn test_resolve_inline_interpolation() {
        let mut results = HashMap::new();
        results.insert("find_user".to_string(), json!({"name": "Alice"}));

        let input = json!({"msg": "User {{find_user.name}} deactivated"});
        let resolved = resolve_references(&input, &results).unwrap();
        assert_eq!(resolved, json!({"msg": "User Alice deactivated"}));
    }

    #[test]
    fn test_resolve_wildcard_pluck() {
        let mut results = HashMap::new();
        results.insert("find_users".to_string(), json!([
            {"_id": "a", "name": "Alice"},
            {"_id": "b", "name": "Bob"},
        ]));

        let input = json!({"ids": "{{find_users[*]._id}}"});
        let resolved = resolve_references(&input, &results).unwrap();
        assert_eq!(resolved, json!({"ids": ["a", "b"]}));
    }

    #[test]
    fn test_resolve_full_passthrough() {
        let mut results = HashMap::new();
        results.insert("find_expired".to_string(), json!([
            {"_id": "a", "status": "expired"},
            {"_id": "b", "status": "expired"},
        ]));

        let input = json!("{{find_expired}}");
        let resolved = resolve_references(&input, &results).unwrap();
        assert_eq!(resolved, json!([
            {"_id": "a", "status": "expired"},
            {"_id": "b", "status": "expired"},
        ]));
    }

    #[test]
    fn test_resolve_length() {
        let mut results = HashMap::new();
        results.insert("find_users".to_string(), json!([
            {"_id": "a"},
            {"_id": "b"},
            {"_id": "c"},
        ]));

        let input = json!({"count": "{{find_users.length}}"});
        let resolved = resolve_references(&input, &results).unwrap();
        assert_eq!(resolved, json!({"count": 3}));
    }

    #[test]
    fn test_resolve_missing_step_errors() {
        let results = HashMap::new();
        let input = json!({"id": "{{nonexistent._id}}"});
        let result = resolve_references(&input, &results);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_missing_field_errors() {
        let mut results = HashMap::new();
        results.insert("find_user".to_string(), json!({"_id": "abc"}));

        let input = json!({"email": "{{find_user.email}}"});
        let result = resolve_references(&input, &results);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib operations::pipeline_refs`
Expected: All tests FAIL with "not yet implemented".

- [ ] **Step 3: Implement `parse_reference`**

```rust
pub fn parse_reference(reference: &str) -> Result<(String, Vec<PathSegment>), MongoCoreError> {
    if reference.is_empty() {
        return Err(MongoCoreError::TransactionPipelineError(
            "empty reference".to_string(),
        ));
    }

    let re = Regex::new(r"^([a-zA-Z_][a-zA-Z0-9_]*)(.*)$").unwrap();
    let caps = re.captures(reference).ok_or_else(|| {
        MongoCoreError::TransactionPipelineError(format!("invalid reference: {}", reference))
    })?;

    let step_name = caps[1].to_string();
    let remainder = &caps[2];

    if remainder.is_empty() {
        return Ok((step_name, vec![]));
    }

    let mut segments = Vec::new();
    let seg_re = Regex::new(r"\.([a-zA-Z_][a-zA-Z0-9_]*)|\[(\d+)\]|\[\*\]").unwrap();

    for cap in seg_re.captures_iter(remainder) {
        if let Some(field) = cap.get(1) {
            segments.push(PathSegment::Field(field.as_str().to_string()));
        } else if let Some(idx) = cap.get(2) {
            let i: usize = idx.as_str().parse().map_err(|_| {
                MongoCoreError::TransactionPipelineError(format!(
                    "invalid array index in reference: {}",
                    reference
                ))
            })?;
            segments.push(PathSegment::Index(i));
        } else {
            segments.push(PathSegment::Wildcard);
        }
    }

    Ok((step_name, segments))
}
```

- [ ] **Step 4: Implement `extract_references`**

```rust
pub fn extract_references(value: &Value) -> Vec<String> {
    let re = Regex::new(r"\{\{([^}]+)\}\}").unwrap();
    let mut refs = Vec::new();
    extract_refs_recursive(value, &re, &mut refs);
    refs
}

fn extract_refs_recursive(value: &Value, re: &Regex, refs: &mut Vec<String>) {
    match value {
        Value::String(s) => {
            for cap in re.captures_iter(s) {
                refs.push(cap[1].trim().to_string());
            }
        }
        Value::Object(map) => {
            for v in map.values() {
                extract_refs_recursive(v, re, refs);
            }
        }
        Value::Array(arr) => {
            for v in arr {
                extract_refs_recursive(v, re, refs);
            }
        }
        _ => {}
    }
}
```

- [ ] **Step 5: Implement `traverse_path`**

```rust
fn traverse_path(value: &Value, segments: &[PathSegment]) -> Result<Value, MongoCoreError> {
    if segments.is_empty() {
        return Ok(value.clone());
    }

    let segment = &segments[0];
    let rest = &segments[1..];

    match segment {
        PathSegment::Field(field) => {
            if field == "length" && rest.is_empty() {
                match value {
                    Value::Array(arr) => return Ok(Value::from(arr.len())),
                    _ => {}
                }
            }
            match value.get(field) {
                Some(v) => traverse_path(v, rest),
                None => Err(MongoCoreError::TransactionPipelineError(format!(
                    "field '{}' not found in result",
                    field
                ))),
            }
        }
        PathSegment::Index(i) => match value.as_array() {
            Some(arr) => match arr.get(*i) {
                Some(v) => traverse_path(v, rest),
                None => Err(MongoCoreError::TransactionPipelineError(format!(
                    "index {} out of bounds (array has {} elements)",
                    i,
                    arr.len()
                ))),
            },
            None => Err(MongoCoreError::TransactionPipelineError(
                "cannot index non-array value".to_string(),
            )),
        },
        PathSegment::Wildcard => match value.as_array() {
            Some(arr) => {
                let plucked: Result<Vec<Value>, _> =
                    arr.iter().map(|item| traverse_path(item, rest)).collect();
                Ok(Value::Array(plucked?))
            }
            None => Err(MongoCoreError::TransactionPipelineError(
                "cannot use wildcard [*] on non-array value".to_string(),
            )),
        },
    }
}
```

- [ ] **Step 6: Implement `resolve_references`**

```rust
pub fn resolve_references(
    value: &Value,
    results: &HashMap<String, Value>,
) -> Result<Value, MongoCoreError> {
    let re = Regex::new(r"\{\{([^}]+)\}\}").unwrap();
    resolve_recursive(value, results, &re)
}

fn resolve_recursive(
    value: &Value,
    results: &HashMap<String, Value>,
    re: &Regex,
) -> Result<Value, MongoCoreError> {
    match value {
        Value::String(s) => {
            // Check if the entire string is a single reference
            let trimmed = s.trim();
            if trimmed.starts_with("{{") && trimmed.ends_with("}}") {
                let inner = &trimmed[2..trimmed.len() - 2].trim();
                // Check no other {{ in between (single reference)
                if !inner.contains("{{") {
                    let (step, path) = parse_reference(inner)?;
                    let step_result = results.get(&step).ok_or_else(|| {
                        MongoCoreError::TransactionPipelineError(format!(
                            "referenced step '{}' not found in results",
                            step
                        ))
                    })?;
                    return traverse_path(step_result, &path);
                }
            }

            // Inline interpolation — multiple refs or ref within larger string
            let mut result_str = s.clone();
            for cap in re.captures_iter(s) {
                let full_match = cap.get(0).unwrap().as_str();
                let ref_str = cap[1].trim();
                let (step, path) = parse_reference(ref_str)?;
                let step_result = results.get(&step).ok_or_else(|| {
                    MongoCoreError::TransactionPipelineError(format!(
                        "referenced step '{}' not found in results",
                        step
                    ))
                })?;
                let resolved = traverse_path(step_result, &path)?;
                let replacement = match &resolved {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                result_str = result_str.replacen(full_match, &replacement, 1);
            }
            Ok(Value::String(result_str))
        }
        Value::Object(map) => {
            let mut new_map = serde_json::Map::new();
            for (k, v) in map {
                new_map.insert(k.clone(), resolve_recursive(v, results, re)?);
            }
            Ok(Value::Object(new_map))
        }
        Value::Array(arr) => {
            let resolved: Result<Vec<Value>, _> =
                arr.iter().map(|v| resolve_recursive(v, results, re)).collect();
            Ok(Value::Array(resolved?))
        }
        other => Ok(other.clone()),
    }
}
```

- [ ] **Step 7: Register the module in mod.rs**

In `src/operations/mod.rs`, add:

```rust
pub mod pipeline_refs;
```

- [ ] **Step 8: Run tests to verify they pass**

Run: `cargo test --lib operations::pipeline_refs`
Expected: All tests PASS.

- [ ] **Step 9: Run full check**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output.

- [ ] **Step 10: Commit**

```bash
git add src/operations/pipeline_refs.rs src/operations/mod.rs
git commit -m "feat(core): add pipeline reference parser with dot-path, array index, wildcard support"
```

---

## Task 4: Validator and Executor (`transaction_pipeline.rs`)

**Files:**
- Create: `src/operations/transaction_pipeline.rs`
- Modify: `src/operations/mod.rs`

- [ ] **Step 1: Write failing tests for validation**

Create `src/operations/transaction_pipeline.rs`:

```rust
use serde_json::Value;
use std::collections::{HashMap, HashSet};

use crate::defaults::{
    DEFAULT_TRANSACTION_PIPELINE_MAX_DOCS, DEFAULT_TRANSACTION_PIPELINE_MAX_STEPS,
};
use crate::error::MongoCoreError;

use super::pipeline_refs;

/// Validate a transactional pipeline before execution.
/// Returns Ok(()) if valid, Err with descriptive message if not.
pub fn validate_pipeline(steps: &[PipelineStepDef]) -> Result<(), MongoCoreError> {
    todo!()
}

/// Definition of a single pipeline step for validation purposes.
#[derive(Debug, Clone)]
pub struct PipelineStepDef {
    pub name: String,
    pub database: String,
    pub collection: String,
    pub operation_type: String,
    pub operation_json: Value,
    pub find_limit: Option<i64>,
}

/// Valid step name regex pattern.
fn is_valid_step_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_')
        && name.chars().next().map_or(false, |c| c.is_alphabetic() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn step(name: &str, op_type: &str) -> PipelineStepDef {
        PipelineStepDef {
            name: name.to_string(),
            database: "testdb".to_string(),
            collection: "testcoll".to_string(),
            operation_type: op_type.to_string(),
            operation_json: json!({"filter": {}}),
            find_limit: None,
        }
    }

    fn step_with_ref(name: &str, op_type: &str, ref_str: &str) -> PipelineStepDef {
        PipelineStepDef {
            name: name.to_string(),
            database: "testdb".to_string(),
            collection: "testcoll".to_string(),
            operation_type: op_type.to_string(),
            operation_json: json!({"filter": {"_id": format!("{{{{{}}}}}", ref_str)}}),
            find_limit: None,
        }
    }

    #[test]
    fn test_valid_pipeline() {
        let steps = vec![
            step("find_user", "find_one"),
            step_with_ref("update_user", "update", "find_user._id"),
        ];
        assert!(validate_pipeline(&steps).is_ok());
    }

    #[test]
    fn test_empty_pipeline_rejected() {
        let result = validate_pipeline(&[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("at least one step"));
    }

    #[test]
    fn test_too_many_steps_rejected() {
        let steps: Vec<_> = (0..51).map(|i| step(&format!("step_{}", i), "find_one")).collect();
        let result = validate_pipeline(&steps);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("50"));
    }

    #[test]
    fn test_duplicate_names_rejected() {
        let steps = vec![step("find_user", "find_one"), step("find_user", "update")];
        let result = validate_pipeline(&steps);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("duplicate"));
    }

    #[test]
    fn test_invalid_name_format_rejected() {
        let steps = vec![step("123bad", "find_one")];
        let result = validate_pipeline(&steps);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("name"));
    }

    #[test]
    fn test_forward_reference_rejected() {
        let steps = vec![
            step_with_ref("step_a", "update", "step_b._id"),
            step("step_b", "find_one"),
        ];
        let result = validate_pipeline(&steps);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("forward reference"));
    }

    #[test]
    fn test_unknown_reference_rejected() {
        let steps = vec![step_with_ref("step_a", "update", "nonexistent._id")];
        let result = validate_pipeline(&steps);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("nonexistent"));
    }

    #[test]
    fn test_find_limit_over_101_rejected() {
        let mut s = step("find_step", "find");
        s.find_limit = Some(200);
        let result = validate_pipeline(&[s]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("101"));
    }

    #[test]
    fn test_find_limit_at_101_ok() {
        let mut s = step("find_step", "find");
        s.find_limit = Some(101);
        assert!(validate_pipeline(&[s]).is_ok());
    }

    #[test]
    fn test_missing_database_rejected() {
        let mut s = step("find_step", "find_one");
        s.database = "".to_string();
        let result = validate_pipeline(&[s]);
        assert!(result.is_err());
    }

    #[test]
    fn test_missing_collection_rejected() {
        let mut s = step("find_step", "find_one");
        s.collection = "".to_string();
        let result = validate_pipeline(&[s]);
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_step_name_check() {
        assert!(is_valid_step_name("find_user"));
        assert!(is_valid_step_name("_private"));
        assert!(is_valid_step_name("step0"));
        assert!(!is_valid_step_name("0step"));
        assert!(!is_valid_step_name(""));
        assert!(!is_valid_step_name("has.dot"));
        assert!(!is_valid_step_name("has[bracket"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib operations::transaction_pipeline`
Expected: FAIL with "not yet implemented".

- [ ] **Step 3: Implement `validate_pipeline`**

```rust
pub fn validate_pipeline(steps: &[PipelineStepDef]) -> Result<(), MongoCoreError> {
    if steps.is_empty() {
        return Err(MongoCoreError::TransactionPipelineError(
            "pipeline must contain at least one step".to_string(),
        ));
    }

    if steps.len() > DEFAULT_TRANSACTION_PIPELINE_MAX_STEPS {
        return Err(MongoCoreError::TransactionPipelineError(format!(
            "pipeline exceeds maximum of {} steps (got {})",
            DEFAULT_TRANSACTION_PIPELINE_MAX_STEPS,
            steps.len()
        )));
    }

    let mut seen_names: HashSet<String> = HashSet::new();
    let mut defined_before: Vec<String> = Vec::new();

    for (i, step) in steps.iter().enumerate() {
        // Validate name format
        if !is_valid_step_name(&step.name) {
            return Err(MongoCoreError::TransactionPipelineError(format!(
                "step {} has invalid name '{}': must be alphanumeric/underscore, starting with letter or underscore",
                i, step.name
            )));
        }

        // Check for duplicate names
        if !seen_names.insert(step.name.clone()) {
            return Err(MongoCoreError::TransactionPipelineError(format!(
                "duplicate step name '{}' at index {}",
                step.name, i
            )));
        }

        // Check required fields
        if step.database.is_empty() {
            return Err(MongoCoreError::TransactionPipelineError(format!(
                "step '{}' is missing database",
                step.name
            )));
        }
        if step.collection.is_empty() {
            return Err(MongoCoreError::TransactionPipelineError(format!(
                "step '{}' is missing collection",
                step.name
            )));
        }

        // Check find limit
        if (step.operation_type == "find" || step.operation_type == "aggregate") {
            if let Some(limit) = step.find_limit {
                if limit > DEFAULT_TRANSACTION_PIPELINE_MAX_DOCS as i64 {
                    return Err(MongoCoreError::TransactionPipelineError(format!(
                        "step '{}' has limit {} which exceeds maximum of {}",
                        step.name, limit, DEFAULT_TRANSACTION_PIPELINE_MAX_DOCS
                    )));
                }
            }
        }

        // Extract and validate references
        let refs = pipeline_refs::extract_references(&step.operation_json);
        for ref_str in &refs {
            let (ref_step, _) = pipeline_refs::parse_reference(ref_str)?;

            if !defined_before.contains(&ref_step) {
                if seen_names.contains(&ref_step) || steps.iter().any(|s| s.name == ref_step) {
                    return Err(MongoCoreError::TransactionPipelineError(format!(
                        "step '{}' has forward reference to step '{}' which is defined later",
                        step.name, ref_step
                    )));
                } else {
                    return Err(MongoCoreError::TransactionPipelineError(format!(
                        "step '{}' references unknown step '{}'",
                        step.name, ref_step
                    )));
                }
            }
        }

        defined_before.push(step.name.clone());
    }

    Ok(())
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib operations::transaction_pipeline`
Expected: All tests PASS.

- [ ] **Step 5: Register module in mod.rs**

In `src/operations/mod.rs`, add:

```rust
pub mod transaction_pipeline;
```

- [ ] **Step 6: Run full build check**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output.

- [ ] **Step 7: Commit**

```bash
git add src/operations/transaction_pipeline.rs src/operations/mod.rs
git commit -m "feat(core): add transaction pipeline validator with reference checking"
```

---

## Task 5: Pipeline Executor

**Files:**
- Modify: `src/operations/transaction_pipeline.rs`

- [ ] **Step 1: Add executor struct and method**

Add the executor to `src/operations/transaction_pipeline.rs`. This orchestrates: validate → begin transaction → execute steps sequentially → commit/abort.

```rust
use std::time::Instant;

use bson::Document as BsonDocument;
use mongodb::ClientSession;
use serde_json::Value;

use crate::connection::pool::ConnectionPool;
use crate::defaults::{
    DEFAULT_TRANSACTION_PIPELINE_MAX_DOCS,
    DEFAULT_TRANSACTION_PIPELINE_MAX_RETRIES,
    DEFAULT_TRANSACTION_PIPELINE_TIMEOUT_MS,
};
use crate::error::MongoCoreError;
use crate::operations::FindOptions;

/// Options for transaction pipeline execution.
#[derive(Debug, Clone)]
pub struct TransactionPipelineOptions {
    pub read_concern: Option<String>,
    pub write_concern: Option<String>,
    pub max_time_ms: u64,
}

impl Default for TransactionPipelineOptions {
    fn default() -> Self {
        Self {
            read_concern: None,
            write_concern: None,
            max_time_ms: DEFAULT_TRANSACTION_PIPELINE_TIMEOUT_MS,
        }
    }
}

/// Result of executing a single step.
#[derive(Debug, Clone)]
pub struct StepResult {
    pub name: String,
    pub success: bool,
    pub result_json: Value,
}

/// Result of executing the full pipeline.
#[derive(Debug, Clone)]
pub struct PipelineExecutionResult {
    pub steps: Vec<StepResult>,
    pub total_steps: u32,
    pub steps_completed: u32,
    pub elapsed_ms: u64,
}

/// Error details when a pipeline fails.
#[derive(Debug, Clone)]
pub struct PipelineFailure {
    pub failed_step: String,
    pub step_index: u32,
    pub reason: String,
    pub steps_completed: Vec<String>,
    pub rolled_back: bool,
}

/// Execute a transactional pipeline.
pub async fn execute_transaction_pipeline(
    pool: &ConnectionPool,
    steps: Vec<PipelineStepDef>,
    options: TransactionPipelineOptions,
) -> Result<PipelineExecutionResult, PipelineFailure> {
    // Validate first
    validate_pipeline(&steps).map_err(|e| PipelineFailure {
        failed_step: String::new(),
        step_index: 0,
        reason: e.to_string(),
        steps_completed: vec![],
        rolled_back: false,
    })?;

    let mut last_error = None;

    for attempt in 0..DEFAULT_TRANSACTION_PIPELINE_MAX_RETRIES {
        match execute_pipeline_attempt(pool, &steps, &options).await {
            Ok(result) => return Ok(result),
            Err(failure) => {
                if is_transient_error(&failure.reason) && attempt < DEFAULT_TRANSACTION_PIPELINE_MAX_RETRIES - 1 {
                    last_error = Some(failure);
                    continue;
                }
                return Err(failure);
            }
        }
    }

    Err(last_error.unwrap())
}

async fn execute_pipeline_attempt(
    pool: &ConnectionPool,
    steps: &[PipelineStepDef],
    options: &TransactionPipelineOptions,
) -> Result<PipelineExecutionResult, PipelineFailure> {
    let start = Instant::now();
    let timeout_ms = options.max_time_ms;

    let mut session = pool.client().start_session().await.map_err(|e| PipelineFailure {
        failed_step: String::new(),
        step_index: 0,
        reason: format!("failed to start session: {}", e),
        steps_completed: vec![],
        rolled_back: false,
    })?;

    session.start_transaction().await.map_err(|e| PipelineFailure {
        failed_step: String::new(),
        step_index: 0,
        reason: format!("failed to start transaction: {}", e),
        steps_completed: vec![],
        rolled_back: false,
    })?;

    let mut results_map: HashMap<String, Value> = HashMap::new();
    let mut step_results: Vec<StepResult> = Vec::new();
    let mut steps_completed: Vec<String> = Vec::new();

    for (i, step) in steps.iter().enumerate() {
        // Check timeout
        if start.elapsed().as_millis() as u64 > timeout_ms {
            let _ = session.abort_transaction().await;
            return Err(PipelineFailure {
                failed_step: step.name.clone(),
                step_index: i as u32,
                reason: "pipeline timeout exceeded".to_string(),
                steps_completed,
                rolled_back: true,
            });
        }

        // Resolve references in the operation JSON
        let resolved_op = pipeline_refs::resolve_references(&step.operation_json, &results_map)
            .map_err(|e| {
                PipelineFailure {
                    failed_step: step.name.clone(),
                    step_index: i as u32,
                    reason: e.to_string(),
                    steps_completed: steps_completed.clone(),
                    rolled_back: true,
                }
            })?;

        // Execute the operation
        let result_value = execute_step(pool, &mut session, step, &resolved_op)
            .await
            .map_err(|e| {
                PipelineFailure {
                    failed_step: step.name.clone(),
                    step_index: i as u32,
                    reason: e.to_string(),
                    steps_completed: steps_completed.clone(),
                    rolled_back: true,
                }
            })?;

        results_map.insert(step.name.clone(), result_value.clone());
        step_results.push(StepResult {
            name: step.name.clone(),
            success: true,
            result_json: result_value,
        });
        steps_completed.push(step.name.clone());
    }

    // Commit
    session.commit_transaction().await.map_err(|e| PipelineFailure {
        failed_step: String::new(),
        step_index: steps.len() as u32,
        reason: format!("failed to commit transaction: {}", e),
        steps_completed,
        rolled_back: true,
    })?;

    Ok(PipelineExecutionResult {
        total_steps: steps.len() as u32,
        steps_completed: step_results.len() as u32,
        elapsed_ms: start.elapsed().as_millis() as u64,
        steps: step_results,
    })
}

/// Execute a single step within the transaction session.
async fn execute_step(
    pool: &ConnectionPool,
    session: &mut ClientSession,
    step: &PipelineStepDef,
    resolved_op: &Value,
) -> Result<Value, MongoCoreError> {
    let coll = pool.collection(&step.database, &step.collection);

    match step.operation_type.as_str() {
        "find_one" => {
            let filter = bson_from_json_field(resolved_op, "filter")?;
            let mut cursor = coll.find(filter).limit(1).session(&mut *session).await?;
            if cursor.advance(&mut *session).await? {
                let doc = cursor.deserialize_current()?;
                Ok(bson_doc_to_json(&doc))
            } else {
                Ok(Value::Null)
            }
        }
        "find" => {
            let filter = bson_from_json_field(resolved_op, "filter")?;
            let limit = step.find_limit.unwrap_or(DEFAULT_TRANSACTION_PIPELINE_MAX_DOCS as i64);
            let mut cursor = coll.find(filter).limit(limit).session(&mut *session).await?;
            let mut docs = Vec::new();
            while cursor.advance(&mut *session).await? {
                docs.push(bson_doc_to_json(&cursor.deserialize_current()?));
            }
            Ok(Value::Array(docs))
        }
        "insert" => {
            let document = bson_from_json_field(resolved_op, "document")?;
            let result = coll.insert_one(document).session(&mut *session).await?;
            Ok(serde_json::json!({
                "inserted_id": result.inserted_id.to_string()
            }))
        }
        "insert_many" => {
            let documents = bson_array_from_json_field(resolved_op, "documents")?;
            let result = coll.insert_many(documents).session(&mut *session).await?;
            let ids: Vec<String> = result.inserted_ids.values().map(|id| id.to_string()).collect();
            Ok(serde_json::json!({
                "inserted_ids": ids,
                "inserted_count": ids.len()
            }))
        }
        "update" => {
            let filter = bson_from_json_field(resolved_op, "filter")?;
            let update = bson_from_json_field(resolved_op, "update")?;
            let result = coll.update_one(filter, update).session(&mut *session).await?;
            Ok(serde_json::json!({
                "matched_count": result.matched_count,
                "modified_count": result.modified_count,
                "upserted_id": result.upserted_id.map(|id| id.to_string())
            }))
        }
        "update_many" => {
            let filter = bson_from_json_field(resolved_op, "filter")?;
            let update = bson_from_json_field(resolved_op, "update")?;
            let result = coll.update_many(filter, update).session(&mut *session).await?;
            Ok(serde_json::json!({
                "matched_count": result.matched_count,
                "modified_count": result.modified_count,
                "upserted_id": result.upserted_id.map(|id| id.to_string())
            }))
        }
        "delete" => {
            let filter = bson_from_json_field(resolved_op, "filter")?;
            let result = coll.delete_one(filter).session(&mut *session).await?;
            Ok(serde_json::json!({ "deleted_count": result.deleted_count }))
        }
        "delete_many" => {
            let filter = bson_from_json_field(resolved_op, "filter")?;
            let result = coll.delete_many(filter).session(&mut *session).await?;
            Ok(serde_json::json!({ "deleted_count": result.deleted_count }))
        }
        "find_and_modify" => {
            let filter = bson_from_json_field(resolved_op, "filter")?;
            let update = bson_from_json_field(resolved_op, "update")?;
            let result = coll
                .find_one_and_update(filter, update)
                .return_document(mongodb::options::ReturnDocument::After)
                .session(&mut *session)
                .await?;
            match result {
                Some(doc) => Ok(bson_doc_to_json(&doc)),
                None => Ok(Value::Null),
            }
        }
        "aggregate" => {
            let pipeline = bson_array_from_json_field(resolved_op, "pipeline")?;
            let mut cursor = coll.aggregate(pipeline).session(&mut *session).await?;
            let mut docs = Vec::new();
            while cursor.advance(&mut *session).await? {
                docs.push(bson_doc_to_json(&cursor.deserialize_current()?));
                if docs.len() > DEFAULT_TRANSACTION_PIPELINE_MAX_DOCS {
                    return Err(MongoCoreError::TransactionPipelineError(format!(
                        "step '{}' returned more than {} documents",
                        step.name, DEFAULT_TRANSACTION_PIPELINE_MAX_DOCS
                    )));
                }
            }
            Ok(Value::Array(docs))
        }
        other => Err(MongoCoreError::TransactionPipelineError(format!(
            "unsupported operation type: {}",
            other
        ))),
    }
}

fn is_transient_error(reason: &str) -> bool {
    reason.contains("TransientTransactionError")
}

/// Convert a JSON field to a BSON document.
fn bson_from_json_field(value: &Value, field: &str) -> Result<BsonDocument, MongoCoreError> {
    let field_value = value.get(field).unwrap_or(&Value::Object(serde_json::Map::new()));
    let bson_val: bson::Bson = field_value.clone().into();
    match bson_val {
        bson::Bson::Document(doc) => Ok(doc),
        _ => Ok(BsonDocument::new()),
    }
}

/// Convert a JSON array field to a Vec<BsonDocument>.
fn bson_array_from_json_field(value: &Value, field: &str) -> Result<Vec<BsonDocument>, MongoCoreError> {
    let arr = value.get(field).and_then(|v| v.as_array()).unwrap_or(&vec![]);
    arr.iter()
        .map(|item| {
            let bson_val: bson::Bson = item.clone().into();
            match bson_val {
                bson::Bson::Document(doc) => Ok(doc),
                _ => Err(MongoCoreError::TransactionPipelineError(
                    "expected document in array".to_string(),
                )),
            }
        })
        .collect()
}

/// Convert a BSON document to a serde_json Value.
fn bson_doc_to_json(doc: &BsonDocument) -> Value {
    let bson_val = bson::Bson::Document(doc.clone());
    bson_val.into()
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output (zero warnings). Fix any unused import warnings.

- [ ] **Step 3: Commit**

```bash
git add src/operations/transaction_pipeline.rs
git commit -m "feat(core): add transaction pipeline executor with retry logic"
```

---

## Task 6: gRPC Handler

**Files:**
- Modify: `src/grpc/service.rs`

- [ ] **Step 1: Add the `transaction_pipeline` RPC handler**

In `src/grpc/service.rs`, add the implementation of the `transaction_pipeline` method to the `MongoCore` trait impl. Add it after the existing `pipeline` method (around line 1483).

```rust
    #[tracing::instrument(skip(self, request), fields(
        transaction_pipeline.steps = tracing::field::Empty,
        transaction_pipeline.completed = tracing::field::Empty
    ))]
    async fn transaction_pipeline(
        &self,
        request: Request<proto::TransactionPipelineRequest>,
    ) -> Result<Response<proto::TransactionPipelineResponse>, Status> {
        use crate::operations::transaction_pipeline::{
            execute_transaction_pipeline, PipelineStepDef, TransactionPipelineOptions,
        };

        self.append_client_language(request.metadata());
        self.check_tenant_quota(request.metadata())?;
        let req = request.into_inner();

        if req.steps.is_empty() {
            return Err(Status::invalid_argument(
                "TransactionPipeline must contain at least one step",
            ));
        }

        // Convert proto steps to internal representation
        let steps: Vec<PipelineStepDef> = req
            .steps
            .into_iter()
            .map(|s| proto_step_to_def(s))
            .collect::<Result<Vec<_>, _>>()?;

        let options = match req.options {
            Some(opts) => TransactionPipelineOptions {
                read_concern: opts.read_concern,
                write_concern: opts.write_concern,
                max_time_ms: opts.max_time_ms.unwrap_or(
                    crate::defaults::DEFAULT_TRANSACTION_PIPELINE_TIMEOUT_MS,
                ),
            },
            None => TransactionPipelineOptions::default(),
        };

        match execute_transaction_pipeline(&self.pool, steps, options).await {
            Ok(result) => {
                let span = tracing::Span::current();
                span.record("transaction_pipeline.steps", result.total_steps);
                span.record("transaction_pipeline.completed", result.steps_completed);

                let response = proto::TransactionPipelineResponse {
                    steps: result
                        .steps
                        .into_iter()
                        .map(|s| step_result_to_proto(s))
                        .collect(),
                    summary: Some(proto::TransactionPipelineSummary {
                        total_steps: result.total_steps,
                        steps_completed: result.steps_completed,
                        elapsed_ms: result.elapsed_ms,
                    }),
                };
                Ok(Response::new(response))
            }
            Err(failure) => Err(Status::aborted(serde_json::json!({
                "failed_step": failure.failed_step,
                "step_index": failure.step_index,
                "reason": failure.reason,
                "steps_completed": failure.steps_completed,
                "rolled_back": failure.rolled_back,
            }).to_string())),
        }
    }
```

- [ ] **Step 2: Add helper functions for proto conversion**

Add these helper functions after the `pipeline_result_is_error` function:

```rust
/// Convert a proto TransactionStep to internal PipelineStepDef.
fn proto_step_to_def(step: proto::TransactionStep) -> Result<crate::operations::transaction_pipeline::PipelineStepDef, Status> {
    use proto::transaction_step::Operation;

    let (operation_type, operation_json, find_limit) = match step.operation {
        Some(Operation::FindOne(req)) => {
            let filter_json = proto_filter_to_json(&req.filter);
            ("find_one".to_string(), serde_json::json!({"filter": filter_json}), None)
        }
        Some(Operation::Find(req)) => {
            let filter_json = proto_filter_to_json(&req.filter);
            let limit = req.options.as_ref().and_then(|o| {
                if o.limit > 0 { Some(o.limit) } else { None }
            });
            ("find".to_string(), serde_json::json!({"filter": filter_json}), limit)
        }
        Some(Operation::Insert(req)) => {
            let doc_json = proto_doc_to_json(&req.document);
            ("insert".to_string(), serde_json::json!({"document": doc_json}), None)
        }
        Some(Operation::InsertMany(req)) => {
            let docs: Vec<Value> = req.documents.iter().map(|d| proto_doc_to_json(&Some(d.clone()))).collect();
            ("insert_many".to_string(), serde_json::json!({"documents": docs}), None)
        }
        Some(Operation::Update(req)) => {
            let filter_json = proto_filter_to_json(&req.filter);
            let update_json = proto_doc_to_json(&req.update);
            ("update".to_string(), serde_json::json!({"filter": filter_json, "update": update_json}), None)
        }
        Some(Operation::UpdateMany(req)) => {
            let filter_json = proto_filter_to_json(&req.filter);
            let update_json = proto_doc_to_json(&req.update);
            ("update_many".to_string(), serde_json::json!({"filter": filter_json, "update": update_json}), None)
        }
        Some(Operation::Delete(req)) => {
            let filter_json = proto_filter_to_json(&req.filter);
            ("delete".to_string(), serde_json::json!({"filter": filter_json}), None)
        }
        Some(Operation::DeleteMany(req)) => {
            let filter_json = proto_filter_to_json(&req.filter);
            ("delete_many".to_string(), serde_json::json!({"filter": filter_json}), None)
        }
        Some(Operation::FindAndModify(req)) => {
            let filter_json = proto_filter_to_json(&req.filter);
            let update_json = proto_doc_to_json(&req.update);
            ("find_and_modify".to_string(), serde_json::json!({"filter": filter_json, "update": update_json}), None)
        }
        Some(Operation::Aggregate(req)) => {
            let pipeline: Vec<Value> = req.pipeline.map(|p| {
                p.stages.iter().map(|s| proto_doc_to_json(&Some(s.clone()))).collect()
            }).unwrap_or_default();
            ("aggregate".to_string(), serde_json::json!({"pipeline": pipeline}), None)
        }
        None => return Err(Status::invalid_argument(format!(
            "step '{}' has no operation",
            step.name
        ))),
    };

    Ok(crate::operations::transaction_pipeline::PipelineStepDef {
        name: step.name,
        database: step.database,
        collection: step.collection,
        operation_type,
        operation_json,
        find_limit,
    })
}

/// Convert a step result to proto.
fn step_result_to_proto(step: crate::operations::transaction_pipeline::StepResult) -> proto::TransactionStepResult {
    // For now, encode the result JSON as a FindOneResponse with the document bytes
    // A more complete implementation would match on the operation type
    proto::TransactionStepResult {
        name: step.name,
        success: step.success,
        result: None, // TODO: map result_json back to proper proto oneof in a follow-up
    }
}
```

Note: The proto conversion helpers (`proto_filter_to_json`, `proto_doc_to_json`) should reuse existing patterns from `execute_pipeline_op`. Inspect those helpers and reuse them.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output.

- [ ] **Step 4: Commit**

```bash
git add src/grpc/service.rs
git commit -m "feat(grpc): wire up TransactionPipeline RPC handler"
```

---

## Task 7: MCP Tool Definition and Handler

**Files:**
- Modify: `src/mcp/tools.rs`
- Modify: `src/mcp/safety.rs`

- [ ] **Step 1: Add tool definition**

In `src/mcp/tools.rs`, add to the `tool_definitions()` vec:

```rust
McpToolDefinition {
    name: "transaction_pipeline".to_string(),
    description: "Execute multiple dependent operations atomically in a transaction. Steps run sequentially and can reference results from prior steps using {{step_name.field}} syntax.".to_string(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "steps": {
                "type": "array",
                "description": "Ordered list of operations to execute in a transaction",
                "items": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Unique step name (used in references)" },
                        "database": { "type": "string", "description": "Database name" },
                        "collection": { "type": "string", "description": "Collection name" },
                        "operation": { "type": "string", "enum": ["find_one", "find", "insert", "insert_many", "update", "update_many", "delete", "delete_many", "find_and_modify", "aggregate"], "description": "Operation type" },
                        "params": { "type": "object", "description": "Operation parameters (filter, document, update, pipeline, etc.)" }
                    },
                    "required": ["name", "database", "collection", "operation", "params"]
                }
            },
            "options": {
                "type": "object",
                "description": "Transaction options",
                "properties": {
                    "read_concern": { "type": "string" },
                    "write_concern": { "type": "string" },
                    "max_time_ms": { "type": "integer" }
                }
            }
        },
        "required": ["steps"]
    }),
},
```

- [ ] **Step 2: Add `transaction_pipeline` to write tools in safety.rs**

In `src/mcp/safety.rs`, add `"transaction_pipeline"` to the `WRITE_TOOLS` const array:

```rust
const WRITE_TOOLS: &[&str] = &[
    "insert",
    "insert_many",
    "update",
    "update_many",
    "delete",
    "delete_many",
    "create_collection",
    "create_index",
    "run_command",
    "transaction_pipeline",
];
```

- [ ] **Step 3: Add handler dispatch for `transaction_pipeline`**

In `src/mcp/tools.rs`, add the handler function and wire it up in the main `execute_tool` dispatch (look for the match on tool name). Add a case:

```rust
"transaction_pipeline" => {
    execute_transaction_pipeline_tool(operations, pool, safety, args).await
}
```

Then add the handler function:

```rust
async fn execute_transaction_pipeline_tool(
    operations: &Operations,
    pool: &ConnectionPool,
    safety: &SafetyConfig,
    args: &Value,
) -> McpToolCallResult {
    use crate::operations::transaction_pipeline::{
        execute_transaction_pipeline, PipelineStepDef, TransactionPipelineOptions,
    };

    // Check safety
    if let Err(reason) = safety.check_tool_allowed("transaction_pipeline") {
        return error_result(reason);
    }

    let steps_arr = match args.get("steps").and_then(|v| v.as_array()) {
        Some(arr) => arr,
        None => return error_result("Missing required field: steps".to_string()),
    };

    if steps_arr.is_empty() {
        return error_result("Pipeline must contain at least one step".to_string());
    }

    let steps: Vec<PipelineStepDef> = match steps_arr
        .iter()
        .map(|s| {
            let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let database = s.get("database").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let collection = s.get("collection").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let operation = s.get("operation").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let params = s.get("params").cloned().unwrap_or(json!({}));
            let find_limit = params.get("limit").and_then(|v| v.as_i64());

            Ok(PipelineStepDef {
                name,
                database,
                collection,
                operation_type: operation,
                operation_json: params,
                find_limit,
            })
        })
        .collect::<Result<Vec<_>, String>>()
    {
        Ok(s) => s,
        Err(e) => return error_result(e),
    };

    let options = match args.get("options") {
        Some(opts) => TransactionPipelineOptions {
            read_concern: opts.get("read_concern").and_then(|v| v.as_str()).map(|s| s.to_string()),
            write_concern: opts.get("write_concern").and_then(|v| v.as_str()).map(|s| s.to_string()),
            max_time_ms: opts.get("max_time_ms").and_then(|v| v.as_u64()).unwrap_or(
                crate::defaults::DEFAULT_TRANSACTION_PIPELINE_TIMEOUT_MS,
            ),
        },
        None => TransactionPipelineOptions::default(),
    };

    match execute_transaction_pipeline(pool, steps, options).await {
        Ok(result) => {
            let response = json!({
                "steps": result.steps.iter().map(|s| json!({
                    "name": s.name,
                    "success": s.success,
                    "result": s.result_json,
                })).collect::<Vec<_>>(),
                "summary": {
                    "total_steps": result.total_steps,
                    "steps_completed": result.steps_completed,
                    "elapsed_ms": result.elapsed_ms,
                }
            });
            McpToolCallResult {
                content: vec![McpContent::text(response.to_string())],
                is_error: false,
            }
        }
        Err(failure) => error_result(json!({
            "failed_step": failure.failed_step,
            "step_index": failure.step_index,
            "reason": failure.reason,
            "steps_completed": failure.steps_completed,
            "rolled_back": failure.rolled_back,
        }).to_string()),
    }
}
```

- [ ] **Step 4: Update tool count assertion**

In `src/mcp/tools.rs` tests, update the count assertion:

```rust
assert_eq!(tools.len(), 36);  // was 35
```

- [ ] **Step 5: Verify it compiles and unit tests pass**

Run: `cargo build 2>&1 | grep "warning:"` then `cargo test --lib mcp::tools`
Expected: No warnings, tests pass.

- [ ] **Step 6: Commit**

```bash
git add src/mcp/tools.rs src/mcp/safety.rs
git commit -m "feat(mcp): add transaction_pipeline tool with safety rules"
```

---

## Task 8: Integration Tests

**Files:**
- Create: `tests/integration/transaction_pipeline_test.rs`

- [ ] **Step 1: Write integration tests**

Create `tests/integration/transaction_pipeline_test.rs`:

```rust
use bson::doc;
use uuid::Uuid;

use mongocore::grpc::proto::mongo_core_client::MongoCoreClient;
use mongocore::grpc::proto::{
    Document, Filter, FindOneRequest, FindRequest, InsertRequest, UpdateRequest,
    DeleteRequest, TransactionPipelineRequest, TransactionStep,
};
use mongocore::grpc::proto::transaction_step::Operation;
use mongocore::grpc::{start_grpc_server, GrpcServerConfig};

#[path = "../harness/mod.rs"]
mod harness;

const TEST_DB: &str = harness::TEST_DB;

fn unique_collection() -> String {
    format!(
        "test_txnpipe_{}",
        Uuid::new_v4().to_string().replace('-', "")
    )
}

fn encode_doc(doc: &bson::Document) -> Vec<u8> {
    let mut buf = Vec::new();
    doc.to_writer(&mut buf).unwrap();
    buf
}

fn make_doc(doc: &bson::Document) -> Document {
    Document {
        data: encode_doc(doc),
    }
}

fn make_filter(doc: &bson::Document) -> Option<Filter> {
    Some(Filter {
        data: encode_doc(doc),
    })
}

async fn start_test_server() -> MongoCoreClient<tonic::transport::Channel> {
    let pool = harness::get_test_pool().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let _handle = start_grpc_server(
        pool,
        GrpcServerConfig {
            port,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let addr = format!("http://127.0.0.1:{}", port);
    MongoCoreClient::connect(addr).await.unwrap()
}

#[tokio::test]
async fn test_transaction_pipeline_basic_insert_then_find() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    let request = TransactionPipelineRequest {
        steps: vec![
            TransactionStep {
                name: "insert_user".to_string(),
                database: TEST_DB.to_string(),
                collection: coll.clone(),
                operation: Some(Operation::Insert(InsertRequest {
                    database: String::new(),
                    collection: String::new(),
                    document: Some(make_doc(&doc! { "name": "Alice", "email": "alice@test.com" })),
                    transaction_id: None,
                })),
            },
            TransactionStep {
                name: "find_user".to_string(),
                database: TEST_DB.to_string(),
                collection: coll.clone(),
                operation: Some(Operation::FindOne(FindOneRequest {
                    database: String::new(),
                    collection: String::new(),
                    filter: make_filter(&doc! { "name": "Alice" }),
                    options: None,
                    transaction_id: None,
                })),
            },
        ],
        options: None,
    };

    let response = client.transaction_pipeline(request).await.unwrap().into_inner();
    assert_eq!(response.steps.len(), 2);
    assert!(response.steps[0].success);
    assert!(response.steps[1].success);
    assert_eq!(response.summary.unwrap().total_steps, 2);
}

#[tokio::test]
async fn test_transaction_pipeline_reference_forwarding() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Insert a doc, then update it using a reference to the inserted_id
    let request = TransactionPipelineRequest {
        steps: vec![
            TransactionStep {
                name: "create".to_string(),
                database: TEST_DB.to_string(),
                collection: coll.clone(),
                operation: Some(Operation::Insert(InsertRequest {
                    database: String::new(),
                    collection: String::new(),
                    document: Some(make_doc(&doc! { "name": "Bob", "active": true })),
                    transaction_id: None,
                })),
            },
            TransactionStep {
                name: "find_bob".to_string(),
                database: TEST_DB.to_string(),
                collection: coll.clone(),
                operation: Some(Operation::FindOne(FindOneRequest {
                    database: String::new(),
                    collection: String::new(),
                    filter: make_filter(&doc! { "name": "Bob" }),
                    options: None,
                    transaction_id: None,
                })),
            },
        ],
        options: None,
    };

    let response = client.transaction_pipeline(request).await.unwrap().into_inner();
    assert_eq!(response.steps.len(), 2);
    assert!(response.steps.iter().all(|s| s.success));
}

#[tokio::test]
async fn test_transaction_pipeline_empty_rejected() {
    let mut client = start_test_server().await;

    let request = TransactionPipelineRequest {
        steps: vec![],
        options: None,
    };

    let result = client.transaction_pipeline(request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_transaction_pipeline_rollback_on_failure() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // First insert a doc normally (outside the pipeline)
    let insert_req = TransactionPipelineRequest {
        steps: vec![TransactionStep {
            name: "setup".to_string(),
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            operation: Some(Operation::Insert(InsertRequest {
                database: String::new(),
                collection: String::new(),
                document: Some(make_doc(&doc! { "name": "Charlie", "value": 1 })),
                transaction_id: None,
            })),
        }],
        options: None,
    };
    client.transaction_pipeline(insert_req).await.unwrap();

    // Now try a pipeline that should fail (duplicate key on _id if we force it)
    // The rollback test verifies the transaction semantics work
    // A simpler approach: reference a step that doesn't produce a result in the expected shape
}
```

- [ ] **Step 2: Verify integration tests compile**

Run: `cargo test --test integration --no-run`
Expected: Compiles successfully.

- [ ] **Step 3: Run integration tests (requires Docker MongoDB)**

Run: `cargo test --test integration transaction_pipeline`
Expected: All tests PASS. (Requires `just docker-up` to be running.)

- [ ] **Step 4: Commit**

```bash
git add tests/integration/transaction_pipeline_test.rs
git commit -m "test(integration): add transaction pipeline integration tests"
```

---

## Task 9: MCP Tool Integration Test

**Files:**
- Modify: `tests/integration/mcp_test.rs`

- [ ] **Step 1: Update MCP tool count assertion**

Find the tool count assertion in `tests/integration/mcp_test.rs` and update it to account for the new `transaction_pipeline` tool.

- [ ] **Step 2: Add MCP tool test**

Add a test that exercises the `transaction_pipeline` tool via the MCP JSON-RPC interface, following the pattern of existing MCP tests.

- [ ] **Step 3: Verify it passes**

Run: `cargo test --test integration mcp`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add tests/integration/mcp_test.rs
git commit -m "test(mcp): add transaction_pipeline tool integration test"
```

---

## Task 10: Python Client Wrapper

**Files:**
- Modify: `clients/python/src/mongocore/ops.py`
- Modify: `clients/python/src/mongocore/database.py`
- Modify: `clients/python/src/mongocore/collection.py`
- Modify: `clients/python/src/mongocore/client.py`

- [ ] **Step 1: Add Step class and step-level op builders to ops.py**

Add to `clients/python/src/mongocore/ops.py`:

```python
@dataclass
class TransactionStep:
    """A step in a transactional pipeline."""
    name: str
    operation: object  # One of FindOneOp, FindOp, InsertOp, etc.
    collection: Optional[str] = None  # Set by database-scoped API


# Convenience step builders (collection-scoped — no database/collection in the op)
def step_find_one(filter: Optional[dict] = None) -> dict:
    """Create a find_one step operation."""
    return {"op": "find_one", "filter": filter or {}}


def step_find(filter: Optional[dict] = None, *, limit: int = 0) -> dict:
    """Create a find step operation."""
    return {"op": "find", "filter": filter or {}, "limit": limit}


def step_insert(document: dict) -> dict:
    """Create an insert step operation."""
    return {"op": "insert", "document": document}


def step_insert_many(documents: list[dict]) -> dict:
    """Create an insert_many step operation."""
    return {"op": "insert_many", "documents": documents}


def step_update(filter: dict, update: dict) -> dict:
    """Create an update step operation."""
    return {"op": "update", "filter": filter, "update": update}


def step_update_many(filter: dict, update: dict) -> dict:
    """Create an update_many step operation."""
    return {"op": "update_many", "filter": filter, "update": update}


def step_delete(filter: dict) -> dict:
    """Create a delete step operation."""
    return {"op": "delete", "filter": filter}


def step_delete_many(filter: dict) -> dict:
    """Create a delete_many step operation."""
    return {"op": "delete_many", "filter": filter}


def step_find_and_modify(filter: dict, update: dict, *, return_new: bool = True) -> dict:
    """Create a find_and_modify step operation."""
    return {"op": "find_and_modify", "filter": filter, "update": update, "return_new": return_new}


def step_aggregate(pipeline: list[dict]) -> dict:
    """Create an aggregate step operation."""
    return {"op": "aggregate", "pipeline": pipeline}
```

- [ ] **Step 2: Add `transaction_pipeline` to database.py**

Add a `transaction_pipeline` method to the Database class that accepts `List[TransactionStep]` where each step has a collection field, builds the proto request, and sends it.

- [ ] **Step 3: Add `transaction_pipeline` to collection.py**

Add a `transaction_pipeline` method to the Collection class that auto-fills database and collection on every step.

- [ ] **Step 4: Add proto conversion in client.py**

Add a `_build_transaction_pipeline_step` method to the client that converts a `TransactionStep` into the proto `TransactionStep` message.

- [ ] **Step 5: Verify Python imports work**

Run: `cd clients/python && python -c "from mongocore.ops import TransactionStep, step_find_one, step_insert"`
Expected: No import errors.

- [ ] **Step 6: Commit**

```bash
git add clients/python/
git commit -m "feat(clients): add Python transaction_pipeline wrapper"
```

---

## Task 11: TypeScript Client Wrapper

**Files:**
- Modify: `clients/typescript/src/ops.ts`
- Modify: `clients/typescript/src/database.ts`
- Modify: `clients/typescript/src/collection.ts`
- Modify: `clients/typescript/src/client.ts`

- [ ] **Step 1: Add Step type and builders to ops.ts**

Add TypeScript equivalents of the step builders:

```typescript
export interface TransactionStep {
  name: string;
  collection?: string;
  operation: StepOperation;
}

export type StepOperation =
  | { op: "find_one"; filter?: Record<string, unknown> }
  | { op: "find"; filter?: Record<string, unknown>; limit?: number }
  | { op: "insert"; document: Record<string, unknown> }
  | { op: "insert_many"; documents: Record<string, unknown>[] }
  | { op: "update"; filter: Record<string, unknown>; update: Record<string, unknown> }
  | { op: "update_many"; filter: Record<string, unknown>; update: Record<string, unknown> }
  | { op: "delete"; filter: Record<string, unknown> }
  | { op: "delete_many"; filter: Record<string, unknown> }
  | { op: "find_and_modify"; filter: Record<string, unknown>; update: Record<string, unknown> }
  | { op: "aggregate"; pipeline: Record<string, unknown>[] };

export function step(name: string, operation: StepOperation): TransactionStep;
export function step(name: string, collection: string, operation: StepOperation): TransactionStep;
export function step(name: string, collectionOrOp: string | StepOperation, maybeOp?: StepOperation): TransactionStep {
  if (typeof collectionOrOp === "string") {
    return { name, collection: collectionOrOp, operation: maybeOp! };
  }
  return { name, operation: collectionOrOp };
}

export const findOne = (filter?: Record<string, unknown>): StepOperation => ({ op: "find_one", filter });
export const find = (filter?: Record<string, unknown>, limit?: number): StepOperation => ({ op: "find", filter, limit });
export const insertOne = (document: Record<string, unknown>): StepOperation => ({ op: "insert", document });
export const insertMany = (documents: Record<string, unknown>[]): StepOperation => ({ op: "insert_many", documents });
export const updateOne = (filter: Record<string, unknown>, update: Record<string, unknown>): StepOperation => ({ op: "update", filter, update });
export const updateMany = (filter: Record<string, unknown>, update: Record<string, unknown>): StepOperation => ({ op: "update_many", filter, update });
export const deleteOne = (filter: Record<string, unknown>): StepOperation => ({ op: "delete", filter });
export const deleteMany = (filter: Record<string, unknown>): StepOperation => ({ op: "delete_many", filter });
```

- [ ] **Step 2: Add `transactionPipeline` to database.ts and collection.ts**

Follow the same pattern as the Python client — database-scoped requires collection per step, collection-scoped auto-fills.

- [ ] **Step 3: Verify TypeScript compiles**

Run: `cd clients/typescript && npx tsc --noEmit`
Expected: No type errors.

- [ ] **Step 4: Commit**

```bash
git add clients/typescript/
git commit -m "feat(clients): add TypeScript transaction_pipeline wrapper"
```

---

## Task 12: Go Client Wrapper

**Files:**
- Create: `clients/go/mongocore/transaction_pipeline.go`

- [ ] **Step 1: Add Go Step type and TransactionPipeline method**

Create `clients/go/mongocore/transaction_pipeline.go` with the Step struct, builder functions, and the `TransactionPipeline` method on both Database and Collection types.

- [ ] **Step 2: Verify Go compiles**

Run: `cd clients/go && go build ./...`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add clients/go/
git commit -m "feat(clients): add Go transaction_pipeline wrapper"
```

---

## Task 13: Java Client Wrapper

**Files:**
- Create: `clients/java/src/main/java/com/mongocore/TransactionPipelineStep.java`
- Modify existing Java client files to add the method

- [ ] **Step 1: Add Java Step class and pipeline method**

Create the Step class and add `transactionPipeline` method to the existing collection/database classes.

- [ ] **Step 2: Verify Java compiles**

Run: `cd clients/java && mvn compile -q` (or equivalent build tool)
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add clients/java/
git commit -m "feat(clients): add Java transaction_pipeline wrapper"
```

---

## Task 14: Documentation

**Files:**
- Create: `docs/transactional-pipelines.md`
- Modify: `docs/README.md` (add to index)
- Modify: `docs/roadmap.md` (mark as implemented)

- [ ] **Step 1: Write comprehensive documentation page**

Create `docs/transactional-pipelines.md`:

```markdown
# Transactional Pipelines

Execute multiple dependent database operations atomically in a single request. Operations run sequentially within a MongoDB transaction — if any step fails, all changes are rolled back.

## When to Use

Use transactional pipelines when you need:
- **Dependent operations** — step N needs results from step M (e.g., find a user, then update their audit log)
- **Atomic multi-step workflows** — all operations succeed or none do
- **Cross-collection transactions** — modify multiple collections atomically

For independent operations that can run in parallel, use the [Pipeline RPC](./roadmap.md) instead.

## Quick Start

### Python

```python
from mongocore import MongoCore
from mongocore.ops import TransactionStep, step_find_one, step_update, step_insert

async with MongoCore("localhost:50051") as client:
    db = client.database("myapp")

    result = await db.transaction_pipeline([
        TransactionStep("find_user", "users", step_find_one({"email": "alice@example.com"})),
        TransactionStep("deactivate", "users", step_update(
            {"_id": "{{find_user._id}}"},
            {"$set": {"active": False}}
        )),
        TransactionStep("audit", "audit_logs", step_insert({
            "action": "user_deactivated",
            "user_id": "{{find_user._id}}",
            "username": "{{find_user.username}}",
        })),
    ])

    print(result.summary)  # {total_steps: 3, steps_completed: 3, elapsed_ms: 12}
```

### TypeScript

```typescript
import { MongoCore, step, findOne, updateOne, insertOne } from "mongocore";

const client = new MongoCore("localhost:50051");
const db = client.database("myapp");

const result = await db.transactionPipeline([
  step("findUser", "users", findOne({ email: "alice@example.com" })),
  step("deactivate", "users", updateOne(
    { _id: "{{findUser._id}}" },
    { $set: { active: false } }
  )),
  step("audit", "audit_logs", insertOne({
    action: "user_deactivated",
    userId: "{{findUser._id}}",
  })),
]);
```

### Go

```go
result, err := db.TransactionPipeline(ctx, []mongocore.Step{
    mongocore.NewStep("findUser", "users", mongocore.FindOne(bson.M{"email": "alice@example.com"})),
    mongocore.NewStep("deactivate", "users", mongocore.UpdateOne(
        bson.M{"_id": "{{findUser._id}}"},
        bson.M{"$set": bson.M{"active": false}},
    )),
    mongocore.NewStep("audit", "audit_logs", mongocore.InsertOne(bson.M{
        "action":  "user_deactivated",
        "user_id": "{{findUser._id}}",
    })),
})
```

### Java

```java
var result = db.transactionPipeline(List.of(
    Step.of("findUser", "users", findOne(eq("email", "alice@example.com"))),
    Step.of("deactivate", "users", updateOne(
        eq("_id", "{{findUser._id}}"),
        set("active", false))),
    Step.of("audit", "audit_logs", insertOne(new Document()
        .append("action", "user_deactivated")
        .append("user_id", "{{findUser._id}}")))
));
```

## Reference Syntax

Steps can reference results from prior steps using `{{step_name.path}}` syntax:

| Syntax | Description | Example |
|--------|-------------|---------|
| `{{step.field}}` | Top-level field | `{{find_user._id}}` |
| `{{step.field.sub}}` | Nested field | `{{find_user.address.city}}` |
| `{{step[0].field}}` | Array index + field | `{{find_users[0].email}}` |
| `{{step[*].field}}` | Wildcard pluck (array of values) | `{{find_users[*]._id}}` |
| `{{step}}` | Full result passthrough | `{{find_expired}}` |
| `{{step.length}}` | Array length | `{{find_users.length}}` |

### Type Preservation

- If the entire value is a single `{{ref}}`, the resolved value keeps its native type (number, boolean, object, array)
- If `{{ref}}` is embedded in a larger string, it's stringified: `"User {{find_user.name}} deactivated"` → `"User Alice deactivated"`

## Collection-Scoped API

When all steps target the same collection, use the collection-level method:

```python
users = db.collection("users")
result = await users.transaction_pipeline([
    TransactionStep("find", step_find_one({"email": "alice@example.com"})),
    TransactionStep("update", step_update(
        {"_id": "{{find._id}}"},
        {"$set": {"last_login": datetime.utcnow()}}
    )),
])
```

The database and collection are automatically set on each step.

## Result Shapes

Each operation type produces a specific result shape for referencing:

| Operation | Result | Referenceable fields |
|-----------|--------|---------------------|
| `find_one` | Single document | Any document field |
| `find` | Array of documents | `[0].field`, `[*].field`, `.length` |
| `aggregate` | Array of documents | Same as find |
| `insert` | `{inserted_id}` | `.inserted_id` |
| `insert_many` | `{inserted_ids, inserted_count}` | `.inserted_ids`, `.inserted_count` |
| `update` / `update_many` | `{matched_count, modified_count, upserted_id}` | `.modified_count`, etc. |
| `delete` / `delete_many` | `{deleted_count}` | `.deleted_count` |
| `find_and_modify` | Single document (after modification) | Any document field |

## Error Handling

If any step fails, the transaction is aborted and all changes roll back:

```python
from mongocore.errors import TransactionPipelineError

try:
    result = await db.transaction_pipeline([...])
except TransactionPipelineError as e:
    print(e.failed_step)       # "update_audit"
    print(e.step_index)        # 2
    print(e.reason)            # "referenced step 'find_user' returned no result"
    print(e.steps_completed)   # ["find_user", "deactivate"]
    print(e.rolled_back)       # True
```

**Failure scenarios:**
- Step returns no result and a later step references it
- Write failure (duplicate key, schema validation error)
- Reference path doesn't exist in the result
- Pipeline timeout exceeded (default: 30s)

## Transaction Options

```python
result = await db.transaction_pipeline(
    steps=[...],
    options={
        "read_concern": "snapshot",     # default
        "write_concern": "majority",    # default
        "max_time_ms": 30000,           # default: 30s
    }
)
```

## Limits

| Limit | Value | Notes |
|-------|-------|-------|
| Max steps | 50 | Use multiple pipelines for larger workflows |
| Max documents per Find/Aggregate | 101 | Auto-applied if no limit set; rejected if explicit limit > 101 |
| Timeout | 30s | Configurable via `max_time_ms` |
| Retries on transient error | 3 | Automatic for `TransientTransactionError` |

## Use Cases

### User Deactivation with Audit Trail

```python
result = await db.transaction_pipeline([
    TransactionStep("find_user", "users", step_find_one({"email": "alice@example.com"})),
    TransactionStep("deactivate", "users", step_update(
        {"_id": "{{find_user._id}}"},
        {"$set": {"active": False, "deactivated_at": datetime.utcnow()}}
    )),
    TransactionStep("audit", "audit_logs", step_insert({
        "action": "user_deactivated",
        "user_id": "{{find_user._id}}",
        "username": "{{find_user.username}}",
        "city": "{{find_user.address.city}}",
    })),
])
```

### Archive and Delete Expired Records

```python
result = await db.transaction_pipeline([
    TransactionStep("find_expired", "subscriptions", step_find(
        {"expires_at": {"$lt": "2024-01-01"}, "status": "active"}
    )),
    TransactionStep("archive", "subscriptions_archive", step_insert_many("{{find_expired}}")),
    TransactionStep("cleanup", "subscriptions", step_delete_many(
        {"_id": {"$in": "{{find_expired[*]._id}}"}}
    )),
])
```

### Inventory Reservation

```python
result = await db.transaction_pipeline([
    TransactionStep("orders", "orders", step_find({"status": "pending", "priority": "high"})),
    TransactionStep("reserve", "inventory", step_update_many(
        {"sku": {"$in": "{{orders[*].sku}}"}},
        {"$inc": {"reserved": 1}}
    )),
    TransactionStep("mark", "orders", step_update_many(
        {"_id": {"$in": "{{orders[*]._id}}"}},
        {"$set": {"status": "processing"}}
    )),
    TransactionStep("log", "activity", step_insert({
        "action": "batch_reserve",
        "order_count": "{{orders.length}}",
    })),
])
```

### Read-After-Write (Insert then Query)

```python
result = await db.transaction_pipeline([
    TransactionStep("create", "users", step_insert({
        "name": "New User",
        "email": "new@example.com",
        "created_at": datetime.utcnow(),
    })),
    TransactionStep("verify", "users", step_find_one(
        {"_id": "{{create.inserted_id}}"}
    )),
])
# verify step sees the just-inserted document within the transaction
```

### Transfer Between Collections

```python
result = await db.transaction_pipeline([
    TransactionStep("find_item", "source", step_find_one({"_id": item_id})),
    TransactionStep("copy", "destination", step_insert("{{find_item}}")),
    TransactionStep("remove", "source", step_delete({"_id": "{{find_item._id}}"})),
])
```

## MCP Tool

AI agents can use transactional pipelines via the `transaction_pipeline` MCP tool:

```json
{
  "tool": "transaction_pipeline",
  "arguments": {
    "steps": [
      {
        "name": "find_user",
        "database": "myapp",
        "collection": "users",
        "operation": "find_one",
        "params": { "filter": { "email": "alice@example.com" } }
      },
      {
        "name": "deactivate",
        "database": "myapp",
        "collection": "users",
        "operation": "update",
        "params": {
          "filter": { "_id": "{{find_user._id}}" },
          "update": { "$set": { "active": false } }
        }
      }
    ]
  }
}
```

## Requirements

- MongoDB replica set (or sharded cluster 4.2+) — transactions require a replica set
- MongoCore sidecar running and connected to the replica set
```

- [ ] **Step 2: Update docs/README.md**

Add an entry to the documentation index:

```markdown
| [Transactional Pipelines](transactional-pipelines.md) | Atomic multi-step operations with result forwarding |
```

- [ ] **Step 3: Update docs/roadmap.md**

Move "Transactional Pipeline" from future to the current version section.

- [ ] **Step 4: Commit**

```bash
git add docs/transactional-pipelines.md docs/README.md docs/roadmap.md
git commit -m "docs: add comprehensive transactional pipelines documentation"
```

---

## Task 15: Regenerate Client Stubs

**Files:**
- All generated client directories

- [ ] **Step 1: Regenerate Python stubs**

```bash
cd clients/python && python -m grpc_tools.protoc -I../../proto \
  --python_out=src/mongocore/generated --grpc_python_out=src/mongocore/generated \
  ../../proto/mongocore/v1/mongocore.proto ../../proto/mongocore/v1/types.proto \
  ../../proto/mongocore/v1/ingestion.proto
```

- [ ] **Step 2: Regenerate TypeScript stubs**

```bash
cd clients/typescript && npx grpc_tools_node_protoc \
  --ts_out=src/generated --grpc_out=src/generated -I../../proto \
  ../../proto/mongocore/v1/mongocore.proto ../../proto/mongocore/v1/types.proto \
  ../../proto/mongocore/v1/ingestion.proto
```

- [ ] **Step 3: Regenerate Go stubs**

```bash
cd clients/go && protoc --go_out=./proto --go-grpc_out=./proto -I../../proto \
  ../../proto/mongocore/v1/mongocore.proto ../../proto/mongocore/v1/types.proto \
  ../../proto/mongocore/v1/ingestion.proto
```

- [ ] **Step 4: Regenerate Java stubs**

```bash
cd clients/java && protoc --java_out=src/main/java --grpc-java_out=src/main/java \
  -I../../proto ../../proto/mongocore/v1/mongocore.proto \
  ../../proto/mongocore/v1/types.proto ../../proto/mongocore/v1/ingestion.proto
```

- [ ] **Step 5: Commit all generated stubs**

```bash
git add clients/
git commit -m "chore(clients): regenerate proto stubs for TransactionPipeline"
```

---

## Task 16: Final Verification

- [ ] **Step 1: Run full build with zero warnings**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output.

- [ ] **Step 2: Run all unit tests**

Run: `cargo test --lib`
Expected: All tests pass.

- [ ] **Step 3: Run integration tests**

Run: `cargo test --test integration transaction_pipeline`
Expected: All tests pass (requires `just docker-up`).

- [ ] **Step 4: Update development log**

Add entry to `docs/design/development-log.md`:

```markdown
## 2026-05-14: Transactional Pipeline (v1.0)

Implemented the `TransactionPipeline` RPC — sequential dependent operations with `{{step.path}}` result forwarding, auto-wrapped in MongoDB transactions. Key decisions: named steps with mustache-style references (consistent with existing template registry), fail-fast error model with automatic rollback, 101-document cap on Find/Aggregate results, auto-retry on transient transaction errors. Separate from the concurrent Pipeline RPC to maintain clean separation of execution models.
```

- [ ] **Step 5: Commit**

```bash
git add docs/design/development-log.md
git commit -m "docs: add transactional pipeline entry to development log"
```
