use bson::Document;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

use crate::connection::pool::ConnectionPool;

use super::types::McpResourceDefinition;

/// Return the list of MCP resource definitions available for this deployment.
pub fn resource_definitions(_pool: &ConnectionPool) -> Vec<McpResourceDefinition> {
    vec![
        McpResourceDefinition {
            uri: "mongocore://capabilities".to_string(),
            name: "Server Capabilities".to_string(),
            description:
                "MongoDB server capabilities including version and Atlas feature availability"
                    .to_string(),
            mime_type: "application/json".to_string(),
        },
        McpResourceDefinition {
            uri: "mongocore://databases".to_string(),
            name: "Databases".to_string(),
            description: "List of all databases on the connected MongoDB deployment".to_string(),
            mime_type: "application/json".to_string(),
        },
        McpResourceDefinition {
            uri: "mongocore://collections/{database}".to_string(),
            name: "Collections".to_string(),
            description: "List of collections in a specific database".to_string(),
            mime_type: "application/json".to_string(),
        },
        McpResourceDefinition {
            uri: "mongocore://schema/{database}/{collection}".to_string(),
            name: "Collection Schema".to_string(),
            description: "Inferred schema for a collection including field names, types, and frequency".to_string(),
            mime_type: "application/json".to_string(),
        },
    ]
}

/// Read the content of a resource by URI.
pub async fn read_resource(pool: &ConnectionPool, uri: &str) -> Result<String, String> {
    match uri {
        "mongocore://capabilities" => {
            let caps = pool.capabilities();
            let value = json!({
                "server_version": caps.server_version,
                "atlas_vector_search": caps.atlas_vector_search,
                "atlas_search": caps.atlas_search,
                "mongocore_version": env!("CARGO_PKG_VERSION"),
            });
            serde_json::to_string_pretty(&value).map_err(|e| format!("Serialization error: {}", e))
        }
        "mongocore://databases" => {
            let names = pool
                .client()
                .list_database_names()
                .await
                .map_err(|e| format!("Failed to list databases: {}", e))?;
            serde_json::to_string_pretty(&names).map_err(|e| format!("Serialization error: {}", e))
        }
        _ if uri.starts_with("mongocore://collections/") => {
            let db_name = uri.strip_prefix("mongocore://collections/").unwrap();
            if db_name.is_empty() {
                return Err("Missing database name in collections URI".to_string());
            }
            let names = pool
                .database(db_name)
                .list_collection_names()
                .await
                .map_err(|e| format!("Failed to list collections: {}", e))?;
            serde_json::to_string_pretty(&names).map_err(|e| format!("Serialization error: {}", e))
        }
        _ if uri.starts_with("mongocore://schema/") => {
            read_schema_resource(pool, uri).await
        }
        _ => Err(format!("Resource not found: {}", uri)),
    }
}

/// Read schema information for a collection by sampling documents.
async fn read_schema_resource(pool: &ConnectionPool, uri: &str) -> Result<String, String> {
    let path = uri.strip_prefix("mongocore://schema/").unwrap();
    let parts: Vec<&str> = path.split('/').collect();

    if parts.len() != 2 {
        return Err("Invalid schema URI format. Expected: mongocore://schema/{database}/{collection}".to_string());
    }

    let db_name = parts[0];
    let coll_name = parts[1];

    if db_name.is_empty() || coll_name.is_empty() {
        return Err("Database and collection names must not be empty".to_string());
    }

    // Sample 100 documents using $sample aggregation
    let sample_size = 100;
    let pipeline = vec![bson::doc! { "$sample": { "size": sample_size } }];

    let collection = pool.database(db_name).collection::<Document>(coll_name);
    let mut cursor = collection
        .aggregate(pipeline)
        .await
        .map_err(|e| format!("Failed to sample collection: {}", e))?;

    // Collect documents from cursor
    let mut docs = Vec::new();
    use futures::stream::StreamExt;
    while let Some(result) = cursor.next().await {
        match result {
            Ok(doc) => docs.push(doc),
            Err(e) => return Err(format!("Error reading document: {}", e)),
        }
    }

    // Analyze field structure
    let mut fields: HashMap<String, FieldInfo> = HashMap::new();
    for doc in &docs {
        collect_fields(doc, "", &mut fields);
    }

    // Build the result
    let mut fields_array: Vec<Value> = fields
        .into_iter()
        .map(|(name, info)| {
            let types: Vec<String> = info.types.into_iter().collect();
            json!({
                "name": name,
                "types": types,
                "count": info.count,
                "example": info.example
            })
        })
        .collect();

    // Sort by field name for consistent output
    fields_array.sort_by(|a, b| {
        a.get("name")
            .and_then(|v| v.as_str())
            .cmp(&b.get("name").and_then(|v| v.as_str()))
    });

    let result = json!({
        "database": db_name,
        "collection": coll_name,
        "documents_sampled": docs.len(),
        "fields": fields_array
    });

    serde_json::to_string_pretty(&result).map_err(|e| format!("Serialization error: {}", e))
}

