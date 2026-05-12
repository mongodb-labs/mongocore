use bson::{doc, Document};
use mongocore::compiled::providers::claude::ClaudeProvider;
use mongocore::compiled::providers::gateway::{GatewayConfig, GatewayProvider};
use mongocore::compiled::providers::openai::OpenAiProvider;
use mongocore::compiled::providers::{LlmProvider, TranslationContext};
use mongocore::compiled::translator::CompiledQueryTranslator;
use mongocore::compiled::CompiledMql;
use mongocore::connection::pool::ConnectionPool;

#[allow(unused_imports)]
#[path = "../harness/mod.rs"]
mod harness;

/// Check if LLM integration tests are enabled via TEST_LLM_INTEGRATION env var.
fn llm_tests_enabled() -> bool {
    std::env::var("TEST_LLM_INTEGRATION")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
}

/// Returns a real LLM provider based on configuration, or None if not configured.
/// Checks gateway first (LLM_BASE_URL), then direct keys (ANTHROPIC_API_KEY, OPENAI_API_KEY).
fn get_llm_provider() -> Option<Box<dyn LlmProvider>> {
    // Gateway takes precedence
    if let Ok(base_url) = std::env::var("LLM_BASE_URL") {
        let api_key = std::env::var("LLM_API_KEY").unwrap_or_default();
        let auth_header = std::env::var("LLM_AUTH_HEADER").unwrap_or_else(|_| "api-key".to_string());
        let model = std::env::var("LLM_MODEL").unwrap_or_else(|_| "claude-sonnet-4-6".to_string());
        let provider_type = std::env::var("LLM_PROVIDER_TYPE").unwrap_or_else(|_| "anthropic".to_string());
        return Some(Box::new(GatewayProvider::new(GatewayConfig {
            base_url,
            api_key,
            auth_header,
            model,
            provider_type,
        })));
    }
    // Direct API keys
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        Some(Box::new(ClaudeProvider::new(key)))
    } else if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        Some(Box::new(OpenAiProvider::new(key)))
    } else {
        None
    }
}

/// Check if sample_restaurants data is loaded. Returns false if not available (test should skip).
async fn has_sample_data(pool: &ConnectionPool) -> bool {
    let count = pool
        .database("sample_restaurants")
        .collection::<Document>("restaurants")
        .count_documents(doc! {})
        .await
        .unwrap_or(0);
    count > 100
}

/// Build a TranslationContext with the sample_restaurants.restaurants schema.
fn restaurants_context() -> TranslationContext {
    TranslationContext {
        schema_hint: Some(
            "Fields: name (String), cuisine (String), borough (String), \
             address.building (String), address.street (String), address.zipcode (String), \
             address.coord ([Double]), grades ([{date: Date, grade: String, score: Int}]), \
             restaurant_id (String)"
                .to_string(),
        ),
        sample_documents: vec![],
        available_indexes: vec![],
    }
}

/// Execute a CompiledMql against the restaurants collection and return results.
async fn execute_mql(pool: &ConnectionPool, mql: &CompiledMql) -> Vec<Document> {
    let coll = pool
        .database("sample_restaurants")
        .collection::<Document>("restaurants");

    match mql {
        CompiledMql::Find { filter, options } => {
            use futures::StreamExt;
            let mut find = coll.find(filter.clone());
            if let Some(opts) = options {
                if let Ok(limit) = opts.get_i64("limit") {
                    find = find.limit(limit);
                }
                if let Some(sort) = opts.get_document("sort").ok() {
                    find = find.sort(sort.clone());
                }
            }
            find.await
                .expect("find failed")
                .collect::<Vec<_>>()
                .await
                .into_iter()
                .filter_map(|r| r.ok())
                .take(20)
                .collect()
        }
        CompiledMql::Aggregate { pipeline } => {
            use futures::StreamExt;
            coll.aggregate(pipeline.clone())
                .await
                .expect("aggregate failed")
                .collect::<Vec<_>>()
                .await
                .into_iter()
                .filter_map(|r| r.ok())
                .take(20)
                .collect()
        }
    }
}

// ==================== Tests ====================

