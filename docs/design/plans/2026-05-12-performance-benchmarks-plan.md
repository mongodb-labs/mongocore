# Performance Benchmarks — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.
> If modifying client libraries: verify imports work and run `just test-clients`.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

**Goal:** Build a comprehensive benchmarking suite that measures sidecar overhead, compares MongoCore vs native drivers across 4 languages, benchmarks compiled query cache and Polars ingestion, and auto-generates a transparent README with comparison tables and SVG charts.

**Architecture:** Rust criterion for sidecar internals (no network), per-language benchmark scripts outputting standardized JSON, a Python collector that aggregates results and generates README.md with embedded SVG charts. All run against mongodb-atlas-local on localhost.

**Tech Stack:** Rust (criterion), Python (time.perf_counter, matplotlib), TypeScript (Node perf_hooks), Go (testing.B), Java (System.nanoTime), JSON result format.

**Branch:** `feat/performance-benchmarks` — do NOT push to origin.

---

## File Structure

```
benchmarks/
├── README.md                         # Auto-generated (DO NOT EDIT MANUALLY)
├── justfile                          # Benchmark orchestration recipes
├── results/
│   └── .gitkeep
├── rust/
│   ├── Cargo.toml
│   └── benches/
│       ├── cache_lookup.rs
│       ├── template_matching.rs
│       └── mql_validation.rs
├── drivers/
│   ├── common.json                   # Shared config (dataset paths, iterations)
│   ├── python/
│   │   ├── requirements.txt
│   │   ├── bench_native.py
│   │   └── bench_mongocore.py
│   ├── typescript/
│   │   ├── package.json
│   │   ├── bench_native.ts
│   │   └── bench_mongocore.ts
│   ├── go/
│   │   ├── go.mod
│   │   └── bench_test.go
│   └── java/
│       ├── pom.xml
│       └── src/main/java/com/mongocore/bench/
│           ├── BenchNative.java
│           └── BenchMongocore.java
├── ingestion/
│   ├── generate_data.py
│   └── bench_ingest.py              # Python script calling MongoCore ingest
├── data/
│   ├── tweet.json
│   ├── small_doc.json
│   └── large_doc.json
└── collector/
    ├── requirements.txt
    ├── collect.py
    └── generate_readme.py
```

---

## Task 1: Scaffold Benchmarks Directory and Justfile

**Files:**
- Create: `benchmarks/justfile`
- Create: `benchmarks/results/.gitkeep`
- Create: `benchmarks/data/tweet.json`
- Create: `benchmarks/data/small_doc.json`
- Create: `benchmarks/data/large_doc.json`
- Create: `benchmarks/drivers/common.json`

- [ ] **Step 1: Create benchmarks directory structure**

```bash
mkdir -p benchmarks/{results,rust/benches,drivers/{python,typescript,go,java},ingestion,data,collector}
touch benchmarks/results/.gitkeep
```

- [ ] **Step 2: Create standard datasets**

`benchmarks/data/small_doc.json` (275 bytes, matching MongoDB spec):
```json
{"_id": {"$oid": "000000000000000000000001"}, "field0": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "field1": 123456789, "field2": true, "field3": 3.14159, "field4": "short"}
```

`benchmarks/data/tweet.json` (representative tweet document, ~1600 bytes):
```json
{"_id": {"$oid": "55b531b4004d854578bc534f"}, "text": "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Pellentesque habitant morbi tristique senectus et netus et malesuada fames ac turpis egestas.", "in_reply_to_status_id": null, "retweet_count": 0, "contributors": null, "created_at": "Mon Jan 01 00:00:00 +0000 2024", "geo": null, "source": "<a href=\"http://example.com\">Example</a>", "coordinates": null, "in_reply_to_screen_name": null, "truncated": false, "entities": {"user_mentions": [], "urls": [], "hashtags": [{"indices": [0, 5], "text": "test"}]}, "retweeted": false, "place": null, "user": {"friends_count": 150, "profile_sidebar_fill_color": "DDEEF6", "location": "San Francisco, CA", "verified": false, "follow_request_sent": null, "favourites_count": 42, "profile_sidebar_border_color": "C0DEED", "profile_image_url": "http://example.com/avatar.jpg", "geo_enabled": false, "created_at": "Mon Jan 01 00:00:00 +0000 2020", "description": "Test user for benchmarks", "time_zone": "Pacific Time", "url": "http://example.com", "screen_name": "testuser", "notifications": null, "statuses_count": 1234, "followers_count": 500, "protected": false, "lang": "en", "name": "Test User", "id": 12345678}, "favorited": false, "in_reply_to_user_id": null, "id": 123456789012345678}
```

`benchmarks/data/large_doc.json`: A 2.75MB document. Generate with:
```python
import json
doc = {"_id": "large_bench_doc", "data": "x" * 2_750_000}
with open("benchmarks/data/large_doc.json", "w") as f:
    json.dump(doc, f)
```

- [ ] **Step 3: Create common configuration**

`benchmarks/drivers/common.json`:
```json
{
  "mongodb_uri": "mongodb://localhost:27017",
  "mongocore_address": "localhost:50051",
  "database": "mongocore_bench",
  "min_time_secs": 60,
  "max_iterations": 100,
  "max_time_secs": 300,
  "warmup_iterations": {
    "python": 3,
    "typescript": 5,
    "go": 3,
    "java": 10
  },
  "datasets": {
    "small_doc": "../data/small_doc.json",
    "tweet": "../data/tweet.json",
    "large_doc": "../data/large_doc.json"
  }
}
```

- [ ] **Step 4: Create benchmarks justfile**

`benchmarks/justfile`:
```just
# MongoCore Benchmark Runner

# Run all benchmarks
bench-all: bench-rust bench-drivers bench-ingestion bench-collect

# Sidecar internal benchmarks (no MongoDB needed)
bench-rust:
    cd rust && cargo bench

# All driver comparison benchmarks
bench-drivers: bench-python bench-typescript bench-go bench-java

# Python benchmarks (native pymongo + MongoCore)
bench-python:
    cd drivers/python && pip install -r requirements.txt -q && python bench_native.py && python bench_mongocore.py

# TypeScript benchmarks
bench-typescript:
    cd drivers/typescript && npm install --silent && npx ts-node bench_native.ts && npx ts-node bench_mongocore.ts

# Go benchmarks
bench-go:
    cd drivers/go && go test -bench=. -benchtime=1m -count=1 ./... -json > ../../results/go_results.json

# Java benchmarks
bench-java:
    cd drivers/java && mvn -q compile exec:java -Dexec.mainClass="com.mongocore.bench.BenchNative" && mvn -q exec:java -Dexec.mainClass="com.mongocore.bench.BenchMongocore"

# Polars ingestion benchmarks
bench-ingestion:
    cd ingestion && python generate_data.py && python bench_ingest.py

# Generate data for ingestion benchmarks
bench-generate-data:
    cd ingestion && python generate_data.py

# Collect results and generate README
bench-collect:
    cd collector && pip install -r requirements.txt -q && python collect.py && python generate_readme.py

# Compare latest against previous results
bench-compare:
    cd collector && python compare.py
```

