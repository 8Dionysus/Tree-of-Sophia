"""Compatibility API for starting the installed native MCP owner."""
from __future__ import annotations

from pathlib import Path
from typing import Any

from .native_mcp import NativeMCPServer





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
