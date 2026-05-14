use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Query, State};
use axum::response::{Html, IntoResponse, Json};
use serde::Deserialize;

use crate::analytics::aggregator::aggregate;

use super::WebUiState;

// --- Status Handler ---

pub async fn status(State(state): State<Arc<WebUiState>>) -> impl IntoResponse {
    let uptime = state.start_time.elapsed();
    let uptime_str = format_duration(uptime);

    let (cpu, mem_mb) = get_process_stats();

    let (total_ops, error_rate) = match &state.analytics {
        Some(analytics) => {
            let ops = analytics.total_operations();
            let errors = analytics.total_errors();
            let rate = if ops > 0 {
                (errors as f64 / ops as f64) * 100.0
            } else {
                0.0
            };
            (ops, rate)
        }
        None => (0, 0.0),
    };

    let health = match state.pool.health_check().await {
        Ok(()) => r#"<span class="dot green"></span> Connected"#,
        Err(_) => r#"<span class="dot red"></span> Disconnected"#,
    };

    Html(format!(
        r#"<span class="status-item">{health}</span>
<span class="status-item">Uptime: {uptime_str}</span>
<span class="status-item">CPU: {cpu:.1}%</span>
<span class="status-item">Mem: {mem_mb:.1} MB</span>
<span class="status-item">Ops: {total_ops}</span>
<span class="status-item">Errors: {error_rate:.2}%</span>"#
    ))
}

// --- Metrics Handler (JSON for charts) ---

#[derive(Deserialize)]
pub struct MetricsQuery {
    window: Option<String>,
}

pub async fn metrics(
    State(state): State<Arc<WebUiState>>,
    Query(query): Query<MetricsQuery>,
) -> impl IntoResponse {
    let window_secs = parse_window(query.window.as_deref().unwrap_or("5m"));

    let events = match &state.analytics {
        Some(analytics) => analytics.snapshot(),
        None => Vec::new(),
    };

    let now = std::time::Instant::now();
    let cutoff = now - Duration::from_secs(window_secs);

    // Filter events within window
    let windowed: Vec<_> = events.iter().filter(|e| e.timestamp >= cutoff).collect();

    // Bucket into 2-second intervals
    let bucket_size = 2u64;
    let num_buckets = (window_secs / bucket_size).max(1) as usize;

    let mut timestamps = Vec::with_capacity(num_buckets);
    let mut ops_per_sec = Vec::with_capacity(num_buckets);
    let mut p50 = Vec::with_capacity(num_buckets);
    let mut p95 = Vec::with_capacity(num_buckets);
    let mut p99 = Vec::with_capacity(num_buckets);

    for i in 0..num_buckets {
        let bucket_start = cutoff + Duration::from_secs(i as u64 * bucket_size);
        let bucket_end = bucket_start + Duration::from_secs(bucket_size);

        let bucket_events: Vec<_> = windowed
            .iter()
            .filter(|e| e.timestamp >= bucket_start && e.timestamp < bucket_end)
            .collect();

        timestamps.push(i as f64 * bucket_size as f64);
        ops_per_sec.push(bucket_events.len() as f64 / bucket_size as f64);

        if bucket_events.is_empty() {
            p50.push(0.0);
            p95.push(0.0);
            p99.push(0.0);
        } else {
            let mut latencies: Vec<f64> = bucket_events
                .iter()
                .map(|e| e.latency.as_secs_f64() * 1000.0)
                .collect();
            latencies.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            p50.push(percentile_of(&latencies, 0.50));
            p95.push(percentile_of(&latencies, 0.95));
            p99.push(percentile_of(&latencies, 0.99));
        }
    }

    Json(serde_json::json!({
        "timestamps": timestamps,
        "ops_per_sec": ops_per_sec,
        "p50": p50,
        "p95": p95,
        "p99": p99,
    }))
}

// --- Operations Handler ---

pub async fn operations(State(state): State<Arc<WebUiState>>) -> impl IntoResponse {
    let events = match &state.analytics {
        Some(analytics) => analytics.snapshot(),
        None => return Html("<p class=\"empty-state\">No analytics data</p>".to_string()),
    };

    if events.is_empty() {
        return Html("<p class=\"empty-state\">No operations recorded</p>".to_string());
    }

    let summary = aggregate(&events);

    let mut html = String::from(
        "<table><thead><tr><th>Operation</th><th>Count</th></tr></thead><tbody>",
    );
    for (op, count) in &summary.top_operations {
        html.push_str(&format!("<tr><td>{:?}</td><td>{}</td></tr>", op, count));
    }
    html.push_str("</tbody></table>");

    if !summary.top_collections.is_empty() {
        html.push_str(
            "<table><thead><tr><th>Namespace</th><th>Count</th></tr></thead><tbody>",
        );
        for (coll, count) in &summary.top_collections {
            html.push_str(&format!("<tr><td>{}</td><td>{}</td></tr>", coll, count));
        }
        html.push_str("</tbody></table>");
    }

    Html(html)
}

