# MongoCore v3: Intelligent Data Ingestion

## Overview

A Polars-based data ingestion engine built into the MongoCore Rust sidecar. Reads static files (CSV, JSON, Parquet), infers BSON-compatible schemas via multi-row sampling, applies optional user-provided Polars expressions (including LLM-powered ones when an API key is configured), and bulk-writes to MongoDB with deduplication, conflict resolution, progress tracking, resumability, and a dead letter queue.

## Design Principles

- **Native-first** — The default ingestion path requires no LLM. Polars handles I/O, type inference, and parallel execution natively in Rust.
- **LLM as optional expressions** — LLM capabilities are exposed as expression functions (`llm_classify`, `llm_extract`, `llm_normalize`, `llm_embed`), not as a required pipeline stage. Available only when an API key is configured.
- **Spark-inspired schema inference** — Multi-row sampling with type widening and conflict resolution, modeled on the MongoDB Spark Connector's `InferSchema` algorithm.
- **Compiled transforms (implicit)** — Transform specs are cached by source schema hash. Same file shape = automatic cache hit. No user-facing "named transform" concept.
- **Parallel by default** — Polars columnar parallelism for reads/transforms, concurrent bulk writes for MongoDB insertion.

## Architecture

```
┌─────────────┐     ┌──────────────────────────────────────────────────┐     ┌──────────┐
│ File Source  │────▶│              Ingestion Engine                     │────▶│ MongoDB  │
│ (CSV/JSON/  │     │                                                  │     └──────────┘
│  Parquet)   │     │  ┌──────────┐  ┌──────────┐  ┌─────────────┐   │
├─────────────┤     │  │  Polars  │  │ Schema   │  │   Bulk      │   │     ┌──────────┐
│ Watch Dir   │────▶│  │  Reader  │→ │ Infer +  │→ │   Writer    │   │────▶│   DLQ    │
└─────────────┘     │  │          │  │ Transform│  │             │   │     └──────────┘
                    │  └──────────┘  └──────────┘  └─────────────┘   │
                    │       ↑              ↑              ↑           │
                    │  Format Detect   Expressions   Dedup/Conflict  │
                    │                      ↑                          │
                    │              ┌────────────────┐                 │
                    │              │ LLM Expressions │ (optional)     │
                    │              │ (if API key set)│                 │
                    │              └────────────────┘                 │
                    │  ┌─────────────────────────────────────────┐   │
                    │  │       Transform Cache (L1/L2/L3)         │   │
                    │  └─────────────────────────────────────────┘   │
                    └──────────────────────────────────────────────────┘
```

## Components

### 1. Polars Reader

Reads source files into Polars LazyFrames based on detected format.

- **Format detection:** Extension-based with magic byte fallback (`.csv`, `.json`/`.ndjson`, `.parquet`)
- **Lazy scanning:** Uses `LazyCsvReader`, `LazyJsonLineReader`, `scan_parquet` for memory-efficient streaming
- **Sampling:** Reads min 1000 rows (configurable) for schema inference before full execution

### 2. Schema Inference Engine

Rust implementation inspired by the MongoDB Spark Connector's `InferSchema`:

**Algorithm:**
1. Sample N rows (min 1000, configurable via `sample_size`)
2. For each row, infer per-column types
3. Reduce all row schemas into a unified schema via compatible-type resolution
4. Output a `BsonSchema` (column names, BSON types, nullability)

**Polars-to-BSON type mapping:**

| Polars Type | BSON Type |
|-------------|-----------|
| Utf8/String | String |
| Int32 | Int32 |
| Int64 | Int64 |
| Float32/Float64 | Double |
| Boolean | Boolean |
| Date/Datetime | DateTime |
| Null | Null |
| Struct | Document (recursive) |
| List | Array (element type inferred) |
| Binary | Binary |

**Type conflict resolution (priority order):**
1. Same type → keep
2. Int32 + Int64 → Int64 (widen)
3. Any numeric + Float → Double (widen)
4. Struct + Struct → recursive merge
5. Array + Array → merge element types
6. Incompatible (e.g., Int + String) → String (universal fallback)
7. Field missing in some rows → mark nullable

### 3. Transform Engine

Applies user-provided transformations expressed as Polars operations.

**User-provided expressions (optional):**
- Column renames: `rename({"old_name": "new_name"})`
- Filters: `filter(col("amount") > 0)`
- Derived columns: `with_columns(col("price") * col("qty")).alias("total")`
- Type overrides: `cast({"date_str": Datetime})`
- Drop columns: `drop(["internal_id", "debug"])`

These are passed as part of the ingestion request and compiled into Polars lazy operations.

**Schema overrides:**
Users can provide explicit `column → BSON type` overrides that take precedence over inference.

### 4. LLM Expression Functions (Optional)

Available only when `llm_provider` and API key are configured. Implemented as custom Polars expression functions in Rust, reusing the existing LLM provider infrastructure from v1/v2.