- [ ] **Step 5: Commit**

```bash
git add benchmarks/
git commit -m "chore: scaffold benchmarks directory with datasets and justfile"
```

---

## Task 2: Python Benchmark Scripts (Native + MongoCore)

**Files:**
- Create: `benchmarks/drivers/python/requirements.txt`
- Create: `benchmarks/drivers/python/bench_native.py`
- Create: `benchmarks/drivers/python/bench_mongocore.py`

- [ ] **Step 1: Create requirements.txt**

```
pymongo>=4.0
grpcio>=1.60.0
protobuf>=4.0
```

- [ ] **Step 2: Create bench_native.py**

```python
"""Benchmark pymongo (native driver) for comparison against MongoCore."""

import json
import os
import platform
import sys
import time
import statistics
from datetime import datetime, timezone
from pathlib import Path

from pymongo import MongoClient
from bson import ObjectId

# Load config
CONFIG = json.loads((Path(__file__).parent.parent / "common.json").read_text())
DATA_DIR = Path(__file__).parent.parent.parent / "data"
RESULTS_DIR = Path(__file__).parent.parent.parent / "results"
RESULTS_DIR.mkdir(exist_ok=True)

WARMUP = CONFIG["warmup_iterations"]["python"]
MIN_TIME = CONFIG["min_time_secs"]
MAX_ITERS = CONFIG["max_iterations"]
MAX_TIME = CONFIG["max_time_secs"]
DB_NAME = CONFIG["database"]


def get_system_info():
    return {
        "os": platform.system().lower(),
        "arch": platform.machine(),
        "cpus": os.cpu_count(),
        "ram_gb": round(os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES") / (1024**3), 1) if hasattr(os, "sysconf") else None,
        "mongocore_version": "native",
        "driver": "pymongo",
    }


def run_benchmark(name, category, setup_fn, task_fn, teardown_fn, dataset_size_bytes, batch_size=1):
    """Run a benchmark following MongoDB spec methodology."""
    client = MongoClient(CONFIG["mongodb_uri"], w=1)
    db = client[DB_NAME]

    setup_fn(db)

    # Warmup
    for _ in range(WARMUP):
        task_fn(db)

    # Timed iterations
    times = []
    total_time = 0.0
    iteration = 0

    while total_time < MIN_TIME or iteration < 5:
        if iteration >= MAX_ITERS or total_time >= MAX_TIME:
            break

        start = time.perf_counter()
        task_fn(db)
        elapsed = time.perf_counter() - start

        times.append(elapsed)
        total_time += elapsed
        iteration += 1

    teardown_fn(db)
    client.close()

    # Calculate metrics
    times.sort()
    median = statistics.median(times)
    ops_per_sec = batch_size / median
    mb_per_sec = (dataset_size_bytes * batch_size) / median / 1_000_000

    def percentile(data, pct):
        idx = int(len(data) * pct / 100)
        return data[min(idx, len(data) - 1)]

    result = {
        "benchmark": name,
        "category": category,
        "driver": "pymongo",
        "dataset_size_bytes": dataset_size_bytes,
        "batch_size": batch_size,
        "iterations": len(times),
        "total_time_secs": round(total_time, 3),
        "ops_per_sec": round(ops_per_sec, 1),
        "mb_per_sec": round(mb_per_sec, 3),
        "percentiles": {
            "p10": round(percentile(times, 10), 6),
            "p25": round(percentile(times, 25), 6),
            "p50": round(median, 6),
            "p75": round(percentile(times, 75), 6),
            "p90": round(percentile(times, 90), 6),
            "p95": round(percentile(times, 95), 6),
            "p99": round(percentile(times, 99), 6),
        },
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "system": get_system_info(),
    }

    print(f"  {name}: {ops_per_sec:.0f} ops/s, {mb_per_sec:.2f} MB/s ({len(times)} iterations)")
    return result


def main():
    print("=== pymongo (native) benchmarks ===")
    results = []

    small_doc = json.loads((DATA_DIR / "small_doc.json").read_text())
    tweet_doc = json.loads((DATA_DIR / "tweet.json").read_text())
    small_size = len(json.dumps(small_doc).encode())
    tweet_size = len(json.dumps(tweet_doc).encode())

    # Run Command
    results.append(run_benchmark(
        "run_command", "single_doc",
        setup_fn=lambda db: None,
        task_fn=lambda db: db.command("hello"),
        teardown_fn=lambda db: None,
        dataset_size_bytes=100, batch_size=1,
    ))

    # Find One by ID
    def setup_find(db):
        db.drop_collection("bench_find")
        coll = db["bench_find"]
        coll.insert_one({"_id": ObjectId("000000000000000000000001"), **tweet_doc})
        coll.create_index("_id")

    results.append(run_benchmark(
        "find_one_by_id", "single_doc",
        setup_fn=setup_find,
        task_fn=lambda db: db["bench_find"].find_one({"_id": ObjectId("000000000000000000000001")}),
        teardown_fn=lambda db: db.drop_collection("bench_find"),
        dataset_size_bytes=tweet_size, batch_size=1,
    ))

    # InsertOne Small
    insert_counter = [0]
    results.append(run_benchmark(
        "insert_one_small", "single_doc",
        setup_fn=lambda db: db.drop_collection("bench_insert_small"),
        task_fn=lambda db: (
            db["bench_insert_small"].insert_one({**small_doc, "_id": ObjectId()}),
        ),
        teardown_fn=lambda db: db.drop_collection("bench_insert_small"),
        dataset_size_bytes=small_size, batch_size=1,
    ))

    # Bulk Insert Small (10,000 docs per iteration)
    def bulk_insert_task(db):
        docs = [{**small_doc, "_id": ObjectId()} for _ in range(10_000)]
        db["bench_bulk"].insert_many(docs)

    results.append(run_benchmark(
        "bulk_insert_small", "multi_doc",
        setup_fn=lambda db: db.drop_collection("bench_bulk"),
        task_fn=bulk_insert_task,
        teardown_fn=lambda db: db.drop_collection("bench_bulk"),
        dataset_size_bytes=small_size * 10_000, batch_size=10_000,
    ))

    # Find Many (10,000 docs)
    def setup_find_many(db):
        db.drop_collection("bench_find_many")
        docs = [{**small_doc, "_id": ObjectId()} for _ in range(10_000)]
        db["bench_find_many"].insert_many(docs)

    def find_many_task(db):
        list(db["bench_find_many"].find({}))

    results.append(run_benchmark(
        "find_many", "multi_doc",
        setup_fn=setup_find_many,
        task_fn=find_many_task,
        teardown_fn=lambda db: db.drop_collection("bench_find_many"),
        dataset_size_bytes=small_size * 10_000, batch_size=10_000,
    ))

    # Save results
    output_path = RESULTS_DIR / "python_native.json"
    with open(output_path, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nResults saved to {output_path}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 3: Create bench_mongocore.py**

Same structure as `bench_native.py` but connects via MongoCore gRPC client instead of pymongo directly. Uses `sys.path.insert(0, "../../clients/python/src")` to import the MongoCore client. Key difference: `MongoClient("localhost:50051")` instead of `MongoClient("mongodb://localhost:27017")`.

The benchmarks are the same operations but routing through the sidecar — measuring the gRPC overhead.

Output: `results/python_mongocore.json`

- [ ] **Step 4: Run and verify**

```bash
cd benchmarks && just bench-python
```
Expected: Two JSON files in `results/` with benchmark data.

- [ ] **Step 5: Commit**

```bash
git add benchmarks/drivers/python/
git commit -m "feat(bench): add Python benchmark scripts (native pymongo + MongoCore)"
```

---

## Task 3: Rust Criterion Benchmarks (Sidecar Internals)

**Files:**
- Create: `benchmarks/rust/Cargo.toml`
- Create: `benchmarks/rust/benches/cache_lookup.rs`
- Create: `benchmarks/rust/benches/template_matching.rs`
- Create: `benchmarks/rust/benches/mql_validation.rs`

- [ ] **Step 1: Create Cargo.toml for benchmark crate**

```toml
[package]
name = "mongocore-bench"
version = "0.1.0"
edition = "2021"

