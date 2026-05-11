use async_trait::async_trait;
use mongocore::compiled::providers::{LlmError, LlmProvider, TranslationContext};
use mongocore::compiled::translator::CompiledQueryTranslator;
use mongocore::compiled::CompiledMql;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[allow(unused_imports)]
#[path = "../harness/mod.rs"]
mod harness;

/// Mock LLM provider that returns known MQL for test intents.
struct MockLlmProvider {
    call_count: Arc<AtomicUsize>,
}

impl MockLlmProvider {
    fn new() -> Self {
        Self {
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn with_counter(counter: Arc<AtomicUsize>) -> Self {
        Self {
            call_count: counter,
        }
    }

    #[allow(dead_code)]
    fn calls(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl LlmProvider for MockLlmProvider {
    async fn translate(
        &self,
        intent: &str,
        _database: &str,
        _collection: &str,
        _context: &TranslationContext,
    ) -> Result<String, LlmError> {
        self.call_count.fetch_add(1, Ordering::SeqCst);

        // Return different MQL based on intent
        let response = match intent.to_lowercase().as_str() {
            i if i.contains("active users") => {
                r#"{"type": "find", "filter": {"status": "active"}}"#
            }
            i if i.contains("average salary") => {
                r#"{"type": "aggregate", "pipeline": [{"$group": {"_id": null, "avg_salary": {"$avg": "$salary"}}}]}"#
            }
            i if i.contains("expensive") => {
                r#"{"type": "find", "filter": {"price": {"$gt": 100}}}"#
            }
            i if i.contains("under") => r#"{"type": "find", "filter": {"price": {"$lt": 50}}}"#,
            _ => r#"{"type": "find", "filter": {}}"#,
        };
        Ok(response.to_string())
    }
}

#[tokio::test]
async fn test_compiled_query_basic_translation() {
    let provider = MockLlmProvider::new();
    let translator = CompiledQueryTranslator::new(None, Some(Box::new(provider)), None);
    let context = TranslationContext::default();

    let result = translator
        .translate("find active users", "testdb", "users", &context)
        .await
        .expect("translation should succeed");

    match &result.mql {
        CompiledMql::Find { filter, .. } => {
            assert_eq!(filter.get_str("status").unwrap(), "active");
        }
        _ => panic!("Expected CompiledMql::Find"),
    }
    assert_eq!(result.intent, "find active users");
    assert_eq!(result.database, "testdb");
    assert_eq!(result.collection, "users");
}

#[tokio::test]
async fn test_compiled_query_cache_hit() {
    let counter = Arc::new(AtomicUsize::new(0));
    let provider = MockLlmProvider::with_counter(counter.clone());
    let translator = CompiledQueryTranslator::new(None, Some(Box::new(provider)), None);
    let context = TranslationContext::default();

    // First call - should hit LLM
    let result1 = translator
        .translate("find active users", "testdb", "users", &context)
        .await
        .expect("first translation should succeed");

    // Second call - should be cache hit
    let result2 = translator
        .translate("find active users", "testdb", "users", &context)
        .await
        .expect("second translation should succeed");

    // LLM called only once
    assert_eq!(counter.load(Ordering::SeqCst), 1);
    // Same results
    assert_eq!(result1.hash, result2.hash);
    // Cache has entry
    assert_eq!(translator.cache_size(), 1);
}

#[tokio::test]
async fn test_compiled_query_different_intents() {
    let counter = Arc::new(AtomicUsize::new(0));
    let provider = MockLlmProvider::with_counter(counter.clone());
    let translator = CompiledQueryTranslator::new(None, Some(Box::new(provider)), None);
    let context = TranslationContext::default();

    let result1 = translator
        .translate("find active users", "testdb", "users", &context)
        .await
        .expect("first translation should succeed");

    let result2 = translator
        .translate("find expensive items", "testdb", "products", &context)
        .await
        .expect("second translation should succeed");

    // Both called LLM
    assert_eq!(counter.load(Ordering::SeqCst), 2);
    // Different hashes
    assert_ne!(result1.hash, result2.hash);
    // Cache has both
    assert_eq!(translator.cache_size(), 2);
}

#[tokio::test]
async fn test_compiled_query_aggregate_translation() {
    let provider = MockLlmProvider::new();
    let translator = CompiledQueryTranslator::new(None, Some(Box::new(provider)), None);
    let context = TranslationContext::default();

    let result = translator
        .translate(
            "average salary by department",
            "testdb",
            "employees",
            &context,
        )
        .await
        .expect("translation should succeed");

    match &result.mql {
        CompiledMql::Aggregate { pipeline } => {
            assert!(!pipeline.is_empty());
            assert!(pipeline[0].contains_key("$group"));
        }
        _ => panic!("Expected CompiledMql::Aggregate"),
    }
}

#[tokio::test]
async fn test_compiled_query_with_atlas_cache() {
    let pool = harness::get_test_pool().await;

    // Clean the cache collection first
    harness::mongodb::clean_collection(&pool, "compiled_queries").await;

    let provider = MockLlmProvider::new();
    let translator =
        CompiledQueryTranslator::new(Some(pool.clone()), Some(Box::new(provider)), None);
    let context = TranslationContext::default();

    // Translate - stores in L1 and L3 (Atlas)
    let result = translator
        .translate("find active users", "testdb", "users", &context)
        .await
        .expect("translation should succeed");

    // Verify it was stored in Atlas by checking with the collection directly
    let coll = pool
        .database("__mongocore")
        .collection::<bson::Document>("compiled_queries");
    let stored = coll
        .find_one(bson::doc! { "hash": &result.hash })
        .await
        .expect("Atlas query should succeed");
    assert!(stored.is_some(), "Query should be stored in Atlas cache");

    // Create a new translator with the same pool (no LLM) to verify L3 cache hit
    let translator2 = CompiledQueryTranslator::new(Some(pool.clone()), None, None);

    let cached = translator2
        .translate("find active users", "testdb", "users", &context)
        .await
        .expect("should get cache hit from Atlas");
    assert_eq!(cached.hash, result.hash);
}

#[tokio::test]
async fn test_compiled_query_disk_cache() {
    let dir = tempfile::tempdir().expect("should create temp dir");

    let counter = Arc::new(AtomicUsize::new(0));
    let provider = MockLlmProvider::with_counter(counter.clone());
    let translator = CompiledQueryTranslator::new(
        None,
        Some(Box::new(provider)),
        Some(dir.path().to_path_buf()),
    );
    let context = TranslationContext::default();

    // Translate - stores in L1 and L2 (disk)
    let result = translator
        .translate("find active users", "testdb", "users", &context)
        .await
        .expect("translation should succeed");

    assert_eq!(counter.load(Ordering::SeqCst), 1);

    // Verify file was written to disk
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .expect("should read dir")
        .collect();
    assert!(!entries.is_empty(), "Disk cache should have files");

    // Create new translator with same disk dir but no LLM - simulates fresh start
    let translator2 = CompiledQueryTranslator::new(None, None, Some(dir.path().to_path_buf()));

    let cached = translator2
        .translate("find active users", "testdb", "users", &context)
        .await
        .expect("should get cache hit from disk");
    assert_eq!(cached.hash, result.hash);

    // LLM was never called for the second translator
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_compiled_query_template_extraction() {
    let provider = MockLlmProvider::new();
    let translator = CompiledQueryTranslator::new(None, Some(Box::new(provider)), None);
    let context = TranslationContext::default();

    let result = translator
        .translate("find items under $50", "testdb", "products", &context)
        .await
        .expect("translation should succeed");

    // Template should be extracted since intent contains "$50"
    let template = result.template.expect("should have extracted a template");
    assert!(!template.parameters.is_empty());
    assert!(template.pattern.contains("{price_0}"));
    assert!(matches!(
        template.parameters[0].value_type,
        mongocore::compiled::ParameterType::Number
    ));
}
