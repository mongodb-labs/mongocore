# CRUD Operations

MongoCore exposes the full MongoDB CRUD surface over gRPC. All operations use raw BSON bytes on the wire to avoid double-serialization — your client library handles BSON encoding/decoding natively.

## Operations

| RPC | Description |
|-----|-------------|
| `Find` | Find documents matching a filter |
| `FindOne` | Find a single document |
| `Insert` | Insert one document |
| `InsertMany` | Insert multiple documents |
| `Update` | Update the first matching document |
| `UpdateMany` | Update all matching documents |
| `Delete` | Delete the first matching document |
| `DeleteMany` | Delete all matching documents |
| `FindAndModify` | Atomically find and update/delete |

---

## Python

```python
import asyncio
from mongocore import MongoClient

async def main():
    async with MongoClient("localhost:50051") as client:
        db = client["myapp"]
        users = db["users"]

        # Insert
        user_id = await users.insert_one({
            "name": "Alice",
            "email": "alice@example.com",
            "age": 30
        })
        print(f"Inserted: {user_id}")

        # Insert many
        ids = await users.insert_many([
            {"name": "Bob", "age": 25},
            {"name": "Charlie", "age": 35},
        ])

        # Find (streaming cursor)
        async for user in users.find({"age": {"$gte": 25}}, limit=10):
            print(user["name"])

        # Or collect all at once
        all_users = await users.find({"age": {"$gte": 25}}).to_list()

        # Find one
        alice = await users.find_one({"name": "Alice"})

        # Update
        result = await users.update_one(
            {"name": "Alice"},
            {"$set": {"age": 31}}
        )
        print(f"Modified: {result['modified_count']}")

        # Update many
        result = await users.update_many(
            {"age": {"$lt": 30}},
            {"$inc": {"age": 1}}
        )

        # Delete
        count = await users.delete_one({"name": "Charlie"})

        # Delete many
        count = await users.delete_many({"age": {"$gt": 100}})

asyncio.run(main())
```

## TypeScript

```typescript
import { MongoClient } from '@mongocore/client';

async function main() {
  const client = new MongoClient('localhost:50051');
  await client.connect();

  const users = client.db('myapp').collection('users');

  // Insert
  const result = await users.insertOne({
    name: 'Alice',
    email: 'alice@example.com',
    age: 30,
  });
  console.log(`Inserted: ${result.insertedId}`);

  // Insert many
  const bulk = await users.insertMany([
    { name: 'Bob', age: 25 },
    { name: 'Charlie', age: 35 },
  ]);
  console.log(`Inserted ${bulk.insertedCount} documents`);

  // Find (streaming cursor)
  for await (const doc of users.find({ age: { $gte: 25 } }, { limit: 10 })) {
    console.log(doc.name);
  }

  // Or collect all at once
  const docs = await users.find({ age: { $gte: 25 } }).toArray();

  // Find one
  const alice = await users.findOne({ name: 'Alice' });

  // Update
  const updated = await users.updateOne(
    { name: 'Alice' },
    { $set: { age: 31 } }
  );
  console.log(`Modified: ${updated.modifiedCount}`);

  // Update many
  await users.updateMany(
    { age: { $lt: 30 } },
    { $inc: { age: 1 } }
  );

  // Delete
  await users.deleteOne({ name: 'Charlie' });
  await users.deleteMany({ age: { $gt: 100 } });

  await client.close();
}

main();
```

## Go

