use bson::doc;
use uuid::Uuid;

use mongocore::grpc::proto::mongo_core_client::MongoCoreClient;
use mongocore::grpc::proto::{
    Document, Filter, FindRequest, InsertManyRequest, InsertRequest, PipelineOperation,
    PipelineRequest,
};
use mongocore::grpc::proto::pipeline_operation::Operation;
use mongocore::grpc::proto::pipeline_result::Result as PipelineResultEnum;
use mongocore::grpc::{start_grpc_server, GrpcServerConfig};

#[path = "../harness/mod.rs"]
mod harness;

const TEST_DB: &str = harness::TEST_DB;

fn unique_collection() -> String {
    format!(
        "test_pipeline_{}",
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

#[allow(dead_code)]
fn decode_doc(proto_doc: &Document) -> bson::Document {
    bson::Document::from_reader(&proto_doc.data[..]).unwrap()
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
async fn test_pipeline_mixed_operations() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Seed data
    client
        .insert_many(InsertManyRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            documents: vec![
                make_doc(&doc! { "name": "Alice", "score": 10 }),
                make_doc(&doc! { "name": "Bob", "score": 20 }),
            ],
            transaction_id: None,
        })
        .await
        .unwrap();

    // Pipeline: find all + insert a new doc
    let resp = client
        .pipeline(PipelineRequest {
            operations: vec![
                PipelineOperation {
                    operation: Some(Operation::Find(FindRequest {
                        database: TEST_DB.to_string(),
                        collection: coll.clone(),
                        filter: make_filter(&doc! {}),
                        options: None,
                        transaction_id: None,
                    })),
                },
                PipelineOperation {
                    operation: Some(Operation::Insert(InsertRequest {
                        database: TEST_DB.to_string(),
                        collection: coll.clone(),
                        document: Some(make_doc(&doc! { "name": "Carol", "score": 30 })),
                        transaction_id: None,
                    })),
                },
            ],
        })
        .await
        .unwrap();

    let inner = resp.into_inner();
    assert_eq!(inner.succeeded, 2);
    assert_eq!(inner.failed, 0);
    assert_eq!(inner.results.len(), 2);

    // Verify find result
    match &inner.results[0].result {
        Some(PipelineResultEnum::Find(find_resp)) => {
            assert_eq!(find_resp.documents.len(), 2);
        }
        other => panic!("Expected Find result, got {:?}", other),
    }

    // Verify insert result
    match &inner.results[1].result {
        Some(PipelineResultEnum::Insert(insert_resp)) => {
            assert!(!insert_resp.inserted_id.is_empty());
        }
        other => panic!("Expected Insert result, got {:?}", other),
    }

    // Verify the insert actually persisted
    let find_resp = client
        .find(FindRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            filter: make_filter(&doc! {}),
            options: None,
            transaction_id: None,
        })
        .await
        .unwrap();
    assert_eq!(find_resp.into_inner().documents.len(), 3);
}

#[tokio::test]
async fn test_pipeline_empty_rejected() {
    let mut client = start_test_server().await;

    let resp = client
        .pipeline(PipelineRequest {
            operations: vec![],
        })
        .await;

    let err = resp.unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
}

#[tokio::test]
async fn test_pipeline_partial_failure() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Seed data
    client
        .insert(InsertRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            document: Some(make_doc(&doc! { "name": "Alice" })),
            transaction_id: None,
        })
        .await
        .unwrap();

    // Pipeline: valid find + find with nonexistent transaction_id
    let resp = client
        .pipeline(PipelineRequest {
            operations: vec![
                PipelineOperation {
                    operation: Some(Operation::Find(FindRequest {
                        database: TEST_DB.to_string(),
                        collection: coll.clone(),
                        filter: make_filter(&doc! {}),
                        options: None,
                        transaction_id: None,
                    })),
                },
                PipelineOperation {
                    operation: Some(Operation::Find(FindRequest {
                        database: TEST_DB.to_string(),
                        collection: coll.clone(),
                        filter: make_filter(&doc! {}),
                        options: None,
                        transaction_id: Some("nonexistent_txn_id_12345".to_string()),
                    })),
                },
            ],
        })
        .await
        .unwrap();

    let inner = resp.into_inner();
    assert_eq!(inner.succeeded, 1);
    assert_eq!(inner.failed, 1);

    // First result should be a successful find
    match &inner.results[0].result {
        Some(PipelineResultEnum::Find(find_resp)) => {
            assert_eq!(find_resp.documents.len(), 1);
        }
        other => panic!("Expected Find result, got {:?}", other),
    }

    // Second result should be an error
    match &inner.results[1].result {
        Some(PipelineResultEnum::Error(err)) => {
            assert!(!err.message.is_empty());
        }
        other => panic!("Expected Error result, got {:?}", other),
    }
}

#[tokio::test]
async fn test_pipeline_exceeds_max_ops() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Create 101 operations (exceeds default max of 100)
    let operations: Vec<PipelineOperation> = (0..101)
        .map(|_| PipelineOperation {
            operation: Some(Operation::Find(FindRequest {
                database: TEST_DB.to_string(),
                collection: coll.clone(),
                filter: make_filter(&doc! {}),
                options: None,
                transaction_id: None,
            })),
        })
        .collect();

    let resp = client
        .pipeline(PipelineRequest { operations })
        .await;

    let err = resp.unwrap_err();
    assert_eq!(err.code(), tonic::Code::InvalidArgument);
    assert!(
        err.message().contains("100"),
        "Error message should mention the limit of 100, got: {}",
        err.message()
    );
}

#[tokio::test]
async fn test_pipeline_concurrent_execution() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Insert 10 documents
    let documents: Vec<Document> = (0..10)
        .map(|i| make_doc(&doc! { "index": i, "data": "test_concurrent" }))
        .collect();

    client
        .insert_many(InsertManyRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            documents,
            transaction_id: None,
        })
        .await
        .unwrap();

    // Run 5 concurrent finds in a single pipeline
    let operations: Vec<PipelineOperation> = (0..5)
        .map(|_| PipelineOperation {
            operation: Some(Operation::Find(FindRequest {
                database: TEST_DB.to_string(),
                collection: coll.clone(),
                filter: make_filter(&doc! {}),
                options: None,
                transaction_id: None,
            })),
        })
        .collect();

    let resp = client
        .pipeline(PipelineRequest { operations })
        .await
        .unwrap();

    let inner = resp.into_inner();
    assert_eq!(inner.succeeded, 5);
    assert_eq!(inner.failed, 0);
    assert_eq!(inner.results.len(), 5);

    // Each find should return all 10 documents
    for result in &inner.results {
        match &result.result {
            Some(PipelineResultEnum::Find(find_resp)) => {
                assert_eq!(
                    find_resp.documents.len(),
                    10,
                    "Each concurrent find should return all 10 docs"
                );
            }
            other => panic!("Expected Find result, got {:?}", other),
        }
    }
}