// --- Queries Handler ---

pub async fn queries(State(state): State<Arc<WebUiState>>) -> impl IntoResponse {
    let events = match &state.analytics {
        Some(analytics) => analytics.snapshot(),
        None => return Html("<p class=\"empty-state\">No analytics data</p>".to_string()),
    };

    // Group by fingerprint, find slowest
    let mut by_fingerprint: std::collections::HashMap<String, (Duration, String, String)> =
        std::collections::HashMap::new();

    for event in &events {
        if let Some(fp) = &event.fingerprint {
            let key = fp.as_str().to_string();
            let entry = by_fingerprint
                .entry(key)
                .or_insert((Duration::ZERO, event.database.clone(), event.collection.clone()));
            if event.latency > entry.0 {
                entry.0 = event.latency;
            }
        }
    }

    if by_fingerprint.is_empty() {
        return Html("<p class=\"empty-state\">No query fingerprints recorded</p>".to_string());
    }

    let mut sorted: Vec<_> = by_fingerprint.into_iter().collect();
    sorted.sort_by(|a, b| b.1 .0.cmp(&a.1 .0));
    sorted.truncate(20);

    let mut html = String::from(
        "<table><thead><tr><th>Fingerprint</th><th>Namespace</th><th>Max Latency</th></tr></thead><tbody>",
    );
    for (fp, (latency, db, coll)) in &sorted {
        html.push_str(&format!(
            "<tr><td><code>{}</code></td><td>{}.{}</td><td>{:.1}ms</td></tr>",
            truncate_str(fp, 60),
            db,
            coll,
            latency.as_secs_f64() * 1000.0
        ));
    }
    html.push_str("</tbody></table>");

    Html(html)
}

// --- Pipelines Handler ---

pub async fn pipelines(State(state): State<Arc<WebUiState>>) -> impl IntoResponse {
    let events = match &state.analytics {
        Some(analytics) => analytics.pipeline_events_snapshot(),
        None => return Html("<p class=\"empty-state\">No analytics data</p>".to_string()),
    };

    if events.is_empty() {
        return Html("<p class=\"empty-state\">No pipeline executions recorded</p>".to_string());
    }

    let total = events.len();
    let transactions = events.iter().filter(|e| e.is_transaction).count();
    let successes = events.iter().filter(|e| e.success).count();
    let avg_steps = events.iter().map(|e| e.steps).sum::<usize>() as f64 / total as f64;
    let avg_latency =
        events.iter().map(|e| e.latency.as_secs_f64() * 1000.0).sum::<f64>() / total as f64;
    let total_retries: u32 = events.iter().map(|e| e.retries).sum();

    Html(format!(
        r#"<table>
<tbody>
<tr><td>Total Executions</td><td>{total}</td></tr>
<tr><td>Transactions</td><td>{transactions}</td></tr>
<tr><td>Success Rate</td><td>{:.1}%</td></tr>
<tr><td>Avg Steps</td><td>{avg_steps:.1}</td></tr>
<tr><td>Avg Latency</td><td>{avg_latency:.1}ms</td></tr>
<tr><td>Total Retries</td><td>{total_retries}</td></tr>
</tbody>
</table>"#,
        (successes as f64 / total as f64) * 100.0
    ))
}

// --- Errors Handler ---

pub async fn errors(State(state): State<Arc<WebUiState>>) -> impl IntoResponse {
    let events = match &state.analytics {
        Some(analytics) => analytics.snapshot(),
        None => return Html("<p class=\"empty-state\">No analytics data</p>".to_string()),
    };

    let mut failed: Vec<_> = events.iter().filter(|e| !e.success).collect();
    if failed.is_empty() {
        return Html("<p class=\"empty-state\">No errors recorded</p>".to_string());
    }

    // Take last 50
    failed.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    failed.truncate(50);

    let mut html = String::from(
        "<table><thead><tr><th>Operation</th><th>Namespace</th><th>Latency</th></tr></thead><tbody>",
    );
    for event in &failed {
        html.push_str(&format!(
            "<tr><td>{:?}</td><td>{}.{}</td><td>{:.1}ms</td></tr>",
            event.operation,
            event.database,
            event.collection,
            event.latency.as_secs_f64() * 1000.0
        ));
    }
    html.push_str("</tbody></table>");

    Html(html)
}

// --- Ingestion Handler ---

