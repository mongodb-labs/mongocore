use std::sync::Arc;

use bson::{doc, Document};

use crate::connection::pool::ConnectionPool;
use crate::operations::{FindOptions, Operations};
use crate::voyage::VoyageClient;

/// The method used to produce search results.
#[derive(Debug, Clone, PartialEq)]
pub enum SearchMethod {
    Vector,
    Fulltext,
    Filter,
}

/// Search results with metadata about how they were produced.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub documents: Vec<Document>,
    pub method: SearchMethod,
    pub total: usize,
}

/// Search engine with automatic fallback chain: vector -> fulltext -> filter.
pub struct SearchEngine {
    operations: Operations,
    pool: ConnectionPool,
    voyage_client: Option<Arc<VoyageClient>>,
}

impl SearchEngine {
    pub fn new(pool: ConnectionPool, voyage_client: Option<Arc<VoyageClient>>) -> Self {
        Self {
            operations: Operations::new(pool.clone()),
            pool,
            voyage_client,
        }
    }

    /// Semantic search with automatic fallback chain.
    ///
    /// Falls back through: vector search -> fulltext search -> filter.
    pub async fn search(
        &self,
        database: &str,
        collection: &str,
        query: &str,
        limit: i64,
    ) -> Result<SearchResult, SearchError> {
        let capabilities = self.pool.capabilities();

        // Try vector search first (requires Voyage AI + Atlas Vector Search)
        if capabilities.atlas_vector_search {
            if let Some(ref client) = self.voyage_client {
                match self
                    .vector_search_internal(client, database, collection, query, limit)
                    .await
                {
                    Ok(result) if !result.documents.is_empty() => return Ok(result),
                    _ => {} // Fall through if error or empty results
                }
            }
        }

        // Try full-text search (requires Atlas Search)
        if capabilities.atlas_search {
            match self
                .fulltext_search_internal(database, collection, query, limit)
                .await
            {
                Ok(result) if !result.documents.is_empty() => return Ok(result),
                _ => {} // Fall through if error or empty results
            }
        }

        // Fallback: text/filter search
        self.filter_fallback(database, collection, query, limit)
            .await
    }

    /// Direct vector search. Embeds the query and runs $vectorSearch.
    pub async fn vector_search(
        &self,
        database: &str,
        collection: &str,
        query: &str,
        index_name: &str,
        field_path: &str,
        limit: i64,
    ) -> Result<SearchResult, SearchError> {
        let client = self
            .voyage_client
            .as_ref()
            .ok_or(SearchError::NotConfigured(
                "Voyage AI client not configured".to_string(),
            ))?;

        let embedding = client
            .embed(vec![query.to_string()])
            .await
            .map_err(|e| SearchError::EmbeddingError(e.to_string()))?;

        let vector = embedding
            .embeddings
            .into_iter()
            .next()
            .ok_or(SearchError::EmbeddingError(
                "No embedding returned".to_string(),
            ))?;

        let pipeline = super::vector::VectorSearchBuilder::build_pipeline(
            index_name,
            field_path,
            vector,
            limit * 10,
            limit,
        );

        let docs = self
            .operations
            .aggregate(database, collection, pipeline)
            .await
            .map_err(|e| SearchError::OperationError(e.to_string()))?;

        let total = docs.len();
        Ok(SearchResult {
            documents: docs,
            method: SearchMethod::Vector,
            total,
        })
    }

    async fn vector_search_internal(
        &self,
        client: &VoyageClient,
        database: &str,
        collection: &str,
        query: &str,
        limit: i64,
    ) -> Result<SearchResult, SearchError> {
        let embedding = client
            .embed(vec![query.to_string()])
            .await
            .map_err(|e| SearchError::EmbeddingError(e.to_string()))?;

        let vector = embedding
            .embeddings
            .into_iter()
            .next()
            .ok_or(SearchError::EmbeddingError(
                "No embedding returned".to_string(),
            ))?;

        let pipeline = super::vector::VectorSearchBuilder::build_pipeline(
            "default",
            "embedding",
            vector,
            limit * 10,
            limit,
        );

        let docs = self
            .operations
            .aggregate(database, collection, pipeline)
            .await
            .map_err(|e| SearchError::OperationError(e.to_string()))?;

        let total = docs.len();
        Ok(SearchResult {
            documents: docs,
            method: SearchMethod::Vector,
            total,
        })
    }

    async fn fulltext_search_internal(
        &self,
        database: &str,
        collection: &str,
        query: &str,
        limit: i64,
    ) -> Result<SearchResult, SearchError> {
        let pipeline =
            super::fulltext::FulltextSearchBuilder::build_pipeline("default", query, &["*"], limit);

        let docs = self
            .operations
            .aggregate(database, collection, pipeline)
            .await
            .map_err(|e| SearchError::OperationError(e.to_string()))?;

        let total = docs.len();
        Ok(SearchResult {
            documents: docs,
            method: SearchMethod::Fulltext,
            total,
        })
    }

    async fn filter_fallback(
        &self,
        database: &str,
        collection: &str,
        query: &str,
        limit: i64,
    ) -> Result<SearchResult, SearchError> {
        let filter = doc! {
            "$text": { "$search": query }
        };

        let options = Some(FindOptions {
            limit: Some(limit),
            ..Default::default()
        });

        // Try $text search first, fall back to empty filter
        let docs = match self
            .operations
            .find(database, collection, filter, options.clone())
            .await
        {
            Ok(d) => d,
            Err(_) => {
                // $text not available, return empty result set
                self.operations
                    .find(database, collection, doc! {}, options)
                    .await
                    .map_err(|e| SearchError::OperationError(e.to_string()))?
            }
        };

        let total = docs.len();
        Ok(SearchResult {
            documents: docs,
            method: SearchMethod::Filter,
            total,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("Not configured: {0}")]
    NotConfigured(String),
    #[error("Embedding error: {0}")]
    EmbeddingError(String),
    #[error("Operation error: {0}")]
    OperationError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_method_equality() {
        assert_eq!(SearchMethod::Vector, SearchMethod::Vector);
        assert_eq!(SearchMethod::Fulltext, SearchMethod::Fulltext);
        assert_eq!(SearchMethod::Filter, SearchMethod::Filter);
        assert_ne!(SearchMethod::Vector, SearchMethod::Fulltext);
        assert_ne!(SearchMethod::Vector, SearchMethod::Filter);
        assert_ne!(SearchMethod::Fulltext, SearchMethod::Filter);
    }

    #[test]
    fn test_search_result_construction() {
        let result = SearchResult {
            documents: vec![doc! { "name": "test" }, doc! { "name": "test2" }],
            method: SearchMethod::Vector,
            total: 2,
        };

        assert_eq!(result.documents.len(), 2);
        assert_eq!(result.method, SearchMethod::Vector);
        assert_eq!(result.total, 2);
    }

    #[test]
    fn test_search_result_empty() {
        let result = SearchResult {
            documents: vec![],
            method: SearchMethod::Filter,
            total: 0,
        };

        assert!(result.documents.is_empty());
        assert_eq!(result.method, SearchMethod::Filter);
        assert_eq!(result.total, 0);
    }

    #[test]
    fn test_search_error_display() {
        let err = SearchError::NotConfigured("test".to_string());
        assert_eq!(err.to_string(), "Not configured: test");

        let err = SearchError::EmbeddingError("embed failed".to_string());
        assert_eq!(err.to_string(), "Embedding error: embed failed");

        let err = SearchError::OperationError("op failed".to_string());
        assert_eq!(err.to_string(), "Operation error: op failed");
    }
}
