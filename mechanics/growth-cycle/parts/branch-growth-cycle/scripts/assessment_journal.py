"""Source-owner assessment journal; immutable batches and an atomic head pointer.

This is a local Unix storage/command adapter, not an HTTP authentication boundary.
The owner service, or independently selected protected local configuration,
supplies trusted engine, subject context and Submission bindings. Source records
remain authoritative;
this journal records assessments and their commit-time qualification, not a
second corpus or an independently authoritative cached admission database.
"""
from __future__ import annotations

from contextlib import contextmanager, ExitStack
from dataclasses import dataclass, replace
from datetime import datetime, timezone
import fcntl
import hashlib
import io
import json
import os
from pathlib import Path
import re
import stat
import sys
import tempfile
import threading
import time
from typing import Any, Callable, Iterator, Sequence
from jsonschema import Draft202012Validator, FormatChecker, ValidationError

from knowledge_assessment import (
    AssessmentEngine, CommittedScope, MAX_ASSESSMENTS, MAX_RECORD_BYTES, SubjectContext,
    Record, RequiredAdmission, Submission, _canonical, _instant, _validators,
)


class JournalConflict(ValueError):
    """Stale expected head, reused command identity, or changed subject scope."""


class JournalCorruption(ValueError):
    """Owned history cannot be read consistently; never recover by ignoring it."""


class JournalBusy(TimeoutError):
    """A live writer owns this subject; retry without discarding expected state."""


class AssessmentRejected(ValueError):
    def __init__(self, invalid: list[dict[str, Any]]):
        self.invalid_assessments = invalid
        super().__init__('new assessments failed qualification; no batch was published')


@dataclass(frozen=True)
class _RecordedExecution:
    id: str
    version: int
    digest: str

    @property
    def ref(self) -> dict[str, Any]:
        return {'id': self.id, 'version': self.version, 'digest': self.digest}


def _serialize(submission: Submission) -> dict[str, Any]:
    return {'assessment': submission.assessment, 'principal_id': submission.principal_id,
            'execution_profile': submission.execution_profile.ref}


def _restore(payload: dict[str, Any], scope: CommittedScope | None = None) -> Submission:
    return Submission(payload['assessment'], payload['principal_id'],
                      _RecordedExecution(**payload['execution_profile']), scope)


def _digest(value: Any) -> str:
    return hashlib.sha256(_canonical(value)).hexdigest()


def _sync_directory(directory: Path) -> None:
    descriptor = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


class AssessmentJournal:
    """A configured owner directory, partitioned by stable subject ID.

    Each immutable JSON batch extends one subject's hash chain. Atomic rename
    of its small head pointer publishes the complete batch. Abandoned blobs
    are not visible history and are not silently deleted. A retry can reuse a
    matching blob; a new process needs no in-memory continuation state.
    """

    def __init__(self, directory: Path, *, contract_root: Path | None = None,
                 lock_timeout_seconds: float = 5.0, protected_storage: bool = False,
                 confidential_root: Path | None = None, batch_validator=None):
        if confidential_root is not None:
            if not directory.is_absolute() or not directory.is_relative_to(confidential_root):
                raise PermissionError('confidential journal leaves its selected owner root')
            from source_owner_context import _open
            os.close(_open(directory, directory=True, private_root=confidential_root))
        self.directory = directory.resolve()
        if not 0 <= lock_timeout_seconds <= 60:
            raise ValueError('lock timeout must be between zero and sixty seconds')
        self.lock_timeout_seconds = lock_timeout_seconds
        self.protected_storage = protected_storage or confidential_root is not None
        self.confidential_root = confidential_root
        self._thread_locks = threading.local()
        if not self.directory.parent.is_dir():
            raise ValueError('the configured owner parent directory must already exist')
        self.validator = (batch_validator if batch_validator is not None else
                          _validators((contract_root or Path(__file__).resolve().parents[5]).resolve())['-batch'])

    def _check_path(self, path, *, directory=False):
        if self.confidential_root is not None:
            from source_owner_context import _open
            os.close(_open(path, directory=directory, private_root=self.confidential_root))
        else:
            os.close(_owned_path(path, directory=directory))

    def _home(self, subject_id: str) -> Path:
        # Hashes partition storage only; they do not replace ToS identity.
        return self.directory / hashlib.sha256(subject_id.encode('utf-8')).hexdigest()

    @contextmanager
    def _locked(self, home: Path, *, deadline: float | None = None) -> Iterator[None]:
        held = getattr(self._thread_locks, 'homes', None)
        if held is None:
            held = self._thread_locks.homes = set()
        if home in held:
            # Reuse only this instance's actual same-thread held flock. Other
            # threads/processes still acquire the ordinary filesystem lock.
            yield
            return
        self.directory.mkdir(mode=0o700, exist_ok=True)
        _sync_directory(self.directory.parent)
        home.mkdir(mode=0o700, exist_ok=True)
        _sync_directory(self.directory)
        if self.protected_storage:
            self._check_path(home, directory=True)
        lock_path = home / '.writer.lock'
        descriptor = os.open(lock_path, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_NONBLOCK, 0o600)
        with os.fdopen(descriptor, 'a+b') as lock:
            if not stat.S_ISREG(os.fstat(lock.fileno()).st_mode):
                raise JournalCorruption('writer lock must be a regular file')
            if self.protected_storage:
                self._check_path(lock_path)
            deadline = min(deadline, time.monotonic() + self.lock_timeout_seconds) if deadline is not None else (
                time.monotonic() + self.lock_timeout_seconds)
            while True:
                try:
                    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    break
                except BlockingIOError:
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        raise JournalBusy('subject writer is busy; no history was changed') from None
                    time.sleep(min(0.01, remaining))
            try:
                held.add(home)
                yield
            finally:
                held.discard(home)
                fcntl.flock(lock, fcntl.LOCK_UN)

    @contextmanager
    def locked_subjects(self, subject_ids: Sequence[str]) -> Iterator[None]:
        """One bounded, ordered current-read/commit boundary for dependencies.

        This does not commit multiple subjects atomically. It prevents their
        existing heads from changing while one dependent decision is read or
        written; all writers keep using the same per-subject journal locks.
        """
        if (not isinstance(subject_ids, (list, tuple)) or not subject_ids or len(subject_ids) > 65
                or any(not isinstance(identity, str) or not identity.strip() for identity in subject_ids)
                or len(set(subject_ids)) != len(subject_ids)):
            raise ValueError('journal lock scope needs at most 65 distinct subject identities')
        homes = sorted(self._home(identity) for identity in subject_ids)
        held = getattr(self._thread_locks, 'homes', set())
        if held and not set(homes).issubset(held):
            raise JournalConflict('cannot expand an already acquired journal lock set')
        deadline = time.monotonic() + self.lock_timeout_seconds
        with ExitStack() as stack:
            for home in homes:
                stack.enter_context(self._locked(home, deadline=deadline))
            yield

    def _load(self, subject_id: str) -> tuple[str | None, list[dict[str, Any]]]:
        home = self._home(subject_id)
        if self.protected_storage and (home.exists() or home.is_symlink()):
            self._check_path(home, directory=True)
        head = home / 'head'
        if head.is_symlink():
            raise JournalCorruption('a head pointer cannot be a symlink')
        if not head.exists():
            return None, []
        if self.protected_storage:
            self._check_path(head)
        if not head.is_file() or head.stat().st_size > 65:
            raise JournalCorruption('invalid head pointer')
        try:
            revision = head.read_text(encoding='ascii').strip()
        except (OSError, UnicodeError) as exc:
            raise JournalCorruption('unreadable head pointer') from exc
        cursor: str | None = revision
        chain, seen = [], set()
        event_count = 0
        try:
            while cursor is not None:
                if not re.fullmatch(r'[a-f0-9]{64}', cursor) or cursor in seen:
                    raise JournalCorruption('invalid or cyclic head chain')
                seen.add(cursor)
                path = home / (cursor + '.json')
                if self.protected_storage:
                    self._check_path(path)
                if path.stat().st_size > MAX_RECORD_BYTES:
                    raise JournalCorruption('batch exceeds its record size limit')
                batch = json.loads(path.read_text(encoding='utf-8'))
                self.validator.validate(batch)
                if (_digest(batch) != cursor or batch.get('schema_version') != 'tos_assessment_batch_v1'
                        or batch.get('subject_id') != subject_id or not isinstance(batch.get('events'), list)
                        or not batch['events'] or not isinstance(batch.get('request'), dict)
                        or _digest(batch['request']) != batch.get('request_digest')
                        or batch['request']['subject']['id'] != subject_id
                        or batch['request']['expected_revision'] != batch['previous_revision']):
                    raise JournalCorruption('batch identity or request binding is corrupt')
                event_count += len(batch['events'])
                if event_count > MAX_ASSESSMENTS:
                    raise JournalCorruption('history exceeds bounded materialization; do not truncate it')
                chain.append(batch)
                cursor = batch['previous_revision']
            chain.reverse()
            known_events: dict[str, bytes] = {}
            for index, batch in enumerate(chain):
                if (batch['sequence'] != index + 1
                        or (index and _instant(batch['recorded_at']) < _instant(chain[index - 1]['recorded_at']))):
                    raise JournalCorruption('batch sequence or chronology is corrupt')
                expected: dict[bytes, dict[str, Any]] = {}
                for event in batch['request']['events']:
                    identifier = event['assessment']['assessment_id']
                    encoded = _canonical(event)
                    if identifier in known_events:
                        if known_events[identifier] != encoded:
                            raise JournalCorruption('an assessment identity was rewritten')
                    else:
                        expected[encoded] = event
                if list(expected.values()) != batch['events']:
                    raise JournalCorruption('batch does not record exactly its new request events')
                for event in batch['events']:
                    identifier = event['assessment']['assessment_id']
                    if identifier in known_events:
                        raise JournalCorruption('duplicate assessment event')
                    known_events[identifier] = _canonical(event)
        except (OSError, KeyError, TypeError, ValidationError, json.JSONDecodeError, UnicodeError) as exc:
            raise JournalCorruption('cannot resolve the complete owned history') from exc
        return revision, chain

    @staticmethod
    def _history(chain: Sequence[dict[str, Any]]) -> list[Submission]:
        result = []
        for batch in chain:
            request = batch['request']
            scope = CommittedScope(request['layer'], request['risk'], tuple(request['languages']),
                                   request['maker_id'], request['use'])
            result.extend(_restore(event, scope) for event in batch['events'])
        return result

    def inspect(self, engine: AssessmentEngine, context: SubjectContext, *, now: str) -> dict[str, Any]:
        if context.access_allowed is not True:
            raise PermissionError('subject access is not allowed')
        revision, chain = self._load(context.record.id)
        return {'revision': revision, 'batch_count': len(chain),
                'current_admission': engine.evaluate(context, (), now=now, trusted_history=self._history(chain))}

    def _write_blob(self, home: Path, revision: str, payload: bytes) -> None:
        target = home / (revision + '.json')
        if self.protected_storage and (target.exists() or target.is_symlink()):
            self._check_path(target)
        if target.exists():
            if target.read_bytes() != payload:
                raise JournalCorruption('immutable batch name has conflicting bytes')
            return
        with tempfile.NamedTemporaryFile(dir=home, prefix='.pending-', delete=False) as temporary:
            staging = Path(temporary.name)
            temporary.write(payload)
            temporary.flush()
            os.fsync(temporary.fileno())
        try:
            # No overwrite of a published or abandoned immutable batch.
            os.link(staging, target)
            _sync_directory(home)
        finally:
            staging.unlink(missing_ok=True)

    def _publish_head(self, home: Path, revision: str) -> None:
        with tempfile.NamedTemporaryFile(dir=home, prefix='.head-', delete=False) as temporary:
            staging = Path(temporary.name)
            temporary.write((revision + '\n').encode('ascii'))
            temporary.flush()
            os.fsync(temporary.fileno())
        try:
            os.replace(staging, home / 'head')
            _sync_directory(home)
        finally:
            staging.unlink(missing_ok=True)

    def append(self, engine: AssessmentEngine, context: SubjectContext, reviews: Sequence[Submission],
               *, command_id: str, expected_revision: str | None, now: str,
               snapshot_guard: Callable[[], None] | None = None) -> dict[str, Any]:
        """Atomically record qualified judgments, including rejections/deferrals.

        A successful commit does not mean the assertion was admitted. Replay
        returns the historical receipt AND freshly evaluated current admission;
        it cannot revive an expired or revoked decision. Permission/authentication
        must be established by the owner adapter before calling this method.
        """
        if not isinstance(command_id, str) or not command_id.strip() or len(command_id) > 256:
            raise ValueError('a bounded nonempty command ID is required')
        if not reviews or len(reviews) > MAX_ASSESSMENTS:
            raise ValueError('a nonempty bounded assessment batch is required')
        if context.access_allowed is not True:
            raise PermissionError('subject access is not allowed')
        timestamp = _instant(now)
        events = sorted((_serialize(review) for review in reviews), key=_canonical)
        request = {'command_id': command_id, 'subject': context.record.ref, 'events': events,
                   'expected_revision': expected_revision, 'layer': context.assertion_layer,
                   'risk': context.risk, 'languages': list(context.languages),
                   'maker_id': context.maker_id, 'use': context.requested_use}
        request_digest = _digest(request)
        home = self._home(context.record.id)
        with self._locked(home):
            if snapshot_guard is not None:
                snapshot_guard()
            current, chain = self._load(context.record.id)
            history = self._history(chain)
            for old in chain:
                if old['request']['command_id'] == command_id:
                    if old['request_digest'] != request_digest:
                        raise JournalConflict('command ID is already bound to a different request')
                    if snapshot_guard is not None:
                        snapshot_guard()
                    return {'revision': current, 'receipt': old, 'replayed': True,
                            'current_admission': engine.evaluate(context, (), now=now, trusted_history=history)}
            if current != expected_revision:
                raise JournalConflict('expected assessment revision is stale')
            if chain and timestamp < _instant(chain[-1]['recorded_at']):
                raise JournalConflict('journal time cannot move backward')
            result = engine.evaluate(context, reviews, now=now, trusted_history=history)
            new_ids = {review.assessment.get('assessment_id') for review in reviews}
            invalid = [entry for entry in result['invalid_assessments'] if entry['assessment_id'] in new_ids]
            if invalid:
                raise AssessmentRejected(invalid)
            old_ids = {review.assessment['assessment_id'] for review in history}
            # Repeating an event in a different command is not a new review act.
            unique = {_canonical(event): event for event in events if event['assessment']['assessment_id'] not in old_ids}
            if not unique:
                raise JournalConflict('batch contains no new assessment event; replay the original command')
            events = list(unique.values())
            # Bind precisely what is committed, while retaining the submitted
            # request for exact idempotency (including harmless duplicate events).
            batch = {'schema_version': 'tos_assessment_batch_v1', 'subject_id': context.record.id,
                     'sequence': len(chain) + 1, 'previous_revision': current, 'recorded_at': now,
                     'request': request, 'request_digest': request_digest, 'events': events,
                     'qualification': {'all_new_events_qualified': True, 'policy': engine.policy.ref,
                                       'is_semantic_evaluation': False},
                     'admission_at_commit': result}
            self.validator.validate(batch)
            payload = _canonical(batch)
            if len(payload) > MAX_RECORD_BYTES:
                raise ValueError('batch exceeds bounded journal size')
            revision = hashlib.sha256(payload).hexdigest()
            self._write_blob(home, revision, payload)
            if snapshot_guard is not None:
                snapshot_guard()
            self._publish_head(home, revision)
            return {'revision': revision, 'receipt': batch, 'replayed': False, 'current_admission': result}