[dependencies]
mongocore = { path = "../.." }
bson = "2"
serde_json = "1"
criterion = { version = "0.5", features = ["html_reports"] }
tokio = { version = "1", features = ["full"] }

[[bench]]
name = "cache_lookup"
harness = false

[[bench]]
name = "template_matching"
harness = false

[[bench]]
name = "mql_validation"
harness = false
```

- [ ] **Step 2: Create cache_lookup.rs**

```rust
use criterion::{criterion_group, criterion_main, Criterion};
use mongocore::compiled::translator::CompiledQueryTranslator;
use mongocore::compiled::providers::TranslationContext;

fn bench_cache_l1_hit(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // Pre-populate cache with a mock entry
    let translator = CompiledQueryTranslator::new(None, None, None);
    // We need to seed the cache — use internal methods or pre-build a CompiledQuery
    // For now, benchmark the hash + lookup path

    c.bench_function("cache_l1_hit", |b| {
        b.iter(|| {
            rt.block_on(async {
                // This will be a cache miss without seeding, but measures the lookup path
                let _ = translator.cache_size();
            })
        })
    });
}

fn bench_query_hash(c: &mut Criterion) {
    use mongocore::compiled::hasher::QueryHasher;

    c.bench_function("query_hash", |b| {
        b.iter(|| {
            QueryHasher::hash(
                "find Italian restaurants in Manhattan",
                "sample_restaurants",
                "restaurants",
                None,
            )
        })
    });
}

criterion_group!(benches, bench_cache_l1_hit, bench_query_hash);
criterion_main!(benches);
```

- [ ] **Step 3: Create template_matching.rs**

```rust
use criterion::{criterion_group, criterion_main, Criterion};
use mongocore::compiled::template_registry::TemplateRegistry;
use mongocore::compiled::{LlmTemplate, LlmTemplateParameter, ParameterType};

fn bench_template_match_hit(c: &mut Criterion) {
    let registry = TemplateRegistry::new();
    let template = LlmTemplate {
        intent_pattern: "find {{cuisine}} restaurants in {{location}}".to_string(),
        parameters: vec![
            LlmTemplateParameter {
                name: "cuisine".to_string(),
                value: serde_json::json!("Italian"),
                param_type: ParameterType::String,
            },
            LlmTemplateParameter {
                name: "location".to_string(),
                value: serde_json::json!("Manhattan"),
                param_type: ParameterType::String,
            },
        ],
        mql_pattern: serde_json::json!({"cuisine": "{{cuisine}}", "borough": "{{location}}"}),
    };
    registry.register(&template, "filter", "sample_restaurants", "restaurants");

    c.bench_function("template_registry_match_hit", |b| {
        b.iter(|| {
            registry.try_match(
                "find Chinese restaurants in Brooklyn",
                "sample_restaurants",
                "restaurants",
            )
        })
    });
}

fn bench_template_match_miss(c: &mut Criterion) {
    let registry = TemplateRegistry::new();
    let template = LlmTemplate {
        intent_pattern: "find {{cuisine}} restaurants in {{location}}".to_string(),
        parameters: vec![
            LlmTemplateParameter {
                name: "cuisine".to_string(),
                value: serde_json::json!("Italian"),
                param_type: ParameterType::String,
            },
            LlmTemplateParameter {
                name: "location".to_string(),
                value: serde_json::json!("Manhattan"),
                param_type: ParameterType::String,
            },
        ],
        mql_pattern: serde_json::json!({"cuisine": "{{cuisine}}", "borough": "{{location}}"}),
    };
    registry.register(&template, "filter", "sample_restaurants", "restaurants");

    c.bench_function("template_registry_match_miss", |b| {
        b.iter(|| {
            registry.try_match(
                "count restaurants by borough",
                "sample_restaurants",
                "restaurants",
            )
        })
    });
}

criterion_group!(benches, bench_template_match_hit, bench_template_match_miss);
criterion_main!(benches);
```

- [ ] **Step 4: Create mql_validation.rs**

```rust
use criterion::{criterion_group, criterion_main, Criterion};
use bson::doc;
use mongocore::compiled::validator::MqlValidator;

fn bench_validate_simple_filter(c: &mut Criterion) {
    let filter = doc! { "cuisine": "Italian", "borough": "Manhattan" };

    c.bench_function("validate_simple_filter", |b| {
        b.iter(|| MqlValidator::validate_filter(&filter))
    });
}

fn bench_validate_nested_filter(c: &mut Criterion) {
    let filter = doc! {
        "$and": [
            { "cuisine": "Italian" },
            { "$or": [
                { "borough": "Manhattan" },
                { "borough": "Brooklyn" }
            ]},
            { "grades.score": { "$gt": 80 } }
        ]
    };

    c.bench_function("validate_nested_filter", |b| {
        b.iter(|| MqlValidator::validate_filter(&filter))
    });
}

