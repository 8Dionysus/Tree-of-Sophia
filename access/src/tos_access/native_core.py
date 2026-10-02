"""Explicit installed native implementation of the imported source-read methods.

The reference ToSAccessCore and default discovery remain separate. Software and
selected data are supplied to the existing native association, never inferred.
"""
from __future__ import annotations

import time
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor

from .mcp_server import NativeMCPServer
from .source_read import SourceReadError


class NativeCore:
    """Synchronous source-read slice over an authenticated native MCP child.

    Each call owns one helper thread, including when its caller already runs an
    async event loop. It joins that thread on success or failure; the existing
    native API owns process-group cleanup under the same original50s deadline.
    """

    def __init__(self, native_prefix: str | Path, arguments=()):
        self._server = NativeMCPServer(native_prefix, arguments)

    def _packet(self, tool: str, request: dict, *, absolute_deadline=None, source_errors=True):
        start = time.monotonic()
        if absolute_deadline is not None:
            import math
            if type(absolute_deadline) not in (int, float) or not math.isfinite(absolute_deadline):
                raise ValueError("Native Core deadline must be finite")
            if absolute_deadline <= start:
                raise TimeoutError("Native Core deadline expired before setup")
        deadline = start + 50 if absolute_deadline is None else min(start + 50, absolute_deadline)
        def invoke():
            import anyio

            async def call():
                return await self._server._native_api(
                    "call", (tool, request), absolute_deadline=deadline)

            return anyio.run(call)

        # No detached executor or reusable background session can outlive this
        # method. All native child work uses the deadline established above.
        with ThreadPoolExecutor(max_workers=1, thread_name_prefix="tos-native-core") as owned:
            result = owned.submit(invoke).result()
        if result.isError:
            message = " ".join(item.text for item in result.content if hasattr(item, "text"))
            if source_errors:
                if message == "exact source reader unavailable: no selected owner":
                    message = "source-owner-reader-not-configured"
                raise SourceReadError(message)
            from mcp.server.fastmcp.exceptions import ToolError
            raise ToolError(message)
        if type(result.structuredContent) is not dict:
            raise ValueError("Native source operation did not return a full object packet")
        if time.monotonic() >= deadline:
            raise TimeoutError("Native Core deadline expired before returning the packet")
        return result.structuredContent

    def knowledge_catalog(self) -> dict:
        """Read the selected Original catalog; no synthesized capability fallback."""
        return self._packet("tos_knowledge_catalog", {}, source_errors=False)

    def knowledge_exploration_contracts(self) -> dict:
        """Return software schemas with the selected native exploration capability."""
        return self._packet("tos_knowledge_exploration_contracts", {}, source_errors=False)

    def source_read_capabilities(self) -> dict:
        return self._packet("tos_source_read_capabilities", {})

    def source_read_contract(self) -> dict:
        return self._packet("tos_source_read_contract", {})

    def source_handle_discover(self, request: dict) -> dict:
        return self._packet("tos_source_handle_discover", request)

    def source_read(self, request: dict) -> dict:
        return self._packet("tos_source_read", request)

    def zarathustra_word_analysis_task(self, query: str, language: str = "ru",
                                      rank: int = 1,
                                      include_semantic_neighbors: bool = False) -> dict:
        deadline = time.monotonic() + 50
        normalized_query = str(query).strip()
        if not normalized_query:
            raise ValueError("word-analysis query is required")
        if len(normalized_query) > 256:
            raise ValueError("word-analysis query exceeds 256 characters")
        normalized_language = str(language).strip().lower()
        if normalized_language not in {"de", "ru", "en"}:
            raise ValueError(f"unsupported word-analysis language: {normalized_language}")
        try:
            bounded_rank = int(rank)
        except (TypeError, ValueError):
            bounded_rank = 1
        return self._packet("tos_zarathustra_prepare_word_analysis", {
            "query": normalized_query, "language": normalized_language,
            "rank": max(1, min(100, bounded_rank)),
            "include_semantic_neighbors": bool(include_semantic_neighbors),
        }, absolute_deadline=deadline, source_errors=False)

    def zarathustra_reading_search(self, query: str, language: str = "ru",
                                   limit: int = 20,
                                   include_semantic_neighbors: bool = False,
                                   group_by: list[str] | None = None) -> dict:
        """Return the selected source-bound reading capability and full result."""
        deadline = time.monotonic() + 50
        normalized_query = str(query).strip()
        if not normalized_query or len(normalized_query) > 256:
            raise ValueError("reading query must have 1..256 characters")
        normalized_language = str(language).strip().lower()
        if normalized_language not in {"de", "ru", "en"}:
            raise ValueError("reading language must be de, ru, or en")
        try:
            bounded_limit = int(limit)
        except (TypeError, ValueError):
            bounded_limit = 20
        groups = ["speaker", "formula"] if group_by is None else group_by
        if not isinstance(groups, list) or any(item not in {"speaker", "formula"} for item in groups):
            raise ValueError("reading group_by supports speaker and formula only")
        return self._packet("tos_zarathustra_reading_search", {
            "query": normalized_query, "language": normalized_language,
            "limit": max(0, min(100, bounded_limit)),
            "include_semantic_neighbors": bool(include_semantic_neighbors),
            "group_by": list(dict.fromkeys(groups)),
        }, absolute_deadline=deadline, source_errors=False)
