use crate::analytics::{AnalyticsEvent, RingBuffer};
use crate::analytics::types::{LlmCallEvent, PipelineEvent};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

/// Main analytics collector that tracks operations using a ring buffer and atomic counters.
pub struct AnalyticsCollector {
    buffer: RingBuffer,
    total_ops: AtomicU64,
    total_errors: AtomicU64,
    llm_calls: Mutex<Vec<LlmCallEvent>>,
    pipeline_events: Mutex<Vec<PipelineEvent>>,
}

impl AnalyticsCollector {
    /// Create a new analytics collector with the specified buffer capacity.
    pub fn new(buffer_capacity: usize) -> Self {
        Self {
            buffer: RingBuffer::new(buffer_capacity),
            total_ops: AtomicU64::new(0),
            total_errors: AtomicU64::new(0),
            llm_calls: Mutex::new(Vec::new()),
            pipeline_events: Mutex::new(Vec::new()),
        }
    }

    /// Record an analytics event.
    /// Increments total operations count, increments error count if the event failed,
    /// and adds the event to the ring buffer.
    pub fn record(&self, event: AnalyticsEvent) {
        self.total_ops.fetch_add(1, Ordering::Relaxed);

        if !event.success {
            self.total_errors.fetch_add(1, Ordering::Relaxed);
        }

        self.buffer.push(event);
    }

    /// Returns the current number of events in the buffer.
    pub fn event_count(&self) -> usize {
        self.buffer.len()
    }

    /// Returns the total number of operations recorded since creation.
    pub fn total_operations(&self) -> u64 {
        self.total_ops.load(Ordering::Relaxed)
    }

    /// Returns the total number of errors recorded since creation.
    pub fn total_errors(&self) -> u64 {
        self.total_errors.load(Ordering::Relaxed)
    }

    /// Returns a snapshot of all events currently in the buffer.
    pub fn snapshot(&self) -> Vec<AnalyticsEvent> {
        self.buffer.snapshot()
    }

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::OperationKind;
    use std::time::Duration;

    fn create_test_event(database: &str, success: bool) -> AnalyticsEvent {
        AnalyticsEvent::new(
            OperationKind::Find,
            database.to_string(),
            "test_collection".to_string(),
            Duration::from_millis(100),
            success,
        )
    }

    #[test]
    fn test_collector_records_event() {
        let collector = AnalyticsCollector::new(10);
        assert_eq!(collector.event_count(), 0);

        collector.record(create_test_event("db1", true));
        assert_eq!(collector.event_count(), 1);
    }

    #[test]
    fn test_collector_total_ops() {
        let collector = AnalyticsCollector::new(10);

        for i in 0..10 {
            collector.record(create_test_event(&format!("db{}", i), true));
        }

        assert_eq!(collector.total_operations(), 10);
    }

    #[test]
    fn test_collector_tracks_errors() {
        let collector = AnalyticsCollector::new(10);

        // Record 3 successful operations
        for i in 0..3 {
            collector.record(create_test_event(&format!("db{}", i), true));
        }

        // Record 2 failed operations
        for i in 0..2 {
            collector.record(create_test_event(&format!("db{}", i), false));
        }

        assert_eq!(collector.total_operations(), 5);
        assert_eq!(collector.total_errors(), 2);
    }

    #[test]
    fn test_collector_snapshot() {
        let collector = AnalyticsCollector::new(10);

        collector.record(create_test_event("db1", true));
        collector.record(create_test_event("db2", true));
        collector.record(create_test_event("db3", true));

        let snapshot = collector.snapshot();
        assert_eq!(snapshot.len(), 3);
        assert_eq!(snapshot[0].database, "db1");
        assert_eq!(snapshot[1].database, "db2");
        assert_eq!(snapshot[2].database, "db3");
    }
}
