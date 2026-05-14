"""Generate README.md from collected results into the latest run folder."""

import json
from pathlib import Path
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from jinja2 import Environment, FileSystemLoader

RESULTS_DIR = Path(__file__).parent.parent / "results"
TEMPLATES_DIR = Path(__file__).parent / "templates"


def get_run_dir():
    """Get the latest run directory (via symlink)."""
    latest = RESULTS_DIR / "latest"
    if latest.is_symlink() or latest.is_dir():
        return latest.resolve()
    return None


def load_results():
    run_dir = get_run_dir()
    if not run_dir:
        return []
    results_file = run_dir / "results.json"
    if not results_file.exists():
        return []
    with open(results_file) as f:
        data = json.load(f)
    return data.get("results", [])


def get_language(driver_name):
    if not driver_name:
        return "unknown"
    d = driver_name.lower()
    if "python" in d or "pymongo" in d:
        return "Python"
    elif "java" in d:
        return "Java"
    elif "typescript" in d or "node" in d:
        return "TypeScript"
    elif "go-driver" in d or "+go" in d or d == "go":
        return "Go"
    return "unknown"


def is_native(driver_name):
    if not driver_name:
        return False
    return "mongocore" not in driver_name.lower()


# --- Chart generation ---

LANG_COLORS = {
    "Python": ("#306998", "#FFD43B"),
    "TypeScript": ("#3178C6", "#7FDBFF"),
    "Go": ("#00ADD8", "#66D9EF"),
    "Java": ("#ED8B00", "#F8981D"),
}

BENCHMARKS_TO_CHART = [
    "run_command", "find_one_by_id", "insert_one_small", "insert_one_large",
    "bulk_insert_small", "find_many",
]


def generate_overhead_chart(languages_data, charts_dir, readme_path):
    langs_with_data = [
        lang for lang in sorted(languages_data.keys())
        if languages_data[lang]["native"] and languages_data[lang]["mongocore"]
    ]
    if not langs_with_data:
        return None

    benchmarks_with_data = []
    for bench in BENCHMARKS_TO_CHART:
        for lang in langs_with_data:
            if bench in languages_data[lang]["native"] and bench in languages_data[lang]["mongocore"]:
                benchmarks_with_data.append(bench)
                break

    if not benchmarks_with_data:
        return None

    fig, ax = plt.subplots(figsize=(14, 7))
    n_benchmarks = len(benchmarks_with_data)
    n_langs = len(langs_with_data)
    total_bars_per_group = n_langs * 2
    bar_width = 0.8 / total_bars_per_group
    x = range(n_benchmarks)

    for lang_idx, lang in enumerate(langs_with_data):
        native = languages_data[lang]["native"]
        mc = languages_data[lang]["mongocore"]
        native_color, mc_color = LANG_COLORS.get(lang, ("#333333", "#999999"))

        native_ops = [native.get(b, {}).get("ops_per_sec", 0) for b in benchmarks_with_data]
        mc_ops = [mc.get(b, {}).get("ops_per_sec", 0) for b in benchmarks_with_data]

        offset_native = (lang_idx * 2 - total_bars_per_group / 2 + 0.5) * bar_width
        offset_mc = (lang_idx * 2 + 1 - total_bars_per_group / 2 + 0.5) * bar_width

        ax.bar([i + offset_native for i in x], native_ops, bar_width,
               label=f'{lang} native', color=native_color, alpha=0.9, edgecolor='white', linewidth=0.5)
        ax.bar([i + offset_mc for i in x], mc_ops, bar_width,
               label=f'{lang} MongoCore', color=mc_color, alpha=0.9, edgecolor='white', linewidth=0.5,
               hatch='//')

    ax.set_xlabel('Benchmark', fontsize=11)
    ax.set_ylabel('Operations/sec (log scale)', fontsize=11)
    ax.set_title('Native Driver vs MongoCore Sidecar — Per Language', fontsize=13, fontweight='bold')
    ax.set_xticks(x)
    ax.set_xticklabels([b.replace('_', ' ') for b in benchmarks_with_data], rotation=20, ha='right')
    ax.legend(loc='upper right', fontsize=9, ncol=n_langs, framealpha=0.9)
    ax.grid(axis='y', alpha=0.3)
    ax.set_yscale('log')

    plt.tight_layout()
    chart_path = charts_dir / "sidecar_overhead.svg"
    plt.savefig(chart_path, format='svg', bbox_inches='tight')
    plt.close()
    return chart_path.relative_to(readme_path.parent)


