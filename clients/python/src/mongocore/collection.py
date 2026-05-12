"""Collection operations."""

from typing import Any, AsyncIterator, Optional
from bson import encode, decode
_CLIENT_METADATA = [("x-client-language", "python")]


class Collection:
    """A collection handle with CRUD operations."""

    def __init__(self, client, database: str, name: str):
        self._client = client
        self._database = database
        self._name = name

    def _encode_doc(self, doc: dict) -> bytes:
        """Encode a Python dict to BSON bytes."""
        return encode(doc)

    def _decode_doc(self, data: bytes) -> dict:
        """Decode BSON bytes to a Python dict."""
        return decode(data)

    def _get_stub(self):
        from .generated import mongocore_pb2_grpc
        return mongocore_pb2_grpc.MongoCoreStub(self._client.channel)

    def _make_filter(self, filter_doc: Optional[dict] = None):
        from .generated import types_pb2
        if filter_doc:
            return types_pb2.Filter(data=self._encode_doc(filter_doc))
        return types_pb2.Filter(data=b"")

    def _make_document(self, doc: dict):
        from .generated import types_pb2
        return types_pb2.Document(data=self._encode_doc(doc))

    async def find(self, filter: Optional[dict] = None, *, limit: int = 0, skip: int = 0) -> list[dict]:
        """Find documents matching the filter."""
        from .generated import mongocore_pb2, types_pb2
        stub = self._get_stub()

        options = types_pb2.FindOptions()
        if limit:
            options.limit = limit
        if skip:
            options.skip = skip

        response = await stub.Find(mongocore_pb2.FindRequest(
            database=self._database,
            collection=self._name,
            filter=self._make_filter(filter),
            options=options,
        ), metadata=_CLIENT_METADATA)
        return [self._decode_doc(doc.data) for doc in response.documents]

    async def find_one(self, filter: Optional[dict] = None) -> Optional[dict]:
        """Find a single document."""
        from .generated import mongocore_pb2
        stub = self._get_stub()
        response = await stub.FindOne(mongocore_pb2.FindOneRequest(
            database=self._database,
            collection=self._name,
            filter=self._make_filter(filter),
        ), metadata=_CLIENT_METADATA)
        if response.document and response.document.data:
            return self._decode_doc(response.document.data)
        return None

    async def insert_one(self, document: dict) -> str:
        """Insert a single document. Returns the inserted ID."""
        from .generated import mongocore_pb2
        stub = self._get_stub()
        response = await stub.Insert(mongocore_pb2.InsertRequest(
            database=self._database,
            collection=self._name,
            document=self._make_document(document),
        ), metadata=_CLIENT_METADATA)
        return response.inserted_id

    async def insert_many(self, documents: list[dict]) -> list[str]:
        """Insert multiple documents. Returns list of inserted IDs."""
        from .generated import mongocore_pb2
        stub = self._get_stub()
        response = await stub.InsertMany(mongocore_pb2.InsertManyRequest(
            database=self._database,
            collection=self._name,
            documents=[self._make_document(d) for d in documents],
        ), metadata=_CLIENT_METADATA)
        return list(response.inserted_ids)

    async def update_one(self, filter: dict, update: dict) -> dict:
        """Update a single document. Returns {matched_count, modified_count}."""
        from .generated import mongocore_pb2
        stub = self._get_stub()
        response = await stub.Update(mongocore_pb2.UpdateRequest(
            database=self._database,
            collection=self._name,
            filter=self._make_filter(filter),
            update=self._make_document(update),
        ), metadata=_CLIENT_METADATA)
        return {"matched_count": response.matched_count, "modified_count": response.modified_count}

    async def update_many(self, filter: dict, update: dict) -> dict:
        """Update multiple documents."""
        from .generated import mongocore_pb2
        stub = self._get_stub()
        response = await stub.UpdateMany(mongocore_pb2.UpdateManyRequest(
            database=self._database,
            collection=self._name,
            filter=self._make_filter(filter),
            update=self._make_document(update),
        ), metadata=_CLIENT_METADATA)
        return {"matched_count": response.matched_count, "modified_count": response.modified_count}

    async def delete_one(self, filter: dict) -> int:
        """Delete a single document. Returns deleted count."""
        from .generated import mongocore_pb2
        stub = self._get_stub()
        response = await stub.Delete(mongocore_pb2.DeleteRequest(
            database=self._database,
            collection=self._name,
            filter=self._make_filter(filter),
        ), metadata=_CLIENT_METADATA)
        return response.deleted_count

    async def delete_many(self, filter: dict) -> int:
        """Delete multiple documents. Returns deleted count."""
        from .generated import mongocore_pb2
        stub = self._get_stub()
        response = await stub.DeleteMany(mongocore_pb2.DeleteManyRequest(
            database=self._database,
            collection=self._name,
            filter=self._make_filter(filter),
        ), metadata=_CLIENT_METADATA)
        return response.deleted_count

    async def aggregate(self, pipeline: list[dict]) -> list[dict]:
        """Run an aggregation pipeline."""
        from .generated import mongocore_pb2, types_pb2
        stub = self._get_stub()

        stages = [self._encode_doc(stage) for stage in pipeline]

        response = await stub.Aggregate(mongocore_pb2.AggregateRequest(
            database=self._database,
            collection=self._name,
            pipeline=types_pb2.Pipeline(stages=stages),
        ), metadata=_CLIENT_METADATA)
        return [self._decode_doc(doc.data) for doc in response.documents]

    async def search(self, query: str, *, limit: int = 10) -> dict:
        """Search documents using the best available method (vector → fulltext → filter)."""
        from .generated import mongocore_pb2
        stub = self._get_stub()
        response = await stub.Search(mongocore_pb2.SearchRequest(
            database=self._database,
            collection=self._name,
            query=query,
            limit=limit,
        ), metadata=_CLIENT_METADATA)
        return {
            "documents": [self._decode_doc(doc.data) for doc in response.documents],
            "method": response.method,
            "total": response.total,
        }

    async def find_and_modify(self, filter: dict, update: dict, *, return_new: bool = True, upsert: bool = False) -> Optional[dict]:
        """Atomically find and modify a document, returning the result."""
        from .generated import mongocore_pb2, types_pb2
        stub = self._get_stub()
        options = types_pb2.FindAndModifyOptions(
            return_document=types_pb2.FindAndModifyOptions.AFTER if return_new else types_pb2.FindAndModifyOptions.BEFORE,
            upsert=upsert,
        )
        request = mongocore_pb2.FindAndModifyRequest(
            database=self._database,
            collection=self._name,
            filter=self._make_filter(filter),
            update=self._make_document(update),
            options=options,
        )
        response = await stub.FindAndModify(request, metadata=_CLIENT_METADATA)
        if response.document and response.document.data:
            return self._decode_doc(response.document.data)
        return None

    async def create_index(self, keys: dict, *, unique: bool = False, name: Optional[str] = None) -> str:
        """Create an index on the collection."""
        from .generated import mongocore_pb2
        stub = self._get_stub()
        request = mongocore_pb2.CreateIndexRequest(
            database=self._database,
            collection=self._name,
            keys=self._make_document(keys),
            unique=unique,
        )
        if name:
            request.name = name
        response = await stub.CreateIndex(request, metadata=_CLIENT_METADATA)
        return response.name

    def watch(self, pipeline: Optional[list[dict]] = None) -> "ChangeStream":
        """Open a change stream on this collection. Returns an async context manager."""
        return ChangeStream(self, pipeline)


