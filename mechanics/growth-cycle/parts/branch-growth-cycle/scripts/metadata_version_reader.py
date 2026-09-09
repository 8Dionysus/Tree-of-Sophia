"""Read exact public metadata records without command or current-use authority.

The catalog locates a current record. Its canonical digest binds that record,
and the existing revision receipts/manifests bind retained predecessors. Only
the selected record blobs are opened: unknown companion bytes are neither read
nor certified. This is a record-chain reader, not a whole-package audit, a
native payload resolver, or a historical HumanForm materializer.
"""
from __future__ import annotations

import copy
import errno
import os
from pathlib import Path
import re

from jsonschema.exceptions import SchemaError, ValidationError
from referencing import Registry, Resource
from referencing.exceptions import Unresolvable

from claim_version_reader import _ReadSnapshot, _Unavailable
import source_commands as source
import source_revisions as revisions
from source_metadata_snapshot import (PublicationSnapshot, PublicationStateError, PublicationPending, PublicationChanged)
from build_source_witness_catalog import verify_catalog_publication, CatalogBuildError
from source_record_profiles import (
    SourceRecordProfiles, REGISTRY_REF, CONTRACT_REF, CORPUS_REF,
)

CATALOG_ROOT = 'ToS/source-witnesses/catalog/'
MAX_CATALOG_BYTES = 8 * 1024 * 1024
MAX_CATALOG_ROWS = 8192
MAX_TOTAL_BYTES = 64 * 1024 * 1024
MAX_CONTRACTS = 128
MAX_SAFE_VERSION = 9_007_199_254_740_991
NATIVE_CATALOGS = {'agent': 'agents.jsonl', 'place': 'places.jsonl',
                   'organization': 'organizations.jsonl', 'work': 'works.jsonl',
                   'expression': 'expressions.jsonl'}
PUBLIC = {'public', 'public_metadata_only'}
FORBIDDEN = {'catalog', 'payload', 'private', 'local-content', 'owner-local'}
IDENTITY = re.compile(r'tos\.([a-z][a-z0-9-]*)\.[a-z0-9]+(?:[.-][a-z0-9]+)*')
HASH = re.compile(r'sha256:[a-f0-9]{64}')
SCHEMA_REF = re.compile(r'ToS/contracts/[a-z][a-z0-9-]*\.schema\.json')


def _identity(value):
    match = IDENTITY.fullmatch(value) if isinstance(value, str) else None
    return match.group(1) if match and match.group(1) != 'claim' else None


def _ref(value):
    return (isinstance(value, dict) and set(value) == {'id', 'version', 'digest'}
            and _identity(value['id']) is not None
            and type(value['version']) is int and 1 <= value['version'] <= MAX_SAFE_VERSION
            and isinstance(value['digest'], str) and HASH.fullmatch(value['digest']))


def _record_ref(record):
    value = {'id': record.get('record_id'), 'version': record.get('record_version'),
             'digest': source._digest(source._canonical(record))}
    if not _ref(value):
        raise source.JournalCorruption('invalid exact metadata identity')
    return value


def _key(reference):
    return reference['id'], reference['version'], reference['digest']


def _source_path(value, basename):
    if not isinstance(value, str):
        raise source.JournalCorruption('metadata source locator is not a string')
    path = Path(value)
    if (path.is_absolute() or path.as_posix() != value or '\\' in value or '\x00' in value
            or path.parts[:2] != ('ToS', 'source-witnesses') or len(path.parts) < 5
            or path.name != basename
            or any(part in FORBIDDEN or part.startswith('.') for part in path.parts)):
        raise _Unavailable('access-restricted', 'source-outside-public-metadata')
    return path


class _Snapshot(_ReadSnapshot):
    def reserve(self, size):
        if self.bytes + size > MAX_TOTAL_BYTES:
            raise _Unavailable('over-budget', 'total-read-byte-budget')
        self.bytes += size


