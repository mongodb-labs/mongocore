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
from mongocore import MongoCoreClient

# Explicit connection management
client = MongoCoreClient("localhost:50051")
await client.connect()
# ... use client ...
await client.close()

# Context manager (recommended)
async with MongoCoreClient("localhost:50051") as client:
    db = client["mydb"]
    users = db["users"]

# Auto-spawn the sidecar binary
async with MongoCoreClient(auto_spawn=True) as client:
    ...
```

### TypeScript

```typescript
import { MongoCoreClient } from '@mongocore/client';

const client = new MongoCoreClient('localhost:50051');
await client.connect();

const db = client.db('mydb');
const users = db.collection('users');

// ... use collection ...
await client.close();

// Auto-spawn the sidecar
const client = new MongoCoreClient('localhost:50051', { autoSpawn: true });
await client.connect();
```

### Go

```go
import "github.com/rozza/mongocore/clients/go/mongocore"

client := mongocore.NewClient("localhost:50051")
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
| Aggregate | `await coll.aggregate(pipeline)` | `await coll.aggregate(pipeline)` | `coll.Aggregate(ctx, pipeline)` | `coll.aggregate(pipeline)` |

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
client = MongoCoreClient(auto_spawn=True)
await client.connect()  # Starts mongocore binary if not running
```

```typescript
// TypeScript — auto-spawn
const client = new MongoCoreClient('localhost:50051', { autoSpawn: true });
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
  ../../proto/mongocore/v1/*.proto
```

### TypeScript

```bash
cd clients/typescript
npm run generate
# Uses grpc_tools_node_protoc under the hood
```

### Go

```bash
cd clients/go
protoc --go_out=. --go-grpc_out=. \
  -I../../proto \
  ../../proto/mongocore/v1/*.proto
```

### Java

```bash
cd clients/java
mvn generate-sources
# Uses protobuf-maven-plugin
```

## Project Structure

```
clients/
├── python/
│   ├── src/mongocore/
│   │   ├── client.py          # MongoCoreClient
│   │   ├── database.py        # Database handle
│   │   ├── collection.py      # Collection with CRUD
│   │   ├── sidecar.py         # SidecarManager
│   │   └── generated/         # gRPC stubs (generated)
│   └── tests/
├── typescript/
│   └── src/
│       ├── client.ts          # MongoCoreClient
│       ├── database.ts        # Database
│       ├── collection.ts      # Collection with CRUD
│       ├── sidecar.ts         # SidecarManager
│       └── types.ts           # TypeScript interfaces
├── go/
│   └── mongocore/
│       ├── client.go          # Client
│       ├── database.go        # Database
│       ├── collection.go      # Collection
│       └── sidecar.go         # SidecarManager
└── java/
    └── src/main/java/com/mongocore/
        ├── MongoClient.java       # Client (AutoCloseable)
        ├── MongoDatabase.java     # Database handle
        ├── MongoCollection.java   # Collection with CRUD
        ├── FindOptions.java       # Builder-pattern options
        ├── SidecarManager.java    # Binary lifecycle
        └── *Result.java           # Result types
```
