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

The MCP server exposes 21 tools:

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
| `create_collection` | Create a new collection |
| `create_index` | Create an index |
| `list_databases` | List all databases |
| `list_collections` | List collections in a database |

## Safety Controls

The MCP server includes safety controls for AI agents:

- **Find limit cap**: Maximum 100 documents per `find` call (prevents runaway queries)
- **Tool allowlist/blocklist**: Configure which tools AI agents can access
- **Read-only mode**: Block all write operations for safe exploration

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

### Read a Resource

```json
{
  "jsonrpc": "2.0",
  "method": "resources/read",
  "params": {
    "uri": "mongodb://myapp/users/schema"
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
      "args": ["--connection-uri", "mongodb://localhost:27017"],
      "env": {}
    }
  }
}
```

Or connect to a running instance:

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
      "args": ["--connection-uri", "mongodb://localhost:27017"]
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
