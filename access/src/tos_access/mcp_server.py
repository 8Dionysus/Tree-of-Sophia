from __future__ import annotations

import json
import logging
import os
import sys
from pathlib import Path
from threading import Lock
from types import SimpleNamespace
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from .core import ToSAccessCore


LOGGER = logging.getLogger(__name__)
DEFAULT_HTTP_PORT = 5429


def _native_wire_bytes(value):
    # JSON permits lone UTF-16 surrogates. Preserve ordinary UTF-8 byte costs,
    # escaping only those code units rather than changing normal Unicode text.
    return json.dumps(value, ensure_ascii=False, allow_nan=False,
                      separators=(",", ":")).encode("utf-8", "backslashreplace")


def _transport_options() -> tuple[str, str | None, int | None]:
    transport = os.environ.get("TOS_MCP_TRANSPORT") or os.environ.get("AOA_MCP_TRANSPORT", "stdio").strip() or "stdio"
    if transport == "stdio":
        return transport, None, None
    if transport != "streamable-http":
        raise SystemExit(f"unsupported AOA_MCP_TRANSPORT: {transport}")
    host = os.environ.get("TOS_MCP_HOST") or os.environ.get("AOA_MCP_HOST", "127.0.0.1").strip()
    if host not in {"127.0.0.1", "localhost", "::1"}:
        raise SystemExit("AOA_MCP_HOST must remain loopback-only")
    port = int(os.environ.get("TOS_MCP_PORT") or os.environ.get("AOA_MCP_PORT", DEFAULT_HTTP_PORT))
    if not 0 <= port <= 65535:
        raise SystemExit("AOA_MCP_PORT must be a valid local TCP port")
    return transport, host, port


def _run_server(server: Any) -> None:
    transport, host, port = _transport_options()
    if transport == "streamable-http":
        server.settings.host = host
        server.settings.port = port
    server.run(transport=transport)


