"""Database handle."""

from .collection import Collection


class Database:
    """A database handle providing access to collections."""

    def __init__(self, client, name: str):
        self._client = client
        self._name = name

    @property
    def name(self) -> str:
        return self._name

    def __getitem__(self, collection: str) -> "Collection":
        """Get a collection: db['users']"""
        return Collection(self._client, self._name, collection)

    async def list_collections(self) -> list[str]:
        """List all collections in this database."""
        from .generated import mongocore_pb2, mongocore_pb2_grpc
        stub = mongocore_pb2_grpc.MongoCoreStub(self._client.channel)
        response = await stub.ListCollections(
            mongocore_pb2.ListCollectionsRequest(database=self._name)
        )
        return list(response.collections)

    async def create_collection(self, name: str):
        """Create a new collection."""
        from .generated import mongocore_pb2, mongocore_pb2_grpc
        stub = mongocore_pb2_grpc.MongoCoreStub(self._client.channel)
        await stub.CreateCollection(
            mongocore_pb2.CreateCollectionRequest(database=self._name, collection=name)
        )
