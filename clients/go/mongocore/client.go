package mongocore

import (
	"context"
	"fmt"
	"os"

	pb "github.com/rozza/mongocore/clients/go/proto"
	"go.mongodb.org/mongo-driver/v2/bson"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/metadata"
)

const (
	DefaultSocketPath = "/tmp/mongocore.sock"
	DefaultAddress    = "localhost:50051"
)

// clientContext adds the x-client-language metadata to the context.
func clientContext(ctx context.Context) context.Context {
	return metadata.AppendToOutgoingContext(ctx, "x-client-language", "go")
}

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
	address    string
	socketPath string
	conn       *grpc.ClientConn
	stub       pb.MongoCoreClient
	Transport  string // "uds" or "tcp" after Connect()
}

// MongoClient creates a client that auto-discovers the transport (zero-config).
// Priority: MONGOCORE_SOCKET_PATH env → /tmp/mongocore.sock → MONGOCORE_ADDRESS env → localhost:50051
func MongoClient() *Client {
	return &Client{}
}

// MongoClientTCP creates a client with an explicit TCP address.
func MongoClientTCP(address string) *Client {
	return &Client{address: address}
}

// MongoClientWithSocket creates a client with an explicit UDS path.
func MongoClientWithSocket(socketPath string) *Client {
	return &Client{socketPath: socketPath}
}


func (c *Client) resolveTarget() string {
	if c.socketPath != "" {
		c.Transport = "uds"
		return "unix://" + c.socketPath
	}
	if c.address != "" {
		c.Transport = "tcp"
		return c.address
	}
	if envSocket := os.Getenv("MONGOCORE_SOCKET_PATH"); envSocket != "" {
		c.Transport = "uds"
		return "unix://" + envSocket
	}
	if _, err := os.Stat(DefaultSocketPath); err == nil {
		c.Transport = "uds"
		return "unix://" + DefaultSocketPath
	}
	if envAddr := os.Getenv("MONGOCORE_ADDRESS"); envAddr != "" {
		c.Transport = "tcp"
		return envAddr
	}
	c.Transport = "tcp"
	return DefaultAddress
}

// Connect establishes the gRPC connection.
func (c *Client) Connect(ctx context.Context) error {
	target := c.resolveTarget()
	conn, err := grpc.NewClient(target,
		grpc.WithTransportCredentials(insecure.NewCredentials()),
		grpc.WithDefaultCallOptions(
			grpc.MaxCallRecvMsgSize(64*1024*1024),
			grpc.MaxCallSendMsgSize(64*1024*1024),
		),
	)
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
	resp, err := c.stub.ListDatabases(clientContext(ctx), &pb.ListDatabasesRequest{})
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

	resp, err := c.stub.RunCommand(clientContext(ctx), &pb.RunCommandRequest{
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
	resp, err := c.stub.Ingest(clientContext(ctx), &pb.IngestRequest{
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
	resp, err := c.stub.GetIngestStatus(clientContext(ctx), &pb.GetIngestStatusRequest{
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
	resp, err := c.stub.ListIngestJobs(clientContext(ctx), &pb.ListIngestJobsRequest{})
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
	resp, err := c.stub.CancelIngest(clientContext(ctx), &pb.CancelIngestRequest{
		JobId: jobID,
	})
	if err != nil {
		return false, err
	}
	return resp.Success, nil
}

// WatchDirectory starts watching a directory for new files and automatically ingests them.
func (c *Client) WatchDirectory(ctx context.Context, opts WatchDirectoryOptions) (string, error) {
	resp, err := c.stub.WatchDirectory(clientContext(ctx), &pb.WatchDirectoryRequest{
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
	resp, err := c.stub.StopWatch(clientContext(ctx), &pb.StopWatchRequest{
		WatchId: watchID,
	})
	if err != nil {
		return false, err
	}
	return resp.Success, nil
}

// BeginTransaction starts a new transaction.
func (c *Client) BeginTransaction(ctx context.Context, database string) (string, error) {
	resp, err := c.stub.BeginTransaction(clientContext(ctx), &pb.BeginTransactionRequest{
		Database: database,
	})
	if err != nil {
		return "", err
	}
	return resp.TransactionId, nil
}

// CommitTransaction commits an active transaction.
func (c *Client) CommitTransaction(ctx context.Context, transactionID string) error {
	_, err := c.stub.CommitTransaction(clientContext(ctx), &pb.CommitTransactionRequest{
		TransactionId: transactionID,
	})
	return err
}

// AbortTransaction aborts an active transaction.
func (c *Client) AbortTransaction(ctx context.Context, transactionID string) error {
	_, err := c.stub.AbortTransaction(clientContext(ctx), &pb.AbortTransactionRequest{
		TransactionId: transactionID,
	})
	return err
}

// AnalyticsData contains aggregated analytics data.
type AnalyticsData struct {
	TotalOperations int64
	TotalErrors     int64
	ErrorRate       float64
	P50LatencyMs    float64
	P95LatencyMs    float64
	P99LatencyMs    float64
	TopOperations   []*pb.OperationCount
	TopCollections  []*pb.CollectionCount
}

// GetAnalytics returns aggregated analytics data for recent operations.
func (c *Client) GetAnalytics(ctx context.Context) (*AnalyticsData, error) {
	resp, err := c.stub.GetAnalytics(clientContext(ctx), &pb.GetAnalyticsRequest{
		WindowSeconds: 60, // Default to last 60 seconds
	})
	if err != nil {
		return nil, err
	}
	return &AnalyticsData{
		TotalOperations: resp.TotalOperations,
		TotalErrors:     resp.TotalErrors,
		ErrorRate:       resp.ErrorRate,
		P50LatencyMs:    resp.P50LatencyMs,
		P95LatencyMs:    resp.P95LatencyMs,
		P99LatencyMs:    resp.P99LatencyMs,
		TopOperations:   resp.TopOperations,
		TopCollections:  resp.TopCollections,
	}, nil
}

// PipelineResult represents a single result from a pipeline execution.
type PipelineResult struct {
	Index   uint32
	Success bool
	Error   string
	Raw     *pb.PipelineResult
}

// AsFind returns the result as a FindResponse if it was a Find operation.
func (r *PipelineResult) AsFind() (*pb.FindResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.PipelineResult_Find); ok {
		return resp.Find, true
	}
	return nil, false
}

// AsFindOne returns the result as a FindOneResponse if it was a FindOne operation.
func (r *PipelineResult) AsFindOne() (*pb.FindOneResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.PipelineResult_FindOne); ok {
		return resp.FindOne, true
	}
	return nil, false
}

// AsInsert returns the result as an InsertResponse if it was an Insert operation.
func (r *PipelineResult) AsInsert() (*pb.InsertResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.PipelineResult_Insert); ok {
		return resp.Insert, true
	}
	return nil, false
}

// AsInsertMany returns the result as an InsertManyResponse if it was an InsertMany operation.
func (r *PipelineResult) AsInsertMany() (*pb.InsertManyResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.PipelineResult_InsertMany); ok {
		return resp.InsertMany, true
	}
	return nil, false
}

// AsUpdate returns the result as an UpdateResponse if it was an Update operation.
func (r *PipelineResult) AsUpdate() (*pb.UpdateResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.PipelineResult_Update); ok {
		return resp.Update, true
	}
	return nil, false
}

// AsUpdateMany returns the result as an UpdateManyResponse if it was an UpdateMany operation.
func (r *PipelineResult) AsUpdateMany() (*pb.UpdateManyResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.PipelineResult_UpdateMany); ok {
		return resp.UpdateMany, true
	}
	return nil, false
}

// AsDelete returns the result as a DeleteResponse if it was a Delete operation.
func (r *PipelineResult) AsDelete() (*pb.DeleteResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.PipelineResult_Delete); ok {
		return resp.Delete, true
	}
	return nil, false
}

// AsDeleteMany returns the result as a DeleteManyResponse if it was a DeleteMany operation.
func (r *PipelineResult) AsDeleteMany() (*pb.DeleteManyResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.PipelineResult_DeleteMany); ok {
		return resp.DeleteMany, true
	}
	return nil, false
}

