# Client Libraries

MongoCore provides thin client libraries for Python, TypeScript, Go, and Java. Each library wraps the gRPC interface with idiomatic APIs for its language.

## Installation

### Python

```bash
pip install mongocore
```

Requires: Python 3.10+, `grpcio`, `pymongo` (for BSON)

### TypeScript / Node.js

```bash
npm install @mongocore/client
```

Requires: Node.js 18+, `@grpc/grpc-js`, `bson`

### Go

```bash
go get github.com/rozza/mongocore/clients/go/mongocore
```

Requires: Go 1.21+, `google.golang.org/grpc`, `go.mongodb.org/mongo-driver/v2/bson`

### Java

```xml
<dependency>
    <groupId>com.mongocore</groupId>
    <artifactId>mongocore-client</artifactId>
    <version>0.1.0</version>
</dependency>
```

Requires: Java 11+, gRPC-Java, `org.mongodb:bson`

## Connection

All clients connect to the MongoCore sidecar over gRPC (default `localhost:50051`).

### Python

```python
from mongocore import MongoClient

# Explicit connection management
client = MongoClient("localhost:50051")
await client.connect()
# ... use client ...
await client.close()

# Context manager (recommended)
async with MongoClient("localhost:50051") as client:
    db = client["mydb"]
    users = db["users"]

# Auto-spawn the sidecar binary
async with MongoClient(auto_spawn=True) as client:
    ...
```

### TypeScript

```typescript
import { MongoClient } from '@mongocore/client';

const client = new MongoClient('localhost:50051');
await client.connect();

const db = client.db('mydb');
const users = db.collection('users');

// ... use collection ...
await client.close();

// Auto-spawn the sidecar
const client = new MongoClient('localhost:50051', { autoSpawn: true });
await client.connect();
```

### Go

```go
import "github.com/rozza/mongocore/clients/go/mongocore"

client := mongocore.MongoClientTCP("localhost:50051")
if err := client.Connect(ctx); err != nil {
    log.Fatal(err)
}
defer client.Close()

users := client.Database("mydb").Collection("users")
```

### Java

```java
import com.mongocore.MongoClient;

// AutoCloseable — use try-with-resources
try (MongoClient client = MongoClient.create("localhost:50051")) {
    MongoDatabase db = client.getDatabase("mydb");
    MongoCollection users = db.getCollection("users");
    // ...
}

// Default address
MongoClient client = MongoClient.create(); // localhost:50051
```

## API Comparison

| Operation | Python | TypeScript | Go | Java |
|-----------|--------|------------|-----|------|
| Get database | `client["db"]` | `client.db("db")` | `client.Database("db")` | `client.getDatabase("db")` |
| Get collection | `db["coll"]` | `db.collection("coll")` | `db.Collection("coll")` | `db.getCollection("coll")` |
| Find | `await coll.find(filter)` | `await coll.find(filter)` | `coll.Find(ctx, filter)` | `coll.find(filter)` |
| Find one | `await coll.find_one(filter)` | `await coll.findOne(filter)` | `coll.FindOne(ctx, filter)` | `coll.findOne(filter)` |
| Insert one | `await coll.insert_one(doc)` | `await coll.insertOne(doc)` | `coll.InsertOne(ctx, doc)` | `coll.insertOne(doc)` |
| Insert many | `await coll.insert_many(docs)` | `await coll.insertMany(docs)` | `coll.InsertMany(ctx, docs)` | `coll.insertMany(docs)` |
| Update one | `await coll.update_one(f, u)` | `await coll.updateOne(f, u)` | `coll.UpdateOne(ctx, f, u)` | `coll.updateOne(f, u)` |
| Delete one | `await coll.delete_one(filter)` | `await coll.deleteOne(filter)` | `coll.DeleteOne(ctx, filter)` | `coll.deleteOne(filter)` |
| Delete many | `await coll.delete_many(filter)` | `await coll.deleteMany(filter)` | `coll.DeleteMany(ctx, filter)` | `coll.deleteMany(filter)` |
| Aggregate | `await coll.aggregate(pipeline)` | `await coll.aggregate(pipeline)` | `coll.Aggregate(ctx, pipeline)` | `coll.aggregate(pipeline)` |
| Search | `await coll.search(q, limit=N)` | `await coll.search(q, N)` | `coll.Search(ctx, q, N)` | `coll.search(q, N)` |
| Watch | `async with coll.watch()` | `coll.watch()` | `coll.Watch(ctx, pipeline)` | `coll.watch()` |

