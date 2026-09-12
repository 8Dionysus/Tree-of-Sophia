"""Explicit addressed catalog bootstrap and one selected Agent transition.

Source records own meaning. This module verifies catalog/source mechanics and
returns unselected immutable candidates; it does not publish roots, change
source records, admit Claims, or establish source-reference dependency closure.
"""
from __future__ import annotations

from dataclasses import dataclass
import hashlib
from pathlib import Path
import re
import sys
import tempfile

from jsonschema import Draft202012Validator, FormatChecker

import build_source_witness_catalog as legacy
from source_metadata_snapshot import PublicationSnapshot, _read_owned
from source_record_profiles import SourceRecordProfiles, REGISTRY_REF, CONTRACT_REF, CORPUS_REF

EXECUTION_ROOT = Path(__file__).resolve().parents[1]
for _directory in (EXECUTION_ROOT / 'access/src',
                   EXECUTION_ROOT / 'mechanics/growth-cycle/parts/branch-growth-cycle/scripts'):
    if str(_directory) not in sys.path:
        sys.path.insert(0, str(_directory))

from tos_access.projection_store import (
    Collection, ProjectionReader, ProjectionStoreError, canonical_bytes, write_projection,
    _strict_json, MAX_ROOT_BYTES, MAX_PART_BYTES, DEFAULT_PART_BYTES,
)
from tos_access.projection_mutation import (
    ProjectionSnapshotView, ProjectionChange, ProjectionHeaderChange, MutationLimits,
    stage_projection_snapshot_changes, _SnapshotMutationReader, _Budget, _install,
)
import source_commands as source
import source_revisions as revisions
import source_selected_revisions as selected
import source_metadata_transactions as transactions

SCHEMA = 'tos_source_catalog_projection_v2'
PROFILE = 'tos.source-catalog.public-records.v2'
CONTRACT = 'ToS/contracts/source-catalog-projection-v2.schema.json'
PUBLICATION_PROTOCOL = 'tos_selected_source_metadata_v1'
IDENTITY = re.compile(r'tos\.([a-z][a-z0-9-]*)\.[a-z0-9]+(?:[.-][a-z0-9]+)*\Z')
HASH = re.compile(r'[a-f0-9]{64}\Z')
EXECUTION_REFS = (
    CONTRACT, 'scripts/source_catalog_projection.py', 'scripts/build_source_witness_catalog.py',
    'scripts/source_record_profiles.py', 'scripts/source_metadata_snapshot.py',
    'scripts/source_identity_proposals.py', 'scripts/source_document_catalogue.py',
    'scripts/source_owner_context.py', 'scripts/native_text_binding.py',
    'access/src/tos_access/projection_store.py', 'access/src/tos_access/projection_diff.py',
    'access/src/tos_access/projection_mutation.py',
)


class SourceCatalogError(ValueError):
    """No complete source-bound catalog candidate was established."""


class SourceCatalogBudgetExceeded(SourceCatalogError):
    pass


class SourceCatalogRequiresBootstrap(SourceCatalogError):
    pass


@dataclass(frozen=True)
class CatalogLimits:
    max_records: int = 8192
    max_claims: int = 65536
    max_source_files: int = 16384
    max_input_bytes: int = 64 * 1024 * 1024
    max_read_bytes: int = 256 * 1024 * 1024
    max_record_bytes: int = 1024 * 1024
    max_catalog_bytes: int = 64 * 1024 * 1024
    max_installed_parts: int = 8192
    max_installed_bytes: int = 64 * 1024 * 1024

    def __post_init__(self):
        if any(type(value) is not int or value < 0 for value in vars(self).values()):
            raise ValueError('catalog limits must be nonnegative integers')


def _sha(raw):
    return hashlib.sha256(raw).hexdigest()


def _digest(value):
    if not isinstance(value, str) or HASH.fullmatch(value) is None:
        raise SourceCatalogError('an exact lowercase root or byte SHA-256 is required')
    return value


def _copy(value):
    return _strict_json(canonical_bytes(value))