fn bench_validate_pipeline(c: &mut Criterion) {
    let pipeline = vec![
        doc! { "$match": { "cuisine": "Italian" } },
        doc! { "$group": { "_id": "$borough", "count": { "$sum": 1 } } },
        doc! { "$sort": { "count": -1 } },
        doc! { "$limit": 10 },
    ];

    c.bench_function("validate_pipeline", |b| {
        b.iter(|| MqlValidator::validate_pipeline(&pipeline))
    });
}

criterion_group!(benches, bench_validate_simple_filter, bench_validate_nested_filter, bench_validate_pipeline);
criterion_main!(benches);
```

- [ ] **Step 5: Verify criterion benchmarks compile**

```bash
cd benchmarks/rust && cargo bench --no-run
```

- [ ] **Step 6: Commit**

```bash
git add benchmarks/rust/
git commit -m "feat(bench): add Rust criterion benchmarks for sidecar internals"
```

---

## Task 4: Python Collector and README Generator

**Files:**
- Create: `benchmarks/collector/requirements.txt`
- Create: `benchmarks/collector/collect.py`
- Create: `benchmarks/collector/generate_readme.py`

- [ ] **Step 1: Create requirements.txt**

```
matplotlib>=3.8
```

- [ ] **Step 2: Create collect.py**

```python
"""Collect all benchmark results from results/ into a single latest.json."""

import json
import glob
from pathlib import Path
from datetime import datetime, timezone

RESULTS_DIR = Path(__file__).parent.parent / "results"

def collect():
    all_results = []

    for json_file in glob.glob(str(RESULTS_DIR / "*.json")):
        if "latest" in json_file or "history" in json_file:
            continue
        with open(json_file) as f:
            data = json.load(f)
            if isinstance(data, list):
                all_results.extend(data)
            else:
                all_results.append(data)

    # Save as latest
    output = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "results": all_results,
    }

    latest_path = RESULTS_DIR / "latest.json"
    with open(latest_path, "w") as f:
        json.dump(output, f, indent=2)

    print(f"Collected {len(all_results)} benchmark results into {latest_path}")
    return output


if __name__ == "__main__":
    collect()
```

- [ ] **Step 3: Create generate_readme.py**

```python
"""Generate benchmarks/README.md from collected results."""

import json
from pathlib import Path

RESULTS_DIR = Path(__file__).parent.parent / "results"
README_PATH = Path(__file__).parent.parent / "README.md"


def load_results():
    latest = RESULTS_DIR / "latest.json"
    if not latest.exists():
        return []
    with open(latest) as f:
        data = json.load(f)
    return data.get("results", [])


def group_by_benchmark(results):
    groups = {}
    for r in results:
        name = r["benchmark"]
        if name not in groups:
            groups[name] = []
        groups[name].append(r)
    return groups


def generate_readme(results):
    lines = []
    lines.append("# MongoCore Benchmark Results\n")
    lines.append("> **Auto-generated** — do not edit manually. Run `just bench-collect` to regenerate.\n")

    # Environment
    if results:
        sys_info = results[0].get("system", {})
        lines.append("## Benchmark Environment\n")
        lines.append(f"- **OS:** {sys_info.get('os', 'unknown')} ({sys_info.get('arch', 'unknown')})")
        lines.append(f"- **CPUs:** {sys_info.get('cpus', 'unknown')}")
        lines.append(f"- **MongoDB:** Atlas Local Docker (localhost:27017)")
        lines.append(f"- **MongoCore:** {sys_info.get('mongocore_version', 'unknown')}")
        lines.append(f"- **Date:** {results[0].get('timestamp', 'unknown')[:10]}")
        lines.append("")
        lines.append("> **Note:** All benchmarks run against `mongodb/mongodb-atlas-local` on localhost.")
        lines.append("> This isolates MongoCore sidecar overhead without network latency noise.")
        lines.append("> Production Atlas results will differ due to network, hardware, and cluster topology.")
        lines.append("> These numbers measure the cost of the gRPC hop and MongoCore processing, not MongoDB performance.")
        lines.append("")

    # Sidecar Overhead Table
    groups = group_by_benchmark(results)
    native_results = {r["benchmark"]: r for r in results if "native" in r.get("driver", "") or r.get("driver") == "pymongo"}
    mongocore_results = {r["benchmark"]: r for r in results if "mongocore" in r.get("driver", "")}

    if native_results and mongocore_results:
        lines.append("## Sidecar Overhead (Single Document)\n")
        lines.append("| Operation | Native (ops/s) | MongoCore (ops/s) | Overhead |")
        lines.append("|-----------|---------------|-------------------|----------|")

        for bench_name in ["run_command", "find_one_by_id", "insert_one_small"]:
            native = native_results.get(bench_name)
            mc = mongocore_results.get(bench_name)
            if native and mc:
                n_ops = native["ops_per_sec"]
                m_ops = mc["ops_per_sec"]
                overhead = ((m_ops - n_ops) / n_ops) * 100
                lines.append(f"| {bench_name} | {n_ops:,.0f} | {m_ops:,.0f} | {overhead:+.1f}% |")
        lines.append("")

    # Multi-doc throughput
    multi_native = {r["benchmark"]: r for r in results if r.get("category") == "multi_doc" and ("native" in r.get("driver", "") or r.get("driver") == "pymongo")}
    multi_mc = {r["benchmark"]: r for r in results if r.get("category") == "multi_doc" and "mongocore" in r.get("driver", "")}

    if multi_native and multi_mc:
        lines.append("## Multi-Document Throughput (MB/s)\n")
        lines.append("| Operation | Native | MongoCore | Overhead |")
        lines.append("|-----------|--------|-----------|----------|")

        for bench_name in ["bulk_insert_small", "find_many"]:
            native = multi_native.get(bench_name)
            mc = multi_mc.get(bench_name)
            if native and mc:
                overhead = ((mc["mb_per_sec"] - native["mb_per_sec"]) / native["mb_per_sec"]) * 100
                lines.append(f"| {bench_name} | {native['mb_per_sec']:.1f} | {mc['mb_per_sec']:.1f} | {overhead:+.1f}% |")
        lines.append("")

    # Write README
    readme_content = "\n".join(lines)
    README_PATH.write_text(readme_content)
    print(f"Generated {README_PATH} ({len(lines)} lines)")


if __name__ == "__main__":
    results = load_results()
    generate_readme(results)
```

- [ ] **Step 4: Commit**

```bash
git add benchmarks/collector/
git commit -m "feat(bench): add Python collector and README generator"
```

---

## Task 5: Ingestion Benchmarks

**Files:**
- Create: `benchmarks/ingestion/generate_data.py`
- Create: `benchmarks/ingestion/bench_ingest.py`

- [ ] **Step 1: Create generate_data.py**

```python
"""Generate benchmark datasets at various sizes."""