## Change Streams (Watch)

All clients support real-time change streams with auto-close semantics. See the [Change Streams](./streaming.md) guide for full details.

| Language | Auto-Close Pattern | Type |
|----------|-------------------|------|
| Python | `async with coll.watch() as stream` | `ChangeStream` (async context manager) |
| TypeScript | `await using stream = coll.watch()` | `ChangeStream` (AsyncDisposable) |
| Go | `defer cs.Close()` | `*ChangeStream` (io.Closer) |
| Java | `try (ChangeStream s = coll.watch())` | `ChangeStream` (AutoCloseable + Iterable) |

## BSON Encoding

All clients handle BSON serialization internally. Documents are encoded to raw BSON bytes before being sent over gRPC, avoiding any JSON intermediate format.

| Language | BSON Library | Document Type |
|----------|--------------|---------------|
| Python | `bson` (pymongo) | `dict` |
| TypeScript | `bson` (npm) | `object` |
| Go | `go.mongodb.org/mongo-driver/v2/bson` | `bson.D` |
| Java | `org.mongodb:bson` | `org.bson.Document` |

## Sidecar Management

Each client library includes a `SidecarManager` that can automatically start and manage the MongoCore binary:

```python
# Python — auto-spawn
client = MongoClient(auto_spawn=True)
await client.connect()  # Starts mongocore binary if not running
```

```typescript
// TypeScript — auto-spawn
const client = new MongoClient('localhost:50051', { autoSpawn: true });
await client.connect();
```

The sidecar manager:
- Checks if MongoCore is already running on the target port
- Starts the binary if needed
- Monitors the process health
- Stops the process when the client disconnects

## Generating gRPC Stubs

The client libraries need generated gRPC stubs from the proto definitions. Each language has its own code generation step:

### Python

```bash
cd clients/python
pip install grpcio-tools
python -m grpc_tools.protoc \
  -I../../proto \
  --python_out=src/mongocore/generated \
  --grpc_python_out=src/mongocore/generated \
  --pyi_out=src/mongocore/generated \
  ../../proto/mongocore/v1/*.proto
```

### TypeScript

```bash
cd clients/typescript
bash generate_stubs.sh
```

### Go

```bash
cd clients/go
bash generate_stubs.sh
```

### Java

```bash
cd clients/java
bash generate_stubs.sh
```

## Project Structure

```
clients/
├── python/
│   ├── src/mongocore/
│   │   ├── client.py          # MongoClient
│   │   ├── database.py        # Database handle
│   │   ├── collection.py      # Collection with CRUD + ChangeStream (async with)
│   │   ├── ops.py             # Pipeline operation dataclasses
│   │   ├── result.py          # PipelineResult wrapper
│   │   ├── sidecar.py         # SidecarManager
│   │   └── generated/         # gRPC stubs (generated)
│   └── tests/
├── typescript/
│   └── src/
│       ├── client.ts          # MongoClient
│       ├── database.ts        # Database
│       ├── collection.ts      # Collection with CRUD + ChangeStream (AsyncDisposable)
│       ├── ops.ts             # Pipeline operation types
│       ├── sidecar.ts         # SidecarManager
│       └── types.ts           # TypeScript interfaces
├── go/
│   └── mongocore/
│       ├── client.go          # Client
│       ├── database.go        # Database
│       ├── collection.go      # Collection + ChangeStream (io.Closer)
│       ├── transaction_pipeline.go # Transaction pipeline support
│       ├── sidecar.go         # SidecarManager
│       └── ops/               # Pipeline operation helpers
└── java/
    └── src/main/java/com/mongocore/
        ├── MongoClient.java       # Client (AutoCloseable)
        ├── MongoDatabase.java     # Database handle
        ├── MongoCollection.java   # Collection with CRUD
        ├── ChangeStream.java      # AutoCloseable change stream
        ├── ChangeEvent.java       # Change event POJO
        ├── FindOptions.java       # Builder-pattern options
        ├── SidecarManager.java    # Binary lifecycle
        └── *Result.java           # Result types
```
