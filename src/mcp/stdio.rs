use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
use tracing::{debug, error};

use crate::compiled::providers::sampling::SamplingRequest;
use crate::compiled::providers::LlmError;

use super::handler::McpHandler;
use super::types::{JsonRpcRequest, JsonRpcResponse};

/// Run the MCP server using stdio transport (stdin/stdout) with MCP sampling support.
///
/// This loop handles both:
/// 1. Incoming JSON-RPC requests from the host (tool calls, initialize, etc.)
/// 2. Outgoing sampling requests from the compiled query translator (sent to host for LLM completion)
///
/// When the translator needs an LLM call, it sends a `SamplingRequest` through the mpsc channel.
/// This loop writes it to stdout as a JSON-RPC request and routes the response back via oneshot.
pub async fn run_stdio_transport(
    handler: Arc<McpHandler>,
    mut sampling_rx: tokio::sync::mpsc::Receiver<SamplingRequest>,
) {
    let stdin = io::stdin();
    let stdout = Arc::new(Mutex::new(io::stdout()));
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    // Track pending sampling responses keyed by request ID
    let pending_sampling: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<Result<String, LlmError>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let mut sampling_id_counter: u64 = 0;

    loop {
        tokio::select! {
            line_result = lines.next_line() => {
                let line = match line_result {
                    Ok(Some(line)) => line,
                    Ok(None) => break, // stdin closed
                    Err(_) => break,
                };

                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }

                // Try to parse as a JSON-RPC response first (could be a sampling response)
                let value: serde_json::Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(e) => {
                        error!("Failed to parse JSON-RPC: {}", e);
                        let err_resp = JsonRpcResponse::error(None, -32700, format!("Parse error: {}", e));
                        write_line(&stdout, &serde_json::to_string(&err_resp).unwrap_or_default()).await;
                        continue;
                    }
                };

                // Check if this is a response to a pending sampling request
                if value.get("result").is_some() || value.get("error").is_some() {
                    if let Some(id) = value.get("id").and_then(|v| v.as_str()) {
                        let mut pending = pending_sampling.lock().await;
                        if let Some(response_tx) = pending.remove(id) {
                            // Route sampling response back to the provider
                            let result = if let Some(result) = value.get("result") {
                                // Extract text content from sampling response
                                let text = result.get("content")
                                    .and_then(|c| c.get("text"))
                                    .and_then(|t| t.as_str())
                                    .or_else(|| result.get("text").and_then(|t| t.as_str()))
                                    .unwrap_or("");
                                Ok(text.to_string())
                            } else if let Some(err) = value.get("error") {
                                let msg = err.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
                                Err(LlmError::ApiError(format!("Sampling error: {}", msg)))
                            } else {
                                Err(LlmError::ApiError("Invalid sampling response".to_string()))
                            };
                            let _ = response_tx.send(result);
                            continue;
                        }
                    }
                }

                // Normal JSON-RPC request from host
                let request: JsonRpcRequest = match serde_json::from_value(value) {
                    Ok(req) => req,
                    Err(e) => {
                        error!("Failed to parse as JSON-RPC request: {}", e);
                        let err_resp = JsonRpcResponse::error(None, -32700, format!("Parse error: {}", e));
                        write_line(&stdout, &serde_json::to_string(&err_resp).unwrap_or_default()).await;
                        continue;
                    }
                };

                debug!("stdio request: {}", request.method);
                let response = handler.handle_request(request).await;
                let resp_json = serde_json::to_string(&response).unwrap_or_default();
                write_line(&stdout, &resp_json).await;
            }

            Some(sampling_req) = sampling_rx.recv() => {
                // Send sampling request to the host via stdout
                sampling_id_counter += 1;
                let request_id = format!("sampling-{}", sampling_id_counter);

                let messages = vec![
                    serde_json::json!({
                        "role": "user",
                        "content": { "type": "text", "text": sampling_req.prompt }
                    })
                ];

                let mut params = serde_json::json!({
                    "messages": messages,
                    "maxTokens": 2048
                });
                if let Some(system) = &sampling_req.system {
                    params["systemPrompt"] = serde_json::json!(system);
                }

                let jsonrpc_req = serde_json::json!({
                    "jsonrpc": "2.0",
                    "method": "sampling/createMessage",
                    "params": params,
                    "id": request_id
                });

                // Store the response channel before sending
                {
                    let mut pending = pending_sampling.lock().await;
                    pending.insert(request_id.clone(), sampling_req.response_tx);
                }

                let req_json = serde_json::to_string(&jsonrpc_req).unwrap_or_default();
                write_line(&stdout, &req_json).await;
                debug!("Sent sampling request: {}", request_id);
            }
        }
    }

    // Clean up any pending sampling requests
    let mut pending = pending_sampling.lock().await;
    for (_id, tx) in pending.drain() {
        let _ = tx.send(Err(LlmError::ApiError("Stdio transport shutting down".to_string())));
    }
}

async fn write_line(stdout: &Arc<Mutex<io::Stdout>>, data: &str) {
    let mut out = stdout.lock().await;
    if let Err(e) = out.write_all(data.as_bytes()).await {
        error!("Failed to write to stdout: {}", e);
        return;
    }
    if let Err(e) = out.write_all(b"\n").await {
        error!("Failed to write newline: {}", e);
        return;
    }
    if let Err(e) = out.flush().await {
        error!("Failed to flush stdout: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_jsonrpc_line() {
        let line = r#"{"jsonrpc":"2.0","method":"initialize","id":1}"#;
        let req: JsonRpcRequest = serde_json::from_str(line).unwrap();
        assert_eq!(req.method, "initialize");
    }

    #[test]
    fn test_parse_invalid_jsonrpc_line() {
        let line = "not json";
        let result: Result<JsonRpcRequest, _> = serde_json::from_str(line);
        assert!(result.is_err());
    }
}
