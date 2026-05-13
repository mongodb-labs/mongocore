# MongoCore Benchmarks

Cross-language performance benchmarks comparing native MongoDB drivers against MongoCore sidecar.

## Quick Start

```bash
just bench-all        # Run all benchmarks
just bench-collect    # Collect results into timestamped folder + generate README
```

## What's Measured

- **Single-doc ops:** run_command, find_one_by_id, insert_one_small, insert_one_large
- **Multi-doc ops:** bulk_insert_small, bulk_insert_large, find_many, find_many_large
- **Ingestion:** Polars pipeline vs native bulk insert (1MB, 10MB, 100MB)

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

Results are stored in timestamped folders under `results/`. A `latest` symlink points to the most recent run.

**Latest results:** [results/latest/README.md](results/latest/README.md)

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
| **Other** | |
| `just bench-rust` | Sidecar internal criterion benchmarks (no MongoDB needed) |
| `just bench-ingestion` | Polars ingestion vs native bulk insert |
| `just bench-compiled` | Compiled query cache benchmarks (waits for sample data) |
| `just bench-collect` | Collect results into timestamped folder + generate README |
| `just bench-compare` | Compare latest results against previous run |
| `just bench-check-regression` | Check for regressions (exits non-zero if found) |
| `just bench-generate-data` | Generate test data for ingestion benchmarks |
| `just bench-setup` | Start benchmark infrastructure (Docker + sidecar) |
| `just bench-teardown` | Stop all benchmark infrastructure (Docker + sidecar) |

> **Note:** All benchmark commands are self-contained — they start Docker (MongoDB),
> build/start the sidecar as needed, and clean up (stop sidecar + Docker) on exit.
> The only exception is `bench-rust` which needs no external services.

## Prerequisites

- Docker (for mongodb-atlas-local)
- Rust toolchain (for building MongoCore sidecar)
- Language toolchains: Python 3.11+, Node 18+, Go 1.21+, Java 17+
