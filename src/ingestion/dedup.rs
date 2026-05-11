use bson::{doc, Bson, Document};
use futures::TryStreamExt;
use mongodb::Collection;

use crate::error::MongoCoreError;
use crate::ingestion::types::ConflictStrategy;

/// Result of deduplication check for a single document.
#[derive(Debug)]
pub enum DedupResult {
    /// Document should be inserted (no conflict found).
    Insert(Document),
    /// Document should be skipped (conflict found, strategy is Skip).
    Skip,
    /// Document should replace the existing one (conflict found, strategy is Overwrite).
    Replace(Document),
    /// Documents should be merged (incoming, existing).
    Merge(Document, Document),
}

/// Checks incoming documents against existing data for deduplication.
pub struct DedupChecker {
    collection: Collection<Document>,
    dedup_key: Vec<String>,
    strategy: ConflictStrategy,
}

impl DedupChecker {
    pub fn new(
        collection: Collection<Document>,
        dedup_key: Vec<String>,
        strategy: ConflictStrategy,
    ) -> Self {
        Self {
            collection,
            dedup_key,
            strategy,
        }
    }

    /// Build a filter document from the dedup key fields of a given document.
    /// Returns None if the document is missing any dedup key field.
    pub fn build_dedup_filter(&self, doc: &Document) -> Option<Document> {
        build_dedup_filter(&self.dedup_key, doc)
    }

    /// Check a batch of documents against existing data and return DedupResults.
    pub async fn check_batch(&self, docs: &[Document]) -> Result<Vec<DedupResult>, MongoCoreError> {
        // If dedup_key is empty, return all as Insert
        if self.dedup_key.is_empty() {
            return Ok(docs.iter().map(|d| DedupResult::Insert(d.clone())).collect());
        }

        // Build $or filter for all docs that have complete dedup keys
        let mut or_filters: Vec<Bson> = Vec::new();

        for d in docs.iter() {
            if let Some(filter) = self.build_dedup_filter(d) {
                or_filters.push(Bson::Document(filter));
            }
        }

        // Query existing documents
        let existing_docs = if or_filters.is_empty() {
            Vec::new()
        } else {
            let batch_filter = doc! { "$or": or_filters };
            let mut cursor = self
                .collection
                .find(batch_filter)
                .await
                .map_err(|e| MongoCoreError::IngestionError(e.to_string()))?;

            let mut existing = Vec::new();
            while let Some(existing_doc) = cursor
                .try_next()
                .await
                .map_err(|e| MongoCoreError::IngestionError(e.to_string()))?
            {
                existing.push(existing_doc);
            }
            existing
        };

        // For each incoming doc, find matching existing doc and resolve
        let mut results = Vec::with_capacity(docs.len());
        for d in docs.iter() {
            let filter = self.build_dedup_filter(d);
            if filter.is_none() {
                // Missing dedup key fields — treat as insert
                results.push(DedupResult::Insert(d.clone()));
                continue;
            }
            let filter = filter.unwrap();

            // Find matching existing document
            let matching = existing_docs.iter().find(|existing| {
                self.dedup_key.iter().all(|key| {
                    filter.get(key) == existing.get(key)
                })
            });

            match matching {
                None => results.push(DedupResult::Insert(d.clone())),
                Some(existing) => {
                    results.push(resolve_conflict(&self.strategy, d, existing));
                }
            }
        }

        Ok(results)
    }

    /// Resolve conflict between incoming and existing document.
    pub fn resolve_conflict(&self, incoming: &Document, existing: &Document) -> DedupResult {
        resolve_conflict(&self.strategy, incoming, existing)
    }

    /// Shallow merge: start with existing, overlay all fields from incoming.
    pub fn merge_documents(incoming: &Document, existing: &Document) -> Document {
        merge_documents(incoming, existing)
    }
}

/// Build a filter document from the dedup key fields of a given document.
/// Returns None if the document is missing any dedup key field.
pub fn build_dedup_filter(dedup_key: &[String], doc: &Document) -> Option<Document> {
    let mut filter = Document::new();
    for key in dedup_key {
        match doc.get(key) {
            Some(value) => {
                filter.insert(key.clone(), value.clone());
            }
            None => return None,
        }
    }
    Some(filter)
}

