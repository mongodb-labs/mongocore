use std::sync::Arc;

use bson::doc;
use uuid::Uuid;

use mongocore::operations::crud::Operations;
use mongocore::operations::IndexOptions;
use mongocore::search::fulltext::FulltextSearchBuilder;
use mongocore::search::vector::VectorSearchBuilder;
use mongocore::search::{SearchEngine, SearchError, SearchMethod};

#[path = "../harness/mod.rs"]
mod harness;

fn unique_collection() -> String {
    format!("test_search_{}", Uuid::new_v4().to_string().replace('-', ""))
}

#[tokio::test]
async fn test_search_fallback_to_filter() {
    let pool = harness::get_test_pool().await;
    let ops = Operations::new(pool.clone());
    let coll = unique_collection();

    // Insert documents
    ops.insert_many(
        harness::TEST_DB,
        &coll,
        vec![
            doc! { "title": "rust programming guide", "content": "learn rust basics" },
            doc! { "title": "python basics", "content": "learn python programming" },
            doc! { "title": "rust advanced patterns", "content": "advanced rust techniques" },
        ],
    )
    .await
    .unwrap();

    // Search without Voyage AI — will use fulltext on Atlas Local, filter otherwise
    let engine = SearchEngine::new(pool, None);
    let result = engine
        .search(harness::TEST_DB, &coll, "rust", 10)
        .await
        .unwrap();

    // On Atlas Local: fulltext via $search with "default" index
    // On plain MongoDB: falls back to filter
    assert!(
        result.method == SearchMethod::Fulltext || result.method == SearchMethod::Filter,
        "Expected Fulltext or Filter, got {:?}", result.method
    );
    // Documents may or may not be returned depending on index readiness
}

#[tokio::test]
async fn test_search_with_text_index() {
    let pool = harness::get_test_pool().await;
    let ops = Operations::new(pool.clone());
    let coll = unique_collection();

    // Insert documents
    ops.insert_many(
        harness::TEST_DB,
        &coll,
        vec![
            doc! { "title": "rust programming guide", "content": "learn rust basics" },
            doc! { "title": "python basics", "content": "learn python programming" },
            doc! { "title": "rust advanced patterns", "content": "advanced rust techniques" },
        ],
    )
    .await
    .unwrap();

    // Create a text index on all string fields
    ops.create_index(
        harness::TEST_DB,
        &coll,
        doc! { "$**": "text" },
        Some(IndexOptions {
            name: Some("text_wildcard".to_string()),
            ..Default::default()
        }),
    )
    .await
    .unwrap();

    // Search — with a text index, the $text filter fallback should work
    let engine = SearchEngine::new(pool, None);
    let result = engine
        .search(harness::TEST_DB, &coll, "rust", 10)
        .await
        .unwrap();

    // On Atlas Local: fulltext may return empty (no "default" search index for this collection),
    // so it falls through to filter which uses $text with our text index
    assert!(
        result.method == SearchMethod::Fulltext || result.method == SearchMethod::Filter,
        "Expected Fulltext or Filter, got {:?}", result.method
    );
    assert!(result.total >= 2, "Expected at least 2 results matching 'rust', got {}", result.total);
    for d in &result.documents {
        let title = d.get_str("title").unwrap_or("");
        let content = d.get_str("content").unwrap_or("");
        assert!(
            title.contains("rust") || content.contains("rust"),
            "Document should contain 'rust': {:?}",
            d
        );
    }
}