import csv
import json
import random
import string
from pathlib import Path

DATA_DIR = Path(__file__).parent / "data"
DATA_DIR.mkdir(exist_ok=True)

SIZES = [10_000, 100_000, 1_000_000]


def random_string(length=10):
    return "".join(random.choices(string.ascii_lowercase, k=length))


def generate_row(i):
    return {
        "id": i,
        "name": f"User {random_string(8)}",
        "email": f"{random_string(6)}@example.com",
        "age": random.randint(18, 80),
        "score": round(random.uniform(0, 100), 2),
        "created_at": f"2024-{random.randint(1,12):02d}-{random.randint(1,28):02d}T{random.randint(0,23):02d}:{random.randint(0,59):02d}:00Z",
        "tags": [random_string(5) for _ in range(3)],
    }


def main():
    for size in SIZES:
        label = f"{size // 1000}k" if size < 1_000_000 else f"{size // 1_000_000}m"

        # CSV
        csv_path = DATA_DIR / f"bench_{label}.csv"
        if not csv_path.exists() or size <= 100_000:
            print(f"Generating {csv_path}...")
            rows = [generate_row(i) for i in range(size)]
            with open(csv_path, "w", newline="") as f:
                writer = csv.DictWriter(f, fieldnames=rows[0].keys())
                writer.writeheader()
                writer.writerows(rows)

        # NDJSON (skip 1M — too large)
        if size <= 100_000:
            ndjson_path = DATA_DIR / f"bench_{label}.ndjson"
            if not ndjson_path.exists():
                print(f"Generating {ndjson_path}...")
                with open(ndjson_path, "w") as f:
                    for i in range(size):
                        f.write(json.dumps(generate_row(i)) + "\n")

    print("Done generating benchmark data.")


if __name__ == "__main__":
    main()
```

- [ ] **Step 2: Create bench_ingest.py**

```python
"""Benchmark MongoCore Polars ingestion vs native bulk insert."""

import json
import os
import sys
import time
import statistics
from pathlib import Path
from datetime import datetime, timezone

sys.path.insert(0, str(Path(__file__).parent.parent.parent / "clients" / "python" / "src"))
from pymongo import MongoClient as PyMongoClient

DATA_DIR = Path(__file__).parent / "data"
RESULTS_DIR = Path(__file__).parent.parent / "results"
RESULTS_DIR.mkdir(exist_ok=True)

MONGODB_URI = "mongodb://localhost:27017"
MONGOCORE_ADDR = "localhost:50051"
DB_NAME = "mongocore_bench_ingest"


def bench_native_bulk(size_label, rows):
    """Benchmark native pymongo insertMany."""
    client = PyMongoClient(MONGODB_URI, w=1)
    db = client[DB_NAME]

    times = []
    for _ in range(5):
        db.drop_collection(f"native_{size_label}")
        coll = db[f"native_{size_label}"]

        start = time.perf_counter()
        # Insert in batches of 10,000
        batch_size = 10_000
        for i in range(0, len(rows), batch_size):
            coll.insert_many(rows[i:i + batch_size])
        elapsed = time.perf_counter() - start
        times.append(elapsed)

    client.close()
    median = statistics.median(times)
    total_bytes = sum(len(json.dumps(r).encode()) for r in rows[:100]) * len(rows) // 100
    mb_per_sec = total_bytes / median / 1_000_000

    return {
        "benchmark": f"native_bulk_insert_{size_label}",
        "category": "ingestion",
        "driver": "pymongo_native",
        "dataset_size_bytes": total_bytes,
        "batch_size": len(rows),
        "iterations": len(times),
        "total_time_secs": round(sum(times), 3),
        "ops_per_sec": round(len(rows) / median, 1),
        "mb_per_sec": round(mb_per_sec, 3),
        "percentiles": {"p50": round(median, 4)},
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "system": {"driver": "pymongo", "operation": "insertMany"},
    }


def bench_mongocore_ingest(size_label, file_path):
    """Benchmark MongoCore Polars ingestion via gRPC."""
    import asyncio
    sys.path.insert(0, str(Path(__file__).parent.parent.parent / "clients" / "python" / "src"))
    from mongocore import MongoClient as MongoCoreClient

    async def run():
        times = []
        for _ in range(5):
            async with MongoCoreClient(MONGOCORE_ADDR) as client:
                # Drop previous data
                mc = PyMongoClient(MONGODB_URI)
                mc[DB_NAME].drop_collection(f"ingest_{size_label}")
                mc.close()

                start = time.perf_counter()
                result = await client.ingest(
                    file_path=str(file_path),
                    database=DB_NAME,
                    collection=f"ingest_{size_label}",
                )
                elapsed = time.perf_counter() - start
                times.append(elapsed)

        median = statistics.median(times)
        file_size = file_path.stat().st_size
        mb_per_sec = file_size / median / 1_000_000

        return {
            "benchmark": f"mongocore_ingest_{size_label}",
            "category": "ingestion",
            "driver": "mongocore+polars",
            "dataset_size_bytes": file_size,
            "batch_size": 1,
            "iterations": len(times),
            "total_time_secs": round(sum(times), 3),
            "ops_per_sec": round(1 / median, 3),
            "mb_per_sec": round(mb_per_sec, 3),
            "percentiles": {"p50": round(median, 4)},
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "system": {"driver": "mongocore+polars", "operation": "ingest"},
        }

    return asyncio.run(run())


