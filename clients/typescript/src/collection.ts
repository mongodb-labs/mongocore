import { BSON } from 'bson';
import { MongoClient, CLIENT_METADATA } from './client';
import { EventEmitter } from 'events';
import type { Document, FindOptions, UpdateResult, InsertResult, InsertManyResult, SearchResult, ChangeEvent, FindAndModifyOptions, FindAndModifyResult, CreateIndexOptions, CreateIndexResult, TransactionPipelineResult } from './types';
import type { TransactionStep } from './ops';

export class Collection {
  private client: MongoClient;
  private database: string;
  private name: string;

  constructor(client: MongoClient, database: string, name: string) {
    this.client = client;
    this.database = database;
    this.name = name;
  }

  getName(): string {
    return this.name;
  }

  getDatabase(): string {
    return this.database;
  }

  private encodeBson(doc: Document): Buffer {
    return Buffer.from(BSON.serialize(doc));
  }

  private decodeBson(data: Buffer | Uint8Array): Document {
    return BSON.deserialize(Buffer.from(data)) as Document;
  }

  async find(filter?: Document, options?: FindOptions): Promise<Document[]> {
    return new Promise((resolve, reject) => {
      const request: any = {
        database: this.database,
        collection: this.name,
        filter: { data: this.encodeBson(filter || {}) },
        options: options ? {
          limit: options.limit,
          skip: options.skip,
          sort: options.sort ? this.encodeBson(options.sort as Document) : undefined,
          projection: options.projection ? this.encodeBson(options.projection as Document) : undefined,
        } : undefined,
      };
      this.client.getGrpcClient().find(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        const docs = (response.documents || []).map((d: any) => this.decodeBson(d.data));
        resolve(docs);
      });
    });
  }

  async findOne(filter?: Document): Promise<Document | null> {
    return new Promise((resolve, reject) => {
      const request = {
        database: this.database,
        collection: this.name,
        filter: { data: this.encodeBson(filter || {}) },
      };
      this.client.getGrpcClient().findOne(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        if (response.document && response.document.data && response.document.data.length > 0) {
          resolve(this.decodeBson(response.document.data));
        } else {
          resolve(null);
        }
      });
    });
  }

  async insertOne(document: Document): Promise<InsertResult> {
    return new Promise((resolve, reject) => {
      const request = {
        database: this.database,
        collection: this.name,
        document: { data: this.encodeBson(document) },
      };
      this.client.getGrpcClient().insert(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        resolve({ insertedId: response.insertedId });
      });
    });
  }

  async insertMany(documents: Document[]): Promise<InsertManyResult> {
    return new Promise((resolve, reject) => {
      const request = {
        database: this.database,
        collection: this.name,
        documents: documents.map(d => ({ data: this.encodeBson(d) })),
      };
      this.client.getGrpcClient().insertMany(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        resolve({
          insertedIds: response.insertedIds || [],
          insertedCount: response.insertedCount || 0,
        });
      });
    });
  }

  async updateOne(filter: Document, update: Document): Promise<UpdateResult> {
    return new Promise((resolve, reject) => {
      const request = {
        database: this.database,
        collection: this.name,
        filter: { data: this.encodeBson(filter) },
        update: { data: this.encodeBson(update) },
        upsert: false,
      };
      this.client.getGrpcClient().update(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        resolve({
          matchedCount: response.matchedCount || 0,
          modifiedCount: response.modifiedCount || 0,
          upsertedId: response.upsertedId,
        });
      });
    });
  }

  async updateMany(filter: Document, update: Document): Promise<UpdateResult> {
    return new Promise((resolve, reject) => {
      const request = {
        database: this.database,
        collection: this.name,
        filter: { data: this.encodeBson(filter) },
        update: { data: this.encodeBson(update) },
        upsert: false,
      };
      this.client.getGrpcClient().updateMany(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        resolve({
          matchedCount: response.matchedCount || 0,
          modifiedCount: response.modifiedCount || 0,
          upsertedId: response.upsertedId,
        });
      });
    });
  }

  async deleteOne(filter: Document): Promise<number> {
    return new Promise((resolve, reject) => {
      const request = {
        database: this.database,
        collection: this.name,
        filter: { data: this.encodeBson(filter) },
      };
      this.client.getGrpcClient().delete(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        resolve(response.deletedCount || 0);
      });
    });
  }

  async deleteMany(filter: Document): Promise<number> {
    return new Promise((resolve, reject) => {
      const request = {
        database: this.database,
        collection: this.name,
        filter: { data: this.encodeBson(filter) },
      };
      this.client.getGrpcClient().deleteMany(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        resolve(response.deletedCount || 0);
      });
    });
  }

  async aggregate(pipeline: Document[]): Promise<Document[]> {
    return new Promise((resolve, reject) => {
      const request = {
        database: this.database,
        collection: this.name,
        pipeline: {
          stages: pipeline.map(stage => this.encodeBson(stage)),
        },
      };
      this.client.getGrpcClient().aggregate(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        const docs = (response.documents || []).map((d: any) => this.decodeBson(d.data));
        resolve(docs);
      });
    });
  }

  async search(query: string, limit: number = 10): Promise<SearchResult> {
    return new Promise((resolve, reject) => {
      const request = {
        database: this.database,
        collection: this.name,
        query,
        limit,
      };
      this.client.getGrpcClient().search(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        const docs = (response.documents || []).map((d: any) => this.decodeBson(d.data));
        resolve({
          documents: docs,
          method: response.method,
          total: response.total,
        });
      });
    });
  }

  async findAndModify(filter: Document, update: Document, options?: FindAndModifyOptions): Promise<FindAndModifyResult> {
    return new Promise((resolve, reject) => {
      const request: any = {
        database: this.database,
        collection: this.name,
        filter: { data: this.encodeBson(filter) },
        update: { data: this.encodeBson(update) },
        options: {
          returnDocument: options?.returnDocument === 'after' ? 1 : 0,
          upsert: options?.upsert || false,
          sort: options?.sort ? this.encodeBson(options.sort as Document) : undefined,
        },
      };
      this.client.getGrpcClient().findAndModify(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        const document = response.document?.data?.length > 0
          ? this.decodeBson(response.document.data)
          : null;
        resolve({ document });
      });
    });
  }

  async createIndex(keys: Document, options?: CreateIndexOptions): Promise<CreateIndexResult> {
    return new Promise((resolve, reject) => {
      const request: any = {
        database: this.database,
        collection: this.name,
        keys: { data: this.encodeBson(keys) },
        options: {
          name: options?.name,
          unique: options?.unique,
          sparse: options?.sparse,
        },
      };
      this.client.getGrpcClient().createIndex(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        resolve({ indexName: response.indexName || response.index_name });
      });
    });
  }

  async transactionPipeline(steps: TransactionStep[]): Promise<TransactionPipelineResult> {
    // Scope all steps to this collection by default
    const scopedSteps = steps.map(s => ({
      ...s,
      collection: s.collection || this.name,
    }));
    const db = (await import('./database')).Database;
    const database = new db(this.client, this.database);
    return database.transactionPipeline(scopedSteps);
  }

  watch(pipeline?: Document[]): ChangeStream {
    return new ChangeStream(this.client, this.database, this.name, pipeline);
  }
}

