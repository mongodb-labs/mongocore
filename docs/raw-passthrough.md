# Raw Wire Protocol Passthrough

The raw passthrough feature provides an escape hatch for power users to execute arbitrary MongoDB commands directly against the database. This is useful when MongoCore's high-level APIs don't cover a specific operation, or when you need fine-grained control over command execution.

## Overview

MongoCore's `RunCommand` RPC allows you to send any BSON document as a MongoDB command. This bypasses MongoCore's opinionated defaults and executes the command exactly as provided, giving you the same flexibility as the MongoDB shell or native drivers.

**Use this feature when:**
- You need to execute administrative commands not exposed via MongoCore's standard API
- You're migrating from another driver and need to preserve legacy command syntax
- You need access to MongoDB features before MongoCore adds native support
- You're prototyping or debugging and need raw access

**Use MongoCore's standard API when:**
- You're performing CRUD operations, aggregations, or transactions
- You want safety guarantees (majority write/read concern, retries)
- You need cross-language compatibility with idiomatic client APIs
- You prefer structured request/response types over raw BSON

## Safety & Validation

By default, MongoCore blocks dangerous administrative commands that could cause data loss or cluster disruption:

### Blocked Commands (default mode)

- `dropDatabase` - Deletes an entire database
- `dropAllUsersFromDatabase` - Removes all database users
- `dropAllRolesFromDatabase` - Removes all database roles
- `shutdown` - Shuts down the MongoDB server
- `replSetReconfig` - Reconfigures replica set topology
- `replSetStepDown` - Forces primary to step down
- `setFeatureCompatibilityVersion` - Changes server compatibility version
- `fsync` - Locks the database for filesystem sync
- `cleanupOrphaned` - Maintenance command for sharded clusters
- `compact` - Rewrites collection data and indexes

### Allowing All Commands

To bypass validation and allow dangerous commands, set `allow_all: true` in the request:

```python
# Python example - USE WITH CAUTION
result = await client.run_command(
    "admin",
    {"shutdown": 1},
    allow_all=True
)
```

**Warning:** `allow_all=True` removes all safety guardrails. Only use this in development environments or when you fully understand the consequences.

## Usage Examples

### Python

```python
from mongocore import MongoClient
import bson

async with MongoClient("localhost:50051") as client:
    # Execute a safe command (no validation needed)
    result = await client.run_command(
        "myapp",
        {"ping": 1}
    )
    print(result)  # {"ok": 1.0}

    # Get server statistics
    stats = await client.run_command(
        "admin",
        {"serverStatus": 1}
    )
    print(f"Uptime: {stats['uptime']} seconds")

    # Create a capped collection with specific options
    result = await client.run_command(
        "myapp",
        {
            "create": "logs",
            "capped": True,
            "size": 1048576,  # 1MB
            "max": 10000
        }
    )

    # Raw find command (prefer client.db.collection.find() instead)
    result = await client.run_command(
        "myapp",
        {
            "find": "users",
            "filter": {"age": {"$gte": 25}},
            "limit": 10
        }
    )
```

### TypeScript

```typescript
import { MongoClient } from '@mongocore/client';

const client = new MongoClient('localhost:50051');
await client.connect();

// Ping the database
const pingResult = await client.runCommand('myapp', { ping: 1 });
console.log(pingResult); // { ok: 1 }

// Get current operations
const currentOp = await client.runCommand('admin', {
  currentOp: 1,
  $all: true
});
console.log(`Active ops: ${currentOp.inprog.length}`);

// Create a TTL index using raw command
await client.runCommand('myapp', {
  createIndexes: 'sessions',
  indexes: [
    {
      key: { expiresAt: 1 },
      name: 'ttl_expiry',
      expireAfterSeconds: 3600
    }
  ]
});

// Validate a collection
const validation = await client.runCommand('myapp', {
  validate: 'users',
  full: true
});
console.log(`Valid: ${validation.valid}`);
```

### Go

