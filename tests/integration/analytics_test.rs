use std::sync::Arc;
use std::time::Duration;
use mongocore::analytics::{AnalyticsCollector, AnalyticsEvent, OperationKind};
use mongocore::analytics::aggregator;

#[tokio::test]
async fn test_analytics_records_and_aggregates() {
    let collector = Arc::new(AnalyticsCollector::new(1000));

    // Simulate operations
    for i in 0..10 {
        let success = i % 3 != 0; // some failures
        collector.record(AnalyticsEvent::new(
            OperationKind::Find,
            "testdb".to_string(),
            "users".to_string(),
            Duration::from_millis(5 + i),
            success,
        ));
    }
    collector.record(AnalyticsEvent::new(
        OperationKind::Insert,
        "testdb".to_string(),
        "orders".to_string(),
        Duration::from_millis(3),
        true,
    ));

    assert_eq!(collector.total_operations(), 11);
    assert!(collector.total_errors() > 0);

    let events = collector.snapshot();
    let summary = aggregator::aggregate(&events);

    assert_eq!(summary.total_operations, 11);
    assert!(summary.error_rate > 0.0);
    assert!(summary.p50_latency_ms > 0.0);
    assert!(!summary.top_operations.is_empty());
    assert!(!summary.top_collections.is_empty());
}

#[tokio::test]
async fn test_analytics_buffer_overflow() {
    let collector = Arc::new(AnalyticsCollector::new(5)); // Small buffer

    for i in 0..20 {
        collector.record(AnalyticsEvent::new(
            OperationKind::Find,
            "db".to_string(),
            "coll".to_string(),
            Duration::from_millis(i + 1),
            true,
        ));
    }

    // Total ops tracked even when buffer overflows
    assert_eq!(collector.total_operations(), 20);
    // But snapshot only has buffer capacity
    assert_eq!(collector.snapshot().len(), 5);
}

#[tokio::test]
async fn test_analytics_multiple_operation_types() {
    let collector = Arc::new(AnalyticsCollector::new(100));

    let ops = vec![
        (OperationKind::Find, 5),
        (OperationKind::Insert, 3),
        (OperationKind::Update, 2),
        (OperationKind::Delete, 1),
    ];

    for (op, count) in &ops {
        for _ in 0..*count {
            collector.record(AnalyticsEvent::new(
                op.clone(),
                "db".to_string(),
                "coll".to_string(),
                Duration::from_millis(10),
                true,
            ));
        }
    }

    let summary = aggregator::aggregate(&collector.snapshot());
    assert_eq!(summary.total_operations, 11);
    // Find should be the top operation
    assert_eq!(summary.top_operations[0].0, OperationKind::Find);
    assert_eq!(summary.top_operations[0].1, 5);
}
