use crate::analytics::aggregator::{aggregate, AnalyticsSummary};
use crate::analytics::AnalyticsCollector;
use crate::connection::ConnectionPool;
use crate::error::MongoCoreError;
use bson::doc;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info};

/// Background persistence for analytics snapshots to MongoDB.
pub struct AnalyticsPersistence {
    pool: ConnectionPool,
    collector: Arc<AnalyticsCollector>,
    flush_interval: Duration,
    database: String,
    collection: String,
}

impl AnalyticsPersistence {
    /// Create a new analytics persistence handler with default database and collection names.
    pub fn new(
        pool: ConnectionPool,
        collector: Arc<AnalyticsCollector>,
        flush_interval: Duration,
    ) -> Self {
        Self {
            pool,
            collector,
            flush_interval,
            database: "__mongocore".to_string(),
            collection: "analytics".to_string(),
        }
    }

    /// Flush a snapshot of analytics events to MongoDB.
    /// Returns early if there are no events to flush.
    pub async fn flush_snapshot(&self) -> Result<(), MongoCoreError> {
        // Get a snapshot of current events
        let events = self.collector.snapshot();

        // Early return if no events to flush
        if events.is_empty() {
            debug!("No analytics events to flush");
            return Ok(());
        }

        // Aggregate the events into a summary
        let summary: AnalyticsSummary = aggregate(&events);

        // Convert top_operations to BSON array of documents
        let top_operations: Vec<bson::Document> = summary
            .top_operations
            .iter()
            .map(|(op, count)| {
                doc! {
                    "operation": format!("{:?}", op),
                    "count": *count as i64,
                }
            })
            .collect();

        // Convert top_collections to BSON array of documents
        let top_collections: Vec<bson::Document> = summary
            .top_collections
            .iter()
            .map(|(collection, count)| {
                doc! {
                    "collection": collection,
                    "count": *count as i64,
                }
            })
            .collect();

        // Build the document to insert
        let doc = doc! {
            "timestamp": bson::DateTime::now(),
            "total_operations": summary.total_operations as i64,
            "total_errors": summary.total_errors as i64,
            "error_rate": summary.error_rate,
            "p50_latency_ms": summary.p50_latency_ms,
            "p95_latency_ms": summary.p95_latency_ms,
            "p99_latency_ms": summary.p99_latency_ms,
            "top_operations": top_operations,
            "top_collections": top_collections,
        };

        // Insert the document
        let collection = self.pool.collection(&self.database, &self.collection);
        collection
            .insert_one(doc)
            .await
            .map_err(|e| MongoCoreError::OperationError(format!("Failed to insert analytics snapshot: {}", e)))?;

        info!(
            "Flushed analytics snapshot: {} operations, {} errors",
            summary.total_operations, summary.total_errors
        );

        Ok(())
    }

    /// Start a background task that flushes analytics snapshots on the configured interval.
    /// Returns a tokio JoinHandle that can be used to await or cancel the task.
    pub fn start_background_flush(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut ticker = interval(self.flush_interval);

            info!(
                "Starting background analytics flush every {:?}",
                self.flush_interval
            );

            loop {
                ticker.tick().await;

                match self.flush_snapshot().await {
                    Ok(()) => {
                        debug!("Background analytics flush completed successfully");
                    }
                    Err(e) => {
                        error!("Background analytics flush failed: {}", e);
                    }
                }
            }
        })
    }
}
