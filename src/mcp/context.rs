use serde_json::{json, Value};

const MAX_SERIALIZED_SIZE: usize = 1024;
const MAX_PIPELINE_STAGES: usize = 5;

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
    let obj = ctx.as_object_mut().expect("context is always an object");
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
    let obj = ctx.as_object_mut().expect("context is always an object");

    if let Some(docs) = args.get("documents").and_then(|d| d.as_array()) {
        obj.insert("document_count".to_string(), json!(docs.len()));
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
    let obj = ctx.as_object_mut().expect("context is always an object");
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
    let obj = ctx.as_object_mut().expect("context is always an object");
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
    let obj = ctx.as_object_mut().expect("context is always an object");
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
    let obj = ctx.as_object_mut().expect("context is always an object");
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
    let obj = ctx.as_object_mut().expect("context is always an object");
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
    let obj = ctx.as_object_mut().expect("context is always an object");
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
    let obj = ctx.as_object_mut().expect("context is always an object");
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
    let obj = ctx.as_object_mut().expect("context is always an object");
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
    let obj = ctx.as_object_mut().expect("context is always an object");
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
    let obj = ctx.as_object_mut().expect("context is always an object");
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
    let obj = ctx.as_object_mut().expect("context is always an object");
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
    let obj = ctx.as_object_mut().expect("context is always an object");
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

fn truncate_value(value: &Value) -> Value {
    let serialized = serde_json::to_string(value).unwrap_or_default();
    if serialized.len() <= MAX_SERIALIZED_SIZE {
        return value.clone();
    }
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