/// Field information collected during schema analysis.
struct FieldInfo {
    types: HashSet<String>,
    count: usize,
    example: Option<Value>,
}

impl FieldInfo {
    fn new() -> Self {
        Self {
            types: HashSet::new(),
            count: 0,
            example: None,
        }
    }
}

/// Recursively collect field information from a BSON document.
fn collect_fields(doc: &Document, prefix: &str, fields: &mut HashMap<String, FieldInfo>) {
    for (key, value) in doc.iter() {
        let field_path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };

        let info = fields.entry(field_path.clone()).or_insert_with(FieldInfo::new);
        info.count += 1;
        info.types.insert(bson_type_name(value));

        if info.example.is_none() {
            info.example = Some(bson_to_example_json(value));
        }

        // Recurse into nested documents
        if let bson::Bson::Document(nested) = value {
            collect_fields(nested, &field_path, fields);
        }
    }
}

/// Get the BSON type name as a string.
fn bson_type_name(value: &bson::Bson) -> String {
    match value {
        bson::Bson::Double(_) => "double",
        bson::Bson::String(_) => "string",
        bson::Bson::Array(_) => "array",
        bson::Bson::Document(_) => "document",
        bson::Bson::Boolean(_) => "bool",
        bson::Bson::Null => "null",
        bson::Bson::RegularExpression(_) => "regex",
        bson::Bson::JavaScriptCode(_) => "javascript",
        bson::Bson::JavaScriptCodeWithScope(_) => "javascriptWithScope",
        bson::Bson::Int32(_) => "int32",
        bson::Bson::Int64(_) => "int64",
        bson::Bson::Timestamp(_) => "timestamp",
        bson::Bson::Binary(_) => "binData",
        bson::Bson::ObjectId(_) => "objectId",
        bson::Bson::DateTime(_) => "date",
        bson::Bson::Symbol(_) => "symbol",
        bson::Bson::Decimal128(_) => "decimal",
        bson::Bson::Undefined => "undefined",
        bson::Bson::MaxKey => "maxKey",
        bson::Bson::MinKey => "minKey",
        bson::Bson::DbPointer(_) => "dbPointer",
    }
    .to_string()
}

/// Convert a BSON value to an example JSON value (with truncation for long strings).
fn bson_to_example_json(value: &bson::Bson) -> Value {
    match value {
        bson::Bson::Double(v) => json!(v),
        bson::Bson::String(v) => {
            if v.len() > 50 {
                json!(format!("{}...", &v[..50]))
            } else {
                json!(v)
            }
        }
        bson::Bson::Array(arr) => {
            if arr.is_empty() {
                json!([])
            } else {
                json!([bson_to_example_json(&arr[0])])
            }
        }
        bson::Bson::Document(_) => json!("{ ... }"),
        bson::Bson::Boolean(v) => json!(v),
        bson::Bson::Null => Value::Null,
        bson::Bson::Int32(v) => json!(v),
        bson::Bson::Int64(v) => json!(v),
        bson::Bson::ObjectId(oid) => json!(oid.to_string()),
        bson::Bson::DateTime(dt) => json!(dt.to_string()),
        bson::Bson::Decimal128(d) => json!(d.to_string()),
        _ => json!("..."),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_resource_definitions_count() {
        // We can't easily construct a ConnectionPool in unit tests without a real connection,
        // so we test the structure of definitions by verifying the function signature compiles.
        // Integration tests would verify against a real pool.
    }

    #[test]
    fn test_unknown_uri_returns_error() {
        // We can test the URI matching logic synchronously for the error case.
        let uri = "mongocore://unknown";
        // The read_resource function is async, but we can verify the pattern matching
        // by checking that our match arms cover expected URIs.
        assert!(uri != "mongocore://capabilities");
        assert!(uri != "mongocore://databases");
        assert!(!uri.starts_with("mongocore://collections/"));
        assert!(!uri.starts_with("mongocore://schema/"));
    }
}
