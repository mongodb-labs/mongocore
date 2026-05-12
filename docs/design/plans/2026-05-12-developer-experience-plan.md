# Developer Experience (AGENTS.md + CLAUDE.md) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create AGENTS.md (universal AI agent guidance) and CLAUDE.md (Claude Code specialization with `@AGENTS.md` import) so any AI coding tool can work effectively on this codebase.

**Architecture:** Two markdown files at project root. AGENTS.md is self-contained and covers build, test, proto workflows, architecture rules, and design doc conventions. CLAUDE.md imports AGENTS.md via `@AGENTS.md` directive and adds Claude-specific commit style, workflow preferences, and MCP config.

**Tech Stack:** Markdown, project conventions.

---

## File Structure

| Action | Path | Responsibility |
|--------|------|----------------|
| Create | `AGENTS.md` | Universal guidance for any AI coding tool |
| Create | `CLAUDE.md` | Claude Code specialization (imports AGENTS.md) |

---

## Task 1: Create AGENTS.md

**Files:**
- Create: `AGENTS.md`

- [ ] **Step 1: Create AGENTS.md with full content**

```markdown
# MongoCore — Agent Development Guide

MongoCore is an AI-native MongoDB driver implemented as a Rust sidecar, serving all languages via gRPC with native AI agent support via MCP.

## Architecture

```
App (any lang) ──gRPC──▶ MongoCore Sidecar (Rust) ──Wire Protocol──▶ MongoDB
AI Agent ───────MCP────▶        │                  ──REST API──────▶ Voyage AI
                                └──Pluggable LLM──▶ Claude / OpenAI
```

Key directories:
- `src/` — Rust sidecar (the core)
- `proto/mongocore/v1/` — Protobuf definitions (API source of truth)
- `clients/{python,typescript,go,java}/` — Thin gRPC client wrappers
- `tests/integration/` — Integration tests (one file per subsystem)
- `docs/` — User-facing documentation
- `docs/design/` — Design specs and implementation plans

## Build

```bash
cargo build              # Compile + regenerate Rust proto stubs (via build.rs/tonic-build)
cargo build --release    # Optimized binary
docker build -t mongocore:dev .  # Container image
```

`cargo build` automatically regenerates Rust gRPC server/client code from proto files via `build.rs` (tonic-build). No manual step needed for Rust.

## Proto Workflow

**Proto files are the API contract. Change proto first, implement second.**

When any `.proto` file in `proto/mongocore/v1/` changes:

1. `cargo build` — regenerates Rust stubs automatically
2. Regenerate client stubs for all languages:

```bash
# Python
cd clients/python && python -m grpc_tools.protoc -I../../proto \
  --python_out=src/mongocore/generated --grpc_python_out=src/mongocore/generated \
  ../../proto/mongocore/v1/mongocore.proto ../../proto/mongocore/v1/types.proto \
  ../../proto/mongocore/v1/ingestion.proto

# TypeScript
cd clients/typescript && npx grpc_tools_node_protoc \
  --ts_out=src/generated --grpc_out=src/generated -I../../proto \
  ../../proto/mongocore/v1/mongocore.proto ../../proto/mongocore/v1/types.proto \
  ../../proto/mongocore/v1/ingestion.proto

# Go
cd clients/go && protoc --go_out=./proto --go-grpc_out=./proto -I../../proto \
  ../../proto/mongocore/v1/mongocore.proto ../../proto/mongocore/v1/types.proto \
  ../../proto/mongocore/v1/ingestion.proto

# Java
cd clients/java && protoc --java_out=src/main/java --grpc-java_out=src/main/java \
  -I../../proto ../../proto/mongocore/v1/mongocore.proto \
  ../../proto/mongocore/v1/types.proto ../../proto/mongocore/v1/ingestion.proto
