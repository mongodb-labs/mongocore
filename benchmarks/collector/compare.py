"""Compare latest benchmark results against previous run."""

import json
from pathlib import Path

RESULTS_DIR = Path(__file__).parent.parent / "results"


def get_run_dirs():
    """Get timestamped run directories sorted newest first."""
    return sorted(
        [d for d in RESULTS_DIR.iterdir() if d.is_dir() and d.name.startswith("202")],
        reverse=True,
    )


def load_run(run_dir):
    results_file = run_dir / "results.json"
    if not results_file.exists():
        return None
    with open(results_file) as f:
        return json.load(f)


def compare():
    runs = get_run_dirs()
    if len(runs) < 2:
        print("Not enough data to compare (need at least 2 runs)")
        return

    latest = load_run(runs[0])
    previous = load_run(runs[1])

    if not latest or not previous:
        print("Not enough data to compare")
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
        flag = "!" if change < -5 else ""
        print(f"{key[0]:<25} {key[1]:<20} {prev_ops:>10,.0f} {curr_ops:>10,.0f} {change:>+8.1f}% {flag}")


if __name__ == "__main__":
    compare()
