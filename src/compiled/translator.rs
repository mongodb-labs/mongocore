use std::path::PathBuf;

use bson::Document;

use crate::connection::pool::ConnectionPool;

use super::cache::CacheHierarchy;
use super::hasher::QueryHasher;
use super::providers::{LlmError, LlmProvider, TranslationContext};
use super::template::TemplateExtractor;
use super::template_registry::TemplateRegistry;
use super::validator::MqlValidator;
use super::{CompiledMql, CompiledQuery, LlmTemplate};

pub struct CompiledQueryTranslator {
    cache: CacheHierarchy,
    provider: Option<Box<dyn LlmProvider>>,
    template_registry: TemplateRegistry,
}

impl CompiledQueryTranslator {
    pub fn new(
        pool: Option<ConnectionPool>,
        provider: Option<Box<dyn LlmProvider>>,
        cache_dir: Option<PathBuf>,
    ) -> Self {
        let cache = CacheHierarchy::new(pool, cache_dir);
        Self {
            cache,
            provider,
            template_registry: TemplateRegistry::new(),
        }
    }

    /// Translate an intent to MQL. Checks cache first, then template registry, then LLM.
    pub async fn translate(
        &self,
        intent: &str,
        database: &str,
        collection: &str,
        context: &TranslationContext,
    ) -> Result<CompiledQuery, TranslateError> {
        let hash = QueryHasher::hash(intent, database, collection, None);

        // 1. Check exact cache
        if let Some(cached) = self.cache.get(&hash).await {
            return Ok(cached);
        }

        // 2. Check template registry
        if let Some(template_match) = self.template_registry.try_match(intent, database, collection) {
            let mql = Self::parse_method_response(&template_match.mql_json, &template_match.method)?;
            // Validate
            self.validate_mql(&mql)?;
            let template = TemplateExtractor::extract(intent);
            let compiled = CompiledQuery {
                hash: hash.clone(),
                intent: intent.to_string(),
                collection: collection.to_string(),
                database: database.to_string(),
                mql,
                template,
                llm_template: None,
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            };
            self.cache.put(&compiled).await;
            return Ok(compiled);
        }

        // 3. Call LLM
        let provider = self.provider.as_ref().ok_or(TranslateError::NoProvider)?;

        let response = provider
            .translate(intent, database, collection, context)
            .await
            .map_err(TranslateError::Llm)?;

        // Parse LLM response (with method and optional template)
        let parsed = Self::parse_llm_response(&response)?;

        // Validate
        self.validate_mql(&parsed.mql)?;

        // Register template if LLM provided one
        if let Some(ref llm_tmpl) = parsed.llm_template {
            self.template_registry.register(llm_tmpl, parsed.mql.method(), database, collection);
        }

        // Extract NL-side template
        let template = TemplateExtractor::extract(intent);

        // Build compiled query
        let compiled = CompiledQuery {
            hash: hash.clone(),
            intent: intent.to_string(),
            collection: collection.to_string(),
            database: database.to_string(),
            mql: parsed.mql,
            template,
            llm_template: parsed.llm_template,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64,
        };

        // Store in cache
        self.cache.put(&compiled).await;

        Ok(compiled)
    }

    fn parse_llm_response(response: &str) -> Result<ParsedLlmResponse, TranslateError> {
        // Strip markdown code fences if present (LLMs sometimes wrap JSON in ```json...```)
        let cleaned = response.trim();
        let cleaned = if cleaned.starts_with("```") {
            let without_start = cleaned
                .strip_prefix("```json")
                .or_else(|| cleaned.strip_prefix("```"))
                .unwrap_or(cleaned);
            without_start
                .strip_suffix("```")
                .unwrap_or(without_start)
                .trim()
        } else {
            cleaned
        };

        let value: serde_json::Value = serde_json::from_str(cleaned)
            .map_err(|e| TranslateError::ParseError(format!("Invalid JSON from LLM: {}", e)))?;

        // Determine method: prefer "method" field, fall back to "type" for backwards compatibility
        let method = if let Some(m) = value.get("method").and_then(|v| v.as_str()) {
            m
        } else {
            // Backwards compatibility: map old "type" to method
            match value.get("type").and_then(|v| v.as_str()) {
                Some("find") => "filter",
                Some("aggregate") => "aggregate",
                _ => "filter", // default
            }
        };

        let mql = Self::parse_method_response(&value, method)?;

        // Parse template if present
        let llm_template = value.get("template").and_then(|t| {
            serde_json::from_value::<LlmTemplate>(t.clone()).ok()
        });

        Ok(ParsedLlmResponse { mql, llm_template })
    }

