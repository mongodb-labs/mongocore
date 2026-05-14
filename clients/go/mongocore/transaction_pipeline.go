package mongocore

import (
	"context"

	pb "github.com/rozza/mongocore/clients/go/proto"
	"go.mongodb.org/mongo-driver/v2/bson"
)

// TransactionStep represents a single step in a transaction pipeline.
type TransactionStep struct {
	proto *pb.TransactionStep
}

// TransactionPipelineOptions configures the transaction pipeline execution.
type TransactionPipelineOptions struct {
	ReadConcern  string
	WriteConcern string
	MaxTimeMs    uint64
}

// TransactionStepResult represents the result of a single transaction step.
type TransactionStepResult struct {
	Name    string
	Success bool
	Raw     *pb.TransactionStepResult
}

// AsFind returns the FindResponse if this step was a Find operation.
func (r *TransactionStepResult) AsFind() (*pb.FindResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.TransactionStepResult_FindResult); ok {
		return resp.FindResult, true
	}
	return nil, false
}

// AsFindOne returns the FindOneResponse if this step was a FindOne operation.
func (r *TransactionStepResult) AsFindOne() (*pb.FindOneResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.TransactionStepResult_FindOneResult); ok {
		return resp.FindOneResult, true
	}
	return nil, false
}

// AsInsert returns the InsertResponse if this step was an Insert operation.
func (r *TransactionStepResult) AsInsert() (*pb.InsertResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.TransactionStepResult_InsertResult); ok {
		return resp.InsertResult, true
	}
	return nil, false
}

// AsInsertMany returns the InsertManyResponse if this step was an InsertMany operation.
func (r *TransactionStepResult) AsInsertMany() (*pb.InsertManyResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.TransactionStepResult_InsertManyResult); ok {
		return resp.InsertManyResult, true
	}
	return nil, false
}

// AsUpdate returns the UpdateResponse if this step was an Update operation.
func (r *TransactionStepResult) AsUpdate() (*pb.UpdateResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.TransactionStepResult_UpdateResult); ok {
		return resp.UpdateResult, true
	}
	return nil, false
}

// AsDelete returns the DeleteResponse if this step was a Delete operation.
func (r *TransactionStepResult) AsDelete() (*pb.DeleteResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.TransactionStepResult_DeleteResult); ok {
		return resp.DeleteResult, true
	}
	return nil, false
}

// AsDeleteMany returns the DeleteManyResponse if this step was a DeleteMany operation.
func (r *TransactionStepResult) AsDeleteMany() (*pb.DeleteManyResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.TransactionStepResult_DeleteManyResult); ok {
		return resp.DeleteManyResult, true
	}
	return nil, false
}

// AsAggregate returns the AggregateResponse if this step was an Aggregate operation.
func (r *TransactionStepResult) AsAggregate() (*pb.AggregateResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.TransactionStepResult_AggregateResult); ok {
		return resp.AggregateResult, true
	}
	return nil, false
}

// AsFindAndModify returns the FindAndModifyResponse if this step was a FindAndModify operation.
func (r *TransactionStepResult) AsFindAndModify() (*pb.FindAndModifyResponse, bool) {
	if r.Raw == nil {
		return nil, false
	}
	if resp, ok := r.Raw.Result.(*pb.TransactionStepResult_FindAndModifyResult); ok {
		return resp.FindAndModifyResult, true
	}
	return nil, false
}

// TransactionPipelineResult contains the full result of a transaction pipeline execution.
type TransactionPipelineResult struct {
	Steps          []TransactionStepResult
	TotalSteps     uint32
	StepsCompleted uint32
	ElapsedMs      uint64
}

// --- Builder Functions ---

// NewFindOneStep creates a transaction step for a FindOne operation.
func NewFindOneStep(name, database, collection string, filter bson.D) (*TransactionStep, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return nil, err
	}
	return &TransactionStep{
		proto: &pb.TransactionStep{
			Name:       name,
			Database:   database,
			Collection: collection,
			Operation: &pb.TransactionStep_FindOne{
				FindOne: &pb.FindOneRequest{
					Database:   database,
					Collection: collection,
					Filter:     &pb.Filter{Data: filterBytes},
				},
			},
		},
	}, nil
}

