use bson::doc;
use uuid::Uuid;

use mongocore::grpc::proto::mongo_core_client::MongoCoreClient;
use mongocore::grpc::proto::{
    AbortTransactionRequest, AggregateRequest, BeginTransactionRequest, CommitTransactionRequest,
    CreateIndexRequest, DeleteRequest, FindOneRequest, FindRequest, InsertManyRequest,
    InsertRequest, ListDatabasesRequest, UpdateRequest,
};
use mongocore::grpc::proto::{Document, Filter, IndexOptions, Pipeline};
use mongocore::grpc::{start_grpc_server, GrpcServerConfig};

#[path = "../harness/mod.rs"]
mod harness;

const TEST_DB: &str = harness::TEST_DB;

fn unique_collection() -> String {
    format!("test_grpc_{}", Uuid::new_v4().to_string().replace('-', ""))
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

async fn start_test_server() -> MongoCoreClient<tonic::transport::Channel> {
    let pool = harness::get_test_pool().await;

    // Find a free port by binding to port 0
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    // Start the gRPC server
    let _handle = start_grpc_server(pool, GrpcServerConfig { port, transport: "tcp".to_string(), socket_path: "/tmp/mongocore.sock".to_string(), socket_permissions: 0o600, max_message_size: 64 * 1024 * 1024, compression: "none".to_string(), stream_idle_timeout_secs: 60 }, None, None, None, None);

    // Give the server time to bind
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Connect client
    MongoCoreClient::connect(format!("http://127.0.0.1:{}", port))
        .await
        .unwrap()
}

#[tokio::test]
async fn test_grpc_insert_and_find() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Insert a document
    let insert_resp = client
        .insert(InsertRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            document: Some(make_doc(&doc! { "name": "Alice", "age": 30 })),
            transaction_id: None,
        })
        .await
        .unwrap();
    assert!(!insert_resp.into_inner().inserted_id.is_empty());

    // Find it back
    let find_resp = client
        .find(FindRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            filter: make_filter(&doc! { "name": "Alice" }),
            options: None,
            transaction_id: None,
        })
        .await
        .unwrap();

    let docs = find_resp.into_inner().documents;
    assert_eq!(docs.len(), 1);
    let found = decode_doc(&docs[0]);
    assert_eq!(found.get_str("name").unwrap(), "Alice");
    assert_eq!(found.get_i32("age").unwrap(), 30);
}

#[tokio::test]
async fn test_grpc_insert_many_and_find() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    let documents = vec![
        make_doc(&doc! { "name": "Bob", "score": 85 }),
        make_doc(&doc! { "name": "Carol", "score": 92 }),
        make_doc(&doc! { "name": "Dave", "score": 78 }),
    ];

    let insert_resp = client
        .insert_many(InsertManyRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            documents,
            transaction_id: None,
        })
        .await
        .unwrap();
    assert_eq!(insert_resp.into_inner().inserted_count, 3);

    // Find all
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
async fn test_grpc_update() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Insert
    client
        .insert(InsertRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            document: Some(make_doc(&doc! { "name": "Eve", "status": "active" })),
            transaction_id: None,
        })
        .await
        .unwrap();

    // Update
    let update_resp = client
        .update(UpdateRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            filter: make_filter(&doc! { "name": "Eve" }),
            update: Some(make_doc(&doc! { "$set": { "status": "inactive" } })),
            upsert: false,
            transaction_id: None,
        })
        .await
        .unwrap();
    assert_eq!(update_resp.into_inner().modified_count, 1);

    // Verify
    let find_resp = client
        .find(FindRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            filter: make_filter(&doc! { "name": "Eve" }),
            options: None,
            transaction_id: None,
        })
        .await
        .unwrap();
    let docs = find_resp.into_inner().documents;
    let found = decode_doc(&docs[0]);
    assert_eq!(found.get_str("status").unwrap(), "inactive");
}

#[tokio::test]
async fn test_grpc_delete() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Insert two docs
    client
        .insert(InsertRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            document: Some(make_doc(&doc! { "name": "Frank" })),
            transaction_id: None,
        })
        .await
        .unwrap();
    client
        .insert(InsertRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            document: Some(make_doc(&doc! { "name": "Grace" })),
            transaction_id: None,
        })
        .await
        .unwrap();

    // Delete one
    let delete_resp = client
        .delete(DeleteRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            filter: make_filter(&doc! { "name": "Frank" }),
            transaction_id: None,
        })
        .await
        .unwrap();
    assert_eq!(delete_resp.into_inner().deleted_count, 1);

    // Verify only Grace remains
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
    let docs = find_resp.into_inner().documents;
    assert_eq!(docs.len(), 1);
    assert_eq!(decode_doc(&docs[0]).get_str("name").unwrap(), "Grace");
}

