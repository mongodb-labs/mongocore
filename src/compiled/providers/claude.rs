use async_trait::async_trait;

use super::{LlmError, LlmProvider, TranslationContext};

pub struct ClaudeProvider {
    api_key: String,
    model: String,
}

impl ClaudeProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "claude-sonnet-4-20250514".to_string(),
        }
    }

    pub fn with_model(api_key: String, model: String) -> Self {
        Self { api_key, model }
    }

    fn build_prompt(
        &self,
        intent: &str,
        database: &str,
        collection: &str,
        context: &TranslationContext,
    ) -> String {
        let mut prompt = format!(
            "Translate this natural language query into a MongoDB query.\n\n\
             Database: {}\nCollection: {}\nIntent: \"{}\"\n\n",
            database, collection, intent
        );
        if let Some(ref schema) = context.schema_hint {
            prompt.push_str(&format!("Schema: {}\n\n", schema));
        }
        if !context.sample_documents.is_empty() {
            prompt.push_str("Sample documents:\n");
            for doc in &context.sample_documents {
                prompt.push_str(&format!("  {}\n", doc));
            }
            prompt.push('\n');
        }
        if !context.available_indexes.is_empty() {
            prompt.push_str("Available indexes:\n");
            for idx in &context.available_indexes {
                prompt.push_str(&format!("  {}\n", idx));
            }
            prompt.push('\n');
        }
        prompt.push_str(
            "Respond with ONLY valid JSON. Either:\n\
             - A filter object for simple queries: {\"type\": \"find\", \"filter\": {...}}\n\
             - A pipeline array for complex queries: {\"type\": \"aggregate\", \"pipeline\": [...]}\n\
             No explanation, no markdown.",
        );
        prompt
    }
}

#[async_trait]
impl LlmProvider for ClaudeProvider {
    async fn translate(
        &self,
        intent: &str,
        database: &str,
        collection: &str,
        context: &TranslationContext,
    ) -> Result<String, LlmError> {
        let prompt = self.build_prompt(intent, database, collection, context);

        let client = reqwest::Client::new();
        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&serde_json::json!({
                "model": self.model,
                "max_tokens": 1024,
                "messages": [{"role": "user", "content": prompt}]
            }))
            .send()
            .await
            .map_err(|e| LlmError::ApiError(e.to_string()))?;

        if response.status() == 429 {
            return Err(LlmError::RateLimited(60));
        }
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(LlmError::ApiError(format!("HTTP {}: {}", status, body)));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| LlmError::InvalidResponse(e.to_string()))?;

        let text = body["content"][0]["text"]
            .as_str()
            .ok_or_else(|| LlmError::InvalidResponse("No text in response".to_string()))?;

        Ok(text.to_string())
    }
}
