# Web UI Dashboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

**Goal:** Add a single-page diagnostic web dashboard to MongoCore, served on localhost:27999, showing real-time metrics, operations, pipelines, LLM usage, and cache stats.

**Architecture:** A new `web_ui` module with its own Axum HTTP server (separate port from MCP). Static assets (Pico CSS, htmx, Alpine.js, uPlot) are embedded in the binary via `rust-embed`. The dashboard polls HTML fragment endpoints every 2s via htmx. New instrumentation types track pipeline metrics, LLM usage, and cache hit/miss counters.

**Tech Stack:** Rust (axum, rust-embed, sysinfo), Pico CSS, htmx, Alpine.js, uPlot

---

## File Structure

```
src/
├── web_ui/
│   ├── mod.rs           — Module root, conditional server spawn, start_web_ui_server()
│   ├── server.rs        — Axum router, routes, static asset serving
│   ├── handlers.rs      — Endpoint handlers (/api/status, /api/metrics, etc.)
│   └── assets/          — Static files (embedded via rust-embed)
│       ├── index.html   — Main page shell (single HTML page)
│       ├── style.css    — Custom CSS (minimal, Pico does most work)
│       └── dashboard.js — Alpine.js components, uPlot charts, htmx config
├── config.rs            — Add web_ui_enabled + web_ui_port fields
├── defaults.rs          — Add DEFAULT_WEB_UI_PORT, DEFAULT_WEB_UI_ENABLED
├── lib.rs               — Add `pub mod web_ui;`
├── main.rs              — Conditionally spawn web UI server
├── analytics/
│   ├── types.rs         — Add LlmCallEvent, PipelineMetricsEvent
│   └── collector.rs     — Add record_llm_call(), record_pipeline(), snapshot methods
└── compiled/
    └── cache/mod.rs     — Add atomic hit/miss/eviction counters
```

---

## Task 1: Add Dependencies (rust-embed, sysinfo)

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add rust-embed and sysinfo to Cargo.toml**

Add under `[dependencies]`:

```toml
rust-embed = "8"
sysinfo = { version = "0.33", default-features = false, features = ["system"] }
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build 2>&1 | grep "warning:" | head -5`
Expected: No warnings (new deps only, no code yet)

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: add rust-embed and sysinfo dependencies for web UI"
```

---

## Task 2: Config — Add Web UI Fields

**Files:**
- Modify: `src/defaults.rs`
- Modify: `src/config.rs`

- [ ] **Step 1: Add defaults**

In `src/defaults.rs`, add:

```rust
/// Default Web UI port.
pub const DEFAULT_WEB_UI_PORT: u16 = 27999;

/// Whether the Web UI is enabled by default.
pub const DEFAULT_WEB_UI_ENABLED: bool = true;
```

- [ ] **Step 2: Add CLI args to CliArgs**

In `src/config.rs`, add to `CliArgs`:

```rust
    /// Enable web UI dashboard
    #[arg(long, env = "MONGOCORE_WEB_UI")]
    pub web_ui: Option<bool>,

    /// Web UI port
    #[arg(long, env = "MONGOCORE_WEB_UI_PORT")]
    pub web_ui_port: Option<u16>,
```

- [ ] **Step 3: Add to FileConfig**

In `src/config.rs`, add a TOML section struct and field:

```rust
/// Web UI configuration from TOML.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct WebUiFileConfig {
    pub enabled: Option<bool>,
    pub port: Option<u16>,
}
```

Add to `FileConfig`:

```rust
    pub web_ui: Option<WebUiFileConfig>,
```

- [ ] **Step 4: Add to resolved Config struct**

Add to `Config`:

```rust
    pub web_ui_enabled: bool,
    pub web_ui_port: u16,
```

- [ ] **Step 5: Add resolution logic in Config::load()**

After the existing config resolution, add:

```rust
        let web_ui_file = file_config.web_ui.unwrap_or_default();
        let web_ui_enabled = cli
            .web_ui
            .or(web_ui_file.enabled)
            .unwrap_or(DEFAULT_WEB_UI_ENABLED);
        let web_ui_port = cli
            .web_ui_port
            .or(web_ui_file.port)
            .unwrap_or(DEFAULT_WEB_UI_PORT);