    fn parse_method_response(value: &serde_json::Value, method: &str) -> Result<CompiledMql, TranslateError> {
        match method {
            "filter" => {
                let filter_val = &value["filter"];
                let filter: Document = bson::to_document(filter_val)
                    .map_err(|e| TranslateError::ParseError(format!("Invalid filter: {}", e)))?;
                let options = value.get("options")
                    .and_then(|o| bson::to_document(o).ok());
                Ok(CompiledMql::Find { filter, options })
            }
            "geo" => {
                let filter_val = &value["filter"];
                let filter: Document = bson::to_document(filter_val)
                    .map_err(|e| TranslateError::ParseError(format!("Invalid filter: {}", e)))?;
                let options = value.get("options")
                    .and_then(|o| bson::to_document(o).ok());
                Ok(CompiledMql::Geo { filter, options })
            }
            "aggregate" => {
                let pipeline_val = value["pipeline"].as_array()
                    .ok_or_else(|| TranslateError::ParseError("Missing pipeline".to_string()))?;
                let pipeline: Vec<Document> = pipeline_val.iter()
                    .map(|v| bson::to_document(v).map_err(|e| TranslateError::ParseError(e.to_string())))
                    .collect::<Result<_, _>>()?;
                Ok(CompiledMql::Aggregate { pipeline })
            }
            "vector_search" => {
                let search_query = value.get("search_query")
                    .or_else(|| value.get("query"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let pre_filter = value.get("pre_filter")
                    .and_then(|f| bson::to_document(f).ok());
                Ok(CompiledMql::VectorSearch { search_query, pre_filter })
            }
            "fulltext" => {
                let search_query = value.get("search_query")
                    .or_else(|| value.get("query"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let pre_filter = value.get("pre_filter")
                    .and_then(|f| bson::to_document(f).ok());
                Ok(CompiledMql::Fulltext { search_query, pre_filter })
            }
            _ => {
                // Default to find for backwards compatibility
                let filter_val = &value["filter"];
                let filter: Document = bson::to_document(filter_val)
                    .unwrap_or_default();
                Ok(CompiledMql::Find { filter, options: None })
            }
        }
    }

    fn validate_mql(&self, mql: &CompiledMql) -> Result<(), TranslateError> {
        match mql {
            CompiledMql::Find { filter, .. } | CompiledMql::Geo { filter, .. } => {
                MqlValidator::validate_filter(filter).map_err(TranslateError::Validation)?;
            }
            CompiledMql::Aggregate { pipeline } => {
                MqlValidator::validate_pipeline(pipeline).map_err(TranslateError::Validation)?;
            }
            CompiledMql::VectorSearch { search_query, .. } | CompiledMql::Fulltext { search_query, .. } => {
                if search_query.is_empty() {
                    return Err(TranslateError::Validation("Empty search query".to_string()));
                }
            }
        }
        Ok(())
    }

    /// Warm the cache from Atlas (L3). Call on startup if Atlas sync is enabled.
    pub async fn warm_cache(&self) {
        self.cache.warm_from_atlas().await;
    }

    pub fn cache_size(&self) -> usize {
        self.cache.l1_size()
    }

    pub fn template_registry_size(&self) -> usize {
        self.template_registry.len()
    }
}

#[derive(Debug)]
struct ParsedLlmResponse {
    mql: CompiledMql,
    llm_template: Option<LlmTemplate>,
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
        let parsed = CompiledQueryTranslator::parse_llm_response(json).unwrap();
        match parsed.mql {
            CompiledMql::Find { filter, options } => {
                assert_eq!(filter.get_str("status").unwrap(), "active");
                assert!(options.is_none());
            }
            _ => panic!("Expected Find"),
        }
        assert!(parsed.llm_template.is_none());
    }

    #[test]
    fn parse_aggregate_response() {
        let json = r#"{"type": "aggregate", "pipeline": [{"$match": {"status": "active"}}, {"$sort": {"name": 1}}]}"#;
        let parsed = CompiledQueryTranslator::parse_llm_response(json).unwrap();
        match parsed.mql {
            CompiledMql::Aggregate { pipeline } => {
                assert_eq!(pipeline.len(), 2);
                assert!(pipeline[0].contains_key("$match"));
                assert!(pipeline[1].contains_key("$sort"));
            }
            _ => panic!("Expected Aggregate"),
        }
        assert!(parsed.llm_template.is_none());
    }

    #[test]
    fn parse_new_format_with_method_and_template() {
        let json = r#"{
            "method": "filter",
            "filter": {"cuisine": "Italian"},
            "template": {
                "intent_pattern": "find {{cuisine}} restaurants",
                "parameters": [{"name": "cuisine", "value": "Italian", "param_type": "String"}],
                "mql_pattern": {"cuisine": "{{cuisine}}"}
            }
        }"#;
        let parsed = CompiledQueryTranslator::parse_llm_response(json).unwrap();
        match parsed.mql {
            CompiledMql::Find { filter, .. } => {
                assert_eq!(filter.get_str("cuisine").unwrap(), "Italian");
            }
            _ => panic!("Expected Find"),
        }
        assert!(parsed.llm_template.is_some());
        let template = parsed.llm_template.unwrap();
        assert_eq!(template.intent_pattern, "find {{cuisine}} restaurants");
        assert_eq!(template.parameters.len(), 1);
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
            llm_template: None,
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