def generate_ingestion_chart(native_ingest, mc_ingest, charts_dir, readme_path):
    sizes = ["1mb", "10mb", "100mb"]
    formats = ["csv", "ndjson"]

    fig, ax = plt.subplots(figsize=(12, 6))
    bar_width = 0.18
    x = range(len(sizes))

    colors = {
        ("native", "csv"): "#2E7D32",
        ("native", "ndjson"): "#66BB6A",
        ("polars", "csv"): "#A04500",
        ("polars", "ndjson"): "#FF8A65",
    }

    bar_idx = 0
    for fmt in formats:
        native_vals = [native_ingest.get((s, fmt), {}).get("mb_per_sec", 0) for s in sizes]
        if any(v > 0 for v in native_vals):
            offset = (bar_idx - 1.5) * bar_width
            ax.bar([i + offset for i in x], native_vals, bar_width,
                   label=f'Native bulk ({fmt})', color=colors[("native", fmt)], alpha=0.9)
            bar_idx += 1

        mc_vals = [mc_ingest.get((s, fmt), {}).get("mb_per_sec", 0) for s in sizes]
        if any(v > 0 for v in mc_vals):
            offset = (bar_idx - 1.5) * bar_width
            ax.bar([i + offset for i in x], mc_vals, bar_width,
                   label=f'Polars ingest ({fmt})', color=colors[("polars", fmt)], alpha=0.9, hatch='//')
            bar_idx += 1

    ax.set_xlabel('File Size')
    ax.set_ylabel('MB/s')
    ax.set_title('Ingestion Performance: Native Bulk Insert vs MongoCore Polars')
    ax.set_xticks(x)
    ax.set_xticklabels([s.upper() for s in sizes])
    ax.legend(loc='upper left', fontsize=9)
    ax.grid(axis='y', alpha=0.3)

    plt.tight_layout()
    chart_path = charts_dir / "ingestion_performance.svg"
    plt.savefig(chart_path, format='svg', bbox_inches='tight')
    plt.close()
    return chart_path.relative_to(readme_path.parent)


def generate_overhead_summary_chart(languages_data, charts_dir, readme_path):
    """Generate horizontal bar chart showing % overhead per benchmark per language."""
    benchmarks = [
        "run_command", "find_one_by_id", "insert_one_small", "insert_one_large",
        "bulk_insert_small", "find_many",
    ]

    langs_with_data = [
        lang for lang in sorted(languages_data.keys())
        if languages_data[lang]["native"] and languages_data[lang]["mongocore"]
    ]
    if not langs_with_data:
        return None

    lang_colors_flat = {
        "Python": "#306998",
        "TypeScript": "#3178C6",
        "Go": "#00ADD8",
        "Java": "#ED8B00",
    }

    fig, ax = plt.subplots(figsize=(12, 7))
    n_benchmarks = len(benchmarks)
    n_langs = len(langs_with_data)
    bar_height = 0.8 / n_langs
    y = range(n_benchmarks)

    for lang_idx, lang in enumerate(langs_with_data):
        native = languages_data[lang]["native"]
        mc = languages_data[lang]["mongocore"]
        overheads = []
        for bench in benchmarks:
            n = native.get(bench)
            m = mc.get(bench)
            if n and m and n.get("ops_per_sec", 0) > 0:
                overhead = ((m["ops_per_sec"] - n["ops_per_sec"]) / n["ops_per_sec"]) * 100
                overheads.append(overhead)
            else:
                overheads.append(0)

        offset = (lang_idx - n_langs / 2 + 0.5) * bar_height
        color = lang_colors_flat.get(lang, "#666666")
        bars = ax.barh(
            [i + offset for i in y], overheads, bar_height,
            label=lang, color=color, alpha=0.85, edgecolor='white', linewidth=0.5,
        )
        for bar, val in zip(bars, overheads):
            if val != 0:
                ha = 'left' if val >= 0 else 'right'
                x_pos = bar.get_width() + (1 if val >= 0 else -1)
                ax.text(x_pos, bar.get_y() + bar.get_height() / 2, f'{val:+.0f}%',
                        va='center', ha=ha, fontsize=8, color=color)

    ax.axvline(x=0, color='black', linewidth=0.8)
    ax.set_xlabel('Overhead % (negative = MongoCore slower)', fontsize=11)
    ax.set_title('MongoCore Sidecar Overhead by Operation', fontsize=13, fontweight='bold')
    ax.set_yticks(y)
    ax.set_yticklabels([b.replace('_', ' ') for b in benchmarks])
    ax.legend(loc='lower right', fontsize=10)
    ax.grid(axis='x', alpha=0.3)

    plt.tight_layout()
    chart_path = charts_dir / "overhead_summary.svg"
    plt.savefig(chart_path, format='svg', bbox_inches='tight')
    plt.close()
    return chart_path.relative_to(readme_path.parent)


