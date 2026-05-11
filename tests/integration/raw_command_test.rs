use bson::doc;

use mongocore::operations::raw::{run_command, RawCommandOptions};
use mongocore::operations::raw_validator::ValidationMode;
use mongocore::error::MongoCoreError;

#[path = "../harness/mod.rs"]
mod harness;

#[tokio::test]
async fn test_raw_ping() {
    let pool = harness::get_test_pool().await;
    let command = doc! { "ping": 1 };
    let options = RawCommandOptions::default();

    let result = run_command(&pool, "admin", command, &options)
        .await
        .unwrap();

    // Verify the response indicates success
    assert_eq!(result.get_f64("ok").unwrap(), 1.0);
}

#[tokio::test]
async fn test_raw_server_status() {
    let pool = harness::get_test_pool().await;
    let command = doc! { "serverStatus": 1 };
    let options = RawCommandOptions::default();

    let result = run_command(&pool, "admin", command, &options)
        .await
        .unwrap();

    // Verify the response contains version information
    assert!(result.contains_key("version"));
    assert_eq!(result.get_f64("ok").unwrap(), 1.0);
}

#[tokio::test]
async fn test_raw_blocked_command_rejected() {
    let pool = harness::get_test_pool().await;
    let command = doc! { "dropDatabase": 1 };
    let options = RawCommandOptions {
        validation_mode: ValidationMode::BlockDangerous,
    };

    let result = run_command(&pool, harness::TEST_DB, command, &options).await;

    // Verify the command is blocked by validation
    assert!(result.is_err());
    match result.unwrap_err() {
        MongoCoreError::ValidationError(msg) => {
            assert!(msg.contains("dropDatabase"));
            assert!(msg.contains("blocked by validation policy"));
        }
        other => panic!("Expected ValidationError, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_raw_allowed_with_override() {
    let pool = harness::get_test_pool().await;
    let command = doc! { "ping": 1 };
    let options = RawCommandOptions {
        validation_mode: ValidationMode::AllowAll,
    };

    let result = run_command(&pool, "admin", command, &options)
        .await
        .unwrap();

    // Verify the command executes successfully with AllowAll mode
    assert_eq!(result.get_f64("ok").unwrap(), 1.0);
}

#[tokio::test]
async fn test_raw_custom_aggregate() {
    let pool = harness::get_test_pool().await;

    // Insert some test documents first
    let db = pool.database(harness::TEST_DB);
    let collection = db.collection::<bson::Document>("raw_test_collection");

    // Clean up any existing data
    collection.drop().await.ok();

    // Insert test documents
    let docs = vec![
        doc! { "name": "Alice", "score": 85 },
        doc! { "name": "Bob", "score": 92 },
        doc! { "name": "Carol", "score": 78 },
    ];
    collection.insert_many(docs).await.unwrap();

    // Run aggregate command through raw command interface
    let command = doc! {
        "aggregate": "raw_test_collection",
        "pipeline": [
            { "$match": { "score": { "$gte": 80 } } },
            { "$sort": { "score": -1 } }
        ],
        "cursor": {}
    };
    let options = RawCommandOptions::default();

    let result = run_command(&pool, harness::TEST_DB, command, &options)
        .await
        .unwrap();

    // Verify the aggregate command succeeded
    assert_eq!(result.get_f64("ok").unwrap(), 1.0);

    // Extract the cursor from the result
    let cursor_doc = result.get_document("cursor").unwrap();
    let first_batch = cursor_doc.get_array("firstBatch").unwrap();

    // Verify we got 2 results (Alice: 85, Bob: 92)
    assert_eq!(first_batch.len(), 2);

    // Verify the results are sorted by score descending
    let first = first_batch[0].as_document().unwrap();
    let second = first_batch[1].as_document().unwrap();

    assert_eq!(first.get_str("name").unwrap(), "Bob");
    assert_eq!(first.get_i32("score").unwrap(), 92);
    assert_eq!(second.get_str("name").unwrap(), "Alice");
    assert_eq!(second.get_i32("score").unwrap(), 85);
}
