@AGENTS.md

# Claude Code Configuration

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
