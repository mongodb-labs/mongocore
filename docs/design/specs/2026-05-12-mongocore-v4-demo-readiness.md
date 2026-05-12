# MongoCore v4: Demo Readiness — Stdio MCP Transport & Demo Flow

## Overview

Make MongoCore usable as a Claude Code MCP server (auto-launched via stdio transport) and create a self-contained demo that showcases the entire MongoCore value proposition: an AI agent ingests a restaurant dataset, explores the data, performs semantic queries, and builds a working web app — all driven by Claude with zero human code.

## Motivation

Internal MongoDB stakeholders need to see MongoCore in action. The most compelling proof is a live session where Claude Code uses MongoCore as its MCP server to do real database work end-to-end. This requires:

1. Claude Code can auto-launch MongoCore (stdio transport)
2. A curated dataset that produces impressive semantic search results
3. A scripted demo flow that hits all the architectural highlights

## Design Principles

- **Zero-friction start** — One `just` command to set up, then Claude does the rest
- **Self-contained** — No external dependencies beyond Docker and a built binary
- **Show, don't tell** — Every feature demonstrated through agent action, not slides

## Architecture

### Stdio MCP Transport

Claude Code launches MCP servers as child processes, communicating via JSON-RPC over stdin/stdout. MongoCore currently only supports HTTP transport (`POST /mcp`). We add a stdio mode:

```
┌─────────────┐   stdin (JSON-RPC)   ┌──────────────────────┐
│ Claude Code │ ───────────────────▶  │                      │
│             │   stdout (JSON-RPC)   │   MongoCore          │
│             │ ◀───────────────────  │   (--stdio mode)     │
└─────────────┘                       │                      │───▶ MongoDB
                                      └──────────────────────┘
```

When launched with `--stdio`:
- No HTTP server starts (no axum, no port binding)
- No gRPC server starts
- Reads newline-delimited JSON-RPC from stdin
- Writes newline-delimited JSON-RPC to stdout
- All logging goes to stderr (critical — stdout is the protocol channel)
- Same `McpHandler` logic as the HTTP path
- Connection to MongoDB still happens on startup (connection_uri from args or env)

### CLI Changes

Add to `CliArgs`:
```
--stdio     Run in stdio MCP mode (no HTTP/gRPC servers, JSON-RPC over stdin/stdout)
```

When `--stdio` is set:
1. Parse config as normal
2. Connect to MongoDB
3. Initialize ingestion engine, analytics, etc. as normal
4. Instead of starting HTTP+gRPC servers, enter a stdin read loop
5. Log startup banner to stderr

### Stdin Read Loop

```rust
async fn run_stdio_mode(handler: McpHandler) {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let reader = BufReader::new(stdin);
    let mut lines = reader.lines();

    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<JsonRpcRequest>(&line) {
            Ok(request) => {
                let response = handler.handle_request(request).await;
                let response_json = serde_json::to_string(&response).unwrap();
                // Write to stdout with newline
                let mut out = stdout.lock().await;
                out.write_all(response_json.as_bytes()).await.unwrap();
                out.write_all(b"\n").await.unwrap();
                out.flush().await.unwrap();
            }
            Err(e) => {
                let error_response = JsonRpcResponse::error(
                    None, -32700, format!("Parse error: {}", e)
                );
                let json = serde_json::to_string(&error_response).unwrap();
                let mut out = stdout.lock().await;
                out.write_all(json.as_bytes()).await.unwrap();
                out.write_all(b"\n").await.unwrap();
                out.flush().await.unwrap();
            }
        }
    }
}
```

### Logging in Stdio Mode

All tracing output must go to stderr. The tracing subscriber is already configured via `tracing_subscriber::fmt()` — we set the writer to stderr when in stdio mode:

```rust
if stdio_mode {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(filter)
        .init();
} else {
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();
}
```

## Claude Code MCP Configuration

### Config file for Claude Code

The user adds this to their Claude Code MCP settings (`.claude/settings.json` or project-level):

```json
{
  "mcpServers": {
    "mongocore": {
      "command": "/path/to/mongocore",
      "args": ["--stdio", "--connection-uri", "mongodb://localhost:27017"],
      "env": {
        "MONGOCORE_LOG_LEVEL": "info"
      }
    }
  }
}
```

### Demo Setup Script

A `just demo-setup` command that:
1. Builds the release binary (`cargo build --release`)
2. Starts MongoDB via Docker (`docker compose -f docker-compose.test.yml up -d`)
3. Prints the MCP config JSON to paste into Claude Code settings
4. Optionally copies the demo dataset to a known location

## Demo Dataset: Restaurants

### Requirements
- ~800-1000 rows (enough for interesting queries, small enough to ingest quickly)
- Rich text descriptions (2-3 sentences per restaurant) for meaningful vector/semantic search
- Diverse cuisines, neighborhoods, price ranges
- Boolean/enum fields for filtering: outdoor_seating, vegetarian_friendly, takes_reservations
- Numeric fields: rating (1-5), average_price, year_opened