```

3. Never modify generated files directly — they will be overwritten on next build.

## Testing

| Command | What | Dependencies |
|---------|------|-------------|
| `cargo test --lib` | Unit tests (~94) | None |
| `cargo test --test integration` | Integration tests (~53) | Docker MongoDB running |
| `just test-clients` | Client integration tests (~40) | Docker MongoDB + running sidecar |
| `just test-all` | Everything | All of the above |

**Starting MongoDB for tests:**
```bash
just docker-up    # or: docker compose -f docker-compose.test.yml up -d
just docker-down  # stop when done
```

### Test Gates

- **Before committing:** `cargo test --lib` must pass (non-negotiable)
- **Before PR:** `cargo test --test integration` must also pass
- **After proto changes:** verify `cargo build` succeeds (proves proto compiles)

## Adding a New RPC

End-to-end workflow when adding a new gRPC RPC:

1. Define messages and RPC in `proto/mongocore/v1/mongocore.proto` (or `ingestion.proto`)
2. Run `cargo build` — generates Rust types
3. Implement the operation logic in `src/operations/` (new file or existing module)
4. Implement gRPC handler in `src/grpc/service.rs`
5. Add MCP tool definition in `src/mcp/tools.rs`
6. Add MCP tool handler in `src/mcp/handler.rs`
7. Add safety rules in `src/mcp/safety.rs` (if it's a write operation)
8. Write integration test in `tests/integration/`
9. Regenerate all client stubs (see Proto Workflow above)
10. Add client method to each language client (`clients/{python,typescript,go,java}/`)
11. Update MCP tool count assertion in `tests/integration/mcp_test.rs`

## Adding a Config Field

1. Add CLI arg to `CliArgs` in `src/config.rs` (with `#[arg(long, env = "MONGOCORE_...")]`)
2. Add field to `FileConfig` (TOML deserialization struct)
3. Add field to `Config` (resolved configuration struct)
4. Add resolution logic in `Config::load()`: CLI > env > file > default
5. Add default constant to `src/defaults.rs` if applicable

## Architecture Rules

- **Proto is the API contract** — change proto first, implement second
- **MCP mirrors gRPC** — every user-facing RPC gets a corresponding MCP tool
- **Client libraries are thin** — ~200 lines of idiomatic wrapper per language, no business logic
- **Operations module** — all database logic lives in `src/operations/`, called by both gRPC and MCP
- **Error handling** — use `MongoCoreError` variants in `src/error.rs`, map to gRPC `Status` in service layer
- **Config layering** — CLI args > env vars > TOML file > hardcoded defaults

## Design Docs

Design specifications and implementation plans live in `docs/design/`:

- `docs/design/specs/` — Design specs describing what to build and why. Named `YYYY-MM-DD-<topic>-design.md`.
- `docs/design/plans/` — Step-by-step implementation plans with checkboxes. Named `YYYY-MM-DD-<topic>-plan.md`.

**Rules:**
- Before starting significant new work, check if a spec/plan exists
- When implementation deviates from the plan, update the plan to reflect reality
- Mark completed checkboxes in plans as work progresses
- If a spec becomes outdated (feature changed, approach abandoned), update or archive it
- The README roadmap section should stay in sync with what's actually in progress

## Don'ts

- Don't commit with `cargo test --lib` failing
- Don't modify files under generated/proto output directories
- Don't add business logic to client libraries (they're gRPC wrappers only)
- Don't add a gRPC RPC without a matching MCP tool
- Don't hardcode connection strings — use config/env
- Don't skip client stub regeneration after proto changes

## Project Layout

```
src/
├── main.rs              # Entrypoint, startup orchestration
├── lib.rs               # Public exports
├── config.rs            # CLI args, file config, resolved config
├── defaults.rs          # Default constants
├── error.rs             # MongoCoreError enum
├── connection/          # Pool, capability detection
├── operations/          # All DB logic (CRUD, aggregation, raw, transactions, admin)
├── ingestion/           # Polars reader, schema, transforms, writer, dedup, DLQ, watch
├── grpc/                # tonic server, service implementation
├── mcp/                 # axum HTTP server, JSON-RPC handler, tools, safety, resources
├── compiled/            # NL→MQL, cache hierarchy, LLM providers
├── search/              # Vector, fulltext, fallback chain
├── analytics/           # Collector, ring buffer, aggregator, persistence
├── tenant/              # Context, registry, isolation, quota
└── voyage/              # Voyage AI REST client, batch embeddings
proto/mongocore/v1/      # Protobuf definitions (source of truth for all APIs)
clients/
├── python/              # Python async client (BSON-native, change streams)
├── typescript/          # TypeScript/Node.js client (AsyncDisposable streams)
├── go/                  # Go client (io.Closer streams)
└── java/                # Java client (AutoCloseable, try-with-resources)
tests/integration/       # Integration tests (one file per subsystem)
docs/
├── design/specs/        # Design specifications
├── design/plans/        # Implementation plans
└── *.md                 # User-facing docs (getting-started, crud, search, etc.)
```
```

- [ ] **Step 2: Verify the file renders correctly**

Run: `head -20 AGENTS.md`
Expected: Shows the title and architecture section

- [ ] **Step 3: Commit**

```bash
git add AGENTS.md
git commit -m "docs: add AGENTS.md for universal AI agent development guidance"
```

---

## Task 2: Create CLAUDE.md

**Files:**
- Create: `CLAUDE.md`

- [ ] **Step 1: Create CLAUDE.md with @AGENTS.md import and Claude-specific content**

```markdown
@AGENTS.md

