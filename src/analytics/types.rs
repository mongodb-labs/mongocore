use bson::{Bson, Document};
use std::time::{Duration, Instant};

/// Represents the kind of operation being performed
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OperationKind {
    Find,
    FindOne,
    Insert,
    InsertMany,
    Update,
    UpdateMany,
    Delete,
    DeleteMany,
    FindAndModify,
    Aggregate,
    Search,
    Watch,
    RunCommand,
    BeginTransaction,
    CommitTransaction,
    AbortTransaction,
    CreateCollection,
    CreateIndex,
    ListDatabases,
    ListCollections,
    Pipeline,
}

/// An analytics event capturing metadata about an operation
#[derive(Clone)]
pub struct AnalyticsEvent {
    pub operation: OperationKind,
    pub database: String,
    pub collection: String,
    pub latency: Duration,
    pub success: bool,
    pub timestamp: Instant,
    pub fingerprint: Option<QueryFingerprint>,
    pub tenant_id: Option<String>,
    pub document_count: Option<u64>,
}

impl AnalyticsEvent {
    /// Create a new analytics event
    pub fn new(
        operation: OperationKind,
        database: String,
        collection: String,
        latency: Duration,
        success: bool,
    ) -> Self {
        Self {
            operation,
            database,
            collection,
            latency,
            success,
            timestamp: Instant::now(),
            fingerprint: None,
            tenant_id: None,
            document_count: None,
        }
    }
}

/// A query fingerprint represents the "shape" of a BSON filter
/// Keeps keys and nested structure but strips values
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QueryFingerprint(String);

impl QueryFingerprint {
    /// Create a fingerprint from a BSON document
    pub fn from_document(doc: &Document) -> Self {
        Self(Self::fingerprint_value(&Bson::Document(doc.clone())))
    }

    /// Recursively fingerprint a BSON value
    fn fingerprint_value(value: &Bson) -> String {
        match value {
            Bson::Document(doc) => {
                let mut keys: Vec<_> = doc.keys().collect();
                keys.sort();
                let entries: Vec<String> = keys
                    .iter()
                    .map(|k| {
                        let v = doc.get(*k).unwrap();
                        format!("{}:{}", k, Self::fingerprint_value(v))
                    })
                    .collect();
                format!("{{{}}}", entries.join(","))
            }
            Bson::Array(arr) => {
                if arr.is_empty() {
                    "[]".to_string()
                } else {
                    // For arrays, fingerprint the first element as representative
                    format!("[{}]", Self::fingerprint_value(&arr[0]))
                }
            }
            // All scalar values get replaced with their type
            Bson::Double(_) => "Double".to_string(),
            Bson::String(_) => "String".to_string(),
            Bson::Binary(_) => "Binary".to_string(),
            Bson::ObjectId(_) => "ObjectId".to_string(),
            Bson::Boolean(_) => "Boolean".to_string(),
            Bson::Null => "Null".to_string(),
            Bson::RegularExpression(_) => "Regex".to_string(),
            Bson::JavaScriptCode(_) => "JavaScript".to_string(),
            Bson::JavaScriptCodeWithScope(_) => "JavaScriptWithScope".to_string(),
            Bson::Int32(_) => "Int32".to_string(),
            Bson::Int64(_) => "Int64".to_string(),
            Bson::Timestamp(_) => "Timestamp".to_string(),
            Bson::DateTime(_) => "DateTime".to_string(),
            Bson::Symbol(_) => "Symbol".to_string(),
            Bson::Decimal128(_) => "Decimal128".to_string(),
            Bson::Undefined => "Undefined".to_string(),
            Bson::MaxKey => "MaxKey".to_string(),
            Bson::MinKey => "MinKey".to_string(),
            Bson::DbPointer(_) => "DbPointer".to_string(),
        }
    }

    /// Get the fingerprint string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Tracks a single LLM API call for the dashboard.
#[derive(Debug, Clone)]
pub struct LlmCallEvent {
    pub provider: String,
    pub model: String,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub latency: Duration,
    pub success: bool,
    pub timestamp: Instant,
}

/// Tracks a pipeline or transaction pipeline execution.
#[derive(Debug, Clone)]
pub struct PipelineEvent {
    pub is_transaction: bool,
    pub steps: usize,
    pub latency: Duration,
    pub success: bool,
    pub retries: u32,
    pub timestamp: Instant,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bson::doc;
    use std::time::Duration;

    #[test]
    fn test_event_creation() {
        let event = AnalyticsEvent::new(
            OperationKind::Find,
            "test_db".to_string(),
            "test_collection".to_string(),
            Duration::from_millis(42),
            true,
        );

        assert_eq!(event.operation, OperationKind::Find);
        assert_eq!(event.database, "test_db");
        assert_eq!(event.collection, "test_collection");
        assert_eq!(event.latency, Duration::from_millis(42));
        assert!(event.success);
        assert!(event.fingerprint.is_none());
        assert!(event.tenant_id.is_none());
        assert!(event.document_count.is_none());
    }

    #[test]
    fn test_query_fingerprint() {
        let filter1 = doc! { "name": "Alice", "age": 30 };
        let filter2 = doc! { "name": "Bob", "age": 25 };

        let fp1 = QueryFingerprint::from_document(&filter1);
        let fp2 = QueryFingerprint::from_document(&filter2);

        // Same shape, different values should produce same fingerprint
        assert_eq!(fp1, fp2);
        assert_eq!(fp1.as_str(), "{age:Int32,name:String}");
    }

    #[test]
    fn test_different_shape_different_fingerprint() {
        let filter1 = doc! { "name": "Alice", "age": 30 };
        let filter2 = doc! { "name": "Bob", "email": "bob@example.com" };

        let fp1 = QueryFingerprint::from_document(&filter1);
        let fp2 = QueryFingerprint::from_document(&filter2);

        // Different keys should produce different fingerprints
        assert_ne!(fp1, fp2);
    }

    #[test]
    fn test_nested_query_fingerprint() {
        let filter1 = doc! {
            "user": {
                "name": "Alice",
                "age": 30
            },
            "status": "active"
        };
        let filter2 = doc! {
            "user": {
                "name": "Bob",
                "age": 25
            },
            "status": "inactive"
        };

        let fp1 = QueryFingerprint::from_document(&filter1);
        let fp2 = QueryFingerprint::from_document(&filter2);

        // Same nested structure should produce same fingerprint
        assert_eq!(fp1, fp2);
    }

    #[test]
    fn test_array_fingerprint() {
        let filter1 = doc! { "tags": ["rust", "mongodb"] };
        let filter2 = doc! { "tags": ["python", "postgres"] };

        let fp1 = QueryFingerprint::from_document(&filter1);
        let fp2 = QueryFingerprint::from_document(&filter2);

        // Arrays with same element types should produce same fingerprint
        assert_eq!(fp1, fp2);
    }
}
