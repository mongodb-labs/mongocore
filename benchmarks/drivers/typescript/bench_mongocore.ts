/**
 * Benchmark MongoCore TypeScript client (via gRPC sidecar).
 * Uses the @mongocore/client library.
 */

import { readFileSync, mkdirSync, writeFileSync } from 'fs';
import { join } from 'path';
import { performance } from 'perf_hooks';
import * as os from 'os';
import * as crypto from 'crypto';
import { MongoClient } from '../../../clients/typescript/src';

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

function newId(): string {
  return crypto.randomUUID().replace(/-/g, '').slice(0, 24);
}

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
  setupFn: () => Promise<void>,
  beforeTaskFn: () => Promise<void>,
  taskFn: () => Promise<void>,
  afterTaskFn: () => Promise<void>,
  teardownFn: () => Promise<void>,
  datasetSizeBytes: number,
  batchSize: number,
): Promise<BenchResult> {
  await setupFn();

  // Warmup
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
    system: { os: os.platform(), arch: os.arch(), cpus: os.cpus().length, driver: 'mongocore+typescript', mongocore_version: '0.6.0', transport: 'tcp' },
  };

  console.log(`  ${name}: ${opsPerSec.toFixed(0)} ops/s, ${mbPerSec.toFixed(2)} MB/s (${times.length} iterations)`);
  return result;
}

async function main() {
  console.log('=== MongoCore+TypeScript benchmarks ===');

  const client = new MongoClient(ADDR);
  await client.connect();
  const results: BenchResult[] = [];

  const smallDoc = JSON.parse(readFileSync(join(DATA_DIR, 'small_doc.json'), 'utf-8'));
  const tweetDoc = JSON.parse(readFileSync(join(DATA_DIR, 'tweet.json'), 'utf-8'));
  const largeDoc = JSON.parse(readFileSync(join(DATA_DIR, 'large_doc.json'), 'utf-8'));
  const smallSize = Buffer.byteLength(JSON.stringify(smallDoc));
  const tweetSize = Buffer.byteLength(JSON.stringify(tweetDoc));
  const largeSize = Buffer.byteLength(JSON.stringify(largeDoc));

  // Run Command (batch 10K)
  results.push(await runBenchmark(
    'run_command', 'single_doc',
    async () => {},
    async () => {},
    async () => {
      for (let i = 0; i < 10_000; i++) {
        await client.runCommand(DB_NAME, { hello: 1 });
      }
    },
    async () => {},
    async () => {},
    10_000 * 100, 10_000,
  ));

  // Find One by ID (batch 10K)
  const findColl = client.db(DB_NAME).collection('bench_find_ts_mc');
  results.push(await runBenchmark(
    'find_one_by_id', 'single_doc',
    async () => {
      await client.runCommand(DB_NAME, { drop: 'bench_find_ts_mc' }).catch(() => {});
      await findColl.insertOne({ ...tweetDoc, _id: 'bench_find_001' });
    },
    async () => {},
    async () => {
      for (let i = 0; i < 10_000; i++) {
        await findColl.findOne({ _id: 'bench_find_001' });
      }
    },
    async () => {},
    async () => {
      await client.runCommand(DB_NAME, { drop: 'bench_find_ts_mc' }).catch(() => {});
    },
    10_000 * tweetSize, 10_000,
  ));

  // InsertOne Small (batch 10K)
  const insertColl = client.db(DB_NAME).collection('bench_insert_ts_mc');
  results.push(await runBenchmark(
    'insert_one_small', 'single_doc',
    async () => {},
    async () => {
      await client.runCommand(DB_NAME, { drop: 'bench_insert_ts_mc' }).catch(() => {});
    },
    async () => {
      for (let i = 0; i < 10_000; i++) {
        await insertColl.insertOne({ ...smallDoc, _id: newId() });
      }
    },
    async () => {},
    async () => {},
    10_000 * smallSize, 10_000,
  ));

  // InsertOne Large (batch 10)
  const insertLargeColl = client.db(DB_NAME).collection('bench_insert_large_ts_mc');
  results.push(await runBenchmark(
    'insert_one_large', 'single_doc',
    async () => {},
    async () => {
      await client.runCommand(DB_NAME, { drop: 'bench_insert_large_ts_mc' }).catch(() => {});
    },
    async () => {
      for (let i = 0; i < 10; i++) {
        await insertLargeColl.insertOne({ ...largeDoc, _id: newId() });
      }
    },
    async () => {},
    async () => {},
    10 * largeSize, 10,
  ));

  // Bulk Insert Small (10K per iteration)
  const bulkColl = client.db(DB_NAME).collection('bench_bulk_ts_mc');
  results.push(await runBenchmark(
    'bulk_insert_small', 'multi_doc',
    async () => {},
    async () => {
      await client.runCommand(DB_NAME, { drop: 'bench_bulk_ts_mc' }).catch(() => {});
    },
    async () => {
      const docs = Array.from({ length: 10_000 }, () => ({ ...smallDoc, _id: newId() }));
      await bulkColl.insertMany(docs);
    },
    async () => {},
    async () => {},
    smallSize * 10_000, 10_000,
  ));

  // Find Many (2K docs)
  const findManyColl = client.db(DB_NAME).collection('bench_find_many_ts_mc');
  results.push(await runBenchmark(
    'find_many', 'multi_doc',
    async () => {
      await client.runCommand(DB_NAME, { drop: 'bench_find_many_ts_mc' }).catch(() => {});
      const docs = Array.from({ length: 2_000 }, () => ({ ...smallDoc, _id: newId() }));
      await findManyColl.insertMany(docs);
    },
    async () => {},
    async () => {
      await findManyColl.find({});
    },
    async () => {},
    async () => {},
    smallSize * 2_000, 2_000,
  ));

  console.log('  bulk_insert_large: SKIPPED (exceeds gRPC 4MB message limit)');
  console.log('  find_many_large: SKIPPED (response exceeds gRPC 4MB message limit)');

  // Save results
  const outputPath = join(RESULTS_DIR, 'typescript_mongocore.json');
  writeFileSync(outputPath, JSON.stringify(results, null, 2));
  console.log(`\nResults saved to ${outputPath}`);

  await client.close();
}

main().catch((err) => { console.error(err); process.exit(1); });
