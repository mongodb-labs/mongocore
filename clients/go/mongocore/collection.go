package mongocore

import (
	"context"
	"fmt"

	"go.mongodb.org/mongo-driver/v2/bson"
)

// Collection provides CRUD operations on a MongoDB collection via MongoCore.
type Collection struct {
	client   *Client
	database string
	name     string
}

// FindOptions configures a find operation.
type FindOptions struct {
	Limit      int64
	Skip       int64
	Sort       bson.D
	Projection bson.D
}

// UpdateResult contains the result of an update operation.
type UpdateResult struct {
	MatchedCount  int64
	ModifiedCount int64
	UpsertedID    string
}

// Find returns documents matching the filter.
func (c *Collection) Find(ctx context.Context, filter bson.D, opts *FindOptions) ([]bson.D, error) {
	// Will use generated gRPC stub to call Find RPC
	// Encodes filter as BSON bytes, sends via proto, decodes response
	return nil, fmt.Errorf("mongocore: requires generated gRPC stubs")
}

// FindOne returns a single document matching the filter.
func (c *Collection) FindOne(ctx context.Context, filter bson.D) (bson.D, error) {
	return nil, fmt.Errorf("mongocore: requires generated gRPC stubs")
}

// InsertOne inserts a single document.
func (c *Collection) InsertOne(ctx context.Context, document bson.D) (string, error) {
	return "", fmt.Errorf("mongocore: requires generated gRPC stubs")
}

// InsertMany inserts multiple documents.
func (c *Collection) InsertMany(ctx context.Context, documents []bson.D) ([]string, error) {
	return nil, fmt.Errorf("mongocore: requires generated gRPC stubs")
}

// UpdateOne updates a single document.
func (c *Collection) UpdateOne(ctx context.Context, filter, update bson.D) (*UpdateResult, error) {
	return nil, fmt.Errorf("mongocore: requires generated gRPC stubs")
}

// UpdateMany updates multiple documents.
func (c *Collection) UpdateMany(ctx context.Context, filter, update bson.D) (*UpdateResult, error) {
	return nil, fmt.Errorf("mongocore: requires generated gRPC stubs")
}

// DeleteOne deletes a single document.
func (c *Collection) DeleteOne(ctx context.Context, filter bson.D) (int64, error) {
	return 0, fmt.Errorf("mongocore: requires generated gRPC stubs")
}

// DeleteMany deletes multiple documents.
func (c *Collection) DeleteMany(ctx context.Context, filter bson.D) (int64, error) {
	return 0, fmt.Errorf("mongocore: requires generated gRPC stubs")
}

// Aggregate executes an aggregation pipeline.
func (c *Collection) Aggregate(ctx context.Context, pipeline []bson.D) ([]bson.D, error) {
	return nil, fmt.Errorf("mongocore: requires generated gRPC stubs")
}
