# MongoCore: Performance Benchmarks

## Overview

A comprehensive benchmarking suite that transparently measures MongoCore's performance: sidecar overhead, client-to-database throughput vs native drivers, compiled query cache effectiveness, and Polars ingestion performance. Results are auto-generated into a README with comparison tables for full transparency.

## Motivation

MongoCore adds a gRPC hop between applications and MongoDB. Users need to know:
1. How much overhead does the sidecar add per operation?
2. Is the multi-doc throughput competitive with native drivers?
3. How fast is compiled query cache lookup vs cold LLM calls?
4. Does Polars ingestion outperform native bulk inserts?

Honest, reproducible benchmarks build trust and identify optimization opportunities.

## Architecture

```
benchmarks/
├── README.md                    # Auto-generated results with comparison tables
├── justfile                     # Benchmark recipes
├── results/                     # Raw JSON results (committed for transparency)
│   └── latest.json
├── rust/                        # Criterion benchmarks for sidecar internals
│   ├── Cargo.toml
│   └── benches/
│       ├── cache_lookup.rs
│       ├── template_matching.rs
│       ├── mql_validation.rs
│       └── bson_conversion.rs
├── drivers/                     # Per-language benchmarks (native vs MongoCore)
│   ├── python/
│   │   ├── bench_native.py
│   │   └── bench_mongocore.py
│   ├── typescript/
│   │   ├── bench_native.ts
│   │   └── bench_mongocore.ts
│   ├── go/
│   │   ├── bench_native_test.go
│   │   └── bench_mongocore_test.go
│   └── java/
│       ├── BenchNative.java
│       └── BenchMongocore.java
├── ingestion/                   # Polars ingestion benchmarks
│   ├── bench_ingest.rs
│   ├── data/
│   └── generate_data.py
└── collector/
    ├── collect.py              # Aggregates all JSON results
    └── generate_readme.py      # Generates README.md with tables
```

## Benchmark Categories

### Category 1: Sidecar Internals (Rust criterion, no network)

Measures pure MongoCore overhead — no MongoDB, no network.

| Benchmark | What it measures |
|-----------|-----------------|
| `cache_lookup_l1_hit` | In-memory cache lookup (DashMap) |
| `cache_lookup_l1_miss_l2_hit` | Disk cache lookup on L1 miss |
| `template_registry_match` | Regex template matching + JSON substitution |
| `template_registry_miss` | Full registry scan with no match |
| `mql_validate_filter` | Filter validation (recursive dangerous operator check) |
| `mql_validate_pipeline` | Pipeline validation (allowlist + recursive check) |
| `bson_encode_small` | Encode small document (~275 bytes) |
| `bson_encode_large` | Encode large document (~2.5MB) |
| `query_hash` | Intent + database + collection SHA-256 hashing |

### Category 2: Single-Document Operations (per language)

Following MongoDB benchmarking spec. Each language runs both native driver and MongoCore client.

| Benchmark | Description | Dataset |
|-----------|-------------|---------|
| `run_command` | `{hello: true}` round-trip | — |
| `find_one_by_id` | Indexed find by _id | tweet.json (1,622B) |
| `insert_one_small` | Insert single small doc | small_doc.json (275B) |
| `insert_one_large` | Insert single large doc | large_doc.json (2.75MB) |

### Category 3: Multi-Document Operations (per language)

| Benchmark | Description | Dataset |
|-----------|-------------|---------|
| `find_many` | Retrieve 10,000 docs via cursor | small_doc × 10,000 |
| `bulk_insert_small` | InsertMany 10,000 small docs | small_doc × 10,000 |
| `bulk_insert_large` | InsertMany 10 large docs | large_doc × 10 |

### Category 4: Compiled Query Cache (MongoCore only)

| Benchmark | What it measures | Expected |
|-----------|-----------------|----------|
| `compiled_cold` | First NL→MQL call (LLM round-trip) | ~500-2000ms |
| `compiled_cache_hit` | Same query, L1 cache hit | <0.1ms |
| `compiled_template_hit` | Different params, template registry hit | <0.5ms |
| `compiled_l2_hit` | L1 miss, L2 disk cache hit | ~1-5ms |

### Category 5: Ingestion (MongoCore Polars vs native bulk)

