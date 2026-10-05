"""Typed lower results and native-issued association over one owned Driver.

The seven-field receipt is a channel association, never a grant or model. The
native owner chooses the configured/default Store before CorpusHeader capture.
No Python source payload, SQLite query, header projection or fallback is used.
"""
import types
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
    __slots__ = ('_session', '_state', '_work_ns', '_source_revision', '_selection', '_failed', '_supports_search', '_supports_indexed_search')
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
                     '_search', '_indexed_search', 'knowledge_search'):
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
        if type(caps) is not list or len(caps) not in (9, 10, 11):
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
        self._supports_indexed_search = len(caps) == 11
        if self._supports_indexed_search:
            cap = caps[10]
            if (type(cap) is not dict or len(cap) != 3
                    or cap.get('operation') != 'tos_native_call'
                    or cap.get('tool') != 'tos_knowledge_search_indexed_v2'
                    or type(cap.get('profiles')) is not list or len(cap['profiles']) != 1
                    or cap['profiles'][0] != 'whole_root'):
                raise ValueError('native indexed Whole capability differs')
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
            if not self._supports_indexed_search:
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
        if operation == 'tos_knowledge_search_indexed_v2' and self._supports_indexed_search:
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
            result = self._call(fragment, absolute_deadline=absolute_deadline)
            if type(result) is not dict or self._selection['profile'] != 'weak_query_store':
                raise ValueError('native Search result owner differs')
            del fragment
            state.release(amount)
            return result
        except BaseException:
            self._failed = True
            raise

    def _indexed_search(self, arguments, *, absolute_deadline=None):
        if self._failed or not self._supports_indexed_search:
            raise ValueError('native indexed Whole Search capability unavailable')
        try:
            state = self._state
            validate_search_arguments(state, arguments, indexed=True)
            # This exact identity registry never refunds external caller state.
            retained_owner_state(state, (arguments,), maximum_objects=state._json.max_visits)
            fragment, amount = search_fragment(state, arguments, self._session._limits.max_call_bytes, indexed=True)
            previous_generation = self._selection['generation']
            result = self._call(fragment, absolute_deadline=absolute_deadline)
            if (type(result) is not dict or self._selection['profile'] != 'whole_root'
                    or self._selection['generation'] != previous_generation + 1
                    or result.get('schema') != 'tos_knowledge_search_indexed_v2'
                    or result.get('source_revision') != self._source_revision):
                raise ValueError('native Search result owner differs')
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
