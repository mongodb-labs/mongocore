# MongoCore: Developer Experience — AGENTS.md & CLAUDE.md

## Overview

Add AGENTS.md (universal AI agent guidance) and CLAUDE.md (Claude Code specialization with `@AGENTS.md` import) to enable any AI coding tool to work effectively on this codebase. Focus on proto regeneration workflows, test gates, architecture rules, and common development patterns.

## Motivation

MongoCore is a multi-module project (Rust sidecar + 4 client libraries + proto definitions) where changes often cascade across boundaries. AI agents need to understand:
- The proto-first workflow (change proto → rebuild → implement → update clients)
- Which tests must pass before committing
- Where to put new code and how to follow existing patterns
- What not to do (modify generated code, add logic to clients, skip regeneration)

Without this guidance, agents make predictable mistakes: implementing without updating protos, forgetting client stub regeneration, committing with test failures.

## Design

### File Structure

```
AGENTS.md          # Universal guidance for any AI coding tool
CLAUDE.md          # Claude Code specialization (imports AGENTS.md)
```

### AGENTS.md — Universal Agent Guidance

The primary file. Any AI tool (Claude, Copilot, Cursor, Codex, etc.) should read this.

#### Sections

**1. Project Overview (brief)**
- What MongoCore is (one sentence)
- Architecture: Rust sidecar → gRPC + MCP → MongoDB
- Key directories and their roles

**2. Build**
- `cargo build` compiles Rust and regenerates proto stubs via `build.rs` (tonic-build)
- Release: `cargo build --release`
- Docker: `docker build -t mongocore:dev .`

**3. Proto Workflow (critical path)**

When any `.proto` file changes:
1. `cargo build` — regenerates Rust server/client stubs automatically
2. Client stubs must be regenerated manually:
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
3. Never modify generated files directly — they'll be overwritten

**4. Testing**

| Command | What | Dependencies |
|---------|------|-------------|
| `cargo test --lib` | Unit tests (94) | None |
| `cargo test --test integration` | Integration tests (53) | Docker MongoDB running |
| `just test-clients` | Client integration (40) | Docker MongoDB + running sidecar |
| `just test-all` | Everything | All of the above |

**Gates:**
- Before committing: `cargo test --lib` must pass
- Before PR: `cargo test --test integration` must pass (run `just docker-up` first)
- After proto changes: verify `cargo build` succeeds (proves proto compiles)

**5. Adding a New RPC (end-to-end workflow)**

1. Define messages and RPC in `proto/mongocore/v1/mongocore.proto` (or `ingestion.proto` for ingestion-related)
2. `cargo build` — generates Rust types
3. Implement handler in `src/grpc/service.rs`
4. Add MCP tool definition in `src/mcp/tools.rs`
5. Add MCP tool handler in `src/mcp/handler.rs`
6. Add safety rules in `src/mcp/safety.rs` (if write operation)
7. Write unit tests inline, integration test in `tests/integration/`
8. Regenerate all client stubs (see Proto Workflow)
9. Add client method to each language client
10. Update MCP tool count assertion in `tests/integration/mcp_test.rs`

**6. Adding a Config Field**

1. Add CLI arg to `CliArgs` in `src/config.rs` (with `#[arg(long, env = "MONGOCORE_...")]`)
2. Add field to `FileConfig` (TOML deserialization)
3. Add field to `Config` (resolved config)
4. Add resolution logic: CLI > env > file > default
5. Add default constant to `src/defaults.rs` if applicable

**7. Architecture Rules**

- **Proto is the API contract** — change proto first, implement second
- **MCP mirrors gRPC** — every user-facing RPC gets a corresponding MCP tool
- **Client libraries are thin** — ~200 lines of idiomatic wrapper per language, no business logic
- **Operations module** — all database logic lives in `src/operations/`, called by both gRPC and MCP
- **Error handling** — use `MongoCoreError` variants, map to gRPC `Status` in service layer
- **Config layering** — CLI args > env vars > TOML file > hardcoded defaults

**8. Don'ts**

- Don't commit with `cargo test --lib` failing
- Don't modify files under generated/proto output directories
- Don't add business logic to client libraries (they're gRPC wrappers)
- Don't add a gRPC RPC without a matching MCP tool
- Don't hardcode connection strings — use config/env
- Don't skip proto regeneration for client languages after proto changes

**9. Project Layout Reference**

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
proto/mongocore/v1/      # Protobuf definitions (source of truth)
clients/{python,typescript,go,java}/  # Thin gRPC client wrappers
tests/integration/       # Integration tests (one file per subsystem)
docs/design/
├── specs/               # Design specifications (the "what and why")
└── plans/               # Implementation plans (the "how", step-by-step)
```

**10. Design Docs**

Design specifications and implementation plans live in `docs/design/`:
- `docs/design/specs/` — Design specs describing what to build and why. Named `YYYY-MM-DD-<topic>-design.md`.
- `docs/design/plans/` — Step-by-step implementation plans with checkboxes. Named `YYYY-MM-DD-<topic>-plan.md`.

**Rules:**
- Before starting significant new work, check if a spec/plan exists
- When implementation deviates from the plan, update the plan to reflect reality
- Mark completed checkboxes in plans as work progresses
- If a spec becomes outdated (feature changed, approach abandoned), update or archive it
- The README roadmap section should stay in sync with what's actually in progress

### CLAUDE.md — Claude Code Specialization

Starts with `@AGENTS.md` import, then adds Claude-specific guidance:

```markdown
@AGENTS.md

# Claude Code Configuration

## Commit Style
- Use conventional commits: `feat(scope):`, `fix(scope):`, `test(scope):`, `docs:`, `chore:`
- Scope matches the subsystem: grpc, mcp, ingestion, analytics, tenant, compiled, search, clients
- Keep messages concise (one sentence for title)

## Workflow Preferences
- Run `cargo test --lib` before committing (non-negotiable)
- When touching proto files, always regenerate client stubs in the same commit
- Prefer `cargo build` over `cargo check` — catches proto issues
- Use `just docker-up` before integration tests, `just docker-down` after

## MCP Server (for development)
MongoCore itself can be used as an MCP server during development:
```json
{
  "mcpServers": {
    "mongocore": {
      "command": "./target/release/mongocore",
      "args": ["--stdio", "--connection-uri", "mongodb://localhost:27017"],
      "env": { "MONGOCORE_LOG_LEVEL": "warn" }
    }
  }
}
```

## Task Runner
Use `just` commands where available:
- `just test-unit` — fast unit tests
- `just test-integration` — needs Docker
- `just docker-up` / `just docker-down` — manage test MongoDB
- `just release-local` — optimized binary
```

## Implementation Scope

| File | Content |
|------|---------|
| `AGENTS.md` | Universal agent guidance: build, test, proto workflow, architecture rules, common patterns |
| `CLAUDE.md` | `@AGENTS.md` import + commit style, workflow preferences, MCP config, task runner reference |

## Won't Include

- IDE-specific configuration (.vscode, .idea settings)
- CI/CD pipeline documentation (belongs in its own docs)
- Detailed API reference (that's the proto files + docs/ folder)
- Tutorial content (that's docs/getting-started.md)

## Success Criteria

- [ ] Any AI agent reading AGENTS.md can add a new RPC end-to-end without asking for help
- [ ] Proto workflow section prevents the "forgot to regenerate stubs" class of errors
- [ ] Test gates are explicit — agents know what must pass before committing vs before PR
- [ ] CLAUDE.md imports AGENTS.md and adds only Claude-specific extras
- [ ] README.md remains accurate (verify project structure section matches reality)
