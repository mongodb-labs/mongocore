pub mod claude;
pub mod openai;

use async_trait::async_trait;

#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Translate a natural language intent into MQL.
    /// Returns the raw MQL response as a JSON string (either a filter doc or pipeline array).
    async fn translate(
        &self,
        intent: &str,
        database: &str,
        collection: &str,
        context: &TranslationContext,
    ) -> Result<String, LlmError>;
}

/// Context provided to the LLM for better translations.
#[derive(Debug, Clone, Default)]
pub struct TranslationContext {
    pub sample_documents: Vec<String>,
    pub available_indexes: Vec<String>,
    pub schema_hint: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum LlmError {
    #[error("LLM API error: {0}")]
    ApiError(String),
    #[error("Invalid response from LLM: {0}")]
    InvalidResponse(String),
    #[error("No LLM provider configured")]
    NotConfigured,
    #[error("Rate limited, retry after {0}s")]
    RateLimited(u64),
}
