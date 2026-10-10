"""Typed explicit native session startup selection, without a stage descriptor.

The native issuer supplies and authenticates the real ticket inside its child.
This DTO selects original limits; it does not allocate, decode results or issue
an OS resource grant. Encoded startup receiving state remains its caller's.
"""
from dataclasses import dataclass

from .native_core_snapshot import (
    NativeCoreSnapshotSelection, NativeCoreJsonLimits, NativeCoreColdLimits,
    NativeCoreProcessLimits, NativeCoreQueryProfile, NativeCoreQueryStoreLimits,
    _integer,
)
from .native_core_session import NativeSDKStageConfiguration
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

    def startup_wire(self, selection, config):
        if not isinstance(selection, NativeCoreSnapshotSelection):
            raise TypeError('native session requires its typed seven-source selection')
        if not isinstance(config, NativeSDKStageConfiguration):
            raise TypeError('native session requires original bootstrap configuration')
        config.active()
        for value, owner in ((self.json, NativeCoreJsonLimits),
                             (self.cold, NativeCoreColdLimits),
                             (self.process, NativeCoreProcessLimits),
                             (self.query_profile, NativeCoreQueryProfile),
                             (self.transport, NativeSessionLimits)):
            if not isinstance(value, owner):
                raise TypeError('native session requires original typed owner limits')
        if selection.query_store_configured:
            raise ValueError('first held SourceRoot session cannot substitute a selected QueryStore')
        limits = {name: _integer(getattr(self, name), 'session.' + name, positive=True)
                  for name in ('max_build_seconds', 'tmpfs_quota_bytes', 'inode_limit',
                    'working_ram_bytes', 'whole_max_rows', 'whole_max_row_bytes',
                    'whole_max_graph_bytes', 'whole_max_catalog_bytes',
                    'whole_max_catalog_inputs_bytes', 'whole_max_state_bytes')}
        if (self.tmpfs_quota_bytes != 536870912 or self.inode_limit != 65536
                or self.working_ram_bytes != 2684354560
                or self.whole_max_state_bytes > self.working_ram_bytes):
            raise ValueError('native session limits exceed or differ from original issued profile')
        process = self.process.wire()
        if (process['address_space_bytes'] != self.working_ram_bytes
                or process['file_size_bytes'] != self.tmpfs_quota_bytes):
            raise ValueError('native session process limits differ from original hard limits')
        query_profile = self.query_profile.wire()
        json_limits = self.json.wire()
        transport = self.transport.wire()
        if self.transport.max_call_bytes > min(json_limits['max_bytes'], 16 * 1024 * 1024):
            raise ValueError('native session request exceeds original JSON allowance')
        # The native HttpAdmission validates its own response/frame/work/state
        # fields. No Python domain/query parser or independent allowance exists.
        if self.query_store_limits is not None and not isinstance(self.query_store_limits, NativeCoreQueryStoreLimits):
            raise TypeError('native session requires typed selected-store limits')
        limits.update(json=json_limits, cold=self.cold.wire(), process=process,
                      operation_seconds=50.0,
                      work_deadline_ns=config.original_work_deadline_ns)
        return {'schema_version': 'tos_native_core_session_startup_v1',
                'admission': limits,
                'source_paths': selection.source_paths_wire(),
                'query_store': selection.query_store_wire(),
                'query_store_limits': (self.query_store_limits.wire()
                                       if self.query_store_limits is not None else None),
                'http': query_profile, 'session': transport,
                'original_whole_deadline_ns': config.original_whole_deadline_ns}
