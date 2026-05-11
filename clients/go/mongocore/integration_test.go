package mongocore_test

import (
	"context"
	"fmt"
	"os"
	"testing"

	"github.com/rozza/mongocore/clients/go/mongocore"
	"go.mongodb.org/mongo-driver/v2/bson"
)

const testDB = "mongocore_client_test"

func getAddress() string {
	if addr := os.Getenv("MONGOCORE_ADDRESS"); addr != "" {
		return addr
	}
	return "localhost:50051"
}

func uniqueCollection() string {
	return fmt.Sprintf("go_test_%d", os.Getpid())
}

func setupClient(t *testing.T) (*mongocore.Client, context.Context) {
	t.Helper()
	ctx := context.Background()
	client := mongocore.NewClient(getAddress())
	if err := client.Connect(ctx); err != nil {
		t.Fatalf("Failed to connect: %v", err)
	}
	t.Cleanup(func() { client.Close() })
	return client, ctx
}

func TestInsertAndFind(t *testing.T) {
	client, ctx := setupClient(t)
	coll := client.Database(testDB).Collection(uniqueCollection() + "_insert_find")

	id, err := coll.InsertOne(ctx, bson.D{
		{Key: "name", Value: "Alice"},
		{Key: "age", Value: 30},
	})
	if err != nil {
		t.Fatalf("InsertOne failed: %v", err)
	}
	if id == "" {
		t.Fatal("Expected non-empty inserted ID")
	}

	docs, err := coll.Find(ctx, bson.D{{Key: "name", Value: "Alice"}}, nil)
	if err != nil {
		t.Fatalf("Find failed: %v", err)
	}
	if len(docs) != 1 {
		t.Fatalf("Expected 1 document, got %d", len(docs))
	}
}

func TestInsertMany(t *testing.T) {
	client, ctx := setupClient(t)
	coll := client.Database(testDB).Collection(uniqueCollection() + "_insert_many")

	ids, err := coll.InsertMany(ctx, []bson.D{
		{{Key: "name", Value: "Bob"}, {Key: "score", Value: 85}},
		{{Key: "name", Value: "Carol"}, {Key: "score", Value: 92}},
		{{Key: "name", Value: "Dave"}, {Key: "score", Value: 78}},
	})
	if err != nil {
		t.Fatalf("InsertMany failed: %v", err)
	}
	if len(ids) != 3 {
		t.Fatalf("Expected 3 IDs, got %d", len(ids))
	}

	docs, err := coll.Find(ctx, bson.D{}, nil)
	if err != nil {
		t.Fatalf("Find failed: %v", err)
	}
	if len(docs) != 3 {
		t.Fatalf("Expected 3 documents, got %d", len(docs))
	}
}

func TestFindOne(t *testing.T) {
	client, ctx := setupClient(t)
	coll := client.Database(testDB).Collection(uniqueCollection() + "_find_one")

	_, err := coll.InsertOne(ctx, bson.D{{Key: "key", Value: "unique_go_value"}})
	if err != nil {
		t.Fatalf("InsertOne failed: %v", err)
	}

	doc, err := coll.FindOne(ctx, bson.D{{Key: "key", Value: "unique_go_value"}})
	if err != nil {
		t.Fatalf("FindOne failed: %v", err)
	}
	if doc == nil {
		t.Fatal("Expected document, got nil")
	}

	missing, err := coll.FindOne(ctx, bson.D{{Key: "key", Value: "nonexistent"}})
	if err != nil {
		t.Fatalf("FindOne for missing failed: %v", err)
	}
	if missing != nil {
		t.Fatalf("Expected nil for missing document, got %v", missing)
	}
}

func TestUpdateOne(t *testing.T) {
	client, ctx := setupClient(t)
	coll := client.Database(testDB).Collection(uniqueCollection() + "_update")

	_, err := coll.InsertOne(ctx, bson.D{
		{Key: "name", Value: "Eve"},
		{Key: "status", Value: "active"},
	})
	if err != nil {
		t.Fatalf("InsertOne failed: %v", err)
	}

	result, err := coll.UpdateOne(ctx,
		bson.D{{Key: "name", Value: "Eve"}},
		bson.D{{Key: "$set", Value: bson.D{{Key: "status", Value: "inactive"}}}},
	)
	if err != nil {
		t.Fatalf("UpdateOne failed: %v", err)
	}
	if result.ModifiedCount != 1 {
		t.Fatalf("Expected 1 modified, got %d", result.ModifiedCount)
	}

	doc, _ := coll.FindOne(ctx, bson.D{{Key: "name", Value: "Eve"}})
	for _, elem := range doc {
		if elem.Key == "status" && elem.Value != "inactive" {
			t.Fatalf("Expected status=inactive, got %v", elem.Value)
		}
	}
}

