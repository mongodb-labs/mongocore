/**
 * Pipeline operation builders for batch execution.
 * Each function returns an operation object that can be passed to client.pipeline().
 */

import type { Document } from './types';

export interface FindOp {
  type: 'find';
  database: string;
  collection: string;
  filter?: Document;
  limit?: number;
  skip?: number;
}

export interface FindOneOp {
  type: 'findOne';
  database: string;
  collection: string;
  filter?: Document;
}

export interface InsertOp {
  type: 'insert';
  database: string;
  collection: string;
  document: Document;
}

export interface InsertManyOp {
  type: 'insertMany';
  database: string;
  collection: string;
  documents: Document[];
}

export interface UpdateOp {
  type: 'update';
  database: string;
  collection: string;
  filter: Document;
  update: Document;
}

export interface UpdateManyOp {
  type: 'updateMany';
  database: string;
  collection: string;
  filter: Document;
  update: Document;
}

export interface DeleteOp {
  type: 'delete';
  database: string;
  collection: string;
  filter: Document;
}

export interface DeleteManyOp {
  type: 'deleteMany';
  database: string;
  collection: string;
  filter: Document;
}

export interface AggregateOp {
  type: 'aggregate';
  database: string;
  collection: string;
  pipeline: Document[];
}

export interface RunCommandOp {
  type: 'runCommand';
  database: string;
  command: Document;
  allowAll?: boolean;
}

export interface ListDatabasesOp {
  type: 'listDatabases';
}

export interface ListCollectionsOp {
  type: 'listCollections';
  database: string;
}

export type PipelineOp =
  | FindOp
  | FindOneOp
  | InsertOp
  | InsertManyOp
  | UpdateOp
  | UpdateManyOp
  | DeleteOp
  | DeleteManyOp
  | AggregateOp
  | RunCommandOp
  | ListDatabasesOp
  | ListCollectionsOp;

// Builder functions

export function find(
  database: string,
  collection: string,
  filter?: Document,
  options?: { limit?: number; skip?: number }
): FindOp {
  return {
    type: 'find',
    database,
    collection,
    filter,
    limit: options?.limit,
    skip: options?.skip,
  };
}

export function findOne(
  database: string,
  collection: string,
  filter?: Document
): FindOneOp {
  return {
    type: 'findOne',
    database,
    collection,
    filter,
  };
}

export function insert(
  database: string,
  collection: string,
  document: Document
): InsertOp {
  return {
    type: 'insert',
    database,
    collection,
    document,
  };
}

export function insertMany(
  database: string,
  collection: string,
  documents: Document[]
): InsertManyOp {
  return {
    type: 'insertMany',
    database,
    collection,
    documents,
  };
}

export function update(
  database: string,
  collection: string,
  filter: Document,
  updateDoc: Document
): UpdateOp {
  return {
    type: 'update',
    database,
    collection,
    filter,
    update: updateDoc,
  };
}

export function updateMany(
  database: string,
  collection: string,
  filter: Document,
  updateDoc: Document
): UpdateManyOp {
  return {
    type: 'updateMany',
    database,
    collection,
    filter,
    update: updateDoc,
  };
}

export function deleteFn(
  database: string,
  collection: string,
  filter: Document
): DeleteOp {
  return {
    type: 'delete',
    database,
    collection,
    filter,
  };
}

export function deleteMany(
  database: string,
  collection: string,
  filter: Document
): DeleteManyOp {
  return {
    type: 'deleteMany',
    database,
    collection,
    filter,
  };
}

export function aggregate(
  database: string,
  collection: string,
  pipeline: Document[]
): AggregateOp {
  return {
    type: 'aggregate',
    database,
    collection,
    pipeline,
  };
}

export function runCommand(
  database: string,
  command: Document,
  allowAll?: boolean
): RunCommandOp {
  return {
    type: 'runCommand',
    database,
    command,
    allowAll,
  };
}

export function listDatabases(): ListDatabasesOp {
  return {
    type: 'listDatabases',
  };
}

export function listCollections(database: string): ListCollectionsOp {
  return {
    type: 'listCollections',
    database,
  };
}
