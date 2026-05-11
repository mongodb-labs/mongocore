use bson::doc;
use uuid::Uuid;

use mongocore::operations::crud::Operations;

#[path = "../harness/mod.rs"]
mod harness;

const TEST_DB: &str = "mongocore_test";

fn unique_collection() -> String {
    format!("test_crud_{}", Uuid::new_v4().to_string().replace('-', ""))
}

#[tokio::test]
async fn test_insert_and_find() {
    let pool = harness::get_test_pool().await;
    let ops = Operations::new(pool);
    let coll = unique_collection();

    let doc = doc! { "name": "Alice", "age": 30 };
    ops.insert(TEST_DB, &coll, doc.clone()).await.unwrap();

    let results = ops.find(TEST_DB, &coll, doc! { "name": "Alice" }, None).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get_str("name").unwrap(), "Alice");
    assert_eq!(results[0].get_i32("age").unwrap(), 30);
}

#[tokio::test]
async fn test_insert_many_and_find() {
    let pool = harness::get_test_pool().await;
    let ops = Operations::new(pool);
    let coll = unique_collection();

    let docs = vec![
        doc! { "name": "Bob", "score": 85 },
        doc! { "name": "Carol", "score": 92 },
        doc! { "name": "Dave", "score": 78 },
    ];
    let result = ops.insert_many(TEST_DB, &coll, docs).await.unwrap();
    assert_eq!(result.inserted_ids.len(), 3);

    let results = ops.find(TEST_DB, &coll, doc! {}, None).await.unwrap();
    assert_eq!(results.len(), 3);
}

#[tokio::test]
async fn test_update_and_verify() {
    let pool = harness::get_test_pool().await;
    let ops = Operations::new(pool);
    let coll = unique_collection();

    ops.insert(TEST_DB, &coll, doc! { "name": "Eve", "status": "active" }).await.unwrap();

    let update_result = ops
        .update(TEST_DB, &coll, doc! { "name": "Eve" }, doc! { "$set": { "status": "inactive" } })
        .await
        .unwrap();
    assert_eq!(update_result.modified_count, 1);

    let results = ops.find(TEST_DB, &coll, doc! { "name": "Eve" }, None).await.unwrap();
    assert_eq!(results[0].get_str("status").unwrap(), "inactive");
}

#[tokio::test]
async fn test_update_many_and_verify() {
    let pool = harness::get_test_pool().await;
    let ops = Operations::new(pool);
    let coll = unique_collection();

    let docs = vec![
        doc! { "category": "A", "processed": false },
        doc! { "category": "A", "processed": false },
        doc! { "category": "B", "processed": false },
    ];
    ops.insert_many(TEST_DB, &coll, docs).await.unwrap();

    let update_result = ops
        .update_many(
            TEST_DB,
            &coll,
            doc! { "category": "A" },
            doc! { "$set": { "processed": true } },
        )
        .await
        .unwrap();
    assert_eq!(update_result.modified_count, 2);

    let results = ops.find(TEST_DB, &coll, doc! { "category": "A" }, None).await.unwrap();
    for r in results {
        assert_eq!(r.get_bool("processed").unwrap(), true);
    }
}

#[tokio::test]
async fn test_delete_and_verify() {
    let pool = harness::get_test_pool().await;
    let ops = Operations::new(pool);
    let coll = unique_collection();

    ops.insert(TEST_DB, &coll, doc! { "name": "Frank" }).await.unwrap();
    ops.insert(TEST_DB, &coll, doc! { "name": "Grace" }).await.unwrap();

    let delete_result = ops.delete(TEST_DB, &coll, doc! { "name": "Frank" }).await.unwrap();
    assert_eq!(delete_result.deleted_count, 1);

    let results = ops.find(TEST_DB, &coll, doc! {}, None).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get_str("name").unwrap(), "Grace");
}

#[tokio::test]
async fn test_delete_many_and_verify() {
    let pool = harness::get_test_pool().await;
    let ops = Operations::new(pool);
    let coll = unique_collection();

    let docs = vec![
        doc! { "group": "x", "val": 1 },
        doc! { "group": "x", "val": 2 },
        doc! { "group": "y", "val": 3 },
    ];
    ops.insert_many(TEST_DB, &coll, docs).await.unwrap();

    let delete_result = ops.delete_many(TEST_DB, &coll, doc! { "group": "x" }).await.unwrap();
    assert_eq!(delete_result.deleted_count, 2);

    let results = ops.find(TEST_DB, &coll, doc! {}, None).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].get_str("group").unwrap(), "y");
}

#[tokio::test]
async fn test_find_one_returns_some() {
    let pool = harness::get_test_pool().await;
    let ops = Operations::new(pool);
    let coll = unique_collection();

    ops.insert(TEST_DB, &coll, doc! { "key": "unique_value" }).await.unwrap();

    let result = ops.find_one(TEST_DB, &coll, doc! { "key": "unique_value" }).await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().get_str("key").unwrap(), "unique_value");
}

#[tokio::test]
async fn test_find_one_returns_none() {
    let pool = harness::get_test_pool().await;
    let ops = Operations::new(pool);
    let coll = unique_collection();

    let result = ops.find_one(TEST_DB, &coll, doc! { "nonexistent": true }).await.unwrap();
    assert!(result.is_none());
}
