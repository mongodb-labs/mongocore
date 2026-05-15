use serde_json::Value;

use super::Language;

/// Capitalize the first letter of a string.
pub fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Convert a snake_case string to camelCase.
pub fn to_camel_case(s: &str) -> String {
    let parts: Vec<&str> = s.split('_').collect();
    if parts.is_empty() {
        return String::new();
    }
    let mut result = parts[0].to_lowercase();
    for part in &parts[1..] {
        result.push_str(&capitalize(part));
    }
    result
}

/// Generate MongoCore client code for CRUD operations.
pub fn generate_crud_code(
    language: Language,
    tool_name: &str,
    params: &Value,
) -> Result<String, String> {
    let db = params
        .get("database")
        .and_then(|v| v.as_str())
        .unwrap_or("mydb");
    let coll = params
        .get("collection")
        .and_then(|v| v.as_str())
        .unwrap_or("docs");

    match tool_name {
        "find" => generate_find(language, db, coll, params),
        "find_one" => generate_find_one(language, db, coll, params),
        "insert" | "insert_one" => generate_insert(language, db, coll, params),
        "insert_many" => generate_insert_many(language, db, coll, params),
        "update" | "update_one" => generate_update(language, db, coll, params),
        "update_many" => generate_update_many(language, db, coll, params),
        "delete" | "delete_one" => generate_delete(language, db, coll, params),
        "delete_many" => generate_delete_many(language, db, coll, params),
        _ => Err(format!("Unknown CRUD operation: {}", tool_name)),
    }
}

fn format_json_value(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
}

