use crate::analytics::types::{AnalyticsEvent, OperationKind};
use std::collections::HashMap;

/// Summary statistics computed from a collection of analytics events
#[derive(Debug, Clone)]
pub struct AnalyticsSummary {
    pub total_operations: usize,
    pub total_errors: usize,
    pub error_rate: f64,
    pub p50_latency_ms: f64,
    pub p95_latency_ms: f64,
    pub p99_latency_ms: f64,
    pub top_operations: Vec<(OperationKind, usize)>,
    pub top_collections: Vec<(String, usize)>,
}

/// Aggregate analytics events into summary statistics
pub fn aggregate(events: &[AnalyticsEvent]) -> AnalyticsSummary {
    // Handle empty input
    if events.is_empty() {
        return AnalyticsSummary {
            total_operations: 0,
            total_errors: 0,
            error_rate: 0.0,
            p50_latency_ms: 0.0,
            p95_latency_ms: 0.0,
            p99_latency_ms: 0.0,
            top_operations: Vec::new(),
            top_collections: Vec::new(),
        };
    }

    let total_operations = events.len();
    let total_errors = events.iter().filter(|e| !e.success).count();
    let error_rate = total_errors as f64 / total_operations as f64;

    // Collect and sort latencies for percentile calculation
    let mut latencies: Vec<f64> = events
        .iter()
        .map(|e| e.latency.as_secs_f64() * 1000.0) // Convert to milliseconds
        .collect();
    latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let p50_latency_ms = percentile(&latencies, 0.50);
    let p95_latency_ms = percentile(&latencies, 0.95);
    let p99_latency_ms = percentile(&latencies, 0.99);

    // Count operations by kind
    let mut operation_counts: HashMap<OperationKind, usize> = HashMap::new();
    for event in events {
        *operation_counts.entry(event.operation.clone()).or_insert(0) += 1;
    }

    // Sort operations by count and take top 10
    let mut top_operations: Vec<(OperationKind, usize)> = operation_counts.into_iter().collect();
    top_operations.sort_by(|a, b| b.1.cmp(&a.1));
    top_operations.truncate(10);

    // Count collections as "database.collection"
    let mut collection_counts: HashMap<String, usize> = HashMap::new();
    for event in events {
        let key = format!("{}.{}", event.database, event.collection);
        *collection_counts.entry(key).or_insert(0) += 1;
    }

    // Sort collections by count and take top 10
    let mut top_collections: Vec<(String, usize)> = collection_counts.into_iter().collect();
    top_collections.sort_by(|a, b| b.1.cmp(&a.1));
    top_collections.truncate(10);

    AnalyticsSummary {
        total_operations,
        total_errors,
        error_rate,
        p50_latency_ms,
        p95_latency_ms,
        p99_latency_ms,
        top_operations,
        top_collections,
    }
}

