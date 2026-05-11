import pytest
from mongocore import MongoCoreClient, Collection, Database


def test_client_creation():
    client = MongoCoreClient("localhost:50051")
    assert client._address == "localhost:50051"


def test_client_database_access():
    client = MongoCoreClient()
    db = client["testdb"]
    assert isinstance(db, Database)
    assert db.name == "testdb"


def test_database_collection_access():
    client = MongoCoreClient()
    db = client["testdb"]
    coll = db["users"]
    assert isinstance(coll, Collection)
