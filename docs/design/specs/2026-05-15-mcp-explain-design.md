# MCP Operation Explain

> **For implementers:** Read and follow `AGENTS.md` at the project root.
> Before committing: `cargo build` must produce ZERO warnings AND `cargo test --lib` must pass.
> If modifying client libraries: verify imports work and run `just test-clients`.
> If modifying shared structs (like `Config`): update ALL struct literals in `src/` AND `tests/`.

## Problem

When users interact with MongoDB through MongoCore's MCP interface (via an AI agent), it can be difficult to understand what operations were actually performed. For example, a user asks an agent to "ingest data from this URL, transform it, and load it into MongoDB" — the agent orchestrates multiple MCP tool calls, but the user has no clear view of what happened underneath or how to reproduce it programmatically.

Additionally, users often want to take an interactive MCP session and turn it into reusable application code — "I just did all that interactively, now show me the Python code to do it again."

## Solution

Three coordinated features, all scoped to the MCP layer only (not gRPC/language clients):

1. **Response Enrichment** — Every MCP tool response includes a `_context` object echoing key input parameters, making responses self-contained and explainable.
2. **Session Recorder** — In-memory operation history per MCP connection.
3. **Explain Tools** — `explain_last` and `explain_session` generate parameterized MongoCore client code from recorded operations.

## Design

### 1. Response Enrichment

Every MCP tool response gains a `_context` field containing the key input parameters relevant to that operation. This allows any consumer (agent or human) to understand what happened from the response alone, without needing the original request context.

**Format:**

```json
{
  "matchedCount": 5,
  "modifiedCount": 3,
  "_context": {
    "operation": "update_many",
    "database": "mydb",
    "collection": "users",
    "filter": {"status": "inactive"},
    "update": {"$set": {"archived": true}}
  }
}
```

**Per-tool `_context` fields:**

| Tool | Context Fields |
|------|---------------|
| find, find_one | operation, database, collection, filter, projection, sort, limit, skip |
| insert, insert_many | operation, database, collection, document_count, document_schema |
| update, update_many | operation, database, collection, filter, update |
| delete, delete_many | operation, database, collection, filter |
| aggregate | operation, database, collection, pipeline |
| count_documents | operation, database, collection, filter |
| ask | operation, database, collection, question, compiled_query (filter or pipeline) |
| ingest | operation, database, collection, file_path, format, transforms, dedup_key, conflict_strategy |
| create_index | operation, database, collection, keys, options |
| drop_collection | operation, database, collection |
| create_collection | operation, database, collection |
| list_collections | operation, database |
| list_databases | operation |
| run_command | operation, database, command_name (top-level key only) |
| embed_and_store | operation, database, collection, embed_field, document_count |
| semantic_search | operation, database, collection, query, index_name, limit |
| watch_directory | operation, database, collection, path, pattern, conflict_strategy |
| pipeline | operation, steps: [{name, tool_name}] |
| transaction_pipeline | operation, steps: [{name, tool_name}] |

**Rules:**
- `_context` is present on both success and error responses (knowing what was attempted is especially useful for debugging failures)
- Fields are only included when relevant to the tool (no null padding)
- Large values are summarized/truncated:
  - Pipelines: include up to 5 stages in full; if more, show first 3 + `"... (N more stages)"` + last stage
  - Filters/updates: include in full up to 1KB serialized; beyond that, show top-level keys only
  - Documents (for insert): show field names and types, not values (e.g., `{"name": "string", "age": "int", "tags": "array"}`)
- For `insert` / `insert_many`: `_context` includes `document_count` and a `document_schema` field showing the keys and types of the first document (sufficient for codegen to produce typed parameters)
- The `_context` is for explanation, not a complete audit log

### 2. Session Recorder

An in-memory recorder that captures each MCP tool invocation within a session.

**Location:** `src/mcp/session.rs` (new module)

**Data structure:**

```rust
pub struct OperationRecord {
    pub index: usize,
    pub tool_name: String,
    pub params: serde_json::Value,
    pub context: serde_json::Value,
    pub success: bool,
    pub error_message: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

pub struct SessionRecorder {
    operations: Vec<OperationRecord>,
}
```

**Behavior:**
- One `SessionRecorder` per MCP connection (stdio or SSE session)
- Appended to after every tool call (both success and error — errors record the attempt for debugging context)
- The `explain_last` and `explain_session` tools are NOT recorded (they are meta-operations, not user operations — avoids self-referential loops)
- Cleared when the MCP connection drops
- No persistence — purely in-memory
- No size limit initially (sessions are bounded by connection lifetime)
- Thread safety: wrapped in `Arc<Mutex<SessionRecorder>>` since multiple async tool calls may execute concurrently within a session

**What's recorded:**
- `tool_name` — which MCP tool was called
- `params` — the full request parameters (needed for code generation — actual document values, filters, etc.)
- `context` — the `_context` object from the response (summarized view for descriptions)
- `success` — whether the operation succeeded
- `error_message` — error details if failed (None on success)
- `timestamp` — when the operation executed

