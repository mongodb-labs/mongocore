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
