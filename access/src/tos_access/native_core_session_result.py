"""Typed results over the existing owned native GraphViews session.

This client receives native JSON mechanically. It neither constructs a graph
nor issues admission. Its ReceiverState is the original setup owner ledger;
launcher/caller objects must already be included in that same ledger.
"""
import types
import time
import math

from .native_core_session_receiver import ReceiverState

_CALL_PREFIX = '{"schema_version":"tos_native_core_session_call_v1","sequence":'
_TOOL = ',"operation":"tos_native_call","arguments":{"tool":"tos_corpus_graph_views","arguments":{}},"work_deadline_ns":'
_RESOURCE_RENDER = ',"operation":"tos_native_resource_read","arguments":{"uri":"tos-corpus://graph-views","render":true},"work_deadline_ns":'
_RESOURCE = ',"operation":"tos_native_resource_read","arguments":{"uri":"tos-corpus://graph-views","render":false},"work_deadline_ns":'


class NativeCoreSessionResultClient:
    __slots__ = ('_session', '_state', '_work_ns', '_source_revision', '_supports_render')

    def __new__(cls, session, state, startup_frame, original_work_deadline_ns):
        if not isinstance(state, ReceiverState):
            raise TypeError('native SDK result requires its original receiving ledger')
        g = state.geometry
        framework = 0
        scalar = max(g.unicode_bytes(1), g.tuple_base + 5 * g.pointer,
                     int.__basicsize__ + 3 * g.int_digit)
        for fn in (cls.__init__, cls._call, cls.corpus_graph_views,
                   cls.read_resource, cls.render_resource, cls.call, cls.close, cls._accept_envelope):
            code = fn.__code__
            slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
            framework += types.FrameType.__basicsize__ + g.gc_header + slots * (g.pointer + scalar)
        state.reserve(cls.__basicsize__ + g.gc_header + framework
                      + types.MethodType.__basicsize__ + g.gc_header)
        return object.__new__(cls)

    def __init__(self, session, state, startup_frame, original_work_deadline_ns):
        if not isinstance(state, ReceiverState):
            raise TypeError('native SDK result requires its original receiving ledger')
        if type(original_work_deadline_ns) is not int or not 0 < original_work_deadline_ns < 1 << 64:
            raise ValueError('native SDK original work clock differs')
        ready = state.decode(startup_frame)
        if (type(ready) is not dict or ready.get('schema_version') != 'tos_native_core_session_ready_v1'
                or ready.get('ok') is not True or ready.get('state_reused') is not False
                or type(ready.get('source_revision')) is not str):
            raise ValueError('native SDK authentic session startup result differs')
        caps = ready.get('capabilities')
        if (type(caps) is not list or len(caps) != 2
                or type(caps[0]) is not dict or type(caps[1]) is not dict
                or caps[0].get('operation') != 'tos_native_call'
                or caps[0].get('tool') != 'tos_corpus_graph_views'
                or caps[1].get('operation') != 'tos_native_resource_read'
                or caps[1].get('uri') != 'tos-corpus://graph-views'
                or type(caps[1].get('render')) is not list
                or len(caps[1]['render']) not in (1, 2) or caps[1]['render'][0] is not False
                or (len(caps[1]['render']) == 2 and caps[1]['render'][1] is not True)):
            raise ValueError('native SDK session capabilities differ')
        self._session, self._state, self._work_ns = session, state, original_work_deadline_ns
        self._source_revision = ready['source_revision']  # borrowed charged value
        self._supports_render = len(caps[1]['render']) == 2

    def _call(self, operation, *, absolute_deadline=None):
        state = self._state
        state.active()
        work = self._work_ns
        if absolute_deadline is not None:
            if (type(absolute_deadline) not in (int, float)
                    or (type(absolute_deadline) is float and not math.isfinite(absolute_deadline))):
                raise ValueError('native SDK original caller cutoff must be finite')
            if absolute_deadline <= 0:
                raise TimeoutError('native SDK narrower original call cutoff expired')
            # A larger finite deadline cannot extend the original u64 cutoff.
            # Compare seconds before multiplying: an arbitrary caller integer
            # or finite float must not allocate a huge PyLong or overflow just
            # to be discarded by min(). Only a positive bounded deadline is
            # converted, bounded by the original whole-second ceiling. The
            # one-second boundary includes every potentially narrowing float
            # despite seconds-rounding; min preserves the previous nano law.
            maximum_convertible_seconds = work // 1000000000 + 1
            if absolute_deadline <= maximum_convertible_seconds:
                work = min(work, int(absolute_deadline * 1000000000))
            if time.monotonic_ns() >= work:
                raise TimeoutError('native SDK narrower original call cutoff expired')
        sequence = self._session._sequence
        if type(sequence) is not int or not 0 < sequence < 1 << 64:
            raise ValueError('native SDK session sequence outside u64')
        # Both decimal clock/sequence are <=20 ASCII digits. The final text
        # and bytes coexist; the two format values and operand tuple are
        # distinct small owners. This is a fixed DTO, not a second serializer.
        g = state.geometry
        max_length = len(_CALL_PREFIX) + 20 + len(operation) + 20 + 1
        scratch = (g.unicode_bytes(max_length) + g.bytes_base + max_length
                   + 2 * g.unicode_bytes(20) + g.tuple_base + 4 * g.pointer)
        state.reserve(scratch)
        text = '%s%d%s%d}' % (_CALL_PREFIX, sequence, operation, work)
        payload = text.encode('ascii')
        if len(payload) > self._session._limits.max_call_bytes:
            raise ValueError('native SDK call exceeds original transport cap')
        reply = self._session.call_bytes(payload)
        del payload, text
        state.release(scratch)
        envelope = state.decode(reply)
        if (type(envelope) is not dict
                or envelope.get('schema_version') != 'tos_native_core_snapshot_result_v1'
                or envelope.get('ok') is not True or 'result' not in envelope):
            raise ValueError('native SDK result envelope differs')
        state.active()
        return self._accept_envelope(envelope)

    def _accept_envelope(self, envelope):
        return envelope['result']  # no copy; envelope/result stay charged

    def call(self, tool, arguments, *, absolute_deadline=None):
        if (tool != 'tos_corpus_graph_views' or type(arguments) is not dict or arguments):
            raise ValueError('native owned session tool capability unavailable')
        return self._call(_TOOL, absolute_deadline=absolute_deadline)

    def close(self):
        self._session.close()

    def corpus_graph_views(self):
        return self._call(_TOOL)

    def read_resource(self, uri):
        if uri != 'tos-corpus://graph-views':
            raise ValueError('native SDK session resource capability unavailable')
        return self._call(_RESOURCE)

    def render_resource(self, uri):
        if uri != 'tos-corpus://graph-views' or not self._supports_render:
            raise ValueError('native SDK session render capability unavailable')
        rendered = self._call(_RESOURCE_RENDER)
        if type(rendered) is not str:
            raise ValueError('native SDK native rendered result type differs')
        return rendered
