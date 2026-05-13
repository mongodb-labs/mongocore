/**
 * Benchmark MongoDB native Node.js driver for comparison against MongoCore.
 */

import { MongoClient, ObjectId } from 'mongodb';
import { readFileSync, mkdirSync, writeFileSync } from 'fs';
import { join } from 'path';
import { performance } from 'perf_hooks';
import * as os from 'os';

// Load config
const CONFIG = JSON.parse(readFileSync(join(__dirname, '..', 'common.json'), 'utf-8'));
const DATA_DIR = join(__dirname, '..', '..', 'data');
const RESULTS_DIR = join(__dirname, '..', '..', 'results');
mkdirSync(RESULTS_DIR, { recursive: true });

const WARMUP = CONFIG.warmup_iterations.typescript;
const MIN_TIME = CONFIG.min_time_secs;
const MAX_ITERS = CONFIG.max_iterations;
const MAX_TIME = CONFIG.max_time_secs;
const DB_NAME = CONFIG.database;

interface SystemInfo {
  os: string;
  arch: string;
  cpus: number;
  ram_gb: number;
  mongocore_version: string;
  driver: string;
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
  percentiles: {
    p10: number;
    p25: number;
    p50: number;
    p75: number;
    p90: number;
    p95: number;
    p99: number;
  };
  timestamp: string;
  system: SystemInfo;
}

function getSystemInfo(): SystemInfo {
  const totalMem = os.totalmem();
  return {
    os: os.platform(),
    arch: os.arch(),
    cpus: os.cpus().length,
    ram_gb: Math.round((totalMem / (1024 ** 3)) * 10) / 10,
    mongocore_version: 'native',
    driver: 'mongodb-node',
  };
}

function percentile(data: number[], pct: number): number {
  const idx = Math.max(0, Math.ceil(data.length * pct / 100) - 1);
  return data[Math.min(idx, data.length - 1)];
}

async function runBenchmark(
  name: string,
  category: string,
  setupFn: (client: MongoClient) => Promise<void>,
  beforeTaskFn: (client: MongoClient) => Promise<void>,
  taskFn: (client: MongoClient) => Promise<void>,
  afterTaskFn: (client: MongoClient) => Promise<void>,
  teardownFn: (client: MongoClient) => Promise<void>,
  datasetSizeBytes: number,
  batchSize: number,
): Promise<BenchResult> {
  const client = new MongoClient(CONFIG.mongodb_uri);
  await client.connect();

  await setupFn(client);

  // Warmup
  for (let i = 0; i < WARMUP; i++) {
    await beforeTaskFn(client);
    await taskFn(client);
    await afterTaskFn(client);
  }

  // Timed iterations
  const times: number[] = [];
  let totalTime = 0.0;
  let iteration = 0;

  while (totalTime < MIN_TIME || iteration < 5) {
    if (iteration >= MAX_ITERS || totalTime >= MAX_TIME) {
      break;
    }

    await beforeTaskFn(client);
    const start = performance.now();
    await taskFn(client);
    const elapsed = (performance.now() - start) / 1000;
    await afterTaskFn(client);

    times.push(elapsed);
    totalTime += elapsed;
    iteration++;
  }

  await teardownFn(client);
  await client.close();

  // Calculate metrics
  times.sort((a, b) => a - b);
  const median = times[Math.floor(times.length / 2)];
  const opsPerSec = batchSize / median;
  const mbPerSec = datasetSizeBytes / median / 1_000_000;

  const result: BenchResult = {
    benchmark: name,
    category: category,
    driver: 'mongodb-node',
    dataset_size_bytes: datasetSizeBytes,
    batch_size: batchSize,
    iterations: times.length,
    total_time_secs: Math.round(totalTime * 1000) / 1000,
    ops_per_sec: Math.round(opsPerSec * 10) / 10,
    mb_per_sec: Math.round(mbPerSec * 1000) / 1000,
    percentiles: {
      p10: Math.round(percentile(times, 10) * 1_000_000) / 1_000_000,
      p25: Math.round(percentile(times, 25) * 1_000_000) / 1_000_000,
      p50: Math.round(median * 1_000_000) / 1_000_000,
      p75: Math.round(percentile(times, 75) * 1_000_000) / 1_000_000,
      p90: Math.round(percentile(times, 90) * 1_000_000) / 1_000_000,
      p95: Math.round(percentile(times, 95) * 1_000_000) / 1_000_000,
      p99: Math.round(percentile(times, 99) * 1_000_000) / 1_000_000,
    },
    timestamp: new Date().toISOString(),
    system: getSystemInfo(),
  };

  console.log(`  ${name}: ${opsPerSec.toFixed(0)} ops/s, ${mbPerSec.toFixed(2)} MB/s (${times.length} iterations)`);
  return result;
}