// NewFindStep creates a transaction step for a Find operation.
func NewFindStep(name, database, collection string, filter bson.D, opts *FindOptions) (*TransactionStep, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return nil, err
	}
	req := &pb.FindRequest{
		Database:   database,
		Collection: collection,
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
	return &TransactionStep{
		proto: &pb.TransactionStep{
			Name:       name,
			Database:   database,
			Collection: collection,
			Operation:  &pb.TransactionStep_Find{Find: req},
		},
	}, nil
}

// NewInsertStep creates a transaction step for an Insert operation.
func NewInsertStep(name, database, collection string, document bson.D) (*TransactionStep, error) {
	docBytes, err := encodeBson(document)
	if err != nil {
		return nil, err
	}
	return &TransactionStep{
		proto: &pb.TransactionStep{
			Name:       name,
			Database:   database,
			Collection: collection,
			Operation: &pb.TransactionStep_Insert{
				Insert: &pb.InsertRequest{
					Database:   database,
					Collection: collection,
					Document:   &pb.Document{Data: docBytes},
				},
			},
		},
	}, nil
}

// NewInsertManyStep creates a transaction step for an InsertMany operation.
func NewInsertManyStep(name, database, collection string, documents []bson.D) (*TransactionStep, error) {
	pbDocs := make([]*pb.Document, 0, len(documents))
	for _, doc := range documents {
		docBytes, err := encodeBson(doc)
		if err != nil {
			return nil, err
		}
		pbDocs = append(pbDocs, &pb.Document{Data: docBytes})
	}
	return &TransactionStep{
		proto: &pb.TransactionStep{
			Name:       name,
			Database:   database,
			Collection: collection,
			Operation: &pb.TransactionStep_InsertMany{
				InsertMany: &pb.InsertManyRequest{
					Database:   database,
					Collection: collection,
					Documents:  pbDocs,
				},
			},
		},
	}, nil
}

// NewUpdateStep creates a transaction step for an Update operation.
func NewUpdateStep(name, database, collection string, filter, update bson.D) (*TransactionStep, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return nil, err
	}
	updateBytes, err := encodeBson(update)
	if err != nil {
		return nil, err
	}
	return &TransactionStep{
		proto: &pb.TransactionStep{
			Name:       name,
			Database:   database,
			Collection: collection,
			Operation: &pb.TransactionStep_Update{
				Update: &pb.UpdateRequest{
					Database:   database,
					Collection: collection,
					Filter:     &pb.Filter{Data: filterBytes},
					Update:     &pb.Document{Data: updateBytes},
				},
			},
		},
	}, nil
}

// NewUpdateManyStep creates a transaction step for an UpdateMany operation.
func NewUpdateManyStep(name, database, collection string, filter, update bson.D) (*TransactionStep, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return nil, err
	}
	updateBytes, err := encodeBson(update)
	if err != nil {
		return nil, err
	}
	return &TransactionStep{
		proto: &pb.TransactionStep{
			Name:       name,
			Database:   database,
			Collection: collection,
			Operation: &pb.TransactionStep_UpdateMany{
				UpdateMany: &pb.UpdateManyRequest{
					Database:   database,
					Collection: collection,
					Filter:     &pb.Filter{Data: filterBytes},
					Update:     &pb.Document{Data: updateBytes},
				},
			},
		},
	}, nil
}

// NewDeleteStep creates a transaction step for a Delete operation.
func NewDeleteStep(name, database, collection string, filter bson.D) (*TransactionStep, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return nil, err
	}
	return &TransactionStep{
		proto: &pb.TransactionStep{
			Name:       name,
			Database:   database,
			Collection: collection,
			Operation: &pb.TransactionStep_Delete{
				Delete: &pb.DeleteRequest{
					Database:   database,
					Collection: collection,
					Filter:     &pb.Filter{Data: filterBytes},
				},
			},
		},
	}, nil
}