#[tokio::test]
async fn test_llm_find_italian_restaurants() {
    if !llm_tests_enabled() {
        eprintln!("Skipping: TEST_LLM_INTEGRATION not set");
        return;
    }
    let Some(provider) = get_llm_provider() else {
        eprintln!("Skipping test_llm_find_italian_restaurants: no API key set");
        return;
    };
    let pool = harness::get_test_pool().await;
    if !has_sample_data(&pool).await {
        eprintln!("Skipping test_llm_find_italian_restaurants: sample_restaurants not loaded (set LOAD_SAMPLE_DATA=true)");
        return;
    }

    let translator = CompiledQueryTranslator::new(None, Some(provider), None);
    let context = restaurants_context();

    let result = translator
        .translate("find Italian restaurants", "sample_restaurants", "restaurants", &context)
        .await
        .expect("translation should succeed");

    eprintln!("MQL: {:?}", result.mql);

    let docs = execute_mql(&pool, &result.mql).await;
    assert!(!docs.is_empty(), "Expected results for Italian restaurants query");

    for doc in &docs {
        if let Ok(cuisine) = doc.get_str("cuisine") {
            assert_eq!(cuisine, "Italian", "Expected Italian cuisine, got {}", cuisine);
        }
    }
}

#[tokio::test]
async fn test_llm_find_restaurants_in_borough() {
    if !llm_tests_enabled() {
        eprintln!("Skipping: TEST_LLM_INTEGRATION not set");
        return;
    }
    let Some(provider) = get_llm_provider() else {
        eprintln!("Skipping test_llm_find_restaurants_in_borough: no API key set");
        return;
    };
    let pool = harness::get_test_pool().await;
    if !has_sample_data(&pool).await {
        eprintln!("Skipping test_llm_find_restaurants_in_borough: sample_restaurants not loaded");
        return;
    }

    let translator = CompiledQueryTranslator::new(None, Some(provider), None);
    let context = restaurants_context();

    let result = translator
        .translate("find restaurants in Manhattan", "sample_restaurants", "restaurants", &context)
        .await
        .expect("translation should succeed");

    eprintln!("MQL: {:?}", result.mql);

    let docs = execute_mql(&pool, &result.mql).await;
    assert!(!docs.is_empty(), "Expected results for Manhattan restaurants query");

    for doc in &docs {
        if let Ok(borough) = doc.get_str("borough") {
            assert_eq!(borough, "Manhattan", "Expected Manhattan, got {}", borough);
        }
    }
}

#[tokio::test]
async fn test_llm_find_high_scoring_restaurants() {
    if !llm_tests_enabled() {
        eprintln!("Skipping: TEST_LLM_INTEGRATION not set");
        return;
    }
    let Some(provider) = get_llm_provider() else {
        eprintln!("Skipping test_llm_find_high_scoring_restaurants: no API key set");
        return;
    };
    let pool = harness::get_test_pool().await;
    if !has_sample_data(&pool).await {
        eprintln!("Skipping test_llm_find_high_scoring_restaurants: sample_restaurants not loaded");
        return;
    }

    let translator = CompiledQueryTranslator::new(None, Some(provider), None);
    let context = restaurants_context();

    let result = translator
        .translate("find restaurants with a grade score above 50", "sample_restaurants", "restaurants", &context)
        .await
        .expect("translation should succeed");

    eprintln!("MQL: {:?}", result.mql);

    let docs = execute_mql(&pool, &result.mql).await;
    assert!(!docs.is_empty(), "Expected results for high scoring restaurants query");
}

#[tokio::test]
async fn test_llm_count_by_cuisine() {
    if !llm_tests_enabled() {
        eprintln!("Skipping: TEST_LLM_INTEGRATION not set");
        return;
    }
    let Some(provider) = get_llm_provider() else {
        eprintln!("Skipping test_llm_count_by_cuisine: no API key set");
        return;
    };
    let pool = harness::get_test_pool().await;
    if !has_sample_data(&pool).await {
        eprintln!("Skipping test_llm_count_by_cuisine: sample_restaurants not loaded");
        return;
    }

    let translator = CompiledQueryTranslator::new(None, Some(provider), None);
    let context = restaurants_context();

    let result = translator
        .translate("count restaurants by cuisine type", "sample_restaurants", "restaurants", &context)
        .await
        .expect("translation should succeed");

    eprintln!("MQL: {:?}", result.mql);

    match &result.mql {
        CompiledMql::Aggregate { pipeline } => {
            assert!(!pipeline.is_empty(), "Pipeline should not be empty");
        }
        CompiledMql::Find { .. } => {
            eprintln!("WARN: LLM returned Find instead of Aggregate for count query — acceptable");
        }
    }

    let docs = execute_mql(&pool, &result.mql).await;
    assert!(!docs.is_empty(), "Expected results for count by cuisine query");
}

