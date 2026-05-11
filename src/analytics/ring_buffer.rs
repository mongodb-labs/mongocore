use crate::analytics::AnalyticsEvent;
use std::collections::VecDeque;
use std::sync::Mutex;

/// A thread-safe fixed-size ring buffer for analytics events.
/// When at capacity, new events evict the oldest ones.
pub struct RingBuffer {
    buffer: Mutex<VecDeque<AnalyticsEvent>>,
    capacity: usize,
}

impl RingBuffer {
    /// Create a new ring buffer with the specified capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: Mutex::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    /// Add an event to the buffer. If at capacity, evicts the oldest event.
    pub fn push(&self, event: AnalyticsEvent) {
        let mut buffer = self.buffer.lock().unwrap();

        if buffer.len() >= self.capacity {
            buffer.pop_front(); // Remove oldest event
        }

        buffer.push_back(event);
    }

    /// Returns the current number of events in the buffer.
    pub fn len(&self) -> usize {
        let buffer = self.buffer.lock().unwrap();
        buffer.len()
    }

    /// Returns true if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Returns a snapshot (copy) of all events currently in the buffer.
    /// The buffer contents remain unchanged.
    pub fn snapshot(&self) -> Vec<AnalyticsEvent> {
        let buffer = self.buffer.lock().unwrap();
        buffer.iter().cloned().collect()
    }

    /// Removes and returns all events from the buffer, leaving it empty.
    pub fn drain(&self) -> Vec<AnalyticsEvent> {
        let mut buffer = self.buffer.lock().unwrap();
        buffer.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::OperationKind;
    use std::time::Duration;

    fn create_test_event(database: &str) -> AnalyticsEvent {
        AnalyticsEvent::new(
            OperationKind::Find,
            database.to_string(),
            "test_collection".to_string(),
            Duration::from_millis(100),
            true,
        )
    }

    #[test]
    fn test_push_and_len() {
        let buffer = RingBuffer::new(10);
        assert_eq!(buffer.len(), 0);

        buffer.push(create_test_event("db1"));
        assert_eq!(buffer.len(), 1);

        buffer.push(create_test_event("db2"));
        assert_eq!(buffer.len(), 2);
    }

    #[test]
    fn test_overflow_evicts_oldest() {
        let buffer = RingBuffer::new(2);

        buffer.push(create_test_event("db1"));
        buffer.push(create_test_event("db2"));
        assert_eq!(buffer.len(), 2);

        // This should evict the first event (db1)
        buffer.push(create_test_event("db3"));
        assert_eq!(buffer.len(), 2);

        // Verify that db1 was evicted and we have db2 and db3
        let snapshot = buffer.snapshot();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot[0].database, "db2");
        assert_eq!(snapshot[1].database, "db3");
    }

    #[test]
    fn test_snapshot_returns_copy() {
        let buffer = RingBuffer::new(10);

        buffer.push(create_test_event("db1"));
        assert_eq!(buffer.len(), 1);

        let snapshot = buffer.snapshot();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot[0].database, "db1");

        // Buffer should still contain the event
        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn test_drain_empties_buffer() {
        let buffer = RingBuffer::new(10);

        buffer.push(create_test_event("db1"));
        buffer.push(create_test_event("db2"));
        assert_eq!(buffer.len(), 2);

        let drained = buffer.drain();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].database, "db1");
        assert_eq!(drained[1].database, "db2");

        // Buffer should now be empty
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn test_is_empty() {
        let buffer = RingBuffer::new(10);
        assert!(buffer.is_empty());

        buffer.push(create_test_event("db1"));
        assert!(!buffer.is_empty());

        buffer.drain();
        assert!(buffer.is_empty());
    }
}
