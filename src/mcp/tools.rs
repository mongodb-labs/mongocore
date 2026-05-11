use bson::Document;
use serde_json::{json, Value};

use crate::connection::pool::ConnectionPool;
use crate::operations::{FindOptions, IndexOptions, Operations};

use super::types::{McpContent, McpToolCallResult, McpToolDefinition};

/// Maximum number of documents returned by find operations (safety control for AI agents).
const MAX_FIND_LIMIT: i64 = 100;

/// Return all MCP tool definitions with their JSON Schema input schemas.
pub fn tool_definitions() -> Vec<McpToolDefinition> {
    vec![
        McpToolDefinition {
            name: "find".to_string(),
            description: "Find documents in a MongoDB collection matching a filter".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "filter": { "type": "object", "description": "Query filter (default: {})" },
                    "limit": { "type": "integer", "description": "Maximum documents to return (max 100)" },
                    "skip": { "type": "integer", "description": "Number of documents to skip" }
                },
                "required": ["database", "collection"]
            }),
        },
        McpToolDefinition {
            name: "find_one".to_string(),
            description: "Find a single document in a MongoDB collection matching a filter".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "filter": { "type": "object", "description": "Query filter (default: {})" }
                },
                "required": ["database", "collection"]
            }),
        },
        McpToolDefinition {
            name: "insert".to_string(),
            description: "Insert a single document into a MongoDB collection".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "document": { "type": "object", "description": "Document to insert" }
                },
                "required": ["database", "collection", "document"]
            }),
        },
        McpToolDefinition {
            name: "insert_many".to_string(),
            description: "Insert multiple documents into a MongoDB collection".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "documents": {
                        "type": "array",
                        "items": { "type": "object" },
                        "description": "Array of documents to insert"
                    }
                },
                "required": ["database", "collection", "documents"]
            }),
        },
        McpToolDefinition {
            name: "update".to_string(),
            description: "Update the first document matching a filter in a MongoDB collection".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "filter": { "type": "object", "description": "Query filter to match documents" },
                    "update": { "type": "object", "description": "Update operations to apply" }
                },
                "required": ["database", "collection", "filter", "update"]
            }),
        },
        McpToolDefinition {
            name: "update_many".to_string(),
            description: "Update all documents matching a filter in a MongoDB collection".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "filter": { "type": "object", "description": "Query filter to match documents" },
                    "update": { "type": "object", "description": "Update operations to apply" }
                },
                "required": ["database", "collection", "filter", "update"]
            }),
        },
        McpToolDefinition {
            name: "delete".to_string(),
            description: "Delete the first document matching a filter in a MongoDB collection".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "filter": { "type": "object", "description": "Query filter to match the document to delete" }
                },
                "required": ["database", "collection", "filter"]
            }),
        },
        McpToolDefinition {
            name: "delete_many".to_string(),
            description: "Delete all documents matching a filter in a MongoDB collection".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "filter": { "type": "object", "description": "Query filter to match documents to delete" }
                },
                "required": ["database", "collection", "filter"]
            }),
        },
        McpToolDefinition {
            name: "aggregate".to_string(),
            description: "Execute an aggregation pipeline on a MongoDB collection".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "pipeline": {
                        "type": "array",
                        "items": { "type": "object" },
                        "description": "Array of aggregation pipeline stages"
                    }
                },
                "required": ["database", "collection", "pipeline"]
            }),
        },
        McpToolDefinition {
            name: "create_collection".to_string(),
            description: "Create a new collection in a MongoDB database".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name to create" }
                },
                "required": ["database", "collection"]
            }),
        },
        McpToolDefinition {
            name: "create_index".to_string(),
            description: "Create an index on a MongoDB collection".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" },
                    "collection": { "type": "string", "description": "Collection name" },
                    "keys": { "type": "object", "description": "Index key specification (e.g. {\"field\": 1})" },
                    "unique": { "type": "boolean", "description": "Whether the index enforces uniqueness" }
                },
                "required": ["database", "collection", "keys"]
            }),
        },
        McpToolDefinition {
            name: "list_databases".to_string(),
            description: "List all databases on the MongoDB server".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        },
        McpToolDefinition {
            name: "list_collections".to_string(),
            description: "List all collections in a MongoDB database".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "database": { "type": "string", "description": "Database name" }
                },
                "required": ["database"]
            }),
        },
    ]
}

