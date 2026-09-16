#!/usr/bin/env python3
"""Immutable local corpus revisions and atomic, validator-owned admission.

Bytes and admission state live outside the software checkout. A revision is
mechanically admitted only by the supplied *program-owned* validator; this
transport does not assess source meaning, grant rights or promote canon.
"""
from __future__ import annotations

from contextlib import contextmanager
from dataclasses import dataclass
import fcntl
import ctypes
import errno
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
from types import MappingProxyType
import re
import shutil
import stat
import tempfile
from typing import Callable


class CorpusStoreError(ValueError):
    pass


def canonical(value) -> bytes:
    return (json.dumps(value, sort_keys=True, ensure_ascii=False,
                       separators=(',', ':'), allow_nan=False) + '\n').encode()


def digest_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def hex_digest(value: str) -> str:
    if not isinstance(value, str) or not re.fullmatch('[0-9a-f]{64}', value):
        raise CorpusStoreError('expected an exact SHA-256 digest')
    return value


def relative_path(value: str) -> str:
    if (not isinstance(value, str) or not value or '\\' in value or '\x00' in value
            or any(ord(c) < 32 for c in value)):
        raise CorpusStoreError('invalid corpus path')
    path = PurePosixPath(value)
    if (path.is_absolute() or path.as_posix() != value
            or any(part in {'.', '..', '.git'} for part in path.parts)):
        raise CorpusStoreError('corpus path must be normalized and relative')
    return value


def regular(path: Path) -> os.stat_result:
    if path.absolute() != path.resolve():
        raise CorpusStoreError(f'linked corpus path: {path}')
    result = path.lstat()
    if not stat.S_ISREG(result.st_mode):
        raise CorpusStoreError(f'non-regular corpus file: {path}')
    return result


def read_json(path: Path):
    regular(path)
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise CorpusStoreError('duplicate JSON field')
            result[key] = value
        return result
    raw = path.read_bytes()
    try:
        value = json.loads(raw, object_pairs_hook=pairs)
        if raw != canonical(value):
            raise CorpusStoreError('noncanonical corpus manifest')
    except (ValueError, UnicodeError) as error:
        raise CorpusStoreError('invalid canonical corpus manifest') from error
    return value


def _rename_new(source: Path, destination: Path):
    """Publish a directory without replacing even an empty concurrent target."""
    if os.name == 'nt':
        os.rename(source, destination)  # Windows rename refuses existing targets.
        return
    libc = ctypes.CDLL(None, use_errno=True)
    rename = getattr(libc, 'renameat2', None)
    if rename is None:
        raise CorpusStoreError('atomic exclusive directory publication is unsupported')
    rename.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    rename.restype = ctypes.c_int
    if rename(-100, os.fsencode(source), -100, os.fsencode(destination), 1) != 0:
        error = ctypes.get_errno()
        if error == errno.EEXIST:
            raise CorpusStoreError('refusing to replace existing restore output')
        raise OSError(error, os.strerror(error), str(destination))


def _sync_dir(path: Path):
    descriptor = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def _write(path: Path, value):
    with path.open('xb') as stream:
        stream.write(canonical(value))
        stream.flush()
        os.fsync(stream.fileno())


@dataclass(frozen=True)
class ValidationIndex:
    """Complete facts returned by the source validator, never a batch hint.

    identities maps stable IDs to their owning exact source paths. dependencies
    maps a source path to the paths whose change can invalidate it. The owner
    adapter includes schema, rights, fixity, evidence and referent dependencies.
    """
    identities: dict[str, str]
    dependencies: dict[str, list[str]]


def affected_paths(base: dict | None, changed: set[str]) -> set[str]:
    """Transitive incoming dependencies, including deleted referents."""
    affected = set(changed)
    reverse: dict[str, set[str]] = {}
    for source, targets in (base or {}).get('dependencies', {}).items():
        for target in targets:
            reverse.setdefault(target, set()).add(source)
    todo = list(changed)
    while todo:
        for dependent in reverse.get(todo.pop(), ()):
            if dependent not in affected:
                affected.add(dependent)
                todo.append(dependent)
    return affected


