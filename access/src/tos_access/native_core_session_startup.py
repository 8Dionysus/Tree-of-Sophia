"""Emit the typed native startup DTO in its original setup workspace.

No semantic query object is decoded or reconstructed. The native owner parses
its authentic immutable query profile and supplies the stage ticket inside its
owned child. Fixed-size output is admitted before allocation.
"""
import os
import sys
import types

from .native_core_snapshot import _SOURCE_PATH_FIELDS

_CAP = 65536


class _StartupWriter:
    __slots__ = ('state', 'buffer', 'offset')
    def __new__(cls, state):
        g = state.geometry
        framework = 0
        scalar = max(g.unicode_bytes(1), g.tuple_base + 5 * g.pointer,
                     int.__basicsize__ + 3 * g.int_digit)
        for fn in (cls.__init__, cls.literal, cls.integer, cls.string, cls.path, cls.fields):
            code = fn.__code__
            slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
            framework += types.FrameType.__basicsize__ + g.gc_header + slots * (g.pointer + scalar)
        state.reserve(cls.__basicsize__ + g.gc_header + bytearray.__basicsize__ + _CAP + 1
                      + framework + g.unicode_bytes(20) + g.bytes_base + 20)
        return object.__new__(cls)

    def __init__(self, state):
        self.state = state
        self.buffer = bytearray(_CAP)
        self.offset = 0

    def literal(self, value):
        self.state.active()
        size = len(value)
        if size > _CAP - self.offset:
            raise ValueError('native session original startup cap65536 exceeded')
        self.buffer[self.offset:self.offset + size] = value
        self.offset += size

    def integer(self, value):
        if type(value) is not int or not 0 < value < 1 << 64:
            raise ValueError('native session explicit unsigned original limit refused')
        self.literal(str(value).encode('ascii'))

    def string(self, value):
        if type(value) is not str:
            raise TypeError('native startup ordinary JSON string required')
        self.literal(b'"')
        for char in value:
            self.state.visit()
            point = ord(char)
            if point in (34, 92):
                self.literal(b'\\')
                self.literal(char.encode('ascii'))
            elif point < 32 or 0xd800 <= point <= 0xdfff:
                self.literal(('\\u%04x' % point).encode('ascii'))
            else:
                self.literal(char.encode('utf-8'))
        self.literal(b'"')

    def path(self, path):
        g = self.state.geometry
        # Selection already consulted .parts during its maintained __post_init__.
        # Read its real parsed cache; .parts itself creates tuples on3.12+ and
        # must not be called before their original-state preflight.
        parts = getattr(path, '_tail_cached', None)
        if parts is None:
            parts = getattr(path, '_parts', None)  # maintained CPython3.11 cache
        if type(parts) is not list:
            raise ValueError('native selected Path parsed-cache ABI unavailable')
        characters = len(parts) + 2  # Linux absolute root/separator positions
        for part in parts:
            self.state.visit()
            characters += len(part)
        cache = getattr(path, '_str', None)
        forecast = (2 * g.unicode_bytes(characters) + g.list_bytes(len(parts))
                    + g.tuple_base + len(parts) * g.pointer)
        self.state.reserve(forecast)
        value = os.fspath(path)
        if len(value) > characters:
            raise MemoryError('native Path owner exceeded formatting geometry')
        self.string(value)
        retained = sys.getsizeof(value) if cache is None else 0
        self.state.release(forecast - retained)

    def fields(self, owner, names):
        for index, name in enumerate(names):
            if index:
                self.literal(b',')
            self.string(name)
            self.literal(b':')
            self.integer(getattr(owner, name))


