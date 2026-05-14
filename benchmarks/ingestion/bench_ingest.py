"""Benchmark MongoCore Polars ingestion vs native pymongo bulk insert.

Tests at 3 row counts: 10K, 100K, 500K in CSV and NDJSON formats.
Both use concurrent writes (4 threads) for fair comparison.
"""

import asyncio
import json
import os
import sys
import time
import statistics
from concurrent.futures import ThreadPoolExecutor, as_completed
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

FILE_SIZES = ["10k", "100k", "500k"]
FORMATS = ["csv", "ndjson"]


WRITE_CONCURRENCY = 4
BATCH_SIZE = 1000


def _insert_batch(uri, db_name, coll_name, batch):
    """Worker function for concurrent inserts."""
    client = PyMongoClient(uri, w=1)
    client[db_name][coll_name].insert_many(batch)
    client.close()
    return len(batch)


def bench_native_bulk(label, format_ext):
    """Benchmark native pymongo end-to-end: read file + parse + concurrent insert.

    Times the full pipeline: open file, parse CSV/NDJSON into documents,
    batch insert with concurrent threads (matching MongoCore's concurrency).
    """
    file_path = DATA_DIR / f"bench_{label}.{format_ext}"
    if not file_path.exists():
        print(f"    SKIPPED: {file_path.name} not found (run: just bench-generate-data)")
        return None

    file_size = file_path.stat().st_size

    client = PyMongoClient(MONGODB_URI, w=1)
    db = client[DB_NAME]
    coll_name = f"native_{label}_{format_ext}"

    iterations = 3 if file_size > 50_000_000 else 5
    times = []
    row_count = 0

    for _ in range(iterations):
        db.drop_collection(coll_name)

        start = time.perf_counter()

        # Read and parse file
        rows = []
        if format_ext == "csv":
            import csv as csv_mod
            with open(file_path) as f:
                reader = csv_mod.DictReader(f)
                rows = list(reader)
        elif format_ext == "ndjson":
            with open(file_path) as f:
                rows = [json.loads(line) for line in f]

        # Concurrent batch insert (matching MongoCore's write concurrency)
        batches = [rows[i:i + BATCH_SIZE] for i in range(0, len(rows), BATCH_SIZE)]
        with ThreadPoolExecutor(max_workers=WRITE_CONCURRENCY) as executor:
            futures = [executor.submit(_insert_batch, MONGODB_URI, DB_NAME, coll_name, batch) for batch in batches]
            for f in as_completed(futures):
                f.result()

        elapsed = time.perf_counter() - start
        times.append(elapsed)
        row_count = len(rows)

    client.close()

    median = statistics.median(times)
    mb_per_sec = file_size / median / 1_000_000
    rows_per_sec = row_count / median

    result = {
        "benchmark": f"native_bulk_{label}_{format_ext}",
        "category": "ingestion",
        "driver": "pymongo_native",
        "dataset_size_bytes": file_size,
        "batch_size": row_count,
        "iterations": len(times),
        "total_time_secs": round(sum(times), 3),
        "ops_per_sec": round(rows_per_sec, 1),
        "mb_per_sec": round(mb_per_sec, 3),
        "percentiles": {"p50": round(median, 4)},
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "system": {"driver": "pymongo_native", "operation": "file_read+parse+concurrent_insertMany", "file_size": label, "format": format_ext},
    }

    print(f"    native: {mb_per_sec:.1f} MB/s ({row_count:,} rows in {median:.1f}s)")
    return result


