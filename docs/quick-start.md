# Quick Start Examples

## Connect from Your Language

### Python

```python
from mongocore import MongoClient

async with MongoClient("localhost:50051") as client:
    users = client["myapp"]["users"]
    await users.insert_one({"name": "Alice", "age": 30})
    docs = await users.find({"age": {"$gte": 25}})

    # Change streams with auto-close
    async with users.watch() as stream:
        async for event in stream:
            print(event["operation_type"], event["document"])
```

### TypeScript

```typescript
import { MongoClient } from '@mongocore/client';

const client = new MongoClient('localhost:50051');
await client.connect();
const users = client.db('myapp').collection('users');
await users.insertOne({ name: 'Alice', age: 30 });

// Change streams with auto-dispose
await using stream = users.watch();
for await (const event of stream) {
  console.log(event.operationType, event.document);
}
```

### Go

```go
client := mongocore.NewClient("localhost:50051")
client.Connect(ctx)
users := client.Database("myapp").Collection("users")
users.InsertOne(ctx, bson.D{{Key: "name", Value: "Alice"}, {Key: "age", Value: 30}})

// Change streams with io.Closer
cs, _ := users.Watch(ctx, nil)
defer cs.Close()
for {
    event, err := cs.Next()
    if err != nil { break }
    fmt.Println(event.OperationType, event.Document)
}
```

### Java

```java
try (MongoClient client = MongoClient.create("localhost:50051")) {
    MongoCollection users = client.getDatabase("myapp").getCollection("users");
    users.insertOne(new Document("name", "Alice").append("age", 30));

    // Change streams with AutoCloseable
    try (ChangeStream stream = users.watch()) {
        for (ChangeEvent event : stream) {
            System.out.println(event.getOperationType() + ": " + event.getDocument());
        }
    }
}
```

## Configuration

MongoCore uses layered configuration (CLI args > environment variables > TOML file > defaults):

```toml
# config.toml
connection_uri = "mongodb://localhost:27017"
grpc_port = 50051
mcp_port = 3000
log_level = "info"
compiled_cache_sync = true

# Optional: API keys for LLM and embeddings
# ANTHROPIC_API_KEY = "your-api-key-here"
# OPENAI_API_KEY = "your-api-key-here"
# VOYAGE_API_KEY = "your-api-key-here"
```

See [Getting Started](./getting-started.md) for full configuration reference.

## Testing

### Test Configuration

Copy the example config to create your local test configuration:

```bash
cp config.test.toml.example config.test.toml
```

Edit `config.test.toml` to enable AI features for testing:

```toml
# Uncomment and set to test compiled queries
ANTHROPIC_API_KEY = "your-api-key-here"

# Uncomment and set to test vector search
VOYAGE_API_KEY = "your-api-key-here"
```

> **Note:** `config.test.toml` is gitignored since it may contain API key references. Only the `.example` template is committed.

### Running Tests

```bash
# Unit tests (no dependencies)
cargo test --lib
# or: just test-unit

# Integration tests (needs Docker MongoDB)
just docker-up
cargo test --test integration
# or: just test-integration

# Client integration tests (needs Docker MongoDB + running sidecar)
cargo run -- --config config.test.toml &
just test-clients

# Everything
just test-all
```

### Docker

Build and run MongoCore as a container:

```bash
docker build -t mongocore .
docker run -p 50051:50051 -p 3000:3000 mongocore \
  --connection-uri "mongodb://host.docker.internal:27017"
```
