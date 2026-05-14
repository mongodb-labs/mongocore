package mongocore_test

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"testing"

	pb "github.com/rozza/mongocore/clients/go/proto"
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
	client := mongocore.MongoClientTCP(getAddress())
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

	cursor := coll.Find(ctx, bson.D{{Key: "name", Value: "Alice"}}, nil)
	docs, err := cursor.All(ctx)
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

	cursor := coll.Find(ctx, bson.D{}, nil)
	docs, err := cursor.All(ctx)
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

	cursor := coll.Find(ctx, bson.D{}, nil)
	docs, _ := cursor.All(ctx)
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

	cursor := coll.Find(ctx, bson.D{}, nil)
	docs, _ := cursor.All(ctx)
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

	cursor := coll.Find(ctx, bson.D{}, &mongocore.FindOptions{Limit: 3})
	results, err := cursor.All(ctx)
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

	cursor := coll.Aggregate(ctx, []bson.D{
		{{Key: "$group", Value: bson.D{
			{Key: "_id", Value: "$category"},
			{Key: "total", Value: bson.D{{Key: "$sum", Value: "$value"}}},
		}}},
		{{Key: "$sort", Value: bson.D{{Key: "_id", Value: 1}}}},
	}, nil)
	results, err := cursor.All(ctx)
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

func TestUpdateMany(t *testing.T) {
	client, ctx := setupClient(t)
	coll := client.Database(testDB).Collection(uniqueCollection() + "_update_many")

	coll.InsertMany(ctx, []bson.D{
		{{Key: "status", Value: "pending"}, {Key: "priority", Value: 1}},
		{{Key: "status", Value: "pending"}, {Key: "priority", Value: 2}},
		{{Key: "status", Value: "done"}, {Key: "priority", Value: 3}},
	})

	result, err := coll.UpdateMany(ctx,
		bson.D{{Key: "status", Value: "pending"}},
		bson.D{{Key: "$set", Value: bson.D{{Key: "status", Value: "active"}}}},
	)
	if err != nil {
		t.Fatalf("UpdateMany failed: %v", err)
	}
	if result.ModifiedCount != 2 {
		t.Fatalf("Expected 2 modified, got %d", result.ModifiedCount)
	}
}

func TestFindAndModify(t *testing.T) {
	client, ctx := setupClient(t)
	coll := client.Database(testDB).Collection(uniqueCollection() + "_find_modify")

	coll.InsertOne(ctx, bson.D{{Key: "counter", Value: 10}})

	doc, err := coll.FindAndModify(ctx,
		bson.D{{Key: "counter", Value: 10}},
		bson.D{{Key: "$inc", Value: bson.D{{Key: "counter", Value: 5}}}},
		true, // returnNew
	)
	if err != nil {
		t.Fatalf("FindAndModify failed: %v", err)
	}
	if doc == nil {
		t.Fatal("Expected document, got nil")
	}

	// Verify the returned document has the updated value
	for _, elem := range doc {
		if elem.Key == "counter" {
			if v, ok := elem.Value.(int32); ok {
				if v != 15 {
					t.Fatalf("Expected counter=15, got %d", v)
				}
			} else if v, ok := elem.Value.(int64); ok {
				if v != 15 {
					t.Fatalf("Expected counter=15, got %d", v)
				}
			}
		}
	}
}

func TestListCollections(t *testing.T) {
	client, ctx := setupClient(t)
	db := client.Database(testDB)
	collName := uniqueCollection() + "_list_colls"

	// Insert to ensure collection exists
	coll := db.Collection(collName)
	_, err := coll.InsertOne(ctx, bson.D{{Key: "test", Value: true}})
	if err != nil {
		t.Fatalf("InsertOne failed: %v", err)
	}

	collections, err := db.ListCollections(ctx)
	if err != nil {
		t.Fatalf("ListCollections failed: %v", err)
	}

	found := false
	for _, c := range collections {
		if c == collName {
			found = true
			break
		}
	}
	if !found {
		t.Fatalf("Expected collection %s in list", collName)
	}
}

func TestCreateCollection(t *testing.T) {
	client, ctx := setupClient(t)
	db := client.Database(testDB)
	collName := uniqueCollection() + "_create_coll"

	err := db.CreateCollection(ctx, collName)
	if err != nil {
		t.Fatalf("CreateCollection failed: %v", err)
	}

	collections, err := db.ListCollections(ctx)
	if err != nil {
		t.Fatalf("ListCollections failed: %v", err)
	}

	found := false
	for _, c := range collections {
		if c == collName {
			found = true
			break
		}
	}
	if !found {
		t.Fatalf("Expected collection %s in list", collName)
	}
}

