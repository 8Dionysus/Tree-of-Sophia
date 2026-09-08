"""Source-owner assessment journal; immutable batches and an atomic head pointer.

This is a local Unix storage/command adapter, not an HTTP authentication boundary.
The owner service, or independently selected protected local configuration,
supplies trusted engine, subject context and Submission bindings. Source records
remain authoritative;
this journal records assessments and their commit-time qualification, not a
second corpus or an independently authoritative cached admission database.
"""
from __future__ import annotations

from contextlib import contextmanager
from dataclasses import dataclass
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
import time
from typing import Any, Callable, Iterator, Sequence
from jsonschema import Draft202012Validator, FormatChecker, ValidationError

from knowledge_assessment import (
    AssessmentEngine, MAX_ASSESSMENTS, MAX_RECORD_BYTES, SubjectContext,
    Record, Submission, _canonical, _instant, _validators,
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


def _restore(payload: dict[str, Any]) -> Submission:
    return Submission(payload['assessment'], payload['principal_id'],
                      _RecordedExecution(**payload['execution_profile']))


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
                 lock_timeout_seconds: float = 5.0, protected_storage: bool = False):
        self.directory = directory.resolve()
        if not 0 <= lock_timeout_seconds <= 60:
            raise ValueError('lock timeout must be between zero and sixty seconds')
        self.lock_timeout_seconds = lock_timeout_seconds
        self.protected_storage = protected_storage
        if not self.directory.parent.is_dir():
            raise ValueError('the configured owner parent directory must already exist')
        self.validator = _validators((contract_root or Path(__file__).resolve().parents[5]).resolve())['-batch']

    def _home(self, subject_id: str) -> Path:
        # Hashes partition storage only; they do not replace ToS identity.
        return self.directory / hashlib.sha256(subject_id.encode('utf-8')).hexdigest()

    @contextmanager
    def _locked(self, home: Path) -> Iterator[None]:
        self.directory.mkdir(mode=0o700, exist_ok=True)
        _sync_directory(self.directory.parent)
        home.mkdir(mode=0o700, exist_ok=True)
        _sync_directory(self.directory)
        if self.protected_storage:
            os.close(_owned_path(home, directory=True))
        lock_path = home / '.writer.lock'
        descriptor = os.open(lock_path, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW | os.O_NONBLOCK, 0o600)
        with os.fdopen(descriptor, 'a+b') as lock:
            if not stat.S_ISREG(os.fstat(lock.fileno()).st_mode):
                raise JournalCorruption('writer lock must be a regular file')
            if self.protected_storage:
                os.close(_owned_path(lock_path))
            deadline = time.monotonic() + self.lock_timeout_seconds
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
                yield
            finally:
                fcntl.flock(lock, fcntl.LOCK_UN)

    def _load(self, subject_id: str) -> tuple[str | None, list[dict[str, Any]]]:
        home = self._home(subject_id)
        if self.protected_storage and (home.exists() or home.is_symlink()):
            os.close(_owned_path(home, directory=True))
        head = home / 'head'
        if head.is_symlink():
            raise JournalCorruption('a head pointer cannot be a symlink')
        if not head.exists():
            return None, []
        if self.protected_storage:
            os.close(_owned_path(head))
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
                    os.close(_owned_path(path))
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
        return [_restore(event) for batch in chain for event in batch['events']]

    def inspect(self, engine: AssessmentEngine, context: SubjectContext, *, now: str) -> dict[str, Any]:
        if context.access_allowed is not True:
            raise PermissionError('subject access is not allowed')
        revision, chain = self._load(context.record.id)
        return {'revision': revision, 'batch_count': len(chain),
                'current_admission': engine.evaluate(context, (), now=now, trusted_history=self._history(chain))}

    def _write_blob(self, home: Path, revision: str, payload: bytes) -> None:
        target = home / (revision + '.json')
        if self.protected_storage and (target.exists() or target.is_symlink()):
            os.close(_owned_path(target))
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
                    identity_snapshots: dict | None = None) -> tuple[list[dict[str, Any]], list[dict[str, Any]]]:
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
    return resolved, fixity