# --- Template context building ---

ALL_BENCHMARKS = [
    "run_command", "find_one_by_id", "insert_one_small", "insert_one_large",
    "bulk_insert_small", "bulk_insert_large", "find_many", "find_many_large",
]

PIPELINE_OPERATIONS = ["run_command", "insert_one_small", "find_one_by_id"]
PIPELINE_BATCH_SIZES = [100, 1000, 10000]


def build_pipeline_context(results, languages_data):
    """Build context for pipeline batching section."""
    pipeline_results = [r for r in results if r.get("category") == "pipeline"]
    if not pipeline_results:
        return []

    pipeline_by_lang = {}
    for r in pipeline_results:
        lang = get_language(r.get("driver", ""))
        if lang == "unknown":
            continue
        if lang not in pipeline_by_lang:
            pipeline_by_lang[lang] = {}
        pipeline_by_lang[lang][r["benchmark"]] = r

    lang_sections = []
    for lang in sorted(pipeline_by_lang.keys()):
        pl = pipeline_by_lang[lang]
        native = languages_data.get(lang, {}).get("native", {})
        mc = languages_data.get(lang, {}).get("mongocore", {})

        rows = []
        for op in PIPELINE_OPERATIONS:
            native_result = native.get(op)
            mc_result = mc.get(op)
            native_ops = f"{native_result['ops_per_sec']:,.0f}" if native_result else "—"
            mc_individual_ops = f"{mc_result['ops_per_sec']:,.0f}" if mc_result else "—"

            pipeline_ops = {}
            for bs in PIPELINE_BATCH_SIZES:
                key = f"pipeline_{op}_{bs}"
                pr = pl.get(key)
                pipeline_ops[bs] = f"{pr['ops_per_sec']:,.0f}" if pr else "—"

            best_speedup = "—"
            if native_result and native_result["ops_per_sec"] > 0:
                best_pipeline_ops = max(
                    (pl.get(f"pipeline_{op}_{bs}", {}).get("ops_per_sec", 0) for bs in PIPELINE_BATCH_SIZES),
                    default=0,
                )
                if best_pipeline_ops > 0:
                    speedup = best_pipeline_ops / native_result["ops_per_sec"]
                    best_speedup = f"{speedup:.1f}x"

            rows.append({
                "operation": op,
                "native_ops": native_ops,
                "mc_individual_ops": mc_individual_ops,
                "p100": pipeline_ops[100],
                "p1000": pipeline_ops[1000],
                "p10000": pipeline_ops[10000],
                "best_speedup": best_speedup,
            })

        lang_sections.append({"name": lang, "rows": rows})

    return lang_sections


def build_compiled_context(results):
    """Build context for compiled query cache section."""
    compiled_results = [r for r in results if r.get("category") == "compiled_query"]
    if not compiled_results:
        return []

    rows = []
    for r in compiled_results:
        pct = r.get("percentiles", {})
        p50_val = pct.get("p50", 0)
        p99_val = pct.get("p99", 0)

        def fmt_latency(v):
            if v == 0:
                return "—"
            us = v * 1_000_000
            if us < 1000:
                return f"{us:.0f}us"
            return f"{us / 1000:.2f}ms"

        rows.append({
            "benchmark": r["benchmark"],
            "ops_per_sec": f"{r['ops_per_sec']:,.0f}",
            "p50": fmt_latency(p50_val),
            "p99": fmt_latency(p99_val),
        })

    return rows


