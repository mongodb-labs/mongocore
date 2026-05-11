import * as grpc from '@grpc/grpc-js';
import { Database } from './database';
import { SidecarManager } from './sidecar';

export class MongoClient {
  private address: string;
  private autoSpawn: boolean;
  private channel: grpc.Channel | null = null;
  private sidecar: SidecarManager | null = null;

  constructor(address: string = 'localhost:50051', options?: { autoSpawn?: boolean }) {
    this.address = address;
    this.autoSpawn = options?.autoSpawn ?? false;
  }

  async connect(): Promise<this> {
    if (this.autoSpawn) {
      this.sidecar = new SidecarManager();
      await this.sidecar.ensureRunning();
    }

    this.channel = new grpc.Channel(this.address, grpc.credentials.createInsecure(), {});
    return this;
  }

  async close(): Promise<void> {
    if (this.channel) {
      this.channel.close();
    }
    if (this.sidecar) {
      this.sidecar.stop();
    }
  }

  db(name: string): Database {
    return new Database(this, name);
  }

  getChannel(): grpc.Channel {
    if (!this.channel) {
      throw new Error('Not connected. Call connect() first.');
    }
    return this.channel;
  }

  getAddress(): string {
    return this.address;
  }

  async listDatabases(): Promise<string[]> {
    // Will call gRPC ListDatabases via generated stub
    throw new Error('Requires generated gRPC stubs. Run: npm run generate');
  }
}