class NativeMCPServer:
    """Imported serving caller for one explicit installed native MCP process.

    Serving replaces the process through the maintained held-image association.
    Async tool calls use one bounded child with that same explicit association;
    reference Core/default discovery remain separate.
    """

    def __init__(self, prefix: str | Path, arguments=()):
        self.prefix = Path(prefix)
        if not self.prefix.is_absolute() or '..' in self.prefix.parts:
            raise ValueError('native MCP requires an explicit absolute software prefix')
        if (not isinstance(arguments, (list, tuple))
                or any(type(value) is not str or '\0' in value for value in arguments)
                or sum(len(value.encode('utf-8')) for value in arguments) > 1048576):
            raise ValueError('native MCP options exceed the bounded argv contract')
        self.arguments = tuple(arguments)
        self.settings = SimpleNamespace(host="127.0.0.1", port=DEFAULT_HTTP_PORT)

    async def list_tools(self):
        """List only tools declared by this explicitly selected native session."""
        return (await self._native_api("list", None)).tools

    async def list_resources(self):
        """Keep the maintained imported resource metadata result shape."""
        return (await self._native_api("resources", None)).resources

    async def list_resource_templates(self):
        return (await self._native_api("resource_templates", None)).resourceTemplates

    async def read_resource(self, uri):
        from mcp.server.fastmcp.exceptions import ResourceError
        from mcp.server.lowlevel.helper_types import ReadResourceContents
        from mcp.types import TextResourceContents
        try:
            result = await self._native_api("read_resource", str(uri))
            if any(not isinstance(item, TextResourceContents) for item in result.contents):
                raise ResourceError("Native ToS resource returned an unsupported representation")
            return [ReadResourceContents(content=item.text, mime_type=item.mimeType,
                                         meta=item.meta) for item in result.contents]
        except ResourceError:
            raise
        except Exception as error:
            raise ResourceError(str(error)) from error

    async def list_prompts(self):
        return (await self._native_api("prompts", None)).prompts

    async def get_prompt(self, name, arguments=None):
        # FastMCP imported calls validate the Python rank type; MCP wire prompt
        # arguments are strings. Preserve that caller coercion mechanically.
        from pydantic import TypeAdapter
        try:
            if arguments is not None:
                if type(arguments) is not dict:
                    raise ValueError("Prompt arguments must be an object")
                arguments = dict(arguments)
                if name == "tos-zarathustra-word-analysis" and "rank" in arguments:
                    arguments["rank"] = str(TypeAdapter(int).validate_python(arguments["rank"]))
            return await self._native_api("get_prompt", (name, arguments))
        except Exception as error:
            raise ValueError(str(error)) from error

    async def call_tool(self, name: str, arguments: dict[str, Any]):
        """Keep the maintained imported MCP result shapes for native tools."""
        from mcp.server.fastmcp.exceptions import ToolError
        if type(name) is not str or not name or type(arguments) is not dict:
            raise ToolError("Native tool call requires a name and object arguments")
        try:
            result = await self._native_api("call", (name, arguments))
            if result.isError:
                reason = " ".join(item.text for item in result.content if hasattr(item, "text"))
                raise ToolError(reason or "Native tool refused the request")
            if name == "tos_knowledge_search" and arguments.get("mode") == "compressed":
                return result
            return result.content, result.structuredContent
        except ToolError:
            raise
        except Exception as error:
            raise ToolError(f"Error executing tool {name}: {error}") from error

    async def _native_api(self, operation, arguments, *, absolute_deadline=None):
        # Existing SDK owns MCP lifecycle/types; this adapter owns the exact
        # child and bounded JSONL I/O so cancellation cannot abandon its PID.
        import signal
        import math
        import time
        from datetime import timedelta
        import anyio
        # The synchronous bridge supplies a system-monotonic deadline. Public
        # AnyIO callers retain their backend clock (including Trio clock epochs).
        now = time.monotonic if absolute_deadline is not None else anyio.current_time
        start = now()
        if absolute_deadline is not None:
            if (type(absolute_deadline) not in (int, float)
                    or not math.isfinite(absolute_deadline)):
                raise ValueError("Native caller deadline must be finite")
            if absolute_deadline <= start:
                raise TimeoutError("Native caller deadline expired before setup")
        child_deadline = min(start + 50, absolute_deadline) if absolute_deadline is not None else start + 50
        operation_deadline = min(start + 45, child_deadline - 5)
        if operation_deadline <= start:
            raise TimeoutError("Native caller has no remaining operation budget")
        from mcp import ClientSession, types
        from mcp.shared.message import SessionMessage

        request_cap = 65_536
        # Fixed native main profile: 1MiB packet, worst-case escaped duplicate
        # carrier and request-sized RPC id, plus bounded framing punctuation.
        # The explicit prepared profile declares 4MiB packets; other fixed
        # native profiles declare 1MiB. This is transport framing capacity,
        # never a data admission or query-budget grant.
        prepared = any(arg == "--prepared-read-model" or
                       arg.startswith("--prepared-read-model=") for arg in self.arguments)
        packet_cap = 4 * 1_048_576 if prepared else 1_048_576
        frame_cap = 7 * packet_cap + 6 * request_cap + 1024
        if arguments is not None:
            encoded = _native_wire_bytes(arguments)
            if len(encoded) + 256 > request_cap:
                raise ValueError("Native MCP request exceeds the frame byte budget")
        from . import native_dispatch
        dispatch = Path(native_dispatch.__file__).resolve(strict=True)
        program = (
            "import importlib.util,sys,json;from pathlib import Path;"
            "s=importlib.util.spec_from_file_location('native_selected_dispatch',sys.argv[1]);"
            "m=importlib.util.module_from_spec(s);s.loader.exec_module(m);"
            "m.run(Path(sys.argv[2]),json.loads(sys.argv[3]))"
        )
        child = None
        sender, incoming = anyio.create_memory_object_stream(0)
        outgoing, receiver = anyio.create_memory_object_stream(0)
        try:
            with anyio.fail_after(max(0, operation_deadline - now())):
                child = await anyio.open_process(
                    [sys.executable, "-B", "-c", program, str(dispatch), str(self.prefix),
                     json.dumps([*self.arguments, "mcp"])],
                    stderr=-3, start_new_session=True,
                )
                async def read_frames():
                    pending = bytearray()
                    async with sender:
                        while True:
                            try:
                                chunk = await child.stdout.receive(65_536)
                            except anyio.EndOfStream:
                                if pending:
                                    raise ValueError("Native MCP ended inside a frame")
                                return
                            pending.extend(chunk)
                            while b"\n" in pending:
                                line, _, rest = pending.partition(b"\n")
                                if len(line) + 1 > frame_cap:
                                    raise ValueError("Native MCP response exceeds frame budget")
                                pending = bytearray(rest)
                                message = types.JSONRPCMessage.model_validate_json(line)
                                await sender.send(SessionMessage(message))
                            if len(pending) >= frame_cap:
                                raise ValueError("Native MCP response exceeds frame budget")
                async def write_frames():
                    async with receiver:
                        async for message in receiver:
                            raw = _native_wire_bytes(message.message.model_dump(
                                mode="json", by_alias=True, exclude_none=True))
                            if len(raw) + 1 > request_cap:
                                raise ValueError("Native MCP request exceeds frame budget")
                            await child.stdin.send(raw + b"\n")
                async with anyio.create_task_group() as tasks:
                    tasks.start_soon(read_frames)
                    tasks.start_soon(write_frames)
                    try:
                        # Initial client wait includes one authentic software
                        # admission30 plus the unchanged native request5.
                        async with ClientSession(incoming, outgoing,
                                read_timeout_seconds=timedelta(seconds=35)) as session:
                            await session.initialize()
                            with anyio.fail_after(min(5, operation_deadline - now())):
                                if operation == "list":
                                    result = await session.list_tools()
                                elif operation == "call":
                                    result = await session.call_tool(arguments[0], arguments[1],
                                        read_timeout_seconds=timedelta(seconds=5))
                                elif operation == "resources":
                                    result = await session.list_resources()
                                elif operation == "resource_templates":
                                    result = await session.list_resource_templates()
                                elif operation == "read_resource":
                                    from pydantic import AnyUrl
                                    result = await session.read_resource(AnyUrl(arguments))
                                elif operation == "prompts":
                                    result = await session.list_prompts()
                                elif operation == "get_prompt":
                                    result = await session.get_prompt(arguments[0], arguments[1])
                                else:
                                    raise ValueError("Unknown native imported API operation")
                    finally:
                        tasks.cancel_scope.cancel()
        finally:
            # No preliminary close/wait error may skip owned group termination.
            # All grace, kill, reap and stream closure share the original50s.
            cleanup_error = None
            with anyio.CancelScope(shield=True):
                try:
                    if child is not None:
                        try:
                            with anyio.move_on_after(min(2, max(0, child_deadline - now()))):
                                if child.stdin is not None:
                                    await child.stdin.aclose()
                                await child.wait()
                        except Exception as error:
                            cleanup_error = error
                        finally:
                            # Even an exited leader can leave owned descendants
                            # holding pipes. This group was created for this call.
                            try:
                                os.killpg(child.pid, signal.SIGKILL)
                            except ProcessLookupError:
                                pass
                            except OSError as error:
                                cleanup_error = error
                            try:
                                remaining = max(0, child_deadline - now())
                                with anyio.fail_after(remaining):
                                    await child.wait()
                                    await child.aclose()
                            except Exception as error:
                                cleanup_error = error
                finally:
                    # Memory-stream closure is synchronous and cannot spend a
                    # fresh timeout or obstruct the unconditional kill/reap.
                    incoming.close()
                    outgoing.close()
                    sender.close()
                    receiver.close()
            if cleanup_error is not None:
                raise RuntimeError("Native MCP cleanup refused within absolute child deadline") from cleanup_error
        if child.returncode != 0:
            raise ValueError(f"Native MCP child exited {child.returncode}")
        if now() >= child_deadline:
            raise TimeoutError("Native MCP deadline expired before returning the result")
        return result

    def run(self, *, transport='stdio'):
        options = []
        if transport == 'streamable-http':
            if self.settings.host not in {'127.0.0.1', 'localhost', '::1'}:
                raise ValueError('native MCP HTTP must remain loopback-only')
            if type(self.settings.port) is not int or not 0 <= self.settings.port <= 65535:
                raise ValueError('native MCP HTTP requires a valid local TCP port')
            options = ['--transport', 'streamable-http', '--host', self.settings.host,
                       '--port', str(self.settings.port)]
        elif transport != 'stdio':
            raise ValueError('unsupported native MCP transport')
        from .native_dispatch import run
        return run(self.prefix, [*self.arguments, 'mcp', *options])


