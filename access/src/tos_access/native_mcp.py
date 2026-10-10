"""Platform lifecycle and MCP SDK adapter for an installed native child."""
from __future__ import annotations

import json
import os
import sys
from pathlib import Path
from types import SimpleNamespace
from typing import Any

DEFAULT_HTTP_PORT = 5429

def _native_wire_bytes(value):
    # JSON permits lone UTF-16 surrogates. Preserve ordinary UTF-8 byte costs,
    # escaping only those code units rather than changing normal Unicode text.
    return json.dumps(value, ensure_ascii=False, allow_nan=False,
                      separators=(",", ":")).encode("utf-8", "backslashreplace")


class NativeMCPServer:
    """Imported serving caller for one explicit installed native MCP process.

    Serving replaces the process through the maintained held-image association.
    Async tool calls use one bounded child with that same explicit association;
    reference Core/default discovery remain separate.
    """

    def __init__(self, prefix: str | Path, arguments=(), *, inherit_data_selection: bool = True):
        self.prefix = Path(prefix)
        if not self.prefix.is_absolute() or '..' in self.prefix.parts:
            raise ValueError('native MCP requires an explicit absolute software prefix')
        if (not isinstance(arguments, (list, tuple))
                or any(type(value) is not str or '\0' in value for value in arguments)
                or sum(len(value.encode('utf-8')) for value in arguments) > 1048576):
            raise ValueError('native MCP options exceed the bounded argv contract')
        if type(inherit_data_selection) is not bool:
            raise TypeError('inherit_data_selection must be a boolean')
        self.inherit_data_selection = inherit_data_selection
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
        import math
        import time
        from datetime import timedelta
        import anyio
        # The synchronous bridge supplies a system-monotonic deadline. Public
        # AnyIO callers retain their backend clock (including Trio clock epochs).
        now = time.monotonic if absolute_deadline is not None else anyio.current_time
        wall_start = time.monotonic()
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
        wall_deadline = (child_deadline if absolute_deadline is not None else
                         wall_start + min(50, child_deadline - start))
        from .native_io import owned_exchange, _bounded_json, MAX_NATIVE_MCP_FRAME_BYTES
        from mcp import ClientSession, types
        from mcp.shared.message import SessionMessage

        request_cap = 65_536
        # Accept every declared native frame, including managed and prepared
        # catalogs. Rust selects the packet budget from actual arguments/env;
        # the SDK does not duplicate that routing or grant data access.
        frame_cap = MAX_NATIVE_MCP_FRAME_BYTES
        if arguments is not None:
            _bounded_json(arguments, request_cap - 256, wall_deadline - 5)
        from threading import Event
        from functools import partial

        # The shared stdlib owner never delegates its child to AnyIO's watcher.
        # One wall-clock deadline also bounds callers whose backend clock uses
        # another epoch; explicit synchronous deadlines pass through unchanged.
        cancelled = Event()
        channel_ready = anyio.Event()
        owner_done = anyio.Event()
        channel_box = []
        sender, incoming = anyio.create_memory_object_stream(0)
        outgoing, receiver = anyio.create_memory_object_stream(0)

        def read_owned_frames():
            try:
                with owned_exchange([*self.arguments, "mcp"], prefix=self.prefix,
                        input_cap=4 * request_cap, frame_cap=frame_cap,
                        cancelled=cancelled, absolute_deadline=wall_deadline,
                        env=None if self.inherit_data_selection else {
                            key: value for key, value in os.environ.items()
                            if key not in {'TOS_RELEASE_ROOT', 'TOS_DATA_ROOT'}
                        }) as channel:
                    channel_box.append(channel)
                    anyio.from_thread.run_sync(channel_ready.set)
                    for line in channel.frames():
                        message = types.JSONRPCMessage.model_validate_json(line)
                        anyio.from_thread.run(sender.send, SessionMessage(message))
            except InterruptedError:
                if not cancelled.is_set():
                    raise
            finally:
                anyio.from_thread.run_sync(channel_ready.set)
                anyio.from_thread.run(sender.aclose)

        async def read_frames():
            # Cancellation sets the Event in the SDK/session owner below. This
            # shield joins the sole child owner, including its bounded cleanup.
            with anyio.CancelScope(shield=True):
                try:
                    await anyio.to_thread.run_sync(read_owned_frames, abandon_on_cancel=False)
                finally:
                    owner_done.set()

        async def write_frames():
            await channel_ready.wait()
            if not channel_box:
                return
            async with receiver:
                async for message in receiver:
                    value = message.message.model_dump(mode="json", by_alias=True,
                                                       exclude_none=True)
                    _bounded_json(value, request_cap - 1, wall_deadline - 5, cancelled)
                    await anyio.to_thread.run_sync(partial(channel_box[0].send, value),
                                                   abandon_on_cancel=False)

        try:
            with anyio.fail_after(max(0, operation_deadline - now())):
                async with anyio.create_task_group() as tasks:
                    tasks.start_soon(read_frames)
                    tasks.start_soon(write_frames)
                    try:
                        # Software admission30 and the unchanged native5
                        # remain intersected with the original caller clock.
                        async with ClientSession(incoming, outgoing,
                                read_timeout_seconds=timedelta(seconds=35)) as session:
                            completed = False
                            try:
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
                                completed = True
                            finally:
                                if completed:
                                    # A successful RPC still requires native
                                    # EOF and an accepted terminal status. The
                                    # same owner observes it without reaping.
                                    await anyio.to_thread.run_sync(channel_box[0].close_input,
                                                                  abandon_on_cancel=False)
                                    await owner_done.wait()
                                else:
                                    cancelled.set()
                                    await incoming.aclose()
                    finally:
                        cancelled.set()
                        # Closing the SDK sink releases a worker waiting to
                        # deliver its last frame before the bounded owner join.
                        await incoming.aclose()
                        tasks.cancel_scope.cancel()
        finally:
            cancelled.set()
            incoming.close()
            outgoing.close()
            sender.close()
            receiver.close()
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
