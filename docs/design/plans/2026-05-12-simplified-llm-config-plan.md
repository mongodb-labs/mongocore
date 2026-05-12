# Simplified LLM Configuration — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `just test-all` must pass (this runs all Rust tests + all client tests).
> If modifying client libraries: verify imports work and run `just test-clients`.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

**Goal:** Replace the indirect `llm_provider`/`llm_api_key_env`/`voyage_api_key_env` config fields with direct API key fields (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `VOYAGE_API_KEY`) that auto-detect the provider and support both TOML and env var sources.

**Architecture:** Remove 3 old fields from CliArgs/FileConfig/Config, add 3 new direct key fields. Resolution: TOML > env var. Provider auto-detected from which key is present. All struct literals in tests updated.

**Tech Stack:** Rust (clap, serde/toml), existing LLM provider types.

---

## File Structure

| Action | Path | Responsibility |
|--------|------|----------------|
| Modify | `src/config.rs` | Replace old fields with new ones, add resolution logic |
| Modify | `src/main.rs` | Update voyage key usage |
| Modify | `src/connection/pool.rs` | Update 4 test Config struct literals |
| Modify | `tests/harness/mongodb.rs` | Update test Config struct literal |
| Modify | `config.test.toml.example` | Update to new format |

---

## Task 1: Update Config Structs and Resolution Logic

**Files:**
- Modify: `src/config.rs`

- [ ] **Step 1: Update CliArgs — remove old fields, add new ones**

In `src/config.rs`, replace:
```rust
    /// LLM provider name
    #[arg(long, env = "MONGOCORE_LLM_PROVIDER")]
    pub llm_provider: Option<String>,

    /// Environment variable name containing the LLM API key
    #[arg(long, env = "MONGOCORE_LLM_API_KEY_ENV")]
    pub llm_api_key_env: Option<String>,

    /// Environment variable name containing the Voyage API key
    #[arg(long, env = "MONGOCORE_VOYAGE_API_KEY_ENV")]
    pub voyage_api_key_env: Option<String>,
```

With:
```rust
    /// Anthropic API key for compiled queries
    #[arg(long, env = "ANTHROPIC_API_KEY")]
    pub anthropic_api_key: Option<String>,

    /// OpenAI API key for compiled queries
    #[arg(long, env = "OPENAI_API_KEY")]
    pub openai_api_key: Option<String>,

    /// Voyage AI API key for embeddings
    #[arg(long, env = "VOYAGE_API_KEY")]
    pub voyage_api_key: Option<String>,
```

- [ ] **Step 2: Update FileConfig — remove old fields, add new ones**

Replace in `FileConfig`:
```rust
    pub llm_provider: Option<String>,
    pub llm_api_key_env: Option<String>,
    pub voyage_api_key_env: Option<String>,
```

With:
```rust
    #[serde(rename = "ANTHROPIC_API_KEY")]
    pub anthropic_api_key: Option<String>,
    #[serde(rename = "OPENAI_API_KEY")]
    pub openai_api_key: Option<String>,
    #[serde(rename = "VOYAGE_API_KEY")]
    pub voyage_api_key: Option<String>,
```

- [ ] **Step 3: Update Config struct — remove old fields, add new ones**

Replace in `Config`:
```rust
    pub llm_provider: Option<String>,
    pub llm_api_key_env: Option<String>,
    pub voyage_api_key_env: Option<String>,
```

With:
```rust
    pub llm_api_key: Option<String>,
    pub llm_provider_name: Option<String>,
    pub voyage_api_key: Option<String>,
```

- [ ] **Step 4: Update Config::load() resolution logic**

Replace:
```rust
        let llm_provider = cli.llm_provider.clone().or(file_config.llm_provider);

        let llm_api_key_env = cli.llm_api_key_env.clone().or(file_config.llm_api_key_env);

        let voyage_api_key_env = cli
            .voyage_api_key_env
            .clone()
            .or(file_config.voyage_api_key_env);
```

