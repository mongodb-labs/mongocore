use async_trait::async_trait;

use super::{LlmError, LlmProvider, TranslationContext};

#[derive(Debug, Clone)]
pub struct GatewayConfig {
    pub base_url: String,
    pub api_key: String,
    pub auth_header: String,
    pub model: String,
    pub provider_type: String,
}

pub struct GatewayProvider {
    config: GatewayConfig,
}

impl GatewayProvider {
    pub fn new(config: GatewayConfig) -> Self {
        Self { config }
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

    fn build_anthropic_body(&self, prompt: &str) -> serde_json::Value {
        serde_json::json!({
            "model": self.config.model,
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": prompt}]
        })
    }

    fn build_openai_body(&self, prompt: &str) -> serde_json::Value {
        serde_json::json!({
            "model": self.config.model,
            "max_tokens": 1024,
            "messages": [
                {"role": "system", "content": "You are a MongoDB query translator. Output only valid JSON."},
                {"role": "user", "content": prompt}
            ]
        })
    }

    fn extract_anthropic_text(body: &serde_json::Value) -> Result<String, LlmError> {
        body["content"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| LlmError::InvalidResponse("No text in Anthropic response".to_string()))
    }

    fn extract_openai_text(body: &serde_json::Value) -> Result<String, LlmError> {
        body["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| LlmError::InvalidResponse("No content in OpenAI response".to_string()))
    }
}

#[async_trait]
impl LlmProvider for GatewayProvider {
    async fn translate(
        &self,
        intent: &str,
        database: &str,
        collection: &str,
        context: &TranslationContext,
    ) -> Result<String, LlmError> {
        let prompt = self.build_prompt(intent, database, collection, context);

        let request_body = match self.config.provider_type.as_str() {
            "openai" => self.build_openai_body(&prompt),
            _ => self.build_anthropic_body(&prompt),
        };

        let client = reqwest::Client::new();
        let mut request = client
            .post(&self.config.base_url)
            .header("content-type", "application/json")
            .header(&self.config.auth_header, &self.config.api_key)
            .json(&request_body);

        // Add anthropic-version header for Anthropic format
        if self.config.provider_type != "openai" {
            request = request.header("anthropic-version", "2023-06-01");
        }

        let response = request
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

        match self.config.provider_type.as_str() {
            "openai" => Self::extract_openai_text(&body),
            _ => Self::extract_anthropic_text(&body),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gateway_config_defaults() {
        let config = GatewayConfig {
            base_url: "https://gateway.example.com/v1/messages".to_string(),
            api_key: "test-key".to_string(),
            auth_header: "api-key".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            provider_type: "anthropic".to_string(),
        };
        let provider = GatewayProvider::new(config.clone());
        assert_eq!(provider.config.base_url, "https://gateway.example.com/v1/messages");
        assert_eq!(provider.config.auth_header, "api-key");
    }

    #[test]
    fn test_build_anthropic_body() {
        let config = GatewayConfig {
            base_url: "https://gw.example.com/anthropic/v1/messages".to_string(),
            api_key: "key".to_string(),
            auth_header: "api-key".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            provider_type: "anthropic".to_string(),
        };
        let provider = GatewayProvider::new(config);
        let body = provider.build_anthropic_body("test prompt");
        assert_eq!(body["model"], "claude-sonnet-4-6");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "test prompt");
    }

    #[test]
    fn test_build_openai_body() {
        let config = GatewayConfig {
            base_url: "https://gw.example.com/openai/v1/chat/completions".to_string(),
            api_key: "key".to_string(),
            auth_header: "api-key".to_string(),
            model: "gpt-5.1".to_string(),
            provider_type: "openai".to_string(),
        };
        let provider = GatewayProvider::new(config);
        let body = provider.build_openai_body("test prompt");
        assert_eq!(body["model"], "gpt-5.1");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "test prompt");
    }

    #[test]
    fn test_extract_anthropic_text() {
        let body = serde_json::json!({
            "content": [{"type": "text", "text": "{\"type\": \"find\", \"filter\": {}}"}]
        });
        let result = GatewayProvider::extract_anthropic_text(&body).unwrap();
        assert_eq!(result, "{\"type\": \"find\", \"filter\": {}}");
    }

    #[test]
    fn test_extract_openai_text() {
        let body = serde_json::json!({
            "choices": [{"message": {"content": "{\"type\": \"find\", \"filter\": {}}"}}]
        });
        let result = GatewayProvider::extract_openai_text(&body).unwrap();
        assert_eq!(result, "{\"type\": \"find\", \"filter\": {}}");
    }

    #[test]
    fn test_extract_anthropic_text_missing() {
        let body = serde_json::json!({"content": []});
        let result = GatewayProvider::extract_anthropic_text(&body);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_openai_text_missing() {
        let body = serde_json::json!({"choices": []});
        let result = GatewayProvider::extract_openai_text(&body);
        assert!(result.is_err());
    }
}
