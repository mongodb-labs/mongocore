# MongoCore Python Client

Thin, idiomatic Python client for MongoCore.

## Installation

```bash
pip install mongocore
```

## Quick Start

```python
import asyncio
from mongocore import MongoClient

async def main():
    async with MongoClient("localhost:50051") as client:
        db = client["mydb"]
        coll = db["users"]
        
        # Insert
        await coll.insert_one({"name": "Alice", "age": 30})
        
        # Find
        users = await coll.find({"age": {"$gte": 25}})
        print(users)
        
        # Aggregate
        result = await coll.aggregate([
            {"$group": {"_id": "$status", "count": {"$sum": 1}}}
        ])

asyncio.run(main())
```

## Dev Mode

```python
# Auto-spawn sidecar in development
client = MongoClient(auto_spawn=True)
await client.connect()
```

## Generate gRPC Stubs

```bash
pip install grpcio-tools
./generate_stubs.sh
```