func TestCreateIndex(t *testing.T) {
	client, ctx := setupClient(t)
	coll := client.Database(testDB).Collection(uniqueCollection() + "_create_index")

	// Insert a document first
	_, err := coll.InsertOne(ctx, bson.D{{Key: "email", Value: "test@example.com"}})
	if err != nil {
		t.Fatalf("InsertOne failed: %v", err)
	}

	indexName, err := coll.CreateIndex(ctx,
		bson.D{{Key: "email", Value: 1}},
		true, // unique
	)
	if err != nil {
		t.Fatalf("CreateIndex failed: %v", err)
	}
	if indexName == "" {
		t.Fatal("Expected non-empty index name")
	}
}

func TestRunCommand(t *testing.T) {
	client, ctx := setupClient(t)

	result, err := client.RunCommand(ctx, "admin", bson.D{{Key: "ping", Value: 1}}, false)
	if err != nil {
		t.Fatalf("RunCommand failed: %v", err)
	}
	if result == nil {
		t.Fatal("Expected non-nil result")
	}

	// Verify we got a result (bson.Unmarshal into interface{} produces bson.D)
	resultDoc, ok := result.(bson.D)
	if !ok {
		t.Fatalf("Expected bson.D result, got %T", result)
	}
	found := false
	for _, elem := range resultDoc {
		if elem.Key == "ok" {
			found = true
			break
		}
	}
	if !found {
		t.Fatal("Expected 'ok' field in result")
	}
}

func TestGetAnalytics(t *testing.T) {
	client, ctx := setupClient(t)
	coll := client.Database(testDB).Collection(uniqueCollection() + "_analytics")

	// Do some operations
	coll.InsertOne(ctx, bson.D{{Key: "test", Value: 1}})
	cursor := coll.Find(ctx, bson.D{}, nil)
	_, _ = cursor.All(ctx)

	analytics, err := client.GetAnalytics(ctx)
	if err != nil {
		t.Fatalf("GetAnalytics failed: %v", err)
	}
	if analytics == nil {
		t.Fatal("Expected non-nil analytics")
	}
	if analytics.TotalOperations < 0 {
		t.Fatal("Expected valid TotalOperations field")
	}
}

func TestTransactionCommit(t *testing.T) {
	client, ctx := setupClient(t)

	txnID, err := client.BeginTransaction(ctx, testDB)
	if err != nil {
		t.Fatalf("BeginTransaction failed: %v", err)
	}
	if txnID == "" {
		t.Fatal("Expected non-empty transaction ID")
	}

	err = client.CommitTransaction(ctx, txnID)
	if err != nil {
		t.Fatalf("CommitTransaction failed: %v", err)
	}
}

func TestTransactionAbort(t *testing.T) {
	client, ctx := setupClient(t)

	txnID, err := client.BeginTransaction(ctx, testDB)
	if err != nil {
		t.Fatalf("BeginTransaction failed: %v", err)
	}
	if txnID == "" {
		t.Fatal("Expected non-empty transaction ID")
	}

	err = client.AbortTransaction(ctx, txnID)
	if err != nil {
		t.Fatalf("AbortTransaction failed: %v", err)
	}
}

func TestIngestCSV(t *testing.T) {
	client, ctx := setupClient(t)

	// Resolve path to test fixture
	csvPath, err := filepath.Abs(filepath.Join("..", "..", "test_fixtures", "sample.csv"))
	if err != nil {
		t.Fatalf("Failed to resolve CSV path: %v", err)
	}

	result, err := client.Ingest(ctx, mongocore.IngestOptions{
		FilePath:   csvPath,
		Database:   testDB,
		Collection: uniqueCollection() + "_ingest",
		Format:     pb.FileFormat_FILE_FORMAT_CSV,
	})
	if err != nil {
		t.Fatalf("Ingest failed: %v", err)
	}
	if result.JobID == "" {
		t.Fatal("Expected non-empty job ID")
	}
}

