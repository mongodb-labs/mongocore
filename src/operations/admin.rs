use bson::{doc, Document};
use mongodb::IndexModel;
use tokio::time::timeout;

use super::crud::{Operations, Result};
use crate::defaults::DEFAULT_QUERY_TIMEOUT;
use crate::error::MongoCoreError;

/// Options for index creation.
#[derive(Debug, Clone, Default)]
pub struct IndexOptions {
    /// Custom name for the index.
    pub name: Option<String>,
    /// Whether the index enforces uniqueness.
    pub unique: Option<bool>,
    /// Whether the index only includes documents with the indexed field.
    pub sparse: Option<bool>,
}

impl Operations {
    /// Create a collection in the specified database.
    pub async fn create_collection(&self, db: &str, name: &str) -> Result<()> {
        let database = self.pool.database(db);

        timeout(DEFAULT_QUERY_TIMEOUT, database.create_collection(name))
            .await
            .map_err(|_| {
                MongoCoreError::TimeoutError("create_collection operation timed out".to_string())
            })??;

        Ok(())
    }

    /// Drop a collection from the specified database.
    pub async fn drop_collection(&self, db: &str, name: &str) -> Result<()> {
        let collection = self.pool.collection(db, name);

        timeout(DEFAULT_QUERY_TIMEOUT, collection.drop())
            .await
            .map_err(|_| {
                MongoCoreError::TimeoutError("drop_collection operation timed out".to_string())
            })??;

        Ok(())
    }

    /// Create a database by verifying connectivity to it.
    ///
    /// MongoDB creates databases implicitly when data is first written.
    /// This method verifies that the database name is reachable by running a ping command.
    pub async fn create_database(&self, db: &str) -> Result<()> {
        let database = self.pool.database(db);

        timeout(
            DEFAULT_QUERY_TIMEOUT,
            database.run_command(doc! { "ping": 1 }),
        )
        .await
        .map_err(|_| {
            MongoCoreError::TimeoutError("create_database operation timed out".to_string())
        })??;

        Ok(())
    }

    /// Create a user in the specified database with the given roles.
    pub async fn create_user(
        &self,
        db: &str,
        username: &str,
        password: &str,
        roles: Vec<Document>,
    ) -> Result<()> {
        let database = self.pool.database(db);

        let cmd = doc! {
            "createUser": username,
            "pwd": password,
            "roles": roles,
        };

        timeout(DEFAULT_QUERY_TIMEOUT, database.run_command(cmd))
            .await
            .map_err(|_| {
                MongoCoreError::TimeoutError("create_user operation timed out".to_string())
            })??;

        Ok(())
    }

    /// Create an index on the specified collection.
    ///
    /// Returns the name of the created index.
    pub async fn create_index(
        &self,
        db: &str,
        collection: &str,
        keys: Document,
        options: Option<IndexOptions>,
    ) -> Result<String> {
        let coll = self.pool.collection(db, collection);

        let mut driver_index_opts = mongodb::options::IndexOptions::builder().build();
        if let Some(opts) = options {
            driver_index_opts.name = opts.name;
            driver_index_opts.unique = opts.unique;
            driver_index_opts.sparse = opts.sparse;
        }

        let index_model = IndexModel::builder()
            .keys(keys)
            .options(driver_index_opts)
            .build();

        let result = timeout(DEFAULT_QUERY_TIMEOUT, coll.create_index(index_model))
            .await
            .map_err(|_| {
                MongoCoreError::TimeoutError("create_index operation timed out".to_string())
            })??;

        Ok(result.index_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_options_default() {
        let opts = IndexOptions::default();
        assert!(opts.name.is_none());
        assert!(opts.unique.is_none());
        assert!(opts.sparse.is_none());
    }

    #[test]
    fn test_index_options_custom() {
        let opts = IndexOptions {
            name: Some("my_index".to_string()),
            unique: Some(true),
            sparse: Some(false),
        };
        assert_eq!(opts.name, Some("my_index".to_string()));
        assert_eq!(opts.unique, Some(true));
        assert_eq!(opts.sparse, Some(false));
    }
}
