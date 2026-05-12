# MongoCore: Custom LLM Gateway Support

## Overview

Add support for calling LLM providers via custom API gateway URLs with configurable authentication headers. This enables use with corporate AI gateways, proxies, and self-hosted LLM endpoints that use non-standard URLs or auth schemes.

## Motivation

Organizations often route LLM traffic through internal API gateways (for security, auditing, cost management). These gateways:
- Use custom base URLs (e.g., `https://my-ai-gateway.example.com/anthropic/v1/messages`)
- Use different auth headers (e.g., `api-key` instead of `x-api-key` or `Authorization: Bearer`)
- May use different API keys than the direct provider keys

MongoCore currently hardcodes provider URLs and auth header formats. This change makes both configurable.

## Current State

**Claude provider:** Hardcoded to `https://api.anthropic.com/v1/messages` with `x-api-key` header.

**OpenAI provider:** Hardcoded to `https://api.openai.com/v1/chat/completions` with `Authorization: Bearer` header.

## Design

### Config Changes

Add optional fields to the TOML config:

```toml
# Custom LLM gateway (optional — overrides default provider URLs)
# LLM_BASE_URL = "https://my-ai-gateway.example.com/anthropic/v1/messages"
# LLM_API_KEY = "your-gateway-api-key"
# LLM_AUTH_HEADER = "api-key"
# LLM_MODEL = "claude-sonnet-4-6"
# LLM_PROVIDER_TYPE = "anthropic"  # or "openai" — determines request/response format
```

### Resolution Logic

1. If `LLM_BASE_URL` is set → use custom gateway mode
2. If not set → fall back to current behavior (detect provider from `ANTHROPIC_API_KEY` or `OPENAI_API_KEY`)

### Custom Gateway Mode

When `LLM_BASE_URL` is configured:
- **URL:** Uses the configured base URL as-is (no path appending)
- **Auth:** Sends the key via the configured header name (default: `api-key`)
- **Format:** `LLM_PROVIDER_TYPE` determines the request/response shape:
  - `"anthropic"` → Anthropic Messages API format (messages array, `anthropic-version` header, response in `content[0].text`)
  - `"openai"` → OpenAI Chat Completions format (messages array with system message, response in `choices[0].message.content`)
- **Model:** Uses `LLM_MODEL` (required when using custom gateway)
- **Key:** Uses `LLM_API_KEY` for auth (distinct from direct `ANTHROPIC_API_KEY`)

### Precedence

```
Custom gateway (LLM_BASE_URL set) > Direct Anthropic (ANTHROPIC_API_KEY) > Direct OpenAI (OPENAI_API_KEY)
```

If custom gateway is configured, direct provider keys are ignored for NL→MQL.

### Implementation Approach

Add a new `GatewayProvider` struct in `src/compiled/providers/gateway.rs` that:
- Takes base_url, api_key, auth_header_name, model, and provider_type
- Uses the same prompt-building logic as existing providers
- Sends requests in either Anthropic or OpenAI format based on `provider_type`
- Parses responses accordingly

This is cleaner than modifying the existing providers — it's a third provider type alongside `ClaudeProvider` and `OpenAiProvider`.

### Config Structs

**FileConfig additions:**
```rust
#[serde(rename = "LLM_BASE_URL")]
pub llm_base_url: Option<String>,
#[serde(rename = "LLM_API_KEY")]
pub llm_api_key_direct: Option<String>,
#[serde(rename = "LLM_AUTH_HEADER")]
pub llm_auth_header: Option<String>,
#[serde(rename = "LLM_MODEL")]
pub llm_model: Option<String>,
#[serde(rename = "LLM_PROVIDER_TYPE")]
pub llm_provider_type: Option<String>,
```