class CorpusCandidate:
    """Manifest-bound, lazy reads for one source transaction.

    An accepted base supplies membership and validated indexes. Its unrelated
    objects are not copied or rehashed for a new batch. Every requested object
    is checked against its immutable binding before use and again at closeout.
    Path-based owner tools may request private copies of an explicit closure;
    no returned path points into the object store.
    """

    def __init__(self, store: 'CorpusStore', files: dict[str, dict], root: Path,
                 *, retirements=()):
        self._store = store
        self._files = {path: MappingProxyType(dict(entry)) for path, entry in files.items()}
        self.paths = frozenset(self._files)
        self.retirements = tuple(MappingProxyType(dict(event)) for event in retirements)
        self._root = root
        self._read_paths: set[str] = set()
        self._materialized: set[str] = set()
        self._active = True

    def entry(self, relative: str):
        if not self._active:
            raise CorpusStoreError('source candidate is closed')
        path = relative_path(relative)
        if path not in self._files:
            raise CorpusStoreError(f'source is outside candidate membership: {path}')
        return self._files[path]

    def read_bytes(self, relative: str, *, max_bytes: int | None = None) -> bytes:
        entry = self.entry(relative)
        if max_bytes is not None and (type(max_bytes) is not int or max_bytes < 0):
            raise CorpusStoreError('invalid source read limit')
        if max_bytes is not None and entry['size_bytes'] > max_bytes:
            raise CorpusStoreError('source exceeds selected read limit')
        source = self._store._object(entry['sha256'])
        if regular(source).st_size != entry['size_bytes']:
            raise CorpusStoreError(f'corrupt corpus object for {relative}')
        with source.open('rb') as stream:
            raw = stream.read(entry['size_bytes'] + 1)
        if len(raw) != entry['size_bytes'] or hashlib.sha256(raw).hexdigest() != entry['sha256']:
            raise CorpusStoreError(f'corrupt corpus object for {relative}')
        self._read_paths.add(relative)
        return raw

    def materialize(self, paths) -> Path:
        """Copy exactly these members into this transaction's private view."""
        for relative in sorted(set(paths)):
            entry = self.entry(relative)
            target = self._root / relative
            if relative in self._materialized:
                if regular(target).st_size != entry['size_bytes'] or digest_file(target) != entry['sha256']:
                    raise CorpusStoreError('source validator modified admitted bytes')
                continue
            if target.exists() or target.is_symlink():
                raise CorpusStoreError('source materialization overlaps an existing output')
            directory = self._root
            for part in Path(relative).parts[:-1]:
                if directory.resolve() != directory.absolute() or not directory.is_dir():
                    raise CorpusStoreError('source materialization contains a linked directory')
                directory = directory / part
                if directory.is_symlink():
                    raise CorpusStoreError('source materialization contains a linked directory')
                directory.mkdir(exist_ok=True)
            if directory.resolve() != directory.absolute() or not directory.is_dir():
                raise CorpusStoreError('source materialization contains a linked directory')
            source = self._store._object(entry['sha256'])
            self._store._verify_object(entry)
            # A private streamed copy keeps large source documents out of RAM
            # and cannot turn a producer write into an object-store mutation.
            with source.open('rb') as stream, target.open('xb') as output:
                shutil.copyfileobj(stream, output, 1024 * 1024)
            os.chmod(target, entry['mode'])
            if target.stat().st_size != entry['size_bytes'] or digest_file(target) != entry['sha256']:
                raise CorpusStoreError('source object changed during materialization')
            self._read_paths.add(relative)
            self._materialized.add(relative)
        return self._root

    def verify_reads(self, *, additional=()):
        """Check accessed objects and private inputs, never unrelated base bytes."""
        for relative in sorted(self._read_paths | set(additional)):
            self._store._verify_object(self.entry(relative))
        for relative in sorted(self._materialized):
            entry = self.entry(relative)
            path = self._root / relative
            if regular(path).st_size != entry['size_bytes'] or digest_file(path) != entry['sha256']:
                raise CorpusStoreError('source validator modified admitted bytes')

    def close(self):
        self._active = False


