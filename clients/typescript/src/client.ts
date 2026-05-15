import * as grpc from '@grpc/grpc-js';
import * as protoLoader from '@grpc/proto-loader';
import * as path from 'path';
import { Database } from './database';
import { SidecarManager } from './sidecar';
import type { Analytics, PipelineResult } from './types';
import type { PipelineOp } from './ops';

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

const DEFAULT_SOCKET_PATH = '/tmp/mongocore.sock';
const DEFAULT_ADDRESS = 'localhost:50051';
const MAX_MESSAGE_SIZE = 64 * 1024 * 1024;

export class MongoClient {
  private address: string | undefined;
  private socketPath: string | undefined;
  private autoSpawn: boolean;
  private client: any = null;
  private sidecar: SidecarManager | null = null;
  public transport: string | null = null;

  constructor(address?: string, options?: { socketPath?: string; autoSpawn?: boolean }) {
    this.address = address;
    this.socketPath = options?.socketPath;
    this.autoSpawn = options?.autoSpawn ?? false;
  }

  private resolveTarget(): string {
    const fs = require('fs');

    if (this.socketPath) {
      this.transport = 'uds';
      return `unix://${this.socketPath}`;
    }
    if (this.address) {
      this.transport = 'tcp';
      return this.address;
    }
    const envSocket = process.env.MONGOCORE_SOCKET_PATH;
    if (envSocket) {
      this.transport = 'uds';
      return `unix://${envSocket}`;
    }
    try {
      fs.accessSync(DEFAULT_SOCKET_PATH);
      this.transport = 'uds';
      return `unix://${DEFAULT_SOCKET_PATH}`;
    } catch {}
    const envAddr = process.env.MONGOCORE_ADDRESS;
    if (envAddr) {
      this.transport = 'tcp';
      return envAddr;
    }
    this.transport = 'tcp';
    return DEFAULT_ADDRESS;
  }

  async connect(): Promise<this> {
    if (this.autoSpawn) {
      this.sidecar = new SidecarManager();
      await this.sidecar.ensureRunning();
    }

    const target = this.resolveTarget();
    const proto: any = loadProto();
    const MongoCore = proto.mongocore.v1.MongoCore;
    this.client = new MongoCore(target, grpc.credentials.createInsecure(), {
      'grpc.max_send_message_length': MAX_MESSAGE_SIZE,
      'grpc.max_receive_message_length': MAX_MESSAGE_SIZE,
    });
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
    return this.address ?? DEFAULT_ADDRESS;
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
        filePath: options.source,
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
      this.getGrpcClient().getIngestStatus({ jobId }, CLIENT_METADATA, (err: any, response: any) => {
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

  // --- Embed & Search Methods ---

  /** Generate embeddings for documents and store them */
  async embedAndStore(database: string, collection: string, documents: string, embedField: string, embeddingField?: string): Promise<{ documentsStored: number; embeddingsGenerated: number; embeddingDimensions: number }> {
    return new Promise((resolve, reject) => {
      const request = {
        database,
        collection,
        documents,
        embedField,
        embeddingField: embeddingField || '',
      };
      this.getGrpcClient().embedAndStore(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        resolve({
          documentsStored: response.documentsStored || response.documents_stored || 0,
          embeddingsGenerated: response.embeddingsGenerated || response.embeddings_generated || 0,
          embeddingDimensions: response.embeddingDimensions || response.embedding_dimensions || 0,
        });
      });
    });
  }

  /** Perform semantic search using vector embeddings */
  async semanticSearch(database: string, collection: string, query: string, indexName?: string, limit?: number): Promise<{ results: string; count: number }> {
    return new Promise((resolve, reject) => {
      const request = {
        database,
        collection,
        query,
        indexName: indexName || '',
        limit: limit || 10,
      };
      this.getGrpcClient().semanticSearch(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);
        resolve({
          results: response.results || '',
          count: response.count || 0,
        });
      });
    });
  }

  // --- Pipeline Methods ---

