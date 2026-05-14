# Pipeline Benchmarks Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add pipeline-batched benchmark variants (at 3 batch sizes) to all 4 language benchmark suites, move the Rust pipeline bench to the main benchmarks folder, and update the results template/collector to render pipeline and compiled query results.

**Architecture:** New `bench_pipeline.*` files per language use the existing `ops` module and `client.pipeline()` method to batch operations into single RPC calls. The collector groups pipeline results by batch size to render comparison tables and a scaling chart.

**Tech Stack:** Python (asyncio + mongocore client), TypeScript (mongocore TS client via ts-node), Go (mongocore client + pb proto), Java (MongoClient + Ops builder), Jinja2 templates, matplotlib charts.

---

### Task 1: Move Rust pipeline benchmark

**Files:**
- Delete: `benches/pipeline.rs`
- Create: `benchmarks/rust/benches/pipeline.rs`
- Modify: `benchmarks/rust/Cargo.toml`
- Modify: `Cargo.toml` (root, lines 60-64)

- [ ] **Step 1: Add `tonic` dependency and bench entry to `benchmarks/rust/Cargo.toml`**

Add to `[dependencies]` section:
```toml
tonic = "0.12"
```

Add at end of file:
```toml
[[bench]]
name = "pipeline"
harness = false
```

- [ ] **Step 2: Copy `benches/pipeline.rs` to `benchmarks/rust/benches/pipeline.rs` and update paths**

Copy the file, then change line 8 from:
```rust
#[path = "../tests/harness/mod.rs"]
```
to:
```rust
#[path = "../../../tests/harness/mod.rs"]
```

- [ ] **Step 3: Remove `benches/pipeline.rs` and clean root `Cargo.toml`**

Delete `benches/pipeline.rs` and remove these lines from root `Cargo.toml` (around lines 60-64):
```toml
criterion = { version = "0.5", features = ["async_tokio"] }

[[bench]]
name = "pipeline"
harness = false
```

- [ ] **Step 4: Verify it compiles**

