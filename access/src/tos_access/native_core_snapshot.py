"""Explicit source-selected Core bridge over the private native snapshot CLI.

This is an opt-in imported SDK adapter. It does not replace ReferenceToSAccessCore,
resolve paths from the process at call time, or run the Python semantic builders.
"""
from __future__ import annotations

from contextlib import contextmanager
from dataclasses import dataclass
import fcntl
import json
import math
import os
from pathlib import Path
import socket
import stat
import time
from threading import RLock
from typing import Any, Callable

from .native_io import owned_exchange, _bounded_json

_SOURCE_PATH_FIELDS = (
    'index_path',
    'philosophy_graph_projection_path',
    'bibliographic_graph_path',
    'entity_type_registry_path',
    'relation_type_registry_path',
    'philosophy_post_planting_audit_path',
    'evidence_projection_path',
)
_SOURCE_OPERATION = {
    'index_exists': 'tos_corpus_index_exists',
    'index': 'tos_corpus_index',
    'source_navigation': 'tos_source_navigation',
    'bibliographic_graph': 'tos_bibliographic_graph',
    'evidence_projection_exists': 'tos_evidence_projection_exists',
    'evidence_projection': 'tos_evidence_projection',
    'corpus_header': 'tos_corpus_header',
    'knowledge_header': 'tos_knowledge_header',
    'philosophy_projection_exists': 'tos_philosophy_projection_exists',
    'philosophy_projection': 'tos_philosophy_projection',
    'philosophy_audit_exists': 'tos_philosophy_audit_exists',
    'philosophy_audit_payload': 'tos_philosophy_audit_payload',
}
_U64_MAX = (1 << 64) - 1
_INPUT_CAP = 16 * 1024 * 1024
_FRAME_CAP = 64 * 1024 * 1024
_STATE_ROLE = 'tos-native-core-snapshot-state-v1'
_STATE_SCHEMA = 'tos_native_core_snapshot_state_v1'
_RESULT_SCHEMA = 'tos_native_core_snapshot_result_v1'


def _absolute_path(value: str | Path, name: str) -> Path:
    path = Path(value).expanduser()
    if not path.is_absolute() or '..' in path.parts:
        raise ValueError(f'{name} requires a selected absolute path without parent traversal')
    return path


def _integer(value: Any, name: str, *, positive: bool = False) -> int:
    if type(value) is not int or value < (1 if positive else 0) or value > _U64_MAX:
        raise ValueError(f'{name} must be an explicitly admitted unsigned integer')
    return value


def _unique_pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('native Core packet contains duplicate JSON members')
        result[key] = value
    return result


def _reject_constant(value):
    raise ValueError(f'native Core packet contains non-finite JSON number {value}')


def _decode_object(raw: bytes, label: str) -> dict[str, Any]:
    try:
        value = json.loads(raw, object_pairs_hook=_unique_pairs, parse_constant=_reject_constant)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError(f'native Core {label} is not one strict JSON object') from error
    if type(value) is not dict:
        raise ValueError(f'native Core {label} is not one strict JSON object')
    return value


