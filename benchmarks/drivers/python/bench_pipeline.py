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