#[tokio::test]
async fn test_grpc_aggregate() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Insert docs with categories
    let documents = vec![
        make_doc(&doc! { "category": "A", "value": 10 }),
        make_doc(&doc! { "category": "A", "value": 20 }),
        make_doc(&doc! { "category": "B", "value": 30 }),
    ];
    client
        .insert_many(InsertManyRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            documents,
            transaction_id: None,
        })
        .await
        .unwrap();

    // Run $group aggregation
    let group_stage = doc! {
        "$group": {
            "_id": "$category",
            "total": { "$sum": "$value" }
        }
    };
    let agg_resp = client
        .aggregate(AggregateRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            pipeline: Some(Pipeline {
                stages: vec![encode_doc(&group_stage)],
            }),
            transaction_id: None,
        })
        .await
        .unwrap();

    let results = agg_resp.into_inner().documents;
    assert_eq!(results.len(), 2);

    // Verify totals (order may vary)
    let mut totals: Vec<(String, i32)> = results
        .iter()
        .map(|d| {
            let doc = decode_doc(d);
            let id = doc.get_str("_id").unwrap().to_string();
            let total = doc.get_i32("total").unwrap();
            (id, total)
        })
        .collect();
    totals.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(totals[0], ("A".to_string(), 30));
    assert_eq!(totals[1], ("B".to_string(), 30));
}

#[tokio::test]
async fn test_grpc_find_one() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    client
        .insert(InsertRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            document: Some(make_doc(&doc! { "key": "unique_grpc_value" })),
            transaction_id: None,
        })
        .await
        .unwrap();

    let resp = client
        .find_one(FindOneRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            filter: make_filter(&doc! { "key": "unique_grpc_value" }),
            options: None,
            transaction_id: None,
        })
        .await
        .unwrap();

    let doc = resp.into_inner().document.unwrap();
    assert_eq!(
        decode_doc(&doc).get_str("key").unwrap(),
        "unique_grpc_value"
    );
}

#[tokio::test]
async fn test_grpc_transaction_commit() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Begin transaction
    let begin_resp = client
        .begin_transaction(BeginTransactionRequest {
            database: TEST_DB.to_string(),
        })
        .await
        .unwrap();
    let txn_id = begin_resp.into_inner().transaction_id;

    // Insert within transaction
    client
        .insert(InsertRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            document: Some(make_doc(&doc! { "txn_test": "committed" })),
            transaction_id: Some(txn_id.clone()),
        })
        .await
        .unwrap();

    // Commit
    client
        .commit_transaction(CommitTransactionRequest {
            transaction_id: txn_id,
        })
        .await
        .unwrap();

    // Verify document exists after commit
    let find_resp = client
        .find(FindRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            filter: make_filter(&doc! { "txn_test": "committed" }),
            options: None,
            transaction_id: None,
        })
        .await
        .unwrap();
    assert_eq!(find_resp.into_inner().documents.len(), 1);
}

#[tokio::test]
async fn test_grpc_transaction_abort() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Begin transaction
    let begin_resp = client
        .begin_transaction(BeginTransactionRequest {
            database: TEST_DB.to_string(),
        })
        .await
        .unwrap();
    let txn_id = begin_resp.into_inner().transaction_id;

    // Insert within transaction
    client
        .insert(InsertRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            document: Some(make_doc(&doc! { "txn_test": "aborted" })),
            transaction_id: Some(txn_id.clone()),
        })
        .await
        .unwrap();

    // Abort
    client
        .abort_transaction(AbortTransactionRequest {
            transaction_id: txn_id,
        })
        .await
        .unwrap();

    // Verify document does NOT exist after abort
    let find_resp = client
        .find(FindRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            filter: make_filter(&doc! { "txn_test": "aborted" }),
            options: None,
            transaction_id: None,
        })
        .await
        .unwrap();
    assert_eq!(find_resp.into_inner().documents.len(), 0);
}

#[tokio::test]
async fn test_grpc_list_databases() {
    let mut client = start_test_server().await;

    let resp = client
        .list_databases(ListDatabasesRequest {})
        .await
        .unwrap();

    let databases = resp.into_inner().databases;
    // At minimum, "admin" and "local" should exist on any MongoDB instance
    assert!(!databases.is_empty());
}

#[tokio::test]
async fn test_grpc_create_index() {
    let mut client = start_test_server().await;
    let coll = unique_collection();

    // Insert a doc to ensure collection exists
    client
        .insert(InsertRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            document: Some(make_doc(&doc! { "indexed_field": 1 })),
            transaction_id: None,
        })
        .await
        .unwrap();

    // Create index
    let resp = client
        .create_index(CreateIndexRequest {
            database: TEST_DB.to_string(),
            collection: coll.clone(),
            keys: Some(make_doc(&doc! { "indexed_field": 1 })),
            options: Some(IndexOptions {
                name: Some("idx_indexed_field".to_string()),
                unique: Some(false),
                sparse: Some(false),
            }),
        })
        .await
        .unwrap();

    let index_name = resp.into_inner().index_name;
    assert_eq!(index_name, "idx_indexed_field");
}