def generate_pipeline_charts(pipeline_data, languages_data, charts_dir, readme_path):
    """Generate one line chart per language showing ops/s vs batch size for pipeline operations."""
    if not pipeline_data:
        return {}

    chart_paths = {}
    batch_sizes = [100, 1000, 10000]
    colors = {"run_command": "#306998", "insert_one_small": "#ED8B00", "find_one_by_id": "#00ADD8"}

    for lang_data in pipeline_data:
        lang_name = lang_data["name"]
        fig, ax = plt.subplots(figsize=(12, 7))

        for row in lang_data["rows"]:
            op = row["operation"]
            ops_values = []
            for bs in batch_sizes:
                val_str = row[f"p{bs}"]
                if val_str == "—":
                    ops_values.append(0)
                else:
                    ops_values.append(float(val_str.replace(",", "")))

            if any(v > 0 for v in ops_values):
                ax.plot(batch_sizes, ops_values, 'o-', label=f'{op}',
                        color=colors.get(op, '#666666'), linewidth=2, markersize=8)

            native_str = row["native_ops"]
            if native_str != "—":
                native_val = float(native_str.replace(",", ""))
                ax.axhline(y=native_val, linestyle='--', alpha=0.4,
                           color=colors.get(op, '#666666'))

            mc_str = row["mc_individual_ops"]
            if mc_str != "—":
                mc_val = float(mc_str.replace(",", ""))
                ax.axhline(y=mc_val, linestyle=':', alpha=0.4,
                           color=colors.get(op, '#666666'))

        ax.set_xscale('log')
        ax.set_xlabel('Batch Size (ops per pipeline call)', fontsize=11)
        ax.set_ylabel('Operations/sec', fontsize=11)
        ax.set_title(f'Pipeline Batching Scaling — {lang_name}', fontsize=13, fontweight='bold')
        ax.set_xticks(batch_sizes)
        ax.set_xticklabels(['100', '1K', '10K'])
        ax.legend(loc='upper left', fontsize=10)
        ax.grid(axis='y', alpha=0.3)

        plt.tight_layout()
        chart_filename = f"pipeline_scaling_{lang_name.lower()}.svg"
        chart_path = charts_dir / chart_filename
        plt.savefig(chart_path, format='svg', bbox_inches='tight')
        plt.close()
        chart_paths[lang_name] = str(chart_path.relative_to(readme_path.parent))

    return chart_paths


