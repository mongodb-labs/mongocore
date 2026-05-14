use serde_json::{json, Value};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::analytics::AnalyticsCollector;
use crate::compiled::translator::CompiledQueryTranslator;
use crate::connection::pool::ConnectionPool;
use crate::ingestion::engine::IngestionEngine;
use crate::ingestion::watch::DirectoryWatcher;
use crate::operations::Operations;
use crate::voyage::client::VoyageClient;

use super::resources;
use super::safety::SafetyConfig;
use super::skills::registry::SkillRegistry;
use super::tools;
use super::types::{JsonRpcRequest, JsonRpcResponse, McpContent, McpToolCallResult};

/// MCP request handler that dispatches JSON-RPC methods to operations.
pub struct McpHandler {
    operations: Operations,
    pool: ConnectionPool,
    safety: SafetyConfig,
    analytics: Option<Arc<AnalyticsCollector>>,
    ingestion: Option<Arc<IngestionEngine>>,
    watcher: Option<Arc<DirectoryWatcher>>,
    translator: Option<Arc<CompiledQueryTranslator>>,
    voyage: Option<Arc<VoyageClient>>,
    is_stdio: bool,
    mcp_metadata_appended: AtomicBool,
    skills: SkillRegistry,
}

impl McpHandler {
    /// Create a new handler backed by the given operations instance and connection pool.
    pub fn new(
        operations: Operations,
        pool: ConnectionPool,
        safety: SafetyConfig,
        analytics: Option<Arc<AnalyticsCollector>>,
        ingestion: Option<Arc<IngestionEngine>>,
        watcher: Option<Arc<DirectoryWatcher>>,
        translator: Option<Arc<CompiledQueryTranslator>>,
        voyage: Option<Arc<VoyageClient>>,
        is_stdio: bool,
    ) -> Self {
        Self {
            operations,
            pool,
            safety,
            analytics,
            ingestion,
            watcher,
            translator,
            voyage,
            is_stdio,
            mcp_metadata_appended: AtomicBool::new(false),
            skills: SkillRegistry::new(),
        }
    }

    /// Handle a single JSON-RPC request and return a response.
    #[tracing::instrument(skip(self), fields(method = %request.method))]
    pub async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        if !self.mcp_metadata_appended.load(Ordering::Relaxed) {
            self.pool.append_interface_metadata("mcp");
            self.mcp_metadata_appended.store(true, Ordering::Relaxed);
        }

