package mongocore

import (
	"context"
	"io"

	pb "github.com/rozza/mongocore/clients/go/proto"
	"go.mongodb.org/mongo-driver/v2/bson"
)

// batchStream is the interface shared by both FindStream and AggregateStream clients.
// Both return DocumentBatch messages via Recv().
type batchStream interface {
	Recv() (*pb.DocumentBatch, error)
}

// Cursor iterates over documents from a streaming gRPC call.
// The underlying RPC is called on first Next() invocation (lazy).
// Must be closed when done to release resources.
type Cursor struct {
	stream   batchStream
	cancelFn context.CancelFunc
	buffer   []bson.D
	index    int
	done     bool
	err      error

	// Lazy init fields
	initFn func(ctx context.Context) (batchStream, context.CancelFunc, error)
}

// Next advances the cursor to the next document.
// Returns true if a document is available via Doc(), false when exhausted or on error.
func (c *Cursor) Next(ctx context.Context) bool {
	if c.err != nil || c.done {
		return false
	}

	// Lazy initialization
	if c.stream == nil {
		stream, cancel, err := c.initFn(ctx)
		if err != nil {
			c.err = err
			return false
		}
		c.stream = stream
		c.cancelFn = cancel
	}

	// Try buffer first
	if c.index < len(c.buffer) {
		return true
	}

	// Fetch next batch via the batchStream interface
	batch, err := c.stream.Recv()
	if err != nil {
		if err == io.EOF {
			c.done = true
		} else {
			c.err = err
		}
		return false
	}

	c.buffer = make([]bson.D, 0, len(batch.Documents))
	for _, d := range batch.Documents {
		doc, err := decodeBsonDoc(d.Data)
		if err != nil {
			c.err = err
			return false
		}
		c.buffer = append(c.buffer, doc)
	}
	c.index = 0

	if !batch.HasMore {
		c.done = true
	}

	return len(c.buffer) > 0
}

// Doc returns the current document. Must only be called after Next() returns true.
func (c *Cursor) Doc() bson.D {
	doc := c.buffer[c.index]
	c.index++
	return doc
}

// Err returns any error that occurred during iteration.
func (c *Cursor) Err() error {
	return c.err
}

// Close cancels the underlying stream and releases resources.
func (c *Cursor) Close() error {
	if c.cancelFn != nil {
		c.cancelFn()
	}
	c.done = true
	return nil
}

// All collects all remaining documents into a slice.
func (c *Cursor) All(ctx context.Context) ([]bson.D, error) {
	var results []bson.D
	for c.Next(ctx) {
		results = append(results, c.Doc())
	}
	if c.err != nil {
		return nil, c.err
	}
	return results, nil
}

// Ensure Cursor implements io.Closer.
var _ io.Closer = (*Cursor)(nil)