With:
```rust
        // Resolve LLM API key: CLI/env > TOML > env var fallback
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

        // Resolve Voyage API key: CLI/env > TOML > env var fallback
        let voyage_api_key = cli.voyage_api_key
            .clone()
            .or(file_config.voyage_api_key)
            .or_else(|| std::env::var("VOYAGE_API_KEY").ok());
```

- [ ] **Step 5: Update the Config struct literal in Ok(...)**

Replace:
```rust
            llm_provider,
            llm_api_key_env,
            voyage_api_key_env,
```

With:
```rust
            llm_api_key,
            llm_provider_name,
            voyage_api_key,
```

- [ ] **Step 6: Update the `default_cli()` helper in tests**

Replace:
```rust
    fn default_cli() -> CliArgs {
        CliArgs {
            config: None,
            connection_uri: None,
            grpc_port: None,
            mcp_port: None,
            llm_provider: None,
            llm_api_key_env: None,
            voyage_api_key_env: None,
            compiled_cache_sync: None,
            log_level: None,
            otel_enabled: None,
            otel_endpoint: None,
            otel_service_name: None,
        }
    }
```

With:
```rust
    fn default_cli() -> CliArgs {
        CliArgs {
            config: None,
            connection_uri: None,
            grpc_port: None,
            mcp_port: None,
            anthropic_api_key: None,
            openai_api_key: None,
            voyage_api_key: None,
            compiled_cache_sync: None,
            log_level: None,
            otel_enabled: None,
            otel_endpoint: None,
            otel_service_name: None,
        }
    }
```

- [ ] **Step 7: Update ALL other CliArgs struct literals in tests**

Search for every `CliArgs {` in `src/config.rs` and replace `llm_provider: None, llm_api_key_env: None, voyage_api_key_env: None,` with `anthropic_api_key: None, openai_api_key: None, voyage_api_key: None,`.

There are 5 occurrences (test_default_values, test_toml_parsing, test_cli_overrides_toml, test_invalid_toml_returns_error, test_tenant_config_parsing) plus `default_cli()`.

- [ ] **Step 8: Update test_default_values assertions**

Replace:
```rust
        assert!(config.llm_provider.is_none());
        assert!(config.llm_api_key_env.is_none());
        assert!(config.voyage_api_key_env.is_none());
```

With:
```rust
        assert!(config.llm_api_key.is_none());
        assert!(config.llm_provider_name.is_none());
        assert!(config.voyage_api_key.is_none());
```

- [ ] **Step 9: Update test_toml_parsing TOML content and assertions**

Replace the TOML content in the test:
```rust
        let toml_content = r#"
connection_uri = "mongodb://myhost:27018"
grpc_port = 9090
mcp_port = 4000
ANTHROPIC_API_KEY = "sk-ant-test-key"
VOYAGE_API_KEY = "voyage-test-key"
compiled_cache_sync = false
log_level = "debug"
"#;
```

Replace assertions:
```rust
        assert_eq!(config.llm_provider_name.as_deref(), Some("anthropic"));
        assert_eq!(config.llm_api_key.as_deref(), Some("sk-ant-test-key"));
        assert_eq!(config.voyage_api_key.as_deref(), Some("voyage-test-key"));
```

- [ ] **Step 10: Run config tests**

Run: `cargo test --lib config`
Expected: All tests pass

- [ ] **Step 11: Commit**

```bash
git add src/config.rs
git commit -m "feat(config): replace llm_provider/llm_api_key_env with direct API key fields"
```

---

## Task 2: Update Main.rs and Test Harnesses

**Files:**
- Modify: `src/main.rs`
- Modify: `src/connection/pool.rs`
- Modify: `tests/harness/mongodb.rs`

- [ ] **Step 1: Update main.rs voyage key resolution**

