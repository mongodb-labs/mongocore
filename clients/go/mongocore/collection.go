package mongocore

import (
	"context"
	"io"

	pb "github.com/rozza/mongocore/clients/go/proto"
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

// SearchResult contains the results of a search operation.
type SearchResult struct {
	Documents []bson.D
	Method    string
	Total     int64
}

func encodeBson(doc bson.D) ([]byte, error) {
	return bson.Marshal(doc)
}

func decodeBsonDoc(data []byte) (bson.D, error) {
	var doc bson.D
	err := bson.Unmarshal(data, &doc)
	return doc, err
}

// Find returns documents matching the filter.
func (c *Collection) Find(ctx context.Context, filter bson.D, opts *FindOptions) ([]bson.D, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return nil, err
	}

	req := &pb.FindRequest{
		Database:   c.database,
		Collection: c.name,
		Filter:     &pb.Filter{Data: filterBytes},
	}

	if opts != nil {
		findOpts := &pb.FindOptions{}
		if opts.Limit > 0 {
			limit := opts.Limit
			findOpts.Limit = &limit
		}
		if opts.Skip > 0 {
			skip := opts.Skip
			findOpts.Skip = &skip
		}
		if opts.Sort != nil {
			sortBytes, err := encodeBson(opts.Sort)
			if err != nil {
				return nil, err
			}
			findOpts.Sort = sortBytes
		}
		if opts.Projection != nil {
			projBytes, err := encodeBson(opts.Projection)
			if err != nil {
				return nil, err
			}
			findOpts.Projection = projBytes
		}
		req.Options = findOpts
	}

	resp, err := c.client.stub.Find(clientContext(ctx), req)
	if err != nil {
		return nil, err
	}

	docs := make([]bson.D, 0, len(resp.Documents))
	for _, d := range resp.Documents {
		doc, err := decodeBsonDoc(d.Data)
		if err != nil {
			return nil, err
		}
		docs = append(docs, doc)
	}
	return docs, nil
}

// FindOne returns a single document matching the filter.
func (c *Collection) FindOne(ctx context.Context, filter bson.D) (bson.D, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return nil, err
	}

	resp, err := c.client.stub.FindOne(clientContext(ctx), &pb.FindOneRequest{
		Database:   c.database,
		Collection: c.name,
		Filter:     &pb.Filter{Data: filterBytes},
	})
	if err != nil {
		return nil, err
	}

	if resp.Document == nil || len(resp.Document.Data) == 0 {
		return nil, nil
	}
	return decodeBsonDoc(resp.Document.Data)
}

// InsertOne inserts a single document.
func (c *Collection) InsertOne(ctx context.Context, document bson.D) (string, error) {
	docBytes, err := encodeBson(document)
	if err != nil {
		return "", err
	}

	resp, err := c.client.stub.Insert(clientContext(ctx), &pb.InsertRequest{
		Database:   c.database,
		Collection: c.name,
		Document:   &pb.Document{Data: docBytes},
	})
	if err != nil {
		return "", err
	}
	return resp.InsertedId, nil
}

// InsertMany inserts multiple documents.
func (c *Collection) InsertMany(ctx context.Context, documents []bson.D) ([]string, error) {
	pbDocs := make([]*pb.Document, 0, len(documents))
	for _, doc := range documents {
		docBytes, err := encodeBson(doc)
		if err != nil {
			return nil, err
		}
		pbDocs = append(pbDocs, &pb.Document{Data: docBytes})
	}

	resp, err := c.client.stub.InsertMany(clientContext(ctx), &pb.InsertManyRequest{
		Database:   c.database,
		Collection: c.name,
		Documents:  pbDocs,
	})
	if err != nil {
		return nil, err
	}
	return resp.InsertedIds, nil
}