**Excluded from recording (meta/diagnostic tools):**
- `explain_last`, `explain_session` — meta-operations (avoids self-referential loops)
- `get_analytics` — diagnostic, not a workflow step
- `collection_schema` — informational, MCP-only diagnostic

**Included despite being read-only:**
- `list_collections`, `list_databases` — users may include these in workflows (e.g., iterate over all collections)

### 3. Explain Tools

Two new MCP tools that generate parameterized MongoCore client code from the session history.

#### `explain_last`

**Parameters:**
- `language` (required): `"python"` | `"typescript"` | `"go"` | `"java"`
- `offset` (optional, default 0): How many operations back (0 = most recent)

**Returns:**
```json
{
  "code": "def update_many_users(client, db_name: str = 'mydb', ...):\n    ...",
  "language": "python",
  "operation": "update_many"
}
```

**Behavior:**
- Looks up the operation at `operations[len - 1 - offset]`
- Generates a parameterized function using the MongoCore client for that language
- Function parameters have defaults matching the actual values used
- No docstring — the agent has already narrated the operation to the user
- Returns error if offset is out of bounds or session is empty

#### `explain_session`

**Parameters:**
- `language` (required): `"python"` | `"typescript"` | `"go"` | `"java"`

**Returns:**
```json
{
  "code": "# MongoCore session script\n\nfrom mongocore import MongoCore\n\ndef ingest_contacts(...):\n    ...\n\ndef update_status(...):\n    ...\n\ndef main():\n    client = MongoCore()\n    ingest_contacts(client)\n    update_status(client)\n\nif __name__ == '__main__':\n    main()",
  "language": "python",
  "operation_count": 5,
  "operations": ["ingest", "create_index", "update_many", "aggregate", "find"]
}
```

**Behavior:**
- Iterates all operations in session order
- Generates one parameterized function per operation
- Creates a `main` function (or equivalent) that calls them in sequence
- Client initialization at the top
- Each function is self-contained (could be extracted and used independently)
- Each function gets a template-based docstring derived from `_context` for readability (e.g., `"""insert_many on users in mydb"""`)
- Appropriate language idioms (async/await for Python/TS, error handling style per language)
- **Failed operations** are included as commented-out code with the error reason (e.g., `# FAILED: duplicate key error on index "email_1"\n# def insert_users(...): ...`)
- `run_command` operations generate a literal `client.run_command(...)` pass-through with the command dict as a parameter
- **Empty session**: returns an error indicating no operations have been recorded yet

### 4. Extended Codegen

The existing `src/mcp/codegen/` module currently supports:
- `query_gen.rs` — find/aggregate code
- `model_gen.rs` — typed data models
- `index_gen.rs` — index creation code

**New additions needed:**

| Module | Generates |
|--------|-----------|
| `crud_gen.rs` | insert, insert_many, update, update_many, delete, delete_many |
| `ingest_gen.rs` | ingest operations (file loading, transforms, options) |
| `search_gen.rs` | embed_and_store, semantic_search |
| `session_gen.rs` | full session stitching (imports, client init, main flow) |

**Code generation principles:**
- Parameterized functions with typed arguments and sensible defaults
- MongoCore client library as the target (not raw drivers)
- Language-idiomatic patterns (async for Python/TS, builders for Java, etc.)
- No comments unless a parameter name is ambiguous
- Imports at the top, one function per operation
- Function naming: `{operation}_{collection}` (e.g., `insert_many_users`, `aggregate_orders`). If the same operation+collection appears multiple times in a session, append a numeric suffix (`update_users_2`). For `ask` operations, use the intent to derive a descriptive name (e.g., `find_active_premium_users`).

### 5. Documentation

**New file:** `docs/explain.md`

Contents:
- Feature overview (what explain does, when to use it)
- `explain_last` usage with examples in each language
- `explain_session` usage with a multi-step example
- Supported languages
- Note that `_context` fields are available on all responses for agent-driven explanation

**Tool descriptions** (in tool definitions):
- `explain_last`: "Generate reusable MongoCore client code for a recent operation. Produces a parameterized function in the specified language."
- `explain_session`: "Generate a complete MongoCore client script reproducing all operations performed in this session. Produces parameterized functions with a main entry point."

## Scope

- **MCP only** — gRPC responses and language client libraries are not modified
- **MongoCore client codegen** — generated code targets the MongoCore client libraries, not native MongoDB drivers
- **Four languages** — Python, TypeScript, Go, Java (matching existing codegen support)
- **No persistence** — session history is in-memory only, lost on disconnect
- **No LLM in core** — the operations layer (`src/operations/`) remains LLM-free. Code generation and descriptions are template-based

## Non-Goals

- Persistent operation audit log (can be added later with config flag)
- Native driver code generation (only MongoCore client)

- Streaming/partial explain during long-running operations
- Cross-session replay

## Testing

- Unit tests for each codegen module (verify generated code structure per language)
- Unit tests for session recorder (append, retrieve by offset, clear)
- Integration tests for `explain_last` and `explain_session` (run operations, then explain, verify output)
- Verify `_context` is present and correct on all tool responses
