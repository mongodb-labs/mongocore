import { MongoClient, CLIENT_METADATA } from '../src/client';
import { Database } from '../src/database';
import { Collection } from '../src/collection';

describe('Unit tests', () => {
  test('client creation with address', () => {
    const client = new MongoClient('custom:9999');
    expect(client).toBeTruthy();
    expect(client.getAddress()).toBe('custom:9999');
  });

  test('client default address', () => {
    const client = new MongoClient();
    expect(client).toBeTruthy();
    expect(client.getAddress()).toBe('localhost:50051');
  });

  test('database access', () => {
    const client = new MongoClient('localhost:50051');
    const db = client.db('testdb');
    expect(db).toBeInstanceOf(Database);
    expect(db.getName()).toBe('testdb');
  });

  test('collection access', () => {
    const client = new MongoClient('localhost:50051');
    const coll = client.db('testdb').collection('users');
    expect(coll).toBeInstanceOf(Collection);
  });

  test('client metadata header set', () => {
    expect(CLIENT_METADATA.get('x-client-language')).toEqual(['typescript']);
  });
});
