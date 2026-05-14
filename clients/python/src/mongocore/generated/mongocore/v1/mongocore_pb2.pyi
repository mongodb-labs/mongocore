from mongocore.v1 import types_pb2 as _types_pb2
from mongocore.v1 import ingestion_pb2 as _ingestion_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class FindRequest(_message.Message):
    __slots__ = ("database", "collection", "filter", "options", "transaction_id")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    FILTER_FIELD_NUMBER: _ClassVar[int]
    OPTIONS_FIELD_NUMBER: _ClassVar[int]
    TRANSACTION_ID_FIELD_NUMBER: _ClassVar[int]
    database: str
    collection: str
    filter: _types_pb2.Filter
    options: _types_pb2.FindOptions
    transaction_id: str
    def __init__(self, database: _Optional[str] = ..., collection: _Optional[str] = ..., filter: _Optional[_Union[_types_pb2.Filter, _Mapping]] = ..., options: _Optional[_Union[_types_pb2.FindOptions, _Mapping]] = ..., transaction_id: _Optional[str] = ...) -> None: ...

class FindResponse(_message.Message):
    __slots__ = ("documents", "metadata")
    DOCUMENTS_FIELD_NUMBER: _ClassVar[int]
    METADATA_FIELD_NUMBER: _ClassVar[int]
    documents: _containers.RepeatedCompositeFieldContainer[_types_pb2.Document]
    metadata: _types_pb2.ResponseMetadata
    def __init__(self, documents: _Optional[_Iterable[_Union[_types_pb2.Document, _Mapping]]] = ..., metadata: _Optional[_Union[_types_pb2.ResponseMetadata, _Mapping]] = ...) -> None: ...

class FindOneRequest(_message.Message):
    __slots__ = ("database", "collection", "filter", "options", "transaction_id")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    FILTER_FIELD_NUMBER: _ClassVar[int]
    OPTIONS_FIELD_NUMBER: _ClassVar[int]
    TRANSACTION_ID_FIELD_NUMBER: _ClassVar[int]
    database: str
    collection: str
    filter: _types_pb2.Filter
    options: _types_pb2.FindOptions
    transaction_id: str
    def __init__(self, database: _Optional[str] = ..., collection: _Optional[str] = ..., filter: _Optional[_Union[_types_pb2.Filter, _Mapping]] = ..., options: _Optional[_Union[_types_pb2.FindOptions, _Mapping]] = ..., transaction_id: _Optional[str] = ...) -> None: ...

class FindOneResponse(_message.Message):
    __slots__ = ("document", "metadata")
    DOCUMENT_FIELD_NUMBER: _ClassVar[int]
    METADATA_FIELD_NUMBER: _ClassVar[int]
    document: _types_pb2.Document
    metadata: _types_pb2.ResponseMetadata
    def __init__(self, document: _Optional[_Union[_types_pb2.Document, _Mapping]] = ..., metadata: _Optional[_Union[_types_pb2.ResponseMetadata, _Mapping]] = ...) -> None: ...

class InsertRequest(_message.Message):
    __slots__ = ("database", "collection", "document", "transaction_id")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    DOCUMENT_FIELD_NUMBER: _ClassVar[int]
    TRANSACTION_ID_FIELD_NUMBER: _ClassVar[int]
    database: str
    collection: str
    document: _types_pb2.Document
    transaction_id: str
    def __init__(self, database: _Optional[str] = ..., collection: _Optional[str] = ..., document: _Optional[_Union[_types_pb2.Document, _Mapping]] = ..., transaction_id: _Optional[str] = ...) -> None: ...

class InsertResponse(_message.Message):
    __slots__ = ("inserted_id",)
    INSERTED_ID_FIELD_NUMBER: _ClassVar[int]
    inserted_id: str
    def __init__(self, inserted_id: _Optional[str] = ...) -> None: ...

class InsertManyRequest(_message.Message):
    __slots__ = ("database", "collection", "documents", "transaction_id")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    DOCUMENTS_FIELD_NUMBER: _ClassVar[int]
    TRANSACTION_ID_FIELD_NUMBER: _ClassVar[int]
    database: str
    collection: str
    documents: _containers.RepeatedCompositeFieldContainer[_types_pb2.Document]
    transaction_id: str
    def __init__(self, database: _Optional[str] = ..., collection: _Optional[str] = ..., documents: _Optional[_Iterable[_Union[_types_pb2.Document, _Mapping]]] = ..., transaction_id: _Optional[str] = ...) -> None: ...

