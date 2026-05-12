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
async def test_watch():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]

        # Insert a doc first so the collection exists
        await coll.insert_one({"setup": True})

        events = []
        async with coll.watch() as stream:
            # Insert in a separate task while watching
            async def do_insert():
                await asyncio.sleep(0.1)
                await coll.insert_one({"name": "watched"})
                await asyncio.sleep(0.1)

            insert_task = asyncio.create_task(do_insert())

            async for event in stream:
                events.append(event)
                if len(events) >= 1:
                    break

            await insert_task

        assert len(events) == 1
        assert events[0]["operation_type"] == 0  # INSERT


@pytest.mark.asyncio
async def test_search():
    async with MongoClient("localhost:50051") as client:
        coll = client["mongocore_client_test"]["py_test_search"]
        await coll.insert_many([
            {"title": "rust programming guide", "content": "learn rust basics"},
            {"title": "python basics", "content": "learn python programming"},
            {"title": "rust advanced patterns", "content": "advanced rust techniques"},
        ])
        result = await coll.search("rust", limit=10)
        assert result["method"] in ("vector", "fulltext", "filter")
        assert result["total"] >= 2
        assert len(result["documents"]) >= 2


@pytest.mark.asyncio
async def test_list_databases():
    async with MongoClient("localhost:50051") as client:
        databases = await client.list_databases()
        assert isinstance(databases, list)
        assert len(databases) > 0


@pytest.mark.asyncio
async def test_update_many():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]

        await coll.insert_many([
            {"category": "test", "value": 1},
            {"category": "test", "value": 2},
            {"category": "other", "value": 3},
        ])

        result = await coll.update_many(
            {"category": "test"},
            {"$set": {"updated": True}}
        )
        assert result["modified_count"] == 2

        docs = await coll.find({"updated": True})
        assert len(docs) == 2


@pytest.mark.asyncio
async def test_find_and_modify():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]

        await coll.insert_one({"counter": 10})
        result = await coll.find_and_modify(
            {"counter": 10},
            {"$inc": {"counter": 5}}
        )
        assert result is not None
        assert result["counter"] == 15


@pytest.mark.asyncio
async def test_list_collections():
    async with MongoClient("localhost:50051") as client:
        db = client[TEST_DB]
        coll_name = unique_collection()
        coll = db[coll_name]

        await coll.insert_one({"test": "data"})
        collections = await db.list_collections()
        assert isinstance(collections, list)
        assert coll_name in collections


@pytest.mark.asyncio
async def test_create_collection():
    async with MongoClient("localhost:50051") as client:
        db = client[TEST_DB]
        coll_name = unique_collection()

        await db.create_collection(coll_name)
        collections = await db.list_collections()
        assert coll_name in collections


@pytest.mark.asyncio
async def test_create_index():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]

        await coll.insert_one({"field": "value"})
        index_name = await coll.create_index({"field": 1}, unique=True)
        assert index_name
        assert len(index_name) > 0


@pytest.mark.asyncio
async def test_run_command():
    async with MongoClient("localhost:50051") as client:
        result = await client.run_command("admin", {"ping": 1})
        assert result.get("ok") == 1.0


@pytest.mark.asyncio
async def test_get_analytics():
    async with MongoClient("localhost:50051") as client:
        coll = client[TEST_DB][unique_collection()]
        await coll.insert_one({"test": "data"})

        analytics = await client.get_analytics()
        assert "total_operations" in analytics
        assert analytics["total_operations"] >= 0


@pytest.mark.asyncio
async def test_transaction_commit():
    async with MongoClient("localhost:50051") as client:
        txn_id = await client.begin_transaction()
        assert txn_id
        assert len(txn_id) > 0

        result = await client.commit_transaction(txn_id)
        assert result is True


@pytest.mark.asyncio
async def test_transaction_abort():
    async with MongoClient("localhost:50051") as client:
        txn_id = await client.begin_transaction()
        assert txn_id
        assert len(txn_id) > 0

        result = await client.abort_transaction(txn_id)
        assert result is True


@pytest.mark.asyncio
async def test_ingest_csv():
    import os

    async with MongoClient("localhost:50051") as client:
        csv_path = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../test_fixtures/sample.csv"))
        result = await client.ingest(
            file_path=csv_path,
            database=TEST_DB,
            collection=unique_collection()
        )
        assert result["job_id"]
        assert len(result["job_id"]) > 0


@pytest.mark.asyncio
async def test_ingest_status():
    import os

    async with MongoClient("localhost:50051") as client:
        csv_path = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../test_fixtures/sample.csv"))
        result = await client.ingest(
            file_path=csv_path,
            database=TEST_DB,
            collection=unique_collection()
        )
        job_id = result["job_id"]

        status = await client.ingest_status(job_id)
        assert status
        assert status.get("job_id") == job_id


@pytest.mark.asyncio
async def test_list_ingest_jobs():
    async with MongoClient("localhost:50051") as client:
        jobs = await client.list_ingest_jobs()
        assert isinstance(jobs, list)


@pytest.mark.asyncio
async def test_cancel_ingest():
    import os

    async with MongoClient("localhost:50051") as client:
        csv_path = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../test_fixtures/sample.csv"))
        ingest_result = await client.ingest(
            file_path=csv_path,
            database=TEST_DB,
            collection=unique_collection()
        )
        job_id = ingest_result["job_id"]

        result = await client.cancel_ingest(job_id)
        assert isinstance(result, bool)


@pytest.mark.asyncio
async def test_watch_directory():
    import tempfile

    async with MongoClient("localhost:50051") as client:
        with tempfile.TemporaryDirectory() as tmpdir:
            watch_id = await client.watch_directory(
                path=tmpdir,
                database=TEST_DB,
                collection=unique_collection()
            )
            assert watch_id
            assert len(watch_id) > 0

            await client.stop_watch(watch_id)


@pytest.mark.asyncio
async def test_stop_watch():
    import tempfile

    async with MongoClient("localhost:50051") as client:
        with tempfile.TemporaryDirectory() as tmpdir:
            watch_id = await client.watch_directory(
                path=tmpdir,
                database=TEST_DB,
                collection=unique_collection()
            )

            result = await client.stop_watch(watch_id)
            assert result is True
