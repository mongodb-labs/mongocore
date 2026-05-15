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
        "watch_directory" => generate_watch_directory(language, params),
        _ => Err(format!("Unknown ingestion operation: {}", tool_name)),
    }
}

fn generate_ingest(language: Language, params: &Value) -> Result<String, String> {
    let db = params.get("database").and_then(|v| v.as_str()).unwrap_or("mydb");
    let coll = params.get("collection").and_then(|v| v.as_str()).unwrap_or("docs");
    let source = params.get("source").and_then(|v| v.as_str()).unwrap_or("data.csv");
    let format = params.get("format").and_then(|v| v.as_str());
    let dedup_key = params.get("dedup_key").and_then(|v| v.as_str());
    let conflict_strategy = params.get("conflict_strategy").and_then(|v| v.as_str());
    let transforms = params.get("transforms");

    match language {
        Language::Python => {
            let mut code = format!(
                r#"async def ingest_{coll}(client, db_name: str = "{db}", collection: str = "{coll}", source: str = "{source}"):
    """Ingest data into {coll} from {source}."""
    result = await client.ingest(
        database=db_name,
        collection=collection,
        source=source,
"#
            );
            if let Some(fmt) = format {
                code.push_str(&format!("        format=\"{}\",\n", fmt));
            }
            if let Some(key) = dedup_key {
                code.push_str(&format!("        dedup_key=\"{}\",\n", key));
            }
            if let Some(strategy) = conflict_strategy {
                code.push_str(&format!("        conflict_strategy=\"{}\",\n", strategy));
            }
            if let Some(t) = transforms {
                let t_str = serde_json::to_string(t).unwrap_or_else(|_| "[]".to_string());
                code.push_str(&format!("        transforms={},\n", t_str));
            }
            code.push_str("    )\n    return result\n");
            Ok(code)
        }
        Language::TypeScript => {
            let mut code = format!(
                r#"async function ingest{coll_cap}(client: MongoCore, dbName = "{db}", collection = "{coll}", source = "{source}") {{
  // Ingest data into {coll} from {source}
  const result = await client.ingest({{
    database: dbName,
    collection,
    source,
"#,
                coll_cap = super::crud_gen::capitalize(coll)
            );
            if let Some(fmt) = format {
                code.push_str(&format!("    format: \"{}\",\n", fmt));
            }
            if let Some(key) = dedup_key {
                code.push_str(&format!("    dedupKey: \"{}\",\n", key));
            }
            if let Some(strategy) = conflict_strategy {
                code.push_str(&format!("    conflictStrategy: \"{}\",\n", strategy));
            }
            if let Some(t) = transforms {
                let t_str = serde_json::to_string_pretty(t).unwrap_or_else(|_| "[]".to_string());
                code.push_str(&format!("    transforms: {},\n", t_str));
            }
            code.push_str("  });\n  return result;\n}\n");
            Ok(code)
        }
        Language::Go => {
            let mut code = format!(
                r#"func ingest{coll_cap}(client *mongocore.Client, dbName string, collection string, source string) (*pb.IngestResponse, error) {{
	// Ingest data into {coll} from {source}
	req := &pb.IngestRequest{{
		Database:   dbName,
		Collection: collection,
		Source:     source,
"#,
                coll_cap = super::crud_gen::capitalize(coll)
            );
            if let Some(fmt) = format {
                code.push_str(&format!("\t\tFormat: \"{}\",\n", fmt));
            }
            if let Some(key) = dedup_key {
                code.push_str(&format!("\t\tDedupKey: \"{}\",\n", key));
            }
            if let Some(strategy) = conflict_strategy {
                code.push_str(&format!("\t\tConflictStrategy: \"{}\",\n", strategy));
            }
            code.push_str("\t}\n\treturn client.Ingest(context.Background(), req)\n}\n");
            Ok(code)
        }
        Language::Java => {
            let method_name = format!("ingest{}", super::crud_gen::capitalize(coll));
            let mut code = format!(
                r#"public IngestResponse {method_name}(MongoClient client, String dbName, String collection, String source) {{
    // Ingest data into {coll} from {source}
    IngestRequest.Builder request = IngestRequest.newBuilder()
        .setDatabase(dbName)
        .setCollection(collection)
        .setSource(source);
"#
            );
            if let Some(fmt) = format {
                code.push_str(&format!("    request.setFormat(\"{}\");\n", fmt));
            }
            if let Some(key) = dedup_key {
                code.push_str(&format!("    request.setDedupKey(\"{}\");\n", key));
            }
            if let Some(strategy) = conflict_strategy {
                code.push_str(&format!("    request.setConflictStrategy(\"{}\");\n", strategy));
            }
            code.push_str("    return client.ingest(request.build());\n}\n");
            Ok(code)
        }
    }
}

fn generate_watch_directory(language: Language, params: &Value) -> Result<String, String> {
    let db = params.get("database").and_then(|v| v.as_str()).unwrap_or("mydb");
    let coll = params.get("collection").and_then(|v| v.as_str()).unwrap_or("docs");
    let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("./data");

    match language {
        Language::Python => Ok(format!(
            r#"async def watch_directory_{coll}(client, db_name: str = "{db}", collection: str = "{coll}", path: str = "{path}"):
    """Watch directory and ingest new files into {coll}."""
    result = await client.watch_directory(
        database=db_name,
        collection=collection,
        path=path,
    )
    return result
"#
        )),
        Language::TypeScript => Ok(format!(
            r#"async function watchDirectory{coll_cap}(client: MongoCore, dbName = "{db}", collection = "{coll}", path = "{path}") {{
  // Watch directory and ingest new files into {coll}
  const result = await client.watchDirectory({{
    database: dbName,
    collection,
    path,
  }});
  return result;
}}
"#,
            coll_cap = super::crud_gen::capitalize(coll)
        )),
        Language::Go => Ok(format!(
            r#"func watchDirectory{coll_cap}(client *mongocore.Client, dbName string, collection string, path string) (*pb.WatchDirectoryResponse, error) {{
	// Watch directory and ingest new files into {coll}
	req := &pb.WatchDirectoryRequest{{
		Database:   dbName,
		Collection: collection,
		Path:       path,
	}}
	return client.WatchDirectory(context.Background(), req)
}}
"#,
            coll_cap = super::crud_gen::capitalize(coll)
        )),
        Language::Java => Ok(format!(
            r#"public WatchDirectoryResponse watchDirectory{coll_cap}(MongoClient client, String dbName, String collection, String path) {{
    // Watch directory and ingest new files into {coll}
    WatchDirectoryRequest request = WatchDirectoryRequest.newBuilder()
        .setDatabase(dbName)
        .setCollection(collection)
        .setPath(path)
        .build();
    return client.watchDirectory(request);
}}
"#,
            coll_cap = super::crud_gen::capitalize(coll)
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_generate_ingest_python_with_options() {
        let params = json!({
            "database": "warehouse",
            "collection": "sales",
            "source": "/data/sales.parquet",
            "format": "parquet",
            "dedup_key": "order_id",
            "conflict_strategy": "upsert"
        });
        let code = generate_ingest_code(Language::Python, "ingest", &params).unwrap();
        assert!(code.contains("async def ingest_sales"));
        assert!(code.contains("parquet"));
        assert!(code.contains("dedup_key=\"order_id\""));
        assert!(code.contains("conflict_strategy=\"upsert\""));
    }

    #[test]
    fn test_generate_ingest_typescript_minimal() {
        let params = json!({
            "database": "app",
            "collection": "logs",
            "source": "logs.csv"
        });
        let code = generate_ingest_code(Language::TypeScript, "ingest", &params).unwrap();
        assert!(code.contains("ingestLogs"));
        assert!(code.contains("logs.csv"));
        assert!(!code.contains("dedupKey"));
    }

    #[test]
    fn test_generate_watch_directory_go() {
        let params = json!({
            "database": "media",
            "collection": "images",
            "path": "/uploads/images"
        });
        let code = generate_ingest_code(Language::Go, "watch_directory", &params).unwrap();
        assert!(code.contains("watchDirectoryImages"));
        assert!(code.contains("WatchDirectory"));
    }

    #[test]
    fn test_unknown_ingest_operation() {
        let params = json!({});
        let result = generate_ingest_code(Language::Python, "unknown", &params);
        assert!(result.is_err());
    }
}
