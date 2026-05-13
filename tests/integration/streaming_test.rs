use bson::doc;
use futures::StreamExt;
use uuid::Uuid;

use mongocore::grpc::proto::mongo_core_client::MongoCoreClient;
use mongocore::grpc::proto::{
    AggregateStreamRequest, Document, Filter, FindStreamRequest, InsertBatch, InsertManyRequest,
    Pipeline,
};
use mongocore::grpc::{start_grpc_server, GrpcServerConfig};

#[path = "../harness/mod.rs"]
mod harness;

const TEST_DB: &str = harness::TEST_DB;

fn unique_collection() -> String {
    format!(
        "test_stream_{}",
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

fn decode_doc(proto_doc: &Document) -> bson::Document {
    bson::Document::from_reader(&proto_doc.data[..]).unwrap()
}

async fn start_test_server() -> MongoCoreClient<tonic::transport::Channel> {
    let pool = harness::get_test_pool().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

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

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    MongoCoreClient::connect(format!("http://127.0.0.1:{}", port))
        .await
        .expect("Failed to connect")
}

#[tokio::test]
async fn test_find_stream_basic() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Insert 250 documents
    let documents: Vec<Document> = (0..250)
        .map(|i| make_doc(&doc! { "index": i, "data": format!("item_{}", i) }))
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

    // Stream with batch_size=100
    let response = client
        .find_stream(FindStreamRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            filter: None,
            options: None,
            transaction_id: None,
            batch_size: 100,
        })
        .await
        .unwrap();

    let mut stream = response.into_inner();
    let mut total_docs = 0u32;
    let mut batch_count = 0u32;

    while let Some(batch) = stream.next().await {
        let batch = batch.unwrap();
        total_docs += batch.documents.len() as u32;
        batch_count += 1;

        // Verify batch_index is sequential (0-based)
        assert_eq!(batch.batch_index, batch_count - 1);
    }

    assert_eq!(total_docs, 250);
    // With batch_size=100 and 250 docs, expect at least 3 batches
    assert!(
        batch_count >= 3,
        "Expected at least 3 batches, got {}",
        batch_count
    );
}

#[tokio::test]
async fn test_find_stream_empty_result() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Insert one doc so collection exists, then query for something that doesn't match
    client
        .insert_many(InsertManyRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            documents: vec![make_doc(&doc! { "x": 1 })],
            transaction_id: None,
        })
        .await
        .unwrap();

    // Stream with filter matching nothing
    let response = client
        .find_stream(FindStreamRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            filter: Some(Filter {
                data: encode_doc(&doc! { "nonexistent_field": "no_match" }),
            }),
            options: None,
            transaction_id: None,
            batch_size: 100,
        })
        .await
        .unwrap();

    let mut stream = response.into_inner();
    let mut batch_count = 0u32;

    while let Some(batch) = stream.next().await {
        let batch = batch.unwrap();
        batch_count += 1;
        assert!(batch.documents.is_empty());
        assert!(!batch.has_more);
    }

    // Should get exactly one empty batch indicating end of stream
    assert_eq!(batch_count, 1);
}

#[tokio::test]
async fn test_aggregate_stream() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Insert 100 docs with 5 categories (20 each)
    let categories = ["alpha", "beta", "gamma", "delta", "epsilon"];
    let documents: Vec<Document> = (0..100)
        .map(|i| {
            make_doc(&doc! {
                "category": categories[i % 5],
                "value": (i as i32) * 10
            })
        })
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

    // Aggregate with $group by category
    let group_stage = doc! {
        "$group": {
            "_id": "$category",
            "count": { "$sum": 1 }
        }
    };

    let response = client
        .aggregate_stream(AggregateStreamRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            pipeline: Some(Pipeline {
                stages: vec![encode_doc(&group_stage)],
            }),
            transaction_id: None,
            batch_size: 10,
        })
        .await
        .unwrap();

    let mut stream = response.into_inner();
    let mut all_docs: Vec<bson::Document> = Vec::new();

    while let Some(batch) = stream.next().await {
        let batch = batch.unwrap();
        for proto_doc in &batch.documents {
            all_docs.push(decode_doc(proto_doc));
        }
    }

    // Should have exactly 5 groups
    assert_eq!(all_docs.len(), 5);

    // Each group should have count=20
    for d in &all_docs {
        assert_eq!(d.get_i32("count").unwrap(), 20);
    }

    // Verify all categories present
    let mut found_categories: Vec<String> = all_docs
        .iter()
        .map(|d| d.get_str("_id").unwrap().to_string())
        .collect();
    found_categories.sort();
    let mut expected: Vec<String> = categories.iter().map(|s| s.to_string()).collect();
    expected.sort();
    assert_eq!(found_categories, expected);
}

#[tokio::test]
async fn test_insert_many_bidi() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Create 3 batches of 50 documents each
    let batches: Vec<InsertBatch> = (0..3)
        .map(|batch_idx| {
            let documents: Vec<Document> = (0..50)
                .map(|i| {
                    make_doc(
                        &doc! { "batch": batch_idx, "index": i, "data": format!("b{}i{}", batch_idx, i) },
                    )
                })
                .collect();
            InsertBatch {
                database: TEST_DB.to_string(),
                collection: coll.clone(),
                documents,
            }
        })
        .collect();

    // Send via bidirectional stream
    let inbound = tokio_stream::iter(batches);
    let response = client.insert_many_bidi(inbound).await.unwrap();
    let mut stream = response.into_inner();

    let mut acks: Vec<(u32, u32)> = Vec::new();
    while let Some(ack) = stream.next().await {
        let ack = ack.unwrap();
        assert!(ack.errors.is_empty());
        acks.push((ack.batch_index, ack.inserted_count));
    }

    // Should get 3 acks, one per batch, each with 50 inserted
    assert_eq!(acks.len(), 3);
    for (idx, (batch_index, inserted_count)) in acks.iter().enumerate() {
        assert_eq!(*batch_index, idx as u32);
        assert_eq!(*inserted_count, 50);
    }
}
