# MCP + Claude Integration — Phase 1: Foundation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

**Goal:** Add stdio transport for MCP (enabling Claude Desktop/Code integration), implement the `collection_schema` tool, add `ask` and `explain_query` tools using the existing compiled query engine, and integrate MCP sampling as a zero-config LLM fallback.

**Architecture:** The existing HTTP-based MCP server (`src/mcp/server.rs`) stays unchanged. A new `--stdio` flag adds an alternative transport that reads JSON-RPC from stdin and writes responses to stdout. The `McpHandler` is transport-agnostic — both HTTP and stdio use the same handler. A new `McpSamplingProvider` implements the `LlmProvider` trait, sending `sampling/createMessage` requests to the MCP host instead of calling an external API.

**Tech Stack:** Rust, tokio (stdin/stdout async), serde_json (JSON-RPC framing), existing `LlmProvider` trait, existing `CompiledQueryTranslator`.

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `src/config.rs` | Modify | Add `--stdio` CLI flag |
| `src/main.rs` | Modify | Branch on `--stdio`: skip gRPC, use stdio transport |
| `src/mcp/stdio.rs` | Create | Stdio transport: read JSON-RPC from stdin, write to stdout |
| `src/mcp/mod.rs` | Modify | Export new module |
| `src/mcp/handler.rs` | Modify | Add `prompts/list`, `prompts/get`, `sampling/createMessage` support; add `collection_schema`, `ask`, `explain_query` dispatch |
| `src/mcp/tools.rs` | Modify | Add `collection_schema`, `ask`, `explain_query` tool definitions and implementations |
| `src/mcp/types.rs` | Modify | Add `McpSamplingRequest`, `McpSamplingResponse`, `McpPromptDefinition` types |
| `src/compiled/providers/sampling.rs` | Create | `McpSamplingProvider` implementing `LlmProvider` via MCP sampling |
| `src/compiled/providers/mod.rs` | Modify | Export sampling module |
| `tests/integration/mcp_stdio_test.rs` | Create | Spawn MongoCore as child process, test stdio JSON-RPC |

---

### Task 1: Add `--stdio` CLI flag

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Write the failing test**

Add a test to `src/config.rs` in the existing `mod tests` block:

```rust
#[test]
fn test_stdio_flag_default_false() {
    let cli = default_cli();
    assert!(!cli.stdio);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib config::tests::test_stdio_flag_default_false`
Expected: FAIL — `stdio` field doesn't exist on `CliArgs`

- [ ] **Step 3: Add the `--stdio` flag to `CliArgs`**

In `src/config.rs`, add to the `CliArgs` struct after the `otel_service_name` field:

```rust
    /// Run in MCP stdio mode (stdin/stdout JSON-RPC, no gRPC server)
    #[arg(long, env = "MONGOCORE_STDIO")]
    pub stdio: bool,
```

Also add `stdio: false` to every `CliArgs` struct literal in the test module (`default_cli()` and any other places).

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib config::tests::test_stdio_flag_default_false`
Expected: PASS

- [ ] **Step 5: Verify zero warnings**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output

- [ ] **Step 6: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): add --stdio flag for MCP stdio transport mode"
```

---

### Task 2: Create stdio transport

**Files:**
- Create: `src/mcp/stdio.rs`
- Modify: `src/mcp/mod.rs`

- [ ] **Step 1: Write the module with a basic test**

Create `src/mcp/stdio.rs`:

```rust
use std::sync::Arc;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error};

use super::handler::McpHandler;
use super::types::{JsonRpcRequest, JsonRpcResponse};

/// Run the MCP server using stdio transport (stdin/stdout).
/// Each line on stdin is a JSON-RPC request; each response is written as a line to stdout.
pub async fn run_stdio_transport(handler: Arc<McpHandler>) {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        let request: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                let err_resp = JsonRpcResponse::error(
                    None,
                    -32700,
                    format!("Parse error: {}", e),
                );
                let resp_json = serde_json::to_string(&err_resp).unwrap_or_default();
                let _ = stdout.write_all(resp_json.as_bytes()).await;
                let _ = stdout.write_all(b"\n").await;
                let _ = stdout.flush().await;
                continue;
            }
        };

        debug!("stdio request: {}", request.method);
        let response = handler.handle_request(request).await;
        let resp_json = serde_json::to_string(&response).unwrap_or_default();
        let _ = stdout.write_all(resp_json.as_bytes()).await;
        let _ = stdout.write_all(b"\n").await;
        let _ = stdout.flush().await;
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
```

