import { BSON } from 'bson';
import { MongoClient, CLIENT_METADATA } from './client';
import { Collection } from './collection';
import type { Document, TransactionPipelineResult } from './types';
import type { TransactionStep } from './ops';

export class Database {
  private client: MongoClient;
  private name: string;

  constructor(client: MongoClient, name: string) {
    this.client = client;
    this.name = name;
  }

  getName(): string {
    return this.name;
  }

  collection(name: string): Collection {
    return new Collection(this.client, this.name, name);
  }

  async listCollections(): Promise<string[]> {
    return new Promise((resolve, reject) => {
      this.client.getGrpcClient().listCollections(
        { database: this.name },
        CLIENT_METADATA,
        (err: any, response: any) => {
          if (err) return reject(err);
          resolve(response.collections || []);
        }
      );
    });
  }

  async createCollection(name: string): Promise<void> {
    return new Promise((resolve, reject) => {
      this.client.getGrpcClient().createCollection(
        { database: this.name, collection: name },
        CLIENT_METADATA,
        (err: any) => {
          if (err) return reject(err);
          resolve();
        }
      );
    });
  }

  async transactionPipeline(steps: TransactionStep[]): Promise<TransactionPipelineResult> {
    return new Promise((resolve, reject) => {
      const request = {
        database: this.name,
        steps: steps.map(s => this.buildStepRequest(s)),
      };
      this.client.getGrpcClient().transactionPipeline(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        resolve({
          committed: response.committed || false,
          results: (response.results || []).map((r: any) => ({
            stepName: r.stepName || r.step_name || '',
            success: r.success || false,
            error: r.error,
            documents: r.documents?.map((d: any) => BSON.deserialize(Buffer.from(d.data)) as Document),
            matchedCount: r.matchedCount ?? r.matched_count,
            modifiedCount: r.modifiedCount ?? r.modified_count,
            deletedCount: r.deletedCount ?? r.deleted_count,
            insertedId: r.insertedId ?? r.inserted_id,
            insertedIds: r.insertedIds ?? r.inserted_ids,
          })),
          error: response.error,
        });
      });
    });
  }

  private buildStepRequest(step: TransactionStep): any {
    const op = step.operation;
    const base: any = {
      name: step.name,
      collection: step.collection,
      operationType: op.op,
    };

    switch (op.op) {
      case 'find_one':
        base.filter = op.filter ? { data: Buffer.from(BSON.serialize(op.filter)) } : undefined;
        break;
      case 'find':
        base.filter = op.filter ? { data: Buffer.from(BSON.serialize(op.filter)) } : undefined;
        base.limit = op.limit;
        break;
      case 'insert':
        base.document = { data: Buffer.from(BSON.serialize(op.document)) };
        break;
      case 'insert_many':
        base.documents = op.documents.map(d => ({ data: Buffer.from(BSON.serialize(d)) }));
        break;
      case 'update':
      case 'update_many':
        base.filter = { data: Buffer.from(BSON.serialize(op.filter)) };
        base.update = { data: Buffer.from(BSON.serialize(op.update)) };
        break;
      case 'delete':
      case 'delete_many':
        base.filter = { data: Buffer.from(BSON.serialize(op.filter)) };
        break;
      case 'find_and_modify':
        base.filter = { data: Buffer.from(BSON.serialize(op.filter)) };
        base.update = { data: Buffer.from(BSON.serialize(op.update)) };
        break;
      case 'aggregate':
        base.pipeline = { stages: op.pipeline.map(s => Buffer.from(BSON.serialize(s))) };
        break;
    }

    return base;
  }
}
