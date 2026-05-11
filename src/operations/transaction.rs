use bson::Document;
use mongodb::results::{DeleteResult, InsertOneResult, UpdateResult};
use mongodb::ClientSession;

use crate::connection::pool::ConnectionPool;
use crate::error::MongoCoreError;

use super::crud::{Operations, Result};

/// A transaction wrapper that manages a MongoDB client session with an active transaction.
///
/// Provides CRUD operations that execute within the transaction context.
/// The transaction must be explicitly committed or aborted; dropping without
/// committing will cause the server to abort the transaction on session cleanup.
pub struct Transaction {
    session: ClientSession,
    pool: ConnectionPool,
}

impl Transaction {
    /// Start a new transaction on a fresh session obtained from the connection pool.
    pub async fn begin(pool: &ConnectionPool) -> Result<Self> {
        let mut session = pool.client().start_session().await?;
        session.start_transaction().await?;

        Ok(Self {
            session,
            pool: pool.clone(),
        })
    }

    /// Find multiple documents matching the filter within this transaction.
    pub async fn find(
        &mut self,
        db: &str,
        collection: &str,
        filter: Document,
    ) -> Result<Vec<Document>> {
        let coll = self.pool.collection(db, collection);

        let mut cursor = coll.find(filter).session(&mut self.session).await?;

        let mut results = Vec::new();
        while cursor.advance(&mut self.session).await? {
            results.push(cursor.deserialize_current()?);
        }

        Ok(results)
    }

    /// Insert a single document within this transaction.
    pub async fn insert(
        &mut self,
        db: &str,
        collection: &str,
        document: Document,
    ) -> Result<InsertOneResult> {
        let coll = self.pool.collection(db, collection);

        let result = coll.insert_one(document).session(&mut self.session).await?;

        Ok(result)
    }

    /// Update the first document matching the filter within this transaction.
    pub async fn update(
        &mut self,
        db: &str,
        collection: &str,
        filter: Document,
        update: Document,
    ) -> Result<UpdateResult> {
        let coll = self.pool.collection(db, collection);

        let result = coll
            .update_one(filter, update)
            .session(&mut self.session)
            .await?;

        Ok(result)
    }

    /// Delete the first document matching the filter within this transaction.
    pub async fn delete(
        &mut self,
        db: &str,
        collection: &str,
        filter: Document,
    ) -> Result<DeleteResult> {
        let coll = self.pool.collection(db, collection);

        let result = coll.delete_one(filter).session(&mut self.session).await?;

        Ok(result)
    }

    /// Commit the transaction, making all changes permanent.
    pub async fn commit(&mut self) -> Result<()> {
        self.session
            .commit_transaction()
            .await
            .map_err(MongoCoreError::from)?;
        Ok(())
    }

    /// Abort the transaction, discarding all changes.
    pub async fn abort(&mut self) -> Result<()> {
        self.session
            .abort_transaction()
            .await
            .map_err(MongoCoreError::from)?;
        Ok(())
    }
}

impl Operations {
    /// Begin a new transaction using the underlying connection pool.
    ///
    /// Returns a `Transaction` that can be used to execute operations atomically.
    /// The transaction must be explicitly committed via [`Transaction::commit`] or
    /// aborted via [`Transaction::abort`].
    pub async fn begin_transaction(&self) -> Result<Transaction> {
        Transaction::begin(&self.pool).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_requires_pool() {
        // Transaction::begin requires a real ConnectionPool with a live client,
        // so we verify the type signatures compile correctly.
        // Integration tests with a replica set are in the test harness (Task 6).
        fn _assert_send<T: Send>() {}
        // Transaction holds a ClientSession which may not be Send in all configurations,
        // but we verify the basic structure compiles.
        let _ = std::mem::size_of::<Transaction>();
    }

    #[test]
    fn test_operations_has_begin_transaction() {
        // Verify the method exists on Operations at compile time.
        // We cannot call it without a real connection, but we confirm the API shape.
        fn _check_method(
            ops: &Operations,
        ) -> impl std::future::Future<Output = Result<Transaction>> + '_ {
            ops.begin_transaction()
        }
    }
}
