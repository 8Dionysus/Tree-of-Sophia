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
import json
import os
from pathlib import Path
import re
import stat
import tempfile
import time
from typing import Any, Iterator, Sequence
from jsonschema import ValidationError

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
               *, command_id: str, expected_revision: str | None, now: str) -> dict[str, Any]:
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
    _keys(config, {'schema_version', 'uid', 'principal_id', 'execution_profile',
                   'policy', 'authorities', 'competencies', 'records', 'subjects',
                   'journal_directory'})
    if (config['schema_version'] != 'tos_local_assessment_owner_v1'
            or type(config['uid']) is not int or config['uid'] != os.getuid()
            or not isinstance(config['principal_id'], str) or not config['principal_id'].strip()):
        raise PermissionError('configuration does not bind this local account')
    snapshot = 'sha256:' + _digest(config)
    if len(_canonical(request)) > MAX_RECORD_BYTES:
        raise ValueError('command exceeds the 1 MiB input budget')
    operation = request.get('operation') if isinstance(request, dict) else None
    fields = {'schema_version', 'operation', 'subject_id', 'expected_subject', 'expected_snapshot'}
    if operation == 'append':
        fields |= {'command_id', 'expected_revision', 'assessments'}
    elif operation != 'inspect':
        raise ValueError('unknown assessment command')
    _keys(request, fields)
    if request['schema_version'] != 'tos_local_assessment_command_v1':
        raise ValueError('unknown assessment command version')
    if request['expected_snapshot'] != snapshot:
        raise JournalConflict('expected owner snapshot is stale')

    def record(value):
        _keys(value, {'id', 'version', 'payload', 'origin_id'})
        return Record.from_payload(**value)

    for key in ('authorities', 'competencies', 'records'):
        if not isinstance(config[key], list) or len(config[key]) > MAX_ASSESSMENTS:
            raise ValueError('owner record collections must be bounded lists')
    engine = AssessmentEngine(contract_root or Path(__file__).resolve().parents[5],
                              record(config['policy']),
                              [record(item) for item in config['authorities']],
                              [record(item) for item in config['competencies']],
                              [record(item) for item in config['records']])
    subjects = config['subjects']
    identifier = request['subject_id']
    if (not isinstance(subjects, dict) or len(subjects) > MAX_ASSESSMENTS
            or not isinstance(identifier, str) or identifier not in subjects):
        raise PermissionError('subject is outside the configured command scope')
    scope = subjects[identifier]
    _keys(scope, {'record', 'assertion_layer', 'risk', 'languages', 'maker_id',
                  'requested_use', 'access_allowed'})
    current = engine.records.get(identifier)
    if (current is None or _canonical(current.ref) != _canonical(scope['record'])
            or _canonical(current.ref) != _canonical(request['expected_subject'])):
        raise JournalConflict('expected subject or owner scope is stale')
    if (not isinstance(scope['languages'], list) or not scope['languages']
            or any(not isinstance(item, str) or not item for item in scope['languages'])
            or any(not isinstance(scope[key], str) or not scope[key]
                   for key in ('assertion_layer', 'risk', 'maker_id', 'requested_use'))):
        raise ValueError('owner subject scope is incomplete')
    if scope['access_allowed'] is not True:
        raise PermissionError('subject access is not allowed')
    context = SubjectContext(current, scope['assertion_layer'], scope['risk'],
                             tuple(scope['languages']), scope['maker_id'], scope['requested_use'], True)
    execution = config['execution_profile']
    _keys(execution, {'id', 'version', 'digest'})
    executor = engine.records.get(execution['id'])
    if executor is None or _canonical(executor.ref) != _canonical(execution):
        raise PermissionError('configured execution profile is not current')
    directory = Path(config['journal_directory'])
    descriptor = _owned_path(directory, directory=True)
    os.close(descriptor)
    journal = AssessmentJournal(directory, contract_root=contract_root, protected_storage=True)
    now = datetime.now(timezone.utc).isoformat()
    if operation == 'inspect':
        result = journal.inspect(engine, context, now=now)
    else:
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
                                expected_revision=revision, now=now)
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
