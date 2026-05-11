import { MongoClient } from './client';
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
    throw new Error('Requires generated gRPC stubs. Run: npm run generate');
  }

  async createCollection(name: string): Promise<void> {
    throw new Error('Requires generated gRPC stubs. Run: npm run generate');
  }
}