| Expression | Purpose | Example |
|-----------|---------|---------|
| `llm_classify(col, categories)` | Classify text into categories | `llm_classify(col("desc"), ["electronics", "clothing"])` |
| `llm_extract(col, schema)` | Extract structured data from text | `llm_extract(col("notes"), {"amount": "float", "currency": "str"})` |
| `llm_normalize(col)` | Semantic normalization | `llm_normalize(col("company"))` → "IBM Corp" → "IBM" |
| `llm_embed(col)` | Generate vector embeddings | `llm_embed(col("description"))` → List[Float64] |

**Implementation details:**
- Batch processing: accumulate column values, send as batch to LLM provider
- Configurable concurrency: `max_llm_concurrency` (default 4)
- Caching: identical inputs produce cached outputs (reuses compiled query cache infra)
- Error handling: LLM failures on individual rows → null value + warning, not job failure
- Uses existing Voyage AI client for embeddings when configured

**When no API key:** Using LLM expressions returns a clear error at job start (validation phase), not a runtime failure mid-ingestion.

### 5. Bulk Writer

Converts transformed DataFrames to BSON documents and writes to MongoDB.

**Process:**
1. Polars collects LazyFrame in configurable chunk sizes (default 1000 docs per batch)
2. Each chunk: DataFrame rows → `Vec<bson::Document>` via the inferred/override schema
3. Dedup check (if key configured): batch query for existing docs matching dedup key values
4. Apply conflict strategy for duplicates
5. `bulkWrite` to MongoDB (ordered=false for throughput)
6. Failed docs → Dead Letter Queue
7. Update progress tracker

**Parallelism:** Configurable write concurrency (default 4 parallel bulk write tasks).

### 6. Deduplication & Conflict Resolution

**Dedup key:**
- User-provided field(s) in the ingestion request (e.g., `dedup_key: ["email"]`)
- If not specified → no dedup, straight insert (fastest path)
- Dedup check: before each batch, query existing docs by dedup key values

**Conflict strategies (per-ingestion config):**

| Strategy | Behavior |
|----------|----------|
| `skip` (default) | Duplicate found → skip incoming doc |
| `overwrite` | Duplicate found → replace existing doc entirely |
| `merge` | Duplicate found → shallow merge (incoming fields overwrite, existing-only fields preserved) |

### 7. Dead Letter Queue

Documents that fail at any stage are routed to `__mongocore.dead_letter`:

```json
{
  "job_id": "abc-123",
  "source_row": 4582,
  "document": { ... },
  "error": "Write conflict: duplicate key on email field",
  "stage": "bulk_write",
  "timestamp": "2026-05-11T14:30:00Z"
}
```

Queryable by job_id for inspection and retry.

### 8. Progress Tracking & Resumability

**Job state** persisted to `__mongocore.ingestion_jobs`:

```json
{
  "job_id": "abc-123",
  "file_path": "/data/orders.csv",
  "database": "myapp",
  "collection": "orders",
  "status": "running",
  "total_rows": 500000,
  "rows_processed": 125000,
  "rows_inserted": 124800,
  "rows_skipped": 150,
  "rows_failed": 50,
  "last_committed_chunk": 125,
  "started_at": "2026-05-11T14:00:00Z",
  "options": { ... }
}
```

**Resumability:** On restart or crash recovery, jobs with status "running" are detected. Ingestion resumes from `last_committed_chunk * batch_size` offset in the source file.

### 9. Watch Directory Service

Monitors a filesystem path for new files and auto-triggers ingestion.

- Uses the `notify` crate for cross-platform filesystem event watching
- Configurable: path, file glob pattern, target database/collection, conflict strategy
- New file detected → triggers Ingest with pre-configured defaults
- Debounce: waits for file to stop being written (no modification for 2s) before triggering

## Interfaces

### gRPC RPCs

```protobuf
service MongoCore {
  // ... existing RPCs ...

  // Ingestion
  rpc Ingest(IngestRequest) returns (IngestResponse);
  rpc GetIngestStatus(GetIngestStatusRequest) returns (GetIngestStatusResponse);
  rpc ListIngestJobs(ListIngestJobsRequest) returns (ListIngestJobsResponse);
  rpc CancelIngest(CancelIngestRequest) returns (CancelIngestResponse);
  rpc WatchDirectory(WatchDirectoryRequest) returns (WatchDirectoryResponse);
  rpc StopWatch(StopWatchRequest) returns (StopWatchResponse);
}
```

**IngestRequest:**

| Field | Type | Description |
|-------|------|-------------|
| `file_path` | string | Path to source file (sidecar-accessible) |
| `database` | string | Target database |
| `collection` | string | Target collection |
| `format` | enum | AUTO, CSV, JSON, NDJSON, PARQUET |
| `dedup_key` | repeated string | Fields for dedup (empty = no dedup) |
| `conflict_strategy` | enum | SKIP, OVERWRITE, MERGE |
| `batch_size` | int32 | Docs per bulk write (default 1000) |
| `concurrency` | int32 | Parallel write tasks (default 4) |
| `expressions` | repeated string | Polars expressions to apply |
| `schema_overrides` | map<string,string> | Column → BSON type overrides |
| `sample_size` | int32 | Rows for inference (min/default 1000) |
| `csv_options` | CsvOptions | Delimiter, quote char, has_header, etc. |

