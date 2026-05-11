import { BSON } from 'bson';
import { MongoCoreClient } from './client';
import type { Document, FindOptions, UpdateResult, InsertResult, InsertManyResult } from './types';

export class Collection {
  private client: MongoCoreClient;
  private database: string;
  private name: string;

  constructor(client: MongoCoreClient, database: string, name: string) {
    this.client = client;
    this.database = database;
    this.name = name;
  }

  private encodeBson(doc: Document): Uint8Array {
    return BSON.serialize(doc);
  }

  private decodeBson(data: Uint8Array): Document {
    return BSON.deserialize(data) as Document;
  }

  async find(filter?: Document, options?: FindOptions): Promise<Document[]> {
    // Will use generated gRPC stub to call Find RPC
    // Encodes filter as BSON bytes, sends via proto, decodes response
    throw new Error('Requires generated gRPC stubs. Run: npm run generate');
  }

  async findOne(filter?: Document): Promise<Document | null> {
    throw new Error('Requires generated gRPC stubs. Run: npm run generate');
  }

  async insertOne(document: Document): Promise<InsertResult> {
    throw new Error('Requires generated gRPC stubs. Run: npm run generate');
  }

  async insertMany(documents: Document[]): Promise<InsertManyResult> {
    throw new Error('Requires generated gRPC stubs. Run: npm run generate');
  }

  async updateOne(filter: Document, update: Document): Promise<UpdateResult> {
    throw new Error('Requires generated gRPC stubs. Run: npm run generate');
  }

  async updateMany(filter: Document, update: Document): Promise<UpdateResult> {
    throw new Error('Requires generated gRPC stubs. Run: npm run generate');
  }

  async deleteOne(filter: Document): Promise<number> {
    throw new Error('Requires generated gRPC stubs. Run: npm run generate');
  }

  async deleteMany(filter: Document): Promise<number> {
    throw new Error('Requires generated gRPC stubs. Run: npm run generate');
  }

  async aggregate(pipeline: Document[]): Promise<Document[]> {
    throw new Error('Requires generated gRPC stubs. Run: npm run generate');
  }
}