def _source_ref(ref):
    path = Path(ref) if isinstance(ref, str) else Path()
    if (not isinstance(ref, str) or path.is_absolute() or path.as_posix() != ref
            or '\\' in ref or '\x00' in ref or len(path.parts) < 4
            or path.parts[:2] != ('ToS', 'source-witnesses')
            or any(part.startswith('.') or part in {'catalog', 'payload', 'private', 'owner-local', 'local-content'}
                   for part in path.parts)):
        raise SourceCatalogError('source locator is outside exact public metadata')
    return path


def _schema(definition):
    raw = _read_owned(EXECUTION_ROOT / CONTRACT, 1024 * 1024)
    schema = _strict_json(raw)
    if schema.get('$id') != 'https://tree-of-sophia.local/' + CONTRACT:
        raise SourceCatalogError('addressed catalog schema has another owner identity')
    Draft202012Validator.check_schema(schema)
    return Draft202012Validator({'$defs': schema['$defs'], '$ref': '#/$defs/' + definition},
                               format_checker=FormatChecker())


class _Capture:
    """Protected exact-byte observations, with explicit aggregate refusal bounds."""
    def __init__(self, root, limits):
        self.root, self.limits = Path(root), limits
        self.observed = {}
        self.input_bytes = self.read_bytes = 0

    def read(self, ref, limit=None):
        path = Path(ref)
        if path.is_absolute() or path.as_posix() != ref or '..' in path.parts:
            raise SourceCatalogError('input must be an exact repository-relative path')
        cap = self.limits.max_record_bytes if limit is None else limit
        remaining = self.limits.max_read_bytes - self.read_bytes
        if remaining < 0:
            raise SourceCatalogBudgetExceeded('catalog aggregate read budget exceeded')
        raw = _read_owned(self.root / path, min(cap, remaining))
        self.read_bytes += len(raw)
        binding = {'sha256': _sha(raw), 'bytes': len(raw)}
        if ref in self.observed and self.observed[ref] != binding:
            raise SourceCatalogError('observed source or contract bytes changed')
        if ref not in self.observed:
            if (len(self.observed) >= self.limits.max_source_files
                    or self.input_bytes + len(raw) > self.limits.max_input_bytes):
                raise SourceCatalogBudgetExceeded('catalog unique input budget exceeded')
            self.input_bytes += len(raw)
            self.observed[ref] = binding
        return raw

    def verify(self):
        for ref, binding in list(self.observed.items()):
            self.read(ref, binding['bytes'])


@dataclass(frozen=True)
class CatalogRecord:
    """Detached addressed row; no catalog-wide revision in record provenance."""
    row_bytes: bytes
    catalog_namespace: str

    @property
    def entry(self):
        return _strict_json(self.row_bytes)['entry']

    @property
    def source(self):
        return _strict_json(self.row_bytes)['source']

    @property
    def row_sha256(self):
        return _sha(self.row_bytes)

    @property
    def provenance(self):
        row = _strict_json(self.row_bytes)
        return {'schema_version': 'tos_source_catalog_address_v2',
                'catalog_namespace': self.catalog_namespace, 'profile_id': PROFILE,
                'record_key': row['record_id'], 'row_sha256': self.row_sha256,
                **row['source']}