def startup_bytes(admission, selection, config, state, *, selected_probe=False, selected_lazy=False):
    """Consume typed selectors/limits into a bounded encoded startup.

    The calling factory has already counted admission/selection/config objects
    in this same receiving ledger; this adds only new writer/output ownership.
    """
    if type(selected_probe) is not bool or type(selected_lazy) is not bool or (selected_probe and selected_lazy):
        raise TypeError('native startup selected profile required')
    config.active()
    if selection.query_store_configured and not (selected_probe or selected_lazy):
        raise ValueError('native first SourceRoot session cannot substitute selected QueryStore')
    if (admission.tmpfs_quota_bytes != 536870912 or admission.inode_limit != 65536
            or admission.working_ram_bytes != 2684354560
            or admission.whole_max_state_bytes > admission.working_ram_bytes
            or admission.process.address_space_bytes != admission.working_ram_bytes
            or admission.process.file_size_bytes != admission.tmpfs_quota_bytes):
        raise ValueError('native startup original issued physical/resource limits differ')
    admission.transport.validate()
    raw_profile = admission.query_profile.wire_json
    if type(raw_profile) is not bytes or not raw_profile:
        raise TypeError('native startup authentic immutable query profile required')
    w = _StartupWriter(state)
    w.literal(b'{"schema_version":"tos_native_core_lazy_session_startup_v1","admission":{'
              if selected_lazy else
              b'{"schema_version":"tos_native_core_probe_session_startup_v1","admission":{'
              if selected_probe else
              b'{"schema_version":"tos_native_core_session_startup_v1","admission":{')
    w.fields(admission, ('max_build_seconds', 'tmpfs_quota_bytes', 'inode_limit',
        'working_ram_bytes', 'whole_max_rows', 'whole_max_row_bytes', 'whole_max_graph_bytes',
        'whole_max_catalog_bytes', 'whole_max_catalog_inputs_bytes', 'whole_max_state_bytes'))
    w.literal(b',"json":{')
    w.fields(admission.json, ('max_bytes', 'max_depth', 'max_visits', 'max_integer_digits'))
    w.literal(b'},"cold":{')
    w.fields(admission.cold, ('max_file_bytes', 'max_vm_steps', 'sqlite_cache_kib', 'max_rows',
        'max_work_bytes', 'max_row_bytes', 'max_metadata_bytes', 'max_sources'))
    w.literal(b'},"process":{')
    w.fields(admission.process, ('address_space_bytes', 'file_size_bytes'))
    w.literal(b'},"operation_seconds":50.0,"work_deadline_ns":')
    w.integer(config.original_work_deadline_ns)
    w.literal(b'},"source_paths":{')
    for index, name in enumerate(_SOURCE_PATH_FIELDS):
        if index:
            w.literal(b',')
        w.string(name)
        w.literal(b':')
        w.path(getattr(selection, name))
    w.literal(b'},"query_store":{"path":')
    w.path(selection.query_store_path)
    w.literal(b',"configured":true},"query_store_limits":'
              if selected_lazy and selection.query_store_configured else
              b',"configured":false},"query_store_limits":')
    if admission.query_store_limits is None:
        w.literal(b'null')
    else:
        w.literal(b'{')
        w.fields(admission.query_store_limits, ('max_database_bytes', 'max_input_bytes',
            'max_json_bytes', 'max_rows', 'max_work_steps', 'max_sql_vm_steps', 'sqlite_cache_kib'))
        w.literal(b'}')
    w.literal(b',"http":')
    w.literal(raw_profile)
    w.literal(b',"session":{')
    w.fields(admission.transport, ('max_call_bytes', 'max_reply_bytes', 'max_chunks_per_frame',
        'max_calls', 'max_total_request_bytes', 'max_total_reply_bytes'))
    w.literal(b'},"original_whole_deadline_ns":')
    w.integer(config.original_whole_deadline_ns)
    w.literal(b'}')
    if w.offset > admission.json.max_bytes:
        raise ValueError('native startup exceeds original JSON byte allowance')
    g = state.geometry
    state.reserve(g.bytes_base + w.offset + 2 * (memoryview.__basicsize__ + g.gc_header))
    result = bytes(memoryview(w.buffer)[:w.offset])
    state.active()
    # Conservatively retain writer/buffer/framework reservation until session
    # cleanup. It is not recycled into responses while constructor frames live.
    return result


def ordinary_startup_bytes(selection, transport, config, state, *, search_read_model=None, snapshot_root=None, expected_snapshot_guard=None, expected_reference_release_guard=None):
    """Serialize selectors for the native-issued ordinary operation profile.

    Native owns Stage/capture/Cold/Query admission. Python supplies no operation
    allowance and retains the existing transport and original bootstrap clock.
    """
    config.active()
    transport.validate()
    if state._deadline != config.original_work_deadline_ns / 1e9:
        raise ValueError('ordinary startup must retain original bootstrap clock')
    w = _StartupWriter(state)
    w.literal(b'{"schema_version":"tos_native_core_ordinary_session_startup_v1","source_paths":{')
    for index, name in enumerate(_SOURCE_PATH_FIELDS):
        if index:
            w.literal(b',')
        w.string(name)
        w.literal(b':')
        w.path(getattr(selection, name))
    w.literal(b'},"query_store":{"path":')
    w.path(selection.query_store_path)
    w.literal(b',"configured":true},"session":{'
              if selection.query_store_configured else
              b',"configured":false},"session":{')
    w.fields(transport, ('max_call_bytes', 'max_reply_bytes', 'max_chunks_per_frame',
        'max_calls', 'max_total_request_bytes', 'max_total_reply_bytes'))
    if search_read_model is not None:
        if (type(search_read_model) is not dict or set(search_read_model) !=
                {'path', 'max_bytes', 'max_postings', 'max_verify_chars'}):
            raise ValueError('ordinary search sidecar selector shape differs')
        path = search_read_model['path']
        if type(path) is not str or not path.startswith('/') or '..' in path.split('/'):
            raise ValueError('ordinary search sidecar requires resolved absolute path')
        w.literal(b'},"search_read_model":{"path":')
        w.string(path)
        for name in ('max_bytes', 'max_postings', 'max_verify_chars'):
            w.literal(b',')
            w.string(name)
            w.literal(b':')
            value = search_read_model[name]
            if type(value) is not int or not 0 <= value < 1 << 64:
                raise ValueError('ordinary search sidecar selector requires u64')
            w.literal(str(value).encode('ascii'))
    w.literal(b'}')
    if snapshot_root is not None:
        w.literal(b',"snapshot_root":')
        w.path(snapshot_root)
    if expected_snapshot_guard is not None:
        if (snapshot_root is None or type(expected_snapshot_guard) is not str
                or len(expected_snapshot_guard) != 64
                or any(c not in '0123456789abcdef' for c in expected_snapshot_guard)):
            raise ValueError('native snapshot guard receipt differs')
        w.literal(b',"expected_snapshot_guard":')
        w.string(expected_snapshot_guard)
    if expected_reference_release_guard is not None:
        if (snapshot_root is None or type(expected_reference_release_guard) is not str
                or len(expected_reference_release_guard) != 64
                or any(c not in '0123456789abcdef' for c in expected_reference_release_guard)):
            raise ValueError('native Reference release guard receipt differs')
        w.literal(b',"expected_reference_release_guard":')
        w.string(expected_reference_release_guard)
    w.literal(b',"original_whole_deadline_ns":')
    w.integer(config.original_whole_deadline_ns)
    w.literal(b'}')
    g = state.geometry
    state.reserve(g.bytes_base + w.offset + 2 * (memoryview.__basicsize__ + g.gc_header))
    result = bytes(memoryview(w.buffer)[:w.offset])
    state.active()
    return result
