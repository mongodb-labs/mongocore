export interface FindOptions {
  limit?: number;
  skip?: number;
  sort?: Record<string, 1 | -1>;
  projection?: Record<string, 0 | 1>;
}

export interface UpdateResult {
  matchedCount: number;
  modifiedCount: number;
  upsertedId?: string;
}

export interface InsertResult {
  insertedId: string;
}

export interface InsertManyResult {
  insertedIds: string[];
  insertedCount: number;
}

export interface Document {
  [key: string]: any;
}

export interface SearchResult {
  documents: Document[];
  method: string;
  total: number;
}

export interface ChangeEvent {
  operationType: string;
  database?: string;
  collection?: string;
  document?: Document;
  updateDescription?: Document;
  documentKey?: Document;
}

export interface FindAndModifyOptions {
  returnDocument?: 'before' | 'after';
  upsert?: boolean;
  sort?: Record<string, 1 | -1>;
}

export interface FindAndModifyResult {
  document: Document | null;
}

export interface CreateIndexOptions {
  name?: string;
  unique?: boolean;
  sparse?: boolean;
}

export interface CreateIndexResult {
  indexName: string;
}

export interface Analytics {
  totalOperations: number;
  totalErrors: number;
  errorRate: number;
  p50LatencyMs: number;
  p95LatencyMs: number;
  p99LatencyMs: number;
  topOperations: Array<{ operation: string; count: number }>;
  topCollections: Array<{ collection: string; count: number }>;
}

export interface PipelineResult {
  index: number;
  success: boolean;
  error?: string;
  result?: any;
}

export interface TransactionStepResult {
  stepName: string;
  success: boolean;
  error?: string;
  documents?: Document[];
  matchedCount?: number;
  modifiedCount?: number;
  deletedCount?: number;
  insertedId?: string;
  insertedIds?: string[];
}

export interface TransactionPipelineResult {
  committed: boolean;
  results: TransactionStepResult[];
  error?: string;
}
