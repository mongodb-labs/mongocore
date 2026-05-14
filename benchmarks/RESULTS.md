# Benchmark Results

Generated: 2026-05-14 16:34 UTC

![Native vs MongoCore Overhead](results/charts/sidecar_overhead.svg)

## Single-Document Operations

| Benchmark | Language | Native (ops/s) | MongoCore (ops/s) | Overhead |
|-----------|----------|---------------:|------------------:|---------:|
| find_one_by_id | Python | 4438 | 2418 | +46% |
| find_one_by_id | Typescript | 4352 | 2682 | +38% |
| find_one_by_id | Go | 5854 | 3390 | +42% |
| find_one_by_id | Java | 5558 | 3430 | +38% |
| insert_one_large | Python | 42 | 39 | +8% |
| insert_one_large | Typescript | 45 | 38 | +16% |
| insert_one_large | Go | 46 | 38 | +17% |
| insert_one_large | Java | 31 | 31 | -0% |
| insert_one_small | Python | 4288 | 1286 | +70% |
| insert_one_small | Typescript | 1814 | 1440 | +21% |
| insert_one_small | Go | 2226 | 1499 | +33% |
| insert_one_small | Java | 2125 | 1400 | +34% |
| run_command | Python | 5094 | 2628 | +48% |
| run_command | Typescript | 4872 | 2933 | +40% |
| run_command | Go | 6081 | 3607 | +41% |
| run_command | Java | 6002 | 3849 | +36% |

## Multi-Document Operations

| Benchmark | Language | Native (ops/s) | MongoCore (ops/s) | Overhead |
|-----------|----------|---------------:|------------------:|---------:|
| bulk_insert_large | Python | 50 | 42 | +15% |
| bulk_insert_large | Go | — | 42 | — |
| bulk_insert_small | Python | 172.1K | 113.5K | +34% |
| bulk_insert_small | Typescript | 215.9K | 137.0K | +37% |
| bulk_insert_small | Go | 142.5K | 148.5K | -4% |
| bulk_insert_small | Java | 187.1K | 147.5K | +21% |
| find_many | Python | 421.8K | 245.0K | +42% |
| find_many | Typescript | 347.6K | 249.7K | +28% |
| find_many | Go | 248.5K | 182.7K | +26% |
| find_many | Java | 522.4K | 227.7K | +56% |
| find_many_large | Python | 57 | 48 | +15% |
| find_many_large | Go | — | 45 | — |

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

| Benchmark | Driver | Ops/s | MB/s | p50 (s) |
|-----------|--------|------:|-----:|--------:|
| mongocore_ingest_100mb_csv | mongocore+polars | 0 | 10.77 | 7.788 |
| mongocore_ingest_100mb_ndjson | mongocore+polars | 0 | 17.24 | 5.563 |
| mongocore_ingest_10mb_csv | mongocore+polars | 1 | 10.69 | 0.777 |
| mongocore_ingest_10mb_ndjson | mongocore+polars | 2 | 16.45 | 0.579 |
| mongocore_ingest_1mb_csv | mongocore+polars | 9 | 7.76 | 0.106 |
| mongocore_ingest_1mb_ndjson | mongocore+polars | 9 | 8.66 | 0.109 |
| native_bulk_100mb_csv | pymongo_native | 199.4K | 20.08 | 4.178 |
| native_bulk_100mb_ndjson | pymongo_native | 182.1K | 31.43 | 3.050 |
| native_bulk_10mb_csv | pymongo_native | 194.8K | 19.42 | 0.428 |
| native_bulk_10mb_ndjson | pymongo_native | 187.8K | 32.23 | 0.296 |
| native_bulk_1mb_csv | pymongo_native | 178.8K | 17.65 | 0.047 |
| native_bulk_1mb_ndjson | pymongo_native | 167.3K | 28.54 | 0.033 |

## Environment

- **OS:** darwin (arm64)
- **CPUs:** 12
- **MongoCore:** 0.6.0