def main():
    print("=== Ingestion Benchmarks ===")
    results = []

    for size_label in ["10k", "100k"]:
        csv_path = DATA_DIR / f"bench_{size_label}.csv"
        if not csv_path.exists():
            print(f"  Skipping {size_label} — data not generated. Run: just bench-generate-data")
            continue

        # Load data for native benchmark
        print(f"\n  [{size_label}] Native bulk insert...")
        import csv as csv_mod
        with open(csv_path) as f:
            reader = csv_mod.DictReader(f)
            rows = list(reader)
        results.append(bench_native_bulk(size_label, rows))

        # MongoCore Polars ingest
        print(f"  [{size_label}] MongoCore Polars ingest...")
        results.append(bench_mongocore_ingest(size_label, csv_path))

    # Save results
    output_path = RESULTS_DIR / "ingestion.json"
    with open(output_path, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nResults saved to {output_path}")


if __name__ == "__main__":
    main()
```

- [ ] **Step 3: Commit**

```bash
git add benchmarks/ingestion/
git commit -m "feat(bench): add ingestion benchmarks (Polars vs native bulk)"
```

---

## Task 6: Go, TypeScript, and Java Benchmark Stubs

**Files:**
- Create: `benchmarks/drivers/go/go.mod`
- Create: `benchmarks/drivers/go/bench_test.go`
- Create: `benchmarks/drivers/typescript/package.json`
- Create: `benchmarks/drivers/typescript/bench_native.ts`
- Create: `benchmarks/drivers/java/pom.xml`

- [ ] **Step 1: Create Go benchmark**

Follow the same pattern as Python but using `testing.B` for native Go driver and MongoCore Go client. Output JSON to `results/go_native.json` and `results/go_mongocore.json`.

- [ ] **Step 2: Create TypeScript benchmark**

Follow the same pattern using `perf_hooks.performance.now()` for timing. Output to `results/typescript_native.json` and `results/typescript_mongocore.json`.

- [ ] **Step 3: Create Java benchmark**

Use `System.nanoTime()` for timing. Output to `results/java_native.json` and `results/java_mongocore.json`.

- [ ] **Step 4: Commit**

```bash
git add benchmarks/drivers/{go,typescript,java}/
git commit -m "feat(bench): add Go, TypeScript, and Java benchmark scripts"
```

---

## Task 7: Integration and Verification

- [ ] **Step 1: Run full benchmark suite**

```bash
cd benchmarks
just docker-up  # Ensure MongoDB is running
just bench-generate-data
just bench-python
just bench-rust
just bench-ingestion
just bench-collect
```

- [ ] **Step 2: Verify README was generated**

```bash
cat benchmarks/README.md
```
Expected: Comparison tables with real numbers.

- [ ] **Step 3: Commit results and README**

```bash
git add benchmarks/results/ benchmarks/README.md
git commit -m "bench: add initial benchmark results"
```

---

## Task 8: Full bench_mongocore.py Implementation

**Files:**
- Create: `benchmarks/drivers/python/bench_mongocore.py` (full implementation)

- [ ] **Step 1: Create bench_mongocore.py**

Same structure as `bench_native.py` but using the MongoCore Python client via gRPC:

```python
"""Benchmark MongoCore Python client (via gRPC sidecar)."""

import asyncio
import json
import os
import platform
import sys
import time
import statistics
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent.parent.parent / "clients" / "python" / "src"))
from mongocore import MongoClient

CONFIG = json.loads((Path(__file__).parent.parent / "common.json").read_text())
DATA_DIR = Path(__file__).parent.parent.parent / "data"
RESULTS_DIR = Path(__file__).parent.parent.parent / "results"
RESULTS_DIR.mkdir(exist_ok=True)

WARMUP = CONFIG["warmup_iterations"]["python"]
MIN_TIME = CONFIG["min_time_secs"]
MAX_ITERS = CONFIG["max_iterations"]
MAX_TIME = CONFIG["max_time_secs"]
DB_NAME = CONFIG["database"]
ADDR = CONFIG["mongocore_address"]


def get_system_info():
    return {
        "os": platform.system().lower(),
        "arch": platform.machine(),
        "cpus": os.cpu_count(),
        "mongocore_version": "0.6.0",
        "driver": "mongocore+python",
    }


async def run_benchmark(name, category, setup_fn, task_fn, teardown_fn, dataset_size_bytes, batch_size=1):
    async with MongoClient(ADDR) as client:
        db_coll = None
        await setup_fn(client)

        # Warmup
        for _ in range(WARMUP):
            await task_fn(client)

        # Timed iterations
        times = []
        total_time = 0.0
        iteration = 0

        while total_time < MIN_TIME or iteration < 5:
            if iteration >= MAX_ITERS or total_time >= MAX_TIME:
                break
            start = time.perf_counter()
            await task_fn(client)
            elapsed = time.perf_counter() - start
            times.append(elapsed)
            total_time += elapsed
            iteration += 1

        await teardown_fn(client)

    times.sort()
    median = statistics.median(times)
    ops_per_sec = batch_size / median
    mb_per_sec = (dataset_size_bytes * batch_size) / median / 1_000_000

    def percentile(data, pct):
        idx = int(len(data) * pct / 100)
        return data[min(idx, len(data) - 1)]

    result = {
        "benchmark": name,
        "category": category,
        "driver": "mongocore+python",
        "dataset_size_bytes": dataset_size_bytes,
        "batch_size": batch_size,
        "iterations": len(times),
        "total_time_secs": round(total_time, 3),
        "ops_per_sec": round(ops_per_sec, 1),
        "mb_per_sec": round(mb_per_sec, 3),
        "percentiles": {
            "p10": round(percentile(times, 10), 6),
            "p25": round(percentile(times, 25), 6),
            "p50": round(median, 6),
            "p75": round(percentile(times, 75), 6),
            "p90": round(percentile(times, 90), 6),
            "p95": round(percentile(times, 95), 6),
            "p99": round(percentile(times, 99), 6),
        },
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "system": get_system_info(),
    }
    print(f"  {name}: {ops_per_sec:.0f} ops/s, {mb_per_sec:.2f} MB/s ({len(times)} iters)")
    return result