```go
package main

import (
    "context"
    "fmt"
    "log"

    "github.com/rozza/mongocore/clients/go/mongocore"
    "go.mongodb.org/mongo-driver/v2/bson"
)

func main() {
    ctx := context.Background()

    client := mongocore.NewClient("localhost:50051")
    if err := client.Connect(ctx); err != nil {
        log.Fatal(err)
    }
    defer client.Close()

    users := client.Database("myapp").Collection("users")

    // Insert
    id, err := users.InsertOne(ctx, bson.D{
        {Key: "name", Value: "Alice"},
        {Key: "email", Value: "alice@example.com"},
        {Key: "age", Value: 30},
    })
    if err != nil {
        log.Fatal(err)
    }
    fmt.Printf("Inserted: %s\n", id)

    // Insert many
    ids, err := users.InsertMany(ctx, []bson.D{
        {{Key: "name", Value: "Bob"}, {Key: "age", Value: 25}},
        {{Key: "name", Value: "Charlie"}, {Key: "age", Value: 35}},
    })

    // Find (streaming cursor)
    cursor := users.Find(ctx, bson.D{
        {Key: "age", Value: bson.D{{Key: "$gte", Value: 25}}},
    }, &mongocore.FindOptions{Limit: 10})
    defer cursor.Close()
    for cursor.Next(ctx) {
        fmt.Println(cursor.Doc())
    }
    if cursor.Err() != nil {
        log.Fatal(cursor.Err())
    }

    // Or collect all at once
    docs, err := users.Find(ctx, bson.D{}, nil).All(ctx)

    // Find one
    alice, err := users.FindOne(ctx, bson.D{{Key: "name", Value: "Alice"}})

    // Update
    result, err := users.UpdateOne(ctx,
        bson.D{{Key: "name", Value: "Alice"}},
        bson.D{{Key: "$set", Value: bson.D{{Key: "age", Value: 31}}}},
    )
    fmt.Printf("Modified: %d\n", result.ModifiedCount)

    // Update many
    result, _ = users.UpdateMany(ctx,
        bson.D{{Key: "age", Value: bson.D{{Key: "$lt", Value: 30}}}},
        bson.D{{Key: "$inc", Value: bson.D{{Key: "age", Value: 1}}}},
    )

    // Delete
    count, _ := users.DeleteOne(ctx, bson.D{{Key: "name", Value: "Charlie"}})
    count, _ = users.DeleteMany(ctx, bson.D{{Key: "age", Value: bson.D{{Key: "$gt", Value: 100}}}})
    _ = count
}
```

## Java

```java
import com.mongocore.MongoClient;
import com.mongocore.MongoDatabase;
import com.mongocore.MongoCollection;
import com.mongocore.FindOptions;
import org.bson.Document;
import java.util.Arrays;
import java.util.List;

public class CrudExample {
    public static void main(String[] args) throws Exception {
        try (MongoClient client = MongoClient.create("localhost:50051")) {
            MongoDatabase db = client.getDatabase("myapp");
            MongoCollection users = db.getCollection("users");

            // Insert
            Document user = new Document("name", "Alice")
                    .append("email", "alice@example.com")
                    .append("age", 30);
            users.insertOne(user);

            // Insert many
            List<Document> docs = Arrays.asList(
                new Document("name", "Bob").append("age", 25),
                new Document("name", "Charlie").append("age", 35)
            );
            users.insertMany(docs);

            // Find (streaming cursor)
            Document filter = new Document("age",
                new Document("$gte", 25));
            try (MongoCursor cursor = users.find(filter, new FindOptions().limit(10))) {
                for (Document doc : cursor) {
                    System.out.println(doc);
                }
            }

            // Or collect all at once
            List<Document> results = users.find(filter).toList();

            // Find one
            Document alice = users.findOne(
                new Document("name", "Alice"));

            // Update
            users.updateOne(
                new Document("name", "Alice"),
                new Document("$set", new Document("age", 31))
            );

            // Update many
            users.updateMany(
                new Document("age", new Document("$lt", 30)),
                new Document("$inc", new Document("age", 1))
            );

            // Delete
            users.deleteOne(new Document("name", "Charlie"));
            users.deleteMany(new Document("age",
                new Document("$gt", 100)));
        }
    }
}
```

## FindAndModify

Atomically finds a document, modifies it, and returns either the original or modified version.

### Python

```python
# Find and update, returning the new document
result = await users.find_and_modify(
    filter={"name": "Alice"},
    update={"$inc": {"login_count": 1}},
    return_document="after",
    upsert=True
)
```

### TypeScript

```typescript
const result = await users.findAndModify({
  filter: { name: 'Alice' },
  update: { $inc: { loginCount: 1 } },
  returnDocument: 'after',
  upsert: true,
});
```

## Wire Format

All BSON documents are sent as raw bytes in the protobuf messages. This avoids JSON-to-BSON conversion on the server side — your client library serializes directly to BSON, and MongoCore passes those bytes straight to MongoDB.

```protobuf
message Document {
  bytes data = 1;  // Raw BSON
}

message Filter {
  bytes data = 1;  // BSON filter document
}
```
