# Custom LLM Gateway — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `just test-all` must pass (this runs all Rust tests + all client tests).
> If modifying client libraries: verify imports work and run `just test-clients`.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

**Goal:** Add a `GatewayProvider` that sends LLM requests to a configurable URL with configurable auth headers, supporting both Anthropic and OpenAI request formats.

**Architecture:** New provider file (`gateway.rs`) alongside existing `claude.rs` and `openai.rs`. Config adds optional `LLM_BASE_URL`/`LLM_API_KEY`/`LLM_AUTH_HEADER`/`LLM_MODEL`/`LLM_PROVIDER_TYPE` fields. When `LLM_BASE_URL` is set, gateway takes precedence over direct API keys.

**Tech Stack:** Rust, reqwest (already a dependency), existing `LlmProvider` trait.

**Branch:** `feat/opencode-support` — do NOT push to origin.

---

## File Structure

| Action | Path | Responsibility |
|--------|------|----------------|
| Create | `src/compiled/providers/gateway.rs` | GatewayProvider implementation |
| Modify | `src/compiled/providers/mod.rs` | Export gateway module |
| Modify | `src/config.rs` | Add gateway config fields, LlmGatewayConfig struct, resolution logic |
| Modify | `src/connection/pool.rs` | Update test Config struct literals |
| Modify | `tests/harness/mongodb.rs` | Update test Config struct literal |
| Modify | `config.test.toml.example` | Add gateway config section |
| Modify | `docs/compiled-queries.md` | Document gateway configuration |

---

## Task 1: Create GatewayProvider

**Files:**
- Create: `src/compiled/providers/gateway.rs`
- Modify: `src/compiled/providers/mod.rs`

- [ ] **Step 1: Create gateway.rs with the provider implementation**

Create `src/compiled/providers/gateway.rs`:

```rust
use async_trait::async_trait;

use super::{LlmError, LlmProvider, TranslationContext};

#[derive(Debug, Clone)]
pub struct GatewayConfig {
    pub base_url: String,
    pub api_key: String,
    pub auth_header: String,
    pub model: String,
    pub provider_type: String,
}

pub struct GatewayProvider {
    config: GatewayConfig,
}

impl GatewayProvider {
    pub fn new(config: GatewayConfig) -> Self {
        Self { config }
    }

    fn build_prompt(
        &self,
        intent: &str,
        database: &str,
        collection: &str,
        context: &TranslationContext,
    ) -> String {
        let mut prompt = format!(
            "Translate this natural language query into a MongoDB query.\n\n\
             Database: {}\nCollection: {}\nIntent: \"{}\"\n\n",
            database, collection, intent
        );
        if let Some(ref schema) = context.schema_hint {
            prompt.push_str(&format!("Schema: {}\n\n", schema));
        }
        if !context.sample_documents.is_empty() {
            prompt.push_str("Sample documents:\n");
            for doc in &context.sample_documents {
                prompt.push_str(&format!("  {}\n", doc));
            }
            prompt.push('\n');
        }
        if !context.available_indexes.is_empty() {
            prompt.push_str("Available indexes:\n");
            for idx in &context.available_indexes {
                prompt.push_str(&format!("  {}\n", idx));
            }
            prompt.push('\n');
        }
        prompt.push_str(
            "Respond with ONLY valid JSON. Either:\n\
             - A filter object for simple queries: {\"type\": \"find\", \"filter\": {...}}\n\
             - A pipeline array for complex queries: {\"type\": \"aggregate\", \"pipeline\": [...]}\n\
             No explanation, no markdown.",
        );
        prompt
    }

    fn build_anthropic_body(&self, prompt: &str) -> serde_json::Value {
        serde_json::json!({
            "model": self.config.model,
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": prompt}]
        })
    }

    fn build_openai_body(&self, prompt: &str) -> serde_json::Value {
        serde_json::json!({
            "model": self.config.model,
            "max_tokens": 1024,
            "messages": [
                {"role": "system", "content": "You are a MongoDB query translator. Output only valid JSON."},
                {"role": "user", "content": prompt}
            ]
        })
    }

    fn extract_anthropic_text(body: &serde_json::Value) -> Result<String, LlmError> {
        body["content"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| LlmError::InvalidResponse("No text in Anthropic response".to_string()))
    }

    fn extract_openai_text(body: &serde_json::Value) -> Result<String, LlmError> {
        body["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| LlmError::InvalidResponse("No content in OpenAI response".to_string()))
    }
}

#[async_trait]
impl LlmProvider for GatewayProvider {
    async fn translate(
        &self,
        intent: &str,
        database: &str,
        collection: &str,
        context: &TranslationContext,
    ) -> Result<String, LlmError> {
        let prompt = self.build_prompt(intent, database, collection, context);

        let request_body = match self.config.provider_type.as_str() {
            "openai" => self.build_openai_body(&prompt),
            _ => self.build_anthropic_body(&prompt),
        };

        let client = reqwest::Client::new();
        let mut request = client
            .post(&self.config.base_url)
            .header("content-type", "application/json")
            .header(&self.config.auth_header, &self.config.api_key)
            .json(&request_body);

        // Add anthropic-version header for Anthropic format
        if self.config.provider_type != "openai" {
            request = request.header("anthropic-version", "2023-06-01");
        }

        let response = request
            .send()
            .await
            .map_err(|e| LlmError::ApiError(e.to_string()))?;

        if response.status() == 429 {
            return Err(LlmError::RateLimited(60));
        }
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(LlmError::ApiError(format!("HTTP {}: {}", status, body)));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| LlmError::InvalidResponse(e.to_string()))?;

        match self.config.provider_type.as_str() {
            "openai" => Self::extract_openai_text(&body),
            _ => Self::extract_anthropic_text(&body),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gateway_config_defaults() {
        let config = GatewayConfig {
            base_url: "https://gateway.example.com/v1/messages".to_string(),
            api_key: "test-key".to_string(),
            auth_header: "api-key".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            provider_type: "anthropic".to_string(),
        };
        let provider = GatewayProvider::new(config.clone());
        assert_eq!(provider.config.base_url, "https://gateway.example.com/v1/messages");
        assert_eq!(provider.config.auth_header, "api-key");
    }

    #[test]
    fn test_build_anthropic_body() {
        let config = GatewayConfig {
            base_url: "https://gw.example.com/anthropic/v1/messages".to_string(),
            api_key: "key".to_string(),
            auth_header: "api-key".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            provider_type: "anthropic".to_string(),
        };
        let provider = GatewayProvider::new(config);
        let body = provider.build_anthropic_body("test prompt");
        assert_eq!(body["model"], "claude-sonnet-4-6");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "test prompt");
    }

    #[test]
    fn test_build_openai_body() {
        let config = GatewayConfig {
            base_url: "https://gw.example.com/openai/v1/chat/completions".to_string(),
            api_key: "key".to_string(),
            auth_header: "api-key".to_string(),
            model: "gpt-5.1".to_string(),
            provider_type: "openai".to_string(),
        };
        let provider = GatewayProvider::new(config);
        let body = provider.build_openai_body("test prompt");
        assert_eq!(body["model"], "gpt-5.1");
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["messages"][1]["content"], "test prompt");
    }

    #[test]
    fn test_extract_anthropic_text() {
        let body = serde_json::json!({
            "content": [{"type": "text", "text": "{\"type\": \"find\", \"filter\": {}}"}]
        });
        let result = GatewayProvider::extract_anthropic_text(&body).unwrap();
        assert_eq!(result, "{\"type\": \"find\", \"filter\": {}}");
    }

    #[test]
    fn test_extract_openai_text() {
        let body = serde_json::json!({
            "choices": [{"message": {"content": "{\"type\": \"find\", \"filter\": {}}"}}]
        });
        let result = GatewayProvider::extract_openai_text(&body).unwrap();
        assert_eq!(result, "{\"type\": \"find\", \"filter\": {}}");
    }

    #[test]
    fn test_extract_anthropic_text_missing() {
        let body = serde_json::json!({"content": []});
        let result = GatewayProvider::extract_anthropic_text(&body);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_openai_text_missing() {
        let body = serde_json::json!({"choices": []});
        let result = GatewayProvider::extract_openai_text(&body);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Export gateway module**

Add to `src/compiled/providers/mod.rs` after the existing exports:

```rust
pub mod gateway;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib gateway`
Expected: 7 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/compiled/providers/gateway.rs src/compiled/providers/mod.rs
git commit -m "feat(compiled): add GatewayProvider for custom LLM endpoints"
```

