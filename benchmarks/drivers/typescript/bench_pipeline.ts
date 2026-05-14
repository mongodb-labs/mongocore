/**
 * Benchmark MongoCore pipeline batching at different batch sizes.
 * Uses raw gRPC proto (same approach as bench_mongocore.ts).
 */

import { BSON } from "bson";
import * as grpc from '@grpc/grpc-js';
import * as protoLoader from '@grpc/proto-loader';
import { readFileSync, mkdirSync, writeFileSync } from 'fs';
import { join, resolve } from 'path';
import { performance } from 'perf_hooks';
import * as os from 'os';
import * as crypto from 'crypto';

const PROTO_PATH = resolve(__dirname, '..', '..', '..', 'proto', 'mongocore', 'v1', 'mongocore.proto');
const packageDef = protoLoader.loadSync(PROTO_PATH, {
  keepCase: false,
  longs: Number,
  enums: String,
  defaults: true,
  oneofs: true,
  includeDirs: [resolve(__dirname, '..', '..', '..', 'proto')],
});
const proto = grpc.loadPackageDefinition(packageDef) as any;
const MongoCore = proto.mongocore.v1.MongoCore;

const CONFIG = JSON.parse(readFileSync(join(__dirname, '..', 'common.json'), 'utf-8'));
const DATA_DIR = join(__dirname, '..', '..', 'data');
const RESULTS_DIR = join(__dirname, '..', '..', 'results');
mkdirSync(RESULTS_DIR, { recursive: true });

const WARMUP = CONFIG.warmup_iterations.typescript;
const MIN_TIME = CONFIG.min_time_secs;
const MAX_ITERS = CONFIG.max_iterations;
const MAX_TIME = CONFIG.max_time_secs;
const DB_NAME = CONFIG.database;
const ADDR = CONFIG.mongocore_address;

const BATCH_SIZES = [100, 1000, 10000];
const TOTAL_OPS = 10000;

function newId(): string {
  return crypto.randomUUID().replace(/-/g, '').slice(0, 24);
}

function promisify(client: any, method: string, request: any): Promise<any> {
  return new Promise((resolve, reject) => {
    client[method](request, (err: any, response: any) => {
      if (err) reject(err);
      else resolve(response);
    });
  });
}

const encodeDoc = (doc: any) => Buffer.from(BSON.serialize(doc));

interface BenchResult {
  benchmark: string;
  category: string;
  driver: string;
  dataset_size_bytes: number;
  batch_size: number;
  iterations: number;
  total_time_secs: number;
  ops_per_sec: number;
  mb_per_sec: number;
  percentiles: Record<string, number>;
  timestamp: string;
  system: Record<string, any>;
}

async function runBenchmark(
  name: string,
  category: string,
  client: any,
  setupFn: () => Promise<void>,
  beforeTaskFn: () => Promise<void>,
  taskFn: () => Promise<void>,
  afterTaskFn: () => Promise<void>,
  teardownFn: () => Promise<void>,
  datasetSizeBytes: number,
  batchSize: number,
): Promise<BenchResult> {
  await setupFn();

  for (let i = 0; i < WARMUP; i++) {
    await beforeTaskFn();
    await taskFn();
    await afterTaskFn();
  }

  const times: number[] = [];
  let totalTime = 0;
  let iteration = 0;

  while (totalTime < MIN_TIME || iteration < 5) {
    if (iteration >= MAX_ITERS || totalTime >= MAX_TIME) break;

    await beforeTaskFn();
    const start = performance.now();
    await taskFn();
    const elapsed = (performance.now() - start) / 1000;
    await afterTaskFn();

    times.push(elapsed);
    totalTime += elapsed;
    iteration++;
  }

  await teardownFn();

  times.sort((a, b) => a - b);
  const median = times[Math.floor(times.length / 2)];
  const opsPerSec = batchSize / median;
  const mbPerSec = datasetSizeBytes / median / 1_000_000;

  const pct = (p: number) => times[Math.min(Math.max(0, Math.ceil(times.length * p / 100) - 1), times.length - 1)];

  const result: BenchResult = {
    benchmark: name,
    category,
    driver: 'mongocore+typescript',
    dataset_size_bytes: datasetSizeBytes,
    batch_size: batchSize,
    iterations: times.length,
    total_time_secs: Math.round(totalTime * 1000) / 1000,
    ops_per_sec: Math.round(opsPerSec * 10) / 10,
    mb_per_sec: Math.round(mbPerSec * 1000) / 1000,
    percentiles: {
      p10: Math.round(pct(10) * 1000000) / 1000000,
      p25: Math.round(pct(25) * 1000000) / 1000000,
      p50: Math.round(median * 1000000) / 1000000,
      p75: Math.round(pct(75) * 1000000) / 1000000,
      p90: Math.round(pct(90) * 1000000) / 1000000,
      p95: Math.round(pct(95) * 1000000) / 1000000,
      p99: Math.round(pct(99) * 1000000) / 1000000,
    },
    timestamp: new Date().toISOString(),
    system: { os: os.platform(), arch: os.arch(), cpus: os.cpus().length, driver: 'mongocore+typescript', mongocore_version: '0.6.0' },
  };

  console.log(`  ${name}: ${opsPerSec.toFixed(0)} ops/s, ${mbPerSec.toFixed(2)} MB/s (${times.length} iterations)`);
  return result;
}