@dataclass(frozen=True)
class NativeCoreSnapshotSelection:
    """Caller-selected root, seven original carrier paths, and QueryStore selector.

    Use ``from_reference`` to capture the maintained Reference selector choices
    once. The search read model/compressed store is deliberately not an input.
    """

    tos_root: Path
    index_path: Path
    philosophy_graph_projection_path: Path
    bibliographic_graph_path: Path
    entity_type_registry_path: Path
    relation_type_registry_path: Path
    philosophy_post_planting_audit_path: Path
    evidence_projection_path: Path
    query_store_path: Path
    query_store_configured: bool

    def __post_init__(self):
        for field_name in ('tos_root', *_SOURCE_PATH_FIELDS, 'query_store_path'):
            object.__setattr__(self, field_name,
                               _absolute_path(getattr(self, field_name), field_name))
        if type(self.query_store_configured) is not bool:
            raise TypeError('query_store_configured must be a boolean')
        if len(os.fsencode(self.query_store_path)) > 8193:
            raise ValueError('selected QueryStore path exceeds the native selector bound')

    @classmethod
    def from_reference(cls, reference, *, query_store_path: str | Path | None = None):
        """Copy Reference's already selected paths without calling its readers.

        ``query_store_path`` is a separate additive native selector. When absent,
        this captures only the legacy TOS_QUERY_STORE_PATH or reference default;
        search_read_model_path is never consulted.
        """
        from .query_store import DEFAULT_RELATIVE_PATH

        root = Path(reference.tos_root).expanduser()
        if '..' in root.parts:
            raise ValueError('Reference tos_root parent traversal cannot be frozen as a native selector')
        if not root.is_absolute():
            root = Path(os.path.abspath(root))
        if query_store_path is not None:
            selected = Path(query_store_path).expanduser()
            configured = True
        else:
            ambient = os.environ.get('TOS_QUERY_STORE_PATH')
            configured = bool(ambient)
            selected = Path(ambient).expanduser() if configured else root / DEFAULT_RELATIVE_PATH
        if not selected.is_absolute():
            selected = root / selected
        fields = {}
        for name in _SOURCE_PATH_FIELDS:
            selected_path = Path(getattr(reference, name)).expanduser()
            if '..' in selected_path.parts:
                raise ValueError(f'Reference {name} parent traversal cannot be frozen as a native selector')
            if not selected_path.is_absolute():
                selected_path = Path(os.path.abspath(selected_path))
            fields[name] = selected_path
        return cls(tos_root=root, **fields,
                   query_store_path=selected,
                   query_store_configured=configured)

    def source_paths_wire(self) -> dict[str, str]:
        return {name: os.fspath(getattr(self, name)) for name in _SOURCE_PATH_FIELDS}

    def query_store_wire(self) -> dict[str, Any]:
        return {'path': os.fspath(self.query_store_path),
                'configured': self.query_store_configured}


@dataclass(frozen=True)
class NativeCoreJsonLimits:
    max_bytes: int
    max_depth: int
    max_visits: int
    max_integer_digits: int

    def wire(self):
        return {name: _integer(getattr(self, name), f'json.{name}', positive=True)
                for name in ('max_bytes', 'max_depth', 'max_visits', 'max_integer_digits')}


@dataclass(frozen=True)
class NativeCoreColdLimits:
    max_file_bytes: int
    max_vm_steps: int
    sqlite_cache_kib: int
    max_rows: int
    max_work_bytes: int
    max_row_bytes: int
    max_metadata_bytes: int
    max_sources: int

    def wire(self):
        return {name: _integer(getattr(self, name), f'cold.{name}', positive=True) for name in (
            'max_file_bytes', 'max_vm_steps', 'sqlite_cache_kib', 'max_rows',
            'max_work_bytes', 'max_row_bytes', 'max_metadata_bytes', 'max_sources')}


@dataclass(frozen=True)
class NativeCoreProcessLimits:
    address_space_bytes: int
    file_size_bytes: int

    def wire(self):
        return {name: _integer(getattr(self, name), f'process.{name}', positive=True)
                for name in ('address_space_bytes', 'file_size_bytes')}


@dataclass(frozen=True)
class NativeCoreQueryProfile:
    """Immutable JSON transport of explicit native query/HTTP owner budgets."""
    wire_json: bytes

    def wire(self):
        if type(self.wire_json) is not bytes or not 0 < len(self.wire_json) <= _INPUT_CAP:
            raise ValueError('Root query profile requires bounded immutable JSON bytes')
        return _decode_object(self.wire_json, 'query profile')


@dataclass(frozen=True)
class NativeCoreQueryStoreLimits:
    """Explicit per-operation immutable-store/header owner allowances."""
    max_database_bytes: int
    max_input_bytes: int
    max_json_bytes: int
    max_rows: int
    max_work_steps: int
    max_sql_vm_steps: int
    sqlite_cache_kib: int

    def wire(self):
        values = {name: _integer(getattr(self, name), f'query_store_limits.{name}', positive=True)
                  for name in ('max_database_bytes', 'max_input_bytes', 'max_json_bytes',
                               'max_rows', 'max_work_steps', 'max_sql_vm_steps', 'sqlite_cache_kib')}
        if values['max_json_bytes'] > (1 << 63) - 1:
            raise ValueError('query_store_limits.max_json_bytes exceeds native i64')
        if values['sqlite_cache_kib'] > (1 << 32) - 1:
            raise ValueError('query_store_limits.sqlite_cache_kib exceeds native u32')
        return values


