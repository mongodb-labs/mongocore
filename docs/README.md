# MongoCore Documentation

MongoCore is an AI-native MongoDB driver implemented as a Rust sidecar. It provides high-performance database access over gRPC and an MCP interface for AI agents.

## Features

- **Polyglot gRPC API** — Python, TypeScript, Go, Java client libraries
- **MCP Server** — AI agents (Claude, GPT) interact with MongoDB via JSON-RPC
- **Compiled Queries** — Intelligent NL→MQL with method routing, LLM-provided templates, and 3-level caching
- **Vector Search** — Semantic search with Voyage AI embeddings and Atlas Vector Search
- **Full-Text Search** — Atlas Search with automatic fallback
- **Data Ingestion** — Polars-powered CSV/JSON/Parquet/URL/S3 ingestion with schema inference
- **Transactions** — Multi-document ACID with concurrent session management
- **Change Streams** — Real-time database notifications via server-streaming gRPC
- **Query Analytics** — Real-time latency percentiles, error rates, operation insights
- **Multi-Tenant** — Isolated caches, rate limiting, per-tenant connection pools
- **OpenTelemetry** — Optional distributed tracing with driver and MongoCore-level spans
- **Custom LLM Gateway** — Corporate AI gateway support with configurable auth

## Documentation

| Guide | Description |
|-------|-------------|
| [Getting Started](./getting-started.md) | Installation, configuration, running MongoCore |
| [CRUD Operations](./crud-operations.md) | Find, insert, update, delete with all 4 languages |
| [Aggregation](./aggregation.md) | Pipeline operations and common patterns |
| [Transactions](./transactions.md) | Multi-document ACID transactions |
| [Search](./search.md) | Vector search, full-text search, fallback chains |
| [Compiled Queries](./compiled-queries.md) | Intelligent NL→MQL with routing, templates, and caching |
| [Change Streams](./streaming.md) | Real-time notifications via Watch |
| [Admin Operations](./admin-operations.md) | Collections, indexes, introspection |
| [MCP Server](./mcp-server.md) | AI agent integration via JSON-RPC |
| [Client Libraries](./client-libraries.md) | Language-specific setup and API reference |
| [Raw Passthrough](./raw-passthrough.md) | Arbitrary MongoDB commands for power users |
| [Analytics](./analytics.md) | Query performance insights and operation tracking |
| [Multi-Tenant](./multi-tenant.md) | Shared sidecar with per-tenant isolation |
| [Ingestion](./ingestion.md) | Polars-powered data ingestion and ETL |
| [OpenTelemetry](./opentelemetry.md) | Distributed tracing setup and configuration |
| [Testing](./testing.md) | Test configuration, running tests, Docker setup |
| [Roadmap](./roadmap.md) | Version history and future roadmap |
| [Design & Plans](./design/) | Architecture specs, implementation plans, and development log |

## Quick Start

```bash
# Start MongoCore
mongocore --connection-uri "mongodb://localhost:27017"

# Python
pip install mongocore
```

```python
from mongocore import MongoClient

async with MongoClient() as client:
    users = client["myapp"]["users"]
    await users.insert_one({"name": "Alice", "age": 30})
    async for doc in users.find({"age": {"$gte": 25}}):
        print(doc["name"])
```

```bash
# TypeScript
npm install @mongocore/client
```

```typescript
import { MongoClient } from '@mongocore/client';

const client = new MongoClient();
await client.connect();
const users = client.db('myapp').collection('users');
await users.insertOne({ name: 'Alice', age: 30 });
for await (const doc of users.find({ age: { $gte: 25 } })) {
  console.log(doc.name);
}
```

```bash
# Go
go get github.com/rozza/mongocore/clients/go/mongocore
```

```go
client := mongocore.MongoClient()
client.Connect(ctx)
users := client.Database("myapp").Collection("users")
users.InsertOne(ctx, bson.D{{Key: "name", Value: "Alice"}, {Key: "age", Value: 30}})
cursor := users.Find(ctx, bson.D{{Key: "age", Value: bson.D{{Key: "$gte", Value: 25}}}}, nil)
defer cursor.Close()
for cursor.Next(ctx) {
    fmt.Println(cursor.Doc())
}
```

```bash
# Java (Maven)
```

```java
try (MongoClient client = MongoClient.create()) {
    MongoCollection users = client.getDatabase("myapp").getCollection("users");
    users.insertOne(new Document("name", "Alice").append("age", 30));
    try (MongoCursor cursor = users.find(new Document("age", new Document("$gte", 25)))) {
        for (Document doc : cursor) {
            System.out.println(doc.getString("name"));
        }
    }
}
```

## Architecture

```
┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│   Python    │  │ TypeScript  │  │     Go      │  │    Java     │
│   Client    │  │   Client    │  │   Client    │  │   Client    │
└──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘
       │                 │                 │                 │
       └────────────────┬┴─────────────────┴─────────────────┘
                        │ gRPC (localhost:50051)
                        ▼
              ┌─────────────────────┐
              │      MongoCore      │
              │    (Rust Sidecar)   │
              │                     │
              │  ┌───────────────┐  │        ┌──────────────┐
              │  │   gRPC API    │  │        │   AI Agents  │
              │  ├───────────────┤  │◀──────▶│  (Claude,    │
              │  │   MCP Server  │  │  :3000 │   GPT, etc.) │
              │  ├───────────────┤  │        └──────────────┘
              │  │  Search Engine│  │
              │  ├───────────────┤  │        ┌──────────────┐
              │  │Compiled Query │  │──────▶ │  Voyage AI   │
              │  ├───────────────┤  │        │  (Embeddings)│
              │  │  Voyage AI    │  │        └──────────────┘
              │  ├───────────────┤  │
              │  │  Transactions │  │        ┌──────────────┐
              │  └───────────────┘  │──────▶ │   LLM API    │
              └──────────┬──────────┘        │  (Anthropic) │
                         │                   └──────────────┘
                         │ MongoDB Wire Protocol
                         ▼
              ┌─────────────────────┐
              │      MongoDB        │
              │  (Atlas / Local)    │
              └─────────────────────┘
```
