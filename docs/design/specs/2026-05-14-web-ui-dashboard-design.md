# MongoCore Web UI Dashboard

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.
> If modifying client libraries: verify imports work and run `just test-clients`.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

## Overview

A single-page diagnostic dashboard embedded in the MongoCore binary, providing real-time visibility into sidecar operations, performance, and health. Served locally only — no external access.

**Prior art:** Envoy Admin UI (localhost-only sidecar diagnostics), Meilisearch Mini-Dashboard (embedded in Rust binary), Go's `net/http/pprof` (zero-config diagnostic endpoints).

## Configuration

| Config | CLI Flag | Env Var | Default | Description |
|--------|----------|---------|---------|-------------|
| Enabled | `--web-ui` | `MONGOCORE_WEB_UI` | `true` | Toggle dashboard on/off |
| Port | `--web-ui-port` | `MONGOCORE_WEB_UI_PORT` | `27999` | Port for dashboard HTTP server |

The dashboard binds exclusively to `127.0.0.1` — this is not configurable. No authentication is required since localhost access implies trust (same model as Envoy admin, Go pprof).

TOML file configuration:

```toml
[web_ui]
enabled = true
port = 27999
```

### Startup Logging

When the web UI is enabled and starts successfully, MongoCore logs to stdout:

```
Web UI available at http://127.0.0.1:27999
```

This ensures discoverability without users needing to know the feature exists. If the port is in use, log a warning and continue without the dashboard (non-fatal).

## Tech Stack

All assets are embedded in the binary at compile time via `rust-embed`. No npm, no build step, no external CDN dependencies.

| Layer | Library | Size (min+gz) | Purpose |
|-------|---------|---------------|---------|
| CSS | Pico CSS | ~10 KB | Classless styling, automatic dark mode, semantic HTML |
| Interactivity | htmx | ~16 KB | Polling-based fragment updates (`hx-trigger="every 2s"`) |
| Client reactivity | Alpine.js | ~15 KB | Accordion state, tabs, toggles, filters |
| Charts | uPlot | ~48 KB | Real-time time-series (ops/sec, latency, CPU/memory) |
| **Total** | | **~89 KB** | |

## Architecture

```
Browser (localhost:27999)
    │
    ├── GET /              → Full HTML page (embedded static assets)
    ├── GET /assets/*      → JS/CSS libraries (rust-embed, cache-forever)
    ├── GET /api/status    → HTML fragment: status bar
    ├── GET /api/metrics   → HTML fragment: real-time charts data (JSON for uPlot)
    ├── GET /api/operations → HTML fragment: operation breakdown
    ├── GET /api/queries   → HTML fragment: query insights
    ├── GET /api/pipelines → HTML fragment: pipeline & txn pipeline stats
    ├── GET /api/errors    → HTML fragment: recent errors table
    ├── GET /api/ingestion → HTML fragment: ingestion progress
    ├── GET /api/llm       → HTML fragment: LLM usage stats
    └── GET /api/cache     → HTML fragment: cached query stats
```

The main page loads once. htmx polls individual fragment endpoints every 2 seconds, swapping in fresh HTML. uPlot charts receive JSON data and append points client-side.

### Server Implementation

- New Axum router, separate from the MCP server (different port, different purpose)
- Shares `Arc<AppState>` with existing MCP/gRPC servers for access to analytics, config, connection pool
- Spawned conditionally based on `config.web_ui` flag
- Uses `rust-embed` to serve static assets with appropriate cache headers

### Data Sources

| Section | Source | Already exists? |
|---------|--------|-----------------|
| Process stats (uptime, CPU, mem) | `sysinfo` crate or `/proc/self/stat` | No — new |
| Real-time metrics | `AnalyticsCollector` ring buffer | Yes |
| Operation breakdown | `AnalyticsSummary::top_operations` | Yes |
| Query insights | `AnalyticsSummary` + query fingerprints | Yes |
| Pipeline metrics | Pipeline/TransactionPipeline execution tracking | No — new instrumentation |
| Recent errors | `AnalyticsCollector` error events | Yes |
| Ingestion progress | `GetIngestStatus` / `ListIngestJobs` | Yes |
| LLM usage | Compiled query LLM call tracking | No — new instrumentation |
| Cached queries | Compiled query cache stats (L1/L2/L3) | Partial — needs hit/miss counters |

## Page Layout

### Status Bar (always visible, top of page)

```
┌─────────────────────────────────────────────────────────────┐
│ MongoCore Dashboard    ⬤ Connected    ↑ 4h 23m              │
│ cpu: 2.1%  mem: 48MB  ops: 1.2k/s  errors: 0.01%           │
└─────────────────────────────────────────────────────────────┘
```