class MetadataVersionReader:
    """One bounded build snapshot; reuse, then verify immediately before export.

    ``resolve`` returns the same evidence/status envelope as ClaimVersionReader.
    ``exact_refs`` returns {status, reason, record_id, current_ref, refs,
    provenance, grants_current_use, performs_assessment, writes_to_source}.
    Refs run from the retained baseline (which need not be version 1) through
    current. Unavailability returns no current_ref, refs or provenance. Neither
    operation enumerates source/private trees or accepts a caller-supplied path.
    Instances are not concurrent-reader objects or persistent process caches.
    """
    def __init__(self, root):
        self.root = Path(root)
        if not self.root.is_absolute() or '..' in self.root.parts:
            raise ValueError('an absolute public source root is required')
        self._snapshot = _Snapshot()
        self._contracts = {}
        self._profiles = None
        self._catalogs = {}
        self._records = {}
        self._native_validator = None
        self._publication_error = None
        try:
            self._publication = PublicationSnapshot(self.root)
        except PublicationStateError as error:
            self._publication_error = error
        self._catalog_manifest = None

    def verify_current(self):
        self._verify_publication()
        self._snapshot.verify()

    def _verify_publication(self):
        if self._publication_error is not None:
            raise self._publication_error
        self._publication.verify_current()

    def _contract(self, ref):
        if ref not in {REGISTRY_REF, CONTRACT_REF} and not SCHEMA_REF.fullmatch(ref):
            raise source.JournalCorruption('metadata schema is not a declared local contract')
        if ref not in self._contracts:
            if len(self._contracts) >= MAX_CONTRACTS:
                raise _Unavailable('over-budget', 'contract-count-budget')
            raw = self._snapshot.read(self.root / ref, source.MAX_COMMAND_BYTES, 'contract-byte-budget')
            self._contracts[ref] = raw
        return self._contracts[ref]

    def _check_profile_inputs(self, profiles, previous):
        # The owner shape reader uses its own bounded JSON loader. Preflight
        # every selected public contract first, count its second read, then
        # compare exact bytes; never monkeypatch its reader or call validate().
        for ref, digest in profiles.input_digests.items():
            raw = self._contracts.get(ref)
            if raw is None:
                raise source.JournalCorruption('profile reader consumed an undeclared contract')
            if ref not in previous:
                self._snapshot.reserve(len(raw))
            if source._digest(raw) != 'sha256:' + digest:
                raise source.JournalConflict('metadata profile changed during inspection')

    def _profile_contract(self):
        if self._profiles is None:
            for ref in (REGISTRY_REF, CONTRACT_REF):
                self._contract(ref)
            profiles = SourceRecordProfiles(self.root)
            self._check_profile_inputs(profiles, set())
            self._profiles = profiles
        return self._profiles

    def _route(self, kind):
        self._verify_publication()
        if kind in NATIVE_CATALOGS:
            return {'record_type': kind, 'id_prefix': 'tos.' + kind + '.',
                    'source_basename': kind + '.json', 'catalog_filename': NATIVE_CATALOGS[kind],
                    'adapter': 'native-corpus', 'profile_type_id': None}
        profiles = self._profile_contract()
        profile = profiles.profiles.get(kind)
        if profile is None:
            raise _Unavailable('access-restricted', 'metadata-family-not-supported')
        entry = next(entry for entry in profiles.registry['types']
                     if entry.get('source_record_profile') == profile)
        return {key: profile[key] for key in ('record_type', 'id_prefix', 'source_basename', 'catalog_filename')} | {
            'adapter': 'declared-profile', 'profile_type_id': entry['type_id']}

    def supports(self, record_type, *, source_ref=None):
        """Checked family routing, optionally excluding a native basename alias.

        A retained Composite catalog may contain both native witness and declared
        metadata shapes. Pass its source_ref to select the declared basename;
        this checks a locator without opening the referenced source file.
        """
        if not isinstance(record_type, str) or not re.fullmatch(r'[a-z][a-z0-9-]*', record_type):
            return False
        try:
            route = self._route(record_type)
            if source_ref is not None:
                _source_path(source_ref, route['source_basename'])
        except _Unavailable as error:
            if error.status == 'access-restricted':
                self.verify_current()
                return False
            raise
        self.verify_current()
        return True

    def _catalog(self, route):
        self.verify_current()
        ref = CATALOG_ROOT + route['catalog_filename']
        if ref in self._catalogs:
            return self._catalogs[ref]
        raw = self._snapshot.read(self.root / ref, MAX_CATALOG_BYTES, 'catalog-byte-budget')
        if self._publication.token is not None or os.path.lexists(self.root / CATALOG_ROOT / 'catalog.manifest.json'):
            if self._catalog_manifest is None:
                self._catalog_manifest = source._json_object(self._snapshot.read(
                    self.root / CATALOG_ROOT / 'catalog.manifest.json', source.MAX_COMMAND_BYTES,
                    'catalog-manifest-byte-budget'))
            verify_catalog_publication(self._catalog_manifest, self._publication.token,
                                       {ref: source._digest(raw)[7:]})
        entries = {}
        for line, encoded in enumerate(raw.splitlines(), start=1):
            if not encoded.strip():
                continue
            if len(entries) >= MAX_CATALOG_ROWS or len(encoded) > source.MAX_COMMAND_BYTES:
                raise _Unavailable('over-budget', 'catalog-record-budget')
            entry = source._json_object(encoded)
            identity = entry.get('record_id')
            if (entry.get('schema_version') != 'tos_source_witness_catalog_entry_v1'
                    or _identity(identity) != route['record_type'] or identity in entries
                    or entry.get('record_type') != route['record_type']):
                raise source.JournalCorruption('metadata catalog identity is invalid or duplicated')
            entries[identity] = (entry, line)
        value = entries, ref, source._digest(raw)
        self._catalogs[ref] = value
        return value

    def _public_record(self, record, route, schema_version=None):
        reference = _record_ref(record)
        if (record.get('record_type') != route['record_type']
                or not reference['id'].startswith(route['id_prefix'])
                or not isinstance(record.get('schema_version'), str)
                or schema_version is not None and record['schema_version'] != schema_version):
            raise source.JournalCorruption('metadata descriptor changed within the record chain')
        if route['adapter'] == 'native-corpus':
            if record['schema_version'] != 'tos_corpus_record_v1' or 'visibility' in record:
                raise _Unavailable('access-restricted', 'source-outside-native-corpus-public-contract')
        elif record.get('visibility') not in PUBLIC:
            raise _Unavailable('access-restricted', 'metadata-record-not-public')
        return reference

    def _validate_current(self, record, route, relative):
        self._public_record(record, route)
        if route['adapter'] == 'native-corpus':
            if self._native_validator is None:
                schema = source._json_object(self._contract(CORPUS_REF))
                if schema.get('$id') not in {'https://tree-of-sophia.local/' + CORPUS_REF,
                                             'https://treeofsophia.local/' + CORPUS_REF}:
                    raise source.JournalCorruption('native Corpus schema identity differs')
                source.Draft202012Validator.check_schema(schema)
                registry = Registry().with_resource(schema['$id'], Resource.from_contents(schema))
                self._native_validator = source.Draft202012Validator(schema, registry=registry,
                    format_checker=source.FormatChecker())
            self._native_validator.validate(record)
            return CORPUS_REF
        profiles = self._profile_contract()
        schema_route = profiles.schema_routes.get((route['record_type'], record['schema_version']))
        if schema_route is None:
            raise _Unavailable('access-restricted', 'metadata-source-schema-not-supported')
        for ref in dict.fromkeys([CORPUS_REF, *schema_route['schema_dependencies'], schema_route['schema_ref']]):
            self._contract(ref)
        previous = set(profiles.input_digests)
        profiles.validate_path(route['record_type'], relative.as_posix())
        # Shape only. validate() additionally resolves native identity/binding
        # sources and is intentionally outside this read-only evidence route.
        profiles._validate_shape(route['record_type'], record)
        self._check_profile_inputs(profiles, previous)
        return schema_route['schema_ref']

    def _archive_record(self, relative, route, receipt, schema_version):
        before = receipt['previous_source']
        revision = receipt['previous_revision']
        if not _ref(before) or not isinstance(revision, str) or not HASH.fullmatch(revision):
            raise source.JournalCorruption('invalid retained metadata reference')
        archive = revisions._archive_path({'record_id': before['id']}, revision)
        if receipt['archive_path'] != archive.as_posix():
            raise source.JournalCorruption('archive locator is not derived from exact metadata identity')
        directory = self.root / archive
        try:
            self._snapshot.mark(directory, directory=True)
            raw_manifest = self._snapshot.read(directory / 'manifest.json', source.MAX_SET_BYTES,
                                               'archive-manifest-byte-budget')
            manifest = source._json_object(raw_manifest)
            selected_scope = manifest.get('schema_version') == 'tos_source_package_archive_v2'
            source._keys(manifest, {'schema_version', 'source_path', 'source', 'revision', 'files'}
                         | ({'publication_protocol'} if selected_scope else set()))
            if (manifest['schema_version'] not in {'tos_source_package_archive_v1', 'tos_source_package_archive_v2'}
                    or selected_scope and manifest['publication_protocol'] != revisions.SELECTED_PROTOCOL
                    or manifest['source_path'] != relative.as_posix() or not _ref(manifest['source'])
                    or manifest['source'] != before or manifest['revision'] != revision
                    or not isinstance(manifest['files'], dict) or not manifest['files']):
                raise source.JournalCorruption('archive manifest does not bind this metadata revision')
            if selected_scope and (not set(manifest['files']) <= set(revisions._selected_names(relative))
                                   or relative.name not in manifest['files']):
                raise source.JournalCorruption('archive exceeds the selected metadata scope')
            if len(manifest['files']) > revisions.MAX_FILES:
                raise _Unavailable('over-budget', 'archive-file-binding-count-budget')
            bindings, total = {}, 0
            for name, binding in manifest['files'].items():
                source._keys(binding, {'blob', 'sha256', 'bytes'})
                if (not isinstance(name, str) or not name or name in {'.', '..'}
                        or Path(name).name != name or '\\' in name or '\x00' in name
                        or not isinstance(binding['sha256'], str) or not HASH.fullmatch(binding['sha256'])
                        or binding['blob'] != binding['sha256'][7:] + '.blob'
                        or type(binding['bytes']) is not int or binding['bytes'] < 0):
                    raise source.JournalCorruption('invalid archive manifest byte binding')
                if binding['bytes'] > source.MAX_SET_BYTES:
                    raise _Unavailable('over-budget', 'archive-declared-file-byte-budget')
                total += binding['bytes']
                bindings[name] = {key: binding[key] for key in ('sha256', 'bytes')}
            if total > revisions.MAX_PACKAGE_BYTES:
                raise _Unavailable('over-budget', 'archive-declared-package-byte-budget')
            if source._digest(source._canonical(bindings)) != revision:
                raise source.JournalCorruption('archive manifest package binding is invalid')
            selected = manifest['files'][relative.name]
            # Deliberately do not open, stat or disclose other blob contents.
            raw = self._snapshot.read(directory / selected['blob'], source.MAX_COMMAND_BYTES,
                                      'metadata-record-byte-budget')
            if source._digest(raw) != selected['sha256'] or len(raw) != selected['bytes']:
                raise source.JournalCorruption('selected archive record byte binding differs')
            record = source._json_object(raw)
            if self._public_record(record, route, schema_version) != before:
                raise source.JournalCorruption('archive does not preserve the exact previous metadata record')
        except FileNotFoundError as error:
            raise _Unavailable('missing', 'retained-record-file-missing') from error
        return record, {
            'source_ref': relative.as_posix(), 'record_bytes': len(raw), 'record_sha256': source._digest(raw),
            'archive_blob_ref': (archive / selected['blob']).as_posix(), 'package_revision': revision,
            'archive_manifest_ref': (archive / 'manifest.json').as_posix(),
            'archive_manifest_sha256': source._digest(raw_manifest),
        }

    def _load(self, identity):
        if identity in self._records:
            return self._records[identity]
        route = self._route(_identity(identity))
        entries, catalog_ref, catalog_digest = self._catalog(route)
        if identity not in entries:
            raise _Unavailable('missing', 'record-not-in-public-catalog')
        entry, line = entries[identity]
        relative = _source_path(entry.get('source_record_ref'), route['source_basename'])
        path = self.root / relative
        self._snapshot.mark(path.parent, directory=True)
        raw = self._snapshot.read(path, source.MAX_COMMAND_BYTES, 'metadata-record-byte-budget')
        record = source._json_object(raw)
        schema_ref = self._validate_current(record, route, relative)
        current_ref = _record_ref(record)
        if (record['record_id'] != identity or entry.get('record_sha256') != current_ref['digest'][7:]
                or entry.get('preferred_label') != record.get('preferred_label')
                or entry.get('identity_status') != record.get('identity_status')
                or entry.get('source_schema_ref', schema_ref) != schema_ref
                or route['adapter'] == 'declared-profile' and 'source_schema_ref' not in entry):
            raise _Unavailable('stale', 'catalog-source-binding-mismatch')
        history_path = path.parent / revisions.HISTORY
        history_raw = (self._snapshot.read(history_path, source.MAX_SET_BYTES, 'revision-history-byte-budget')
                       if os.path.lexists(history_path) else None)
        files = {relative.name: raw, **({revisions.HISTORY: history_raw} if history_raw is not None else {})}
        if history_raw is not None:
            receipts = source._json_object(history_raw).get('receipts')
            if isinstance(receipts, list) and len(receipts) > revisions.MAX_REVISIONS:
                raise _Unavailable('over-budget', 'correction-receipt-count-budget')
        history = revisions._history(files, record)
        versions = {}
        allowed_fields = source.CORPUS_REVISION_FIELDS if route['adapter'] == 'native-corpus' else source.REVISION_FIELDS
        for receipt in history['receipts']:
            request = receipt['request']
            selected = 'publication' in receipt
            attachment = request.get('operation') == 'expression.responsibility.attach'
            compound = request.get('operation') == 'work.expression.create' or attachment
            if compound:
                if attachment:
                    from source_responsibility_commands import validate_parent_receipt
                else:
                    from source_expression_commands import validate_parent_receipt
                validate_parent_receipt(receipt)
                if route['record_type'] != ('expression' if attachment else 'work'):
                    raise source.JournalCorruption('compound history must belong to its declared existing parent')
            else:
                source._keys(request, {'schema_version', 'operation', 'fields', 'forms', 'reason', 'command_id',
                                  'expected_configuration', 'expected_source', 'expected_revision', 'expected_dependencies'}
                         | ({'expected_publication'} if selected else set()))
            if selected and receipt['publication']['selected_files'] != sorted(revisions._selected_names(relative)):
                raise source.JournalCorruption('retained publication selected another metadata unit')
            if (not compound and (request['schema_version'] != 'tos_local_source_command_v1'
                                  or request['operation'] != 'record.revise')
                    or not _ref(receipt['source']) or not _ref(request['expected_source'])
                    or not isinstance(request['fields'], dict) or not request['fields']
                    or not set(request['fields']) <= ({'responsibility_claim_refs'} if attachment
                        else {'expression_claim_refs'} if compound else allowed_fields)):
                raise source.JournalCorruption('retained request is not a metadata correction')
            previous, binding = self._archive_record(relative, route, receipt, record['schema_version'])
            revised = {**previous, **request['fields'], 'record_version': previous['record_version'] + 1}
            if self._public_record(revised, route, record['schema_version']) != receipt['source']:
                raise source.JournalCorruption('retained request does not reconstruct its metadata successor')
            versions[_key(receipt['previous_source'])] = {
                'record': previous, 'ref': receipt['previous_source'], 'source': binding, 'version_status': 'historical',
                'transition': {key: copy.deepcopy(receipt[key]) for key in
                               ('command_id', 'recorded_at', 'previous_source', 'source', 'request_digest')},
            }
        current = {'record': record, 'ref': current_ref, 'version_status': 'current', 'transition': None,
                   'source': {'source_ref': relative.as_posix(), 'record_bytes': len(raw),
                              'record_sha256': source._digest(raw), 'archive_blob_ref': None,
                              'archive_manifest_ref': None, 'archive_manifest_sha256': None, 'package_revision': None}}
        versions[_key(current_ref)] = current
        provenance = {
            'verification_scope': 'selected-record-chain', 'all_package_bytes_verified': False,
            'catalog': {'source_ref': catalog_ref, 'line': line, 'sha256': catalog_digest,
                        'source_record_ref': relative.as_posix(), 'current_record_ref': current_ref},
            'descriptor': {'adapter': route['adapter'], 'record_type': route['record_type'],
                           'profile_type_id': route['profile_type_id'], 'source_schema_ref': schema_ref,
                           'source_schema_version': record['schema_version'], 'source_scope': 'public_metadata_only'},
            'history': {'source_ref': history_path.relative_to(self.root).as_posix() if history_raw is not None else None,
                        'sha256': source._digest(history_raw) if history_raw is not None else None,
                        'receipt_count': len(history['receipts']), 'retained_record_chain_verified': True,
                        'retained_baseline_ref': next(iter(versions.values()))['ref']},
        }
        self.verify_current()
        result = {'current': current, 'versions': versions, 'provenance': provenance}
        self._records[identity] = result
        return result

    def _error(self, error):
        # An unavailable result is still a statement about this read epoch.
        # A newly added subject must not be reported absent from an old catalog.
        try:
            self.verify_current()
        except (_Unavailable, OSError, ValueError) as changed:
            error = changed
        if isinstance(error, _Unavailable):
            return error.status, error.reason
        if isinstance(error, FileNotFoundError):
            return 'missing', 'metadata-input-file-missing'
        if isinstance(error, source.JournalConflict):
            return 'stale', 'source-changed-during-read'
        if isinstance(error, PublicationPending):
            return 'stale', 'source-publication-pending'
        if isinstance(error, PublicationChanged):
            return 'stale', 'source-publication-changed'
        if isinstance(error, PermissionError):
            return 'access-restricted', 'metadata-path-restricted'
        if isinstance(error, OSError):
            restricted = error.errno in {errno.ELOOP, errno.ENOTDIR, errno.EACCES, errno.EPERM}
            return ('access-restricted', 'metadata-path-restricted') if restricted else ('corrupt', 'metadata-io-error')
        return 'corrupt', 'metadata-integrity-failed'

    def resolve(self, exact_ref):
        if not _ref(exact_ref):
            raise ValueError('an exact metadata record ref is required')
        exact_ref = copy.deepcopy(exact_ref)
        result = {'status': None, 'reason': None, 'exact_ref': exact_ref,
                  'version_status': None, 'record': None, 'record_digest': None, 'provenance': None,
                  'grants_current_use': False, 'performs_assessment': False, 'writes_to_source': False}
        try:
            package = self._load(exact_ref['id'])
            selected = package['versions'].get(_key(exact_ref))
            self.verify_current()
            if selected is None:
                present = any(key[:2] == (exact_ref['id'], exact_ref['version']) for key in package['versions'])
                raise _Unavailable('stale' if present else 'missing',
                    'exact-version-digest-mismatch' if present else 'exact-version-not-retained')
            return {**result, 'status': 'available', 'reason': 'exact-' + selected['version_status'] + '-version',
                    'version_status': selected['version_status'], 'record': copy.deepcopy(selected['record']),
                    'record_digest': exact_ref['digest'], 'provenance': copy.deepcopy({**package['provenance'],
                        'source': selected['source'], 'transition': selected['transition']})}
        except (_Unavailable, OSError, ValueError, CatalogBuildError, TypeError, KeyError, AttributeError, RecursionError,
                Unresolvable, SchemaError, ValidationError) as error:
            status, reason = self._error(error)
            return {**result, 'status': status, 'reason': reason}

    def exact_refs(self, record_id):
        if _identity(record_id) is None:
            raise ValueError('a metadata record identity is required')
        result = {'status': None, 'reason': None, 'record_id': record_id, 'current_ref': None,
                  'refs': [], 'provenance': None,
                  'grants_current_use': False, 'performs_assessment': False, 'writes_to_source': False}
        try:
            package = self._load(record_id)
            self.verify_current()
            return {**result, 'status': 'available', 'reason': 'verified-record-references',
                    'current_ref': copy.deepcopy(package['current']['ref']),
                    'refs': [copy.deepcopy(value['ref']) for value in package['versions'].values()],
                    'provenance': copy.deepcopy(package['provenance'])}
        except (_Unavailable, OSError, ValueError, CatalogBuildError, TypeError, KeyError, AttributeError, RecursionError,
                Unresolvable, SchemaError, ValidationError) as error:
            status, reason = self._error(error)
            return {**result, 'status': status, 'reason': reason}

    def resolve_source_bytes(self, original_source_path, raw_sha256):
        """Verify exact current/retained bytes at their original logical source.

        This is a source-provenance join, not a blob search. Only the supported
        public typed metadata route and its committed record lineage may
        resolve the raw digest. Returned JSON contains the verified record and
        its byte provenance, not arbitrary file contents or a latest fallback.
        """
        if (not isinstance(original_source_path, str) or not isinstance(raw_sha256, str)
                or not re.fullmatch(r'[a-f0-9]{64}', raw_sha256)):
            raise ValueError('exact logical metadata path and raw SHA-256 are required')
        result = {'status': None, 'reason': None, 'source_path': original_source_path,
                  'requested_sha256': raw_sha256, 'exact_ref': None, 'record': None,
                  'provenance': None, 'grants_current_use': False,
                  'performs_assessment': False, 'writes_to_source': False}
        try:
            relative = Path(original_source_path)
            route = self._route(relative.stem)
            _source_path(original_source_path, route['source_basename'])
            entries, _, _ = self._catalog(route)
            identities = [identity for identity, (entry, _) in entries.items()
                          if entry.get('source_record_ref') == original_source_path]
            if len(identities) != 1:
                raise _Unavailable('missing' if not identities else 'corrupt', 'logical-source-not-unique-in-catalog')
            package = self._load(identities[0])
            candidates = [value for value in package['versions'].values()
                          if value['source']['source_ref'] == original_source_path
                          and value['source']['record_sha256'] == 'sha256:' + raw_sha256]
            self.verify_current()
            if not candidates:
                raise _Unavailable('missing', 'exact-source-bytes-not-retained')
            if len(candidates) != 1:
                raise source.JournalCorruption('one raw source digest has conflicting retained identities')
            selected = candidates[0]
            return {**result, 'status': 'available', 'reason': 'exact-' + selected['version_status'] + '-source-bytes',
                    'exact_ref': copy.deepcopy(selected['ref']), 'record': copy.deepcopy(selected['record']),
                    'provenance': copy.deepcopy({**package['provenance'], 'source': selected['source'],
                                                'transition': selected['transition']})}
        except (_Unavailable, OSError, ValueError, CatalogBuildError, TypeError, KeyError, AttributeError, RecursionError,
                Unresolvable, SchemaError, ValidationError) as error:
            status, reason = self._error(error)
            return {**result, 'status': status, 'reason': reason}


def resolve_metadata_version(root, exact_ref):
    """One-shot helper; multi-reference builds should reuse the snapshot object."""
    return MetadataVersionReader(root).resolve(exact_ref)