async function main() {
  console.log('=== MongoCore+TypeScript Pipeline benchmarks ===');

  const client = new MongoCore(ADDR, grpc.credentials.createInsecure());
  const results: BenchResult[] = [];

  const smallDoc = JSON.parse(readFileSync(join(DATA_DIR, 'small_doc.json'), 'utf-8'));
  const tweetDoc = JSON.parse(readFileSync(join(DATA_DIR, 'tweet.json'), 'utf-8'));
  const smallSize = Buffer.byteLength(JSON.stringify(smallDoc));
  const tweetSize = Buffer.byteLength(JSON.stringify(tweetDoc));

  for (const batchSize of BATCH_SIZES) {
    const callsPerIter = TOTAL_OPS / batchSize;

    // --- pipeline_run_command ---
    results.push(await runBenchmark(
      `pipeline_run_command_${batchSize}`, 'pipeline', client,
      async () => {},
      async () => {},
      async () => {
        for (let c = 0; c < callsPerIter; c++) {
          const operations = Array.from({ length: batchSize }, () => ({
            runCommand: {
              database: DB_NAME,
              command: { data: encodeDoc({ hello: 1 }) },
              allowAll: false,
            },
          }));
          await promisify(client, 'pipeline', { operations });
        }
      },
      async () => {},
      async () => {},
      TOTAL_OPS * 100, TOTAL_OPS,
    ));

    // --- pipeline_insert_one_small ---
    results.push(await runBenchmark(
      `pipeline_insert_one_small_${batchSize}`, 'pipeline', client,
      async () => {},
      async () => {
        await promisify(client, 'runCommand', { database: DB_NAME, command: { data: encodeDoc({ drop: 'bench_pipeline_insert_ts' }) }, allowAll: false }).catch(() => {});
      },
      async () => {
        for (let c = 0; c < callsPerIter; c++) {
          const operations = Array.from({ length: batchSize }, () => ({
            insert: {
              database: DB_NAME,
              collection: 'bench_pipeline_insert_ts',
              document: { data: encodeDoc({ ...smallDoc, _id: newId() }) },
            },
          }));
          await promisify(client, 'pipeline', { operations });
        }
      },
      async () => {},
      async () => {},
      TOTAL_OPS * smallSize, TOTAL_OPS,
    ));

    // --- pipeline_find_one_by_id ---
    results.push(await runBenchmark(
      `pipeline_find_one_by_id_${batchSize}`, 'pipeline', client,
      async () => {
        await promisify(client, 'runCommand', { database: DB_NAME, command: { data: encodeDoc({ drop: 'bench_pipeline_find_ts' }) }, allowAll: false }).catch(() => {});
        await promisify(client, 'insert', {
          database: DB_NAME,
          collection: 'bench_pipeline_find_ts',
          document: { data: encodeDoc({ ...tweetDoc, _id: 'bench_find_001' }) },
        });
      },
      async () => {},
      async () => {
        for (let c = 0; c < callsPerIter; c++) {
          const operations = Array.from({ length: batchSize }, () => ({
            findOne: {
              database: DB_NAME,
              collection: 'bench_pipeline_find_ts',
              filter: { data: encodeDoc({ _id: 'bench_find_001' }) },
            },
          }));
          await promisify(client, 'pipeline', { operations });
        }
      },
      async () => {},
      async () => {
        await promisify(client, 'runCommand', { database: DB_NAME, command: { data: encodeDoc({ drop: 'bench_pipeline_find_ts' }) }, allowAll: false }).catch(() => {});
      },
      TOTAL_OPS * tweetSize, TOTAL_OPS,
    ));
  }

  client.close();

  const outputPath = join(RESULTS_DIR, 'typescript_pipeline.json');
  writeFileSync(outputPath, JSON.stringify(results, null, 2));
  console.log(`\nResults saved to ${outputPath}`);
}

main().catch((err) => { console.error(err); process.exit(1); });
