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


def run_benchmark(name, category, setup_fn, before_task_fn, task_fn, after_task_fn, teardown_fn, dataset_size_bytes, batch_size=1):
    """Run a benchmark following MongoDB spec methodology."""
    client = MongoClient(CONFIG["mongodb_uri"], w=1)
    db = client[DB_NAME]

    setup_fn(db)

    # Warmup
    for _ in range(WARMUP):
        before_task_fn(db)
        task_fn(db)
        after_task_fn(db)

    # Timed iterations
    times = []
    total_time = 0.0
    iteration = 0

    while total_time < MIN_TIME or iteration < 5:
        if iteration >= MAX_ITERS or total_time >= MAX_TIME:
            break

        before_task_fn(db)

        start = time.perf_counter()
        task_fn(db)
        elapsed = time.perf_counter() - start

        after_task_fn(db)

        times.append(elapsed)
        total_time += elapsed
        iteration += 1

    teardown_fn(db)
    client.close()

    # Calculate metrics
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
    large_doc = json.loads((DATA_DIR / "large_doc.json").read_text())
    small_size = len(json.dumps(small_doc).encode())
    tweet_size = len(json.dumps(tweet_doc).encode())
    large_size = len(json.dumps(large_doc).encode())

    # Run Command (batch 10,000 hello commands per iteration)
    def task_run_command(db):
        for _ in range(10_000):
            db.command("hello")

    results.append(run_benchmark(
        "run_command", "single_doc",
        setup_fn=lambda db: None,
        before_task_fn=lambda db: None,
        task_fn=task_run_command,
        after_task_fn=lambda db: None,
        teardown_fn=lambda db: None,
        dataset_size_bytes=10_000 * 100, batch_size=10_000,
    ))

    # Find One by ID (batch 10,000 finds per iteration)
    def setup_find(db):
        db.drop_collection("bench_find")
        coll = db["bench_find"]
        doc = {k: v for k, v in tweet_doc.items() if k != "_id"}
        doc["_id"] = ObjectId("000000000000000000000001")
        coll.insert_one(doc)
        coll.create_index("_id")

    def task_find_one(db):
        for _ in range(10_000):
            db["bench_find"].find_one({"_id": ObjectId("000000000000000000000001")})

    results.append(run_benchmark(
        "find_one_by_id", "single_doc",
        setup_fn=setup_find,
        before_task_fn=lambda db: None,
        task_fn=task_find_one,
        after_task_fn=lambda db: None,
        teardown_fn=lambda db: db.drop_collection("bench_find"),
        dataset_size_bytes=10_000 * tweet_size, batch_size=10_000,
    ))

    # InsertOne Small (batch 10,000 inserts per iteration)
    def task_insert_small(db):
        for _ in range(10_000):
            db["bench_insert_small"].insert_one({**small_doc, "_id": ObjectId()})

    results.append(run_benchmark(
        "insert_one_small", "single_doc",
        setup_fn=lambda db: None,
        before_task_fn=lambda db: db.drop_collection("bench_insert_small"),
        task_fn=task_insert_small,
        after_task_fn=lambda db: None,
        teardown_fn=lambda db: db.drop_collection("bench_insert_small"),
        dataset_size_bytes=10_000 * small_size, batch_size=10_000,
    ))

    # InsertOne Large (batch 10 inserts per iteration, large docs ~2.75MB each)
    def task_insert_large(db):
        for _ in range(10):
            db["bench_insert_large"].insert_one({**large_doc, "_id": ObjectId()})

    results.append(run_benchmark(
        "insert_one_large", "single_doc",
        setup_fn=lambda db: None,
        before_task_fn=lambda db: db.drop_collection("bench_insert_large"),
        task_fn=task_insert_large,
        after_task_fn=lambda db: None,
        teardown_fn=lambda db: db.drop_collection("bench_insert_large"),
        dataset_size_bytes=10 * large_size, batch_size=10,
    ))

    # Bulk Insert Small (10,000 docs per iteration)
    def bulk_insert_task(db):
        docs = [{**small_doc, "_id": ObjectId()} for _ in range(10_000)]
        db["bench_bulk"].insert_many(docs)

    results.append(run_benchmark(
        "bulk_insert_small", "multi_doc",
        setup_fn=lambda db: None,
        before_task_fn=lambda db: db.drop_collection("bench_bulk"),
        task_fn=bulk_insert_task,
        after_task_fn=lambda db: None,
        teardown_fn=lambda db: db.drop_collection("bench_bulk"),
        dataset_size_bytes=small_size * 10_000, batch_size=10_000,
    ))

    # Bulk Insert Large (10 x 2.75MB docs per iteration)
    def bulk_insert_large_task(db):
        docs = [{**large_doc, "_id": ObjectId()} for _ in range(10)]
        db["bench_bulk_large"].insert_many(docs)

    results.append(run_benchmark(
        "bulk_insert_large", "multi_doc",
        setup_fn=lambda db: None,
        before_task_fn=lambda db: db.drop_collection("bench_bulk_large"),
        task_fn=bulk_insert_large_task,
        after_task_fn=lambda db: None,
        teardown_fn=lambda db: db.drop_collection("bench_bulk_large"),
        dataset_size_bytes=large_size * 10, batch_size=10,
    ))

    # Find Many (10,000 small docs)
    def setup_find_many(db):
        db.drop_collection("bench_find_many")
        docs = [{**small_doc, "_id": ObjectId()} for _ in range(10_000)]
        db["bench_find_many"].insert_many(docs)

    def find_many_task(db):
        list(db["bench_find_many"].find({}))

    results.append(run_benchmark(
        "find_many", "multi_doc",
        setup_fn=setup_find_many,
        before_task_fn=lambda db: None,
        task_fn=find_many_task,
        after_task_fn=lambda db: None,
        teardown_fn=lambda db: db.drop_collection("bench_find_many"),
        dataset_size_bytes=small_size * 10_000, batch_size=10_000,
    ))

    # Find Many Large (100 large docs, emptying cursor)
    def setup_find_many_large(db):
        db.drop_collection("bench_find_many_large")
        docs = [{**large_doc, "_id": ObjectId()} for _ in range(10)]
        db["bench_find_many_large"].insert_many(docs)

    def find_many_large_task(db):
        list(db["bench_find_many_large"].find({}))

    results.append(run_benchmark(
        "find_many_large", "multi_doc",
        setup_fn=setup_find_many_large,
        before_task_fn=lambda db: None,
        task_fn=find_many_large_task,
        after_task_fn=lambda db: None,
        teardown_fn=lambda db: db.drop_collection("bench_find_many_large"),
        dataset_size_bytes=large_size * 10, batch_size=10,
    ))

    # Save results
    output_path = RESULTS_DIR / "python_native.json"
    with open(output_path, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nResults saved to {output_path}")


if __name__ == "__main__":
    main()
