# Data Ingestion

MongoCore includes a Polars-powered data ingestion engine that loads CSV, JSON, NDJSON, and Parquet from local files or remote URLs (HTTP/HTTPS, S3, GCS, Azure Blob) into MongoDB with parallel processing, schema inference, transforms, deduplication, and progress tracking.

## Overview

The ingestion pipeline:

1. **Read** — Polars reads the source file (CSV, JSON, NDJSON, Parquet)
2. **Infer schema** — Sample rows to determine BSON types
3. **Transform** — Apply user-defined Polars expressions (rename, filter, cast, drop, select)
4. **Deduplicate** — Detect duplicates by key with configurable conflict resolution
5. **Write** — Batch-insert documents into the target collection
6. **Dead letter** — Route failed documents to `__mongocore.dead_letter`

All steps run in parallel where possible, and jobs are resumable on crash.

## Quick Start

### Python

```python
from mongocore import MongoClient

async with MongoClient("localhost:50051") as client:
    # Ingest a CSV file
    job = await client.ingest(
        file="./data/customers.csv",
        database="myapp",
        collection="customers",
        transforms=["cast(age, int32)", "rename(email_addr, email)"],
        dedup_key="email",
    )
    print(f"Job {job.id} started, {job.total_rows} rows")

    # Check status
    status = await client.get_ingest_status(job.id)
    print(f"{status.rows_inserted}/{status.total_rows} inserted")
```

### TypeScript

```typescript
import { MongoClient } from '@mongocore/client';

const client = new MongoClient('localhost:50051');
await client.connect();

const job = await client.ingest({
  file: './data/customers.csv',
  database: 'myapp',
  collection: 'customers',
  transforms: ['cast(age, int32)', 'rename(email_addr, email)'],
  dedupKey: 'email',
});
console.log(`Job ${job.id}: ${job.totalRows} rows`);

const status = await client.getIngestStatus(job.id);
console.log(`${status.rowsInserted}/${status.totalRows} inserted`);
```

### Go

```go
client := mongocore.NewClient("localhost:50051")
client.Connect(ctx)

job, _ := client.Ingest(ctx, &mongocore.IngestRequest{
    File:       "./data/customers.csv",
    Database:   "myapp",
    Collection: "customers",
    Transforms: []string{"cast(age, int32)", "rename(email_addr, email)"},
    DedupKey:   "email",
})
fmt.Printf("Job %s: %d rows\n", job.ID, job.TotalRows)

status, _ := client.GetIngestStatus(ctx, job.ID)
fmt.Printf("%d/%d inserted\n", status.RowsInserted, status.TotalRows)
```

### Java

```java
try (MongoClient client = MongoClient.create("localhost:50051")) {
    IngestJob job = client.ingest(IngestRequest.builder()
        .file("./data/customers.csv")
        .database("myapp")
        .collection("customers")
        .transforms(List.of("cast(age, int32)", "rename(email_addr, email)"))
        .dedupKey("email")
        .build());
    System.out.printf("Job %s: %d rows%n", job.getId(), job.getTotalRows());

    IngestStatus status = client.getIngestStatus(job.getId());
    System.out.printf("%d/%d inserted%n", status.getRowsInserted(), status.getTotalRows());
}
```

## Configuration

Add an `[ingestion]` section to your `config.toml`:

```toml
[ingestion]
# Maximum rows to sample for schema inference (default: 1000)
schema_sample_size = 1000

# Batch size for inserts (default: 5000)
batch_size = 5000

# Number of parallel workers (default: num_cpus)
parallelism = 4

# Dead letter collection (default: "__mongocore.dead_letter")
dead_letter_collection = "__mongocore.dead_letter"

# Directory watch poll interval in seconds (default: 5)
watch_interval_secs = 5

# Optional: Set API key in config or environment for llm_* expressions
# ANTHROPIC_API_KEY = "your-api-key-here"
```

## Schema Inference

MongoCore samples rows from the source file and infers BSON types using a strategy inspired by the MongoDB Spark Connector.

### How it Works

1. Sample up to `schema_sample_size` rows (default 1000)
2. For each column, collect all observed data types
3. Apply majority-vote type selection with nullable fallback
4. Map to the closest BSON type

### Type Mapping

| Source Type | BSON Type | Notes |
|-------------|-----------|-------|
| Integer (i32) | `int32` | Small integers |
| Integer (i64) | `int64` | Large integers |
| Float (f64) | `double` | All floating point |
| Boolean | `bool` | |
| String | `string` | Default fallback |
| Date / DateTime | `date` | ISO-8601 strings auto-detected |
| List | `array` | Nested arrays |
| Struct | `document` | Nested objects |
| Null | `null` | Nullable columns |
| Binary | `binData` | Raw byte columns |

If a column has mixed types, MongoCore falls back to `string` and logs a warning.

## Transform Expressions

Transforms are Polars expressions specified as strings. They run in order after schema inference and before insertion.

### Supported Operations

| Expression | Description | Example |
|-----------|-------------|---------|
| `rename(old, new)` | Rename a column | `rename(email_addr, email)` |
| `cast(col, type)` | Cast column to type | `cast(age, int32)` |
| `filter(expr)` | Keep rows matching condition | `filter(age > 18)` |
| `drop(col)` | Remove a column | `drop(internal_id)` |
| `select(col1, col2, ...)` | Keep only listed columns | `select(name, email, age)` |
| `add(col, expr)` | Add a computed column | `add(full_name, concat(first, " ", last))` |
| `replace_null(col, value)` | Fill nulls with a default | `replace_null(age, 0)` |

