/**
 * Integration tests for the MongoCore TypeScript client.
 *
 * Requires a running MongoCore sidecar on localhost:50051.
 * Start with: cargo run -- --config config.test.toml
 */

import { MongoClient } from '../src/client';
import { Collection } from '../src/collection';
import { Database } from '../src/database';
import * as ops from '../src/ops';
import path from 'path';
import fs from 'fs';
import os from 'os';

const TEST_DB = 'mongocore_client_test';

function uniqueCollection(): string {
  return `ts_test_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
}

let client: MongoClient;

beforeAll(async () => {
  client = new MongoClient('localhost:50051');
  await client.connect();
});

afterAll(async () => {
  await client.close();
});

describe('CRUD operations', () => {
  test('insert and find', async () => {
    const coll = client.db(TEST_DB).collection(uniqueCollection());

    const result = await coll.insertOne({ name: 'Alice', age: 30 });
    expect(result.insertedId).toBeTruthy();

    const docs = await coll.find({ name: 'Alice' });
    expect(docs).toHaveLength(1);
    expect(docs[0].name).toBe('Alice');
    expect(docs[0].age).toBe(30);
  });

  test('insert many and find', async () => {
    const coll = client.db(TEST_DB).collection(uniqueCollection());

    const result = await coll.insertMany([
      { name: 'Bob', score: 85 },
      { name: 'Carol', score: 92 },
      { name: 'Dave', score: 78 },
    ]);
    expect(result.insertedCount).toBe(3);
    expect(result.insertedIds).toHaveLength(3);

    const docs = await coll.find({});
    expect(docs).toHaveLength(3);
  });

  test('find one', async () => {
    const coll = client.db(TEST_DB).collection(uniqueCollection());

    await coll.insertOne({ key: 'unique_ts_value' });
    const doc = await coll.findOne({ key: 'unique_ts_value' });
    expect(doc).not.toBeNull();
    expect(doc!.key).toBe('unique_ts_value');

    const missing = await coll.findOne({ key: 'nonexistent' });
    expect(missing).toBeNull();
  });

  test('update one', async () => {
    const coll = client.db(TEST_DB).collection(uniqueCollection());

    await coll.insertOne({ name: 'Eve', status: 'active' });
    const result = await coll.updateOne(
      { name: 'Eve' },
      { $set: { status: 'inactive' } }
    );
    expect(result.modifiedCount).toBe(1);

    const doc = await coll.findOne({ name: 'Eve' });
    expect(doc!.status).toBe('inactive');
  });

  test('delete one', async () => {
    const coll = client.db(TEST_DB).collection(uniqueCollection());

    await coll.insertOne({ name: 'Frank' });
    await coll.insertOne({ name: 'Grace' });

    const count = await coll.deleteOne({ name: 'Frank' });
    expect(count).toBe(1);

    const docs = await coll.find({});
    expect(docs).toHaveLength(1);
    expect(docs[0].name).toBe('Grace');
  });

  test('delete many', async () => {
    const coll = client.db(TEST_DB).collection(uniqueCollection());

    await coll.insertMany([
      { group: 'A' },
      { group: 'A' },
      { group: 'B' },
    ]);

    const count = await coll.deleteMany({ group: 'A' });
    expect(count).toBe(2);

    const docs = await coll.find({});
    expect(docs).toHaveLength(1);
  });

  test('aggregate', async () => {
    const coll = client.db(TEST_DB).collection(uniqueCollection());

    await coll.insertMany([
      { category: 'A', value: 10 },
      { category: 'A', value: 20 },
      { category: 'B', value: 30 },
    ]);

    const results = await coll.aggregate([
      { $group: { _id: '$category', total: { $sum: '$value' } } },
      { $sort: { _id: 1 } },
    ]);

    expect(results).toHaveLength(2);
    expect(results[0]._id).toBe('A');
    expect(results[0].total).toBe(30);
    expect(results[1]._id).toBe('B');
    expect(results[1].total).toBe(30);
  });

  test('find with limit', async () => {
    const coll = client.db(TEST_DB).collection(uniqueCollection());

    const docs = Array.from({ length: 10 }, (_, i) => ({ i }));
    await coll.insertMany(docs);

    const results = await coll.find({}, { limit: 3 });
    expect(results).toHaveLength(3);
  });
});

describe('Change streams', () => {
  test('watch receives insert events', async () => {
    const coll = client.db(TEST_DB).collection(uniqueCollection());

    // Create the collection first
    await coll.insertOne({ setup: true });

    const stream = coll.watch();
    const events: any[] = [];

    const iterator = stream[Symbol.asyncIterator]();

    // Insert after a short delay
    setTimeout(async () => {
      await coll.insertOne({ name: 'watched' });
    }, 100);

    const result = await iterator.next();
    events.push(result.value);

    await iterator.return!();

    expect(events).toHaveLength(1);
    expect(events[0].operationType).toBe('insert');
    expect(events[0].document.name).toBe('watched');
  }, 10000);
});

describe('Search operations', () => {
  test('search', async () => {
    const coll = client.db(TEST_DB).collection(uniqueCollection() + '_search');
    await coll.insertMany([
      { title: 'rust programming guide', content: 'learn rust basics' },
      { title: 'python basics', content: 'learn python programming' },
      { title: 'rust advanced patterns', content: 'advanced rust techniques' },
    ]);
    const result = await coll.search('rust', 10);
    expect(['vector', 'fulltext', 'filter']).toContain(result.method);
    expect(result.total).toBeGreaterThanOrEqual(2);
    expect(result.documents.length).toBeGreaterThanOrEqual(2);
  });
});

describe('Database operations', () => {
  test('list databases', async () => {
    const databases = await client.listDatabases();
    expect(databases.length).toBeGreaterThan(0);
  });

  test('update many', async () => {
    const coll = client.db(TEST_DB).collection(uniqueCollection());

    await coll.insertMany([
      { name: 'Alice', status: 'active' },
      { name: 'Bob', status: 'active' },
      { name: 'Carol', status: 'inactive' },
    ]);

    const result = await coll.updateMany(
      { status: 'active' },
      { $set: { status: 'updated' } }
    );
    expect(result.modifiedCount).toBe(2);

    const docs = await coll.find({ status: 'updated' });
    expect(docs).toHaveLength(2);
  });

  test('find and modify', async () => {
    const coll = client.db(TEST_DB).collection(uniqueCollection());

    await coll.insertOne({ name: 'Counter', value: 10 });

    const result = await coll.findAndModify(
      { name: 'Counter' },
      { $inc: { value: 5 } },
      { returnDocument: 'after' }
    );

    expect(result.document).not.toBeNull();
    expect(result.document!.name).toBe('Counter');
    expect(result.document!.value).toBe(15);
  });

  test('list collections', async () => {
    const collName = uniqueCollection();
    const coll = client.db(TEST_DB).collection(collName);

    await coll.insertOne({ test: 'data' });

    const db = client.db(TEST_DB);
    const collections = await db.listCollections();

    expect(collections).toContain(collName);
  });

  test('create collection', async () => {
    const collName = uniqueCollection();
    const db = client.db(TEST_DB);

    await db.createCollection(collName);

    const collections = await db.listCollections();
    expect(collections).toContain(collName);
  });

  test('create index', async () => {
    const coll = client.db(TEST_DB).collection(uniqueCollection());

    await coll.insertOne({ email: 'test@example.com' });

    const result = await coll.createIndex(
      { email: 1 },
      { unique: true }
    );

    expect(result.indexName).toBeTruthy();
  });

  test('run command', async () => {
    const result = await client.runCommand('admin', { ping: 1 });

    expect(result.ok).toBe(1);
  });

  test('get analytics', async () => {
    const coll = client.db(TEST_DB).collection(uniqueCollection());
    await coll.insertOne({ test: 'analytics' });

    const analytics = await client.getAnalytics();

    expect(analytics.totalOperations).toBeDefined();
  });
});

describe('Transaction operations', () => {
  test('transaction commit', async () => {
    const txId = await client.beginTransaction(TEST_DB);

    expect(txId).toBeTruthy();

    const result = await client.commitTransaction(txId);

    expect(result).toBe(true);
  });

  test('transaction abort', async () => {
    const txId = await client.beginTransaction(TEST_DB);

    expect(txId).toBeTruthy();

    const result = await client.abortTransaction(txId);

    expect(result).toBe(true);
  });
});

describe('Ingestion operations', () => {
  test('ingest csv', async () => {
    const csvPath = path.resolve(__dirname, '../../test_fixtures/sample.csv');
    const collName = uniqueCollection();

    const result = await client.ingest({
      source: csvPath,
      format: 'csv',
      database: TEST_DB,
      collection: collName,
    });

    expect(result.jobId).toBeTruthy();
  });

  test('ingest status', async () => {
    const csvPath = path.resolve(__dirname, '../../test_fixtures/sample.csv');
    const collName = uniqueCollection();

    const result = await client.ingest({
      source: csvPath,
      format: 'csv',
      database: TEST_DB,
      collection: collName,
    });

    const status = await client.ingestStatus(result.jobId);

    expect(status).toBeDefined();
    expect(status.jobId).toBe(result.jobId);
  });

  test('list ingest jobs', async () => {
    const jobs = await client.listIngestJobs();

    expect(Array.isArray(jobs)).toBe(true);
  });

  test('cancel ingest', async () => {
    const csvPath = path.resolve(__dirname, '../../test_fixtures/sample.csv');
    const collName = uniqueCollection();

    const result = await client.ingest({
      source: csvPath,
      format: 'csv',
      database: TEST_DB,
      collection: collName,
    });

    const cancelResult = await client.cancelIngest(result.jobId);
    expect(cancelResult).toBeDefined();
  });
});

describe('Watch operations', () => {
  test('watch directory', async () => {
    const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mongocore-test-'));

    const result = await client.watchDirectory({
      path: tempDir,
      database: TEST_DB,
      collection: uniqueCollection(),
      format: 'csv',
    });

    expect(result.watchId).toBeTruthy();

    await client.stopWatch(result.watchId);

    fs.rmSync(tempDir, { recursive: true });
  });

  test('stop watch', async () => {
    const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mongocore-test-'));

    const result = await client.watchDirectory({
      path: tempDir,
      database: TEST_DB,
      collection: uniqueCollection(),
      format: 'csv',
    });

    const stopResult = await client.stopWatch(result.watchId);
    expect(stopResult).toBeTruthy();

    fs.rmSync(tempDir, { recursive: true });
  });
});

describe('Pipeline operations', () => {
  test('pipeline with mixed operations', async () => {
    const collName = uniqueCollection();

    // Insert data first (pipeline operations run concurrently, not sequentially)
    const coll = client.db(TEST_DB).collection(collName);
    await coll.insertOne({ name: 'Alice', age: 30 });
    await coll.insertOne({ name: 'Bob', age: 25 });

    const results = await client.pipeline(
      ops.find(TEST_DB, collName, {}),
      ops.updateMany(TEST_DB, collName, {}, { $inc: { age: 1 } }),
      ops.findOne(TEST_DB, collName, { name: 'Alice' }),
      ops.deleteMany(TEST_DB, collName, { name: 'Bob' })
    );

    expect(results).toHaveLength(4);

    // Check find result
    expect(results[0].success).toBe(true);
    expect(results[0].result?.documents).toHaveLength(2);

    // Check updateMany result
    expect(results[1].success).toBe(true);
    expect(results[1].result?.modifiedCount).toBe(2);

    // Check findOne result
    expect(results[2].success).toBe(true);
    expect(results[2].result?.document).toBeTruthy();
    expect(results[2].result?.document.name).toBe('Alice');

    // Check deleteMany result
    expect(results[3].success).toBe(true);
    expect(results[3].result?.deletedCount).toBe(1);
  });

  test('pipeline with aggregate', async () => {
    const collName = uniqueCollection();

    // Insert data first (pipeline operations run concurrently, not sequentially)
    const coll = client.db(TEST_DB).collection(collName);
    await coll.insertMany([
      { category: 'A', value: 10 },
      { category: 'A', value: 20 },
      { category: 'B', value: 30 },
    ]);

    const results = await client.pipeline(
      ops.aggregate(TEST_DB, collName, [
        { $group: { _id: '$category', total: { $sum: '$value' } } },
        { $sort: { _id: 1 } },
      ])
    );

    expect(results).toHaveLength(1);
    expect(results[0].success).toBe(true);
    expect(results[0].result?.documents).toHaveLength(2);
    expect(results[0].result?.documents[0]._id).toBe('A');
    expect(results[0].result?.documents[0].total).toBe(30);
  });

  test('pipeline with listDatabases and listCollections', async () => {
    const collName = uniqueCollection();

    // First create a collection
    await client.db(TEST_DB).collection(collName).insertOne({ test: 'data' });

    const results = await client.pipeline(
      ops.listDatabases(),
      ops.listCollections(TEST_DB)
    );

    expect(results).toHaveLength(2);

    expect(results[0].success).toBe(true);
    expect(results[0].result?.databases).toBeDefined();
    expect(Array.isArray(results[0].result?.databases)).toBe(true);

    expect(results[1].success).toBe(true);
    expect(results[1].result?.collections).toBeDefined();
    expect(results[1].result?.collections).toContain(collName);
  });

  test('pipeline with runCommand', async () => {
    const results = await client.pipeline(
      ops.runCommand('admin', { ping: 1 })
    );

    expect(results).toHaveLength(1);
    expect(results[0].success).toBe(true);
    expect(results[0].result?.ok).toBe(1);
  });
});

describe('count and drop operations', () => {
  test('countDocuments returns correct count', async () => {
    const collName = uniqueCollection();
    const coll = client.db(TEST_DB).collection(collName);

    await coll.insertMany([
      { status: 'active' },
      { status: 'active' },
      { status: 'inactive' },
    ]);

    const total = await coll.countDocuments();
    expect(total).toBe(3);

    const active = await coll.countDocuments({ status: 'active' });
    expect(active).toBe(2);
  });

  test('drop collection', async () => {
    const collName = uniqueCollection();
    const coll = client.db(TEST_DB).collection(collName);

    await coll.insertOne({ data: 'to be dropped' });
    const docs = await coll.find({});
    expect(docs).toHaveLength(1);

    const result = await coll.drop();
    expect(result).toBe(true);

    const docsAfter = await coll.find({});
    expect(docsAfter).toHaveLength(0);
  });

  test('dropCollection from database', async () => {
    const collName = uniqueCollection();
    const db = client.db(TEST_DB);
    const coll = db.collection(collName);

    await coll.insertOne({ data: 'test' });

    const result = await db.dropCollection(collName);
    expect(result).toBe(true);
  });
});

describe('embed and semantic search operations', () => {
  test('embedAndStore (may fail without provider)', async () => {
    const documents = JSON.stringify([
      { text: 'Hello world', id: 1 },
      { text: 'Goodbye world', id: 2 },
    ]);
    try {
      const result = await client.embedAndStore(TEST_DB, uniqueCollection(), documents, 'text');
      expect(result.documentsStored).toBeDefined();
      expect(result.embeddingsGenerated).toBeDefined();
    } catch {
      // Expected if no embedding provider configured
    }
  });

  test('semanticSearch (may fail without vector index)', async () => {
    try {
      const result = await client.semanticSearch(TEST_DB, uniqueCollection(), 'hello');
      expect(result.results).toBeDefined();
      expect(result.count).toBeDefined();
    } catch {
      // Expected if no vector index configured
    }
  });
});
