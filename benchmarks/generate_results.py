"""Generate RESULTS.md and pipeline chart from benchmark result JSON files."""

import json
import glob
from pathlib import Path
from datetime import datetime, timezone

RESULTS_DIR = Path(__file__).parent / "results"
OUTPUT_PATH = Path(__file__).parent / "RESULTS.md"
CHARTS_DIR = Path(__file__).parent / "results" / "charts"


def load_all_results():
    results = []
    for json_file in sorted(glob.glob(str(RESULTS_DIR / "*.json"))):
        with open(json_file) as f:
            data = json.load(f)
            if isinstance(data, list):
                results.extend(data)
    return results


def group_by_category(results):
    groups = {}
    for r in results:
        cat = r.get("category", "other")
        groups.setdefault(cat, []).append(r)
    return groups


def format_ops(ops_per_sec):
    if ops_per_sec >= 10000:
        return f"{ops_per_sec/1000:.1f}K"
    return f"{ops_per_sec:.0f}"


def driver_label(driver):
    labels = {
        "pymongo": "Python (pymongo)",
        "mongocore+python": "Python (MongoCore)",
        "mongodb-node": "TypeScript (native)",
        "mongocore+typescript": "TypeScript (MongoCore)",
        "mongo-go-driver": "Go (native)",
        "mongocore+go": "Go (MongoCore)",
        "mongodb-java-sync": "Java (native)",
        "mongocore+java": "Java (MongoCore)",
    }
    return labels.get(driver, driver)


DRIVER_TO_LANGUAGE = {
    "pymongo": "python",
    "mongocore+python": "python",
    "mongodb-node": "typescript",
    "mongocore+typescript": "typescript",
    "mongo-go-driver": "go",
    "mongocore+go": "go",
    "mongodb-java-sync": "java",
    "mongocore+java": "java",
}


def get_language(driver):
    return DRIVER_TO_LANGUAGE.get(driver)


def is_native(driver):
    return "mongocore" not in driver.lower()


