"""Initial native D1 product from admitted retained source and rights snapshots.

This explicit full-product operation never recompiles normalized knowledge.
Source/rights admission, currentness and permission to apply remain upstream.
"""
import hashlib
from pathlib import Path

import prepared_delta_runtime as delta
from incremental_runtime import DeltaRecorder, _search_address_revision
from tos_access.projection_mutation import (
    ProjectionSnapshotView, MutationLimits, _SnapshotMutationReader, _Budget,
)

SCHEMA = 'tos_native_navigation_d1_bootstrap_v1'
RIGHTS_SCHEMA = 'tos_source_navigation_rights_v1'


def _own_digest():
    with Path(__file__).open('rb') as stream:
        raw = stream.read(1_048_577)
    if len(raw) > 1_048_576:
        raise ValueError('bootstrap implementation byte budget exceeded')
    return hashlib.sha256(raw).hexdigest()


def build_source_navigation_bootstrap_sql(db, prepared_db, target, *,
        expected_d1_revision, prepared_binding, rights_view,
        expected_rights_sha256, trusted_rights_sha256, rollback_target,
        limits=None, projection_limits=None):
    """Emit guarded forward/reverse SQL for an absent native product only.

    The exact D1/prepared pair and complete rights snapshot must already be
    admitted by their owners. The helper verifies those supplied identities,
    all selected immutable parts, reader pairing and absence, not authority.
    Both SQLite snapshots are caller-held read transactions; no writes occur.
    """
    limits = limits or delta.PreparedD1DeltaLimits()
    projection_limits = projection_limits or MutationLimits()
    paths = [Path(target), Path(rollback_target)]
    resolved = [p.resolve() for path in paths for p in (path, path.with_name(path.name + '.next'))]
    if len(set(resolved)) != 4 or any(path.exists() for path in resolved):
        raise ValueError('distinct fresh forward and reverse SQL targets required')
    if not db.in_transaction or not prepared_db.in_transaction:
        raise ValueError('caller-owned D1 and prepared snapshots required')
    if (not isinstance(rights_view, ProjectionSnapshotView)
            or rights_view.snapshot_digest != expected_rights_sha256
            or expected_rights_sha256 != trusted_rights_sha256):
        raise ValueError('exact admitted rights snapshot required')
    _search_address_revision(db, expected_d1_revision)
    capture = delta.Capture(limits)
    prepared_top, descriptor, source = capture.local(prepared_db, prepared_binding)
    d1_top = capture.metadata(db, delta.TOP_KEY)[0]
    ignored = {'data_revision', 'read_model_schema'}
    if (d1_top.get('read_model_schema') != delta.full.READ_MODEL_SCHEMA_VERSION
            or {k: v for k, v in d1_top.items() if k not in ignored}
            != {k: v for k, v in prepared_top.items() if k not in ignored}):
        raise ValueError('D1 and admitted prepared predecessor differ')
    for key in (delta.CATALOG_KEY, delta.LENS_META_KEY):
        if capture.metadata(db, key)[0] != capture.metadata(prepared_db, key)[0]:
            raise ValueError('D1 and prepared reader metadata differ')
    native_tables = tuple(delta.navigation.projection.COLUMNS)
    if capture.metadata(db, 'source_navigation_top')[0] != {} or any(
            db.execute(f'SELECT 1 FROM {table} LIMIT 1').fetchone() for table in native_tables):
        raise ValueError('native product must be completely absent')
    installed = delta.auxiliary.admit(db, capture, d1_top)
    navigation = source.roots().get('source-navigation')
    if navigation is None:
        raise ValueError('prepared source-navigation root required')
    budget = _Budget(projection_limits)
    readers = {
        'navigation': _SnapshotMutationReader(navigation, navigation.snapshot_digest, budget),
        'rights': _SnapshotMutationReader(rights_view, expected_rights_sha256, budget),
    }
    nav_root, rights_root = (readers[key].manifest for key in ('navigation', 'rights'))
    if (nav_root['logical_schema'] not in ('tos_agent_source_navigation_rows_v1', 'tos_source_navigation_v1')
            or set(nav_root['collections']) != {'nodes', 'edges'}
            or rights_root['logical_schema'] != RIGHTS_SCHEMA
            or set(rights_root['collections']) != {'rights'}):
        raise ValueError('explicit native nodes/edges and separate rights products required')
    header = rights_view.metadata().get('navigation_header')
    if (not isinstance(header, dict) or header.get('schema_version') != 'tos_source_navigation_v1'
            or not isinstance(header.get('authority_boundary'), str) or not header['authority_boundary']):
        raise ValueError('complete native navigation header required in admitted rights snapshot')
    counts = {}
    for name in ('nodes', 'edges', 'rights'):
        reader = readers['rights' if name == 'rights' else 'navigation']
        spec = reader.manifest['collections'][name]
        key = delta.navigation.projection.COLUMNS['source_navigation_' + name][0]
        if spec['key_field'] != key or spec['order_fields'] != [key]:
            raise ValueError('native source identity/order profile differs')
        counts[name] = spec['root']['count']
    if header.get('counts') != counts:
        raise ValueError('complete native product counts differ')
    if sum(counts.values()) > limits.max_rows:
        raise ValueError('native initial product row budget exceeded')
    implementation = delta.execution_profile()
    own_digest = _own_digest()
    lineage = {'schema': SCHEMA, 'base_d1_revision': expected_d1_revision,
        'prepared_binding': prepared_binding, 'source_inputs_sha256': source.digest,
        'source_navigation_sha256': navigation.snapshot_digest,
        'rights_sha256': expected_rights_sha256, 'implementation_sha256': implementation,
        'bootstrap_implementation_sha256': own_digest}
    revision = delta._sha(delta._compact(lineage))
    accounting = [0, 0]
    previous, successor = (delta.Rows(limits, accounting) for _ in range(2))
    for name in ('nodes', 'edges', 'rights'):
        reader = readers['rights' if name == 'rights' else 'navigation']
        for _, item in reader.iter_items(name):
            if delta.normalize_paths(item, delta.full.REPO_ROOT) != item:
                raise ValueError('native input requires explicit portable-path migration')
            rows = delta.navigation.projection.project_rows(name, 0, item, repo_root=delta.full.REPO_ROOT)
            for table, values in rows.items():
                for row in values:
                    successor.put(table, row)
    source_header = descriptor['header']
    catalog = capture.metadata(prepared_db, delta.CATALOG_KEY)[0]
    lens = capture.metadata(prepared_db, delta.LENS_META_KEY)[0]
    metadata = delta.published_reader_metadata(source_header, catalog,
        delta.full.READ_MODEL_SCHEMA_VERSION, revision, lens_metadata=lens)
    metadata.update(data_revision={'sha256': revision}, source_navigation_top=header)
    for key, value in metadata.items():
        for row in capture.metadata(db, key)[1]:
            previous.put('edge_meta', row)
        for part, chunk in enumerate(delta.full.chunk_text(delta._compact(value))):
            successor.put('edge_meta', (key, part, chunk))
    recorders, sql_bytes = [], 0
    try:
        for index, (path, before, after, base, destination) in enumerate((
                (paths[0], previous, successor, expected_d1_revision, revision),
                (paths[1], successor, previous, revision, expected_d1_revision))):
            path.parent.mkdir(parents=True, exist_ok=True)
            tops = (d1_top, metadata[delta.TOP_KEY]) if index == 0 else (metadata[delta.TOP_KEY], d1_top)
            recorder = DeltaRecorder(path, destination, delta.full.READ_MODEL_SCHEMA_VERSION,
                before.index(base), auxiliary_bindings={table: tops for table in installed},
                expected_empty_tables=native_tables if index == 0 else ())
            recorders.append(recorder)
            for table, rows in after.data.items():
                for values in rows.values():
                    for statement in after.statements(table, values):
                        recorder.observe(statement)
                    if sql_bytes + recorder.stream.tell() > limits.max_sql_bytes:
                        raise ValueError('native product SQL budget exceeded')
            recorder.finish(publish=False)
            sql_bytes += recorder.pending_path.stat().st_size
            if sql_bytes > limits.max_sql_bytes:
                raise ValueError('native product SQL budget exceeded')
        _search_address_revision(db, expected_d1_revision)
        if delta.execution_profile() != implementation or _own_digest() != own_digest:
            raise ValueError('native bootstrap implementation changed during capture')
        for recorder in reversed(recorders):
            recorder.publish()
        return {**lineage, 'target_d1_revision': revision, 'counts': counts,
            'delta': recorders[0].summary(), 'rollback': recorders[1].summary(),
            'normalized_rows_changed': 0, 'prepared_rows_changed': 0,
            'source_currentness_verified': False, 'rights_admission_verified_by_helper': False,
            'projection_reads': budget.usage, 'metadata_read_bytes': capture.read_bytes,
            'retained_bytes': accounting[0], 'sql_bytes': sql_bytes}
    finally:
        for recorder in recorders:
            recorder.close()
