"""Typed lower results and native-issued association over one owned Driver.

The seven-field receipt is a channel association, never a grant or model. The
native owner chooses the configured/default Store before CorpusHeader capture.
No Python source payload, SQLite query, header projection or fallback is used.
"""
import types
import math
import time
from .native_core_search_request import search_fragment, _string, validate_search_arguments
from .native_core_session_census import retained_owner_state
from .native_core_session_receiver import ReceiverState
from .native_core_session_result import NativeCoreSessionResultClient

_OPERATIONS = (
    'tos_corpus_index_exists', 'tos_philosophy_projection_exists',
    'tos_evidence_projection_exists', 'tos_philosophy_audit_exists',
    'tos_corpus_index', 'tos_bibliographic_graph', 'tos_philosophy_projection',
    'tos_philosophy_audit_payload', 'tos_corpus_header',
)
_ORDINARY_DIRECT = ('tos_source_navigation', 'tos_knowledge_header', 'tos_evidence_projection')
_FRAGMENTS = (
    ',"operation":"tos_corpus_index_exists","arguments":{},"work_deadline_ns":',
    ',"operation":"tos_philosophy_projection_exists","arguments":{},"work_deadline_ns":',
    ',"operation":"tos_evidence_projection_exists","arguments":{},"work_deadline_ns":',
    ',"operation":"tos_philosophy_audit_exists","arguments":{},"work_deadline_ns":',
    ',"operation":"tos_corpus_index","arguments":{},"work_deadline_ns":',
    ',"operation":"tos_bibliographic_graph","arguments":{},"work_deadline_ns":',
    ',"operation":"tos_philosophy_projection","arguments":{},"work_deadline_ns":',
    ',"operation":"tos_philosophy_audit_payload","arguments":{},"work_deadline_ns":',
    ',"operation":"tos_corpus_header","arguments":{},"work_deadline_ns":',
)
_SELECTION_KEYS = ('schema_version', 'generation', 'profile', 'source_revision',
                   'data_revision', 'exploration_revision', 'state_reused')
_PROFILES = ('selected_paths', 'carrier_corpus', 'carrier_bibliographic',
             'carrier_philosophy', 'carrier_philosophy_audit', 'weak_query_store', 'whole_root')


