use bson::doc;
use uuid::Uuid;

use mongocore::operations::crud::Operations;

#[path = "../harness/mod.rs"]
mod harness;

const TEST_DB: &str = "mongocore_test";

fn unique_collection() -> String {
    format!("test_txn_{}", Uuid::new_v4().to_string().replace('-', ""))
}

#[tokio::test]
async fn test_transaction_commit() {
    let pool = harness::get_test_pool().await;
    let ops = Operations::new(pool.clone());
    let coll = unique_collection();

    // Begin transaction, insert, commit
    let mut txn = ops.begin_transaction().await.unwrap();
    txn.insert(TEST_DB, &coll, doc! { "name": "committed_doc", "value": 42 })
        .await
        .unwrap();
    txn.commit().await.unwrap();

    // Document should be visible outside the transaction
    let results = ops.find(TEST_DB, &coll, doc! { "name": "committed_doc" }, None).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get_i32("value").unwrap(), 42);
}

#[tokio::test]
async fn test_transaction_abort() {
    let pool = harness::get_test_pool().await;
    let ops = Operations::new(pool.clone());
    let coll = unique_collection();

    // Begin transaction, insert, abort
    let mut txn = ops.begin_transaction().await.unwrap();
    txn.insert(TEST_DB, &coll, doc! { "name": "aborted_doc", "value": 99 })
        .await
        .unwrap();
    txn.abort().await.unwrap();

    // Document should NOT be visible
    let results = ops.find(TEST_DB, &coll, doc! { "name": "aborted_doc" }, None).await.unwrap();
    assert_eq!(results.len(), 0);
}