func TestIngestStatus(t *testing.T) {
	client, ctx := setupClient(t)

	csvPath, err := filepath.Abs(filepath.Join("..", "..", "test_fixtures", "sample.csv"))
	if err != nil {
		t.Fatalf("Failed to resolve CSV path: %v", err)
	}

	ingestResult, err := client.Ingest(ctx, mongocore.IngestOptions{
		FilePath:   csvPath,
		Database:   testDB,
		Collection: uniqueCollection() + "_ingest_status",
		Format:     pb.FileFormat_FILE_FORMAT_CSV,
	})
	if err != nil {
		t.Fatalf("Ingest failed: %v", err)
	}

	status, err := client.IngestStatus(ctx, ingestResult.JobID)
	if err != nil {
		t.Fatalf("IngestStatus failed: %v", err)
	}
	if status == nil {
		t.Fatal("Expected non-nil status")
	}
	if status.JobID != ingestResult.JobID {
		t.Fatalf("Expected JobID %s, got %s", ingestResult.JobID, status.JobID)
	}
}

func TestListIngestJobs(t *testing.T) {
	client, ctx := setupClient(t)

	jobs, err := client.ListIngestJobs(ctx)
	if err != nil {
		t.Fatalf("ListIngestJobs failed: %v", err)
	}
	// jobs can be empty, just verify it returns a slice
	if jobs == nil {
		t.Fatal("Expected non-nil jobs slice")
	}
}

func TestCancelIngest(t *testing.T) {
	client, ctx := setupClient(t)

	csvPath, err := filepath.Abs(filepath.Join("..", "..", "test_fixtures", "sample.csv"))
	if err != nil {
		t.Fatalf("Failed to resolve CSV path: %v", err)
	}

	ingestResult, err := client.Ingest(ctx, mongocore.IngestOptions{
		FilePath:   csvPath,
		Database:   testDB,
		Collection: uniqueCollection() + "_cancel_ingest",
		Format:     pb.FileFormat_FILE_FORMAT_CSV,
	})
	if err != nil {
		t.Fatalf("Ingest failed: %v", err)
	}

	success, err := client.CancelIngest(ctx, ingestResult.JobID)
	if err != nil {
		t.Fatalf("CancelIngest failed: %v", err)
	}
	// success can be false if job already completed, just verify the call works
	_ = success
}

func TestWatchDirectory(t *testing.T) {
	client, ctx := setupClient(t)

	// Create temp directory
	tmpDir, err := os.MkdirTemp("", "mongocore_watch_test_*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tmpDir)

	watchID, err := client.WatchDirectory(ctx, mongocore.WatchDirectoryOptions{
		Path:       tmpDir,
		Database:   testDB,
		Collection: uniqueCollection() + "_watch_dir",
	})
	if err != nil {
		t.Fatalf("WatchDirectory failed: %v", err)
	}
	if watchID == "" {
		t.Fatal("Expected non-empty watch ID")
	}

	// Stop the watch
	success, err := client.StopWatch(ctx, watchID)
	if err != nil {
		t.Fatalf("StopWatch failed: %v", err)
	}
	if !success {
		t.Fatal("Expected StopWatch to return true")
	}
}

func TestStopWatch(t *testing.T) {
	client, ctx := setupClient(t)

	tmpDir, err := os.MkdirTemp("", "mongocore_stop_watch_test_*")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tmpDir)

	watchID, err := client.WatchDirectory(ctx, mongocore.WatchDirectoryOptions{
		Path:       tmpDir,
		Database:   testDB,
		Collection: uniqueCollection() + "_stop_watch",
	})
	if err != nil {
		t.Fatalf("WatchDirectory failed: %v", err)
	}

	success, err := client.StopWatch(ctx, watchID)
	if err != nil {
		t.Fatalf("StopWatch failed: %v", err)
	}
	if !success {
		t.Fatal("Expected StopWatch to return true")
	}
}

