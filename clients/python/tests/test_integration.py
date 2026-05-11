"""Integration tests for the MongoCore Python client.

Requires a running MongoCore sidecar on localhost:50051.
Start with: cargo run -- --config config.test.toml
"""

import asyncio
import sys
import uuid

import pytest

sys.path.insert(0, "src")
from mongocore import MongoClient


TEST_DB = "mongocore_client_test"


def unique_collection():
    return f"py_test_{uuid.uuid4().hex[:12]}"


@pytest.fixture
def event_loop():
    loop = asyncio.new_event_loop()
    yield loop
    loop.close()


@pytest.mark.asyncio
async def test_insert_and_find():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]

        inserted_id = await coll.insert_one({"name": "Alice", "age": 30})
        assert inserted_id

        docs = await coll.find({"name": "Alice"})
        assert len(docs) == 1
        assert docs[0]["name"] == "Alice"
        assert docs[0]["age"] == 30


@pytest.mark.asyncio
async def test_insert_many_and_find():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]

        ids = await coll.insert_many([
            {"name": "Bob", "score": 85},
            {"name": "Carol", "score": 92},
            {"name": "Dave", "score": 78},
        ])
        assert len(ids) == 3

        docs = await coll.find({})
        assert len(docs) == 3


@pytest.mark.asyncio
async def test_find_one():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]

        await coll.insert_one({"key": "unique_value"})
        doc = await coll.find_one({"key": "unique_value"})
        assert doc is not None
        assert doc["key"] == "unique_value"

        missing = await coll.find_one({"key": "nonexistent"})
        assert missing is None


@pytest.mark.asyncio
async def test_update_one():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]

        await coll.insert_one({"name": "Eve", "status": "active"})
        result = await coll.update_one(
            {"name": "Eve"},
            {"$set": {"status": "inactive"}}
        )
        assert result["modified_count"] == 1

        doc = await coll.find_one({"name": "Eve"})
        assert doc["status"] == "inactive"


@pytest.mark.asyncio
async def test_delete_one():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]

        await coll.insert_one({"name": "Frank"})
        await coll.insert_one({"name": "Grace"})

        count = await coll.delete_one({"name": "Frank"})
        assert count == 1

        docs = await coll.find({})
        assert len(docs) == 1
        assert docs[0]["name"] == "Grace"


@pytest.mark.asyncio
async def test_delete_many():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]

        await coll.insert_many([
            {"group": "A"},
            {"group": "A"},
            {"group": "B"},
        ])

        count = await coll.delete_many({"group": "A"})
        assert count == 2

        docs = await coll.find({})
        assert len(docs) == 1


@pytest.mark.asyncio
async def test_aggregate():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]

        await coll.insert_many([
            {"category": "A", "value": 10},
            {"category": "A", "value": 20},
            {"category": "B", "value": 30},
        ])

        results = await coll.aggregate([
            {"$group": {"_id": "$category", "total": {"$sum": "$value"}}},
            {"$sort": {"_id": 1}},
        ])

        assert len(results) == 2
        assert results[0]["_id"] == "A"
        assert results[0]["total"] == 30
        assert results[1]["_id"] == "B"
        assert results[1]["total"] == 30


@pytest.mark.asyncio
async def test_find_with_limit():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]

        await coll.insert_many([{"i": i} for i in range(10)])

        docs = await coll.find({}, limit=3)
        assert len(docs) == 3


@pytest.mark.asyncio
async def test_list_databases():
    async with MongoClient("localhost:50051") as client:
        databases = await client.list_databases()
        assert isinstance(databases, list)
        assert len(databases) > 0
