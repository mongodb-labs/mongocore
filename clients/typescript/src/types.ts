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

export interface ChangeEvent {
  operationType: string;
  database?: string;
  collection?: string;
  document?: Document;
  updateDescription?: Document;
  documentKey?: Document;
}
