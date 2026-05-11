use reqwest::Client as HttpClient;
use serde_json::json;

use mongocore::grpc::proto::mongo_core_client::MongoCoreClient;
use mongocore::grpc::proto::ListDatabasesRequest;
use mongocore::grpc::start_grpc_server;
use mongocore::mcp::start_mcp_server;

#[path = "../harness/mod.rs"]
mod harness;

async fn find_free_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

#[tokio::test]
async fn test_both_servers_start_and_respond() {
    let pool = harness::get_test_pool().await;

    let grpc_port = find_free_port().await;
    let mcp_port = find_free_port().await;

    let grpc_handle = start_grpc_server(pool.clone(), grpc_port);
    let mcp_handle = start_mcp_server(pool.clone(), mcp_port);

    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    // Verify gRPC server responds
    let mut grpc_client =
        MongoCoreClient::connect(format!("http://127.0.0.1:{}", grpc_port))
            .await
            .expect("gRPC client should connect");

    let resp = grpc_client
        .list_databases(ListDatabasesRequest {})
        .await
        .expect("gRPC ListDatabases should succeed");
    assert!(!resp.into_inner().databases.is_empty());

    // Verify MCP server health endpoint responds
    let http = HttpClient::new();
    let health_resp = http
        .get(format!("http://127.0.0.1:{}/health", mcp_port))
        .send()
        .await
        .expect("MCP health should respond");
    assert_eq!(health_resp.status(), 200);

    // Verify MCP JSON-RPC responds
    let init_resp = http
        .post(format!("http://127.0.0.1:{}/mcp", mcp_port))
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "initialize",
            "id": 1
        }))
        .send()
        .await
        .expect("MCP initialize should respond");
    assert_eq!(init_resp.status(), 200);

    let body: serde_json::Value = init_resp.json().await.unwrap();
    assert_eq!(body["result"]["serverInfo"]["name"], "mongocore");

    // Both handles should still be running (not finished)
    assert!(!grpc_handle.is_finished());
    assert!(!mcp_handle.is_finished());
}

#[tokio::test]
async fn test_grpc_server_serves_on_configured_port() {
    let pool = harness::get_test_pool().await;
    let port = find_free_port().await;

    let _handle = start_grpc_server(pool, port);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut client = MongoCoreClient::connect(format!("http://127.0.0.1:{}", port))
        .await
        .expect("Should connect to gRPC server");

    let resp = client
        .list_databases(ListDatabasesRequest {})
        .await
        .expect("Should get response");
    assert!(!resp.into_inner().databases.is_empty());
}

#[tokio::test]
async fn test_mcp_server_serves_on_configured_port() {
    let pool = harness::get_test_pool().await;
    let port = find_free_port().await;

    let _handle = start_mcp_server(pool, port);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let http = HttpClient::new();

    // Health check
    let resp = http
        .get(format!("http://127.0.0.1:{}/health", port))
        .send()
        .await
        .expect("Should respond to health check");
    assert_eq!(resp.status(), 200);

    // Tools list
    let resp = http
        .post(format!("http://127.0.0.1:{}/mcp", port))
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "tools/list",
            "id": 1
        }))
        .send()
        .await
        .expect("Should respond to tools/list");
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    let tools = body["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 13);
}

#[tokio::test]
async fn test_shared_pool_across_servers() {
    let pool = harness::get_test_pool().await;
    let grpc_port = find_free_port().await;
    let mcp_port = find_free_port().await;

    let _grpc_handle = start_grpc_server(pool.clone(), grpc_port);
    let _mcp_handle = start_mcp_server(pool.clone(), mcp_port);
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    let coll_name = format!("startup_test_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));

    // Insert via gRPC
    let mut grpc_client =
        MongoCoreClient::connect(format!("http://127.0.0.1:{}", grpc_port))
            .await
            .unwrap();

    let doc = bson::doc! { "source": "grpc", "test": true };
    let mut buf = Vec::new();
    doc.to_writer(&mut buf).unwrap();

    grpc_client
        .insert(mongocore::grpc::proto::InsertRequest {
            database: harness::TEST_DB.to_string(),
            collection: coll_name.clone(),
            document: Some(mongocore::grpc::proto::Document { data: buf }),
            transaction_id: None,
        })
        .await
        .expect("gRPC insert should succeed");

    // Read via MCP — should see the same document
    let http = HttpClient::new();
    let resp = http
        .post(format!("http://127.0.0.1:{}/mcp", mcp_port))
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": "find",
                "arguments": {
                    "database": harness::TEST_DB,
                    "collection": coll_name,
                    "filter": { "source": "grpc" }
                }
            },
            "id": 2
        }))
        .send()
        .await
        .unwrap();

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["result"]["isError"], json!(false));

    let text = body["result"]["content"][0]["text"].as_str().unwrap();
    let docs: Vec<serde_json::Value> = serde_json::from_str(text).unwrap();
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0]["source"], "grpc");
}