- MongoCore process uptime, CPU usage, memory usage
- Connection status (connected/disconnected, MongoDB URI masked)
- Summary counters: total ops/sec, error rate

### Main Grid (always visible)

**Real-time Metrics** — Two uPlot time-series charts:
- Operations per second (stacked by type)
- Latency percentiles (p50, p95, p99 as separate lines)
- Rolling window: configurable via buttons — 1m, 5m (default), 15m, 1h

**Operation Breakdown** — Bar chart or table:
- Top operations ranked by count (Find, Insert, Update, Delete, Aggregate, Search)
- Top collections ranked by activity
- Percentage distribution

**Query Insights** — Table:
- Slowest queries (collection, shape, avg latency)
- Most frequent query fingerprints
- Compiled query cache hit rate summary

**Pipeline & Transaction Pipeline** — Stats panel:
- Active/completed pipeline executions
- Average steps per pipeline, step-level latency
- Transaction pipeline: commit/abort rates, retry counts
- Dependent operation success chains

**Recent Errors** — Scrollable table:
- Timestamp, operation type, collection, error message
- Filterable by operation type
- Last 50 errors shown

### Expandable Accordion (collapsed by default)

**Ingestion Progress:**
- Active jobs with progress bars (records processed / total)
- Completed jobs timeline
- Error count, DLQ entries

**LLM Usage:**
- Total calls, tokens in/out
- Latency per call (avg, p95)
- Provider breakdown (Claude / OpenAI)
- Estimated cost
- Success/failure rate

**Cached Queries:**
- Overall hit/miss ratio (with sparkline over time)
- Cache hierarchy breakdown: L1 (in-memory) / L2 (file) / L3 (MongoDB) hit rates
- Cache size (entries, estimated memory)
- Most frequently cached queries
- Recent evictions

## New Instrumentation Required

### Pipeline Metrics

Add counters/timers to `src/operations/pipeline.rs` and `src/operations/transaction_pipeline.rs`:
- Execution count, success/failure
- Per-step latency tracking
- Result forwarding chain length
- Transaction commit/abort/retry counters

### LLM Call Tracking

Add instrumentation to `src/compiled/` LLM provider calls:
- Tokens sent/received per call
- Call latency
- Provider and model used
- Success/failure with error categorization

### Process Metrics

Use `sysinfo` crate (or minimal `/proc/self` reading on Linux, `mach` APIs on macOS):
- CPU usage percentage (process-level)
- RSS memory
- Process uptime (already available via `Instant::now()` at startup)

### Cache Hit Counters

Extend compiled query cache in `src/compiled/` with atomic counters:
- Hits/misses per cache level (L1, L2, L3)
- Total entries per level
- Eviction count

## File Structure

```
src/
├── web_ui/
│   ├── mod.rs           # Module root, server startup, conditional spawn
│   ├── server.rs        # Axum router, routes, handlers
│   ├── handlers.rs      # Individual endpoint handlers (status, metrics, etc.)
│   ├── templates.rs     # HTML fragment generation (Tera or manual)
│   └── assets/          # Static files (embedded via rust-embed)
│       ├── index.html   # Main page shell
│       ├── style.css    # Minimal custom CSS (Pico does most of the work)
│       ├── dashboard.js # Alpine.js components, uPlot init, htmx config
│       ├── pico.min.css
│       ├── htmx.min.js
│       ├── alpine.min.js
│       └── uplot.min.js + uplot.min.css
```

## Rust Dependencies (new)

| Crate | Purpose |
|-------|---------|
| `rust-embed` | Embed static assets in binary |
| `sysinfo` | Process CPU/memory metrics |

Axum, Tera, serde_json, tokio — already in the dependency tree.

## Empty States

On fresh start (no operations yet), the dashboard shows helpful empty states rather than blank panels:

- **Status bar:** Shows "Connected" status and uptime, zeros for ops/sec and error rate
- **Charts:** Empty chart area with centered text: "Waiting for operations..."
- **Tables (operations, queries, errors):** "No data yet — operations will appear here as they occur"
- **Accordions (ingestion, LLM, cache):** Summary shows "No activity" instead of zeros, expandable content shows a brief explanation of what would appear

This ensures the dashboard is immediately useful for confirming MongoCore is running and connected, even before any application traffic flows through it.

## Non-Goals

- No authentication (localhost-only makes it unnecessary)
- No persistent storage of dashboard state (it's a live view)
- No write/mutation operations from the dashboard (read-only)
- No WebSocket complexity (polling is sufficient for localhost)
- No external CDN or network dependencies (fully embedded)
- No mobile-responsive design (developer tool, desktop browser assumed)