pub async fn ingestion(State(state): State<Arc<WebUiState>>) -> impl IntoResponse {
    let engine = match &state.ingestion_engine {
        Some(e) => e,
        None => {
            return Html(
                "<p class=\"empty-state\">Ingestion not enabled</p>".to_string(),
            )
        }
    };

    let jobs = match engine.list_jobs().await {
        Ok(jobs) => jobs,
        Err(_) => {
            return Html(
                "<p class=\"empty-state\">Unable to retrieve ingestion jobs</p>".to_string(),
            )
        }
    };

    if jobs.is_empty() {
        return Html(
            "<p class=\"empty-state\">No active ingestion jobs</p>".to_string(),
        );
    }

    let mut html = String::from(
        "<header>Ingestion Progress</header>\
         <table><thead><tr><th>Job</th><th>Status</th><th>Progress</th><th>Errors</th></tr></thead><tbody>",
    );
    for job in &jobs {
        let progress = if job.total_rows > 0 {
            format!(
                "{}/{} ({:.0}%)",
                job.rows_processed,
                job.total_rows,
                (job.rows_processed as f64 / job.total_rows as f64) * 100.0
            )
        } else {
            format!("{} processed", job.rows_processed)
        };
        html.push_str(&format!(
            "<tr><td>{}</td><td>{:?}</td><td>{}</td><td>{}</td></tr>",
            job.job_id, job.status, progress, job.rows_failed
        ));
    }
    html.push_str("</tbody></table>");

    Html(html)
}

// --- LLM Handler ---

pub async fn llm(State(state): State<Arc<WebUiState>>) -> impl IntoResponse {
    let calls = match &state.analytics {
        Some(analytics) => analytics.llm_calls_snapshot(),
        None => return Html("<p class=\"empty-state\">No analytics data</p>".to_string()),
    };

    if calls.is_empty() {
        return Html("<p class=\"empty-state\">No LLM calls recorded</p>".to_string());
    }

    let total = calls.len();
    let successes = calls.iter().filter(|c| c.success).count();
    let total_tokens_in: u32 = calls.iter().map(|c| c.tokens_in).sum();
    let total_tokens_out: u32 = calls.iter().map(|c| c.tokens_out).sum();
    let avg_latency =
        calls.iter().map(|c| c.latency.as_secs_f64() * 1000.0).sum::<f64>() / total as f64;

    Html(format!(
        r#"<table>
<tbody>
<tr><td>Total Calls</td><td>{total}</td></tr>
<tr><td>Success Rate</td><td>{:.1}%</td></tr>
<tr><td>Total Tokens In</td><td>{total_tokens_in}</td></tr>
<tr><td>Total Tokens Out</td><td>{total_tokens_out}</td></tr>
<tr><td>Avg Latency</td><td>{avg_latency:.1}ms</td></tr>
</tbody>
</table>"#,
        (successes as f64 / total as f64) * 100.0
    ))
}

// --- Cache Handler ---

pub async fn cache(State(state): State<Arc<WebUiState>>) -> impl IntoResponse {
    let translator = match &state.translator {
        Some(t) => t,
        None => {
            return Html(
                "<p class=\"empty-state\">Compiled query cache not active</p>".to_string(),
            )
        }
    };

    let stats = translator.cache_stats();
    let l1_total = stats.l1_hits + stats.l1_misses;
    let l2_total = stats.l2_hits + stats.l2_misses;
    let l3_total = stats.l3_hits + stats.l3_misses;

    let l1_hit_rate = if l1_total > 0 {
        (stats.l1_hits as f64 / l1_total as f64) * 100.0
    } else {
        0.0
    };
    let l2_hit_rate = if l2_total > 0 {
        (stats.l2_hits as f64 / l2_total as f64) * 100.0
    } else {
        0.0
    };
    let l3_hit_rate = if l3_total > 0 {
        (stats.l3_hits as f64 / l3_total as f64) * 100.0
    } else {
        0.0
    };

    Html(format!(
        r#"<table>
<tbody>
<tr><td>L1 Size</td><td>{}</td></tr>
<tr><td>L1 Hits</td><td>{}</td></tr>
<tr><td>L1 Hit Rate</td><td>{:.1}%</td></tr>
<tr><td>L2 Hits</td><td>{}</td></tr>
<tr><td>L2 Hit Rate</td><td>{:.1}%</td></tr>
<tr><td>L3 Hits</td><td>{}</td></tr>
<tr><td>L3 Hit Rate</td><td>{:.1}%</td></tr>
<tr><td>Evictions</td><td>{}</td></tr>
</tbody>
</table>"#,
        stats.l1_size,
        stats.l1_hits,
        l1_hit_rate,
        stats.l2_hits,
        l2_hit_rate,
        stats.l3_hits,
        l3_hit_rate,
        stats.evictions
    ))
}

// --- Utility Functions ---

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

fn parse_window(s: &str) -> u64 {
    match s {
        "1m" => 60,
        "5m" => 300,
        "15m" => 900,
        "1h" => 3600,
        _ => 300,
    }
}

fn percentile_of(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let n = sorted.len();
    if n == 1 {
        return sorted[0];
    }
    let rank = (pct * n as f64).ceil() as usize;
    let index = rank.saturating_sub(1).min(n - 1);
    sorted[index]
}

fn truncate_str(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        &s[..max_len]
    }
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
