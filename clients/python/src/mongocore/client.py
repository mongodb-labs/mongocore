"""MongoCore Python client."""

import grpc
from typing import Optional
from .database import Database
from .sidecar import SidecarManager


class MongoClient:
    """Client for connecting to a MongoCore sidecar.

    Usage:
        client = MongoClient("localhost:50051")
        db = client["mydb"]
        result = await db["users"].find({"active": True})
    """

    def __init__(self, address: str = "localhost:50051", *, auto_spawn: bool = False):
        self._address = address
        self._auto_spawn = auto_spawn
        self._sidecar = None
        self._channel = None

    async def connect(self):
        """Connect to the MongoCore sidecar."""
        if self._auto_spawn:
            self._sidecar = SidecarManager()
            await self._sidecar.ensure_running()

        self._channel = grpc.aio.insecure_channel(self._address)
        await self._channel.channel_ready()
        return self

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
        response = await stub.ListDatabases(mongocore_pb2.ListDatabasesRequest())
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
        ))
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
        ))
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
        response = await stub.GetIngestStatus(ingestion_pb2.GetIngestStatusRequest(job_id=job_id))
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
        response = await stub.ListIngestJobs(ingestion_pb2.ListIngestJobsRequest())
        return [{"job_id": j.job_id, "file_path": j.file_path, "database": j.database,
                 "collection": j.collection, "status": j.status, "total_rows": j.total_rows,
                 "rows_processed": j.rows_processed} for j in response.jobs]

    async def cancel_ingest(self, job_id: str) -> bool:
        """Cancel a running ingestion job."""
        from .generated import mongocore_pb2_grpc, ingestion_pb2
        stub = mongocore_pb2_grpc.MongoCoreStub(self.channel)
        response = await stub.CancelIngest(ingestion_pb2.CancelIngestRequest(job_id=job_id))
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
        ))
        return response.watch_id

    async def stop_watch(self, watch_id: str) -> bool:
        """Stop watching a directory."""
        from .generated import mongocore_pb2_grpc, ingestion_pb2
        stub = mongocore_pb2_grpc.MongoCoreStub(self.channel)
        response = await stub.StopWatch(ingestion_pb2.StopWatchRequest(watch_id=watch_id))
        return response.success

    async def __aenter__(self):
        await self.connect()
        return self

    async def __aexit__(self, *args):
        await self.close()