**CliArgs additions:**
```rust
#[arg(long, env = "LLM_BASE_URL")]
pub llm_base_url: Option<String>,
#[arg(long, env = "LLM_API_KEY")]
pub llm_api_key_direct: Option<String>,
#[arg(long, env = "LLM_AUTH_HEADER")]
pub llm_auth_header: Option<String>,
#[arg(long, env = "LLM_MODEL")]
pub llm_model: Option<String>,
#[arg(long, env = "LLM_PROVIDER_TYPE")]
pub llm_provider_type: Option<String>,
```

**Resolved Config:**
```rust
pub llm_gateway: Option<LlmGatewayConfig>,
```

```rust
#[derive(Debug, Clone)]
pub struct LlmGatewayConfig {
    pub base_url: String,
    pub api_key: String,
    pub auth_header: String,  // default: "api-key"
    pub model: String,
    pub provider_type: String, // "anthropic" or "openai"
}
```

### Updated Resolution in Config::load()

```rust
// Check for custom gateway first
let llm_gateway = if let Some(base_url) = cli.llm_base_url.or(file_config.llm_base_url) {
    let api_key = cli.llm_api_key_direct
        .or(file_config.llm_api_key_direct)
        .or_else(|| std::env::var("LLM_API_KEY").ok())
        .unwrap_or_default();
    let auth_header = cli.llm_auth_header
        .or(file_config.llm_auth_header)
        .unwrap_or_else(|| "api-key".to_string());
    let model = cli.llm_model
        .or(file_config.llm_model)
        .or_else(|| std::env::var("LLM_MODEL").ok())
        .unwrap_or_else(|| "claude-sonnet-4-6".to_string());
    let provider_type = cli.llm_provider_type
        .or(file_config.llm_provider_type)
        .unwrap_or_else(|| "anthropic".to_string());
    Some(LlmGatewayConfig { base_url, api_key, auth_header, model, provider_type })
} else {
    None
};
```

### Provider Selection (where translator is created)

```rust
let provider: Option<Box<dyn LlmProvider>> = if let Some(ref gw) = config.llm_gateway {
    Some(Box::new(GatewayProvider::new(gw.clone())))
} else if let Some(ref key) = config.llm_api_key {
    match config.llm_provider_name.as_deref() {
        Some("anthropic") => Some(Box::new(ClaudeProvider::new(key.clone()))),
        Some("openai") => Some(Box::new(OpenAiProvider::new(key.clone()))),
        _ => None,
    }
} else {
    None
};
```

### config.test.toml.example Update

```toml
# Custom LLM gateway (optional — overrides direct API keys)
# LLM_BASE_URL = "https://my-ai-gateway.example.com/anthropic/v1/messages"
# LLM_API_KEY = "your-gateway-api-key"
# LLM_AUTH_HEADER = "api-key"
# LLM_MODEL = "claude-sonnet-4-6"
# LLM_PROVIDER_TYPE = "anthropic"
```

## Implementation Scope

| File | Change |
|------|--------|
| `src/compiled/providers/gateway.rs` | Create new GatewayProvider |
| `src/compiled/providers/mod.rs` | Export gateway module |
| `src/config.rs` | Add gateway config fields and resolution |
| `config.test.toml.example` | Add gateway config section |
| `docs/compiled-queries.md` | Document gateway configuration |
| `tests/integration/compiled_llm_test.rs` | Update to use gateway config if set |

## Won't Build

- No changes to the LlmProvider trait
- No changes to the compiled query translator or cache
- No changes to existing ClaudeProvider/OpenAiProvider (they stay for direct API use)

## Success Criteria

- [ ] `GatewayProvider` sends requests to custom URL with configurable auth header
- [ ] `LLM_BASE_URL` in TOML or env var activates gateway mode
- [ ] Gateway takes precedence over direct `ANTHROPIC_API_KEY`
- [ ] Supports both Anthropic and OpenAI request/response formats
- [ ] `config.test.toml.example` documents the gateway fields
- [ ] All existing tests pass (`cargo test --lib`)
- [ ] Works with the example curl commands from the motivation (Anthropic and OpenAI format via gateway)
