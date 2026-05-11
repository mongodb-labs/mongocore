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

        let read_tools = ["find", "aggregate", "count", "list_collections", "list_databases"];

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
}
