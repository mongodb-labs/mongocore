# Web UI Dashboard

MongoCore includes a built-in diagnostic dashboard that provides real-time visibility into sidecar operations, performance, and health.

## Configuration

| Setting | CLI | Env Var | TOML | Default |
|---------|-----|---------|------|---------|
| Enable/disable | `--web-ui` | `MONGOCORE_WEB_UI` | `[web_ui] enabled` | `true` |
| Port | `--web-ui-port` | `MONGOCORE_WEB_UI_PORT` | `[web_ui] port` | `27999` |

TOML example:

```toml
[web_ui]
enabled = true
port = 27999
```

The dashboard binds exclusively to `127.0.0.1` — it is not accessible from other machines. No authentication is required since localhost access implies trust.

## Accessing the Dashboard

After starting MongoCore, open in your browser:

```
http://127.0.0.1:27999
```

MongoCore logs the URL on startup:

```
Web UI available at http://127.0.0.1:27999
```

If the port is already in use, MongoCore logs a warning and continues without the dashboard (non-fatal).

## Dashboard Sections

### Always Visible

**Status Bar** — Connection status, process uptime, CPU and memory usage, total operations count, error rate.

**Real-time Charts** — Two time-series charts showing operations/sec and latency percentiles (p50, p95, p99). Time window is configurable: 1m, 5m (default), 15m, 1h.

**Top Operations** — Operation counts by type (Find, Insert, Update, Delete, Aggregate, Search, etc.) and top namespaces (database.collection) by activity.

### Expandable (click to open)

**Slowest Queries** — Queries ranked by latency with their fingerprint (shape) and namespace.

**Pipelines** — Pipeline and transaction pipeline execution stats: total count, success rate, average steps, average latency, retry counts.

**Recent Errors** — Last 50 failed operations with operation type and namespace.

**Ingestion** — Active ingestion job progress bars, records processed, and error counts.

**LLM Usage** — Compiled query LLM call stats: total calls, success rate, average latency, token counts.

**Cache** — Compiled query cache performance: L1 (in-memory), L2 (disk), and L3 (MongoDB) hit rates, cache size, and eviction counts.

## Data Sources

The dashboard shows a unified view of operations from both:

- **gRPC clients** (Python, TypeScript, Go, Java) — operations recorded in the gRPC service layer
- **MCP tool calls** (AI agents, Claude) — operations recorded in the MCP tool execution layer

Both paths write to the same analytics collector, so the dashboard reflects all traffic through MongoCore regardless of protocol.

## Tech Stack

The dashboard is fully self-contained with no external dependencies:

| Library | Size | Purpose |
|---------|------|---------|
| Pico CSS | ~10 KB | Dark theme, semantic HTML styling |
| htmx | ~16 KB | Automatic polling for fresh data |
| Alpine.js | ~15 KB | Client-side state (time window, accordions) |
| uPlot | ~48 KB | High-performance time-series charts |

All assets are compiled into the binary at build time via `rust-embed`. No CDN, no npm, no build step.

## Disabling the Dashboard

```bash
mongocore --web-ui=false
```

Or via environment variable:

```bash
MONGOCORE_WEB_UI=false mongocore
```
