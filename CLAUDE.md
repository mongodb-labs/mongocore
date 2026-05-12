@AGENTS.md

# Claude Code Configuration

## Subagent Dispatch Rules

When dispatching implementation subagents, always include in the prompt:
1. "Read and follow `AGENTS.md` at the project root"
2. "Before committing: run `cargo test --lib` AND verify `cargo test --test integration` compiles"
3. If changes touch client libraries: "Run `just test-clients` or verify client imports work"
4. If changes add fields to shared structs (like `Config`): "Search for ALL struct literals across `src/` AND `tests/` and update them"

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
