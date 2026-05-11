# Change Streams

MongoCore supports MongoDB change streams via a server-streaming gRPC `Watch` RPC. You can watch an entire database or a specific collection for real-time notifications of inserts, updates, deletes, and replacements.

## How It Works

The `Watch` RPC returns a stream of `WatchEvent` messages. Each event includes the operation type, the affected database/collection, and the document data.

## Python

```python
from mongocore import MongoCoreClient

async with MongoCoreClient() as client:
    # Watch a specific collection
    async for event in client.watch("myapp", collection="orders"):
        print(f"Operation: {event.operation_type}")
        print(f"Collection: {event.collection}")
        if event.document:
            print(f"Document: {event.document}")

    # Watch entire database
    async for event in client.watch("myapp"):
        print(f"{event.operation_type} on {event.collection}")

    # Watch with a filter pipeline
    async for event in client.watch("myapp", collection="orders", pipeline=[
        {"$match": {"fullDocument.status": "shipped"}}
    ]):
        print(f"Order shipped: {event.document}")
```

## TypeScript

```typescript
const stream = client.watch('myapp', {
  collection: 'orders',
  pipeline: [
    { $match: { 'fullDocument.status': 'shipped' } }
  ],
});

for await (const event of stream) {
  console.log(`${event.operationType} on ${event.collection}`);
  if (event.document) {
    console.log(event.document);
  }
}
```

## Go

```go
stream, err := client.Watch(ctx, "myapp", &mongocore.WatchOptions{
    Collection: "orders",
    Pipeline: []bson.D{
        {{Key: "$match", Value: bson.D{
            {Key: "fullDocument.status", Value: "shipped"},
        }}},
    },
})
if err != nil {
    log.Fatal(err)
}

for event := range stream {
    fmt.Printf("%s on %s\n", event.OperationType, event.Collection)
}
```

## Java

```java
MongoClient client = MongoClient.create();

client.watch("myapp", "orders", event -> {
    System.out.printf("%s on %s%n",
        event.getOperationType(),
        event.getCollection());
    if (event.getDocument() != null) {
        System.out.println(event.getDocument().toJson());
    }
});
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
