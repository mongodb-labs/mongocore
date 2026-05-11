use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents BSON types for schema inference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BsonType {
    String,
    Int32,
    Int64,
    Double,
    Boolean,
    DateTime,
    Null,
    Document(Vec<SchemaField>),
    Array(Box<BsonType>),
    Binary,
    ObjectId,
}

/// A field in a BSON schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaField {
    pub name: String,
    pub bson_type: BsonType,
    pub nullable: bool,
}

/// Inferred BSON schema for a dataset.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BsonSchema {
    pub fields: Vec<SchemaField>,
}

/// Supported file formats for ingestion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileFormat {
    Auto,
    Csv,
    Json,
    NdJson,
    Parquet,
}

impl Default for FileFormat {
    fn default() -> Self {
        Self::Auto
    }
}

/// Strategy for handling conflicts during ingestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConflictStrategy {
    Skip,
    Overwrite,
    Merge,
}

impl Default for ConflictStrategy {
    fn default() -> Self {
        Self::Skip
    }
}

/// Status of an ingestion job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IngestStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Options for CSV parsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvOptions {
    pub delimiter: Option<u8>,
    pub quote_char: Option<u8>,
    pub has_header: Option<bool>,
    pub comment_char: Option<u8>,
}

impl Default for CsvOptions {
    fn default() -> Self {
        Self {
            delimiter: None,
            quote_char: None,
            has_header: Some(true),
            comment_char: None,
        }
    }
}

/// Options for configuring an ingestion job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestOptions {
    pub file_path: String,
    pub database: String,
    pub collection: String,
    pub format: FileFormat,
    pub dedup_key: Vec<String>,
    pub conflict_strategy: ConflictStrategy,
    pub batch_size: u32,
    pub concurrency: u32,
    pub expressions: Vec<String>,
    pub schema_overrides: HashMap<String, String>,
    pub sample_size: u32,
    pub csv_options: CsvOptions,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            file_path: String::new(),
            database: String::new(),
            collection: String::new(),
            format: FileFormat::default(),
            dedup_key: Vec::new(),
            conflict_strategy: ConflictStrategy::default(),
            batch_size: 1000,
            concurrency: 4,
            expressions: Vec::new(),
            schema_overrides: HashMap::new(),
            sample_size: 1000,
            csv_options: CsvOptions::default(),
        }
    }
}

/// Represents a running or completed ingestion job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestJob {
    pub job_id: String,
    pub file_path: String,
    pub database: String,
    pub collection: String,
    pub status: IngestStatus,
    pub total_rows: i64,
    pub rows_processed: i64,
    pub rows_inserted: i64,
    pub rows_skipped: i64,
    pub rows_failed: i64,
    pub last_committed_chunk: i64,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub inferred_schema: BsonSchema,
}

/// An entry in the dead letter queue for failed rows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterEntry {
    pub job_id: String,
    pub source_row: i64,
    pub document: bson::Document,
    pub error: String,
    pub stage: String,
    pub timestamp: DateTime<Utc>,
}