def _materialize_source_form(config, sourced, form_sets, engine, context, history, *, now, contract_root):
    """Render the selected form from this exact source/journal snapshot only.

    This owner lane requires whole-subject context for assessed freeform. It
    does not make submitted bindings the authority for dropping qualifications.
    Existing source-copy/template adapters retain their separate contracts.
    """
    from human_forms import FormScope, SourceBinding, materialize_form
    from source_witness_human_forms import _validator, claim_forms_path
    form = context.record
    body = form.payload
    selected = {row['id'] for row in sourced}
    if (form.id not in selected or body.get('schema_version') != 'tos_human_form_v1'
            or not isinstance(body.get('content'), dict) or body['content'].get('kind') != 'freeform'):
        raise PermissionError('assessed materialization requires an explicitly selected freeform source form')
    subject_ref = body.get('subject')
    subject = engine.records.get(subject_ref.get('id')) if isinstance(subject_ref, dict) else None
    if subject is None or subject.id not in selected or subject.id == form.id:
        raise PermissionError('form subject must be explicitly selected from source')
    if subject.ref != subject_ref:
        raise JournalConflict('form binds a different subject snapshot')
    paths = {binding['record_id']: Path(binding['path']) for binding in config['source_records']}
    if subject.id not in paths or form.id not in paths:
        raise PermissionError('native evidence needs its own explicit form adapter')
    source_path, form_path = paths[subject.id], paths[form.id]
    expected = (claim_forms_path(source_path, subject.id) if source_path.name == 'source-claims.jsonl'
                else source_path.with_name(source_path.stem + '.human-forms.json'))
    package = form_sets.get(form_path.as_posix())
    if form_path != expected or package is None or not _validator().is_valid(package):
        raise ValueError('form must belong to its validated adjacent source set')
    if package['subject'] != subject.ref:
        raise JournalConflict('form set binds a different subject snapshot')
    if sum(row == body for row in package['forms']) != 1:
        raise ValueError('current form does not resolve uniquely in its source set')
    for binding in body['bindings'].values():
        if binding['record']['id'] not in selected:
            raise PermissionError('form bindings require explicit source-selected records')
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
    scope = FormScope(subject, (SourceBinding(subject, ''),), context.maker_id, context.risk,
                      context.languages, context.requested_use, access_allowed=context.access_allowed,
                      language_context=language_binding)
    return materialize_form(contract_root, form, scope,
                            [engine.records[identity] for identity in selected], prior_forms=prior,
                            engine=engine, trusted_history=history, now=now)


def _native_text_records(config):
    """Explicit owner-local v3 selection, not a corpus crawl or public reader."""
    scripts = str(Path(__file__).resolve().parents[5] / 'scripts')
    if scripts not in sys.path:
        sys.path.insert(0, scripts)
    from native_text_binding import NativeTextBindingResolver
    selections, subjects = config['native_text_units'], config['subjects']
    if not isinstance(selections, list) or len(selections) > 64 or not isinstance(subjects, dict):
        raise ValueError('native assessment selection exceeds its bounded contract')
    seen, observed, total = set(), {}, 0
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
        resolver = NativeTextBindingResolver(Path(config['source_root']), read_bytes=protected_read)
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


