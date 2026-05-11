import * as grpc from '@grpc/grpc-js';
import * as protoLoader from '@grpc/proto-loader';
import * as path from 'path';
import { Database } from './database';
import { SidecarManager } from './sidecar';

const PROTO_PATH = path.resolve(__dirname, '../../../proto/mongocore/v1/mongocore.proto');

function loadProto() {
  const packageDef = protoLoader.loadSync(PROTO_PATH, {
    keepCase: false,
    longs: Number,
    enums: String,
    defaults: true,
    oneofs: true,
    includeDirs: [path.resolve(__dirname, '../../../proto')],
  });
  return grpc.loadPackageDefinition(packageDef);
}

export class MongoClient {
  private address: string;
  private autoSpawn: boolean;
  private client: any = null;
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

    const proto: any = loadProto();
    const MongoCore = proto.mongocore.v1.MongoCore;
    this.client = new MongoCore(this.address, grpc.credentials.createInsecure());
    return this;
  }

  async close(): Promise<void> {
    if (this.client) {
      grpc.closeClient(this.client);
    }
    if (this.sidecar) {
      this.sidecar.stop();
    }
  }

  db(name: string): Database {
    return new Database(this, name);
  }

  getGrpcClient(): any {
    if (!this.client) {
      throw new Error('Not connected. Call connect() first.');
    }
    return this.client;
  }

  getAddress(): string {
    return this.address;
  }

  async listDatabases(): Promise<string[]> {
    return new Promise((resolve, reject) => {
      this.getGrpcClient().listDatabases({}, (err: any, response: any) => {
        if (err) return reject(err);
        resolve(response.databases || []);
      });
    });
  }

  async runCommand(database: string, command: Record<string, unknown>, allowAll = false): Promise<Record<string, unknown>> {
    const { BSON } = await import('bson');
    return new Promise((resolve, reject) => {
      const request = {
        database,
        command: { data: Buffer.from(BSON.serialize(command)) },
        allowAll,
      };
      this.getGrpcClient().runCommand(request, (err: any, response: any) => {
        if (err) return reject(err);
        const result = BSON.deserialize(Buffer.from(response.result.data)) as Record<string, unknown>;
        resolve(result);
      });
    });
  }
}