/// Execute an MCP tool by name with the given JSON arguments.
pub async fn execute_tool(
    operations: &Operations,
    pool: &ConnectionPool,
    name: &str,
    arguments: &Value,
) -> McpToolCallResult {
    match name {
        "find" => execute_find(operations, arguments).await,
        "find_one" => execute_find_one(operations, arguments).await,
        "insert" => execute_insert(operations, arguments).await,
        "insert_many" => execute_insert_many(operations, arguments).await,
        "update" => execute_update(operations, arguments).await,
        "update_many" => execute_update_many(operations, arguments).await,
        "delete" => execute_delete(operations, arguments).await,
        "delete_many" => execute_delete_many(operations, arguments).await,
        "aggregate" => execute_aggregate(operations, arguments).await,
        "create_collection" => execute_create_collection(operations, arguments).await,
        "create_index" => execute_create_index(operations, arguments).await,
        "list_databases" => execute_list_databases(pool).await,
        "list_collections" => execute_list_collections(pool, arguments).await,
        _ => error_result(format!("Unknown tool: {}", name)),
    }
}

// --- Helper functions ---

fn error_result(message: String) -> McpToolCallResult {
    McpToolCallResult {
        content: vec![McpContent {
            type_: "text".to_string(),
            text: message,
        }],
        is_error: true,
    }
}

fn success_result(text: String) -> McpToolCallResult {
    McpToolCallResult {
        content: vec![McpContent {
            type_: "text".to_string(),
            text,
        }],
        is_error: false,
    }
}

fn get_str<'a>(args: &'a Value, field: &str) -> Result<&'a str, McpToolCallResult> {
    args.get(field)
        .and_then(|v| v.as_str())
        .ok_or_else(|| error_result(format!("Missing required field: {}", field)))
}

fn json_to_document(value: &Value) -> Result<Document, McpToolCallResult> {
    let bson_val = bson::to_bson(value)
        .map_err(|e| error_result(format!("Failed to convert to BSON: {}", e)))?;
    bson_val
        .as_document()
        .cloned()
        .ok_or_else(|| error_result("Expected a JSON object convertible to BSON document".to_string()))
}

fn json_to_documents(value: &Value) -> Result<Vec<Document>, McpToolCallResult> {
    let arr = value
        .as_array()
        .ok_or_else(|| error_result("Expected a JSON array".to_string()))?;
    arr.iter().map(json_to_document).collect()
}

// --- Tool executors ---

async fn execute_find(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let filter = if let Some(f) = args.get("filter") {
        match json_to_document(f) {
            Ok(d) => d,
            Err(e) => return e,
        }
    } else {
        Document::new()
    };

    let mut limit = args
        .get("limit")
        .and_then(|v| v.as_i64())
        .unwrap_or(MAX_FIND_LIMIT);
    if limit > MAX_FIND_LIMIT || limit <= 0 {
        limit = MAX_FIND_LIMIT;
    }

    let skip = args.get("skip").and_then(|v| v.as_u64());

    let options = Some(FindOptions {
        limit: Some(limit),
        skip,
        sort: None,
        projection: None,
    });

    match operations.find(db, coll, filter, options).await {
        Ok(docs) => {
            let json_docs: Vec<Value> = docs
                .iter()
                .map(|d| serde_json::to_value(d).unwrap_or(Value::Null))
                .collect();
            let text = serde_json::to_string_pretty(&json_docs)
                .unwrap_or_else(|_| "[]".to_string());
            success_result(text)
        }
        Err(e) => error_result(format!("find failed: {}", e)),
    }
}

async fn execute_find_one(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let filter = if let Some(f) = args.get("filter") {
        match json_to_document(f) {
            Ok(d) => d,
            Err(e) => return e,
        }
    } else {
        Document::new()
    };

    match operations.find_one(db, coll, filter).await {
        Ok(Some(doc)) => {
            let text = serde_json::to_string_pretty(&doc)
                .unwrap_or_else(|_| "null".to_string());
            success_result(text)
        }
        Ok(None) => success_result("null".to_string()),
        Err(e) => error_result(format!("find_one failed: {}", e)),
    }
}

