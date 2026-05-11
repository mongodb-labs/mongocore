use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

use super::client::{VoyageClient, VoyageError};

const BATCH_WINDOW: Duration = Duration::from_millis(10);
const MAX_BATCH_SIZE: usize = 100;

/// A batching layer over VoyageClient that groups embed requests.
pub struct BatchEmbedder {
    client: Arc<VoyageClient>,
    pending: Arc<Mutex<Vec<PendingEmbed>>>,
}

struct PendingEmbed {
    text: String,
    sender: tokio::sync::oneshot::Sender<Result<Vec<f64>, VoyageError>>,
}

impl BatchEmbedder {
    pub fn new(client: Arc<VoyageClient>) -> Self {
        Self {
            client,
            pending: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Embed a single text. May be batched with other concurrent calls.
    pub async fn embed_one(&self, text: String) -> Result<Vec<f64>, VoyageError> {
        let (sender, receiver) = tokio::sync::oneshot::channel();

        {
            let mut pending = self.pending.lock().await;
            pending.push(PendingEmbed { text, sender });

            if pending.len() >= MAX_BATCH_SIZE {
                let batch = std::mem::take(&mut *pending);
                drop(pending);
                self.flush_batch(batch).await;
                return receiver
                    .await
                    .map_err(|_| VoyageError::ApiError("Batch cancelled".to_string()))?;
            }
        }

        // Wait for batch window
        sleep(BATCH_WINDOW).await;

        // Try to flush
        let batch = {
            let mut pending = self.pending.lock().await;
            if pending.is_empty() {
                Vec::new()
            } else {
                std::mem::take(&mut *pending)
            }
        };

        if !batch.is_empty() {
            self.flush_batch(batch).await;
        }

        receiver
            .await
            .map_err(|_| VoyageError::ApiError("Batch cancelled".to_string()))?
    }

    async fn flush_batch(&self, batch: Vec<PendingEmbed>) {
        let texts: Vec<String> = batch.iter().map(|p| p.text.clone()).collect();
        let result = self.client.embed(texts).await;

        match result {
            Ok(embedding_result) => {
                for (i, pending) in batch.into_iter().enumerate() {
                    let embedding = embedding_result
                        .embeddings
                        .get(i)
                        .cloned()
                        .unwrap_or_default();
                    let _ = pending.sender.send(Ok(embedding));
                }
            }
            Err(e) => {
                let err_msg = e.to_string();
                for pending in batch {
                    let _ = pending
                        .sender
                        .send(Err(VoyageError::ApiError(err_msg.clone())));
                }
            }
        }
    }
}
