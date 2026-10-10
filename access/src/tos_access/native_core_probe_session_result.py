"""Exact metadata-probe results under the owned native selected-path profile.

No model or source revision is produced. The initial native method law matches
Reference CPython3.14 Path.is_file; older Reference OS-error parity stays open.
"""
import types
from .native_core_session_receiver import ReceiverState
from .native_core_session_result import NativeCoreSessionResultClient

_OPERATIONS = ('tos_corpus_index_exists', 'tos_philosophy_projection_exists',
               'tos_evidence_projection_exists', 'tos_philosophy_audit_exists')
_FRAGMENTS = (
    ',"operation":"tos_corpus_index_exists","arguments":{},"work_deadline_ns":',
    ',"operation":"tos_philosophy_projection_exists","arguments":{},"work_deadline_ns":',
    ',"operation":"tos_evidence_projection_exists","arguments":{},"work_deadline_ns":',
    ',"operation":"tos_philosophy_audit_exists","arguments":{},"work_deadline_ns":',
)

class NativeCoreProbeSessionResultClient:
    __slots__ = ('_session', '_state', '_work_ns', '_source_revision')
    # Existing mechanical DTO/envelope/cutoff owner. This is not GraphViews
    # dispatch: each caller supplies an immutable admitted probe fragment.
    _call = NativeCoreSessionResultClient._call
    close = NativeCoreSessionResultClient.close
    _accept_envelope = NativeCoreSessionResultClient._accept_envelope

    def __new__(cls, session, state, startup_frame, original_work_deadline_ns):
        if not isinstance(state, ReceiverState):
            raise TypeError('native probe requires original receiving ledger')
        g = state.geometry
        framework = 0
        scalar = max(g.unicode_bytes(1), g.tuple_base + 5 * g.pointer,
                     int.__basicsize__ + 3 * g.int_digit)
        for fn in (cls.__init__, cls._call, cls._observe, cls.call, cls.close,
                   cls.index_exists, cls.philosophy_projection_exists,
                   cls.evidence_projection_exists, cls.philosophy_audit_exists, cls._accept_envelope):
            code = fn.__code__
            slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
            framework += types.FrameType.__basicsize__ + g.gc_header + slots * (g.pointer + scalar)
        state.reserve(cls.__basicsize__ + g.gc_header + framework
                      + types.MethodType.__basicsize__ + g.gc_header)
        return object.__new__(cls)

    def __init__(self, session, state, startup_frame, original_work_deadline_ns):
        if not isinstance(state, ReceiverState):
            raise TypeError('native probe requires original receiving ledger')
        if type(original_work_deadline_ns) is not int or not 0 < original_work_deadline_ns < 1 << 64:
            raise ValueError('native probe original work cutoff differs')
        ready = state.decode(startup_frame)
        if (type(ready) is not dict
                or ready.get('schema_version') != 'tos_native_core_probe_session_ready_v1'
                or ready.get('ok') is not True
                or ready.get('profile') != 'tos_core_selected_probes_v1'
                or ready.get('reference_semantics') != 'cpython_pathlib_is_file_3_14'
                or 'source_revision' not in ready or ready['source_revision'] is not None
                or ready.get('state_reused') is not False):
            raise ValueError('native probe startup profile differs')
        caps = ready.get('capabilities')
        if type(caps) is not list or len(caps) != 4:
            raise ValueError('native probe capabilities differ')
        for index in range(4):
            state.visit()
            cap = caps[index]
            if type(cap) is not dict or len(cap) != 1 or cap.get('operation') != _OPERATIONS[index]:
                raise ValueError('native probe operation capability differs')
        self._session, self._state, self._work_ns = session, state, original_work_deadline_ns
        self._source_revision = None

    def _observe(self, fragment, *, absolute_deadline=None):
        value = self._call(fragment, absolute_deadline=absolute_deadline)
        if type(value) is not bool:
            raise ValueError('native selected probe result is not boolean')
        return value

    def call(self, operation, arguments, *, absolute_deadline=None):
        if type(operation) is not str or type(arguments) is not dict or arguments:
            raise ValueError('native selected probe requires exact empty arguments')
        for index in range(4):
            self._state.visit()
            if operation == _OPERATIONS[index]:
                return self._observe(_FRAGMENTS[index], absolute_deadline=absolute_deadline)
        raise ValueError('native selected probe operation unavailable')

    def index_exists(self):
        return self._observe(_FRAGMENTS[0])

    def philosophy_projection_exists(self):
        return self._observe(_FRAGMENTS[1])

    def evidence_projection_exists(self):
        return self._observe(_FRAGMENTS[2])

    def philosophy_audit_exists(self):
        return self._observe(_FRAGMENTS[3])