async fn execute_insert(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let document = match args.get("document") {
        Some(d) => match json_to_document(d) {
            Ok(doc) => doc,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: document".to_string()),
    };

    match operations.insert(db, coll, document).await {
        Ok(result) => {
            let text = json!({
                "insertedId": result.inserted_id.to_string()
            })
            .to_string();
            success_result(text)
        }
        Err(e) => error_result(format!("insert failed: {}", e)),
    }
}

async fn execute_insert_many(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let documents = match args.get("documents") {
        Some(d) => match json_to_documents(d) {
            Ok(docs) => docs,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: documents".to_string()),
    };

    match operations.insert_many(db, coll, documents).await {
        Ok(result) => {
            let ids: Vec<String> = result
                .inserted_ids
                .values()
                .map(|id| id.to_string())
                .collect();
            let text = json!({
                "insertedIds": ids,
                "insertedCount": ids.len()
            })
            .to_string();
            success_result(text)
        }
        Err(e) => error_result(format!("insert_many failed: {}", e)),
    }
}

async fn execute_update(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let filter = match args.get("filter") {
        Some(f) => match json_to_document(f) {
            Ok(d) => d,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: filter".to_string()),
    };

    let update = match args.get("update") {
        Some(u) => match json_to_document(u) {
            Ok(d) => d,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: update".to_string()),
    };

    match operations.update(db, coll, filter, update).await {
        Ok(result) => {
            let text = json!({
                "matchedCount": result.matched_count,
                "modifiedCount": result.modified_count
            })
            .to_string();
            success_result(text)
        }
        Err(e) => error_result(format!("update failed: {}", e)),
    }
}

async fn execute_update_many(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let filter = match args.get("filter") {
        Some(f) => match json_to_document(f) {
            Ok(d) => d,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: filter".to_string()),
    };

    let update = match args.get("update") {
        Some(u) => match json_to_document(u) {
            Ok(d) => d,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: update".to_string()),
    };

    match operations.update_many(db, coll, filter, update).await {
        Ok(result) => {
            let text = json!({
                "matchedCount": result.matched_count,
                "modifiedCount": result.modified_count
            })
            .to_string();
            success_result(text)
        }
        Err(e) => error_result(format!("update_many failed: {}", e)),
    }
}

async fn execute_delete(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let filter = match args.get("filter") {
        Some(f) => match json_to_document(f) {
            Ok(d) => d,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: filter".to_string()),
    };

    match operations.delete(db, coll, filter).await {
        Ok(result) => {
            let text = json!({
                "deletedCount": result.deleted_count
            })
            .to_string();
            success_result(text)
        }
        Err(e) => error_result(format!("delete failed: {}", e)),
    }
}

async fn execute_delete_many(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let filter = match args.get("filter") {
        Some(f) => match json_to_document(f) {
            Ok(d) => d,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: filter".to_string()),
    };

    match operations.delete_many(db, coll, filter).await {
        Ok(result) => {
            let text = json!({
                "deletedCount": result.deleted_count
            })
            .to_string();
            success_result(text)
        }
        Err(e) => error_result(format!("delete_many failed: {}", e)),
    }
}

async fn execute_aggregate(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let pipeline = match args.get("pipeline") {
        Some(p) => match json_to_documents(p) {
            Ok(docs) => docs,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: pipeline".to_string()),
    };

    match operations.aggregate(db, coll, pipeline).await {
        Ok(docs) => {
            let json_docs: Vec<Value> = docs
                .iter()
                .map(|d| serde_json::to_value(d).unwrap_or(Value::Null))
                .collect();
            let text = serde_json::to_string_pretty(&json_docs)
                .unwrap_or_else(|_| "[]".to_string());
            success_result(text)
        }
        Err(e) => error_result(format!("aggregate failed: {}", e)),
    }
}

async fn execute_create_collection(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    match operations.create_collection(db, coll).await {
        Ok(()) => success_result(json!({ "ok": 1, "collection": coll }).to_string()),
        Err(e) => error_result(format!("create_collection failed: {}", e)),
    }
}

async fn execute_create_index(operations: &Operations, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };
    let coll = match get_str(args, "collection") {
        Ok(v) => v,
        Err(e) => return e,
    };

    let keys = match args.get("keys") {
        Some(k) => match json_to_document(k) {
            Ok(d) => d,
            Err(e) => return e,
        },
        None => return error_result("Missing required field: keys".to_string()),
    };

    let unique = args.get("unique").and_then(|v| v.as_bool());
    let options = if unique.is_some() {
        Some(IndexOptions {
            name: None,
            unique,
            sparse: None,
        })
    } else {
        None
    };

    match operations.create_index(db, coll, keys, options).await {
        Ok(index_name) => {
            success_result(json!({ "ok": 1, "indexName": index_name }).to_string())
        }
        Err(e) => error_result(format!("create_index failed: {}", e)),
    }
}

async fn execute_list_databases(pool: &ConnectionPool) -> McpToolCallResult {
    match pool.client().list_database_names().await {
        Ok(names) => success_result(
            serde_json::to_string_pretty(&json!({ "databases": names }))
                .unwrap_or_else(|_| "[]".to_string()),
        ),
        Err(e) => error_result(format!("list_databases failed: {}", e)),
    }
}

async fn execute_list_collections(pool: &ConnectionPool, args: &Value) -> McpToolCallResult {
    let db = match get_str(args, "database") {
        Ok(v) => v,
        Err(e) => return e,
    };

    match pool.database(db).list_collection_names().await {
        Ok(names) => success_result(
            serde_json::to_string_pretty(&json!({ "collections": names }))
                .unwrap_or_else(|_| "[]".to_string()),
        ),
        Err(e) => error_result(format!("list_collections failed: {}", e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definitions_count() {
        let tools = tool_definitions();
        assert_eq!(tools.len(), 13);
    }

    #[test]
    fn test_tool_definitions_have_required_fields() {
        let tools = tool_definitions();
        for tool in &tools {
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            assert_eq!(tool.input_schema["type"], "object");
            assert!(tool.input_schema.get("properties").is_some());
            assert!(tool.input_schema.get("required").is_some());
        }
    }

    #[test]
    fn test_tool_definitions_names() {
        let tools = tool_definitions();
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"find"));
        assert!(names.contains(&"find_one"));
        assert!(names.contains(&"insert"));
        assert!(names.contains(&"insert_many"));
        assert!(names.contains(&"update"));
        assert!(names.contains(&"update_many"));
        assert!(names.contains(&"delete"));
        assert!(names.contains(&"delete_many"));
        assert!(names.contains(&"aggregate"));
        assert!(names.contains(&"create_collection"));
        assert!(names.contains(&"create_index"));
        assert!(names.contains(&"list_databases"));
        assert!(names.contains(&"list_collections"));
    }

    #[test]
    fn test_json_to_document_valid() {
        let val = json!({"name": "test", "age": 30});
        let doc = json_to_document(&val).unwrap();
        assert_eq!(doc.get_str("name").unwrap(), "test");
        assert_eq!(doc.get_i64("age").unwrap(), 30);
    }

    #[test]
    fn test_json_to_document_invalid() {
        let val = json!("not an object");
        let result = json_to_document(&val);
        assert!(result.is_err());
    }

    #[test]
    fn test_json_to_documents_valid() {
        let val = json!([{"a": 1}, {"b": 2}]);
        let docs = json_to_documents(&val).unwrap();
        assert_eq!(docs.len(), 2);
    }

    #[test]
    fn test_json_to_documents_invalid() {
        let val = json!("not an array");
        let result = json_to_documents(&val);
        assert!(result.is_err());
    }

    #[test]
    fn test_error_result() {
        let result = error_result("something went wrong".to_string());
        assert!(result.is_error);
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].text, "something went wrong");
    }

    #[test]
    fn test_success_result() {
        let result = success_result("ok".to_string());
        assert!(!result.is_error);
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].text, "ok");
    }
}
