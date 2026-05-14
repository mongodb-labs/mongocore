"""Benchmark MongoCore transactional pipeline (transfer pattern)."""

import asyncio
import json
import os
import platform
import random
import sys
import time
import statistics
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent.parent.parent / "clients" / "python" / "src"))
from mongocore import MongoClient
from mongocore.ops import TransactionStep, step_find_one, step_update

CONFIG = json.loads((Path(__file__).parent.parent / "common.json").read_text())
RESULTS_DIR = Path(__file__).parent.parent.parent / "results"
RESULTS_DIR.mkdir(exist_ok=True)

WARMUP = CONFIG["warmup_iterations"]["python"]
MIN_TIME = CONFIG["min_time_secs"]
MAX_ITERS = CONFIG["max_iterations"]
MAX_TIME = CONFIG["max_time_secs"]
DB_NAME = CONFIG["database"]
ADDR = CONFIG["mongocore_address"]
SOCKET_PATH = CONFIG.get("mongocore_socket_path", "/tmp/mongocore.sock")

NUM_ACCOUNTS = 1000
INITIAL_BALANCE = 10000
TRANSFER_AMOUNT = 10
BATCH_SIZES = [10, 100, 1000]
COLLECTION = "bench_txn_accounts"

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


async def run_benchmark(name, category, client, setup_fn, before_task_fn, task_fn, after_task_fn, teardown_fn, batch_size):
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
    txns_per_sec = batch_size / median

    def percentile(data, pct):
        import math
        idx = max(0, math.ceil(len(data) * pct / 100) - 1)
        return data[min(idx, len(data) - 1)]

    result = {
        "benchmark": name,
        "category": category,
        "driver": "mongocore+python",
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


async def main():
    print("=== MongoCore Transactional Pipeline benchmarks ===")
    results = []

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

    db = client[DB_NAME]

    for txns_per_iter in BATCH_SIZES:

        async def setup(c):
            try:
                await c.run_command(DB_NAME, {"drop": COLLECTION})
            except Exception:
                pass
            accounts = [
                {"_id": f"acct_{i:04d}", "balance": INITIAL_BALANCE}
                for i in range(NUM_ACCOUNTS)
            ]
            coll = c[DB_NAME][COLLECTION]
            await coll.insert_many(accounts)

        async def before_task(c):
            pass

        async def task(c, _n=txns_per_iter):
            for _ in range(_n):
                src_idx = random.randint(0, NUM_ACCOUNTS - 1)
                dst_idx = random.randint(0, NUM_ACCOUNTS - 2)
                if dst_idx >= src_idx:
                    dst_idx += 1
                src_id = f"acct_{src_idx:04d}"
                dst_id = f"acct_{dst_idx:04d}"

                steps = [
                    TransactionStep(
                        name="lookup_source",
                        operation=step_find_one({"_id": src_id}),
                        collection=COLLECTION,
                    ),
                    TransactionStep(
                        name="debit_source",
                        operation=step_update(
                            {"_id": "{{lookup_source._id}}"},
                            {"$inc": {"balance": -TRANSFER_AMOUNT}},
                        ),
                        collection=COLLECTION,
                    ),
                    TransactionStep(
                        name="credit_target",
                        operation=step_update(
                            {"_id": dst_id},
                            {"$inc": {"balance": TRANSFER_AMOUNT}},
                        ),
                        collection=COLLECTION,
                    ),
                ]
                await db.transaction_pipeline(steps)

        async def after_task(c):
            pass

        async def teardown(c):
            try:
                await c.run_command(DB_NAME, {"drop": COLLECTION})
            except Exception:
                pass

        results.append(await run_benchmark(
            f"txn_transfer_{txns_per_iter}", "txn_pipeline", client,
            setup_fn=setup,
            before_task_fn=before_task,
            task_fn=task,
            after_task_fn=after_task,
            teardown_fn=teardown,
            batch_size=txns_per_iter,
        ))

    await client.close()

    output_path = RESULTS_DIR / "python_txn_pipeline.json"
    with open(output_path, "w") as f:
        json.dump(results, f, indent=2)
    print(f"\nResults saved to {output_path}")


if __name__ == "__main__":
    asyncio.run(main())
