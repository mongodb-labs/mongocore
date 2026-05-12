# MongoCore: Simplified LLM Configuration

## Overview

Replace the current two-field LLM config (`llm_provider` + `llm_api_key_env`) with direct API key fields that auto-detect the provider. Keys can be set in the TOML config file or as environment variables.

## Motivation

The current approach requires users to know two things: which provider name to use AND which env var holds the key. The new approach is simpler — just set your API key and MongoCore figures out the rest.

## Current Config (to be replaced)

```toml
llm_provider = "anthropic"
llm_api_key_env = "ANTHROPIC_API_KEY"
```

This means: "read the API key from the env var named ANTHROPIC_API_KEY and use the anthropic provider."

## New Config

```toml
# API Keys for LLM providers (pick one)
# ANTHROPIC_API_KEY = "sk-ant-your-key-here"
# OPENAI_API_KEY = "sk-your-key-here"
```

Provider is auto-detected from which key is present.

## Resolution Order

1. Check TOML config for `ANTHROPIC_API_KEY` or `OPENAI_API_KEY`
2. If not in TOML, check environment variables with the same names
3. First one found wins (check order: Anthropic, then OpenAI)

## Design

### Config Struct Changes

**Remove from `FileConfig`:**
- `llm_provider: Option<String>`
- `llm_api_key_env: Option<String>`

**Add to `FileConfig`:**
- `anthropic_api_key: Option<String>` (maps to TOML field `ANTHROPIC_API_KEY`)
- `openai_api_key: Option<String>` (maps to TOML field `OPENAI_API_KEY`)

**Remove from `CliArgs`:**
- `--llm-provider`
- `--llm-api-key-env`

**Add to `CliArgs`:**
- `--anthropic-api-key` / env `ANTHROPIC_API_KEY`
- `--openai-api-key` / env `OPENAI_API_KEY`

**Change in `Config` (resolved):**
- Remove: `llm_provider: Option<String>`, `llm_api_key_env: Option<String>`
- Add: `llm_api_key: Option<String>`, `llm_provider_name: Option<String>`

The `llm_provider_name` is auto-detected — not user-configured:
- If `anthropic_api_key` is set → `llm_provider_name = Some("anthropic")`
- If `openai_api_key` is set → `llm_provider_name = Some("openai")`
- If neither → both are `None`

### Resolution Logic in `Config::load()`

```rust
let anthropic_key = cli.anthropic_api_key
    .clone()
    .or(file_config.anthropic_api_key)
    .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok());

let openai_key = cli.openai_api_key
    .clone()
    .or(file_config.openai_api_key)
    .or_else(|| std::env::var("OPENAI_API_KEY").ok());

let (llm_api_key, llm_provider_name) = if let Some(key) = anthropic_key {
    (Some(key), Some("anthropic".to_string()))
} else if let Some(key) = openai_key {
    (Some(key), Some("openai".to_string()))
} else {
    (None, None)
};
```

### TOML Field Names

Use serde rename to map Rust snake_case to the TOML key format:

```rust
#[serde(rename = "ANTHROPIC_API_KEY")]
pub anthropic_api_key: Option<String>,

#[serde(rename = "OPENAI_API_KEY")]
pub openai_api_key: Option<String>,
```

### Impact on Existing Code

**`src/main.rs`:** Currently doesn't create an LLM provider at startup (the compiled query translator creates it on demand). No change needed here.

**`tests/integration/compiled_llm_test.rs`:** Already reads `ANTHROPIC_API_KEY` directly from env — no change needed.

**`docker-compose.test.yml`:** Already uses `${ANTHROPIC_API_KEY:+true}` — no change needed.

**Voyage AI:** `voyage_api_key_env` should get the same treatment → `VOYAGE_API_KEY` directly in TOML or env. Same pattern.

### config.test.toml.example Update

```toml
connection_uri = "mongodb://localhost:27017"
grpc_port = 50051
mcp_port = 3000
log_level = "debug"
compiled_cache_sync = false

# API Keys for LLM providers (pick one)
# ANTHROPIC_API_KEY = "your-api-key-here"
# OPENAI_API_KEY = "your-api-key-here"

# Voyage AI for embeddings
# VOYAGE_API_KEY = "your-api-key-here"

# OpenTelemetry tracing (requires --features otel)
# otel_enabled = true
# otel_endpoint = "http://localhost:4317"
# otel_service_name = "mongocore"
```

### .gitignore

`config.test.toml` is already gitignored. Any file with API keys stays out of source control.

## Implementation Scope

| File | Change |
|------|--------|
| `src/config.rs` | Replace llm_provider/llm_api_key_env with new key fields, add resolution logic |
| `src/defaults.rs` | Remove any LLM-related defaults (if any) |
| `config.test.toml.example` | Update to new format |
| `tests/harness/mongodb.rs` | Update Config struct literal |
| `src/connection/pool.rs` | Update Config struct literals in tests |
| `AGENTS.md` | Update "Adding a Config Field" if it references old fields |
| `docs/getting-started.md` | Update config documentation |

## Won't Build

- No Gemini provider (deferred)
- No changes to the compiled query translator or LLM provider trait
- No changes to how the search RPC uses compiled queries

## Success Criteria

- [ ] `ANTHROPIC_API_KEY = "sk-ant-..."` in config.toml works
- [ ] `export ANTHROPIC_API_KEY=sk-ant-...` (env var) works as fallback
- [ ] Provider auto-detected from key name (no `llm_provider` field needed)
- [ ] Old `llm_provider` / `llm_api_key_env` fields removed
- [ ] `config.test.toml.example` uses new format
- [ ] All tests pass (`just test-all`)
- [ ] Voyage AI key follows same pattern (`VOYAGE_API_KEY` in TOML or env)
