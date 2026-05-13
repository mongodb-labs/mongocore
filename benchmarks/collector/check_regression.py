"""Check for performance regressions. Exit non-zero if any exceed threshold."""

import json
import sys
from pathlib import Path

RESULTS_DIR = Path(__file__).parent.parent / "results"
DEFAULT_THRESHOLD = 10  # percent


def load_run(run_dir):
    results_file = run_dir / "results.json"
    if not results_file.exists():
        return None
    with open(results_file) as f:
        return json.load(f)


def get_run_dirs():
    """Get timestamped run directories sorted newest first."""
    return sorted(
        [d for d in RESULTS_DIR.iterdir() if d.is_dir() and d.name.startswith("202")],
        reverse=True,
    )


def check(threshold=DEFAULT_THRESHOLD):
    runs = get_run_dirs()
    if len(runs) < 2:
        print("Not enough data to check regressions (need at least 2 runs)")
        return 0

    latest = load_run(runs[0])
    previous = load_run(runs[1])

    if not latest or not previous:
        print("Not enough data to check regressions")
        return 0

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