class NativeCoreLazySessionResultClient:
    __slots__ = ('_session', '_state', '_work_ns', '_source_revision', '_selection', '_failed', '_supports_search', '_supports_indexed_search', '_supports_whole_legacy_search')
    _call = NativeCoreSessionResultClient._call
    close = NativeCoreSessionResultClient.close

    def __new__(cls, session, state, startup_frame, original_work_deadline_ns):
        if not isinstance(state, ReceiverState):
            raise TypeError('native lazy results require original receiving ledger')
        g = state.geometry
        framework = 0
        scalar = max(g.unicode_bytes(20), g.tuple_base + 9 * g.pointer,
                     int.__basicsize__ + 3 * g.int_digit)
        # The constant name tuple is code-owned. Its iterator has PyObject,
        # Py_ssize_t index and one tuple pointer in all admitted CPython ABIs.
        state.reserve(object.__basicsize__ + 2 * g.pointer + g.gc_header)
        for name in ('__init__', '_validate_selection', '_accept_envelope',
                     '_execute', '_call', 'call', 'close', 'index_exists',
                     'philosophy_projection_exists', 'evidence_projection_exists',
                     'philosophy_audit_exists', 'corpus_index', 'bibliographic_graph',
                     'philosophy_projection', 'philosophy_audit_payload', 'corpus_header',
                     '_search', '_indexed_search', '_indexed_search_available',
                     '_validate_indexed_result', 'knowledge_search'):
            code = getattr(cls, name).__code__
            slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
            framework += types.FrameType.__basicsize__ + g.gc_header + slots * (g.pointer + scalar)
        state.reserve(g.tuple_base + 3 * g.pointer)
        for function in (search_fragment, _string, validate_search_arguments):
            code = function.__code__
            slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
            framework += types.FrameType.__basicsize__ + g.gc_header + slots * (g.pointer + scalar)
        # The nested mechanical method chain has five simultaneous bound-method
        # wrappers; decoded ready/envelopes and receipts stay charged by decode.
        state.reserve(cls.__basicsize__ + g.gc_header + framework
                      + 5 * (types.MethodType.__basicsize__ + g.gc_header))
        return object.__new__(cls)

    def __init__(self, session, state, startup_frame, original_work_deadline_ns):
        if type(original_work_deadline_ns) is not int or not 0 < original_work_deadline_ns < 1 << 64:
            raise ValueError('native lazy original work cutoff differs')
        self._state = state
        ready = state.decode(startup_frame)
        if (type(ready) is not dict
                or ready.get('schema_version') != 'tos_native_core_lazy_session_ready_v1'
                or ready.get('ok') is not True
                or ready.get('profile') != 'tos_core_lazy_selected_v1'
                or ready.get('reference_semantics') != 'cpython_pathlib_is_file_3_14'
                or 'source_revision' not in ready or ready['source_revision'] is not None
                or 'data_revision' not in ready or ready['data_revision'] is not None
                or ready.get('state_reused') is not False):
            raise ValueError('native lazy startup profile differs')
        caps = ready.get('capabilities')
        if type(caps) is not list or len(caps) not in (9, 10, 11, 12):
            raise ValueError('native lazy operation capabilities differ')
        for index in range(9):
            state.visit()
            cap = caps[index]
            if type(cap) is not dict or len(cap) != 1 or cap.get('operation') != _OPERATIONS[index]:
                raise ValueError('native lazy operation capability differs')
        self._supports_search = len(caps) >= 10
        if self._supports_search:
            cap = caps[9]
            if (type(cap) is not dict or len(cap) != 3
                    or cap.get('operation') != 'tos_native_call'
                    or cap.get('tool') != 'tos_knowledge_search'
                    or type(cap.get('profiles')) is not list or len(cap['profiles']) != 1
                    or cap['profiles'][0] != 'weak_query_store'):
                raise ValueError('native Search selected Store capability differs')
        self._supports_indexed_search = len(caps) >= 11
        if self._supports_indexed_search:
            cap = caps[10]
            if (type(cap) is not dict or len(cap) != 3
                    or cap.get('operation') != 'tos_native_call'
                    or cap.get('tool') != 'tos_knowledge_search_indexed_v2'
                    or type(cap.get('profiles')) is not list or len(cap['profiles']) != 1
                    or cap['profiles'][0] != 'whole_root'):
                raise ValueError('native indexed Whole capability differs')
        self._supports_whole_legacy_search = len(caps) == 12
        if self._supports_whole_legacy_search:
            cap = caps[11]
            if (type(cap) is not dict or len(cap) != 3
                    or cap.get('operation') != 'tos_native_call'
                    or cap.get('tool') != 'tos_knowledge_search'
                    or type(cap.get('profiles')) is not list or len(cap['profiles']) != 1
                    or cap['profiles'][0] != 'whole_root'):
                raise ValueError('native legacy Whole Search capability differs')
        selection = ready.get('selection')
        self._validate_selection(selection)
        if selection['generation'] != 0 or selection['profile'] != 'selected_paths':
            raise ValueError('native lazy initial selection differs')
        self._session, self._work_ns = session, original_work_deadline_ns
        self._selection, self._source_revision, self._failed = selection, None, False

    def _validate_selection(self, selection):
        if type(selection) is not dict or len(selection) != 7:
            raise ValueError('native lazy selected-owner receipt differs')
        for key in _SELECTION_KEYS:
            self._state.visit()
            if key not in selection:
                raise ValueError('native lazy selected-owner field absent')
        if (selection['schema_version'] != 'tos_native_core_selected_profile_v1'
                or type(selection['generation']) is not int
                or not 0 <= selection['generation'] < 1 << 64
                or type(selection['profile']) is not str or selection['profile'] not in _PROFILES
                or selection['data_revision'] is not None
                or selection['state_reused'] is not False):
            raise ValueError('native lazy selected-owner association differs')
        source_revision = selection['source_revision']
        if selection['profile'] == 'whole_root':
            if not self._supports_indexed_search and not getattr(self, '_ordinary_whole', False):
                raise ValueError('native Whole source capability unavailable')
            if type(source_revision) is not str or len(source_revision) != 64:
                raise ValueError('native Whole source cut differs')
            index = 0
            while index < 64:
                self._state.visit()
                if source_revision[index] not in '0123456789abcdef':
                    raise ValueError('native Whole source cut is not canonical')
                index += 1
        elif source_revision is not None:
            raise ValueError('native partial carrier invented source cut')
        revision = selection['exploration_revision']
        if selection['profile'] == 'weak_query_store':
            if type(revision) is not str:
                raise ValueError('native weak Store exploration binding differs')
        elif revision is not None:
            raise ValueError('native partial carrier invented revision')

    def _accept_envelope(self, envelope):
        selection = envelope.get('selection')
        self._validate_selection(selection)
        previous = self._selection
        generation = selection['generation']
        if generation == previous['generation']:
            for key in _SELECTION_KEYS:
                self._state.visit()
                if selection[key] != previous[key]:
                    raise ValueError('native same-generation association changed')
        elif (generation != previous['generation'] + 1
              or (selection['profile'] == 'selected_paths' and previous['profile'] != 'whole_root')):
            raise ValueError('native selected-owner generation is not successive')
        self._selection = selection  # Borrow the already-charged native receipt.
        self._source_revision = selection['source_revision']
        return envelope['result']

    def _execute(self, index, *, absolute_deadline=None):
        if self._failed:
            raise ValueError('native lazy receiving owner is terminal')
        try:
            result = self._call(_FRAGMENTS[index], absolute_deadline=absolute_deadline)
            if index < 4:
                if type(result) is not bool:
                    raise ValueError('native selected probe result is not boolean')
            else:
                if type(result) is not dict:
                    raise ValueError('native selected carrier result is not an object')
                profile = self._selection['profile']
                expected = _PROFILES[index - 3] if index < 8 else 'carrier_corpus'
                if profile != expected and not (index == 8 and profile == 'weak_query_store'):
                    raise ValueError('native lower result owner association differs')
            return result
        except BaseException:
            self._failed = True  # No new request after transport/DTO/receipt refusal.
            raise

    def call(self, operation, arguments, *, absolute_deadline=None):
        if type(operation) is not str:
            raise ValueError('native lazy operation requires exact string')
        if operation == 'tos_knowledge_search_indexed_v2' and self._indexed_search_available():
            return self._indexed_search(arguments, absolute_deadline=absolute_deadline)
        if operation == 'tos_knowledge_search' and self._supports_search:
            return self._search(arguments, absolute_deadline=absolute_deadline)
        if type(operation) is not str or type(arguments) is not dict or arguments:
            raise ValueError('native lazy operation requires exact empty arguments')
        for index in range(9):
            self._state.visit()
            if operation == _OPERATIONS[index]:
                return self._execute(index, absolute_deadline=absolute_deadline)
        raise ValueError('native lazy operation capability unavailable')

    def index_exists(self):
        return self._execute(0)

    def philosophy_projection_exists(self):
        return self._execute(1)

    def evidence_projection_exists(self):
        return self._execute(2)

    def philosophy_audit_exists(self):
        return self._execute(3)

    def corpus_index(self):
        return self._execute(4)

    def bibliographic_graph(self):
        return self._execute(5)

    def philosophy_projection(self):
        return self._execute(6)

    def philosophy_audit_payload(self):
        return self._execute(7)

    def corpus_header(self):
        return self._execute(8)

    def _search(self, arguments, *, absolute_deadline=None):
        if self._failed or not self._supports_search:
            raise ValueError('native owned Store Search capability unavailable')
        try:
            state = self._state
            validate_search_arguments(state, arguments)
            # This exact identity registry never refunds external caller state.
            retained_owner_state(state, (arguments,), maximum_objects=state._json.max_visits)
            fragment, amount = search_fragment(state, arguments, self._session._limits.max_call_bytes)
            previous_generation = self._selection['generation']
            result = self._call(fragment, absolute_deadline=absolute_deadline)
            if type(result) is not dict:
                raise ValueError('native Search result owner differs')
            if self._selection['profile'] == 'whole_root':
                if (not self._supports_whole_legacy_search
                        or self._selection['generation'] != previous_generation + 1
                        or result.get('schema') != 'tos_knowledge_search_v1'
                        or result.get('source_revision') != self._source_revision):
                    raise ValueError('native legacy Whole Search result owner differs')
            elif self._selection['profile'] != 'weak_query_store':
                raise ValueError('native Search result owner differs')
            del fragment
            state.release(amount)
            return result
        except BaseException:
            self._failed = True
            raise

    def _indexed_search_available(self):
        return self._supports_indexed_search

    def _validate_indexed_result(self, result, previous_generation):
        if (type(result) is not dict or self._selection['profile'] != 'whole_root'
                or self._selection['generation'] != previous_generation + 1
                or result.get('schema') != 'tos_knowledge_search_indexed_v2'
                or result.get('source_revision') != self._source_revision):
            raise ValueError('native Search result owner differs')

    def _indexed_search(self, arguments, *, absolute_deadline=None):
        if self._failed or not self._indexed_search_available():
            raise ValueError('native indexed Search capability unavailable')
        try:
            state = self._state
            validate_search_arguments(state, arguments, indexed=True)
            # This exact identity registry never refunds external caller state.
            retained_owner_state(state, (arguments,), maximum_objects=state._json.max_visits)
            fragment, amount = search_fragment(state, arguments, self._session._limits.max_call_bytes, indexed=True)
            previous_generation = self._selection['generation']
            result = self._call(fragment, absolute_deadline=absolute_deadline)
            self._validate_indexed_result(result, previous_generation)
            del fragment
            state.release(amount)
            return result
        except BaseException:
            self._failed = True
            raise

    def knowledge_search(self, query='', *, sources=None, kind_ids=None,
                         predicate_ids=None, offset=0, limit=40):
        if self._failed or not self._supports_search:
            raise ValueError('native owned Store Search capability unavailable')
        try:
            state = self._state
            g = state.geometry
            # Mechanical public kwargs map before allocation; actual input strings
            # and lists are validated then identity-censused by _search.
            state.reserve(g.dict_bytes(6) + g.tuple_base + 2 * g.pointer)
            arguments = {'query': query, 'sources': sources, 'kind_ids': kind_ids,
                         'predicate_ids': predicate_ids, 'offset': offset, 'limit': limit}
            return self._search(arguments)
        except BaseException:
            self._failed = True
            raise


