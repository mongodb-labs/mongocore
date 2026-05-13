"""MongoCore Python client."""

import os
import grpc
from typing import Optional
from .database import Database
from .sidecar import SidecarManager

_CLIENT_METADATA = [("x-client-language", "python")]

DEFAULT_SOCKET_PATH = "/tmp/mongocore.sock"
DEFAULT_ADDRESS = "localhost:50051"


class MongoClient:
    """Client for connecting to a MongoCore sidecar.

    Connection priority (auto-discovery when no explicit config):
      1. MONGOCORE_SOCKET_PATH env var → UDS
      2. /tmp/mongocore.sock exists → UDS
      3. MONGOCORE_ADDRESS env var → TCP
      4. localhost:50051 → TCP

    Usage:
        # Auto-discover (prefers UDS, falls back to TCP)
        client = MongoClient()

        # Explicit UDS
        client = MongoClient(socket_path="/tmp/mongocore.sock")

        # Explicit TCP
        client = MongoClient(address="custom-host:50051")
    """

    def __init__(
        self,
        address: Optional[str] = None,
        *,
        socket_path: Optional[str] = None,
        auto_spawn: bool = False,
        max_message_size: int = 64 * 1024 * 1024,
    ):
        self._address = address
        self._socket_path = socket_path
        self._auto_spawn = auto_spawn
        self._max_message_size = max_message_size
        self._sidecar = None
        self._channel = None
        self._transport = None

    @property
    def transport(self) -> Optional[str]:
        """The transport in use after connect(): 'uds' or 'tcp'."""
        return self._transport

    async def connect(self):
        """Connect to the MongoCore sidecar."""
        if self._auto_spawn:
            self._sidecar = SidecarManager()
            await self._sidecar.ensure_running()

        options = [
            ("grpc.max_send_message_length", self._max_message_size),
            ("grpc.max_receive_message_length", self._max_message_size),
        ]

        target = self._resolve_target()
        self._channel = grpc.aio.insecure_channel(target, options=options)
        await self._channel.channel_ready()
        return self

    def _resolve_target(self) -> str:
        """Resolve the gRPC target using auto-discovery."""
        # Explicit socket_path takes highest priority
        if self._socket_path:
            self._transport = "uds"
            return f"unix://{self._socket_path}"

        # Explicit address (no socket_path) → TCP
        if self._address:
            self._transport = "tcp"
            return self._address

        # Auto-discovery
        env_socket = os.environ.get("MONGOCORE_SOCKET_PATH")
        if env_socket:
            self._transport = "uds"
            return f"unix://{env_socket}"

        if os.path.exists(DEFAULT_SOCKET_PATH):
            self._transport = "uds"
            return f"unix://{DEFAULT_SOCKET_PATH}"

        env_addr = os.environ.get("MONGOCORE_ADDRESS")
        if env_addr:
            self._transport = "tcp"
            return env_addr

        self._transport = "tcp"
        return DEFAULT_ADDRESS

    async def close(self):
        """Close the connection."""
        if self._channel:
            await self._channel.close()
        if self._sidecar:
            self._sidecar.stop()

    def __getitem__(self, database: str) -> "Database":
        """Get a database by name: client['mydb']"""
        return Database(self, database)

    @property
    def channel(self) -> grpc.aio.Channel:
        """Get the underlying gRPC channel."""
        if self._channel is None:
            raise RuntimeError("Not connected. Call connect() first.")
        return self._channel

    async def list_databases(self) -> list[str]:
        """List all databases."""
        from .generated import mongocore_pb2, mongocore_pb2_grpc
        stub = mongocore_pb2_grpc.MongoCoreStub(self.channel)
        response = await stub.ListDatabases(mongocore_pb2.ListDatabasesRequest(), metadata=_CLIENT_METADATA)
        return list(response.databases)

    async def run_command(self, database: str, command: dict, allow_all: bool = False) -> dict:
        """Execute an arbitrary MongoDB command via raw passthrough."""
        from bson import encode, decode
        from .generated import mongocore_pb2, mongocore_pb2_grpc, types_pb2
        stub = mongocore_pb2_grpc.MongoCoreStub(self.channel)
        response = await stub.RunCommand(mongocore_pb2.RunCommandRequest(
            database=database,
            command=types_pb2.Document(data=encode(command)),
            allow_all=allow_all,
        ), metadata=_CLIENT_METADATA)
        return decode(response.result.data)

    async def ingest(
        self,
        file_path: str,
        database: str,
        collection: str,
        *,
        format: str = "auto",
        dedup_key: Optional[list[str]] = None,
        conflict_strategy: str = "skip",
        batch_size: int = 1000,
        concurrency: int = 4,
        expressions: Optional[list[str]] = None,
        schema_overrides: Optional[dict[str, str]] = None,
        sample_size: int = 1000,
    ) -> dict:
        """Start a file ingestion job."""
        from .generated import mongocore_pb2_grpc, ingestion_pb2
        stub = mongocore_pb2_grpc.MongoCoreStub(self.channel)

        format_enum = {
            "auto": ingestion_pb2.FILE_FORMAT_AUTO,
            "csv": ingestion_pb2.FILE_FORMAT_CSV,
            "json": ingestion_pb2.FILE_FORMAT_JSON,
            "ndjson": ingestion_pb2.FILE_FORMAT_NDJSON,
            "parquet": ingestion_pb2.FILE_FORMAT_PARQUET,
        }.get(format.lower(), ingestion_pb2.FILE_FORMAT_AUTO)

        conflict_enum = {
            "skip": ingestion_pb2.CONFLICT_STRATEGY_SKIP,
            "overwrite": ingestion_pb2.CONFLICT_STRATEGY_OVERWRITE,
            "merge": ingestion_pb2.CONFLICT_STRATEGY_MERGE,
        }.get(conflict_strategy.lower(), ingestion_pb2.CONFLICT_STRATEGY_SKIP)

        response = await stub.Ingest(ingestion_pb2.IngestRequest(
            file_path=file_path,
            database=database,
            collection=collection,
            format=format_enum,
            dedup_key=dedup_key or [],
            conflict_strategy=conflict_enum,
            batch_size=batch_size,
            concurrency=concurrency,
            expressions=expressions or [],
            schema_overrides=schema_overrides or {},
            sample_size=sample_size,
        ), metadata=_CLIENT_METADATA)
        return {
            "job_id": response.job_id,
            "status": response.status,
            "inferred_schema": dict(response.inferred_schema),
            "total_rows": response.total_rows,
        }

    async def ingest_status(self, job_id: str) -> dict:
        """Get ingestion job status."""
        from .generated import mongocore_pb2_grpc, ingestion_pb2
        stub = mongocore_pb2_grpc.MongoCoreStub(self.channel)
        response = await stub.GetIngestStatus(ingestion_pb2.GetIngestStatusRequest(job_id=job_id), metadata=_CLIENT_METADATA)
        return {
            "job_id": response.job_id,
            "status": response.status,
            "total_rows": response.total_rows,
            "rows_processed": response.rows_processed,
            "rows_inserted": response.rows_inserted,
            "rows_skipped": response.rows_skipped,
            "rows_failed": response.rows_failed,
            "elapsed_ms": response.elapsed_ms,
            "estimated_remaining_ms": response.estimated_remaining_ms,
        }

    async def list_ingest_jobs(self) -> list[dict]:
        """List all ingestion jobs."""
        from .generated import mongocore_pb2_grpc, ingestion_pb2
        stub = mongocore_pb2_grpc.MongoCoreStub(self.channel)
        response = await stub.ListIngestJobs(ingestion_pb2.ListIngestJobsRequest(), metadata=_CLIENT_METADATA)
        return [{"job_id": j.job_id, "file_path": j.file_path, "database": j.database,
                 "collection": j.collection, "status": j.status, "total_rows": j.total_rows,
                 "rows_processed": j.rows_processed} for j in response.jobs]

    async def cancel_ingest(self, job_id: str) -> bool:
        """Cancel a running ingestion job."""
        from .generated import mongocore_pb2_grpc, ingestion_pb2
        stub = mongocore_pb2_grpc.MongoCoreStub(self.channel)
        response = await stub.CancelIngest(ingestion_pb2.CancelIngestRequest(job_id=job_id), metadata=_CLIENT_METADATA)
        return response.success

    async def watch_directory(
        self,
        path: str,
        database: str,
        collection: str,
        *,
        file_pattern: str = "*.csv",
        conflict_strategy: str = "skip",
        dedup_key: Optional[list[str]] = None,
    ) -> str:
        """Start watching a directory for new files to ingest."""
        from .generated import mongocore_pb2_grpc, ingestion_pb2
        stub = mongocore_pb2_grpc.MongoCoreStub(self.channel)
        conflict_enum = {
            "skip": ingestion_pb2.CONFLICT_STRATEGY_SKIP,
            "overwrite": ingestion_pb2.CONFLICT_STRATEGY_OVERWRITE,
            "merge": ingestion_pb2.CONFLICT_STRATEGY_MERGE,
        }.get(conflict_strategy.lower(), ingestion_pb2.CONFLICT_STRATEGY_SKIP)
        response = await stub.WatchDirectory(ingestion_pb2.WatchDirectoryRequest(
            path=path, file_pattern=file_pattern, database=database,
            collection=collection, conflict_strategy=conflict_enum,
            dedup_key=dedup_key or [],
        ), metadata=_CLIENT_METADATA)
        return response.watch_id

    async def stop_watch(self, watch_id: str) -> bool:
        """Stop watching a directory."""
        from .generated import mongocore_pb2_grpc, ingestion_pb2
        stub = mongocore_pb2_grpc.MongoCoreStub(self.channel)
        response = await stub.StopWatch(ingestion_pb2.StopWatchRequest(watch_id=watch_id), metadata=_CLIENT_METADATA)
        return response.success

    async def begin_transaction(self) -> str:
        """Begin a new transaction, returns transaction_id."""
        from .generated import mongocore_pb2, mongocore_pb2_grpc
        stub = mongocore_pb2_grpc.MongoCoreStub(self.channel)
        response = await stub.BeginTransaction(mongocore_pb2.BeginTransactionRequest(), metadata=_CLIENT_METADATA)
        return response.transaction_id

    async def commit_transaction(self, transaction_id: str) -> bool:
        """Commit a transaction."""
        from .generated import mongocore_pb2, mongocore_pb2_grpc
        stub = mongocore_pb2_grpc.MongoCoreStub(self.channel)
        await stub.CommitTransaction(
            mongocore_pb2.CommitTransactionRequest(transaction_id=transaction_id),
            metadata=_CLIENT_METADATA,
        )
        return True

    async def abort_transaction(self, transaction_id: str) -> bool:
        """Abort a transaction."""
        from .generated import mongocore_pb2, mongocore_pb2_grpc
        stub = mongocore_pb2_grpc.MongoCoreStub(self.channel)
        await stub.AbortTransaction(
            mongocore_pb2.AbortTransactionRequest(transaction_id=transaction_id),
            metadata=_CLIENT_METADATA,
        )
        return True

    async def get_analytics(self) -> dict:
        """Get query analytics summary."""
        from .generated import mongocore_pb2, mongocore_pb2_grpc
        stub = mongocore_pb2_grpc.MongoCoreStub(self.channel)
        response = await stub.GetAnalytics(
            mongocore_pb2.GetAnalyticsRequest(window_seconds=0),
            metadata=_CLIENT_METADATA,
        )
        return {
            "total_operations": response.total_operations,
            "total_errors": response.total_errors,
            "error_rate": response.error_rate,
            "p50_latency_ms": response.p50_latency_ms,
            "p95_latency_ms": response.p95_latency_ms,
            "p99_latency_ms": response.p99_latency_ms,
        }

    async def __aenter__(self):
        await self.connect()
        return self

    async def __aexit__(self, *args):
        await self.close()