@dataclass(frozen=True)
class NativeCoreSnapshotAdmission:
    """One caller-owned immutable operation allowance; no adapter defaults."""

    absolute_deadline: float
    operation_seconds: float
    stage_ticket_fd: int
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
    query_profile: NativeCoreQueryProfile | None = None
    query_store_limits: NativeCoreQueryStoreLimits | None = None

    def validate(self, boundary_started: float) -> float:
        try:
            finite = (type(self.absolute_deadline) in (int, float)
                      and math.isfinite(self.absolute_deadline)
                      and type(self.operation_seconds) in (int, float)
                      and math.isfinite(self.operation_seconds))
        except OverflowError:
            finite = False
        if not finite or self.operation_seconds <= 5:
            raise ValueError('Native Core admission requires its original finite deadline and span >5s')
        if type(self.stage_ticket_fd) is not int or self.stage_ticket_fd < 3:
            raise ValueError('Native Core admission requires the caller-owned stage ticket descriptor')
        fcntl.fcntl(self.stage_ticket_fd, fcntl.F_GETFD)
        os.fstat(self.stage_ticket_fd)
        for name in (
            'max_build_seconds', 'tmpfs_quota_bytes', 'inode_limit', 'working_ram_bytes',
            'whole_max_rows', 'whole_max_row_bytes', 'whole_max_graph_bytes',
            'whole_max_catalog_bytes', 'whole_max_catalog_inputs_bytes', 'whole_max_state_bytes',
        ):
            _integer(getattr(self, name), f'admission.{name}', positive=True)
        if not isinstance(self.json, NativeCoreJsonLimits):
            raise TypeError('Native Core admission requires the original JSON limits')
        if not isinstance(self.cold, NativeCoreColdLimits):
            raise TypeError('Native Core admission requires the original cold-query limits')
        if not isinstance(self.process, NativeCoreProcessLimits):
            raise TypeError('Native Core admission requires the original process limits')
        child_deadline = min(self.absolute_deadline,
                             boundary_started + self.operation_seconds)
        if not math.isfinite(child_deadline) or child_deadline <= boundary_started + 5:
            raise TimeoutError('Native Core admission has no work time before its cleanup reserve')
        return child_deadline

    def wire(self, work_deadline_ns: int) -> dict[str, Any]:
        values = {name: _integer(getattr(self, name), f'admission.{name}', positive=True) for name in (
            'max_build_seconds', 'tmpfs_quota_bytes', 'inode_limit', 'working_ram_bytes',
            'whole_max_rows', 'whole_max_row_bytes', 'whole_max_graph_bytes',
            'whole_max_catalog_bytes', 'whole_max_catalog_inputs_bytes', 'whole_max_state_bytes',
        )}
        values.update({
            'json': self.json.wire(),
            'operation_seconds': float(self.operation_seconds),
            'work_deadline_ns': _integer(work_deadline_ns, 'admission.work_deadline_ns', positive=True),
            'stage_ticket_fd': self.stage_ticket_fd,
            'cold': self.cold.wire(),
            'process': self.process.wire(),
        })
        return values


@dataclass(frozen=True)
class _QueryStoreSnapshot:
    """Authenticated zero-FD profile marker, never a native producer state."""
    def close(self):
        pass


@dataclass(frozen=True)
class _CallClock:
    admission: NativeCoreSnapshotAdmission
    child_deadline: float
    work_deadline_ns: int

    @property
    def work_deadline(self) -> float:
        return self.work_deadline_ns / 1_000_000_000