def bench_mongocore_ingest(label, format_ext):
    """Benchmark MongoCore Polars ingestion via gRPC."""
    file_path = DATA_DIR / f"bench_{label}.{format_ext}"
    if not file_path.exists():
        print(f"    SKIPPED: {file_path.name} not found")
        return None

    file_size = file_path.stat().st_size

    async def run():
        from mongocore import MongoClient as MongoCoreClient

        iterations = 3 if file_size > 50_000_000 else 5
        times = []

        for _ in range(iterations):
            async with MongoCoreClient(MONGOCORE_ADDR) as client:
                try:
                    await client.run_command(DB_NAME, {"drop": f"ingest_{label}_{format_ext}"})
                except:
                    pass

                start = time.perf_counter()
                resp = await client.ingest(
                    file_path=str(file_path.absolute()),
                    database=DB_NAME,
                    collection=f"ingest_{label}_{format_ext}",
                )

                # Poll until job completes (timeout after 5 minutes)
                job_id = resp["job_id"]
                poll_start = time.perf_counter()
                while True:
                    status = await client.ingest_status(job_id)
                    if status["status"] != 0:  # 0 = RUNNING
                        break
                    if time.perf_counter() - poll_start > 300:
                        print(f"    TIMEOUT: job {job_id} still running after 5 minutes")
                        break
                    await asyncio.sleep(0.05)

                elapsed = time.perf_counter() - start

                if status["status"] != 1:  # 1 = COMPLETED
                    print(f"    FAILED: job {job_id} status={status['status']}")
                    continue

                times.append(elapsed)

        if not times:
            return None

        median = statistics.median(times)
        mb_per_sec = file_size / median / 1_000_000

        return {
            "benchmark": f"mongocore_ingest_{label}_{format_ext}",
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
            "system": {"driver": "mongocore+polars", "operation": "ingest", "file_size": label, "format": format_ext},
        }

    result = asyncio.run(run())
    if result:
        print(f"    polars: {result['mb_per_sec']:.1f} MB/s (median {result['percentiles']['p50']:.1f}s)")
    return result


TRANSFORMS = [
    "cast(age, Int64)",
    "cast(score, Float64)",
    "filter(age >= 21)",
    "rename(name, full_name)",
    "drop(tags)",
]


def bench_native_transform(label, format_ext):
    """Benchmark native Python: read + parse + transform + concurrent insert.

    Applies the same transformations as MongoCore: cast age/score,
    filter age>=21, rename name→full_name, drop tags.
    Uses concurrent writes (4 threads) matching MongoCore's concurrency.
    """
    file_path = DATA_DIR / f"bench_{label}.{format_ext}"
    if not file_path.exists():
        print(f"    SKIPPED: {file_path.name} not found")
        return None

    file_size = file_path.stat().st_size

    client = PyMongoClient(MONGODB_URI, w=1)
    db = client[DB_NAME]
    coll_name = f"native_transform_{label}_{format_ext}"

    iterations = 3 if file_size > 50_000_000 else 5
    times = []
    row_count = 0

    for _ in range(iterations):
        db.drop_collection(coll_name)

        start = time.perf_counter()

        # Read and parse
        rows = []
        if format_ext == "csv":
            import csv as csv_mod
            with open(file_path) as f:
                reader = csv_mod.DictReader(f)
                rows = list(reader)
        elif format_ext == "ndjson":
            with open(file_path) as f:
                rows = [json.loads(line) for line in f]

        # Transform: cast age/score, filter age>=21, rename name→full_name, drop tags
        transformed = []
        for row in rows:
            age = int(row["age"]) if isinstance(row["age"], str) else row["age"]
            score = float(row["score"]) if isinstance(row["score"], str) else float(row["score"])
            if age < 21:
                continue
            doc = {
                "id": row["id"],
                "full_name": row["name"],
                "email": row["email"],
                "age": age,
                "score": score,
                "created_at": row["created_at"],
            }
            transformed.append(doc)

        # Concurrent batch insert (matching MongoCore's write concurrency)
        batches = [transformed[i:i + BATCH_SIZE] for i in range(0, len(transformed), BATCH_SIZE)]
        with ThreadPoolExecutor(max_workers=WRITE_CONCURRENCY) as executor:
            futures = [executor.submit(_insert_batch, MONGODB_URI, DB_NAME, coll_name, batch) for batch in batches]
            for f in as_completed(futures):
                f.result()

        elapsed = time.perf_counter() - start
        times.append(elapsed)
        row_count = len(transformed)

    client.close()

    median = statistics.median(times)
    mb_per_sec = file_size / median / 1_000_000
    rows_per_sec = row_count / median

    result = {
        "benchmark": f"native_transform_{label}_{format_ext}",
        "category": "ingestion",
        "driver": "pymongo_native",
        "dataset_size_bytes": file_size,
        "batch_size": row_count,
        "iterations": len(times),
        "total_time_secs": round(sum(times), 3),
        "ops_per_sec": round(rows_per_sec, 1),
        "mb_per_sec": round(mb_per_sec, 3),
        "percentiles": {"p50": round(median, 4)},
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "system": {"driver": "pymongo_native", "operation": "file_read+parse+transform+insertMany", "file_size": label, "format": format_ext},
    }

    print(f"    native: {mb_per_sec:.1f} MB/s ({row_count:,} rows in {median:.1f}s)")
    return result