class SourceCatalogSnapshot:
    """Budgeted addressed access to explicit immutable catalog root bytes.

    The caller supplies exact root binding and independent baseline trust.
    No root pathname, legacy JSONL, source body, selected publication or global
    membership is read here. A missing row means absent in this snapshot only.
    """
    def __init__(self, view: ProjectionSnapshotView, *, expected_root_sha256: str,
                 trusted_baseline_sha256: str, limits: MutationLimits | None = None):
        if not isinstance(view, ProjectionSnapshotView):
            raise TypeError('an explicit immutable ProjectionSnapshotView is required')
        if _digest(expected_root_sha256) != _digest(trusted_baseline_sha256):
            raise SourceCatalogError('catalog trust and selected immutable binding differ')
        if limits is not None and not isinstance(limits, MutationLimits):
            raise TypeError('catalog read limits must be MutationLimits')
        self.view = view
        self._reader = _SnapshotMutationReader(view, expected_root_sha256, _Budget(limits or MutationLimits()))
        manifest = self._reader.manifest
        header = manifest['header']
        _schema('header').validate(header)
        if (manifest['logical_schema'] != SCHEMA or set(manifest['collections']) != {'records'}
                or manifest['collections']['records']['key_field'] != 'record_id'
                or manifest['collections']['records']['order_fields'] != ['record_id']
                or manifest['collections']['records']['root']['count'] != header['record_count']):
            raise SourceCatalogError('catalog collection identity or declared count differs')
        if (not {REGISTRY_REF, CONTRACT_REF} <= header['profile_bindings']['source'].keys()
                or set(header['profile_bindings']['execution']) != set(EXECUTION_REFS)
                or header['legacy_baseline']['files'].get(str(legacy.MANIFEST_PATH), {}).get('sha256')
                   != header['legacy_baseline']['manifest_sha256']):
            raise SourceCatalogError('catalog baseline or required profile bindings are incomplete')
        self._header_bytes = canonical_bytes(header)
        self._row_validator = _schema('row')

    @property
    def root_sha256(self):
        return self.view.snapshot_digest

    @property
    def header(self):
        return _strict_json(self._header_bytes)

    @property
    def accounting(self):
        return dict(self._reader.budget.usage)

    def lookup(self, record_id):
        if not isinstance(record_id, str) or IDENTITY.fullmatch(record_id) is None:
            raise SourceCatalogError('catalog lookup requires a stable record identity')
        descriptor = self._reader.manifest['collections']['records']['root']
        prefix, hashed = '', _sha(record_id.encode('utf-8'))
        while descriptor['kind'] == 'index':
            digit = hashed[len(prefix)]
            children = self._reader._children(descriptor, prefix)
            if digit not in children:
                return None
            descriptor, prefix = children[digit], prefix + digit
        rows = dict(self._reader._rows('records', descriptor, prefix))
        if record_id not in rows:
            return None
        row = rows[record_id]
        self._row_validator.validate(row)
        entry, binding = row['entry'], row['source']
        if (entry['record_id'] != record_id or binding['record_ref']['id'] != record_id
                or entry['record_type'] != IDENTITY.fullmatch(record_id).group(1)
                or entry['record_type'] not in self.header['record_families']
                or entry['source_record_ref'] != binding['source_ref']
                or 'sha256:' + entry['record_sha256'] != binding['record_ref']['digest']):
            raise SourceCatalogError('catalog addressed row identities or source bindings differ')
        _source_ref(binding['source_ref'])
        return CatalogRecord(canonical_bytes(row), self.header['catalog_namespace'])

    def get(self, record_id):
        row = self.lookup(record_id)
        if row is None:
            raise SourceCatalogError('record is absent from this explicit catalog snapshot')
        return row

    def require_current(self):
        raise SourceCatalogError('immutable catalog access does not assert live source or selection currentness')


@dataclass(frozen=True)
class SourceCatalogCandidate:
    namespace_path: Path
    root_bytes: bytes
    before_root_sha256: str | None
    created_parts: tuple[str, ...]
    verification_bytes: bytes
    published: bool = False
    establishes_epoch: bool = False
    target_closure_verified: bool = False
    source_reference_closure_verified: bool = False

    @property
    def root_sha256(self):
        return _sha(self.root_bytes)

    @property
    def verification(self):
        return _strict_json(self.verification_bytes)

    def snapshot(self):
        return ProjectionSnapshotView(self.root_bytes, self.namespace_path)


