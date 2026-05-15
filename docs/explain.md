# Operation Explain

MongoCore's MCP interface includes built-in operation explanation — every response tells you what happened, and two tools generate reusable client code from your session.

## Response Context

Every MCP tool response includes a `_context` field with the key parameters of the operation:

```json
{
  "insertedCount": 500,
  "_context": {
    "operation": "insert_many",
    "database": "analytics",
    "collection": "events",
    "document_count": 500,
    "document_schema": {"event_type": "string", "timestamp": "string", "payload": "object"}
  }
}
```

This makes responses self-contained — you can understand what happened without scrolling back to the request.

## explain_last

Generate MongoCore client code for the most recent operation (or Nth most recent).

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| language | string | Yes | `python`, `typescript`, `go`, or `java` |
| offset | integer | No | How many operations back (default: 0 = most recent) |

**Example:**

```json
{"tool": "explain_last", "arguments": {"language": "python"}}
```

**Response:**

```json
{
  "code": "async def insert_many_events(\n    client,\n    db_name: str = \"analytics\",\n    ...\n) -> dict:\n    ...",
  "language": "python",
  "operation": "insert_many"
}
```

## explain_session

Generate a complete script reproducing all operations from the current session.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| language | string | Yes | `python`, `typescript`, `go`, or `java` |

**Example:**

```json
{"tool": "explain_session", "arguments": {"language": "python"}}
```

**Response:**

```json
{
  "code": "from mongocore import MongoCore\n\nasync def ingest_contacts(...):\n    ...\n\nasync def find_contacts(...):\n    ...\n\nasync def main():\n    client = MongoCore()\n    await ingest_contacts(client)\n    await find_contacts(client)\n\nif __name__ == '__main__':\n    import asyncio\n    asyncio.run(main())\n",
  "language": "python",
  "operation_count": 2,
  "operations": ["ingest", "find"]
}
```

## Supported Languages

| Language | Import | Client Init |
|----------|--------|-------------|
| Python | `from mongocore import MongoCore` | `MongoCore()` |
| TypeScript | `import { MongoCore } from 'mongocore'` | `new MongoCore()` |
| Go | `"github.com/mongocore/mongocore-go/mongocore"` | `mongocore.NewClient()` |
| Java | `import com.mongocore.MongoClient` | `new MongoClient()` |

## Session Scope

- Operations are recorded per MCP connection (stdio or SSE session)
- History is in-memory only — cleared when the connection drops
- Diagnostic tools (`get_analytics`, `collection_schema`) are not recorded
- Failed operations appear as commented-out code with the error reason

## Notes

- Generated code produces parameterized functions with defaults matching the actual values used
- Functions are named `{operation}_{collection}` (e.g., `insert_many_users`)
- `embed_and_store` and `semantic_search` generate comments noting they are MCP-only (no client library support yet)