        let id = request.id.clone();
        match request.method.as_str() {
            "initialize" => self.handle_initialize(id),
            "tools/list" => self.handle_tools_list(id),
            "tools/call" => self.handle_tools_call(id, request.params).await,
            "resources/list" => self.handle_resources_list(id),
            "resources/read" => self.handle_resources_read(id, request.params).await,
            "prompts/list" => self.handle_prompts_list(id),
            "prompts/get" => self.handle_prompts_get(id, request.params),
            _ => JsonRpcResponse::error(id, -32601, "Method not found"),
        }
    }

    fn handle_initialize(&self, id: Option<Value>) -> JsonRpcResponse {
        let mut capabilities = json!({
            "tools": { "listChanged": false },
            "resources": { "subscribe": false, "listChanged": false }
        });

        if self.is_stdio {
            capabilities.as_object_mut().unwrap().insert(
                "prompts".to_string(),
                json!({ "listChanged": false }),
            );
        }

        JsonRpcResponse::success(
            id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": capabilities,
                "serverInfo": {
                    "name": "mongocore",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
    }

    fn handle_tools_list(&self, id: Option<Value>) -> JsonRpcResponse {
        let definitions = tools::tool_definitions();
        let tools_json: Vec<Value> = definitions
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": t.input_schema
                })
            })
            .collect();
        JsonRpcResponse::success(id, json!({ "tools": tools_json }))
    }

    async fn handle_tools_call(&self, id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
        let tool_name = match params
            .as_ref()
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
        {
            Some(name) => name.to_string(),
            None => {
                return JsonRpcResponse::error(id, -32602, "Missing tool name in params");
            }
        };

        // Safety check: block disallowed tools before execution
        if let Err(reason) = self.safety.check_tool_allowed(&tool_name) {
            let result = McpToolCallResult {
                content: vec![McpContent {
                    type_: "text".to_string(),
                    text: reason,
                }],
                is_error: true,
            };
            return JsonRpcResponse::success(
                id,
                serde_json::to_value(&result).unwrap_or(json!(null)),
            );
        }

        let arguments = params
            .as_ref()
            .and_then(|p| p.get("arguments"))
            .cloned()
            .unwrap_or(json!({}));

        let result =
            tools::execute_tool(&self.operations, &self.pool, self.analytics.as_ref(), self.ingestion.as_ref(), self.watcher.as_ref(), &self.safety, self.translator.as_ref(), self.voyage.as_ref(), &self.skills, &tool_name, &arguments).await;

        JsonRpcResponse::success(id, serde_json::to_value(&result).unwrap_or(json!(null)))
    }

    fn handle_resources_list(&self, id: Option<Value>) -> JsonRpcResponse {
        let definitions = resources::resource_definitions(&self.pool);
        let resources_json: Vec<Value> = definitions
            .iter()
            .map(|r| {
                json!({
                    "uri": r.uri,
                    "name": r.name,
                    "description": r.description,
                    "mimeType": r.mime_type
                })
            })
            .collect();
        JsonRpcResponse::success(id, json!({ "resources": resources_json }))
    }

    async fn handle_resources_read(
        &self,
        id: Option<Value>,
        params: Option<Value>,
    ) -> JsonRpcResponse {
        let uri = params
            .as_ref()
            .and_then(|p| p.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("unknown");

        match resources::read_resource(&self.pool, uri).await {
            Ok(content) => JsonRpcResponse::success(
                id,
                json!({
                    "contents": [{
                        "uri": uri,
                        "mimeType": "application/json",
                        "text": content
                    }]
                }),
            ),
            Err(e) => JsonRpcResponse::error(id, -32602, e),
        }
    }

    fn handle_prompts_list(&self, id: Option<Value>) -> JsonRpcResponse {
        let prompts: Vec<Value> = self.skills.list(None)
            .iter()
            .map(|skill| {
                json!({
                    "name": skill.name,
                    "description": skill.description,
                    "arguments": skill.arguments.iter().map(|a| json!({
                        "name": a.name,
                        "description": a.description,
                        "required": a.required
                    })).collect::<Vec<_>>()
                })
            })
            .collect();
        JsonRpcResponse::success(id, json!({ "prompts": prompts }))
    }

    fn handle_prompts_get(&self, id: Option<Value>, params: Option<Value>) -> JsonRpcResponse {
        let name = match params.as_ref().and_then(|p| p.get("name")).and_then(|n| n.as_str()) {
            Some(n) => n,
            None => return JsonRpcResponse::error(id, -32602, "Missing prompt name"),
        };

        let skill = match self.skills.get(name) {
            Some(s) => s,
            None => return JsonRpcResponse::error(id, -32602, format!("Skill not found: {}", name)),
        };

        let steps_text: String = skill.steps.iter().enumerate()
            .map(|(i, step)| {
                let prefix = if step.tool.is_some() {
                    format!("**Step {}:** [Tool: {}]", i + 1, step.tool.as_deref().unwrap_or(""))
                } else if step.analysis {
                    format!("**Step {}:** [Analysis]", i + 1)
                } else if step.synthesis {
                    format!("**Step {}:** [Synthesis]", i + 1)
                } else {
                    format!("**Step {}:**", i + 1)
                };
                format!("{} {}", prefix, step.description)
            })
            .collect::<Vec<_>>()
            .join("\n");

        let message_text = format!(
            "I'll guide you through: {}\n\n{}\n\nStarting now...",
            skill.description, steps_text
        );

        JsonRpcResponse::success(id, json!({
            "description": skill.description,
            "messages": [{
                "role": "assistant",
                "content": { "type": "text", "text": message_text }
            }]
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_request(method: &str, params: Option<Value>, id: Value) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: Some(id),
        }
    }

    #[tokio::test]
    async fn test_initialize_response() {
        let req = make_request("initialize", None, json!(1));
        assert_eq!(req.method, "initialize");
        assert_eq!(req.id, Some(json!(1)));
    }

    #[test]
    fn test_initialize_response_shape() {
        let expected = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": { "listChanged": false },
                "resources": { "subscribe": false, "listChanged": false }
            },
            "serverInfo": {
                "name": "mongocore",
                "version": env!("CARGO_PKG_VERSION")
            }
        });
        assert!(expected["protocolVersion"].is_string());
        assert_eq!(expected["serverInfo"]["name"], "mongocore");
        assert!(expected["serverInfo"]["version"].is_string());
        assert_eq!(
            expected["capabilities"]["tools"]["listChanged"],
            json!(false)
        );
    }

    #[test]
    fn test_unknown_method_error() {
        let resp = JsonRpcResponse::error(Some(json!(99)), -32601, "Method not found");
        assert_eq!(resp.error.as_ref().unwrap().code, -32601);
        assert_eq!(resp.error.as_ref().unwrap().message, "Method not found");
        assert_eq!(resp.id, Some(json!(99)));
    }

    #[test]
    fn test_tool_not_found_error() {
        let resp = JsonRpcResponse::error(Some(json!(5)), -32602, "Tool not found: nonexistent");
        assert_eq!(resp.error.as_ref().unwrap().code, -32602);
        assert!(resp.error.as_ref().unwrap().message.contains("nonexistent"));
    }

    #[test]
    fn test_tools_list_returns_definitions() {
        let definitions = tools::tool_definitions();
        assert!(!definitions.is_empty());
        assert_eq!(definitions.len(), 36);
    }
}
