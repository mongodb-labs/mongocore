import { MongoClient, CLIENT_METADATA } from './client';
import { Collection } from './collection';

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
}
