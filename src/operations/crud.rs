use bson::Document;
use mongodb::results::{DeleteResult, InsertManyResult, InsertOneResult, UpdateResult};
use tokio::time::timeout;

use crate::connection::pool::ConnectionPool;
use crate::defaults::DEFAULT_QUERY_TIMEOUT;
use crate::error::MongoCoreError;

/// Result type alias for CRUD operations.
pub type Result<T> = std::result::Result<T, MongoCoreError>;

/// Options for find operations.
#[derive(Debug, Clone, Default)]
pub struct FindOptions {
    /// Maximum number of documents to return.
    pub limit: Option<i64>,
    /// Number of documents to skip.
    pub skip: Option<u64>,
    /// Sort specification document.
    pub sort: Option<Document>,
    /// Projection specification document.
    pub projection: Option<Document>,
}

impl FindOptions {
    /// Convert to the mongodb driver's FindOptions.
    fn to_driver_options(&self) -> mongodb::options::FindOptions {
        mongodb::options::FindOptions::builder()
            .limit(self.limit)
            .skip(self.skip)
            .sort(self.sort.clone())
            .projection(self.projection.clone())
            .build()
    }
}

/// CRUD operations backed by a ConnectionPool.
#[derive(Debug, Clone)]
pub struct Operations {
    pub(crate) pool: ConnectionPool,
}

impl Operations {
    /// Create a new Operations instance from a ConnectionPool.
    pub fn new(pool: ConnectionPool) -> Self {
        Self { pool }
    }

    /// Find multiple documents matching the filter.
    pub async fn find(
        &self,
        db: &str,
        collection: &str,
        filter: Document,
        options: Option<FindOptions>,
    ) -> Result<Vec<Document>> {
        let coll = self.pool.collection(db, collection);
        let driver_opts = options.map(|o| o.to_driver_options());

        let docs = timeout(DEFAULT_QUERY_TIMEOUT, async {
            let mut cursor = coll.find(filter).with_options(driver_opts).await?;
            let mut results = Vec::new();
            while cursor.advance().await? {
                results.push(cursor.deserialize_current()?);
            }
            Ok::<Vec<Document>, mongodb::error::Error>(results)
        })
        .await
        .map_err(|_| MongoCoreError::TimeoutError("find operation timed out".to_string()))??;

        Ok(docs)
    }

    /// Find documents and return the raw cursor for streaming.
    pub async fn find_cursor(
        &self,
        db: &str,
        collection: &str,
        filter: Document,
        options: Option<FindOptions>,
    ) -> Result<mongodb::Cursor<Document>> {
        let coll = self.pool.collection(db, collection);
        let driver_opts = options.map(|o| o.to_driver_options());
        let cursor = coll.find(filter).with_options(driver_opts).await?;
        Ok(cursor)
    }

    /// Find a single document matching the filter.
    pub async fn find_one(
        &self,
        db: &str,
        collection: &str,
        filter: Document,
    ) -> Result<Option<Document>> {
        let coll = self.pool.collection(db, collection);

        let doc = timeout(DEFAULT_QUERY_TIMEOUT, coll.find_one(filter))
            .await
            .map_err(|_| {
                MongoCoreError::TimeoutError("find_one operation timed out".to_string())
            })??;

        Ok(doc)
    }

    /// Insert a single document.
    pub async fn insert(
        &self,
        db: &str,
        collection: &str,
        document: Document,
    ) -> Result<InsertOneResult> {
        let coll = self.pool.collection(db, collection);

        let result = timeout(DEFAULT_QUERY_TIMEOUT, coll.insert_one(document))
            .await
            .map_err(|_| {
                MongoCoreError::TimeoutError("insert operation timed out".to_string())
            })??;

        Ok(result)
    }

    /// Insert multiple documents.
    pub async fn insert_many(
        &self,
        db: &str,
        collection: &str,
        documents: Vec<Document>,
    ) -> Result<InsertManyResult> {
        let coll = self.pool.collection(db, collection);

        let result = timeout(DEFAULT_QUERY_TIMEOUT, coll.insert_many(documents))
            .await
            .map_err(|_| {
                MongoCoreError::TimeoutError("insert_many operation timed out".to_string())
            })??;

        Ok(result)
    }

    /// Update the first document matching the filter.
    pub async fn update(
        &self,
        db: &str,
        collection: &str,
        filter: Document,
        update: Document,
    ) -> Result<UpdateResult> {
        let coll = self.pool.collection(db, collection);

        let result = timeout(DEFAULT_QUERY_TIMEOUT, coll.update_one(filter, update))
            .await
            .map_err(|_| {
                MongoCoreError::TimeoutError("update operation timed out".to_string())
            })??;

        Ok(result)
    }

    /// Update all documents matching the filter.
    pub async fn update_many(
        &self,
        db: &str,
        collection: &str,
        filter: Document,
        update: Document,
    ) -> Result<UpdateResult> {
        let coll = self.pool.collection(db, collection);

        let result = timeout(DEFAULT_QUERY_TIMEOUT, coll.update_many(filter, update))
            .await
            .map_err(|_| {
                MongoCoreError::TimeoutError("update_many operation timed out".to_string())
            })??;

        Ok(result)
    }

    /// Delete the first document matching the filter.
    pub async fn delete(
        &self,
        db: &str,
        collection: &str,
        filter: Document,
    ) -> Result<DeleteResult> {
        let coll = self.pool.collection(db, collection);

        let result = timeout(DEFAULT_QUERY_TIMEOUT, coll.delete_one(filter))
            .await
            .map_err(|_| {
                MongoCoreError::TimeoutError("delete operation timed out".to_string())
            })??;

        Ok(result)
    }

    /// Delete all documents matching the filter.
    pub async fn delete_many(
        &self,
        db: &str,
        collection: &str,
        filter: Document,
    ) -> Result<DeleteResult> {
        let coll = self.pool.collection(db, collection);

        let result = timeout(DEFAULT_QUERY_TIMEOUT, coll.delete_many(filter))
            .await
            .map_err(|_| {
                MongoCoreError::TimeoutError("delete_many operation timed out".to_string())
            })??;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bson::doc;

    #[test]
    fn test_find_options_default() {
        let opts = FindOptions::default();
        assert!(opts.limit.is_none());
        assert!(opts.skip.is_none());
        assert!(opts.sort.is_none());
        assert!(opts.projection.is_none());
    }

    #[test]
    fn test_find_options_to_driver_options() {
        let opts = FindOptions {
            limit: Some(10),
            skip: Some(5),
            sort: Some(doc! { "name": 1 }),
            projection: Some(doc! { "name": 1, "_id": 0 }),
        };

        let driver_opts = opts.to_driver_options();
        assert_eq!(driver_opts.limit, Some(10));
        assert_eq!(driver_opts.skip, Some(5));
        assert_eq!(driver_opts.sort, Some(doc! { "name": 1 }));
        assert_eq!(driver_opts.projection, Some(doc! { "name": 1, "_id": 0 }));
    }

    #[test]
    fn test_find_options_partial() {
        let opts = FindOptions {
            limit: Some(100),
            skip: None,
            sort: None,
            projection: Some(doc! { "field": 1 }),
        };

        let driver_opts = opts.to_driver_options();
        assert_eq!(driver_opts.limit, Some(100));
        assert!(driver_opts.skip.is_none());
        assert!(driver_opts.sort.is_none());
        assert_eq!(driver_opts.projection, Some(doc! { "field": 1 }));
    }
}
