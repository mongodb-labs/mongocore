# Benchmark Results

Generated: 2026-05-14 16:49 UTC

![Native vs MongoCore Overhead](results/charts/sidecar_overhead.svg)

## Driver Operations

MongoCore ops/s per language vs fastest native driver.

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

Pipeline sends multiple operations in a single gRPC call. Native column shows the equivalent single-call benchmark for comparison.

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

![Ingestion Performance](results/charts/ingestion_performance.svg)

| Operation | Size | MongoCore (MB/s) | Native (MB/s) | p50 (s) |
|-----------|-----:|-----------------:|--------------:|--------:|
| csv | 1mb | 7.76 | 17.65 | 0.106 |
| ndjson | 1mb | 8.66 | 28.54 | 0.109 |
| csv | 10mb | 10.69 | 19.42 | 0.777 |
| ndjson | 10mb | 16.45 | 32.23 | 0.579 |
| csv | 100mb | 10.77 | 20.08 | 7.788 |
| ndjson | 100mb | 17.24 | 31.43 | 5.563 |

## Environment

- **OS:** darwin (arm64)
- **CPUs:** 12
- **MongoCore:** 0.6.0