// AsAggregate returns the result as an AggregateResponse if it was an Aggregate operation.
func (r *PipelineResult) AsAggregate() (*pb.AggregateResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.PipelineResult_Aggregate); ok {
		return resp.Aggregate, true
	}
	return nil, false
}

// AsRunCommand returns the result as a RunCommandResponse if it was a RunCommand operation.
func (r *PipelineResult) AsRunCommand() (*pb.RunCommandResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.PipelineResult_RunCommand); ok {
		return resp.RunCommand, true
	}
	return nil, false
}

// AsListDatabases returns the result as a ListDatabasesResponse if it was a ListDatabases operation.
func (r *PipelineResult) AsListDatabases() (*pb.ListDatabasesResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.PipelineResult_ListDatabases); ok {
		return resp.ListDatabases, true
	}
	return nil, false
}

// AsListCollections returns the result as a ListCollectionsResponse if it was a ListCollections operation.
func (r *PipelineResult) AsListCollections() (*pb.ListCollectionsResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.PipelineResult_ListCollections); ok {
		return resp.ListCollections, true
	}
	return nil, false
}

// AsError returns the error if the operation failed.
func (r *PipelineResult) AsError() (*pb.PipelineError, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if errResp, ok := r.Raw.Result.(*pb.PipelineResult_Error); ok {
		return errResp.Error, true
	}
	return nil, false
}

// Pipeline executes a sequence of operations atomically (or as a batch).
func (c *Client) Pipeline(ctx context.Context, operations ...*pb.PipelineOperation) ([]PipelineResult, error) {
	resp, err := c.stub.Pipeline(clientContext(ctx), &pb.PipelineRequest{
		Operations: operations,
	})
	if err != nil {
		return nil, err
	}

	results := make([]PipelineResult, len(resp.Results))
	for i, res := range resp.Results {
		results[i] = PipelineResult{
			Index:   res.Index,
			Success: res.Result != nil,
			Raw:     res,
		}
		if pipeErr, ok := results[i].AsError(); ok {
			results[i].Success = false
			results[i].Error = pipeErr.Message
		}
	}
	return results, nil
}