#[tokio::test]
async fn test_llm_average_score_by_borough() {
    if !llm_tests_enabled() {
        eprintln!("Skipping: TEST_LLM_INTEGRATION not set");
        return;
    }
    let Some(provider) = get_llm_provider() else {
        eprintln!("Skipping test_llm_average_score_by_borough: no API key set");
        return;
    };
    let pool = harness::get_test_pool().await;
    if !has_sample_data(&pool).await {
        eprintln!("Skipping test_llm_average_score_by_borough: sample_restaurants not loaded");
        return;
    }

    let translator = CompiledQueryTranslator::new(None, Some(provider), None);
    let context = restaurants_context();

    let result = translator
        .translate("average inspection score by borough", "sample_restaurants", "restaurants", &context)
        .await
        .expect("translation should succeed");

    eprintln!("MQL: {:?}", result.mql);

    let docs = execute_mql(&pool, &result.mql).await;
    assert!(!docs.is_empty(), "Expected results for average score query");
}

#[tokio::test]
async fn test_llm_cache_reuse() {
    if !llm_tests_enabled() {
        eprintln!("Skipping: TEST_LLM_INTEGRATION not set");
        return;
    }
    let Some(provider) = get_llm_provider() else {
        eprintln!("Skipping test_llm_cache_reuse: no API key set");
        return;
    };
    let pool = harness::get_test_pool().await;
    if !has_sample_data(&pool).await {
        eprintln!("Skipping test_llm_cache_reuse: sample_restaurants not loaded");
        return;
    }

    let translator = CompiledQueryTranslator::new(None, Some(provider), None);
    let context = restaurants_context();

    // First call — hits LLM
    let result1 = translator
        .translate("find Italian restaurants", "sample_restaurants", "restaurants", &context)
        .await
        .expect("first translation should succeed");

    // Second call — should be cache hit
    let result2 = translator
        .translate("find Italian restaurants", "sample_restaurants", "restaurants", &context)
        .await
        .expect("second translation should succeed");

    // Same hash = cache hit
    assert_eq!(result1.hash, result2.hash, "Second call should be a cache hit");
    assert_eq!(translator.cache_size(), 1);
}

#[tokio::test]
async fn test_llm_template_cache_reuse() {
    if !llm_tests_enabled() {
        eprintln!("Skipping: TEST_LLM_INTEGRATION not set");
        return;
    }
    let Some(provider) = get_llm_provider() else {
        eprintln!("Skipping test_llm_template_cache_reuse: no API key set");
        return;
    };
    let pool = harness::get_test_pool().await;
    if !has_sample_data(&pool).await {
        eprintln!("Skipping test_llm_template_cache_reuse: sample_restaurants not loaded");
        return;
    }

    let translator = CompiledQueryTranslator::new(None, Some(provider), None);
    let context = restaurants_context();

    // First call with "Manhattan" — hits LLM
    let result1 = translator
        .translate("find restaurants in Manhattan", "sample_restaurants", "restaurants", &context)
        .await
        .expect("first translation should succeed");

    eprintln!("First MQL: {:?}", result1.mql);
    eprintln!("Template: {:?}", result1.template);

    // Second call with "Brooklyn" — should reuse template (no LLM call)
    let result2 = translator
        .translate("find restaurants in Brooklyn", "sample_restaurants", "restaurants", &context)
        .await
        .expect("second translation should succeed");

    eprintln!("Second MQL: {:?}", result2.mql);

    // Execute second query and verify borough == Brooklyn
    let docs = execute_mql(&pool, &result2.mql).await;
    assert!(!docs.is_empty(), "Expected results for Brooklyn restaurants query");

    for doc in &docs {
        if let Ok(borough) = doc.get_str("borough") {
            assert_eq!(borough, "Brooklyn", "Expected Brooklyn from template reuse, got {}", borough);
        }
    }
}