class InsertManyResponse(_message.Message):
    __slots__ = ("inserted_ids", "inserted_count")
    INSERTED_IDS_FIELD_NUMBER: _ClassVar[int]
    INSERTED_COUNT_FIELD_NUMBER: _ClassVar[int]
    inserted_ids: _containers.RepeatedScalarFieldContainer[str]
    inserted_count: int
    def __init__(self, inserted_ids: _Optional[_Iterable[str]] = ..., inserted_count: _Optional[int] = ...) -> None: ...

class UpdateRequest(_message.Message):
    __slots__ = ("database", "collection", "filter", "update", "upsert", "transaction_id")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    FILTER_FIELD_NUMBER: _ClassVar[int]
    UPDATE_FIELD_NUMBER: _ClassVar[int]
    UPSERT_FIELD_NUMBER: _ClassVar[int]
    TRANSACTION_ID_FIELD_NUMBER: _ClassVar[int]
    database: str
    collection: str
    filter: _types_pb2.Filter
    update: _types_pb2.Document
    upsert: bool
    transaction_id: str
    def __init__(self, database: _Optional[str] = ..., collection: _Optional[str] = ..., filter: _Optional[_Union[_types_pb2.Filter, _Mapping]] = ..., update: _Optional[_Union[_types_pb2.Document, _Mapping]] = ..., upsert: bool = ..., transaction_id: _Optional[str] = ...) -> None: ...

class UpdateResponse(_message.Message):
    __slots__ = ("matched_count", "modified_count", "upserted_id")
    MATCHED_COUNT_FIELD_NUMBER: _ClassVar[int]
    MODIFIED_COUNT_FIELD_NUMBER: _ClassVar[int]
    UPSERTED_ID_FIELD_NUMBER: _ClassVar[int]
    matched_count: int
    modified_count: int
    upserted_id: str
    def __init__(self, matched_count: _Optional[int] = ..., modified_count: _Optional[int] = ..., upserted_id: _Optional[str] = ...) -> None: ...

class UpdateManyRequest(_message.Message):
    __slots__ = ("database", "collection", "filter", "update", "upsert", "transaction_id")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    FILTER_FIELD_NUMBER: _ClassVar[int]
    UPDATE_FIELD_NUMBER: _ClassVar[int]
    UPSERT_FIELD_NUMBER: _ClassVar[int]
    TRANSACTION_ID_FIELD_NUMBER: _ClassVar[int]
    database: str
    collection: str
    filter: _types_pb2.Filter
    update: _types_pb2.Document
    upsert: bool
    transaction_id: str
    def __init__(self, database: _Optional[str] = ..., collection: _Optional[str] = ..., filter: _Optional[_Union[_types_pb2.Filter, _Mapping]] = ..., update: _Optional[_Union[_types_pb2.Document, _Mapping]] = ..., upsert: bool = ..., transaction_id: _Optional[str] = ...) -> None: ...

class UpdateManyResponse(_message.Message):
    __slots__ = ("matched_count", "modified_count", "upserted_id")
    MATCHED_COUNT_FIELD_NUMBER: _ClassVar[int]
    MODIFIED_COUNT_FIELD_NUMBER: _ClassVar[int]
    UPSERTED_ID_FIELD_NUMBER: _ClassVar[int]
    matched_count: int
    modified_count: int
    upserted_id: str
    def __init__(self, matched_count: _Optional[int] = ..., modified_count: _Optional[int] = ..., upserted_id: _Optional[str] = ...) -> None: ...

class DeleteRequest(_message.Message):
    __slots__ = ("database", "collection", "filter", "transaction_id")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    FILTER_FIELD_NUMBER: _ClassVar[int]
    TRANSACTION_ID_FIELD_NUMBER: _ClassVar[int]
    database: str
    collection: str
    filter: _types_pb2.Filter
    transaction_id: str
    def __init__(self, database: _Optional[str] = ..., collection: _Optional[str] = ..., filter: _Optional[_Union[_types_pb2.Filter, _Mapping]] = ..., transaction_id: _Optional[str] = ...) -> None: ...

