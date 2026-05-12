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