func TestPipeline(t *testing.T) {
	client, ctx := setupClient(t)
	collName := uniqueCollection() + "_pipeline"

	// Seed a document first
	coll := client.Database(testDB).Collection(collName)
	seedID, err := coll.InsertOne(ctx, bson.D{{Key: "name", Value: "seed"}, {Key: "value", Value: 100}})
	if err != nil {
		t.Fatalf("Failed to seed document: %v", err)
	}
	if seedID == "" {
		t.Fatal("Expected non-empty seed ID")
	}

	// Build operations manually (ops package is in ../ops)
	// For the test, we'll construct them directly
	filterBytes, _ := bson.Marshal(bson.D{{Key: "name", Value: "seed"}})
	op1 := &pb.PipelineOperation{
		Operation: &pb.PipelineOperation_Find{
			Find: &pb.FindRequest{
				Database:   testDB,
				Collection: collName,
				Filter:     &pb.Filter{Data: filterBytes},
			},
		},
	}

	insertBytes, _ := bson.Marshal(bson.D{{Key: "name", Value: "inserted_via_pipeline"}, {Key: "value", Value: 200}})
	op2 := &pb.PipelineOperation{
		Operation: &pb.PipelineOperation_Insert{
			Insert: &pb.InsertRequest{
				Database:   testDB,
				Collection: collName,
				Document:   &pb.Document{Data: insertBytes},
			},
		},
	}

	op3 := &pb.PipelineOperation{
		Operation: &pb.PipelineOperation_ListDatabases{
			ListDatabases: &pb.ListDatabasesRequest{},
		},
	}

	// Execute pipeline
	results, err := client.Pipeline(ctx, op1, op2, op3)
	if err != nil {
		t.Fatalf("Pipeline failed: %v", err)
	}

	// Verify we got 3 results
	if len(results) != 3 {
		t.Fatalf("Expected 3 results, got %d", len(results))
	}

	// Verify all succeeded
	for i, result := range results {
		if !result.Success {
			t.Fatalf("Result %d failed: %s", i, result.Error)
		}
	}

	// Verify Find result
	if findResp, ok := results[0].AsFind(); ok {
		if len(findResp.Documents) != 1 {
			t.Fatalf("Expected 1 document from Find, got %d", len(findResp.Documents))
		}
	} else {
		t.Fatal("Expected Find result at index 0")
	}

	// Verify Insert result
	if insertResp, ok := results[1].AsInsert(); ok {
		if insertResp.InsertedId == "" {
			t.Fatal("Expected non-empty inserted ID")
		}
	} else {
		t.Fatal("Expected Insert result at index 1")
	}

	// Verify ListDatabases result
	if listResp, ok := results[2].AsListDatabases(); ok {
		if len(listResp.Databases) == 0 {
			t.Fatal("Expected at least one database")
		}
	} else {
		t.Fatal("Expected ListDatabases result at index 2")
	}
}

func TestFindCursorIteration(t *testing.T) {
	ctx := context.Background()
	client := mongocore.MongoClientTCP(getAddress())
	if err := client.Connect(ctx); err != nil {
		t.Fatal(err)
	}
	defer client.Close()

	coll := client.Database(testDB).Collection(uniqueCollection())

	// Insert 50 docs
	docs := make([]bson.D, 50)
	for i := range docs {
		docs[i] = bson.D{{Key: "i", Value: i}}
	}
	_, err := coll.InsertMany(ctx, docs)
	if err != nil {
		t.Fatal(err)
	}

	cursor := coll.Find(ctx, bson.D{}, nil)
	defer cursor.Close()

	count := 0
	for cursor.Next(ctx) {
		doc := cursor.Doc()
		if doc == nil {
			t.Fatal("expected non-nil document")
		}
		count++
	}
	if cursor.Err() != nil {
		t.Fatal(cursor.Err())
	}
	if count != 50 {
		t.Fatalf("expected 50 docs, got %d", count)
	}
}

func TestFindCursorEarlyClose(t *testing.T) {
	ctx := context.Background()
	client := mongocore.MongoClientTCP(getAddress())
	if err := client.Connect(ctx); err != nil {
		t.Fatal(err)
	}
	defer client.Close()

	coll := client.Database(testDB).Collection(uniqueCollection())

	docs := make([]bson.D, 100)
	for i := range docs {
		docs[i] = bson.D{{Key: "i", Value: i}}
	}
	_, err := coll.InsertMany(ctx, docs)
	if err != nil {
		t.Fatal(err)
	}

	cursor := coll.Find(ctx, bson.D{}, &mongocore.FindOptions{BatchSize: 10})
	count := 0
	for cursor.Next(ctx) {
		_ = cursor.Doc()
		count++
		if count >= 5 {
			break
		}
	}
	cursor.Close()
	if count != 5 {
		t.Fatalf("expected 5 docs, got %d", count)
	}
}

func TestFindCursorEmpty(t *testing.T) {
	ctx := context.Background()
	client := mongocore.MongoClientTCP(getAddress())
	if err := client.Connect(ctx); err != nil {
		t.Fatal(err)
	}
	defer client.Close()

	coll := client.Database(testDB).Collection(uniqueCollection())

	cursor := coll.Find(ctx, bson.D{{Key: "nonexistent", Value: true}}, nil)
	docs, err := cursor.All(ctx)
	if err != nil {
		t.Fatal(err)
	}
	if len(docs) != 0 {
		t.Fatalf("expected 0 docs, got %d", len(docs))
	}
}
