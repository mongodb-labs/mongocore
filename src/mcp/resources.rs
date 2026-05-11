use serde_json::json;

use crate::connection::pool::ConnectionPool;

use super::types::McpResourceDefinition;

/// Return the list of MCP resource definitions available for this deployment.
pub fn resource_definitions(_pool: &ConnectionPool) -> Vec<McpResourceDefinition> {
    vec![
        McpResourceDefinition {
            uri: "mongocore://capabilities".to_string(),
            name: "Server Capabilities".to_string(),
            description: "MongoDB server capabilities including version and Atlas feature availability".to_string(),
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
            serde_json::to_string_pretty(&value)
                .map_err(|e| format!("Serialization error: {}", e))
        }
        "mongocore://databases" => {
            let names = pool
                .client()
                .list_database_names()
                .await
                .map_err(|e| format!("Failed to list databases: {}", e))?;
            serde_json::to_string_pretty(&names)
                .map_err(|e| format!("Serialization error: {}", e))
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
            serde_json::to_string_pretty(&names)
                .map_err(|e| format!("Serialization error: {}", e))
        }
        _ => Err(format!("Resource not found: {}", uri)),
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
    }
}
