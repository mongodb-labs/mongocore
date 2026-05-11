use bson::Document;
use mongodb::options::ReturnDocument;
use tokio::time::timeout;

use super::crud::{Operations, Result};
use crate::defaults::DEFAULT_QUERY_TIMEOUT;
use crate::error::MongoCoreError;

/// Controls whether the returned document is the version before or after the modification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReturnDocumentOption {
    /// Return the document before the modification.
    Before,
    /// Return the document after the modification (default).
    #[default]
    After,
}

/// Options for find-and-modify operations.
#[derive(Debug, Clone, Default)]
pub struct FindAndModifyOptions {
    /// Whether to return the document before or after the update.
    pub return_document: ReturnDocumentOption,
    /// If true, insert a new document if no match is found.
    pub upsert: bool,
    /// Sort order to determine which document to modify when multiple match.
    pub sort: Option<Document>,
}

impl Operations {
    /// Atomically find a document and update it.
    ///
    /// Returns the matched document (before or after modification, depending on options).
    pub async fn find_and_modify(
        &self,
        db: &str,
        collection: &str,
        filter: Document,
        update: Document,
        options: Option<FindAndModifyOptions>,
    ) -> Result<Option<Document>> {
        let coll = self.pool.collection(db, collection);
        let opts = options.unwrap_or_default();

        let return_doc = match opts.return_document {
            ReturnDocumentOption::Before => ReturnDocument::Before,
            ReturnDocumentOption::After => ReturnDocument::After,
        };

        let mut driver_opts = mongodb::options::FindOneAndUpdateOptions::builder()
            .return_document(return_doc)
            .upsert(opts.upsert)
            .build();

        driver_opts.sort = opts.sort;

        let result = timeout(
            DEFAULT_QUERY_TIMEOUT,
            coll.find_one_and_update(filter, update)
                .with_options(driver_opts),
        )
        .await
        .map_err(|_| {
            MongoCoreError::TimeoutError("find_and_modify operation timed out".to_string())
        })??;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_return_document_option_default() {
        let opt = ReturnDocumentOption::default();
        assert_eq!(opt, ReturnDocumentOption::After);
    }

    #[test]
    fn test_find_and_modify_options_default() {
        let opts = FindAndModifyOptions::default();
        assert_eq!(opts.return_document, ReturnDocumentOption::After);
        assert!(!opts.upsert);
        assert!(opts.sort.is_none());
    }

    #[test]
    fn test_find_and_modify_options_custom() {
        let opts = FindAndModifyOptions {
            return_document: ReturnDocumentOption::Before,
            upsert: true,
            sort: Some(bson::doc! { "created_at": -1 }),
        };
        assert_eq!(opts.return_document, ReturnDocumentOption::Before);
        assert!(opts.upsert);
        assert_eq!(opts.sort, Some(bson::doc! { "created_at": -1 }));
    }
}