- [ ] **Step 2: Export the module from `src/mcp/mod.rs`**

Add to `src/mcp/mod.rs`:

```rust
pub mod stdio;
```

And add to the `pub use` line:

```rust
pub use stdio::run_stdio_transport;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib mcp::stdio::tests`
Expected: PASS (2 tests)

- [ ] **Step 4: Verify zero warnings**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output

- [ ] **Step 5: Commit**

```bash
git add src/mcp/stdio.rs src/mcp/mod.rs
git commit -m "feat(mcp): add stdio transport for MCP JSON-RPC over stdin/stdout"
```

---

### Task 3: Wire stdio mode into `main.rs`

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Modify `main.rs` to branch on `--stdio`**

After the `print_banner(&config)` call and MongoDB connection, add the stdio branch. The key change: when `cli.stdio` is true, skip the gRPC server and run `run_stdio_transport` instead.

Replace the section from `// Start gRPC server` to the `tokio::select!` block with:

```rust
    if cli.stdio {
        // Stdio MCP mode: no gRPC server, no banner on stdout, logs to stderr
        use mongocore::mcp::run_stdio_transport;
        use mongocore::mcp::handler::McpHandler;
        use mongocore::mcp::safety::SafetyConfig;
        use mongocore::operations::Operations;

        let operations = Operations::new(pool.clone());
        let safety = SafetyConfig::default();
        let handler = McpHandler::new(operations, pool, safety, analytics, ingestion_engine, directory_watcher);
        let handler = Arc::new(handler);

        run_stdio_transport(handler).await;
    } else {
        // Normal mode: gRPC + HTTP MCP servers
        let grpc_handle = start_grpc_server(
            pool.clone(),
            config.grpc_port,
            voyage_api_key.as_deref(),
            analytics.clone(),
            ingestion_engine.clone(),
            directory_watcher.clone(),
        );

        let mcp_handle = start_mcp_server(pool.clone(), config.mcp_port, analytics, ingestion_engine, directory_watcher);

        info!("MongoCore started successfully");

        tokio::select! {
            result = grpc_handle => {
                match result {
                    Ok(Ok(())) => info!("gRPC server shut down"),
                    Ok(Err(e)) => error!("gRPC server error: {e}"),
                    Err(e) => error!("gRPC server task panicked: {e}"),
                }
            }
            result = mcp_handle => {
                match result {
                    Ok(()) => info!("MCP server shut down"),
                    Err(e) => error!("MCP server task panicked: {e}"),
                }
            }
        }
    }
```