class DeleteResponse(_message.Message):
    __slots__ = ("deleted_count",)
    DELETED_COUNT_FIELD_NUMBER: _ClassVar[int]
    deleted_count: int
    def __init__(self, deleted_count: _Optional[int] = ...) -> None: ...

class DeleteManyRequest(_message.Message):
    __slots__ = ("database", "collection", "filter", "transaction_id")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    FILTER_FIELD_NUMBER: _ClassVar[int]
    TRANSACTION_ID_FIELD_NUMBER: _ClassVar[int]
    database: str
    collection: str
    filter: _types_pb2.Filter
    transaction_id: str
    def __init__(self, database: _Optional[str] = ..., collection: _Optional[str] = ..., filter: _Optional[_Union[_types_pb2.Filter, _Mapping]] = ..., transaction_id: _Optional[str] = ...) -> None: ...

class DeleteManyResponse(_message.Message):
    __slots__ = ("deleted_count",)
    DELETED_COUNT_FIELD_NUMBER: _ClassVar[int]
    deleted_count: int
    def __init__(self, deleted_count: _Optional[int] = ...) -> None: ...

class FindAndModifyRequest(_message.Message):
    __slots__ = ("database", "collection", "filter", "update", "options", "transaction_id")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    FILTER_FIELD_NUMBER: _ClassVar[int]
    UPDATE_FIELD_NUMBER: _ClassVar[int]
    OPTIONS_FIELD_NUMBER: _ClassVar[int]
    TRANSACTION_ID_FIELD_NUMBER: _ClassVar[int]
    database: str
    collection: str
    filter: _types_pb2.Filter
    update: _types_pb2.Document
    options: _types_pb2.FindAndModifyOptions
    transaction_id: str
    def __init__(self, database: _Optional[str] = ..., collection: _Optional[str] = ..., filter: _Optional[_Union[_types_pb2.Filter, _Mapping]] = ..., update: _Optional[_Union[_types_pb2.Document, _Mapping]] = ..., options: _Optional[_Union[_types_pb2.FindAndModifyOptions, _Mapping]] = ..., transaction_id: _Optional[str] = ...) -> None: ...

class FindAndModifyResponse(_message.Message):
    __slots__ = ("document",)
    DOCUMENT_FIELD_NUMBER: _ClassVar[int]
    document: _types_pb2.Document
    def __init__(self, document: _Optional[_Union[_types_pb2.Document, _Mapping]] = ...) -> None: ...

class AggregateRequest(_message.Message):
    __slots__ = ("database", "collection", "pipeline", "transaction_id")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    PIPELINE_FIELD_NUMBER: _ClassVar[int]
    TRANSACTION_ID_FIELD_NUMBER: _ClassVar[int]
    database: str
    collection: str
    pipeline: _types_pb2.Pipeline
    transaction_id: str
    def __init__(self, database: _Optional[str] = ..., collection: _Optional[str] = ..., pipeline: _Optional[_Union[_types_pb2.Pipeline, _Mapping]] = ..., transaction_id: _Optional[str] = ...) -> None: ...

class AggregateResponse(_message.Message):
    __slots__ = ("documents", "metadata")
    DOCUMENTS_FIELD_NUMBER: _ClassVar[int]
    METADATA_FIELD_NUMBER: _ClassVar[int]
    documents: _containers.RepeatedCompositeFieldContainer[_types_pb2.Document]
    metadata: _types_pb2.ResponseMetadata
    def __init__(self, documents: _Optional[_Iterable[_Union[_types_pb2.Document, _Mapping]]] = ..., metadata: _Optional[_Union[_types_pb2.ResponseMetadata, _Mapping]] = ...) -> None: ...

class BeginTransactionRequest(_message.Message):
    __slots__ = ("database",)
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    database: str
    def __init__(self, database: _Optional[str] = ...) -> None: ...

class BeginTransactionResponse(_message.Message):
    __slots__ = ("transaction_id",)
    TRANSACTION_ID_FIELD_NUMBER: _ClassVar[int]
    transaction_id: str
    def __init__(self, transaction_id: _Optional[str] = ...) -> None: ...

