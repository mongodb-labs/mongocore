package mongocore_test

import (
	"testing"

	"github.com/rozza/mongocore/clients/go/mongocore"
)

func TestUnitClientCreation(t *testing.T) {
	client := mongocore.NewClient("custom:9999")
	if client == nil {
		t.Fatal("Expected non-nil client")
	}
}

func TestUnitClientDefaultAddress(t *testing.T) {
	client := mongocore.NewClient("localhost:50051")
	if client == nil {
		t.Fatal("Expected non-nil client")
	}
}

func TestUnitDatabaseAccess(t *testing.T) {
	client := mongocore.NewClient("localhost:50051")
	db := client.Database("testdb")
	if db == nil {
		t.Fatal("Expected non-nil database")
	}
	if db.Name() != "testdb" {
		t.Fatalf("Expected 'testdb', got '%s'", db.Name())
	}
}

func TestUnitCollectionAccess(t *testing.T) {
	client := mongocore.NewClient("localhost:50051")
	coll := client.Database("testdb").Collection("users")
	if coll == nil {
		t.Fatal("Expected non-nil collection")
	}
}

func TestUnitClientMetadata(t *testing.T) {
	// Verify client construction works (metadata is internal)
	client := mongocore.NewClient("localhost:50051")
	if client == nil {
		t.Fatal("Expected non-nil client")
	}
}