func TestDeleteOne(t *testing.T) {
	client, ctx := setupClient(t)
	coll := client.Database(testDB).Collection(uniqueCollection() + "_delete")

	coll.InsertOne(ctx, bson.D{{Key: "name", Value: "Frank"}})
	coll.InsertOne(ctx, bson.D{{Key: "name", Value: "Grace"}})

	count, err := coll.DeleteOne(ctx, bson.D{{Key: "name", Value: "Frank"}})
	if err != nil {
		t.Fatalf("DeleteOne failed: %v", err)
	}
	if count != 1 {
		t.Fatalf("Expected 1 deleted, got %d", count)
	}

	docs, _ := coll.Find(ctx, bson.D{}, nil)
	if len(docs) != 1 {
		t.Fatalf("Expected 1 remaining, got %d", len(docs))
	}
}

func TestDeleteMany(t *testing.T) {
	client, ctx := setupClient(t)
	coll := client.Database(testDB).Collection(uniqueCollection() + "_delete_many")

	coll.InsertMany(ctx, []bson.D{
		{{Key: "group", Value: "A"}},
		{{Key: "group", Value: "A"}},
		{{Key: "group", Value: "B"}},
	})

	count, err := coll.DeleteMany(ctx, bson.D{{Key: "group", Value: "A"}})
	if err != nil {
		t.Fatalf("DeleteMany failed: %v", err)
	}
	if count != 2 {
		t.Fatalf("Expected 2 deleted, got %d", count)
	}

	docs, _ := coll.Find(ctx, bson.D{}, nil)
	if len(docs) != 1 {
		t.Fatalf("Expected 1 remaining, got %d", len(docs))
	}
}

func TestFindWithLimit(t *testing.T) {
	client, ctx := setupClient(t)
	coll := client.Database(testDB).Collection(uniqueCollection() + "_find_limit")

	docs := make([]bson.D, 10)
	for i := range docs {
		docs[i] = bson.D{{Key: "i", Value: i}}
	}
	coll.InsertMany(ctx, docs)

	results, err := coll.Find(ctx, bson.D{}, &mongocore.FindOptions{Limit: 3})
	if err != nil {
		t.Fatalf("Find with limit failed: %v", err)
	}
	if len(results) != 3 {
		t.Fatalf("Expected 3 documents, got %d", len(results))
	}
}

func TestAggregate(t *testing.T) {
	client, ctx := setupClient(t)
	coll := client.Database(testDB).Collection(uniqueCollection() + "_aggregate")

	coll.InsertMany(ctx, []bson.D{
		{{Key: "category", Value: "A"}, {Key: "value", Value: 10}},
		{{Key: "category", Value: "A"}, {Key: "value", Value: 20}},
		{{Key: "category", Value: "B"}, {Key: "value", Value: 30}},
	})

	results, err := coll.Aggregate(ctx, []bson.D{
		{{Key: "$group", Value: bson.D{
			{Key: "_id", Value: "$category"},
			{Key: "total", Value: bson.D{{Key: "$sum", Value: "$value"}}},
		}}},
		{{Key: "$sort", Value: bson.D{{Key: "_id", Value: 1}}}},
	})
	if err != nil {
		t.Fatalf("Aggregate failed: %v", err)
	}
	if len(results) != 2 {
		t.Fatalf("Expected 2 groups, got %d", len(results))
	}
}

func TestWatch(t *testing.T) {
	client, ctx := setupClient(t)
	coll := client.Database(testDB).Collection(uniqueCollection() + "_watch")

	// Create collection first
	coll.InsertOne(ctx, bson.D{{Key: "setup", Value: true}})

	cs, err := coll.Watch(ctx, nil)
	if err != nil {
		t.Fatalf("Watch failed: %v", err)
	}
	defer cs.Close()

	// Insert in a goroutine
	go func() {
		// small delay to let the stream establish
		<-ctx.Done()
	}()
	go func() {
		coll.InsertOne(ctx, bson.D{{Key: "name", Value: "watched"}})
	}()

	event, err := cs.Next()
	if err != nil {
		t.Fatalf("Next failed: %v", err)
	}
	if event.OperationType != "insert" {
		t.Fatalf("Expected insert, got %s", event.OperationType)
	}
}

func TestSearch(t *testing.T) {
	client, ctx := setupClient(t)
	coll := client.Database(testDB).Collection(uniqueCollection() + "_search")

	coll.InsertMany(ctx, []bson.D{
		{{Key: "title", Value: "rust programming guide"}, {Key: "content", Value: "learn rust basics"}},
		{{Key: "title", Value: "python basics"}, {Key: "content", Value: "learn python programming"}},
		{{Key: "title", Value: "rust advanced patterns"}, {Key: "content", Value: "advanced rust techniques"}},
	})

	result, err := coll.Search(ctx, "rust", 10)
	if err != nil {
		t.Fatalf("Search failed: %v", err)
	}
	if result.Total < 2 {
		t.Fatalf("Expected at least 2 results for 'rust', got %d", result.Total)
	}
	if result.Method == "" {
		t.Fatal("Expected non-empty search method")
	}
}

func TestListDatabases(t *testing.T) {
	client, ctx := setupClient(t)

	dbs, err := client.ListDatabases(ctx)
	if err != nil {
		t.Fatalf("ListDatabases failed: %v", err)
	}
	if len(dbs) == 0 {
		t.Fatal("Expected at least one database")
	}
}
