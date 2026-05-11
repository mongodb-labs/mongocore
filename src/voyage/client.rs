use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct VoyageClient {
    api_key: String,
    http_client: reqwest::Client,
    embed_model: String,
    rerank_model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResult {
    pub embeddings: Vec<Vec<f64>>,
    pub total_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankResult {
    pub rankings: Vec<RerankEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RerankEntry {
    pub index: usize,
    pub relevance_score: f64,
}

#[derive(Debug, thiserror::Error)]
pub enum VoyageError {
    #[error("Voyage AI API error: {0}")]
    ApiError(String),
    #[error("HTTP error: {0}")]
    HttpError(String),
    #[error("Rate limited, retry after {0}s")]
    RateLimited(u64),
    #[error("Not configured: {0}")]
    NotConfigured(String),
}

impl VoyageClient {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            http_client: reqwest::Client::new(),
            embed_model: "voyage-3".to_string(),
            rerank_model: "rerank-2".to_string(),
        }
    }

    pub fn with_models(api_key: String, embed_model: String, rerank_model: String) -> Self {
        Self {
            api_key,
            http_client: reqwest::Client::new(),
            embed_model,
            rerank_model,
        }
    }

    /// Embed one or more texts into vector representations.
    pub async fn embed(&self, texts: Vec<String>) -> Result<EmbeddingResult, VoyageError> {
        let body = serde_json::json!({
            "input": texts,
            "model": self.embed_model,
        });

        let response = self
            .http_client
            .post("https://api.voyageai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| VoyageError::HttpError(e.to_string()))?;

        if response.status() == 429 {
            return Err(VoyageError::RateLimited(60));
        }
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(VoyageError::ApiError(format!("HTTP {}: {}", status, text)));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| VoyageError::ApiError(e.to_string()))?;

        let embeddings: Vec<Vec<f64>> = json["data"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|item| {
                item["embedding"]
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|v| v.as_f64())
                    .collect()
            })
            .collect();

        let total_tokens = json["usage"]["total_tokens"].as_u64().unwrap_or(0);

        Ok(EmbeddingResult {
            embeddings,
            total_tokens,
        })
    }

    /// Rerank documents by relevance to a query.
    pub async fn rerank(
        &self,
        query: &str,
        documents: Vec<String>,
    ) -> Result<RerankResult, VoyageError> {
        let body = serde_json::json!({
            "query": query,
            "documents": documents,
            "model": self.rerank_model,
        });

        let response = self
            .http_client
            .post("https://api.voyageai.com/v1/reranking")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| VoyageError::HttpError(e.to_string()))?;

        if response.status() == 429 {
            return Err(VoyageError::RateLimited(60));
        }
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(VoyageError::ApiError(format!("HTTP {}: {}", status, text)));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| VoyageError::ApiError(e.to_string()))?;

        let rankings: Vec<RerankEntry> = json["data"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter_map(|item| {
                Some(RerankEntry {
                    index: item["index"].as_u64()? as usize,
                    relevance_score: item["relevance_score"].as_f64()?,
                })
            })
            .collect();

        Ok(RerankResult { rankings })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_creates_with_defaults() {
        let client = VoyageClient::new("test-key".to_string());
        assert_eq!(client.api_key, "test-key");
        assert_eq!(client.embed_model, "voyage-3");
        assert_eq!(client.rerank_model, "rerank-2");
    }

    #[test]
    fn test_with_models_uses_custom_models() {
        let client = VoyageClient::with_models(
            "test-key".to_string(),
            "voyage-custom".to_string(),
            "rerank-custom".to_string(),
        );
        assert_eq!(client.api_key, "test-key");
        assert_eq!(client.embed_model, "voyage-custom");
        assert_eq!(client.rerank_model, "rerank-custom");
    }

    #[test]
    fn test_embedding_result_serialization() {
        let result = EmbeddingResult {
            embeddings: vec![vec![0.1, 0.2, 0.3], vec![0.4, 0.5, 0.6]],
            total_tokens: 10,
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: EmbeddingResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.embeddings.len(), 2);
        assert_eq!(deserialized.total_tokens, 10);
        assert_eq!(deserialized.embeddings[0], vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn test_rerank_result_serialization() {
        let result = RerankResult {
            rankings: vec![
                RerankEntry {
                    index: 0,
                    relevance_score: 0.9,
                },
                RerankEntry {
                    index: 1,
                    relevance_score: 0.5,
                },
            ],
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: RerankResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.rankings.len(), 2);
        assert_eq!(deserialized.rankings[0].index, 0);
        assert_eq!(deserialized.rankings[0].relevance_score, 0.9);
        assert_eq!(deserialized.rankings[1].index, 1);
        assert_eq!(deserialized.rankings[1].relevance_score, 0.5);
    }
}