def generate_overhead_chart(single_doc_results, multi_doc_results):
    """Generate SVG bar chart showing native vs MongoCore overhead per benchmark."""
    all_results = single_doc_results + multi_doc_results
    if not all_results:
        return None

    CHARTS_DIR.mkdir(parents=True, exist_ok=True)

    languages = ["python", "typescript", "go", "java"]
    lang_labels = {"python": "Python", "typescript": "TypeScript", "go": "Go", "java": "Java"}
    benchmarks = sorted(set(r["benchmark"] for r in all_results))

    # Collect data: for each benchmark, avg native and avg MongoCore across languages
    chart_data = []
    for bench in benchmarks:
        native_ops = []
        mc_ops = []
        for lang in languages:
            n = next((r for r in all_results if r["benchmark"] == bench and get_language(r["driver"]) == lang and is_native(r["driver"])), None)
            m = next((r for r in all_results if r["benchmark"] == bench and get_language(r["driver"]) == lang and not is_native(r["driver"])), None)
            if n:
                native_ops.append(n["ops_per_sec"])
            if m:
                mc_ops.append(m["ops_per_sec"])
        if native_ops and mc_ops:
            chart_data.append({
                "benchmark": bench,
                "native": sum(native_ops) / len(native_ops),
                "mongocore": sum(mc_ops) / len(mc_ops),
            })

    if not chart_data:
        return None

    num_groups = len(chart_data)
    chart_width = max(700, num_groups * 120)
    chart_height = 400
    margin_left = 80
    margin_right = 30
    margin_top = 40
    margin_bottom = 100
    plot_width = chart_width - margin_left - margin_right
    plot_height = chart_height - margin_top - margin_bottom

    group_width = plot_width / num_groups
    bar_width = group_width / 3
    colors = ["#3b82f6", "#f97316"]  # blue = native, orange = MongoCore

    max_val = max(max(d["native"], d["mongocore"]) for d in chart_data)
    y_scale = plot_height / max_val

    svg = []
    svg.append(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {chart_width} {chart_height}" font-family="system-ui, sans-serif" font-size="12">')
    svg.append(f'  <rect width="{chart_width}" height="{chart_height}" fill="white"/>')
    svg.append(f'  <text x="{chart_width/2}" y="20" text-anchor="middle" font-size="14" font-weight="bold">Native vs MongoCore: Avg ops/s Across Languages</text>')

    # Y-axis
    num_ticks = 5
    for i in range(num_ticks + 1):
        y_val = max_val * i / num_ticks
        y_pos = margin_top + plot_height - (y_val * y_scale)
        svg.append(f'  <line x1="{margin_left}" y1="{y_pos}" x2="{chart_width - margin_right}" y2="{y_pos}" stroke="#e5e7eb" stroke-width="1"/>')
        label = f"{y_val/1000:.0f}K" if y_val >= 1000 else f"{y_val:.0f}"
        svg.append(f'  <text x="{margin_left - 8}" y="{y_pos + 4}" text-anchor="end" font-size="11" fill="#6b7280">{label}</text>')

    # Bars
    for g_idx, d in enumerate(chart_data):
        group_x = margin_left + g_idx * group_width

        for b_idx, (key, color) in enumerate(zip(["native", "mongocore"], colors)):
            val = d[key]
            x = group_x + (b_idx + 0.5) * bar_width
            bar_h = val * y_scale
            y = margin_top + plot_height - bar_h
            svg.append(f'  <rect x="{x}" y="{y}" width="{bar_width * 0.8}" height="{bar_h}" fill="{color}" rx="2"/>')

        # Label
        label = d["benchmark"].replace("_", "\n")
        short_label = d["benchmark"].replace("_", " ")
        label_x = group_x + group_width / 2
        svg.append(f'  <text x="{label_x}" y="{margin_top + plot_height + 16}" text-anchor="middle" font-size="10" fill="#374151">{short_label}</text>')

    # Legend
    legend_x = margin_left + 10
    legend_y = margin_top + plot_height + 55
    for i, (color, label) in enumerate(zip(colors, ["Native driver", "MongoCore sidecar"])):
        x = legend_x + i * 170
        svg.append(f'  <rect x="{x}" y="{legend_y}" width="12" height="12" fill="{color}" rx="2"/>')
        svg.append(f'  <text x="{x + 16}" y="{legend_y + 10}" font-size="11" fill="#374151">{label}</text>')

    svg.append('</svg>')

    chart_path = CHARTS_DIR / "sidecar_overhead.svg"
    chart_path.write_text("\n".join(svg))
    return chart_path


def generate_ingestion_chart(ingestion_results):
    """Generate SVG chart comparing MongoCore ingestion vs native bulk insert."""
    if not ingestion_results:
        return None

    CHARTS_DIR.mkdir(parents=True, exist_ok=True)

    # Group by size (1mb, 10mb, 100mb) and format (csv, ndjson)
    mc_results = [r for r in ingestion_results if "mongocore" in r["driver"]]
    native_results = [r for r in ingestion_results if "native" in r["driver"]]

    if not mc_results or not native_results:
        return None

    # Extract size from benchmark name, e.g. "mongocore_ingest_10mb_csv"
    sizes = ["1mb", "10mb", "100mb"]
    formats = ["csv", "ndjson"]

    chart_data = []
    for size in sizes:
        for fmt in formats:
            mc = next((r for r in mc_results if size in r["benchmark"] and fmt in r["benchmark"]), None)
            native = next((r for r in native_results if size in r["benchmark"] and fmt in r["benchmark"]), None)
            if mc and native:
                chart_data.append({
                    "label": f"{size} {fmt}",
                    "mongocore_mbps": mc["mb_per_sec"],
                    "native_mbps": native["mb_per_sec"],
                })

    if not chart_data:
        return None

    num_groups = len(chart_data)
    chart_width = max(600, num_groups * 100)
    chart_height = 350
    margin_left = 70
    margin_right = 30
    margin_top = 40
    margin_bottom = 80
    plot_width = chart_width - margin_left - margin_right
    plot_height = chart_height - margin_top - margin_bottom

    group_width = plot_width / num_groups
    bar_width = group_width / 3
    colors = ["#f97316", "#3b82f6"]  # orange = MongoCore, blue = native

    max_val = max(max(d["mongocore_mbps"], d["native_mbps"]) for d in chart_data)
    y_scale = plot_height / max_val

    svg = []
    svg.append(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {chart_width} {chart_height}" font-family="system-ui, sans-serif" font-size="12">')
    svg.append(f'  <rect width="{chart_width}" height="{chart_height}" fill="white"/>')
    svg.append(f'  <text x="{chart_width/2}" y="20" text-anchor="middle" font-size="14" font-weight="bold">Ingestion Throughput (MB/s)</text>')

    # Y-axis
    num_ticks = 5
    for i in range(num_ticks + 1):
        y_val = max_val * i / num_ticks
        y_pos = margin_top + plot_height - (y_val * y_scale)
        svg.append(f'  <line x1="{margin_left}" y1="{y_pos}" x2="{chart_width - margin_right}" y2="{y_pos}" stroke="#e5e7eb" stroke-width="1"/>')
        svg.append(f'  <text x="{margin_left - 8}" y="{y_pos + 4}" text-anchor="end" font-size="11" fill="#6b7280">{y_val:.0f}</text>')

    # Y-axis label
    svg.append(f'  <text x="15" y="{margin_top + plot_height/2}" text-anchor="middle" font-size="11" fill="#6b7280" transform="rotate(-90 15 {margin_top + plot_height/2})">MB/s</text>')

    # Bars
    for g_idx, d in enumerate(chart_data):
        group_x = margin_left + g_idx * group_width

        for b_idx, (key, color) in enumerate(zip(["mongocore_mbps", "native_mbps"], colors)):
            val = d[key]
            x = group_x + (b_idx + 0.5) * bar_width
            bar_h = val * y_scale
            y = margin_top + plot_height - bar_h
            svg.append(f'  <rect x="{x}" y="{y}" width="{bar_width * 0.8}" height="{bar_h}" fill="{color}" rx="2"/>')

        # Label
        label_x = group_x + group_width / 2
        svg.append(f'  <text x="{label_x}" y="{margin_top + plot_height + 18}" text-anchor="middle" font-size="11">{d["label"]}</text>')

    # Legend
    legend_x = margin_left + 10
    legend_y = margin_top + plot_height + 45
    for i, (color, label) in enumerate(zip(colors, ["MongoCore (Polars)", "Native (pymongo bulk)"])):
        x = legend_x + i * 200
        svg.append(f'  <rect x="{x}" y="{legend_y}" width="12" height="12" fill="{color}" rx="2"/>')
        svg.append(f'  <text x="{x + 16}" y="{legend_y + 10}" font-size="11" fill="#374151">{label}</text>')

    svg.append('</svg>')

    chart_path = CHARTS_DIR / "ingestion_performance.svg"
    chart_path.write_text("\n".join(svg))
    return chart_path


def generate_pipeline_chart(pipeline_results, single_doc_results):
    """Generate an SVG bar chart comparing pipeline batch sizes vs native."""
    if not pipeline_results:
        return None

    CHARTS_DIR.mkdir(parents=True, exist_ok=True)

    pipeline_to_native = {
        "pipeline_run_command": "run_command",
        "pipeline_find_one_by_id": "find_one_by_id",
        "pipeline_insert_one_small": "insert_one_small",
    }

    # Group pipeline results by operation type, collecting all languages averaged
    op_types = sorted(set(r["benchmark"].rsplit("_", 1)[0] for r in pipeline_results))
    batch_sizes = sorted(set(int(r["benchmark"].rsplit("_", 1)[1]) for r in pipeline_results if r["benchmark"].rsplit("_", 1)[1].isdigit()))
    languages = ["python", "typescript", "go", "java"]

    # For each op type, compute average ops/s across languages for each batch size + native
    chart_data = {}  # {op_base: {"native": avg, 100: avg, 1000: avg, 10000: avg}}
    for op_base in op_types:
        chart_data[op_base] = {}
        native_bench = pipeline_to_native.get(op_base)

        # Native average
        if native_bench:
            native_ops = [r["ops_per_sec"] for r in single_doc_results
                         if r["benchmark"] == native_bench and is_native(r["driver"]) and get_language(r["driver"]) in languages]
            if native_ops:
                chart_data[op_base]["native"] = sum(native_ops) / len(native_ops)

        # Pipeline batch sizes
        for bs in batch_sizes:
            bench_name = f"{op_base}_{bs}"
            ops = [r["ops_per_sec"] for r in pipeline_results
                   if r["benchmark"] == bench_name and get_language(r["driver"]) in languages]
            if ops:
                chart_data[op_base][bs] = sum(ops) / len(ops)

    # Generate SVG
    chart_width = 800
    chart_height = 400
    margin_left = 80
    margin_right = 30
    margin_top = 40
    margin_bottom = 80
    plot_width = chart_width - margin_left - margin_right
    plot_height = chart_height - margin_top - margin_bottom

    # Each op_type gets a group of bars
    num_groups = len(op_types)
    bar_categories = ["native"] + batch_sizes  # native, 100, 1000, 10000
    num_bars = len(bar_categories)
    group_width = plot_width / num_groups
    bar_width = group_width / (num_bars + 1)
    colors = ["#6b7280", "#3b82f6", "#10b981", "#f59e0b"]  # gray, blue, green, amber

    # Find max value for y-axis
    all_values = []
    for op_data in chart_data.values():
        all_values.extend(op_data.values())
    max_val = max(all_values) if all_values else 1
    y_scale = plot_height / max_val

    svg_lines = []
    svg_lines.append(f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {chart_width} {chart_height}" font-family="system-ui, sans-serif" font-size="12">')
    svg_lines.append(f'  <rect width="{chart_width}" height="{chart_height}" fill="white"/>')
    svg_lines.append(f'  <text x="{chart_width/2}" y="20" text-anchor="middle" font-size="14" font-weight="bold">Pipeline Batching: Avg ops/s Across Languages</text>')

    # Y-axis gridlines and labels
    num_ticks = 5
    for i in range(num_ticks + 1):
        y_val = max_val * i / num_ticks
        y_pos = margin_top + plot_height - (y_val * y_scale)
        svg_lines.append(f'  <line x1="{margin_left}" y1="{y_pos}" x2="{chart_width - margin_right}" y2="{y_pos}" stroke="#e5e7eb" stroke-width="1"/>')
        label = f"{y_val/1000:.0f}K" if y_val >= 1000 else f"{y_val:.0f}"
        svg_lines.append(f'  <text x="{margin_left - 8}" y="{y_pos + 4}" text-anchor="end" font-size="11" fill="#6b7280">{label}</text>')

    # Bars
    for g_idx, op_base in enumerate(op_types):
        group_x = margin_left + g_idx * group_width
        op_data = chart_data[op_base]

        for b_idx, cat in enumerate(bar_categories):
            val = op_data.get(cat, 0)
            if val == 0:
                continue
            x = group_x + (b_idx + 0.5) * bar_width
            bar_h = val * y_scale
            y = margin_top + plot_height - bar_h
            svg_lines.append(f'  <rect x="{x}" y="{y}" width="{bar_width * 0.8}" height="{bar_h}" fill="{colors[b_idx]}" rx="2"/>')

        # Group label
        label = op_base.replace("pipeline_", "").replace("_", " ")
        label_x = group_x + group_width / 2
        svg_lines.append(f'  <text x="{label_x}" y="{margin_top + plot_height + 20}" text-anchor="middle" font-size="11">{label}</text>')

    # Legend
    legend_x = margin_left + 10
    legend_y = margin_top + plot_height + 45
    legend_labels = ["Native (single call)", "Batch 100", "Batch 1,000", "Batch 10,000"]
    for i, (color, label) in enumerate(zip(colors, legend_labels)):
        x = legend_x + i * 170
        svg_lines.append(f'  <rect x="{x}" y="{legend_y}" width="12" height="12" fill="{color}" rx="2"/>')
        svg_lines.append(f'  <text x="{x + 16}" y="{legend_y + 10}" font-size="11" fill="#374151">{label}</text>')

    svg_lines.append('</svg>')

    chart_path = CHARTS_DIR / "pipeline_performance.svg"
    chart_path.write_text("\n".join(svg_lines))
    return chart_path


def build_comparison_rows(cat_results):
    """Build native vs MongoCore comparison rows for a category."""
    rows = []
    benchmarks = sorted(set(r["benchmark"] for r in cat_results))
    languages = ["python", "typescript", "go", "java"]

    for bench in benchmarks:
        for lang in languages:
            native = next((r for r in cat_results if r["benchmark"] == bench and get_language(r["driver"]) == lang and is_native(r["driver"])), None)
            mc = next((r for r in cat_results if r["benchmark"] == bench and get_language(r["driver"]) == lang and not is_native(r["driver"])), None)

            if not native and not mc:
                continue

            native_ops = native["ops_per_sec"] if native else None
            mc_ops = mc["ops_per_sec"] if mc else None

            native_str = format_ops(native_ops) if native_ops else "—"
            mc_str = format_ops(mc_ops) if mc_ops else "—"

            if native_ops and mc_ops:
                overhead = ((native_ops - mc_ops) / native_ops) * 100
                overhead_str = f"+{overhead:.0f}%" if overhead > 0 else f"{overhead:.0f}%"
            else:
                overhead_str = "—"

            rows.append({
                "benchmark": bench,
                "language": lang.capitalize(),
                "native_ops": native_str,
                "mc_ops": mc_str,
                "overhead": overhead_str,
            })
    return rows


def build_pipeline_rows(pipeline_results, single_doc_results):
    """Build pipeline rows: one row per operation+batch_size, languages as columns."""
    pipeline_to_native = {
        "pipeline_run_command": "run_command",
        "pipeline_find_one_by_id": "find_one_by_id",
        "pipeline_insert_one_small": "insert_one_small",
    }

    rows = []
    benchmarks = sorted(set(r["benchmark"] for r in pipeline_results))
    languages = ["python", "typescript", "go", "java"]

    for bench in benchmarks:
        parts = bench.rsplit("_", 1)
        base_name = parts[0] if len(parts) == 2 else bench
        native_bench = pipeline_to_native.get(base_name)
        batch_size = parts[1] if len(parts) == 2 else "—"
        operation = base_name.replace("pipeline_", "")

        # Get ops/s for each language
        lang_ops = {}
        for lang in languages:
            r = next((r for r in pipeline_results if r["benchmark"] == bench and get_language(r["driver"]) == lang), None)
            lang_ops[lang] = format_ops(r["ops_per_sec"]) if r else "—"

        # Find fastest native equivalent across all languages
        native_ops_values = []
        if native_bench:
            for lang in languages:
                n = next((n for n in single_doc_results if n["benchmark"] == native_bench and get_language(n["driver"]) == lang and is_native(n["driver"])), None)
                if n:
                    native_ops_values.append(n["ops_per_sec"])

        fastest_native = max(native_ops_values) if native_ops_values else None
        native_str = format_ops(fastest_native) if fastest_native else "—"

        # Speedup: average pipeline ops across languages vs fastest native
        pipeline_ops_values = [r["ops_per_sec"] for r in pipeline_results if r["benchmark"] == bench and get_language(r["driver"]) in languages]
        avg_pipeline = sum(pipeline_ops_values) / len(pipeline_ops_values) if pipeline_ops_values else 0

        if fastest_native and avg_pipeline:
            speedup_str = f"{avg_pipeline / fastest_native:.1f}x"
        else:
            speedup_str = "—"

        rows.append({
            "operation": operation,
            "batch_size": batch_size,
            "python": lang_ops["python"],
            "typescript": lang_ops["typescript"],
            "go": lang_ops["go"],
            "java": lang_ops["java"],
            "native": native_str,
            "speedup": speedup_str,
        })
    return rows


def build_ingestion_rows(ingestion_results):
    """Build ingestion table rows."""
    rows = []
    for r in sorted(ingestion_results, key=lambda x: x["benchmark"]):
        rows.append({
            "benchmark": r["benchmark"],
            "driver": driver_label(r["driver"]),
            "ops": format_ops(r["ops_per_sec"]),
            "mbps": f"{r['mb_per_sec']:.2f}",
            "p50": f"{r['percentiles']['p50']:.3f}",
        })
    return rows


def generate():
    from jinja2 import Environment, FileSystemLoader

    results = load_all_results()
    if not results:
        print("No results found in results/. Run benchmarks first.")
        return

    groups = group_by_category(results)
    base_dir = Path(__file__).parent

    single_doc_results = groups.get("single_doc", [])
    multi_doc_results = groups.get("multi_doc", [])
    pipeline_results = groups.get("pipeline", [])
    ingestion_results = groups.get("ingestion", [])

    # Generate charts
    overhead_chart = generate_overhead_chart(single_doc_results, multi_doc_results)
    pipeline_chart = generate_pipeline_chart(pipeline_results, single_doc_results)
    ingestion_chart = generate_ingestion_chart(ingestion_results)

    def rel(chart_path):
        return str(chart_path.relative_to(base_dir)) if chart_path else None

    # Build template data
    system = next((r.get("system", {}) for r in results if r.get("system")), {})

    context = {
        "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M UTC"),
        "overhead_chart": rel(overhead_chart),
        "pipeline_chart": rel(pipeline_chart),
        "ingestion_chart": rel(ingestion_chart),
        "single_doc": build_comparison_rows(single_doc_results),
        "multi_doc": build_comparison_rows(multi_doc_results),
        "pipeline": build_pipeline_rows(pipeline_results, single_doc_results),
        "ingestion": build_ingestion_rows(ingestion_results),
        "system": system,
    }

    # Render template
    env = Environment(
        loader=FileSystemLoader(str(base_dir / "templates")),
        trim_blocks=True,
        lstrip_blocks=True,
    )
    template = env.get_template("results.md.j2")
    output = template.render(**context)

    OUTPUT_PATH.write_text(output)
    print(f"Results written to {OUTPUT_PATH}")


if __name__ == "__main__":
    generate()
