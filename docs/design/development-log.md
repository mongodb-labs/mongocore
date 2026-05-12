# Development Log — Building MongoCore with Claude Code

This is a summarized session history capturing how MongoCore was built across two intensive pairing sessions with Claude Code (Anthropic's AI coding assistant). It documents the conversational flow, key decisions, debugging stories, and reflections on the process.

---

## Session 1: The Foundation (v0.1 → v0.3)

### The Vision

The session opened with a brainstorm: "What if there was one Rust sidecar that served every language via gRPC, with AI agents as first-class citizens via MCP?" The design spec was written first — establishing five principles:

1. Opinionated defaults, escape hatch for power users
2. Schema is opt-in, not a gate
3. AI-native from the outset
4. Single Rust core serves all languages
5. Compiled queries eliminate repeated AI costs

The "compiled queries" concept was the novel insight: translate NL→MQL once via LLM, cache forever. Pay once, run at native speed.

### Building in Order

The implementation followed a strict dependency chain:
- Connection pool with opinionated defaults (majority concerns, retryable ops)
- CRUD operations → aggregation → transactions → admin
- gRPC server (tonic) exposing 25 RPCs
- MCP server (axum) with 13 tools + safety controls
- Compiled query system with 3-level cache (memory → disk → Atlas)
- Voyage AI integration for vector embeddings
- Atlas Vector Search and Full-Text Search with fallback chain
- Change streams with auto-close semantics

Each layer was spec'd, planned, and built task-by-task with tests.

### The "Thin Wrapper" Philosophy

Client libraries (Python, TypeScript, Go, Java) were deliberately kept to ~200 lines each. All business logic lives in the Rust sidecar. Clients are pure gRPC wrappers with idiomatic language patterns (Python `async with`, TypeScript `AsyncDisposable`, Go `io.Closer`, Java `AutoCloseable`).

### v0.2: Power User Features

Added raw wire protocol passthrough (RunCommand), query analytics with ring buffers and percentiles, and multi-tenant support with isolated caches and per-tenant rate limiting. Each was an independent subsystem built in parallel.

### v0.3: The Polars Bet

For data ingestion, the decision was to use Polars (not just the MongoDB driver bulk write). Polars gave us: format detection, lazy evaluation, columnar parallelism, and eventually cloud URL support for free. The ingestion engine handles CSV/JSON/Parquet with schema inference, transforms, dedup, DLQ, progress tracking, and directory watching.

### Reflection on Session 1

99 commits in a single session. The spec-first approach meant nothing was built without understanding why. The implementation plans served as both instructions and documentation. The biggest risk was scope — v0.1 through v0.3 is an enormous amount of functionality. But because each layer was independent and testable, it worked.

---

## Session 2: Hardening & Intelligence (v0.4 → v0.6)

### Starting Point: "Where are we on the roadmap?"

Session 2 opened by reviewing what existed. The codebase was complete (v0.1-v0.3 all implemented) but had no developer guidance, no test coverage verification, and the LLM features weren't tested against real providers. The question was: "What should we build next, and what should the demo look like?"

### AGENTS.md: Born from Necessity

**The conversation:** "I want to add AGENTS.md and CLAUDE.md for this project."

This became the first task — establishing rules for both humans and AI agents working on the codebase. AGENTS.md covers build, test, proto workflow, architecture rules. CLAUDE.md extends it with Claude-specific extras. A key decision: "Is this primarily for Claude Code or multi-agent?" → "Multi-agent" — AGENTS.md is universal.

**The lesson that made it essential:** Later, when subagents were deployed to implement features, they didn't know the proto regeneration workflow, didn't know about the integration test harness, and committed with missing struct fields. Every failure led to a new rule in AGENTS.md.

### Integration Improvements: The OTel Saga

**Driver metadata** was straightforward — the Rust driver has `append_metadata()`. The interesting question was: "Does the Rust MongoDB driver allow users to set client metadata?" This led to discovering the per-interface append pattern.

**URL ingestion** was a happy surprise. "Can we just send the string to Polars and let it read it?" → Yes. Polars with the `cloud` feature handles `http://`, `s3://`, `gs://`, `az://` identically to local paths. Zero branching code needed.

**OpenTelemetry** hit a wall. The plan used the old pipeline API (`opentelemetry_otlp::new_pipeline().tracing()...`) but opentelemetry 0.28 completely changed the API. The fix required reading the crate's source code to find `SpanExporter::builder().with_tonic()` and `SdkTracerProvider::builder()`. This was the first time `just test-all` would have caught the issue — it only manifested when building with `--features otel`.

### Client Test Coverage: The Proto Regeneration Saga

**The problem:** Only 11 of 27 RPCs were tested per client. Six methods (FindAndModify, transactions, etc.) didn't even have client wrappers.

**The Python circular import:** When adding `_CLIENT_METADATA` to each file, `collection.py` imported from `client.py` which imported `database.py` which imported `collection.py`. Fix: define the constant locally in each file.

**The Go proto marshaling failure:** `WatchDirectoryRequest` failed with "proto: failed to marshal, message is *proto.WatchDirectoryRequest, want proto.Message". The Go proto stubs were generated with an incompatible protoc-gen-go version. Fix: `go install google.golang.org/protobuf/cmd/protoc-gen-go@latest` and regenerate.

**The exit code 143:** `just test-clients` started the sidecar in background and killed it at the end via `trap`. But `wait` on the killed process returned 143 (SIGTERM), which `just` treated as failure. Fix: `wait $PID 2>/dev/null || true`.

**Reflection:** Every one of these failures led to a new rule in AGENTS.md or CLAUDE.md. The documentation grew organically from real pain.

### LLM Gateway: From Hardcoded to Corporate

**The conversation:** "I need to call my LLM via HTTPS through a corporate gateway with a custom auth header."

This spawned the `GatewayProvider` — a new provider alongside Claude and OpenAI that takes configurable URL, auth header, model, and provider type. The key insight: the gateway uses the same request/response format as the direct providers, just at a different URL with different auth.

**The simplified config:** "What is my ANTHROPIC_API_KEY?" → This question revealed the old config was confusing (indirect `llm_api_key_env` pointing to an env var). Simplified to: put the key directly in TOML or set as env var. Provider auto-detected from which key is present.

### NQL→MQL Testing: Real LLM Calls

**The markdown fence discovery:** First time calling the real LLM, every test failed with "Invalid JSON from LLM: expected value at line 1 column 1". Added debug output → the LLM wraps responses in ` ```json ``` ` despite being told not to. Fix: strip markdown fences in the parser.

**The config.test.toml revelation:** "just test-llm doesn't seem to be picking up my config.test.toml" → The tests read env vars directly but the config was in a TOML file. Fix: `load_test_config()` reads TOML first, env vars override. Single source of truth.

**Sample data loading:** Went through three iterations:
1. `${ANTHROPIC_API_KEY:+true}` in docker-compose (coupled to LLM config)
2. `LOAD_SAMPLE_DATA=true` env var (separate flag)
3. Just always load it (simplest — it's fast after first start)

### Intelligent Routing: The Revolutionary Insight

**The conversation that changed everything:**

"Could we ask the LLM to provide the template and values?"

This was the breakthrough. Instead of trying to reverse-engineer templates from the NL input (the existing approach only caught numeric literals), just ask the LLM to tell us what's variable. The LLM already understands intent — it knows "Italian" in "find Italian restaurants" is a parameter.

**The follow-up:** "Could we use layered caching for text numbers like 'a million'?" This led to the realization that the LLM is not just a translator — it's a **router**. It decides:
- Which execution method (filter/aggregate/vector/fulltext/geo)
- What the parameterized template looks like
- Which values are variable

**Research-driven roadmap:** Studied 94 repos in `mongodb-industry-solutions` and MongoDB docs to identify the top real-world patterns. This informed both what to build now (routing, templates, $lookup) and what to defer (graph queries, window functions, hybrid search).

### Zero Warnings Policy

After the third time a subagent committed code with compiler warnings:
1. First incident: unused `BsonSchema` import
2. Second incident: unused `LlmTemplateParameter` import
3. Third incident: dead `tenant_registry` field

Added to AGENTS.md: "`cargo build` must produce ZERO warnings before every commit." Added to CLAUDE.md subagent rules. Added to the plan header template. The policy is now enforced at three levels.

### Reflection on Session 2

The session evolved from "let's add some features" to fundamentally rethinking how the compiled query system works. The LLM-as-router pattern, template registry, and method classification emerged from conversation — not from the initial plan. The best features came from asking "what if?" during brainstorming rather than from a predetermined roadmap.

The debugging stories (OTel API, proto stubs, markdown fences) all share a theme: the real world doesn't match documentation. Every fix made the system more robust and the AGENTS.md more comprehensive.

---

## Methodology Evolution

### Spec → Plan → Execute

Every significant feature followed this flow:
1. **Brainstorm** — understand the problem, explore 2-3 approaches, get user decision
2. **Spec** — write a design doc capturing what and why
3. **Plan** — step-by-step implementation with exact code, files, and commands
4. **Execute** — dispatch subagents per task with review between each

The specs and plans serve dual purpose: instructions for implementation AND documentation for future maintainers.

### Subagent-Driven Development

Fresh subagent per task + two-stage review. What worked:
- Parallel execution of independent tasks
- Clean context per subagent (no accumulated confusion)
- Each subagent gets exactly the context it needs

What broke:
- Subagents don't inherit project rules (AGENTS.md) automatically
- Shared struct changes break in files the subagent doesn't know about
- Proto regeneration is invisible to subagents
- Warning checks only happen if explicitly requested

Every failure → new rule in AGENTS.md or CLAUDE.md.

### The AGENTS.md Story

Started as a simple "here's how to build and test." Grew to include:
- Proto workflow (the most forgotten step)
- Test gates (what must pass before commit)
- Architecture rules (proto first, MCP mirrors gRPC)
- Zero warnings policy (born from subagent failures)
- Testing rules (every RPC gets a test in every client)
- Development log updates (keep the narrative current)
- Plan header requirements (rules travel with the plan)

It's now the single most important file for anyone (human or AI) working on this codebase.

---

## Statistics & Final State

| Metric | Count |
|--------|-------|
| Total commits | 99+ |
| Rust unit tests | 233 |
| Rust integration tests | ~97 |
| LLM integration tests | 23 |
| Client tests | ~126 (26 integration + 5 unit × 4 languages) |
| gRPC RPCs | 25 |
| MCP tools | 21 |
| Client libraries | 4 (Python, TypeScript, Go, Java) |
| Design specs | 11 |
| Implementation plans | 11 |
| Versions | v0.1 → v0.6 |

---

## What's Next

See [Roadmap](./roadmap.md) for the full future roadmap including:
- Search RPC integration (wire translator into search handler)
- Query explanation (show generated MQL to users)
- Hybrid search with reciprocal rank fusion
- Window functions, graph queries
- Enterprise compliance (audit trail, RBAC, governance)