### Examples

```toml
# In config.toml or passed via RPC
transforms = [
    "rename(email_address, email)",
    "cast(created_at, date)",
    "filter(status != 'deleted')",
    "drop(internal_notes)",
    "add(name_upper, upper(name))",
]
```

## LLM Expressions (Optional)

When an LLM API key is configured, additional transform expressions become available:

| Expression | Description | Example |
|-----------|-------------|---------|
| `llm_classify(col, labels...)` | Classify text into categories | `llm_classify(feedback, positive, negative, neutral)` |
| `llm_extract(col, field)` | Extract structured data from text | `llm_extract(description, price)` |
| `llm_normalize(col, format)` | Normalize messy values | `llm_normalize(phone, E.164)` |
| `llm_embed(col)` | Generate vector embedding | `llm_embed(description)` |

LLM expressions are applied per-batch and cached to minimize API calls. They add a new column with the result (original column is preserved).

```python
job = await client.ingest(
    file="./data/reviews.csv",
    database="myapp",
    collection="reviews",
    transforms=[
        "llm_classify(text, positive, negative, neutral)",
        "llm_embed(text)",
    ],
)
```

## Deduplication & Conflict Resolution

When `dedup_key` is specified, MongoCore checks for existing documents with the same key value before inserting.

### Strategies

| Strategy | Behavior |
|----------|----------|
| `skip` (default) | Skip the incoming document if key already exists |
| `overwrite` | Replace the existing document entirely |
| `merge` | Shallow-merge incoming fields into the existing document |

### Usage

```python
job = await client.ingest(
    file="./data/customers.csv",
    database="myapp",
    collection="customers",
    dedup_key="email",
    dedup_strategy="merge",  # or "skip", "overwrite"
)
```

For compound keys, pass a comma-separated string:

```python
dedup_key="tenant_id,email"
```

## Dead Letter Queue

Documents that fail validation, transform errors, or insertion errors are routed to the dead letter collection (`__mongocore.dead_letter` by default).

Each dead letter document contains:

```json
{
  "job_id": "abc-123",
  "source_file": "./data/customers.csv",
  "row_index": 42,
  "error": "cast failed: 'not_a_number' cannot convert to int32",
  "original_document": { "name": "Bob", "age": "not_a_number" },
  "timestamp": "2026-05-11T10:30:00Z"
}
```

Query dead letters for a specific job:

```python
dead = client["__mongocore"]["dead_letter"]
failures = await dead.find({"job_id": job.id})
```

## Progress Tracking & Resumability

Every ingestion job tracks progress in real time:

```python
status = await client.get_ingest_status(job.id)
# IngestStatus:
#   id: "abc-123"
#   state: "running" | "completed" | "failed" | "cancelled"
#   total_rows: 100000
#   rows_inserted: 45000
#   rows_failed: 3
#   rows_skipped: 12  (dedup)
#   elapsed_secs: 8.2
#   file: "./data/customers.csv"
```

### Resumability

If MongoCore crashes or is restarted mid-ingestion, incomplete jobs are detected on startup. Jobs resume from the last committed batch offset, avoiding re-insertion of already-written documents.

### List and Cancel Jobs

```python
# List all jobs
jobs = await client.list_ingest_jobs()

# Cancel a running job
await client.cancel_ingest(job.id)
```

## Directory Watching

MongoCore can watch a directory and auto-trigger ingestion when new files appear:

```python
# Start watching
watch = await client.watch_directory(
    path="./incoming/",
    database="myapp",
    collection="events",
    pattern="*.ndjson",          # optional glob filter
    transforms=["cast(ts, date)"],
)
print(f"Watch ID: {watch.id}")

# Stop watching
await client.stop_watch(watch.id)
```

Watched directories are polled at the configured interval (default 5 seconds). Each new file triggers a separate ingestion job. Processed files are tracked to avoid re-ingestion.

## gRPC RPCs

| RPC | Description |
|-----|-------------|
| `Ingest` | Start a new ingestion job |
| `GetIngestStatus` | Get current status of a job |
| `ListIngestJobs` | List all ingestion jobs |
| `CancelIngest` | Cancel a running job |
| `WatchDirectory` | Start watching a directory for new files |
| `StopWatch` | Stop a directory watch |

## MCP Tools

| Tool | Description |
|------|-------------|
| `ingest` | Start a file ingestion job to load data into a collection |
| `ingest_status` | Get the status of an ingestion job |
| `list_ingest_jobs` | List all ingestion jobs |
| `cancel_ingest` | Cancel a running ingestion job |
| `watch_directory` | Watch a directory for new files and auto-ingest them |
| `stop_watch` | Stop watching a directory |
| `ingest_and_embed` | Parse a file, embed a text field, and store with vectors |

## Error Handling

| Error | Cause | Resolution |
|-------|-------|------------|
| `FileNotFound` | Source file does not exist | Check file path |
| `UnsupportedFormat` | File extension not recognized | Use .csv, .json, .ndjson, or .parquet |
| `SchemaInferenceFailure` | Cannot determine types from sample | Increase `schema_sample_size` or add explicit casts |
| `TransformError` | Invalid transform expression | Check expression syntax |
| `DedupKeyMissing` | Specified dedup key not found in data | Verify column name matches source |
| `LlmUnavailable` | LLM expression used without API key | Set ANTHROPIC_API_KEY in config or environment |
| `BatchInsertFailure` | MongoDB rejected a batch | Check dead letter queue for details |
| `JobCancelled` | Job was cancelled by user | Re-run if needed |
