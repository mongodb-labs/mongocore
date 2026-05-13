use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 request.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
    #[serde(default)]
    pub id: Option<Value>,
}

/// JSON-RPC 2.0 response.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: Option<Value>,
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// MCP server capabilities advertised during initialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<McpToolsCapability>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<McpResourcesCapability>,
}

/// Tools capability descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolsCapability {
    pub list_changed: bool,
}

/// Resources capability descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpResourcesCapability {
    pub subscribe: bool,
    pub list_changed: bool,
}

/// Definition of an MCP tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Result from an MCP tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpToolCallResult {
    pub content: Vec<McpContent>,
    pub is_error: bool,
}

/// Content block in a tool call result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpContent {
    #[serde(rename = "type")]
    pub type_: String,
    pub text: String,
}

/// Definition of an MCP resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceDefinition {
    pub uri: String,
    pub name: String,
    pub description: String,
    pub mime_type: String,
}

/// MCP sampling request — sent to the host to request an LLM completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSamplingRequest {
    pub method: String,
    pub params: McpSamplingParams,
}

/// Parameters for a sampling request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpSamplingParams {
    pub messages: Vec<McpSamplingMessage>,
    pub max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
}

/// A message in a sampling request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSamplingMessage {
    pub role: String,
    pub content: McpSamplingContent,
}

/// Content of a sampling message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSamplingContent {
    #[serde(rename = "type")]
    pub type_: String,
    pub text: String,
}

/// Response from a sampling request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSamplingResponse {
    pub role: String,
    pub content: McpSamplingContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// MCP Prompt definition for prompts/list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptDefinition {
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<Vec<McpPromptArgument>>,
}

/// An argument for an MCP prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptArgument {
    pub name: String,
    pub description: String,
    pub required: bool,
}

impl JsonRpcResponse {
    /// Create a successful JSON-RPC response.
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    /// Create an error JSON-RPC response.
    pub fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
            id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_json_rpc_request() {
        let raw = r#"{"jsonrpc":"2.0","method":"initialize","id":1}"#;
        let req: JsonRpcRequest = serde_json::from_str(raw).unwrap();
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "initialize");
        assert_eq!(req.id, Some(json!(1)));
        assert!(req.params.is_none());
    }

    #[test]
    fn test_parse_json_rpc_request_with_params() {
        let raw = r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"find"},"id":"abc"}"#;
        let req: JsonRpcRequest = serde_json::from_str(raw).unwrap();
        assert_eq!(req.method, "tools/call");
        assert_eq!(req.params, Some(json!({"name": "find"})));
        assert_eq!(req.id, Some(json!("abc")));
    }

    #[test]
    fn test_json_rpc_response_success_serialization() {
        let resp = JsonRpcResponse::success(Some(json!(1)), json!({"ok": true}));
        let serialized = serde_json::to_value(&resp).unwrap();
        assert_eq!(serialized["jsonrpc"], "2.0");
        assert_eq!(serialized["result"]["ok"], true);
        assert!(serialized.get("error").is_none());
        assert_eq!(serialized["id"], 1);
    }

    #[test]
    fn test_json_rpc_response_error_serialization() {
        let resp = JsonRpcResponse::error(Some(json!(2)), -32601, "Method not found");
        let serialized = serde_json::to_value(&resp).unwrap();
        assert_eq!(serialized["jsonrpc"], "2.0");
        assert!(serialized.get("result").is_none());
        assert_eq!(serialized["error"]["code"], -32601);
        assert_eq!(serialized["error"]["message"], "Method not found");
    }

    #[test]
    fn test_sampling_request_serialization() {
        let req = McpSamplingRequest {
            method: "sampling/createMessage".to_string(),
            params: McpSamplingParams {
                messages: vec![McpSamplingMessage {
                    role: "user".to_string(),
                    content: McpSamplingContent {
                        type_: "text".to_string(),
                        text: "Translate to MQL".to_string(),
                    },
                }],
                max_tokens: 1024,
                system_prompt: None,
            },
        };
        let serialized = serde_json::to_value(&req).unwrap();
        assert_eq!(serialized["method"], "sampling/createMessage");
        assert_eq!(serialized["params"]["maxTokens"], 1024);
        assert_eq!(serialized["params"]["messages"][0]["role"], "user");
    }

    #[test]
    fn test_sampling_response_deserialization() {
        let raw = r#"{"role":"assistant","content":{"type":"text","text":"{\"method\":\"filter\",\"filter\":{\"status\":\"active\"}}"}}"#;
        let resp: McpSamplingResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(resp.role, "assistant");
        assert_eq!(resp.content.type_, "text");
        assert!(resp.content.text.contains("filter"));
    }

    #[test]
    fn test_prompt_definition_serialization() {
        let prompt = McpPromptDefinition {
            name: "explore_dataset".to_string(),
            description: "Explore a database systematically".to_string(),
            arguments: Some(vec![McpPromptArgument {
                name: "database".to_string(),
                description: "Database to explore".to_string(),
                required: true,
            }]),
        };
        let serialized = serde_json::to_value(&prompt).unwrap();
        assert_eq!(serialized["name"], "explore_dataset");
        assert_eq!(serialized["arguments"][0]["name"], "database");
    }
}