Also move the `print_banner` call inside the `else` branch (don't print banner in stdio mode — stdout is the transport).

- [ ] **Step 2: Make `McpHandler` public**

In `src/mcp/handler.rs`, ensure `McpHandler` and its `new()` are `pub`. Check the current visibility — they already are public based on the code read. Also ensure `McpHandler` is re-exported from `src/mcp/mod.rs`:

```rust
pub use handler::McpHandler;
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output (zero warnings)

- [ ] **Step 4: Verify all unit tests pass**

Run: `cargo test --lib`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/mcp/mod.rs
git commit -m "feat(mcp): wire --stdio mode into main, skip gRPC in stdio mode"
```

---

### Task 4: Implement `collection_schema` tool

**Files:**
- Modify: `src/mcp/tools.rs`

- [ ] **Step 1: Add tool definition**

In the `tool_definitions()` function in `src/mcp/tools.rs`, add after the existing tool definitions:

```rust
    McpToolDefinition {
        name: "collection_schema".to_string(),
        description: "Sample documents from a collection and infer the schema (field names, BSON types, cardinality, example values)".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "database": { "type": "string", "description": "Database name" },
                "collection": { "type": "string", "description": "Collection name" },
                "sample_size": { "type": "integer", "description": "Number of documents to sample (default 100)", "default": 100 }
            },
            "required": ["database", "collection"]
        }),
    },
```

- [ ] **Step 2: Add execution handler**

In the `execute_tool()` function, add a match arm for `"collection_schema"`:

```rust
        "collection_schema" => {
            let database = args["database"].as_str().unwrap_or("test");
            let collection = args["collection"].as_str().unwrap_or("test");
            let sample_size = args.get("sample_size")
                .and_then(|v| v.as_u64())
                .unwrap_or(100) as i64;

            let db = pool.client().database(database);
            let coll = db.collection::<bson::Document>(collection);

            // Sample documents using $sample aggregation
            let pipeline = vec![bson::doc! { "$sample": { "size": sample_size } }];
            let mut cursor = match coll.aggregate(pipeline).await {
                Ok(c) => c,
                Err(e) => return error_result(&format!("Failed to sample: {}", e)),
            };

            let mut docs: Vec<bson::Document> = Vec::new();
            while let Ok(Some(doc)) = cursor.advance().await.and_then(|advanced| {
                if advanced { Ok(Some(cursor.deserialize_current().map_err(|e| e)?)) } else { Ok(None) }
            }) {
                docs.push(doc);
            }

            // Infer schema from sampled documents
            let mut field_info: std::collections::HashMap<String, FieldSchema> = std::collections::HashMap::new();
            let total_docs = docs.len();

            for doc in &docs {
                collect_fields(doc, "", &mut field_info);
            }

            // Build response
            let fields: Vec<serde_json::Value> = field_info.iter()
                .map(|(path, info)| {
                    let frequency = if total_docs > 0 {
                        (info.count as f64 / total_docs as f64 * 100.0).round()
                    } else {
                        0.0
                    };
                    json!({
                        "path": path,
                        "types": info.types.iter().collect::<Vec<_>>(),
                        "frequency_percent": frequency,
                        "count": info.count,
                        "example": info.example
                    })
                })
                .collect();

            let result = json!({
                "database": database,
                "collection": collection,
                "documents_sampled": total_docs,
                "fields": fields
            });

            success_result(&serde_json::to_string_pretty(&result).unwrap_or_default())
        }
```

- [ ] **Step 3: Add helper types and functions**

Add these at the bottom of `src/mcp/tools.rs` (before the `#[cfg(test)]` block):

```rust
struct FieldSchema {
    types: std::collections::HashSet<String>,
    count: usize,
    example: Option<serde_json::Value>,
}

fn collect_fields(
    doc: &bson::Document,
    prefix: &str,
    fields: &mut std::collections::HashMap<String, FieldSchema>,
) {
    for (key, value) in doc {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };

        let type_name = bson_type_name(value);
        let entry = fields.entry(path.clone()).or_insert_with(|| FieldSchema {
            types: std::collections::HashSet::new(),
            count: 0,
            example: None,
        });
        entry.types.insert(type_name);
        entry.count += 1;
        if entry.example.is_none() {
            entry.example = bson_to_json_example(value);
        }

        // Recurse into nested documents
        if let bson::Bson::Document(nested) = value {
            collect_fields(nested, &path, fields);
        }
    }
}

fn bson_type_name(value: &bson::Bson) -> String {
    match value {
        bson::Bson::Double(_) => "Double".to_string(),
        bson::Bson::String(_) => "String".to_string(),
        bson::Bson::Array(_) => "Array".to_string(),
        bson::Bson::Document(_) => "Document".to_string(),
        bson::Bson::Boolean(_) => "Boolean".to_string(),
        bson::Bson::Null => "Null".to_string(),
        bson::Bson::Int32(_) => "Int32".to_string(),
        bson::Bson::Int64(_) => "Int64".to_string(),
        bson::Bson::ObjectId(_) => "ObjectId".to_string(),
        bson::Bson::DateTime(_) => "DateTime".to_string(),
        _ => "Other".to_string(),
    }
}

fn bson_to_json_example(value: &bson::Bson) -> Option<serde_json::Value> {
    match value {
        bson::Bson::String(s) => {
            let truncated = if s.len() > 50 { &s[..50] } else { s };
            Some(json!(truncated))
        }
        bson::Bson::Int32(n) => Some(json!(n)),
        bson::Bson::Int64(n) => Some(json!(n)),
        bson::Bson::Double(n) => Some(json!(n)),
        bson::Bson::Boolean(b) => Some(json!(b)),
        bson::Bson::ObjectId(oid) => Some(json!(oid.to_hex())),
        _ => None,
    }
}
```

- [ ] **Step 4: Update tool count assertion**

In `tests/integration/mcp_test.rs`, update the tool count assertion from 21 to 22.

- [ ] **Step 5: Verify it compiles with zero warnings**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output

- [ ] **Step 6: Run unit tests**

Run: `cargo test --lib`
Expected: All pass (tool count in `handler.rs` test also needs updating from 21 to 22)

- [ ] **Step 7: Commit**

```bash
git add src/mcp/tools.rs tests/integration/mcp_test.rs src/mcp/handler.rs
git commit -m "feat(mcp): add collection_schema tool for schema inference via sampling"
```

---

### Task 5: Add MCP sampling types

**Files:**
- Modify: `src/mcp/types.rs`

- [ ] **Step 1: Add sampling request/response types**

Add to `src/mcp/types.rs`:

```rust
/// MCP sampling request — sent to the host to request an LLM completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSamplingRequest {
    pub method: String, // "sampling/createMessage"
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
    pub role: String, // "user" or "assistant"
    pub content: McpSamplingContent,
}

/// Content of a sampling message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpSamplingContent {
    #[serde(rename = "type")]
    pub type_: String, // "text"
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
```

- [ ] **Step 2: Add tests for new types**

Add to the existing `mod tests` block in `src/mcp/types.rs`:

```rust
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
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib mcp::types::tests`
Expected: All pass

- [ ] **Step 4: Commit**

```bash
git add src/mcp/types.rs
git commit -m "feat(mcp): add sampling request/response and prompt definition types"
```

---

### Task 6: Implement `McpSamplingProvider`

**Files:**
- Create: `src/compiled/providers/sampling.rs`
- Modify: `src/compiled/providers/mod.rs`

- [ ] **Step 1: Create the sampling provider**

Create `src/compiled/providers/sampling.rs`:

```rust
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

use super::{LlmError, LlmProvider, TranslationContext};
use super::prompt::build_translation_prompt;

/// Callback type for sending MCP sampling requests and receiving responses.
/// The callback takes the prompt text and returns the LLM response text.
pub type SamplingCallback = Arc<dyn Fn(String) -> tokio::sync::oneshot::Receiver<Result<String, String>> + Send + Sync>;

/// LLM provider that delegates to the MCP host via the sampling protocol.
/// Used when no API key is configured but MongoCore is running as an MCP server.
pub struct McpSamplingProvider {
    sender: tokio::sync::mpsc::Sender<SamplingRequest>,
}

/// A sampling request sent through the channel.
pub struct SamplingRequest {
    pub prompt: String,
    pub system: Option<String>,
    pub response_tx: tokio::sync::oneshot::Sender<Result<String, LlmError>>,
}

impl McpSamplingProvider {
    /// Create a new sampling provider with the given request channel.
    pub fn new(sender: tokio::sync::mpsc::Sender<SamplingRequest>) -> Self {
        Self { sender }
    }
}

#[async_trait]
impl LlmProvider for McpSamplingProvider {
    async fn translate(
        &self,
        intent: &str,
        database: &str,
        collection: &str,
        context: &TranslationContext,
    ) -> Result<String, LlmError> {
        let prompt = build_translation_prompt(intent, database, collection, context);

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        let request = SamplingRequest {
            prompt,
            system: Some("You are a MongoDB query translator. Respond with valid JSON only.".to_string()),
            response_tx,
        };

        self.sender.send(request).await
            .map_err(|_| LlmError::ApiError("Sampling channel closed".to_string()))?;

        response_rx.await
            .map_err(|_| LlmError::ApiError("Sampling response channel dropped".to_string()))?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sampling_provider_channel_closed() {
        let (sender, _receiver) = tokio::sync::mpsc::channel::<SamplingRequest>(1);
        drop(_receiver); // close the channel
        let provider = McpSamplingProvider::new(sender);
        let context = TranslationContext::default();
        let result = provider.translate("find users", "mydb", "users", &context).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            LlmError::ApiError(msg) => assert!(msg.contains("channel")),
            other => panic!("Expected ApiError, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_sampling_provider_receives_response() {
        let (sender, mut receiver) = tokio::sync::mpsc::channel::<SamplingRequest>(1);
        let provider = McpSamplingProvider::new(sender);

        // Spawn a task to handle the request
        tokio::spawn(async move {
            if let Some(req) = receiver.recv().await {
                assert!(req.prompt.contains("find users"));
                let _ = req.response_tx.send(Ok(r#"{"method":"filter","filter":{"status":"active"}}"#.to_string()));
            }
        });

        let context = TranslationContext::default();
        let result = provider.translate("find users", "mydb", "users", &context).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("filter"));
    }
}
```

- [ ] **Step 2: Export from providers mod**

Add to `src/compiled/providers/mod.rs`:

```rust
pub mod sampling;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib compiled::providers::sampling::tests`
Expected: All pass

- [ ] **Step 4: Verify zero warnings**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output

- [ ] **Step 5: Commit**

```bash
git add src/compiled/providers/sampling.rs src/compiled/providers/mod.rs
git commit -m "feat(compiled): add McpSamplingProvider for zero-config LLM via MCP host"
```

---

### Task 7: Implement `ask` and `explain_query` tools

**Files:**
- Modify: `src/mcp/tools.rs`
- Modify: `src/mcp/handler.rs`

- [ ] **Step 1: Add tool definitions**

In `tool_definitions()` in `src/mcp/tools.rs`:

```rust
    McpToolDefinition {
        name: "ask".to_string(),
        description: "Ask a natural language question about your data. Translates to MQL, executes, and returns the answer with the generated query.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "question": { "type": "string", "description": "Natural language question about your data" },
                "database": { "type": "string", "description": "Database to query" },
                "collection": { "type": "string", "description": "Collection to query (optional — auto-detect if omitted)" }
            },
            "required": ["question", "database"]
        }),
    },
    McpToolDefinition {
        name: "explain_query".to_string(),
        description: "Translate a natural language question to MQL and show the execution plan without running it. Safe for expensive queries.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "question": { "type": "string", "description": "Natural language question" },
                "database": { "type": "string", "description": "Database name" },
                "collection": { "type": "string", "description": "Collection name (optional)" }
            },
            "required": ["question", "database"]
        }),
    },
```

- [ ] **Step 2: Add `ask` execution handler**

In the `execute_tool()` function, add:

```rust
        "ask" => {
            let question = match args.get("question").and_then(|v| v.as_str()) {
                Some(q) => q,
                None => return error_result("Missing required field: question"),
            };
            let database = match args.get("database").and_then(|v| v.as_str()) {
                Some(d) => d,
                None => return error_result("Missing required field: database"),
            };
            let collection = args.get("collection").and_then(|v| v.as_str()).unwrap_or("").to_string();

            // If no collection specified, we need one for the translator
            if collection.is_empty() {
                return error_result("Collection is required for 'ask' (auto-detection not yet implemented)");
            }

            // Use compiled query translator if available
            // For now, return a structured error suggesting direct query tools
            // This will be wired to the actual translator once sampling is connected in main.rs
            let result = json!({
                "isError": true,
                "error_type": "llm_unavailable",
                "message": "Natural language queries require an LLM provider. Configure ANTHROPIC_API_KEY or use MongoCore within Claude (MCP sampling).",
                "suggestion": "Use 'find' or 'aggregate' tools directly with MQL filters.",
                "recoverable": true
            });
            error_result(&serde_json::to_string_pretty(&result).unwrap_or_default())
        }
```

- [ ] **Step 3: Add `explain_query` execution handler**

```rust
        "explain_query" => {
            let question = match args.get("question").and_then(|v| v.as_str()) {
                Some(q) => q,
                None => return error_result("Missing required field: question"),
            };
            let database = match args.get("database").and_then(|v| v.as_str()) {
                Some(d) => d,
                None => return error_result("Missing required field: database"),
            };
            let collection = args.get("collection").and_then(|v| v.as_str()).unwrap_or("");

            if collection.is_empty() {
                return error_result("Collection is required for 'explain_query' (auto-detection not yet implemented)");
            }

            // Same graceful degradation as 'ask'
            let result = json!({
                "isError": true,
                "error_type": "llm_unavailable",
                "message": "Query explanation requires an LLM provider. Configure ANTHROPIC_API_KEY or use MongoCore within Claude.",
                "suggestion": "Use 'find' or 'aggregate' tools directly.",
                "recoverable": true
            });
            error_result(&serde_json::to_string_pretty(&result).unwrap_or_default())
        }
```

Note: The actual NL→MQL translation wiring (connecting the `CompiledQueryTranslator` to these tools) will be completed in Task 8 when we integrate sampling into the handler.

- [ ] **Step 4: Update tool count**

Update tool count assertion in `src/mcp/handler.rs` tests from 22 to 24. Update `tests/integration/mcp_test.rs` from 22 to 24.

- [ ] **Step 5: Verify zero warnings and tests pass**

Run: `cargo build 2>&1 | grep "warning:"` — no output
Run: `cargo test --lib` — all pass

- [ ] **Step 6: Commit**

```bash
git add src/mcp/tools.rs src/mcp/handler.rs tests/integration/mcp_test.rs
git commit -m "feat(mcp): add ask and explain_query tools with graceful degradation"
```

---

### Task 8: Wire sampling into stdio handler and connect translator to `ask`

**Files:**
- Modify: `src/main.rs`
- Modify: `src/mcp/handler.rs`
- Modify: `src/mcp/stdio.rs`
- Modify: `src/mcp/tools.rs`

This is the integration task that connects the pieces: in stdio mode, the handler can send sampling requests back through stdout and receive responses on stdin (interleaved with normal tool calls).

- [ ] **Step 1: Add `CompiledQueryTranslator` to `McpHandler`**

In `src/mcp/handler.rs`, add a field to `McpHandler`:

```rust
use crate::compiled::translator::CompiledQueryTranslator;

pub struct McpHandler {
    operations: Operations,
    pool: ConnectionPool,
    safety: SafetyConfig,
    analytics: Option<Arc<AnalyticsCollector>>,
    ingestion: Option<Arc<IngestionEngine>>,
    watcher: Option<Arc<DirectoryWatcher>>,
    mcp_metadata_appended: AtomicBool,
    translator: Option<Arc<CompiledQueryTranslator>>,
}
```

Update the `new()` constructor to accept `translator: Option<Arc<CompiledQueryTranslator>>` and store it.

- [ ] **Step 2: Update `McpHandler::new()` call sites**

In `src/mcp/server.rs`, pass `None` for translator (HTTP mode doesn't support sampling yet):

```rust
let handler = McpHandler::new(operations, pool, safety, analytics, ingestion, watcher, None);
```

In `src/main.rs` stdio branch, create the translator with the sampling provider:

```rust
use mongocore::compiled::translator::CompiledQueryTranslator;
use mongocore::compiled::providers::sampling::{McpSamplingProvider, SamplingRequest};

let (sampling_tx, sampling_rx) = tokio::sync::mpsc::channel::<SamplingRequest>(32);

// Create translator with sampling provider if no direct LLM key
let translator = if config.llm_api_key.is_some() || config.llm_gateway.is_some() {
    // Use configured LLM provider (existing behavior)
    // ... create provider from config ...
    None // TODO: wire existing provider creation here
} else {
    // Use MCP sampling
    let provider = McpSamplingProvider::new(sampling_tx.clone());
    Some(Arc::new(CompiledQueryTranslator::new(
        Some(pool.clone()),
        Some(Box::new(provider)),
        None,
    )))
};

let handler = McpHandler::new(operations, pool, safety, analytics, ingestion_engine, directory_watcher, translator);
```

- [ ] **Step 3: Update `execute_tool` to accept translator**

Pass the translator reference through `execute_tool()`. Update the `ask` handler to use the translator when available:

```rust
        "ask" => {
            let question = match args.get("question").and_then(|v| v.as_str()) {
                Some(q) => q,
                None => return error_result("Missing required field: question"),
            };
            let database = match args.get("database").and_then(|v| v.as_str()) {
                Some(d) => d,
                None => return error_result("Missing required field: database"),
            };
            let collection = match args.get("collection").and_then(|v| v.as_str()) {
                Some(c) => c,
                None => return error_result("Collection is required for 'ask'"),
            };

            let translator = match translator {
                Some(t) => t,
                None => return error_result(&serde_json::to_string(&json!({
                    "isError": true,
                    "error_type": "llm_unavailable",
                    "message": "NL queries require an LLM. Configure ANTHROPIC_API_KEY or use within Claude.",
                    "suggestion": "Use 'find' or 'aggregate' tools directly.",
                    "recoverable": true
                })).unwrap_or_default()),
            };

            let context = crate::compiled::providers::TranslationContext::default();
            let start = std::time::Instant::now();

            match translator.translate(question, database, collection, &context).await {
                Ok(compiled) => {
                    // Execute the compiled query
                    let exec_result = match &compiled.mql {
                        crate::compiled::CompiledMql::Find { filter, options } => {
                            operations.find(database, collection, Some(filter.clone()), None, None, None, None).await
                        }
                        crate::compiled::CompiledMql::Aggregate { pipeline } => {
                            operations.aggregate(database, collection, pipeline.clone()).await
                        }
                        _ => Ok(vec![]),
                    };

                    let elapsed = start.elapsed().as_millis();
                    match exec_result {
                        Ok(docs) => {
                            let result = json!({
                                "documents": docs.iter().take(20).map(|d| {
                                    serde_json::to_value(d).unwrap_or(json!(null))
                                }).collect::<Vec<_>>(),
                                "count": docs.len(),
                                "query": {
                                    "method": compiled.mql.method(),
                                    "intent": compiled.intent
                                },
                                "execution_time_ms": elapsed,
                                "from_cache": compiled.created_at > 0
                            });
                            success_result(&serde_json::to_string_pretty(&result).unwrap_or_default())
                        }
                        Err(e) => error_result(&format!("Query execution failed: {}", e)),
                    }
                }
                Err(e) => error_result(&format!("Translation failed: {}", e)),
            }
        }
```

- [ ] **Step 4: Handle sampling requests in stdio loop**

Update `src/mcp/stdio.rs` to process sampling requests from the handler (sent via the mpsc channel) interleaved with normal stdin requests. The stdio loop needs to both read from stdin AND forward sampling requests to stdout:

```rust
pub async fn run_stdio_transport(
    handler: Arc<McpHandler>,
    mut sampling_rx: tokio::sync::mpsc::Receiver<SamplingRequest>,
) {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    // Process stdin requests and sampling responses
    loop {
        tokio::select! {
            line = lines.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        let line = line.trim().to_string();
                        if line.is_empty() { continue; }
                        // ... handle request as before ...
                    }
                    Ok(None) => break, // stdin closed
                    Err(_) => break,
                }
            }
            Some(sampling_req) = sampling_rx.recv() => {
                // Send sampling request to stdout as JSON-RPC
                let jsonrpc_req = json!({
                    "jsonrpc": "2.0",
                    "method": "sampling/createMessage",
                    "params": {
                        "messages": [{
                            "role": "user",
                            "content": { "type": "text", "text": sampling_req.prompt }
                        }],
                        "maxTokens": 2048
                    },
                    "id": "sampling-1"
                });
                let req_json = serde_json::to_string(&jsonrpc_req).unwrap_or_default();
                let _ = stdout.write_all(req_json.as_bytes()).await;
                let _ = stdout.write_all(b"\n").await;
                let _ = stdout.flush().await;
                // The response will come back on stdin as a normal JSON-RPC response
                // We need to match it by id and route to the sampling_req.response_tx
                // For now, store pending sampling requests keyed by id
                // TODO: implement response routing
            }
        }
    }
}
```

Note: Full bidirectional sampling is complex. For the initial implementation, the simpler approach is to make `ask` return a structured response that tells Claude what to do (the "Claude-in-the-loop" fallback from the spec). The full sampling integration can be refined in a follow-up.

- [ ] **Step 5: Verify compilation**

Run: `cargo build 2>&1 | grep "warning:"`
Expected: No output (may need to suppress unused warnings with `_` prefixes during development)

- [ ] **Step 6: Run unit tests**

Run: `cargo test --lib`
Expected: All pass

- [ ] **Step 7: Commit**

```bash
git add src/main.rs src/mcp/handler.rs src/mcp/stdio.rs src/mcp/tools.rs src/mcp/server.rs
git commit -m "feat(mcp): wire compiled query translator into ask tool with sampling support"
```

---

### Task 9: Update `initialize` response for stdio mode

**Files:**
- Modify: `src/mcp/handler.rs`

- [ ] **Step 1: Add `stdio_mode` flag to handler**

Add `is_stdio: bool` to `McpHandler` fields and constructor. In `handle_initialize`, conditionally advertise sampling and prompts capabilities:

```rust
    fn handle_initialize(&self, id: Option<Value>) -> JsonRpcResponse {
        let mut capabilities = json!({
            "tools": { "listChanged": false },
            "resources": { "subscribe": false, "listChanged": false }
        });

        if self.is_stdio {
            // Advertise prompts capability (for skills)
            capabilities["prompts"] = json!({ "listChanged": false });
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
```

- [ ] **Step 2: Update constructor call sites**

Pass `is_stdio: true` from the stdio branch in `main.rs`, and `is_stdio: false` from `server.rs`.

- [ ] **Step 3: Update the initialize test**

In `src/mcp/handler.rs` tests, update `test_initialize_response_shape` to check for the new version format.

- [ ] **Step 4: Verify and commit**

Run: `cargo build 2>&1 | grep "warning:"` — no output
Run: `cargo test --lib` — all pass

```bash
git add src/mcp/handler.rs src/mcp/server.rs src/main.rs
git commit -m "feat(mcp): advertise prompts capability in stdio mode initialize response"
```

---

### Task 10: Integration test — stdio transport

**Files:**
- Create: `tests/integration/mcp_stdio_test.rs`

- [ ] **Step 1: Write a basic stdio integration test**

Create `tests/integration/mcp_stdio_test.rs`:

```rust
//! Integration tests for MCP stdio transport.
//! These tests spawn MongoCore as a child process with --stdio flag
//! and verify JSON-RPC communication over stdin/stdout.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use serde_json::{json, Value};

#[test]
fn test_stdio_initialize() {
    let mut child = Command::new("./target/debug/mongocore")
        .args(["--stdio", "--connection-uri", "mongodb://localhost:27017"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start mongocore");

    let stdin = child.stdin.as_mut().unwrap();
    let stdout = BufReader::new(child.stdout.as_mut().unwrap());

    // Send initialize request
    let init_req = json!({
        "jsonrpc": "2.0",
        "method": "initialize",
        "id": 1
    });
    writeln!(stdin, "{}", serde_json::to_string(&init_req).unwrap()).unwrap();
    stdin.flush().unwrap();

    // Read response
    let mut response_line = String::new();
    let mut reader = stdout;
    reader.read_line(&mut response_line).unwrap();

    let response: Value = serde_json::from_str(&response_line).unwrap();
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert!(response["result"]["protocolVersion"].is_string());
    assert_eq!(response["result"]["serverInfo"]["name"], "mongocore");

    // Kill child
    child.kill().unwrap();
}

#[test]
fn test_stdio_tools_list() {
    let mut child = Command::new("./target/debug/mongocore")
        .args(["--stdio", "--connection-uri", "mongodb://localhost:27017"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to start mongocore");

    let stdin = child.stdin.as_mut().unwrap();
    let stdout = BufReader::new(child.stdout.take().unwrap());
    let mut reader = stdout;

    // Initialize first
    let init_req = json!({"jsonrpc":"2.0","method":"initialize","id":1});
    writeln!(stdin, "{}", serde_json::to_string(&init_req).unwrap()).unwrap();
    stdin.flush().unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    line.clear();

    // Request tools list
    let list_req = json!({"jsonrpc":"2.0","method":"tools/list","id":2});
    writeln!(stdin, "{}", serde_json::to_string(&list_req).unwrap()).unwrap();
    stdin.flush().unwrap();
    reader.read_line(&mut line).unwrap();

    let response: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(response["id"], 2);
    let tools = response["result"]["tools"].as_array().unwrap();
    assert!(tools.len() >= 22); // 21 existing + at least collection_schema

    // Verify new tools are present
    let tool_names: Vec<&str> = tools.iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(tool_names.contains(&"collection_schema"));
    assert!(tool_names.contains(&"ask"));
    assert!(tool_names.contains(&"explain_query"));

    child.kill().unwrap();
}
```

- [ ] **Step 2: Ensure the binary is built**

Run: `cargo build`

- [ ] **Step 3: Run the integration test (requires Docker MongoDB)**

Run: `cargo test --test integration mcp_stdio -- --nocapture`
Expected: PASS (requires MongoDB running on localhost:27017)

- [ ] **Step 4: Commit**

```bash
git add tests/integration/mcp_stdio_test.rs
git commit -m "test(mcp): add stdio transport integration tests"
```

---

## Verification Checklist

After completing all tasks:

- [ ] `cargo build 2>&1 | grep "warning:"` produces no output
- [ ] `cargo test --lib` passes all unit tests
- [ ] `cargo test --test integration` passes (with Docker MongoDB running)
- [ ] `./target/debug/mongocore --stdio --connection-uri mongodb://localhost:27017` starts and responds to JSON-RPC on stdin
- [ ] `collection_schema` tool returns field information when called via stdio
- [ ] `ask` tool returns graceful error when no LLM configured (not in Claude)
- [ ] `initialize` response in stdio mode includes `prompts` capability
