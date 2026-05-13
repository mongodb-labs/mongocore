# MongoCore Go Client

Idiomatic Go client for MongoCore.

## Installation

```bash
go get github.com/rozza/mongocore/clients/go
```

## Quick Start

```go
package main

import (
    "context"
    "fmt"

    "github.com/rozza/mongocore/clients/go/mongocore"
    "go.mongodb.org/mongo-driver/v2/bson"
)

func main() {
    client := mongocore.MongoClient()
    if err := client.Connect(context.Background()); err != nil {
        panic(err)
    }
    defer client.Close()

    coll := client.Database("mydb").Collection("users")
    
    // Insert
    id, _ := coll.InsertOne(context.Background(), bson.D{{"name", "Alice"}, {"age", 30}})
    fmt.Println("Inserted:", id)
    
    // Find
    docs, _ := coll.Find(context.Background(), bson.D{{"age", bson.D{{"$gte", 25}}}}, nil)
    fmt.Println("Found:", docs)
}
```

## Generate gRPC Stubs

```bash
# Install protoc-gen-go and protoc-gen-go-grpc first
./generate_stubs.sh
```