---

## Task 2: Add Gateway Config Fields

**Files:**
- Modify: `src/config.rs`
- Modify: `src/connection/pool.rs`
- Modify: `tests/harness/mongodb.rs`

- [ ] **Step 1: Add LlmGatewayConfig struct to config.rs**

Add after the `ResolvedIngestionConfig` impl block (around line 127), before `FileConfig`:

```rust
/// Configuration for a custom LLM gateway endpoint.
#[derive(Debug, Clone)]
pub struct LlmGatewayConfig {
    pub base_url: String,
    pub api_key: String,
    pub auth_header: String,
    pub model: String,
    pub provider_type: String,
}
```

- [ ] **Step 2: Add fields to CliArgs**

Add after `voyage_api_key` in `CliArgs`:

```rust
    /// Custom LLM gateway base URL (overrides direct API keys)
    #[arg(long, env = "LLM_BASE_URL")]
    pub llm_base_url: Option<String>,

    /// API key for custom LLM gateway
    #[arg(long, env = "LLM_API_KEY")]
    pub llm_gateway_key: Option<String>,

    /// Auth header name for custom LLM gateway
    #[arg(long, env = "LLM_AUTH_HEADER")]
    pub llm_auth_header: Option<String>,

    /// Model name for custom LLM gateway
    #[arg(long, env = "LLM_MODEL")]
    pub llm_model: Option<String>,

    /// Provider type for custom LLM gateway (anthropic or openai)
    #[arg(long, env = "LLM_PROVIDER_TYPE")]
    pub llm_provider_type: Option<String>,
```

- [ ] **Step 3: Add fields to FileConfig**

Add after `voyage_api_key` in `FileConfig`:

```rust
    #[serde(rename = "LLM_BASE_URL")]
    pub llm_base_url: Option<String>,
    #[serde(rename = "LLM_API_KEY")]
    pub llm_gateway_key: Option<String>,
    #[serde(rename = "LLM_AUTH_HEADER")]
    pub llm_auth_header: Option<String>,
    #[serde(rename = "LLM_MODEL")]
    pub llm_model: Option<String>,
    #[serde(rename = "LLM_PROVIDER_TYPE")]
    pub llm_provider_type: Option<String>,
```

- [ ] **Step 4: Add llm_gateway field to Config struct**

Add after `voyage_api_key` in `Config`:

```rust
    pub llm_gateway: Option<LlmGatewayConfig>,
```

- [ ] **Step 5: Add gateway resolution logic in Config::load()**

Add BEFORE the existing LLM key resolution (before `let anthropic_key = ...`):

```rust
        // Check for custom LLM gateway first (takes precedence over direct keys)
        let llm_gateway = if let Some(base_url) = cli.llm_base_url.clone()
            .or(file_config.llm_base_url)
            .or_else(|| std::env::var("LLM_BASE_URL").ok())
        {
            let api_key = cli.llm_gateway_key.clone()
                .or(file_config.llm_gateway_key)
                .or_else(|| std::env::var("LLM_API_KEY").ok())
                .unwrap_or_default();
            let auth_header = cli.llm_auth_header.clone()
                .or(file_config.llm_auth_header)
                .unwrap_or_else(|| "api-key".to_string());
            let model = cli.llm_model.clone()
                .or(file_config.llm_model)
                .or_else(|| std::env::var("LLM_MODEL").ok())
                .unwrap_or_else(|| "claude-sonnet-4-6".to_string());
            let provider_type = cli.llm_provider_type.clone()
                .or(file_config.llm_provider_type)
                .unwrap_or_else(|| "anthropic".to_string());
            Some(LlmGatewayConfig { base_url, api_key, auth_header, model, provider_type })
        } else {
            None
        };
```

- [ ] **Step 6: Add llm_gateway to the Config struct literal in Ok(...)**

