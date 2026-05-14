from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class FileFormat(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    FILE_FORMAT_AUTO: _ClassVar[FileFormat]
    FILE_FORMAT_CSV: _ClassVar[FileFormat]
    FILE_FORMAT_JSON: _ClassVar[FileFormat]
    FILE_FORMAT_NDJSON: _ClassVar[FileFormat]
    FILE_FORMAT_PARQUET: _ClassVar[FileFormat]

class ConflictStrategy(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    CONFLICT_STRATEGY_SKIP: _ClassVar[ConflictStrategy]
    CONFLICT_STRATEGY_OVERWRITE: _ClassVar[ConflictStrategy]
    CONFLICT_STRATEGY_MERGE: _ClassVar[ConflictStrategy]

class IngestJobStatus(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    INGEST_JOB_STATUS_RUNNING: _ClassVar[IngestJobStatus]
    INGEST_JOB_STATUS_COMPLETED: _ClassVar[IngestJobStatus]
    INGEST_JOB_STATUS_FAILED: _ClassVar[IngestJobStatus]
    INGEST_JOB_STATUS_CANCELLED: _ClassVar[IngestJobStatus]
FILE_FORMAT_AUTO: FileFormat
FILE_FORMAT_CSV: FileFormat
FILE_FORMAT_JSON: FileFormat
FILE_FORMAT_NDJSON: FileFormat
FILE_FORMAT_PARQUET: FileFormat
CONFLICT_STRATEGY_SKIP: ConflictStrategy
CONFLICT_STRATEGY_OVERWRITE: ConflictStrategy
CONFLICT_STRATEGY_MERGE: ConflictStrategy
INGEST_JOB_STATUS_RUNNING: IngestJobStatus
INGEST_JOB_STATUS_COMPLETED: IngestJobStatus
INGEST_JOB_STATUS_FAILED: IngestJobStatus
INGEST_JOB_STATUS_CANCELLED: IngestJobStatus

class CsvOptions(_message.Message):
    __slots__ = ("delimiter", "quote_char", "has_header", "comment_char")
    DELIMITER_FIELD_NUMBER: _ClassVar[int]
    QUOTE_CHAR_FIELD_NUMBER: _ClassVar[int]
    HAS_HEADER_FIELD_NUMBER: _ClassVar[int]
    COMMENT_CHAR_FIELD_NUMBER: _ClassVar[int]
    delimiter: str
    quote_char: str
    has_header: bool
    comment_char: str
    def __init__(self, delimiter: _Optional[str] = ..., quote_char: _Optional[str] = ..., has_header: bool = ..., comment_char: _Optional[str] = ...) -> None: ...

class IngestRequest(_message.Message):
    __slots__ = ("file_path", "database", "collection", "format", "dedup_key", "conflict_strategy", "batch_size", "concurrency", "expressions", "schema_overrides", "sample_size", "csv_options")
    class SchemaOverridesEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: str
        def __init__(self, key: _Optional[str] = ..., value: _Optional[str] = ...) -> None: ...
    FILE_PATH_FIELD_NUMBER: _ClassVar[int]
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    FORMAT_FIELD_NUMBER: _ClassVar[int]
    DEDUP_KEY_FIELD_NUMBER: _ClassVar[int]
    CONFLICT_STRATEGY_FIELD_NUMBER: _ClassVar[int]
    BATCH_SIZE_FIELD_NUMBER: _ClassVar[int]
    CONCURRENCY_FIELD_NUMBER: _ClassVar[int]
    EXPRESSIONS_FIELD_NUMBER: _ClassVar[int]
    SCHEMA_OVERRIDES_FIELD_NUMBER: _ClassVar[int]
    SAMPLE_SIZE_FIELD_NUMBER: _ClassVar[int]
    CSV_OPTIONS_FIELD_NUMBER: _ClassVar[int]
    file_path: str
    database: str
    collection: str
    format: FileFormat
    dedup_key: _containers.RepeatedScalarFieldContainer[str]
    conflict_strategy: ConflictStrategy
    batch_size: int
    concurrency: int
    expressions: _containers.RepeatedScalarFieldContainer[str]
    schema_overrides: _containers.ScalarMap[str, str]
    sample_size: int
    csv_options: CsvOptions
    def __init__(self, file_path: _Optional[str] = ..., database: _Optional[str] = ..., collection: _Optional[str] = ..., format: _Optional[_Union[FileFormat, str]] = ..., dedup_key: _Optional[_Iterable[str]] = ..., conflict_strategy: _Optional[_Union[ConflictStrategy, str]] = ..., batch_size: _Optional[int] = ..., concurrency: _Optional[int] = ..., expressions: _Optional[_Iterable[str]] = ..., schema_overrides: _Optional[_Mapping[str, str]] = ..., sample_size: _Optional[int] = ..., csv_options: _Optional[_Union[CsvOptions, _Mapping]] = ...) -> None: ...

class IngestResponse(_message.Message):
    __slots__ = ("job_id", "status", "inferred_schema", "total_rows")
    class InferredSchemaEntry(_message.Message):
        __slots__ = ("key", "value")
        KEY_FIELD_NUMBER: _ClassVar[int]
        VALUE_FIELD_NUMBER: _ClassVar[int]
        key: str
        value: str
        def __init__(self, key: _Optional[str] = ..., value: _Optional[str] = ...) -> None: ...
    JOB_ID_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    INFERRED_SCHEMA_FIELD_NUMBER: _ClassVar[int]
    TOTAL_ROWS_FIELD_NUMBER: _ClassVar[int]
    job_id: str
    status: IngestJobStatus
    inferred_schema: _containers.ScalarMap[str, str]
    total_rows: int
    def __init__(self, job_id: _Optional[str] = ..., status: _Optional[_Union[IngestJobStatus, str]] = ..., inferred_schema: _Optional[_Mapping[str, str]] = ..., total_rows: _Optional[int] = ...) -> None: ...

class GetIngestStatusRequest(_message.Message):
    __slots__ = ("job_id",)
    JOB_ID_FIELD_NUMBER: _ClassVar[int]
    job_id: str
    def __init__(self, job_id: _Optional[str] = ...) -> None: ...

class GetIngestStatusResponse(_message.Message):
    __slots__ = ("job_id", "status", "total_rows", "rows_processed", "rows_inserted", "rows_skipped", "rows_failed", "elapsed_ms", "estimated_remaining_ms")
    JOB_ID_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    TOTAL_ROWS_FIELD_NUMBER: _ClassVar[int]
    ROWS_PROCESSED_FIELD_NUMBER: _ClassVar[int]
    ROWS_INSERTED_FIELD_NUMBER: _ClassVar[int]
    ROWS_SKIPPED_FIELD_NUMBER: _ClassVar[int]
    ROWS_FAILED_FIELD_NUMBER: _ClassVar[int]
    ELAPSED_MS_FIELD_NUMBER: _ClassVar[int]
    ESTIMATED_REMAINING_MS_FIELD_NUMBER: _ClassVar[int]
    job_id: str
    status: IngestJobStatus
    total_rows: int
    rows_processed: int
    rows_inserted: int
    rows_skipped: int
    rows_failed: int
    elapsed_ms: int
    estimated_remaining_ms: int
    def __init__(self, job_id: _Optional[str] = ..., status: _Optional[_Union[IngestJobStatus, str]] = ..., total_rows: _Optional[int] = ..., rows_processed: _Optional[int] = ..., rows_inserted: _Optional[int] = ..., rows_skipped: _Optional[int] = ..., rows_failed: _Optional[int] = ..., elapsed_ms: _Optional[int] = ..., estimated_remaining_ms: _Optional[int] = ...) -> None: ...

class ListIngestJobsRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class ListIngestJobsResponse(_message.Message):
    __slots__ = ("jobs",)
    JOBS_FIELD_NUMBER: _ClassVar[int]
    jobs: _containers.RepeatedCompositeFieldContainer[IngestJobSummary]
    def __init__(self, jobs: _Optional[_Iterable[_Union[IngestJobSummary, _Mapping]]] = ...) -> None: ...

class IngestJobSummary(_message.Message):
    __slots__ = ("job_id", "file_path", "database", "collection", "status", "total_rows", "rows_processed")
    JOB_ID_FIELD_NUMBER: _ClassVar[int]
    FILE_PATH_FIELD_NUMBER: _ClassVar[int]
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    STATUS_FIELD_NUMBER: _ClassVar[int]
    TOTAL_ROWS_FIELD_NUMBER: _ClassVar[int]
    ROWS_PROCESSED_FIELD_NUMBER: _ClassVar[int]
    job_id: str
    file_path: str
    database: str
    collection: str
    status: IngestJobStatus
    total_rows: int
    rows_processed: int
    def __init__(self, job_id: _Optional[str] = ..., file_path: _Optional[str] = ..., database: _Optional[str] = ..., collection: _Optional[str] = ..., status: _Optional[_Union[IngestJobStatus, str]] = ..., total_rows: _Optional[int] = ..., rows_processed: _Optional[int] = ...) -> None: ...

class CancelIngestRequest(_message.Message):
    __slots__ = ("job_id",)
    JOB_ID_FIELD_NUMBER: _ClassVar[int]
    job_id: str
    def __init__(self, job_id: _Optional[str] = ...) -> None: ...

class CancelIngestResponse(_message.Message):
    __slots__ = ("success",)
    SUCCESS_FIELD_NUMBER: _ClassVar[int]
    success: bool
    def __init__(self, success: bool = ...) -> None: ...

class WatchDirectoryRequest(_message.Message):
    __slots__ = ("path", "file_pattern", "database", "collection", "conflict_strategy", "dedup_key")
    PATH_FIELD_NUMBER: _ClassVar[int]
    FILE_PATTERN_FIELD_NUMBER: _ClassVar[int]
    DATABASE_FIELD_NUMBER: _ClassVar[int]
    COLLECTION_FIELD_NUMBER: _ClassVar[int]
    CONFLICT_STRATEGY_FIELD_NUMBER: _ClassVar[int]
    DEDUP_KEY_FIELD_NUMBER: _ClassVar[int]
    path: str
    file_pattern: str
    database: str
    collection: str
    conflict_strategy: ConflictStrategy
    dedup_key: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, path: _Optional[str] = ..., file_pattern: _Optional[str] = ..., database: _Optional[str] = ..., collection: _Optional[str] = ..., conflict_strategy: _Optional[_Union[ConflictStrategy, str]] = ..., dedup_key: _Optional[_Iterable[str]] = ...) -> None: ...

class WatchDirectoryResponse(_message.Message):
    __slots__ = ("watch_id",)
    WATCH_ID_FIELD_NUMBER: _ClassVar[int]
    watch_id: str
    def __init__(self, watch_id: _Optional[str] = ...) -> None: ...

class StopWatchRequest(_message.Message):
    __slots__ = ("watch_id",)
    WATCH_ID_FIELD_NUMBER: _ClassVar[int]
    watch_id: str
    def __init__(self, watch_id: _Optional[str] = ...) -> None: ...

class StopWatchResponse(_message.Message):
    __slots__ = ("success",)
    SUCCESS_FIELD_NUMBER: _ClassVar[int]
    success: bool
    def __init__(self, success: bool = ...) -> None: ...