| Benchmark | Description | Sizes |
|-----------|-------------|-------|
| `ingest_csv` | CSV → MongoDB via Polars engine | 10K, 100K, 1M rows |
| `ingest_ndjson` | NDJSON → MongoDB via Polars engine | 10K, 100K rows |
| `ingest_parquet` | Parquet → MongoDB via Polars engine | 10K, 100K, 1M rows |
| `native_bulk_insert` | Direct insertMany (same data pre-loaded in memory) | 10K, 100K, 1M rows |

Ingestion benchmarks include the full pipeline: read → schema inference → transform → write.

## Methodology

Following the [MongoDB Driver Benchmarking Specification](https://github.com/mongodb/specifications/blob/master/source/benchmarking/benchmarking.md):

### Timing
- High-resolution monotonic clock (Rust `Instant`, Python `time.perf_counter`, Go `time.Now`, etc.)
- Only the "Do Task" phase is timed (not setup/teardown)
- Each iteration operates on a batch (e.g., 10,000 inserts), not individual operations

### Iterations
- Minimum: 1 minute cumulative execution time
- Maximum: 100 iterations OR 5 minutes cumulative (whichever first)
- JIT languages (Java): warmup iterations discarded

### Metrics
- **Primary:** MB/s = task_size_bytes / median_time_seconds / 1,000,000
- **Secondary:** ops/sec (for single-doc benchmarks)
- **Percentiles:** p10, p25, p50, p75, p90, p95, p99 (Nearest Rank method)
- **Composite:** p50 (median) used for comparison tables

### Write Concern
- All write operations use `w:1` (matching MongoDB spec)

### Environment
- Record: OS, architecture, CPU count, RAM, MongoDB version, MongoCore version
- All benchmarks run against `mongodb/mongodb-atlas-local` Docker container

## JSON Output Format

All language benchmarks output standardized JSON:

```json
{
  "benchmark": "find_one_by_id",
  "category": "single_doc",
  "driver": "mongocore+python",
  "dataset_size_bytes": 1622,
  "batch_size": 1,
  "iterations": 87,
  "total_time_secs": 61.2,
  "ops_per_sec": 14210.5,
  "mb_per_sec": 22.3,
  "percentiles": {
    "p10": 0.062, "p25": 0.065, "p50": 0.068,
    "p75": 0.072, "p90": 0.081, "p95": 0.089, "p99": 0.112
  },
  "timestamp": "2026-05-12T17:00:00Z",
  "system": {
    "os": "darwin",
    "arch": "arm64",
    "cpus": 10,
    "ram_gb": 32,
    "mongodb_version": "7.0.4",
    "mongocore_version": "0.6.0"
  }
}
```

Criterion (Rust) benchmarks output their native format — the collector converts.

## README.md Output (Auto-Generated)

The collector script generates `benchmarks/README.md` with:

### Section 1: Environment
```
## Benchmark Environment
- OS: macOS 15.4 (arm64)
- CPU: Apple M4 Pro (10 cores)
- RAM: 32 GB
- MongoDB: 7.0.4 (Atlas Local Docker — localhost:27017)
- MongoCore: v0.6.0
- Date: 2026-05-12

> **Note:** All benchmarks run against `mongodb/mongodb-atlas-local` on localhost.
> This isolates MongoCore sidecar overhead without network latency noise.
> Production Atlas results will differ due to network, hardware, and cluster topology.
> These numbers measure the cost of the gRPC hop and MongoCore processing, not MongoDB performance.
```

### Section 2: Sidecar Overhead
```
## Sidecar Overhead (Single Document)

| Operation      | pymongo   | MongoCore+Python | Overhead | Go driver | MongoCore+Go | Overhead |
|----------------|-----------|------------------|----------|-----------|--------------|----------|
| Run Command    | 12,500/s  | 11,200/s         | -10.4%   | 15,200/s  | 13,800/s     | -9.2%    |
| Find One       | 9,800/s   | 9,100/s          | -7.1%    | 12,100/s  | 11,400/s     | -5.8%    |
| InsertOne (sm) | 8,200/s   | 7,600/s          | -7.3%    | 10,400/s  | 9,700/s      | -6.7%    |
| InsertOne (lg) | 450/s     | 420/s            | -6.7%    | 580/s     | 540/s        | -6.9%    |
```

### Section 3: Multi-Document Throughput
```
## Multi-Document Throughput (MB/s)

| Operation         | pymongo | MongoCore+Python | Go driver | MongoCore+Go |
|-------------------|---------|------------------|-----------|--------------|
| Find Many (10K)   | 45.2    | 41.8             | 62.3      | 58.1         |
| Bulk Insert (10K) | 38.7    | 35.2             | 51.4      | 47.8         |
| Bulk Insert (lg)  | 112.4   | 105.6            | 145.8     | 138.2        |
```

### Section 4: Compiled Query Performance
```
## Compiled Query Performance

| Operation          | Latency (p50) | ops/sec |
|--------------------|---------------|---------|
| Cold (LLM call)    | 1,240ms       | 0.8     |
| L1 Cache Hit       | 0.02ms        | 45,000  |
| Template Match     | 0.08ms        | 12,500  |
| L2 Disk Hit        | 1.2ms         | 830     |
```

### Section 5: Ingestion
```
## Ingestion Performance (MB/s)

| Format  | 10K rows | 100K rows | 1M rows | Native Bulk (1M) |
|---------|----------|-----------|---------|------------------|
| CSV     | 28.5     | 42.1      | 48.3    | 38.7             |
| NDJSON  | 31.2     | 45.8      | —       | 38.7             |
| Parquet | 52.4     | 78.6      | 95.2    | 38.7             |
```

### Section 6: Raw Data
Collapsible `<details>` sections with full percentile data for each benchmark.

## Justfile Commands

```
# Run all benchmarks
bench-all: bench-rust bench-drivers bench-ingestion bench-collect

# Sidecar internal benchmarks (no MongoDB needed)
bench-rust:
    cd benchmarks/rust && cargo bench

# Driver comparison benchmarks (needs MongoDB + sidecar)
bench-drivers: bench-python bench-typescript bench-go bench-java

bench-python:
    cd benchmarks/drivers/python && python bench_native.py && python bench_mongocore.py

bench-typescript:
    cd benchmarks/drivers/typescript && npx ts-node bench_native.ts && npx ts-node bench_mongocore.ts

bench-go:
    cd benchmarks/drivers/go && go test -bench=. -benchtime=1m ./...

bench-java:
    cd benchmarks/drivers/java && mvn exec:java -Dexec.mainClass="com.mongocore.BenchNative" && mvn exec:java -Dexec.mainClass="com.mongocore.BenchMongocore"

# Ingestion benchmarks
bench-ingestion:
    cd benchmarks/ingestion && cargo bench

# Generate datasets for ingestion benchmarks
bench-generate-data:
    cd benchmarks/ingestion && python generate_data.py

# Collect results and generate README
bench-collect:
    cd benchmarks/collector && python collect.py && python generate_readme.py
```

## Datasets

### Standard (from MongoDB spec)
- `tweet.json` (1,622 bytes) — single-doc find benchmark
- `small_doc.json` (275 bytes) — bulk operations
- `large_doc.json` (2,731,089 bytes) — large doc operations

### Ingestion (generated)
- `bench_10k.csv` / `.ndjson` / `.parquet` — 10,000 rows
- `bench_100k.csv` / `.ndjson` / `.parquet` — 100,000 rows
- `bench_1m.csv` / `.parquet` — 1,000,000 rows

Generated with `just bench-generate-data`. Schema: id (int), name (string), email (string), age (int), score (float), created_at (datetime), tags (array of 3 strings).

## Implementation Scope

| Component | Language | Purpose |
|-----------|----------|---------|
| `benchmarks/rust/` | Rust | Criterion benchmarks for sidecar internals |
| `benchmarks/drivers/python/` | Python | pymongo + MongoCore Python benchmarks |
| `benchmarks/drivers/typescript/` | TypeScript | mongodb driver + MongoCore TS benchmarks |
| `benchmarks/drivers/go/` | Go | go-driver + MongoCore Go benchmarks |
| `benchmarks/drivers/java/` | Java | java-driver + MongoCore Java benchmarks |
| `benchmarks/ingestion/` | Rust | Polars ingestion benchmarks |
| `benchmarks/collector/` | Python | Result aggregation + README generation |
| `benchmarks/justfile` | Just | Orchestration recipes |

## Network Latency Isolation

Benchmarks run in two modes to separate network overhead from sidecar overhead:

**Loopback mode (default):** MongoDB, sidecar, and clients all on `localhost`. Measures pure sidecar processing overhead without network noise.

**Remote mode (optional):** Sidecar connects to a remote MongoDB (Atlas or separate host). Measures real-world latency including network. Configure via `BENCH_MONGODB_URI` env var.

The README reports loopback results by default (most meaningful for sidecar overhead measurement). Remote results are separate if run.

## Graph & Chart Format

The auto-generated README uses:
- **Markdown tables** for quick scanning (always present)
- **SVG bar charts** embedded in README for visual comparison (generated by `generate_readme.py` using matplotlib/plotly)
- **HTML report** at `benchmarks/results/report.html` with interactive charts (optional, not committed)

SVG files committed to `benchmarks/results/charts/`:
- `sidecar_overhead.svg` — bar chart comparing native vs MongoCore per operation
- `ingestion_throughput.svg` — line chart showing MB/s at different dataset sizes
- `compiled_query_latency.svg` — log-scale chart showing cold/cached/template/disk

## Historical Tracking & Regression Detection

Results are committed with timestamps. The collector supports:

**Historical comparison:**
```bash
just bench-compare  # Compare latest.json against previous.json
```

Produces a diff table showing performance changes:
```
| Benchmark         | Previous | Current | Change  |
|-------------------|----------|---------|---------|
| find_one (python) | 9,100/s  | 8,800/s | -3.3% ⚠ |
| bulk_insert       | 35.2 MB/s| 36.1 MB/s| +2.6% ✓ |
```

**Regression threshold:** Flag benchmarks that regress more than 5% from previous run with ⚠ warning.

**Result history:** `benchmarks/results/` keeps the last 10 runs:
```
results/
├── latest.json          # Symlink to most recent
├── 2026-05-12T17-00.json
├── 2026-05-11T14-30.json
└── ...
```

## CI Integration Hooks

While benchmarks don't run in CI (too slow, need dedicated hardware), provide hooks for optional CI integration:

```yaml
# Example GitHub Actions workflow (not auto-created, user sets up if needed)
bench-check:
  runs-on: self-hosted  # Dedicated benchmark runner
  steps:
    - just bench-all
    - just bench-compare
    # Fail if any benchmark regresses >10%
    - python collector/check_regression.py --threshold 10
```

The `check_regression.py` script:
- Compares `latest.json` against a baseline
- Exits non-zero if any benchmark regresses beyond threshold
- Outputs summary to stdout

## Warmup Strategy Per Language

| Language | Warmup | Rationale |
|----------|--------|-----------|
| Rust (criterion) | Built-in (criterion handles it) | Criterion automatically detects warm-up |
| Python | 3 iterations discarded | CPython has no JIT but connection pool needs warming |
| TypeScript (Node) | 5 iterations discarded | V8 JIT needs optimization passes |
| Go | 3 iterations discarded | GC and runtime scheduling settle after first few |
| Java | 10 iterations discarded | JVM HotSpot JIT requires significant warmup |

Each language benchmark script implements warmup before starting the timed iterations. Warmup iterations are NOT included in results.

## LLM Benchmark Conditioning

The `compiled_cold` benchmark requires a real LLM call. This is handled the same way as LLM integration tests:

- Reads `config.test.toml` for LLM configuration
- Skips `compiled_cold` if no LLM is configured (prints warning)
- `compiled_cache_hit`, `compiled_template_hit`, and `compiled_l2_hit` do NOT need LLM — they pre-seed the cache in setup
- Only `compiled_cold` is conditional; all other compiled query benchmarks always run

The README shows `compiled_cold` results only when available, with a note: "Measured against [provider] via [gateway/direct]".

## Won't Build

- No continuous benchmarking in CI (manual runs only, results committed)
- No flame graphs or profiling (separate concern)
- No GridFS benchmarks (not implemented in MongoCore)
- No parallel/ETL benchmarks from spec (future work)
- No latency histograms (percentile tables are sufficient)

## Success Criteria

- [ ] `just bench-rust` runs criterion benchmarks for sidecar internals
- [ ] `just bench-drivers` runs all 4 language benchmarks (native + MongoCore)
- [ ] `just bench-ingestion` benchmarks CSV/JSON/Parquet ingestion at multiple sizes
- [ ] `just bench-collect` generates `benchmarks/README.md` with comparison tables
- [ ] Results committed as JSON in `benchmarks/results/`
- [ ] README shows honest overhead numbers (MongoCore will be slower for single ops)
- [ ] README shows where MongoCore wins (compiled query cache, Polars ingestion)
- [ ] All benchmarks follow MongoDB spec methodology (iterations, timing, percentiles)
- [ ] Datasets downloadable or generatable via `just bench-generate-data`