// UpdateOne updates a single document.
func (c *Collection) UpdateOne(ctx context.Context, filter, update bson.D) (*UpdateResult, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return nil, err
	}
	updateBytes, err := encodeBson(update)
	if err != nil {
		return nil, err
	}

	resp, err := c.client.stub.Update(clientContext(ctx), &pb.UpdateRequest{
		Database:   c.database,
		Collection: c.name,
		Filter:     &pb.Filter{Data: filterBytes},
		Update:     &pb.Document{Data: updateBytes},
	})
	if err != nil {
		return nil, err
	}
	return &UpdateResult{
		MatchedCount:  resp.MatchedCount,
		ModifiedCount: resp.ModifiedCount,
	}, nil
}

// UpdateMany updates multiple documents.
func (c *Collection) UpdateMany(ctx context.Context, filter, update bson.D) (*UpdateResult, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return nil, err
	}
	updateBytes, err := encodeBson(update)
	if err != nil {
		return nil, err
	}

	resp, err := c.client.stub.UpdateMany(clientContext(ctx), &pb.UpdateManyRequest{
		Database:   c.database,
		Collection: c.name,
		Filter:     &pb.Filter{Data: filterBytes},
		Update:     &pb.Document{Data: updateBytes},
	})
	if err != nil {
		return nil, err
	}
	return &UpdateResult{
		MatchedCount:  resp.MatchedCount,
		ModifiedCount: resp.ModifiedCount,
	}, nil
}

// DeleteOne deletes a single document.
func (c *Collection) DeleteOne(ctx context.Context, filter bson.D) (int64, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return 0, err
	}

	resp, err := c.client.stub.Delete(clientContext(ctx), &pb.DeleteRequest{
		Database:   c.database,
		Collection: c.name,
		Filter:     &pb.Filter{Data: filterBytes},
	})
	if err != nil {
		return 0, err
	}
	return resp.DeletedCount, nil
}

// DeleteMany deletes multiple documents.
func (c *Collection) DeleteMany(ctx context.Context, filter bson.D) (int64, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return 0, err
	}

	resp, err := c.client.stub.DeleteMany(clientContext(ctx), &pb.DeleteManyRequest{
		Database:   c.database,
		Collection: c.name,
		Filter:     &pb.Filter{Data: filterBytes},
	})
	if err != nil {
		return 0, err
	}
	return resp.DeletedCount, nil
}

// ChangeEvent represents a single change stream event.
type ChangeEvent struct {
	OperationType string
	Database      string
	Collection    string
	Document      bson.D
	UpdateDesc    bson.D
	DocumentKey   bson.D
}

// ChangeStream wraps a server-streaming Watch RPC. It implements io.Closer.
type ChangeStream struct {
	stream   pb.MongoCore_WatchClient
	cancelFn context.CancelFunc
}

// Next returns the next event from the change stream.
// Returns io.EOF when the stream is closed.
func (cs *ChangeStream) Next() (*ChangeEvent, error) {
	event, err := cs.stream.Recv()
	if err != nil {
		return nil, err
	}

	opNames := []string{"insert", "update", "delete", "replace", "invalidate"}
	opType := "unknown"
	if int(event.OperationType) < len(opNames) {
		opType = opNames[event.OperationType]
	}

	ce := &ChangeEvent{
		OperationType: opType,
		Database:      event.Database,
		Collection:    event.Collection,
	}

	if event.Document != nil && len(event.Document.Data) > 0 {
		doc, err := decodeBsonDoc(event.Document.Data)
		if err == nil {
			ce.Document = doc
		}
	}
	if event.UpdateDescription != nil && len(event.UpdateDescription.Data) > 0 {
		doc, err := decodeBsonDoc(event.UpdateDescription.Data)
		if err == nil {
			ce.UpdateDesc = doc
		}
	}
	if event.DocumentKey != nil && len(event.DocumentKey.Data) > 0 {
		doc, err := decodeBsonDoc(event.DocumentKey.Data)
		if err == nil {
			ce.DocumentKey = doc
		}
	}

	return ce, nil
}

// Close terminates the change stream.
func (cs *ChangeStream) Close() error {
	cs.cancelFn()
	return nil
}

