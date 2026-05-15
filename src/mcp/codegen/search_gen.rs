use serde_json::Value;

use super::Language;

/// Generate MongoCore client code for search/embedding operations.
pub fn generate_search_code(
    language: Language,
    tool_name: &str,
    params: &Value,
) -> Result<String, String> {
    match tool_name {
        "embed_and_store" => generate_embed_and_store(language, params),
        "semantic_search" => generate_semantic_search(language, params),
        _ => Err(format!("Unknown search operation: {}", tool_name)),
    }
}

fn get_str_param<'a>(params: &'a Value, key: &str) -> &'a str {
    params.get(key).and_then(|v| v.as_str()).unwrap_or("unknown")
}

fn generate_embed_and_store(language: Language, params: &Value) -> Result<String, String> {
    let db = get_str_param(params, "database");
    let coll = get_str_param(params, "collection");
    let embed_field = get_str_param(params, "embed_field");

    match language {
        Language::Python => Ok(format!(
            "async def embed_and_store_{coll}(\n    client,\n    db_name: str = \"{db}\",\n    collection_name: str = \"{coll}\",\n    documents: list[dict] = None,\n    embed_field: str = \"{embed_field}\",\n) -> dict:\n    return await client.embed_and_store(\n        database=db_name,\n        collection=collection_name,\n        documents=documents or [],\n        embed_field=embed_field,\n    )\n",
        )),
        Language::TypeScript => Ok(format!(
            "async function embedAndStore_{coll}(\n  client: MongoCore,\n  dbName = \"{db}\",\n  collectionName = \"{coll}\",\n  documents: Record<string, unknown>[] = [],\n  embedField = \"{embed_field}\",\n) {{\n  return await client.embedAndStore({{\n    database: dbName,\n    collection: collectionName,\n    documents,\n    embedField,\n  }});\n}}\n",
        )),
        Language::Go => Ok(format!(
            "func embedAndStore_{coll}(client *mongocore.Client, dbName string, collectionName string, documents []interface{{}}, embedField string) (*EmbedResult, error) {{\n    return client.EmbedAndStore(&EmbedOptions{{\n        Database:   dbName,\n        Collection: collectionName,\n        Documents:  documents,\n        EmbedField: embedField,\n    }})\n}}\n",
        )),
        Language::Java => Ok(format!(
            "public EmbedResult embedAndStore{coll_cap}(MongoClient client, String dbName, String collectionName, List<Document> documents, String embedField) {{\n    return client.embedAndStore(dbName, collectionName, documents, embedField);\n}}\n",
            coll_cap = super::crud_gen::capitalize(coll),
        )),
    }
}

fn generate_semantic_search(language: Language, params: &Value) -> Result<String, String> {
    let db = get_str_param(params, "database");
    let coll = get_str_param(params, "collection");
    let query = get_str_param(params, "query");
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(10);

    match language {
        Language::Python => Ok(format!(
            "async def semantic_search_{coll}(\n    client,\n    db_name: str = \"{db}\",\n    collection_name: str = \"{coll}\",\n    query: str = \"{query}\",\n    limit: int = {limit},\n) -> list[dict]:\n    return await client.semantic_search(\n        database=db_name,\n        collection=collection_name,\n        query=query,\n        limit=limit,\n    )\n",
        )),
        Language::TypeScript => Ok(format!(
            "async function semanticSearch_{coll}(\n  client: MongoCore,\n  dbName = \"{db}\",\n  collectionName = \"{coll}\",\n  query = \"{query}\",\n  limit = {limit},\n) {{\n  return await client.semanticSearch({{\n    database: dbName,\n    collection: collectionName,\n    query,\n    limit,\n  }});\n}}\n",
        )),
        Language::Go => Ok(format!(
            "func semanticSearch_{coll}(client *mongocore.Client, dbName string, collectionName string, query string, limit int) ([]SearchResult, error) {{\n    return client.SemanticSearch(&SearchOptions{{\n        Database:   dbName,\n        Collection: collectionName,\n        Query:      query,\n        Limit:      limit,\n    }})\n}}\n",
        )),
        Language::Java => Ok(format!(
            "public List<SearchResult> semanticSearch{coll_cap}(MongoClient client, String dbName, String collectionName, String query, int limit) {{\n    return client.semanticSearch(dbName, collectionName, query, limit);\n}}\n",
            coll_cap = super::crud_gen::capitalize(coll),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_generate_embed_and_store_python() {
        let params = json!({
            "database": "knowledge",
            "collection": "articles",
            "embed_field": "content"
        });
        let code = generate_search_code(Language::Python, "embed_and_store", &params).unwrap();
        assert!(code.contains("async def embed_and_store_articles("));
        assert!(code.contains("client.embed_and_store("));
        assert!(code.contains("embed_field: str = \"content\""));
    }

    #[test]
    fn test_generate_semantic_search_typescript() {
        let params = json!({
            "database": "docs",
            "collection": "pages",
            "query": "how to configure auth",
            "limit": 5
        });
        let code = generate_search_code(Language::TypeScript, "semantic_search", &params).unwrap();
        assert!(code.contains("async function semanticSearch_pages("));
        assert!(code.contains("client.semanticSearch("));
        assert!(code.contains("limit = 5"));
    }

    #[test]
    fn test_generate_embed_go() {
        let params = json!({
            "database": "mydb",
            "collection": "docs",
            "embed_field": "text"
        });
        let code = generate_search_code(Language::Go, "embed_and_store", &params).unwrap();
        assert!(code.contains("func embedAndStore_docs("));
        assert!(code.contains("client.EmbedAndStore("));
    }

    #[test]
    fn test_unknown_search_operation() {
        let params = json!({});
        let result = generate_search_code(Language::Python, "unknown", &params);
        assert!(result.is_err());
    }
}