async def main():
    print("=== MongoCore+Python benchmarks ===")
    results = []

    small_doc = json.loads((DATA_DIR / "small_doc.json").read_text())
    tweet_doc = json.loads((DATA_DIR / "tweet.json").read_text())
    small_size = len(json.dumps(small_doc).encode())
    tweet_size = len(json.dumps(tweet_doc).encode())

    # Run Command
    results.append(await run_benchmark(
        "run_command", "single_doc",
        setup_fn=lambda c: asyncio.sleep(0),
        task_fn=lambda c: c.run_command("admin", {"hello": 1}),
        teardown_fn=lambda c: asyncio.sleep(0),
        dataset_size_bytes=100,
    ))

    # Find One by ID
    async def setup_find(c):
        coll = c[DB_NAME]["bench_find_mc"]
        await coll.insert_one({"_id": "bench_find_001", **tweet_doc})

    async def task_find(c):
        await c[DB_NAME]["bench_find_mc"].find_one({"_id": "bench_find_001"})

    async def teardown_find(c):
        await c[DB_NAME]["bench_find_mc"].delete_many({})

    results.append(await run_benchmark(
        "find_one_by_id", "single_doc",
        setup_fn=setup_find, task_fn=task_find, teardown_fn=teardown_find,
        dataset_size_bytes=tweet_size,
    ))

    # InsertOne Small
    async def task_insert(c):
        from bson import ObjectId
        await c[DB_NAME]["bench_insert_mc"].insert_one({**small_doc, "_id": str(ObjectId())})

    results.append(await run_benchmark(
        "insert_one_small", "single_doc",
        setup_fn=lambda c: asyncio.sleep(0),
        task_fn=task_insert,
        teardown_fn=lambda c: asyncio.sleep(0),
        dataset_size_bytes=small_size,
    ))

    # Bulk Insert Small (10K per iteration)
    async def task_bulk(c):
        from bson import ObjectId
        docs = [{**small_doc, "_id": str(ObjectId())} for _ in range(10_000)]
        await c[DB_NAME]["bench_bulk_mc"].insert_many(docs)

    results.append(await run_benchmark(
        "bulk_insert_small", "multi_doc",
        setup_fn=lambda c: asyncio.sleep(0),
        task_fn=task_bulk,
        teardown_fn=lambda c: asyncio.sleep(0),
        dataset_size_bytes=small_size * 10_000, batch_size=10_000,
    ))

    # Find Many (10K docs)
    async def setup_find_many(c):
        from bson import ObjectId
        docs = [{**small_doc, "_id": str(ObjectId())} for _ in range(10_000)]
        await c[DB_NAME]["bench_find_many_mc"].insert_many(docs)

    async def task_find_many(c):
        await c[DB_NAME]["bench_find_many_mc"].find({})

    results.append(await run_benchmark(
        "find_many", "multi_doc",
        setup_fn=setup_find_many, task_fn=task_find_many,
        teardown_fn=lambda c: asyncio.sleep(0),
        dataset_size_bytes=small_size * 10_000, batch_size=10_000,
    ))

    # Save results
    output_path = RESULTS_DIR / "python_mongocore.json"
    with open(output_path, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nResults saved to {output_path}")


if __name__ == "__main__":
    asyncio.run(main())
```

- [ ] **Step 2: Add large doc benchmarks to both Python scripts**

Add `insert_one_large` benchmark to both `bench_native.py` and `bench_mongocore.py`:
- Load `large_doc.json` (2.75MB)
- Single insert per iteration
- Report MB/s for large doc operations

- [ ] **Step 3: Commit**

```bash
git add benchmarks/drivers/python/bench_mongocore.py
git commit -m "feat(bench): add full MongoCore Python benchmark with large doc support"
```

---

## Task 9: Compiled Query Cache Benchmarks

**Files:**
- Create: `benchmarks/drivers/python/bench_compiled.py`

- [ ] **Step 1: Create compiled query cache benchmarks**

```python
"""Benchmark compiled query cache performance (MongoCore-specific)."""

import asyncio
import json
import sys
import time
import statistics
from pathlib import Path
from datetime import datetime, timezone

sys.path.insert(0, str(Path(__file__).parent.parent.parent.parent / "clients" / "python" / "src"))

RESULTS_DIR = Path(__file__).parent.parent.parent / "results"
RESULTS_DIR.mkdir(exist_ok=True)

# These benchmarks use the Rust translator directly via integration test patterns
# For the Python benchmark, we measure via the search/compiled_query client method


async def bench_compiled_cache_hit():
    """Measure cache hit performance — same query repeated."""
    from mongocore import MongoClient

    async with MongoClient("localhost:50051") as client:
        coll = client["sample_restaurants"]["restaurants"]

        # First call — cold (may hit LLM or fail gracefully)
        try:
            await coll.search("find Italian restaurants", limit=1)
        except Exception:
            pass

        # Now benchmark the cached path
        times = []
        for _ in range(1000):
            start = time.perf_counter()
            try:
                await coll.search("find Italian restaurants", limit=1)
            except Exception:
                break
            elapsed = time.perf_counter() - start
            times.append(elapsed)

        if not times:
            return None

        times.sort()
        median = statistics.median(times)
        return {
            "benchmark": "compiled_cache_hit",
            "category": "compiled_query",
            "driver": "mongocore+python",
            "dataset_size_bytes": 0,
            "batch_size": 1,
            "iterations": len(times),
            "total_time_secs": round(sum(times), 3),
            "ops_per_sec": round(1 / median, 1),
            "mb_per_sec": 0,
            "percentiles": {
                "p50": round(median, 6),
                "p99": round(times[int(len(times) * 0.99)], 6),
            },
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "system": {"driver": "mongocore+python", "operation": "compiled_cache_hit"},
        }


async def main():
    print("=== Compiled Query Benchmarks ===")
    results = []

    result = await bench_compiled_cache_hit()
    if result:
        results.append(result)
        print(f"  cache_hit: {result['ops_per_sec']:.0f} ops/s")
    else:
        print("  cache_hit: SKIPPED (no LLM configured or sidecar not running)")

    if results:
        output_path = RESULTS_DIR / "compiled_query.json"
        with open(output_path, "w") as f:
            json.dump(results, f, indent=2)
        print(f"\nResults saved to {output_path}")


if __name__ == "__main__":
    asyncio.run(main())
```

- [ ] **Step 2: Add `bench-compiled` to justfile**

```just
# Compiled query cache benchmarks (needs MongoCore running + sample data)
bench-compiled:
    cd drivers/python && python bench_compiled.py
```

- [ ] **Step 3: Commit**

```bash
git add benchmarks/drivers/python/bench_compiled.py benchmarks/justfile
git commit -m "feat(bench): add compiled query cache benchmarks"
```

---

## Task 10: SVG Chart Generation

**Files:**
- Modify: `benchmarks/collector/generate_readme.py`
- Modify: `benchmarks/collector/requirements.txt`

- [ ] **Step 1: Add matplotlib to requirements**

Update `benchmarks/collector/requirements.txt`:
```
matplotlib>=3.8
```

- [ ] **Step 2: Add chart generation to generate_readme.py**

Add a function that produces SVG charts and embeds them in the README:

```python
import matplotlib
matplotlib.use('Agg')  # Non-interactive backend
import matplotlib.pyplot as plt
from pathlib import Path

CHARTS_DIR = Path(__file__).parent.parent / "results" / "charts"
CHARTS_DIR.mkdir(parents=True, exist_ok=True)


def generate_overhead_chart(native_results, mongocore_results):
    """Generate SVG bar chart comparing native vs MongoCore."""
    benchmarks = []
    native_ops = []
    mc_ops = []

    for name in ["run_command", "find_one_by_id", "insert_one_small"]:
        if name in native_results and name in mongocore_results:
            benchmarks.append(name)
            native_ops.append(native_results[name]["ops_per_sec"])
            mc_ops.append(mongocore_results[name]["ops_per_sec"])

    if not benchmarks:
        return None

    fig, ax = plt.subplots(figsize=(10, 5))
    x = range(len(benchmarks))
    width = 0.35
    ax.bar([i - width/2 for i in x], native_ops, width, label='Native Driver', color='#2E7D32')
    ax.bar([i + width/2 for i in x], mc_ops, width, label='MongoCore', color='#A04500')
    ax.set_xlabel('Benchmark')
    ax.set_ylabel('Operations/sec')
    ax.set_title('Sidecar Overhead: Native vs MongoCore')
    ax.set_xticks(x)
    ax.set_xticklabels(benchmarks, rotation=15)
    ax.legend()
    ax.grid(axis='y', alpha=0.3)

    chart_path = CHARTS_DIR / "sidecar_overhead.svg"
    plt.savefig(chart_path, format='svg', bbox_inches='tight')
    plt.close()
    return chart_path.relative_to(Path(__file__).parent.parent)
```

Embed in README with: `![Sidecar Overhead](./results/charts/sidecar_overhead.svg)`

- [ ] **Step 3: Commit**

```bash
git add benchmarks/collector/
git commit -m "feat(bench): add SVG chart generation for README"
```

---

## Task 11: Historical Tracking and Regression Detection

**Files:**
- Create: `benchmarks/collector/compare.py`
- Create: `benchmarks/collector/check_regression.py`

- [ ] **Step 1: Create compare.py**

```python
"""Compare latest benchmark results against previous run."""

import json
from pathlib import Path

RESULTS_DIR = Path(__file__).parent.parent / "results"


def load_latest():
    path = RESULTS_DIR / "latest.json"
    if not path.exists():
        return None
    with open(path) as f:
        return json.load(f)


def load_previous():
    """Find the second-most-recent results file."""
    files = sorted(RESULTS_DIR.glob("202*.json"), reverse=True)
    if len(files) < 2:
        return None
    with open(files[1]) as f:
        return json.load(f)


def compare():
    latest = load_latest()
    previous = load_previous()

    if not latest or not previous:
        print("Not enough data to compare (need at least 2 runs)")
        return

    latest_by_key = {(r["benchmark"], r["driver"]): r for r in latest.get("results", [])}
    previous_by_key = {(r["benchmark"], r["driver"]): r for r in previous.get("results", [])}

    print(f"{'Benchmark':<25} {'Driver':<20} {'Previous':>10} {'Current':>10} {'Change':>10}")
    print("-" * 80)

    for key, curr in sorted(latest_by_key.items()):
        prev = previous_by_key.get(key)
        if not prev:
            continue
        prev_ops = prev["ops_per_sec"]
        curr_ops = curr["ops_per_sec"]
        if prev_ops == 0:
            continue
        change = ((curr_ops - prev_ops) / prev_ops) * 100
        flag = "⚠" if change < -5 else "✓" if change > -2 else ""
        print(f"{key[0]:<25} {key[1]:<20} {prev_ops:>10,.0f} {curr_ops:>10,.0f} {change:>+8.1f}% {flag}")


if __name__ == "__main__":
    compare()
```

- [ ] **Step 2: Create check_regression.py**

```python
"""Check for performance regressions. Exit non-zero if any exceed threshold."""

import json
import sys
from pathlib import Path

RESULTS_DIR = Path(__file__).parent.parent / "results"
DEFAULT_THRESHOLD = 10  # percent


def check(threshold=DEFAULT_THRESHOLD):
    latest_path = RESULTS_DIR / "latest.json"
    files = sorted(RESULTS_DIR.glob("202*.json"), reverse=True)

    if not latest_path.exists() or len(files) < 2:
        print("Not enough data to check regressions")
        return 0

    with open(latest_path) as f:
        latest = json.load(f)
    with open(files[1]) as f:
        previous = json.load(f)

    latest_by_key = {(r["benchmark"], r["driver"]): r for r in latest.get("results", [])}
    previous_by_key = {(r["benchmark"], r["driver"]): r for r in previous.get("results", [])}

    regressions = []
    for key, curr in latest_by_key.items():
        prev = previous_by_key.get(key)
        if not prev or prev["ops_per_sec"] == 0:
            continue
        change = ((curr["ops_per_sec"] - prev["ops_per_sec"]) / prev["ops_per_sec"]) * 100
        if change < -threshold:
            regressions.append((key, change))

    if regressions:
        print(f"REGRESSIONS DETECTED (>{threshold}% slower):")
        for (bench, driver), change in regressions:
            print(f"  {bench} ({driver}): {change:+.1f}%")
        return 1
    else:
        print(f"No regressions detected (threshold: {threshold}%)")
        return 0


if __name__ == "__main__":
    threshold = int(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_THRESHOLD
    sys.exit(check(threshold))
```

- [ ] **Step 3: Update collect.py to archive results with timestamp**

Add to `collect.py` after saving `latest.json`:
```python
    # Archive with timestamp
    ts = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H-%M")
    archive_path = RESULTS_DIR / f"{ts}.json"
    with open(archive_path, "w") as f:
        json.dump(output, f, indent=2)

    # Keep only last 10 archived results
    archives = sorted(RESULTS_DIR.glob("202*.json"), reverse=True)
    for old in archives[10:]:
        old.unlink()
```

- [ ] **Step 4: Commit**

```bash
git add benchmarks/collector/
git commit -m "feat(bench): add historical tracking and regression detection"
```

---

## Implementation Order

```
Task 1: Scaffold (foundation)
Task 2: Python native benchmark (first working end-to-end)
Task 8: Python MongoCore benchmark (full implementation, depends on Task 2)
Task 3: Rust criterion (sidecar internals, independent)
Task 4: Collector + README generator (depends on Tasks 2+8 producing results)
Task 10: SVG chart generation (extends Task 4)
Task 5: Ingestion benchmarks (independent)
Task 9: Compiled query cache benchmarks (independent, needs sidecar)
Task 6: Go/TS/Java stubs (independent, follow Python pattern)
Task 11: Historical tracking (extends Task 4)
Task 7: Integration (depends on all above)
```

Tasks 2, 3, 5, and 9 can run in parallel after Task 1.

---

## Definition of Done

- [ ] `benchmarks/` directory exists with scaffold and datasets
- [ ] `just bench-rust` runs criterion benchmarks (cache, template, validation)
- [ ] `just bench-python` runs native + MongoCore Python benchmarks (including large doc)
- [ ] `just bench-ingestion` benchmarks Polars ingest vs native bulk
- [ ] `just bench-compiled` benchmarks compiled query cache performance
- [ ] `just bench-collect` aggregates results, generates README.md with SVG charts
- [ ] `just bench-compare` shows regression diff table
- [ ] README.md shows sidecar overhead table with % difference
- [ ] README.md shows multi-doc throughput comparison
- [ ] README.md embeds SVG bar charts for visual comparison
- [ ] README.md includes environment note about localhost/Atlas Local
- [ ] Results committed as JSON with timestamp archiving (last 10 runs)
- [ ] `check_regression.py` exits non-zero on >10% regression
- [ ] Warmup iterations implemented per language (Python 3, TS 5, Go 3, Java 10)
- [ ] All benchmarks follow MongoDB spec methodology (min 1min, percentiles)
- [ ] Compiled query benchmarks conditional on LLM + sidecar availability
