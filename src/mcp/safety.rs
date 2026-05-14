use serde_json::Value;

/// Safety controls for AI agents interacting with MongoDB through MongoCore.
#[derive(Debug, Clone)]
pub struct SafetyConfig {
    /// When true, all write operations are blocked.
    pub read_only: bool,
    /// Maximum number of documents returned by queries.
    pub max_documents: usize,
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            read_only: false,
            max_documents: 100,
        }
    }
}

impl SafetyConfig {
    /// Check if a tool call is allowed under current safety config.
    /// Returns Ok(()) if allowed, Err(reason) if blocked.
    pub fn check_tool_allowed(&self, tool_name: &str) -> Result<(), String> {
        if self.read_only {
            const WRITE_TOOLS: &[&str] = &[
                "insert",
                "insert_many",
                "update",
                "update_many",
                "delete",
                "delete_many",
                "create_collection",
                "create_index",
                "run_command",
                "transaction_pipeline",
            ];
            if WRITE_TOOLS.contains(&tool_name) {
                return Err(format!(
                    "Tool '{}' is blocked: server is in read-only mode",
                    tool_name
                ));
            }
        }
        Ok(())
    }

    /// Check if all operations in a pipeline are allowed under current safety config.
    /// Returns Ok(()) if all allowed, Err(reason) if any violate safety rules.
    /// This provides all-or-nothing validation — if any operation violates rules,
    /// the entire pipeline is rejected before execution.
    pub fn check_pipeline_allowed(&self, operations: &[Value]) -> Result<(), String> {
        if !self.read_only {
            return Ok(());
        }

        let mut violations = Vec::new();
        for (i, op) in operations.iter().enumerate() {
            if let Some(op_type) = op.get("op").and_then(|v| v.as_str()) {
                const WRITE_OPS: &[&str] = &[
                    "insert",
                    "insert_many",
                    "update",
                    "update_many",
                    "delete",
                    "delete_many",
                    "create_collection",
                    "create_index",
                    "run_command",
                    "find_and_modify",
                ];
                if WRITE_OPS.contains(&op_type) {
                    violations.push(format!(
                        "operation[{}]: '{}' is a write operation",
                        i, op_type
                    ));
                }
            }
        }

        if violations.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Pipeline rejected: server is in read-only mode. Violations:\n{}",
                violations.join("\n")
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = SafetyConfig::default();
        assert!(!config.read_only);
        assert_eq!(config.max_documents, 100);
    }

    #[test]
    fn test_read_only_blocks_write_tools() {
        let config = SafetyConfig {
            read_only: true,
            max_documents: 100,
        };

        let write_tools = [
            "insert",
            "insert_many",
            "update",
            "update_many",
            "delete",
            "delete_many",
            "create_collection",
            "create_index",
            "run_command",
        ];

        for tool in &write_tools {
            let result = config.check_tool_allowed(tool);
            assert!(result.is_err(), "Expected '{}' to be blocked", tool);
            assert!(result.unwrap_err().contains("read-only mode"));
        }
    }

    #[test]
    fn test_read_only_allows_read_tools() {
        let config = SafetyConfig {
            read_only: true,
            max_documents: 100,
        };

        let read_tools = [
            "find",
            "aggregate",
            "count",
            "list_collections",
            "list_databases",
        ];

        for tool in &read_tools {
            let result = config.check_tool_allowed(tool);
            assert!(result.is_ok(), "Expected '{}' to be allowed", tool);
        }
    }

    #[test]
    fn test_non_read_only_allows_all_tools() {
        let config = SafetyConfig {
            read_only: false,
            max_documents: 100,
        };

        let all_tools = ["find", "insert", "update", "delete", "aggregate"];

        for tool in &all_tools {
            let result = config.check_tool_allowed(tool);
            assert!(result.is_ok(), "Expected '{}' to be allowed", tool);
        }
    }

    #[test]
    fn test_pipeline_allowed_when_not_read_only() {
        let safety = SafetyConfig {
            read_only: false,
            max_documents: 100,
        };
        let ops = vec![
            serde_json::json!({"op": "insert"}),
            serde_json::json!({"op": "delete"}),
        ];
        assert!(safety.check_pipeline_allowed(&ops).is_ok());
    }

    #[test]
    fn test_pipeline_blocked_when_read_only_with_writes() {
        let safety = SafetyConfig {
            read_only: true,
            max_documents: 100,
        };
        let ops = vec![
            serde_json::json!({"op": "find"}),
            serde_json::json!({"op": "insert"}),
            serde_json::json!({"op": "delete_many"}),
        ];
        let err = safety.check_pipeline_allowed(&ops).unwrap_err();
        assert!(err.contains("operation[1]: 'insert'"));
        assert!(err.contains("operation[2]: 'delete_many'"));
        assert!(!err.contains("operation[0]"));
    }

    #[test]
    fn test_pipeline_allowed_when_read_only_with_only_reads() {
        let safety = SafetyConfig {
            read_only: true,
            max_documents: 100,
        };
        let ops = vec![
            serde_json::json!({"op": "find"}),
            serde_json::json!({"op": "find_one"}),
            serde_json::json!({"op": "aggregate"}),
            serde_json::json!({"op": "list_databases"}),
        ];
        assert!(safety.check_pipeline_allowed(&ops).is_ok());
    }

    #[test]
    fn test_pipeline_find_and_modify_blocked_in_read_only() {
        let safety = SafetyConfig {
            read_only: true,
            max_documents: 100,
        };
        let ops = vec![serde_json::json!({"op": "find_and_modify"})];
        assert!(safety.check_pipeline_allowed(&ops).is_err());
    }

    #[test]
    fn test_pipeline_empty_ops_allowed() {
        let safety = SafetyConfig {
            read_only: true,
            max_documents: 100,
        };
        let ops: Vec<serde_json::Value> = vec![];
        assert!(safety.check_pipeline_allowed(&ops).is_ok());
    }
}