Add `llm_gateway,` after `voyage_api_key,` in the `Ok(Config { ... })` block.

- [ ] **Step 7: Update default_cli() and ALL CliArgs test literals**

Add to `default_cli()` and every other `CliArgs { ... }` in tests:
```rust
            llm_base_url: None,
            llm_gateway_key: None,
            llm_auth_header: None,
            llm_model: None,
            llm_provider_type: None,
```

- [ ] **Step 8: Update Config struct literals in pool.rs and harness**

Add `llm_gateway: None,` to every `Config { ... }` in:
- `src/connection/pool.rs` (4 occurrences)
- `tests/harness/mongodb.rs` (1 occurrence)

- [ ] **Step 9: Add a test for gateway config parsing**

Add to the tests module in `src/config.rs`:

```rust
    #[test]
    fn test_gateway_config_from_toml() {
        let toml_content = r#"
connection_uri = "mongodb://localhost:27017"
LLM_BASE_URL = "https://gateway.example.com/anthropic/v1/messages"
LLM_API_KEY = "gw-key-123"
LLM_AUTH_HEADER = "x-custom-key"
LLM_MODEL = "claude-sonnet-4-6"
LLM_PROVIDER_TYPE = "anthropic"
"#;
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(toml_content.as_bytes()).unwrap();
        let mut cli = default_cli();
        cli.config = Some(tmp.path().to_path_buf());

        let config = Config::load(&cli).unwrap();
        let gw = config.llm_gateway.expect("gateway should be configured");
        assert_eq!(gw.base_url, "https://gateway.example.com/anthropic/v1/messages");
        assert_eq!(gw.api_key, "gw-key-123");
        assert_eq!(gw.auth_header, "x-custom-key");
        assert_eq!(gw.model, "claude-sonnet-4-6");
        assert_eq!(gw.provider_type, "anthropic");
    }

    #[test]
    fn test_gateway_takes_precedence_over_direct_keys() {
        let toml_content = r#"
connection_uri = "mongodb://localhost:27017"
ANTHROPIC_API_KEY = "direct-key"
LLM_BASE_URL = "https://gateway.example.com/v1/messages"
LLM_API_KEY = "gw-key"
"#;
        let mut tmp = NamedTempFile::new().unwrap();
        tmp.write_all(toml_content.as_bytes()).unwrap();
        let mut cli = default_cli();
        cli.config = Some(tmp.path().to_path_buf());

        let config = Config::load(&cli).unwrap();
        assert!(config.llm_gateway.is_some(), "Gateway should be configured");
        // Direct key is still resolved (for non-gateway uses) but gateway takes priority
        assert!(config.llm_api_key.is_some());
    }
```

- [ ] **Step 10: Run all tests**

```bash
cargo test --lib
```
Expected: All pass (including new gateway config tests)

- [ ] **Step 11: Commit**

```bash
git add src/config.rs src/connection/pool.rs tests/harness/mongodb.rs
git commit -m "feat(config): add LLM gateway configuration fields"
```

---

## Task 3: Update Config Example and Documentation

**Files:**
- Modify: `config.test.toml.example`
- Modify: `docs/compiled-queries.md`

- [ ] **Step 1: Add gateway section to config.test.toml.example**

Add after the Voyage AI section:

```toml

# Custom LLM gateway (optional — overrides direct API keys)
# Use this for corporate AI gateways or self-hosted endpoints
# LLM_BASE_URL = "https://my-ai-gateway.example.com/anthropic/v1/messages"
# LLM_API_KEY = "your-gateway-api-key"
# LLM_AUTH_HEADER = "api-key"
# LLM_MODEL = "claude-sonnet-4-6"
# LLM_PROVIDER_TYPE = "anthropic"
```

- [ ] **Step 2: Add gateway documentation to docs/compiled-queries.md**

Read the file and add a "Custom Gateway" section after the existing configuration section:

