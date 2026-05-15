use chrono::{DateTime, Utc};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct OperationRecord {
    pub index: usize,
    pub tool_name: String,
    pub params: Value,
    pub context: Value,
    pub success: bool,
    pub error_message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Default)]
pub struct SessionRecorder {
    operations: Vec<OperationRecord>,
}

/// Tools excluded from recording (meta/diagnostic)
const EXCLUDED_TOOLS: &[&str] = &[
    "explain_last",
    "explain_session",
    "get_analytics",
    "collection_schema",
];

impl SessionRecorder {
    pub fn new() -> Self {
        Self { operations: Vec::new() }
    }

    pub fn should_record(tool_name: &str) -> bool {
        !EXCLUDED_TOOLS.contains(&tool_name)
    }

    pub fn record(
        &mut self,
        tool_name: String,
        params: Value,
        context: Value,
        success: bool,
        error_message: Option<String>,
    ) {
        let index = self.operations.len();
        self.operations.push(OperationRecord {
            index,
            tool_name,
            params,
            context,
            success,
            error_message,
            timestamp: Utc::now(),
        });
    }

    pub fn get_last(&self, offset: usize) -> Option<&OperationRecord> {
        if offset >= self.operations.len() {
            return None;
        }
        self.operations.get(self.operations.len() - 1 - offset)
    }

    pub fn get_all(&self) -> &[OperationRecord] {
        &self.operations
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    pub fn len(&self) -> usize {
        self.operations.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_record_and_retrieve_last() {
        let mut recorder = SessionRecorder::new();
        recorder.record(
            "insert_many".to_string(),
            json!({"database": "mydb", "collection": "users", "documents": []}),
            json!({"operation": "insert_many", "database": "mydb", "collection": "users", "document_count": 0}),
            true,
            None,
        );
        recorder.record(
            "find".to_string(),
            json!({"database": "mydb", "collection": "users", "filter": {}}),
            json!({"operation": "find", "database": "mydb", "collection": "users", "filter": {}}),
            true,
            None,
        );

        let last = recorder.get_last(0).unwrap();
        assert_eq!(last.tool_name, "find");
        assert_eq!(last.index, 1);

        let prev = recorder.get_last(1).unwrap();
        assert_eq!(prev.tool_name, "insert_many");
        assert_eq!(prev.index, 0);

        assert!(recorder.get_last(2).is_none());
    }

    #[test]
    fn test_excluded_tools() {
        assert!(!SessionRecorder::should_record("explain_last"));
        assert!(!SessionRecorder::should_record("explain_session"));
        assert!(!SessionRecorder::should_record("get_analytics"));
        assert!(!SessionRecorder::should_record("collection_schema"));
        assert!(SessionRecorder::should_record("find"));
        assert!(SessionRecorder::should_record("insert_many"));
        assert!(SessionRecorder::should_record("list_collections"));
    }

    #[test]
    fn test_error_recording() {
        let mut recorder = SessionRecorder::new();
        recorder.record(
            "delete_many".to_string(),
            json!({"database": "mydb", "collection": "users", "filter": {"status": "inactive"}}),
            json!({"operation": "delete_many", "database": "mydb", "collection": "users", "filter": {"status": "inactive"}}),
            false,
            Some("permission denied".to_string()),
        );

        let last = recorder.get_last(0).unwrap();
        assert!(!last.success);
        assert_eq!(last.error_message.as_deref(), Some("permission denied"));
    }

    #[test]
    fn test_empty_session() {
        let recorder = SessionRecorder::new();
        assert!(recorder.is_empty());
        assert_eq!(recorder.len(), 0);
        assert!(recorder.get_last(0).is_none());
    }

    #[test]
    fn test_get_all() {
        let mut recorder = SessionRecorder::new();
        recorder.record("find".to_string(), json!({}), json!({}), true, None);
        recorder.record("insert".to_string(), json!({}), json!({}), true, None);
        recorder.record("update".to_string(), json!({}), json!({}), true, None);

        let all = recorder.get_all();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].tool_name, "find");
        assert_eq!(all[1].tool_name, "insert");
        assert_eq!(all[2].tool_name, "update");
    }
}
