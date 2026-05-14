from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class Document(_message.Message):
    __slots__ = ("data",)
    DATA_FIELD_NUMBER: _ClassVar[int]
    data: bytes
    def __init__(self, data: _Optional[bytes] = ...) -> None: ...

class Filter(_message.Message):
    __slots__ = ("data",)
    DATA_FIELD_NUMBER: _ClassVar[int]
    data: bytes
    def __init__(self, data: _Optional[bytes] = ...) -> None: ...

class Pipeline(_message.Message):
    __slots__ = ("stages",)
    STAGES_FIELD_NUMBER: _ClassVar[int]
    stages: _containers.RepeatedScalarFieldContainer[bytes]
    def __init__(self, stages: _Optional[_Iterable[bytes]] = ...) -> None: ...

class FindOptions(_message.Message):
    __slots__ = ("limit", "skip", "sort", "projection")
    LIMIT_FIELD_NUMBER: _ClassVar[int]
    SKIP_FIELD_NUMBER: _ClassVar[int]
    SORT_FIELD_NUMBER: _ClassVar[int]
    PROJECTION_FIELD_NUMBER: _ClassVar[int]
    limit: int
    skip: int
    sort: bytes
    projection: bytes
    def __init__(self, limit: _Optional[int] = ..., skip: _Optional[int] = ..., sort: _Optional[bytes] = ..., projection: _Optional[bytes] = ...) -> None: ...

class IndexOptions(_message.Message):
    __slots__ = ("name", "unique", "sparse")
    NAME_FIELD_NUMBER: _ClassVar[int]
    UNIQUE_FIELD_NUMBER: _ClassVar[int]
    SPARSE_FIELD_NUMBER: _ClassVar[int]
    name: str
    unique: bool
    sparse: bool
    def __init__(self, name: _Optional[str] = ..., unique: bool = ..., sparse: bool = ...) -> None: ...

class FindAndModifyOptions(_message.Message):
    __slots__ = ("return_document", "upsert", "sort")
    class ReturnDocument(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
        __slots__ = ()
        BEFORE: _ClassVar[FindAndModifyOptions.ReturnDocument]
        AFTER: _ClassVar[FindAndModifyOptions.ReturnDocument]
    BEFORE: FindAndModifyOptions.ReturnDocument
    AFTER: FindAndModifyOptions.ReturnDocument
    RETURN_DOCUMENT_FIELD_NUMBER: _ClassVar[int]
    UPSERT_FIELD_NUMBER: _ClassVar[int]
    SORT_FIELD_NUMBER: _ClassVar[int]
    return_document: FindAndModifyOptions.ReturnDocument
    upsert: bool
    sort: bytes
    def __init__(self, return_document: _Optional[_Union[FindAndModifyOptions.ReturnDocument, str]] = ..., upsert: bool = ..., sort: _Optional[bytes] = ...) -> None: ...

class ResponseMetadata(_message.Message):
    __slots__ = ("search_method",)
    SEARCH_METHOD_FIELD_NUMBER: _ClassVar[int]
    search_method: str
    def __init__(self, search_method: _Optional[str] = ...) -> None: ...

class DocumentBatch(_message.Message):
    __slots__ = ("documents", "batch_index", "has_more")
    DOCUMENTS_FIELD_NUMBER: _ClassVar[int]
    BATCH_INDEX_FIELD_NUMBER: _ClassVar[int]
    HAS_MORE_FIELD_NUMBER: _ClassVar[int]
    documents: _containers.RepeatedCompositeFieldContainer[Document]
    batch_index: int
    has_more: bool
    def __init__(self, documents: _Optional[_Iterable[_Union[Document, _Mapping]]] = ..., batch_index: _Optional[int] = ..., has_more: bool = ...) -> None: ...

class InsertBatch(_message.Message):
    __slots__ = ("database", "collection", "documents")
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    DOCUMENTS_FIELD_NUMBER: _ClassVar[int]
    database: str
    collection: str
    documents: _containers.RepeatedCompositeFieldContainer[Document]
    def __init__(self, database: _Optional[str] = ..., collection: _Optional[str] = ..., documents: _Optional[_Iterable[_Union[Document, _Mapping]]] = ...) -> None: ...

class InsertBatchAck(_message.Message):
    __slots__ = ("batch_index", "inserted_count", "errors")
    BATCH_INDEX_FIELD_NUMBER: _ClassVar[int]
    INSERTED_COUNT_FIELD_NUMBER: _ClassVar[int]
    ERRORS_FIELD_NUMBER: _ClassVar[int]
    batch_index: int
    inserted_count: int
    errors: _containers.RepeatedCompositeFieldContainer[InsertError]
    def __init__(self, batch_index: _Optional[int] = ..., inserted_count: _Optional[int] = ..., errors: _Optional[_Iterable[_Union[InsertError, _Mapping]]] = ...) -> None: ...

class InsertError(_message.Message):
    __slots__ = ("index", "message", "code")
    INDEX_FIELD_NUMBER: _ClassVar[int]
    MESSAGE_FIELD_NUMBER: _ClassVar[int]
    CODE_FIELD_NUMBER: _ClassVar[int]
    index: int
    message: str
    code: int
    def __init__(self, index: _Optional[int] = ..., message: _Optional[str] = ..., code: _Optional[int] = ...) -> None: ...

class InsertManyStreamResponse(_message.Message):
    __slots__ = ("total_inserted", "errors")
    TOTAL_INSERTED_FIELD_NUMBER: _ClassVar[int]
    ERRORS_FIELD_NUMBER: _ClassVar[int]
    total_inserted: int
    errors: _containers.RepeatedCompositeFieldContainer[InsertError]
    def __init__(self, total_inserted: _Optional[int] = ..., errors: _Optional[_Iterable[_Union[InsertError, _Mapping]]] = ...) -> None: ...