class NativeCoreOrdinarySessionResultClient(NativeCoreLazySessionResultClient):
    """Mechanical public calls admitted by the actual ordinary native READY.

    The old lazy ABI remains exact. This profile borrows native-declared
    capabilities; it cannot advertise an unimplemented method or create Stage,
    Query, Cold, publication or source authority in Python.
    """
    __slots__ = ('_capabilities', '_ordinary_whole', '_snapshot_guard')

    def __new__(cls, session, state, startup_frame, original_work_deadline_ns):
        owner = super().__new__(cls, session, state, startup_frame, original_work_deadline_ns)
        g = state.geometry
        code = NativeCoreLazySessionResultClient._validate_indexed_result.__code__
        slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
        state.reserve(types.FrameType.__basicsize__ + g.gc_header
                      + slots * (g.pointer + g.unicode_bytes(20)))
        for name in ('_has_capability', '_request', 'read_resource', 'render_resource'):
            code = getattr(cls, name).__code__
            slots = code.co_nlocals + len(code.co_cellvars) + len(code.co_freevars) + code.co_stacksize
            state.reserve(types.FrameType.__basicsize__ + g.gc_header
                          + slots * (g.pointer + g.unicode_bytes(20)))
        return owner

    def __init__(self, session, state, startup_frame, original_work_deadline_ns):
        if type(original_work_deadline_ns) is not int or not 0 < original_work_deadline_ns < 1 << 64:
            raise ValueError('native ordinary original work cutoff differs')
        self._state = state
        ready = state.decode(startup_frame)
        if type(ready) is dict and ready.get('ok') is False:
            from .native_core_session import NativeSessionRefused
            raise NativeSessionRefused.from_envelope(ready, state,
                'tos_native_core_ordinary_session_ready_v1')
        if (type(ready) is not dict
                or ready.get('schema_version') != 'tos_native_core_ordinary_session_ready_v1'
                or ready.get('profile') != 'tos_core_ordinary_selected_v1'
                or ready.get('ok') is not True
                or ready.get('reference_semantics') != 'cpython_pathlib_is_file_3_14'
                or 'source_revision' not in ready or ready['source_revision'] is not None
                or 'data_revision' not in ready or ready['data_revision'] is not None
                or ready.get('state_reused') is not False):
            raise ValueError('native ordinary startup profile differs')
        caps = ready.get('capabilities')
        if type(caps) is not list:
            raise ValueError('native ordinary capability receipt differs')
        whole = False
        for cap in caps:
            state.visit()
            if type(cap) is not dict or type(cap.get('operation')) is not str:
                raise ValueError('native ordinary capability entry differs')
            if len(cap) == 1:
                if cap['operation'] not in (*_OPERATIONS, *_ORDINARY_DIRECT, 'tos_native_resource_read'):
                    raise ValueError('native ordinary direct capability differs')
                continue
            profiles = cap.get('profiles')
            if (type(profiles) is not list or not 1 <= len(profiles) <= len(_PROFILES)
                    or any(type(profile) is not str or profile not in _PROFILES
                           or profile in profiles[:index]
                           for index, profile in enumerate(profiles))):
                raise ValueError('native ordinary capability profiles differ')
            for profile in profiles:
                state.visit()
            if len(cap) == 2 and cap['operation'] in _ORDINARY_DIRECT:
                pass
            elif (cap['operation'] == 'tos_native_call'
                    and type(cap.get('tool')) is str and cap['tool'].startswith(('tos_', 'tos.'))
                    and 4 < len(cap['tool']) <= 256):
                if len(cap) == 5 and cap['tool'] == 'tos_native_resource_read':
                    for key in ('resources', 'resource_templates'):
                        values = cap.get(key)
                        if type(values) is not list:
                            raise ValueError('native ordinary resource capability differs')
                        for value in values:
                            state.visit()
                            if type(value) is not str or not value:
                                raise ValueError('native ordinary resource capability differs')
                elif len(cap) != 3:
                    raise ValueError('native ordinary capability shape differs')
            else:
                raise ValueError('native ordinary capability shape differs')
            whole = whole or 'whole_root' in profiles
        guard = ready.get('snapshot_guard')
        if guard is not None and (type(guard) is not str or len(guard) != 64
                or any(c not in '0123456789abcdef' for c in guard)):
            raise ValueError('native ordinary snapshot guard receipt differs')
        self._snapshot_guard = guard
        self._capabilities, self._ordinary_whole = caps, whole
        self._supports_search = self._has_capability('tos_native_call', 'tos_knowledge_search')
        self._supports_indexed_search = self._has_capability('tos_native_call', 'tos_knowledge_search_indexed_v2', 'whole_root')
        self._supports_whole_legacy_search = self._has_capability('tos_native_call', 'tos_knowledge_search', 'whole_root')
        selection = ready.get('selection')
        self._validate_selection(selection)
        if selection['generation'] != 0 or selection['profile'] != 'selected_paths':
            raise ValueError('native ordinary initial selection differs')
        self._session, self._work_ns = session, original_work_deadline_ns
        self._selection, self._source_revision, self._failed = selection, None, False

    def _indexed_search_available(self):
        return (self._supports_indexed_search or self._has_capability(
            'tos_native_call', 'tos_knowledge_search_indexed_v2', 'weak_query_store'))

    def _validate_indexed_result(self, result, previous_generation):
        profile = self._selection['profile']
        if not self._has_capability('tos_native_call', 'tos_knowledge_search_indexed_v2', profile):
            raise ValueError('native indexed result profile was not advertised')
        if profile == 'whole_root':
            return super()._validate_indexed_result(result, previous_generation)
        if (profile != 'weak_query_store' or type(result) is not dict
                or result.get('schema') != 'tos_knowledge_search_indexed_v2'):
            raise ValueError('native indexed Store result owner differs')
        # Weak Store selection keeps its exploration association and no Whole
        # source cut. Native binds this packet/cursor to the held Store header.
        # Weak Store headers have no canonical-cut invariant. Native binds
        # this value to the authentic held header, including legacy null cuts.
        # Whole SourceRoot retains the separate strict64hex selection law.
        if ('source_revision' not in result
                or result['source_revision'] is not None and type(result['source_revision']) is not str):
            raise ValueError('native indexed Store source revision differs')

    def _has_capability(self, operation, tool=None, profile=None):
        for cap in self._capabilities:
            self._state.visit()
            if cap['operation'] == operation and cap.get('tool') == tool:
                if profile is None or (type(cap.get('profiles')) is list and profile in cap['profiles']):
                    return True
        return False

    def call(self, operation, arguments, *, absolute_deadline=None):
        if self._failed or type(operation) is not str or type(arguments) is not dict:
            raise ValueError('native ordinary public call unavailable')
        if operation in _OPERATIONS:
            if not self._has_capability(operation):
                raise ValueError('native ordinary direct capability unavailable')
            return super().call(operation, arguments, absolute_deadline=absolute_deadline)
        if operation in _ORDINARY_DIRECT:
            if not self._has_capability(operation):
                raise ValueError('native ordinary direct capability unavailable')
            return self._request(operation, arguments, absolute_deadline=absolute_deadline)
        if not self._has_capability('tos_native_call', operation):
            raise ValueError('native ordinary tool capability unavailable')
        if operation in ('tos_knowledge_search', 'tos_knowledge_search_indexed_v2'):
            state = self._state
            previous_generation = self._selection['generation']
            wrapper_bytes = state.geometry.dict_bytes(2)
            reserved = False
            try:
                # Admit the ordinary transport wrapper before creating it. The
                # caller's raw Search DTO remains the same object all the way to
                # the native JSON owner; Python adds no Search coercion layer.
                state.reserve(wrapper_bytes)
                reserved = True
                request_arguments = {'tool': operation, 'arguments': arguments}
                result = self._request(
                    'tos_native_call', request_arguments,
                    absolute_deadline=absolute_deadline)
                if operation == 'tos_knowledge_search_indexed_v2':
                    self._validate_indexed_result(result, previous_generation)
                else:
                    profile = self._selection['profile']
                    if (type(result) is not dict
                            or result.get('schema') != 'tos_knowledge_search_v1'):
                        raise ValueError('native legacy Search result owner differs')
                    if profile == 'whole_root':
                        if (not self._supports_whole_legacy_search
                                or self._selection['generation'] != previous_generation + 1
                                or result.get('source_revision') != self._source_revision):
                            raise ValueError('native legacy Whole Search result owner differs')
                    elif profile == 'weak_query_store':
                        # Native QueryStore binds this source revision to its
                        # held Store header; legacy V1 requires only the string
                        # field here, unlike indexed V2's canonical digest.
                        if type(result.get('source_revision')) is not str:
                            raise ValueError('native legacy Store source revision differs')
                    else:
                        raise ValueError('native legacy Search result profile differs')
                return result
            except BaseException:
                self._failed = True
                raise
            finally:
                if reserved:
                    state.release(wrapper_bytes)
        return self._request('tos_native_call', {'tool': operation, 'arguments': arguments},
                             absolute_deadline=absolute_deadline)

    def _execute(self, index, *, absolute_deadline=None):
        if not self._has_capability(_OPERATIONS[index]):
            raise ValueError('native ordinary direct capability unavailable')
        return super()._execute(index, absolute_deadline=absolute_deadline)

    def _request(self, operation, arguments, *, absolute_deadline=None):
        state = self._state
        try:
            state.active()
            work = self._work_ns
            if absolute_deadline is not None:
                if type(absolute_deadline) not in (float, int) or (type(absolute_deadline) is float and not math.isfinite(absolute_deadline)):
                    raise ValueError('native ordinary caller cutoff must be finite')
                if absolute_deadline <= 0:
                    raise TimeoutError('native ordinary narrower cutoff expired')
                if absolute_deadline <= work // 1000000000 + 1:
                    work = min(work, int(absolute_deadline * 1000000000))
            if time.monotonic_ns() >= work:
                raise TimeoutError('native ordinary original cutoff expired')
            retained_owner_state(state, (arguments,), maximum_objects=state._json.max_visits)
            cap = min(65536, self._session._limits.max_call_bytes)
            g = state.geometry
            # Existing NativeIO encoder first bounds ordinary input/UTF8 bytes.
            # This workspace bounds its quoted chunks, output copies, stack and
            # iterator owners before allocation; physical bootstrap stays shared.
            workspace = (8 * g.unicode_bytes(cap) + 4 * (g.bytes_base + cap)
                         + 4 * g.list_bytes(cap) + cap * g.unicode_bytes(24)
                         + 256 * (types.FrameType.__basicsize__ + g.gc_header
                                  + 64 * (g.pointer + g.unicode_bytes(20))))
            state.reserve(workspace)
            from .native_io import _bounded_json
            request = {'schema_version':'tos_native_core_session_call_v1',
                       'sequence':self._session._sequence, 'operation':operation,
                       'arguments':arguments, 'work_deadline_ns':work}
            previous_generation = self._selection['generation']
            encoded = _bounded_json(request, cap, work / 1e9, state._cancelled)
            payload = bytes(encoded)
            reply = self._session.call_bytes(payload)
            del payload, encoded, request
            state.release(workspace)
            envelope = state.decode(reply)
            if (type(envelope) is not dict
                    or envelope.get('schema_version') != 'tos_native_core_snapshot_result_v1'
                    or envelope.get('ok') is not True or 'result' not in envelope):
                raise ValueError('native ordinary result envelope differs')
            result = self._accept_envelope(envelope)
            if operation == 'tos_native_call' and not self._has_capability(
                    operation, arguments['tool'], self._selection['profile']):
                raise ValueError('native ordinary result profile was not advertised')
            if (self._selection['profile'] == 'whole_root'
                    and self._selection['generation'] != previous_generation + 1):
                raise ValueError('native ordinary Whole result generation did not advance')
            if (self._selection['profile'] == 'whole_root' and type(result) is dict
                    and 'source_revision' in result and result['source_revision'] != self._source_revision):
                raise ValueError('native ordinary result source cut differs')
            state.active()
            return result
        except BaseException:
            self._failed = True
            raise

    def read_resource(self, uri):
        if not self._has_capability('tos_native_call', 'tos_native_resource_read'):
            raise ValueError('native ordinary resource capability unavailable')
        return self._request('tos_native_call', {'tool':'tos_native_resource_read',
            'arguments':{'uri':uri, 'render':False}})

    def render_resource(self, uri):
        if not self._has_capability('tos_native_call', 'tos_native_resource_read'):
            raise ValueError('native ordinary render capability unavailable')
        result = self._request('tos_native_call', {'tool':'tos_native_resource_read',
            'arguments':{'uri':uri, 'render':True}})
        if type(result) is not str:
            raise ValueError('native ordinary rendered result type differs')
        return result