def build_context(results, charts_dir, readme_path):
    ctx = {"environment": None, "languages": [], "overhead_chart_path": None, "overhead_summary_chart_path": None, "ingestion": None, "pipeline_data": [], "compiled_data": []}

    if not results:
        return ctx

    # Environment
    sys_info = results[0].get("system", {})
    ctx["environment"] = {
        "os": sys_info.get("os", "unknown"),
        "arch": sys_info.get("arch", "unknown"),
        "cpus": sys_info.get("cpus", "unknown"),
        "mongocore_version": sys_info.get("mongocore_version", "unknown"),
        "date": results[0].get("timestamp", "unknown")[:10],
    }

    # Group by language
    languages_data = {}
    for r in results:
        lang = get_language(r.get("driver", ""))
        if lang == "unknown":
            continue
        if lang not in languages_data:
            languages_data[lang] = {"native": {}, "mongocore": {}}
        if is_native(r.get("driver", "")):
            languages_data[lang]["native"][r["benchmark"]] = r
        else:
            languages_data[lang]["mongocore"][r["benchmark"]] = r

    # Build per-language table rows and latency data
    for lang in sorted(languages_data.keys()):
        native = languages_data[lang]["native"]
        mc = languages_data[lang]["mongocore"]
        if not native and not mc:
            continue

        rows = []
        latency_rows = []
        for bench in ALL_BENCHMARKS:
            n = native.get(bench)
            m = mc.get(bench)
            if n and m:
                overhead = ((m["ops_per_sec"] - n["ops_per_sec"]) / n["ops_per_sec"]) * 100 if n["ops_per_sec"] > 0 else 0
                rows.append({
                    "benchmark": bench,
                    "has_mc": True,
                    "native_ops": f"{n['ops_per_sec']:,.0f}",
                    "mc_ops": f"{m['ops_per_sec']:,.0f}",
                    "overhead": f"{overhead:+.1f}%",
                    "native_mb": f"{n['mb_per_sec']:.1f}",
                    "mc_mb": f"{m['mb_per_sec']:.1f}",
                })
            elif n:
                rows.append({
                    "benchmark": bench,
                    "has_mc": False,
                    "native_ops": f"{n['ops_per_sec']:,.0f}",
                    "native_mb": f"{n['mb_per_sec']:.1f}",
                })

            # Latency percentiles — normalize to per-operation microseconds
            if n or m:
                def fmt_us(result, key):
                    """Format percentile as per-op latency in appropriate unit."""
                    if not result:
                        return "—"
                    pct_dict = result.get("percentiles")
                    if not pct_dict:
                        return "—"
                    v = pct_dict.get(key, 0)
                    if v == 0:
                        return "—"
                    batch = result.get("batch_size", 1)
                    if batch > 1:
                        v = v / batch
                    us = v * 1_000_000
                    if us < 1000:
                        return f"{us:.0f}us"
                    ms = us / 1000
                    if ms < 100:
                        return f"{ms:.2f}ms"
                    return f"{ms:.0f}ms"

                latency_rows.append({
                    "benchmark": bench,
                    "native_p50": fmt_us(n, "p50"),
                    "native_p95": fmt_us(n, "p95"),
                    "native_p99": fmt_us(n, "p99"),
                    "mc_p50": fmt_us(m, "p50"),
                    "mc_p95": fmt_us(m, "p95"),
                    "mc_p99": fmt_us(m, "p99"),
                    "has_mc": bool(m),
                })

        if rows:
            ctx["languages"].append({
                "name": lang,
                "has_both": bool(native and mc),
                "has_native": bool(native),
                "rows": rows,
                "latency_rows": latency_rows,
            })

    # Overhead charts
    chart_path = generate_overhead_chart(languages_data, charts_dir, readme_path)
    if chart_path:
        ctx["overhead_chart_path"] = str(chart_path)

    summary_chart_path = generate_overhead_summary_chart(languages_data, charts_dir, readme_path)
    if summary_chart_path:
        ctx["overhead_summary_chart_path"] = str(summary_chart_path)

    # Ingestion
    ingestion_results = [r for r in results if r.get("category") == "ingestion"]
    if ingestion_results:
        native_ingest = {}
        mc_ingest = {}
        for r in ingestion_results:
            si = r.get("system", {})
            key = (si.get("file_size", ""), si.get("format", ""))
            if "native" in r.get("driver", ""):
                native_ingest[key] = r
            elif "polars" in r.get("driver", "") or "mongocore" in r.get("driver", ""):
                mc_ingest[key] = r

        if native_ingest or mc_ingest:
            ingest_rows = []
            for label in ["1mb", "10mb", "100mb"]:
                for fmt in ["csv", "ndjson"]:
                    key = (label, fmt)
                    n = native_ingest.get(key)
                    m = mc_ingest.get(key)
                    if n and m:
                        speedup = m["mb_per_sec"] / n["mb_per_sec"] if n["mb_per_sec"] > 0 else 0
                        ingest_rows.append({
                            "size": label.upper(),
                            "format": fmt,
                            "native_mb": f"{n['mb_per_sec']:.1f}",
                            "mc_mb": f"{m['mb_per_sec']:.1f}",
                            "speedup": f"{speedup:.1f}x" if speedup >= 1 else f"{speedup:.2f}x",
                        })
                    elif n:
                        ingest_rows.append({"size": label.upper(), "format": fmt, "native_mb": f"{n['mb_per_sec']:.1f}", "mc_mb": "—", "speedup": "—"})
                    elif m:
                        ingest_rows.append({"size": label.upper(), "format": fmt, "native_mb": "—", "mc_mb": f"{m['mb_per_sec']:.1f}", "speedup": "—"})

            ingest_chart = generate_ingestion_chart(native_ingest, mc_ingest, charts_dir, readme_path)
            ctx["ingestion"] = {
                "rows": ingest_rows,
                "chart_path": str(ingest_chart) if ingest_chart else None,
            }

    # Pipeline batching
    pipeline_data = build_pipeline_context(results, languages_data)
    ctx["pipeline_data"] = pipeline_data
    pipeline_chart_paths = generate_pipeline_charts(pipeline_data, languages_data, charts_dir, readme_path)
    for lang_section in pipeline_data:
        lang_section["chart_path"] = pipeline_chart_paths.get(lang_section["name"])

    # Compiled query
    ctx["compiled_data"] = build_compiled_context(results)

    return ctx


def generate_readme(results):
    run_dir = get_run_dir()
    if not run_dir:
        print("No run directory found. Run collect.py first.")
        return

    readme_path = run_dir / "README.md"
    charts_dir = run_dir / "charts"
    charts_dir.mkdir(parents=True, exist_ok=True)

    ctx = build_context(results, charts_dir, readme_path)

    env = Environment(
        loader=FileSystemLoader(str(TEMPLATES_DIR)),
        keep_trailing_newline=True,
        trim_blocks=True,
        lstrip_blocks=True,
    )
    template = env.get_template("results.md.j2")
    readme_content = template.render(**ctx)

    readme_path.write_text(readme_content)
    print(f"Generated {readme_path}")


if __name__ == "__main__":
    results = load_results()
    if not results:
        print("No results found. Run collect.py first, then generate_readme.py")
    else:
        generate_readme(results)