def build_server(
    tos_root: str | Path | None = None,
    index_path: str | Path | None = None,
    philosophy_graph_projection_path: str | Path | None = None,
    philosophy_post_planting_audit_path: str | Path | None = None,
    *,
    core: ToSAccessCore | None = None,
    native_prefix: str | Path | None = None,
    native_arguments: list[str] | tuple[str, ...] = (),
) -> Any:
    if native_prefix is not None:
        if core is not None or any(value is not None for value in (
                tos_root, index_path, philosophy_graph_projection_path, philosophy_post_planting_audit_path)):
            raise ValueError('select native software/options or a reference core/discovery paths')
        return NativeMCPServer(native_prefix, native_arguments)
    if native_arguments:
        raise ValueError('native MCP options require an explicit native prefix')
    from .core import ToSAccessCore
    if core is not None and any(value is not None for value in (
            tos_root, index_path, philosophy_graph_projection_path, philosophy_post_planting_audit_path)):
        raise ValueError("select either an existing core or MCP source discovery paths")
    try:
        from mcp.server.fastmcp import FastMCP  # type: ignore[import-not-found]
    except ImportError as exc:
        raise SystemExit("Missing dependency 'mcp'. Install with: python -m pip install -e .") from exc

    mcp = FastMCP("tree-of-sophia", json_response=True)
    state_lock = Lock()
    cached_state: ToSAccessCore | None = None

    def current_state() -> ToSAccessCore:
        nonlocal cached_state
        if core is not None:
            return core
        resolved = ToSAccessCore.discover(
            tos_root=tos_root,
            index_path=index_path,
            philosophy_graph_projection_path=philosophy_graph_projection_path,
            philosophy_post_planting_audit_path=philosophy_post_planting_audit_path,
        )
        # Discover path changes on every call, but keep the shared graph/index
        # for unchanged paths. Core readers still observe current file versions.
        # Equality excludes disposable indexes/checkpoints and compares paths.
        with state_lock:
            if cached_state is None or cached_state != resolved:
                cached_state = resolved
            return cached_state

    # Default discovery retains its existing server-local dynamic graph route.
    # An explicitly supplied core owns its query engine and checkpoint policy.
    from .exploration import ExplorationService
    exploration = ExplorationService(lambda: current_state().knowledge_graph(),
        query_store_provider=lambda: current_state()._query_store()) if core is None else None

    @mcp.tool()
    def tos_knowledge_explore(request: dict[str, Any]) -> dict[str, Any]:
        """Start a read-only neighborhood or continue with cursor only; expires after 15 minutes.

        Discover schemas: legacy focus_node_id or v2 exact node/relation origin
        pinned by source/content revision. Fixed query/page sizes; no authored
        writes. Upsert context nodes and the repeated origin relation by ID.
        Snapshot conflict or expired checkpoint requires restarting from focus.
        """
        return current_state().knowledge_explore(request) if core is not None else exploration.explore(request)

    @mcp.tool()
    def tos_knowledge_exploration_contracts() -> dict[str, Any]:
        """Read exploration capabilities and request/result schemas; local/native only."""
        return current_state().knowledge_exploration_contracts()

    @mcp.tool()
    def tos_corpus_status() -> dict[str, Any]:
        """Return ToS corpus index path, counts, graph views, and authority boundary."""
        return current_state().status()

    @mcp.tool()
    def tos_corpus_summary() -> dict[str, Any]:
        """Return a compact whole-corpus summary from the ToS-owned index."""
        return current_state().summary()

    @mcp.tool()
    def tos_corpus_search(query: str, limit: int = 20, resource_kind: str | None = None) -> dict[str, Any]:
        """Search nodes, resources, manifests, branches, and graph views in the ToS corpus index."""
        return current_state().search(query=query, limit=limit, resource_kind=resource_kind)

    @mcp.tool()
    def tos_knowledge_catalog() -> dict[str, Any]:
        """Return node kinds, predicates, fields, limits, and stored LensSpec definitions for the generic backend."""
        return current_state().knowledge_catalog()

    @mcp.tool()
    def tos_knowledge_contracts() -> dict[str, Any]:
        """Return the versioned API operation map and JSON Schemas used by human and agent constructors."""
        return current_state().knowledge_contracts()

    @mcp.tool()
    def tos_knowledge_search_capabilities() -> dict[str, Any]:
        """Discover selected search modes; compressed requires an explicit local prepared publication."""
        return current_state().knowledge_search_capabilities()

    @mcp.tool()
    def tos_knowledge_search(
        query: str = "",
        sources: list[str] | None = None,
        kind_ids: list[str] | None = None,
        predicate_ids: list[str] | None = None,
        offset: int = 0,
        limit: int = 40,
        mode: str = "legacy",
        cursor: str | None = None,
    ) -> dict[str, Any]:
        """Search full node/relation carriers. Discover modes with tos_knowledge_search_capabilities.

        Compressed mode supports short queries and authenticated continuation
        on the explicitly selected local prepared profile. No cold build or
        fallback; resume possibly empty pages until has_more is false.
        """
        if mode in {"indexed", "compressed"}:
            if offset:
                raise ValueError(f"{mode} knowledge search uses cursor continuation, not offset")
            state = current_state()
            search = state.knowledge_search_indexed if mode == "indexed" else state.knowledge_search_compressed
            packet = search(
                query,
                sources=sources,
                kind_ids=kind_ids,
                predicate_ids=predicate_ids,
                cursor=cursor,
                limit=limit,
            )
            if mode == "compressed":
                # FastMCP otherwise pretty-prints every nested full carrier.
                # Keep the text carrier in the same bounded compact framing;
                # MCP's protocol envelope still carries both representations.
                from mcp.types import CallToolResult, TextContent
                text = json.dumps(packet, ensure_ascii=False, separators=(",", ":"), allow_nan=False)
                return CallToolResult(content=[TextContent(type="text", text=text)], structuredContent=packet)
            return packet
        if mode != "legacy":
            raise ValueError("knowledge search mode must be legacy, indexed or compressed")
        return current_state().knowledge_search(
            query,
            sources=sources,
            kind_ids=kind_ids,
            predicate_ids=predicate_ids,
            offset=offset,
            limit=limit,
        )

    @mcp.tool()
    def tos_knowledge_node(node_id: str, relation_limit: int = 200) -> dict[str, Any]:
        """Inspect a normalized knowledge node and its human-readable related relations."""
        return current_state().knowledge_node(node_id, relation_limit)

    @mcp.tool()
    def tos_knowledge_relation(relation_id: str) -> dict[str, Any]:
        """Inspect a normalized relation together with its display-complete endpoints."""
        return current_state().knowledge_relation(relation_id)

    @mcp.tool()
    def tos_knowledge_temporal_compare(request: dict[str, Any]) -> dict[str, Any]:
        """Compare two exact Claim date envelopes, not event truth. Discover the request schema with tos_knowledge_contracts."""
        return current_state().knowledge_temporal_compare(request)

    @mcp.tool()
    def tos_knowledge_focus(
        node_id: str,
        sources: list[str] | None = None,
        depth: int = 1,
        direction: str = "either",
        predicate_ids: list[str] | None = None,
        node_limit: int = 200,
        relation_limit: int = 400,
        profile: str = "overview",
    ) -> dict[str, Any]:
        """Center a bounded radial lens on an exact or uniquely resolved node identity."""
        return current_state().knowledge_focus(
            node_id,
            sources=sources,
            depth=depth,
            direction=direction,
            predicate_ids=predicate_ids,
            node_limit=node_limit,
            relation_limit=relation_limit,
            profile=profile,
        )

    @mcp.tool()
    def tos_knowledge_lens_compile(spec: dict[str, Any]) -> dict[str, Any]:
        """Validate and execute an arbitrary bounded, read-only tos_lens_spec_v1 construction."""
        return current_state().compile_knowledge_lens(spec)

    @mcp.tool()
    def tos_knowledge_lens_open(lens_id: str) -> dict[str, Any]:
        """Execute a stored LensSpec through the same generic backend used for arbitrary constructions."""
        return current_state().stored_knowledge_lens(lens_id)

    @mcp.tool()
    def tos_source_descend(node_id: str, max_depth: int = 8, limit: int = 300) -> dict[str, Any]:
        """Walk from an era, region, tradition, planting, or source object down the source-navigation graph."""
        return current_state().source_descend(node_id=node_id, max_depth=max_depth, limit=limit)

    @mcp.tool()
    def tos_dossier_inspect(object_id: str, limit: int = 300) -> dict[str, Any]:
        """Return a compact dossier for one bibliographic carrier or Link without converting availability into a rights conclusion."""
        return current_state().source_dossier(object_id=object_id, limit=limit)

    @mcp.tool()
    def tos_source_read_capabilities() -> dict[str, Any]:
        """Report whether an explicit owner-bound exact-source reader is selected."""
        return current_state().source_read_capabilities()

    @mcp.tool()
    def tos_source_read_contract() -> dict[str, Any]:
        """Return the bounded owner-issued source handle/read contract."""
        return current_state().source_read_contract()

    @mcp.tool()
    def tos_source_handle_discover(
        target: dict[str, Any] | None = None,
        selector: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        """Issue one exact owner handle from a card target or typed catalog selector.

        A selector is resolved only by the selected owner catalog; it cannot
        name a path, range, digest, or ``latest`` fallback.
        """
        if (target is None) == (selector is None):
            raise ValueError("provide exactly one owner target or typed selector")
        return current_state().source_handle_discover(
            {"target": target} if target is not None else {"selector": selector}
        )

    @mcp.tool()
    def tos_source_read(handle: dict[str, Any], representation: str = "record") -> dict[str, Any]:
        """Read an exact record, or native_public_unit/native_local_unit via the selected owner.

        Native text requires its own current public rights/closure checks;
        the metadata handle is not permission. Caller paths/ranges are refused.
        Local reading also requires explicit current owner-selected conditions.
        Retain all local_conditions notices with its text; no publication grant.
        """
        return current_state().source_read({"handle": handle, "representation": representation})

    @mcp.tool()
    def tos_corpus_resources(
        resource_kind: str | None = None,
        owner_branch: str | None = None,
        limit: int = 100,
    ) -> dict[str, Any]:
        """List indexed ToS resources with optional kind and owner-branch filters."""
        return current_state().resources(resource_kind=resource_kind, owner_branch=owner_branch, limit=limit)

    @mcp.tool()
    def tos_corpus_node(node_id: str) -> dict[str, Any]:
        """Return one indexed ToS node and relation edges connected to it."""
        return current_state().node(node_id=node_id)

    @mcp.tool()
    def tos_corpus_relation_pack(pack_id: str) -> dict[str, Any]:
        """Return one indexed ToS relation pack and its edges."""
        return current_state().relation_pack(pack_id=pack_id)

    @mcp.tool()
    def tos_corpus_graph_view(view_id: str, limit: int = 100) -> dict[str, Any]:
        """Return a named graph review view over the whole ToS corpus index."""
        return current_state().graph_view(view_id=view_id, limit=limit)

    @mcp.tool()
    def tos_corpus_packet(query: str = "", view_id: str | None = None, limit: int = 20) -> dict[str, Any]:
        """Return a compact task packet with optional search and graph-view context."""
        return current_state().packet(query=query, view_id=view_id, limit=limit)

    @mcp.tool()
    def tos_zarathustra_prepare_word_analysis(
        query: str,
        language: str = "ru",
        rank: int = 1,
        include_semantic_neighbors: bool = False,
    ) -> dict[str, Any]:
        """Return one exact-source local analysis task; never accept or persist its interpretation."""
        return current_state().zarathustra_word_analysis_task(
            query=query,
            language=language,
            rank=rank,
            include_semantic_neighbors=include_semantic_neighbors,
        )

    @mcp.tool()
    def tos_zarathustra_reading_search(
        query: str,
        language: str = "ru",
        limit: int = 20,
        include_semantic_neighbors: bool = False,
        group_by: list[str] | None = None,
    ) -> dict[str, Any]:
        """Return occurrence-bound source candidates with candidate-only authority."""
        return current_state().zarathustra_reading_search(
            query=query,
            language=language,
            limit=limit,
            include_semantic_neighbors=include_semantic_neighbors,
            group_by=group_by,
        )

    @mcp.tool()
    def tos_philosophy_graph_status() -> dict[str, Any]:
        """Return ToS philosophy graph projection path, counts, graph views, and authority boundary."""
        return current_state().philosophy_status()

    @mcp.tool()
    def tos_philosophy_graph_views() -> dict[str, Any]:
        """List ToS philosophy graph views materialized by the ToS-owned projection export."""
        return current_state().philosophy_views()

    @mcp.tool()
    def tos_philosophy_graph_layers() -> dict[str, Any]:
        """Return ToS-owned philosophy graph layers and layer counts for runtime filtering."""
        return current_state().philosophy_layers()

    @mcp.tool()
    def tos_philosophy_graph_contracts() -> dict[str, Any]:
        """Return the bounded MCP access contract for ToS philosophy graph packets."""
        return current_state().philosophy_contracts()

    @mcp.tool()
    def tos_philosophy_graph_scale_manifest(view_id: str | None = None, layers: list[str] | None = None) -> dict[str, Any]:
        """Return compact row counts and packet routes for ToS philosophy scale projection access."""
        return current_state().philosophy_scale_manifest(view_id=view_id, layers=layers)

    @mcp.tool()
    def tos_philosophy_graph_scale_rows(
        table: str,
        view_id: str | None = None,
        layers: list[str] | None = None,
        offset: int = 0,
        limit: int = 1000,
    ) -> dict[str, Any]:
        """Return a paginated normalized scale table, including cluster membership rows."""
        return current_state().philosophy_scale_packet(
            table=table,
            view_id=view_id,
            layers=layers,
            offset=offset,
            limit=limit,
        )

    @mcp.tool()
    def tos_philosophy_graph_view(view_id: str, limit: int = 1000) -> dict[str, Any]:
        """Return one ToS philosophy graph view packet with projected nodes, edges, and source refs."""
        return current_state().philosophy_view(view_id=view_id, limit=limit)

    @mcp.tool()
    def tos_philosophy_graph_clusters(
        view_id: str | None = None,
        cluster_kind: str | None = None,
        limit: int = 80,
    ) -> dict[str, Any]:
        """Return compact ToS philosophy graph clusters, optionally filtered by view and cluster kind."""
        return current_state().philosophy_clusters(view_id=view_id, cluster_kind=cluster_kind, limit=limit)

    @mcp.tool()
    def tos_philosophy_graph_node(node_id: str) -> dict[str, Any]:
        """Return one projected ToS philosophy node and related projected edges."""
        return current_state().philosophy_node(node_id=node_id)

    @mcp.tool()
    def tos_philosophy_graph_edge(edge_id: str) -> dict[str, Any]:
        """Return one projected ToS philosophy edge and its endpoint nodes."""
        return current_state().philosophy_edge(edge_id=edge_id)

    @mcp.tool()
    def tos_philosophy_epistemic_packet(
        item_id: str,
        view_id: str | None = None,
        limit: int = 80,
    ) -> dict[str, Any]:
        """Return projected source routes, challenge signals, and authority posture for one selected item."""
        return current_state().philosophy_epistemic_packet(item_id=item_id, view_id=view_id, limit=limit)

    @mcp.tool()
    def tos_evidence_lens(
        mode: str,
        item_id: str,
        view_id: str | None = None,
        limit: int = 80,
    ) -> dict[str, Any]:
        """Return a public-safe Evidence Lens packet joining a selection to explicit owner routes and gaps."""
        return current_state().evidence_lens_packet(
            mode=mode,
            item_id=item_id,
            view_id=view_id,
            limit=limit,
        )

    @mcp.tool()
    def tos_philosophy_graph_neighborhood(
        node_id: str,
        depth: int = 1,
        layers: list[str] | None = None,
        predicates: list[str] | None = None,
        limit: int = 80,
    ) -> dict[str, Any]:
        """Return the projected neighborhood around one ToS philosophy node."""
        return current_state().philosophy_neighborhood(
            node_id=node_id,
            depth=depth,
            layers=layers,
            predicates=predicates,
            limit=limit,
        )

    @mcp.tool()
    def tos_philosophy_graph_path(
        from_id: str,
        to_id: str,
        layers: list[str] | None = None,
        predicates: list[str] | None = None,
        max_depth: int = 6,
        direction: str = "outgoing",
        view_id: str | None = None,
        excluded_edge_ids: list[str] | None = None,
        alternative_limit: int = 1,
    ) -> dict[str, Any]:
        """Return deterministic bounded paths with direction, view, and edge-exclusion constraints."""
        return current_state().philosophy_path_between(
            from_id=from_id,
            to_id=to_id,
            layers=layers,
            predicates=predicates,
            max_depth=max_depth,
            direction=direction,
            view_id=view_id,
            excluded_edge_ids=excluded_edge_ids,
            alternative_limit=alternative_limit,
        )

    @mcp.tool()
    def tos_philosophy_graph_review_packet(view_id: str = "chronology") -> dict[str, Any]:
        """Return one compact ToS-owned review packet for a philosophy graph lens."""
        return current_state().philosophy_review_packet(view_id=view_id)

    @mcp.tool()
    def tos_philosophy_graph_snapshot() -> dict[str, Any]:
        """Return ToS-owned philosophy graph snapshot fingerprints for diff-aware review."""
        return current_state().philosophy_snapshot()

    @mcp.tool()
    def tos_philosophy_graph_audit() -> dict[str, Any]:
        """Return the ToS-owned post-planting audit packet when present."""
        return current_state().philosophy_audit()

    @mcp.tool()
    def tos_philosophy_graph_unresolved(view_id: str | None = None) -> dict[str, Any]:
        """Return unresolved review surfaces for all philosophy graph lenses or one selected lens."""
        return current_state().philosophy_unresolved(view_id=view_id)

    @mcp.tool()
    def tos_philosophy_graph_packet(query: str = "", view_id: str | None = None, limit: int = 20) -> dict[str, Any]:
        """Return a compact philosophy graph packet for agents with optional search and view context."""
        return current_state().philosophy_packet(query=query, view_id=view_id, limit=limit)

    @mcp.tool()
    def tos_philosophy_graph_chronology_packet(limit: int = 20) -> dict[str, Any]:
        """Return the chronology lens packet for formation, fixation, canonization, and dating review."""
        return current_state().philosophy_lens_packet(view_id="chronology", limit=limit)

    @mcp.tool()
    def tos_philosophy_graph_source_evidence_packet(limit: int = 20) -> dict[str, Any]:
        """Return the source-evidence lens packet for source refs, confidence, and witness review."""
        return current_state().philosophy_lens_packet(view_id="source-evidence", limit=limit)

    @mcp.tool()
    def tos_philosophy_graph_concept_lineage_packet(limit: int = 20) -> dict[str, Any]:
        """Return the concept-lineage lens packet for concept/problem pressure and lineage review."""
        return current_state().philosophy_lens_packet(view_id="concept-lineage", limit=limit)

    @mcp.resource("tos-corpus://status")
    def status_resource() -> str:
        return json.dumps(current_state().status(), ensure_ascii=False, indent=2)

    @mcp.resource("tos-corpus://summary")
    def summary_resource() -> str:
        return json.dumps(current_state().summary(), ensure_ascii=False, indent=2)

    @mcp.resource("tos-corpus://graph-views")
    def graph_views_resource() -> str:
        return current_state().render_resource("tos-corpus://graph-views")

    @mcp.resource("tos-corpus://graph-view/{view_id}")
    def graph_view_resource(view_id: str) -> str:
        return current_state().render_resource(f"tos-corpus://graph-view/{view_id}")

    @mcp.resource("tos-philosophy://status")
    def philosophy_status_resource() -> str:
        return current_state().render_resource("tos-philosophy://status")

    @mcp.resource("tos-philosophy://views")
    def philosophy_views_resource() -> str:
        return current_state().render_resource("tos-philosophy://views")

    @mcp.resource("tos-philosophy://layers")
    def philosophy_layers_resource() -> str:
        return current_state().render_resource("tos-philosophy://layers")

    @mcp.resource("tos-philosophy://contracts")
    def philosophy_contracts_resource() -> str:
        return current_state().render_resource("tos-philosophy://contracts")

    @mcp.resource("tos-philosophy://scale-manifest")
    def philosophy_scale_manifest_resource() -> str:
        return current_state().render_resource("tos-philosophy://scale-manifest")

    @mcp.resource("tos-philosophy://snapshot")
    def philosophy_snapshot_resource() -> str:
        return current_state().render_resource("tos-philosophy://snapshot")

    @mcp.resource("tos-philosophy://audit")
    def philosophy_audit_resource() -> str:
        return current_state().render_resource("tos-philosophy://audit")

    @mcp.resource("tos-philosophy://clusters")
    def philosophy_clusters_resource() -> str:
        return current_state().render_resource("tos-philosophy://clusters")

    @mcp.resource("tos-philosophy://unresolved")
    def philosophy_unresolved_resource() -> str:
        return current_state().render_resource("tos-philosophy://unresolved")

    @mcp.resource("tos-philosophy://view/{view_id}")
    def philosophy_view_resource(view_id: str) -> str:
        return current_state().render_resource(f"tos-philosophy://view/{view_id}")

    @mcp.resource("tos-philosophy://review-packet/{view_id}")
    def philosophy_review_packet_resource(view_id: str) -> str:
        return current_state().render_resource(f"tos-philosophy://review-packet/{view_id}")

    @mcp.resource("tos-philosophy://edge/{edge_id}")
    def philosophy_edge_resource(edge_id: str) -> str:
        return current_state().render_resource(f"tos-philosophy://edge/{edge_id}")

    @mcp.resource("tos-philosophy://lens/{view_id}")
    def philosophy_lens_resource(view_id: str) -> str:
        return current_state().render_resource(f"tos-philosophy://lens/{view_id}")

    @mcp.prompt(name="tos-corpus-review")
    def tos_corpus_review(view_id: str = "corpus-topology", query: str = "") -> str:
        """Prompt route for reviewing ToS corpus graph context."""
        return (
            f"Use tos_corpus_status(), then tos_corpus_packet(query={query!r}, view_id={view_id!r}). "
            "Treat Tree-of-Sophia source_refs returned by the packet as authority; treat native MCP and standalone runtime as read-only access surfaces."
        )

    @mcp.prompt(name="tos-philosophy-graph-review")
    def tos_philosophy_graph_review(view_id: str = "chronology", query: str = "") -> str:
        """Prompt route for reviewing ToS philosophy graph projection context."""
        return (
            f"Use tos_philosophy_graph_status(), tos_philosophy_graph_layers(), "
            f"tos_philosophy_graph_review_packet(view_id={view_id!r}), then "
            f"tos_philosophy_graph_packet(query={query!r}, view_id={view_id!r}). "
            "Treat ToS source_ref values as meaning authority; treat native MCP, UI, and optional integrations as projection/access surfaces only."
        )

    @mcp.prompt(name="tos-zarathustra-word-analysis")
    def tos_zarathustra_word_analysis(query: str, language: str = "ru", rank: int = 1) -> str:
        """Prompt route for source-first morphology, semantics, etymology, and English rendering."""
        return (
            "Call tos_zarathustra_prepare_word_analysis"
            f"(query={query!r}, language={language!r}, rank={rank}). "
            "Analyze every required stage in the returned task. Use point citations for etymology, "
            "keep German as source authority, Russian as a historical comparator, and English as an "
            "unreviewed candidate. Do not infer contextual meaning from etymology alone."
        )

    LOGGER.info("ToS corpus MCP server ready")
    return mcp


def main(arguments=None) -> None:
    # Imported main() historically uses configured reference discovery, not
    # the hosting application's argv. The executable module passes argv below.
    args = list(() if arguments is None else arguments)
    if args:
        # Reuse the maintained explicit-prefix parser and image custody. This
        # fixed server entry does not introduce another operation registry.
        if not (args[0] == '--native-prefix' or args[0].startswith('--native-prefix=')):
            raise SystemExit('native MCP module options require --native-prefix')
        transport, host, port = _transport_options()
        options = (['--transport', transport, '--host', host, '--port', str(port)]
                   if transport == 'streamable-http' else [])
        from .__main__ import main as module_main
        return module_main([*args, 'mcp', *options])
    logging.basicConfig(level=logging.INFO)
    _run_server(build_server())


if __name__ == '__main__':
    main(sys.argv[1:])
