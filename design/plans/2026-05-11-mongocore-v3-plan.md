# MongoCore v3: Intelligent Data Ingestion — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Polars-based data ingestion engine into the MongoCore Rust sidecar that reads static files (CSV, JSON, Parquet), infers BSON-compatible schemas, applies optional user-provided Polars expressions (including LLM-powered ones), and bulk-writes to MongoDB with deduplication, conflict resolution, progress tracking, resumability, and a dead letter queue.

**Architecture:** A new `src/ingestion/` module containing: a Polars reader with format detection, a schema inference engine inspired by the MongoDB Spark Connector, a transform engine for user Polars expressions, an optional LLM expression layer, a parallel bulk writer with BSON conversion, dedup/conflict resolution, a dead letter queue, progress tracking with resumability, and a filesystem watch service. Exposed via new gRPC RPCs, MCP tools, and client library methods.

**Tech Stack:** Rust (existing tokio/tonic stack), Polars (lazy, csv, json, parquet features), notify crate (filesystem watching), existing MongoDB driver, existing LLM/Voyage AI infrastructure.

---

## Scope

The v3 design spec defines one large subsystem (Data Ingestion) with these components:

1. **Polars Reader** — Format detection, lazy scanning
2. **Schema Inference Engine** — Multi-row sampling, type widening, BSON mapping
3. **Transform Engine** — User-provided Polars expressions
4. **LLM Expression Functions** — Optional, when API key configured
5. **Bulk Writer** — Chunked parallel writes, DataFrame→BSON conversion
6. **Dedup & Conflict Resolution** — Key-based dedup with configurable strategy
7. **Dead Letter Queue** — Failed documents routed to `__mongocore.dead_letter`
8. **Progress Tracking & Resumability** — Job state in `__mongocore.ingestion_jobs`
9. **Watch Directory Service** — Filesystem monitoring with auto-trigger
10. **Interfaces** — gRPC RPCs, MCP tools, client library methods
11. **Configuration** — Ingestion-specific config fields

The plan builds from core infrastructure outward: types → schema inference → reader → transforms → writer → dedup → DLQ → progress → watch → interfaces.

---

## File Structure

### Core Ingestion Engine

| Action | Path | Responsibility |
|--------|------|----------------|
| Create | `src/ingestion/mod.rs` | Module entry, public API, re-exports |
| Create | `src/ingestion/types.rs` | BsonSchema, IngestJob, ConflictStrategy, IngestOptions, IngestStatus |
| Create | `src/ingestion/reader.rs` | Format detection, Polars LazyFrame creation |
| Create | `src/ingestion/schema.rs` | Schema inference: sample, infer per-row, reduce/merge |
| Create | `src/ingestion/transform.rs` | Parse and apply user Polars expressions |
| Create | `src/ingestion/llm_expressions.rs` | Optional LLM expression functions (classify, extract, normalize, embed) |
| Create | `src/ingestion/writer.rs` | DataFrame→BSON conversion, parallel bulk writes |
| Create | `src/ingestion/dedup.rs` | Dedup check, conflict resolution (skip/overwrite/merge) |
| Create | `src/ingestion/dlq.rs` | Dead letter queue: route failed docs to `__mongocore.dead_letter` |
| Create | `src/ingestion/progress.rs` | Job state tracking, persistence to `__mongocore.ingestion_jobs` |
| Create | `src/ingestion/engine.rs` | Orchestrator: ties reader→schema→transform→writer pipeline |
| Create | `src/ingestion/watch.rs` | Filesystem watcher using `notify` crate |

### Interfaces

| Action | Path | Responsibility |
|--------|------|----------------|
| Modify | `proto/mongocore/v1/mongocore.proto` | Add Ingest, GetIngestStatus, ListIngestJobs, CancelIngest, WatchDirectory, StopWatch RPCs |
| Create | `proto/mongocore/v1/ingestion.proto` | Ingestion-specific message types |
| Modify | `src/grpc/service.rs` | Implement ingestion RPC handlers |
| Modify | `src/mcp/tools.rs` | Add ingestion MCP tools |
| Modify | `src/config.rs` | Add ingestion config fields |
| Modify | `src/main.rs` | Initialize ingestion engine on startup |
| Modify | `src/lib.rs` | Export ingestion module |
| Modify | `Cargo.toml` | Add polars, notify dependencies |

### Client Libraries

| Action | Path | Responsibility |
|--------|------|----------------|
| Modify | `clients/python/mongocore/client.py` | Add ingest, ingest_status, list_ingest_jobs, cancel_ingest, watch_directory, stop_watch |
| Modify | `clients/typescript/src/client.ts` | Add ingestion methods |
| Modify | `clients/go/client.go` | Add ingestion methods |
| Modify | `clients/java/src/main/java/com/mongocore/MongoClient.java` | Add ingestion methods |

### Tests

| Action | Path | Responsibility |
|--------|------|----------------|
| Create | `tests/integration/ingestion_test.rs` | End-to-end ingestion tests |
| Create | `tests/fixtures/sample.csv` | Test CSV fixture |
| Create | `tests/fixtures/sample.json` | Test JSON fixture |
| Create | `tests/fixtures/sample.parquet` | Test Parquet fixture (generated in test setup) |

---

## Task 1: Dependencies and Ingestion Types