class CommitTransactionRequest(_message.Message):
    __slots__ = ("transaction_id",)
    TRANSACTION_ID_FIELD_NUMBER: _ClassVar[int]
    transaction_id: str
    def __init__(self, transaction_id: _Optional[str] = ...) -> None: ...

class CommitTransactionResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class AbortTransactionRequest(_message.Message):
    __slots__ = ("transaction_id",)
    TRANSACTION_ID_FIELD_NUMBER: _ClassVar[int]
    transaction_id: str
    def __init__(self, transaction_id: _Optional[str] = ...) -> None: ...

class AbortTransactionResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class CreateCollectionRequest(_message.Message):
    __slots__ = ("database", "collection")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    database: str
    collection: str
    def __init__(self, database: _Optional[str] = ..., collection: _Optional[str] = ...) -> None: ...

class CreateCollectionResponse(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class CreateIndexRequest(_message.Message):
    __slots__ = ("database", "collection", "keys", "options")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    KEYS_FIELD_NUMBER: _ClassVar[int]
    OPTIONS_FIELD_NUMBER: _ClassVar[int]
    database: str
    collection: str
    keys: _types_pb2.Document
    options: _types_pb2.IndexOptions
    def __init__(self, database: _Optional[str] = ..., collection: _Optional[str] = ..., keys: _Optional[_Union[_types_pb2.Document, _Mapping]] = ..., options: _Optional[_Union[_types_pb2.IndexOptions, _Mapping]] = ...) -> None: ...

class CreateIndexResponse(_message.Message):
    __slots__ = ("index_name",)
    INDEX_NAME_FIELD_NUMBER: _ClassVar[int]
    index_name: str
    def __init__(self, index_name: _Optional[str] = ...) -> None: ...

class ListDatabasesRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class ListDatabasesResponse(_message.Message):
    __slots__ = ("databases",)
    DATABASES_FIELD_NUMBER: _ClassVar[int]
    databases: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, databases: _Optional[_Iterable[str]] = ...) -> None: ...

class ListCollectionsRequest(_message.Message):
    __slots__ = ("database",)
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    database: str
    def __init__(self, database: _Optional[str] = ...) -> None: ...

class ListCollectionsResponse(_message.Message):
    __slots__ = ("collections",)
    COLLECTIONS_FIELD_NUMBER: _ClassVar[int]
    collections: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, collections: _Optional[_Iterable[str]] = ...) -> None: ...

class SearchRequest(_message.Message):
    __slots__ = ("database", "collection", "query", "limit")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    QUERY_FIELD_NUMBER: _ClassVar[int]
    LIMIT_FIELD_NUMBER: _ClassVar[int]
    database: str
    collection: str
    query: str
    limit: int
    def __init__(self, database: _Optional[str] = ..., collection: _Optional[str] = ..., query: _Optional[str] = ..., limit: _Optional[int] = ...) -> None: ...

class SearchResponse(_message.Message):
    __slots__ = ("documents", "method", "total")
    DOCUMENTS_FIELD_NUMBER: _ClassVar[int]
    METHOD_FIELD_NUMBER: _ClassVar[int]
    TOTAL_FIELD_NUMBER: _ClassVar[int]
    documents: _containers.RepeatedCompositeFieldContainer[_types_pb2.Document]
    method: str
    total: int
    def __init__(self, documents: _Optional[_Iterable[_Union[_types_pb2.Document, _Mapping]]] = ..., method: _Optional[str] = ..., total: _Optional[int] = ...) -> None: ...

class WatchRequest(_message.Message):
    __slots__ = ("database", "collection", "pipeline")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    PIPELINE_FIELD_NUMBER: _ClassVar[int]
    database: str
    collection: str
    pipeline: _types_pb2.Pipeline
    def __init__(self, database: _Optional[str] = ..., collection: _Optional[str] = ..., pipeline: _Optional[_Union[_types_pb2.Pipeline, _Mapping]] = ...) -> None: ...

