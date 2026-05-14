use reqwest::Client as HttpClient;
use serde_json::{json, Value};
use uuid::Uuid;

use mongocore::mcp::start_mcp_server;

#[path = "../harness/mod.rs"]
mod harness;

const TEST_DB: &str = harness::TEST_DB;

fn unique_collection() -> String {
    format!("test_mcp_{}", Uuid::new_v4().to_string().replace('-', ""))
}

async fn start_test_mcp_server() -> (HttpClient, String) {
    let pool = harness::get_test_pool().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let _handle = start_mcp_server(pool, port, None, None, None, None, None);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let client = HttpClient::new();
    let url = format!("http://127.0.0.1:{}", port);
    (client, url)
}

async fn rpc_call(client: &HttpClient, url: &str, method: &str, params: Option<Value>) -> Value {
    let body = json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": 1
    });
    let resp = client
        .post(format!("{}/mcp", url))
        .json(&body)
        .send()
        .await
        .unwrap();
    resp.json().await.unwrap()
}

async fn tool_call(client: &HttpClient, url: &str, name: &str, arguments: Value) -> Value {
    rpc_call(
        client,
        url,
        "tools/call",
        Some(json!({ "name": name, "arguments": arguments })),
    )
    .await
}

/// Parse the text content from a successful tool call response.
fn parse_tool_result(response: &Value) -> Value {
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("Expected text content in tool result");
    serde_json::from_str(text).unwrap_or_else(|_| json!(text))
}

#[tokio::test]
async fn test_mcp_health() {
    let (client, url) = start_test_mcp_server().await;

    let resp = client.get(format!("{}/health", url)).send().await.unwrap();
    assert_eq!(resp.status(), 200);
}

#[tokio::test]
async fn test_mcp_initialize() {
    let (client, url) = start_test_mcp_server().await;

    let resp = rpc_call(&client, &url, "initialize", None).await;
    let result = &resp["result"];

    assert_eq!(result["protocolVersion"], "2024-11-05");
    assert_eq!(result["serverInfo"]["name"], "mongocore");
    assert_eq!(result["serverInfo"]["version"], "0.1.0");
    assert!(result["capabilities"]["tools"].is_object());
    assert!(result["capabilities"]["resources"].is_object());
}

#[tokio::test]
async fn test_mcp_tools_list() {
    let (client, url) = start_test_mcp_server().await;

    let resp = rpc_call(&client, &url, "tools/list", None).await;
    let tools = resp["result"]["tools"].as_array().unwrap();

    assert_eq!(tools.len(), 36);

    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    let expected = [
        "find",
        "find_one",
        "insert",
        "insert_many",
        "update",
        "update_many",
        "delete",
        "delete_many",
        "aggregate",
        "create_collection",
        "create_index",
        "list_databases",
        "list_collections",
        "run_command",
        "get_analytics",
        "ingest",
        "ingest_status",
        "list_ingest_jobs",
        "cancel_ingest",
        "watch_directory",
        "stop_watch",
        "pipeline",
        "collection_schema",
        "ask",
        "explain_query",
        "generate_code",
        "generate_model",
        "generate_index",
        "embed_and_store",
        "semantic_search",
        "ingest_and_embed",
        "list_skills",
        "get_skill",
        "suggest_indexes",
        "slow_queries",
    ];
    for name in &expected {
        assert!(names.contains(name), "Missing tool: {}", name);
    }
}

#[tokio::test]
async fn test_mcp_insert_and_find() {
    let (client, url) = start_test_mcp_server().await;
    let coll = unique_collection();

    // Insert a document
    let insert_resp = tool_call(
        &client,
        &url,
        "insert",
        json!({
            "database": TEST_DB,
            "collection": coll,
            "document": { "name": "Alice", "age": 30 }
        }),
    )
    .await;
    assert_eq!(insert_resp["result"]["isError"], json!(false));

    // Find it back
    let find_resp = tool_call(
        &client,
        &url,
        "find",
        json!({
            "database": TEST_DB,
            "collection": coll,
            "filter": { "name": "Alice" }
        }),
    )
    .await;
    assert_eq!(find_resp["result"]["isError"], json!(false));

    let results = parse_tool_result(&find_resp);
    let docs = results.as_array().expect("Expected array of documents");
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0]["name"], "Alice");
    assert_eq!(docs[0]["age"], 30);
}

#[tokio::test]
async fn test_mcp_insert_many_and_find() {
    let (client, url) = start_test_mcp_server().await;
    let coll = unique_collection();

    // Insert multiple documents
    let insert_resp = tool_call(
        &client,
        &url,
        "insert_many",
        json!({
            "database": TEST_DB,
            "collection": coll,
            "documents": [
                { "name": "Bob", "score": 85 },
                { "name": "Carol", "score": 92 },
                { "name": "Dave", "score": 78 }
            ]
        }),
    )
    .await;
    assert_eq!(insert_resp["result"]["isError"], json!(false));

    // Find all
    let find_resp = tool_call(
        &client,
        &url,
        "find",
        json!({
            "database": TEST_DB,
            "collection": coll,
            "filter": {}
        }),
    )
    .await;

    let results = parse_tool_result(&find_resp);
    let docs = results.as_array().expect("Expected array of documents");
    assert_eq!(docs.len(), 3);
}

