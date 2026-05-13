package ops

import (
	"fmt"

	pb "github.com/rozza/mongocore/clients/go/proto"
	"go.mongodb.org/mongo-driver/v2/bson"
)

// Helper function to encode BSON documents
func encodeBson(doc interface{}) ([]byte, error) {
	return bson.Marshal(doc)
}

// Find creates a Find operation for use in a pipeline.
func Find(database, collection string, filter bson.D, options *pb.FindOptions) (*pb.PipelineOperation, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return nil, fmt.Errorf("failed to encode filter: %w", err)
	}

	return &pb.PipelineOperation{
		Operation: &pb.PipelineOperation_Find{
			Find: &pb.FindRequest{
				Database:   database,
				Collection: collection,
				Filter:     &pb.Filter{Data: filterBytes},
				Options:    options,
			},
		},
	}, nil
}

// FindOne creates a FindOne operation for use in a pipeline.
func FindOne(database, collection string, filter bson.D) (*pb.PipelineOperation, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return nil, fmt.Errorf("failed to encode filter: %w", err)
	}

	return &pb.PipelineOperation{
		Operation: &pb.PipelineOperation_FindOne{
			FindOne: &pb.FindOneRequest{
				Database:   database,
				Collection: collection,
				Filter:     &pb.Filter{Data: filterBytes},
			},
		},
	}, nil
}

// Insert creates an Insert operation for use in a pipeline.
func Insert(database, collection string, document bson.D) (*pb.PipelineOperation, error) {
	docBytes, err := encodeBson(document)
	if err != nil {
		return nil, fmt.Errorf("failed to encode document: %w", err)
	}

	return &pb.PipelineOperation{
		Operation: &pb.PipelineOperation_Insert{
			Insert: &pb.InsertRequest{
				Database:   database,
				Collection: collection,
				Document:   &pb.Document{Data: docBytes},
			},
		},
	}, nil
}

// InsertMany creates an InsertMany operation for use in a pipeline.
func InsertMany(database, collection string, documents []bson.D) (*pb.PipelineOperation, error) {
	pbDocs := make([]*pb.Document, 0, len(documents))
	for _, doc := range documents {
		docBytes, err := encodeBson(doc)
		if err != nil {
			return nil, fmt.Errorf("failed to encode document: %w", err)
		}
		pbDocs = append(pbDocs, &pb.Document{Data: docBytes})
	}

	return &pb.PipelineOperation{
		Operation: &pb.PipelineOperation_InsertMany{
			InsertMany: &pb.InsertManyRequest{
				Database:   database,
				Collection: collection,
				Documents:  pbDocs,
			},
		},
	}, nil
}

// Update creates an Update operation for use in a pipeline.
func Update(database, collection string, filter, update bson.D, upsert bool) (*pb.PipelineOperation, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return nil, fmt.Errorf("failed to encode filter: %w", err)
	}
	updateBytes, err := encodeBson(update)
	if err != nil {
		return nil, fmt.Errorf("failed to encode update: %w", err)
	}

	return &pb.PipelineOperation{
		Operation: &pb.PipelineOperation_Update{
			Update: &pb.UpdateRequest{
				Database:   database,
				Collection: collection,
				Filter:     &pb.Filter{Data: filterBytes},
				Update:     &pb.Document{Data: updateBytes},
				Upsert:     upsert,
			},
		},
	}, nil
}

// UpdateMany creates an UpdateMany operation for use in a pipeline.
func UpdateMany(database, collection string, filter, update bson.D, upsert bool) (*pb.PipelineOperation, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return nil, fmt.Errorf("failed to encode filter: %w", err)
	}
	updateBytes, err := encodeBson(update)
	if err != nil {
		return nil, fmt.Errorf("failed to encode update: %w", err)
	}

	return &pb.PipelineOperation{
		Operation: &pb.PipelineOperation_UpdateMany{
			UpdateMany: &pb.UpdateManyRequest{
				Database:   database,
				Collection: collection,
				Filter:     &pb.Filter{Data: filterBytes},
				Update:     &pb.Document{Data: updateBytes},
				Upsert:     upsert,
			},
		},
	}, nil
}

// Delete creates a Delete operation for use in a pipeline.
func Delete(database, collection string, filter bson.D) (*pb.PipelineOperation, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return nil, fmt.Errorf("failed to encode filter: %w", err)
	}

	return &pb.PipelineOperation{
		Operation: &pb.PipelineOperation_Delete{
			Delete: &pb.DeleteRequest{
				Database:   database,
				Collection: collection,
				Filter:     &pb.Filter{Data: filterBytes},
			},
		},
	}, nil
}

// DeleteMany creates a DeleteMany operation for use in a pipeline.
func DeleteMany(database, collection string, filter bson.D) (*pb.PipelineOperation, error) {
	filterBytes, err := encodeBson(filter)
	if err != nil {
		return nil, fmt.Errorf("failed to encode filter: %w", err)
	}

	return &pb.PipelineOperation{
		Operation: &pb.PipelineOperation_DeleteMany{
			DeleteMany: &pb.DeleteManyRequest{
				Database:   database,
				Collection: collection,
				Filter:     &pb.Filter{Data: filterBytes},
			},
		},
	}, nil
}

// Aggregate creates an Aggregate operation for use in a pipeline.
func Aggregate(database, collection string, pipeline []bson.D) (*pb.PipelineOperation, error) {
	stages := make([][]byte, 0, len(pipeline))
	for _, stage := range pipeline {
		stageBytes, err := encodeBson(stage)
		if err != nil {
			return nil, fmt.Errorf("failed to encode pipeline stage: %w", err)
		}
		stages = append(stages, stageBytes)
	}

	return &pb.PipelineOperation{
		Operation: &pb.PipelineOperation_Aggregate{
			Aggregate: &pb.AggregateRequest{
				Database:   database,
				Collection: collection,
				Pipeline:   &pb.Pipeline{Stages: stages},
			},
		},
	}, nil
}

// RunCommand creates a RunCommand operation for use in a pipeline.
func RunCommand(database string, command bson.D, allowAll bool) (*pb.PipelineOperation, error) {
	commandBytes, err := encodeBson(command)
	if err != nil {
		return nil, fmt.Errorf("failed to encode command: %w", err)
	}

	return &pb.PipelineOperation{
		Operation: &pb.PipelineOperation_RunCommand{
			RunCommand: &pb.RunCommandRequest{
				Database: database,
				Command:  &pb.Document{Data: commandBytes},
				AllowAll: allowAll,
			},
		},
	}, nil
}

// ListDatabases creates a ListDatabases operation for use in a pipeline.
func ListDatabases() *pb.PipelineOperation {
	return &pb.PipelineOperation{
		Operation: &pb.PipelineOperation_ListDatabases{
			ListDatabases: &pb.ListDatabasesRequest{},
		},
	}
}

// ListCollections creates a ListCollections operation for use in a pipeline.
func ListCollections(database string) *pb.PipelineOperation {
	return &pb.PipelineOperation{
		Operation: &pb.PipelineOperation_ListCollections{
			ListCollections: &pb.ListCollectionsRequest{
				Database: database,
			},
		},
	}
}
