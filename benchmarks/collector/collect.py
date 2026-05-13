"""Collect benchmark results into a timestamped run folder.

Results are stored at: benchmarks/results/<timestamp>/
A 'latest' symlink always points to the most recent run.
"""

import json
import glob
import os
import shutil
from pathlib import Path
from datetime import datetime, timezone

RESULTS_DIR = Path(__file__).parent.parent / "results"


def collect():
    # Find all raw result JSON files in results/ root (not in subfolders)
    all_results = []
    raw_files = []

    for json_file in glob.glob(str(RESULTS_DIR / "*.json")):
        basename = Path(json_file).name
        if "latest" in basename:
            continue
        with open(json_file) as f:
            data = json.load(f)
            if isinstance(data, list):
                all_results.extend(data)
                raw_files.append(json_file)
            elif isinstance(data, dict) and "benchmark" in data:
                all_results.append(data)
                raw_files.append(json_file)

    if not all_results:
        print("No benchmark results found in results/. Run benchmarks first.")
        return None

    # Create timestamped run folder
    ts = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H-%M")
    run_dir = RESULTS_DIR / ts
    run_dir.mkdir(parents=True, exist_ok=True)
    charts_dir = run_dir / "charts"
    charts_dir.mkdir(exist_ok=True)

    # Move raw result files into run folder
    for raw_file in raw_files:
        dest = run_dir / Path(raw_file).name
        shutil.move(raw_file, dest)

    # Save combined results
    output = {
        "timestamp": datetime.now(timezone.utc).isoformat(),
        "results": all_results,
    }
    combined_path = run_dir / "results.json"
    with open(combined_path, "w") as f:
        json.dump(output, f, indent=2)

    # Move any existing charts
    root_charts = RESULTS_DIR / "charts"
    if root_charts.exists():
        for chart in root_charts.glob("*.svg"):
            shutil.move(str(chart), str(charts_dir / chart.name))
        # Clean up empty root charts dir
        if not list(root_charts.iterdir()):
            root_charts.rmdir()

    # Update 'latest' symlink
    latest_link = RESULTS_DIR / "latest"
    if latest_link.exists() or latest_link.is_symlink():
        latest_link.unlink()
    latest_link.symlink_to(ts)

    # Keep only last 10 run folders
    run_dirs = sorted(
        [d for d in RESULTS_DIR.iterdir() if d.is_dir() and d.name.startswith("202")],
        reverse=True,
    )
    for old_dir in run_dirs[10:]:
        shutil.rmtree(old_dir)

    print(f"Collected {len(all_results)} benchmark results into {run_dir}/")
    print(f"Symlink: results/latest → {ts}")
    return output


if __name__ == "__main__":
    output = collect()
    if output:
        from generate_readme import generate_readme
        generate_readme(output["results"])
