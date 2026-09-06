"""Source-owner assessment journal; immutable batches and an atomic head pointer.

This is a local Unix storage adapter, not an HTTP authentication boundary.
Only the owning command service supplies the trusted engine, subject context
and authenticated Submission bindings. Source records remain authoritative;
this journal records assessments and their commit-time qualification, not a
second corpus or an independently authoritative cached admission database.
"""
from __future__ import annotations

from contextlib import contextmanager
from dataclasses import dataclass
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import tempfile
import time
from typing import Any, Iterator, Sequence
from jsonschema import ValidationError

from knowledge_assessment import (
    AssessmentEngine, MAX_ASSESSMENTS, MAX_RECORD_BYTES, SubjectContext,
    Submission, _canonical, _instant, _validators,
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
                 lock_timeout_seconds: float = 5.0):
        self.directory = directory.resolve()
        if not 0 <= lock_timeout_seconds <= 60:
            raise ValueError('lock timeout must be between zero and sixty seconds')
        self.lock_timeout_seconds = lock_timeout_seconds
        if not self.directory.parent.is_dir():
            raise ValueError('the configured owner parent directory must already exist')
        self.validator = _validators((contract_root or Path(__file__).resolve().parents[5]).resolve())['-batch']

    def _home(self, subject_id: str) -> Path:
        # Hashes partition storage only; they do not replace ToS identity.
        return self.directory / hashlib.sha256(subject_id.encode('utf-8')).hexdigest()

    @contextmanager
    def _locked(self, home: Path) -> Iterator[None]:
        self.directory.mkdir(exist_ok=True)
        _sync_directory(self.directory.parent)
        home.mkdir(exist_ok=True)
        _sync_directory(self.directory)
        with (home / '.writer.lock').open('a+b') as lock:
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
        head = home / 'head'
        if head.is_symlink():
            raise JournalCorruption('a head pointer cannot be a symlink')
        if not head.exists():
            return None, []
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