In `src/main.rs`, replace:
```rust
    // Resolve Voyage AI API key if configured
    let voyage_api_key = config
        .voyage_api_key_env
        .as_deref()
        .and_then(|env_var| std::env::var(env_var).ok());
```

With:
```rust
    // Voyage AI API key (already resolved from TOML or env in config)
    let voyage_api_key = config.voyage_api_key.as_deref().map(|s| s.to_string());
```

Note: The variable is used later as `voyage_api_key.as_deref()` so ensure the type stays compatible. It was `Option<String>` before and still will be.

- [ ] **Step 2: Update connection/pool.rs test Config struct literals**

There are 4 Config struct literals in `src/connection/pool.rs` tests. In each one, replace:
```rust
            llm_provider: None,
            llm_api_key_env: None,
            voyage_api_key_env: None,
```

With:
```rust
            llm_api_key: None,
            llm_provider_name: None,
            voyage_api_key: None,
```

- [ ] **Step 3: Update tests/harness/mongodb.rs Config struct literal**

Replace:
```rust
        llm_provider: None,
        llm_api_key_env: None,
        voyage_api_key_env: None,
```

With:
```rust
        llm_api_key: None,
        llm_provider_name: None,
        voyage_api_key: None,
```

- [ ] **Step 4: Build and run all tests**

```bash
cargo test --lib
cargo test --test integration
```

Expected: All pass (208 unit, 81 integration including 7 LLM skips)

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/connection/pool.rs tests/harness/mongodb.rs
git commit -m "fix: update main.rs and test harnesses for new API key config fields"
```

---

## Task 3: Update Config Example File

**Files:**
- Modify: `config.test.toml.example`

- [ ] **Step 1: Rewrite config.test.toml.example**

Replace the entire contents with:
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

- [ ] **Step 2: Commit**

```bash
git add config.test.toml.example
git commit -m "docs: update config.test.toml.example with new direct API key format"
```

---

## Task 4: Verification and Regression

- [ ] **Step 1: Run full Rust test suite**

```bash
cargo test --lib
cargo test --test integration
```

Expected: All pass.

- [ ] **Step 2: Verify env var fallback works**

The `Config::load()` resolution falls back to `std::env::var("ANTHROPIC_API_KEY")`. This is the same env var that `docker-compose.test.yml` and `compiled_llm_test.rs` check. Confirm the tests still skip/pass correctly:

```bash
unset ANTHROPIC_API_KEY
cargo test --test integration compiled_llm -- --nocapture 2>&1 | grep -i skip
```

Expected: All 7 LLM tests print "Skipping" messages.

- [ ] **Step 3: Run client tests (if sidecar running)**

```bash
just test-clients
```

Expected: All pass (no client code changed, just config struct).

- [ ] **Step 4: Commit any fixes**

If anything failed, fix and commit.

---

## Implementation Order

```
Task 1: Config struct changes (foundation — everything depends on this)
Task 2: main.rs + test harnesses (depends on Task 1)
Task 3: Config example file (independent, cosmetic)
Task 4: Verification (depends on all above)
```

---

## Definition of Done

- [ ] `CliArgs` has `anthropic_api_key`, `openai_api_key`, `voyage_api_key` (old fields removed)
- [ ] `FileConfig` uses `#[serde(rename = "ANTHROPIC_API_KEY")]` etc. for TOML parsing
- [ ] `Config` has `llm_api_key`, `llm_provider_name`, `voyage_api_key` (auto-detected)
- [ ] Resolution: TOML key > env var fallback, provider auto-detected from key name
- [ ] `config.test.toml.example` uses new `ANTHROPIC_API_KEY = "..."` format
- [ ] All test struct literals updated (config.rs, pool.rs, harness)
- [ ] `cargo test --lib` passes (208 tests)
- [ ] `cargo test --test integration` passes
- [ ] Voyage API key resolved directly (no env var indirection in main.rs)