def _membership(root, profiles, limits):
    basenames = {*(kind + '.json' for kind in legacy.RECORD_FILES),
                 *profiles.source_basenames.values(), 'artifact-witness.json', 'composite-witness.json',
                 *legacy.CLAIM_SOURCE_BASENAMES, 'source-claims.jsonl'}
    refs = set()
    for basename in sorted(basenames):
        for path in (root / legacy.SOURCE_ROOT).rglob(basename):
            ref = path.relative_to(root).as_posix()
            if legacy.CATALOG_ROOT in path.relative_to(root).parents:
                continue
            _source_ref(ref)
            refs.add(ref)
            if len(refs) > limits.max_source_files:
                raise SourceCatalogBudgetExceeded('source membership file-count budget exceeded')
    return refs


def _native_mapping(profiles, kind):
    owners = []
    for graph in ('source-navigation', 'source-claims'):
        matches = [entry['type_id'] for entry in profiles.registry['types']
                   for mapping in entry.get('source_mappings', [])
                   if mapping.get('source_graph') == graph and mapping.get('source_kind_id') == kind]
        if len(matches) != 1:
            raise SourceCatalogError('native source kind requires one exact mapping in both source carriers')
        owners.append(matches[0])
    if owners[0] != owners[1]:
        raise SourceCatalogError('native source mappings disagree on the type owner')


def _verified_row(entry, raw, profiles, capture):
    record = _strict_json(raw)
    kind, ref = entry['record_type'], entry['source_record_ref']
    path = _source_ref(ref)
    if not isinstance(record, dict):
        raise SourceCatalogError('source record must be a strict JSON object')
    if kind in profiles.profiles and not (kind == 'composite' and path.name == 'composite-witness.json'):
        profiles.validate_path(kind, ref)
        expected = profiles.catalog_entry(kind, record, ref)
    else:
        _native_mapping(profiles, kind)
        if kind in {'artifact', 'composite'}:
            schema_ref, _, actual_kind = legacy.native_witness_contract(record, ref)
            if kind != actual_kind:
                raise SourceCatalogError('native record family differs from its path and schema')
        else:
            if kind not in legacy.RECORD_FILES or path.name != kind + '.json':
                raise SourceCatalogError('native source path and family differ')
            schema_ref = 'ToS/contracts/source-link.schema.json' if kind == 'link' else CORPUS_REF
        schema = _strict_json(capture.read(schema_ref))
        if schema.get('$id') not in {'https://tree-of-sophia.local/' + schema_ref,
                                     'https://treeofsophia.local/' + schema_ref}:
            raise SourceCatalogError('native source schema has another owner identity')
        validator = Draft202012Validator(schema, format_checker=FormatChecker())
        validator.validate(record)
        if kind == 'artifact':
            expected = legacy.artifact_catalog_entry(capture.root, record, ref, {schema_ref: validator})
        elif kind == 'composite':
            expected = legacy.composite_catalog_entry(capture.root, record, ref, {schema_ref: validator})
        else:
            expected = legacy.render_native_catalog_entry(record, ref)
    reference = source.metadata_subject(record).ref
    if (entry != expected or reference['id'] != entry['record_id']
            or IDENTITY.fullmatch(reference['id']) is None
            or IDENTITY.fullmatch(reference['id']).group(1) != kind
            or type(reference['version']) is not int or not 1 <= reference['version'] <= 9_007_199_254_740_991
            or reference['digest'] != 'sha256:' + entry['record_sha256']):
        raise SourceCatalogError('catalog row does not reproduce its exact source identity and rendering')
    return {'record_id': entry['record_id'], 'entry': entry,
            'source': {'source_ref': ref, 'raw_sha256': _sha(raw), 'raw_bytes': len(raw),
                       'record_ref': reference}}


def _execution_bindings(limits):
    capture = _Capture(EXECUTION_ROOT, limits)
    for ref in EXECUTION_REFS:
        capture.read(ref)
    return capture


def _profile_bindings(profiles, claim_inputs, capture):
    refs = set(profiles.input_digests) | set(claim_inputs) | {REGISTRY_REF, CONTRACT_REF}
    refs |= {ref for ref in capture.observed if ref.startswith('ToS/contracts/')}
    for ref in refs:
        raw = capture.read(ref)
        for values in (profiles.input_digests, claim_inputs):
            if ref in values and _sha(raw) != values[ref]:
                raise SourceCatalogError('source profile inputs changed during verification')
    return {ref: capture.observed[ref] for ref in sorted(refs)}


