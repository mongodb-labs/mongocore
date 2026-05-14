"""Pipeline operation builders for batch execution."""

from dataclasses import dataclass
from typing import Optional


@dataclass
class FindOp:
    """Find documents matching a filter."""
    database: str
    collection: str
    filter: Optional[dict] = None
    limit: int = 0
    skip: int = 0


@dataclass
class FindOneOp:
    """Find a single document."""
    database: str
    collection: str
    filter: Optional[dict] = None


@dataclass
class InsertOp:
    """Insert a single document."""
    database: str
    collection: str
    document: dict


@dataclass
class InsertManyOp:
    """Insert multiple documents."""
    database: str
    collection: str
    documents: list[dict]


@dataclass
class UpdateOp:
    """Update a single document."""
    database: str
    collection: str
    filter: dict
    update: dict


@dataclass
class UpdateManyOp:
    """Update multiple documents."""
    database: str
    collection: str
    filter: dict
    update: dict


@dataclass
class DeleteOp:
    """Delete a single document."""
    database: str
    collection: str
    filter: dict


@dataclass
class DeleteManyOp:
    """Delete multiple documents."""
    database: str
    collection: str
    filter: dict


@dataclass
class AggregateOp:
    """Run an aggregation pipeline."""
    database: str
    collection: str
    pipeline: list[dict]


@dataclass
class RunCommandOp:
    """Execute a raw MongoDB command."""
    database: str
    command: dict
    allow_all: bool = False


@dataclass
class ListDatabasesOp:
    """List all databases."""
    pass


@dataclass
class ListCollectionsOp:
    """List collections in a database."""
    database: str


@dataclass
class CreateCollectionOp:
    """Create a new collection."""
    database: str
    collection: str


@dataclass
class CreateIndexOp:
    """Create an index on a collection."""
    database: str
    collection: str
    keys: dict
    unique: bool = False
    name: Optional[str] = None


@dataclass
class SearchOp:
    """Search documents using vector/fulltext/filter."""
    database: str
    collection: str
    query: str
    limit: int = 10


@dataclass
class FindAndModifyOp:
    """Atomically find and modify a document."""
    database: str
    collection: str
    filter: dict
    update: dict
    return_new: bool = True
    upsert: bool = False


@dataclass
class BeginTransactionOp:
    """Begin a new transaction."""
    pass


@dataclass
class CommitTransactionOp:
    """Commit a transaction."""
    transaction_id: str


@dataclass
class AbortTransactionOp:
    """Abort a transaction."""
    transaction_id: str


@dataclass
class GetAnalyticsOp:
    """Get query analytics summary."""
    window_seconds: int = 0


# Convenience functions

def find(database: str, collection: str, filter: Optional[dict] = None, *, limit: int = 0, skip: int = 0) -> FindOp:
    """Create a find operation."""
    return FindOp(database, collection, filter, limit, skip)


def find_one(database: str, collection: str, filter: Optional[dict] = None) -> FindOneOp:
    """Create a find_one operation."""
    return FindOneOp(database, collection, filter)


def insert(database: str, collection: str, document: dict) -> InsertOp:
    """Create an insert operation."""
    return InsertOp(database, collection, document)


def insert_many(database: str, collection: str, documents: list[dict]) -> InsertManyOp:
    """Create an insert_many operation."""
    return InsertManyOp(database, collection, documents)


def update(database: str, collection: str, filter: dict, update: dict) -> UpdateOp:
    """Create an update operation."""
    return UpdateOp(database, collection, filter, update)


def update_many(database: str, collection: str, filter: dict, update: dict) -> UpdateManyOp:
    """Create an update_many operation."""
    return UpdateManyOp(database, collection, filter, update)


def delete(database: str, collection: str, filter: dict) -> DeleteOp:
    """Create a delete operation."""
    return DeleteOp(database, collection, filter)


def delete_many(database: str, collection: str, filter: dict) -> DeleteManyOp:
    """Create a delete_many operation."""
    return DeleteManyOp(database, collection, filter)