```

And include `web_ui_enabled` and `web_ui_port` in the `Config` struct literal.

- [ ] **Step 6: Update ALL Config struct literals in tests**

Search for `Config {` across `src/` and `tests/` and add the new fields:

```bash
grep -rn "Config {" src/ tests/
```

Add `web_ui_enabled: true, web_ui_port: 27999,` (or `DEFAULT_WEB_UI_ENABLED` / `DEFAULT_WEB_UI_PORT`) to each.

- [ ] **Step 7: Verify it compiles with zero warnings**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output

- [ ] **Step 8: Run unit tests**

Run: `cargo test --lib`
Expected: All pass

- [ ] **Step 9: Commit**

```bash
git add src/defaults.rs src/config.rs tests/
git commit -m "feat(config): add web_ui_enabled and web_ui_port configuration"
```

---

## Task 3: Instrumentation Types — LLM, Pipeline, Cache Counters

**Files:**
- Modify: `src/analytics/types.rs`
- Modify: `src/analytics/collector.rs`
- Modify: `src/analytics/mod.rs`
- Modify: `src/compiled/cache/mod.rs`
- Modify: `src/compiled/cache/memory.rs`

- [ ] **Step 1: Add LlmCallEvent and PipelineEvent to analytics/types.rs**

```rust
/// Tracks a single LLM API call for the dashboard.
#[derive(Debug, Clone)]
pub struct LlmCallEvent {
    pub provider: String,
    pub model: String,
    pub tokens_in: u32,
    pub tokens_out: u32,
    pub latency: Duration,
    pub success: bool,
    pub timestamp: Instant,
}

/// Tracks a pipeline or transaction pipeline execution.
#[derive(Debug, Clone)]
pub struct PipelineEvent {
    pub is_transaction: bool,
    pub steps: usize,
    pub latency: Duration,
    pub success: bool,
    pub retries: u32,
    pub timestamp: Instant,
}
```

- [ ] **Step 2: Add LLM and pipeline recording to AnalyticsCollector**

In `src/analytics/collector.rs`, add fields and methods:

```rust
use crate::analytics::types::{LlmCallEvent, PipelineEvent};
use std::sync::Mutex;

// Add to AnalyticsCollector struct:
    llm_calls: Mutex<Vec<LlmCallEvent>>,
    pipeline_events: Mutex<Vec<PipelineEvent>>,

// Add to new():
    llm_calls: Mutex::new(Vec::new()),
    pipeline_events: Mutex::new(Vec::new()),

// Add methods:
    pub fn record_llm_call(&self, event: LlmCallEvent) {
        let mut calls = self.llm_calls.lock().unwrap();
        if calls.len() >= 1000 {
            calls.remove(0);
        }
        calls.push(event);
    }

    pub fn llm_calls_snapshot(&self) -> Vec<LlmCallEvent> {
        self.llm_calls.lock().unwrap().clone()
    }

    pub fn record_pipeline(&self, event: PipelineEvent) {
        let mut events = self.pipeline_events.lock().unwrap();
        if events.len() >= 1000 {
            events.remove(0);
        }
        events.push(event);
    }

    pub fn pipeline_events_snapshot(&self) -> Vec<PipelineEvent> {
        self.pipeline_events.lock().unwrap().clone()
    }
```

- [ ] **Step 3: Export new types from analytics/mod.rs**

```rust
pub use types::{AnalyticsEvent, LlmCallEvent, OperationKind, PipelineEvent, QueryFingerprint};
```

- [ ] **Step 4: Add atomic cache counters to CacheHierarchy**

In `src/compiled/cache/mod.rs`, add:

```rust
use std::sync::atomic::{AtomicU64, Ordering};

// Add to CacheHierarchy struct:
    pub l1_hits: AtomicU64,
    pub l1_misses: AtomicU64,
    pub l2_hits: AtomicU64,
    pub l2_misses: AtomicU64,
    pub l3_hits: AtomicU64,
    pub l3_misses: AtomicU64,
    pub evictions: AtomicU64,
```

Initialize all to `AtomicU64::new(0)` in `new()`.

Update the `get()` method to increment counters on hit/miss at each level.

Add a method:

```rust
    pub fn cache_stats(&self) -> CacheStats {
        CacheStats {
            l1_hits: self.l1_hits.load(Ordering::Relaxed),
            l1_misses: self.l1_misses.load(Ordering::Relaxed),
            l2_hits: self.l2_hits.load(Ordering::Relaxed),
            l2_misses: self.l2_misses.load(Ordering::Relaxed),
            l3_hits: self.l3_hits.load(Ordering::Relaxed),
            l3_misses: self.l3_misses.load(Ordering::Relaxed),
            evictions: self.evictions.load(Ordering::Relaxed),
            l1_size: self.l1_size(),
        }
    }
```

```rust
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub l1_hits: u64,
    pub l1_misses: u64,
    pub l2_hits: u64,
    pub l2_misses: u64,
    pub l3_hits: u64,
    pub l3_misses: u64,
    pub evictions: u64,
    pub l1_size: usize,
}
```

- [ ] **Step 5: Add eviction counter increment in MemoryCache**

In `src/compiled/cache/memory.rs`, if there's eviction logic (or in the L1 `put()` if it has a capacity limit), increment `evictions`. If MemoryCache doesn't evict (it's a DashMap), skip this — the counter stays at 0 until we add LRU later.

- [ ] **Step 6: Verify it compiles with zero warnings**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output (use `#[allow(dead_code)]` on new types only if nothing uses them yet — but handlers in Task 5 will use them, so they shouldn't be dead)

- [ ] **Step 7: Run unit tests**

Run: `cargo test --lib`
Expected: All pass

- [ ] **Step 8: Commit**

```bash
git add src/analytics/ src/compiled/cache/
git commit -m "feat(analytics): add LLM call, pipeline event, and cache hit/miss instrumentation"
```

---

## Task 4: Web UI Module — Server and Static Assets

**Files:**
- Create: `src/web_ui/mod.rs`
- Create: `src/web_ui/server.rs`
- Create: `src/web_ui/handlers.rs`
- Create: `src/web_ui/assets/index.html`
- Create: `src/web_ui/assets/style.css`
- Create: `src/web_ui/assets/dashboard.js`
- Modify: `src/lib.rs`

