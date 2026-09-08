"""Read one exact public Claim version, never its current permission to use.

The tracked catalog provides a source locator, not assertion authority. Current
source bytes must agree with that locator before a retained correction chain
can expose one predecessor. This is not the configured command/assessment API,
a historical form materializer, or a reader for private/native payloads.
"""
from __future__ import annotations

import copy
import errno
import os
from pathlib import Path
import re
import stat

import claim_revisions as claims
import source_commands as source
import source_revisions as packages
from source_record_profiles import SOURCE_CLAIM_BASENAME
from source_owner_context import OWNER_LOCAL_HOME

CATALOG_REF = 'ToS/source-witnesses/catalog/claims.jsonl'
MAX_CATALOG_BYTES = 8 * 1024 * 1024
MAX_CATALOG_ROWS = 8192
MAX_TOTAL_BYTES = 64 * 1024 * 1024
PUBLIC = {'public', 'public_metadata_only'}
FORBIDDEN = {'catalog', 'payload', 'private', 'local-content', 'owner-local'}
HASH = re.compile(r'sha256:[a-f0-9]{64}')
IDENTITY = re.compile(r'tos\.claim\.[a-z0-9]+(?:[.-][a-z0-9]+)*')


class _Unavailable(Exception):
    def __init__(self, status, reason):
        self.status, self.reason = status, reason


def _ref(value):
    return (isinstance(value, dict) and set(value) == {'id', 'version', 'digest'}
            and isinstance(value['id'], str) and IDENTITY.fullmatch(value['id'])
            and type(value['version']) is int and 1 <= value['version'] <= 9_007_199_254_740_991
            and isinstance(value['digest'], str) and HASH.fullmatch(value['digest']))


def _source_path(value):
    if not isinstance(value, str):
        raise _Unavailable('corrupt', 'catalog-source-locator-invalid')
    path = Path(value)
    if (path.is_absolute() or path.as_posix() != value or '\\' in value
            or path.parts[:2] != ('ToS', 'source-witnesses') or len(path.parts) < 4
            or path.name != SOURCE_CLAIM_BASENAME or path.is_relative_to(OWNER_LOCAL_HOME)
            or any(part in FORBIDDEN or part.startswith('.') for part in path.parts)):
        raise _Unavailable('access-restricted', 'source-outside-public-claim-metadata')
    return path


def _metadata_name(name):
    # Empty form-writer lock files are part of retained public source packages.
    lock = re.fullmatch(r'\.source-claims\.[a-f0-9]{64}\.human-forms\.json\.writer\.lock', name)
    if (Path(name).name != name or '\\' in name or name in FORBIDDEN
            or name.startswith('.') and not lock):
        raise _Unavailable('access-restricted', 'package-outside-public-metadata')


