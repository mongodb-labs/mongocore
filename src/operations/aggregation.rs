use bson::Document;
use tokio::time::timeout;

use super::crud::{Operations, Result};
use crate::defaults::DEFAULT_AGGREGATION_TIMEOUT;
use crate::error::MongoCoreError;

impl Operations {
    /// Execute an aggregation pipeline and return results.
    pub async fn aggregate(
        &self,
        db: &str,
        collection: &str,
        pipeline: Vec<Document>,
    ) -> Result<Vec<Document>> {
        let coll = self.pool.collection(db, collection);

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
