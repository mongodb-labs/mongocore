package mongocore

import (
	"context"
	"fmt"

	pb "github.com/rozza/mongocore/clients/go/proto"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
)

// Client connects to a MongoCore sidecar.
type Client struct {
	address string
	conn    *grpc.ClientConn
	stub    pb.MongoCoreClient
}

// NewClient creates a new MongoCore client.
func NewClient(address string) *Client {
	return &Client{address: address}
}

// Connect establishes the gRPC connection.
func (c *Client) Connect(ctx context.Context) error {
	conn, err := grpc.NewClient(c.address, grpc.WithTransportCredentials(insecure.NewCredentials()))
	if err != nil {
		return fmt.Errorf("mongocore: failed to connect: %w", err)
	}
	c.conn = conn
	c.stub = pb.NewMongoCoreClient(conn)
	return nil
}

// Close closes the connection.
func (c *Client) Close() error {
	if c.conn != nil {
		return c.conn.Close()
	}
	return nil
}

// Database returns a database handle.
func (c *Client) Database(name string) *Database {
	return &Database{client: c, name: name}
}

// Stub returns the gRPC stub for direct access.
func (c *Client) Stub() pb.MongoCoreClient {
	return c.stub
}

// ListDatabases returns all database names.
func (c *Client) ListDatabases(ctx context.Context) ([]string, error) {
	resp, err := c.stub.ListDatabases(ctx, &pb.ListDatabasesRequest{})
	if err != nil {
		return nil, err
	}
	return resp.Databases, nil
}
