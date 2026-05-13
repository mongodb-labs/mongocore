# Benchmark Fixes — Plan for Next Session

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.

## Context

The benchmark suite was built and verified for Python (native + MongoCore). When running `just bench-all`, failures occurred — likely related to:
1. Extended JSON loading (`{"$oid": "..."}` format in dataset files)
2. MongoCore client differences (async patterns, missing methods)
3. Go/TypeScript/Java scripts not yet validated against running sidecar

## Issues to Investigate

### 1. Extended JSON in Datasets
- `tweet.json` uses `{"$oid": "..."}` which pymongo doesn't accept for `_id` on insert
- Already fixed in `bench_native.py` (strips `_id` before insert)
- Check if Go/TS/Java benchmarks have the same issue
- Consider: store datasets as plain JSON (no extended JSON) or handle conversion in each language

### 2. gRPC Message Size Limits
- Confirmed: 4MB default limit prevents bulk_insert_large (27.5MB) and find_many at 10K docs
- MongoCore benchmarks skip these with clear messages
- Future fix: increase `max_receive_message_length` in MongoCore gRPC server config

### 3. Go/TypeScript/Java Benchmark Validation
- Scripts were generated but not run against the sidecar
- Need to verify: imports resolve, connections work, JSON output matches format
- May need `go mod tidy`, `npm install`, `mvn compile` first

## Changes to Make

### A. Results stored per-run in date-based folders

Change from:
```
benchmarks/results/
├── latest.json
├── python_native.json
├── python_mongocore.json
```

To:
```
benchmarks/results/
├── latest -> 2026-05-13T10-30/    (symlink to latest run)
├── 2026-05-12T20-35/
│   ├── README.md                   (auto-generated for this run)
│   ├── python_native.json
│   ├── python_mongocore.json
│   ├── ingestion.json
│   └── charts/
│       ├── sidecar_overhead.svg
│       └── ingestion_performance.svg
├── 2026-05-13T10-30/
│   ├── README.md
│   ├── ...
```

### B. Main benchmarks/README.md

Static file (not auto-generated) that:
- Explains how to run benchmarks
- Links to `results/latest/README.md` for current results
- Documents prerequisites, methodology, known limitations

### C. Allow results in git

Remove from `.gitignore`:
```
results/*.json
results/charts/*.svg
README.md
```

Results are committed per-run so history is transparent and reviewable.

### D. Update collector

- `collect.py`: create timestamped folder, save all results there, update `latest` symlink
- `generate_readme.py`: generate README.md inside the run folder (not at benchmarks/ root)
- Main `benchmarks/README.md`: static, manually maintained, links to latest

## File Changes

| File | Change |
|------|--------|
| `benchmarks/.gitignore` | Remove results exclusions, keep ingestion/data/ and build artifacts |
| `benchmarks/README.md` | Static: how to run, methodology, links to latest results |
| `benchmarks/collector/collect.py` | Create date-based folder, symlink latest |
| `benchmarks/collector/generate_readme.py` | Output to run folder, not root |
| `benchmarks/drivers/python/bench_native.py` | Verify extended JSON handling |
| `benchmarks/drivers/python/bench_mongocore.py` | Verify extended JSON handling |
| `benchmarks/drivers/go/*.go` | Validate and fix if needed |
| `benchmarks/drivers/typescript/*.ts` | Validate and fix if needed |
| `benchmarks/drivers/java/**/*.java` | Validate and fix if needed |

## Verification

After fixes:
1. `just bench-all` completes without errors
2. Results stored in `benchmarks/results/YYYY-MM-DDTHH-MM/`
3. `latest` symlink points to most recent run
4. `benchmarks/README.md` is static with run instructions
5. Each run folder has its own README.md with tables and charts
6. `git status` shows new results ready to commit