class _ReadSnapshot:
    """Protected-path observations around the existing byte/package verifiers.

Preflight reserves the aggregate budget before package IO. A same-UID writer
may race that preflight; existing per-file/package limits still bound the read,
and changed metadata is refused before returning any record. Same-UID hostile
code remains outside the existing local-owner filesystem threat model.
"""
    def __init__(self):
        self.bytes = 0
        self.observed = {}

    def mark(self, path, *, directory=False):
        descriptor = source._owned_path(path, directory=directory)
        try:
            info = os.fstat(descriptor)
        finally:
            os.close(descriptor)
        value = (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_size,
                 info.st_mtime_ns, info.st_ctime_ns)
        key = (path, directory)
        if key in self.observed and self.observed[key] != value:
            raise source.JournalConflict('source changed during exact-version read')
        self.observed[key] = value
        return info

    def reserve(self, size):
        if self.bytes + size > MAX_TOTAL_BYTES:
            raise _Unavailable('over-budget', 'total-read-byte-budget')
        self.bytes += size

    def read(self, path, limit, reason):
        info = self.mark(path)
        if info.st_size > limit:
            raise _Unavailable('over-budget', reason)
        self.reserve(info.st_size)
        raw = source._read(path, limit)
        self.mark(path)
        return raw

    def package(self, directory, *, archive=False):
        self.mark(directory, directory=True)
        paths = []
        for path in directory.iterdir():
            if len(paths) >= packages.MAX_FILES + int(archive):
                raise _Unavailable('over-budget', 'package-file-count-budget')
            paths.append(path)
        if not paths:
            raise _Unavailable('missing', 'retained-archive-file-missing' if archive else 'source-file-missing')
        paths.sort()
        size = 0
        for path in paths:
            if archive:
                if path.name != 'manifest.json' and not re.fullmatch(r'[a-f0-9]{64}\.blob', path.name):
                    raise source.JournalCorruption('archive has a non-blob member')
            else:
                _metadata_name(path.name)
            if not stat.S_ISREG(path.lstat().st_mode):
                raise _Unavailable('access-restricted', 'package-not-flat-regular-metadata')
            info = self.mark(path)
            if info.st_size > source.MAX_SET_BYTES:
                raise _Unavailable('over-budget', 'package-file-byte-budget')
            if path.name.endswith('.writer.lock') and info.st_size:
                raise source.JournalCorruption('public form lock contains data')
            size += info.st_size
        if size > packages.MAX_PACKAGE_BYTES + (source.MAX_SET_BYTES if archive else 0):
            raise _Unavailable('over-budget', 'package-byte-budget')
        self.reserve(size)
        self.mark(directory, directory=True)

    def verify(self):
        for path, directory in tuple(self.observed):
            self.mark(path, directory=directory)


def _catalog(snapshot, root):
    raw = snapshot.read(root / CATALOG_REF, MAX_CATALOG_BYTES, 'catalog-byte-budget')
    entries, count = {}, 0
    for line, encoded in enumerate(raw.splitlines(), start=1):
        if not encoded.strip():
            continue
        count += 1
        if count > MAX_CATALOG_ROWS or len(encoded) > source.MAX_COMMAND_BYTES:
            raise _Unavailable('over-budget', 'catalog-record-budget')
        entry = source._json_object(encoded)
        candidate = entry.get('claim_id')
        if (entry.get('schema_version') != 'tos_source_witness_claim_catalog_entry_v1'
                or not isinstance(candidate, str) or not candidate or candidate in entries):
            raise source.JournalCorruption('catalog identity is invalid or duplicated')
        entries[candidate] = (entry, line)
    return entries, source._digest(raw)


def _public_records(raw):
    if len(raw) > source.MAX_COMMAND_BYTES:
        raise _Unavailable('over-budget', 'claim-stream-byte-budget')
    records = claims._claims(raw)  # Also refuses duplicate identities in siblings.
    for record in records.values():
        if record.get('visibility') not in PUBLIC:
            raise _Unavailable('access-restricted', 'claim-stream-not-public-metadata')
        if (record.get('claim_type') != 'relation' or not _ref(claims._subject(record).ref)):
            raise source.JournalCorruption('invalid versioned source Claim identity')
    return records


def _key(ref):
    return ref['id'], ref['version'], ref['digest']


def _stream_bindings(stream, relative, revision, location=None):
    common = {'source_ref': relative.as_posix(),
              'stream_sha256': source._digest(stream), 'stream_bytes': len(stream),
              'package_revision': revision,
              'archive_blob_ref': location['archive_path'] if location else None}
    return {source._json_object(row)['claim_id']: {**common, 'line': line}
            for line, row in enumerate(stream.splitlines(), start=1) if row.strip()}


def _version(record, binding, transition=None):
    return {'record': record, 'ref': claims._subject(record).ref,
            'version_status': 'historical' if binding['archive_blob_ref'] else 'current',
            'source': binding,
            'transition': {key: copy.deepcopy(transition[key]) for key in
                           ('command_id', 'recorded_at', 'previous_source', 'source', 'request_digest')} if transition else None}