def bench_mongocore_transform(label, format_ext):
    """Benchmark MongoCore Polars ingestion with transforms."""
    file_path = DATA_DIR / f"bench_{label}.{format_ext}"
    if not file_path.exists():
        print(f"    SKIPPED: {file_path.name} not found")
        return None

    file_size = file_path.stat().st_size

    async def run():
        from mongocore import MongoClient as MongoCoreClient

        iterations = 3 if file_size > 50_000_000 else 5
        times = []

        for _ in range(iterations):
            async with MongoCoreClient(MONGOCORE_ADDR) as client:
                try:
                    await client.run_command(DB_NAME, {"drop": f"mc_transform_{label}_{format_ext}"})
                except:
                    pass

                start = time.perf_counter()
                resp = await client.ingest(
                    file_path=str(file_path.absolute()),
                    database=DB_NAME,
                    collection=f"mc_transform_{label}_{format_ext}",
                    expressions=TRANSFORMS,
                )

                job_id = resp["job_id"]
                poll_start = time.perf_counter()
                while True:
                    status = await client.ingest_status(job_id)
                    if status["status"] != 0:
                        break
                    if time.perf_counter() - poll_start > 300:
                        print(f"    TIMEOUT: job {job_id} still running after 5 minutes")
                        break
                    await asyncio.sleep(0.05)

                elapsed = time.perf_counter() - start

                if status["status"] != 1:
                    print(f"    FAILED: job {job_id} status={status['status']}")
                    continue

                times.append(elapsed)

        if not times:
            return None

        median = statistics.median(times)
        mb_per_sec = file_size / median / 1_000_000

        return {
            "benchmark": f"mongocore_transform_{label}_{format_ext}",
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
            "system": {"driver": "mongocore+polars", "operation": "ingest+transform", "file_size": label, "format": format_ext},
        }

    result = asyncio.run(run())
    if result:
        print(f"    polars: {result['mb_per_sec']:.1f} MB/s (median {result['percentiles']['p50']:.1f}s)")
    return result


def main():
    print("=== Ingestion Benchmarks ===")
    print(f"    Data directory: {DATA_DIR}")
    print()
    results = []

    for label in FILE_SIZES:
        for fmt in FORMATS:
            file_path = DATA_DIR / f"bench_{label}.{fmt}"
            if not file_path.exists():
                print(f"  [{label} {fmt}] SKIPPED — not generated")
                continue

            file_mb = file_path.stat().st_size / 1_000_000
            print(f"  [{label} {fmt}] ({file_mb:.1f} MB)")

            print(f"    Running native end-to-end (read+parse+insert)...")
            native_result = bench_native_bulk(label, fmt)
            if native_result:
                results.append(native_result)

            print(f"    Running MongoCore ingest...")
            mc_result = bench_mongocore_ingest(label, fmt)
            if mc_result:
                results.append(mc_result)

            print(f"    Running native with transforms (read+parse+transform+insert)...")
            native_transform = bench_native_transform(label, fmt)
            if native_transform:
                results.append(native_transform)

            print(f"    Running MongoCore ingest with transforms...")
            mc_transform = bench_mongocore_transform(label, fmt)
            if mc_transform:
                results.append(mc_transform)
            print()

    # Save results
    if results:
        output_path = RESULTS_DIR / "ingestion.json"
        with open(output_path, "w") as f:
            json.dump(results, f, indent=2)
        print(f"Results saved to {output_path}")
    else:
        print("No results. Generate data first: just bench-generate-data")


if __name__ == "__main__":
    main()
