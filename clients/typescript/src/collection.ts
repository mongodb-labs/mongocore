import { BSON } from 'bson';
import { MongoClient } from './client';
import type { Document, FindOptions, UpdateResult, InsertResult, InsertManyResult } from './types';

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
      this.client.getGrpcClient().find(request, (err: any, response: any) => {
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
      this.client.getGrpcClient().findOne(request, (err: any, response: any) => {
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
      this.client.getGrpcClient().insert(request, (err: any, response: any) => {
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
      this.client.getGrpcClient().insertMany(request, (err: any, response: any) => {
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
      this.client.getGrpcClient().update(request, (err: any, response: any) => {
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
      this.client.getGrpcClient().updateMany(request, (err: any, response: any) => {
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
      this.client.getGrpcClient().delete(request, (err: any, response: any) => {
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
      this.client.getGrpcClient().deleteMany(request, (err: any, response: any) => {
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
      this.client.getGrpcClient().aggregate(request, (err: any, response: any) => {
        if (err) return reject(err);
        const docs = (response.documents || []).map((d: any) => this.decodeBson(d.data));
        resolve(docs);
      });
    });
  }
}