class NativeCoreSnapshotClient:
    """Synchronous explicit native bridge for the maintained Core public owners."""

    def __init__(self, native_prefix: str | Path,
                 selection: NativeCoreSnapshotSelection,
                 admission_provider: Callable[[str], NativeCoreSnapshotAdmission]):
        self.native_prefix = _absolute_path(native_prefix, 'native_prefix')
        if not isinstance(selection, NativeCoreSnapshotSelection):
            raise TypeError('native Core snapshot requires an explicit carrier selection')
        if not callable(admission_provider):
            raise TypeError('native Core snapshot requires a caller-owned admission provider')
        self.selection = selection
        self._admission_provider = admission_provider
        self._lock = RLock()
        self._closed = False
        self._state_file = None
        self._published_graph = None
        self._published_catalog = None
        self._catalog_graph = None

    @contextmanager
    def _operation(self, operation_id: str, absolute_deadline=None):
        boundary = time.monotonic()
        admission = self._admission_provider(operation_id)
        if not isinstance(admission, NativeCoreSnapshotAdmission):
            raise TypeError('admission provider must return one immutable NativeCoreSnapshotAdmission')
        child_deadline = admission.validate(boundary)
        if absolute_deadline is not None:
            if type(absolute_deadline) not in (int, float) or not math.isfinite(absolute_deadline):
                raise ValueError('outer native Root deadline must be finite')
            child_deadline = min(child_deadline, absolute_deadline)
            if child_deadline <= boundary + 5:
                raise TimeoutError('outer native Root cutoff expired before setup')
        work_deadline_ns = math.floor((child_deadline - 5.0) * 1_000_000_000)
        if work_deadline_ns <= 0:
            raise TimeoutError('Native Core work cutoff expired before request setup')
        remaining = child_deadline - time.monotonic()
        if remaining <= 0 or not self._lock.acquire(timeout=remaining):
            raise TimeoutError('Native Core admission expired waiting for its owner lock')
        try:
            if self._closed:
                raise RuntimeError('native Core snapshot client is closed')
            if time.monotonic() >= child_deadline:
                raise TimeoutError('Native Core admission expired before request setup')
            yield _CallClock(admission, child_deadline, work_deadline_ns)
        finally:
            self._lock.release()

    @staticmethod
    def _validate_state_file(file, admission: NativeCoreSnapshotAdmission):
        if file is None or file.closed:
            raise ValueError('native Core state descriptor is absent or closed')
        fd = file.fileno()
        fcntl.fcntl(fd, fcntl.F_GETFD)
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_size <= 0:
            raise ValueError('native Core state descriptor is not a nonempty regular sealed file')
        if info.st_size > admission.whole_max_state_bytes:
            raise ValueError('native Core state descriptor exceeds its caller-owned byte admission')
        try:
            seals = fcntl.fcntl(fd, fcntl.F_GET_SEALS)
            required = (fcntl.F_SEAL_SEAL | fcntl.F_SEAL_SHRINK |
                        fcntl.F_SEAL_GROW | fcntl.F_SEAL_WRITE)
        except (AttributeError, OSError) as error:
            raise ValueError('native Core state descriptor does not expose Linux memfd seals') from error
        if seals & required != required:
            raise ValueError('native Core state descriptor lacks the producer-required seals')

    def _exchange(self, operation_id: str, arguments: dict[str, Any], call: _CallClock,
                  *, prior_state=None, state_reply=False):
        admission = call.admission
        fds = [admission.stage_ticket_fd]
        argv = ['core-snapshot', '--root', os.fspath(self.selection.tos_root),
                '--operation', operation_id,
                '--work-deadline-ns', str(call.work_deadline_ns)]
        if prior_state is not None:
            self._validate_state_file(prior_state, admission)
            state_fd = prior_state.fileno()
            argv += ['--snapshot-state-fd', str(state_fd)]
            fds.append(state_fd)
        receiver = sender = None
        request = {
            'arguments': arguments,
            'admission': admission.wire(call.work_deadline_ns),
            'source_paths': self.selection.source_paths_wire(),
            'query_store': self.selection.query_store_wire(),
        }
        if admission.query_store_limits is not None:
            if not isinstance(admission.query_store_limits, NativeCoreQueryStoreLimits):
                raise TypeError('selected QueryStore requires typed caller limits')
            request['query_store_limits'] = admission.query_store_limits.wire()
        if operation_id == 'tos_native_call' and admission.query_profile is not None:
            if not isinstance(admission.query_profile, NativeCoreQueryProfile):
                raise TypeError('Root query requires typed caller-owned immutable query limits')
            request['http'] = admission.query_profile.wire()
        # Native selection requires this profile when the actual Root query
        # owner is used. A selected immutable-store catalog uses its separate
        # explicit limits, so the SDK does not require unrelated Root budgets.
        new_state = None
        received_state = None
        try:
            payload = _bounded_json(request, _INPUT_CAP, call.work_deadline)
            if state_reply:
                receiver, sender = socket.socketpair(socket.AF_UNIX, socket.SOCK_SEQPACKET)
                receiver.setsockopt(socket.SOL_SOCKET, socket.SO_PASSCRED, 1)
                argv += ['--state-reply-fd', str(sender.fileno())]
                fds.append(sender.fileno())
            inherited = dict(os.environ)
            for key in tuple(inherited):
                if key.startswith('TOS_') or key == 'ABYSS_STAGE_TICKET_FD':
                    inherited.pop(key, None)
            with owned_exchange(argv, prefix=self.native_prefix,
                                input_cap=_INPUT_CAP, frame_cap=_FRAME_CAP,
                                absolute_deadline=call.child_deadline,
                                operation_seconds=admission.operation_seconds,
                                env=inherited, pass_fds=tuple(fds)) as channel:
                channel.write_input(payload + b'\n', close=True)
                frames = iter(channel.frames())
                try:
                    first_frame = next(frames)
                except StopIteration as error:
                    raise ValueError('native Core private CLI returned no JSON frame') from error
                envelope = _decode_object(first_frame, 'result envelope')
                del first_frame
                if next(frames, None) is not None:
                    raise ValueError('native Core private CLI returned more than one JSON frame')
                base_keys = {'schema_version', 'ok', 'result'}
                weak_store = envelope.get('state_profile') == 'tos_query_store_v1'
                expected_keys = (base_keys | {'state_reused'}) if state_reply else base_keys
                if weak_store:
                    if not state_reply or operation_id not in ('tos_knowledge_graph', 'tos_knowledge_snapshot'):
                        raise ValueError('QueryStore snapshot profile is invalid for this operation')
                    expected_keys |= {'state_profile'}
                if (set(envelope) != expected_keys
                        or envelope.get('schema_version') != _RESULT_SCHEMA
                        or envelope.get('ok') is not True):
                    raise ValueError('native Core private result envelope differs from its source contract')
                if state_reply:
                    reused = envelope['state_reused']
                    if type(reused) is not bool:
                        raise ValueError('native Core state_reused marker must be boolean')
                    if operation_id == 'tos_knowledge_graph_addressed' and reused:
                        raise ValueError('addressed native Core updates must issue a successor state')
                    if weak_store and reused:
                        raise ValueError('QueryStore full export cannot claim native state reuse')
                    expected_count = 0 if reused or weak_store else 1
                    received_state, marker = channel.receive_descriptors(
                        receiver, expected_count=expected_count)
                    marker_object = _decode_object(marker, 'state marker')
                    expected_marker = ({'role': 'tos-native-query-store-snapshot-v1',
                                        'schema_version': 'tos_query_store_v1'} if weak_store
                                       else {'role': _STATE_ROLE, 'schema_version': _STATE_SCHEMA})
                    if marker_object != expected_marker:
                        raise ValueError('native Core state reply marker differs')
                    if weak_store:
                        new_state = _QueryStoreSnapshot()
                    if received_state is not None:
                        self._validate_state_file(received_state, admission)
                        new_state, received_state = received_state, None
                return envelope['result'], envelope.get('state_reused'), new_state
        except BaseException:
            if received_state is not None:
                received_state.close()
            if new_state is not None:
                new_state.close()
            raise
        finally:
            if sender is not None:
                sender.close()
            if receiver is not None:
                receiver.close()

    @staticmethod
    def _object(value, label):
        if type(value) is not dict:
            raise ValueError(f'native Core {label} must be an object')
        return value

    def _commit_successor(self, new_state, graph, catalog=None):
        if new_state is None:
            raise ValueError('native Core did not issue its required successor state')
        old_state = self._state_file
        self._state_file = None if isinstance(new_state, _QueryStoreSnapshot) else new_state
        self._published_graph = graph
        self._published_catalog = catalog
        self._catalog_graph = graph if catalog is not None else None
        if old_state is not None:
            old_state.close()

    def knowledge_graph(self) -> dict[str, Any]:
        op = 'tos_knowledge_graph'
        with self._operation(op) as call:
            result, reused, successor = self._exchange(
                op, {}, call, prior_state=self._state_file, state_reply=True)
            try:
                result = self._object(result, 'knowledge graph')
                if reused:
                    if self._state_file is None or self._published_graph is None:
                        raise ValueError('native Core reused absent SDK snapshot state')
                    # The public cache identity survives only after the native
                    # producer reports its checked unchanged-source verdict.
                    if successor is not None:
                        raise ValueError('reused native Core state unexpectedly returned a successor')
                    return self._published_graph
                self._commit_successor(successor, result)
                return result
            except BaseException:
                if successor is not None and successor is not self._state_file:
                    successor.close()
                raise

    def knowledge_graph_addressed(
        self, previous_graph: dict[str, Any], source_graph: str, source_id: str,
        source_record: dict[str, Any], *, source_revision: str,
        expected_parent_revision: str | None = None, return_report: bool = False,
    ) -> dict[str, Any]:
        op = 'tos_knowledge_graph_addressed'
        with self._operation(op) as call:
            if (self._published_graph is None or previous_graph is not self._published_graph
                    or self._state_file is None):
                from .core import AddressedUpdateError
                raise AddressedUpdateError(
                    'addressed update parent is stale; previous snapshot is not the current published snapshot')
            arguments = {
                'source_graph': source_graph,
                'source_id': source_id,
                'source_record': source_record,
                'source_revision': source_revision,
                'expected_parent_revision': expected_parent_revision,
                'return_report': return_report,
            }
            result, reused, successor = self._exchange(
                op, arguments, call, prior_state=self._state_file, state_reply=True)
            try:
                if reused is not False or successor is None:
                    raise ValueError('addressed native Core update did not issue its successor state')
                result = self._object(result, 'addressed result')
                graph = result.get('graph') if return_report else result
                self._object(graph, 'addressed graph')
                self._commit_successor(successor, graph)
                return result
            except BaseException:
                if successor is not None and successor is not self._state_file:
                    successor.close()
                raise

    def knowledge_snapshot(self) -> dict[str, Any]:
        op = 'tos_knowledge_snapshot'
        with self._operation(op) as call:
            result, reused, successor = self._exchange(
                op, {}, call, prior_state=self._state_file, state_reply=True)
            try:
                result = self._object(result, 'knowledge snapshot')
                if set(result) != {'graph', 'catalog'}:
                    raise ValueError('native Core snapshot public result keys differ')
                packet_graph = self._object(result['graph'], 'snapshot graph')
                catalog = self._object(result['catalog'], 'snapshot catalog')
                if reused:
                    if self._state_file is None or self._published_graph is None:
                        raise ValueError('native Core reused absent SDK snapshot state')
                    if successor is not None:
                        raise ValueError('reused native Core state unexpectedly returned a successor')
                    graph = self._published_graph
                    if self._catalog_graph is graph and self._published_catalog is not None:
                        catalog = self._published_catalog
                    else:
                        self._published_catalog = catalog
                        self._catalog_graph = graph
                    result = {'graph': graph, 'catalog': catalog}
                    return result
                self._commit_successor(successor, packet_graph, catalog)
                return {'graph': packet_graph, 'catalog': catalog}
            except BaseException:
                if successor is not None and successor is not self._state_file:
                    successor.close()
                raise

    def knowledge_snapshot_once(self, *, include_catalog_inputs: bool = False) -> dict[str, Any]:
        if type(include_catalog_inputs) is not bool:
            raise ValueError('include_catalog_inputs must be a boolean')
        op = 'tos_knowledge_snapshot_once'
        with self._operation(op) as call:
            result, reused, successor = self._exchange(
                op, {'include_catalog_inputs': include_catalog_inputs}, call)
            if reused is not None or successor is not None:
                if successor is not None:
                    successor.close()
                raise ValueError('one-shot native Core call must not return retained state')
            result = self._object(result, 'one-shot snapshot')
            expected = {'graph', 'catalog', 'source_state'}
            if include_catalog_inputs:
                expected.add('catalog_inputs')
            if set(result) != expected:
                raise ValueError('native one-shot snapshot public result keys differ')
            self._object(result['graph'], 'one-shot graph')
            self._object(result['catalog'], 'one-shot catalog')
            source_state = result['source_state']
            if type(source_state) is not list or len(source_state) != 5:
                raise ValueError('native one-shot source_state must contain the five original carriers')
            converted = []
            for row in source_state:
                if (type(row) is not list or len(row) != 5 or type(row[0]) is not str
                        or any(type(item) is not int for item in row[1:])):
                    raise ValueError('native one-shot source_state tuple shape differs')
                converted.append(tuple(row))
            result['source_state'] = tuple(converted)
            if include_catalog_inputs:
                fields = self._object(result['catalog_inputs'], 'catalog inputs')
                if set(fields) != {'header', 'entity_type_registry', 'relation_type_registry',
                                   'lenses', 'source_order_profile'}:
                    raise ValueError('native CatalogInputs fields differ')
                from .catalog_semantics import CatalogInputs, SEQUENCE_ORDER
                if fields['source_order_profile'] != SEQUENCE_ORDER:
                    raise ValueError('native source order profile differs from the original graph encounter order')
                result['catalog_inputs'] = CatalogInputs(
                    fields['header'], fields['entity_type_registry'],
                    fields['relation_type_registry'], fields['lenses'],
                    source_order_profile=fields['source_order_profile'])
            return result

    def _read(self, public_name: str, arguments: dict[str, Any] | None = None):
        op = _SOURCE_OPERATION[public_name]
        with self._operation(op) as call:
            result, reused, successor = self._exchange(op, arguments or {}, call)
            if reused is not None or successor is not None:
                if successor is not None:
                    successor.close()
                raise ValueError('native lower Core carrier call unexpectedly returned snapshot state')
            return result

    def call(self, tool: str, arguments: dict[str, Any], *, absolute_deadline=None) -> dict[str, Any]:
        """Native registry/parser owns tool availability and query semantics."""
        if type(tool) is not str or type(arguments) is not dict:
            raise TypeError('native Root call requires tool string and argument object')
        with self._operation(tool, absolute_deadline=absolute_deadline) as call:
            result, reused, successor = self._exchange(
                'tos_native_call', {'tool': tool, 'arguments': arguments}, call)
            if reused is not None or successor is not None:
                if successor is not None:
                    successor.close()
                raise ValueError('native Root query unexpectedly returned retained state')
            return self._object(result, 'Root query packet')

    def index_exists(self) -> bool:
        value = self._read('index_exists')
        if type(value) is not bool:
            raise ValueError('native index_exists result must be boolean')
        return value

    def index(self) -> dict[str, Any]:
        return self._object(self._read('index'), 'corpus index')

    def source_navigation(self, *, bibliographic_only: bool = False) -> dict[str, Any]:
        return self._object(self._read('source_navigation', {
            'bibliographic_only': bool(bibliographic_only)}), 'source navigation')

    def bibliographic_graph(self) -> dict[str, Any]:
        return self._object(self._read('bibliographic_graph'), 'bibliographic graph')

    def evidence_projection_exists(self) -> bool:
        value = self._read('evidence_projection_exists')
        if type(value) is not bool:
            raise ValueError('native evidence_projection_exists result must be boolean')
        return value

    def evidence_projection(self) -> dict[str, Any]:
        return self._object(self._read('evidence_projection'), 'evidence projection')

    def corpus_header(self) -> dict[str, Any]:
        return self._object(self._read('corpus_header'), 'corpus header')

    def knowledge_header(self) -> dict[str, Any]:
        return self._object(self._read('knowledge_header'), 'knowledge header')

    def philosophy_projection_exists(self) -> bool:
        value = self._read('philosophy_projection_exists')
        if type(value) is not bool:
            raise ValueError('native philosophy_projection_exists result must be boolean')
        return value

    def philosophy_projection(self) -> dict[str, Any]:
        return self._object(self._read('philosophy_projection'), 'philosophy projection')

    def philosophy_audit_exists(self) -> bool:
        value = self._read('philosophy_audit_exists')
        if type(value) is not bool:
            raise ValueError('native philosophy_audit_exists result must be boolean')
        return value

    def philosophy_audit_payload(self) -> dict[str, Any]:
        return self._object(self._read('philosophy_audit_payload'), 'philosophy audit payload')

    def close(self):
        with self._lock:
            if self._closed:
                return
            self._closed = True
            if self._state_file is not None:
                self._state_file.close()
                self._state_file = None
            self._published_graph = None
            self._published_catalog = None
            self._catalog_graph = None