// NewDeleteManyStep creates a transaction step for a DeleteMany operation.
func NewDeleteManyStep(name, database, collection string, filter bson.D) (*TransactionStep, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return nil, err
	}
	return &TransactionStep{
		proto: &pb.TransactionStep{
			Name:       name,
			Database:   database,
			Collection: collection,
			Operation: &pb.TransactionStep_DeleteMany{
				DeleteMany: &pb.DeleteManyRequest{
					Database:   database,
					Collection: collection,
					Filter:     &pb.Filter{Data: filterBytes},
				},
			},
		},
	}, nil
}

// NewFindAndModifyStep creates a transaction step for a FindAndModify operation.
func NewFindAndModifyStep(name, database, collection string, filter, update bson.D, returnNew bool) (*TransactionStep, error) {
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
	return &TransactionStep{
		proto: &pb.TransactionStep{
			Name:       name,
			Database:   database,
			Collection: collection,
			Operation: &pb.TransactionStep_FindAndModify{
				FindAndModify: &pb.FindAndModifyRequest{
					Database:   database,
					Collection: collection,
					Filter:     &pb.Filter{Data: filterBytes},
					Update:     &pb.Document{Data: updateBytes},
					Options: &pb.FindAndModifyOptions{
						ReturnDocument: returnDoc,
					},
				},
			},
		},
	}, nil
}

// NewAggregateStep creates a transaction step for an Aggregate operation.
func NewAggregateStep(name, database, collection string, pipeline []bson.D) (*TransactionStep, error) {
	stages := make([][]byte, 0, len(pipeline))
	for _, stage := range pipeline {
		stageBytes, err := encodeBson(stage)
		if err != nil {
			return nil, err
		}
		stages = append(stages, stageBytes)
	}
	return &TransactionStep{
		proto: &pb.TransactionStep{
			Name:       name,
			Database:   database,
			Collection: collection,
			Operation: &pb.TransactionStep_Aggregate{
				Aggregate: &pb.AggregateRequest{
					Database:   database,
					Collection: collection,
					Pipeline:   &pb.Pipeline{Stages: stages},
				},
			},
		},
	}, nil
}

// --- Client Method ---

// TransactionPipeline executes a sequence of operations atomically within a transaction.
// All steps succeed or all are rolled back.
func (c *Client) TransactionPipeline(ctx context.Context, steps []*TransactionStep, opts *TransactionPipelineOptions) (*TransactionPipelineResult, error) {
	protoSteps := make([]*pb.TransactionStep, len(steps))
	for i, s := range steps {
		protoSteps[i] = s.proto
	}

	req := &pb.TransactionPipelineRequest{
		Steps: protoSteps,
	}

	if opts != nil {
		pipeOpts := &pb.TransactionPipelineOptions{}
		if opts.ReadConcern != "" {
			pipeOpts.ReadConcern = &opts.ReadConcern
		}
		if opts.WriteConcern != "" {
			pipeOpts.WriteConcern = &opts.WriteConcern
		}
		if opts.MaxTimeMs > 0 {
			pipeOpts.MaxTimeMs = &opts.MaxTimeMs
		}
		req.Options = pipeOpts
	}

	resp, err := c.stub.TransactionPipeline(clientContext(ctx), req)
	if err != nil {
		return nil, err
	}

	results := make([]TransactionStepResult, len(resp.Steps))
	for i, step := range resp.Steps {
		results[i] = TransactionStepResult{
			Name:    step.Name,
			Success: step.Success,
			Raw:     step,
		}
	}

	result := &TransactionPipelineResult{
		Steps: results,
	}
	if resp.Summary != nil {
		result.TotalSteps = resp.Summary.TotalSteps
		result.StepsCompleted = resp.Summary.StepsCompleted
		result.ElapsedMs = resp.Summary.ElapsedMs
	}

	return result, nil
}