def bootstrap_source_catalog(root: Path, namespace_path: Path, *, catalog_namespace: str,
                             expected_manifest_sha256: str, expected_publication_token: str | None,
                             work_dir: Path, limits: CatalogLimits | None = None,
                             target_part_bytes: int = DEFAULT_PART_BYTES) -> SourceCatalogCandidate:
    """Explicit full baseline verification and staging; never target-root selection.

    Reads the full public record/Claim catalog scope, not source payloads. This
    certifies mechanical row/source parity and membership at the observed
    publication, not historical completeness, source-reference closure or
    semantic admission. Full-writer staging is confined to explicit scratch.
    """
    limits = CatalogLimits() if limits is None else limits
    if not isinstance(limits, CatalogLimits):
        raise TypeError('bootstrap limits must be CatalogLimits')
    _digest(expected_manifest_sha256)
    root, namespace_path = Path(root).absolute(), Path(namespace_path).absolute()
    publication = PublicationSnapshot(root)
    if publication.token != expected_publication_token:
        raise SourceCatalogError('bootstrap selected publication differs')
    capture = _Capture(root, limits)
    manifest_raw = capture.read(str(legacy.MANIFEST_PATH))
    if _sha(manifest_raw) != expected_manifest_sha256:
        raise SourceCatalogError('explicit legacy catalog manifest binding differs')
    execution = _execution_bindings(limits)
    profiles = SourceRecordProfiles(root)
    membership = _membership(root, profiles, limits)
    for ref in sorted(membership):
        capture.read(ref, 16 * 1024 * 1024 if ref.endswith('.jsonl') else limits.max_record_bytes)
    records = legacy.collect_records(root, profiles=profiles)
    claim_inputs = {}
    claims = legacy.collect_claims(root, input_digests=claim_inputs)
    count = sum(map(len, records.values()))
    if count > limits.max_records or len(claims) > limits.max_claims:
        raise SourceCatalogBudgetExceeded('catalog record or claim count budget exceeded')
    outputs = legacy.render_catalog_rows(records, claims, profile_files=profiles.catalog_files,
                                         publication_token=publication.token)
    if sum(len(text.encode('utf-8')) for text in outputs.values()) > limits.max_catalog_bytes:
        raise SourceCatalogBudgetExceeded('legacy catalog byte budget exceeded')
    for ref, text in outputs.items():
        if capture.read(str(ref), limits.max_catalog_bytes) != text.encode('utf-8'):
            raise SourceCatalogError('legacy catalog differs from the complete source-backed collector')
    manifest = _strict_json(manifest_raw)
    rows = []
    for entries in records.values():
        for entry in entries:
            raw = capture.read(entry['source_record_ref'])
            rows.append(_verified_row(entry, raw, profiles, capture))
    if len({row['record_id'] for row in rows}) != len(rows):
        raise SourceCatalogError('duplicate stable identity in bootstrap rows')
    native_identity = profiles.native_identity_snapshot(
        read_bytes=lambda path, limit: capture.read(path.relative_to(root).as_posix(), limit))
    native_text = profiles.native_text_snapshot(
        read_bytes=lambda path, limit: capture.read(path.relative_to(root).as_posix(), limit))
    profile_bindings = _profile_bindings(profiles, claim_inputs, capture)
    header = {'schema_version': SCHEMA, 'catalog_namespace': catalog_namespace, 'profile_id': PROFILE,
              'record_families': sorted(records), 'record_count': count,
              'source_publication': {'protocol': PUBLICATION_PROTOCOL, 'token': publication.token,
                                     'generation': publication.generation},
              'legacy_baseline': {'manifest_ref': str(legacy.MANIFEST_PATH),
                                  'manifest_sha256': expected_manifest_sha256,
                                  'catalog_sha256': manifest['catalog_sha256'],
                                  'files': {str(ref): capture.observed[str(ref)] for ref in sorted(outputs)},
                                  'native_identity_snapshot': native_identity,
                                  'native_text_snapshot': native_text},
              'profile_bindings': {'source': profile_bindings, 'execution': execution.observed},
              'membership_basis': 'verified-full-baseline-plus-explicit-transitions',
              'last_transition': None, 'claims_addressed': False,
              'source_reference_closure_verified': False}
    _schema('header').validate(header)
    row_validator = _schema('row')
    for row in rows:
        row_validator.validate(row)

    def verify():
        publication.verify_current()
        if _membership(root, profiles, limits) != membership:
            raise SourceCatalogError('source membership changed during bootstrap')
        profiles.native_identity_snapshot(
            read_bytes=lambda path, limit: capture.read(path.relative_to(root).as_posix(), limit))
        profiles.native_text_snapshot(
            read_bytes=lambda path, limit: capture.read(path.relative_to(root).as_posix(), limit))
        capture.verify()
        execution.verify()
        publication.verify_current()
        if _membership(root, profiles, limits) != membership:
            raise SourceCatalogError('source membership changed during bootstrap verification')

    verify()
    install_budget = _Budget(MutationLimits(max_opened_parts=limits.max_installed_parts,
        max_stored_read_bytes=limits.max_installed_bytes, max_written_parts=limits.max_installed_parts,
        max_written_stored_bytes=limits.max_installed_bytes))
    created = []
    with tempfile.TemporaryDirectory(prefix='tos-catalog-bootstrap-', dir=work_dir) as temporary:
        staging_path = Path(temporary) / namespace_path.name
        write_projection(staging_path, header,
                         {'records': Collection(rows, 'record_id', ('record_id',))},
                         target_part_bytes=target_part_bytes, work_dir=Path(temporary))
        staged = ProjectionReader(staging_path, cache_bytes=0)
        root_bytes = staged._root_bytes
        parts = sorted(part for part in staged.closure_paths() if part != staging_path)
        if len(parts) > limits.max_installed_parts:
            raise SourceCatalogBudgetExceeded('bootstrap installed part-count budget exceeded')
        for part in parts:
            raw = _read_owned(part, MAX_PART_BYTES + 65536)
            install_budget.take(written_parts=1, written_stored_bytes=len(raw))
            relative = part.relative_to(staging_path.parent)
            if _install(namespace_path.parent / relative, raw, install_budget):
                created.append(relative.as_posix())
        verify()
    verification = {'mode': 'explicit-full-bootstrap', 'records_verified': count,
                    'legacy_claims_anchored': len(claims), 'publication_token': publication.token,
                    'source_input_bytes': capture.input_bytes, 'source_read_bytes': capture.read_bytes,
                    'source_reference_closure_verified': False, 'grants_admission': False}
    return SourceCatalogCandidate(namespace_path, root_bytes, None, tuple(created), canonical_bytes(verification))


