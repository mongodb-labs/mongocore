use bson::{doc, Document};
use chrono::Utc;
use mongodb::Collection;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::MongoCoreError;
use crate::ingestion::types::{IngestJob, IngestStatus};

pub struct ProgressTracker {
    collection: Collection<Document>,
    jobs: Arc<RwLock<Vec<IngestJob>>>,
}

impl ProgressTracker {
    pub fn new(collection: Collection<Document>) -> Self {
        Self {
            collection,
            jobs: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn create_job(&self, job: IngestJob) -> Result<(), MongoCoreError> {
        let doc = Self::job_to_document(&job);
        self.collection
            .insert_one(doc)
            .await
            .map_err(|e| MongoCoreError::IngestionError(format!("Failed to create job: {e}")))?;
        let mut jobs = self.jobs.write().await;
        jobs.push(job);
        Ok(())
    }

    pub async fn update_total_rows(&self, job_id: &str, total_rows: i64) -> Result<(), MongoCoreError> {
        let filter = doc! { "job_id": job_id };
        let update = doc! { "$set": { "total_rows": total_rows } };
        self.collection.update_one(filter, update).await.map_err(|e| {
            MongoCoreError::IngestionError(format!("Failed to update total_rows: {e}"))
        })?;
        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.job_id == job_id) {
            job.total_rows = total_rows;
        }
        Ok(())
    }

    pub async fn update_progress(
        &self,
        job_id: &str,
        rows_processed: i64,
        rows_inserted: i64,
        rows_skipped: i64,
        rows_failed: i64,
        last_chunk: i64,
    ) -> Result<(), MongoCoreError> {
        let filter = doc! { "job_id": job_id };
        let update = doc! {
            "$set": {
                "rows_processed": rows_processed,
                "rows_inserted": rows_inserted,
                "rows_skipped": rows_skipped,
                "rows_failed": rows_failed,
                "last_committed_chunk": last_chunk,
            }
        };
        self.collection
            .update_one(filter, update)
            .await
            .map_err(|e| {
                MongoCoreError::IngestionError(format!("Failed to update progress: {e}"))
            })?;

        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.job_id == job_id) {
            job.rows_processed = rows_processed;
            job.rows_inserted = rows_inserted;
            job.rows_skipped = rows_skipped;
            job.rows_failed = rows_failed;
            job.last_committed_chunk = last_chunk;
        }
        Ok(())
    }

    pub async fn complete_job(&self, job_id: &str) -> Result<(), MongoCoreError> {
        let now = Utc::now();
        let bson_now = bson::DateTime::from_millis(now.timestamp_millis());
        let filter = doc! { "job_id": job_id };
        let update = doc! {
            "$set": {
                "status": "completed",
                "completed_at": bson_now,
            }
        };
        self.collection
            .update_one(filter, update)
            .await
            .map_err(|e| {
                MongoCoreError::IngestionError(format!("Failed to complete job: {e}"))
            })?;

        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.job_id == job_id) {
            job.status = IngestStatus::Completed;
            job.completed_at = Some(now);
        }
        Ok(())
    }

    pub async fn fail_job(&self, job_id: &str, error: &str) -> Result<(), MongoCoreError> {
        let now = Utc::now();
        let bson_now = bson::DateTime::from_millis(now.timestamp_millis());
        let filter = doc! { "job_id": job_id };
        let update = doc! {
            "$set": {
                "status": "failed",
                "error": error,
                "completed_at": bson_now,
            }
        };
        self.collection
            .update_one(filter, update)
            .await
            .map_err(|e| MongoCoreError::IngestionError(format!("Failed to fail job: {e}")))?;

        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.job_id == job_id) {
            job.status = IngestStatus::Failed;
            job.error = Some(error.to_string());
            job.completed_at = Some(now);
        }
        Ok(())
    }

    pub async fn cancel_job(&self, job_id: &str) -> Result<(), MongoCoreError> {
        let now = Utc::now();
        let bson_now = bson::DateTime::from_millis(now.timestamp_millis());
        let filter = doc! { "job_id": job_id };
        let update = doc! {
            "$set": {
                "status": "cancelled",
                "completed_at": bson_now,
            }
        };
        self.collection
            .update_one(filter, update)
            .await
            .map_err(|e| {
                MongoCoreError::IngestionError(format!("Failed to cancel job: {e}"))
            })?;

        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.job_id == job_id) {
            job.status = IngestStatus::Cancelled;
            job.completed_at = Some(now);
        }
        Ok(())
    }

    pub async fn get_job(&self, job_id: &str) -> Result<Option<IngestJob>, MongoCoreError> {
        let jobs = self.jobs.read().await;
        Ok(jobs.iter().find(|j| j.job_id == job_id).cloned())
    }

    pub async fn list_jobs(&self) -> Result<Vec<IngestJob>, MongoCoreError> {
        let jobs = self.jobs.read().await;
        Ok(jobs.clone())
    }

    pub async fn get_resumable_jobs(&self) -> Result<Vec<IngestJob>, MongoCoreError> {
        let jobs = self.jobs.read().await;
        Ok(jobs
            .iter()
            .filter(|j| j.status == IngestStatus::Running)
            .cloned()
            .collect())
    }

    /// Convert an IngestJob to a BSON document for MongoDB storage.
    pub fn job_to_document(job: &IngestJob) -> Document {
        let status_str = match job.status {
            IngestStatus::Running => "running",
            IngestStatus::Completed => "completed",
            IngestStatus::Failed => "failed",
            IngestStatus::Cancelled => "cancelled",
        };

        let mut doc = doc! {
            "job_id": &job.job_id,
            "file_path": &job.file_path,
            "database": &job.database,
            "collection": &job.collection,
            "status": status_str,
            "total_rows": job.total_rows,
            "rows_processed": job.rows_processed,
            "rows_inserted": job.rows_inserted,
            "rows_skipped": job.rows_skipped,
            "rows_failed": job.rows_failed,
            "last_committed_chunk": job.last_committed_chunk,
            "started_at": bson::DateTime::from_millis(job.started_at.timestamp_millis()),
        };

        if let Some(completed_at) = job.completed_at {
            doc.insert(
                "completed_at",
                bson::DateTime::from_millis(completed_at.timestamp_millis()),
            );
        }

        if let Some(ref error) = job.error {
            doc.insert("error", error);
        }

        doc
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingestion::types::BsonSchema;
    use chrono::Utc;

    #[test]
    fn test_job_to_document() {
        let job = IngestJob {
            job_id: "test-job-1".to_string(),
            file_path: "/data/test.csv".to_string(),
            database: "testdb".to_string(),
            collection: "testcol".to_string(),
            status: IngestStatus::Running,
            total_rows: 1000,
            rows_processed: 0,
            rows_inserted: 0,
            rows_skipped: 0,
            rows_failed: 0,
            last_committed_chunk: 0,
            started_at: Utc::now(),
            completed_at: None,
            error: None,
            inferred_schema: BsonSchema::default(),
        };
        let doc = ProgressTracker::job_to_document(&job);
        assert_eq!(doc.get_str("job_id"), Ok("test-job-1"));
        assert_eq!(doc.get_str("file_path"), Ok("/data/test.csv"));
        assert_eq!(doc.get_str("database"), Ok("testdb"));
        assert_eq!(doc.get_str("collection"), Ok("testcol"));
        assert_eq!(doc.get_str("status"), Ok("running"));
        assert_eq!(doc.get_i64("total_rows"), Ok(1000));
        assert_eq!(doc.get_i64("rows_processed"), Ok(0));
    }
}
