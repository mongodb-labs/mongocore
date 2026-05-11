use bson::{doc, Document};
use futures::TryStreamExt;
use mongodb::Collection;

use crate::error::MongoCoreError;
use crate::ingestion::types::DeadLetterEntry;

pub struct DeadLetterQueue {
    collection: Collection<Document>,
}

impl DeadLetterQueue {
    pub fn new(collection: Collection<Document>) -> Self {
        Self { collection }
    }

    /// Insert a single failed document entry
    pub async fn push(&self, entry: DeadLetterEntry) -> Result<(), MongoCoreError> {
        let doc = Self::entry_to_document(&entry);
        self.collection
            .insert_one(doc)
            .await
            .map_err(|e| MongoCoreError::IngestionError(format!("DLQ push failed: {e}")))?;
        Ok(())
    }

    /// Insert a batch of failed document entries
    pub async fn push_batch(&self, entries: Vec<DeadLetterEntry>) -> Result<(), MongoCoreError> {
        if entries.is_empty() {
            return Ok(());
        }
        let docs: Vec<Document> = entries.iter().map(Self::entry_to_document).collect();
        self.collection
            .insert_many(docs)
            .await
            .map_err(|e| MongoCoreError::IngestionError(format!("DLQ push_batch failed: {e}")))?;
        Ok(())
    }

    /// Query all DLQ entries for a given job
    pub async fn get_by_job(&self, job_id: &str) -> Result<Vec<DeadLetterEntry>, MongoCoreError> {
        let filter = doc! { "job_id": job_id };
        let mut cursor = self
            .collection
            .find(filter)
            .await
            .map_err(|e| MongoCoreError::IngestionError(format!("DLQ query failed: {e}")))?;

        let mut entries = Vec::new();
        while let Some(doc) = cursor
            .try_next()
            .await
            .map_err(|e| MongoCoreError::IngestionError(format!("DLQ cursor error: {e}")))?
        {
            entries.push(Self::document_to_entry(&doc)?);
        }
        Ok(entries)
    }

    /// Convert a DeadLetterEntry to a BSON Document for storage
    pub fn entry_to_document(entry: &DeadLetterEntry) -> Document {
        doc! {
            "job_id": &entry.job_id,
            "source_row": entry.source_row,
            "document": entry.document.clone(),
            "error": &entry.error,
            "stage": &entry.stage,
            "timestamp": bson::DateTime::from_millis(entry.timestamp.timestamp_millis()),
        }
    }

    /// Convert a stored Document back to a DeadLetterEntry
    fn document_to_entry(doc: &Document) -> Result<DeadLetterEntry, MongoCoreError> {
        let job_id = doc
            .get_str("job_id")
            .unwrap_or_default()
            .to_string();
        let source_row = doc.get_i64("source_row").unwrap_or_default();
        let document = doc
            .get_document("document")
            .cloned()
            .unwrap_or_default();
        let error = doc
            .get_str("error")
            .unwrap_or_default()
            .to_string();
        let stage = doc
            .get_str("stage")
            .unwrap_or_default()
            .to_string();
        let timestamp = doc
            .get_datetime("timestamp")
            .map(|dt| {
                chrono::DateTime::from_timestamp_millis(dt.timestamp_millis())
                    .unwrap_or_else(|| chrono::Utc::now())
            })
            .unwrap_or_else(|_| chrono::Utc::now());

        Ok(DeadLetterEntry {
            job_id,
            source_row,
            document,
            error,
            stage,
            timestamp,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_entry_to_document() {
        let entry = DeadLetterEntry {
            job_id: "job-123".to_string(),
            source_row: 42,
            document: doc! { "name": "test" },
            error: "duplicate key".to_string(),
            stage: "bulk_write".to_string(),
            timestamp: Utc::now(),
        };

        let doc = DeadLetterQueue::entry_to_document(&entry);
        assert_eq!(doc.get_str("job_id"), Ok("job-123"));
        assert_eq!(doc.get_i64("source_row"), Ok(42));
        assert_eq!(doc.get_str("error"), Ok("duplicate key"));
        assert_eq!(doc.get_str("stage"), Ok("bulk_write"));
        assert!(doc.get("document").is_some());
        assert!(doc.get("timestamp").is_some());
    }
}