# Claude Code Configuration

## Commit Style

Use conventional commits matching the existing git history:
- `feat(scope):` — new feature
- `fix(scope):` — bug fix
- `test(scope):` — test additions/changes
- `docs:` — documentation only
- `chore:` — build, deps, tooling

Scopes match subsystems: `grpc`, `mcp`, `ingestion`, `analytics`, `tenant`, `compiled`, `search`, `clients`, `config`

Keep commit message titles concise (one sentence). Use the body for details when needed.

## Workflow Preferences

- Run `cargo test --lib` before committing — this is non-negotiable
- When touching proto files, always regenerate client stubs in the same commit
- Prefer `cargo build` over `cargo check` — it catches proto compilation issues
- Use `just docker-up` before integration tests, `just docker-down` when done
- When adding new MCP tools, update the tool count assertion in `tests/integration/mcp_test.rs`

## MCP Server (for development)

MongoCore can be used as an MCP server for development and testing. After building with `cargo build --release` and starting MongoDB (`just docker-up`):

```json
{
  "mcpServers": {
    "mongocore": {
      "command": "./target/release/mongocore",
      "args": ["--stdio", "--connection-uri", "mongodb://localhost:27017"],
      "env": {
        "MONGOCORE_LOG_LEVEL": "warn"
      }
    }
  }
}
```

## Task Runner

Use `just` commands where available:
- `just test-unit` — fast unit tests (no dependencies)
- `just test-integration` — needs Docker MongoDB running
- `just test-clients` — needs Docker MongoDB + running sidecar
- `just test-all` — everything
- `just docker-up` / `just docker-down` — manage test MongoDB container
- `just release-local` — build optimized binary
```

- [ ] **Step 2: Verify the @AGENTS.md import is the first line**

Run: `head -1 CLAUDE.md`
Expected: `@AGENTS.md`

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: add CLAUDE.md with Claude Code specialization (imports AGENTS.md)"
```

---

## Task 3: Verify README Accuracy

**Files:**
- Read: `README.md` (verify Project Structure section matches AGENTS.md)

- [ ] **Step 1: Compare README project structure with AGENTS.md**

Run: `grep -A 30 "## Project Structure" README.md`
Expected: Should match the layout in AGENTS.md. If there are discrepancies, update README to match current reality.

- [ ] **Step 2: Update README if needed**

If the Project Structure section in README.md has drifted from what's in AGENTS.md (which was written from current file listings), update README to match.

- [ ] **Step 3: Commit if changes were made**

```bash
git add README.md
git commit -m "docs: sync README project structure with AGENTS.md"
```

---

## Implementation Order

```
Task 1: AGENTS.md (independent, no prerequisites)
Task 2: CLAUDE.md (depends on Task 1 existing for the @import)
Task 3: README verification (depends on Task 1 for reference)
```

Tasks 2 and 3 can be done in parallel after Task 1.

---

## Definition of Done

- [ ] `AGENTS.md` exists at project root with complete development guidance
- [ ] `CLAUDE.md` exists at project root, starts with `@AGENTS.md`, adds Claude-specific config
- [ ] Proto workflow section includes all 4 client language regeneration commands
- [ ] Test gates are explicit (what must pass before commit vs before PR)
- [ ] README project structure matches AGENTS.md
- [ ] All files committed to git