// Watch opens a change stream on this collection. The returned ChangeStream must be closed when done.
func (c *Collection) Watch(ctx context.Context, pipeline []bson.D) (*ChangeStream, error) {
	stages := make([][]byte, 0, len(pipeline))
	for _, stage := range pipeline {
		stageBytes, err := encodeBson(stage)
		if err != nil {
			return nil, err
		}
		stages = append(stages, stageBytes)
	}

	watchCtx, cancel := context.WithCancel(ctx)
	stream, err := c.client.stub.Watch(clientContext(watchCtx), &pb.WatchRequest{
		Database:   c.database,
		Collection: &c.name,
		Pipeline:   &pb.Pipeline{Stages: stages},
	})
	if err != nil {
		cancel()
		return nil, err
	}

	return &ChangeStream{stream: stream, cancelFn: cancel}, nil
}

// Ensure ChangeStream implements io.Closer.
var _ io.Closer = (*ChangeStream)(nil)

// Aggregate executes an aggregation pipeline.
func (c *Collection) Aggregate(ctx context.Context, pipeline []bson.D) ([]bson.D, error) {
	stages := make([][]byte, 0, len(pipeline))
	for _, stage := range pipeline {
		stageBytes, err := encodeBson(stage)
		if err != nil {
			return nil, err
		}
		stages = append(stages, stageBytes)
	}

	resp, err := c.client.stub.Aggregate(clientContext(ctx), &pb.AggregateRequest{
		Database:   c.database,
		Collection: c.name,
		Pipeline:   &pb.Pipeline{Stages: stages},
	})
	if err != nil {
		return nil, err
	}

	docs := make([]bson.D, 0, len(resp.Documents))
	for _, d := range resp.Documents {
		doc, err := decodeBsonDoc(d.Data)
		if err != nil {
			return nil, err
		}
		docs = append(docs, doc)
	}
	return docs, nil
}

// Search performs a unified search using the best available method (vector → fulltext → filter).
func (c *Collection) Search(ctx context.Context, query string, limit int64) (*SearchResult, error) {
	resp, err := c.client.stub.Search(clientContext(ctx), &pb.SearchRequest{
		Database:   c.database,
		Collection: c.name,
		Query:      query,
		Limit:      limit,
	})
	if err != nil {
		return nil, err
	}

	docs := make([]bson.D, 0, len(resp.Documents))
	for _, d := range resp.Documents {
		doc, err := decodeBsonDoc(d.Data)
		if err != nil {
			return nil, err
		}
		docs = append(docs, doc)
	}
	return &SearchResult{
		Documents: docs,
		Method:    resp.Method,
		Total:     resp.Total,
	}, nil
}

// FindAndModify atomically modifies a document and returns it.
func (c *Collection) FindAndModify(ctx context.Context, filter, update bson.D, returnNew bool) (bson.D, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return nil, err
	}
	updateBytes, err := encodeBson(update)
	if err != nil {
		return nil, err
	}

	returnDoc := pb.FindAndModifyOptions_BEFORE
	if returnNew {
		returnDoc = pb.FindAndModifyOptions_AFTER
	}

	resp, err := c.client.stub.FindAndModify(clientContext(ctx), &pb.FindAndModifyRequest{
		Database:   c.database,
		Collection: c.name,
		Filter:     &pb.Filter{Data: filterBytes},
		Update:     &pb.Document{Data: updateBytes},
		Options: &pb.FindAndModifyOptions{
			ReturnDocument: returnDoc,
		},
	})
	if err != nil {
		return nil, err
	}

	if resp.Document == nil || len(resp.Document.Data) == 0 {
		return nil, nil
	}
	return decodeBsonDoc(resp.Document.Data)
}

// CreateIndex creates an index on the collection.
func (c *Collection) CreateIndex(ctx context.Context, keys bson.D, unique bool) (string, error) {
	keysBytes, err := encodeBson(keys)
	if err != nil {
		return "", err
	}

	resp, err := c.client.stub.CreateIndex(clientContext(ctx), &pb.CreateIndexRequest{
		Database:   c.database,
		Collection: c.name,
		Keys:       &pb.Document{Data: keysBytes},
		Options: &pb.IndexOptions{
			Unique: &unique,
		},
	})
	if err != nil {
		return "", err
	}
	return resp.IndexName, nil
}
