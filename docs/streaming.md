# Change Streams

MongoCore supports MongoDB change streams via a server-streaming gRPC `Watch` RPC. Watch a collection for real-time notifications of inserts, updates, deletes, and replacements with auto-close semantics in every language.

## How It Works

The `Watch` RPC returns a stream of `WatchEvent` messages. Each event includes the operation type, the affected database/collection, and the document data. All client libraries provide auto-close patterns to ensure streams are properly terminated.

## Python

Uses `async with` for automatic stream cleanup:

```python
from mongocore import MongoClient

async with MongoClient("localhost:50051") as client:
    users = client["myapp"]["users"]

    # Watch with auto-close (recommended)
    async with users.watch() as stream:
        async for event in stream:
            print(f"Operation: {event.operation_type}")
            print(f"Collection: {event.collection}")
            if event.document:
                print(f"Document: {event.document}")
```

## TypeScript

Uses `AsyncDisposable` (`await using`) for automatic cleanup, or manual `close()`:

```typescript
import { MongoClient } from '@mongocore/client';

const client = new MongoClient('localhost:50051');
await client.connect();
const users = client.db('myapp').collection('users');

// With AsyncDisposable (recommended)
await using stream = users.watch();
for await (const event of stream) {
  console.log(event.operationType, event.document);
}

// Or manual close
const stream = users.watch();
try {
  for await (const event of stream) {
    console.log(event.operationType, event.document);
  }
} finally {
  stream.close();
}
```

## Go

Implements `io.Closer` for use with `defer`:

```go
client := mongocore.NewClient("localhost:50051")
client.Connect(ctx)
users := client.Database("myapp").Collection("users")

cs, err := users.Watch(ctx, nil)
if err != nil {
    log.Fatal(err)
}
defer cs.Close()

for {
    event, err := cs.Next()
    if err != nil {
        break
    }
    fmt.Printf("%s: %v\n", event.OperationType, event.Document)
}
```

## Java

Implements `AutoCloseable` for use with try-with-resources:

```java
try (MongoClient client = MongoClient.create("localhost:50051")) {
    MongoCollection users = client.getDatabase("myapp").getCollection("users");

    try (ChangeStream stream = users.watch()) {
        for (ChangeEvent event : stream) {
            System.out.println(event.getOperationType() + ": " + event.getDocument());
        }
    }
}
```

## Event Types

| Operation | Description |
|-----------|-------------|
| `INSERT` | New document created |
| `UPDATE` | Existing document modified |
| `DELETE` | Document removed |
| `REPLACE` | Document replaced entirely |
| `INVALIDATE` | Stream invalidated (collection dropped, etc.) |

## Event Structure

```protobuf
message WatchEvent {
  OperationType operation_type = 1;
  string database = 2;
  string collection = 3;
  optional Document document = 4;            // Full document (insert/update/replace)
  optional Document update_description = 5;  // {updatedFields, removedFields}
  optional Document document_key = 6;        // The _id of the affected document
}
```

For `UPDATE` events, `update_description` contains the specific fields that changed, while `document` contains the full post-update document (if full document lookup is enabled).

## Notes

- Change streams require a MongoDB replica set or sharded cluster
- The stream automatically resumes from the last received event if the connection drops
- Use the pipeline parameter to filter events server-side for efficiency
- An `INVALIDATE` event terminates the stream (e.g., when the watched collection is dropped)
