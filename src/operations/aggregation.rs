use bson::Document;
use mongodb::options::{CollectionOptions, ReadConcern};
use tokio::time::timeout;

use super::crud::{Operations, Result};
use crate::defaults::DEFAULT_AGGREGATION_TIMEOUT;
use crate::error::MongoCoreError;

fn requires_local_read_concern(pipeline: &[Document]) -> bool {
    pipeline.first().map_or(false, |stage| {
        stage.contains_key("$search") || stage.contains_key("$vectorSearch")
    })
}

impl Operations {
    /// Execute an aggregation pipeline and return the raw cursor for streaming.
    pub async fn aggregate_cursor(
        &self,
        db: &str,
        collection: &str,
        pipeline: Vec<Document>,
    ) -> Result<mongodb::Cursor<Document>> {
        let coll = if requires_local_read_concern(&pipeline) {
            self.pool.database(db).collection_with_options::<Document>(
                collection,
                CollectionOptions::builder()
                    .read_concern(ReadConcern::local())
                    .build(),
            )
        } else {
            self.pool.collection(db, collection)
        };
        let cursor = coll.aggregate(pipeline).await?;
        Ok(cursor)
    }

    /// Execute an aggregation pipeline and return results.
    pub async fn aggregate(
        &self,
        db: &str,
        collection: &str,
        pipeline: Vec<Document>,
    ) -> Result<Vec<Document>> {
        let coll = if requires_local_read_concern(&pipeline) {
            self.pool.database(db).collection_with_options::<Document>(
                collection,
                CollectionOptions::builder()
                    .read_concern(ReadConcern::local())
                    .build(),
            )
        } else {
            self.pool.collection(db, collection)
        };

        let docs = timeout(DEFAULT_AGGREGATION_TIMEOUT, async {
            let mut cursor = coll.aggregate(pipeline).await?;
            let mut results = Vec::new();
            while cursor.advance().await? {
                results.push(cursor.deserialize_current()?);
            }
            Ok::<Vec<Document>, mongodb::error::Error>(results)
        })
        .await
        .map_err(|_| {
            MongoCoreError::TimeoutError("aggregation operation timed out".to_string())
        })??;

        Ok(docs)
    }
}