- [ ] **Step 1: Download vendor libraries into assets/**

Download minified versions of Pico CSS, htmx, Alpine.js, and uPlot into `src/web_ui/assets/`:

```bash
mkdir -p src/web_ui/assets
curl -sL "https://cdn.jsdelivr.net/npm/@picocss/pico@2/css/pico.min.css" -o src/web_ui/assets/pico.min.css
curl -sL "https://unpkg.com/htmx.org@2.0.4/dist/htmx.min.js" -o src/web_ui/assets/htmx.min.js
curl -sL "https://cdn.jsdelivr.net/npm/alpinejs@3/dist/cdn.min.js" -o src/web_ui/assets/alpine.min.js
curl -sL "https://cdn.jsdelivr.net/npm/uplot@1.6.31/dist/uPlot.iife.min.js" -o src/web_ui/assets/uplot.min.js
curl -sL "https://cdn.jsdelivr.net/npm/uplot@1.6.31/dist/uPlot.min.css" -o src/web_ui/assets/uplot.min.css
```

- [ ] **Step 2: Create src/web_ui/assets/index.html**

The main page shell — loads all assets, sets up htmx polling targets, Alpine.js components for accordion state, and uPlot chart containers:

```html
<!DOCTYPE html>
<html lang="en" data-theme="dark">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>MongoCore Dashboard</title>
    <link rel="stylesheet" href="/assets/pico.min.css">
    <link rel="stylesheet" href="/assets/uplot.min.css">
    <link rel="stylesheet" href="/assets/style.css">
</head>
<body x-data="dashboard()">
    <!-- Status Bar -->
    <header class="container-fluid" id="status-bar"
            hx-get="/api/status" hx-trigger="every 2s" hx-swap="innerHTML">
        <p>Loading...</p>
    </header>

    <main class="container-fluid">
        <!-- Time Window Selector -->
        <nav class="time-controls">
            <button :class="{'selected': window === '1m'}" @click="setWindow('1m')">1m</button>
            <button :class="{'selected': window === '5m'}" @click="setWindow('5m')">5m</button>
            <button :class="{'selected': window === '15m'}" @click="setWindow('15m')">15m</button>
            <button :class="{'selected': window === '1h'}" @click="setWindow('1h')">1h</button>
        </nav>

        <!-- Real-time Charts -->
        <section class="grid">
            <article>
                <header>Operations/sec</header>
                <div id="chart-ops"></div>
            </article>
            <article>
                <header>Latency (ms)</header>
                <div id="chart-latency"></div>
            </article>
        </section>

        <!-- Operation Breakdown -->
        <section>
            <article id="operations-panel"
                     hx-get="/api/operations" hx-trigger="every 2s" hx-swap="innerHTML">
                <p>Loading operations...</p>
            </article>
        </section>

        <!-- Query Insights -->
        <section>
            <article id="queries-panel"
                     hx-get="/api/queries" hx-trigger="every 2s" hx-swap="innerHTML">
                <p>Loading query insights...</p>
            </article>
        </section>

        <!-- Pipeline Stats -->
        <section>
            <article id="pipelines-panel"
                     hx-get="/api/pipelines" hx-trigger="every 2s" hx-swap="innerHTML">
                <p>Loading pipeline stats...</p>
            </article>
        </section>

        <!-- Recent Errors -->
        <section>
            <article id="errors-panel"
                     hx-get="/api/errors" hx-trigger="every 2s" hx-swap="innerHTML">
                <p>Loading errors...</p>
            </article>
        </section>

        <!-- Expandable Accordion Sections -->
        <details x-data="{ open: false }">
            <summary>Ingestion Progress</summary>
            <div id="ingestion-panel"
                 hx-get="/api/ingestion" hx-trigger="every 5s" hx-swap="innerHTML">
                <p>No activity</p>
            </div>
        </details>

        <details x-data="{ open: false }">
            <summary>LLM Usage</summary>
            <div id="llm-panel"
                 hx-get="/api/llm" hx-trigger="every 5s" hx-swap="innerHTML">
                <p>No activity</p>
            </div>
        </details>

        <details x-data="{ open: false }">
            <summary>Cached Queries</summary>
            <div id="cache-panel"
                 hx-get="/api/cache" hx-trigger="every 5s" hx-swap="innerHTML">
                <p>No activity</p>
            </div>
        </details>
    </main>

    <script src="/assets/htmx.min.js"></script>
    <script src="/assets/alpine.min.js" defer></script>
    <script src="/assets/uplot.min.js"></script>
    <script src="/assets/dashboard.js"></script>
</body>
</html>
```

- [ ] **Step 3: Create src/web_ui/assets/style.css**

Minimal custom CSS for layout tweaks:

```css
:root {
    --spacing: 0.75rem;
}

header.container-fluid {
    background: var(--pico-card-background-color);
    padding: var(--spacing);
    border-bottom: 1px solid var(--pico-muted-border-color);
    position: sticky;
    top: 0;
    z-index: 10;
}

.status-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--spacing);
    margin: 0;
}

.status-row .indicator {
    display: inline-block;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--pico-ins-color);
}

.status-row .indicator.disconnected {
    background: var(--pico-del-color);
}

.time-controls {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 1rem;
}

.time-controls button {
    padding: 0.25rem 0.75rem;
    width: auto;
}

.time-controls button.selected {
    background: var(--pico-primary);
    color: var(--pico-primary-inverse);
}

.grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
}

.empty-state {
    text-align: center;
    padding: 2rem;
    color: var(--pico-muted-color);
    font-style: italic;
}

details {
    margin-bottom: 1rem;
}

#chart-ops, #chart-latency {
    height: 200px;
}

table {
    font-size: 0.875rem;
}
```

- [ ] **Step 4: Create src/web_ui/assets/dashboard.js**

Alpine.js component + uPlot initialization + metrics polling:

```javascript
// Dashboard state and chart management
function dashboard() {
    return {
        window: '5m',
        opsChart: null,
        latencyChart: null,
        metricsInterval: null,

        init() {
            this.initCharts();
            this.startMetricsPolling();
        },

        setWindow(w) {
            this.window = w;
            this.fetchMetrics();
        },

        initCharts() {
            const opsEl = document.getElementById('chart-ops');
            const latEl = document.getElementById('chart-latency');
            if (!opsEl || !latEl) return;

            const baseOpts = {
                width: opsEl.clientWidth,
                height: 200,
                cursor: { show: true },
                axes: [{ stroke: '#888' }, { stroke: '#888' }],
            };

            this.opsChart = new uPlot({
                ...baseOpts,
                series: [
                    {},
                    { label: 'ops/s', stroke: '#6366f1', fill: 'rgba(99,102,241,0.1)' },
                ],
                scales: { x: { time: true }, y: { min: 0 } },
            }, [[],[]], opsEl);

            this.latencyChart = new uPlot({
                ...baseOpts,
                width: latEl.clientWidth,
                series: [
                    {},
                    { label: 'p50', stroke: '#22c55e' },
                    { label: 'p95', stroke: '#eab308' },
                    { label: 'p99', stroke: '#ef4444' },
                ],
                scales: { x: { time: true }, y: { min: 0 } },
            }, [[],[],[],[]], latEl);
        },

        startMetricsPolling() {
            this.fetchMetrics();
            this.metricsInterval = setInterval(() => this.fetchMetrics(), 2000);
        },

        async fetchMetrics() {
            try {
                const resp = await fetch(`/api/metrics?window=${this.window}`);
                if (!resp.ok) return;
                const data = await resp.json();
                if (data.timestamps && data.timestamps.length > 0) {
                    this.opsChart.setData([data.timestamps, data.ops_per_sec]);
                    this.latencyChart.setData([data.timestamps, data.p50, data.p95, data.p99]);
                }
            } catch (e) {
                // Silently retry on next interval
            }
        },
    };
}
```

- [ ] **Step 5: Create src/web_ui/mod.rs**

```rust
pub mod handlers;
pub mod server;

use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{info, warn};

use crate::analytics::AnalyticsCollector;
use crate::compiled::translator::CompiledQueryTranslator;
use crate::config::Config;
use crate::connection::pool::ConnectionPool;

pub struct WebUiState {
    pub analytics: Option<Arc<AnalyticsCollector>>,
    pub pool: ConnectionPool,
    pub config: Config,
    pub translator: Option<Arc<CompiledQueryTranslator>>,
    pub start_time: std::time::Instant,
}

pub fn start_web_ui_server(
    config: &Config,
    pool: ConnectionPool,
    analytics: Option<Arc<AnalyticsCollector>>,
    translator: Option<Arc<CompiledQueryTranslator>>,
) -> Option<JoinHandle<()>> {
    if !config.web_ui_enabled {
        return None;
    }

    let state = Arc::new(WebUiState {
        analytics,
        pool,
        config: config.clone(),
        translator,
        start_time: std::time::Instant::now(),
    });

    let port = config.web_ui_port;

    let handle = tokio::spawn(async move {
        match server::create_router(state).await {
            Ok(app) => {
                let addr = format!("127.0.0.1:{}", port);
                match tokio::net::TcpListener::bind(&addr).await {
                    Ok(listener) => {
                        info!("Web UI available at http://{}", addr);
                        if let Err(e) = axum::serve(listener, app).await {
                            warn!("Web UI server error: {}", e);
                        }
                    }
                    Err(e) => {
                        warn!("Web UI: failed to bind port {} ({}), continuing without dashboard", port, e);
                    }
                }
            }
            Err(e) => {
                warn!("Web UI: failed to create router ({}), continuing without dashboard", e);
            }
        }
    });

    Some(handle)
}
```

- [ ] **Step 6: Create src/web_ui/server.rs**

```rust
use std::sync::Arc;

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use rust_embed::Embed;

use super::handlers;
use super::WebUiState;

#[derive(Embed)]
#[folder = "src/web_ui/assets/"]
struct Assets;

pub async fn create_router(state: Arc<WebUiState>) -> Result<Router, String> {
    let app = Router::new()
        .route("/", get(serve_index))
        .route("/assets/{*path}", get(serve_asset))
        .route("/api/status", get(handlers::status))
        .route("/api/metrics", get(handlers::metrics))
        .route("/api/operations", get(handlers::operations))
        .route("/api/queries", get(handlers::queries))
        .route("/api/pipelines", get(handlers::pipelines))
        .route("/api/errors", get(handlers::errors))
        .route("/api/ingestion", get(handlers::ingestion))
        .route("/api/llm", get(handlers::llm))
        .route("/api/cache", get(handlers::cache))
        .with_state(state);

    Ok(app)
}

async fn serve_index() -> impl IntoResponse {
    match Assets::get("index.html") {
        Some(content) => Html(String::from_utf8_lossy(content.data.as_ref()).to_string()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn serve_asset(axum::extract::Path(path): axum::extract::Path<String>) -> impl IntoResponse {
    match Assets::get(&path) {
        Some(content) => {
            let mime = mime_from_path(&path);
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime)
                .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
                .body(axum::body::Body::from(content.data.to_vec()))
                .unwrap()
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn mime_from_path(path: &str) -> &'static str {
    if path.ends_with(".css") {
        "text/css"
    } else if path.ends_with(".js") {
        "application/javascript"
    } else if path.ends_with(".html") {
        "text/html"
    } else {
        "application/octet-stream"
    }
}
```

- [ ] **Step 7: Create src/web_ui/handlers.rs (stub implementations)**

```rust
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::response::Html;
use serde::Deserialize;

use crate::analytics::aggregator::aggregate;
use super::WebUiState;

#[derive(Deserialize)]
pub struct MetricsQuery {
    pub window: Option<String>,
}

pub async fn status(State(state): State<Arc<WebUiState>>) -> Html<String> {
    let uptime = state.start_time.elapsed();
    let uptime_str = format_duration(uptime);

    let (total_ops, total_errors) = match &state.analytics {
        Some(a) => (a.total_operations(), a.total_errors()),
        None => (0, 0),
    };

    let error_rate = if total_ops > 0 {
        (total_errors as f64 / total_ops as f64) * 100.0
    } else {
        0.0
    };

    // Process stats via sysinfo
    let (cpu, mem_mb) = get_process_stats();

    Html(format!(
        r#"<div class="status-row">
            <span><strong>MongoCore Dashboard</strong></span>
            <span><span class="indicator"></span> Connected</span>
            <span>↑ {uptime_str}</span>
            <span>cpu: {cpu:.1}%</span>
            <span>mem: {mem_mb:.0}MB</span>
            <span>ops: {total_ops}</span>
            <span>errors: {error_rate:.2}%</span>
        </div>"#
    ))
}

pub async fn metrics(
    State(state): State<Arc<WebUiState>>,
    Query(params): Query<MetricsQuery>,
) -> axum::Json<serde_json::Value> {
    let window_secs = parse_window(&params.window.unwrap_or_else(|| "5m".to_string()));
    let now = std::time::Instant::now();

    let events = match &state.analytics {
        Some(a) => a.snapshot(),
        None => vec![],
    };

    // Filter events within window
    let filtered: Vec<_> = events
        .iter()
        .filter(|e| now.duration_since(e.timestamp) <= Duration::from_secs(window_secs))
        .collect();

    // Bucket into 2-second intervals for time-series
    let bucket_secs = 2u64;
    let num_buckets = (window_secs / bucket_secs) as usize;
    let mut timestamps = Vec::with_capacity(num_buckets);
    let mut ops_per_sec = Vec::with_capacity(num_buckets);
    let mut p50 = Vec::with_capacity(num_buckets);
    let mut p95 = Vec::with_capacity(num_buckets);
    let mut p99 = Vec::with_capacity(num_buckets);

    let base_time = now - Duration::from_secs(window_secs);
    for i in 0..num_buckets {
        let bucket_start = base_time + Duration::from_secs(i as u64 * bucket_secs);
        let bucket_end = bucket_start + Duration::from_secs(bucket_secs);

        let bucket_events: Vec<_> = filtered
            .iter()
            .filter(|e| e.timestamp >= bucket_start && e.timestamp < bucket_end)
            .copied()
            .cloned()
            .collect();

        let ts = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64)
            - (window_secs as i64 - (i as i64 * bucket_secs as i64));

        timestamps.push(ts);
        ops_per_sec.push(bucket_events.len() as f64 / bucket_secs as f64);

        if bucket_events.is_empty() {
            p50.push(0.0);
            p95.push(0.0);
            p99.push(0.0);
        } else {
            let summary = aggregate(&bucket_events);
            p50.push(summary.p50_latency_ms);
            p95.push(summary.p95_latency_ms);
            p99.push(summary.p99_latency_ms);
        }
    }

    axum::Json(serde_json::json!({
        "timestamps": timestamps,
        "ops_per_sec": ops_per_sec,
        "p50": p50,
        "p95": p95,
        "p99": p99,
    }))
}

pub async fn operations(State(state): State<Arc<WebUiState>>) -> Html<String> {
    let events = match &state.analytics {
        Some(a) => a.snapshot(),
        None => return Html(r#"<p class="empty-state">No data yet — operations will appear here as they occur</p>"#.to_string()),
    };

    if events.is_empty() {
        return Html(r#"<p class="empty-state">No data yet — operations will appear here as they occur</p>"#.to_string());
    }

    let summary = aggregate(&events);
    let mut html = String::from("<header>Operation Breakdown</header><table><thead><tr><th>Operation</th><th>Count</th><th>%</th></tr></thead><tbody>");

    for (op, count) in &summary.top_operations {
        let pct = (*count as f64 / summary.total_operations as f64) * 100.0;
        html.push_str(&format!("<tr><td>{:?}</td><td>{}</td><td>{:.1}%</td></tr>", op, count, pct));
    }
    html.push_str("</tbody></table>");

    if !summary.top_collections.is_empty() {
        html.push_str("<h6>Top Collections</h6><table><thead><tr><th>Collection</th><th>Count</th></tr></thead><tbody>");
        for (coll, count) in &summary.top_collections {
            html.push_str(&format!("<tr><td>{}</td><td>{}</td></tr>", coll, count));
        }
        html.push_str("</tbody></table>");
    }

    Html(html)
}

pub async fn queries(State(state): State<Arc<WebUiState>>) -> Html<String> {
    let events = match &state.analytics {
        Some(a) => a.snapshot(),
        None => return Html(r#"<p class="empty-state">No data yet — operations will appear here as they occur</p>"#.to_string()),
    };

    if events.is_empty() {
        return Html(r#"<p class="empty-state">No data yet — operations will appear here as they occur</p>"#.to_string());
    }

    // Find slowest queries and most frequent fingerprints
    let mut with_fingerprint: Vec<_> = events.iter().filter(|e| e.fingerprint.is_some()).collect();
    with_fingerprint.sort_by(|a, b| b.latency.cmp(&a.latency));

    let mut html = String::from("<header>Query Insights</header>");
    html.push_str("<h6>Slowest Queries</h6><table><thead><tr><th>Collection</th><th>Shape</th><th>Latency</th></tr></thead><tbody>");

    for event in with_fingerprint.iter().take(10) {
        let fp = event.fingerprint.as_ref().unwrap();
        html.push_str(&format!(
            "<tr><td>{}.{}</td><td><code>{}</code></td><td>{:.1}ms</td></tr>",
            event.database, event.collection, fp.as_str(), event.latency.as_secs_f64() * 1000.0
        ));
    }
    html.push_str("</tbody></table>");

    Html(html)
}

pub async fn pipelines(State(state): State<Arc<WebUiState>>) -> Html<String> {
    let events = match &state.analytics {
        Some(a) => a.pipeline_events_snapshot(),
        None => return Html(r#"<p class="empty-state">No pipeline executions yet</p>"#.to_string()),
    };

    if events.is_empty() {
        return Html(r#"<p class="empty-state">No pipeline executions yet</p>"#.to_string());
    }

    let total = events.len();
    let successes = events.iter().filter(|e| e.success).count();
    let txn_count = events.iter().filter(|e| e.is_transaction).count();
    let avg_steps = events.iter().map(|e| e.steps).sum::<usize>() as f64 / total as f64;
    let avg_latency = events.iter().map(|e| e.latency.as_millis()).sum::<u128>() as f64 / total as f64;
    let total_retries: u32 = events.iter().map(|e| e.retries).sum();

    Html(format!(
        r#"<header>Pipeline & Transaction Pipeline</header>
        <table>
            <tbody>
                <tr><td>Total executions</td><td>{total}</td></tr>
                <tr><td>Success rate</td><td>{:.1}%</td></tr>
                <tr><td>Transaction pipelines</td><td>{txn_count}</td></tr>
                <tr><td>Avg steps</td><td>{avg_steps:.1}</td></tr>
                <tr><td>Avg latency</td><td>{avg_latency:.0}ms</td></tr>
                <tr><td>Total retries</td><td>{total_retries}</td></tr>
            </tbody>
        </table>"#,
        (successes as f64 / total as f64) * 100.0
    ))
}

pub async fn errors(State(state): State<Arc<WebUiState>>) -> Html<String> {
    let events = match &state.analytics {
        Some(a) => a.snapshot(),
        None => return Html(r#"<p class="empty-state">No errors recorded</p>"#.to_string()),
    };

    let errors: Vec<_> = events.iter().filter(|e| !e.success).rev().take(50).collect();

    if errors.is_empty() {
        return Html(r#"<p class="empty-state">No errors recorded</p>"#.to_string());
    }

    let mut html = String::from("<header>Recent Errors</header><table><thead><tr><th>Operation</th><th>Collection</th></tr></thead><tbody>");
    for event in &errors {
        html.push_str(&format!(
            "<tr><td>{:?}</td><td>{}.{}</td></tr>",
            event.operation, event.database, event.collection
        ));
    }
    html.push_str("</tbody></table>");

    Html(html)
}

pub async fn ingestion(State(_state): State<Arc<WebUiState>>) -> Html<String> {
    // TODO: Wire up to ingestion engine state when available in WebUiState
    Html(r#"<p class="empty-state">No active ingestion jobs</p>"#.to_string())
}

pub async fn llm(State(state): State<Arc<WebUiState>>) -> Html<String> {
    let calls = match &state.analytics {
        Some(a) => a.llm_calls_snapshot(),
        None => return Html(r#"<p class="empty-state">No LLM calls recorded</p>"#.to_string()),
    };

    if calls.is_empty() {
        return Html(r#"<p class="empty-state">No LLM calls recorded</p>"#.to_string());
    }

    let total = calls.len();
    let successes = calls.iter().filter(|c| c.success).count();
    let total_tokens_in: u32 = calls.iter().map(|c| c.tokens_in).sum();
    let total_tokens_out: u32 = calls.iter().map(|c| c.tokens_out).sum();
    let avg_latency = calls.iter().map(|c| c.latency.as_millis()).sum::<u128>() as f64 / total as f64;

    Html(format!(
        r#"<header>LLM Usage</header>
        <table>
            <tbody>
                <tr><td>Total calls</td><td>{total}</td></tr>
                <tr><td>Success rate</td><td>{:.1}%</td></tr>
                <tr><td>Tokens in</td><td>{total_tokens_in}</td></tr>
                <tr><td>Tokens out</td><td>{total_tokens_out}</td></tr>
                <tr><td>Avg latency</td><td>{avg_latency:.0}ms</td></tr>
            </tbody>
        </table>"#,
        (successes as f64 / total as f64) * 100.0
    ))
}

pub async fn cache(State(state): State<Arc<WebUiState>>) -> Html<String> {
    let stats = match &state.translator {
        Some(t) => t.cache_stats(),
        None => return Html(r#"<p class="empty-state">Compiled query cache not active</p>"#.to_string()),
    };

    let total_lookups = stats.l1_hits + stats.l1_misses;
    let hit_rate = if total_lookups > 0 {
        (stats.l1_hits as f64 / total_lookups as f64) * 100.0
    } else {
        0.0
    };

    Html(format!(
        r#"<header>Cached Queries</header>
        <table>
            <tbody>
                <tr><td>Overall hit rate</td><td>{hit_rate:.1}%</td></tr>
                <tr><td>L1 (memory) hits/misses</td><td>{} / {}</td></tr>
                <tr><td>L2 (disk) hits/misses</td><td>{} / {}</td></tr>
                <tr><td>L3 (MongoDB) hits/misses</td><td>{} / {}</td></tr>
                <tr><td>L1 entries</td><td>{}</td></tr>
                <tr><td>Evictions</td><td>{}</td></tr>
            </tbody>
        </table>"#,
        stats.l1_hits, stats.l1_misses,
        stats.l2_hits, stats.l2_misses,
        stats.l3_hits, stats.l3_misses,
        stats.l1_size,
        stats.evictions,
    ))
}

fn get_process_stats() -> (f64, f64) {
    use sysinfo::{Pid, System};
    let pid = Pid::from_u32(std::process::id());
    let mut sys = System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]), true);
    match sys.process(pid) {
        Some(proc) => (proc.cpu_usage() as f64, proc.memory() as f64 / 1_048_576.0),
        None => (0.0, 0.0),
    }
}

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

fn parse_window(w: &str) -> u64 {
    match w {
        "1m" => 60,
        "5m" => 300,
        "15m" => 900,
        "1h" => 3600,
        _ => 300,
    }
}
```

- [ ] **Step 8: Add `pub mod web_ui;` to src/lib.rs**

- [ ] **Step 9: Verify it compiles with zero warnings**

Run: `cargo build 2>&1 | grep "warning:"`

Fix any issues (unused imports, dead code on `cache_stats` method on translator needs to be exposed — see Task 5).

- [ ] **Step 10: Commit**

```bash
git add src/web_ui/ src/lib.rs
git commit -m "feat(web-ui): add web UI module with embedded dashboard and API handlers"
```

---

## Task 5: Expose cache_stats on CompiledQueryTranslator

**Files:**
- Modify: `src/compiled/translator.rs`
- Modify: `src/compiled/cache/mod.rs`

- [ ] **Step 1: Add public cache_stats() method to CompiledQueryTranslator**

In `src/compiled/translator.rs`, add:

```rust
use super::cache::CacheStats;

impl CompiledQueryTranslator {
    pub fn cache_stats(&self) -> CacheStats {
        self.cache.cache_stats()
    }
}
```

- [ ] **Step 2: Export CacheStats from compiled module**

Ensure `CacheStats` is accessible from `src/compiled/cache/mod.rs` (it's already defined there from Task 3). Add to `src/compiled/mod.rs`:

```rust
pub use cache::CacheStats;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build 2>&1 | grep "warning:"`

- [ ] **Step 4: Commit**

```bash
git add src/compiled/
git commit -m "feat(compiled): expose cache_stats() for web UI dashboard"
```

---

## Task 6: Wire Web UI into main.rs (with full state)

**Files:**
- Modify: `src/main.rs`
- Modify: `src/web_ui/mod.rs`

- [ ] **Step 1: Update WebUiState to include ingestion and connection health**

In `src/web_ui/mod.rs`, update `WebUiState`:

```rust
use crate::ingestion::engine::IngestionEngine;
use crate::ingestion::watch::DirectoryWatcher;

pub struct WebUiState {
    pub analytics: Option<Arc<AnalyticsCollector>>,
    pub pool: ConnectionPool,
    pub config: Config,
    pub translator: Option<Arc<CompiledQueryTranslator>>,
    pub ingestion_engine: Option<Arc<IngestionEngine>>,
    pub directory_watcher: Option<Arc<DirectoryWatcher>>,
    pub start_time: std::time::Instant,
}
```

Update `start_web_ui_server` signature to accept these additional parameters:

```rust
pub fn start_web_ui_server(
    config: &Config,
    pool: ConnectionPool,
    analytics: Option<Arc<AnalyticsCollector>>,
    translator: Option<Arc<CompiledQueryTranslator>>,
    ingestion_engine: Option<Arc<IngestionEngine>>,
    directory_watcher: Option<Arc<DirectoryWatcher>>,
) -> Option<JoinHandle<()>> {
```

- [ ] **Step 2: Add web UI server spawn to main.rs**

Add import at top:

```rust
use mongocore::web_ui::start_web_ui_server;
```

In the `else` branch (non-stdio mode), after `print_banner(&config)` and before `tokio::select!`, add:

```rust
        // Start Web UI dashboard (if enabled)
        let _web_ui_handle = start_web_ui_server(
            &config,
            pool.clone(),
            analytics.clone(),
            None, // translator - wired in Task 7
            ingestion_engine.clone(),
            directory_watcher.clone(),
        );
```

- [ ] **Step 3: Add web UI port to print_banner**

In `print_banner()`, add after the MCP port line:

```rust
    if config.web_ui_enabled {
        println!("  Web UI:    http://127.0.0.1:{}", config.web_ui_port);
    }
```

- [ ] **Step 4: Verify it compiles with zero warnings**

Run: `cargo build 2>&1 | grep "warning:"`

- [ ] **Step 5: Run unit tests**

Run: `cargo test --lib`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/web_ui/mod.rs
git commit -m "feat(web-ui): wire web UI server into main startup flow with full state"
```

---

## Task 7: Wire Translator into Web UI and Main

**Files:**
- Modify: `src/main.rs`

The compiled query translator is currently created inside the `if cli.stdio` branch. To share it with the web UI in non-stdio mode, it needs to be created in the `else` branch too and passed to `start_web_ui_server`.

- [ ] **Step 1: Create translator in non-stdio branch and pass to web UI**

In `main.rs`, in the non-stdio `else` branch, before `start_web_ui_server`:

```rust
        // Create compiled query translator for cache stats visibility
        let translator = if config.llm_api_key.is_some() || config.llm_gateway.is_some() {
            Some(Arc::new(CompiledQueryTranslator::new(
                Some(pool.clone()),
                None, // LLM provider created separately per gRPC call
                None,
            )))
        } else {
            None
        };
```

Update the `start_web_ui_server` call to pass `translator.clone()` instead of `None`.

Add import:

```rust
use mongocore::compiled::translator::CompiledQueryTranslator;
```

- [ ] **Step 2: Verify it compiles with zero warnings**

Run: `cargo build 2>&1 | grep "warning:"`

- [ ] **Step 3: Run unit tests**

Run: `cargo test --lib`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat(web-ui): wire compiled query translator for cache stats"
```

---

## Task 8: Instrument LLM Providers with Call Tracking

**Files:**
- Modify: `src/compiled/providers/mod.rs`
- Modify: `src/compiled/providers/claude.rs`
- Modify: `src/compiled/providers/openai.rs`
- Modify: `src/compiled/providers/gateway.rs`
- Modify: `src/compiled/translator.rs`

The `LlmProvider` trait `translate()` is called from `CompiledQueryTranslator::translate()`. We instrument at the translator level (wrapping the provider call) rather than in each provider, to keep changes minimal.

- [ ] **Step 1: Add analytics reference to CompiledQueryTranslator**

In `src/compiled/translator.rs`, add an optional analytics field:

```rust
use std::sync::Arc;
use crate::analytics::AnalyticsCollector;

pub struct CompiledQueryTranslator {
    cache: CacheHierarchy,
    provider: Option<Box<dyn LlmProvider>>,
    template_registry: TemplateRegistry,
    analytics: Option<Arc<AnalyticsCollector>>,
}
```

Update `new()` to accept `analytics: Option<Arc<AnalyticsCollector>>`:

```rust
    pub fn new(
        pool: Option<ConnectionPool>,
        provider: Option<Box<dyn LlmProvider>>,
        cache_dir: Option<PathBuf>,
        analytics: Option<Arc<AnalyticsCollector>>,
    ) -> Self {
        let cache = CacheHierarchy::new(pool, cache_dir);
        Self {
            cache,
            provider,
            template_registry: TemplateRegistry::new(),
            analytics,
        }
    }
```

- [ ] **Step 2: Wrap LLM call with timing and recording**

In the `translate()` method, around the `provider.translate(...)` call:

```rust
        // 3. Call LLM
        let provider = self.provider.as_ref().ok_or(TranslateError::NoProvider)?;

        let llm_start = std::time::Instant::now();
        let response = provider
            .translate(intent, database, collection, context)
            .await;
        let llm_latency = llm_start.elapsed();

        // Record LLM call event
        if let Some(ref analytics) = self.analytics {
            use crate::analytics::LlmCallEvent;
            analytics.record_llm_call(LlmCallEvent {
                provider: context.provider_name.clone().unwrap_or_else(|| "unknown".to_string()),
                model: context.model_name.clone().unwrap_or_else(|| "unknown".to_string()),
                tokens_in: 0,  // Not available from trait response — can be enhanced later
                tokens_out: 0,
                latency: llm_latency,
                success: response.is_ok(),
                timestamp: std::time::Instant::now(),
            });
        }

        let response = response.map_err(TranslateError::Llm)?;
```

Note: The `TranslationContext` struct may need `provider_name` and `model_name` fields. Check existing fields and add if missing. If they don't exist, use the provider's known name (e.g., from config). Adapt based on what `TranslationContext` already has.

- [ ] **Step 3: Update all call sites of CompiledQueryTranslator::new()**

Search for all places `CompiledQueryTranslator::new(` is called and add the `analytics` parameter:

```bash
grep -rn "CompiledQueryTranslator::new(" src/ tests/
```

Update each call site to pass the appropriate `analytics: Option<Arc<AnalyticsCollector>>` or `None` for tests.

- [ ] **Step 4: Verify it compiles with zero warnings**

Run: `cargo build 2>&1 | grep "warning:"`

- [ ] **Step 5: Run unit tests**

Run: `cargo test --lib`
Expected: All pass

- [ ] **Step 6: Commit**

```bash
git add src/compiled/ src/main.rs
git commit -m "feat(analytics): instrument LLM provider calls with timing and event recording"
```

---

## Task 9: Instrument Transaction Pipeline with Event Recording

**Files:**
- Modify: `src/grpc/service.rs` (the `transaction_pipeline` handler at line ~1490)

The `execute_transaction_pipeline()` function is called from the gRPC service handler. We instrument at the call site to record pipeline metrics.

- [ ] **Step 1: Add pipeline event recording after execute_transaction_pipeline**

In `src/grpc/service.rs`, in the `transaction_pipeline` method (around line 1513), wrap the call:

```rust
        let pipeline_start = std::time::Instant::now();
        let result = execute_transaction_pipeline(&self.pool, steps, options).await;
        let pipeline_latency = pipeline_start.elapsed();

        // Record pipeline event for web UI
        if let Some(ref analytics) = self.analytics {
            use crate::analytics::PipelineEvent;
            analytics.record_pipeline(PipelineEvent {
                is_transaction: true,
                steps: step_count, // capture len before moving steps
                latency: pipeline_latency,
                success: result.is_ok(),
                retries: 0, // retries happen inside execute_transaction_pipeline
                timestamp: std::time::Instant::now(),
            });
        }
```

Note: You'll need to capture `steps.len()` before the `steps` vec is moved into `execute_transaction_pipeline`. Add `let step_count = steps.len();` before the call.

- [ ] **Step 2: Verify the analytics field is available in the gRPC service**

Check that the gRPC service struct has access to `analytics: Option<Arc<AnalyticsCollector>>`. It's already passed in via `start_grpc_server` — verify it's stored on the service struct and accessible in the handler.

- [ ] **Step 3: Verify it compiles with zero warnings**

Run: `cargo build 2>&1 | grep "warning:"`

- [ ] **Step 4: Run unit tests**

Run: `cargo test --lib`
Expected: All pass

- [ ] **Step 5: Commit**

```bash
git add src/grpc/service.rs
git commit -m "feat(analytics): instrument transaction pipeline execution with event recording"
```

---

## Task 10: Wire Ingestion Handler with Real Data

**Files:**
- Modify: `src/web_ui/handlers.rs`

- [ ] **Step 1: Update ingestion handler to use real state**

Replace the stub `ingestion` handler:

```rust
pub async fn ingestion(State(state): State<Arc<WebUiState>>) -> Html<String> {
    let engine = match &state.ingestion_engine {
        Some(e) => e,
        None => return Html(r#"<p class="empty-state">Ingestion not enabled</p>"#.to_string()),
    };

    // Get job status from the ingestion engine
    let jobs = engine.list_jobs().await;

    if jobs.is_empty() {
        return Html(r#"<p class="empty-state">No active ingestion jobs</p>"#.to_string());
    }

    let mut html = String::from("<header>Ingestion Progress</header><table><thead><tr><th>Job</th><th>Status</th><th>Progress</th></tr></thead><tbody>");
    for job in &jobs {
        let progress = if job.total_records > 0 {
            format!("{}/{} ({:.0}%)", job.records_processed, job.total_records,
                (job.records_processed as f64 / job.total_records as f64) * 100.0)
        } else {
            format!("{} processed", job.records_processed)
        };
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>",
            job.id, job.status, progress
        ));
    }
    html.push_str("</tbody></table>");

    Html(html)
}
```

Note: The exact method names on `IngestionEngine` (e.g., `list_jobs()`) and job struct fields need to be verified against the actual code. Adapt field names to match what `IngestionEngine` exposes. If it doesn't have a `list_jobs()` method, check the `GetIngestStatus` / `ListIngestJobs` gRPC handlers for how they access job data and replicate that pattern.

- [ ] **Step 2: Verify it compiles with zero warnings**

Run: `cargo build 2>&1 | grep "warning:"`

- [ ] **Step 3: Commit**

```bash
git add src/web_ui/handlers.rs
git commit -m "feat(web-ui): wire ingestion handler to real IngestionEngine state"
```

---

## Task 11: Fix Status Handler — Real Connection Health Check

**Files:**
- Modify: `src/web_ui/handlers.rs`

- [ ] **Step 1: Use pool.health_check() for real connection status**

Update the `status` handler to actually check connection health:

```rust
pub async fn status(State(state): State<Arc<WebUiState>>) -> Html<String> {
    let uptime = state.start_time.elapsed();
    let uptime_str = format_duration(uptime);

    let (total_ops, total_errors) = match &state.analytics {
        Some(a) => (a.total_operations(), a.total_errors()),
        None => (0, 0),
    };

    let error_rate = if total_ops > 0 {
        (total_errors as f64 / total_ops as f64) * 100.0
    } else {
        0.0
    };

    // Real connection health check
    let connected = state.pool.health_check().await.is_ok();
    let (indicator_class, status_text) = if connected {
        ("indicator", "Connected")
    } else {
        ("indicator disconnected", "Disconnected")
    };

    let (cpu, mem_mb) = get_process_stats();

    Html(format!(
        r#"<div class="status-row">
            <span><strong>MongoCore Dashboard</strong></span>
            <span><span class="{indicator_class}"></span> {status_text}</span>
            <span>↑ {uptime_str}</span>
            <span>cpu: {cpu:.1}%</span>
            <span>mem: {mem_mb:.0}MB</span>
            <span>ops: {total_ops}</span>
            <span>errors: {error_rate:.2}%</span>
        </div>"#
    ))
}
```

- [ ] **Step 2: Verify it compiles with zero warnings**

Run: `cargo build 2>&1 | grep "warning:"`

- [ ] **Step 3: Commit**

```bash
git add src/web_ui/handlers.rs
git commit -m "feat(web-ui): use real connection health check in status bar"
```

---

## Task 12: Unit Tests for Handlers

**Files:**
- Modify: `src/web_ui/handlers.rs` (add tests module)

- [ ] **Step 1: Add unit tests for handler helper functions**

At the bottom of `src/web_ui/handlers.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration_seconds() {
        assert_eq!(format_duration(Duration::from_secs(45)), "45s");
    }

    #[test]
    fn test_format_duration_minutes() {
        assert_eq!(format_duration(Duration::from_secs(125)), "2m 5s");
    }

    #[test]
    fn test_format_duration_hours() {
        assert_eq!(format_duration(Duration::from_secs(7380)), "2h 3m");
    }

    #[test]
    fn test_parse_window() {
        assert_eq!(parse_window("1m"), 60);
        assert_eq!(parse_window("5m"), 300);
        assert_eq!(parse_window("15m"), 900);
        assert_eq!(parse_window("1h"), 3600);
        assert_eq!(parse_window("invalid"), 300);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib web_ui`
Expected: All pass

- [ ] **Step 3: Commit**

```bash
git add src/web_ui/handlers.rs
git commit -m "test(web-ui): add unit tests for handler utility functions"
```

---

## Task 13: Integration Test — Web UI Server Responds

**Files:**
- Create: `tests/integration/web_ui_test.rs` (or add to existing integration test file)

- [ ] **Step 1: Add integration test verifying the web UI starts and serves index**

This test uses axum's `tower::ServiceExt` to test the router without binding a real port:

```rust
use axum::body::Body;
use axum::http::Request;
use http::StatusCode;
use tower::ServiceExt;
use std::sync::Arc;

#[tokio::test]
async fn test_web_ui_serves_index() {
    // Requires Docker MongoDB for ConnectionPool
    use mongocore::config::{CliArgs, Config};
    use mongocore::connection::ConnectionPool;
    use mongocore::web_ui::server::create_router;
    use mongocore::web_ui::WebUiState;

    let cli = CliArgs::parse_from(["mongocore"]);
    let config = Config::load(&cli).unwrap();
    let pool = ConnectionPool::connect(&config).await.unwrap();

    let state = Arc::new(WebUiState {
        analytics: None,
        pool,
        config,
        translator: None,
        ingestion_engine: None,
        directory_watcher: None,
        start_time: std::time::Instant::now(),
    });

    let app = create_router(state).await.unwrap();

    let response = app
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("MongoCore Dashboard"));
}

#[tokio::test]
async fn test_web_ui_serves_assets() {
    // Same setup as above, test static asset serving
    // GET /assets/pico.min.css should return 200 with text/css content-type
}

#[tokio::test]
async fn test_web_ui_api_status_returns_html() {
    // GET /api/status should return HTML fragment
}
```

- [ ] **Step 2: Run integration test compilation check**

Run: `cargo test --test integration --no-run`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add tests/
git commit -m "test(web-ui): add integration tests for web UI server and API endpoints"
```

---

## Task 14: Documentation Updates

**Files:**
- Modify: `docs/getting-started.md` (add web_ui config)
- Modify: `docs/roadmap.md` (mark visualization as implemented)

- [ ] **Step 1: Add web UI section to getting-started docs**

Add a section documenting the `[web_ui]` TOML config, CLI flags, and env vars:

```markdown
## Web UI Dashboard

MongoCore includes a built-in diagnostic dashboard served on localhost.

| Setting | CLI | Env Var | TOML | Default |
|---------|-----|---------|------|---------|
| Enable/disable | `--web-ui` | `MONGOCORE_WEB_UI` | `[web_ui] enabled` | `true` |
| Port | `--web-ui-port` | `MONGOCORE_WEB_UI_PORT` | `[web_ui] port` | `27999` |

The dashboard binds to `127.0.0.1` only (not accessible from other machines).
Open `http://127.0.0.1:27999` after starting MongoCore to view:
- Real-time operations/sec and latency charts
- Operation breakdown and query insights
- Pipeline and transaction pipeline stats
- Recent errors
- LLM usage, ingestion progress, and cache statistics (expandable)
```

- [ ] **Step 2: Update roadmap**

Move "Visualizations — Web UI for analytics, query flow, and ingestion progress" from backlog to the current version section.

- [ ] **Step 3: Commit**

```bash
git add docs/
git commit -m "docs: add web UI configuration and update roadmap"
```

---

## Summary of Dependencies Between Tasks

```
Task 1 (deps)
  └► Task 2 (config)
       └► Task 3 (instrumentation types)
            └► Task 4 (web_ui module + handlers)
                 └► Task 5 (expose cache_stats)
                      └► Task 6 (wire main.rs with full state)
                           └► Task 7 (wire translator)
                                ├► Task 8 (instrument LLM providers)
                                ├► Task 9 (instrument txn pipeline)
                                ├► Task 10 (ingestion handler real data)
                                └► Task 11 (real connection health)
                                     ├► Task 12 (unit tests)
                                     ├► Task 13 (integration tests)
                                     └► Task 14 (docs)
```

**Sequential chain:** Tasks 1 → 2 → 3 → 4 → 5 → 6 → 7

**Parallelizable after Task 7:** Tasks 8, 9, 10, 11 (all add instrumentation/wiring independently)

**Parallelizable after Tasks 8-11:** Tasks 12, 13, 14

## Vendored Assets Note

Tasks 4 Step 1 downloads ~89KB of third-party JS/CSS into `src/web_ui/assets/`. These are committed to the repo so that `rust-embed` can bake them into the binary at compile time. This is intentional — no external network dependencies at build time or runtime. Add a comment in `src/web_ui/assets/README` noting the versions and sources for future updates.
