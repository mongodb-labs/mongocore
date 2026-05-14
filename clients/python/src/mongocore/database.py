"""Database handle."""

from .collection import Collection
_CLIENT_METADATA = [("x-client-language", "python")]


def _parse_pipeline_response(response):
    """Parse a TransactionPipelineResponse into a friendly dict."""
    steps = []
    for step in response.steps:
        steps.append({"name": step.name, "success": step.success})

    summary = None
    if response.summary:
        summary = {
            "total_steps": response.summary.total_steps,
            "steps_completed": response.summary.steps_completed,
            "elapsed_ms": response.summary.elapsed_ms,
        }

    return {"steps": steps, "summary": summary}


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
            mongocore_pb2.ListCollectionsRequest(database=self._name),
            metadata=_CLIENT_METADATA
        )
        return list(response.collections)

    async def create_collection(self, name: str):
        """Create a new collection."""
        from .generated import mongocore_pb2, mongocore_pb2_grpc
        stub = mongocore_pb2_grpc.MongoCoreStub(self._client.channel)
        await stub.CreateCollection(
            mongocore_pb2.CreateCollectionRequest(database=self._name, collection=name),
            metadata=_CLIENT_METADATA
        )

    async def transaction_pipeline(self, steps: list, *, options: dict = None):
        """Execute a transactional pipeline across collections in this database.

        Each step is a TransactionStep(name, operation, collection).
        """
        from .generated import mongocore_pb2, mongocore_pb2_grpc
        stub = mongocore_pb2_grpc.MongoCoreStub(self._client.channel)

        proto_steps = [
            self._client._build_transaction_step(s, self._name)
            for s in steps
        ]

        request = mongocore_pb2.TransactionPipelineRequest(steps=proto_steps)
        if options:
            request.options.CopyFrom(mongocore_pb2.TransactionPipelineOptions(
                read_concern=options.get("read_concern", ""),
                write_concern=options.get("write_concern", ""),
                max_time_ms=options.get("max_time_ms", 0),
            ))

        response = await stub.TransactionPipeline(request, metadata=_CLIENT_METADATA)
        return _parse_pipeline_response(response)