```go
import (
    "context"
    "fmt"
    "go.mongodb.org/mongo-driver/bson"
    "github.com/rozza/mongocore/clients/go/mongocore"
)

client := mongocore.NewClient("localhost:50051")
client.Connect(ctx)
defer client.Close()

// Ping command
result, err := client.RunCommand(ctx, "myapp", bson.D{{Key: "ping", Value: 1}}, false)
if err != nil {
    panic(err)
}
fmt.Println(result)

// Get database statistics
stats, err := client.RunCommand(ctx, "myapp", bson.D{{Key: "dbStats", Value: 1}}, false)
if err != nil {
    panic(err)
}
fmt.Printf("Stats: %v\n", stats)

// Create a unique index
indexCmd := bson.D{
    {Key: "createIndexes", Value: "users"},
    {Key: "indexes", Value: bson.A{
        bson.D{
            {Key: "key", Value: bson.D{{Key: "email", Value: 1}}},
            {Key: "name", Value: "unique_email"},
            {Key: "unique", Value: true},
        },
    }},
}
_, err = client.RunCommand(ctx, "myapp", indexCmd, false)
if err != nil {
    panic(err)
}
```

### Java

```java
import com.mongocore.MongoClient;
import org.bson.Document;

try (MongoClient client = MongoClient.create("localhost:50051")) {
    // Ping command
    Document pingCmd = new Document("ping", 1);
    Document result = client.runCommand("myapp", pingCmd, false);
    System.out.println(result.toJson());

    // Get build information
    Document buildInfo = client.runCommand("admin", 
        new Document("buildInfo", 1), false);
    System.out.println("MongoDB version: " + buildInfo.getString("version"));

    // Create a text index
    Document indexCmd = new Document("createIndexes", "articles")
        .append("indexes", Arrays.asList(
            new Document("key", new Document("content", "text"))
                .append("name", "text_content")
        ));
    client.runCommand("myapp", indexCmd, false);

    // Get collection stats
    Document collStats = client.runCommand("myapp",
        new Document("collStats", "users"), false);
    System.out.println("Document count: " + collStats.getInteger("count"));
}
```

## gRPC API

The `RunCommand` RPC is defined in the MongoCore protobuf schema:

```protobuf
rpc RunCommand(RunCommandRequest) returns (RunCommandResponse);

message RunCommandRequest {
  string database = 1;
  Document command = 2;
  bool allow_all = 3;
}

message RunCommandResponse {
  Document result = 1;
}
```

**Fields:**
- `database` - The target database name (use "admin" for administrative commands)
- `command` - The BSON command document to execute
- `allow_all` - Set to `true` to bypass validation (default: `false`)

## MCP Server

The MCP server does not expose raw command execution for security reasons. AI agents should use the high-level tools (`find`, `insert`, `update`, etc.) instead.

## Error Handling

### Validation Errors

If a dangerous command is blocked, you'll receive a validation error:

```python
try:
    await client.run_command("admin", {"dropDatabase": 1})
except ValidationError as e:
    print(e)  # "Command 'dropDatabase' is blocked by validation policy"
```

### Execution Errors

If the command fails on MongoDB's side, the error is returned in the response:

```python
result = await client.run_command("myapp", {"invalidCommand": 1})
# result = {"ok": 0, "errmsg": "no such command: 'invalidCommand'", "code": 59}
```

## Best Practices

1. **Prefer high-level APIs** - Use MongoCore's structured operations (find, insert, aggregate) when possible
2. **Test with allow_all=false first** - Ensure your command isn't on the blocklist
3. **Validate responses** - Check the `ok` field in the result document
4. **Use read-only commands in production** - Limit write operations to development/testing
5. **Document your usage** - Add comments explaining why raw commands are necessary

## When NOT to Use Raw Passthrough

Avoid raw commands for operations covered by MongoCore's standard API:

| Instead of... | Use... |
|---------------|--------|
| `{find: "users", filter: {...}}` | `collection.find(filter)` |
| `{insert: "users", documents: [...]}` | `collection.insert_one()` / `insert_many()` |
| `{aggregate: "users", pipeline: [...]}` | `collection.aggregate(pipeline)` |
| `{update: "users", updates: [...]}` | `collection.update_one()` / `update_many()` |
| `{delete: "users", deletes: [...]}` | `collection.delete_one()` / `delete_many()` |

The high-level API provides:
- Type safety and validation
- Automatic retries on transient failures
- Majority write/read concerns by default
- Cross-language consistency
- Better error messages

## Related Documentation

- [CRUD Operations](./crud-operations.md) - High-level find, insert, update, delete
- [Aggregation](./aggregation.md) - Pipeline operations
- [Admin Operations](./admin-operations.md) - Collections, indexes, introspection
