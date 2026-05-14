# Pipeline Benchmarks Design

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.
> If modifying client libraries: verify imports work and run `just test-clients`.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

## Goal

Add pipeline-batched benchmark variants to the benchmark suite that demonstrate the throughput benefit of MongoCore's `Pipeline` RPC (batching multiple operations into a single gRPC call) compared to individual RPCs and native drivers. Show the impact of different batch sizes (100, 1000, 10000) to reveal the optimal batching sweet spot.

Additionally: consolidate the `benches/pipeline.rs` Criterion benchmark into the main `benchmarks/` folder, and add a "Compiled Query" section to the results template (currently collected but not rendered).

## Changes

### 1. Move `benches/pipeline.rs` → `benchmarks/rust/benches/pipeline.rs`

- Move the file from `benches/pipeline.rs` to `benchmarks/rust/benches/pipeline.rs`
- Add a `[[bench]]` entry to `benchmarks/rust/Cargo.toml`
- Remove the now-empty `benches/` directory
- Update root `Cargo.toml` if it references the `benches/` directory

### 2. New benchmark files

Create `bench_pipeline.*` in each language directory:

- `benchmarks/drivers/python/bench_pipeline.py`
- `benchmarks/drivers/typescript/bench_pipeline.ts`
- `benchmarks/drivers/go/bench_pipeline.go`
- `benchmarks/drivers/java/src/main/java/com/mongocore/bench/BenchPipeline.java`

#### Operations benchmarked

| Operation | Description | Equivalent existing benchmark |
|-----------|-------------|-------------------------------|
| `pipeline_run_command` | Batch N `hello` commands into one pipeline | `run_command` (10K individual calls) |
| `pipeline_insert_one_small` | Batch N small doc inserts into one pipeline | `insert_one_small` (10K individual calls) |
| `pipeline_find_one_by_id` | Batch N find_one lookups into one pipeline | `find_one_by_id` (10K individual calls) |

#### Batch sizes

Each operation is tested at 3 batch sizes: **100**, **1000**, **10000** operations per pipeline call.

Total work per benchmark iteration is always 10K operations:
- Batch size 100 → 100 pipeline calls per iteration
- Batch size 1000 → 10 pipeline calls per iteration
- Batch size 10000 → 1 pipeline call per iteration

#### Benchmark naming

Format: `pipeline_{operation}_{batch_size}`

Examples: `pipeline_run_command_100`, `pipeline_run_command_1000`, `pipeline_run_command_10000`

#### Output

Each language outputs results to `benchmarks/results/{language}_pipeline.json` using the same JSON schema as existing benchmarks. The `category` field is `"pipeline"`.

### 3. Justfile additions

#### Private recipes

```
_bench-python-pipeline
_bench-typescript-pipeline
_bench-go-pipeline
_bench-java-pipeline
```

#### Public recipes

```
bench-python-pipeline
bench-typescript-pipeline
bench-go-pipeline
bench-java-pipeline
bench-drivers-pipeline    # all 4 languages
```

#### Updates to existing recipes

- `bench-drivers` — add pipeline variants after existing mongocore benchmarks
- `bench-all` — include pipeline benchmarks in the full run

### 4. Results template — Pipeline Batching section

Add a new section to `results.md.j2` after the per-language throughput/latency tables:

```markdown
## Pipeline Batching

Shows throughput gain from batching operations into single Pipeline RPC calls.

### {Language}

| Operation | Native (ops/s) | MC Individual (ops/s) | Pipeline×100 | Pipeline×1K | Pipeline×10K | Best Speedup vs Native |
|-----------|---------------|----------------------|--------------|-------------|--------------|----------------------|
| run_command | ... | ... | ... | ... | ... | ... |
| insert_one_small | ... | ... | ... | ... | ... | ... |
| find_one_by_id | ... | ... | ... | ... | ... | ... |
```

"Best Speedup vs Native" = max(pipeline variants) / native ops/s, formatted as `1.4x`.

