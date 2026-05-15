import pytest
from mongocore import MongoClient, Collection, Database


def test_client_creation():
    client = MongoClient("localhost:50051")
    assert client._address == "localhost:50051"


def test_client_database_access():
    client = MongoClient()
    db = client["testdb"]
    assert isinstance(db, Database)
    assert db.name == "testdb"


def test_database_collection_access():
    client = MongoClient()
    db = client["testdb"]
    coll = db["users"]
    assert isinstance(coll, Collection)


def test_client_default_address():
    client = MongoClient()
    # No explicit address — auto-discovery resolves target at connect time
    assert client._address is None
    # Explicit address is stored as-is
    client2 = MongoClient(address="localhost:50051")
    assert client2._address == "localhost:50051"


def test_client_metadata_constant():
    from mongocore.client import _CLIENT_METADATA
    assert _CLIENT_METADATA == [("x-client-language", "python")]
