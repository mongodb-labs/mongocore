package mongocore

import (
	"context"
	"fmt"

	pb "github.com/rozza/mongocore/clients/go/proto"
	"go.mongodb.org/mongo-driver/v2/bson"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
)

// IngestOptions configures an ingestion job.
type IngestOptions struct {
	FilePath         string
	Database         string
	Collection       string
	Format           pb.FileFormat
	DedupKey         []string
	ConflictStrategy pb.ConflictStrategy
	BatchSize        int32
	Concurrency      int32
	Expressions      []string
	SchemaOverrides  map[string]string
	SampleSize       int32
	CsvOptions       *pb.CsvOptions
}

// IngestResult is the result of starting an ingestion job.
type IngestResult struct {
	JobID          string
	Status         pb.IngestJobStatus
	InferredSchema map[string]string
	TotalRows      int64
}

// IngestStatusResult is the detailed status of an ingestion job.
type IngestStatusResult struct {
	JobID                string
	Status               pb.IngestJobStatus
	TotalRows            int64
	RowsProcessed        int64
	RowsInserted         int64
	RowsSkipped          int64
	RowsFailed           int64
	ElapsedMs            int64
	EstimatedRemainingMs int64
}

// IngestJobSummary is a summary of an ingestion job.
type IngestJobSummary struct {
	JobID         string
	FilePath      string
	Database      string
	Collection    string
	Status        pb.IngestJobStatus
	TotalRows     int64
	RowsProcessed int64
}

// WatchDirectoryOptions configures directory watching.
type WatchDirectoryOptions struct {
	Path             string
	FilePattern      string
	Database         string
	Collection       string
	ConflictStrategy pb.ConflictStrategy
	DedupKey         []string
}

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

// RunCommand executes an arbitrary MongoDB command via raw passthrough.
func (c *Client) RunCommand(ctx context.Context, database string, command interface{}, allowAll bool) (interface{}, error) {
	commandBytes, err := bson.Marshal(command)
	if err != nil {
		return nil, fmt.Errorf("mongocore: failed to marshal command: %w", err)
	}

	resp, err := c.stub.RunCommand(ctx, &pb.RunCommandRequest{
		Database: database,
		Command:  &pb.Document{Data: commandBytes},
		AllowAll: allowAll,
	})
	if err != nil {
		return nil, err
	}

	var result interface{}
	if err := bson.Unmarshal(resp.Result.Data, &result); err != nil {
		return nil, fmt.Errorf("mongocore: failed to unmarshal result: %w", err)
	}
	return result, nil
}

// Ingest starts an ingestion job to load a file into MongoDB.
func (c *Client) Ingest(ctx context.Context, opts IngestOptions) (*IngestResult, error) {
	resp, err := c.stub.Ingest(ctx, &pb.IngestRequest{
		FilePath:         opts.FilePath,
		Database:         opts.Database,
		Collection:       opts.Collection,
		Format:           opts.Format,
		DedupKey:         opts.DedupKey,
		ConflictStrategy: opts.ConflictStrategy,
		BatchSize:        opts.BatchSize,
		Concurrency:      opts.Concurrency,
		Expressions:      opts.Expressions,
		SchemaOverrides:  opts.SchemaOverrides,
		SampleSize:       opts.SampleSize,
		CsvOptions:       opts.CsvOptions,
	})
	if err != nil {
		return nil, err
	}
	return &IngestResult{
		JobID:          resp.JobId,
		Status:         resp.Status,
		InferredSchema: resp.InferredSchema,
		TotalRows:      resp.TotalRows,
	}, nil
}

// IngestStatus returns the current status of an ingestion job.
func (c *Client) IngestStatus(ctx context.Context, jobID string) (*IngestStatusResult, error) {
	resp, err := c.stub.GetIngestStatus(ctx, &pb.GetIngestStatusRequest{
		JobId: jobID,
	})
	if err != nil {
		return nil, err
	}
	return &IngestStatusResult{
		JobID:                resp.JobId,
		Status:               resp.Status,
		TotalRows:            resp.TotalRows,
		RowsProcessed:        resp.RowsProcessed,
		RowsInserted:         resp.RowsInserted,
		RowsSkipped:          resp.RowsSkipped,
		RowsFailed:           resp.RowsFailed,
		ElapsedMs:            resp.ElapsedMs,
		EstimatedRemainingMs: resp.EstimatedRemainingMs,
	}, nil
}

// ListIngestJobs returns all ingestion jobs.
func (c *Client) ListIngestJobs(ctx context.Context) ([]IngestJobSummary, error) {
	resp, err := c.stub.ListIngestJobs(ctx, &pb.ListIngestJobsRequest{})
	if err != nil {
		return nil, err
	}
	jobs := make([]IngestJobSummary, len(resp.Jobs))
	for i, j := range resp.Jobs {
		jobs[i] = IngestJobSummary{
			JobID:         j.JobId,
			FilePath:      j.FilePath,
			Database:      j.Database,
			Collection:    j.Collection,
			Status:        j.Status,
			TotalRows:     j.TotalRows,
			RowsProcessed: j.RowsProcessed,
		}
	}
	return jobs, nil
}

// CancelIngest cancels an in-progress ingestion job.
func (c *Client) CancelIngest(ctx context.Context, jobID string) (bool, error) {
	resp, err := c.stub.CancelIngest(ctx, &pb.CancelIngestRequest{
		JobId: jobID,
	})
	if err != nil {
		return false, err
	}
	return resp.Success, nil
}

// WatchDirectory starts watching a directory for new files and automatically ingests them.
func (c *Client) WatchDirectory(ctx context.Context, opts WatchDirectoryOptions) (string, error) {
	resp, err := c.stub.WatchDirectory(ctx, &pb.WatchDirectoryRequest{
		Path:             opts.Path,
		FilePattern:      opts.FilePattern,
		Database:         opts.Database,
		Collection:       opts.Collection,
		ConflictStrategy: opts.ConflictStrategy,
		DedupKey:         opts.DedupKey,
	})
	if err != nil {
		return "", err
	}
	return resp.WatchId, nil
}

// StopWatch stops a previously started directory watch.
func (c *Client) StopWatch(ctx context.Context, watchID string) (bool, error) {
	resp, err := c.stub.StopWatch(ctx, &pb.StopWatchRequest{
		WatchId: watchID,
	})
	if err != nil {
		return false, err
	}
	return resp.Success, nil
}