### 5. Results template — Compiled Query section

Add after Pipeline Batching:

```markdown
## Compiled Query Cache

| Benchmark | ops/s | p50 | p99 |
|-----------|-------|-----|-----|
| compiled_cache_hit | ... | ... | ... |
```

### 6. Pipeline chart

Generate a line chart saved to `charts/pipeline_scaling.svg`:

- X-axis: batch size (100, 1K, 10K) — log scale
- Y-axis: ops/s
- One line per operation (3 lines), with marker points at each batch size
- Horizontal dashed reference lines for:
  - Native individual ops/s (labeled "Native baseline")
  - MC individual ops/s (labeled "MC individual")
- Legend showing operation names and reference lines
- One chart per language that has pipeline data

### 7. Collector updates

**`generate_readme.py`:**

- Add `build_pipeline_context(results)` function that:
  - Groups pipeline results by language and operation
  - Looks up corresponding native and MC-individual results for reference
  - Returns data structured for the template
- Add `build_compiled_context(results)` function for compiled query results
- Add `generate_pipeline_chart()` function
- Update `build_context()` to call both new functions
- Add pipeline benchmarks to chart constants if needed

**`collect.py`:** No changes needed — already picks up all `*.json` in `results/`.

## Implementation notes

- Pipeline benchmarks use the same `run_benchmark()` harness as existing benchmarks for consistency
- Each pipeline benchmark file follows the same pattern: connect, warmup, timed iterations, save JSON
- For `pipeline_find_one_by_id`, setup inserts a single doc first (same as existing `find_one_by_id`)
- For `pipeline_insert_one_small`, `before_task` drops the collection (same as existing)
- The Python client uses `client.pipeline([ops.insert(...), ...])` — the existing `ops` module
- The TypeScript client uses `client.pipeline([ops.insert(...), ...])` — the existing `ops` module
- Go and Java clients use their respective pipeline methods

## Files changed

| File | Action |
|------|--------|
| `benches/pipeline.rs` | Delete |
| `benchmarks/rust/benches/pipeline.rs` | Create (moved content) |
| `benchmarks/rust/Cargo.toml` | Update (add bench entry) |
| `benchmarks/drivers/python/bench_pipeline.py` | Create |
| `benchmarks/drivers/typescript/bench_pipeline.ts` | Create |
| `benchmarks/drivers/go/bench_pipeline.go` | Create |
| `benchmarks/drivers/java/src/main/java/com/mongocore/bench/BenchPipeline.java` | Create |
| `benchmarks/justfile` | Update (add pipeline recipes) |
| `benchmarks/collector/templates/results.md.j2` | Update (add pipeline + compiled sections) |
| `benchmarks/collector/generate_readme.py` | Update (pipeline + compiled context + chart) |
| Root `Cargo.toml` | Update if it references `benches/` |
| `benchmarks/README.md` | Update to mention pipeline benchmarks |

## Implementation footnotes

1. **Root `Cargo.toml` cleanup:** Lines 60-64 have a `criterion` dev-dependency and `[[bench]] name = "pipeline"` entry that must be removed when deleting `benches/`.
2. **`benchmarks/rust/Cargo.toml` new deps:** The moved `pipeline.rs` uses `tonic` and `mongocore::grpc` — add `tonic` as a dependency. The `#[path = "../tests/harness/mod.rs"]` path reference needs updating to the new relative location from `benchmarks/rust/benches/`.
3. **`benchmarks/README.md`:** Update to describe the pipeline benchmark suite alongside existing sections.
4. **`bench-all` ordering:** Pipeline benchmarks should run after existing mongocore benchmarks (sidecar must be running). Insert after the `bench-drivers-mongocore` calls.
5. **No new Python/TS/Go/Java deps:** Pipeline benchmarks reuse existing client libraries and their `requirements.txt`/`package.json`/`go.mod`/`pom.xml` — no new dependencies needed.