### Schema

```csv
name,cuisine,neighborhood,city,description,rating,average_price,outdoor_seating,vegetarian_friendly,takes_reservations,year_opened
```

### Source

Generate a synthetic dataset optimized for demo queries. Key properties:
- Descriptions are written in natural language (not just keyword lists)
- Enough variety that "cozy Italian with outdoor seating" returns 3-5 results, not 100
- Some entries that exercise edge cases: new restaurants (high/low ratings), unique cuisines

The dataset lives at `demo/data/restaurants.csv`.

## Demo Flow (Scripted)

The demo is driven by a human typing prompts into Claude Code. The file `demo/DEMO_SCRIPT.md` contains the exact prompts and expected outcomes.

### Act 1: Setup & Ingestion (~2 minutes)

**Human says:** "I have a restaurant dataset at demo/data/restaurants.csv. Ingest it into MongoDB — put it in a database called 'foodfinder' collection 'restaurants'. Embed the description field for semantic search."

**Claude does (via MCP tools):**
1. `ingest` — loads CSV, infers schema, bulk writes to MongoDB
2. Confirms: row count, inferred schema, embedding status

### Act 2: Exploration & Queries (~3 minutes)

**Human says:** "What did we just load? Show me the schema and some stats."

**Claude does:**
1. `list_collections` — shows foodfinder.restaurants
2. `get_analytics` — shows ingestion stats
3. `find` with limit — shows sample documents

**Human says:** "Find me cozy Italian restaurants with outdoor seating, not too expensive"

**Claude does:**
1. `search` — compiled query system translates NL to MQL with vector search
2. Returns ranked results with relevance explanation

**Human says:** "What about a quiet place for a date night, good wine list?"

**Claude does:**
1. `search` — another semantic query, showing it handles subjective/vibe-based queries

### Act 3: Build the App (~5 minutes)

**Human says:** "Build me a simple web app for searching these restaurants. A search bar, filter chips for cuisine and price range, and result cards showing the restaurant details."

**Claude does (via normal code writing):**
1. Creates `demo/app/index.html` — single-page app
2. Creates `demo/app/server.py` (or `server.js`) — thin backend that uses MongoCore client
3. Wires up: search bar calls MongoCore `search`, filters use `find` with query
4. Starts the dev server

**Human opens browser:** Working restaurant search app, powered by MongoCore.

### Act 4: (Optional) Live Update

**Human says:** "Add a few more restaurants to the dataset and re-ingest"

**Claude does:**
1. Appends rows to CSV
2. `ingest` with dedup (skip strategy) — only new rows inserted
3. App immediately shows new restaurants

## File Structure

```
demo/
├── DEMO_SCRIPT.md          # Scripted prompts and expected outcomes
├── SETUP.md                # One-time setup instructions
├── data/
│   └── restaurants.csv     # Curated demo dataset
├── app/                    # (Created during demo by Claude)
│   ├── index.html
│   └── server.py
└── mcp-config.json         # Example Claude Code MCP config
```

## Implementation Scope

### Must Build (v4)

| Component | Description |
|-----------|-------------|
| Stdio transport | `--stdio` flag, stdin/stdout JSON-RPC loop, stderr logging |
| CLI arg | Add `--stdio` to `CliArgs` |
| Main.rs branching | If `--stdio`, skip HTTP/gRPC, run stdio loop |
| Demo dataset | Generate `restaurants.csv` with ~800 rows |
| Demo script | `DEMO_SCRIPT.md` with prompts and expected outcomes |
| Setup automation | `just demo-setup` command |
| MCP config example | `demo/mcp-config.json` |

### Won't Build

- No changes to MCP handler logic (already complete)
- No new MCP tools
- No client library changes
- No changes to ingestion, analytics, or tenant systems
- No app code (Claude builds this live during the demo)

## Testing

### Stdio Transport Tests

1. **Unit test:** Feed JSON-RPC bytes to the stdio handler, verify correct responses on stdout
2. **Integration test:** Launch binary with `--stdio`, pipe requests, verify full round-trip (initialize → tools/list → tools/call)
3. **Error handling:** Malformed JSON, missing method, invalid params

### Demo Dry Run

Before presenting:
1. Run `just demo-setup`
2. Add MCP config to Claude Code
3. Execute the full demo script
4. Verify: ingestion works, queries return relevant results, app builds and runs

## Success Criteria

- [ ] `mongocore --stdio` launches, accepts JSON-RPC on stdin, responds on stdout
- [ ] Claude Code can auto-launch MongoCore as an MCP server via config
- [ ] Demo dataset ingests in under 10 seconds
- [ ] Semantic search queries return relevant, ranked results
- [ ] Complete demo (Acts 1-3) runs in under 10 minutes with no manual intervention beyond typing prompts
- [ ] Stakeholders see: zero-code database setup, NL queries, and a working app — all driven by an AI agent