class ChangeStream:
    """An async iterable change stream that auto-closes when exiting the context."""

    def __init__(self, collection: Collection, pipeline: Optional[list[dict]] = None):
        self._collection = collection
        self._pipeline = pipeline
        self._stream = None
        self._cancelled = False

    async def __aenter__(self) -> "ChangeStream":
        from .generated import mongocore_pb2, types_pb2
        stub = self._collection._get_stub()

        stages = [self._collection._encode_doc(s) for s in self._pipeline] if self._pipeline else []

        self._stream = stub.Watch(mongocore_pb2.WatchRequest(
            database=self._collection._database,
            collection=self._collection._name,
            pipeline=types_pb2.Pipeline(stages=stages) if stages else None,
        ), metadata=_CLIENT_METADATA)
        return self

    async def __aexit__(self, *exc):
        self._cancelled = True
        if self._stream is not None:
            self._stream.cancel()
        return False

    def __aiter__(self) -> AsyncIterator[dict]:
        return self

    async def __anext__(self) -> dict:
        if self._cancelled or self._stream is None:
            raise StopAsyncIteration
        try:
            event = await self._stream.read()
            if event is None:
                raise StopAsyncIteration
            result = {"operation_type": event.operation_type}
            if event.database:
                result["database"] = event.database
            if event.collection:
                result["collection"] = event.collection
            if event.document and event.document.data:
                result["document"] = self._collection._decode_doc(event.document.data)
            if event.update_description and event.update_description.data:
                result["update_description"] = self._collection._decode_doc(event.update_description.data)
            if event.document_key and event.document_key.data:
                result["document_key"] = self._collection._decode_doc(event.document_key.data)
            return result
        except Exception:
            if self._cancelled:
                raise StopAsyncIteration
            raise