fn generate_find(language: Language, db: &str, coll: &str, params: &Value) -> Result<String, String> {
    let filter = params.get("filter").cloned().unwrap_or(Value::Object(Default::default()));
    let filter_str = format_json_value(&filter);
    let limit = params.get("limit").and_then(|v| v.as_u64());

    match language {
        Language::Python => {
            let mut code = format!(
                r#"async def find_{coll}(client, db_name: str = "{db}", collection: str = "{coll}", filter: dict | None = None):
    """Find documents in {coll}."""
    result = await client.find(
        database=db_name,
        collection=collection,
        filter=filter or {filter_str},
"#
            );
            if let Some(lim) = limit {
                code.push_str(&format!("        limit={},\n", lim));
            }
            code.push_str("    )\n    return result.documents\n");
            Ok(code)
        }
        Language::TypeScript => {
            let mut code = format!(
                r#"async function find{coll_cap}(client: MongoCore, dbName = "{db}", collection = "{coll}", filter?: object) {{
  // Find documents in {coll}
  const result = await client.find({{
    database: dbName,
    collection,
    filter: filter ?? {filter_str},
"#,
                coll_cap = capitalize(coll)
            );
            if let Some(lim) = limit {
                code.push_str(&format!("    limit: {},\n", lim));
            }
            code.push_str("  });\n  return result.documents;\n}\n");
            Ok(code)
        }
        Language::Go => {
            let mut code = format!(
                r#"func find{coll_cap}(client *mongocore.Client, dbName string, collection string, filter map[string]interface{{}}) ([]bson.M, error) {{
	// Find documents in {coll}
	if filter == nil {{
		filter = map[string]interface{{}}{{}}
	}}
	result, err := client.Find(context.Background(), &pb.FindRequest{{
		Database:   dbName,
		Collection: collection,
		Filter:     toJSON(filter),
"#,
                coll_cap = capitalize(coll)
            );
            if let Some(lim) = limit {
                code.push_str(&format!("\t\tLimit: {},\n", lim));
            }
            code.push_str("\t})\n\tif err != nil {\n\t\treturn nil, err\n\t}\n\treturn result.Documents, nil\n}\n");
            Ok(code)
        }
        Language::Java => {
            let method_name = format!("find{}", capitalize(coll));
            let mut code = format!(
                r#"public List<Document> {method_name}(MongoClient client, String dbName, String collection, Document filter) {{
    // Find documents in {coll}
    if (filter == null) {{
        filter = Document.parse("{filter_escaped}");
    }}
    FindRequest.Builder request = FindRequest.newBuilder()
        .setDatabase(dbName)
        .setCollection(collection)
        .setFilter(toJson(filter));
"#,
                filter_escaped = filter_str.replace('"', "\\\"").replace('\n', " ")
            );
            if let Some(lim) = limit {
                code.push_str(&format!("    request.setLimit({});\n", lim));
            }
            code.push_str("    FindResponse response = client.find(request.build());\n");
            code.push_str("    return response.getDocumentsList();\n}\n");
            Ok(code)
        }
    }
}

fn generate_find_one(language: Language, db: &str, coll: &str, params: &Value) -> Result<String, String> {
    let filter = params.get("filter").cloned().unwrap_or(Value::Object(Default::default()));
    let filter_str = format_json_value(&filter);

    match language {
        Language::Python => Ok(format!(
            r#"async def find_one_{coll}(client, db_name: str = "{db}", collection: str = "{coll}", filter: dict | None = None):
    """Find one document in {coll}."""
    result = await client.find_one(
        database=db_name,
        collection=collection,
        filter=filter or {filter_str},
    )
    return result.document
"#
        )),
        Language::TypeScript => Ok(format!(
            r#"async function findOne{coll_cap}(client: MongoCore, dbName = "{db}", collection = "{coll}", filter?: object) {{
  // Find one document in {coll}
  const result = await client.findOne({{
    database: dbName,
    collection,
    filter: filter ?? {filter_str},
  }});
  return result.document;
}}
"#,
            coll_cap = capitalize(coll)
        )),
        Language::Go => Ok(format!(
            r#"func findOne{coll_cap}(client *mongocore.Client, dbName string, collection string, filter map[string]interface{{}}) (bson.M, error) {{
	// Find one document in {coll}
	if filter == nil {{
		filter = map[string]interface{{}}{{}}
	}}
	result, err := client.FindOne(context.Background(), &pb.FindOneRequest{{
		Database:   dbName,
		Collection: collection,
		Filter:     toJSON(filter),
	}})
	if err != nil {{
		return nil, err
	}}
	return result.Document, nil
}}
"#,
            coll_cap = capitalize(coll)
        )),
        Language::Java => Ok(format!(
            r#"public Document findOne{coll_cap}(MongoClient client, String dbName, String collection, Document filter) {{
    // Find one document in {coll}
    if (filter == null) {{
        filter = Document.parse("{filter_escaped}");
    }}
    FindOneRequest request = FindOneRequest.newBuilder()
        .setDatabase(dbName)
        .setCollection(collection)
        .setFilter(toJson(filter))
        .build();
    return client.findOne(request).getDocument();
}}
"#,
            coll_cap = capitalize(coll),
            filter_escaped = filter_str.replace('"', "\\\"").replace('\n', " ")
        )),
    }
}

fn generate_insert(language: Language, db: &str, coll: &str, params: &Value) -> Result<String, String> {
    let doc = params.get("document").cloned().unwrap_or(Value::Object(Default::default()));
    let doc_str = format_json_value(&doc);

    match language {
        Language::Python => Ok(format!(
            r#"async def insert_{coll}(client, db_name: str = "{db}", collection: str = "{coll}", document: dict | None = None):
    """Insert a document into {coll}."""
    result = await client.insert_one(
        database=db_name,
        collection=collection,
        document=document or {doc_str},
    )
    return result.inserted_id
"#
        )),
        Language::TypeScript => Ok(format!(
            r#"async function insert{coll_cap}(client: MongoCore, dbName = "{db}", collection = "{coll}", document?: object) {{
  // Insert a document into {coll}
  const result = await client.insertOne({{
    database: dbName,
    collection,
    document: document ?? {doc_str},
  }});
  return result.insertedId;
}}
"#,
            coll_cap = capitalize(coll)
        )),
        Language::Go => Ok(format!(
            r#"func insert{coll_cap}(client *mongocore.Client, dbName string, collection string, document map[string]interface{{}}) (string, error) {{
	// Insert a document into {coll}
	result, err := client.InsertOne(context.Background(), &pb.InsertOneRequest{{
		Database:   dbName,
		Collection: collection,
		Document:   toJSON(document),
	}})
	if err != nil {{
		return "", err
	}}
	return result.InsertedId, nil
}}
"#,
            coll_cap = capitalize(coll)
        )),
        Language::Java => Ok(format!(
            r#"public String insert{coll_cap}(MongoClient client, String dbName, String collection, Document document) {{
    // Insert a document into {coll}
    InsertOneRequest request = InsertOneRequest.newBuilder()
        .setDatabase(dbName)
        .setCollection(collection)
        .setDocument(toJson(document))
        .build();
    return client.insertOne(request).getInsertedId();
}}
"#,
            coll_cap = capitalize(coll)
        )),
    }
}

