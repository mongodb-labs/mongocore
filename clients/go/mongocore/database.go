package mongocore

import (
	"context"

	pb "github.com/rozza/mongocore/clients/go/proto"
)

// Database provides access to collections within a database.
type Database struct {
	client *Client
	name   string
}

// Name returns the database name.
func (d *Database) Name() string {
	return d.name
}

// Collection returns a collection handle.
func (d *Database) Collection(name string) *Collection {
	return &Collection{
		client:   d.client,
		database: d.name,
		name:     name,
	}
}

// ListCollections returns all collection names in the database.
func (d *Database) ListCollections(ctx context.Context) ([]string, error) {
	resp, err := d.client.stub.ListCollections(clientContext(ctx), &pb.ListCollectionsRequest{
		Database: d.name,
	})
	if err != nil {
		return nil, err
	}
	return resp.Collections, nil
}

// CreateCollection creates a new collection in the database.
func (d *Database) CreateCollection(ctx context.Context, name string) error {
	_, err := d.client.stub.CreateCollection(clientContext(ctx), &pb.CreateCollectionRequest{
		Database:   d.name,
		Collection: name,
	})
	return err
}
