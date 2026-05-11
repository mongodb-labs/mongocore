from mongocore.v1 import types_pb2 as _types_pb2
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
