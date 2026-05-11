package mongocore

import (
	"context"
	"fmt"

	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
)

// Client connects to a MongoCore sidecar.
type Client struct {
	address string
	conn    *grpc.ClientConn
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

// Connection returns the underlying gRPC connection.
func (c *Client) Connection() *grpc.ClientConn {
	return c.conn
}
