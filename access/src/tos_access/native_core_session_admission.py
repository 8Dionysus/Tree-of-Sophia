"""Typed explicit native session startup selection, without a stage descriptor.

The native issuer supplies and authenticates the real ticket inside its child.
This DTO selects original limits; it does not allocate, decode results or issue
an OS resource grant. Encoded startup receiving state remains its caller's.
"""
from dataclasses import dataclass

from .native_core_snapshot import (
    NativeCoreJsonLimits, NativeCoreColdLimits,
    NativeCoreProcessLimits, NativeCoreQueryProfile, NativeCoreQueryStoreLimits,
)
from .native_core_session_control import NativeSessionLimits


@dataclass(frozen=True)
class NativeCoreSessionAdmission:
    max_build_seconds: int
    tmpfs_quota_bytes: int
    inode_limit: int
    working_ram_bytes: int
    whole_max_rows: int
    whole_max_row_bytes: int
    whole_max_graph_bytes: int
    whole_max_catalog_bytes: int
    whole_max_catalog_inputs_bytes: int
    whole_max_state_bytes: int
    json: NativeCoreJsonLimits
    cold: NativeCoreColdLimits
    process: NativeCoreProcessLimits
    query_profile: NativeCoreQueryProfile
    transport: NativeSessionLimits
    query_store_limits: NativeCoreQueryStoreLimits | None = None
