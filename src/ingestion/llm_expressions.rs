use crate::error::MongoCoreError;

pub struct LlmExpressionConfig {
    pub provider: String,
    pub api_key: String,
    pub max_concurrency: u32,
}

/// Validate that LLM expressions can be used (API key must be configured)
pub fn validate_llm_available(config: &Option<LlmExpressionConfig>) -> Result<(), MongoCoreError> {
    match config {
        Some(_) => Ok(()),
        None => Err(MongoCoreError::IngestionError(
            "LLM expressions require an API key. Set llm_provider and llm_api_key_env in config."
                .to_string(),
        )),
    }
}

/// Classify text values into categories using LLM
pub async fn llm_classify(
    values: &[String],
    categories: &[String],
    _config: &LlmExpressionConfig,
) -> Result<Vec<String>, MongoCoreError> {
    // Stub: returns first category for all values
    // Real implementation would batch-call the LLM provider
    Ok(values
        .iter()
        .map(|_| categories.first().cloned().unwrap_or_default())
        .collect())
}

/// Extract structured data from text using LLM
pub async fn llm_extract(
    values: &[String],
    _schema: &std::collections::HashMap<String, String>,
    _config: &LlmExpressionConfig,
) -> Result<Vec<String>, MongoCoreError> {
    // Stub: returns empty JSON for all values
    Ok(values.iter().map(|_| "{}".to_string()).collect())
}

/// Normalize text values semantically using LLM
pub async fn llm_normalize(
    values: &[String],
    _config: &LlmExpressionConfig,
) -> Result<Vec<String>, MongoCoreError> {
    // Stub: returns values as-is
    Ok(values.to_vec())
}

/// Generate vector embeddings using LLM/Voyage AI
pub async fn llm_embed(
    values: &[String],
    _config: &LlmExpressionConfig,
) -> Result<Vec<Vec<f64>>, MongoCoreError> {
    // Stub: returns zero vectors
    // Real implementation would use existing Voyage AI client
    let dim = 1024;
    Ok(values.iter().map(|_| vec![0.0f64; dim]).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_llm_none_errors() {
        let result = validate_llm_available(&None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("API key"));
    }

    #[test]
    fn test_validate_llm_some_ok() {
        let config = Some(LlmExpressionConfig {
            provider: "anthropic".to_string(),
            api_key: "test-key".to_string(),
            max_concurrency: 4,
        });
        assert!(validate_llm_available(&config).is_ok());
    }

    #[tokio::test]
    async fn test_classify_stub() {
        let config = LlmExpressionConfig {
            provider: "test".to_string(),
            api_key: "key".to_string(),
            max_concurrency: 4,
        };
        let values = vec!["laptop".to_string(), "shirt".to_string()];
        let categories = vec!["electronics".to_string(), "clothing".to_string()];
        let result = llm_classify(&values, &categories, &config).await.unwrap();
        assert_eq!(result.len(), 2);
    }

    #[tokio::test]
    async fn test_normalize_stub() {
        let config = LlmExpressionConfig {
            provider: "test".to_string(),
            api_key: "key".to_string(),
            max_concurrency: 4,
        };
        let values = vec!["IBM Corp".to_string(), "Microsoft".to_string()];
        let result = llm_normalize(&values, &config).await.unwrap();
        assert_eq!(result, values);
    }
}
