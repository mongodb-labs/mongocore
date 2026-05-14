# Aggregation

MongoCore supports the full MongoDB aggregation framework. Pipeline stages are sent as individual BSON documents, giving you access to every aggregation operator MongoDB provides.

## Python

```python
from mongocore import MongoClient

async def main():
    async with MongoClient() as client:
        orders = client["shop"]["orders"]

        # Group by status, calculate totals
        async for doc in orders.aggregate([
            {"$match": {"year": 2024}},
            {"$group": {
                "_id": "$status",
                "total_revenue": {"$sum": "$amount"},
                "count": {"$sum": 1}
            }},
            {"$sort": {"total_revenue": -1}}
        ]):
            print(f"{doc['_id']}: ${doc['total_revenue']} ({doc['count']} orders)")
```

## TypeScript

```typescript
import { MongoClient } from '@mongocore/client';

const client = new MongoClient();
await client.connect();

const orders = client.db('shop').collection('orders');

for await (const doc of orders.aggregate([
  { $match: { year: 2024 } },
  { $group: {
    _id: '$status',
    totalRevenue: { $sum: '$amount' },
    count: { $sum: 1 },
  }},
  { $sort: { totalRevenue: -1 } },
])) {
  console.log(`${doc._id}: $${doc.totalRevenue} (${doc.count} orders)`);
}

await client.close();
```

## Go

```go
orders := client.Database("shop").Collection("orders")

pipeline := []bson.D{
    {{Key: "$match", Value: bson.D{{Key: "year", Value: 2024}}}},
    {{Key: "$group", Value: bson.D{
        {Key: "_id", Value: "$status"},
        {Key: "total_revenue", Value: bson.D{{Key: "$sum", Value: "$amount"}}},
        {Key: "count", Value: bson.D{{Key: "$sum", Value: 1}}},
    }}},
    {{Key: "$sort", Value: bson.D{{Key: "total_revenue", Value: -1}}}},
}

cursor := orders.Aggregate(ctx, pipeline, nil)
defer cursor.Close()
for cursor.Next(ctx) {
    fmt.Println(cursor.Doc())
}
if cursor.Err() != nil {
    log.Fatal(cursor.Err())
}
```

## Java

```java
MongoCollection orders = client.getDatabase("shop").getCollection("orders");

List<Document> pipeline = Arrays.asList(
    new Document("$match", new Document("year", 2024)),
    new Document("$group", new Document("_id", "$status")
        .append("total_revenue", new Document("$sum", "$amount"))
        .append("count", new Document("$sum", 1))),
    new Document("$sort", new Document("total_revenue", -1))
);

try (MongoCursor cursor = orders.aggregate(pipeline)) {
    for (Document doc : cursor) {
        System.out.println(doc);
    }
}
```

## Common Patterns

### Lookup (Join)

```python
async for doc in orders.aggregate([
    {"$lookup": {
        "from": "customers",
        "localField": "customer_id",
        "foreignField": "_id",
        "as": "customer"
    }},
    {"$unwind": "$customer"},
    {"$project": {
        "order_id": 1,
        "amount": 1,
        "customer_name": "$customer.name"
    }}
]):
    print(doc)
```

### Bucket

```python
async for doc in products.aggregate([
    {"$bucket": {
        "groupBy": "$price",
        "boundaries": [0, 25, 50, 100, 500],
        "default": "expensive",
        "output": {
            "count": {"$sum": 1},
            "avg_price": {"$avg": "$price"}
        }
    }}
]):
    print(doc)
```

### Window Functions

```python
async for doc in sales.aggregate([
    {"$setWindowFields": {
        "partitionBy": "$region",
        "sortBy": {"date": 1},
        "output": {
            "running_total": {
                "$sum": "$amount",
                "window": {"documents": ["unbounded", "current"]}
            }
        }
    }}
]):
    print(doc)
```

## Wire Format

Pipeline stages are sent as an array of raw BSON documents:

```protobuf
message Pipeline {
  repeated bytes stages = 1;  // Each stage is a BSON document
}

message AggregateRequest {
  string database = 1;
  string collection = 2;
  Pipeline pipeline = 3;
  optional string transaction_id = 4;
}
```