**IngestResponse:**

| Field | Type | Description |
|-------|------|-------------|
| `job_id` | string | Unique job identifier |
| `status` | enum | RUNNING, COMPLETED, FAILED, CANCELLED |
| `inferred_schema` | map<string,string> | Detected column → BSON type mapping |
| `total_rows` | int64 | Total rows detected in source |

**GetIngestStatusResponse:**

| Field | Type | Description |
|-------|------|-------------|
| `job_id` | string | Job identifier |
| `status` | enum | RUNNING, COMPLETED, FAILED, CANCELLED |
| `total_rows` | int64 | Total source rows |
| `rows_processed` | int64 | Rows processed so far |
| `rows_inserted` | int64 | Successfully inserted |
| `rows_skipped` | int64 | Skipped (dedup) |
| `rows_failed` | int64 | Failed (sent to DLQ) |
| `elapsed_ms` | int64 | Time elapsed |
| `estimated_remaining_ms` | int64 | Estimated time remaining |

**WatchDirectoryRequest:**

| Field | Type | Description |
|-------|------|-------------|
| `path` | string | Directory to watch |
| `file_pattern` | string | Glob pattern (e.g., "*.csv") |
| `database` | string | Target database |
| `collection` | string | Target collection |
| `conflict_strategy` | enum | Default conflict strategy |
| `dedup_key` | repeated string | Default dedup key |

### MCP Tools

| Tool | Description |
|------|-------------|
| `ingest` | Start an ingestion job |
| `ingest_status` | Check job progress |
| `list_ingest_jobs` | List active/recent jobs |
| `cancel_ingest` | Cancel a running job |
| `watch_directory` | Start watching a directory |
| `stop_watch` | Stop watching a directory |

### Client Library Methods

All four client libraries (Python, TypeScript, Go, Java) will expose:

```python
# Python example
job = await client.ingest(
    file_path="/data/orders.csv",
    database="myapp",
    collection="orders",
    dedup_key=["order_id"],
    conflict_strategy="merge",
)
status = await client.ingest_status(job.job_id)
```

## Configuration

```toml
[ingestion]
enabled = true                    # Enable ingestion engine (default true)
sample_size = 1000                # Min rows for schema inference
default_batch_size = 1000         # Docs per bulk write
default_concurrency = 4           # Parallel write tasks
max_file_size_mb = 10240          # Max file size (10GB default)
llm_expressions = false           # Enable LLM expression functions (default off)
max_llm_concurrency = 4           # Max concurrent LLM calls
watch_debounce_ms = 2000          # File stable time before triggering

[ingestion.watch]
enabled = false
path = ""
file_pattern = "*.csv"
database = ""
collection = ""
conflict_strategy = "skip"
```

## Dependencies

New Rust crates required:

| Crate | Purpose |
|-------|---------|
| `polars` | DataFrame engine (features: lazy, csv, json, parquet) |
| `notify` | Filesystem event watching |

Both are well-maintained, pure-Rust crates that integrate with the existing tokio async runtime.

## Error Handling

| Error Condition | Behavior |
|----------------|----------|
| File not found / unreadable | Job fails immediately with clear error |
| Unsupported format | Job fails at detection phase |
| Schema inference failure (0 valid rows) | Job fails with "no valid data" |
| LLM expression used without API key | Job fails at validation (before processing) |
| Individual row conversion failure | Row → DLQ, job continues |
| Bulk write partial failure | Failed docs → DLQ, successful docs committed |
| MongoDB connection lost mid-job | Job paused, auto-retry with backoff, resumable |
| File modified during ingestion | Warning logged, processes snapshot at start time |

## Testing Strategy

| Test Type | Scope |
|-----------|-------|
| Unit tests | Schema inference logic, type mapping, expression compilation, conflict resolution |
| Integration tests | End-to-end: CSV/JSON/Parquet → MongoDB with real Polars + real MongoDB |
| DLQ tests | Verify failed docs routed correctly with metadata |
| Resumability tests | Kill mid-ingestion, restart, verify picks up correctly |
| Watch tests | Drop file in directory, verify auto-ingestion triggers |
| Performance tests | 1M row CSV ingestion latency and throughput |

## v3.1 Future Enhancements (Documented, Not Implemented)

The following capabilities are explicitly deferred to a future v3.1 release:

- **LLM-inferred dedup keys** — LLM examines source schema and sample data to suggest which field(s) represent unique identity
- **LLM per-document conflict resolution** — For ambiguous merge cases, LLM examines both the existing and incoming document to decide the best resolution
- **Semantic deduplication** — Recognizing semantically equivalent values as duplicates (e.g., "IBM Corp" and "International Business Machines")
  - Note: partially achievable in v3 by using `llm_normalize()` on the dedup key column before dedup runs
- **API source ingestion** — Pulling data from REST/GraphQL APIs as sources (v3 focuses on static files)
- **Streaming/incremental ingestion** — Processing append-only sources that grow over time