#[tokio::test]
async fn test_mcp_update() {
    let (client, url) = start_test_mcp_server().await;
    let coll = unique_collection();

    // Insert
    tool_call(
        &client,
        &url,
        "insert",
        json!({
            "database": TEST_DB,
            "collection": coll,
            "document": { "name": "Eve", "status": "active" }
        }),
    )
    .await;

    // Update
    let update_resp = tool_call(
        &client,
        &url,
        "update",
        json!({
            "database": TEST_DB,
            "collection": coll,
            "filter": { "name": "Eve" },
            "update": { "$set": { "status": "inactive" } }
        }),
    )
    .await;
    assert_eq!(update_resp["result"]["isError"], json!(false));

    // Verify
    let find_resp = tool_call(
        &client,
        &url,
        "find",
        json!({
            "database": TEST_DB,
            "collection": coll,
            "filter": { "name": "Eve" }
        }),
    )
    .await;

    let results = parse_tool_result(&find_resp);
    let docs = results.as_array().unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0]["status"], "inactive");
}

#[tokio::test]
async fn test_mcp_delete() {
    let (client, url) = start_test_mcp_server().await;
    let coll = unique_collection();

    // Insert two docs
    tool_call(
        &client,
        &url,
        "insert",
        json!({
            "database": TEST_DB,
            "collection": coll,
            "document": { "name": "Frank" }
        }),
    )
    .await;
    tool_call(
        &client,
        &url,
        "insert",
        json!({
            "database": TEST_DB,
            "collection": coll,
            "document": { "name": "Grace" }
        }),
    )
    .await;

    // Delete one
    let delete_resp = tool_call(
        &client,
        &url,
        "delete",
        json!({
            "database": TEST_DB,
            "collection": coll,
            "filter": { "name": "Frank" }
        }),
    )
    .await;
    assert_eq!(delete_resp["result"]["isError"], json!(false));

    // Verify only Grace remains
    let find_resp = tool_call(
        &client,
        &url,
        "find",
        json!({
            "database": TEST_DB,
            "collection": coll,
            "filter": {}
        }),
    )
    .await;

    let results = parse_tool_result(&find_resp);
    let docs = results.as_array().unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0]["name"], "Grace");
}

#[tokio::test]
async fn test_mcp_aggregate() {
    let (client, url) = start_test_mcp_server().await;
    let coll = unique_collection();

    // Insert docs with categories
    tool_call(
        &client,
        &url,
        "insert_many",
        json!({
            "database": TEST_DB,
            "collection": coll,
            "documents": [
                { "category": "A", "value": 10 },
                { "category": "A", "value": 20 },
                { "category": "B", "value": 30 }
            ]
        }),
    )
    .await;

    // Run $group aggregation
    let agg_resp = tool_call(
        &client,
        &url,
        "aggregate",
        json!({
            "database": TEST_DB,
            "collection": coll,
            "pipeline": [
                { "$group": { "_id": "$category", "total": { "$sum": "$value" } } }
            ]
        }),
    )
    .await;
    assert_eq!(agg_resp["result"]["isError"], json!(false));

    let results = parse_tool_result(&agg_resp);
    let docs = results.as_array().unwrap();
    assert_eq!(docs.len(), 2);

    // Verify totals (order may vary)
    let mut totals: Vec<(&str, i64)> = docs
        .iter()
        .map(|d| (d["_id"].as_str().unwrap(), d["total"].as_i64().unwrap()))
        .collect();
    totals.sort_by(|a, b| a.0.cmp(b.0));
    assert_eq!(totals[0], ("A", 30));
    assert_eq!(totals[1], ("B", 30));
}

#[tokio::test]
async fn test_mcp_list_databases() {
    let (client, url) = start_test_mcp_server().await;

    let resp = tool_call(&client, &url, "list_databases", json!({})).await;
    assert_eq!(resp["result"]["isError"], json!(false));

    let results = parse_tool_result(&resp);
    let dbs = results["databases"]
        .as_array()
        .expect("Expected databases array in result");
    assert!(!dbs.is_empty());
}

#[tokio::test]
async fn test_mcp_list_collections() {
    let (client, url) = start_test_mcp_server().await;
    let coll = unique_collection();

    // Create a collection by inserting a document
    tool_call(
        &client,
        &url,
        "insert",
        json!({
            "database": TEST_DB,
            "collection": coll,
            "document": { "init": true }
        }),
    )
    .await;

    // List collections
    let resp = tool_call(
        &client,
        &url,
        "list_collections",
        json!({ "database": TEST_DB }),
    )
    .await;
    assert_eq!(resp["result"]["isError"], json!(false));

    let results = parse_tool_result(&resp);
    let collections = results["collections"]
        .as_array()
        .expect("Expected collections array in result");
    let names: Vec<&str> = collections.iter().filter_map(|c| c.as_str()).collect();
    assert!(
        names.iter().any(|n| *n == coll),
        "Expected collection '{}' in list: {:?}",
        coll,
        names
    );
}

#[tokio::test]
async fn test_mcp_resources_list() {
    let (client, url) = start_test_mcp_server().await;

    let resp = rpc_call(&client, &url, "resources/list", None).await;
    let resources = resp["result"]["resources"].as_array().unwrap();

    assert!(!resources.is_empty());
    // Each resource should have uri, name, description
    for resource in resources {
        assert!(resource["uri"].is_string());
        assert!(resource["name"].is_string());
        assert!(resource["description"].is_string());
    }
}

#[tokio::test]
async fn test_mcp_resources_read_capabilities() {
    let (client, url) = start_test_mcp_server().await;

    let resp = rpc_call(
        &client,
        &url,
        "resources/read",
        Some(json!({ "uri": "mongocore://capabilities" })),
    )
    .await;

    // Should return contents array
    let contents = resp["result"]["contents"].as_array().unwrap();
    assert!(!contents.is_empty());
    assert_eq!(contents[0]["uri"], "mongocore://capabilities");
    assert!(contents[0]["text"].is_string());
}