class ClaimVersionReader:
    """One build's bounded catalog/package snapshots; never a process cache.

    Call `verify_current()` again immediately before publishing a build result.
    It raises on filesystem drift/restriction; it does not refresh the snapshots
    or silently rebind an earlier result to later source bytes. Concurrent calls
    on one instance are not supported.
    """
    def __init__(self, root):
        self.root = Path(root)
        if not self.root.is_absolute() or '..' in self.root.parts:
            raise ValueError('an absolute public root is required')
        self._snapshot = _ReadSnapshot()
        self._catalog_entries = None
        self._catalog_digest = None
        self._packages = {}

    def verify_current(self):
        self._snapshot.verify()

    def _package(self, relative):
        if relative in self._packages:
            return self._packages[relative]
        root, snapshot = self.root, self._snapshot
        config = {'source_root': str(root), 'source_path': relative.as_posix()}
        snapshot.package(root / relative.parent)
        snapshot.mark(root / relative)
        files = packages._package(root / relative.parent)
        records = _public_records(files[SOURCE_CLAIM_BASENAME])
        historical = {}
        if claims.HISTORY in files:
            receipts = source._json_object(files[claims.HISTORY]).get('receipts')
            if isinstance(receipts, list) and len(receipts) > packages.MAX_REVISIONS:
                raise _Unavailable('over-budget', 'correction-receipt-count-budget')

        def archive_reader(archive_root, archive_config, receipt):
            if (not _ref(receipt['previous_source']) or not _ref(receipt['source'])
                    or not isinstance(receipt['previous_revision'], str)
                    or not HASH.fullmatch(receipt['previous_revision'])):
                raise source.JournalCorruption('invalid retained exact source binding')
            bound = claims._archive_config(archive_config, receipt['previous_source']['id'])
            archive_ref = packages._archive_path(bound, receipt['previous_revision'])
            if receipt['archive_path'] != archive_ref.as_posix():
                raise source.JournalCorruption('archive locator is not source-derived')
            directory = archive_root / archive_ref
            try:
                snapshot.package(directory, archive=True)
                manifest = source._json_object(snapshot.read(directory / 'manifest.json',
                    source.MAX_SET_BYTES, 'archive-manifest-byte-budget'))
                if not isinstance(manifest['files'], dict):
                    raise source.JournalCorruption('archive file bindings are not an object')
                if not manifest['files']:
                    raise source.JournalCorruption('archive has no source file bindings')
                if len(manifest['files']) > packages.MAX_FILES:
                    raise _Unavailable('over-budget', 'archive-file-binding-count-budget')
                for name, binding in manifest['files'].items():
                    _metadata_name(name)
                    if (not isinstance(binding, dict) or not isinstance(binding.get('blob'), str)
                            or not re.fullmatch(r'[a-f0-9]{64}\.blob', binding['blob'])):
                        raise source.JournalCorruption('invalid archive blob locator')
                    snapshot.mark(directory / binding['blob'])
                # Reuse the owner's manifest/blob/package-digest verifier and exact
                # predecessor/successor reconstruction, not command configuration.
                archived, locations = claims._read_archive(archive_root, archive_config, receipt)
            except FileNotFoundError as error:
                raise _Unavailable('missing', 'retained-archive-file-missing') from error
            previous = _public_records(archived[SOURCE_CLAIM_BASENAME])
            key = _key(receipt['previous_source'])
            if key in historical:
                raise source.JournalCorruption('duplicate retained exact Claim version')
            bindings = _stream_bindings(archived[SOURCE_CLAIM_BASENAME], relative,
                                       receipt['previous_revision'], locations[SOURCE_CLAIM_BASENAME])
            historical[key] = _version(previous[key[0]], bindings[key[0]], receipt)
            return archived, locations

        try:
            history = claims._history(files, config, archive_reader=archive_reader)
        except source.JournalConflict:
            raise
        except (ValueError, TypeError, KeyError, AttributeError, RecursionError) as error:
            raise _Unavailable('corrupt', 'history-integrity-failed') from error
        revision = packages._revision(files)
        bindings = _stream_bindings(files[SOURCE_CLAIM_BASENAME], relative, revision)
        package = {
            'current': {identity: _version(record, bindings[identity])
                        for identity, record in records.items()},
            'historical': historical,
            'history': {'source_ref': (relative.parent / claims.HISTORY).as_posix(),
                        'sha256': source._digest(files[claims.HISTORY]) if claims.HISTORY in files else None,
                        'receipt_count': len(history['receipts']), 'correction_chain_verified': True},
        }
        self.verify_current()
        self._packages[relative] = package
        return package

    def resolve(self, exact_ref):
        if not _ref(exact_ref):
            raise ValueError('an exact Claim ref is required')
        exact_ref = copy.deepcopy(exact_ref)
        result = {'status': None, 'reason': None, 'exact_ref': copy.deepcopy(exact_ref),
                  'version_status': None, 'record': None, 'record_digest': None, 'provenance': None,
                  'grants_current_use': False, 'performs_assessment': False, 'writes_to_source': False}
        stage = 'catalog'
        try:
            if self._catalog_entries is None:
                os.close(source._owned_path(self.root, directory=True))
                self._catalog_entries, self._catalog_digest = _catalog(self._snapshot, self.root)
            if exact_ref['id'] not in self._catalog_entries:
                raise _Unavailable('missing', 'claim-not-in-public-catalog')
            entry, catalog_line = self._catalog_entries[exact_ref['id']]
            if entry.get('visibility') not in PUBLIC:
                raise _Unavailable('access-restricted', 'catalog-claim-not-public-metadata')
            relative = _source_path(entry.get('source_claim_file_ref'))
            stage = 'source'
            package = self._package(relative)
            current = package['current'].get(exact_ref['id'])
            if (current is None or type(entry.get('source_claim_line')) is not int
                    or entry['source_claim_line'] != current['source']['line']):
                raise _Unavailable('stale', 'catalog-source-line-mismatch')
            current_ref = current['ref']
            if (type(entry.get('claim_version')) is not int or entry['claim_version'] != current_ref['version']
                    or entry.get('claim_sha256') != current_ref['digest'].removeprefix('sha256:')
                    or entry['visibility'] != current['record']['visibility']):
                raise _Unavailable('stale', 'catalog-source-binding-mismatch')
            selected = current if current_ref == exact_ref else package['historical'].get(_key(exact_ref))
            self.verify_current()
            if selected is None:
                present_version = (exact_ref['version'] == current_ref['version'] or any(
                    key[:2] == (exact_ref['id'], exact_ref['version']) for key in package['historical']))
                raise _Unavailable('stale' if present_version else 'missing',
                    'exact-version-digest-mismatch' if present_version else 'exact-version-not-retained')
            provenance = {
                'catalog': {'source_ref': CATALOG_REF, 'line': catalog_line, 'sha256': self._catalog_digest,
                            'source_claim_file_ref': relative.as_posix(), 'source_claim_line': entry['source_claim_line'],
                            'current_record_ref': current_ref, 'visibility': entry['visibility']},
                'source': selected['source'], 'history': package['history'], 'transition': selected['transition'],
            }
            return {**result, 'status': 'available', 'reason': 'exact-' + selected['version_status'] + '-version',
                    'version_status': selected['version_status'], 'record': copy.deepcopy(selected['record']),
                    'record_digest': exact_ref['digest'], 'provenance': copy.deepcopy(provenance)}
        except _Unavailable as error:
            return {**result, 'status': error.status, 'reason': error.reason}
        except FileNotFoundError:
            return {**result, 'status': 'missing', 'reason': stage + '-file-missing'}
        except source.JournalConflict:
            return {**result, 'status': 'stale', 'reason': 'source-changed-during-read'}
        except PermissionError:
            return {**result, 'status': 'access-restricted', 'reason': stage + '-path-restricted'}
        except OSError as error:
            restricted = error.errno in {errno.ELOOP, errno.ENOTDIR, errno.EACCES, errno.EPERM}
            return {**result, 'status': 'access-restricted' if restricted else 'corrupt',
                    'reason': stage + ('-path-restricted' if restricted else '-io-error')}
        except (ValueError, TypeError, KeyError, AttributeError, RecursionError, StopIteration):
            return {**result, 'status': 'corrupt', 'reason': stage + '-integrity-failed'}


def resolve_claim_version(root, exact_ref):
    """One-shot wrapper; build callers can reuse one ClaimVersionReader instead."""
    return ClaimVersionReader(root).resolve(exact_ref)