def stage_agent_catalog_transition(root: Path, before: SourceCatalogSnapshot, *,
                                   transaction_id: str, expected_publication_token: str,
                                   limits: CatalogLimits | None = None,
                                   mutation_limits: MutationLimits | None = None,
                                   target_part_bytes: int = DEFAULT_PART_BYTES) -> SourceCatalogCandidate:
    """One current committed present-to-present Agent descriptive transition.

    Retained source transport, request reconstruction and current selected bytes
    are checked; no source command is executed. Source authorization is not
    acquired by this read. Claim lookup and reverse source-reference dependencies
    remain a separate integration gap, not certified by catalog membership.
    """
    if not isinstance(before, SourceCatalogSnapshot):
        raise TypeError('an explicit SourceCatalogSnapshot baseline is required')
    limits = CatalogLimits() if limits is None else limits
    if not isinstance(limits, CatalogLimits):
        raise TypeError('catalog limits must be CatalogLimits')
    root = Path(root).absolute()
    capture, execution = _Capture(root, limits), _Capture(EXECUTION_ROOT, limits)
    header = before.header
    for owner, observer in (('source', capture), ('execution', execution)):
        for ref, binding in header['profile_bindings'][owner].items():
            raw = observer.read(ref)
            if {'sha256': _sha(raw), 'bytes': len(raw)} != binding:
                raise SourceCatalogRequiresBootstrap('source catalog profile inputs changed; explicit bootstrap required')
    publication = PublicationSnapshot(root)
    if publication.token != expected_publication_token:
        raise SourceCatalogError('selected successor publication differs')
    retained = transactions.inspect_transaction(root, transaction_id)
    manifest, plan = retained['manifest'], retained['plan']
    if (retained['status'] != 'committed' or not retained['is_current_publication']
            or retained['publication']['token'] != publication.token
            or manifest['base_publication'] != {key: header['source_publication'][key] for key in ('token', 'generation')}):
        raise SourceCatalogError('catalog requires the exact immediate committed publication successor')
    authorization = plan['authorization']
    if (set(authorization) != {'schema_version', 'principal_id', 'authority_ref', 'source_path',
                              'record_id', 'record_type', 'request'}
            or authorization['schema_version'] != selected.AUTHORIZATION
            or authorization['record_type'] != 'agent'
            or set(plan) != {'authorization', 'files', 'new_directories'} or plan['new_directories']):
        raise SourceCatalogRequiresBootstrap('only an existing Agent descriptive revision is supported')
    ref = authorization['source_path']
    path = _source_ref(ref)
    if path.name != 'agent.json':
        raise SourceCatalogError('selected Agent source basename differs')
    row = before.get(authorization['record_id'])
    if row.entry['record_type'] != 'agent' or row.source['source_ref'] != ref:
        raise SourceCatalogError('selected Agent identity or path moved')
    request = authorization['request']
    selected._request(request)
    if (not request['fields'] or not set(request['fields']) <= source.CORPUS_REVISION_FIELDS
            or selected._transaction_id(request) != transaction_id
            or request['expected_publication'] != header['source_publication']['token']):
        raise SourceCatalogError('selected request is not the supported exact descriptive transition')
    names = set(revisions._selected_names(path))
    if ({item['path'] for item in plan['files']} != {str(path.parent / name) for name in names}
            or any(item['after'] is None for item in plan['files'])):
        raise SourceCatalogError('selected revision file closure differs or deletes a successor')
    old = {Path(item['path']).name: item['before'] for item in plan['files'] if item['before'] is not None}
    new = {Path(item['path']).name: item['after'] for item in plan['files']}
    if path.name not in old:
        raise SourceCatalogError('Agent insertion cannot enter present-to-present catalog staging')
    old_record, new_record = _strict_json(old[path.name]), _strict_json(new[path.name])
    if (row.source['raw_sha256'] != _sha(old[path.name]) or row.source['raw_bytes'] != len(old[path.name])
            or row.source['record_ref'] != source.metadata_subject(old_record).ref
            or request['expected_source'] != row.source['record_ref']
            or request['expected_revision'] != revisions._revision(old)):
        raise SourceCatalogError('retained predecessor differs from the exact addressed source binding')
    profiles = SourceRecordProfiles(root)
    _verified_row(row.entry, old[path.name], profiles, capture)
    expected_record = {**old_record, **request['fields'], 'record_version': old_record['record_version'] + 1}
    if new_record != expected_record or new[path.name] != revisions._encode(expected_record):
        raise SourceCatalogError('retained request does not reconstruct the exact Agent successor bytes')
    entry = legacy.render_native_catalog_entry(new_record, ref)
    new_row = _verified_row(entry, new[path.name], profiles, capture)
    history, new_history = revisions._history(old, old_record), revisions._history(new, new_record)
    if (len(new_history['receipts']) != len(history['receipts']) + 1
            or new_history['receipts'][:-1] != history['receipts']):
        raise SourceCatalogError('selected Agent history must append exactly one transition')
    receipt = new_history['receipts'][-1]
    formname = path.stem + '.human-forms.json'
    old_forms = _strict_json(old[formname]) if formname in old else None
    if old_forms is not None:
        source._validate_history(old_forms)
        if {form['form_id'] for form in old_forms['forms']} - {form['form_id'] for form in request['forms']}:
            raise SourceCatalogError('selected successor drops a prior current HumanForm')
    changes = [source.prepare_metadata_change(new_record, old_forms, authorization['principal_id'], **item)
               for item in request['forms']]
    forms = source._apply(old_forms, source.metadata_subject(new_record), changes)
    form_refs = [source._form_ref(change['form']) for change in changes]
    expected_receipt = selected._receipt(authorization, request, source.metadata_subject(old_record),
        source.metadata_subject(new_record), form_refs, recorded_at=receipt['recorded_at'])
    if (new[formname] != revisions._encode(forms) or receipt != expected_receipt
            or receipt['publication']['transaction_id'] != transaction_id
            or new[revisions.HISTORY] != revisions._encode({**history,
                'schema_version': 'tos_source_revision_history_v2', 'receipts': [*history['receipts'], expected_receipt]})):
        raise SourceCatalogError('selected metadata output does not reproduce its retained revision request')
    descriptor = {'schema_version': source.CORPUS_SELECTED_REVISION_CONFIG,
                  'source_root': str(root), 'source_path': ref, 'record_id': authorization['record_id'],
                  'record_type': 'agent'}
    # This descriptor feeds read-only shape/dependency validation, never _scope,
    # a source command, a delegation configuration, or a publication guard.
    if revisions._dependencies(descriptor, old_record) != request['expected_dependencies']:
        raise SourceCatalogRequiresBootstrap('retained revision processor or schema dependencies changed')
    restored, _ = revisions._read_archive(root, descriptor, receipt)
    if restored != old:
        raise SourceCatalogError('retained predecessor archive differs from transaction before bytes')
    for item in plan['files']:
        if capture.read(item['path'], transactions.MAX_SIDE_BYTES) != item['after']:
            raise SourceCatalogError('current selected metadata differs from committed after bytes')
    _profile_bindings(profiles, {}, capture)
    new_header = {**header,
                  'source_publication': {'protocol': PUBLICATION_PROTOCOL, 'token': publication.token,
                                         'generation': publication.generation},
                  'last_transition': {'transaction_id': transaction_id,
                                      'manifest_sha256': retained['manifest_sha256'],
                                      'record_id': authorization['record_id']}}
    _schema('header').validate(new_header)
    _schema('row').validate(new_row)

    def verify():
        publication.verify_current()
        capture.verify()
        execution.verify()
        if transactions.inspect_transaction(root, transaction_id) != retained:
            raise SourceCatalogError('retained selected transaction evidence changed')
        if revisions._read_archive(root, descriptor, receipt)[0] != old:
            raise SourceCatalogError('retained predecessor archive changed')
        publication.verify_current()

    verify()
    candidate = stage_projection_snapshot_changes(before.view,
        expected_before_sha256=before.root_sha256, trusted_baseline_sha256=before.root_sha256,
        changes=[ProjectionChange('records', authorization['record_id'], True, row.row_sha256, True, new_row)],
        header_change=ProjectionHeaderChange(_sha(canonical_bytes(header)), new_header),
        limits=mutation_limits, target_part_bytes=target_part_bytes)
    verify()
    verification = {'mode': 'selected-agent-present-to-present', 'transaction_id': transaction_id,
                    'record_id': authorization['record_id'], 'records_verified': 1,
                    'publication_token': publication.token, 'source_read_bytes': capture.read_bytes,
                    'projection_accounting': dict(candidate.accounting),
                    'source_reference_closure_verified': False, 'grants_admission': False}
    return SourceCatalogCandidate(candidate.namespace_path, candidate.root_bytes, before.root_sha256,
                                  candidate.created_parts, canonical_bytes(verification))
