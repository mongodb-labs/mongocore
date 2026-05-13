use std::time::Duration;

use async_trait::async_trait;

use super::{LlmError, LlmProvider, TranslationContext};
use super::prompt::build_translation_prompt;

const SAMPLING_TIMEOUT: Duration = Duration::from_secs(60);

/// A sampling request sent through the channel to the stdio transport.
pub struct SamplingRequest {
    pub prompt: String,
    pub system: Option<String>,
    pub response_tx: tokio::sync::oneshot::Sender<Result<String, LlmError>>,
}

/// LLM provider that delegates to the MCP host via the sampling protocol.
/// Used when no API key is configured but MongoCore is running as an MCP server.
pub struct McpSamplingProvider {
    sender: tokio::sync::mpsc::Sender<SamplingRequest>,
}

impl McpSamplingProvider {
    /// Create a new sampling provider with the given request channel.
    pub fn new(sender: tokio::sync::mpsc::Sender<SamplingRequest>) -> Self {
        Self { sender }
    }
}

#[async_trait]
impl LlmProvider for McpSamplingProvider {
    async fn translate(
        &self,
        intent: &str,
        database: &str,
        collection: &str,
        context: &TranslationContext,
    ) -> Result<String, LlmError> {
        let prompt = build_translation_prompt(intent, database, collection, context);

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let request = SamplingRequest {
            prompt,
            system: Some("You are a MongoDB query translator. Respond with valid JSON only.".to_string()),
            response_tx,
        };

        self.sender.send(request).await
            .map_err(|_| LlmError::ApiError("Sampling channel closed".to_string()))?;

        match tokio::time::timeout(SAMPLING_TIMEOUT, response_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(LlmError::ApiError("Sampling response channel dropped".to_string())),
            Err(_) => Err(LlmError::ApiError("Sampling request timed out after 60s".to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sampling_provider_channel_closed() {
        let (sender, receiver) = tokio::sync::mpsc::channel::<SamplingRequest>(1);
        drop(receiver);
        let provider = McpSamplingProvider::new(sender);
        let context = TranslationContext::default();
        let result = provider.translate("find users", "mydb", "users", &context).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            LlmError::ApiError(msg) => assert!(msg.contains("channel")),
            other => panic!("Expected ApiError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_sampling_provider_receives_response() {
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<SamplingRequest>(1);
        let provider = McpSamplingProvider::new(sender);

        tokio::spawn(async move {
            if let Some(req) = receiver.recv().await {
                assert!(req.prompt.contains("find users"));
                let _ = req.response_tx.send(Ok(
                    r#"{"method":"filter","filter":{"status":"active"}}"#.to_string()
                ));
            }
        });

        let context = TranslationContext::default();
        let result = provider.translate("find users", "mydb", "users", &context).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("filter"));
    }
}
