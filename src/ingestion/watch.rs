use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use glob::Pattern;
use mongodb::Client;
use notify::{EventKind, RecursiveMode, Watcher};
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::time::{Duration, Instant};

use crate::error::MongoCoreError;
use crate::ingestion::engine::IngestionEngine;
use crate::ingestion::types::*;

/// Configuration for a directory watch.
pub struct WatchConfig {
    pub path: PathBuf,
    pub file_pattern: String,
    pub database: String,
    pub collection: String,
    pub conflict_strategy: ConflictStrategy,
    pub dedup_key: Vec<String>,
    pub debounce_ms: u64,
}

struct WatchHandle {
    id: String,
    #[allow(dead_code)]
    config: WatchConfig,
    cancel_tx: broadcast::Sender<()>,
}

/// Watches directories for new/modified files and auto-triggers ingestion.
pub struct DirectoryWatcher {
    engine: Arc<IngestionEngine>,
    client: Client,
    watches: Arc<RwLock<Vec<WatchHandle>>>,
}

impl DirectoryWatcher {
    pub fn new(engine: Arc<IngestionEngine>, client: Client) -> Self {
        Self {
            engine,
            client,
            watches: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Start watching a directory. Returns a watch ID that can be used to stop the watch.
    pub async fn start_watch(&self, config: WatchConfig) -> Result<String, MongoCoreError> {
        // Validate path exists
        if !config.path.exists() {
            return Err(MongoCoreError::ValidationError(format!(
                "Watch path does not exist: {}",
                config.path.display()
            )));
        }
        if !config.path.is_dir() {
            return Err(MongoCoreError::ValidationError(format!(
                "Watch path is not a directory: {}",
                config.path.display()
            )));
        }

        // Validate pattern
        let pattern = Pattern::new(&config.file_pattern).map_err(|e| {
            MongoCoreError::ValidationError(format!("Invalid file pattern: {e}"))
        })?;

        let watch_id = uuid::Uuid::new_v4().to_string();
        let (cancel_tx, mut cancel_rx) = broadcast::channel::<()>(1);

        // Channel for sending file events from notify callback to tokio task
        let (event_tx, mut event_rx) = mpsc::channel::<PathBuf>(256);

        let watch_path = config.path.clone();
        let debounce_duration = Duration::from_millis(config.debounce_ms);
        let database = config.database.clone();
        let collection = config.collection.clone();
        let conflict_strategy = config.conflict_strategy.clone();
        let dedup_key = config.dedup_key.clone();
        let engine = self.engine.clone();
        let client = self.client.clone();

        // Spawn the watcher task
        tokio::spawn(async move {
            // Create notify watcher - must be kept alive in this scope
            let tx = event_tx.clone();
            let _watcher = {
                let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                    if let Ok(event) = res {
                        match event.kind {
                            EventKind::Create(_) | EventKind::Modify(_) => {
                                for path in event.paths {
                                    let _ = tx.blocking_send(path);
                                }
                            }
                            _ => {}
                        }
                    }
                }).expect("Failed to create file watcher");

                watcher.watch(&watch_path, RecursiveMode::NonRecursive)
                    .expect("Failed to start watching directory");

                watcher
            };

            // Debounce tracking: path -> last event time
            let mut pending: HashMap<PathBuf, Instant> = HashMap::new();
            let tick_interval = Duration::from_millis(500);

            loop {
                tokio::select! {
                    _ = cancel_rx.recv() => {
                        break;
                    }
                    Some(path) = event_rx.recv() => {
                        // Check if file matches the pattern
                        if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                            if pattern.matches(filename) {
                                pending.insert(path, Instant::now());
                            }
                        }
                    }
                    _ = tokio::time::sleep(tick_interval) => {
                        // Check for stable files ready for ingestion
                        let now = Instant::now();
                        let ready: Vec<PathBuf> = pending.iter()
                            .filter(|(_, last_event)| now.duration_since(**last_event) >= debounce_duration)
                            .map(|(path, _)| path.clone())
                            .collect();

                        for path in ready {
                            pending.remove(&path);

                            let options = IngestOptions {
                                file_path: path.to_string_lossy().to_string(),
                                database: database.clone(),
                                collection: collection.clone(),
                                conflict_strategy: conflict_strategy.clone(),
                                dedup_key: dedup_key.clone(),
                                ..Default::default()
                            };

                            let engine = engine.clone();
                            let client = client.clone();
                            tokio::spawn(async move {
                                match engine.ingest(&client, options).await {
                                    Ok(job) => {
                                        tracing::info!(
                                            job_id = %job.job_id,
                                            file = %job.file_path,
                                            "Auto-ingestion triggered by watcher"
                                        );
                                    }
                                    Err(e) => {
                                        tracing::error!(
                                            error = %e,
                                            "Auto-ingestion failed"
                                        );
                                    }
                                }
                            });
                        }
                    }
                }
            }
        });

        // Store the handle
        let handle = WatchHandle {
            id: watch_id.clone(),
            config,
            cancel_tx,
        };
        self.watches.write().await.push(handle);

        Ok(watch_id)
    }

    /// Stop a watch by its ID.
    pub async fn stop_watch(&self, watch_id: &str) -> Result<(), MongoCoreError> {
        let mut watches = self.watches.write().await;
        let pos = watches.iter().position(|h| h.id == watch_id).ok_or_else(|| {
            MongoCoreError::ValidationError(format!("Watch not found: {watch_id}"))
        })?;

        let handle = watches.remove(pos);
        let _ = handle.cancel_tx.send(());
        Ok(())
    }
}