class WatchEvent(_message.Message):
    __slots__ = ("operation_type", "database", "collection", "document", "update_description", "document_key")
    class OperationType(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
        __slots__ = ()
        INSERT: _ClassVar[WatchEvent.OperationType]
        UPDATE: _ClassVar[WatchEvent.OperationType]
        DELETE: _ClassVar[WatchEvent.OperationType]
        REPLACE: _ClassVar[WatchEvent.OperationType]
        INVALIDATE: _ClassVar[WatchEvent.OperationType]
    INSERT: WatchEvent.OperationType
    UPDATE: WatchEvent.OperationType
    DELETE: WatchEvent.OperationType
    REPLACE: WatchEvent.OperationType
    INVALIDATE: WatchEvent.OperationType
    OPERATION_TYPE_FIELD_NUMBER: _ClassVar[int]
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    DOCUMENT_FIELD_NUMBER: _ClassVar[int]
    UPDATE_DESCRIPTION_FIELD_NUMBER: _ClassVar[int]
    DOCUMENT_KEY_FIELD_NUMBER: _ClassVar[int]
    operation_type: WatchEvent.OperationType
    database: str
    collection: str
    document: _types_pb2.Document
    update_description: _types_pb2.Document
    document_key: _types_pb2.Document
    def __init__(self, operation_type: _Optional[_Union[WatchEvent.OperationType, str]] = ..., database: _Optional[str] = ..., collection: _Optional[str] = ..., document: _Optional[_Union[_types_pb2.Document, _Mapping]] = ..., update_description: _Optional[_Union[_types_pb2.Document, _Mapping]] = ..., document_key: _Optional[_Union[_types_pb2.Document, _Mapping]] = ...) -> None: ...

class FindStreamRequest(_message.Message):
    __slots__ = ("database", "collection", "filter", "options", "transaction_id", "batch_size")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    FILTER_FIELD_NUMBER: _ClassVar[int]
    OPTIONS_FIELD_NUMBER: _ClassVar[int]
    TRANSACTION_ID_FIELD_NUMBER: _ClassVar[int]
    BATCH_SIZE_FIELD_NUMBER: _ClassVar[int]
    database: str
    collection: str
    filter: _types_pb2.Filter
    options: _types_pb2.FindOptions
    transaction_id: str
    batch_size: int
    def __init__(self, database: _Optional[str] = ..., collection: _Optional[str] = ..., filter: _Optional[_Union[_types_pb2.Filter, _Mapping]] = ..., options: _Optional[_Union[_types_pb2.FindOptions, _Mapping]] = ..., transaction_id: _Optional[str] = ..., batch_size: _Optional[int] = ...) -> None: ...

class AggregateStreamRequest(_message.Message):
    __slots__ = ("database", "collection", "pipeline", "transaction_id", "batch_size")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    PIPELINE_FIELD_NUMBER: _ClassVar[int]
    TRANSACTION_ID_FIELD_NUMBER: _ClassVar[int]
    BATCH_SIZE_FIELD_NUMBER: _ClassVar[int]
    database: str
    collection: str
    pipeline: _types_pb2.Pipeline
    transaction_id: str
    batch_size: int
    def __init__(self, database: _Optional[str] = ..., collection: _Optional[str] = ..., pipeline: _Optional[_Union[_types_pb2.Pipeline, _Mapping]] = ..., transaction_id: _Optional[str] = ..., batch_size: _Optional[int] = ...) -> None: ...

class RunCommandRequest(_message.Message):
    __slots__ = ("database", "command", "allow_all")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COMMAND_FIELD_NUMBER: _ClassVar[int]
    ALLOW_ALL_FIELD_NUMBER: _ClassVar[int]
    database: str
    command: _types_pb2.Document
    allow_all: bool
    def __init__(self, database: _Optional[str] = ..., command: _Optional[_Union[_types_pb2.Document, _Mapping]] = ..., allow_all: bool = ...) -> None: ...

class RunCommandResponse(_message.Message):
    __slots__ = ("result",)
    RESULT_FIELD_NUMBER: _ClassVar[int]
    result: _types_pb2.Document
    def __init__(self, result: _Optional[_Union[_types_pb2.Document, _Mapping]] = ...) -> None: ...

class GetAnalyticsRequest(_message.Message):
    __slots__ = ("window_seconds",)
    WINDOW_SECONDS_FIELD_NUMBER: _ClassVar[int]
    window_seconds: int
    def __init__(self, window_seconds: _Optional[int] = ...) -> None: ...

class GetAnalyticsResponse(_message.Message):
    __slots__ = ("total_operations", "total_errors", "error_rate", "p50_latency_ms", "p95_latency_ms", "p99_latency_ms", "top_operations", "top_collections")
    TOTAL_OPERATIONS_FIELD_NUMBER: _ClassVar[int]
    TOTAL_ERRORS_FIELD_NUMBER: _ClassVar[int]
    ERROR_RATE_FIELD_NUMBER: _ClassVar[int]
    P50_LATENCY_MS_FIELD_NUMBER: _ClassVar[int]
    P95_LATENCY_MS_FIELD_NUMBER: _ClassVar[int]
    P99_LATENCY_MS_FIELD_NUMBER: _ClassVar[int]
    TOP_OPERATIONS_FIELD_NUMBER: _ClassVar[int]
    TOP_COLLECTIONS_FIELD_NUMBER: _ClassVar[int]
    total_operations: int
    total_errors: int
    error_rate: float
    p50_latency_ms: float
    p95_latency_ms: float
    p99_latency_ms: float
    top_operations: _containers.RepeatedCompositeFieldContainer[OperationCount]
    top_collections: _containers.RepeatedCompositeFieldContainer[CollectionCount]
    def __init__(self, total_operations: _Optional[int] = ..., total_errors: _Optional[int] = ..., error_rate: _Optional[float] = ..., p50_latency_ms: _Optional[float] = ..., p95_latency_ms: _Optional[float] = ..., p99_latency_ms: _Optional[float] = ..., top_operations: _Optional[_Iterable[_Union[OperationCount, _Mapping]]] = ..., top_collections: _Optional[_Iterable[_Union[CollectionCount, _Mapping]]] = ...) -> None: ...

class OperationCount(_message.Message):
    __slots__ = ("operation", "count")
    OPERATION_FIELD_NUMBER: _ClassVar[int]
    COUNT_FIELD_NUMBER: _ClassVar[int]
    operation: str
    count: int
    def __init__(self, operation: _Optional[str] = ..., count: _Optional[int] = ...) -> None: ...

class CollectionCount(_message.Message):
    __slots__ = ("collection", "count")
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    COUNT_FIELD_NUMBER: _ClassVar[int]
    collection: str
    count: int
    def __init__(self, collection: _Optional[str] = ..., count: _Optional[int] = ...) -> None: ...

class PipelineRequest(_message.Message):
    __slots__ = ("operations",)
    OPERATIONS_FIELD_NUMBER: _ClassVar[int]
    operations: _containers.RepeatedCompositeFieldContainer[PipelineOperation]
    def __init__(self, operations: _Optional[_Iterable[_Union[PipelineOperation, _Mapping]]] = ...) -> None: ...

class PipelineOperation(_message.Message):
    __slots__ = ("find", "find_one", "insert", "insert_many", "update", "update_many", "delete", "delete_many", "aggregate", "find_and_modify", "run_command", "search", "create_collection", "create_index", "list_databases", "list_collections", "begin_transaction", "commit_transaction", "abort_transaction", "get_analytics")
    FIND_FIELD_NUMBER: _ClassVar[int]
    FIND_ONE_FIELD_NUMBER: _ClassVar[int]
    INSERT_FIELD_NUMBER: _ClassVar[int]
    INSERT_MANY_FIELD_NUMBER: _ClassVar[int]
    UPDATE_FIELD_NUMBER: _ClassVar[int]
    UPDATE_MANY_FIELD_NUMBER: _ClassVar[int]
    DELETE_FIELD_NUMBER: _ClassVar[int]
    DELETE_MANY_FIELD_NUMBER: _ClassVar[int]
    AGGREGATE_FIELD_NUMBER: _ClassVar[int]
    FIND_AND_MODIFY_FIELD_NUMBER: _ClassVar[int]
    RUN_COMMAND_FIELD_NUMBER: _ClassVar[int]
    SEARCH_FIELD_NUMBER: _ClassVar[int]
    CREATE_COLLECTION_FIELD_NUMBER: _ClassVar[int]
    CREATE_INDEX_FIELD_NUMBER: _ClassVar[int]
    LIST_DATABASES_FIELD_NUMBER: _ClassVar[int]
    LIST_COLLECTIONS_FIELD_NUMBER: _ClassVar[int]
    BEGIN_TRANSACTION_FIELD_NUMBER: _ClassVar[int]
    COMMIT_TRANSACTION_FIELD_NUMBER: _ClassVar[int]
    ABORT_TRANSACTION_FIELD_NUMBER: _ClassVar[int]
    GET_ANALYTICS_FIELD_NUMBER: _ClassVar[int]
    find: FindRequest
    find_one: FindOneRequest
    insert: InsertRequest
    insert_many: InsertManyRequest
    update: UpdateRequest
    update_many: UpdateManyRequest
    delete: DeleteRequest
    delete_many: DeleteManyRequest
    aggregate: AggregateRequest
    find_and_modify: FindAndModifyRequest
    run_command: RunCommandRequest
    search: SearchRequest
    create_collection: CreateCollectionRequest
    create_index: CreateIndexRequest
    list_databases: ListDatabasesRequest
    list_collections: ListCollectionsRequest
    begin_transaction: BeginTransactionRequest
    commit_transaction: CommitTransactionRequest
    abort_transaction: AbortTransactionRequest
    get_analytics: GetAnalyticsRequest
    def __init__(self, find: _Optional[_Union[FindRequest, _Mapping]] = ..., find_one: _Optional[_Union[FindOneRequest, _Mapping]] = ..., insert: _Optional[_Union[InsertRequest, _Mapping]] = ..., insert_many: _Optional[_Union[InsertManyRequest, _Mapping]] = ..., update: _Optional[_Union[UpdateRequest, _Mapping]] = ..., update_many: _Optional[_Union[UpdateManyRequest, _Mapping]] = ..., delete: _Optional[_Union[DeleteRequest, _Mapping]] = ..., delete_many: _Optional[_Union[DeleteManyRequest, _Mapping]] = ..., aggregate: _Optional[_Union[AggregateRequest, _Mapping]] = ..., find_and_modify: _Optional[_Union[FindAndModifyRequest, _Mapping]] = ..., run_command: _Optional[_Union[RunCommandRequest, _Mapping]] = ..., search: _Optional[_Union[SearchRequest, _Mapping]] = ..., create_collection: _Optional[_Union[CreateCollectionRequest, _Mapping]] = ..., create_index: _Optional[_Union[CreateIndexRequest, _Mapping]] = ..., list_databases: _Optional[_Union[ListDatabasesRequest, _Mapping]] = ..., list_collections: _Optional[_Union[ListCollectionsRequest, _Mapping]] = ..., begin_transaction: _Optional[_Union[BeginTransactionRequest, _Mapping]] = ..., commit_transaction: _Optional[_Union[CommitTransactionRequest, _Mapping]] = ..., abort_transaction: _Optional[_Union[AbortTransactionRequest, _Mapping]] = ..., get_analytics: _Optional[_Union[GetAnalyticsRequest, _Mapping]] = ...) -> None: ...

class PipelineResponse(_message.Message):
    __slots__ = ("results", "succeeded", "failed")
    RESULTS_FIELD_NUMBER: _ClassVar[int]
    SUCCEEDED_FIELD_NUMBER: _ClassVar[int]
    FAILED_FIELD_NUMBER: _ClassVar[int]
    results: _containers.RepeatedCompositeFieldContainer[PipelineResult]
    succeeded: int
    failed: int
    def __init__(self, results: _Optional[_Iterable[_Union[PipelineResult, _Mapping]]] = ..., succeeded: _Optional[int] = ..., failed: _Optional[int] = ...) -> None: ...

class PipelineResult(_message.Message):
    __slots__ = ("index", "find", "find_one", "insert", "insert_many", "update", "update_many", "delete", "delete_many", "aggregate", "find_and_modify", "run_command", "search", "create_collection", "create_index", "list_databases", "list_collections", "begin_transaction", "commit_transaction", "abort_transaction", "get_analytics", "error")
    INDEX_FIELD_NUMBER: _ClassVar[int]
    FIND_FIELD_NUMBER: _ClassVar[int]
    FIND_ONE_FIELD_NUMBER: _ClassVar[int]
    INSERT_FIELD_NUMBER: _ClassVar[int]
    INSERT_MANY_FIELD_NUMBER: _ClassVar[int]
    UPDATE_FIELD_NUMBER: _ClassVar[int]
    UPDATE_MANY_FIELD_NUMBER: _ClassVar[int]
    DELETE_FIELD_NUMBER: _ClassVar[int]
    DELETE_MANY_FIELD_NUMBER: _ClassVar[int]
    AGGREGATE_FIELD_NUMBER: _ClassVar[int]
    FIND_AND_MODIFY_FIELD_NUMBER: _ClassVar[int]
    RUN_COMMAND_FIELD_NUMBER: _ClassVar[int]
    SEARCH_FIELD_NUMBER: _ClassVar[int]
    CREATE_COLLECTION_FIELD_NUMBER: _ClassVar[int]
    CREATE_INDEX_FIELD_NUMBER: _ClassVar[int]
    LIST_DATABASES_FIELD_NUMBER: _ClassVar[int]
    LIST_COLLECTIONS_FIELD_NUMBER: _ClassVar[int]
    BEGIN_TRANSACTION_FIELD_NUMBER: _ClassVar[int]
    COMMIT_TRANSACTION_FIELD_NUMBER: _ClassVar[int]
    ABORT_TRANSACTION_FIELD_NUMBER: _ClassVar[int]
    GET_ANALYTICS_FIELD_NUMBER: _ClassVar[int]
    ERROR_FIELD_NUMBER: _ClassVar[int]
    index: int
    find: FindResponse
    find_one: FindOneResponse
    insert: InsertResponse
    insert_many: InsertManyResponse
    update: UpdateResponse
    update_many: UpdateManyResponse
    delete: DeleteResponse
    delete_many: DeleteManyResponse
    aggregate: AggregateResponse
    find_and_modify: FindAndModifyResponse
    run_command: RunCommandResponse
    search: SearchResponse
    create_collection: CreateCollectionResponse
    create_index: CreateIndexResponse
    list_databases: ListDatabasesResponse
    list_collections: ListCollectionsResponse
    begin_transaction: BeginTransactionResponse
    commit_transaction: CommitTransactionResponse
    abort_transaction: AbortTransactionResponse
    get_analytics: GetAnalyticsResponse
    error: PipelineError
    def __init__(self, index: _Optional[int] = ..., find: _Optional[_Union[FindResponse, _Mapping]] = ..., find_one: _Optional[_Union[FindOneResponse, _Mapping]] = ..., insert: _Optional[_Union[InsertResponse, _Mapping]] = ..., insert_many: _Optional[_Union[InsertManyResponse, _Mapping]] = ..., update: _Optional[_Union[UpdateResponse, _Mapping]] = ..., update_many: _Optional[_Union[UpdateManyResponse, _Mapping]] = ..., delete: _Optional[_Union[DeleteResponse, _Mapping]] = ..., delete_many: _Optional[_Union[DeleteManyResponse, _Mapping]] = ..., aggregate: _Optional[_Union[AggregateResponse, _Mapping]] = ..., find_and_modify: _Optional[_Union[FindAndModifyResponse, _Mapping]] = ..., run_command: _Optional[_Union[RunCommandResponse, _Mapping]] = ..., search: _Optional[_Union[SearchResponse, _Mapping]] = ..., create_collection: _Optional[_Union[CreateCollectionResponse, _Mapping]] = ..., create_index: _Optional[_Union[CreateIndexResponse, _Mapping]] = ..., list_databases: _Optional[_Union[ListDatabasesResponse, _Mapping]] = ..., list_collections: _Optional[_Union[ListCollectionsResponse, _Mapping]] = ..., begin_transaction: _Optional[_Union[BeginTransactionResponse, _Mapping]] = ..., commit_transaction: _Optional[_Union[CommitTransactionResponse, _Mapping]] = ..., abort_transaction: _Optional[_Union[AbortTransactionResponse, _Mapping]] = ..., get_analytics: _Optional[_Union[GetAnalyticsResponse, _Mapping]] = ..., error: _Optional[_Union[PipelineError, _Mapping]] = ...) -> None: ...

class PipelineError(_message.Message):
    __slots__ = ("code", "message")
    CODE_FIELD_NUMBER: _ClassVar[int]
    MESSAGE_FIELD_NUMBER: _ClassVar[int]
    code: int
    message: str
    def __init__(self, code: _Optional[int] = ..., message: _Optional[str] = ...) -> None: ...
