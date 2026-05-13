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
