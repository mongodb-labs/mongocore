# MongoCore Java Client

Idiomatic Java client for MongoCore with builder-pattern API.

## Installation

Maven:
```xml
<dependency>
    <groupId>com.mongocore</groupId>
    <artifactId>mongocore-client</artifactId>
    <version>0.1.0</version>
</dependency>
```

## Quick Start

```java
import com.mongocore.MongoClient;
import com.mongocore.MongoDatabase;
import com.mongocore.MongoCollection;
import org.bson.Document;

try (MongoClient client = MongoClient.create("localhost:50051")) {
    MongoDatabase db = client.getDatabase("mydb");
    MongoCollection users = db.getCollection("users");
    
    // Insert
    users.insertOne(new Document("name", "Alice").append("age", 30));
    
    // Find
    List<Document> results = users.find(new Document("age", new Document("$gte", 25)));
    
    // Aggregate
    List<Document> agg = users.aggregate(List.of(
        new Document("$group", new Document("_id", "$status")
            .append("count", new Document("$sum", 1)))
    ));
}
```

## Generate gRPC Stubs

```bash
mvn generate-sources
```
