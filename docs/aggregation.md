# Aggregation

MongoCore supports the full MongoDB aggregation framework. Pipeline stages are sent as individual BSON documents, giving you access to every aggregation operator MongoDB provides.

## Python

```python
from mongocore import MongoCoreClient

async def main():
    async with MongoCoreClient() as client:
        orders = client["shop"]["orders"]

        # Group by status, calculate totals
        results = await orders.aggregate([
            {"$match": {"year": 2024}},
            {"$group": {
                "_id": "$status",
                "total_revenue": {"$sum": "$amount"},
                "count": {"$sum": 1}
            }},
            {"$sort": {"total_revenue": -1}}
        ])

        for doc in results:
            print(f"{doc['_id']}: ${doc['total_revenue']} ({doc['count']} orders)")
```

## TypeScript

```typescript
import { MongoCoreClient } from '@mongocore/client';

const client = new MongoCoreClient();
await client.connect();

const orders = client.db('shop').collection('orders');

const results = await orders.aggregate([
  { $match: { year: 2024 } },
  { $group: {
    _id: '$status',
    totalRevenue: { $sum: '$amount' },
    count: { $sum: 1 },
  }},
  { $sort: { totalRevenue: -1 } },
]);

results.forEach(doc =>
  console.log(`${doc._id}: $${doc.totalRevenue} (${doc.count} orders)`)
);

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

results, err := orders.Aggregate(ctx, pipeline)
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

List<Document> results = orders.aggregate(pipeline);
```

## Common Patterns

### Lookup (Join)

```python
results = await orders.aggregate([
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
])
```

### Bucket

```python
results = await products.aggregate([
    {"$bucket": {
        "groupBy": "$price",
        "boundaries": [0, 25, 50, 100, 500],
        "default": "expensive",
        "output": {
            "count": {"$sum": 1},
            "avg_price": {"$avg": "$price"}
        }
    }}
])
```

### Window Functions

```python
results = await sales.aggregate([
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
])
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