fn generate_insert_many(language: Language, db: &str, coll: &str, params: &Value) -> Result<String, String> {
    let docs = params.get("documents").cloned().unwrap_or(Value::Array(vec![]));
    let docs_str = format_json_value(&docs);

    match language {
        Language::Python => Ok(format!(
            r#"async def insert_many_{coll}(client, db_name: str = "{db}", collection: str = "{coll}", documents: list | None = None):
    """Insert multiple documents into {coll}."""
    result = await client.insert_many(
        database=db_name,
        collection=collection,
        documents=documents or {docs_str},
    )
    return result.inserted_ids
"#
        )),
        Language::TypeScript => Ok(format!(
            r#"async function insertMany{coll_cap}(client: MongoCore, dbName = "{db}", collection = "{coll}", documents?: object[]) {{
  // Insert multiple documents into {coll}
  const result = await client.insertMany({{
    database: dbName,
    collection,
    documents: documents ?? {docs_str},
  }});
  return result.insertedIds;
}}
"#,
            coll_cap = capitalize(coll)
        )),
        Language::Go => Ok(format!(
            r#"func insertMany{coll_cap}(client *mongocore.Client, dbName string, collection string, documents []map[string]interface{{}}) ([]string, error) {{
	// Insert multiple documents into {coll}
	result, err := client.InsertMany(context.Background(), &pb.InsertManyRequest{{
		Database:   dbName,
		Collection: collection,
		Documents:  toJSONArray(documents),
	}})
	if err != nil {{
		return nil, err
	}}
	return result.InsertedIds, nil
}}
"#,
            coll_cap = capitalize(coll)
        )),
        Language::Java => Ok(format!(
            r#"public List<String> insertMany{coll_cap}(MongoClient client, String dbName, String collection, List<Document> documents) {{
    // Insert multiple documents into {coll}
    InsertManyRequest request = InsertManyRequest.newBuilder()
        .setDatabase(dbName)
        .setCollection(collection)
        .addAllDocuments(toJsonList(documents))
        .build();
    return client.insertMany(request).getInsertedIdsList();
}}
"#,
            coll_cap = capitalize(coll)
        )),
    }
}

fn generate_update(language: Language, db: &str, coll: &str, params: &Value) -> Result<String, String> {
    let filter = params.get("filter").cloned().unwrap_or(Value::Object(Default::default()));
    let update = params.get("update").cloned().unwrap_or(Value::Object(Default::default()));
    let filter_str = format_json_value(&filter);
    let update_str = format_json_value(&update);

    match language {
        Language::Python => Ok(format!(
            r#"async def update_{coll}(client, db_name: str = "{db}", collection: str = "{coll}", filter: dict | None = None, update: dict | None = None):
    """Update a document in {coll}."""
    result = await client.update_one(
        database=db_name,
        collection=collection,
        filter=filter or {filter_str},
        update=update or {update_str},
    )
    return result.modified_count
"#
        )),
        Language::TypeScript => Ok(format!(
            r#"async function update{coll_cap}(client: MongoCore, dbName = "{db}", collection = "{coll}", filter?: object, update?: object) {{
  // Update a document in {coll}
  const result = await client.updateOne({{
    database: dbName,
    collection,
    filter: filter ?? {filter_str},
    update: update ?? {update_str},
  }});
  return result.modifiedCount;
}}
"#,
            coll_cap = capitalize(coll)
        )),
        Language::Go => Ok(format!(
            r#"func update{coll_cap}(client *mongocore.Client, dbName string, collection string, filter, update map[string]interface{{}}) (int64, error) {{
	// Update a document in {coll}
	result, err := client.UpdateOne(context.Background(), &pb.UpdateOneRequest{{
		Database:   dbName,
		Collection: collection,
		Filter:     toJSON(filter),
		Update:     toJSON(update),
	}})
	if err != nil {{
		return 0, err
	}}
	return result.ModifiedCount, nil
}}
"#,
            coll_cap = capitalize(coll)
        )),
        Language::Java => Ok(format!(
            r#"public long update{coll_cap}(MongoClient client, String dbName, String collection, Document filter, Document update) {{
    // Update a document in {coll}
    UpdateOneRequest request = UpdateOneRequest.newBuilder()
        .setDatabase(dbName)
        .setCollection(collection)
        .setFilter(toJson(filter))
        .setUpdate(toJson(update))
        .build();
    return client.updateOne(request).getModifiedCount();
}}
"#,
            coll_cap = capitalize(coll)
        )),
    }
}

