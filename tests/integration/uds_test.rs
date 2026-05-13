use std::time::Duration;

use bson::doc;
use uuid::Uuid;

use mongocore::grpc::proto::mongo_core_client::MongoCoreClient;
use mongocore::grpc::proto::{FindOneRequest, InsertRequest};
use mongocore::grpc::proto::{Document, Filter};
use mongocore::grpc::{start_grpc_server, GrpcServerConfig};

use hyper_util::rt::TokioIo;
use tonic::transport::{Channel, Endpoint, Uri};
use tower::service_fn;

#[path = "../harness/mod.rs"]
mod harness;

const TEST_DB: &str = harness::TEST_DB;

fn unique_collection() -> String {
    format!("test_uds_{}", Uuid::new_v4().to_string().replace('-', ""))
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

fn decode_doc(proto_doc: &Document) -> bson::Document {
    bson::Document::from_reader(&proto_doc.data[..]).unwrap()
}

fn test_socket_path() -> String {
    format!("/tmp/mongocore_test_{}.sock", Uuid::new_v4())
}

async fn start_uds_test_server(socket_path: &str) -> MongoCoreClient<Channel> {
    let pool = harness::get_test_pool().await;

    // Find a free port for TCP (required by server but we won't use it)
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let _handle = start_grpc_server(
        pool,
        GrpcServerConfig {
            port,
            transport: "both".to_string(),
            socket_path: socket_path.to_string(),
            socket_permissions: 0o600,
            max_message_size: 64 * 1024 * 1024,
            compression: "none".to_string(),
            stream_idle_timeout_secs: 60,
        },
        None,
        None,
        None,
        None,
    );

    // Give the server time to bind
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Connect via UDS
    let path = socket_path.to_string();
    let channel = Endpoint::try_from("http://[::]:50051")
        .unwrap()
        .connect_with_connector(service_fn(move |_: Uri| {
            let path = path.clone();
            async move {
                let stream = tokio::net::UnixStream::connect(path).await?;
                Ok::<_, std::io::Error>(TokioIo::new(stream))
            }
        }))
        .await
        .unwrap();

    MongoCoreClient::new(channel)
}

#[tokio::test]
async fn test_find_one_over_uds() {
    let socket_path = test_socket_path();
    let mut client = start_uds_test_server(&socket_path).await;
    let coll = unique_collection();

    // Insert a document
    let insert_resp = client
        .insert(InsertRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            document: Some(make_doc(&doc! { "transport": "uds", "value": 42 })),
            transaction_id: None,
        })
        .await
        .unwrap();
    assert!(!insert_resp.into_inner().inserted_id.is_empty());

    // Find it back via UDS
    let resp = client
        .find_one(FindOneRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            filter: make_filter(&doc! { "transport": "uds" }),
            options: None,
            transaction_id: None,
        })
        .await
        .unwrap();

    let found = resp.into_inner().document.unwrap();
    let doc = decode_doc(&found);
    assert_eq!(doc.get_str("transport").unwrap(), "uds");
    assert_eq!(doc.get_i32("value").unwrap(), 42);

    // Cleanup
    let _ = std::fs::remove_file(&socket_path);
}

#[tokio::test]
async fn test_socket_cleanup_on_restart() {
    let socket_path = test_socket_path();

    // Create a stale file at the socket path (simulating leftover from a crash)
    std::fs::write(&socket_path, b"stale socket data").unwrap();
    assert!(std::path::Path::new(&socket_path).exists());

    // Start the server — it should remove the stale file and bind successfully
    let mut client = start_uds_test_server(&socket_path).await;
    let coll = unique_collection();

    // Verify the server is functional over UDS
    let insert_resp = client
        .insert(InsertRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            document: Some(make_doc(&doc! { "cleanup_test": true })),
            transaction_id: None,
        })
        .await
        .unwrap();
    assert!(!insert_resp.into_inner().inserted_id.is_empty());

    let resp = client
        .find_one(FindOneRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            filter: make_filter(&doc! { "cleanup_test": true }),
            options: None,
            transaction_id: None,
        })
        .await
        .unwrap();

    assert!(resp.into_inner().document.is_some());

    // Cleanup
    let _ = std::fs::remove_file(&socket_path);
}
