"""Synchronous retained-owner platform bridge for Rust source-read phases.

Owner callbacks keep their existing latency semantics and run only after the
previous native child has joined. Currentness is checked at owner bookends;
retaining Python objects does not freeze publication authority or create a lease.
"""
from __future__ import annotations

import fcntl
import json
import math
import os
from pathlib import Path
import secrets
import socket
import stat
import threading
import time
from types import SimpleNamespace

from .native_io import owned_exchange
from .source_read_errors import SourceReadError, SourceReadBudgetExceeded


class NativeSourceProvider:
    """Retain one actual service's objects; native code owns its domain rules."""

    def __init__(self, prefix, service):
        self.prefix = Path(prefix)
        if not self.prefix.is_absolute() or '..' in self.prefix.parts:
            raise ValueError('source provider requires an absolute native prefix')
        self._lock = threading.RLock()
        self._closed = False
        self._service = service
        self._binding = service.binding
        epoch = getattr(self._binding, 'epoch', None)
        self._expected_initial_epoch = None if epoch is None else {
            name: getattr(epoch, name) for name in (
                'source_revision', 'catalog_root_sha256', 'catalog_namespace', 'source_publication')}
        self._roles = {}
        for role, attribute in (('metadata', 'metadata_reader'), ('claim', 'claim_reader'),
                                ('slot', 'slot_reader'), ('authored', 'authored_reader')):
            reader = getattr(self._binding, attribute)
            if getattr(service, attribute) is not reader:
                raise SourceReadError('source provider reader differs from retained binding object')
            if reader is not None:
                self._roles[role] = reader
        if service.target_issuer is not None:
            self._roles['issuer'] = service.target_issuer
        self._descriptor = service.slot_descriptor
        catalog = getattr(self._binding, 'catalog_snapshot', None)
        self._stable_objects = tuple(self._roles.values()) + (() if catalog is None else (catalog,))
        record_types = service.metadata_record_types
        if (type(record_types) not in (tuple, list, set, frozenset)
                or len(record_types) > 2097152 // 2
                or any(type(value) is not str for value in record_types)):
            raise ValueError('owner record-type host collection exceeds the native frame profile')
        self._types = tuple(sorted(record_types))
        self._limits = {name: getattr(service.limits, name) for name in (
            'max_handle_bytes', 'max_request_bytes', 'max_record_bytes', 'max_response_bytes')}
        ceilings = (16384, 65536, 1048576, 2097152)
        for value, ceiling in zip(self._limits.values(), ceilings):
            if type(value) is not int or not 0 <= value <= ceiling:
                raise ValueError('custom source limits exceed the supported native provider profile')
        self._identities = {}
        self._held_objects = []
        self._transient_identities = {}
        self._transient_objects = []
        self._callback_measured_ns = 0
        self._session = secrets.token_hex(32)
        self._binding_context = None

    @classmethod
    def from_owner_readers(cls, prefix, *, metadata_reader=None, claim_reader=None,
                           slot_reader=None, target_issuer=None, metadata_record_types=(),
                           slot_descriptor=None, limits=None):
        """Native initialization of retained objects without a Python epoch builder."""
        binding = SimpleNamespace(binding_kind='owner-issued-reader',
            metadata_reader=metadata_reader, claim_reader=claim_reader,
            slot_reader=slot_reader, authored_reader=None)
        return cls._initialize(prefix, binding, target_issuer, metadata_record_types,
                               slot_descriptor, limits)

    @classmethod
    def from_prepared_source(cls, prefix, source_inputs_raw, *, catalog_snapshot,
                             metadata_reader=None, claim_reader=None, slot_reader=None,
                             authored_reader=None, target_issuer=None,
                             metadata_record_types=(), slot_descriptor=None, limits=None):
        """Rust parses actual immutable vector bytes and checks the actual readers."""
        if type(source_inputs_raw) is not bytes:
            raise TypeError('native prepared owner initialization requires retained raw bytes')
        binding = SimpleNamespace(binding_kind='prepared-source-vector',
            source_inputs=SimpleNamespace(raw=source_inputs_raw), catalog_snapshot=catalog_snapshot,
            metadata_reader=metadata_reader, claim_reader=claim_reader,
            slot_reader=slot_reader, authored_reader=authored_reader)
        return cls._initialize(prefix, binding, target_issuer, metadata_record_types,
                               slot_descriptor, limits)

    @classmethod
    def _initialize(cls, prefix, binding, issuer, record_types, descriptor, limits):
        if limits is None:
            limits = SimpleNamespace(max_handle_bytes=16384, max_request_bytes=65536,
                                     max_record_bytes=1048576, max_response_bytes=2097152)
        service = SimpleNamespace(binding=binding, metadata_reader=binding.metadata_reader,
            claim_reader=binding.claim_reader, slot_reader=binding.slot_reader,
            authored_reader=binding.authored_reader, target_issuer=issuer,
            metadata_record_types=record_types, slot_descriptor=descriptor, limits=limits)
        provider = cls(prefix, service)
        try:
            # Only the real native kernel validates/derives epochs and policy.
            provider.initial_capabilities = provider.capabilities()
            return provider
        except BaseException as primary:
            try:
                provider.close()
            except BaseException as cleanup:
                primary.add_note('native source provider initialization cleanup failed')
                raise primary from cleanup
            raise

    def _identity(self, value):
        if value is None:
            return None
        key = id(value)
        if not any(value is selected for selected in self._stable_objects):
            if key not in self._transient_identities:
                self._transient_identities[key] = secrets.token_hex(32)
                self._transient_objects.append(value)
            return self._transient_identities[key]
        if key not in self._identities:
            self._identities[key] = secrets.token_hex(32)
            self._held_objects.append(value)
        return self._identities[key]

    def _snapshot(self, reader):
        value = getattr(reader, 'catalog_snapshot', None)
        return value if value is not None else getattr(reader, '_catalog_snapshot', None)

    def _snapshot_value(self, snapshot, *, stable=False):
        if stable:
            identity = self._identity(snapshot)
        else:
            key = id(snapshot)
            if key not in self._transient_identities:
                self._transient_identities[key] = secrets.token_hex(32)
                self._transient_objects.append(snapshot)
            identity = self._transient_identities[key]
        return {'identity': identity, 'root_sha256': snapshot.root_sha256,
                'header': snapshot.header, 'namespace_path': (
                    None if getattr(snapshot, 'view', None) is None
                    or getattr(snapshot.view, 'namespace_path', None) is None
                    else str(snapshot.view.namespace_path))}

    def _invoke(self, method, *arguments, **keywords):
        started = time.monotonic_ns()
        try:
            return method(*arguments, **keywords)
        finally:
            self._callback_measured_ns += time.monotonic_ns() - started

    def _issued(self, reader):
        value = self._invoke(reader.source_read_binding)
        if type(value) is dict:
            return value
        # Scalar observation of an actual issued SourceEpoch, no epoch rules.
        return {name: getattr(value, name) for name in (
            'source_revision', 'catalog_root_sha256', 'catalog_namespace', 'source_publication')}

    def _observation(self, verify):
        roles = []
        kind = self._binding.binding_kind
        for role, reader in sorted(self._roles.items()):
            if verify:
                self._invoke(reader.verify_current)
            row = {'role': role, 'identity': self._identity(reader), 'epoch': None,
                   'snapshot': None, 'view': None, 'authored_identity': None}
            if kind == 'owner-issued-reader' or role == 'authored':
                row['epoch'] = self._issued(reader)
            if kind == 'prepared-source-vector':
                if role == 'authored':
                    view = reader.view
                    row['view'] = {'namespace_path': str(view.namespace_path),
                                   'root_json': view.root_bytes.decode('utf-8'),
                                   'snapshot_sha256': view.snapshot_digest}
                else:
                    row['snapshot'] = self._snapshot_value(self._snapshot(reader), stable=role == 'issuer')
                    if role == 'issuer':
                        row['authored_identity'] = self._identity(getattr(reader, 'authored_reader', None))
            roles.append(row)
        prepared = None
        if kind == 'prepared-source-vector':
            prepared = {'source_inputs_raw': self._binding.source_inputs.raw.decode('utf-8'),
                        'catalog_snapshot': self._snapshot_value(self._binding.catalog_snapshot, stable=True)}
        return {'binding_kind': kind, 'roles': roles,
                'metadata_record_types': list(self._types), 'prepared': prepared}

    def _callback(self, operation, arguments):
        if operation == 'binding':
            return self._observation(False)
        if operation == 'verify_current':
            return self._observation(True)
        if operation == 'issue_target':
            return self._invoke(self._roles['issuer'].issue, arguments['selector'])
        if operation == 'resolve_metadata':
            return self._invoke(self._roles['metadata'].resolve_typed, arguments['record_ref'])
        if operation == 'resolve_claim':
            return self._invoke(self._roles['claim'].resolve, arguments['record_ref'])
        if operation == 'slot_descriptor':
            kind, identity = arguments['kind'], arguments['identity']
            if self._descriptor is not None:
                selected = self._invoke(self._descriptor, kind, identity)
                if type(selected) is not dict:
                    return selected
                return {name: selected.get(name) for name in (
                    'row_sha256', 'canonical_sha256', 'visibility', 'provenance')}
            selected = self._invoke(self._snapshot(self._roles['slot']).get_slot, kind, identity)
            return {'row_sha256': selected.row_sha256,
                    'canonical_sha256': selected.source.get('canonical_sha256'),
                    'visibility': selected.source.get('visibility'), 'provenance': selected.provenance}
        if operation == 'read_slot':
            selected = self._invoke(self._roles['slot'].read_slot, arguments['kind'], arguments['identity'],
                expected_row_sha256=arguments['expected_row_sha256'])
            row = getattr(selected, 'row_sha256', None)
            if row is None:
                row = getattr(getattr(selected, 'slot', None), 'row_sha256', None)
            raw = getattr(selected, 'raw_bytes', None)
            return {'row_sha256': row, 'payload': getattr(selected, 'payload', None),
                    'raw_bytes_len': len(raw) if isinstance(raw, bytes) else None,
                    'provenance': getattr(selected, 'provenance', None)}
        if operation == 'resolve_authored_csv':
            selected = self._invoke(self._roles['authored'].read, arguments['target'])
            if selected is None:
                return {'status': 'missing', 'reason': 'authored-corpus-target-missing',
                        'record': None, 'provenance': None}
            record, provenance = selected
            return {'status': 'available', 'reason': 'exact-owner-source-record',
                    'record': record, 'provenance': provenance}
        raise SourceReadError('native provider requested an unknown callback')

    @staticmethod
    def _state(file, marker, expected=b'tos-source-owner-state-v1'):
        value = os.fstat(file.fileno())
        seals = fcntl.F_SEAL_WRITE | fcntl.F_SEAL_GROW | fcntl.F_SEAL_SHRINK | fcntl.F_SEAL_SEAL
        if (marker != expected or not stat.S_ISREG(value.st_mode)
                or value.st_nlink != 0 or value.st_uid != os.geteuid()
                or not 0 < value.st_size <= 4 * 1048576
                or fcntl.fcntl(file.fileno(), fcntl.F_GET_SEALS) != seals):
            raise SourceReadError('native provider continuation descriptor differs')

    @staticmethod
    def _clock(clock, started, callbacks, terminal, caller_deadline):
        fields = {'execution_started_ns', 'callback_elapsed_ns', 'completed_native_ns',
                  'phase_started_ns', 'native_budget_ns', 'work_budget_ns',
                  'execution_budget_ns', 'caller_deadline_ns'}
        if (type(clock) is not dict or set(clock) != fields
                or any(type(clock[name]) is not int or clock[name] < 0
                       for name in fields - {'caller_deadline_ns'})
                or clock['execution_started_ns'] != started
                or clock['callback_elapsed_ns'] != callbacks
                or clock['caller_deadline_ns'] != caller_deadline
                or clock['native_budget_ns'] != 5_000_000_000
                or clock['work_budget_ns'] != 45_000_000_000
                or clock['execution_budget_ns'] != 50_000_000_000
                or not started <= clock['phase_started_ns'] <= terminal):
            raise SourceReadError('native provider clock ledger differs')
        if clock['completed_native_ns'] + terminal - clock['phase_started_ns'] >= 5_000_000_000:
            raise TimeoutError('cumulative native source domain budget exceeded')
        if time.monotonic_ns() - started - callbacks >= 45_000_000_000:
            raise TimeoutError('cumulative native source work envelope exceeded')

    def call(self, operation, request, *, absolute_deadline=None, cancelled=None):
        # This explicit profile cannot preempt a noncooperative Python callback.
        if absolute_deadline is not None and (
                type(absolute_deadline) not in (int, float) or not math.isfinite(absolute_deadline)):
            raise ValueError('source provider deadline must be finite')
        def active():
            if self._closed or (cancelled is not None and cancelled.is_set()):
                raise SourceReadError('source provider closed or cancelled')
            if absolute_deadline is not None and time.monotonic() >= absolute_deadline:
                raise TimeoutError('source provider deadline expired')
        # Existing synchronous ownership semantics: no abandoned lock waiter.
        with self._lock:
            active()
            self._transient_identities = {}
            self._transient_objects = []
            session = self._session
            outer_started_ns = time.monotonic_ns()
            envelope = {'schema_version': 'tos_source_owner_begin_v1', 'session': session,
                        'operation': operation, 'request': request, 'limits': self._limits,
                        'caller_deadline_ns': None if absolute_deadline is None else int(absolute_deadline * 1e9),
                        'execution_started_ns': outer_started_ns,
                        'expected_initial_epoch': self._expected_initial_epoch}
            caller_deadline_ns = envelope['caller_deadline_ns']
            state = None
            outer_started = outer_started_ns / 1e9
            callback_total_ns = 0
            previous_terminal_ns = None
            accepted_complete = False
            try:
                for phase in range(8):
                    active()
                    successor = None
                    parent, child = socket.socketpair(socket.AF_UNIX, socket.SOCK_SEQPACKET)
                    try:
                        parent.setsockopt(socket.SOL_SOCKET, socket.SO_PASSCRED, 1)
                        parent.setblocking(False)
                        arguments = ['source', 'owner-provider-phase', '--channel-fd', str(child.fileno())]
                        inherited = [child.fileno()]
                        if state is not None:
                            arguments += ['--state-fd', str(state.fileno())]
                            inherited.append(state.fileno())
                        elif self._binding_context is not None:
                            arguments += ['--binding-fd', str(self._binding_context.fileno())]
                            inherited.append(self._binding_context.fileno())
                        # One non-provider clock across ALL phases. Only actual
                        # completed synchronous callback intervals extend it.
                        phase_deadline = outer_started + 50 + callback_total_ns / 1e9
                        if absolute_deadline is not None:
                            phase_deadline = min(phase_deadline, absolute_deadline)
                        with owned_exchange(arguments, prefix=self.prefix,
                                input_cap=2097152 + 4096, frame_cap=2097152 + 65536,
                                absolute_deadline=phase_deadline, cancelled=cancelled,
                                pass_fds=tuple(inherited), env={
                                    key: value for key, value in os.environ.items()
                                    if key not in {'TOS_RELEASE_ROOT', 'TOS_DATA_ROOT'}
                                }) as channel:
                            channel.send(envelope)
                            channel.close_input()
                            frames = iter(channel.frames())
                            try:
                                result = json.loads(next(frames))
                            except StopIteration as error:
                                raise SourceReadError('native provider returned no phase result') from error
                            if next(frames, None) is not None:
                                raise SourceReadError('native provider returned multiple phase results')
                            if (type(result) is not dict or set(result) != {
                                    'schema_version', 'session', 'request_id', 'status', 'callback', 'packet', 'clock'}
                                    or result['schema_version'] != 'tos_source_owner_phase_v1'
                                    or result['session'] != session or type(result['request_id']) is not int
                                    or result['request_id'] != phase + 1):
                                raise SourceReadError('native provider phase envelope differs')
                            if result['status'] == 'callback':
                                successor, marker = channel.receive_descriptor(parent)
                                self._state(successor, marker)
                            elif result['status'] == 'complete':
                                successor, marker = channel.receive_descriptor(parent)
                                self._state(successor, marker, b'tos-source-owner-binding-v1')
                            else:
                                raise SourceReadError('native provider phase status differs')
                            observed_terminal_ns = channel.terminal_observed_ns()
                        # owned_exchange has signalled then solely reaped and
                        # joined. No callback is allowed before this boundary.
                        active()
                        self._clock(result['clock'], outer_started_ns, callback_total_ns,
                                    observed_terminal_ns, caller_deadline_ns)
                        previous_terminal_ns = observed_terminal_ns
                        if state is not None:
                            state.close()
                            state = None
                        if result['status'] == 'complete':
                            if result['callback'] is not None or type(result['packet']) is not dict:
                                raise SourceReadError('native provider final packet envelope differs')
                            previous, self._binding_context = self._binding_context, successor
                            successor = None
                            if previous is not None:
                                previous.close()
                            accepted_complete = True
                            return result['packet']
                        if result['packet'] is not None:
                            raise SourceReadError('native callback phase disclosed a packet')
                        state, successor = successor, None
                        plan = result['callback']
                        if (type(plan) is not dict or set(plan) != {
                                'schema_version', 'session', 'request_id', 'operation', 'arguments'}
                                or plan['schema_version'] != 'tos_source_owner_provider_request_v1'
                                or plan['session'] != session or plan['request_id'] != phase + 1
                                or type(plan['arguments']) is not dict):
                            raise SourceReadError('native provider callback envelope differs')
                        active()
                        self._callback_measured_ns = 0
                        try:
                            value = self._callback(plan['operation'], plan['arguments'])
                            status, error = 'ok', None
                        except Exception as failure:
                            value, status = None, 'error'
                            error = {'status': 'corrupt', 'reason': 'owner-callback-refused'}
                            if isinstance(failure, SourceReadBudgetExceeded):
                                error = {'status': 'over-budget', 'reason': 'source-read-budget'}
                            elif isinstance(failure, PermissionError):
                                error = {'status': 'access-restricted', 'reason': 'owner-source-path-restricted'}
                            elif isinstance(failure, FileNotFoundError):
                                error = {'status': 'missing', 'reason': 'owner-source-record-missing'}
                            elif isinstance(failure, SourceReadError):
                                error = {'status': 'corrupt', 'reason': str(failure)[:256]}
                            elif isinstance(failure, OSError):
                                error = {'status': 'corrupt', 'reason': 'owner-source-io-failed'}
                            elif isinstance(failure, ValueError) and plan['operation'] == 'resolve_authored_csv':
                                error = {'status': 'corrupt', 'reason': 'authored-csv-source-binding-not-verified'}
                            elif type(failure).__name__ == '_OwnerUnavailable':
                                error = {'status': failure.status, 'reason': failure.reason}
                        elapsed = self._callback_measured_ns
                        callback_total_ns += elapsed
                        active()  # Expired callback output is never disclosed.
                        envelope = {'schema_version': 'tos_source_owner_provider_response_v1',
                                    'session': session, 'request_id': plan['request_id'],
                                    'operation': plan['operation'], 'status': status,
                                    'value': value, 'error': error, 'callback_elapsed_ns': elapsed,
                                    'previous_phase_terminal_ns': previous_terminal_ns}
                    finally:
                        parent.close()
                        child.close()
                        if successor is not None:
                            successor.close()
                raise SourceReadError('native provider phase count exceeded')
            finally:
                if state is not None:
                    state.close()
                self._transient_identities.clear()
                self._transient_objects.clear()
                if accepted_complete:
                    # Final cleanup belongs to the same work/deadline envelope.
                    active()
                    self._clock(result['clock'], outer_started_ns, callback_total_ns,
                                observed_terminal_ns, caller_deadline_ns)

    def capabilities(self):
        return self.call('capabilities', {})

    def discover(self, request):
        return self.call('discover', request)

    def read(self, request):
        return self.call('read', request)

    def close(self):
        with self._lock:
            self._closed = True
            context, self._binding_context = self._binding_context, None
            try:
                if context is not None:
                    context.close()
            finally:
                self._identities.clear()
                self._held_objects.clear()
                self._transient_identities.clear()
                self._transient_objects.clear()
