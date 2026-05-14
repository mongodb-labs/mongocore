# MongoCore Benchmarks

Cross-language performance benchmarks comparing native MongoDB drivers against MongoCore sidecar.

**[View latest results](RESULTS.md)**

## Quick Start

```bash
just bench-all        # Run all benchmarks (skips any with existing results)
just bench-report     # Generate RESULTS.md from current results
just bench-clean      # Delete all results to force a full rerun
```

## What's Measured

- **Single-doc ops:** run_command, find_one_by_id, insert_one_small, insert_one_large
- **Multi-doc ops:** bulk_insert_small, bulk_insert_large, find_many, find_many_large
- **Ingestion:** Polars pipeline vs native bulk insert (1MB, 10MB, 100MB)
- **Pipeline batching:** pipeline_run_command, pipeline_insert_one_small, pipeline_find_one_by_id (at 100/1K/10K batch sizes)
- **Compiled query:** cache hit latency

## Languages

| Language | Native Driver | MongoCore Client |
|----------|--------------|-----------------|
| Python | pymongo | mongocore+python (gRPC) |
| TypeScript | mongodb (official) | mongocore+typescript (gRPC) |
| Go | go.mongodb.org/mongo-driver/v2 | mongocore+go (gRPC) |
| Java | mongodb-driver-sync | mongocore+java (gRPC) |

## Methodology

Follows the [MongoDB Driver Benchmarking Specification](https://github.com/mongodb/specifications/blob/master/source/benchmarking/benchmarking.md):
- Batched iterations (10K ops per iteration for single-doc, 10K docs per batch for multi-doc)
- Minimum 1 second of measurement time per benchmark
- Per-language warmup iterations (Python 3, TS 5, Go 3, Java 10)
- Percentile reporting (p10, p25, p50, p75, p90, p95, p99)
- All tests against `mongodb/mongodb-atlas-local` on localhost

## Caveats

These benchmarks provide a directional comparison, not absolute performance numbers. Key caveats:

1. **Uncontrolled environment.** Benchmarks run on a developer workstation with no attempt to disable CPU frequency scaling, isolate cores, or eliminate background noise. Results will vary between runs.
2. **No tuning.** Neither the native drivers nor MongoCore have been tuned for optimal throughput (e.g. connection pool sizes, batch sizes, write concerns are all defaults).
3. **Localhost only.** Running against a local Docker MongoDB eliminates network latency — the dominant cost in production. These numbers isolate sidecar overhead, not end-to-end performance.
4. **gRPC message limits.** MongoCore benchmarks skip `bulk_insert_large` and `find_many_large` due to the default 4MB gRPC message size limit. Native drivers have no such constraint.
5. **Single-client.** All benchmarks use a single connection/client. MongoCore's connection pooling and multiplexing benefits don't appear in these results.

## Results

Results are stored as flat JSON files in `results/` (one per benchmark). If a result file exists, that benchmark is skipped on the next run. Delete a specific file to force a rerun of just that benchmark, or use `just bench-clean` to wipe everything.

## Tasks

All commands run from `benchmarks/` via `just`:

| Command | Description |
|---------|-------------|
| `just bench-all` | Run everything (builds sidecar, all languages, ingestion, collects results) |
| `just bench-drivers` | All driver benchmarks (native + MongoCore, all languages) |
| `just bench-drivers-native` | All native driver benchmarks only |
| `just bench-drivers-mongocore` | All MongoCore benchmarks only |
| **Python** | |
| `just bench-python` | Python native + MongoCore |
| `just bench-python-native` | Python pymongo only |
| `just bench-python-mongocore` | Python MongoCore gRPC client |
| **TypeScript** | |
| `just bench-typescript` | TypeScript native + MongoCore |
| `just bench-typescript-native` | TypeScript native MongoDB driver only |
| `just bench-typescript-mongocore` | TypeScript MongoCore gRPC client |
| **Go** | |
| `just bench-go` | Go native + MongoCore |
| `just bench-go-native` | Go mongo-go-driver only |
| `just bench-go-mongocore` | Go MongoCore gRPC client |
| **Java** | |
| `just bench-java` | Java native + MongoCore |
| `just bench-java-native` | Java MongoDB sync driver only |
| `just bench-java-mongocore` | Java MongoCore gRPC client |
| **Pipeline** | |
| `just bench-drivers-pipeline` | Pipeline batching benchmarks (all languages) |
| `just bench-python-pipeline` | Python pipeline batching |
| `just bench-typescript-pipeline` | TypeScript pipeline batching |
| `just bench-go-pipeline` | Go pipeline batching |
| `just bench-java-pipeline` | Java pipeline batching |
| **Other** | |
| `just bench-rust` | Sidecar internal criterion benchmarks (no MongoDB needed) |
| `just bench-ingestion` | Polars ingestion vs native bulk insert |
| `just bench-compiled` | Compiled query cache benchmarks (waits for sample data) |
| `just bench-report` | Generate RESULTS.md from current result files |
| `just bench-clean` | Delete all result files (forces full rerun) |
| `just bench-generate-data` | Generate test data for ingestion benchmarks |
| `just bench-setup` | Start benchmark infrastructure (Docker + sidecar) |
| `just bench-teardown` | Stop all benchmark infrastructure (Docker + sidecar) |

> **Note:** `bench-all` starts Docker (MongoDB) and the sidecar automatically, but does
> NOT tear them down — run `just bench-teardown` when you're done. This lets you quickly
> rerun failed benchmarks without waiting for Atlas sample data to reload. Individual
> benchmarks (e.g. `bench-python`) require `bench-setup` first. `bench-rust` needs no
> external services. Benchmarks with existing results are automatically skipped.

## Prerequisites

- Docker (for mongodb-atlas-local)
- Rust toolchain (for building MongoCore sidecar)
- Language toolchains: Python 3.11+, Node 18+, Go 1.21+, Java 17+