async function main() {
  console.log('=== MongoDB Node.js driver (native) benchmarks ===');
  const results: BenchResult[] = [];

  const smallDoc = JSON.parse(readFileSync(join(DATA_DIR, 'small_doc.json'), 'utf-8'));
  const tweetDoc = JSON.parse(readFileSync(join(DATA_DIR, 'tweet.json'), 'utf-8'));
  const largeDoc = JSON.parse(readFileSync(join(DATA_DIR, 'large_doc.json'), 'utf-8'));
  const smallSize = Buffer.byteLength(JSON.stringify(smallDoc));
  const tweetSize = Buffer.byteLength(JSON.stringify(tweetDoc));
  const largeSize = Buffer.byteLength(JSON.stringify(largeDoc));

  // Run Command (batch 10,000 hello commands per iteration)
  results.push(await runBenchmark(
    'run_command', 'single_doc',
    async (_client) => {},
    async (_client) => {},
    async (client) => {
      for (let i = 0; i < 10_000; i++) {
        await client.db(DB_NAME).command({ hello: 1 });
      }
    },
    async (_client) => {},
    async (_client) => {},
    10_000 * 100, 10_000,
  ));

  // Find One by ID (batch 10,000 finds per iteration)
  results.push(await runBenchmark(
    'find_one_by_id', 'single_doc',
    async (client) => {
      const coll = client.db(DB_NAME).collection('bench_find');
      await coll.drop().catch(() => {});
      await coll.insertOne({ _id: new ObjectId('000000000000000000000001'), ...tweetDoc });
    },
    async (_client) => {},
    async (client) => {
      for (let i = 0; i < 10_000; i++) {
        await client.db(DB_NAME).collection('bench_find').findOne({ _id: new ObjectId('000000000000000000000001') });
      }
    },
    async (_client) => {},
    async (client) => {
      await client.db(DB_NAME).collection('bench_find').drop().catch(() => {});
    },
    10_000 * tweetSize, 10_000,
  ));

  // InsertOne Small (batch 10,000 inserts per iteration)
  results.push(await runBenchmark(
    'insert_one_small', 'single_doc',
    async (_client) => {},
    async (client) => {
      await client.db(DB_NAME).collection('bench_insert_small').drop().catch(() => {});
    },
    async (client) => {
      for (let i = 0; i < 10_000; i++) {
        await client.db(DB_NAME).collection('bench_insert_small').insertOne({ ...smallDoc, _id: new ObjectId() });
      }
    },
    async (_client) => {},
    async (_client) => {},
    10_000 * smallSize, 10_000,
  ));

  // InsertOne Large (batch 10 inserts per iteration)
  results.push(await runBenchmark(
    'insert_one_large', 'single_doc',
    async (_client) => {},
    async (client) => {
      await client.db(DB_NAME).collection('bench_insert_large').drop().catch(() => {});
    },
    async (client) => {
      for (let i = 0; i < 10; i++) {
        await client.db(DB_NAME).collection('bench_insert_large').insertOne({ ...largeDoc, _id: new ObjectId() });
      }
    },
    async (_client) => {},
    async (_client) => {},
    10 * largeSize, 10,
  ));

  // Bulk Insert Small (10,000 docs per iteration)
  results.push(await runBenchmark(
    'bulk_insert_small', 'multi_doc',
    async (_client) => {},
    async (client) => {
      await client.db(DB_NAME).collection('bench_bulk').drop().catch(() => {});
    },
    async (client) => {
      const docs = Array.from({ length: 10_000 }, () => ({ ...smallDoc, _id: new ObjectId() }));
      await client.db(DB_NAME).collection('bench_bulk').insertMany(docs);
    },
    async (_client) => {},
    async (_client) => {},
    smallSize * 10_000, 10_000,
  ));

  // Find Many (10,000 docs)
  results.push(await runBenchmark(
    'find_many', 'multi_doc',
    async (client) => {
      const coll = client.db(DB_NAME).collection('bench_find_many');
      await coll.drop().catch(() => {});
      const docs = Array.from({ length: 10_000 }, () => ({ ...smallDoc, _id: new ObjectId() }));
      await coll.insertMany(docs);
    },
    async (_client) => {},
    async (client) => {
      await client.db(DB_NAME).collection('bench_find_many').find({}).toArray();
    },
    async (_client) => {},
    async (client) => {
      await client.db(DB_NAME).collection('bench_find_many').drop().catch(() => {});
    },
    smallSize * 10_000, 10_000,
  ));

  // Save results
  const outputPath = join(RESULTS_DIR, 'typescript_native.json');
  writeFileSync(outputPath, JSON.stringify(results, null, 2));
  console.log(`\nResults saved to ${outputPath}`);
}

main().catch((err) => { console.error(err); process.exit(1); });