#[tokio::test]
async fn test_vector_search_requires_voyage() {
    let pool = harness::get_test_pool().await;
    let coll = unique_collection();

    // SearchEngine without Voyage client
    let engine = SearchEngine::new(pool, None);
    let result = engine
        .vector_search(harness::TEST_DB, &coll, "test query", "default", "embedding", 10)
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        SearchError::NotConfigured(msg) => {
            assert!(msg.contains("Voyage AI"), "Error should mention Voyage AI: {}", msg);
        }
        other => panic!("Expected NotConfigured error, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_vector_search_pipeline_builder() {
    let query_vector = vec![0.1, 0.2, 0.3, 0.4, 0.5];
    let pipeline = VectorSearchBuilder::build_pipeline(
        "my_vector_index",
        "embedding_field",
        query_vector.clone(),
        100,
        10,
    );

    // Pipeline should have 2 stages: $vectorSearch and $addFields
    assert_eq!(pipeline.len(), 2);

    // Verify $vectorSearch stage structure
    let vs_stage = &pipeline[0];
    let vs = vs_stage.get_document("$vectorSearch").unwrap();
    assert_eq!(vs.get_str("index").unwrap(), "my_vector_index");
    assert_eq!(vs.get_str("path").unwrap(), "embedding_field");
    assert_eq!(vs.get_i64("numCandidates").unwrap(), 100);
    assert_eq!(vs.get_i64("limit").unwrap(), 10);

    let bson_vec = vs.get_array("queryVector").unwrap();
    assert_eq!(bson_vec.len(), 5);

    // Verify $addFields stage
    let add_fields = &pipeline[1];
    let fields = add_fields.get_document("$addFields").unwrap();
    let score = fields.get_document("search_score").unwrap();
    assert_eq!(score.get_str("$meta").unwrap(), "vectorSearchScore");

    // Verify the pipeline is valid BSON by serializing/deserializing
    for stage in &pipeline {
        let bytes = bson::to_vec(stage).unwrap();
        let _roundtrip: bson::Document = bson::from_slice(&bytes).unwrap();
    }
}

#[tokio::test]
async fn test_fulltext_pipeline_builder() {
    let pipeline = FulltextSearchBuilder::build_pipeline(
        "default",
        "search query terms",
        &["title", "content", "description"],
        25,
    );

    // Pipeline should have 3 stages: $search, $addFields, $limit
    assert_eq!(pipeline.len(), 3);

    // Verify $search stage
    let search_stage = &pipeline[0];
    let search = search_stage.get_document("$search").unwrap();
    assert_eq!(search.get_str("index").unwrap(), "default");

    let text = search.get_document("text").unwrap();
    assert_eq!(text.get_str("query").unwrap(), "search query terms");

    let path = text.get_array("path").unwrap();
    assert_eq!(path.len(), 3);
    assert_eq!(path[0].as_str().unwrap(), "title");
    assert_eq!(path[1].as_str().unwrap(), "content");
    assert_eq!(path[2].as_str().unwrap(), "description");

    // Verify $addFields stage
    let add_fields = &pipeline[1];
    let fields = add_fields.get_document("$addFields").unwrap();
    let score = fields.get_document("search_score").unwrap();
    assert_eq!(score.get_str("$meta").unwrap(), "searchScore");

    // Verify $limit stage
    let limit_stage = &pipeline[2];
    assert_eq!(limit_stage.get_i64("$limit").unwrap(), 25);

    // Verify all stages are valid BSON
    for stage in &pipeline {
        let bytes = bson::to_vec(stage).unwrap();
        let _roundtrip: bson::Document = bson::from_slice(&bytes).unwrap();
    }
}

#[tokio::test]
async fn test_search_engine_creation() {
    let pool = harness::get_test_pool().await;

    // Create without Voyage client
    let engine = SearchEngine::new(pool.clone(), None);
    // Verify it works by running a search on an empty collection
    let coll = unique_collection();
    let result = engine
        .search(harness::TEST_DB, &coll, "anything", 5)
        .await
        .unwrap();
    assert!(
        result.method == SearchMethod::Fulltext || result.method == SearchMethod::Filter,
        "Expected Fulltext or Filter, got {:?}", result.method
    );
    assert_eq!(result.total, 0); // Empty collection

    // Create with a (fake) Voyage client - just verify construction works
    let voyage_client = Arc::new(mongocore::voyage::VoyageClient::new("fake-api-key".to_string()));
    let _engine_with_voyage = SearchEngine::new(pool, Some(voyage_client));
    // We can't test actual vector search without real credentials,
    // but the engine is created successfully
}

#[tokio::test]
async fn test_atlas_vector_search_end_to_end() {
    let pool = harness::get_test_pool().await;

    if !pool.capabilities().atlas_vector_search {
        eprintln!("Skipping: Atlas Vector Search not available");
        return;
    }

    let ops = mongocore::operations::crud::Operations::new(pool.clone());
    let coll = unique_collection();

    // Insert documents with pre-computed embeddings (3 dimensions for simplicity)
    ops.insert_many(
        harness::TEST_DB,
        &coll,
        vec![
            doc! { "title": "cats are great", "embedding": [1.0, 0.0, 0.0] },
            doc! { "title": "dogs are loyal", "embedding": [0.0, 1.0, 0.0] },
            doc! { "title": "cats and dogs", "embedding": [0.7, 0.7, 0.0] },
            doc! { "title": "fish swim fast", "embedding": [0.0, 0.0, 1.0] },
        ],
    )
    .await
    .unwrap();

    // Create a vector search index
    let db = pool.database(harness::TEST_DB);
    let create_result = db.run_command(doc! {
        "createSearchIndexes": &coll,
        "indexes": [{
            "name": "vector_test_idx",
            "type": "vectorSearch",
            "definition": {
                "fields": [{
                    "type": "vector",
                    "path": "embedding",
                    "numDimensions": 3,
                    "similarity": "cosine"
                }]
            }
        }]
    }).await;

    if create_result.is_err() {
        eprintln!("Skipping: Could not create vector search index");
        return;
    }

    // Give the index a moment to become ready
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Run $vectorSearch pipeline directly via aggregation
    let pipeline = VectorSearchBuilder::build_pipeline(
        "vector_test_idx",
        "embedding",
        vec![1.0, 0.0, 0.0], // query vector close to "cats are great"
        10,
        3,
    );

    let results = ops.aggregate(harness::TEST_DB, &coll, pipeline).await.unwrap();

    assert!(!results.is_empty(), "Expected vector search results");
    assert!(results.len() <= 3, "Expected at most 3 results");

    // The closest match to [1,0,0] should be "cats are great" ([1,0,0])
    let first_title = results[0].get_str("title").unwrap();
    assert_eq!(first_title, "cats are great");
}

#[tokio::test]
async fn test_atlas_fulltext_search_end_to_end() {
    let pool = harness::get_test_pool().await;

    if !pool.capabilities().atlas_search {
        eprintln!("Skipping: Atlas Search not available");
        return;
    }

    let ops = mongocore::operations::crud::Operations::new(pool.clone());
    let coll = unique_collection();

    // Insert documents
    ops.insert_many(
        harness::TEST_DB,
        &coll,
        vec![
            doc! { "title": "Introduction to Rust Programming", "body": "Rust is a systems language focused on safety" },
            doc! { "title": "Python for Data Science", "body": "Python excels at data analysis and machine learning" },
            doc! { "title": "Advanced Rust Patterns", "body": "Explore ownership, lifetimes, and trait objects in Rust" },
            doc! { "title": "JavaScript Basics", "body": "Learn the fundamentals of JavaScript programming" },
        ],
    )
    .await
    .unwrap();

    // Create an Atlas Search index with dynamic mappings
    let db = pool.database(harness::TEST_DB);
    let create_result = db.run_command(doc! {
        "createSearchIndexes": &coll,
        "indexes": [{
            "name": "fulltext_test_idx",
            "type": "search",
            "definition": {
                "mappings": { "dynamic": true }
            }
        }]
    }).await;

    if create_result.is_err() {
        eprintln!("Skipping: Could not create Atlas Search index");
        return;
    }

    // Give the index time to build
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Run $search pipeline
    let pipeline = FulltextSearchBuilder::build_pipeline(
        "fulltext_test_idx",
        "rust",
        &["title", "body"],
        10,
    );

    let results = ops.aggregate(harness::TEST_DB, &coll, pipeline).await.unwrap();

    assert!(results.len() >= 2, "Expected at least 2 documents mentioning 'rust', got {}", results.len());
    for d in &results {
        let title = d.get_str("title").unwrap_or("");
        let body = d.get_str("body").unwrap_or("");
        assert!(
            title.to_lowercase().contains("rust") || body.to_lowercase().contains("rust"),
            "Document should match 'rust': title={}, body={}", title, body
        );
    }
}

#[tokio::test]
async fn test_search_fallback_chain_with_atlas() {
    let pool = harness::get_test_pool().await;
    let ops = mongocore::operations::crud::Operations::new(pool.clone());
    let coll = unique_collection();

    ops.insert_many(
        harness::TEST_DB,
        &coll,
        vec![
            doc! { "title": "mongodb atlas", "content": "cloud database service" },
            doc! { "title": "atlas search", "content": "full text search on atlas" },
            doc! { "title": "postgres", "content": "relational database" },
        ],
    )
    .await
    .unwrap();

    // Allow index to process
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // SearchEngine without Voyage — on Atlas Local uses fulltext, on plain MongoDB uses filter
    let engine = SearchEngine::new(pool, None);
    let result = engine
        .search(harness::TEST_DB, &coll, "atlas", 10)
        .await
        .unwrap();

    assert!(
        result.method == SearchMethod::Fulltext || result.method == SearchMethod::Filter,
        "Expected Fulltext or Filter, got {:?}", result.method
    );
    // Should find documents containing "atlas"
    assert!(result.total >= 2, "Expected at least 2 results for 'atlas', got {}", result.total);
}