  /** Execute a batch of operations in a pipeline */
  async pipeline(...operations: PipelineOp[]): Promise<PipelineResult[]> {
    const { BSON } = await import('bson');

    const encodeBson = (doc: Record<string, unknown>): Buffer => {
      return Buffer.from(BSON.serialize(doc));
    };

    const decodeBson = (data: Buffer | Uint8Array): Record<string, unknown> => {
      return BSON.deserialize(Buffer.from(data)) as Record<string, unknown>;
    };

    // Convert each operation to a proto PipelineOperation
    const protoOps = operations.map((op) => {
      const protoOp: any = {};

      switch (op.type) {
        case 'find':
          protoOp.find = {
            database: op.database,
            collection: op.collection,
            filter: { data: encodeBson(op.filter || {}) },
            options: {
              limit: op.limit,
              skip: op.skip,
            },
          };
          break;

        case 'findOne':
          protoOp.findOne = {
            database: op.database,
            collection: op.collection,
            filter: { data: encodeBson(op.filter || {}) },
          };
          break;

        case 'insert':
          protoOp.insert = {
            database: op.database,
            collection: op.collection,
            document: { data: encodeBson(op.document) },
          };
          break;

        case 'insertMany':
          protoOp.insertMany = {
            database: op.database,
            collection: op.collection,
            documents: op.documents.map((d) => ({ data: encodeBson(d) })),
          };
          break;

        case 'update':
          protoOp.update = {
            database: op.database,
            collection: op.collection,
            filter: { data: encodeBson(op.filter) },
            update: { data: encodeBson(op.update) },
            upsert: false,
          };
          break;

        case 'updateMany':
          protoOp.updateMany = {
            database: op.database,
            collection: op.collection,
            filter: { data: encodeBson(op.filter) },
            update: { data: encodeBson(op.update) },
            upsert: false,
          };
          break;

        case 'delete':
          protoOp.delete = {
            database: op.database,
            collection: op.collection,
            filter: { data: encodeBson(op.filter) },
          };
          break;

        case 'deleteMany':
          protoOp.deleteMany = {
            database: op.database,
            collection: op.collection,
            filter: { data: encodeBson(op.filter) },
          };
          break;

        case 'aggregate':
          protoOp.aggregate = {
            database: op.database,
            collection: op.collection,
            pipeline: {
              stages: op.pipeline.map((stage) => encodeBson(stage)),
            },
          };
          break;

        case 'runCommand':
          protoOp.runCommand = {
            database: op.database,
            command: { data: encodeBson(op.command) },
            allowAll: op.allowAll || false,
          };
          break;

        case 'listDatabases':
          protoOp.listDatabases = {};
          break;

        case 'listCollections':
          protoOp.listCollections = {
            database: op.database,
          };
          break;

        default:
          throw new Error(`Unknown operation type: ${(op as any).type}`);
      }

      return protoOp;
    });

    return new Promise((resolve, reject) => {
      const request = { operations: protoOps };
      this.getGrpcClient().pipeline(request, CLIENT_METADATA, (err: any, response: any) => {
        if (err) return reject(err);

        const results = (response.results || []).map((r: any) => {
          const result: PipelineResult = {
            index: r.index,
            success: false,
          };

          // Check which result variant is present
          if (r.find) {
            result.success = true;
            result.result = {
              documents: (r.find.documents || []).map((d: any) => decodeBson(d.data)),
            };
          } else if (r.findOne) {
            result.success = true;
            result.result = {
              document: r.findOne.document?.data ? decodeBson(r.findOne.document.data) : null,
            };
          } else if (r.insert) {
            result.success = true;
            result.result = {
              insertedId: r.insert.insertedId,
            };
          } else if (r.insertMany) {
            result.success = true;
            result.result = {
              insertedIds: r.insertMany.insertedIds || [],
              insertedCount: r.insertMany.insertedCount || 0,
            };
          } else if (r.update) {
            result.success = true;
            result.result = {
              matchedCount: r.update.matchedCount || 0,
              modifiedCount: r.update.modifiedCount || 0,
              upsertedId: r.update.upsertedId,
            };
          } else if (r.updateMany) {
            result.success = true;
            result.result = {
              matchedCount: r.updateMany.matchedCount || 0,
              modifiedCount: r.updateMany.modifiedCount || 0,
              upsertedId: r.updateMany.upsertedId,
            };
          } else if (r.delete) {
            result.success = true;
            result.result = {
              deletedCount: r.delete.deletedCount || 0,
            };
          } else if (r.deleteMany) {
            result.success = true;
            result.result = {
              deletedCount: r.deleteMany.deletedCount || 0,
            };
          } else if (r.aggregate) {
            result.success = true;
            result.result = {
              documents: (r.aggregate.documents || []).map((d: any) => decodeBson(d.data)),
            };
          } else if (r.runCommand) {
            result.success = true;
            result.result = decodeBson(r.runCommand.result.data);
          } else if (r.listDatabases) {
            result.success = true;
            result.result = {
              databases: r.listDatabases.databases || [],
            };
          } else if (r.listCollections) {
            result.success = true;
            result.result = {
              collections: r.listCollections.collections || [],
            };
          } else if (r.error) {
            result.success = false;
            result.error = r.error;
          }

          return result;
        });

        resolve(results);
      });
    });
  }
}