class CorpusStore:
    def __init__(self, root: Path):
        self.root = Path(root).absolute()
        if self.root != self.root.resolve():
            raise CorpusStoreError('store root may not contain symlinks')
        self.root.mkdir(parents=True, exist_ok=True)
        for part in ('objects', 'revisions', 'staging'):
            directory = self.root / part
            directory.mkdir(exist_ok=True)
            if directory.is_symlink() or not directory.is_dir():
                raise CorpusStoreError('invalid store directory')

    @contextmanager
    def _lock(self):
        path = self.root / '.admission.lock'
        fd = os.open(path, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
        try:
            fcntl.flock(fd, fcntl.LOCK_EX)
            yield
        finally:
            os.close(fd)

    def current(self) -> str | None:
        path = self.root / 'current.json'
        if not path.exists():
            if path.is_symlink():
                raise CorpusStoreError('invalid current pointer')
            return None
        pointer = read_json(path)
        if set(pointer) != {'schema_version', 'current', 'previous'} or pointer['schema_version'] != 'tos_corpus_pointer_v1':
            raise CorpusStoreError('invalid current pointer')
        hex_digest(pointer['current'])
        if pointer['previous'] is not None:
            hex_digest(pointer['previous'])
        return pointer['current']

    def load(self, revision: str, *, verify_objects: bool = False) -> dict:
        revision = hex_digest(revision)
        manifest = read_json(self.root / 'revisions' / revision / 'snapshot.json')
        return self._validate_manifest(manifest, revision, verify_objects=verify_objects)

    def _validate_manifest(self, manifest: dict, revision: str, *,
                           verify_objects: bool = False) -> dict:
        if not isinstance(manifest, dict):
            raise CorpusStoreError('unsupported corpus snapshot')
        expected = {'schema_version', 'base_revision', 'validator_sha256', 'files',
                    'identities', 'dependencies', 'retirements', 'revision'}
        if set(manifest) != expected or manifest['schema_version'] != 'tos_corpus_snapshot_v1':
            raise CorpusStoreError('unsupported corpus snapshot')
        body = {key: value for key, value in manifest.items() if key != 'revision'}
        if manifest['revision'] != revision or hashlib.sha256(canonical(body)).hexdigest() != revision:
            raise CorpusStoreError('corpus revision digest mismatch')
        hex_digest(manifest['validator_sha256'])
        if manifest['base_revision'] is not None:
            hex_digest(manifest['base_revision'])
        if not isinstance(manifest['files'], list) or not isinstance(manifest['retirements'], list):
            raise CorpusStoreError('invalid corpus snapshot members')
        previous = None
        paths = set()
        for entry in manifest['files']:
            if not isinstance(entry, dict):
                raise CorpusStoreError('invalid snapshot file metadata')
            if set(entry) != {'path', 'sha256', 'size_bytes', 'mode'}:
                raise CorpusStoreError('invalid snapshot file metadata')
            path = relative_path(entry['path'])
            if previous is not None and path <= previous:
                raise CorpusStoreError('invalid or duplicate snapshot member')
            previous = path
            paths.add(path)
            hex_digest(entry['sha256'])
            if type(entry['size_bytes']) is not int or entry['size_bytes'] < 0 or entry['mode'] not in (0o644, 0o755):
                raise CorpusStoreError('invalid snapshot file metadata')
            if verify_objects:
                self._verify_object(entry)
        self._check_index(ValidationIndex(manifest['identities'], manifest['dependencies']), paths)
        retirement_events = set()
        for event in manifest['retirements']:
            if not isinstance(event, dict) or set(event) != {
                    'path', 'sha256', 'event_ref', 'event_sha256', 'event_size_bytes'}:
                raise CorpusStoreError('invalid retirement event')
            retired_path = relative_path(event['path'])
            source_sha256 = hex_digest(event['sha256'])
            event_ref = relative_path(event['event_ref'])
            event_sha256 = hex_digest(event['event_sha256'])
            event_key = (retired_path, source_sha256, event_ref, event_sha256)
            if event_key in retirement_events:
                raise CorpusStoreError('duplicate retirement event')
            retirement_events.add(event_key)
            if event_ref == retired_path:
                raise CorpusStoreError('retirement event cannot retire itself')
            if type(event['event_size_bytes']) is not int or event['event_size_bytes'] < 0:
                raise CorpusStoreError('invalid retirement event size')
            if verify_objects:
                self._verify_digest_object(source_sha256, label=retired_path)
                self._verify_object({
                    'path': event_ref,
                    'sha256': event_sha256,
                    'size_bytes': event['event_size_bytes'],
                })
        return manifest

    def _object(self, digest: str) -> Path:
        return self.root / 'objects' / hex_digest(digest)

    def _verify_object(self, entry: dict):
        self._verify_digest_object(
            entry['sha256'], expected_size=entry['size_bytes'], label=entry['path'])

    def _verify_digest_object(self, digest: str, *, expected_size: int | None = None,
                              label: str = 'corpus object'):
        path = self._object(digest)
        info = regular(path)
        if expected_size is not None and info.st_size != expected_size:
            raise CorpusStoreError(f'corrupt corpus object for {label}')
        if digest_file(path) != digest:
            raise CorpusStoreError(f'corrupt corpus object for {label}')

    @staticmethod
    def _retirement_spec(spec: dict) -> tuple[str, str]:
        if not isinstance(spec, dict) or set(spec) != {'event_ref', 'event_sha256'}:
            raise CorpusStoreError('retirement needs an exact event binding')
        event_ref = relative_path(spec['event_ref'])
        event_sha256 = hex_digest(spec['event_sha256'])
        return event_ref, event_sha256

    def _resolve_retirement(self, target: str, spec: dict, files: dict[str, dict],
                            retirement_targets: set[str]) -> dict:
        event_ref, event_sha256 = self._retirement_spec(spec)
        if target not in files:
            raise CorpusStoreError('retirement needs an existing source path')
        if event_ref == target or event_ref in retirement_targets:
            raise CorpusStoreError('retirement event is removed by the same batch')
        event_entry = files.get(event_ref)
        if event_entry is None:
            raise CorpusStoreError('retirement event is missing from the candidate')
        if event_entry['sha256'] != event_sha256:
            raise CorpusStoreError('retirement event digest does not match candidate binding')
        self._verify_object(event_entry)
        self._verify_object(files[target])
        source_entry = files.pop(target)
        return {
            'path': target,
            'sha256': source_entry['sha256'],
            'event_ref': event_ref,
            'event_sha256': event_sha256,
            'event_size_bytes': event_entry['size_bytes'],
        }

    def _ingest(self, source: Path, *, path: str, expected_sha256: str, expected_size: int, mode: int) -> dict:
        entry = {'path': relative_path(path), 'sha256': hex_digest(expected_sha256),
                 'size_bytes': expected_size, 'mode': mode}
        if type(expected_size) is not int or expected_size < 0 or mode not in (0o644, 0o755):
            raise CorpusStoreError('invalid input file metadata')
        before = regular(source)
        destination = self._object(expected_sha256)
        if destination.exists():
            self._verify_object(entry)
            if before.st_size != expected_size or digest_file(source) != expected_sha256:
                raise CorpusStoreError('source differs from declared digest')
            return entry
        with tempfile.NamedTemporaryFile(dir=self.root / 'staging', delete=False) as target:
            temporary = Path(target.name)
            try:
                with source.open('rb') as stream:
                    shutil.copyfileobj(stream, target, 1024 * 1024)
                target.flush()
                os.fsync(target.fileno())
                after = regular(source)
                if ((before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns) !=
                        (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
                        or temporary.stat().st_size != expected_size
                        or digest_file(temporary) != expected_sha256):
                    raise CorpusStoreError('source changed or digest differs during admission')
                os.chmod(temporary, 0o444)
                try:
                    os.link(temporary, destination)
                except FileExistsError:
                    self._verify_object(entry)
            finally:
                temporary.unlink(missing_ok=True)
        return entry

    @staticmethod
    def _check_index(index: ValidationIndex, paths: set[str]):
        if not isinstance(index, ValidationIndex) or not isinstance(index.identities, dict) or not isinstance(index.dependencies, dict):
            raise CorpusStoreError('source validator did not return complete indexes')
        for identity, path in index.identities.items():
            if not isinstance(identity, str) or not identity.strip() or relative_path(path) not in paths:
                raise CorpusStoreError('identity index refers outside snapshot')
        for source, targets in index.dependencies.items():
            if relative_path(source) not in paths or not isinstance(targets, list) or targets != sorted(set(targets)):
                raise CorpusStoreError('invalid dependency index')
            if any(relative_path(target) not in paths for target in targets):
                raise CorpusStoreError('dependency refers outside snapshot')

    def admit(self, *, base_revision: str | None, updates: dict[str, dict],
              retirements: dict[str, dict], validator_sha256: str,
              validate: Callable[[CorpusCandidate, dict | None, frozenset[str]], ValidationIndex]) -> dict:
        """Admit one complete batch, then compare-and-swap the accepted pointer.

        Each update supplies source Path, sha256, size_bytes, mode. The validator
        is selected by program code, never imported from a source/data manifest.
        It receives a lazy candidate and the transitive affected path set.
        Orphaned immutable objects after a failed batch are safe and not accepted.
        """
        hex_digest(validator_sha256)
        if not callable(validate) or not isinstance(updates, dict) or not isinstance(retirements, dict):
            raise CorpusStoreError('invalid admission request')
        # The immutable manifest/index is the accepted base. Accessed objects
        # are verified by CorpusCandidate; full custody scrubs and restores are
        # separate operations, not the cost of every unrelated source batch.
        base = self.load(base_revision) if base_revision else None
        if base_revision is not None:
            hex_digest(base_revision)
        files = {entry['path']: entry for entry in (base or {}).get('files', [])}
        normalized_updates = {}
        for raw_path, update in updates.items():
            path = relative_path(raw_path)
            if path in normalized_updates:
                raise CorpusStoreError('duplicate source update path')
            normalized_updates[path] = update
        normalized_retirements = {}
        for raw_path, spec in retirements.items():
            path = relative_path(raw_path)
            if path in normalized_retirements:
                raise CorpusStoreError('duplicate retirement path')
            self._retirement_spec(spec)
            normalized_retirements[path] = spec
        if set(normalized_updates) & set(normalized_retirements):
            raise CorpusStoreError('a batch cannot update and retire the same path')
        events = list((base or {}).get('retirements', []))
        historical_events = {
            (event['path'], event['sha256'], event['event_ref'], event['event_sha256'])
            for event in events
        }
        changed = set()
        for path, update in sorted(normalized_updates.items()):
            if set(update) != {'source', 'sha256', 'size_bytes', 'mode'}:
                raise CorpusStoreError('update must declare exact file bytes and mode')
            entry = self._ingest(Path(update['source']), path=path,
                                 expected_sha256=update['sha256'], expected_size=update['size_bytes'], mode=update['mode'])
            if entry != files.get(path):
                changed.add(path)
            files[path] = entry
        # Each object file was flushed above. Make the complete object namespace
        # durable once per atomic batch, before validation or pointer publication.
        # Failed batches may leave unreferenced objects, never an accepted revision.
        _sync_dir(self.root / 'objects')
        retirement_targets = set(normalized_retirements)
        for path, spec in sorted(normalized_retirements.items()):
            event = self._resolve_retirement(path, spec, files, retirement_targets)
            event_key = (event['path'], event['sha256'], event['event_ref'], event['event_sha256'])
            if event_key in historical_events:
                raise CorpusStoreError('retirement event already exists in history')
            events.append(event)
            historical_events.add(event_key)
            changed.add(path)
        if not changed and base is not None and base['validator_sha256'] == validator_sha256:
            with self._lock():
                if self.current() != base_revision:
                    raise CorpusStoreError('accepted base changed; re-admit against current revision')
            return base
        affected = affected_paths(base, changed)
        if base is None or base['validator_sha256'] != validator_sha256:
            affected.update(files)
        with tempfile.TemporaryDirectory(prefix='admission-', dir=self.root / 'staging') as raw:
            candidate = CorpusCandidate(self, files, Path(raw),
                                        retirements=events[len((base or {}).get('retirements', [])):])
            try:
                index = validate(candidate, json.loads(canonical(base)), frozenset(affected))
                candidate.verify_reads(additional=changed & files.keys())
            finally:
                candidate.close()
            self._check_index(index, set(files))
            # Retired stable identities stay reserved to their original path.
            reserved = {}
            ancestor = base
            while ancestor is not None:
                for identity, path in ancestor['identities'].items():
                    if identity in reserved and reserved[identity] != path:
                        raise CorpusStoreError('historical identity ownership conflict')
                    reserved[identity] = path
                ancestor = self.load(ancestor['base_revision']) if ancestor['base_revision'] else None
            for identity, path in index.identities.items():
                if identity in reserved and reserved[identity] != path:
                    raise CorpusStoreError('stable identity is reserved by a historical source path')
            body = {'schema_version': 'tos_corpus_snapshot_v1', 'base_revision': base_revision,
                    'validator_sha256': validator_sha256, 'files': [files[path] for path in sorted(files)],
                    'identities': index.identities, 'dependencies': index.dependencies,
                    'retirements': events}
            revision = hashlib.sha256(canonical(body)).hexdigest()
            manifest = {**body, 'revision': revision}
            self._validate_manifest(manifest, revision)
            destination = self.root / 'revisions' / revision
            with self._lock():
                current = self.current()
                if current not in (base_revision, revision):
                    raise CorpusStoreError('accepted base changed; re-admit against current revision')
                if not destination.exists():
                    with tempfile.TemporaryDirectory(dir=self.root / 'staging') as stage:
                        staged = Path(stage) / revision
                        staged.mkdir()
                        _write(staged / 'snapshot.json', manifest)
                        _sync_dir(staged)
                        os.rename(staged, destination)
                    _sync_dir(destination.parent)
                elif self.load(revision, verify_objects=True) != manifest:
                    raise CorpusStoreError('existing corpus revision differs')
                if current != revision:
                    pointer = self.root / 'current.json'
                    with tempfile.NamedTemporaryFile(dir=self.root, delete=False) as output:
                        temporary = Path(output.name)
                        output.write(canonical({'schema_version': 'tos_corpus_pointer_v1',
                                                'current': revision, 'previous': current}))
                        output.flush()
                        os.fsync(output.fileno())
                    try:
                        os.replace(temporary, pointer)
                        _sync_dir(self.root)
                    finally:
                        temporary.unlink(missing_ok=True)
            return manifest

    def restore(self, revision: str, output: Path) -> dict:
        manifest = self.load(revision, verify_objects=True)
        output = Path(output).absolute()
        if output.exists() or output.is_symlink() or output != output.resolve():
            raise CorpusStoreError('restore output must be a new, non-symlink path')
        output.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix='corpus-restore-', dir=output.parent) as raw:
            stage = Path(raw) / 'source'
            stage.mkdir()
            for entry in manifest['files']:
                path = stage / entry['path']
                path.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(self._object(entry['sha256']), path)
                os.chmod(path, entry['mode'])
                if digest_file(path) != entry['sha256']:
                    raise CorpusStoreError('corpus object changed during restore')
            _rename_new(stage, output)
            _sync_dir(output.parent)
        return manifest