def _owned_path(path: Path, *, directory: bool = False) -> int:
    """Open a local owner path without following links, including ancestors.

    The owner UID and root are trusted. Other local users, request contents,
    and symlink targets are not. A root-owned sticky ancestor (e.g. /tmp) is
    allowed, but never as the final owner directory. Same-UID hostile code is
    outside this Unix account boundary; this is not a setuid service.
    """
    uid = os.getuid()
    if os.geteuid() != uid or not path.is_absolute() or '..' in path.parts:
        raise PermissionError('an absolute owner path and non-setuid process are required')
    descriptor = os.open('/', os.O_RDONLY | os.O_DIRECTORY)
    try:
        for index, part in enumerate(path.parts[1:]):
            final = index == len(path.parts) - 2
            flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK
            if not final or directory:
                flags |= os.O_DIRECTORY
            child = os.open(part, flags, dir_fd=descriptor)
            os.close(descriptor)
            descriptor = child
            info = os.fstat(descriptor)
            sticky_ancestor = (not final and stat.S_ISDIR(info.st_mode)
                               and info.st_uid == 0 and info.st_mode & stat.S_ISVTX)
            if (info.st_uid not in (0, uid)
                    or (info.st_mode & 0o022 and not sticky_ancestor)
                    or (final and not directory and not stat.S_ISREG(info.st_mode))):
                raise PermissionError('owner path is not protected from other local users')
        if directory and path == Path('/'):
            raise PermissionError('a dedicated owner directory is required')
        return descriptor
    except BaseException:
        os.close(descriptor)
        raise


def _json_object(encoded: bytes) -> dict[str, Any]:
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError('duplicate JSON key')
            result[key] = value
        return result
    payload = json.loads(encoded, object_pairs_hook=pairs)
    if not isinstance(payload, dict):
        raise ValueError('a JSON object is required')
    _canonical(payload)  # Refuse nonfinite numbers, including JSON decoder extensions.
    return payload


def _keys(payload: Any, expected: set[str]) -> None:
    if not isinstance(payload, dict) or set(payload) != expected:
        raise ValueError('command/configuration fields do not match the declared contract')