def run_local_command(owner_config: Path, request: dict[str, Any], *,
                      contract_root: Path | None = None) -> dict[str, Any]:
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
    fields = {'schema_version', 'uid', 'principal_id', 'execution_profile',
              'policy', 'authorities', 'competencies', 'records', 'subjects', 'journal_directory'}
    native_bound = config.get('schema_version') == 'tos_local_assessment_owner_v3'
    source_bound = native_bound or config.get('schema_version') == 'tos_local_assessment_owner_v2'
    if source_bound:
        fields |= {'source_root', 'source_records'}
    if native_bound:
        fields.add('native_text_units')
    _keys(config, fields)
    if (config['schema_version'] not in ('tos_local_assessment_owner_v1', 'tos_local_assessment_owner_v2', 'tos_local_assessment_owner_v3')
            or type(config['uid']) is not int or config['uid'] != os.getuid()
            or not isinstance(config['principal_id'], str) or not config['principal_id'].strip()):
        raise PermissionError('configuration does not bind this local account')
    snapshot = 'sha256:' + _digest(config)
    sourced, regular_sourced, form_sets, identity_snapshots = [], [], {}, {}
    native_summaries, native_resolvers, native_contracts, native_records = [], [], {}, []
    if source_bound:
        sourced, fixity = _source_records(Path(config['source_root']), config['source_records'],
                                         form_sets=form_sets, identity_snapshots=identity_snapshots)
        regular_sourced = list(sourced)
        if native_bound:
            native_records, native_summaries, native_resolvers, native_contracts = _native_text_records(config)
            if {item['id'] for item in sourced} & {item['id'] for item in native_records}:
                raise ValueError('source records cannot shadow native assessment identities')
            sourced.extend(native_records)
            identity_snapshots['native_text_snapshots'] = [resolver.snapshot() for resolver in native_resolvers]
        snapshot = 'sha256:' + _digest({'configuration': config, 'source_files': fixity,
                                       'resolved_records': sourced, **identity_snapshots})
    if len(_canonical(request)) > MAX_RECORD_BYTES:
        raise ValueError('command exceeds the 1 MiB input budget')
    operation = request.get('operation') if isinstance(request, dict) else None
    fields = {'schema_version', 'operation', 'subject_id'}
    if operation != 'describe':
        fields |= {'expected_subject', 'expected_snapshot'}
    if operation == 'append':
        fields |= {'command_id', 'expected_revision', 'assessments'}
    elif operation not in ('inspect', 'describe', 'materialize-form'):
        raise ValueError('unknown assessment command')
    _keys(request, fields)
    if request['schema_version'] != 'tos_local_assessment_command_v1':
        raise ValueError('unknown assessment command version')
    if operation != 'describe' and request['expected_snapshot'] != snapshot:
        raise JournalConflict('expected owner snapshot is stale')

    def record(value):
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
                              record(config['policy']),
                              [record(item) for item in config['authorities']],
                              [record(item) for item in config['competencies']],
                              [record(item) for item in [*config['records'], *sourced]])
    subjects = config['subjects']
    identifier = request['subject_id']
    if (not isinstance(subjects, dict) or len(subjects) > MAX_ASSESSMENTS
            or not isinstance(identifier, str) or identifier not in subjects):
        raise PermissionError('subject is outside the configured command scope')
    if (identifier in {row['id'] for row in native_records}
            and identifier not in {row['unit_id'] for row in native_summaries}):
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
    context = SubjectContext(current, scope['assertion_layer'], scope['risk'],
                             tuple(scope['languages']), scope['maker_id'], scope['requested_use'], True)
    source_form = source_bound and identifier in {item['id'] for item in sourced} and current.payload.get('schema_version') == 'tos_human_form_v1'
    if 'form_language_context' in scope and not source_form:
        raise PermissionError('form linguistic context is outside a source-form scope')
    directory = Path(config['journal_directory'])
    descriptor = _owned_path(directory, directory=True)
    os.close(descriptor)
    journal = AssessmentJournal(directory, contract_root=contract_root, protected_storage=True)
    now = datetime.now(timezone.utc).isoformat()
    if operation == 'materialize-form':
        if not source_form:
            raise PermissionError('form materialization requires a source-bound form scope')
        revision, chain = journal._load(identifier)
        materialized = _materialize_source_form(config, sourced, form_sets, engine, context,
            journal._history(chain), now=now, contract_root=contract_root or Path(__file__).resolve().parents[5])
        result = {'revision': revision, 'batch_count': len(chain),
                  'current_admission': materialized['admission'], 'materialization': materialized}
    elif operation in ('inspect', 'describe'):
        result = journal.inspect(engine, context, now=now)
        if operation == 'describe':
            result['command_context'] = {'subject': current.ref, 'policy': engine.policy.ref,
                                         'scope': scope, 'supported_operations': ['describe', 'inspect', 'append',
                                             *(['materialize-form'] if source_form and isinstance(current.payload.get('content'), dict)
                                               and current.payload['content'].get('kind') == 'freeform' else [])],
                                         'grants_authority': False}
            if source_bound:
                digests = {item['path']: item['digest'] for item in fixity}
                result['command_context']['source_records'] = [
                    {'record': record(item).ref, 'path': binding['path'],
                     'file_digest': digests[binding['path']], 'origin_id': item['origin_id']}
                    for item, binding in zip(regular_sourced, config['source_records'], strict=True)]
                paths = {binding['path'] for binding in config['source_records']}
                result['command_context']['source_contracts'] = [item for item in fixity if item['path'] not in paths]
                if native_bound:
                    result['command_context']['native_text_units'] = native_summaries
                    result['command_context']['native_contracts'] = [
                        {'path': path, 'digest': 'sha256:' + digest}
                        for path, digest in sorted(native_contracts.items())]
                    if any(not row['content_verified'] for row in native_summaries):
                        result['command_context']['supported_operations'] = ['describe', 'inspect']
    else:
        if any(not row['content_verified'] for row in native_summaries):
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
        def native_snapshot_guard():
            # A command may have waited for the journal lock. Recheck again
            # after writing its immutable blob but BEFORE making it history.
            # Failed publication can retain an orphan; it never deletes history.
            with os.fdopen(_owned_path(owner_config), 'rb') as stream:
                current_config = stream.read(len(encoded) + 1)
            if current_config != encoded:
                raise JournalConflict('protected native assessment configuration changed')
            for resolver in native_resolvers:
                resolver.snapshot()
            current_identities = {}
            current_records, current_fixity = _source_records(Path(config['source_root']), config['source_records'],
                                                              identity_snapshots=current_identities)
            if (current_records != regular_sourced or current_fixity != fixity
                    or any(identity_snapshots.get(key) != value for key, value in current_identities.items())
                    or set(current_identities) != set(identity_snapshots) - {'native_text_snapshots'}):
                raise JournalConflict('native assessment supporting source snapshot changed')
        result = journal.append(engine, context, reviews, command_id=request['command_id'],
                                expected_revision=revision, now=now,
                                **({'snapshot_guard': native_snapshot_guard} if native_bound else {}))
    return {'schema_version': 'tos_local_assessment_result_v1', 'owner_snapshot': snapshot,
            'authentication': 'local-unix-account', 'result': result}


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
