# Benchmark Results

Generated: 2026-05-14 19:12 UTC

![Native vs MongoCore Overhead](results/charts/sidecar_overhead.svg)

## Driver Operations

MongoCore ops/s per language vs fastest native driver.

**Methodology:** Each benchmark performs 10,000 operations per iteration (single-doc) or 10,000 documents per batch (multi-doc) using the language-specific client library. The native driver connects directly to MongoDB; MongoCore goes through the gRPC sidecar. Both use localhost connections, default connection pools, and `w:1` write concern. The overhead column shows how much slower MongoCore is vs the fastest native driver — this represents the pure sidecar cost (gRPC serialization + proxy hop) with no network latency to amortize it.

| Operation | Python | TypeScript | Go | Java | Fastest Native | Overhead |
|-----------|-------:|-----------:|---:|-----:|---------------:|---------:|
| find_one_by_id | 2418 | 2682 | 3390 | 3430 | 5854 | +49% |
| insert_one_large | 39 | 38 | 38 | 31 | 46 | +21% |
| insert_one_small | 1286 | 1440 | 1499 | 1400 | 4288 | +67% |
| run_command | 2628 | 2933 | 3607 | 3849 | 6081 | +46% |
| bulk_insert_large | 42 | — | 42 | — | 50 | +15% |
| bulk_insert_small | 113.5K | 137.0K | 148.5K | 147.5K | 215.9K | +37% |
| find_many | 245.0K | 249.7K | 182.7K | 227.7K | 522.4K | +57% |
| find_many_large | 48 | — | 45 | — | 57 | +18% |

## Pipeline Batching

**Methodology:** Pipeline sends N operations in a single gRPC call, reducing round-trip overhead. Each iteration performs 10,000 total operations split into batches of the given size (e.g. batch 1000 = 10 pipeline calls of 1000 ops each). The "Fastest Native" column shows the equivalent operation executed one-at-a-time via the fastest native driver — the speedup demonstrates the benefit of batching multiple operations into fewer network calls.

![Pipeline Batching Performance](results/charts/pipeline_performance.svg)

| Operation | Batch Size | Python | TypeScript | Go | Java | Fastest Native | Speedup |
|-----------|----------:|-------:|-----------:|---:|-----:|---------------:|--------:|
| find_one_by_id | 100 | 15.1K | 14.0K | 16.0K | 16.7K | 5854 | 2.6x |
| find_one_by_id | 1000 | 16.3K | 14.9K | 17.1K | 17.6K | 5854 | 2.8x |
| find_one_by_id | 10000 | 16.5K | 14.9K | 17.1K | 17.7K | 5854 | 2.8x |
| insert_one_small | 100 | 6797 | 7020 | 6943 | 7237 | 4288 | 1.6x |
| insert_one_small | 1000 | 7341 | 7476 | 7267 | 7620 | 4288 | 1.7x |
| insert_one_small | 10000 | 7449 | 7656 | 7368 | 7690 | 4288 | 1.8x |
| run_command | 100 | 16.3K | 15.7K | 17.7K | 17.7K | 6081 | 2.8x |
| run_command | 1000 | 17.9K | 16.8K | 18.8K | 19.6K | 6081 | 3.0x |
| run_command | 10000 | 18.2K | 16.8K | 19.2K | 19.7K | 6081 | 3.0x |

## Ingestion

**Methodology:** End-to-end file-to-collection ingestion. Both sides perform the same work: read file from disk, parse CSV/NDJSON, optionally apply transforms (cast types, filter rows, rename columns, drop columns), then batch insert into MongoDB with 4 concurrent writers and batch size of 1,000 documents. The native benchmark uses Python's `csv`/`json` stdlib for parsing and `ThreadPoolExecutor(4)` for concurrent pymongo inserts. MongoCore uses Polars (Rust) for parsing and vectorized transforms, with 4 concurrent tokio tasks for writes — all triggered by a single gRPC call. The speedup comes from Polars' columnar processing and Rust-native I/O, especially at larger row counts where Python per-row overhead dominates.

![Ingestion Performance](results/charts/ingestion_performance.svg)

| Scenario | Format | Size | Native (MB/s) | MongoCore (MB/s) | Speedup |
|----------|--------|-----:|--------------:|-----------------:|--------:|
| ingest | csv | 10k | 11.20 | 17.79 | 1.6x |
| ingest | ndjson | 10k | 18.78 | 15.79 | 0.8x |
| ingest | csv | 100k | 13.49 | 32.05 | 2.4x |
| ingest | ndjson | 100k | 16.83 | 44.92 | 2.7x |
| ingest | csv | 500k | 13.27 | 40.62 | 3.1x |
| ingest | ndjson | 500k | 16.31 | 51.50 | 3.2x |
| ingest + transform | csv | 10k | 11.45 | 15.90 | 1.4x |
| ingest + transform | ndjson | 10k | 17.69 | 28.38 | 1.6x |
| ingest + transform | csv | 100k | 13.21 | 36.25 | 2.7x |
| ingest + transform | ndjson | 100k | 17.91 | 58.40 | 3.3x |
| ingest + transform | csv | 500k | 12.68 | 41.98 | 3.3x |
| ingest + transform | ndjson | 500k | 16.84 | 65.36 | 3.9x |

## Environment

- **OS:** darwin (arm64)
- **CPUs:** 12
- **MongoCore:** 0.6.0