/// Calculate a percentile from a sorted slice using nearest-rank method
fn percentile(sorted: &[f64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }

    let n = sorted.len();
    if n == 1 {
        return sorted[0];
    }

    // Nearest-rank method: ceil(pct * n)
    let rank = (pct * n as f64).ceil() as usize;
    let index = rank.saturating_sub(1).min(n - 1);
    sorted[index]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::types::OperationKind;
    use std::time::Duration;

    #[test]
    fn test_empty_events() {
        let events: Vec<AnalyticsEvent> = vec![];
        let summary = aggregate(&events);

        assert_eq!(summary.total_operations, 0);
        assert_eq!(summary.total_errors, 0);
        assert_eq!(summary.error_rate, 0.0);
        assert_eq!(summary.p50_latency_ms, 0.0);
        assert_eq!(summary.p95_latency_ms, 0.0);
        assert_eq!(summary.p99_latency_ms, 0.0);
        assert!(summary.top_operations.is_empty());
        assert!(summary.top_collections.is_empty());
    }

    #[test]
    fn test_top_operations() {
        let events = vec![
            AnalyticsEvent::new(
                OperationKind::Find,
                "db1".to_string(),
                "coll1".to_string(),
                Duration::from_millis(10),
                true,
            ),
            AnalyticsEvent::new(
                OperationKind::Find,
                "db1".to_string(),
                "coll1".to_string(),
                Duration::from_millis(20),
                true,
            ),
            AnalyticsEvent::new(
                OperationKind::Find,
                "db1".to_string(),
                "coll1".to_string(),
                Duration::from_millis(30),
                true,
            ),
            AnalyticsEvent::new(
                OperationKind::Insert,
                "db1".to_string(),
                "coll1".to_string(),
                Duration::from_millis(40),
                true,
            ),
        ];

        let summary = aggregate(&events);

        assert_eq!(summary.total_operations, 4);
        assert_eq!(summary.top_operations.len(), 2);
        assert_eq!(summary.top_operations[0].0, OperationKind::Find);
        assert_eq!(summary.top_operations[0].1, 3);
        assert_eq!(summary.top_operations[1].0, OperationKind::Insert);
        assert_eq!(summary.top_operations[1].1, 1);
    }

    #[test]
    fn test_latency_percentiles() {
        // Create events with known latencies: 10, 20, 30, 40, 50 ms
        let events = vec![
            AnalyticsEvent::new(
                OperationKind::Find,
                "db".to_string(),
                "coll".to_string(),
                Duration::from_millis(10),
                true,
            ),
            AnalyticsEvent::new(
                OperationKind::Find,
                "db".to_string(),
                "coll".to_string(),
                Duration::from_millis(20),
                true,
            ),
            AnalyticsEvent::new(
                OperationKind::Find,
                "db".to_string(),
                "coll".to_string(),
                Duration::from_millis(30),
                true,
            ),
            AnalyticsEvent::new(
                OperationKind::Find,
                "db".to_string(),
                "coll".to_string(),
                Duration::from_millis(40),
                true,
            ),
            AnalyticsEvent::new(
                OperationKind::Find,
                "db".to_string(),
                "coll".to_string(),
                Duration::from_millis(50),
                true,
            ),
        ];

        let summary = aggregate(&events);

        // p50 (median) of [10, 20, 30, 40, 50] -> ceil(0.5 * 5) = 3rd element = 30
        assert_eq!(summary.p50_latency_ms, 30.0);

        // p99 of 5 elements -> ceil(0.99 * 5) = 5th element = 50
        assert_eq!(summary.p99_latency_ms, 50.0);

        // Verify all percentiles are in reasonable range
        assert!(summary.p50_latency_ms >= 10.0);
        assert!(summary.p95_latency_ms <= 50.0);
        assert!(summary.p99_latency_ms <= 50.0);
    }

    #[test]
    fn test_error_rate() {
        let events = vec![
            AnalyticsEvent::new(
                OperationKind::Find,
                "db".to_string(),
                "coll".to_string(),
                Duration::from_millis(10),
                true,
            ),
            AnalyticsEvent::new(
                OperationKind::Find,
                "db".to_string(),
                "coll".to_string(),
                Duration::from_millis(20),
                true,
            ),
            AnalyticsEvent::new(
                OperationKind::Find,
                "db".to_string(),
                "coll".to_string(),
                Duration::from_millis(30),
                false, // Error
            ),
            AnalyticsEvent::new(
                OperationKind::Find,
                "db".to_string(),
                "coll".to_string(),
                Duration::from_millis(40),
                true,
            ),
        ];

        let summary = aggregate(&events);

        assert_eq!(summary.total_operations, 4);
        assert_eq!(summary.total_errors, 1);
        assert_eq!(summary.error_rate, 0.25);
    }

    #[test]
    fn test_top_collections() {
        let events = vec![
            AnalyticsEvent::new(
                OperationKind::Find,
                "db1".to_string(),
                "coll1".to_string(),
                Duration::from_millis(10),
                true,
            ),
            AnalyticsEvent::new(
                OperationKind::Find,
                "db1".to_string(),
                "coll1".to_string(),
                Duration::from_millis(20),
                true,
            ),
            AnalyticsEvent::new(
                OperationKind::Find,
                "db1".to_string(),
                "coll1".to_string(),
                Duration::from_millis(30),
                true,
            ),
            AnalyticsEvent::new(
                OperationKind::Find,
                "db2".to_string(),
                "coll2".to_string(),
                Duration::from_millis(40),
                true,
            ),
            AnalyticsEvent::new(
                OperationKind::Find,
                "db2".to_string(),
                "coll2".to_string(),
                Duration::from_millis(50),
                true,
            ),
            AnalyticsEvent::new(
                OperationKind::Find,
                "db3".to_string(),
                "coll3".to_string(),
                Duration::from_millis(60),
                true,
            ),
        ];

        let summary = aggregate(&events);

        assert_eq!(summary.top_collections.len(), 3);
        // db1.coll1 should be first with 3 occurrences
        assert_eq!(summary.top_collections[0].0, "db1.coll1");
        assert_eq!(summary.top_collections[0].1, 3);
        // db2.coll2 should be second with 2 occurrences
        assert_eq!(summary.top_collections[1].0, "db2.coll2");
        assert_eq!(summary.top_collections[1].1, 2);
        // db3.coll3 should be third with 1 occurrence
        assert_eq!(summary.top_collections[2].0, "db3.coll3");
        assert_eq!(summary.top_collections[2].1, 1);
    }
}
