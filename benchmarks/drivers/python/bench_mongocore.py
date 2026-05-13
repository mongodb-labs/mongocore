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


async def run_benchmark(name, category, setup_fn, before_task_fn, task_fn, after_task_fn, teardown_fn, dataset_size_bytes, batch_size=1):
    """Run a benchmark following MongoDB spec methodology."""
    client = MongoClient(ADDR)
    await client.connect()

    await setup_fn(client)

    # Warmup
    for _ in range(WARMUP):
        await before_task_fn(client)
        await task_fn(client)
        await after_task_fn(client)

    # Timed iterations
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
    await client.close()

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
    print("=== MongoCore+Python benchmarks ===")
    results = []

    small_doc = json.loads((DATA_DIR / "small_doc.json").read_text())
    tweet_doc = json.loads((DATA_DIR / "tweet.json").read_text())
    large_doc = json.loads((DATA_DIR / "large_doc.json").read_text())
    small_size = len(json.dumps(small_doc).encode())
    tweet_size = len(json.dumps(tweet_doc).encode())
    large_size = len(json.dumps(large_doc).encode())

    # Run Command (batch 10,000 hello commands per iteration)
    async def task_run_command(c):
        for _ in range(10_000):
            await c.run_command(DB_NAME, {"hello": 1})

    results.append(await run_benchmark(
        "run_command", "single_doc",
        setup_fn=lambda c: asyncio.sleep(0),
        before_task_fn=lambda c: asyncio.sleep(0),
        task_fn=task_run_command,
        after_task_fn=lambda c: asyncio.sleep(0),
        teardown_fn=lambda c: asyncio.sleep(0),
        dataset_size_bytes=10_000 * 100, batch_size=10_000,
    ))

    # Find One by ID (batch 10,000 finds per iteration)
    async def setup_find(c):
        try:
            await c.run_command(DB_NAME, {"drop": "bench_find_mc"})
        except:
            pass
        coll = c[DB_NAME]["bench_find_mc"]
        await coll.insert_one({"_id": "bench_find_001", **tweet_doc})

    async def task_find(c):
        for _ in range(10_000):
            await c[DB_NAME]["bench_find_mc"].find_one({"_id": "bench_find_001"})

    async def teardown_find(c):
        try:
            await c.run_command(DB_NAME, {"drop": "bench_find_mc"})
        except:
            pass

    results.append(await run_benchmark(
        "find_one_by_id", "single_doc",
        setup_fn=setup_find,
        before_task_fn=lambda c: asyncio.sleep(0),
        task_fn=task_find,
        after_task_fn=lambda c: asyncio.sleep(0),
        teardown_fn=teardown_find,
        dataset_size_bytes=10_000 * tweet_size, batch_size=10_000,
    ))

    # InsertOne Small (batch 10,000 inserts per iteration)
    async def task_insert_small(c):
        from bson import ObjectId
        for _ in range(10_000):
            await c[DB_NAME]["bench_insert_small_mc"].insert_one({**small_doc, "_id": str(ObjectId())})

    async def before_insert_small(c):
        try:
            await c.run_command(DB_NAME, {"drop": "bench_insert_small_mc"})
        except:
            pass

    results.append(await run_benchmark(
        "insert_one_small", "single_doc",
        setup_fn=lambda c: asyncio.sleep(0),
        before_task_fn=before_insert_small,
        task_fn=task_insert_small,
        after_task_fn=lambda c: asyncio.sleep(0),
        teardown_fn=lambda c: asyncio.sleep(0),
        dataset_size_bytes=10_000 * small_size, batch_size=10_000,
    ))

    # InsertOne Large (batch 10 inserts per iteration, large docs ~2.75MB each)
    async def task_insert_large(c):
        from bson import ObjectId
        for _ in range(10):
            await c[DB_NAME]["bench_insert_large_mc"].insert_one({**large_doc, "_id": str(ObjectId())})

    async def before_insert_large(c):
        try:
            await c.run_command(DB_NAME, {"drop": "bench_insert_large_mc"})
        except:
            pass

    results.append(await run_benchmark(
        "insert_one_large", "single_doc",
        setup_fn=lambda c: asyncio.sleep(0),
        before_task_fn=before_insert_large,
        task_fn=task_insert_large,
        after_task_fn=lambda c: asyncio.sleep(0),
        teardown_fn=lambda c: asyncio.sleep(0),
        dataset_size_bytes=10 * large_size, batch_size=10,
    ))

    # Bulk Insert Small (10K per iteration)
    async def task_bulk(c):
        from bson import ObjectId
        docs = [{**small_doc, "_id": str(ObjectId())} for _ in range(10_000)]
        await c[DB_NAME]["bench_bulk_mc"].insert_many(docs)

    async def before_bulk(c):
        try:
            await c.run_command(DB_NAME, {"drop": "bench_bulk_mc"})
        except:
            pass

    results.append(await run_benchmark(
        "bulk_insert_small", "multi_doc",
        setup_fn=lambda c: asyncio.sleep(0),
        before_task_fn=before_bulk,
        task_fn=task_bulk,
        after_task_fn=lambda c: asyncio.sleep(0),
        teardown_fn=lambda c: asyncio.sleep(0),
        dataset_size_bytes=small_size * 10_000, batch_size=10_000,
    ))

    # Find Many (2K docs — limited by gRPC 4MB message size)
    # NOTE: Native pymongo does 10K but proto-encoded response exceeds 4MB at higher counts
    # TODO: Increase gRPC max_receive_message_length or implement response streaming
    async def setup_find_many(c):
        from bson import ObjectId
        try:
            await c.run_command(DB_NAME, {"drop": "bench_find_many_mc"})
        except:
            pass
        docs = [{**small_doc, "_id": str(ObjectId())} for _ in range(2_000)]
        await c[DB_NAME]["bench_find_many_mc"].insert_many(docs)

    async def task_find_many(c):
        await c[DB_NAME]["bench_find_many_mc"].find({})

    results.append(await run_benchmark(
        "find_many", "multi_doc",
        setup_fn=setup_find_many,
        before_task_fn=lambda c: asyncio.sleep(0),
        task_fn=task_find_many,
        after_task_fn=lambda c: asyncio.sleep(0),
        teardown_fn=lambda c: asyncio.sleep(0),
        dataset_size_bytes=small_size * 2_000, batch_size=2_000,
    ))

    # Bulk Insert Large — SKIPPED: exceeds gRPC default 4MB message limit (10 x 2.75MB = 27.5MB)
    # TODO: Increase gRPC max_message_size in MongoCore config to enable this benchmark
    print("  bulk_insert_large: SKIPPED (exceeds gRPC 4MB message limit)")

    # Find Many Large — SKIPPED: 10 x 2.75MB = 27.5MB response exceeds gRPC 4MB limit
    # TODO: Implement streaming/pagination in MongoCore Find RPC to handle large result sets
    print("  find_many_large: SKIPPED (response exceeds gRPC 4MB message limit)")

    # Save results
    output_path = RESULTS_DIR / "python_mongocore.json"
    with open(output_path, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nResults saved to {output_path}")


if __name__ == "__main__":
    asyncio.run(main())