def _source_records(root: Path, bindings: Any, *, form_sets: dict | None = None,
                    identity_snapshots: dict | None = None,
                    claim_dependencies: dict | None = None) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
    """Resolve bounded explicit public inputs, without selecting corpus neighbors.

    Record identity selects JSONL entries and current forms, not line numbers
    or labels. Unknown fields stay in the exact payload; only the declared
    identity envelope is interpreted. Declared metadata profiles additionally
    reserve native semantic IDs through a bounded metadata-only inventory;
    its opaque closure hash is separate from exposed public source fixity.
    This does not replace source validators or read payload/local-content.
    """
    if not isinstance(bindings, list) or len(bindings) > MAX_ASSESSMENTS:
        raise ValueError('source bindings must be a bounded list')
    os.close(_owned_path(root, directory=True))
    scripts = str(Path(__file__).resolve().parents[5] / 'scripts')
    if scripts not in sys.path:
        sys.path.insert(0, scripts)
    from source_record_profiles import (SourceRecordProfiles, SourceClaimProfiles, SOURCE_CLAIM_BASENAME,
                                        RESERVED_BASENAMES, _read_json, CORPUS_REF)
    families = {'tos_corpus_record_v1': ('record_id', 'record_version'),
                'tos_historical_record_v1': ('record_id', 'record_version'),
                'tos_claim_packet_v1': ('claim_id', 'claim_version'),
                'tos_historical_claim_v1': ('claim_id', 'claim_version'),
                'tos_human_form_v1': ('form_id', 'form_version')}
    files, resolved, fixity, total, selected = {}, [], [], 0, set()
    metadata_profiles, claim_profiles = None, None
    native_dependencies = {}
    native_types = {}
    declared_claims = set()

    def declared_family(row, path):
        nonlocal metadata_profiles, claim_profiles
        # Owner-declared readers only. Unknown neighbors remain opaque, and
        # the selected record still fails if no exact schema route exists.
        if path.name in {'artifact-witness.json', 'composite-witness.json'}:
            from build_source_witness_catalog import native_witness_contract
            schema_ref, identity, kind = native_witness_contract(row, path.as_posix())
            schema = _read_json(root, schema_ref, native_dependencies)
            if not Draft202012Validator(schema, format_checker=FormatChecker()).is_valid(row):
                raise ValueError('native assessment source violates its exact public metadata schema')
            native_types[row[identity]] = kind
            return identity, 'record_version'
        if path.name == SOURCE_CLAIM_BASENAME:
            if claim_profiles is None:
                claim_profiles = SourceClaimProfiles(root)
            key = row.get('predicate'), row.get('schema_version')
            if not all(isinstance(value, str) for value in key) or key not in claim_profiles.schema_routes:
                return None
            claim_profiles.validate(row)
            declared_claims.add(row['claim_id'])
            return 'claim_id', 'claim_version'
        kind = row.get('record_type')
        if isinstance(kind, str) and path.name == kind + '.json':
            if metadata_profiles is None:
                metadata_profiles = SourceRecordProfiles(root)
            if kind not in metadata_profiles.profiles:
                return None
            metadata_profiles.validate_path(kind, path.as_posix())
            metadata_profiles.validate(kind, row)
            return 'record_id', 'record_version'
        return None

    for binding in bindings:
        _keys(binding, {'path', 'record_id', 'origin_id'})
        identifier = binding['record_id']
        if not isinstance(identifier, str) or not identifier or identifier in selected:
            raise ValueError('source bindings must select distinct record identities')
        selected.add(identifier)
        relative = binding['path']
        if not isinstance(relative, str):
            raise ValueError('source path must be a repository-relative string')
        path = Path(relative)
        if (path.is_absolute() or path.as_posix() != relative or '..' in path.parts
                or path.parts[:2] != ('ToS', 'source-witnesses')
                or path.is_relative_to('ToS/source-witnesses/owner-local')
                or any(part in ('payload', 'local-content', 'catalog') for part in path.parts)
                or path.suffix not in ('.json', '.jsonl')):
            raise PermissionError('source binding must name an explicit source-witness metadata file')
        if relative not in files:
            descriptor = _owned_path(root / path)
            with os.fdopen(descriptor, 'rb') as stream:
                before = os.fstat(stream.fileno())
                raw = stream.read(8 * MAX_RECORD_BYTES - total + 1)
                after = os.fstat(stream.fileno())
            if (before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (after.st_size, after.st_mtime_ns, after.st_ctime_ns):
                raise JournalConflict('source file changed during read')
            total += len(raw)
            if total > 8 * MAX_RECORD_BYTES:
                raise ValueError('source files exceed the shared 8 MiB read budget')
            if path.suffix == '.jsonl':
                rows = []
                for line in io.BytesIO(raw):
                    if not line.strip():
                        continue
                    if len(rows) == MAX_ASSESSMENTS:
                        raise ValueError('source file exceeds bounded record selection')
                    rows.append(_json_object(line))
            else:
                payload = _json_object(raw)
                rows = payload.get('forms') if payload.get('schema_version') == 'tos_human_form_set_v1' else [payload]
                if form_sets is not None and payload.get('schema_version') == 'tos_human_form_set_v1':
                    form_sets[relative] = payload
            if not isinstance(rows, list) or len(rows) > MAX_ASSESSMENTS:
                raise ValueError('source file exceeds bounded record selection')
            indexed = {}
            for row in rows:
                if not isinstance(row, dict):
                    raise ValueError('source records must be JSON objects')
                family = families.get(row.get('schema_version'))
                if (family is None or path.name in {SOURCE_CLAIM_BASENAME, 'composite.json', 'composite-witness.json', 'artifact-witness.json'}
                        or (isinstance(row.get('record_type'), str) and path.suffix == '.json'
                            and path.name not in RESERVED_BASENAMES)):
                    family = declared_family(row, path)
                if family is None:
                    continue  # Opaque neighboring records are neither dropped from source nor interpreted.
                identity, version = family
                item = Record.from_payload(row[identity], row[version], row)
                if item.id in indexed:
                    raise ValueError('source file repeats a current record identity')
                indexed[item.id] = item
            files[relative] = indexed
            fixity.append({'path': relative, 'digest': 'sha256:' + hashlib.sha256(raw).hexdigest()})
        if identifier not in files[relative]:
            raise ValueError('source record is missing or has an unsupported identity family')
        record = files[relative][identifier]
        payload = record.payload
        if (payload['schema_version'] in {'tos_claim_packet_v1', 'tos_historical_claim_v1', 'tos_historical_record_v1'}
                and payload.get('visibility') not in ('public', 'public_metadata_only')):
            raise PermissionError('nonpublic records need a separately authorized source adapter')
        origin = binding['origin_id']
        Record.from_payload(record.id, record.version, payload, origin_id=origin)
        resolved.append({'id': record.id, 'version': record.version, 'payload': payload, 'origin_id': origin})
    selected_records = {item['id']: Record.from_payload(**item) for item in resolved}
    declared_dependencies = {}
    if claim_profiles is not None:
        # Endpoints must be in this independently selected source snapshot,
        # not inline shadows or discovered by crawling the surrounding corpus.
        objects = {item['id']: item['payload'] for item in resolved if 'record_type' in item['payload']}
        # Typed endpoint descriptors serve domain/range validation only. The
        # selected record above retains every original native field unchanged.
        objects.update({identifier: {'record_type': kind} for identifier, kind in native_types.items()})
        paths = {binding['record_id']: Path(binding['path']) for binding in bindings}
        endpoint_ids = {identity for item in resolved if item['id'] in declared_claims
                        for identity in claim_profiles.identity_refs(item['payload'])}
        native_validator = None
        for identifier in endpoint_ids & objects.keys():
            if identifier in native_types:
                continue  # Its exact schema and canonical owner path were checked above.
            body = objects[identifier]
            kind = body.get('record_type')
            if (not isinstance(kind, str) or paths[identifier].name != kind + '.json'
                    or not identifier.startswith('tos.' + kind + '.')):
                raise ValueError('claim endpoint identity and source basename disagree')
            if body.get('schema_version') == 'tos_corpus_record_v1':
                if native_validator is None:
                    schema = _read_json(root, CORPUS_REF, native_dependencies)
                    native_validator = Draft202012Validator(schema, format_checker=FormatChecker())
                if not native_validator.is_valid(body):
                    raise ValueError('claim endpoint violates the native corpus source schema')
            else:
                if metadata_profiles is None:
                    metadata_profiles = SourceRecordProfiles(root)
                if kind not in metadata_profiles.profiles:
                    raise ValueError('claim endpoint has no declared source metadata profile')
                metadata_profiles.validate(kind, body)
        for item in resolved:
            if item['id'] in declared_claims:
                claim_profiles.validate(item['payload'], objects)
                declared_dependencies[item['id']] = [selected_records[identifier].ref
                    for identifier in sorted(claim_profiles.identity_refs(item['payload']))]
    source_claim_ids = frozenset(declared_dependencies)
    for item in resolved:
        body = item['payload']
        if body.get('schema_version') != 'tos_human_form_v1':
            continue
        subject_ref = body.get('subject')
        identifier = subject_ref.get('id') if isinstance(subject_ref, dict) else None
        if isinstance(identifier, str) and identifier.startswith('tos.claim.'):
            # This guard must also run when NO Claim was source-selected:
            # an inline shadow or an unsupported family cannot supply closure.
            if identifier not in source_claim_ids:
                raise ValueError('source Claim form requires its source-selected declared Claim')
            if _canonical(selected_records[identifier].ref) != _canonical(subject_ref):
                raise JournalConflict('source Claim form binds a different current subject')
            declared_dependencies[item['id']] = [subject_ref, *declared_dependencies[identifier]]
    if claim_dependencies is not None:
        claim_dependencies.update(declared_dependencies)
    dependencies = dict(native_dependencies)
    for profiles in (metadata_profiles, claim_profiles):
        if profiles is not None:
            for ref, digest in profiles.input_digests.items():
                if ref in dependencies and dependencies[ref] != digest:
                    raise JournalConflict('source profile dependency changed during resolution')
                dependencies[ref] = digest
    for ref, digest in sorted(dependencies.items()):
        with os.fdopen(_owned_path(root / ref), 'rb') as stream:
            raw = stream.read(8 * MAX_RECORD_BYTES - total + 1)
        total += len(raw)
        if total > 8 * MAX_RECORD_BYTES:
            raise ValueError('source and profile files exceed the shared 8 MiB read budget')
        if hashlib.sha256(raw).hexdigest() != digest:
            raise JournalConflict('source profile dependency changed during resolution')
        fixity.append({'path': ref, 'digest': 'sha256:' + digest})
    if metadata_profiles is not None:
        def protected_identity_read(path, limit):
            nonlocal total
            with os.fdopen(_owned_path(path), 'rb') as stream:
                before = os.fstat(stream.fileno())
                raw = stream.read(min(limit, 8 * MAX_RECORD_BYTES - total) + 1)
                after = os.fstat(stream.fileno())
            total += len(raw)
            if total > 8 * MAX_RECORD_BYTES or len(raw) > limit:
                raise ValueError('source and identity files exceed the shared 8 MiB read budget')
            if (before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (after.st_size, after.st_mtime_ns, after.st_ctime_ns):
                raise JournalConflict('native identity file changed during read')
            return raw

        snapshot = metadata_profiles.native_identity_snapshot(read_bytes=protected_identity_read, only_if_used=True)
        if identity_snapshots is not None and snapshot is not None:
            identity_snapshots['native_semantic_identity_snapshot'] = snapshot
        text_snapshot = metadata_profiles.native_text_snapshot(read_bytes=protected_identity_read)
        if identity_snapshots is not None and text_snapshot is not None:
            identity_snapshots['native_text_binding_snapshot'] = text_snapshot
    return resolved, fixity


def _public_claim_required_sources(identifier, dependencies, sourced, native_summaries):
    """Exact declared Claim grounding, including its native endpoint returns.

    Only owner-selected, source-resolved records participate. Unknown value
    fields and unrelated selected neighbors never become inferred references.
    A missing native read is separately denied by source_read_ready; it cannot
    be supplied as an inline record or as a claim in assessment prose.
    """
    records = {row['id']: Record.from_payload(**row) for row in sourced}
    selected = {}

    def add(ref):
        record = records.get(ref['id'])
        if record is None or _canonical(record.ref) != _canonical(ref) or record.id == identifier:
            raise JournalConflict('public Claim grounding differs from its selected source snapshot')
        selected[record.id] = record

    for ref in dependencies.get(identifier, ()):
        add(ref)
    for record in list(selected.values()):
        binding = record.payload.get('native_text_binding')
        if not isinstance(binding, dict):
            continue
        for summary in native_summaries:
            refs = ([summary['record'], summary['evidence_record']] if 'record' in summary
                    else summary.get('record_refs', ()))
            if any((unit := records.get(ref['id'])) is not None and _canonical(unit.ref) == _canonical(ref)
                    and _canonical(unit.payload.get('native_binding')) == _canonical(binding) for ref in refs):
                for ref in refs:
                    add(ref)
    return tuple(selected[key] for key in sorted(selected))


def _materialize_source_form(config, sourced, form_sets, engine, context, history, *, now, contract_root,
                             owner_local_paths=None, owner_local_form_validator=None,
                             materializer_validators=None, subject_assessment=None):
    """Render the selected form from this exact source/journal snapshot only.

    This owner lane requires whole-subject context for assessed source forms. It
    does not make submitted bindings the authority for dropping qualifications.
    Source-only readiness and admitted templates retain their separate contracts.
    """
    from human_forms import FormScope, SourceBinding, materialize_form
    from source_witness_human_forms import (_validator, claim_forms_path, claim_field_catalog,
                                           metadata_field_catalog, source_copy_field)
    form = context.record
    body = form.payload
    selected = {row['id'] for row in sourced}
    if (form.id not in selected or body.get('schema_version') != 'tos_human_form_v1'
            or not isinstance(body.get('content'), dict)
            or body['content'].get('kind') not in ('source-copy', 'freeform')):
        raise PermissionError('assessed materialization requires an explicitly selected source-copy or freeform source form')
    subject_ref = body.get('subject')
    subject = engine.records.get(subject_ref.get('id')) if isinstance(subject_ref, dict) else None
    if subject is None or subject.id not in selected or subject.id == form.id:
        raise PermissionError('form subject must be explicitly selected from source')
    if subject.ref != subject_ref:
        raise JournalConflict('form binds a different subject snapshot')
    paths = {binding['record_id']: Path(binding['path']) for binding in config['source_records']}
    paths.update({identity: Path(path) for identity, path in (owner_local_paths or {}).items()})
    if subject.id not in paths or form.id not in paths:
        raise PermissionError('native evidence needs its own explicit form adapter')
    source_path, form_path = paths[subject.id], paths[form.id]
    expected = (claim_forms_path(source_path, subject.id) if source_path.name == 'source-claims.jsonl'
                else source_path.with_name(source_path.stem + '.human-forms.json'))
    package = form_sets.get(form_path.as_posix())
    validator = (owner_local_form_validator if owner_local_form_validator is not None else _validator())
    if form_path != expected or package is None or validator is None or not validator.is_valid(package):
        raise ValueError('form must belong to its validated adjacent source set')
    if package['subject'] != subject.ref:
        raise JournalConflict('form set binds a different subject snapshot')
    if sum(row == body for row in package['forms']) != 1:
        raise ValueError('current form does not resolve uniquely in its source set')
    for binding in body['bindings'].values():
        if binding['record']['id'] not in selected:
            raise PermissionError('form bindings require explicit source-selected records')
    required_context, source_languages = [SourceBinding(subject, '')], ()
    if body['content']['kind'] == 'source-copy':
        catalog = (claim_field_catalog(subject.payload) if source_path.name == 'source-claims.jsonl'
                   else metadata_field_catalog(subject.payload))
        field = source_copy_field(subject, body, catalog)
        if field is None:
            raise PermissionError('assessed source-copy requires an exact source-owned field and role')
        required_context.extend(SourceBinding(subject, pointer) for pointer in field['context'] if pointer)
        source_languages = ((SourceBinding(subject, field['pointer']), field['language'], field['script']),)
    language = config['subjects'][form.id].get('form_language_context')
    language_binding = None
    if language is not None:
        _keys(language, {'record', 'pointer'})
        record_ref = language['record']
        record = engine.records.get(record_ref.get('id')) if isinstance(record_ref, dict) else None
        if (record is None or record.id not in selected or record.id == form.id
                or record.ref != record_ref or not isinstance(language['pointer'], str)
                or (language['pointer'] and not language['pointer'].startswith('/'))):
            raise PermissionError('linguistic context must bind an exact selected source field')
        language_binding = SourceBinding(record, language['pointer'])
    prior = [Record.from_payload(row['form_id'], row['form_version'], row) for row in package['prior_forms']]
    scope = FormScope(subject, tuple(required_context), context.maker_id, context.risk,
                      context.languages, context.requested_use, access_allowed=context.access_allowed,
                      language_context=language_binding, required_sources=context.required_sources,
                      required_admissions=context.required_admissions, source_languages=source_languages,
                      require_current_assessment=True, subject_assessment=subject_assessment)
    return materialize_form(contract_root, form, scope,
                            [engine.records[identity] for identity in selected], prior_forms=prior,
                            engine=engine, trusted_history=history, now=now,
                            **({'validators': materializer_validators} if materializer_validators is not None else {}))


def _form_parent_claim_context(form_context, subjects, records, sourced, private_sources,
                               public_claim_dependencies, native_summaries):
    """Resolve a v5 form's parent Claim scope, never infer its admission.

    Raw Claim posture and current journal status are distinct. Reading that
    status requires its own explicit same-use owner scope and exact grounding.
    An unreviewed parent is still displayable as such, not silently endorsed.
    """
    ref = form_context.record.payload.get('subject')
    parent = records.get(ref.get('id')) if isinstance(ref, dict) else None
    if parent is None or parent.ref != ref:
        raise JournalConflict('form parent is not its exact selected source')
    body = parent.payload
    if 'claim_id' not in body or 'claim_version' not in body:
        return None
    scope = subjects.get(parent.id)
    if scope is None:
        raise PermissionError('v5 Claim form needs an explicit parent assessment read scope')
    _keys(scope, {'record', 'assertion_layer', 'risk', 'languages', 'maker_id', 'requested_use', 'access_allowed'})
    if scope['record'] != parent.ref:
        raise JournalConflict('parent Claim assessment scope is stale')
    if (scope['access_allowed'] is not True or scope['requested_use'] != form_context.requested_use
            or scope['assertion_layer'] != body['assertion_layer']
            or scope['maker_id'] != body.get('maker', {}).get('agent_ref')):
        raise PermissionError('parent Claim scope disagrees with its source, use or access')
    if (not isinstance(scope['languages'], list) or not scope['languages']
            or any(not isinstance(language, str) or not language for language in scope['languages'])
            or any(not isinstance(scope[key], str) or not scope[key]
                   for key in ('assertion_layer', 'risk', 'maker_id', 'requested_use'))):
        raise ValueError('parent Claim assessment scope is incomplete')
    if parent.id in public_claim_dependencies:
        sources = _public_claim_required_sources(parent.id, public_claim_dependencies, sourced, native_summaries)
    elif private_sources is not None and parent.id in private_sources.claim_dependencies:
        sources = private_sources.required_sources(parent.id)
    else:
        raise PermissionError('form parent is not an explicitly source-selected Claim')
    if not {_canonical(record.ref) for record in (parent, *sources)} <= {
            _canonical(record.ref) for record in form_context.required_sources}:
        raise PermissionError('parent Claim grounding is outside the selected form closure')
    if private_sources is not None:
        languages = private_sources.required_languages(parent.id, sourced)
        dependency_ids = {record.id for record in sources}
        languages.update(row['language'].casefold() for row in native_summaries
            if row['unit_id'] in dependency_ids and isinstance(row.get('language'), str) and row['language'])
        if not languages <= {language.casefold() for language in scope['languages']}:
            raise PermissionError('parent Claim assessment scope omits source languages')
    return SubjectContext(parent, scope['assertion_layer'], scope['risk'], tuple(scope['languages']),
        scope['maker_id'], scope['requested_use'], access_allowed=True,
        source_read_ready=form_context.source_read_ready, required_sources=sources)


def _validate_native_selections(config):
    selections, subjects = config['native_text_units'], config['subjects']
    if not isinstance(selections, list) or len(selections) > 64 or not isinstance(subjects, dict):
        raise ValueError('native assessment selection exceeds its bounded contract')
    seen = set()
    # Validate every private-read authorization before opening any native text.
    for selection in selections:
        _keys(selection, {'binding', 'origin_id', 'read_scope'})
        binding = selection['binding']
        if not isinstance(binding, dict) or not isinstance(binding.get('unit_id'), str):
            raise ValueError('native assessment binding lacks a unit identity')
        identifier = binding['unit_id']
        if identifier in seen:
            raise ValueError('native assessment selection repeats a unit identity')
        seen.add(identifier)
        if (not isinstance(selection['read_scope'], str)
                or selection['read_scope'] not in {'metadata_only', 'exact_public', 'exact_owner_local'}):
            raise ValueError('native assessment read scope is unknown')
        scope = subjects.get(identifier)
        if not isinstance(scope, dict) or scope.get('access_allowed') is not True:
            raise PermissionError('native unit is outside the protected owner access scope')
        _keys(scope, {'record', 'assertion_layer', 'risk', 'languages', 'maker_id',
                      'requested_use', 'access_allowed'})


def _native_text_records(config, *, owner_context=None):
    """Explicit owner-local selection, not a corpus crawl or public reader."""
    scripts = str(Path(__file__).resolve().parents[5] / 'scripts')
    if scripts not in sys.path:
        sys.path.insert(0, scripts)
    from native_text_binding import NativeTextBindingResolver
    _validate_native_selections(config)
    selections, subjects = config['native_text_units'], config['subjects']
    observed, total = {}, 0

    def protected_read(path, limit):
        nonlocal total
        if path not in observed and len(observed) >= 128:
            raise ValueError('native assessment exceeds shared dependency budgets')
        read_limit = min(limit, 16 * MAX_RECORD_BYTES - total) if path not in observed else limit
        with os.fdopen(_owned_path(path), 'rb') as stream:
            before = os.fstat(stream.fileno())
            raw = stream.read(read_limit + 1)
            after = os.fstat(stream.fileno())
        if len(raw) > read_limit:
            raise ValueError('native assessment exceeds shared dependency budgets')
        if (before.st_size, before.st_mtime_ns, before.st_ctime_ns) != (after.st_size, after.st_mtime_ns, after.st_ctime_ns):
            raise JournalConflict('native assessment dependency changed during read')
        digest = hashlib.sha256(raw).hexdigest()
        if path in observed and observed[path] != digest:
            raise JournalConflict('native assessment dependency changed during command')
        if path not in observed:
            total += len(raw)
            if total > 16 * MAX_RECORD_BYTES:
                raise ValueError('native assessment exceeds shared dependency budgets')
            observed[path] = digest
        return raw

    records, summaries, resolvers, contracts = {}, [], [], {}
    for selection in selections:
        resolver = NativeTextBindingResolver(Path(config['source_root']), read_bytes=protected_read,
                                            **({'owner_context': owner_context} if owner_context is not None else {}))
        adapted = resolver.assessment_records(selection['binding'], origin_id=selection['origin_id'],
            verify_content=selection['read_scope'] != 'metadata_only',
            allow_private_content=selection['read_scope'] == 'exact_owner_local')
        unit, layer = adapted['records']
        for row in (unit, layer):
            if row['id'] in records and records[row['id']] != row:
                raise ValueError('native assessment repeats an identity with different source or origin')
            records[row['id']] = row
        packet = unit['payload']['packet']
        segment = next(row for row in packet['segmentations']
                       if row['segmentation_id'] == selection['binding']['segmentation_id'])
        scope = subjects[unit['id']]
        if (scope.get('assertion_layer') not in {'textual_observation', 'linguistic_analysis'}
                or scope.get('maker_id') != segment['maker']['agent_ref']
                or scope.get('languages') != [adapted['summary']['language']]):
            raise PermissionError('native assessment scope disagrees with unit layer, language or maker')
        summaries.append({'record': Record.from_payload(**unit).ref,
                          'evidence_record': Record.from_payload(**layer).ref,
                          'read_scope': selection['read_scope'], **adapted['summary']})
        resolvers.append(resolver)
        contracts.update(resolver.schema_digests)
    return list(records.values()), summaries, resolvers, contracts


QUALITY_USES = frozenset({'text-layer:citation', 'text-layer:linguistic-analysis',
                        'text-layer:semantic-analysis', 'text-layer:search-projection'})


def _validate_quality_dependencies(dependencies, subjects):
    """Protected explicit use bindings; no request or source prose supplies them."""
    if (not isinstance(dependencies, dict) or len(dependencies) > MAX_ASSESSMENTS
            or not isinstance(subjects, dict) or not set(dependencies).issubset(subjects)):
        raise ValueError('quality dependencies must select bounded configured subjects')
    for entries in dependencies.values():
        if not isinstance(entries, list) or len(entries) > 8:
            raise ValueError('one subject may require at most eight selected text layers')
        seen = set()
        for entry in entries:
            _keys(entry, {'layer_id', 'use'})
            if (not isinstance(entry['layer_id'], str) or not entry['layer_id'].strip()
                    or entry['layer_id'] in seen or not isinstance(entry['use'], str)
                    or entry['use'] not in QUALITY_USES):
                raise ValueError('quality dependency repeats a layer or names an unsupported use')
            seen.add(entry['layer_id'])


def _quality_requirements(current, required_sources, records, configured, layers, assertion_layer):
    """Derive native coverage from actual selected sources, then check grants.

    The protected mapping chooses the purpose but cannot omit a native source,
    substitute an unrelated layer or promote search-only quality to semantics.
    No graph/corpus scan, guessed relationship or assessment-text parsing.
    """
    needed = set()
    pending = [current, *required_sources]
    seen = set()
    while pending:
        record = pending.pop()
        if record.id in seen:
            continue
        seen.add(record.id)
        if len(seen) > MAX_ASSESSMENTS:
            raise ValueError('quality grounding closure exceeds its bounded source budget')
        body = record.payload
        binding = body.get('native_text_binding', body.get('native_binding'))
        if binding is not None:
            if not isinstance(binding, dict) or not isinstance(binding.get('text_layer'), dict):
                raise ValueError('quality grounding has an incomplete native binding')
            identity = binding['text_layer'].get('layer_id')
            selected = layers.get(identity)
            if (selected is None or selected['binding']['text_layer'] != binding['text_layer']
                    or selected['binding']['source_record_refs'] != binding.get('source_record_refs')):
                raise PermissionError('quality grounding lacks the same exact selected layer binding')
            needed.add(identity)
        if body.get('schema_version') == 'tos_source_text_layer_v1' and record.id != current.id:
            if record.id not in layers or layers[record.id]['record'].ref != record.ref:
                raise PermissionError('quality grounding lacks its current exact layer selection')
            needed.add(record.id)
        if body.get('schema_version') == 'tos_human_form_v1':
            refs = [body['subject'], *(value['record'] for value in body['bindings'].values())]
            for ref in refs:
                if ref['id'].startswith('tos.quality-basis.sha256.'):
                    # This is checked against the freshly derived basis after
                    # dependency journals are locked. It cannot supply native
                    # grounding or replace an exact source binding here.
                    continue
                source = records.get(ref['id'])
                if source is None or source.ref != ref:
                    raise JournalConflict('quality-bound form references another source snapshot')
                pending.append(source)
    entries = configured.get(current.id, [])
    if {entry['layer_id'] for entry in entries} != needed or current.id in needed:
        raise PermissionError('quality requirements omit, add or cycle a native grounding dependency')
    effective_layer = assertion_layer
    if assertion_layer == 'human_projection':
        parent = records.get(current.payload.get('subject', {}).get('id'))
        if parent is not None:
            effective_layer = parent.payload.get('assertion_layer') or (
                'textual_observation' if parent.payload.get('schema_version') == 'tos_native_text_unit_assessment_subject_v1'
                else 'semantic_interpretation')
    required_use = ('text-layer:linguistic-analysis' if effective_layer in {
        'linguistic_analysis', 'translation_alignment', 'translation_judgment'} else
        'text-layer:citation' if effective_layer in {'textual_observation', 'forensic_observation',
                                                    'bibliographic_assertion', 'scholarly_report'} else
        'text-layer:semantic-analysis')
    for entry in entries:
        if entry['use'] != required_use:
            raise PermissionError('native quality purpose does not cover this dependent assertion layer')
    return sorted(entries, key=lambda entry: entry['layer_id'])


def _quality_basis(layer, admission, use, validator):
    identity = 'tos.quality-basis.sha256.' + _digest({
        'layer_id': layer['record'].id, 'use': use, 'scope': layer['scope']})
    body = {'schema_version': 'tos_native_text_layer_quality_basis_v1',
        'basis_id': identity, 'basis_version': 1, 'layer': layer['record'].ref,
        'comparison': layer['comparison'].ref if layer['comparison'] is not None else None,
        'use': use, 'scope': layer['scope'], 'policy': admission['policy'],
        'assessment_refs': admission['assessment_refs'], 'status': admission['status'],
        'can_use': admission['can_use'] is True and layer['read_ready'] is True,
        'limits': admission['limits'], 'visibility': 'local_only',
        'publication_authorized': False, 'performs_semantic_assessment': False}
    validator.validate(body)
    return Record.from_payload(identity, 1, body, origin_id=layer['record'].origin_id)


PUBLIC_SOURCE_OWNER_VERSIONS = frozenset({'tos_local_assessment_owner_v1',
    'tos_local_assessment_owner_v2', 'tos_local_assessment_owner_v3'})


def run_public_source_command(owner_config: Path, request: dict[str, Any], *,
                              contract_root: Path | None = None) -> dict[str, Any]:
    """Keep the existing graph adapter out of confidential v4 source inputs."""
    return run_local_command(owner_config, request, contract_root=contract_root,
                             accepted_owner_versions=PUBLIC_SOURCE_OWNER_VERSIONS)


def run_local_command(owner_config: Path, request: dict[str, Any], *,
                      contract_root: Path | None = None,
                      accepted_owner_versions: frozenset[str] | None = None) -> dict[str, Any]:
    """Apply one command as the configured local account, never a claimed UID.

    The operator selects the owner configuration independently of the request.
    Its issuer must have checked policy, grants, calibration, exact source
    snapshot and execution provenance. Unix ownership authenticates the local
    account only; it does not attest that a particular model produced the prose.
    """
    descriptor = _owned_path(owner_config)
    with os.fdopen(descriptor, 'rb') as stream:
        encoded = stream.read(8 * MAX_RECORD_BYTES + 1)
    if len(encoded) > 8 * MAX_RECORD_BYTES:
        raise ValueError('owner configuration exceeds the 8 MiB snapshot budget')
    config = _json_object(encoded)
    if accepted_owner_versions is not None and config.get('schema_version') not in accepted_owner_versions:
        raise PermissionError('assessment consumer does not accept this source-owner version')
    fields = {'schema_version', 'uid', 'principal_id', 'execution_profile',
              'policy', 'authorities', 'competencies', 'records', 'subjects', 'journal_directory'}
    layer_quality = config.get('schema_version') == 'tos_local_assessment_owner_v5'
    owner_local = layer_quality or config.get('schema_version') == 'tos_local_assessment_owner_v4'
    native_bound = owner_local or config.get('schema_version') == 'tos_local_assessment_owner_v3'
    source_bound = native_bound or config.get('schema_version') == 'tos_local_assessment_owner_v2'
    if source_bound:
        fields |= {'source_context_ref' if owner_local else 'source_root', 'source_records'}
    if owner_local:
        fields.add('owner_local_source_records')
        if 'owner_local_source_claims' in config:
            fields.add('owner_local_source_claims')
    if native_bound:
        fields.add('native_text_units')
    if layer_quality:
        fields |= {'native_text_layers', 'quality_dependencies'}
    _keys(config, fields)
    if (config['schema_version'] not in PUBLIC_SOURCE_OWNER_VERSIONS | {'tos_local_assessment_owner_v4', 'tos_local_assessment_owner_v5'}
            or type(config['uid']) is not int or config['uid'] != os.getuid()
            or not isinstance(config['principal_id'], str) or not config['principal_id'].strip()):
        raise PermissionError('configuration does not bind this local account')
    snapshot = 'sha256:' + _digest(config)
    owner_context, private_sources, private_snapshot = None, None, None
    layer_sources, quality_validator = None, None
    if owner_local:
        scripts = str(Path(__file__).resolve().parents[5] / 'scripts')
        if scripts not in sys.path:
            sys.path.insert(0, scripts)
        from source_owner_context import OwnerLocalSourceContext, _read
        from owner_local_assessment_sources import OwnerLocalAssessmentSources, source_access, confidential_journal
        if _read(owner_config, 8 * MAX_RECORD_BYTES, confidential_file=True) != encoded:
            raise JournalConflict('confidential assessment configuration changed during selection')
        selections = config['native_text_units']
        if not isinstance(selections, list) or len(selections) > 64:
            raise ValueError('private native assessment selection exceeds its bounded contract')
        native_selections = []
        for selection in selections:
            _keys(selection, {'binding', 'origin_id', 'source_access'})
            scope = source_access(selection['source_access'])
            native_selections.append({'binding': selection['binding'], 'origin_id': selection['origin_id'], 'read_scope': scope})
        _validate_native_selections({**config, 'native_text_units': native_selections})
        if layer_quality:
            from native_text_layer_assessment import NativeLayerAssessmentSources, preflight_layer_selections
            preflight_layer_selections(config['native_text_layers'], config['subjects'])
            _validate_quality_dependencies(config['quality_dependencies'], config['subjects'])
        owner_context = OwnerLocalSourceContext.load(config['source_context_ref'])
        confidential_journal(owner_context, config['journal_directory'])
        # Runtime-only normalization; exact retained config still owns the hash.
        source_root = owner_context.public_root
        native_config = {**config, 'source_root': str(source_root), 'native_text_units': native_selections}
        private_sources = OwnerLocalAssessmentSources(owner_context, config['owner_local_source_records'],
            config.get('owner_local_source_claims', ()))
        if layer_quality:
            quality_validator = private_sources.load_quality_grammar()
            layer_sources = NativeLayerAssessmentSources(owner_context, config['native_text_layers'], config['subjects'])
        private_snapshot = private_sources.snapshot()
    else:
        source_root = Path(config['source_root']) if source_bound else None
        native_config = config
    source_publication = None
    if source_bound:
        scripts = str(Path(__file__).resolve().parents[5] / 'scripts')
        if scripts not in sys.path:
            sys.path.insert(0, scripts)
        from source_metadata_snapshot import PublicationSnapshot
        source_publication = PublicationSnapshot(source_root)
    sourced, regular_sourced, form_sets, identity_snapshots = [], [], {}, {}
    public_claim_dependencies = {}
    native_summaries, native_resolvers, native_contracts, native_records = [], [], {}, []
    explicit_native_targets = set()
    if source_bound:
        sourced, fixity = _source_records(source_root, config['source_records'],
                                         form_sets=form_sets, identity_snapshots=identity_snapshots,
                                         claim_dependencies=public_claim_dependencies)
        regular_sourced = list(sourced)
        if private_sources is not None:
            selected_public = {item['id']: item for item in sourced}
            for item in private_sources.records:
                if item['id'] in selected_public:
                    if (item['id'] in private_sources.explicit_ids
                            or _canonical(item) != _canonical(selected_public[item['id']])):
                        raise ValueError('private source records cannot shadow public selected identities')
                else:
                    sourced.append(item)
            form_sets.update(private_sources.form_sets)
        if native_bound:
            native_records, native_summaries, native_resolvers, native_contracts = _native_text_records(
                native_config, owner_context=owner_context)
            explicit_native_targets = {row['unit_id'] for row in native_summaries}
            source_index = {item['id']: item for item in sourced}
            for item in native_records:
                if item['id'] in source_index:
                    if (private_sources is None or item['id'] in private_sources.explicit_ids
                            or item['id'] not in {row['id'] for row in private_sources.native_records}
                            or _canonical(item) != _canonical(source_index[item['id']])):
                        raise ValueError('source records cannot shadow native assessment identities')
                else:
                    sourced.append(item)
            if private_sources is not None:
                native_index = {item['id']: item for item in native_records}
                for item in private_sources.native_records:
                    if item['id'] in native_index and _canonical(native_index[item['id']]) != _canonical(item):
                        raise ValueError('private Claim native evidence has another current body or origin')
                    native_index[item['id']] = item
                native_records = list(native_index.values())
                native_summaries.extend(private_sources.native_summaries)
            identity_snapshots['native_text_snapshots'] = [resolver.snapshot() for resolver in native_resolvers]
        if layer_sources is not None:
            selected = {row['id']: row for row in sourced}
            native_ids = {row['id'] for row in native_records}
            for row in layer_sources.records:
                if row['id'] in selected:
                    if (row['id'] not in native_ids or row['id'] not in layer_sources.layers
                            or _canonical(selected[row['id']]) != _canonical(row)):
                        raise ValueError('layer comparison cannot shadow another source identity or origin')
                else:
                    sourced.append(row)
            identity_snapshots['native_layer_assessment_snapshot'] = layer_sources.snapshot()
        snapshot = 'sha256:' + _digest({'configuration': config, 'source_files': fixity,
                                       'resolved_records': sourced, **identity_snapshots,
                                       **({'public_claim_dependencies': public_claim_dependencies}
                                          if public_claim_dependencies else {}),
                                       **({'owner_local_sources': private_snapshot,
                                           'configuration_bytes': hashlib.sha256(encoded).hexdigest()}
                                          if owner_local else {})})
    if len(_canonical(request)) > MAX_RECORD_BYTES:
        raise ValueError('command exceeds the 1 MiB input budget')
    operation = request.get('operation') if isinstance(request, dict) else None
    fields = {'schema_version', 'operation', 'subject_id'}
    if operation != 'describe':
        fields |= {'expected_subject', 'expected_snapshot'}
    if operation == 'append':
        fields |= {'command_id', 'expected_revision', 'assessments'}
    elif operation not in ('inspect', 'describe', 'materialize-form', *(['read-layer-comparison'] if layer_quality else [])):
        raise ValueError('unknown assessment command')
    _keys(request, fields)
    if request['schema_version'] != 'tos_local_assessment_command_v1':
        raise ValueError('unknown assessment command version')
    if not layer_quality and operation != 'describe' and request['expected_snapshot'] != snapshot:
        raise JournalConflict('expected owner snapshot is stale')

    def envelope_record(value):
        _keys(value, {'id', 'version', 'payload', 'origin_id'})
        return Record.from_payload(**value)

    for key in ('authorities', 'competencies', 'records'):
        if not isinstance(config[key], list) or len(config[key]) > MAX_ASSESSMENTS:
            raise ValueError('owner record collections must be bounded lists')
    if len(config['records']) + len(sourced) > MAX_ASSESSMENTS:
        raise ValueError('combined owner/source records exceed snapshot budget')
    if {item['id'] for item in config['records']} & {item['id'] for item in sourced}:
        raise ValueError('inline records cannot shadow source-bound records')
    engine = AssessmentEngine(contract_root or Path(__file__).resolve().parents[5],
                              envelope_record(config['policy']),
                              [envelope_record(item) for item in config['authorities']],
                              [envelope_record(item) for item in config['competencies']],
                              [envelope_record(item) for item in [*config['records'], *sourced]],
                              **({'validators': private_sources.assessment_validators} if private_sources is not None else {}))
    subjects = config['subjects']
    identifier = request['subject_id']
    if (not isinstance(subjects, dict) or len(subjects) > MAX_ASSESSMENTS
            or not isinstance(identifier, str) or identifier not in subjects):
        raise PermissionError('subject is outside the configured command scope')
    if (identifier in {row['id'] for row in native_records}
            and identifier not in explicit_native_targets
            and (layer_sources is None or identifier not in layer_sources.layers)):
        raise PermissionError('native supporting layer is evidence, not a selected unit assessment target')
    scope = subjects[identifier]
    scope_fields = {'record', 'assertion_layer', 'risk', 'languages', 'maker_id', 'requested_use', 'access_allowed'}
    if source_bound and 'form_language_context' in scope:
        scope_fields.add('form_language_context')
    _keys(scope, scope_fields)
    current = engine.records.get(identifier)
    if (current is None or _canonical(current.ref) != _canonical(scope['record'])
            or (operation != 'describe' and _canonical(current.ref) != _canonical(request['expected_subject']))):
        raise JournalConflict('expected subject or owner scope is stale')
    if (not isinstance(scope['languages'], list) or not scope['languages']
            or any(not isinstance(item, str) or not item for item in scope['languages'])
            or any(not isinstance(scope[key], str) or not scope[key]
                   for key in ('assertion_layer', 'risk', 'maker_id', 'requested_use'))):
        raise ValueError('owner subject scope is incomplete')
    if scope['access_allowed'] is not True:
        raise PermissionError('subject access is not allowed')
    public_claim_bound = identifier in public_claim_dependencies
    required_sources = (_public_claim_required_sources(identifier, public_claim_dependencies, sourced,
                                                       native_summaries) if public_claim_bound
                        else private_sources.required_sources(identifier) if private_sources is not None else ())
    selected_layer = layer_sources.layers.get(identifier) if layer_sources is not None else None
    if selected_layer is not None:
        required_sources = (selected_layer['comparison'],) if selected_layer['comparison'] is not None else ()
    claim_source_bound = public_claim_bound or (private_sources is not None and identifier in private_sources.claim_dependencies)
    active_native_summaries = native_summaries
    if claim_source_bound:
        dependency_ids = {record.id for record in required_sources}
        active_native_summaries = [row for row in native_summaries if row['unit_id'] in dependency_ids]
    if selected_layer is not None:
        active_native_summaries = []
    if (private_sources is not None and identifier in {row['id'] for row in sourced}
            and identifier not in {row['id'] for row in native_records}):
        required_languages = private_sources.required_languages(identifier, sourced)
        required_languages.update(row['language'].casefold() for row in active_native_summaries
                                  if isinstance(row.get('language'), str) and row['language'])
        if not required_languages <= {language.casefold() for language in scope['languages']}:
            raise PermissionError('private assessment scope omits source or authored-form languages')
    if identifier in {item['id'] for item in sourced}:
        body = current.payload
        if 'claim_id' in body and 'claim_version' in body:
            maker = body.get('maker')
            if (scope['assertion_layer'] != body.get('assertion_layer')
                    or not isinstance(maker, dict) or scope['maker_id'] != maker.get('agent_ref')):
                raise PermissionError('configured scope disagrees with source-owned claim layer or maker')
        elif body['schema_version'] == 'tos_human_form_v1':
            if scope['assertion_layer'] != 'human_projection' or scope['maker_id'] != body.get('creator_id'):
                raise PermissionError('configured scope disagrees with source-owned form layer or maker')
    # A public occurrence description is metadata about a use, not proof that
    # its text was read. Match the ENTIRE fixed binding, not merely a unit ID.
    # This applies to supporting occurrence records and source-bound forms too;
    # selecting an unrelated native unit cannot qualify their source return.
    native_subjects = ([record.payload for record in required_sources] if claim_source_bound
                       else [item['payload'] for item in sourced])
    required_native_bindings = [body['native_text_binding'] for body in native_subjects
                               if 'native_text_binding' in body]
    if ('native_text_binding' in current.payload
            or current.payload.get('schema_version') == 'tos_occurrence_description_record_v1'):
        required_native_bindings.append(current.payload.get('native_text_binding'))
    source_read_ready = all(isinstance(binding, dict) and any(
        _canonical(row['payload'].get('native_binding')) == _canonical(binding)
        and row['payload'].get('content_verified') is True for row in native_records)
        for binding in required_native_bindings)
    if owner_local and any(not row['content_verified'] for row in active_native_summaries):
        source_read_ready = False
    if selected_layer is not None:
        source_read_ready = selected_layer['comparison'] is not None
    if operation == 'materialize-form' and not source_read_ready:
        raise PermissionError('native-bound form materialization requires the same explicitly selected exact text read')
    context = SubjectContext(current, scope['assertion_layer'], scope['risk'],
                             tuple(scope['languages']), scope['maker_id'], scope['requested_use'], True,
                             source_read_ready=source_read_ready, required_sources=required_sources,
                             positive_use_allowed=selected_layer['read_ready'] if selected_layer is not None else True)
    source_form = source_bound and identifier in {item['id'] for item in sourced} and current.payload.get('schema_version') == 'tos_human_form_v1'
    if 'form_language_context' in scope and not source_form:
        raise PermissionError('form linguistic context is outside a source-form scope')
    directory = Path(config['journal_directory'])
    descriptor = _owned_path(directory, directory=True)
    os.close(descriptor)
    journal = AssessmentJournal(directory, contract_root=contract_root, protected_storage=True,
                                confidential_root=owner_context.private_root if owner_context is not None else None,
                                batch_validator=private_sources.assessment_validators['-batch'] if private_sources is not None else None)
    quality_requirements = (_quality_requirements(current, required_sources, engine.records,
        config['quality_dependencies'], layer_sources.layers, scope['assertion_layer']) if layer_quality else [])
    parent_context, parent_quality_requirements = None, []
    if layer_quality and source_form:
        parent_context = _form_parent_claim_context(context, subjects, engine.records, sourced, private_sources,
                                                   public_claim_dependencies, native_summaries)
        if parent_context is not None:
            parent_quality_requirements = _quality_requirements(parent_context.record, parent_context.required_sources,
                engine.records, config['quality_dependencies'], layer_sources.layers, parent_context.assertion_layer)
            if any(entry not in quality_requirements for entry in parent_quality_requirements):
                raise PermissionError('parent Claim quality is outside the selected form closure')

    def source_snapshot_guard():
        # Recheck after waiting for a lock, at the commit edge, and before any
        # source-bound current read returns. Unpublished blobs stay outside history.
        source_publication.verify_current()
        with os.fdopen(_owned_path(owner_config), 'rb') as stream:
            current_config = stream.read(len(encoded) + 1)
        if current_config != encoded:
            raise JournalConflict('protected source assessment configuration changed')
        for resolver in native_resolvers:
            resolver.snapshot()
        if layer_sources is not None:
            if layer_sources.snapshot() != identity_snapshots['native_layer_assessment_snapshot']:
                raise JournalConflict('layer comparison inputs changed during assessment')
        if owner_context is not None:
            from source_owner_context import _read
            if _read(owner_config, len(encoded), confidential_file=True) != encoded:
                raise JournalConflict('confidential assessment configuration changed')
            confidential_journal(owner_context, config['journal_directory'])
            if private_sources.snapshot() != private_snapshot:
                raise JournalConflict('private assessment supporting source snapshot changed')
        current_identities, current_dependencies = {}, {}
        current_records, current_fixity = _source_records(source_root, config['source_records'],
                                                          identity_snapshots=current_identities,
                                                          claim_dependencies=current_dependencies)
        if (current_records != regular_sourced or current_fixity != fixity
                or current_dependencies != public_claim_dependencies
                or any(identity_snapshots.get(key) != value for key, value in current_identities.items())
                or set(current_identities) != set(identity_snapshots) - {'native_text_snapshots', 'native_layer_assessment_snapshot'}):
            raise JournalConflict('assessment supporting source snapshot changed')
        source_publication.verify_current()

    def execute_operation():
        nonlocal context, snapshot, source_read_ready
        now = datetime.now(timezone.utc).isoformat()
        dependency_heads, admission_dependencies = {}, []
        subject_assessment = None
        if layer_quality:
            source_snapshot_guard()
            for requirement in quality_requirements:
                layer_id = requirement['layer_id']
                layer = layer_sources.layers[layer_id]
                layer_scope = subjects[layer_id]
                if layer_scope['requested_use'] != requirement['use']:
                    raise PermissionError('quality dependency requires another explicitly selected layer use')
                layer_context = SubjectContext(layer['record'], layer_scope['assertion_layer'],
                    layer_scope['risk'], tuple(layer_scope['languages']), layer_scope['maker_id'],
                    layer_scope['requested_use'], access_allowed=True, source_read_ready=layer['comparison'] is not None,
                    positive_use_allowed=layer['read_ready'],
                    required_sources=(layer['comparison'],) if layer['comparison'] is not None else ())
                dependency = journal.inspect(engine, layer_context, now=now)
                dependency_heads[layer_id] = dependency['revision']
                basis = _quality_basis(layer, dependency['current_admission'], requirement['use'], quality_validator)
                if basis.id in engine.records:
                    raise PermissionError('inline source cannot supply a derived current quality basis')
                engine.records[basis.id] = basis
                sourced.append({'id': basis.id, 'version': basis.version, 'payload': basis.payload,
                                'origin_id': basis.origin_id})
                admission_dependencies.append(RequiredAdmission(basis, basis.payload['can_use'],
                                                               tuple(basis.payload['limits'])))
                source_read_ready = source_read_ready and layer['comparison'] is not None
            context = replace(context, required_admissions=tuple(admission_dependencies),
                              source_read_ready=source_read_ready)
            snapshot = 'sha256:' + _digest({'source_snapshot': snapshot,
                'quality_bases': [entry.basis.ref for entry in admission_dependencies]})
            if parent_context is not None:
                parent_layers = {entry['layer_id'] for entry in parent_quality_requirements}
                parent = replace(parent_context, required_admissions=tuple(entry for entry in admission_dependencies
                    if entry.basis.payload['layer']['id'] in parent_layers))
                revision, chain = journal._load(parent.record.id)
                history = journal._history(chain)
                admission = engine.evaluate(parent, (), now=now, trusted_history=history)
                withdrawals = [Record.from_payload(entry.assessment['assessment_id'], 1, entry.assessment).ref
                    for entry in history if entry.assessment['decision'] == 'withdraw'
                    and entry.assessment['subject'] == parent.record.ref
                    and entry.committed_scope is not None and entry.committed_scope.matches(parent)]
                subject_assessment = {'schema_version': 'tos_human_form_subject_assessment_v1',
                    'subject': parent.record.ref, 'admission': admission, 'journal_revision': revision,
                    'journal_batches': len(chain), 'historical_withdrawals': withdrawals,
                    'form_admission_is_parent_endorsement': False}
                dependency_heads[parent.record.id] = revision
                snapshot = 'sha256:' + _digest({'source_snapshot': snapshot, 'subject_assessment': subject_assessment})
            if operation != 'describe' and request['expected_snapshot'] != snapshot:
                raise JournalConflict('expected source or current quality basis snapshot is stale')
        if operation == 'materialize-form':
            if not source_form:
                raise PermissionError('form materialization requires a source-bound form scope')
            revision, chain = journal._load(identifier)
            materialized = _materialize_source_form(config, sourced, form_sets, engine, context,
                journal._history(chain), now=now, contract_root=contract_root or Path(__file__).resolve().parents[5],
                owner_local_paths=private_sources.paths if private_sources is not None else None,
                owner_local_form_validator=private_sources.form_validator if private_sources is not None else None,
                materializer_validators=private_sources.materializer_validators if private_sources is not None else None,
                subject_assessment=subject_assessment)
            result = {'revision': revision, 'batch_count': len(chain),
                      'current_admission': materialized['admission'], 'materialization': materialized}
        elif operation in ('inspect', 'describe', 'read-layer-comparison'):
            result = journal.inspect(engine, context, now=now)
            if operation == 'read-layer-comparison':
                if selected_layer is None or selected_layer['comparison'] is None:
                    raise PermissionError('comparison reading requires an exact selected layer and original-source grant')
                comparison = selected_layer['comparison']
                result['source_comparison'] = {'record': comparison.ref, 'payload': comparison.payload,
                                               'origin_id': comparison.origin_id}
            if operation == 'describe':
                result['command_context'] = {'subject': current.ref, 'policy': engine.policy.ref,
                                             'scope': scope, 'supported_operations': ['describe', 'inspect', 'append',
                                                 *(['materialize-form'] if source_form and isinstance(current.payload.get('content'), dict)
                                                   and current.payload['content'].get('kind') in ('source-copy', 'freeform') else [])],
                                             'grants_authority': False}
                if required_sources:
                    result['command_context']['required_sources'] = [record.ref for record in required_sources]
                if subject_assessment is not None:
                    result['command_context']['subject_assessment'] = subject_assessment
                if layer_sources is not None:
                    result['command_context']['required_admissions'] = [
                        {'basis': entry.basis.ref, 'can_use': entry.can_use, 'limits': list(entry.limits),
                         'layer': entry.basis.payload['layer'], 'use': entry.basis.payload['use']}
                        for entry in admission_dependencies]
                    result['command_context']['layer_comparison_contracts'] = [
                        {'path': path, 'digest': 'sha256:' + digest}
                        for path, digest in sorted(layer_sources.contracts.items())]
                if selected_layer is not None:
                    result['command_context']['source_comparison'] = {
                        'required': True, 'ready': selected_layer['comparison'] is not None,
                        'positive_use_allowed': selected_layer['read_ready'],
                        'record': selected_layer['comparison'].ref if selected_layer['comparison'] is not None else None}
                    if selected_layer['comparison'] is None:
                        result['command_context']['supported_operations'] = ['describe', 'inspect']
                    if selected_layer['comparison'] is not None:
                        result['command_context']['supported_operations'].append('read-layer-comparison')
                if required_native_bindings or quality_requirements:
                    result['command_context']['source_read'] = {'required': True, 'ready': source_read_ready}
                    if not source_read_ready:
                        result['command_context']['supported_operations'] = ['describe', 'inspect']
                if source_bound:
                    digests = {item['path']: item['digest'] for item in fixity}
                    result['command_context']['source_records'] = [
                        {'record': envelope_record(item).ref, 'path': binding['path'],
                         'file_digest': digests[binding['path']], 'origin_id': item['origin_id']}
                        for item, binding in zip(regular_sourced, config['source_records'], strict=True)]
                    paths = {binding['path'] for binding in config['source_records']}
                    result['command_context']['source_contracts'] = [item for item in fixity if item['path'] not in paths]
                    if native_bound:
                        result['command_context']['native_text_units'] = native_summaries
                        result['command_context']['native_contracts'] = [
                            {'path': path, 'digest': 'sha256:' + digest}
                            for path, digest in sorted(native_contracts.items())]
                        if any(not row['content_verified'] for row in active_native_summaries):
                            result['command_context']['supported_operations'] = ['describe', 'inspect']
                    if private_sources is not None:
                        result['command_context']['owner_local_source_records'] = [
                            {'record': envelope_record(item).ref, 'origin_id': item['origin_id']}
                            for item in private_sources.records]
                        result['command_context']['owner_local_contracts'] = [
                            {'path': path, 'digest': 'sha256:' + digest}
                            for path, digest in sorted(private_sources.contracts.items())]
        else:
            if not source_read_ready:
                raise PermissionError('native-bound source assessment requires the same explicitly selected exact text read')
            if any(not row['content_verified'] for row in active_native_summaries):
                raise PermissionError('native assessment append requires an explicit exact text read')
            execution = config['execution_profile']
            _keys(execution, {'id', 'version', 'digest'})
            executor = engine.records.get(execution['id'])
            if executor is None or _canonical(executor.ref) != _canonical(execution):
                raise PermissionError('configured execution profile is not current')
            assessments = request['assessments']
            revision = request['expected_revision']
            if (not isinstance(assessments, list) or not assessments
                    or len(assessments) > MAX_ASSESSMENTS
                    or any(not isinstance(item, dict) for item in assessments)
                    or (revision is not None and (not isinstance(revision, str)
                                                  or not re.fullmatch(r'[a-f0-9]{64}', revision)))):
                raise ValueError('invalid assessment batch or expected revision')
            reviews = [Submission(item, config['principal_id'], executor) for item in assessments]
            result = journal.append(engine, context, reviews, command_id=request['command_id'],
                                    expected_revision=revision, now=now,
                                    **({'snapshot_guard': source_snapshot_guard} if source_bound else {}))
        if source_bound:
            source_snapshot_guard()
            if any(journal._load(layer_id)[0] != revision for layer_id, revision in dependency_heads.items()):
                raise JournalConflict('source quality or parent history changed before returning its dependent view')
            if journal._load(identifier)[0] != result['revision']:
                raise JournalConflict('source assessment history changed before returning the current view')
        return {'schema_version': 'tos_local_assessment_result_v1', 'owner_snapshot': snapshot,
                'authentication': 'local-unix-account', 'result': result,
                **({'visibility': 'local_only', 'publication_authorized': False} if owner_local else {})}

    if layer_quality:
        with journal.locked_subjects([identifier, *(entry['layer_id'] for entry in quality_requirements),
                                      *([parent_context.record.id] if parent_context is not None else [])]):
            return execute_operation()
    return execute_operation()


def main() -> int:
    import argparse
    import sys
    parser = argparse.ArgumentParser(description='Local source-owner assessment commands; JSON on stdin.')
    parser.add_argument('--owner-config', type=Path, required=True,
                        help='operator-selected protected configuration; never take this path from the request')
    args = parser.parse_args()
    try:
        encoded = sys.stdin.buffer.read(MAX_RECORD_BYTES + 1)
        if len(encoded) > MAX_RECORD_BYTES:
            raise ValueError('command exceeds the 1 MiB input budget')
        result = run_local_command(args.owner_config, _json_object(encoded))
    except (OSError, ValueError, TypeError, KeyError, RecursionError, ValidationError) as exc:
        # No source/configuration payload or exception text is reflected.
        print(json.dumps({'schema_version': 'tos_local_assessment_error_v1',
                          'error': type(exc).__name__}))
        return 2
    print(json.dumps(result, ensure_ascii=False, allow_nan=False))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
