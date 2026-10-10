"""Compatibility API for starting the installed native MCP owner."""
from __future__ import annotations

import os
from pathlib import Path
from typing import Any

from .native_mcp import NativeMCPServer, _native_wire_bytes, DEFAULT_HTTP_PORT


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


def build_server(
    tos_root: str | Path | None = None,
    index_path: str | Path | None = None,
    philosophy_graph_projection_path: str | Path | None = None,
    philosophy_post_planting_audit_path: str | Path | None = None,
    *,
    core: Any | None = None,
    native_prefix: str | Path | None = None,
    native_arguments: list[str] | tuple[str, ...] = (),
) -> NativeMCPServer:
    """Start only native software; Python Core and graph discovery are retired."""
    if native_prefix is None:
        raise ValueError("MCP startup requires an explicit installed native prefix")
    if core is not None or any(value is not None for value in (
        tos_root, index_path, philosophy_graph_projection_path, philosophy_post_planting_audit_path
    )):
        raise ValueError("Python Core and implicit source discovery are retired")
    return NativeMCPServer(native_prefix, native_arguments)


__all__ = ["NativeMCPServer", "build_server"]