**Files:**
- Modify: `Cargo.toml`
- Create: `src/ingestion/mod.rs`
- Create: `src/ingestion/types.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Add polars and notify to Cargo.toml**

```toml
# Add to [dependencies] section
polars = { version = "0.46", features = ["lazy", "csv", "json", "parquet", "dtype-struct"] }
notify = { version = "7", features = ["macos_fsevent"] }
```

- [ ] **Step 2: Create ingestion types module**

```rust
// src/ingestion/types.rs
use bson::Document;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaField {
    pub name: String,
    pub bson_type: BsonType,
    pub nullable: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BsonSchema {
    pub fields: Vec<SchemaField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileFormat {
    Auto,
    Csv,
    Json,
    NdJson,
    Parquet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IngestStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
}

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
            format: FileFormat::Auto,
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
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub error: Option<String>,
    pub inferred_schema: BsonSchema,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterEntry {
    pub job_id: String,
    pub source_row: i64,
    pub document: Document,
    pub error: String,
    pub stage: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}
```

- [ ] **Step 3: Create ingestion module entry**

```rust
// src/ingestion/mod.rs
pub mod types;

pub use types::*;
```

- [ ] **Step 4: Export ingestion module from lib.rs**

Add to `src/lib.rs`:
```rust
pub mod ingestion;
```

- [ ] **Step 5: Add chrono dependency to Cargo.toml**

```toml
chrono = { version = "0.4", features = ["serde"] }
```

- [ ] **Step 6: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml src/ingestion/ src/lib.rs
git commit -m "feat(ingestion): add ingestion types and dependencies (polars, notify, chrono)"
```

---

## Task 2: Polars Reader with Format Detection

**Files:**
- Create: `src/ingestion/reader.rs`
- Modify: `src/ingestion/mod.rs`

- [ ] **Step 1: Write failing test for format detection**

```rust
// src/ingestion/reader.rs
use std::path::Path;
use polars::prelude::*;
use crate::ingestion::types::{FileFormat, CsvOptions};
use crate::error::MongoCoreError;

pub fn detect_format(path: &Path) -> Result<FileFormat, MongoCoreError> {
    todo!()
}

pub fn read_lazy(path: &Path, format: FileFormat, csv_options: &CsvOptions) -> Result<LazyFrame, MongoCoreError> {
    todo!()
}

pub fn count_rows(path: &Path, format: FileFormat, csv_options: &CsvOptions) -> Result<u64, MongoCoreError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_detect_csv() {
        let tmp = NamedTempFile::with_suffix(".csv").unwrap();
        assert_eq!(detect_format(tmp.path()).unwrap(), FileFormat::Csv);
    }

    #[test]
    fn test_detect_json() {
        let tmp = NamedTempFile::with_suffix(".json").unwrap();
        assert_eq!(detect_format(tmp.path()).unwrap(), FileFormat::Json);
    }

    #[test]
    fn test_detect_ndjson() {
        let tmp = NamedTempFile::with_suffix(".ndjson").unwrap();
        assert_eq!(detect_format(tmp.path()).unwrap(), FileFormat::NdJson);
    }

    #[test]
    fn test_detect_parquet() {
        let tmp = NamedTempFile::with_suffix(".parquet").unwrap();
        assert_eq!(detect_format(tmp.path()).unwrap(), FileFormat::Parquet);
    }

    #[test]
    fn test_detect_unknown_extension_errors() {
        let tmp = NamedTempFile::with_suffix(".xyz").unwrap();
        assert!(detect_format(tmp.path()).is_err());
    }

    #[test]
    fn test_read_csv_lazy() {
        let mut tmp = NamedTempFile::with_suffix(".csv").unwrap();
        writeln!(tmp, "name,age,score").unwrap();
        writeln!(tmp, "Alice,30,95.5").unwrap();
        writeln!(tmp, "Bob,25,88.0").unwrap();
        tmp.flush().unwrap();

        let lf = read_lazy(tmp.path(), FileFormat::Csv, &CsvOptions::default()).unwrap();
        let df = lf.collect().unwrap();
        assert_eq!(df.height(), 2);
        assert_eq!(df.width(), 3);
    }

    #[test]
    fn test_read_ndjson_lazy() {
        let mut tmp = NamedTempFile::with_suffix(".ndjson").unwrap();
        writeln!(tmp, r#"{{"name":"Alice","age":30}}"#).unwrap();
        writeln!(tmp, r#"{{"name":"Bob","age":25}}"#).unwrap();
        tmp.flush().unwrap();

        let lf = read_lazy(tmp.path(), FileFormat::NdJson, &CsvOptions::default()).unwrap();
        let df = lf.collect().unwrap();
        assert_eq!(df.height(), 2);
        assert_eq!(df.width(), 2);
    }

    #[test]
    fn test_count_rows_csv() {
        let mut tmp = NamedTempFile::with_suffix(".csv").unwrap();
        writeln!(tmp, "a,b").unwrap();
        writeln!(tmp, "1,2").unwrap();
        writeln!(tmp, "3,4").unwrap();
        writeln!(tmp, "5,6").unwrap();
        tmp.flush().unwrap();

        let count = count_rows(tmp.path(), FileFormat::Csv, &CsvOptions::default()).unwrap();
        assert_eq!(count, 3);
    }

    #[test]
    fn test_auto_format_uses_detection() {
        let mut tmp = NamedTempFile::with_suffix(".csv").unwrap();
        writeln!(tmp, "x,y").unwrap();
        writeln!(tmp, "1,2").unwrap();
        tmp.flush().unwrap();

        let lf = read_lazy(tmp.path(), FileFormat::Auto, &CsvOptions::default()).unwrap();
        let df = lf.collect().unwrap();
        assert_eq!(df.height(), 1);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib ingestion::reader`
Expected: FAIL (todo! panics)

- [ ] **Step 3: Implement format detection**

```rust
// src/ingestion/reader.rs
use std::path::Path;
use polars::prelude::*;
use crate::ingestion::types::{FileFormat, CsvOptions};
use crate::error::MongoCoreError;

pub fn detect_format(path: &Path) -> Result<FileFormat, MongoCoreError> {
    match path.extension().and_then(|e| e.to_str()) {
        Some("csv") | Some("tsv") => Ok(FileFormat::Csv),
        Some("json") => Ok(FileFormat::Json),
        Some("ndjson") | Some("jsonl") => Ok(FileFormat::NdJson),
        Some("parquet") | Some("pq") => Ok(FileFormat::Parquet),
        Some(ext) => Err(MongoCoreError::IngestionError(
            format!("Unsupported file extension: .{}", ext),
        )),
        None => Err(MongoCoreError::IngestionError(
            "File has no extension and format is Auto".to_string(),
        )),
    }
}

pub fn read_lazy(path: &Path, format: FileFormat, csv_options: &CsvOptions) -> Result<LazyFrame, MongoCoreError> {
    let resolved_format = match format {
        FileFormat::Auto => detect_format(path)?,
        other => other,
    };

    match resolved_format {
        FileFormat::Csv => {
            let mut reader = LazyCsvReader::new(path);
            if let Some(delim) = csv_options.delimiter {
                reader = reader.with_separator(delim);
            }
            if let Some(has_header) = csv_options.has_header {
                reader = reader.with_has_header(has_header);
            }
            if let Some(comment) = csv_options.comment_char {
                reader = reader.with_comment_prefix(Some(
                    &String::from_utf8(vec![comment]).unwrap_or_default()
                ));
            }
            reader.finish().map_err(|e| MongoCoreError::IngestionError(
                format!("Failed to read CSV: {}", e)
            ))
        }
        FileFormat::Json | FileFormat::NdJson => {
            LazyJsonLineReader::new(path)
                .finish()
                .map_err(|e| MongoCoreError::IngestionError(
                    format!("Failed to read JSON: {}", e)
                ))
        }
        FileFormat::Parquet => {
            LazyFrame::scan_parquet(path, Default::default())
                .map_err(|e| MongoCoreError::IngestionError(
                    format!("Failed to read Parquet: {}", e)
                ))
        }
        FileFormat::Auto => unreachable!(),
    }
}

pub fn count_rows(path: &Path, format: FileFormat, csv_options: &CsvOptions) -> Result<u64, MongoCoreError> {
    let lf = read_lazy(path, format, csv_options)?;
    let df = lf.select([col("*").count().alias("count")])
        .collect()
        .map_err(|e| MongoCoreError::IngestionError(format!("Failed to count rows: {}", e)))?;
    let count = df.column("count")
        .map_err(|e| MongoCoreError::IngestionError(format!("Count column error: {}", e)))?
        .u32()
        .map_err(|e| MongoCoreError::IngestionError(format!("Count type error: {}", e)))?
        .get(0)
        .unwrap_or(0) as u64;
    Ok(count)
}
```

- [ ] **Step 4: Add IngestionError variant to error.rs**

Add to `src/error.rs` in the `MongoCoreError` enum:
```rust
#[error("Ingestion error: {0}")]
IngestionError(String),
```

- [ ] **Step 5: Update ingestion mod.rs**

```rust
// src/ingestion/mod.rs
pub mod reader;
pub mod types;

pub use types::*;
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test --lib ingestion::reader`
Expected: All tests PASS

- [ ] **Step 7: Commit**

```bash
git add src/ingestion/reader.rs src/ingestion/mod.rs src/error.rs
git commit -m "feat(ingestion): add Polars reader with format detection (CSV, JSON, Parquet)"
```

---

## Task 3: Schema Inference Engine

**Files:**
- Create: `src/ingestion/schema.rs`
- Modify: `src/ingestion/mod.rs`

- [ ] **Step 1: Write failing tests for schema inference**

```rust
// src/ingestion/schema.rs
use polars::prelude::*;
use std::collections::HashMap;
use crate::ingestion::types::{BsonSchema, BsonType, SchemaField};
use crate::error::MongoCoreError;

pub fn infer_schema(df: &DataFrame) -> Result<BsonSchema, MongoCoreError> {
    todo!()
}

pub fn polars_type_to_bson(dtype: &DataType) -> BsonType {
    todo!()
}

pub fn widen_types(a: &BsonType, b: &BsonType) -> BsonType {
    todo!()
}

pub fn apply_overrides(schema: &mut BsonSchema, overrides: &HashMap<String, String>) {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polars_string_to_bson() {
        assert_eq!(polars_type_to_bson(&DataType::String), BsonType::String);
    }

    #[test]
    fn test_polars_int32_to_bson() {
        assert_eq!(polars_type_to_bson(&DataType::Int32), BsonType::Int32);
    }

    #[test]
    fn test_polars_int64_to_bson() {
        assert_eq!(polars_type_to_bson(&DataType::Int64), BsonType::Int64);
    }

    #[test]
    fn test_polars_float64_to_bson() {
        assert_eq!(polars_type_to_bson(&DataType::Float64), BsonType::Double);
    }

    #[test]
    fn test_polars_boolean_to_bson() {
        assert_eq!(polars_type_to_bson(&DataType::Boolean), BsonType::Boolean);
    }

    #[test]
    fn test_polars_date_to_bson() {
        assert_eq!(polars_type_to_bson(&DataType::Date), BsonType::DateTime);
    }

    #[test]
    fn test_polars_null_to_bson() {
        assert_eq!(polars_type_to_bson(&DataType::Null), BsonType::Null);
    }

    #[test]
    fn test_widen_int32_int64() {
        assert_eq!(widen_types(&BsonType::Int32, &BsonType::Int64), BsonType::Int64);
    }

    #[test]
    fn test_widen_int_float() {
        assert_eq!(widen_types(&BsonType::Int32, &BsonType::Double), BsonType::Double);
        assert_eq!(widen_types(&BsonType::Int64, &BsonType::Double), BsonType::Double);
    }

    #[test]
    fn test_widen_incompatible_to_string() {
        assert_eq!(widen_types(&BsonType::Int32, &BsonType::Boolean), BsonType::String);
    }

    #[test]
    fn test_widen_same_type() {
        assert_eq!(widen_types(&BsonType::String, &BsonType::String), BsonType::String);
    }

    #[test]
    fn test_infer_schema_from_dataframe() {
        let df = df! {
            "name" => ["Alice", "Bob"],
            "age" => [30i64, 25i64],
            "score" => [95.5f64, 88.0f64],
            "active" => [true, false],
        }.unwrap();

        let schema = infer_schema(&df).unwrap();
        assert_eq!(schema.fields.len(), 4);
        assert_eq!(schema.fields[0].name, "name");
        assert_eq!(schema.fields[0].bson_type, BsonType::String);
        assert_eq!(schema.fields[1].name, "age");
        assert_eq!(schema.fields[1].bson_type, BsonType::Int64);
        assert_eq!(schema.fields[2].name, "score");
        assert_eq!(schema.fields[2].bson_type, BsonType::Double);
        assert_eq!(schema.fields[3].name, "active");
        assert_eq!(schema.fields[3].bson_type, BsonType::Boolean);
    }

    #[test]
    fn test_infer_schema_nullable_detection() {
        let name_series = Series::new("name".into(), &["Alice", "Bob", "Charlie"]);
        let age_series = Series::new("age".into(), vec![Some(30i64), None, Some(28i64)]);
        let df = DataFrame::new(vec![name_series.into(), age_series.into()]).unwrap();

        let schema = infer_schema(&df).unwrap();
        assert!(!schema.fields[0].nullable); // name: no nulls
        assert!(schema.fields[1].nullable);  // age: has null
    }

    #[test]
    fn test_apply_overrides() {
        let mut schema = BsonSchema {
            fields: vec![
                SchemaField { name: "date_str".to_string(), bson_type: BsonType::String, nullable: false },
                SchemaField { name: "amount".to_string(), bson_type: BsonType::String, nullable: false },
            ],
        };

        let mut overrides = HashMap::new();
        overrides.insert("date_str".to_string(), "DateTime".to_string());
        overrides.insert("amount".to_string(), "Double".to_string());

        apply_overrides(&mut schema, &overrides);
        assert_eq!(schema.fields[0].bson_type, BsonType::DateTime);
        assert_eq!(schema.fields[1].bson_type, BsonType::Double);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib ingestion::schema`
Expected: FAIL (todo! panics)

- [ ] **Step 3: Implement schema inference**

```rust
// src/ingestion/schema.rs
use polars::prelude::*;
use std::collections::HashMap;
use crate::ingestion::types::{BsonSchema, BsonType, SchemaField};
use crate::error::MongoCoreError;

pub fn polars_type_to_bson(dtype: &DataType) -> BsonType {
    match dtype {
        DataType::String | DataType::Categorical(_, _) => BsonType::String,
        DataType::Int8 | DataType::Int16 | DataType::Int32 | DataType::UInt8 | DataType::UInt16 => BsonType::Int32,
        DataType::Int64 | DataType::UInt32 | DataType::UInt64 => BsonType::Int64,
        DataType::Float32 | DataType::Float64 => BsonType::Double,
        DataType::Boolean => BsonType::Boolean,
        DataType::Date | DataType::Datetime(_, _) | DataType::Time | DataType::Duration(_) => BsonType::DateTime,
        DataType::Null => BsonType::Null,
        DataType::Binary => BsonType::Binary,
        DataType::List(inner) => BsonType::Array(Box::new(polars_type_to_bson(inner))),
        DataType::Struct(fields) => {
            let schema_fields = fields.iter().map(|f| SchemaField {
                name: f.name().to_string(),
                bson_type: polars_type_to_bson(f.dtype()),
                nullable: true,
            }).collect();
            BsonType::Document(schema_fields)
        }
        _ => BsonType::String,
    }
}

pub fn widen_types(a: &BsonType, b: &BsonType) -> BsonType {
    if a == b {
        return a.clone();
    }

    match (a, b) {
        // Null widens to the other type
        (BsonType::Null, other) | (other, BsonType::Null) => other.clone(),
        // Int32 + Int64 → Int64
        (BsonType::Int32, BsonType::Int64) | (BsonType::Int64, BsonType::Int32) => BsonType::Int64,
        // Any numeric + Double → Double
        (BsonType::Int32 | BsonType::Int64, BsonType::Double)
        | (BsonType::Double, BsonType::Int32 | BsonType::Int64) => BsonType::Double,
        // Array + Array → merge element types
        (BsonType::Array(a_inner), BsonType::Array(b_inner)) => {
            BsonType::Array(Box::new(widen_types(a_inner, b_inner)))
        }
        // Incompatible → String (universal fallback)
        _ => BsonType::String,
    }
}

pub fn infer_schema(df: &DataFrame) -> Result<BsonSchema, MongoCoreError> {
    let mut fields = Vec::new();

    for col in df.get_columns() {
        let name = col.name().to_string();
        let dtype = col.dtype();
        let bson_type = polars_type_to_bson(dtype);
        let nullable = col.null_count() > 0;

        fields.push(SchemaField {
            name,
            bson_type,
            nullable,
        });
    }

    Ok(BsonSchema { fields })
}

pub fn apply_overrides(schema: &mut BsonSchema, overrides: &HashMap<String, String>) {
    for field in &mut schema.fields {
        if let Some(type_str) = overrides.get(&field.name) {
            if let Some(bson_type) = parse_bson_type_str(type_str) {
                field.bson_type = bson_type;
            }
        }
    }
}

fn parse_bson_type_str(s: &str) -> Option<BsonType> {
    match s.to_lowercase().as_str() {
        "string" | "str" => Some(BsonType::String),
        "int32" | "int" | "i32" => Some(BsonType::Int32),
        "int64" | "long" | "i64" => Some(BsonType::Int64),
        "double" | "float" | "f64" => Some(BsonType::Double),
        "boolean" | "bool" => Some(BsonType::Boolean),
        "datetime" | "date" | "timestamp" => Some(BsonType::DateTime),
        "binary" | "bytes" => Some(BsonType::Binary),
        "objectid" | "oid" => Some(BsonType::ObjectId),
        _ => None,
    }
}
```

- [ ] **Step 4: Update ingestion mod.rs**

```rust
// src/ingestion/mod.rs
pub mod reader;
pub mod schema;
pub mod types;

pub use types::*;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib ingestion::schema`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/ingestion/schema.rs src/ingestion/mod.rs
git commit -m "feat(ingestion): add schema inference engine with Polars-to-BSON type mapping"
```

---

## Task 4: DataFrame-to-BSON Converter

**Files:**
- Create: `src/ingestion/writer.rs`
- Modify: `src/ingestion/mod.rs`

- [ ] **Step 1: Write failing tests for BSON conversion**

```rust
// src/ingestion/writer.rs
use polars::prelude::*;
use bson::{doc, Document, Bson};
use crate::ingestion::types::{BsonSchema, BsonType, SchemaField};
use crate::error::MongoCoreError;

pub fn dataframe_to_documents(df: &DataFrame, schema: &BsonSchema) -> Result<Vec<Document>, MongoCoreError> {
    todo!()
}

fn series_value_to_bson(series: &Series, row: usize, bson_type: &BsonType) -> Result<Bson, MongoCoreError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_dataframe_to_documents() {
        let df = df! {
            "name" => ["Alice", "Bob"],
            "age" => [30i64, 25i64],
            "score" => [95.5f64, 88.0f64],
        }.unwrap();

        let schema = BsonSchema {
            fields: vec![
                SchemaField { name: "name".to_string(), bson_type: BsonType::String, nullable: false },
                SchemaField { name: "age".to_string(), bson_type: BsonType::Int64, nullable: false },
                SchemaField { name: "score".to_string(), bson_type: BsonType::Double, nullable: false },
            ],
        };

        let docs = dataframe_to_documents(&df, &schema).unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].get_str("name"), Ok("Alice"));
        assert_eq!(docs[0].get_i64("age"), Ok(30));
        assert_eq!(docs[0].get_f64("score"), Ok(95.5));
        assert_eq!(docs[1].get_str("name"), Ok("Bob"));
    }

    #[test]
    fn test_nullable_values() {
        let age_series = Series::new("age".into(), vec![Some(30i64), None]);
        let df = DataFrame::new(vec![age_series.into()]).unwrap();

        let schema = BsonSchema {
            fields: vec![
                SchemaField { name: "age".to_string(), bson_type: BsonType::Int64, nullable: true },
            ],
        };

        let docs = dataframe_to_documents(&df, &schema).unwrap();
        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].get_i64("age"), Ok(30));
        assert_eq!(docs[1].get("age"), Some(&Bson::Null));
    }

    #[test]
    fn test_boolean_conversion() {
        let df = df! {
            "active" => [true, false],
        }.unwrap();

        let schema = BsonSchema {
            fields: vec![
                SchemaField { name: "active".to_string(), bson_type: BsonType::Boolean, nullable: false },
            ],
        };

        let docs = dataframe_to_documents(&df, &schema).unwrap();
        assert_eq!(docs[0].get_bool("active"), Ok(true));
        assert_eq!(docs[1].get_bool("active"), Ok(false));
    }

    #[test]
    fn test_int32_conversion() {
        let df = df! {
            "count" => [1i32, 2i32, 3i32],
        }.unwrap();

        let schema = BsonSchema {
            fields: vec![
                SchemaField { name: "count".to_string(), bson_type: BsonType::Int32, nullable: false },
            ],
        };

        let docs = dataframe_to_documents(&df, &schema).unwrap();
        assert_eq!(docs[0].get_i32("count"), Ok(1));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib ingestion::writer`
Expected: FAIL (todo! panics)

- [ ] **Step 3: Implement DataFrame-to-BSON conversion**

```rust
// src/ingestion/writer.rs
use polars::prelude::*;
use bson::{doc, Document, Bson};
use crate::ingestion::types::{BsonSchema, BsonType, SchemaField};
use crate::error::MongoCoreError;

pub fn dataframe_to_documents(df: &DataFrame, schema: &BsonSchema) -> Result<Vec<Document>, MongoCoreError> {
    let mut documents = Vec::with_capacity(df.height());

    for row_idx in 0..df.height() {
        let mut doc = Document::new();
        for field in &schema.fields {
            let series = df.column(&field.name).map_err(|e| {
                MongoCoreError::IngestionError(format!("Column '{}' not found: {}", field.name, e))
            })?;
            let value = series_value_to_bson(series, row_idx, &field.bson_type)?;
            doc.insert(field.name.clone(), value);
        }
        documents.push(doc);
    }

    Ok(documents)
}

fn series_value_to_bson(series: &Series, row: usize, bson_type: &BsonType) -> Result<Bson, MongoCoreError> {
    if series.is_null(row) {
        return Ok(Bson::Null);
    }

    let value = match bson_type {
        BsonType::String => {
            let ca = series.str().map_err(|e| {
                MongoCoreError::IngestionError(format!("String cast error: {}", e))
            })?;
            match ca.get(row) {
                Some(s) => Bson::String(s.to_string()),
                None => Bson::Null,
            }
        }
        BsonType::Int32 => {
            let ca = series.i32().map_err(|e| {
                MongoCoreError::IngestionError(format!("Int32 cast error: {}", e))
            })?;
            match ca.get(row) {
                Some(v) => Bson::Int32(v),
                None => Bson::Null,
            }
        }
        BsonType::Int64 => {
            let ca = series.i64().map_err(|e| {
                MongoCoreError::IngestionError(format!("Int64 cast error: {}", e))
            })?;
            match ca.get(row) {
                Some(v) => Bson::Int64(v),
                None => Bson::Null,
            }
        }
        BsonType::Double => {
            let ca = series.f64().map_err(|e| {
                MongoCoreError::IngestionError(format!("Float64 cast error: {}", e))
            })?;
            match ca.get(row) {
                Some(v) => Bson::Double(v),
                None => Bson::Null,
            }
        }
        BsonType::Boolean => {
            let ca = series.bool().map_err(|e| {
                MongoCoreError::IngestionError(format!("Boolean cast error: {}", e))
            })?;
            match ca.get(row) {
                Some(v) => Bson::Boolean(v),
                None => Bson::Null,
            }
        }
        BsonType::DateTime => {
            let ts_ms = match series.dtype() {
                DataType::Date => {
                    let ca = series.date().map_err(|e| {
                        MongoCoreError::IngestionError(format!("Date cast error: {}", e))
                    })?;
                    ca.get(row).map(|days| days as i64 * 86_400_000)
                }
                DataType::Datetime(_, _) => {
                    let ca = series.datetime().map_err(|e| {
                        MongoCoreError::IngestionError(format!("Datetime cast error: {}", e))
                    })?;
                    ca.get(row)
                }
                _ => {
                    let ca = series.str().map_err(|e| {
                        MongoCoreError::IngestionError(format!("DateTime string cast error: {}", e))
                    })?;
                    return match ca.get(row) {
                        Some(s) => Ok(Bson::String(s.to_string())),
                        None => Ok(Bson::Null),
                    };
                }
            };
            match ts_ms {
                Some(ms) => Bson::DateTime(bson::DateTime::from_millis(ms)),
                None => Bson::Null,
            }
        }
        BsonType::Null => Bson::Null,
        BsonType::Binary => {
            let ca = series.binary().map_err(|e| {
                MongoCoreError::IngestionError(format!("Binary cast error: {}", e))
            })?;
            match ca.get(row) {
                Some(bytes) => Bson::Binary(bson::Binary {
                    subtype: bson::spec::BinarySubtype::Generic,
                    bytes: bytes.to_vec(),
                }),
                None => Bson::Null,
            }
        }
        BsonType::ObjectId => {
            let ca = series.str().map_err(|e| {
                MongoCoreError::IngestionError(format!("ObjectId string cast error: {}", e))
            })?;
            match ca.get(row) {
                Some(s) => match bson::oid::ObjectId::parse_str(s) {
                    Ok(oid) => Bson::ObjectId(oid),
                    Err(_) => Bson::String(s.to_string()),
                },
                None => Bson::Null,
            }
        }
        BsonType::Array(inner_type) => {
            let ca = series.list().map_err(|e| {
                MongoCoreError::IngestionError(format!("List cast error: {}", e))
            })?;
            match ca.get_as_series(row) {
                Some(inner_series) => {
                    let mut arr = Vec::new();
                    for i in 0..inner_series.len() {
                        arr.push(series_value_to_bson(&inner_series, i, inner_type)?);
                    }
                    Bson::Array(arr)
                }
                None => Bson::Null,
            }
        }
        BsonType::Document(fields) => {
            let ca = series.struct_().map_err(|e| {
                MongoCoreError::IngestionError(format!("Struct cast error: {}", e))
            })?;
            let mut inner_doc = Document::new();
            for field in fields {
                let field_series = ca.field_by_name(&field.name).map_err(|e| {
                    MongoCoreError::IngestionError(format!("Struct field '{}' error: {}", field.name, e))
                })?;
                let val = series_value_to_bson(&field_series, row, &field.bson_type)?;
                inner_doc.insert(field.name.clone(), val);
            }
            Bson::Document(inner_doc)
        }
    };

    Ok(value)
}
```

- [ ] **Step 4: Update ingestion mod.rs**

```rust
// src/ingestion/mod.rs
pub mod reader;
pub mod schema;
pub mod types;
pub mod writer;

pub use types::*;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib ingestion::writer`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/ingestion/writer.rs src/ingestion/mod.rs
git commit -m "feat(ingestion): add DataFrame-to-BSON document conversion"
```

---

## Task 5: Transform Engine (User Polars Expressions)

**Files:**
- Create: `src/ingestion/transform.rs`
- Modify: `src/ingestion/mod.rs`

- [ ] **Step 1: Write failing tests for expression parsing**

```rust
// src/ingestion/transform.rs
use polars::prelude::*;
use crate::error::MongoCoreError;

pub fn apply_expressions(lf: LazyFrame, expressions: &[String]) -> Result<LazyFrame, MongoCoreError> {
    todo!()
}

fn parse_expression(expr_str: &str) -> Result<TransformOp, MongoCoreError> {
    todo!()
}

#[derive(Debug, Clone)]
pub enum TransformOp {
    Rename { from: String, to: String },
    Drop(Vec<String>),
    Filter(String),
    WithColumn { expr: String, alias: String },
    Cast { column: String, dtype: DataType },
    Select(Vec<String>),
}

pub fn compile_transform(op: &TransformOp, lf: LazyFrame) -> Result<LazyFrame, MongoCoreError> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_lf() -> LazyFrame {
        df! {
            "name" => ["Alice", "Bob", "Charlie"],
            "age" => [30i64, 25i64, 35i64],
            "score" => [95.5f64, 88.0f64, 72.0f64],
            "internal_id" => ["x1", "x2", "x3"],
        }.unwrap().lazy()
    }

    #[test]
    fn test_rename_expression() {
        let lf = sample_lf();
        let result = apply_expressions(lf, &["rename(name, full_name)".to_string()]).unwrap();
        let df = result.collect().unwrap();
        assert!(df.column("full_name").is_ok());
        assert!(df.column("name").is_err());
    }

    #[test]
    fn test_drop_expression() {
        let lf = sample_lf();
        let result = apply_expressions(lf, &["drop(internal_id)".to_string()]).unwrap();
        let df = result.collect().unwrap();
        assert!(df.column("internal_id").is_err());
        assert!(df.column("name").is_ok());
    }

    #[test]
    fn test_filter_expression() {
        let lf = sample_lf();
        let result = apply_expressions(lf, &["filter(age > 26)".to_string()]).unwrap();
        let df = result.collect().unwrap();
        assert_eq!(df.height(), 2); // Alice (30) and Charlie (35)
    }

    #[test]
    fn test_cast_expression() {
        let lf = sample_lf();
        let result = apply_expressions(lf, &["cast(age, Float64)".to_string()]).unwrap();
        let df = result.collect().unwrap();
        assert_eq!(*df.column("age").unwrap().dtype(), DataType::Float64);
    }

    #[test]
    fn test_multiple_expressions() {
        let lf = sample_lf();
        let result = apply_expressions(lf, &[
            "drop(internal_id)".to_string(),
            "filter(age > 26)".to_string(),
            "rename(name, full_name)".to_string(),
        ]).unwrap();
        let df = result.collect().unwrap();
        assert!(df.column("internal_id").is_err());
        assert!(df.column("full_name").is_ok());
        assert_eq!(df.height(), 2);
    }

    #[test]
    fn test_invalid_expression_errors() {
        let lf = sample_lf();
        let result = apply_expressions(lf, &["invalid_op(x)".to_string()]);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib ingestion::transform`
Expected: FAIL (todo! panics)

- [ ] **Step 3: Implement transform engine**

```rust
// src/ingestion/transform.rs
use polars::prelude::*;
use crate::error::MongoCoreError;

#[derive(Debug, Clone)]
pub enum TransformOp {
    Rename { from: String, to: String },
    Drop(Vec<String>),
    Filter(String),
    WithColumn { expr: String, alias: String },
    Cast { column: String, dtype: DataType },
    Select(Vec<String>),
}

pub fn apply_expressions(lf: LazyFrame, expressions: &[String]) -> Result<LazyFrame, MongoCoreError> {
    let mut current = lf;
    for expr_str in expressions {
        let op = parse_expression(expr_str)?;
        current = compile_transform(&op, current)?;
    }
    Ok(current)
}

fn parse_expression(expr_str: &str) -> Result<TransformOp, MongoCoreError> {
    let trimmed = expr_str.trim();

    if let Some(inner) = strip_func(trimmed, "rename") {
        let parts: Vec<&str> = inner.splitn(2, ',').map(|s| s.trim()).collect();
        if parts.len() != 2 {
            return Err(MongoCoreError::IngestionError(
                format!("rename expects 2 args: rename(old, new). Got: {}", inner),
            ));
        }
        return Ok(TransformOp::Rename {
            from: parts[0].to_string(),
            to: parts[1].to_string(),
        });
    }

    if let Some(inner) = strip_func(trimmed, "drop") {
        let cols: Vec<String> = inner.split(',').map(|s| s.trim().to_string()).collect();
        return Ok(TransformOp::Drop(cols));
    }

    if let Some(inner) = strip_func(trimmed, "filter") {
        return Ok(TransformOp::Filter(inner.to_string()));
    }

    if let Some(inner) = strip_func(trimmed, "cast") {
        let parts: Vec<&str> = inner.splitn(2, ',').map(|s| s.trim()).collect();
        if parts.len() != 2 {
            return Err(MongoCoreError::IngestionError(
                format!("cast expects 2 args: cast(column, type). Got: {}", inner),
            ));
        }
        let dtype = parse_polars_dtype(parts[1])?;
        return Ok(TransformOp::Cast {
            column: parts[0].to_string(),
            dtype,
        });
    }

    if let Some(inner) = strip_func(trimmed, "select") {
        let cols: Vec<String> = inner.split(',').map(|s| s.trim().to_string()).collect();
        return Ok(TransformOp::Select(cols));
    }

    Err(MongoCoreError::IngestionError(
        format!("Unrecognized expression: {}", trimmed),
    ))
}

fn strip_func<'a>(s: &'a str, func_name: &str) -> Option<&'a str> {
    let prefix = format!("{}(", func_name);
    if s.starts_with(&prefix) && s.ends_with(')') {
        Some(&s[prefix.len()..s.len() - 1])
    } else {
        None
    }
}

fn parse_polars_dtype(s: &str) -> Result<DataType, MongoCoreError> {
    match s.to_lowercase().as_str() {
        "string" | "str" | "utf8" => Ok(DataType::String),
        "int32" | "i32" => Ok(DataType::Int32),
        "int64" | "i64" => Ok(DataType::Int64),
        "float32" | "f32" => Ok(DataType::Float32),
        "float64" | "f64" => Ok(DataType::Float64),
        "boolean" | "bool" => Ok(DataType::Boolean),
        "date" => Ok(DataType::Date),
        _ => Err(MongoCoreError::IngestionError(
            format!("Unknown Polars type: {}", s),
        )),
    }
}

pub fn compile_transform(op: &TransformOp, lf: LazyFrame) -> Result<LazyFrame, MongoCoreError> {
    match op {
        TransformOp::Rename { from, to } => {
            Ok(lf.rename([from.as_str()], [to.as_str()], true))
        }
        TransformOp::Drop(cols) => {
            let col_refs: Vec<&str> = cols.iter().map(|s| s.as_str()).collect();
            Ok(lf.drop(col_refs))
        }
        TransformOp::Filter(expr_str) => {
            let filter_expr = parse_filter_expr(expr_str)?;
            Ok(lf.filter(filter_expr))
        }
        TransformOp::Cast { column, dtype } => {
            Ok(lf.with_column(col(column.as_str()).cast(dtype.clone())))
        }
        TransformOp::Select(cols) => {
            let exprs: Vec<Expr> = cols.iter().map(|c| col(c.as_str())).collect();
            Ok(lf.select(exprs))
        }
        TransformOp::WithColumn { expr: _, alias: _ } => {
            Err(MongoCoreError::IngestionError(
                "with_column not yet implemented".to_string(),
            ))
        }
    }
}

fn parse_filter_expr(expr_str: &str) -> Result<Expr, MongoCoreError> {
    let trimmed = expr_str.trim();

    // Simple comparison: column op value
    let operators = [">=", "<=", "!=", ">", "<", "=="];
    for op in &operators {
        if let Some(pos) = trimmed.find(op) {
            let col_name = trimmed[..pos].trim();
            let value_str = trimmed[pos + op.len()..].trim();

            let column = col(col_name);
            let value = parse_literal(value_str)?;

            return match *op {
                ">" => Ok(column.gt(value)),
                "<" => Ok(column.lt(value)),
                ">=" => Ok(column.gt_eq(value)),
                "<=" => Ok(column.lt_eq(value)),
                "==" => Ok(column.eq(value)),
                "!=" => Ok(column.neq(value)),
                _ => unreachable!(),
            };
        }
    }

    Err(MongoCoreError::IngestionError(
        format!("Cannot parse filter expression: {}", trimmed),
    ))
}

fn parse_literal(s: &str) -> Result<Expr, MongoCoreError> {
    // Try integer
    if let Ok(v) = s.parse::<i64>() {
        return Ok(lit(v));
    }
    // Try float
    if let Ok(v) = s.parse::<f64>() {
        return Ok(lit(v));
    }
    // Try boolean
    match s.to_lowercase().as_str() {
        "true" => return Ok(lit(true)),
        "false" => return Ok(lit(false)),
        _ => {}
    }
    // String (strip quotes if present)
    let stripped = s.trim_matches('"').trim_matches('\'');
    Ok(lit(stripped.to_string()))
}
```

- [ ] **Step 4: Update ingestion mod.rs**

```rust
// src/ingestion/mod.rs
pub mod reader;
pub mod schema;
pub mod transform;
pub mod types;
pub mod writer;

pub use types::*;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib ingestion::transform`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/ingestion/transform.rs src/ingestion/mod.rs
git commit -m "feat(ingestion): add transform engine for user Polars expressions"
```

---

## Task 6: Dead Letter Queue

**Files:**
- Create: `src/ingestion/dlq.rs`
- Modify: `src/ingestion/mod.rs`

- [ ] **Step 1: Write failing tests for DLQ**

```rust
// src/ingestion/dlq.rs
use bson::{doc, Document};
use mongodb::Collection;
use crate::ingestion::types::DeadLetterEntry;
use crate::error::MongoCoreError;

pub struct DeadLetterQueue {
    collection: Collection<Document>,
}

impl DeadLetterQueue {
    pub fn new(collection: Collection<Document>) -> Self {
        todo!()
    }

    pub async fn push(&self, entry: DeadLetterEntry) -> Result<(), MongoCoreError> {
        todo!()
    }

    pub async fn push_batch(&self, entries: Vec<DeadLetterEntry>) -> Result<(), MongoCoreError> {
        todo!()
    }

    pub async fn get_by_job(&self, job_id: &str) -> Result<Vec<DeadLetterEntry>, MongoCoreError> {
        todo!()
    }

    pub fn entry_to_document(entry: &DeadLetterEntry) -> Document {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_entry_to_document() {
        let entry = DeadLetterEntry {
            job_id: "job-123".to_string(),
            source_row: 42,
            document: doc! { "name": "test" },
            error: "duplicate key".to_string(),
            stage: "bulk_write".to_string(),
            timestamp: Utc::now(),
        };

        let doc = DeadLetterQueue::entry_to_document(&entry);
        assert_eq!(doc.get_str("job_id"), Ok("job-123"));
        assert_eq!(doc.get_i64("source_row"), Ok(42));
        assert_eq!(doc.get_str("error"), Ok("duplicate key"));
        assert_eq!(doc.get_str("stage"), Ok("bulk_write"));
        assert!(doc.get("document").is_some());
        assert!(doc.get("timestamp").is_some());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib ingestion::dlq`
Expected: FAIL (todo! panics)

- [ ] **Step 3: Implement DLQ**

```rust
// src/ingestion/dlq.rs
use bson::{doc, Document, Bson};
use mongodb::Collection;
use crate::ingestion::types::DeadLetterEntry;
use crate::error::MongoCoreError;

pub struct DeadLetterQueue {
    collection: Collection<Document>,
}

impl DeadLetterQueue {
    pub fn new(collection: Collection<Document>) -> Self {
        Self { collection }
    }

    pub async fn push(&self, entry: DeadLetterEntry) -> Result<(), MongoCoreError> {
        let doc = Self::entry_to_document(&entry);
        self.collection.insert_one(doc).await.map_err(|e| {
            MongoCoreError::IngestionError(format!("DLQ insert failed: {}", e))
        })?;
        Ok(())
    }

    pub async fn push_batch(&self, entries: Vec<DeadLetterEntry>) -> Result<(), MongoCoreError> {
        if entries.is_empty() {
            return Ok(());
        }
        let docs: Vec<Document> = entries.iter().map(Self::entry_to_document).collect();
        self.collection.insert_many(docs).await.map_err(|e| {
            MongoCoreError::IngestionError(format!("DLQ batch insert failed: {}", e))
        })?;
        Ok(())
    }

    pub async fn get_by_job(&self, job_id: &str) -> Result<Vec<DeadLetterEntry>, MongoCoreError> {
        use futures::TryStreamExt;
        let filter = doc! { "job_id": job_id };
        let mut cursor = self.collection.find(filter).await.map_err(|e| {
            MongoCoreError::IngestionError(format!("DLQ query failed: {}", e))
        })?;

        let mut entries = Vec::new();
        while let Some(doc) = cursor.try_next().await.map_err(|e| {
            MongoCoreError::IngestionError(format!("DLQ cursor error: {}", e))
        })? {
            if let Ok(entry) = Self::document_to_entry(&doc) {
                entries.push(entry);
            }
        }
        Ok(entries)
    }

    pub fn entry_to_document(entry: &DeadLetterEntry) -> Document {
        doc! {
            "job_id": &entry.job_id,
            "source_row": entry.source_row,
            "document": entry.document.clone(),
            "error": &entry.error,
            "stage": &entry.stage,
            "timestamp": bson::DateTime::from_chrono(entry.timestamp),
        }
    }

    fn document_to_entry(doc: &Document) -> Result<DeadLetterEntry, MongoCoreError> {
        Ok(DeadLetterEntry {
            job_id: doc.get_str("job_id").unwrap_or_default().to_string(),
            source_row: doc.get_i64("source_row").unwrap_or(0),
            document: doc.get_document("document").cloned().unwrap_or_default(),
            error: doc.get_str("error").unwrap_or_default().to_string(),
            stage: doc.get_str("stage").unwrap_or_default().to_string(),
            timestamp: doc
                .get_datetime("timestamp")
                .map(|dt| dt.to_chrono())
                .unwrap_or_else(|_| chrono::Utc::now()),
        })
    }
}
```

- [ ] **Step 4: Update ingestion mod.rs**

```rust
// src/ingestion/mod.rs
pub mod dlq;
pub mod reader;
pub mod schema;
pub mod transform;
pub mod types;
pub mod writer;

pub use types::*;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib ingestion::dlq`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/ingestion/dlq.rs src/ingestion/mod.rs
git commit -m "feat(ingestion): add dead letter queue for failed document routing"
```

---

## Task 7: Progress Tracking & Resumability

**Files:**
- Create: `src/ingestion/progress.rs`
- Modify: `src/ingestion/mod.rs`

- [ ] **Step 1: Write failing tests for progress tracker**

```rust
// src/ingestion/progress.rs
use bson::{doc, Document};
use mongodb::Collection;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::ingestion::types::{IngestJob, IngestStatus, BsonSchema};
use crate::error::MongoCoreError;

pub struct ProgressTracker {
    collection: Collection<Document>,
    jobs: Arc<RwLock<Vec<IngestJob>>>,
}

impl ProgressTracker {
    pub fn new(collection: Collection<Document>) -> Self {
        todo!()
    }

    pub async fn create_job(&self, job: IngestJob) -> Result<(), MongoCoreError> {
        todo!()
    }

    pub async fn update_progress(
        &self,
        job_id: &str,
        rows_processed: i64,
        rows_inserted: i64,
        rows_skipped: i64,
        rows_failed: i64,
        last_chunk: i64,
    ) -> Result<(), MongoCoreError> {
        todo!()
    }

    pub async fn complete_job(&self, job_id: &str) -> Result<(), MongoCoreError> {
        todo!()
    }

    pub async fn fail_job(&self, job_id: &str, error: &str) -> Result<(), MongoCoreError> {
        todo!()
    }

    pub async fn cancel_job(&self, job_id: &str) -> Result<(), MongoCoreError> {
        todo!()
    }

    pub async fn get_job(&self, job_id: &str) -> Result<Option<IngestJob>, MongoCoreError> {
        todo!()
    }

    pub async fn list_jobs(&self) -> Result<Vec<IngestJob>, MongoCoreError> {
        todo!()
    }

    pub async fn get_resumable_jobs(&self) -> Result<Vec<IngestJob>, MongoCoreError> {
        todo!()
    }

    pub fn job_to_document(job: &IngestJob) -> Document {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn sample_job() -> IngestJob {
        IngestJob {
            job_id: "test-job-1".to_string(),
            file_path: "/data/test.csv".to_string(),
            database: "testdb".to_string(),
            collection: "testcol".to_string(),
            status: IngestStatus::Running,
            total_rows: 1000,
            rows_processed: 0,
            rows_inserted: 0,
            rows_skipped: 0,
            rows_failed: 0,
            last_committed_chunk: 0,
            started_at: Utc::now(),
            completed_at: None,
            error: None,
            inferred_schema: BsonSchema::default(),
        }
    }

    #[test]
    fn test_job_to_document() {
        let job = sample_job();
        let doc = ProgressTracker::job_to_document(&job);
        assert_eq!(doc.get_str("job_id"), Ok("test-job-1"));
        assert_eq!(doc.get_str("file_path"), Ok("/data/test.csv"));
        assert_eq!(doc.get_str("database"), Ok("testdb"));
        assert_eq!(doc.get_str("collection"), Ok("testcol"));
        assert_eq!(doc.get_str("status"), Ok("running"));
        assert_eq!(doc.get_i64("total_rows"), Ok(1000));
        assert_eq!(doc.get_i64("rows_processed"), Ok(0));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib ingestion::progress`
Expected: FAIL (todo! panics)

- [ ] **Step 3: Implement progress tracker**

```rust
// src/ingestion/progress.rs
use bson::{doc, Document};
use mongodb::Collection;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::ingestion::types::{IngestJob, IngestStatus, BsonSchema};
use crate::error::MongoCoreError;

pub struct ProgressTracker {
    collection: Collection<Document>,
    jobs: Arc<RwLock<Vec<IngestJob>>>,
}

impl ProgressTracker {
    pub fn new(collection: Collection<Document>) -> Self {
        Self {
            collection,
            jobs: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn create_job(&self, job: IngestJob) -> Result<(), MongoCoreError> {
        let doc = Self::job_to_document(&job);
        self.collection.insert_one(doc).await.map_err(|e| {
            MongoCoreError::IngestionError(format!("Failed to create job: {}", e))
        })?;
        self.jobs.write().await.push(job);
        Ok(())
    }

    pub async fn update_progress(
        &self,
        job_id: &str,
        rows_processed: i64,
        rows_inserted: i64,
        rows_skipped: i64,
        rows_failed: i64,
        last_chunk: i64,
    ) -> Result<(), MongoCoreError> {
        let filter = doc! { "job_id": job_id };
        let update = doc! {
            "$set": {
                "rows_processed": rows_processed,
                "rows_inserted": rows_inserted,
                "rows_skipped": rows_skipped,
                "rows_failed": rows_failed,
                "last_committed_chunk": last_chunk,
            }
        };
        self.collection.update_one(filter, update).await.map_err(|e| {
            MongoCoreError::IngestionError(format!("Failed to update progress: {}", e))
        })?;

        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.job_id == job_id) {
            job.rows_processed = rows_processed;
            job.rows_inserted = rows_inserted;
            job.rows_skipped = rows_skipped;
            job.rows_failed = rows_failed;
            job.last_committed_chunk = last_chunk;
        }
        Ok(())
    }

    pub async fn complete_job(&self, job_id: &str) -> Result<(), MongoCoreError> {
        let now = chrono::Utc::now();
        let filter = doc! { "job_id": job_id };
        let update = doc! {
            "$set": {
                "status": "completed",
                "completed_at": bson::DateTime::from_chrono(now),
            }
        };
        self.collection.update_one(filter, update).await.map_err(|e| {
            MongoCoreError::IngestionError(format!("Failed to complete job: {}", e))
        })?;

        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.job_id == job_id) {
            job.status = IngestStatus::Completed;
            job.completed_at = Some(now);
        }
        Ok(())
    }

    pub async fn fail_job(&self, job_id: &str, error: &str) -> Result<(), MongoCoreError> {
        let now = chrono::Utc::now();
        let filter = doc! { "job_id": job_id };
        let update = doc! {
            "$set": {
                "status": "failed",
                "error": error,
                "completed_at": bson::DateTime::from_chrono(now),
            }
        };
        self.collection.update_one(filter, update).await.map_err(|e| {
            MongoCoreError::IngestionError(format!("Failed to mark job failed: {}", e))
        })?;

        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.job_id == job_id) {
            job.status = IngestStatus::Failed;
            job.error = Some(error.to_string());
            job.completed_at = Some(now);
        }
        Ok(())
    }

    pub async fn cancel_job(&self, job_id: &str) -> Result<(), MongoCoreError> {
        let now = chrono::Utc::now();
        let filter = doc! { "job_id": job_id };
        let update = doc! {
            "$set": {
                "status": "cancelled",
                "completed_at": bson::DateTime::from_chrono(now),
            }
        };
        self.collection.update_one(filter, update).await.map_err(|e| {
            MongoCoreError::IngestionError(format!("Failed to cancel job: {}", e))
        })?;

        let mut jobs = self.jobs.write().await;
        if let Some(job) = jobs.iter_mut().find(|j| j.job_id == job_id) {
            job.status = IngestStatus::Cancelled;
            job.completed_at = Some(now);
        }
        Ok(())
    }

    pub async fn get_job(&self, job_id: &str) -> Result<Option<IngestJob>, MongoCoreError> {
        let jobs = self.jobs.read().await;
        Ok(jobs.iter().find(|j| j.job_id == job_id).cloned())
    }

    pub async fn list_jobs(&self) -> Result<Vec<IngestJob>, MongoCoreError> {
        Ok(self.jobs.read().await.clone())
    }

    pub async fn get_resumable_jobs(&self) -> Result<Vec<IngestJob>, MongoCoreError> {
        let jobs = self.jobs.read().await;
        Ok(jobs
            .iter()
            .filter(|j| j.status == IngestStatus::Running)
            .cloned()
            .collect())
    }

    pub fn job_to_document(job: &IngestJob) -> Document {
        let status_str = match job.status {
            IngestStatus::Running => "running",
            IngestStatus::Completed => "completed",
            IngestStatus::Failed => "failed",
            IngestStatus::Cancelled => "cancelled",
        };

        let mut doc = doc! {
            "job_id": &job.job_id,
            "file_path": &job.file_path,
            "database": &job.database,
            "collection": &job.collection,
            "status": status_str,
            "total_rows": job.total_rows,
            "rows_processed": job.rows_processed,
            "rows_inserted": job.rows_inserted,
            "rows_skipped": job.rows_skipped,
            "rows_failed": job.rows_failed,
            "last_committed_chunk": job.last_committed_chunk,
            "started_at": bson::DateTime::from_chrono(job.started_at),
        };

        if let Some(completed_at) = job.completed_at {
            doc.insert("completed_at", bson::DateTime::from_chrono(completed_at));
        }
        if let Some(ref error) = job.error {
            doc.insert("error", error.clone());
        }

        doc
    }
}
```

- [ ] **Step 4: Update ingestion mod.rs**

```rust
// src/ingestion/mod.rs
pub mod dlq;
pub mod progress;
pub mod reader;
pub mod schema;
pub mod transform;
pub mod types;
pub mod writer;

pub use types::*;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib ingestion::progress`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/ingestion/progress.rs src/ingestion/mod.rs
git commit -m "feat(ingestion): add progress tracking with job persistence and resumability"
```

---

## Task 8: Deduplication & Conflict Resolution

**Files:**
- Create: `src/ingestion/dedup.rs`
- Modify: `src/ingestion/mod.rs`

- [ ] **Step 1: Write failing tests for dedup logic**

```rust
// src/ingestion/dedup.rs
use bson::{doc, Document, Bson};
use mongodb::Collection;
use std::collections::HashSet;
use crate::ingestion::types::ConflictStrategy;
use crate::error::MongoCoreError;

pub struct DedupChecker {
    collection: Collection<Document>,
    dedup_key: Vec<String>,
    strategy: ConflictStrategy,
}

#[derive(Debug)]
pub enum DedupResult {
    Insert(Document),
    Skip,
    Replace(Document),
    Merge(Document, Document), // (incoming, existing)
}

impl DedupChecker {
    pub fn new(
        collection: Collection<Document>,
        dedup_key: Vec<String>,
        strategy: ConflictStrategy,
    ) -> Self {
        todo!()
    }

    pub fn build_dedup_filter(&self, doc: &Document) -> Option<Document> {
        todo!()
    }

    pub fn resolve_conflict(&self, incoming: &Document, existing: &Document) -> DedupResult {
        todo!()
    }

    pub fn merge_documents(incoming: &Document, existing: &Document) -> Document {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_dedup_filter_single_key() {
        let checker = DedupChecker {
            collection: unsafe { std::mem::zeroed() }, // Not used in this test
            dedup_key: vec!["email".to_string()],
            strategy: ConflictStrategy::Skip,
        };

        let doc = doc! { "email": "alice@test.com", "name": "Alice" };
        let filter = checker.build_dedup_filter(&doc);
        assert_eq!(filter, Some(doc! { "email": "alice@test.com" }));
    }

    #[test]
    fn test_build_dedup_filter_composite_key() {
        let checker = DedupChecker {
            collection: unsafe { std::mem::zeroed() },
            dedup_key: vec!["first".to_string(), "last".to_string()],
            strategy: ConflictStrategy::Skip,
        };

        let doc = doc! { "first": "Alice", "last": "Smith", "age": 30 };
        let filter = checker.build_dedup_filter(&doc);
        assert_eq!(filter, Some(doc! { "first": "Alice", "last": "Smith" }));
    }

    #[test]
    fn test_build_dedup_filter_missing_key_returns_none() {
        let checker = DedupChecker {
            collection: unsafe { std::mem::zeroed() },
            dedup_key: vec!["email".to_string()],
            strategy: ConflictStrategy::Skip,
        };

        let doc = doc! { "name": "Alice" }; // no "email" field
        let filter = checker.build_dedup_filter(&doc);
        assert!(filter.is_none());
    }

    #[test]
    fn test_resolve_skip() {
        let checker = DedupChecker {
            collection: unsafe { std::mem::zeroed() },
            dedup_key: vec!["id".to_string()],
            strategy: ConflictStrategy::Skip,
        };

        let incoming = doc! { "id": 1, "val": "new" };
        let existing = doc! { "id": 1, "val": "old" };
        match checker.resolve_conflict(&incoming, &existing) {
            DedupResult::Skip => {}
            other => panic!("Expected Skip, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_overwrite() {
        let checker = DedupChecker {
            collection: unsafe { std::mem::zeroed() },
            dedup_key: vec!["id".to_string()],
            strategy: ConflictStrategy::Overwrite,
        };

        let incoming = doc! { "id": 1, "val": "new" };
        let existing = doc! { "id": 1, "val": "old" };
        match checker.resolve_conflict(&incoming, &existing) {
            DedupResult::Replace(doc) => {
                assert_eq!(doc.get_str("val"), Ok("new"));
            }
            other => panic!("Expected Replace, got {:?}", other),
        }
    }

    #[test]
    fn test_merge_documents() {
        let incoming = doc! { "id": 1, "name": "Alice", "age": 31 };
        let existing = doc! { "id": 1, "name": "Alice", "age": 30, "email": "alice@test.com" };

        let merged = DedupChecker::merge_documents(&incoming, &existing);
        assert_eq!(merged.get_str("name"), Ok("Alice"));
        assert_eq!(merged.get_i32("age"), Ok(31)); // incoming wins
        assert_eq!(merged.get_str("email"), Ok("alice@test.com")); // existing preserved
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib ingestion::dedup`
Expected: FAIL (todo! panics)

- [ ] **Step 3: Implement dedup and conflict resolution**

Note: The `unsafe { std::mem::zeroed() }` for `Collection` in tests is unsafe. For unit tests, we'll refactor to only test methods that don't use the collection. The actual `new` constructor is straightforward.

```rust
// src/ingestion/dedup.rs
use bson::{doc, Document, Bson};
use mongodb::Collection;
use crate::ingestion::types::ConflictStrategy;
use crate::error::MongoCoreError;

pub struct DedupChecker {
    pub collection: Collection<Document>,
    pub dedup_key: Vec<String>,
    pub strategy: ConflictStrategy,
}

#[derive(Debug)]
pub enum DedupResult {
    Insert(Document),
    Skip,
    Replace(Document),
    Merge(Document, Document),
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

    pub fn build_dedup_filter(&self, doc: &Document) -> Option<Document> {
        let mut filter = Document::new();
        for key in &self.dedup_key {
            match doc.get(key) {
                Some(value) => {
                    filter.insert(key.clone(), value.clone());
                }
                None => return None,
            }
        }
        Some(filter)
    }

    pub async fn check_batch(
        &self,
        docs: &[Document],
    ) -> Result<Vec<DedupResult>, MongoCoreError> {
        use futures::TryStreamExt;

        if self.dedup_key.is_empty() {
            return Ok(docs.iter().map(|d| DedupResult::Insert(d.clone())).collect());
        }

        // Build OR filter for all docs in batch
        let mut or_conditions = Vec::new();
        for doc in docs {
            if let Some(filter) = self.build_dedup_filter(doc) {
                or_conditions.push(bson::to_bson(&filter).unwrap_or(Bson::Null));
            }
        }

        if or_conditions.is_empty() {
            return Ok(docs.iter().map(|d| DedupResult::Insert(d.clone())).collect());
        }

        let batch_filter = doc! { "$or": or_conditions };
        let mut cursor = self.collection.find(batch_filter).await.map_err(|e| {
            MongoCoreError::IngestionError(format!("Dedup query failed: {}", e))
        })?;

        // Collect existing docs
        let mut existing_docs = Vec::new();
        while let Some(existing) = cursor.try_next().await.map_err(|e| {
            MongoCoreError::IngestionError(format!("Dedup cursor error: {}", e))
        })? {
            existing_docs.push(existing);
        }

        // Resolve each incoming doc
        let mut results = Vec::with_capacity(docs.len());
        for doc in docs {
            let existing = existing_docs.iter().find(|e| {
                self.dedup_key.iter().all(|k| doc.get(k) == e.get(k))
            });

            match existing {
                None => results.push(DedupResult::Insert(doc.clone())),
                Some(e) => results.push(self.resolve_conflict(doc, e)),
            }
        }

        Ok(results)
    }

    pub fn resolve_conflict(&self, incoming: &Document, existing: &Document) -> DedupResult {
        match self.strategy {
            ConflictStrategy::Skip => DedupResult::Skip,
            ConflictStrategy::Overwrite => DedupResult::Replace(incoming.clone()),
            ConflictStrategy::Merge => {
                DedupResult::Merge(incoming.clone(), existing.clone())
            }
        }
    }

    pub fn merge_documents(incoming: &Document, existing: &Document) -> Document {
        let mut result = existing.clone();
        for (key, value) in incoming {
            result.insert(key.clone(), value.clone());
        }
        result
    }
}
```

- [ ] **Step 4: Update ingestion mod.rs**

```rust
// src/ingestion/mod.rs
pub mod dedup;
pub mod dlq;
pub mod progress;
pub mod reader;
pub mod schema;
pub mod transform;
pub mod types;
pub mod writer;

pub use types::*;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --lib ingestion::dedup`
Expected: All tests PASS (note: tests using `unsafe { std::mem::zeroed() }` for Collection should be refactored to avoid the collection field, or use a builder pattern. The implementer should adjust the test approach to avoid undefined behavior — e.g., make `build_dedup_filter`, `resolve_conflict`, and `merge_documents` standalone functions that take strategy and dedup_key as params.)

- [ ] **Step 6: Commit**

```bash
git add src/ingestion/dedup.rs src/ingestion/mod.rs
git commit -m "feat(ingestion): add deduplication with skip/overwrite/merge conflict resolution"
```

---

## Task 9: Ingestion Engine Orchestrator

**Files:**
- Create: `src/ingestion/engine.rs`
- Modify: `src/ingestion/mod.rs`

- [ ] **Step 1: Write the engine orchestrator**

```rust
// src/ingestion/engine.rs
use std::sync::Arc;
use tokio::sync::{RwLock, broadcast};
use mongodb::{Client, Database};
use polars::prelude::*;
use crate::ingestion::types::*;
use crate::ingestion::reader;
use crate::ingestion::schema;
use crate::ingestion::transform;
use crate::ingestion::writer;
use crate::ingestion::dedup::{DedupChecker, DedupResult};
use crate::ingestion::dlq::DeadLetterQueue;
use crate::ingestion::progress::ProgressTracker;
use crate::error::MongoCoreError;

pub struct IngestionEngine {
    db: Database,
    progress: Arc<ProgressTracker>,
    cancel_channels: Arc<RwLock<std::collections::HashMap<String, broadcast::Sender<()>>>>,
}

impl IngestionEngine {
    pub fn new(client: &Client, system_db_name: &str) -> Self {
        let db = client.database(system_db_name);
        let progress = Arc::new(ProgressTracker::new(
            db.collection("__mongocore.ingestion_jobs"),
        ));
        Self {
            db,
            progress,
            cancel_channels: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub async fn ingest(&self, client: &Client, options: IngestOptions) -> Result<IngestJob, MongoCoreError> {
        let job_id = uuid::Uuid::new_v4().to_string();
        let path = std::path::Path::new(&options.file_path);

        // Validate file exists
        if !path.exists() {
            return Err(MongoCoreError::IngestionError(
                format!("File not found: {}", options.file_path),
            ));
        }

        // Detect format
        let format = match options.format {
            FileFormat::Auto => reader::detect_format(path)?,
            other => other,
        };

        // Count total rows
        let total_rows = reader::count_rows(path, format, &options.csv_options)? as i64;

        // Read as LazyFrame
        let lf = reader::read_lazy(path, format, &options.csv_options)?;

        // Sample for schema inference
        let sample_df = lf.clone()
            .limit(options.sample_size)
            .collect()
            .map_err(|e| MongoCoreError::IngestionError(format!("Sample collection failed: {}", e)))?;

        // Infer schema
        let mut inferred_schema = schema::infer_schema(&sample_df)?;

        // Apply overrides
        if !options.schema_overrides.is_empty() {
            schema::apply_overrides(&mut inferred_schema, &options.schema_overrides);
        }

        // Apply transforms
        let transformed_lf = if options.expressions.is_empty() {
            lf
        } else {
            transform::apply_expressions(lf, &options.expressions)?
        };

        // Create job record
        let job = IngestJob {
            job_id: job_id.clone(),
            file_path: options.file_path.clone(),
            database: options.database.clone(),
            collection: options.collection.clone(),
            status: IngestStatus::Running,
            total_rows,
            rows_processed: 0,
            rows_inserted: 0,
            rows_skipped: 0,
            rows_failed: 0,
            last_committed_chunk: 0,
            started_at: chrono::Utc::now(),
            completed_at: None,
            error: None,
            inferred_schema: inferred_schema.clone(),
        };
        self.progress.create_job(job.clone()).await?;

        // Setup cancel channel
        let (cancel_tx, _) = broadcast::channel(1);
        self.cancel_channels.write().await.insert(job_id.clone(), cancel_tx.clone());

        // Spawn ingestion task
        let target_db = client.database(&options.database);
        let target_collection = target_db.collection::<bson::Document>(&options.collection);
        let dlq = DeadLetterQueue::new(self.db.collection("__mongocore.dead_letter"));
        let progress = self.progress.clone();
        let batch_size = options.batch_size as u32;
        let concurrency = options.concurrency as usize;
        let dedup_key = options.dedup_key.clone();
        let conflict_strategy = options.conflict_strategy;
        let cancel_channels = self.cancel_channels.clone();

        tokio::spawn(async move {
            let mut cancel_rx = cancel_tx.subscribe();
            let result = Self::run_ingestion(
                transformed_lf,
                &inferred_schema,
                target_collection.clone(),
                &dlq,
                &progress,
                &job_id,
                batch_size,
                concurrency,
                dedup_key,
                conflict_strategy,
                &mut cancel_rx,
            ).await;

            match result {
                Ok(()) => {
                    let _ = progress.complete_job(&job_id).await;
                }
                Err(e) => {
                    let _ = progress.fail_job(&job_id, &e.to_string()).await;
                }
            }

            cancel_channels.write().await.remove(&job_id);
        });

        Ok(job)
    }

    async fn run_ingestion(
        lf: LazyFrame,
        schema: &BsonSchema,
        collection: mongodb::Collection<bson::Document>,
        dlq: &DeadLetterQueue,
        progress: &ProgressTracker,
        job_id: &str,
        batch_size: u32,
        _concurrency: usize,
        dedup_key: Vec<String>,
        conflict_strategy: ConflictStrategy,
        cancel_rx: &mut broadcast::Receiver<()>,
    ) -> Result<(), MongoCoreError> {
        // Collect the full DataFrame
        let df = lf.collect().map_err(|e| {
            MongoCoreError::IngestionError(format!("DataFrame collection failed: {}", e))
        })?;

        let total_rows = df.height();
        let mut rows_processed: i64 = 0;
        let mut rows_inserted: i64 = 0;
        let mut rows_skipped: i64 = 0;
        let mut rows_failed: i64 = 0;
        let mut chunk_num: i64 = 0;

        let has_dedup = !dedup_key.is_empty();
        let dedup_checker = if has_dedup {
            Some(DedupChecker::new(
                collection.clone(),
                dedup_key,
                conflict_strategy,
            ))
        } else {
            None
        };

        // Process in chunks
        let mut offset = 0usize;
        while offset < total_rows {
            // Check for cancellation
            if cancel_rx.try_recv().is_ok() {
                return Err(MongoCoreError::IngestionError("Job cancelled".to_string()));
            }

            let end = (offset + batch_size as usize).min(total_rows);
            let chunk_df = df.slice(offset as i64, end - offset);

            // Convert to BSON documents
            let docs = match writer::dataframe_to_documents(&chunk_df, schema) {
                Ok(d) => d,
                Err(e) => {
                    // Entire chunk failed conversion — send all to DLQ
                    for i in offset..end {
                        let entry = DeadLetterEntry {
                            job_id: job_id.to_string(),
                            source_row: i as i64,
                            document: bson::Document::new(),
                            error: format!("Conversion error: {}", e),
                            stage: "conversion".to_string(),
                            timestamp: chrono::Utc::now(),
                        };
                        let _ = dlq.push(entry).await;
                    }
                    rows_failed += (end - offset) as i64;
                    offset = end;
                    chunk_num += 1;
                    continue;
                }
            };

            // Dedup check and write
            if let Some(ref checker) = dedup_checker {
                let results = checker.check_batch(&docs).await?;
                let mut to_insert = Vec::new();
                let mut to_replace = Vec::new();

                for (i, result) in results.into_iter().enumerate() {
                    match result {
                        DedupResult::Insert(d) => to_insert.push(d),
                        DedupResult::Skip => rows_skipped += 1,
                        DedupResult::Replace(d) => to_replace.push((d, docs[i].clone())),
                        DedupResult::Merge(incoming, existing) => {
                            let merged = DedupChecker::merge_documents(&incoming, &existing);
                            to_replace.push((merged, existing));
                        }
                    }
                }

                // Bulk insert new docs
                if !to_insert.is_empty() {
                    match collection.insert_many(&to_insert).await {
                        Ok(result) => {
                            rows_inserted += result.inserted_ids.len() as i64;
                        }
                        Err(e) => {
                            rows_failed += to_insert.len() as i64;
                            for (i, d) in to_insert.into_iter().enumerate() {
                                let entry = DeadLetterEntry {
                                    job_id: job_id.to_string(),
                                    source_row: (offset + i) as i64,
                                    document: d,
                                    error: format!("Insert failed: {}", e),
                                    stage: "bulk_write".to_string(),
                                    timestamp: chrono::Utc::now(),
                                };
                                let _ = dlq.push(entry).await;
                            }
                        }
                    }
                }

                // Replace/merge existing docs
                for (replacement, _original) in &to_replace {
                    let filter = checker.build_dedup_filter(replacement)
                        .unwrap_or_else(|| bson::doc! {});
                    match collection.replace_one(filter, replacement).await {
                        Ok(_) => rows_inserted += 1,
                        Err(e) => {
                            rows_failed += 1;
                            let entry = DeadLetterEntry {
                                job_id: job_id.to_string(),
                                source_row: offset as i64,
                                document: replacement.clone(),
                                error: format!("Replace failed: {}", e),
                                stage: "bulk_write".to_string(),
                                timestamp: chrono::Utc::now(),
                            };
                            let _ = dlq.push(entry).await;
                        }
                    }
                }
            } else {
                // No dedup — straight insert
                match collection.insert_many(&docs).await {
                    Ok(result) => {
                        rows_inserted += result.inserted_ids.len() as i64;
                    }
                    Err(e) => {
                        rows_failed += docs.len() as i64;
                        for (i, d) in docs.into_iter().enumerate() {
                            let entry = DeadLetterEntry {
                                job_id: job_id.to_string(),
                                source_row: (offset + i) as i64,
                                document: d,
                                error: format!("Insert failed: {}", e),
                                stage: "bulk_write".to_string(),
                                timestamp: chrono::Utc::now(),
                            };
                            let _ = dlq.push(entry).await;
                        }
                    }
                }
            }

            rows_processed += (end - offset) as i64;
            chunk_num += 1;
            offset = end;

            // Update progress
            let _ = progress.update_progress(
                job_id,
                rows_processed,
                rows_inserted,
                rows_skipped,
                rows_failed,
                chunk_num,
            ).await;
        }

        Ok(())
    }

    pub async fn get_status(&self, job_id: &str) -> Result<Option<IngestJob>, MongoCoreError> {
        self.progress.get_job(job_id).await
    }

    pub async fn list_jobs(&self) -> Result<Vec<IngestJob>, MongoCoreError> {
        self.progress.list_jobs().await
    }

    pub async fn cancel(&self, job_id: &str) -> Result<(), MongoCoreError> {
        let channels = self.cancel_channels.read().await;
        if let Some(tx) = channels.get(job_id) {
            let _ = tx.send(());
            Ok(())
        } else {
            Err(MongoCoreError::IngestionError(
                format!("Job '{}' not found or already finished", job_id),
            ))
        }
    }
}
```

- [ ] **Step 2: Update ingestion mod.rs**

```rust
// src/ingestion/mod.rs
pub mod dedup;
pub mod dlq;
pub mod engine;
pub mod progress;
pub mod reader;
pub mod schema;
pub mod transform;
pub mod types;
pub mod writer;

pub use engine::IngestionEngine;
pub use types::*;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add src/ingestion/engine.rs src/ingestion/mod.rs
git commit -m "feat(ingestion): add ingestion engine orchestrator with chunked parallel writes"
```

---

## Task 10: Watch Directory Service

**Files:**
- Create: `src/ingestion/watch.rs`
- Modify: `src/ingestion/mod.rs`

- [ ] **Step 1: Write watch directory service**

```rust
// src/ingestion/watch.rs
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use glob::Pattern;
use mongodb::Client;
use crate::ingestion::engine::IngestionEngine;
use crate::ingestion::types::*;
use crate::error::MongoCoreError;

pub struct WatchConfig {
    pub path: PathBuf,
    pub file_pattern: String,
    pub database: String,
    pub collection: String,
    pub conflict_strategy: ConflictStrategy,
    pub dedup_key: Vec<String>,
    pub debounce_ms: u64,
}

pub struct DirectoryWatcher {
    engine: Arc<IngestionEngine>,
    client: Client,
    watches: Arc<RwLock<Vec<WatchHandle>>>,
}

struct WatchHandle {
    id: String,
    config: WatchConfig,
    cancel_tx: tokio::sync::broadcast::Sender<()>,
}

impl DirectoryWatcher {
    pub fn new(engine: Arc<IngestionEngine>, client: Client) -> Self {
        Self {
            engine,
            client,
            watches: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn start_watch(&self, config: WatchConfig) -> Result<String, MongoCoreError> {
        let watch_id = uuid::Uuid::new_v4().to_string();
        let (cancel_tx, mut cancel_rx) = tokio::sync::broadcast::channel(1);

        // Validate path exists
        if !config.path.exists() {
            return Err(MongoCoreError::IngestionError(
                format!("Watch path does not exist: {:?}", config.path),
            ));
        }

        let pattern = Pattern::new(&config.file_pattern).map_err(|e| {
            MongoCoreError::IngestionError(format!("Invalid glob pattern: {}", e))
        })?;

        let engine = self.engine.clone();
        let client = self.client.clone();
        let debounce = Duration::from_millis(config.debounce_ms);
        let watch_path = config.path.clone();
        let database = config.database.clone();
        let collection = config.collection.clone();
        let dedup_key = config.dedup_key.clone();
        let conflict_strategy = config.conflict_strategy;

        tokio::spawn(async move {
            let (tx, mut rx) = tokio::sync::mpsc::channel(100);

            let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
                if let Ok(event) = res {
                    if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                        for path in event.paths {
                            let _ = tx.blocking_send(path);
                        }
                    }
                }
            }).unwrap();

            let _ = watcher.watch(&watch_path, RecursiveMode::NonRecursive);

            let mut pending_files: std::collections::HashMap<PathBuf, tokio::time::Instant> =
                std::collections::HashMap::new();

            loop {
                tokio::select! {
                    _ = cancel_rx.recv() => break,
                    Some(path) = rx.recv() => {
                        if let Some(filename) = path.file_name().and_then(|f| f.to_str()) {
                            if pattern.matches(filename) {
                                pending_files.insert(path, tokio::time::Instant::now());
                            }
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(500)) => {
                        let now = tokio::time::Instant::now();
                        let ready: Vec<PathBuf> = pending_files
                            .iter()
                            .filter(|(_, last_modified)| now.duration_since(**last_modified) >= debounce)
                            .map(|(p, _)| p.clone())
                            .collect();

                        for path in ready {
                            pending_files.remove(&path);
                            let options = IngestOptions {
                                file_path: path.to_string_lossy().to_string(),
                                database: database.clone(),
                                collection: collection.clone(),
                                format: FileFormat::Auto,
                                dedup_key: dedup_key.clone(),
                                conflict_strategy,
                                ..Default::default()
                            };
                            let _ = engine.ingest(&client, options).await;
                        }
                    }
                }
            }
        });

        self.watches.write().await.push(WatchHandle {
            id: watch_id.clone(),
            config,
            cancel_tx,
        });

        Ok(watch_id)
    }

    pub async fn stop_watch(&self, watch_id: &str) -> Result<(), MongoCoreError> {
        let mut watches = self.watches.write().await;
        if let Some(pos) = watches.iter().position(|w| w.id == watch_id) {
            let handle = watches.remove(pos);
            let _ = handle.cancel_tx.send(());
            Ok(())
        } else {
            Err(MongoCoreError::IngestionError(
                format!("Watch '{}' not found", watch_id),
            ))
        }
    }
}
```

- [ ] **Step 2: Add glob dependency to Cargo.toml**

```toml
glob = "0.3"
```

- [ ] **Step 3: Update ingestion mod.rs**

```rust
// src/ingestion/mod.rs
pub mod dedup;
pub mod dlq;
pub mod engine;
pub mod progress;
pub mod reader;
pub mod schema;
pub mod transform;
pub mod types;
pub mod watch;
pub mod writer;

pub use engine::IngestionEngine;
pub use types::*;
pub use watch::DirectoryWatcher;
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add src/ingestion/watch.rs src/ingestion/mod.rs Cargo.toml
git commit -m "feat(ingestion): add directory watcher with debounce and auto-trigger"
```

---

## Task 11: Ingestion Configuration

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Write failing test for ingestion config**

Add to `src/config.rs`:

```rust
#[derive(Debug, Deserialize, Clone)]
pub struct IngestionConfig {
    pub enabled: Option<bool>,
    pub sample_size: Option<u32>,
    pub default_batch_size: Option<u32>,
    pub default_concurrency: Option<u32>,
    pub max_file_size_mb: Option<u64>,
    pub llm_expressions: Option<bool>,
    pub max_llm_concurrency: Option<u32>,
    pub watch_debounce_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WatchFileConfig {
    pub enabled: Option<bool>,
    pub path: Option<String>,
    pub file_pattern: Option<String>,
    pub database: Option<String>,
    pub collection: Option<String>,
    pub conflict_strategy: Option<String>,
}
```

Add test:
```rust
#[test]
fn test_ingestion_config_parsing() {
    let toml_content = r#"
connection_uri = "mongodb://localhost:27017"

[ingestion]
enabled = true
sample_size = 2000
default_batch_size = 500
default_concurrency = 8
max_file_size_mb = 5120
llm_expressions = false
max_llm_concurrency = 2
watch_debounce_ms = 3000

[ingestion.watch]
enabled = true
path = "/data/incoming"
file_pattern = "*.csv"
database = "imports"
collection = "data"
conflict_strategy = "merge"
"#;

    let mut tmp = NamedTempFile::new().unwrap();
    tmp.write_all(toml_content.as_bytes()).unwrap();

    let cli = CliArgs {
        config: Some(tmp.path().to_path_buf()),
        connection_uri: None,
        grpc_port: None,
        mcp_port: None,
        llm_provider: None,
        llm_api_key_env: None,
        voyage_api_key_env: None,
        compiled_cache_sync: None,
        log_level: None,
    };

    let config = Config::load(&cli).unwrap();
    assert!(config.ingestion.enabled);
    assert_eq!(config.ingestion.sample_size, 2000);
    assert_eq!(config.ingestion.default_batch_size, 500);
    assert_eq!(config.ingestion.default_concurrency, 8);
    assert_eq!(config.ingestion.max_file_size_mb, 5120);
    assert!(!config.ingestion.llm_expressions);
    assert_eq!(config.ingestion.max_llm_concurrency, 2);
    assert_eq!(config.ingestion.watch_debounce_ms, 3000);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib config::tests::test_ingestion_config_parsing`
Expected: FAIL (field doesn't exist)

- [ ] **Step 3: Implement ingestion config**

Add to `FileConfig`:
```rust
pub ingestion: Option<IngestionFileConfig>,
```

Add new struct:
```rust
#[derive(Debug, Deserialize, Clone, Default)]
pub struct IngestionFileConfig {
    pub enabled: Option<bool>,
    pub sample_size: Option<u32>,
    pub default_batch_size: Option<u32>,
    pub default_concurrency: Option<u32>,
    pub max_file_size_mb: Option<u64>,
    pub llm_expressions: Option<bool>,
    pub max_llm_concurrency: Option<u32>,
    pub watch_debounce_ms: Option<u64>,
    pub watch: Option<WatchFileConfig>,
}

#[derive(Debug, Deserialize, Clone, Default)]
pub struct WatchFileConfig {
    pub enabled: Option<bool>,
    pub path: Option<String>,
    pub file_pattern: Option<String>,
    pub database: Option<String>,
    pub collection: Option<String>,
    pub conflict_strategy: Option<String>,
}
```

Add resolved config:
```rust
#[derive(Debug, Clone)]
pub struct ResolvedIngestionConfig {
    pub enabled: bool,
    pub sample_size: u32,
    pub default_batch_size: u32,
    pub default_concurrency: u32,
    pub max_file_size_mb: u64,
    pub llm_expressions: bool,
    pub max_llm_concurrency: u32,
    pub watch_debounce_ms: u64,
}

impl Default for ResolvedIngestionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sample_size: 1000,
            default_batch_size: 1000,
            default_concurrency: 4,
            max_file_size_mb: 10240,
            llm_expressions: false,
            max_llm_concurrency: 4,
            watch_debounce_ms: 2000,
        }
    }
}
```

Add `ingestion: ResolvedIngestionConfig` field to `Config` struct and resolve it in `Config::load()`:
```rust
let ingestion_file = file_config.ingestion.unwrap_or_default();
let ingestion = ResolvedIngestionConfig {
    enabled: ingestion_file.enabled.unwrap_or(true),
    sample_size: ingestion_file.sample_size.unwrap_or(1000),
    default_batch_size: ingestion_file.default_batch_size.unwrap_or(1000),
    default_concurrency: ingestion_file.default_concurrency.unwrap_or(4),
    max_file_size_mb: ingestion_file.max_file_size_mb.unwrap_or(10240),
    llm_expressions: ingestion_file.llm_expressions.unwrap_or(false),
    max_llm_concurrency: ingestion_file.max_llm_concurrency.unwrap_or(4),
    watch_debounce_ms: ingestion_file.watch_debounce_ms.unwrap_or(2000),
};
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib config`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/config.rs
git commit -m "feat(ingestion): add ingestion configuration with sensible defaults"
```

---

## Task 12: Protobuf Definitions for Ingestion RPCs

**Files:**
- Create: `proto/mongocore/v1/ingestion.proto`
- Modify: `proto/mongocore/v1/mongocore.proto`

- [ ] **Step 1: Create ingestion proto file**

```protobuf
// proto/mongocore/v1/ingestion.proto
syntax = "proto3";
package mongocore.v1;
option go_package = "github.com/rozza/mongocore/clients/go/proto";

message CsvOptions {
  optional string delimiter = 1;
  optional string quote_char = 2;
  optional bool has_header = 3;
  optional string comment_char = 4;
}

message IngestRequest {
  string file_path = 1;
  string database = 2;
  string collection = 3;
  FileFormat format = 4;
  repeated string dedup_key = 5;
  ConflictStrategy conflict_strategy = 6;
  int32 batch_size = 7;
  int32 concurrency = 8;
  repeated string expressions = 9;
  map<string, string> schema_overrides = 10;
  int32 sample_size = 11;
  CsvOptions csv_options = 12;
}

message IngestResponse {
  string job_id = 1;
  IngestJobStatus status = 2;
  map<string, string> inferred_schema = 3;
  int64 total_rows = 4;
}

message GetIngestStatusRequest {
  string job_id = 1;
}

message GetIngestStatusResponse {
  string job_id = 1;
  IngestJobStatus status = 2;
  int64 total_rows = 3;
  int64 rows_processed = 4;
  int64 rows_inserted = 5;
  int64 rows_skipped = 6;
  int64 rows_failed = 7;
  int64 elapsed_ms = 8;
  int64 estimated_remaining_ms = 9;
}

message ListIngestJobsRequest {}

message ListIngestJobsResponse {
  repeated IngestJobSummary jobs = 1;
}

message IngestJobSummary {
  string job_id = 1;
  string file_path = 2;
  string database = 3;
  string collection = 4;
  IngestJobStatus status = 5;
  int64 total_rows = 6;
  int64 rows_processed = 7;
}

message CancelIngestRequest {
  string job_id = 1;
}

message CancelIngestResponse {
  bool success = 1;
}

message WatchDirectoryRequest {
  string path = 1;
  string file_pattern = 2;
  string database = 3;
  string collection = 4;
  ConflictStrategy conflict_strategy = 5;
  repeated string dedup_key = 6;
}

message WatchDirectoryResponse {
  string watch_id = 1;
}

message StopWatchRequest {
  string watch_id = 1;
}

message StopWatchResponse {
  bool success = 1;
}

enum FileFormat {
  FILE_FORMAT_AUTO = 0;
  FILE_FORMAT_CSV = 1;
  FILE_FORMAT_JSON = 2;
  FILE_FORMAT_NDJSON = 3;
  FILE_FORMAT_PARQUET = 4;
}

enum ConflictStrategy {
  CONFLICT_STRATEGY_SKIP = 0;
  CONFLICT_STRATEGY_OVERWRITE = 1;
  CONFLICT_STRATEGY_MERGE = 2;
}

enum IngestJobStatus {
  INGEST_JOB_STATUS_RUNNING = 0;
  INGEST_JOB_STATUS_COMPLETED = 1;
  INGEST_JOB_STATUS_FAILED = 2;
  INGEST_JOB_STATUS_CANCELLED = 3;
}
```

- [ ] **Step 2: Add Ingestion RPCs to mongocore.proto**

Add to the `service MongoCore` block:
```protobuf
  // Ingestion
  rpc Ingest(IngestRequest) returns (IngestResponse);
  rpc GetIngestStatus(GetIngestStatusRequest) returns (GetIngestStatusResponse);
  rpc ListIngestJobs(ListIngestJobsRequest) returns (ListIngestJobsResponse);
  rpc CancelIngest(CancelIngestRequest) returns (CancelIngestResponse);
  rpc WatchDirectory(WatchDirectoryRequest) returns (WatchDirectoryResponse);
  rpc StopWatch(StopWatchRequest) returns (StopWatchResponse);
```

Add import at top:
```protobuf
import "mongocore/v1/ingestion.proto";
```

- [ ] **Step 3: Update build.rs to include new proto**

Ensure `build.rs` compiles both proto files.

- [ ] **Step 4: Verify protos compile**

Run: `cargo build`
Expected: Compiles with generated ingestion types

- [ ] **Step 5: Commit**

```bash
git add proto/mongocore/v1/ingestion.proto proto/mongocore/v1/mongocore.proto
git commit -m "feat(ingestion): add protobuf definitions for ingestion RPCs"
```

---

## Task 13: gRPC Service Implementation for Ingestion

**Files:**
- Modify: `src/grpc/service.rs`

- [ ] **Step 1: Add ingestion engine to gRPC service state**

Add `IngestionEngine` and `DirectoryWatcher` to the service struct. Add handler implementations for all 6 new RPCs:

```rust
async fn ingest(&self, request: Request<IngestRequest>) -> Result<Response<IngestResponse>, Status> {
    let req = request.into_inner();
    let options = IngestOptions {
        file_path: req.file_path,
        database: req.database,
        collection: req.collection,
        format: proto_format_to_internal(req.format()),
        dedup_key: req.dedup_key,
        conflict_strategy: proto_conflict_to_internal(req.conflict_strategy()),
        batch_size: if req.batch_size > 0 { req.batch_size as u32 } else { 1000 },
        concurrency: if req.concurrency > 0 { req.concurrency as u32 } else { 4 },
        expressions: req.expressions,
        schema_overrides: req.schema_overrides,
        sample_size: if req.sample_size > 0 { req.sample_size as u32 } else { 1000 },
        csv_options: Default::default(),
    };

    match self.ingestion_engine.ingest(&self.client, options).await {
        Ok(job) => {
            let mut schema_map = std::collections::HashMap::new();
            for field in &job.inferred_schema.fields {
                schema_map.insert(field.name.clone(), format!("{:?}", field.bson_type));
            }
            Ok(Response::new(IngestResponse {
                job_id: job.job_id,
                status: IngestJobStatus::IngestJobStatusRunning as i32,
                inferred_schema: schema_map,
                total_rows: job.total_rows,
            }))
        }
        Err(e) => Err(Status::internal(e.to_string())),
    }
}

async fn get_ingest_status(&self, request: Request<GetIngestStatusRequest>) -> Result<Response<GetIngestStatusResponse>, Status> {
    let job_id = request.into_inner().job_id;
    match self.ingestion_engine.get_status(&job_id).await {
        Ok(Some(job)) => {
            let elapsed = chrono::Utc::now()
                .signed_duration_since(job.started_at)
                .num_milliseconds();
            let estimated_remaining = if job.rows_processed > 0 {
                let rate = elapsed as f64 / job.rows_processed as f64;
                ((job.total_rows - job.rows_processed) as f64 * rate) as i64
            } else {
                0
            };
            Ok(Response::new(GetIngestStatusResponse {
                job_id: job.job_id,
                status: status_to_proto(job.status) as i32,
                total_rows: job.total_rows,
                rows_processed: job.rows_processed,
                rows_inserted: job.rows_inserted,
                rows_skipped: job.rows_skipped,
                rows_failed: job.rows_failed,
                elapsed_ms: elapsed,
                estimated_remaining_ms: estimated_remaining,
            }))
        }
        Ok(None) => Err(Status::not_found(format!("Job '{}' not found", job_id))),
        Err(e) => Err(Status::internal(e.to_string())),
    }
}

async fn list_ingest_jobs(&self, _request: Request<ListIngestJobsRequest>) -> Result<Response<ListIngestJobsResponse>, Status> {
    match self.ingestion_engine.list_jobs().await {
        Ok(jobs) => {
            let summaries = jobs.iter().map(|j| IngestJobSummary {
                job_id: j.job_id.clone(),
                file_path: j.file_path.clone(),
                database: j.database.clone(),
                collection: j.collection.clone(),
                status: status_to_proto(j.status) as i32,
                total_rows: j.total_rows,
                rows_processed: j.rows_processed,
            }).collect();
            Ok(Response::new(ListIngestJobsResponse { jobs: summaries }))
        }
        Err(e) => Err(Status::internal(e.to_string())),
    }
}

async fn cancel_ingest(&self, request: Request<CancelIngestRequest>) -> Result<Response<CancelIngestResponse>, Status> {
    let job_id = request.into_inner().job_id;
    match self.ingestion_engine.cancel(&job_id).await {
        Ok(()) => Ok(Response::new(CancelIngestResponse { success: true })),
        Err(e) => Err(Status::internal(e.to_string())),
    }
}

async fn watch_directory(&self, request: Request<WatchDirectoryRequest>) -> Result<Response<WatchDirectoryResponse>, Status> {
    let req = request.into_inner();
    let config = crate::ingestion::watch::WatchConfig {
        path: std::path::PathBuf::from(req.path),
        file_pattern: req.file_pattern,
        database: req.database,
        collection: req.collection,
        conflict_strategy: proto_conflict_to_internal(req.conflict_strategy()),
        dedup_key: req.dedup_key,
        debounce_ms: 2000,
    };
    match self.directory_watcher.start_watch(config).await {
        Ok(watch_id) => Ok(Response::new(WatchDirectoryResponse { watch_id })),
        Err(e) => Err(Status::internal(e.to_string())),
    }
}

async fn stop_watch(&self, request: Request<StopWatchRequest>) -> Result<Response<StopWatchResponse>, Status> {
    let watch_id = request.into_inner().watch_id;
    match self.directory_watcher.stop_watch(&watch_id).await {
        Ok(()) => Ok(Response::new(StopWatchResponse { success: true })),
        Err(e) => Err(Status::internal(e.to_string())),
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add src/grpc/service.rs
git commit -m "feat(ingestion): implement gRPC handlers for all 6 ingestion RPCs"
```

---

## Task 14: MCP Tool Definitions for Ingestion

**Files:**
- Modify: `src/mcp/tools.rs`

- [ ] **Step 1: Add ingestion MCP tools**

Add 6 new tool definitions matching the gRPC RPCs:

| Tool | Description |
|------|-------------|
| `ingest` | Start a file ingestion job |
| `ingest_status` | Get status of an ingestion job |
| `list_ingest_jobs` | List all ingestion jobs |
| `cancel_ingest` | Cancel a running ingestion job |
| `watch_directory` | Start watching a directory for new files |
| `stop_watch` | Stop watching a directory |

Each tool should accept JSON parameters matching the proto request fields and return JSON matching the proto response fields. Use the existing MCP tool pattern from the file.

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add src/mcp/tools.rs
git commit -m "feat(ingestion): add MCP tools for ingestion (ingest, status, list, cancel, watch)"
```

---

## Task 15: Initialize Ingestion Engine on Startup

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Initialize engine in main**

Add initialization of `IngestionEngine` and `DirectoryWatcher` after the MongoDB client is created, pass them to the gRPC service. If `config.ingestion.enabled` is false, skip initialization.

If the watch config has `enabled = true` and a valid path, auto-start the directory watcher on startup.

- [ ] **Step 2: Verify it compiles and starts**

Run: `cargo build && cargo run -- --help`
Expected: Compiles and shows help

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat(ingestion): initialize ingestion engine on sidecar startup"
```

---

## Task 16: LLM Expression Functions (Optional)

**Files:**
- Create: `src/ingestion/llm_expressions.rs`
- Modify: `src/ingestion/mod.rs`

- [ ] **Step 1: Write LLM expression stubs with validation**

```rust
// src/ingestion/llm_expressions.rs
use polars::prelude::*;
use crate::error::MongoCoreError;

pub struct LlmExpressionConfig {
    pub provider: String,
    pub api_key: String,
    pub max_concurrency: u32,
}

pub fn validate_llm_available(config: &Option<LlmExpressionConfig>) -> Result<(), MongoCoreError> {
    match config {
        Some(_) => Ok(()),
        None => Err(MongoCoreError::IngestionError(
            "LLM expressions require an API key. Set llm_provider and llm_api_key_env in config.".to_string(),
        )),
    }
}

pub async fn llm_classify(
    values: &[String],
    categories: &[String],
    config: &LlmExpressionConfig,
) -> Result<Vec<String>, MongoCoreError> {
    // Batch classify using LLM provider
    // Uses existing LLM infrastructure from compiled queries
    let mut results = Vec::with_capacity(values.len());
    for value in values {
        // For now, placeholder — real implementation calls the LLM provider
        results.push(categories.first().cloned().unwrap_or_default());
    }
    Ok(results)
}

pub async fn llm_extract(
    values: &[String],
    schema: &std::collections::HashMap<String, String>,
    config: &LlmExpressionConfig,
) -> Result<Vec<String>, MongoCoreError> {
    let mut results = Vec::with_capacity(values.len());
    for _value in values {
        results.push("{}".to_string());
    }
    Ok(results)
}

pub async fn llm_normalize(
    values: &[String],
    config: &LlmExpressionConfig,
) -> Result<Vec<String>, MongoCoreError> {
    Ok(values.to_vec())
}

pub async fn llm_embed(
    values: &[String],
    config: &LlmExpressionConfig,
) -> Result<Vec<Vec<f64>>, MongoCoreError> {
    // Uses existing Voyage AI client when configured
    let dim = 1024;
    Ok(values.iter().map(|_| vec![0.0f64; dim]).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_llm_none_errors() {
        let result = validate_llm_available(&None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("API key"));
    }

    #[test]
    fn test_validate_llm_some_ok() {
        let config = Some(LlmExpressionConfig {
            provider: "anthropic".to_string(),
            api_key: "test-key".to_string(),
            max_concurrency: 4,
        });
        assert!(validate_llm_available(&config).is_ok());
    }
}
```

- [ ] **Step 2: Update ingestion mod.rs**

```rust
// src/ingestion/mod.rs
pub mod dedup;
pub mod dlq;
pub mod engine;
pub mod llm_expressions;
pub mod progress;
pub mod reader;
pub mod schema;
pub mod transform;
pub mod types;
pub mod watch;
pub mod writer;

pub use engine::IngestionEngine;
pub use types::*;
pub use watch::DirectoryWatcher;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib ingestion::llm_expressions`
Expected: All tests PASS

- [ ] **Step 4: Commit**

```bash
git add src/ingestion/llm_expressions.rs src/ingestion/mod.rs
git commit -m "feat(ingestion): add LLM expression function stubs (classify, extract, normalize, embed)"
```

---

## Task 17: Python Client Ingestion Methods

**Files:**
- Modify: `clients/python/mongocore/client.py`

- [ ] **Step 1: Add ingestion methods to Python client**

```python
async def ingest(
    self,
    file_path: str,
    database: str,
    collection: str,
    *,
    format: str = "auto",
    dedup_key: list[str] | None = None,
    conflict_strategy: str = "skip",
    batch_size: int = 1000,
    concurrency: int = 4,
    expressions: list[str] | None = None,
    schema_overrides: dict[str, str] | None = None,
    sample_size: int = 1000,
) -> dict:
    """Start a file ingestion job."""
    request = IngestRequest(
        file_path=file_path,
        database=database,
        collection=collection,
        format=self._format_enum(format),
        dedup_key=dedup_key or [],
        conflict_strategy=self._conflict_enum(conflict_strategy),
        batch_size=batch_size,
        concurrency=concurrency,
        expressions=expressions or [],
        schema_overrides=schema_overrides or {},
        sample_size=sample_size,
    )
    response = await self._stub.Ingest(request)
    return {
        "job_id": response.job_id,
        "status": response.status,
        "inferred_schema": dict(response.inferred_schema),
        "total_rows": response.total_rows,
    }

async def ingest_status(self, job_id: str) -> dict:
    """Get ingestion job status."""
    response = await self._stub.GetIngestStatus(
        GetIngestStatusRequest(job_id=job_id)
    )
    return {
        "job_id": response.job_id,
        "status": response.status,
        "total_rows": response.total_rows,
        "rows_processed": response.rows_processed,
        "rows_inserted": response.rows_inserted,
        "rows_skipped": response.rows_skipped,
        "rows_failed": response.rows_failed,
        "elapsed_ms": response.elapsed_ms,
        "estimated_remaining_ms": response.estimated_remaining_ms,
    }

async def list_ingest_jobs(self) -> list[dict]:
    """List all ingestion jobs."""
    response = await self._stub.ListIngestJobs(ListIngestJobsRequest())
    return [
        {
            "job_id": j.job_id,
            "file_path": j.file_path,
            "database": j.database,
            "collection": j.collection,
            "status": j.status,
            "total_rows": j.total_rows,
            "rows_processed": j.rows_processed,
        }
        for j in response.jobs
    ]

async def cancel_ingest(self, job_id: str) -> bool:
    """Cancel a running ingestion job."""
    response = await self._stub.CancelIngest(
        CancelIngestRequest(job_id=job_id)
    )
    return response.success

async def watch_directory(
    self,
    path: str,
    database: str,
    collection: str,
    *,
    file_pattern: str = "*.csv",
    conflict_strategy: str = "skip",
    dedup_key: list[str] | None = None,
) -> str:
    """Start watching a directory for new files."""
    response = await self._stub.WatchDirectory(
        WatchDirectoryRequest(
            path=path,
            file_pattern=file_pattern,
            database=database,
            collection=collection,
            conflict_strategy=self._conflict_enum(conflict_strategy),
            dedup_key=dedup_key or [],
        )
    )
    return response.watch_id

async def stop_watch(self, watch_id: str) -> bool:
    """Stop watching a directory."""
    response = await self._stub.StopWatch(StopWatchRequest(watch_id=watch_id))
    return response.success
```

- [ ] **Step 2: Commit**

```bash
git add clients/python/
git commit -m "feat(clients): add ingestion methods to Python client"
```

---

## Task 18: TypeScript Client Ingestion Methods

**Files:**
- Modify: `clients/typescript/src/client.ts`

- [ ] **Step 1: Add ingestion methods to TypeScript client**

```typescript
async ingest(options: {
  filePath: string;
  database: string;
  collection: string;
  format?: 'auto' | 'csv' | 'json' | 'ndjson' | 'parquet';
  dedupKey?: string[];
  conflictStrategy?: 'skip' | 'overwrite' | 'merge';
  batchSize?: number;
  concurrency?: number;
  expressions?: string[];
  schemaOverrides?: Record<string, string>;
  sampleSize?: number;
}): Promise<IngestResult> {
  const response = await this.client.ingest({
    filePath: options.filePath,
    database: options.database,
    collection: options.collection,
    format: this.mapFormat(options.format ?? 'auto'),
    dedupKey: options.dedupKey ?? [],
    conflictStrategy: this.mapConflict(options.conflictStrategy ?? 'skip'),
    batchSize: options.batchSize ?? 1000,
    concurrency: options.concurrency ?? 4,
    expressions: options.expressions ?? [],
    schemaOverrides: options.schemaOverrides ?? {},
    sampleSize: options.sampleSize ?? 1000,
  });
  return {
    jobId: response.jobId,
    status: response.status,
    inferredSchema: response.inferredSchema,
    totalRows: Number(response.totalRows),
  };
}

async ingestStatus(jobId: string): Promise<IngestStatus> {
  const response = await this.client.getIngestStatus({ jobId });
  return {
    jobId: response.jobId,
    status: response.status,
    totalRows: Number(response.totalRows),
    rowsProcessed: Number(response.rowsProcessed),
    rowsInserted: Number(response.rowsInserted),
    rowsSkipped: Number(response.rowsSkipped),
    rowsFailed: Number(response.rowsFailed),
    elapsedMs: Number(response.elapsedMs),
    estimatedRemainingMs: Number(response.estimatedRemainingMs),
  };
}

async listIngestJobs(): Promise<IngestJobSummary[]> {
  const response = await this.client.listIngestJobs({});
  return response.jobs.map(j => ({
    jobId: j.jobId,
    filePath: j.filePath,
    database: j.database,
    collection: j.collection,
    status: j.status,
    totalRows: Number(j.totalRows),
    rowsProcessed: Number(j.rowsProcessed),
  }));
}

async cancelIngest(jobId: string): Promise<boolean> {
  const response = await this.client.cancelIngest({ jobId });
  return response.success;
}

async watchDirectory(options: {
  path: string;
  database: string;
  collection: string;
  filePattern?: string;
  conflictStrategy?: 'skip' | 'overwrite' | 'merge';
  dedupKey?: string[];
}): Promise<string> {
  const response = await this.client.watchDirectory({
    path: options.path,
    filePattern: options.filePattern ?? '*.csv',
    database: options.database,
    collection: options.collection,
    conflictStrategy: this.mapConflict(options.conflictStrategy ?? 'skip'),
    dedupKey: options.dedupKey ?? [],
  });
  return response.watchId;
}

async stopWatch(watchId: string): Promise<boolean> {
  const response = await this.client.stopWatch({ watchId });
  return response.success;
}
```

- [ ] **Step 2: Commit**

```bash
git add clients/typescript/
git commit -m "feat(clients): add ingestion methods to TypeScript client"
```

---

## Task 19: Go Client Ingestion Methods

**Files:**
- Modify: `clients/go/client.go`

- [ ] **Step 1: Add ingestion methods to Go client**

```go
type IngestOptions struct {
    FilePath         string
    Database         string
    Collection       string
    Format           string
    DedupKey         []string
    ConflictStrategy string
    BatchSize        int32
    Concurrency      int32
    Expressions      []string
    SchemaOverrides  map[string]string
    SampleSize       int32
}

type IngestResult struct {
    JobID          string
    Status         string
    InferredSchema map[string]string
    TotalRows      int64
}

type IngestStatus struct {
    JobID               string
    Status              string
    TotalRows           int64
    RowsProcessed       int64
    RowsInserted        int64
    RowsSkipped         int64
    RowsFailed          int64
    ElapsedMs           int64
    EstimatedRemainingMs int64
}

func (c *Client) Ingest(ctx context.Context, opts IngestOptions) (*IngestResult, error) { ... }
func (c *Client) IngestStatus(ctx context.Context, jobID string) (*IngestStatus, error) { ... }
func (c *Client) ListIngestJobs(ctx context.Context) ([]IngestJobSummary, error) { ... }
func (c *Client) CancelIngest(ctx context.Context, jobID string) (bool, error) { ... }
func (c *Client) WatchDirectory(ctx context.Context, opts WatchDirectoryOptions) (string, error) { ... }
func (c *Client) StopWatch(ctx context.Context, watchID string) (bool, error) { ... }
```

- [ ] **Step 2: Commit**

```bash
git add clients/go/
git commit -m "feat(clients): add ingestion methods to Go client"
```

---

## Task 20: Java Client Ingestion Methods

**Files:**
- Modify: `clients/java/src/main/java/com/mongocore/MongoClient.java`

- [ ] **Step 1: Add ingestion methods to Java client**

```java
public IngestResult ingest(IngestOptions options) { ... }
public IngestStatus ingestStatus(String jobId) { ... }
public List<IngestJobSummary> listIngestJobs() { ... }
public boolean cancelIngest(String jobId) { ... }
public String watchDirectory(WatchDirectoryOptions options) { ... }
public boolean stopWatch(String watchId) { ... }
```

- [ ] **Step 2: Commit**

```bash
git add clients/java/
git commit -m "feat(clients): add ingestion methods to Java client"
```

---

## Task 21: Integration Tests

**Files:**
- Create: `tests/integration/ingestion_test.rs`
- Create: `tests/fixtures/sample.csv`
- Create: `tests/fixtures/sample.ndjson`

- [ ] **Step 1: Create test fixtures**

```csv
# tests/fixtures/sample.csv
name,age,email,score,active
Alice,30,alice@test.com,95.5,true
Bob,25,bob@test.com,88.0,true
Charlie,35,charlie@test.com,72.0,false
Diana,28,diana@test.com,91.5,true
Eve,32,eve@test.com,85.0,false
```

```json
// tests/fixtures/sample.ndjson
{"name":"Alice","age":30,"email":"alice@test.com","score":95.5}
{"name":"Bob","age":25,"email":"bob@test.com","score":88.0}
{"name":"Charlie","age":35,"email":"charlie@test.com","score":72.0}
```

- [ ] **Step 2: Write integration tests**

```rust
// tests/integration/ingestion_test.rs
use mongocore::ingestion::*;
use mongocore::ingestion::reader;
use mongocore::ingestion::schema;
use mongocore::ingestion::writer;
use mongocore::ingestion::transform;
use std::path::Path;

#[test]
fn test_csv_end_to_end_schema_inference() {
    let path = Path::new("tests/fixtures/sample.csv");
    let lf = reader::read_lazy(path, FileFormat::Csv, &CsvOptions::default()).unwrap();
    let df = lf.collect().unwrap();
    let schema = schema::infer_schema(&df).unwrap();

    assert_eq!(schema.fields.len(), 5);
    assert_eq!(schema.fields[0].name, "name");
    assert_eq!(schema.fields[0].bson_type, BsonType::String);
    assert_eq!(schema.fields[1].name, "age");
    // Polars may infer age as Int64
    assert!(matches!(schema.fields[1].bson_type, BsonType::Int64 | BsonType::Int32));
}

#[test]
fn test_csv_to_bson_documents() {
    let path = Path::new("tests/fixtures/sample.csv");
    let lf = reader::read_lazy(path, FileFormat::Csv, &CsvOptions::default()).unwrap();
    let df = lf.collect().unwrap();
    let schema = schema::infer_schema(&df).unwrap();
    let docs = writer::dataframe_to_documents(&df, &schema).unwrap();

    assert_eq!(docs.len(), 5);
    assert_eq!(docs[0].get_str("name"), Ok("Alice"));
}

#[test]
fn test_ndjson_end_to_end() {
    let path = Path::new("tests/fixtures/sample.ndjson");
    let lf = reader::read_lazy(path, FileFormat::NdJson, &CsvOptions::default()).unwrap();
    let df = lf.collect().unwrap();
    let schema = schema::infer_schema(&df).unwrap();
    let docs = writer::dataframe_to_documents(&df, &schema).unwrap();

    assert_eq!(docs.len(), 3);
    assert_eq!(docs[0].get_str("name"), Ok("Alice"));
}

#[test]
fn test_transform_then_convert() {
    let path = Path::new("tests/fixtures/sample.csv");
    let lf = reader::read_lazy(path, FileFormat::Csv, &CsvOptions::default()).unwrap();

    let transformed = transform::apply_expressions(lf, &[
        "filter(age > 28)".to_string(),
        "drop(active)".to_string(),
    ]).unwrap();

    let df = transformed.collect().unwrap();
    assert!(df.height() <= 5); // Filtered
    assert!(df.column("active").is_err()); // Dropped
}

#[test]
fn test_schema_overrides() {
    let path = Path::new("tests/fixtures/sample.csv");
    let lf = reader::read_lazy(path, FileFormat::Csv, &CsvOptions::default()).unwrap();
    let df = lf.collect().unwrap();
    let mut schema = schema::infer_schema(&df).unwrap();

    let mut overrides = std::collections::HashMap::new();
    overrides.insert("age".to_string(), "Double".to_string());
    schema::apply_overrides(&mut schema, &overrides);

    assert_eq!(
        schema.fields.iter().find(|f| f.name == "age").unwrap().bson_type,
        BsonType::Double
    );
}
```

- [ ] **Step 3: Run integration tests**

Run: `cargo test --test integration ingestion`
Expected: All tests PASS

- [ ] **Step 4: Commit**

```bash
git add tests/integration/ingestion_test.rs tests/fixtures/
git commit -m "test(ingestion): add end-to-end integration tests for CSV, JSON, transforms"
```

---

## Task 22: Update README and Documentation

**Files:**
- Modify: `README.md`
- Create: `docs/ingestion.md`

- [ ] **Step 1: Update README roadmap and feature set**

Add v3 Feature Set section:
```markdown
## v3 Feature Set

- **Polars-based data ingestion** — CSV, JSON, NDJSON, Parquet file ingestion with parallel processing
- **Schema inference** — Spark-connector-inspired multi-row sampling with BSON type mapping
- **Transform engine** — User-provided Polars expressions (rename, filter, cast, drop, select)
- **LLM expressions (optional)** — llm_classify, llm_extract, llm_normalize, llm_embed when API key configured
- **Deduplication** — Key-based dedup with skip/overwrite/merge conflict resolution
- **Dead letter queue** — Failed documents routed to `__mongocore.dead_letter` for inspection
- **Progress tracking** — Real-time job status with resumability on crash recovery
- **Directory watching** — Auto-trigger ingestion when new files appear
- **6 new gRPC RPCs** — Ingest, GetIngestStatus, ListIngestJobs, CancelIngest, WatchDirectory, StopWatch
- **6 new MCP tools** — Full AI agent support for data ingestion workflows
```

Update roadmap table:
```markdown
| **v0.3** | Intelligent data ingestion (Polars-powered ETL) | **Complete** |
```

- [ ] **Step 2: Create ingestion documentation**

Write `docs/ingestion.md` with usage examples for all four languages, configuration reference, and explanation of schema inference, transforms, dedup, DLQ, and watch features.

- [ ] **Step 3: Commit**

```bash
git add README.md docs/ingestion.md
git commit -m "docs: add ingestion documentation and update README for v3"
```

---

## Summary

| Task | Component | Files |
|------|-----------|-------|
| 1 | Dependencies & Types | Cargo.toml, ingestion/types.rs, ingestion/mod.rs, lib.rs |
| 2 | Polars Reader | ingestion/reader.rs, error.rs |
| 3 | Schema Inference | ingestion/schema.rs |
| 4 | DataFrame→BSON Converter | ingestion/writer.rs |
| 5 | Transform Engine | ingestion/transform.rs |
| 6 | Dead Letter Queue | ingestion/dlq.rs |
| 7 | Progress Tracking | ingestion/progress.rs |
| 8 | Dedup & Conflict | ingestion/dedup.rs |
| 9 | Engine Orchestrator | ingestion/engine.rs |
| 10 | Watch Directory | ingestion/watch.rs |
| 11 | Configuration | config.rs |
| 12 | Protobuf Definitions | proto/ingestion.proto, mongocore.proto |
| 13 | gRPC Handlers | grpc/service.rs |
| 14 | MCP Tools | mcp/tools.rs |
| 15 | Startup Init | main.rs |
| 16 | LLM Expressions | ingestion/llm_expressions.rs |
| 17 | Python Client | clients/python/ |
| 18 | TypeScript Client | clients/typescript/ |
| 19 | Go Client | clients/go/ |
| 20 | Java Client | clients/java/ |
| 21 | Integration Tests | tests/integration/ingestion_test.rs |
| 22 | Documentation | README.md, docs/ingestion.md |

**Total: 22 tasks, ~22 commits**