fn generate_update_many(language: Language, db: &str, coll: &str, params: &Value) -> Result<String, String> {
    let filter = params.get("filter").cloned().unwrap_or(Value::Object(Default::default()));
    let update = params.get("update").cloned().unwrap_or(Value::Object(Default::default()));
    let filter_str = format_json_value(&filter);
    let update_str = format_json_value(&update);

    match language {
        Language::Python => Ok(format!(
            r#"async def update_many_{coll}(client, db_name: str = "{db}", collection: str = "{coll}", filter: dict | None = None, update: dict | None = None):
    """Update multiple documents in {coll}."""
    result = await client.update_many(
        database=db_name,
        collection=collection,
        filter=filter or {filter_str},
        update=update or {update_str},
    )
    return result.modified_count
"#
        )),
        Language::TypeScript => Ok(format!(
            r#"async function updateMany{coll_cap}(client: MongoCore, dbName = "{db}", collection = "{coll}", filter?: object, update?: object) {{
  // Update multiple documents in {coll}
  const result = await client.updateMany({{
    database: dbName,
    collection,
    filter: filter ?? {filter_str},
    update: update ?? {update_str},
  }});
  return result.modifiedCount;
}}
"#,
            coll_cap = capitalize(coll)
        )),
        Language::Go => Ok(format!(
            r#"func updateMany{coll_cap}(client *mongocore.Client, dbName string, collection string, filter, update map[string]interface{{}}) (int64, error) {{
	// Update multiple documents in {coll}
	result, err := client.UpdateMany(context.Background(), &pb.UpdateManyRequest{{
		Database:   dbName,
		Collection: collection,
		Filter:     toJSON(filter),
		Update:     toJSON(update),
	}})
	if err != nil {{
		return 0, err
	}}
	return result.ModifiedCount, nil
}}
"#,
            coll_cap = capitalize(coll)
        )),
        Language::Java => Ok(format!(
            r#"public long updateMany{coll_cap}(MongoClient client, String dbName, String collection, Document filter, Document update) {{
    // Update multiple documents in {coll}
    UpdateManyRequest request = UpdateManyRequest.newBuilder()
        .setDatabase(dbName)
        .setCollection(collection)
        .setFilter(toJson(filter))
        .setUpdate(toJson(update))
        .build();
    return client.updateMany(request).getModifiedCount();
}}
"#,
            coll_cap = capitalize(coll)
        )),
    }
}

fn generate_delete(language: Language, db: &str, coll: &str, params: &Value) -> Result<String, String> {
    let filter = params.get("filter").cloned().unwrap_or(Value::Object(Default::default()));
    let filter_str = format_json_value(&filter);

    match language {
        Language::Python => Ok(format!(
            r#"async def delete_{coll}(client, db_name: str = "{db}", collection: str = "{coll}", filter: dict | None = None):
    """Delete a document from {coll}."""
    result = await client.delete_one(
        database=db_name,
        collection=collection,
        filter=filter or {filter_str},
    )
    return result.deleted_count
"#
        )),
        Language::TypeScript => Ok(format!(
            r#"async function delete{coll_cap}(client: MongoCore, dbName = "{db}", collection = "{coll}", filter?: object) {{
  // Delete a document from {coll}
  const result = await client.deleteOne({{
    database: dbName,
    collection,
    filter: filter ?? {filter_str},
  }});
  return result.deletedCount;
}}
"#,
            coll_cap = capitalize(coll)
        )),
        Language::Go => Ok(format!(
            r#"func delete{coll_cap}(client *mongocore.Client, dbName string, collection string, filter map[string]interface{{}}) (int64, error) {{
	// Delete a document from {coll}
	result, err := client.DeleteOne(context.Background(), &pb.DeleteOneRequest{{
		Database:   dbName,
		Collection: collection,
		Filter:     toJSON(filter),
	}})
	if err != nil {{
		return 0, err
	}}
	return result.DeletedCount, nil
}}
"#,
            coll_cap = capitalize(coll)
        )),
        Language::Java => Ok(format!(
            r#"public long delete{coll_cap}(MongoClient client, String dbName, String collection, Document filter) {{
    // Delete a document from {coll}
    DeleteOneRequest request = DeleteOneRequest.newBuilder()
        .setDatabase(dbName)
        .setCollection(collection)
        .setFilter(toJson(filter))
        .build();
    return client.deleteOne(request).getDeletedCount();
}}
"#,
            coll_cap = capitalize(coll)
        )),
    }
}

