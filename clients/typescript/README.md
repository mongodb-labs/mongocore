# MongoCore TypeScript Client

TypeScript/Node.js client for MongoCore, the AI-native MongoDB driver sidecar.

## Installation

```bash
npm install @mongocore/client
```

## Quick Start

```typescript
import { MongoClient } from '@mongocore/client';

const client = new MongoClient('localhost:50051');
await client.connect();

const db = client.db('mydb');
const users = db.collection('users');

await users.insertOne({ name: 'Alice', age: 30 });
const results = await users.find({ age: { $gte: 25 } });

await client.close();
```

## Auto-Spawn Sidecar

The client can automatically manage the MongoCore sidecar process:

```typescript
const client = new MongoClient('localhost:50051', { autoSpawn: true });
await client.connect(); // Starts sidecar if not running
```

## Development

### Generate gRPC Stubs

```bash
npm run generate
```

### Build

```bash
npm run build
```

### Test

```bash
npm test
```

## Requirements

- Node.js 18+
- MongoCore sidecar binary (for runtime usage)
- Protocol buffer compiler (for stub generation)
