use std::path::PathBuf;

use bson::Document;

use crate::connection::pool::ConnectionPool;

use super::cache::CacheHierarchy;
use super::hasher::QueryHasher;
use super::providers::{LlmError, LlmProvider, TranslationContext};
use super::template::TemplateExtractor;
use super::validator::MqlValidator;
use super::{CompiledMql, CompiledQuery};

pub struct CompiledQueryTranslator {
    cache: CacheHierarchy,
    provider: Option<Box<dyn LlmProvider>>,
}

impl CompiledQueryTranslator {
    pub fn new(
        pool: Option<ConnectionPool>,
        provider: Option<Box<dyn LlmProvider>>,
        cache_dir: Option<PathBuf>,
    ) -> Self {
        let cache = CacheHierarchy::new(pool, cache_dir);
        Self { cache, provider }
    }

    /// Translate an intent to MQL. Checks cache first, then LLM.
    pub async fn translate(
        &self,
        intent: &str,
        database: &str,
        collection: &str,
        context: &TranslationContext,
    ) -> Result<CompiledQuery, TranslateError> {
        let hash = QueryHasher::hash(intent, database, collection, None);

        // Check cache
        if let Some(cached) = self.cache.get(&hash).await {
            return Ok(cached);
        }

        // Need LLM
        let provider = self.provider.as_ref().ok_or(TranslateError::NoProvider)?;

        let response = provider
            .translate(intent, database, collection, context)
            .await
            .map_err(TranslateError::Llm)?;

        // Parse LLM response
        let mql = Self::parse_llm_response(&response)?;

        // Validate
        match &mql {
            CompiledMql::Find { filter, .. } => {
                MqlValidator::validate_filter(filter).map_err(TranslateError::Validation)?;
            }
            CompiledMql::Aggregate { pipeline } => {
                MqlValidator::validate_pipeline(pipeline).map_err(TranslateError::Validation)?;
            }
        }

        // Extract template
        let template = TemplateExtractor::extract(intent);

        // Build compiled query
        let compiled = CompiledQuery {
            hash: hash.clone(),
            intent: intent.to_string(),
            collection: collection.to_string(),
            database: database.to_string(),
            mql,
            template,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        };

        // Store in cache
        self.cache.put(&compiled).await;

        Ok(compiled)
    }

    fn parse_llm_response(response: &str) -> Result<CompiledMql, TranslateError> {
        let value: serde_json::Value = serde_json::from_str(response)
            .map_err(|e| TranslateError::ParseError(format!("Invalid JSON from LLM: {}", e)))?;

        let query_type = value["type"].as_str().unwrap_or("find");

        match query_type {
            "find" => {
                let filter_val = &value["filter"];
                let filter: Document = bson::to_document(filter_val)
                    .map_err(|e| TranslateError::ParseError(format!("Invalid filter: {}", e)))?;
                Ok(CompiledMql::Find {
                    filter,
                    options: None,
                })
            }
            "aggregate" => {
                let pipeline_val = value["pipeline"].as_array().ok_or_else(|| {
                    TranslateError::ParseError("Missing pipeline array".to_string())
                })?;
                let pipeline: Vec<Document> = pipeline_val
                    .iter()
                    .map(|v| {
                        bson::to_document(v).map_err(|e| TranslateError::ParseError(e.to_string()))
                    })
                    .collect::<Result<_, _>>()?;
                Ok(CompiledMql::Aggregate { pipeline })
            }
            other => Err(TranslateError::ParseError(format!(
                "Unknown query type: {}",
                other
            ))),
        }
    }

    /// Warm the cache from Atlas (L3). Call on startup if Atlas sync is enabled.
    pub async fn warm_cache(&self) {
        self.cache.warm_from_atlas().await;
    }

    pub fn cache_size(&self) -> usize {
        self.cache.l1_size()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TranslateError {
    #[error("No LLM provider configured")]
    NoProvider,
    #[error("LLM error: {0}")]
    Llm(LlmError),
    #[error("Failed to parse LLM response: {0}")]
    ParseError(String),
    #[error("MQL validation failed: {0}")]
    Validation(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_find_response() {
        let json = r#"{"type": "find", "filter": {"status": "active", "age": {"$gt": 25}}}"#;
        let result = CompiledQueryTranslator::parse_llm_response(json).unwrap();
        match result {
            CompiledMql::Find { filter, options } => {
                assert_eq!(filter.get_str("status").unwrap(), "active");
                assert!(options.is_none());
            }
            _ => panic!("Expected Find"),
        }
    }

    #[test]
    fn parse_aggregate_response() {
        let json = r#"{"type": "aggregate", "pipeline": [{"$match": {"status": "active"}}, {"$sort": {"name": 1}}]}"#;
        let result = CompiledQueryTranslator::parse_llm_response(json).unwrap();
        match result {
            CompiledMql::Aggregate { pipeline } => {
                assert_eq!(pipeline.len(), 2);
                assert!(pipeline[0].contains_key("$match"));
                assert!(pipeline[1].contains_key("$sort"));
            }
            _ => panic!("Expected Aggregate"),
        }
    }

    #[test]
    fn parse_invalid_json_returns_error() {
        let json = "not valid json at all";
        let result = CompiledQueryTranslator::parse_llm_response(json);
        assert!(result.is_err());
        match result.unwrap_err() {
            TranslateError::ParseError(msg) => assert!(msg.contains("Invalid JSON")),
            other => panic!("Expected ParseError, got {:?}", other),
        }
    }

    #[test]
    fn parse_unknown_type_returns_error() {
        let json = r#"{"type": "delete", "filter": {}}"#;
        let result = CompiledQueryTranslator::parse_llm_response(json);
        assert!(result.is_err());
        match result.unwrap_err() {
            TranslateError::ParseError(msg) => assert!(msg.contains("Unknown query type")),
            other => panic!("Expected ParseError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn translate_no_provider_returns_error() {
        let translator = CompiledQueryTranslator::new(None, None, None);
        let context = TranslationContext::default();
        let result = translator
            .translate("find all users", "mydb", "users", &context)
            .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            TranslateError::NoProvider => {}
            other => panic!("Expected NoProvider, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn translate_cache_hit() {
        let translator = CompiledQueryTranslator::new(None, None, None);

        // Pre-populate cache
        let hash = QueryHasher::hash("find active users", "mydb", "users", None);
        let query = CompiledQuery {
            hash: hash.clone(),
            intent: "find active users".to_string(),
            collection: "users".to_string(),
            database: "mydb".to_string(),
            mql: CompiledMql::Find {
                filter: bson::doc! { "status": "active" },
                options: None,
            },
            template: None,
            created_at: 0,
        };
        translator.cache.put(&query).await;

        let context = TranslationContext::default();
        let result = translator
            .translate("find active users", "mydb", "users", &context)
            .await
            .unwrap();
        assert_eq!(result.hash, hash);
        assert_eq!(result.intent, "find active users");
    }
}
