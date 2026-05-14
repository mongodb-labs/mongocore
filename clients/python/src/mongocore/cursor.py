"""Async cursor over streaming gRPC query results."""


_CLIENT_METADATA = [("x-client-language", "python")]


class Cursor:
    """Async iterator that yields documents from a streaming gRPC call.

    The underlying RPC is not called until iteration begins (lazy).
    """

    def __init__(self, stub, request, rpc_method: str, decode_fn):
        self._stub = stub
        self._request = request
        self._rpc_method = rpc_method
        self._decode_fn = decode_fn
        self._stream = None
        self._buffer: list = []
        self._buffer_index: int = 0
        self._exhausted: bool = False

    def __aiter__(self):
        return self

    async def __anext__(self):
        if self._buffer_index < len(self._buffer):
            doc = self._buffer[self._buffer_index]
            self._buffer_index += 1
            return doc

        if self._exhausted:
            raise StopAsyncIteration

        await self._fetch_next_batch()

        if self._buffer_index < len(self._buffer):
            doc = self._buffer[self._buffer_index]
            self._buffer_index += 1
            return doc

        raise StopAsyncIteration

    async def _fetch_next_batch(self):
        if self._stream is None:
            rpc = getattr(self._stub, self._rpc_method)
            self._stream = rpc(self._request, metadata=_CLIENT_METADATA)

        try:
            batch = await self._stream.read()
            if batch is None:
                self._exhausted = True
                return
        except Exception:
            self._exhausted = True
            raise

        self._buffer = [self._decode_fn(doc.data) for doc in batch.documents]
        self._buffer_index = 0

        if not batch.has_more:
            self._exhausted = True

    async def to_list(self) -> list:
        """Collect all documents into a list."""
        results = []
        async for doc in self:
            results.append(doc)
        return results

    async def close(self):
        """Cancel the underlying stream."""
        if self._stream is not None:
            self._stream.cancel()
            self._stream = None
        self._exhausted = True
