"""Pipeline result wrapper."""

from typing import Optional
from bson import decode


class PipelineResult:
    """Result from a single pipeline operation."""

    def __init__(self, proto_result):
        """Initialize from a proto PipelineResult."""
        self._proto = proto_result

    @property
    def index(self) -> int:
        """The index of this operation in the pipeline."""
        return self._proto.index

    @property
    def success(self) -> bool:
        """Whether the operation succeeded."""
        return self._proto.WhichOneof("result") != "error"

    @property
    def error(self) -> Optional[str]:
        """Error message if the operation failed."""
        if self._proto.WhichOneof("result") == "error":
            return self._proto.error.message
        return None

    @property
    def error_code(self) -> Optional[int]:
        """Error code if the operation failed."""
        if self._proto.WhichOneof("result") == "error":
            return self._proto.error.code
        return None

    @property
    def documents(self) -> Optional[list[dict]]:
        """Documents returned by find or aggregate operations."""
        result_type = self._proto.WhichOneof("result")
        if result_type == "find":
            return [decode(doc.data) for doc in self._proto.find.documents]
        elif result_type == "aggregate":
            return [decode(doc.data) for doc in self._proto.aggregate.documents]
        elif result_type == "search":
            return [decode(doc.data) for doc in self._proto.search.documents]
        return None

    @property
    def document(self) -> Optional[dict]:
        """Document returned by find_one or find_and_modify operations."""
        result_type = self._proto.WhichOneof("result")
        if result_type == "find_one":
            if self._proto.find_one.document and self._proto.find_one.document.data:
                return decode(self._proto.find_one.document.data)
        elif result_type == "find_and_modify":
            if self._proto.find_and_modify.document and self._proto.find_and_modify.document.data:
                return decode(self._proto.find_and_modify.document.data)
        return None

    @property
    def inserted_id(self) -> Optional[str]:
        """Inserted ID from an insert operation."""
        result_type = self._proto.WhichOneof("result")
        if result_type == "insert":
            return self._proto.insert.inserted_id
        return None

    @property
    def inserted_ids(self) -> Optional[list[str]]:
        """Inserted IDs from an insert_many operation."""
        result_type = self._proto.WhichOneof("result")
        if result_type == "insert_many":
            return list(self._proto.insert_many.inserted_ids)
        return None

    @property
    def matched_count(self) -> Optional[int]:
        """Matched count from an update operation."""
        result_type = self._proto.WhichOneof("result")
        if result_type in ("update", "update_many"):
            return getattr(self._proto, result_type).matched_count
        return None

    @property
    def modified_count(self) -> Optional[int]:
        """Modified count from an update operation."""
        result_type = self._proto.WhichOneof("result")
        if result_type in ("update", "update_many"):
            return getattr(self._proto, result_type).modified_count
        return None

    @property
    def deleted_count(self) -> Optional[int]:
        """Deleted count from a delete operation."""
        result_type = self._proto.WhichOneof("result")
        if result_type in ("delete", "delete_many"):
            return getattr(self._proto, result_type).deleted_count
        return None

    @property
    def databases(self) -> Optional[list[str]]:
        """Databases from a list_databases operation."""
        result_type = self._proto.WhichOneof("result")
        if result_type == "list_databases":
            return list(self._proto.list_databases.databases)
        return None

    @property
    def collections(self) -> Optional[list[str]]:
        """Collections from a list_collections operation."""
        result_type = self._proto.WhichOneof("result")
        if result_type == "list_collections":
            return list(self._proto.list_collections.collections)
        return None

    @property
    def index_name(self) -> Optional[str]:
        """Index name from a create_index operation."""
        result_type = self._proto.WhichOneof("result")
        if result_type == "create_index":
            return self._proto.create_index.index_name
        return None

    @property
    def transaction_id(self) -> Optional[str]:
        """Transaction ID from a begin_transaction operation."""
        result_type = self._proto.WhichOneof("result")
        if result_type == "begin_transaction":
            return self._proto.begin_transaction.transaction_id
        return None

    @property
    def command_result(self) -> Optional[dict]:
        """Result from a run_command operation."""
        result_type = self._proto.WhichOneof("result")
        if result_type == "run_command":
            if self._proto.run_command.result and self._proto.run_command.result.data:
                return decode(self._proto.run_command.result.data)
        return None

    @property
    def search_method(self) -> Optional[str]:
        """Search method used (vector/fulltext/filter)."""
        result_type = self._proto.WhichOneof("result")
        if result_type == "search":
            return self._proto.search.method
        return None

    @property
    def search_total(self) -> Optional[int]:
        """Total search results."""
        result_type = self._proto.WhichOneof("result")
        if result_type == "search":
            return self._proto.search.total
        return None

    @property
    def analytics(self) -> Optional[dict]:
        """Analytics data from get_analytics operation."""
        result_type = self._proto.WhichOneof("result")
        if result_type == "get_analytics":
            return {
                "total_operations": self._proto.get_analytics.total_operations,
                "total_errors": self._proto.get_analytics.total_errors,
                "error_rate": self._proto.get_analytics.error_rate,
                "p50_latency_ms": self._proto.get_analytics.p50_latency_ms,
                "p95_latency_ms": self._proto.get_analytics.p95_latency_ms,
                "p99_latency_ms": self._proto.get_analytics.p99_latency_ms,
            }
        return None

    def __repr__(self) -> str:
        if not self.success:
            return f"PipelineResult(index={self.index}, error={self.error!r})"
        result_type = self._proto.WhichOneof("result")
        return f"PipelineResult(index={self.index}, type={result_type})"
