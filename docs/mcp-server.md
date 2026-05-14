# MCP Server

MongoCore includes a built-in [Model Context Protocol](https://modelcontextprotocol.io/) (MCP) server, allowing AI agents like Claude, GPT, and others to interact with your MongoDB data through a standardized JSON-RPC interface.

## Overview

The MCP server runs on port 3000 (configurable) and exposes MongoDB operations as MCP tools. AI agents can discover available tools, call them with structured arguments, and receive formatted results.

```
┌──────────────┐     JSON-RPC 2.0      ┌───────────────┐
│  AI Agent    │ ──────────────────────│   MongoCore   │
│  (Claude,    │   POST /mcp           │   MCP Server  │
│   GPT, etc.) │                       │   :3000       │
└──────────────┘                       └───────────────┘
```

## Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/mcp` | POST | JSON-RPC 2.0 request handler |
| `/health` | GET | Health check (returns 200 OK) |

## Available Tools

The MCP server exposes 36 tools:

### CRUD & Query

| Tool | Description |
|------|-------------|
| `find` | Find documents matching a filter (max 100 results) |
| `find_one` | Find a single document |
| `insert` | Insert one document |
| `insert_many` | Insert multiple documents |
| `update` | Update the first matching document |
| `update_many` | Update all matching documents |
| `delete` | Delete the first matching document |
| `delete_many` | Delete all matching documents |
| `aggregate` | Run an aggregation pipeline |
| `run_command` | Execute an arbitrary MongoDB command |

### Database Administration

| Tool | Description |
|------|-------------|
| `create_collection` | Create a new collection |
| `create_index` | Create an index |
| `list_databases` | List all databases |
| `list_collections` | List collections in a database |
| `collection_schema` | Sample documents and infer collection schema |

### Ingestion

| Tool | Description |
|------|-------------|
| `ingest` | Start a file ingestion job |
| `ingest_status` | Check ingestion job status |
| `list_ingest_jobs` | List all ingestion jobs |
| `cancel_ingest` | Cancel a running ingestion job |
| `watch_directory` | Watch a directory for new files and auto-ingest |
| `stop_watch` | Stop watching a directory |
| `ingest_and_embed` | Parse a file, embed a text field, and store with vectors |

### Search & Embeddings

| Tool | Description |
|------|-------------|
| `semantic_search` | Search for semantically similar documents using vector embeddings |
| `embed_and_store` | Embed text fields and store documents with vector embeddings |

### Natural Language Queries

| Tool | Description |
|------|-------------|
| `ask` | Ask a natural language question about your data (NL to MQL) |
| `explain_query` | Translate NL to MQL and show execution plan without running it |

### Code Generation

| Tool | Description |
|------|-------------|
| `generate_code` | Generate ready-to-run MongoCore client code for a query |
| `generate_model` | Generate a typed data model from a collection's inferred schema |
| `generate_index` | Analyze a query filter and generate index creation code |

### Observability

| Tool | Description |
|------|-------------|
| `get_analytics` | Get analytics summary (operation counts, error rates, latency percentiles) |
| `suggest_indexes` | Analyze query patterns and recommend missing indexes |
| `slow_queries` | Surface the slowest queries with optimization suggestions |

### Orchestration

| Tool | Description |
|------|-------------|
| `pipeline` | Execute multiple independent operations concurrently in a single round-trip |
| `transaction_pipeline` | Execute multiple dependent operations atomically in a transaction |

### Skills (Guided Workflows)

| Tool | Description |
|------|-------------|
| `list_skills` | List available guided workflows |
| `get_skill` | Get the full workflow guide for a specific skill |

## Safety Controls

The MCP server includes safety controls for AI agents:

- **Find limit cap**: Maximum 100 documents per `find` call (prevents runaway queries)
- **Read-only mode**: Block all write operations (insert, update, delete, create_collection, create_index, run_command, transaction_pipeline) for safe exploration
- **Pipeline validation**: All operations in a `pipeline` call are validated before execution — if any violates safety rules, the entire pipeline is rejected

## Protocol

### Initialize

```json
{
  "jsonrpc": "2.0",
  "method": "initialize",
  "id": 1
}
```

Response:
```json
{
  "jsonrpc": "2.0",
  "result": {
    "protocolVersion": "2024-11-05",
    "capabilities": {
      "tools": { "listChanged": false },
      "resources": { "subscribe": false, "listChanged": false }
    },
    "serverInfo": {
      "name": "mongocore",
      "version": "0.1.0"
    }
  },
  "id": 1
}
```

### List Tools

```json
{
  "jsonrpc": "2.0",
  "method": "tools/list",
  "id": 2
}
```

### Call a Tool

```json
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "find",
    "arguments": {
      "database": "myapp",
      "collection": "users",
      "filter": { "active": true },
      "limit": 5
    }
  },
  "id": 3
}
```

Response:
```json
{
  "jsonrpc": "2.0",
  "result": {
    "content": [{
      "type": "text",
      "text": "[{\"_id\": \"...\", \"name\": \"Alice\", \"active\": true}, ...]"
    }],
    "isError": false
  },
  "id": 3
}
```

### List Resources

```json
{
  "jsonrpc": "2.0",
  "method": "resources/list",
  "id": 4
}
```

Resources provide read-only access to database metadata (schemas, indexes, etc.) without using tool calls.

### Available Resources

| URI | Description |
|-----|-------------|
| `mongocore://capabilities` | MongoDB server capabilities (version, Atlas features) |
| `mongocore://databases` | List of all databases |
| `mongocore://collections/{database}` | List of collections in a database |
| `mongocore://schema/{database}/{collection}` | Inferred schema (field names, types, frequency) |

### Read a Resource

```json
{
  "jsonrpc": "2.0",
  "method": "resources/read",
  "params": {
    "uri": "mongocore://schema/myapp/users"
  },
  "id": 5
}
```

## Configuring for Claude Desktop

Add MongoCore as an MCP server in your Claude Desktop config (`~/Library/Application Support/Claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "mongocore": {
      "command": "mongocore",
      "args": ["--stdio", "--connection-uri", "mongodb://localhost:27017"],
      "env": {}
    }
  }
}
```

Or connect to a running instance via HTTP:

```json
{
  "mcpServers": {
    "mongocore": {
      "url": "http://localhost:3000/mcp"
    }
  }
}
```

## Configuring for Claude Code

Add to your project's `.mcp.json`:

```json
{
  "mcpServers": {
    "mongocore": {
      "command": "mongocore",
      "args": ["--stdio", "--connection-uri", "mongodb://localhost:27017"]
    }
  }
}
```

## Error Handling

Errors follow JSON-RPC 2.0 conventions:

| Code | Meaning |
|------|---------|
| -32601 | Method not found |
| -32602 | Invalid params (missing fields, unknown tool) |

Tool execution errors are returned as successful JSON-RPC responses with `isError: true` in the result, allowing the AI agent to understand and recover from the error.
