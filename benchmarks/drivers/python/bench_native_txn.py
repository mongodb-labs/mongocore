"""Benchmark pymongo native transactions (transfer pattern) for comparison."""

import json
import os
import platform
import random
import sys
import time
import statistics
from datetime import datetime, timezone
from pathlib import Path

from pymongo import MongoClient

CONFIG = json.loads((Path(__file__).parent.parent / "common.json").read_text())
RESULTS_DIR = Path(__file__).parent.parent.parent / "results"
RESULTS_DIR.mkdir(exist_ok=True)

WARMUP = CONFIG["warmup_iterations"]["python"]
MIN_TIME = CONFIG["min_time_secs"]
MAX_ITERS = CONFIG["max_iterations"]
MAX_TIME = CONFIG["max_time_secs"]
DB_NAME = CONFIG["database"]

NUM_ACCOUNTS = 1000
INITIAL_BALANCE = 10000
TRANSFER_AMOUNT = 10
BATCH_SIZES = [10, 100, 1000]
COLLECTION = "bench_txn_accounts"


def get_system_info():
    return {
        "os": platform.system().lower(),
        "arch": platform.machine(),
        "cpus": os.cpu_count(),
        "ram_gb": round(os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES") / (1024**3), 1) if hasattr(os, "sysconf") else None,
        "mongocore_version": "native",
        "driver": "pymongo",
    }


def run_benchmark(name, category, setup_fn, before_task_fn, task_fn, after_task_fn, teardown_fn, batch_size):
    """Run a benchmark following MongoDB spec methodology."""
    client = MongoClient(CONFIG["mongodb_uri"], w="majority")
    db = client[DB_NAME]

    setup_fn(db)

    for _ in range(WARMUP):
        before_task_fn(db)
        task_fn(db, client)
        after_task_fn(db)

    times = []
    total_time = 0.0
    iteration = 0

    while total_time < MIN_TIME or iteration < 5:
        if iteration >= MAX_ITERS or total_time >= MAX_TIME:
            break

        before_task_fn(db)
        start = time.perf_counter()
        task_fn(db, client)
        elapsed = time.perf_counter() - start
        after_task_fn(db)

        times.append(elapsed)
        total_time += elapsed
        iteration += 1

    teardown_fn(db)
    client.close()

    times.sort()
    median = statistics.median(times)
    txns_per_sec = batch_size / median

    def percentile(data, pct):
        import math
        idx = max(0, math.ceil(len(data) * pct / 100) - 1)
        return data[min(idx, len(data) - 1)]

    result = {
        "benchmark": name,
        "category": category,
        "driver": "pymongo",
        "batch_size": batch_size,
        "iterations": len(times),
        "total_time_secs": round(total_time, 3),
        "ops_per_sec": round(txns_per_sec, 1),
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
    print(f"  {name}: {txns_per_sec:.0f} txns/s ({len(times)} iters)")
    return result


def main():
    print("=== pymongo Native Transaction benchmarks ===")
    results = []

    for txns_per_iter in BATCH_SIZES:

        def setup(db):
            db.drop_collection(COLLECTION)
            accounts = [
                {"_id": f"acct_{i:04d}", "balance": INITIAL_BALANCE}
                for i in range(NUM_ACCOUNTS)
            ]
            db[COLLECTION].insert_many(accounts)

        def before_task(db):
            pass

        def task(db, client, _n=txns_per_iter):
            coll = db[COLLECTION]
            for _ in range(_n):
                src_idx = random.randint(0, NUM_ACCOUNTS - 1)
                dst_idx = random.randint(0, NUM_ACCOUNTS - 2)
                if dst_idx >= src_idx:
                    dst_idx += 1
                src_id = f"acct_{src_idx:04d}"
                dst_id = f"acct_{dst_idx:04d}"

                def transfer_callback(session, _src=src_id, _dst=dst_id):
                    source = coll.find_one({"_id": _src}, session=session)
                    coll.update_one(
                        {"_id": source["_id"]},
                        {"$inc": {"balance": -TRANSFER_AMOUNT}},
                        session=session,
                    )
                    coll.update_one(
                        {"_id": _dst},
                        {"$inc": {"balance": TRANSFER_AMOUNT}},
                        session=session,
                    )

                with client.start_session() as session:
                    session.with_transaction(transfer_callback)

        def after_task(db):
            pass

        def teardown(db):
            db.drop_collection(COLLECTION)

        results.append(run_benchmark(
            f"txn_transfer_{txns_per_iter}", "txn_pipeline",
            setup_fn=setup,
            before_task_fn=before_task,
            task_fn=task,
            after_task_fn=after_task,
            teardown_fn=teardown,
            batch_size=txns_per_iter,
        ))

    output_path = RESULTS_DIR / "python_native_txn.json"
    with open(output_path, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nResults saved to {output_path}")


if __name__ == "__main__":
    main()
