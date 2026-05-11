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

    async def __aenter__(self):
        await self.connect()
        return self

    async def __aexit__(self, *args):
        await self.close()