```markdown
## Custom LLM Gateway

For organizations using corporate AI gateways, proxies, or self-hosted endpoints:

\`\`\`toml
LLM_BASE_URL = "https://my-ai-gateway.example.com/anthropic/v1/messages"
LLM_API_KEY = "your-gateway-api-key"
LLM_AUTH_HEADER = "api-key"
LLM_MODEL = "claude-sonnet-4-6"
LLM_PROVIDER_TYPE = "anthropic"  # or "openai"
\`\`\`

Or via environment variables:

\`\`\`bash
export LLM_BASE_URL="https://my-ai-gateway.example.com/anthropic/v1/messages"
export LLM_API_KEY="your-gateway-api-key"
export LLM_AUTH_HEADER="api-key"
export LLM_MODEL="claude-sonnet-4-6"
export LLM_PROVIDER_TYPE="anthropic"
\`\`\`

### Configuration

| Field | Description | Default |
|-------|-------------|---------|
| `LLM_BASE_URL` | Full URL for the LLM endpoint | — (activates gateway mode) |
| `LLM_API_KEY` | API key sent in the auth header | — |
| `LLM_AUTH_HEADER` | HTTP header name for the API key | `api-key` |
| `LLM_MODEL` | Model identifier to send in requests | `claude-sonnet-4-6` |
| `LLM_PROVIDER_TYPE` | Request/response format: `anthropic` or `openai` | `anthropic` |

### Precedence

When `LLM_BASE_URL` is set, MongoCore uses the gateway for all NL→MQL translations. Direct `ANTHROPIC_API_KEY` or `OPENAI_API_KEY` are ignored for compiled queries (but still used for other features if configured).

### Examples

**Anthropic via gateway:**
\`\`\`toml
LLM_BASE_URL = "https://my-ai-gateway.example.com/anthropic/v1/messages"
LLM_API_KEY = "gw-key-123"
LLM_AUTH_HEADER = "api-key"
LLM_MODEL = "claude-sonnet-4-6"
LLM_PROVIDER_TYPE = "anthropic"
\`\`\`

**OpenAI via gateway:**
\`\`\`toml
LLM_BASE_URL = "https://my-ai-gateway.example.com/openai/v1/chat/completions"
LLM_API_KEY = "gw-key-456"
LLM_AUTH_HEADER = "api-key"
LLM_MODEL = "gpt-5.1"
LLM_PROVIDER_TYPE = "openai"
\`\`\`
```

- [ ] **Step 3: Commit**

```bash
git add config.test.toml.example docs/compiled-queries.md
git commit -m "docs: add custom LLM gateway configuration and documentation"
```

---

## Task 4: Verification

- [ ] **Step 1: Run full unit test suite**

```bash
cargo test --lib
```
Expected: All pass (208+ tests including new gateway tests)

- [ ] **Step 2: Verify integration tests compile**

```bash
cargo test --test integration --no-run
```
Expected: Compiles without errors

- [ ] **Step 3: Verify no old fields leaked**

```bash
grep -rn "LLM_BASE_URL\|LLM_API_KEY\|LLM_AUTH_HEADER\|LLM_MODEL\|LLM_PROVIDER_TYPE" src/ tests/ | grep -v "test\|config.rs"
```
Expected: No unexpected references outside config and tests

- [ ] **Step 4: Commit any fixes**

If anything failed, fix and commit.

---

## Implementation Order

```
Task 1: GatewayProvider (independent, can be tested in isolation)
Task 2: Config fields (depends on GatewayConfig struct from Task 1)
Task 3: Documentation (depends on Task 2)
Task 4: Verification (depends on all above)
```

---

## Definition of Done

- [ ] `GatewayProvider` implements `LlmProvider` trait with both Anthropic and OpenAI formats
- [ ] Config accepts `LLM_BASE_URL`, `LLM_API_KEY`, `LLM_AUTH_HEADER`, `LLM_MODEL`, `LLM_PROVIDER_TYPE`
- [ ] Gateway config takes precedence when `LLM_BASE_URL` is set
- [ ] 7+ unit tests for GatewayProvider (body building, response extraction, config)
- [ ] 2 config tests (TOML parsing, precedence over direct keys)
- [ ] `config.test.toml.example` documents gateway fields
- [ ] `docs/compiled-queries.md` has gateway documentation
- [ ] All existing tests pass (`cargo test --lib`)
- [ ] Integration tests compile (`cargo test --test integration --no-run`)