Run: `cd benchmarks/rust && cargo bench --bench pipeline --no-run`
Expected: Compiles without errors (won't run without MongoDB).

- [ ] **Step 5: Verify root still builds clean**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output (zero warnings).

- [ ] **Step 6: Commit**

```bash
git add -A benches/ benchmarks/rust/ Cargo.toml
git commit -m "chore: move pipeline benchmark to benchmarks/rust/"
```

---

### Task 2: Python pipeline benchmark

**Files:**
- Create: `benchmarks/drivers/python/bench_pipeline.py`

- [ ] **Step 1: Create `benchmarks/drivers/python/bench_pipeline.py`**

```python
"""Benchmark MongoCore pipeline batching at different batch sizes."""

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
from mongocore import ops

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
SOCKET_PATH = CONFIG.get("mongocore_socket_path", "/tmp/mongocore.sock")

BATCH_SIZES = [100, 1000, 10000]
TOTAL_OPS = 10000

_ACTUAL_TRANSPORT = "tcp"


def get_system_info():
    return {
        "os": platform.system().lower(),
        "arch": platform.machine(),
        "cpus": os.cpu_count(),
        "mongocore_version": "0.6.0",
        "driver": "mongocore+python",
        "transport": _ACTUAL_TRANSPORT,
    }


async def run_benchmark(name, category, client, setup_fn, before_task_fn, task_fn, after_task_fn, teardown_fn, dataset_size_bytes, batch_size):
    """Run a benchmark following MongoDB spec methodology."""
    await setup_fn(client)

    for _ in range(WARMUP):
        await before_task_fn(client)
        await task_fn(client)
        await after_task_fn(client)

    times = []
    total_time = 0.0
    iteration = 0

    while total_time < MIN_TIME or iteration < 5:
        if iteration >= MAX_ITERS or total_time >= MAX_TIME:
            break

        await before_task_fn(client)
        start = time.perf_counter()
        await task_fn(client)
        elapsed = time.perf_counter() - start
        await after_task_fn(client)

        times.append(elapsed)
        total_time += elapsed
        iteration += 1

    await teardown_fn(client)

    times.sort()
    median = statistics.median(times)
    ops_per_sec = batch_size / median
    mb_per_sec = dataset_size_bytes / median / 1_000_000

    def percentile(data, pct):
        import math
        idx = max(0, math.ceil(len(data) * pct / 100) - 1)
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
    print("=== MongoCore+Python Pipeline benchmarks ===")
    results = []

    small_doc = json.loads((DATA_DIR / "small_doc.json").read_text())
    tweet_doc = json.loads((DATA_DIR / "tweet.json").read_text())
    small_size = len(json.dumps(small_doc).encode())
    tweet_size = len(json.dumps(tweet_doc).encode())

    # Connect (try UDS first, fall back to TCP)
    client = None
    if os.path.exists(SOCKET_PATH):
        try:
            client = MongoClient(ADDR, socket_path=SOCKET_PATH)
            await client.connect()
            await client.run_command("admin", {"hello": 1})
        except Exception:
            await client.close() if client else None
            client = None

    if client is None:
        client = MongoClient(ADDR)
        await client.connect()
    else:
        global _ACTUAL_TRANSPORT
        _ACTUAL_TRANSPORT = "uds"

    for batch_size in BATCH_SIZES:
        calls_per_iter = TOTAL_OPS // batch_size

        # --- pipeline_run_command ---
        async def task_run_command(c, _bs=batch_size, _calls=calls_per_iter):
            for _ in range(_calls):
                operations = [ops.run_command(DB_NAME, {"hello": 1}) for _ in range(_bs)]
                await c.pipeline(*operations)

        results.append(await run_benchmark(
            f"pipeline_run_command_{batch_size}", "pipeline", client,
            setup_fn=lambda c: asyncio.sleep(0),
            before_task_fn=lambda c: asyncio.sleep(0),
            task_fn=task_run_command,
            after_task_fn=lambda c: asyncio.sleep(0),
            teardown_fn=lambda c: asyncio.sleep(0),
            dataset_size_bytes=TOTAL_OPS * 100,
            batch_size=TOTAL_OPS,
        ))

        # --- pipeline_insert_one_small ---
        async def before_insert(c):
            try:
                await c.run_command(DB_NAME, {"drop": "bench_pipeline_insert"})
            except Exception:
                pass

        async def task_insert_small(c, _bs=batch_size, _calls=calls_per_iter):
            from bson import ObjectId
            for _ in range(_calls):
                operations = [
                    ops.insert(DB_NAME, "bench_pipeline_insert", {**small_doc, "_id": str(ObjectId())})
                    for _ in range(_bs)
                ]
                await c.pipeline(*operations)

        results.append(await run_benchmark(
            f"pipeline_insert_one_small_{batch_size}", "pipeline", client,
            setup_fn=lambda c: asyncio.sleep(0),
            before_task_fn=before_insert,
            task_fn=task_insert_small,
            after_task_fn=lambda c: asyncio.sleep(0),
            teardown_fn=lambda c: asyncio.sleep(0),
            dataset_size_bytes=TOTAL_OPS * small_size,
            batch_size=TOTAL_OPS,
        ))

        # --- pipeline_find_one_by_id ---
        async def setup_find(c):
            try:
                await c.run_command(DB_NAME, {"drop": "bench_pipeline_find"})
            except Exception:
                pass
            coll = c[DB_NAME]["bench_pipeline_find"]
            await coll.insert_one({"_id": "bench_find_001", **tweet_doc})

        async def task_find_one(c, _bs=batch_size, _calls=calls_per_iter):
            for _ in range(_calls):
                operations = [
                    ops.find_one(DB_NAME, "bench_pipeline_find", {"_id": "bench_find_001"})
                    for _ in range(_bs)
                ]
                await c.pipeline(*operations)

        async def teardown_find(c):
            try:
                await c.run_command(DB_NAME, {"drop": "bench_pipeline_find"})
            except Exception:
                pass

        results.append(await run_benchmark(
            f"pipeline_find_one_by_id_{batch_size}", "pipeline", client,
            setup_fn=setup_find,
            before_task_fn=lambda c: asyncio.sleep(0),
            task_fn=task_find_one,
            after_task_fn=lambda c: asyncio.sleep(0),
            teardown_fn=teardown_find,
            dataset_size_bytes=TOTAL_OPS * tweet_size,
            batch_size=TOTAL_OPS,
        ))

    await client.close()

    output_path = RESULTS_DIR / "python_pipeline.json"
    with open(output_path, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nResults saved to {output_path}")


if __name__ == "__main__":
    asyncio.run(main())
```

- [ ] **Step 2: Verify syntax**

Run: `cd benchmarks/drivers/python && python -c "import ast; ast.parse(open('bench_pipeline.py').read()); print('OK')"`
Expected: `OK`

- [ ] **Step 3: Commit**

```bash
git add benchmarks/drivers/python/bench_pipeline.py
git commit -m "feat(bench): add Python pipeline benchmark (3 ops × 3 batch sizes)"
```

---

### Task 3: TypeScript pipeline benchmark

**Files:**
- Create: `benchmarks/drivers/typescript/bench_pipeline.ts`

Note: The existing `bench_mongocore.ts` uses raw `@grpc/grpc-js` + `@grpc/proto-loader` (not the TS client library, which has no built `dist/`). This benchmark follows the same pattern and calls the `Pipeline` RPC directly via the proto-loaded gRPC client.

- [ ] **Step 1: Create `benchmarks/drivers/typescript/bench_pipeline.ts`**

```typescript
/**
 * Benchmark MongoCore pipeline batching at different batch sizes.
 * Uses raw gRPC proto (same approach as bench_mongocore.ts).
 */

import { BSON } from "bson";
import * as grpc from '@grpc/grpc-js';
import * as protoLoader from '@grpc/proto-loader';
import { readFileSync, mkdirSync, writeFileSync } from 'fs';
import { join, resolve } from 'path';
import { performance } from 'perf_hooks';
import * as os from 'os';
import * as crypto from 'crypto';

const PROTO_PATH = resolve(__dirname, '..', '..', '..', 'proto', 'mongocore', 'v1', 'mongocore.proto');
const packageDef = protoLoader.loadSync(PROTO_PATH, {
  keepCase: false,
  longs: Number,
  enums: String,
  defaults: true,
  oneofs: true,
  includeDirs: [resolve(__dirname, '..', '..', '..', 'proto')],
});
const proto = grpc.loadPackageDefinition(packageDef) as any;
const MongoCore = proto.mongocore.v1.MongoCore;

const CONFIG = JSON.parse(readFileSync(join(__dirname, '..', 'common.json'), 'utf-8'));
const DATA_DIR = join(__dirname, '..', '..', 'data');
const RESULTS_DIR = join(__dirname, '..', '..', 'results');
mkdirSync(RESULTS_DIR, { recursive: true });

const WARMUP = CONFIG.warmup_iterations.typescript;
const MIN_TIME = CONFIG.min_time_secs;
const MAX_ITERS = CONFIG.max_iterations;
const MAX_TIME = CONFIG.max_time_secs;
const DB_NAME = CONFIG.database;
const ADDR = CONFIG.mongocore_address;

const BATCH_SIZES = [100, 1000, 10000];
const TOTAL_OPS = 10000;

function newId(): string {
  return crypto.randomUUID().replace(/-/g, '').slice(0, 24);
}

function promisify(client: any, method: string, request: any): Promise<any> {
  return new Promise((resolve, reject) => {
    client[method](request, (err: any, response: any) => {
      if (err) reject(err);
      else resolve(response);
    });
  });
}

const encodeDoc = (doc: any) => Buffer.from(BSON.serialize(doc));

interface BenchResult {
  benchmark: string;
  category: string;
  driver: string;
  dataset_size_bytes: number;
  batch_size: number;
  iterations: number;
  total_time_secs: number;
  ops_per_sec: number;
  mb_per_sec: number;
  percentiles: Record<string, number>;
  timestamp: string;
  system: Record<string, any>;
}

async function runBenchmark(
  name: string,
  category: string,
  client: any,
  setupFn: () => Promise<void>,
  beforeTaskFn: () => Promise<void>,
  taskFn: () => Promise<void>,
  afterTaskFn: () => Promise<void>,
  teardownFn: () => Promise<void>,
  datasetSizeBytes: number,
  batchSize: number,
): Promise<BenchResult> {
  await setupFn();

  for (let i = 0; i < WARMUP; i++) {
    await beforeTaskFn();
    await taskFn();
    await afterTaskFn();
  }

  const times: number[] = [];
  let totalTime = 0;
  let iteration = 0;

  while (totalTime < MIN_TIME || iteration < 5) {
    if (iteration >= MAX_ITERS || totalTime >= MAX_TIME) break;

    await beforeTaskFn();
    const start = performance.now();
    await taskFn();
    const elapsed = (performance.now() - start) / 1000;
    await afterTaskFn();

    times.push(elapsed);
    totalTime += elapsed;
    iteration++;
  }

  await teardownFn();

  times.sort((a, b) => a - b);
  const median = times[Math.floor(times.length / 2)];
  const opsPerSec = batchSize / median;
  const mbPerSec = datasetSizeBytes / median / 1_000_000;

  const pct = (p: number) => times[Math.min(Math.max(0, Math.ceil(times.length * p / 100) - 1), times.length - 1)];

  const result: BenchResult = {
    benchmark: name,
    category,
    driver: 'mongocore+typescript',
    dataset_size_bytes: datasetSizeBytes,
    batch_size: batchSize,
    iterations: times.length,
    total_time_secs: Math.round(totalTime * 1000) / 1000,
    ops_per_sec: Math.round(opsPerSec * 10) / 10,
    mb_per_sec: Math.round(mbPerSec * 1000) / 1000,
    percentiles: {
      p10: Math.round(pct(10) * 1000000) / 1000000,
      p25: Math.round(pct(25) * 1000000) / 1000000,
      p50: Math.round(median * 1000000) / 1000000,
      p75: Math.round(pct(75) * 1000000) / 1000000,
      p90: Math.round(pct(90) * 1000000) / 1000000,
      p95: Math.round(pct(95) * 1000000) / 1000000,
      p99: Math.round(pct(99) * 1000000) / 1000000,
    },
    timestamp: new Date().toISOString(),
    system: { os: os.platform(), arch: os.arch(), cpus: os.cpus().length, driver: 'mongocore+typescript', mongocore_version: '0.6.0' },
  };

  console.log(`  ${name}: ${opsPerSec.toFixed(0)} ops/s, ${mbPerSec.toFixed(2)} MB/s (${times.length} iterations)`);
  return result;
}

async function main() {
  console.log('=== MongoCore+TypeScript Pipeline benchmarks ===');

  const client = new MongoCore(ADDR, grpc.credentials.createInsecure());
  const results: BenchResult[] = [];

  const smallDoc = JSON.parse(readFileSync(join(DATA_DIR, 'small_doc.json'), 'utf-8'));
  const tweetDoc = JSON.parse(readFileSync(join(DATA_DIR, 'tweet.json'), 'utf-8'));
  const smallSize = Buffer.byteLength(JSON.stringify(smallDoc));
  const tweetSize = Buffer.byteLength(JSON.stringify(tweetDoc));

  for (const batchSize of BATCH_SIZES) {
    const callsPerIter = TOTAL_OPS / batchSize;

    // --- pipeline_run_command ---
    results.push(await runBenchmark(
      `pipeline_run_command_${batchSize}`, 'pipeline', client,
      async () => {},
      async () => {},
      async () => {
        for (let c = 0; c < callsPerIter; c++) {
          const operations = Array.from({ length: batchSize }, () => ({
            runCommand: {
              database: DB_NAME,
              command: { data: encodeDoc({ hello: 1 }) },
              allowAll: false,
            },
          }));
          await promisify(client, 'pipeline', { operations });
        }
      },
      async () => {},
      async () => {},
      TOTAL_OPS * 100, TOTAL_OPS,
    ));

    // --- pipeline_insert_one_small ---
    results.push(await runBenchmark(
      `pipeline_insert_one_small_${batchSize}`, 'pipeline', client,
      async () => {},
      async () => {
        await promisify(client, 'runCommand', { database: DB_NAME, command: { data: encodeDoc({ drop: 'bench_pipeline_insert_ts' }) }, allowAll: false }).catch(() => {});
      },
      async () => {
        for (let c = 0; c < callsPerIter; c++) {
          const operations = Array.from({ length: batchSize }, () => ({
            insert: {
              database: DB_NAME,
              collection: 'bench_pipeline_insert_ts',
              document: { data: encodeDoc({ ...smallDoc, _id: newId() }) },
            },
          }));
          await promisify(client, 'pipeline', { operations });
        }
      },
      async () => {},
      async () => {},
      TOTAL_OPS * smallSize, TOTAL_OPS,
    ));

    // --- pipeline_find_one_by_id ---
    results.push(await runBenchmark(
      `pipeline_find_one_by_id_${batchSize}`, 'pipeline', client,
      async () => {
        await promisify(client, 'runCommand', { database: DB_NAME, command: { data: encodeDoc({ drop: 'bench_pipeline_find_ts' }) }, allowAll: false }).catch(() => {});
        await promisify(client, 'insert', {
          database: DB_NAME,
          collection: 'bench_pipeline_find_ts',
          document: { data: encodeDoc({ ...tweetDoc, _id: 'bench_find_001' }) },
        });
      },
      async () => {},
      async () => {
        for (let c = 0; c < callsPerIter; c++) {
          const operations = Array.from({ length: batchSize }, () => ({
            findOne: {
              database: DB_NAME,
              collection: 'bench_pipeline_find_ts',
              filter: { data: encodeDoc({ _id: 'bench_find_001' }) },
            },
          }));
          await promisify(client, 'pipeline', { operations });
        }
      },
      async () => {},
      async () => {
        await promisify(client, 'runCommand', { database: DB_NAME, command: { data: encodeDoc({ drop: 'bench_pipeline_find_ts' }) }, allowAll: false }).catch(() => {});
      },
      TOTAL_OPS * tweetSize, TOTAL_OPS,
    ));
  }

  client.close();

  const outputPath = join(RESULTS_DIR, 'typescript_pipeline.json');
  writeFileSync(outputPath, JSON.stringify(results, null, 2));
  console.log(`\nResults saved to ${outputPath}`);
}

main().catch((err) => { console.error(err); process.exit(1); });
```

- [ ] **Step 2: Verify TypeScript compiles**

Run: `cd benchmarks/drivers/typescript && npx tsc --noEmit bench_pipeline.ts --esModuleInterop --skipLibCheck`
Expected: No errors.

- [ ] **Step 3: Commit**

```bash
git add benchmarks/drivers/typescript/bench_pipeline.ts
git commit -m "feat(bench): add TypeScript pipeline benchmark (3 ops × 3 batch sizes)"
```

---

### Task 4: Go pipeline benchmark

**Files:**
- Create: `benchmarks/drivers/go/bench_pipeline.go`

- [ ] **Step 1: Create `benchmarks/drivers/go/bench_pipeline.go`**

```go
// Benchmark MongoCore Go pipeline batching at different batch sizes.
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"math"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"time"

	"github.com/rozza/mongocore/clients/go/mongocore"
	pb "github.com/rozza/mongocore/clients/go/proto"
	"go.mongodb.org/mongo-driver/v2/bson"
)

type Config struct {
	MongoDBURI    string         `json:"mongodb_uri"`
	MongoCoreAddr string         `json:"mongocore_address"`
	Database      string         `json:"database"`
	MinTimeSecs   int            `json:"min_time_secs"`
	MaxIterations int            `json:"max_iterations"`
	MaxTimeSecs   int            `json:"max_time_secs"`
	WarmupIters   map[string]int `json:"warmup_iterations"`
}

type SystemInfo struct {
	OS           string `json:"os"`
	Arch         string `json:"arch"`
	CPUs         int    `json:"cpus"`
	MongoCoreVer string `json:"mongocore_version"`
	Driver       string `json:"driver"`
}

type Percentiles struct {
	P10 float64 `json:"p10"`
	P25 float64 `json:"p25"`
	P50 float64 `json:"p50"`
	P75 float64 `json:"p75"`
	P90 float64 `json:"p90"`
	P95 float64 `json:"p95"`
	P99 float64 `json:"p99"`
}

type BenchResult struct {
	Benchmark     string      `json:"benchmark"`
	Category      string      `json:"category"`
	Driver        string      `json:"driver"`
	DatasetBytes  int         `json:"dataset_size_bytes"`
	BatchSize     int         `json:"batch_size"`
	Iterations    int         `json:"iterations"`
	TotalTimeSecs float64     `json:"total_time_secs"`
	OpsPerSec     float64     `json:"ops_per_sec"`
	MBPerSec      float64     `json:"mb_per_sec"`
	Percentiles   Percentiles `json:"percentiles"`
	Timestamp     string      `json:"timestamp"`
	System        SystemInfo  `json:"system"`
}

var batchSizes = []int{100, 1000, 10000}
const totalOps = 10000

func getSystemInfo() SystemInfo {
	return SystemInfo{
		OS:           runtime.GOOS,
		Arch:         runtime.GOARCH,
		CPUs:         runtime.NumCPU(),
		MongoCoreVer: "0.6.0",
		Driver:       "mongocore+go",
	}
}

func percentile(data []float64, pct int) float64 {
	idx := int(math.Ceil(float64(len(data))*float64(pct)/100.0)) - 1
	if idx < 0 {
		idx = 0
	}
	if idx >= len(data) {
		idx = len(data) - 1
	}
	return data[idx]
}

func runBenchmark(
	name, category string,
	setupFn func() error,
	beforeTaskFn func() error,
	taskFn func() error,
	afterTaskFn func() error,
	teardownFn func() error,
	datasetBytes, batchSize int,
	config Config,
) BenchResult {
	if err := setupFn(); err != nil {
		panic(err)
	}

	warmup := config.WarmupIters["go"]
	for i := 0; i < warmup; i++ {
		_ = beforeTaskFn()
		_ = taskFn()
		_ = afterTaskFn()
	}

	times := []float64{}
	totalTime := 0.0
	iteration := 0

	for totalTime < float64(config.MinTimeSecs) || iteration < 5 {
		if iteration >= config.MaxIterations || totalTime >= float64(config.MaxTimeSecs) {
			break
		}

		_ = beforeTaskFn()
		start := time.Now()
		if err := taskFn(); err != nil {
			panic(err)
		}
		elapsed := time.Since(start).Seconds()
		_ = afterTaskFn()

		times = append(times, elapsed)
		totalTime += elapsed
		iteration++
	}

	_ = teardownFn()

	sort.Float64s(times)
	median := times[len(times)/2]
	opsPerSec := float64(batchSize) / median
	mbPerSec := float64(datasetBytes) / median / 1_000_000

	result := BenchResult{
		Benchmark:     name,
		Category:      category,
		Driver:        "mongocore+go",
		DatasetBytes:  datasetBytes,
		BatchSize:     batchSize,
		Iterations:    len(times),
		TotalTimeSecs: math.Round(totalTime*1000) / 1000,
		OpsPerSec:     math.Round(opsPerSec*10) / 10,
		MBPerSec:      math.Round(mbPerSec*1000) / 1000,
		Percentiles: Percentiles{
			P10: math.Round(percentile(times, 10)*1_000_000) / 1_000_000,
			P25: math.Round(percentile(times, 25)*1_000_000) / 1_000_000,
			P50: math.Round(median*1_000_000) / 1_000_000,
			P75: math.Round(percentile(times, 75)*1_000_000) / 1_000_000,
			P90: math.Round(percentile(times, 90)*1_000_000) / 1_000_000,
			P95: math.Round(percentile(times, 95)*1_000_000) / 1_000_000,
			P99: math.Round(percentile(times, 99)*1_000_000) / 1_000_000,
		},
		Timestamp: time.Now().UTC().Format(time.RFC3339),
		System:    getSystemInfo(),
	}

	fmt.Printf("  %s: %.0f ops/s, %.2f MB/s (%d iterations)\n",
		name, opsPerSec, mbPerSec, len(times))
	return result
}

func encodeBson(doc interface{}) []byte {
	data, err := bson.Marshal(doc)
	if err != nil {
		panic(err)
	}
	return data
}

func main() {
	fmt.Println("=== MongoCore+Go Pipeline benchmarks ===")

	configPath := filepath.Join("..", "common.json")
	configData, err := os.ReadFile(configPath)
	if err != nil {
		panic(err)
	}
	var config Config
	json.Unmarshal(configData, &config)

	dataDir := filepath.Join("..", "..", "data")
	smallDocData, _ := os.ReadFile(filepath.Join(dataDir, "small_doc.json"))
	tweetDocData, _ := os.ReadFile(filepath.Join(dataDir, "tweet.json"))

	var smallDoc map[string]interface{}
	var tweetDoc map[string]interface{}
	json.Unmarshal(smallDocData, &smallDoc)
	json.Unmarshal(tweetDocData, &tweetDoc)

	smallSize := len(smallDocData)
	tweetSize := len(tweetDocData)

	client := mongocore.MongoClientTCP(config.MongoCoreAddr)
	ctx := context.Background()
	if err := client.Connect(ctx); err != nil {
		panic(err)
	}
	defer client.Close()

	results := []BenchResult{}

	for _, bs := range batchSizes {
		callsPerIter := totalOps / bs

		// --- pipeline_run_command ---
		results = append(results, runBenchmark(
			fmt.Sprintf("pipeline_run_command_%d", bs), "pipeline",
			func() error { return nil },
			func() error { return nil },
			func() error {
				for c := 0; c < callsPerIter; c++ {
					ops := make([]*pb.PipelineOperation, bs)
					for i := range ops {
						ops[i] = &pb.PipelineOperation{
							Operation: &pb.PipelineOperation_RunCommand{
								RunCommand: &pb.RunCommandRequest{
									Database: config.Database,
									Command:  &pb.Document{Data: encodeBson(bson.D{{"hello", 1}})},
								},
							},
						}
					}
					_, err := client.Pipeline(ctx, ops...)
					if err != nil {
						return err
					}
				}
				return nil
			},
			func() error { return nil },
			func() error { return nil },
			totalOps*100, totalOps, config,
		))

		// --- pipeline_insert_one_small ---
		collInsert := "bench_pipeline_insert_go"
		results = append(results, runBenchmark(
			fmt.Sprintf("pipeline_insert_one_small_%d", bs), "pipeline",
			func() error { return nil },
			func() error {
				client.RunCommand(ctx, config.Database, bson.D{{"drop", collInsert}}, false)
				return nil
			},
			func() error {
				for c := 0; c < callsPerIter; c++ {
					ops := make([]*pb.PipelineOperation, bs)
					for i := range ops {
						doc := bson.D{{"_id", bson.NewObjectID().Hex()}}
						for k, v := range smallDoc {
							doc = append(doc, bson.E{Key: k, Value: v})
						}
						ops[i] = &pb.PipelineOperation{
							Operation: &pb.PipelineOperation_Insert{
								Insert: &pb.InsertRequest{
									Database:   config.Database,
									Collection: collInsert,
									Document:   &pb.Document{Data: encodeBson(doc)},
								},
							},
						}
					}
					_, err := client.Pipeline(ctx, ops...)
					if err != nil {
						return err
					}
				}
				return nil
			},
			func() error { return nil },
			func() error { return nil },
			totalOps*smallSize, totalOps, config,
		))

		// --- pipeline_find_one_by_id ---
		collFind := "bench_pipeline_find_go"
		results = append(results, runBenchmark(
			fmt.Sprintf("pipeline_find_one_by_id_%d", bs), "pipeline",
			func() error {
				client.RunCommand(ctx, config.Database, bson.D{{"drop", collFind}}, false)
				doc := bson.D{{"_id", "bench_find_001"}}
				for k, v := range tweetDoc {
					doc = append(doc, bson.E{Key: k, Value: v})
				}
				coll := client.Database(config.Database).Collection(collFind)
				_, err := coll.InsertOne(ctx, doc)
				return err
			},
			func() error { return nil },
			func() error {
				for c := 0; c < callsPerIter; c++ {
					ops := make([]*pb.PipelineOperation, bs)
					for i := range ops {
						ops[i] = &pb.PipelineOperation{
							Operation: &pb.PipelineOperation_FindOne{
								FindOne: &pb.FindOneRequest{
									Database:   config.Database,
									Collection: collFind,
									Filter:     &pb.Filter{Data: encodeBson(bson.D{{"_id", "bench_find_001"}})},
								},
							},
						}
					}
					_, err := client.Pipeline(ctx, ops...)
					if err != nil {
						return err
					}
				}
				return nil
			},
			func() error { return nil },
			func() error {
				client.RunCommand(ctx, config.Database, bson.D{{"drop", collFind}}, false)
				return nil
			},
			totalOps*tweetSize, totalOps, config,
		))
	}

	resultsDir := filepath.Join("..", "..", "results")
	os.MkdirAll(resultsDir, 0755)
	outputPath := filepath.Join(resultsDir, "go_pipeline.json")
	data, _ := json.MarshalIndent(results, "", "  ")
	os.WriteFile(outputPath, data, 0644)
	fmt.Printf("\nResults saved to %s\n", outputPath)
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd benchmarks/drivers/go && go build bench_pipeline.go`
Expected: Produces `bench_pipeline` binary without errors.

- [ ] **Step 3: Clean up binary and commit**

```bash
rm -f benchmarks/drivers/go/bench_pipeline
git add benchmarks/drivers/go/bench_pipeline.go
git commit -m "feat(bench): add Go pipeline benchmark (3 ops × 3 batch sizes)"
```

---

### Task 5: Java pipeline benchmark

**Files:**
- Create: `benchmarks/drivers/java/src/main/java/com/mongocore/bench/BenchPipeline.java`

- [ ] **Step 1: Create `benchmarks/drivers/java/src/main/java/com/mongocore/bench/BenchPipeline.java`**

```java
package com.mongocore.bench;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.mongocore.MongoClient;
import com.mongocore.Ops;
import com.mongocore.proto.Mongocore;
import org.bson.Document;
import org.bson.types.ObjectId;

import java.io.FileReader;
import java.io.FileWriter;
import java.lang.management.ManagementFactory;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.time.Instant;
import java.util.*;
import java.util.stream.Collectors;

public class BenchPipeline {
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();
    private static final int[] BATCH_SIZES = {100, 1000, 10000};
    private static final int TOTAL_OPS = 10000;

    static class Config {
        String mongodb_uri;
        String mongocore_address;
        String database;
        int min_time_secs;
        int max_iterations;
        int max_time_secs;
        Map<String, Integer> warmup_iterations;
    }

    static class SystemInfo {
        String os;
        String arch;
        int cpus;
        double ram_gb;
        String mongocore_version;
        String driver;

        SystemInfo() {
            this.os = System.getProperty("os.name").toLowerCase();
            this.arch = System.getProperty("os.arch");
            this.cpus = Runtime.getRuntime().availableProcessors();
            long memory = ((com.sun.management.OperatingSystemMXBean) ManagementFactory.getOperatingSystemMXBean()).getTotalMemorySize();
            this.ram_gb = Math.round(memory / (1024.0 * 1024.0 * 1024.0) * 10) / 10.0;
            this.mongocore_version = "0.6.0";
            this.driver = "mongocore+java";
        }
    }

    static class Percentiles {
        double p10, p25, p50, p75, p90, p95, p99;
    }

    static class BenchResult {
        String benchmark;
        String category;
        String driver;
        int dataset_size_bytes;
        int batch_size;
        int iterations;
        double total_time_secs;
        double ops_per_sec;
        double mb_per_sec;
        Percentiles percentiles;
        String timestamp;
        SystemInfo system;
    }

    interface TaskFn {
        void run(MongoClient client) throws Exception;
    }

    private static double percentile(List<Double> data, int pct) {
        int idx = (int) Math.ceil(data.size() * pct / 100.0) - 1;
        if (idx < 0) idx = 0;
        if (idx >= data.size()) idx = data.size() - 1;
        return data.get(idx);
    }

    private static BenchResult runBenchmark(
            String name,
            String category,
            MongoClient client,
            TaskFn setupFn,
            TaskFn beforeTaskFn,
            TaskFn taskFn,
            TaskFn afterTaskFn,
            TaskFn teardownFn,
            int datasetSizeBytes,
            int batchSize,
            Config config
    ) throws Exception {
        setupFn.run(client);

        int warmup = config.warmup_iterations.get("java");
        for (int i = 0; i < warmup; i++) {
            beforeTaskFn.run(client);
            taskFn.run(client);
            afterTaskFn.run(client);
        }

        List<Double> times = new ArrayList<>();
        double totalTime = 0.0;
        int iteration = 0;

        while (totalTime < config.min_time_secs || iteration < 5) {
            if (iteration >= config.max_iterations || totalTime >= config.max_time_secs) {
                break;
            }

            beforeTaskFn.run(client);
            long start = System.nanoTime();
            taskFn.run(client);
            double elapsed = (System.nanoTime() - start) / 1_000_000_000.0;
            afterTaskFn.run(client);

            times.add(elapsed);
            totalTime += elapsed;
            iteration++;
        }

        teardownFn.run(client);

        Collections.sort(times);
        double median = times.get(times.size() / 2);
        double opsPerSec = batchSize / median;
        double mbPerSec = datasetSizeBytes / median / 1_000_000.0;

        BenchResult result = new BenchResult();
        result.benchmark = name;
        result.category = category;
        result.driver = "mongocore+java";
        result.dataset_size_bytes = datasetSizeBytes;
        result.batch_size = batchSize;
        result.iterations = times.size();
        result.total_time_secs = Math.round(totalTime * 1000) / 1000.0;
        result.ops_per_sec = Math.round(opsPerSec * 10) / 10.0;
        result.mb_per_sec = Math.round(mbPerSec * 1000) / 1000.0;

        Percentiles pct = new Percentiles();
        pct.p10 = Math.round(percentile(times, 10) * 1_000_000) / 1_000_000.0;
        pct.p25 = Math.round(percentile(times, 25) * 1_000_000) / 1_000_000.0;
        pct.p50 = Math.round(median * 1_000_000) / 1_000_000.0;
        pct.p75 = Math.round(percentile(times, 75) * 1_000_000) / 1_000_000.0;
        pct.p90 = Math.round(percentile(times, 90) * 1_000_000) / 1_000_000.0;
        pct.p95 = Math.round(percentile(times, 95) * 1_000_000) / 1_000_000.0;
        pct.p99 = Math.round(percentile(times, 99) * 1_000_000) / 1_000_000.0;
        result.percentiles = pct;

        result.timestamp = Instant.now().toString();
        result.system = new SystemInfo();

        System.out.printf("  %s: %.0f ops/s, %.2f MB/s (%d iterations)%n",
                name, opsPerSec, mbPerSec, times.size());
        return result;
    }

    public static void main(String[] args) throws Exception {
        System.out.println("=== MongoCore+Java Pipeline benchmarks ===");

        Path configPath = Paths.get("..", "common.json");
        Config config = GSON.fromJson(new FileReader(configPath.toFile()), Config.class);

        Path dataDir = Paths.get("..", "..", "data");
        String smallDocJson = Files.readString(dataDir.resolve("small_doc.json"));
        String tweetDocJson = Files.readString(dataDir.resolve("tweet.json"));

        Document smallDoc = Document.parse(smallDocJson);
        Document tweetDoc = Document.parse(tweetDocJson);

        int smallSize = smallDocJson.getBytes().length;
        int tweetSize = tweetDocJson.getBytes().length;

        MongoClient client = MongoClient.create(config.mongocore_address);
        List<BenchResult> results = new ArrayList<>();

        for (int bs : BATCH_SIZES) {
            int callsPerIter = TOTAL_OPS / bs;

            // --- pipeline_run_command ---
            final int fbs = bs;
            final int fcalls = callsPerIter;
            results.add(runBenchmark(
                    "pipeline_run_command_" + bs, "pipeline", client,
                    c -> {},
                    c -> {},
                    c -> {
                        for (int call = 0; call < fcalls; call++) {
                            Mongocore.PipelineOperation[] ops = new Mongocore.PipelineOperation[fbs];
                            for (int i = 0; i < fbs; i++) {
                                ops[i] = Ops.runCommand(config.database, new Document("hello", 1));
                            }
                            c.pipeline(ops);
                        }
                    },
                    c -> {},
                    c -> {},
                    TOTAL_OPS * 100, TOTAL_OPS, config
            ));

            // --- pipeline_insert_one_small ---
            String collInsert = "bench_pipeline_insert_java";
            results.add(runBenchmark(
                    "pipeline_insert_one_small_" + bs, "pipeline", client,
                    c -> {},
                    c -> {
                        try { c.runCommand(config.database, new Document("drop", collInsert), false); } catch (Exception ignored) {}
                    },
                    c -> {
                        for (int call = 0; call < fcalls; call++) {
                            Mongocore.PipelineOperation[] ops = new Mongocore.PipelineOperation[fbs];
                            for (int i = 0; i < fbs; i++) {
                                Document doc = new Document(smallDoc);
                                doc.put("_id", new ObjectId().toHexString());
                                ops[i] = Ops.insert(config.database, collInsert, doc);
                            }
                            c.pipeline(ops);
                        }
                    },
                    c -> {},
                    c -> {},
                    TOTAL_OPS * smallSize, TOTAL_OPS, config
            ));

            // --- pipeline_find_one_by_id ---
            String collFind = "bench_pipeline_find_java";
            results.add(runBenchmark(
                    "pipeline_find_one_by_id_" + bs, "pipeline", client,
                    c -> {
                        try { c.runCommand(config.database, new Document("drop", collFind), false); } catch (Exception ignored) {}
                        Document doc = new Document(tweetDoc);
                        doc.put("_id", "bench_find_001");
                        c.getDatabase(config.database).getCollection(collFind).insertOne(doc);
                    },
                    c -> {},
                    c -> {
                        for (int call = 0; call < fcalls; call++) {
                            Mongocore.PipelineOperation[] ops = new Mongocore.PipelineOperation[fbs];
                            for (int i = 0; i < fbs; i++) {
                                ops[i] = Ops.findOne(config.database, collFind, new Document("_id", "bench_find_001"));
                            }
                            c.pipeline(ops);
                        }
                    },
                    c -> {},
                    c -> {
                        c.runCommand(config.database, new Document("drop", collFind), false);
                    },
                    TOTAL_OPS * tweetSize, TOTAL_OPS, config
            ));
        }

        client.close();

        Path resultsDir = Paths.get("..", "..", "results");
        Files.createDirectories(resultsDir);
        Path outputPath = resultsDir.resolve("java_pipeline.json");
        try (FileWriter writer = new FileWriter(outputPath.toFile())) {
            GSON.toJson(results, writer);
        }
        System.out.printf("%nResults saved to %s%n", outputPath);
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd benchmarks/drivers/java && mvn -q compile`
Expected: BUILD SUCCESS

- [ ] **Step 3: Commit**

```bash
git add benchmarks/drivers/java/src/main/java/com/mongocore/bench/BenchPipeline.java
git commit -m "feat(bench): add Java pipeline benchmark (3 ops × 3 batch sizes)"
```

---

### Task 6: Update justfile with pipeline recipes

**Files:**
- Modify: `benchmarks/justfile`

- [ ] **Step 1: Add private pipeline recipes**

After the `_bench-compiled` recipe (line 349), add:

```just
[private]
_bench-python-pipeline:
    cd drivers/python && pip install -r requirements.txt -q && python bench_pipeline.py

[private]
_bench-typescript-pipeline:
    cd drivers/typescript && npm install --silent && npx ts-node bench_pipeline.ts

[private]
_bench-go-pipeline:
    cd drivers/go && go run bench_pipeline.go

[private]
_bench-java-pipeline:
    cd drivers/java && mvn -q compile exec:java -Dexec.mainClass="com.mongocore.bench.BenchPipeline"
```

- [ ] **Step 2: Add public pipeline recipes**

After `bench-java-mongocore` (line 277) and before `# Polars ingestion benchmarks`, add:

```just
# Pipeline batching benchmarks (all languages)
bench-drivers-pipeline: require-setup
    #!/usr/bin/env bash
    set -e
    cd {{justfile_directory()}}
    just _bench-python-pipeline
    just _bench-typescript-pipeline
    just _bench-go-pipeline
    just _bench-java-pipeline

# Python: pipeline batching
bench-python-pipeline: require-setup
    #!/usr/bin/env bash
    set -e
    cd {{justfile_directory()}}
    just _bench-python-pipeline

# TypeScript: pipeline batching
bench-typescript-pipeline: require-setup
    #!/usr/bin/env bash
    set -e
    cd {{justfile_directory()}}
    just _bench-typescript-pipeline

# Go: pipeline batching
bench-go-pipeline: require-setup
    #!/usr/bin/env bash
    set -e
    cd {{justfile_directory()}}
    just _bench-go-pipeline

# Java: pipeline batching
bench-java-pipeline: require-setup
    #!/usr/bin/env bash
    set -e
    cd {{justfile_directory()}}
    just _bench-java-pipeline
```

- [ ] **Step 3: Update `bench-all` to include pipeline benchmarks**

In the `bench-all` recipe, after ALL language benchmarks but BEFORE the ingestion benchmarks section (before the line `echo ">>> Running ingestion benchmarks..."`), add:

```bash
    echo ""
    echo ">>> Running Pipeline benchmarks..."
    cd drivers/python && python bench_pipeline.py && cd ../..
    cd drivers/typescript && npx ts-node bench_pipeline.ts && cd ../..
    cd drivers/go && go run bench_pipeline.go && cd ../..
    cd drivers/java && mvn -q compile exec:java -Dexec.mainClass="com.mongocore.bench.BenchPipeline" && cd ../..
```

- [ ] **Step 4: Update `bench-drivers` to include pipeline**

In the `bench-drivers` recipe (around line 169), add after `just _bench-java-mongocore`:

```just
    just _bench-python-pipeline
    just _bench-typescript-pipeline
    just _bench-go-pipeline
    just _bench-java-pipeline
```

- [ ] **Step 4b: Update per-language aggregate recipes to include pipeline**

Update `bench-python` to also run pipeline:
```just
# Python: both native and MongoCore
bench-python: require-setup
    #!/usr/bin/env bash
    set -e
    cd {{justfile_directory()}}
    just _bench-python-native
    just _bench-python-mongocore
    just _bench-python-pipeline
```

Apply the same pattern to `bench-typescript`, `bench-go`, and `bench-java` — each should add `just _bench-{lang}-pipeline` as the last line.

- [ ] **Step 5: Verify justfile syntax**

Run: `cd benchmarks && just --list | grep pipeline`
Expected: Shows `bench-drivers-pipeline`, `bench-python-pipeline`, `bench-typescript-pipeline`, `bench-go-pipeline`, `bench-java-pipeline`.

- [ ] **Step 6: Commit**

```bash
git add benchmarks/justfile
git commit -m "chore(bench): add pipeline benchmark recipes to justfile"
```

---

### Task 7: Update results template

**Files:**
- Modify: `benchmarks/collector/templates/results.md.j2`

- [ ] **Step 1: Add Pipeline Batching section to template**

Insert AFTER the ingestion section (at the very end of the template, before the final closing). The template order should be: per-language tables → overhead charts → throughput chart → ingestion → **pipeline batching** → **compiled query**. Add:

```jinja2
{% if pipeline_data %}

## Pipeline Batching

Shows throughput gain from batching operations into single Pipeline RPC calls at different batch sizes.
{% for lang in pipeline_data %}

### {{ lang.name }}

| Operation | Native (ops/s) | MC Individual (ops/s) | Pipeline×100 | Pipeline×1K | Pipeline×10K | Best Speedup vs Native |
|-----------|---------------|----------------------|--------------|-------------|--------------|----------------------|
{% for row in lang.rows -%}
| {{ row.operation }} | {{ row.native_ops }} | {{ row.mc_individual_ops }} | {{ row.p100 }} | {{ row.p1000 }} | {{ row.p10000 }} | {{ row.best_speedup }} |
{% endfor %}
{% if lang.chart_path %}

![Pipeline Scaling — {{ lang.name }}](./{{ lang.chart_path }})
{% endif %}
{% endfor %}
{% endif %}
{% if compiled_data %}

## Compiled Query Cache

| Benchmark | ops/s | p50 | p99 |
|-----------|-------|-----|-----|
{% for row in compiled_data -%}
| {{ row.benchmark }} | {{ row.ops_per_sec }} | {{ row.p50 }} | {{ row.p99 }} |
{% endfor %}
{% endif %}
```

- [ ] **Step 2: Commit**

```bash
git add benchmarks/collector/templates/results.md.j2
git commit -m "docs(bench): add pipeline batching and compiled query sections to results template"
```

---

### Task 8: Update collector to build pipeline and compiled query context

**Files:**
- Modify: `benchmarks/collector/generate_readme.py`

- [ ] **Step 1: Add pipeline context builder function**

After the `ALL_BENCHMARKS` list (line 247), add:

```python
PIPELINE_OPERATIONS = ["run_command", "insert_one_small", "find_one_by_id"]
PIPELINE_BATCH_SIZES = [100, 1000, 10000]


def build_pipeline_context(results, languages_data):
    """Build context for pipeline batching section."""
    pipeline_results = [r for r in results if r.get("category") == "pipeline"]
    if not pipeline_results:
        return []

    # Group pipeline results by language
    pipeline_by_lang = {}
    for r in pipeline_results:
        lang = get_language(r.get("driver", ""))
        if lang == "unknown":
            continue
        if lang not in pipeline_by_lang:
            pipeline_by_lang[lang] = {}
        pipeline_by_lang[lang][r["benchmark"]] = r

    lang_sections = []
    for lang in sorted(pipeline_by_lang.keys()):
        pl = pipeline_by_lang[lang]
        native = languages_data.get(lang, {}).get("native", {})
        mc = languages_data.get(lang, {}).get("mongocore", {})

        rows = []
        for op in PIPELINE_OPERATIONS:
            native_result = native.get(op)
            mc_result = mc.get(op)
            native_ops = f"{native_result['ops_per_sec']:,.0f}" if native_result else "—"
            mc_individual_ops = f"{mc_result['ops_per_sec']:,.0f}" if mc_result else "—"

            pipeline_ops = {}
            for bs in PIPELINE_BATCH_SIZES:
                key = f"pipeline_{op}_{bs}"
                pr = pl.get(key)
                pipeline_ops[bs] = f"{pr['ops_per_sec']:,.0f}" if pr else "—"

            # Best speedup vs native
            best_speedup = "—"
            if native_result and native_result["ops_per_sec"] > 0:
                best_pipeline_ops = max(
                    (pl.get(f"pipeline_{op}_{bs}", {}).get("ops_per_sec", 0) for bs in PIPELINE_BATCH_SIZES),
                    default=0,
                )
                if best_pipeline_ops > 0:
                    speedup = best_pipeline_ops / native_result["ops_per_sec"]
                    best_speedup = f"{speedup:.1f}x"

            rows.append({
                "operation": op,
                "native_ops": native_ops,
                "mc_individual_ops": mc_individual_ops,
                "p100": pipeline_ops[100],
                "p1000": pipeline_ops[1000],
                "p10000": pipeline_ops[10000],
                "best_speedup": best_speedup,
            })

        lang_sections.append({"name": lang, "rows": rows})

    return lang_sections


def build_compiled_context(results):
    """Build context for compiled query cache section."""
    compiled_results = [r for r in results if r.get("category") == "compiled_query"]
    if not compiled_results:
        return []

    rows = []
    for r in compiled_results:
        pct = r.get("percentiles", {})
        p50_val = pct.get("p50", 0)
        p99_val = pct.get("p99", 0)

        def fmt_latency(v):
            if v == 0:
                return "—"
            us = v * 1_000_000
            if us < 1000:
                return f"{us:.0f}us"
            return f"{us / 1000:.2f}ms"

        rows.append({
            "benchmark": r["benchmark"],
            "ops_per_sec": f"{r['ops_per_sec']:,.0f}",
            "p50": fmt_latency(p50_val),
            "p99": fmt_latency(p99_val),
        })

    return rows
```

- [ ] **Step 2: Add pipeline chart generation function**

After `generate_overhead_summary_chart` function, add:

```python
def generate_pipeline_charts(pipeline_data, languages_data, charts_dir, readme_path):
    """Generate one line chart per language showing ops/s vs batch size for pipeline operations."""
    if not pipeline_data:
        return {}

    chart_paths = {}
    batch_sizes = [100, 1000, 10000]
    colors = {"run_command": "#306998", "insert_one_small": "#ED8B00", "find_one_by_id": "#00ADD8"}

    for lang_data in pipeline_data:
        lang_name = lang_data["name"]
        fig, ax = plt.subplots(figsize=(12, 7))

        for row in lang_data["rows"]:
            op = row["operation"]
            ops_values = []
            for bs in batch_sizes:
                val_str = row[f"p{bs}"]
                if val_str == "—":
                    ops_values.append(0)
                else:
                    ops_values.append(float(val_str.replace(",", "")))

            if any(v > 0 for v in ops_values):
                ax.plot(batch_sizes, ops_values, 'o-', label=f'{op}',
                        color=colors.get(op, '#666666'), linewidth=2, markersize=8)

            # Native reference line
            native_str = row["native_ops"]
            if native_str != "—":
                native_val = float(native_str.replace(",", ""))
                ax.axhline(y=native_val, linestyle='--', alpha=0.4,
                           color=colors.get(op, '#666666'),
                           label=f'{op} (native baseline)' if op == lang_data["rows"][0]["operation"] else None)

            # MC individual reference line
            mc_str = row["mc_individual_ops"]
            if mc_str != "—":
                mc_val = float(mc_str.replace(",", ""))
                ax.axhline(y=mc_val, linestyle=':', alpha=0.4,
                           color=colors.get(op, '#666666'))

        ax.set_xscale('log')
        ax.set_xlabel('Batch Size (ops per pipeline call)', fontsize=11)
        ax.set_ylabel('Operations/sec', fontsize=11)
        ax.set_title(f'Pipeline Batching Scaling — {lang_name}', fontsize=13, fontweight='bold')
        ax.set_xticks(batch_sizes)
        ax.set_xticklabels(['100', '1K', '10K'])
        ax.legend(loc='upper left', fontsize=10)
        ax.grid(axis='y', alpha=0.3)

        plt.tight_layout()
        chart_filename = f"pipeline_scaling_{lang_name.lower()}.svg"
        chart_path = charts_dir / chart_filename
        plt.savefig(chart_path, format='svg', bbox_inches='tight')
        plt.close()
        chart_paths[lang_name] = str(chart_path.relative_to(readme_path.parent))

    return chart_paths
```

- [ ] **Step 3: Update `build_context` to include pipeline and compiled data**

In `build_context()`, before the `return ctx` statement, add:

```python
    # Pipeline batching
    pipeline_data = build_pipeline_context(results, languages_data)
    ctx["pipeline_data"] = pipeline_data
    pipeline_chart_paths = generate_pipeline_charts(pipeline_data, languages_data, charts_dir, readme_path)
    # Attach chart path to each language's data
    for lang_section in pipeline_data:
        lang_section["chart_path"] = pipeline_chart_paths.get(lang_section["name"])

    # Compiled query
    ctx["compiled_data"] = build_compiled_context(results)
```

Also update the initial `ctx` dict at the top of `build_context` to include the new keys:

```python
    ctx = {"environment": None, "languages": [], "overhead_chart_path": None, "overhead_summary_chart_path": None, "ingestion": None, "pipeline_data": [], "compiled_data": []}
```

- [ ] **Step 4: Verify Python syntax**

Run: `cd benchmarks/collector && python -c "import ast; ast.parse(open('generate_readme.py').read()); print('OK')"`
Expected: `OK`

- [ ] **Step 5: Commit**

```bash
git add benchmarks/collector/generate_readme.py
git commit -m "feat(bench): add pipeline and compiled query context to results collector"
```

---

### Task 9: Update benchmarks README

**Files:**
- Modify: `benchmarks/README.md`

- [ ] **Step 1: Update "What's Measured" section**

After the "Multi-doc ops" bullet, add:
```markdown
- **Pipeline batching:** pipeline_run_command, pipeline_insert_one_small, pipeline_find_one_by_id (at 100/1K/10K batch sizes)
- **Compiled query:** cache hit latency
```

- [ ] **Step 2: Add pipeline commands to the Tasks table**

After the Java rows and before "Other", add:

```markdown
| **Pipeline** | |
| `just bench-drivers-pipeline` | Pipeline batching benchmarks (all languages) |
| `just bench-python-pipeline` | Python pipeline batching |
| `just bench-typescript-pipeline` | TypeScript pipeline batching |
| `just bench-go-pipeline` | Go pipeline batching |
| `just bench-java-pipeline` | Java pipeline batching |
```

- [ ] **Step 3: Commit**

```bash
git add benchmarks/README.md
git commit -m "docs(bench): add pipeline benchmarks to README"
```

---

### Task 10: Final verification

- [ ] **Step 1: Verify root project builds clean**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output.

- [ ] **Step 2: Verify unit tests pass**

Run: `cargo test --lib`
Expected: All tests pass.

- [ ] **Step 3: Verify justfile lists all new recipes**

Run: `cd benchmarks && just --list | grep -E "pipeline|compiled"`
Expected: Shows all pipeline recipes.

- [ ] **Step 4: Verify `benches/` directory is gone**

Run: `ls benches/ 2>&1`
Expected: `No such file or directory`

- [ ] **Step 5: Verify Rust pipeline bench compiles in new location**

Run: `cd benchmarks/rust && cargo bench --bench pipeline --no-run`
Expected: Compiles successfully.
