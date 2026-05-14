use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use futures::FutureExt;
use mongodb::Client;
use polars::prelude::*;
use tokio::sync::{broadcast, mpsc, RwLock};

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
    #[tracing::instrument(skip(self))]
    pub async fn ingest(
        &self,
        client: &Client,
        options: IngestOptions,
    ) -> Result<IngestJob, MongoCoreError> {
        let job_id = uuid::Uuid::new_v4().to_string();
        let source = &options.file_path;

        // Validate file exists (only for local paths)
        if !source.starts_with("http://")
            && !source.starts_with("https://")
            && !source.starts_with("s3://")
            && !source.starts_with("gs://")
            && !source.starts_with("az://")
        {
            let path = std::path::Path::new(source);
            if !path.exists() {
                return Err(MongoCoreError::IngestionError(format!(
                    "File not found: {}",
                    source
                )));
            }
        }

        // Detect format
        let format = match options.format {
            FileFormat::Auto => reader::detect_format(std::path::Path::new(source))?,
            other => other,
        };

        // Read LazyFrame (no separate count_rows pass — we get total after collect)
        let lf = reader::read_lazy_from_source(source, format, &options.csv_options)?;

        // Apply transforms before schema inference (so schema reflects final shape)
        let transformed_lf = if options.expressions.is_empty() {
            lf
        } else {
            transform::apply_expressions(lf, &options.expressions)?
        };

        // Sample for schema inference (from transformed LazyFrame)
        let sample_df = transformed_lf
            .clone()
            .limit(options.sample_size)
            .collect()
            .map_err(|e| MongoCoreError::IngestionError(format!("Sample failed: {}", e)))?;

        // Infer schema
        let mut inferred_schema = schema::infer_schema(&sample_df)?;
        if !options.schema_overrides.is_empty() {
            schema::apply_overrides(&mut inferred_schema, &options.schema_overrides);
        }

        // Create job record (total_rows updated after collect)
        let job = IngestJob {
            job_id: job_id.clone(),
            file_path: options.file_path.clone(),
            database: options.database.clone(),
            collection: options.collection.clone(),
            status: IngestStatus::Running,
            total_rows: 0,
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
        let concurrency = options.concurrency.max(1) as usize;
        let dedup_key = options.dedup_key.clone();
        let conflict_strategy = options.conflict_strategy;
        let cancel_channels = self.cancel_channels.clone();

        tokio::spawn(async move {
            let mut cancel_rx = cancel_tx.subscribe();
            let result = std::panic::AssertUnwindSafe(Self::run_ingestion(
                transformed_lf,
                &inferred_schema,
                target_collection,
                &dlq,
                &progress,
                &job_id,
                batch_size,
                concurrency,
                dedup_key,
                conflict_strategy,
                &mut cancel_rx,
            ))
            .catch_unwind()
            .await;

            match result {
                Ok(Ok(())) => {
                    let _ = progress.complete_job(&job_id).await;
                }
                Ok(Err(e)) => {
                    let _ = progress.fail_job(&job_id, &e.to_string()).await;
                }
                Err(panic_err) => {
                    let msg = if let Some(s) = panic_err.downcast_ref::<String>() {
                        format!("Internal error (panic): {}", s)
                    } else if let Some(s) = panic_err.downcast_ref::<&str>() {
                        format!("Internal error (panic): {}", s)
                    } else {
                        "Internal error (panic): unknown".to_string()
                    };
                    let _ = progress.fail_job(&job_id, &msg).await;
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
        concurrency: usize,
        dedup_key: Vec<String>,
        conflict_strategy: ConflictStrategy,
        cancel_rx: &mut broadcast::Receiver<()>,
    ) -> Result<(), MongoCoreError> {
        // Collect DataFrame (transforms applied lazily by Polars)
        let df = lf
            .collect()
            .map_err(|e| MongoCoreError::IngestionError(format!("Collect failed: {}", e)))?;
        let total_rows = df.height();

        // Update total_rows now that we know it
        let _ = progress.update_total_rows(job_id, total_rows as i64).await;

        let has_dedup = !dedup_key.is_empty();

        if has_dedup {
            // Dedup path: sequential writes (must check existing data per batch)
            Self::run_sequential_writes(
                &df, schema, &collection, dlq, progress, job_id,
                batch_size, dedup_key, conflict_strategy, cancel_rx,
            ).await
        } else {
            // No dedup: concurrent writes for maximum throughput
            Self::run_concurrent_writes(
                &df, schema, &collection, dlq, progress, job_id,
                batch_size, concurrency, cancel_rx,
            ).await
        }
    }

    async fn run_concurrent_writes(
        df: &DataFrame,
        schema: &BsonSchema,
        collection: &mongodb::Collection<bson::Document>,
        dlq: &DeadLetterQueue,
        progress: &ProgressTracker,
        job_id: &str,
        batch_size: u32,
        concurrency: usize,
        cancel_rx: &mut broadcast::Receiver<()>,
    ) -> Result<(), MongoCoreError> {
        let total_rows = df.height();
        let rows_inserted = Arc::new(AtomicI64::new(0));
        let rows_failed = Arc::new(AtomicI64::new(0));

        // Channel for sending document batches to writer tasks
        let (tx, rx) = mpsc::channel::<(Vec<bson::Document>, usize)>(concurrency * 2);
        let rx = Arc::new(tokio::sync::Mutex::new(rx));

        // Spawn writer tasks
        let mut writer_handles = Vec::with_capacity(concurrency);
        for _ in 0..concurrency {
            let rx = rx.clone();
            let collection = collection.clone();
            let dlq_coll = dlq.collection().clone();
            let job_id_owned = job_id.to_string();
            let inserted = rows_inserted.clone();
            let failed = rows_failed.clone();

            writer_handles.push(tokio::spawn(async move {
                loop {
                    let batch = {
                        let mut guard = rx.lock().await;
                        guard.recv().await
                    };
                    let Some((docs, offset)) = batch else { break };

                    let doc_count = docs.len() as i64;
                    match collection.insert_many(&docs).await {
                        Ok(r) => {
                            inserted.fetch_add(r.inserted_ids.len() as i64, Ordering::Relaxed);
                        }
                        Err(e) => {
                            // Send failed docs to DLQ
                            let dlq = DeadLetterQueue::new(dlq_coll.clone());
                            for (i, doc) in docs.into_iter().enumerate() {
                                let entry = DeadLetterEntry {
                                    job_id: job_id_owned.clone(),
                                    source_row: (offset + i) as i64,
                                    document: doc,
                                    error: format!("Insert error: {}", e),
                                    stage: "write".to_string(),
                                    timestamp: chrono::Utc::now(),
                                };
                                let _ = dlq.push(entry).await;
                            }
                            failed.fetch_add(doc_count, Ordering::Relaxed);
                        }
                    }
                }
            }));
        }

        // Producer: convert chunks to BSON and send to writers
        let mut offset = 0usize;
        let mut chunk_num: i64 = 0;
        while offset < total_rows {
            if cancel_rx.try_recv().is_ok() {
                drop(tx);
                for h in writer_handles {
                    let _ = h.await;
                }
                return Err(MongoCoreError::IngestionError("Job cancelled".to_string()));
            }

            let end = (offset + batch_size as usize).min(total_rows);
            let chunk_df = df.slice(offset as i64, end - offset);

            match writer::dataframe_to_documents(&chunk_df, schema) {
                Ok(docs) => {
                    if tx.send((docs, offset)).await.is_err() {
                        break;
                    }
                }
                Err(e) => {
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
                    rows_failed.fetch_add((end - offset) as i64, Ordering::Relaxed);
                }
            }

            chunk_num += 1;
            offset = end;

            // Periodic progress update (every 4 chunks to reduce overhead)
            if chunk_num % 4 == 0 {
                let _ = progress.update_progress(
                    job_id,
                    offset as i64,
                    rows_inserted.load(Ordering::Relaxed),
                    0,
                    rows_failed.load(Ordering::Relaxed),
                    chunk_num,
                ).await;
            }
        }

        // Close channel and wait for writers to finish
        drop(tx);
        for h in writer_handles {
            let _ = h.await;
        }

        // Final progress update
        let _ = progress.update_progress(
            job_id,
            total_rows as i64,
            rows_inserted.load(Ordering::Relaxed),
            0,
            rows_failed.load(Ordering::Relaxed),
            chunk_num,
        ).await;

        Ok(())
    }

    async fn run_sequential_writes(
        df: &DataFrame,
        schema: &BsonSchema,
        collection: &mongodb::Collection<bson::Document>,
        dlq: &DeadLetterQueue,
        progress: &ProgressTracker,
        job_id: &str,
        batch_size: u32,
        dedup_key: Vec<String>,
        conflict_strategy: ConflictStrategy,
        cancel_rx: &mut broadcast::Receiver<()>,
    ) -> Result<(), MongoCoreError> {
        let total_rows = df.height();
        let dedup_checker = DedupChecker::new(collection.clone(), dedup_key, conflict_strategy);

        let mut rows_processed: i64 = 0;
        let mut rows_inserted: i64 = 0;
        let mut rows_skipped: i64 = 0;
        let mut rows_failed: i64 = 0;
        let mut chunk_num: i64 = 0;

        let mut offset = 0usize;
        while offset < total_rows {
            if cancel_rx.try_recv().is_ok() {
                return Err(MongoCoreError::IngestionError("Job cancelled".to_string()));
            }

            let end = (offset + batch_size as usize).min(total_rows);
            let chunk_df = df.slice(offset as i64, end - offset);

            let docs = match writer::dataframe_to_documents(&chunk_df, schema) {
                Ok(d) => d,
                Err(e) => {
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

            let results = dedup_checker.check_batch(&docs).await?;
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
                let filter = dedup_checker.build_dedup_filter(replacement).unwrap_or_default();
                match collection.replace_one(filter, replacement).await {
                    Ok(_) => rows_inserted += 1,
                    Err(_) => rows_failed += 1,
                }
            }

            rows_processed += (end - offset) as i64;
            chunk_num += 1;
            offset = end;

            let _ = progress.update_progress(
                job_id, rows_processed, rows_inserted, rows_skipped, rows_failed, chunk_num,
            ).await;
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