fn generate_delete_many(language: Language, db: &str, coll: &str, params: &Value) -> Result<String, String> {
    let filter = params.get("filter").cloned().unwrap_or(Value::Object(Default::default()));
    let filter_str = format_json_value(&filter);

    match language {
        Language::Python => Ok(format!(
            r#"async def delete_many_{coll}(client, db_name: str = "{db}", collection: str = "{coll}", filter: dict | None = None):
    """Delete multiple documents from {coll}."""
    result = await client.delete_many(
        database=db_name,
        collection=collection,
        filter=filter or {filter_str},
    )
    return result.deleted_count
"#
        )),
        Language::TypeScript => Ok(format!(
            r#"async function deleteMany{coll_cap}(client: MongoCore, dbName = "{db}", collection = "{coll}", filter?: object) {{
  // Delete multiple documents from {coll}
  const result = await client.deleteMany({{
    database: dbName,
    collection,
    filter: filter ?? {filter_str},
  }});
  return result.deletedCount;
}}
"#,
            coll_cap = capitalize(coll)
        )),
        Language::Go => Ok(format!(
            r#"func deleteMany{coll_cap}(client *mongocore.Client, dbName string, collection string, filter map[string]interface{{}}) (int64, error) {{
	// Delete multiple documents from {coll}
	result, err := client.DeleteMany(context.Background(), &pb.DeleteManyRequest{{
		Database:   dbName,
		Collection: collection,
		Filter:     toJSON(filter),
	}})
	if err != nil {{
		return 0, err
	}}
	return result.DeletedCount, nil
}}
"#,
            coll_cap = capitalize(coll)
        )),
        Language::Java => Ok(format!(
            r#"public long deleteMany{coll_cap}(MongoClient client, String dbName, String collection, Document filter) {{
    // Delete multiple documents from {coll}
    DeleteManyRequest request = DeleteManyRequest.newBuilder()
        .setDatabase(dbName)
        .setCollection(collection)
        .setFilter(toJson(filter))
        .build();
    return client.deleteMany(request).getDeletedCount();
}}
"#,
            coll_cap = capitalize(coll)
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_capitalize() {
        assert_eq!(capitalize("hello"), "Hello");
        assert_eq!(capitalize(""), "");
        assert_eq!(capitalize("A"), "A");
    }

    #[test]
    fn test_to_camel_case() {
        assert_eq!(to_camel_case("find_one"), "findOne");
        assert_eq!(to_camel_case("insert_many"), "insertMany");
        assert_eq!(to_camel_case("hello"), "hello");
    }

    #[test]
    fn test_generate_find_python() {
        let params = json!({
            "database": "testdb",
            "collection": "users",
            "filter": {"status": "active"},
            "limit": 10
        });
        let code = generate_crud_code(Language::Python, "find", &params).unwrap();
        assert!(code.contains("async def find_users"));
        assert!(code.contains("testdb"));
        assert!(code.contains("limit=10"));
    }

    #[test]
    fn test_generate_insert_many_typescript() {
        let params = json!({
            "database": "shop",
            "collection": "products",
            "documents": [{"name": "Widget", "price": 9.99}]
        });
        let code = generate_crud_code(Language::TypeScript, "insert_many", &params).unwrap();
        assert!(code.contains("insertManyProducts"));
        assert!(code.contains("shop"));
        assert!(code.contains("Widget"));
    }

    #[test]
    fn test_generate_update_go() {
        let params = json!({
            "database": "mydb",
            "collection": "items",
            "filter": {"_id": "123"},
            "update": {"$set": {"status": "done"}}
        });
        let code = generate_crud_code(Language::Go, "update", &params).unwrap();
        assert!(code.contains("updateItems"));
        assert!(code.contains("UpdateOne"));
    }

    #[test]
    fn test_generate_delete_java() {
        let params = json!({
            "database": "logs",
            "collection": "entries",
            "filter": {"level": "debug"}
        });
        let code = generate_crud_code(Language::Java, "delete", &params).unwrap();
        assert!(code.contains("deleteEntries"));
        assert!(code.contains("DeleteOneRequest"));
    }

    #[test]
    fn test_unknown_operation() {
        let params = json!({"database": "db", "collection": "c"});
        let result = generate_crud_code(Language::Python, "unknown_op", &params);
        assert!(result.is_err());
    }
}