def aggregate(database: str, collection: str, pipeline: list[dict]) -> AggregateOp:
    """Create an aggregate operation."""
    return AggregateOp(database, collection, pipeline)


def run_command(database: str, command: dict, allow_all: bool = False) -> RunCommandOp:
    """Create a run_command operation."""
    return RunCommandOp(database, command, allow_all)


def list_databases() -> ListDatabasesOp:
    """Create a list_databases operation."""
    return ListDatabasesOp()


def list_collections(database: str) -> ListCollectionsOp:
    """Create a list_collections operation."""
    return ListCollectionsOp(database)


def create_collection(database: str, collection: str) -> CreateCollectionOp:
    """Create a create_collection operation."""
    return CreateCollectionOp(database, collection)


def create_index(database: str, collection: str, keys: dict, *, unique: bool = False, name: Optional[str] = None) -> CreateIndexOp:
    """Create a create_index operation."""
    return CreateIndexOp(database, collection, keys, unique, name)


def search(database: str, collection: str, query: str, limit: int = 10) -> SearchOp:
    """Create a search operation."""
    return SearchOp(database, collection, query, limit)


def find_and_modify(database: str, collection: str, filter: dict, update: dict, *, return_new: bool = True, upsert: bool = False) -> FindAndModifyOp:
    """Create a find_and_modify operation."""
    return FindAndModifyOp(database, collection, filter, update, return_new, upsert)


def begin_transaction() -> BeginTransactionOp:
    """Create a begin_transaction operation."""
    return BeginTransactionOp()


def commit_transaction(transaction_id: str) -> CommitTransactionOp:
    """Create a commit_transaction operation."""
    return CommitTransactionOp(transaction_id)


def abort_transaction(transaction_id: str) -> AbortTransactionOp:
    """Create an abort_transaction operation."""
    return AbortTransactionOp(transaction_id)


def get_analytics(window_seconds: int = 0) -> GetAnalyticsOp:
    """Create a get_analytics operation."""
    return GetAnalyticsOp(window_seconds)


# --- Transaction Pipeline Step Builders ---

@dataclass
class TransactionStep:
    """A step in a transactional pipeline."""
    name: str
    operation: dict  # Operation params (from step_* builders)
    collection: Optional[str] = None  # Set for database-scoped API


def step_find_one(filter: Optional[dict] = None) -> dict:
    """Create a find_one step operation."""
    return {"op": "find_one", "filter": filter or {}}


def step_find(filter: Optional[dict] = None, *, limit: int = 0) -> dict:
    """Create a find step operation."""
    result = {"op": "find", "filter": filter or {}}
    if limit:
        result["limit"] = limit
    return result


def step_insert(document: dict) -> dict:
    """Create an insert step operation."""
    return {"op": "insert", "document": document}


def step_insert_many(documents: list[dict]) -> dict:
    """Create an insert_many step operation."""
    return {"op": "insert_many", "documents": documents}


def step_update(filter: dict, update: dict) -> dict:
    """Create an update step operation."""
    return {"op": "update", "filter": filter, "update": update}


def step_update_many(filter: dict, update: dict) -> dict:
    """Create an update_many step operation."""
    return {"op": "update_many", "filter": filter, "update": update}


def step_delete(filter: dict) -> dict:
    """Create a delete step operation."""
    return {"op": "delete", "filter": filter}


def step_delete_many(filter: dict) -> dict:
    """Create a delete_many step operation."""
    return {"op": "delete_many", "filter": filter}


def step_find_and_modify(filter: dict, update: dict, *, return_new: bool = True) -> dict:
    """Create a find_and_modify step operation."""
    return {"op": "find_and_modify", "filter": filter, "update": update, "return_new": return_new}


def step_aggregate(pipeline: list[dict]) -> dict:
    """Create an aggregate step operation."""
    return {"op": "aggregate", "pipeline": pipeline}
