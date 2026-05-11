use std::collections::HashMap;
use std::sync::Arc;

use mongodb::Client;
use polars::prelude::*;
use tokio::sync::{broadcast, RwLock};

use crate::error::MongoCoreError;
use crate::ingestion::dedup::{DedupChecker, DedupResult};
use crate::ingestion::dlq::DeadLetterQueue;
use crate::ingestion::progress::ProgressTracker;
use crate::ingestion::types::*;
use crate::ingestion::{reader, schema, transform, writer};

/// The ingestion engine orchestrates file-to-MongoDB ingestion jobs.
///
/// It manages background ingestion tasks with progress tracking, deduplication,
/// dead letter queue support, and cancellation.
pub struct IngestionEngine {
    db: mongodb::Database,
    progress: Arc<ProgressTracker>,
    cancel_channels: Arc<RwLock<HashMap<String, broadcast::Sender<()>>>>,
}

impl IngestionEngine {
    pub fn new(client: &Client, system_db_name: &str) -> Self {
        let db = client.database(system_db_name);
        let progress = Arc::new(ProgressTracker::new(
            db.collection("__mongocore.ingestion_jobs"),
        ));
        Self {
            db,
            progress,
            cancel_channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start an ingestion job. Returns immediately with job metadata; processing happens in background.
    pub async fn ingest(
        &self,
        client: &Client,
        options: IngestOptions,
    ) -> Result<IngestJob, MongoCoreError> {
        let job_id = uuid::Uuid::new_v4().to_string();
        let path = std::path::Path::new(&options.file_path);

        // Validate file exists
        if !path.exists() {
            return Err(MongoCoreError::IngestionError(format!(
                "File not found: {}",
                options.file_path
            )));
        }

        // Detect format
        let format = match options.format {
            FileFormat::Auto => reader::detect_format(path)?,
            other => other,
        };

        // Count total rows
        let total_rows = reader::count_rows(path, format.clone(), &options.csv_options)? as i64;

        // Read LazyFrame
        let lf = reader::read_lazy(path, format, &options.csv_options)?;

        // Sample for schema inference
        let sample_df = lf
            .clone()
            .limit(options.sample_size)
            .collect()
            .map_err(|e| MongoCoreError::IngestionError(format!("Sample failed: {}", e)))?;

        // Infer schema
        let mut inferred_schema = schema::infer_schema(&sample_df)?;
        if !options.schema_overrides.is_empty() {
            schema::apply_overrides(&mut inferred_schema, &options.schema_overrides);
        }

        // Apply transforms
        let transformed_lf = if options.expressions.is_empty() {
            lf
        } else {
            transform::apply_expressions(lf, &options.expressions)?
        };

        // Create job record
        let job = IngestJob {
            job_id: job_id.clone(),
            file_path: options.file_path.clone(),
            database: options.database.clone(),
            collection: options.collection.clone(),
            status: IngestStatus::Running,
            total_rows,
            rows_processed: 0,
            rows_inserted: 0,
            rows_skipped: 0,
            rows_failed: 0,
            last_committed_chunk: 0,
            started_at: chrono::Utc::now(),
            completed_at: None,
            error: None,
            inferred_schema: inferred_schema.clone(),
        };
        self.progress.create_job(job.clone()).await?;

        // Setup cancel channel
        let (cancel_tx, _) = broadcast::channel(1);
        self.cancel_channels
            .write()
            .await
            .insert(job_id.clone(), cancel_tx.clone());

        // Spawn background ingestion task
        let target_db = client.database(&options.database);
        let target_collection = target_db.collection::<bson::Document>(&options.collection);
        let dlq = DeadLetterQueue::new(self.db.collection("__mongocore.dead_letter"));
        let progress = self.progress.clone();
        let batch_size = options.batch_size;
        let dedup_key = options.dedup_key.clone();
        let conflict_strategy = options.conflict_strategy;
        let cancel_channels = self.cancel_channels.clone();

        tokio::spawn(async move {
            let mut cancel_rx = cancel_tx.subscribe();
            let result = Self::run_ingestion(
                transformed_lf,
                &inferred_schema,
                target_collection,
                &dlq,
                &progress,
                &job_id,
                batch_size,
                dedup_key,
                conflict_strategy,
                &mut cancel_rx,
            )
            .await;

            match result {
                Ok(()) => {
                    let _ = progress.complete_job(&job_id).await;
                }
                Err(e) => {
                    let _ = progress.fail_job(&job_id, &e.to_string()).await;
                }
            }
            cancel_channels.write().await.remove(&job_id);
        });

        Ok(job)
    }

    async fn run_ingestion(
        lf: LazyFrame,
        schema: &BsonSchema,
        collection: mongodb::Collection<bson::Document>,
        dlq: &DeadLetterQueue,
        progress: &ProgressTracker,
        job_id: &str,
        batch_size: u32,
        dedup_key: Vec<String>,
        conflict_strategy: ConflictStrategy,
        cancel_rx: &mut broadcast::Receiver<()>,
    ) -> Result<(), MongoCoreError> {
        // Collect full DataFrame
        let df = lf
            .collect()
            .map_err(|e| MongoCoreError::IngestionError(format!("Collect failed: {}", e)))?;
        let total_rows = df.height();

        let mut rows_processed: i64 = 0;
        let mut rows_inserted: i64 = 0;
        let mut rows_skipped: i64 = 0;
        let mut rows_failed: i64 = 0;
        let mut chunk_num: i64 = 0;

        let has_dedup = !dedup_key.is_empty();
        let dedup_checker = if has_dedup {
            Some(DedupChecker::new(
                collection.clone(),
                dedup_key,
                conflict_strategy,
            ))
        } else {
            None
        };

        let mut offset = 0usize;
        while offset < total_rows {
            // Check cancellation
            if cancel_rx.try_recv().is_ok() {
                return Err(MongoCoreError::IngestionError(
                    "Job cancelled".to_string(),
                ));
            }

            let end = (offset + batch_size as usize).min(total_rows);
            let chunk_df = df.slice(offset as i64, end - offset);

            // Convert to documents
            let docs = match writer::dataframe_to_documents(&chunk_df, schema) {
                Ok(d) => d,
                Err(e) => {
                    // Whole chunk failed - send to DLQ
                    for i in offset..end {
                        let entry = DeadLetterEntry {
                            job_id: job_id.to_string(),
                            source_row: i as i64,
                            document: bson::Document::new(),
                            error: format!("Conversion error: {}", e),
                            stage: "conversion".to_string(),
                            timestamp: chrono::Utc::now(),
                        };
                        let _ = dlq.push(entry).await;
                    }
                    rows_failed += (end - offset) as i64;
                    rows_processed += (end - offset) as i64;
                    offset = end;
                    chunk_num += 1;
                    continue;
                }
            };

            // Dedup + write
            if let Some(ref checker) = dedup_checker {
                let results = checker.check_batch(&docs).await?;
                let mut to_insert = Vec::new();
                let mut to_replace = Vec::new();

                for result in results.into_iter() {
                    match result {
                        DedupResult::Insert(d) => to_insert.push(d),
                        DedupResult::Skip => rows_skipped += 1,
                        DedupResult::Replace(d) => to_replace.push(d),
                        DedupResult::Merge(incoming, existing) => {
                            to_replace.push(DedupChecker::merge_documents(&incoming, &existing));
                        }
                    }
                }

                if !to_insert.is_empty() {
                    let insert_count = to_insert.len() as i64;
                    match collection.insert_many(&to_insert).await {
                        Ok(r) => rows_inserted += r.inserted_ids.len() as i64,
                        Err(e) => {
                            // Send failed inserts to DLQ
                            for (i, doc) in to_insert.into_iter().enumerate() {
                                let entry = DeadLetterEntry {
                                    job_id: job_id.to_string(),
                                    source_row: (offset + i) as i64,
                                    document: doc,
                                    error: format!("Insert error: {}", e),
                                    stage: "write".to_string(),
                                    timestamp: chrono::Utc::now(),
                                };
                                let _ = dlq.push(entry).await;
                            }
                            rows_failed += insert_count;
                        }
                    }
                }
                for replacement in &to_replace {
                    let filter = checker
                        .build_dedup_filter(replacement)
                        .unwrap_or_default();
                    match collection.replace_one(filter, replacement).await {
                        Ok(_) => rows_inserted += 1,
                        Err(_) => rows_failed += 1,
                    }
                }
            } else {
                // No dedup — straight insert
                let doc_count = docs.len() as i64;
                match collection.insert_many(&docs).await {
                    Ok(r) => rows_inserted += r.inserted_ids.len() as i64,
                    Err(e) => {
                        // Send failed docs to DLQ
                        for (i, doc) in docs.into_iter().enumerate() {
                            let entry = DeadLetterEntry {
                                job_id: job_id.to_string(),
                                source_row: (offset + i) as i64,
                                document: doc,
                                error: format!("Insert error: {}", e),
                                stage: "write".to_string(),
                                timestamp: chrono::Utc::now(),
                            };
                            let _ = dlq.push(entry).await;
                        }
                        rows_failed += doc_count;
                    }
                }
            }

            rows_processed += (end - offset) as i64;
            chunk_num += 1;
            offset = end;

            let _ = progress
                .update_progress(
                    job_id,
                    rows_processed,
                    rows_inserted,
                    rows_skipped,
                    rows_failed,
                    chunk_num,
                )
                .await;
        }

        Ok(())
    }

    /// Get the status of a specific ingestion job.
    pub async fn get_status(&self, job_id: &str) -> Result<Option<IngestJob>, MongoCoreError> {
        self.progress.get_job(job_id).await
    }

    /// List all ingestion jobs.
    pub async fn list_jobs(&self) -> Result<Vec<IngestJob>, MongoCoreError> {
        self.progress.list_jobs().await
    }

    /// Cancel a running ingestion job.
    pub async fn cancel(&self, job_id: &str) -> Result<(), MongoCoreError> {
        let channels = self.cancel_channels.read().await;
        if let Some(tx) = channels.get(job_id) {
            let _ = tx.send(());
            Ok(())
        } else {
            Err(MongoCoreError::IngestionError(format!(
                "Job '{}' not found or already finished",
                job_id
            )))
        }
    }
}
