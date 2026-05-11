# Admin Operations

MongoCore exposes database administration operations for managing collections, indexes, and introspection.

## List Databases

### Python

```python
async with MongoClient() as client:
    databases = await client.list_databases()
    for db_name in databases:
        print(db_name)
```

### TypeScript

```typescript
const databases = await client.listDatabases();
databases.forEach(name => console.log(name));
```

### Go

```go
databases, err := client.ListDatabases(ctx)
for _, name := range databases {
    fmt.Println(name)
}
```

### Java

```java
List<String> databases = client.listDatabases();
databases.forEach(System.out::println);
```

## List Collections

### Python

```python
collections = await client["myapp"].list_collections()
```

### TypeScript

```typescript
const collections = await client.db('myapp').listCollections();
```

### Go

```go
collections, err := client.Database("myapp").ListCollections(ctx)
```

### Java

```java
List<String> collections = client.getDatabase("myapp").listCollections();
```

## Create Collection

### Python

```python
await client["myapp"].create_collection("events")
```

### TypeScript

```typescript
await client.db('myapp').createCollection('events');
```

### Go

```go
err := client.Database("myapp").CreateCollection(ctx, "events")
```

### Java

```java
client.getDatabase("myapp").createCollection("events");
```

## Create Index

### Python

```python
users = client["myapp"]["users"]

# Single field index
await users.create_index({"email": 1}, unique=True)

# Compound index
await users.create_index({"status": 1, "created_at": -1})

# Text index (for $text search fallback)
await users.create_index({"name": "text", "bio": "text"})
```

### TypeScript

```typescript
const users = client.db('myapp').collection('users');

// Unique index
await users.createIndex({ email: 1 }, { unique: true });

// Compound index
await users.createIndex({ status: 1, createdAt: -1 });
```

### Go

```go
users := client.Database("myapp").Collection("users")

indexName, err := users.CreateIndex(ctx, bson.D{
    {Key: "email", Value: 1},
}, &mongocore.IndexOptions{Unique: true})
```

### Java

```java
MongoCollection users = client.getDatabase("myapp").getCollection("users");

// Unique index
String indexName = users.createIndex(
    new Document("email", 1),
    new IndexOptions().unique(true)
);
```

## gRPC Protocol

```protobuf
rpc CreateCollection(CreateCollectionRequest) returns (CreateCollectionResponse);
rpc CreateIndex(CreateIndexRequest) returns (CreateIndexResponse);
rpc ListDatabases(ListDatabasesRequest) returns (ListDatabasesResponse);
rpc ListCollections(ListCollectionsRequest) returns (ListCollectionsResponse);
```

### Index Options

```protobuf
message IndexOptions {
  optional string name = 1;
  optional bool unique = 2;
  optional bool sparse = 3;
}
```