/// Resolve conflict between incoming and existing document based on strategy.
pub fn resolve_conflict(
    strategy: &ConflictStrategy,
    incoming: &Document,
    existing: &Document,
) -> DedupResult {
    match strategy {
        ConflictStrategy::Skip => DedupResult::Skip,
        ConflictStrategy::Overwrite => DedupResult::Replace(incoming.clone()),
        ConflictStrategy::Merge => DedupResult::Merge(incoming.clone(), existing.clone()),
    }
}

/// Shallow merge: start with existing, overlay all fields from incoming.
pub fn merge_documents(incoming: &Document, existing: &Document) -> Document {
    let mut result = existing.clone();
    for (key, value) in incoming {
        result.insert(key.clone(), value.clone());
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use bson::doc;

    #[test]
    fn test_build_dedup_filter_single_key() {
        let dedup_key = vec!["email".to_string()];
        let d = doc! { "email": "test@example.com", "name": "Test" };
        let filter = build_dedup_filter(&dedup_key, &d).unwrap();
        assert_eq!(filter, doc! { "email": "test@example.com" });
    }

    #[test]
    fn test_build_dedup_filter_composite_key() {
        let dedup_key = vec!["first_name".to_string(), "last_name".to_string()];
        let d = doc! { "first_name": "John", "last_name": "Doe", "age": 30 };
        let filter = build_dedup_filter(&dedup_key, &d).unwrap();
        assert_eq!(filter, doc! { "first_name": "John", "last_name": "Doe" });
    }

    #[test]
    fn test_build_dedup_filter_missing_key() {
        let dedup_key = vec!["email".to_string(), "tenant_id".to_string()];
        let d = doc! { "email": "test@example.com", "name": "Test" };
        let result = build_dedup_filter(&dedup_key, &d);
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_skip() {
        let incoming = doc! { "email": "a@b.com", "name": "New" };
        let existing = doc! { "email": "a@b.com", "name": "Old" };
        let result = resolve_conflict(&ConflictStrategy::Skip, &incoming, &existing);
        assert!(matches!(result, DedupResult::Skip));
    }

    #[test]
    fn test_resolve_overwrite() {
        let incoming = doc! { "email": "a@b.com", "name": "New" };
        let existing = doc! { "email": "a@b.com", "name": "Old" };
        let result = resolve_conflict(&ConflictStrategy::Overwrite, &incoming, &existing);
        match result {
            DedupResult::Replace(d) => assert_eq!(d, incoming),
            _ => panic!("Expected Replace"),
        }
    }

    #[test]
    fn test_resolve_merge() {
        let incoming = doc! { "email": "a@b.com", "name": "New" };
        let existing = doc! { "email": "a@b.com", "name": "Old" };
        let result = resolve_conflict(&ConflictStrategy::Merge, &incoming, &existing);
        match result {
            DedupResult::Merge(inc, ext) => {
                assert_eq!(inc, incoming);
                assert_eq!(ext, existing);
            }
            _ => panic!("Expected Merge"),
        }
    }

    #[test]
    fn test_merge_documents() {
        let incoming = doc! { "name": "New", "age": 25 };
        let existing = doc! { "name": "Old", "city": "NYC", "age": 30 };
        let merged = merge_documents(&incoming, &existing);
        // incoming fields win, existing-only fields preserved
        assert_eq!(merged.get_str("name").unwrap(), "New");
        assert_eq!(merged.get_i32("age").unwrap(), 25);
        assert_eq!(merged.get_str("city").unwrap(), "NYC");
    }

    #[test]
    fn test_merge_documents_incoming_adds_new_fields() {
        let incoming = doc! { "new_field": "value" };
        let existing = doc! { "old_field": "existing" };
        let merged = merge_documents(&incoming, &existing);
        assert_eq!(merged.get_str("new_field").unwrap(), "value");
        assert_eq!(merged.get_str("old_field").unwrap(), "existing");
    }
}