const OP_TYPE_MAP: Record<string | number, string> = {
  0: 'insert', INSERT: 'insert',
  1: 'update', UPDATE: 'update',
  2: 'delete', DELETE: 'delete',
  3: 'replace', REPLACE: 'replace',
  4: 'invalidate', INVALIDATE: 'invalidate',
};

export class ChangeStream extends EventEmitter implements AsyncIterable<ChangeEvent>, AsyncDisposable {
  private grpcStream: any = null;
  private buffer: ChangeEvent[] = [];
  private waitResolve: ((value: IteratorResult<ChangeEvent>) => void) | null = null;
  private closed = false;
  private client: MongoClient;
  private database: string;
  private collection: string;
  private pipeline?: Document[];

  constructor(client: MongoClient, database: string, collection: string, pipeline?: Document[]) {
    super();
    this.client = client;
    this.database = database;
    this.collection = collection;
    this.pipeline = pipeline;
  }

  start(): this {
    const request: any = {
      database: this.database,
      collection: this.collection,
      pipeline: this.pipeline ? {
        stages: this.pipeline.map(stage => Buffer.from(BSON.serialize(stage))),
      } : undefined,
    };

    this.grpcStream = this.client.getGrpcClient().watch(request, CLIENT_METADATA);

    this.grpcStream.on('data', (event: any) => {
      const decoded: ChangeEvent = {
        operationType: OP_TYPE_MAP[event.operationType] || 'unknown',
      };
      if (event.database) decoded.database = event.database;
      if (event.collection) decoded.collection = event.collection;
      if (event.document?.data?.length > 0) {
        decoded.document = BSON.deserialize(Buffer.from(event.document.data)) as Document;
      }
      if (event.updateDescription?.data?.length > 0) {
        decoded.updateDescription = BSON.deserialize(Buffer.from(event.updateDescription.data)) as Document;
      }
      if (event.documentKey?.data?.length > 0) {
        decoded.documentKey = BSON.deserialize(Buffer.from(event.documentKey.data)) as Document;
      }

      if (this.waitResolve) {
        const resolve = this.waitResolve;
        this.waitResolve = null;
        resolve({ value: decoded, done: false });
      } else {
        this.buffer.push(decoded);
      }
      this.emit('change', decoded);
    });

    this.grpcStream.on('end', () => {
      this.closed = true;
      if (this.waitResolve) {
        this.waitResolve({ value: undefined as any, done: true });
        this.waitResolve = null;
      }
      this.emit('end');
    });

    this.grpcStream.on('error', (err: any) => {
      if (this.closed) return;
      this.closed = true;
      if (this.waitResolve) {
        this.waitResolve({ value: undefined as any, done: true });
        this.waitResolve = null;
      }
      this.emit('error', err);
    });

    return this;
  }

  close(): void {
    if (!this.closed) {
      this.closed = true;
      if (this.grpcStream) {
        this.grpcStream.cancel();
      }
    }
  }

  async [Symbol.asyncDispose](): Promise<void> {
    this.close();
  }

  [Symbol.asyncIterator](): AsyncIterator<ChangeEvent> {
    if (!this.grpcStream) this.start();
    return {
      next: (): Promise<IteratorResult<ChangeEvent>> => {
        if (this.buffer.length > 0) {
          return Promise.resolve({ value: this.buffer.shift()!, done: false });
        }
        if (this.closed) {
          return Promise.resolve({ value: undefined as any, done: true });
        }
        return new Promise(resolve => { this.waitResolve = resolve; });
      },
      return: (): Promise<IteratorResult<ChangeEvent>> => {
        this.close();
        return Promise.resolve({ value: undefined as any, done: true });
      },
    };
  }
}
