import * as grpc from '@grpc/grpc-js';
import * as protoLoader from '@grpc/proto-loader';
import * as path from 'path';
import { Database } from './database';
import { SidecarManager } from './sidecar';
import type { Analytics } from './types';

const PROTO_PATH = path.resolve(__dirname, '../../../proto/mongocore/v1/mongocore.proto');

export const CLIENT_METADATA = new grpc.Metadata();
CLIENT_METADATA.set('x-client-language', 'typescript');

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

/** Options for starting an ingestion job */
export interface IngestOptions {
  /** Source URI (file path, URL, or S3 URI) */
  source: string;
  /** Target database name */
  database: string;
  /** Target collection name */
  collection: string;
  /** File format (e.g., 'json', 'csv', 'parquet') */
  format?: string;
  /** Batch size for ingestion */
  batchSize?: number;
}

/** Status of an ingestion job */
export interface IngestJobStatus {
  jobId: string;
  status: string;
  documentsProcessed: number;
  documentsTotal: number;
  errors: string[];
  startedAt: string;
  completedAt?: string;
}

/** Options for watching a directory */
export interface WatchDirectoryOptions {
  /** Path to the directory to watch */
  path: string;
  /** Target database name */
  database: string;
  /** Target collection name */
  collection: string;
  /** File format filter (e.g., 'json', 'csv') */
  format?: string;
  /** Whether to watch recursively */
  recursive?: boolean;
}

/** Result of starting a directory watch */
export interface WatchResult {
  watchId: string;
  status: string;
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
      this.getGrpcClient().listDatabases({}, CLIENT_METADATA, (err: any, response: any) => {
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
      this.getGrpcClient().runCommand(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        const result = BSON.deserialize(Buffer.from(response.result.data)) as Record<string, unknown>;
        resolve(result);
      });
    });
  }

  // --- Transaction Methods ---

  async beginTransaction(database: string): Promise<string> {
    return new Promise((resolve, reject) => {
      const request = { database };
      this.getGrpcClient().beginTransaction(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        resolve(response.transactionId || response.transaction_id);
      });
    });
  }

  async commitTransaction(transactionId: string): Promise<boolean> {
    return new Promise((resolve, reject) => {
      const request = { transactionId };
      this.getGrpcClient().commitTransaction(request, CLIENT_METADATA, (err: any) => {
        if (err) return reject(err);
        resolve(true);
      });
    });
  }

  async abortTransaction(transactionId: string): Promise<boolean> {
    return new Promise((resolve, reject) => {
      const request = { transactionId };
      this.getGrpcClient().abortTransaction(request, CLIENT_METADATA, (err: any) => {
        if (err) return reject(err);
        resolve(true);
      });
    });
  }

  // --- Analytics Methods ---

  async getAnalytics(windowSeconds?: number): Promise<Analytics> {
    return new Promise((resolve, reject) => {
      const request = { windowSeconds: windowSeconds || 0 };
      this.getGrpcClient().getAnalytics(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        resolve({
          totalOperations: response.totalOperations || response.total_operations || 0,
          totalErrors: response.totalErrors || response.total_errors || 0,
          errorRate: response.errorRate || response.error_rate || 0,
          p50LatencyMs: response.p50LatencyMs || response.p50_latency_ms || 0,
          p95LatencyMs: response.p95LatencyMs || response.p95_latency_ms || 0,
          p99LatencyMs: response.p99LatencyMs || response.p99_latency_ms || 0,
          topOperations: (response.topOperations || response.top_operations || []).map((op: any) => ({
            operation: op.operation,
            count: op.count || 0,
          })),
          topCollections: (response.topCollections || response.top_collections || []).map((col: any) => ({
            collection: col.collection,
            count: col.count || 0,
          })),
        });
      });
    });
  }

  // --- Ingestion Methods ---

  /** Start an ingestion job */
  async ingest(options: IngestOptions): Promise<IngestJobStatus> {
    return new Promise((resolve, reject) => {
      const request = {
        source: options.source,
        database: options.database,
        collection: options.collection,
        format: options.format || '',
        batchSize: options.batchSize || 0,
      };
      this.getGrpcClient().ingest(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        resolve(response as IngestJobStatus);
      });
    });
  }

  /** Get the status of an ingestion job */
  async ingestStatus(jobId: string): Promise<IngestJobStatus> {
    return new Promise((resolve, reject) => {
      this.getGrpcClient().ingestStatus({ jobId }, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        resolve(response as IngestJobStatus);
      });
    });
  }

  /** List all ingestion jobs */
  async listIngestJobs(): Promise<IngestJobStatus[]> {
    return new Promise((resolve, reject) => {
      this.getGrpcClient().listIngestJobs({}, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        resolve(response.jobs || []);
      });
    });
  }

  /** Cancel an ingestion job */
  async cancelIngest(jobId: string): Promise<IngestJobStatus> {
    return new Promise((resolve, reject) => {
      this.getGrpcClient().cancelIngest({ jobId }, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        resolve(response as IngestJobStatus);
      });
    });
  }

  /** Start watching a directory for new files to ingest */
  async watchDirectory(options: WatchDirectoryOptions): Promise<WatchResult> {
    return new Promise((resolve, reject) => {
      const request = {
        path: options.path,
        database: options.database,
        collection: options.collection,
        format: options.format || '',
        recursive: options.recursive ?? true,
      };
      this.getGrpcClient().watchDirectory(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        resolve(response as WatchResult);
      });
    });
  }

  /** Stop watching a directory */
  async stopWatch(watchId: string): Promise<{ watchId: string; status: string }> {
    return new Promise((resolve, reject) => {
      this.getGrpcClient().stopWatch({ watchId }, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        resolve(response as { watchId: string; status: string });
      });
    });
  }
}
