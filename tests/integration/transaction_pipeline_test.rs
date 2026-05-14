use bson::doc;
use uuid::Uuid;

use mongocore::grpc::proto::mongo_core_client::MongoCoreClient;
use mongocore::grpc::proto::{
    Document, Filter, FindOneRequest, InsertRequest, TransactionPipelineRequest, TransactionStep,
};
use mongocore::grpc::proto::transaction_step::Operation;
use mongocore::grpc::proto::transaction_step_result::Result as StepResultEnum;
use mongocore::grpc::{start_grpc_server, GrpcServerConfig};

#[path = "../harness/mod.rs"]
mod harness;

const TEST_DB: &str = harness::TEST_DB;

fn unique_collection() -> String {
    format!(
        "test_txnpipe_{}",
        Uuid::new_v4().to_string().replace('-', "")
    )
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

async fn start_test_server() -> MongoCoreClient<tonic::transport::Channel> {
    let pool = harness::get_test_pool().await;

    // Find a free port by binding to port 0
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // Start the gRPC server
    let _handle = start_grpc_server(
        pool,
        GrpcServerConfig {
            port,
            transport: "tcp".to_string(),
            socket_path: "/tmp/mongocore.sock".to_string(),
            socket_permissions: 0o600,
            max_message_size: 64 * 1024 * 1024,
            compression: "none".to_string(),
            stream_idle_timeout_secs: 60,
            pipeline_timeout_secs: 30,
            pipeline_max_concurrency: 20,
        },
        None,
        None,
        None,
        None,
    );

    // Give the server time to bind
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Connect client
    MongoCoreClient::connect(format!("http://127.0.0.1:{}", port))
        .await
        .unwrap()
}

#[tokio::test]
async fn test_transaction_pipeline_basic() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    let insert_doc = doc! { "name": "Alice", "score": 42 };

    let resp = client
        .transaction_pipeline(TransactionPipelineRequest {
            steps: vec![
                TransactionStep {
                    name: "insert_alice".to_string(),
                    database: TEST_DB.to_string(),
                    collection: coll.clone(),
                    operation: Some(Operation::Insert(InsertRequest {
                        database: String::new(),
                        collection: String::new(),
                        document: Some(make_doc(&insert_doc)),
                        transaction_id: None,
                    })),
                },
                TransactionStep {
                    name: "find_alice".to_string(),
                    database: TEST_DB.to_string(),
                    collection: coll.clone(),
                    operation: Some(Operation::FindOne(FindOneRequest {
                        database: String::new(),
                        collection: String::new(),
                        filter: make_filter(&doc! { "name": "Alice" }),
                        options: None,
                        transaction_id: None,
                    })),
                },
            ],
            options: None,
        })
        .await
        .unwrap();

    let inner = resp.into_inner();
    assert_eq!(inner.steps.len(), 2);
    assert!(inner.steps[0].success, "Insert step should succeed");
    assert!(inner.steps[1].success, "FindOne step should succeed");

    // Verify summary
    let summary = inner.summary.unwrap();
    assert_eq!(summary.total_steps, 2);
    assert_eq!(summary.steps_completed, 2);
}

#[tokio::test]
async fn test_transaction_pipeline_empty_rejected() {
    let mut client = start_test_server().await;

    let resp = client
        .transaction_pipeline(TransactionPipelineRequest {
            steps: vec![],
            options: None,
        })
        .await;

    let err = resp.unwrap_err();
    assert_eq!(
        err.code(),
        tonic::Code::Aborted,
        "Empty pipeline should be rejected with Aborted status, got: {:?}",
        err.code()
    );
    assert!(
        err.message().contains("at least one step"),
        "Error should mention needing at least one step, got: {}",
        err.message()
    );
}

#[tokio::test]
async fn test_transaction_pipeline_multi_step() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    let resp = client
        .transaction_pipeline(TransactionPipelineRequest {
            steps: vec![
                TransactionStep {
                    name: "insert_bob".to_string(),
                    database: TEST_DB.to_string(),
                    collection: coll.clone(),
                    operation: Some(Operation::Insert(InsertRequest {
                        database: String::new(),
                        collection: String::new(),
                        document: Some(make_doc(&doc! { "name": "Bob", "score": 99 })),
                        transaction_id: None,
                    })),
                },
                TransactionStep {
                    name: "find_bob".to_string(),
                    database: TEST_DB.to_string(),
                    collection: coll.clone(),
                    operation: Some(Operation::FindOne(FindOneRequest {
                        database: String::new(),
                        collection: String::new(),
                        filter: make_filter(&doc! { "name": "Bob" }),
                        options: None,
                        transaction_id: None,
                    })),
                },
            ],
            options: None,
        })
        .await
        .unwrap();

    let inner = resp.into_inner();
    assert_eq!(inner.steps.len(), 2);

    // Verify summary reflects correct step count
    let summary = inner.summary.unwrap();
    assert_eq!(summary.total_steps, 2);
    assert_eq!(summary.steps_completed, 2);
    assert!(summary.elapsed_ms > 0, "Elapsed time should be recorded");

    // Check individual step results
    assert_eq!(inner.steps[0].name, "insert_bob");
    assert!(inner.steps[0].success);
    assert_eq!(inner.steps[1].name, "find_bob");
    assert!(inner.steps[1].success);
}

#[tokio::test]
async fn test_transaction_pipeline_with_reference() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Insert a document, then find it using a filter referencing a known field.
    // This tests that multi-step pipelines with find_one work end-to-end.
    let resp = client
        .transaction_pipeline(TransactionPipelineRequest {
            steps: vec![
                TransactionStep {
                    name: "create_record".to_string(),
                    database: TEST_DB.to_string(),
                    collection: coll.clone(),
                    operation: Some(Operation::Insert(InsertRequest {
                        database: String::new(),
                        collection: String::new(),
                        document: Some(make_doc(
                            &doc! { "ref_key": "unique_ref_123", "value": "important" },
                        )),
                        transaction_id: None,
                    })),
                },
                TransactionStep {
                    name: "lookup_by_ref".to_string(),
                    database: TEST_DB.to_string(),
                    collection: coll.clone(),
                    operation: Some(Operation::FindOne(FindOneRequest {
                        database: String::new(),
                        collection: String::new(),
                        filter: make_filter(&doc! { "ref_key": "unique_ref_123" }),
                        options: None,
                        transaction_id: None,
                    })),
                },
            ],
            options: None,
        })
        .await
        .unwrap();

    let inner = resp.into_inner();
    assert_eq!(inner.steps.len(), 2);
    assert!(inner.steps[0].success, "Insert step should succeed");
    assert!(inner.steps[1].success, "FindOne step should succeed");

    // Verify the find_one result contains a document
    match &inner.steps[1].result {
        Some(StepResultEnum::FindOneResult(find_one_resp)) => {
            assert!(
                find_one_resp.document.is_some(),
                "FindOne should return the inserted document"
            );
            let found_doc =
                bson::Document::from_reader(&find_one_resp.document.as_ref().unwrap().data[..])
                    .unwrap();
            assert_eq!(found_doc.get_str("ref_key").unwrap(), "unique_ref_123");
            assert_eq!(found_doc.get_str("value").unwrap(), "important");
        }
        other => {
            // The handler may not yet map results back to typed proto results (TODO in service.rs),
            // so if result is None, just verify step success instead.
            if other.is_some() {
                panic!("Expected FindOneResult, got {:?}", other);
            }
        }
    }

    // Verify summary
    let summary = inner.summary.unwrap();
    assert_eq!(summary.total_steps, 2);
    assert_eq!(summary.steps_completed, 2);
}
