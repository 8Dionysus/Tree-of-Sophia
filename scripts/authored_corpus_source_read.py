"""Address existing authored CSV corpus rows without creating source truth.

Bootstrap is explicit owner work: verify the supplied corpus-index slice
against tracked authored files, then write an unselected immutable projection.
Serving reads only an addressed member of that selected projection. No corpus
scan, Git invocation, path supplied by a consumer, or current-file fallback is
performed while serving. Canon/intake status is never changed by this reader.
"""
from __future__ import annotations

import csv
import hashlib
import io
import json
from pathlib import Path
import subprocess

from source_metadata_snapshot import _read_owned
from tos_corpus_index_common import read_exact_edge_row, owner_branch, authority_layer
from tos_access.projection_store import Collection, write_projection
from tos_access.projection_mutation import (
    ProjectionSnapshotView, MutationLimits, ProjectionMutationBudgetExceeded,
    _SnapshotMutationReader, _Budget,
)
from tos_access.source_read import (
    SourceReadError, SourceReadBudgetExceeded, _canonical_bytes, _csv_identity,
    _target, exact_csv_target,
)

SCHEMA = 'tos_authored_corpus_raw_v1'
MAX_FILE_BYTES = 8 * 1024 * 1024
MAX_BOOTSTRAP_BYTES = 64 * 1024 * 1024
MAX_ROWS = 65536
MAX_PACKS = 4096


def _key(pack, edge):
    return _canonical_bytes(list(_csv_identity(pack, edge))).decode('utf-8')


def _path(root, pack):
    # Identity syntax is separate from membership. Only bootstrap or an
    # addressed row admitted by the selected source vector calls this helper.
    _csv_identity(pack, 'path-validation')
    root = Path(root)
    if not root.is_absolute() or root.resolve() != root:
        raise SourceReadError('authored source root must be absolute and symlink-free')
    path = root
    for part in ('ToS', *pack.split('/'), 'edges.csv'):
        path = path / part
        if path.is_symlink():
            raise SourceReadError('authored CSV path contains a symlink')
    return path


def bootstrap_authored_csv_index(source_root, output, corpus_index, *, work_dir=None):
    """Verify a bounded existing index slice; publish no selected source vector.

All rows of each included pack must be supplied. This permits an explicit
slice without pretending it is complete corpus membership. Work is linear in
the selected files and rows; exact CSV parsing is not repeated per row here.
    """
    packs, edges = corpus_index['relation_packs'], corpus_index['relation_edges']
    if type(packs) is not list or type(edges) is not list:
        raise SourceReadError('authored corpus collections must be arrays')
    if len(packs) > MAX_PACKS or len(edges) > MAX_ROWS:
        raise SourceReadBudgetExceeded('authored corpus bootstrap row budget')
    if len(_canonical_bytes({'relation_packs': packs, 'relation_edges': edges})) > MAX_BOOTSTRAP_BYTES:
        raise SourceReadBudgetExceeded('authored corpus bootstrap input budget')
    source_root, output = Path(source_root), Path(output)
    by_pack = {}
    for pack in packs:
        identity = pack['pack_id']
        path = _path(source_root, identity)
        if identity in by_pack or pack['path'] != path.relative_to(source_root).as_posix():
            raise SourceReadError('authored pack identity or source path differs')
        if (pack['owner_branch'] != owner_branch(pack['path'])
                or pack['authority_layer'] != authority_layer(pack['path'])):
            raise SourceReadError('authored pack owner posture differs')
        by_pack[identity] = (pack, path, [])
    for edge in edges:
        target = exact_csv_target(edge)
        if target is None or target['pack_id'] not in by_pack:
            raise SourceReadError('authored row has no exact selected pack')
        by_pack[target['pack_id']][2].append((edge, target))
    refs = [entry[0]['path'] for entry in by_pack.values()]
    if refs:
        try:
            tracked = subprocess.run(['git', '-C', str(source_root), 'ls-files', '-z', '--cached', '--',
                                      *[':(literal)' + ref for ref in refs]],
                                     check=True, capture_output=True, timeout=30).stdout
        except (subprocess.SubprocessError, OSError) as error:
            raise SourceReadError('authored tracked membership could not be verified') from error
        if set(tracked.decode('utf-8').rstrip('\0').split('\0')) != set(refs):
            raise SourceReadError('authored CSV membership requires tracked owner files')
    rows, seen, consumed = [], set(), 0
    for pack, path, selected in by_pack.values():
        raw = _read_owned(path, min(MAX_FILE_BYTES, MAX_BOOTSTRAP_BYTES - consumed))
        consumed += len(raw)
        digest = hashlib.sha256(raw).hexdigest()
        reader = csv.DictReader(io.StringIO(raw.decode('utf-8'), newline=''), strict=True)
        columns = reader.fieldnames
        if not columns or any(not name for name in columns) or len(set(columns)) != len(columns):
            raise SourceReadError('authored CSV requires unique named columns')
        if columns != pack['columns'] or digest != pack['sha256']:
            raise SourceReadError('authored pack columns or digest differ')
        indexed = {target['source_row']: (edge, target) for edge, target in selected}
        if len(indexed) != len(selected):
            raise SourceReadError('authored corpus repeats a logical row')
        count = 0
        for count, record in enumerate(reader, 1):
            if count > MAX_ROWS:
                raise SourceReadBudgetExceeded('authored CSV row budget')
            pair = indexed.get(count)
            if pair is None or None in record:
                raise SourceReadError('authored corpus does not cover exact pack rows')
            edge, target = pair
            if (record != edge['properties']['source_record'] or digest != target['source_file_sha256']
                    or record.get('edge_id') and record['edge_id'] != target['edge_id']):
                raise SourceReadError('authored corpus row differs from source')
            key = _key(target['pack_id'], target['edge_id'])
            if key in seen:
                raise SourceReadError('authored corpus repeats a pack/edge identity')
            seen.add(key)
            rows.append({'key': key, 'target': target, 'record': record,
                         'source_ref': pack['path'], 'owner_branch': pack['owner_branch'],
                         'authority_layer': pack['authority_layer']})
        if count != pack['edge_count'] or count != len(selected):
            raise SourceReadError('authored corpus pack row count differs')
        # A concurrent source edit cannot publish a candidate as current.
        _path(source_root, pack['pack_id'])
        if hashlib.sha256(_read_owned(path, MAX_FILE_BYTES)).hexdigest() != digest:
            raise SourceReadError('authored source changed during bootstrap')
    write_projection(output, {'schema_version': SCHEMA, 'row_count': len(rows),
                             'pack_count': len(packs)},
                     {'relations': Collection(rows, 'key', ('key',))}, work_dir=work_dir)
    return ProjectionSnapshotView(output.read_bytes(), output)


def verify_authored_csv_sources(source_root, corpus_index):
    """Recheck admitted file bytes immediately before the caller's commit.

    Use the same detached, bootstrap-validated corpus index. This is a source
    guard, not a cross-filesystem transaction or a rights/semantic assessment.
    """
    consumed = 0
    for pack in corpus_index['relation_packs']:
        path = _path(source_root, pack['pack_id'])
        raw = _read_owned(path, min(MAX_FILE_BYTES, MAX_BOOTSTRAP_BYTES - consumed))
        consumed += len(raw)
        if hashlib.sha256(raw).hexdigest() != pack['sha256']:
            raise SourceReadError('authored source changed before prepared commit')


def _verify_prepared_csv_membership(db, corpus_index, *, limits, progress_owner):
    """Resolve bounded indexed coverage across both CSV owner graphs."""
    from tos_access.prepared_source_dependencies import _operation
    expected = {}
    for edge in corpus_index['relation_edges']:
        target = exact_csv_target(edge)
        if target is None:
            raise SourceReadError('authored corpus has no exact CSV target')
        key = _key(target['pack_id'], target['edge_id'])
        if key in expected:
            raise SourceReadError('authored corpus repeats a pack/edge identity')
        expected[key] = (target, edge['properties']['source_record'])
    seen = set()
    with _operation(db, limits, progress_owner) as budget:
        for graph_id, graph, raw in budget.rows_from(
                'SELECT id,source_graph,CASE WHEN length(CAST(json AS BLOB))<=? THEN json END '
                'FROM knowledge_relations INDEXED BY knowledge_relations_source_predicate_idx '
                "WHERE source_graph IN ('canon','candidate-intake')", (limits.max_row_bytes,)):
            if type(raw) is not str:
                raise SourceReadBudgetExceeded('prepared authored row byte budget')
            row = json.loads(raw)
            payload = row.get('source_record', {}).get('payload')
            if type(payload) is not dict or row.get('id') != graph_id or row.get('source_graph') != graph:
                raise SourceReadError('prepared authored source carrier differs')
            properties = payload.get('properties')
            if type(properties) is not dict:
                raise SourceReadError('prepared authored properties are missing')
            # The other current owner carrier is explicitly a node-contract
            # relation. Missing CSV identity must never silently become non-CSV.
            if ('pack_id' not in payload
                    and properties.get('derivation') == 'authored-node-contract-relation'
                    and not {'source_record', 'source_row', 'source_file_sha256'} & properties.keys()
                    and str(payload.get('source_ref', '')).endswith('/node.json')):
                continue
            target = exact_csv_target(payload)
            if target is None:
                raise SourceReadError('prepared authored relation has no exact CSV identity')
            pack, edge = target['pack_id'], target['edge_id']
            key = _key(pack, edge)
            owner_graph = 'canon' if pack.startswith('canon/') else 'candidate-intake'
            if (graph != owner_graph or graph_id != graph + ':' + pack + ':' + edge
                    or payload.get('source_ref') != 'ToS/' + pack + '/edges.csv'
                    or key in seen or expected.get(key) != (target, properties['source_record'])):
                raise SourceReadError('prepared CSV membership or exact retained content differs')
            seen.add(key)
        if seen != expected.keys():
            raise SourceReadError('authored index does not cover exactly the prepared CSV corpus')
        return {'prepared_csv_rows_verified': len(seen), 'prepared_source_rows_read': budget.rows,
                'prepared_source_bytes_read': budget.read_bytes}


def bootstrap_authored_source_read_transaction(db, *, source_root, output, corpus_index,
        expected_binding, before_source_inputs, before_inputs, progress_owner, work_dir=None,
        publication_limits=None, dependency_limits=None, catalog_limits=None, semantic_limits=None):
    """Admit exact CSV addressing for the existing selected prepared corpus.

    Creates an unselected immutable root, then atomically pairs it with the
    unchanged normalized rows, dependencies and Agent context. No arbitrary
    prebuilt root is admitted. The caller owns complete rollback on error and
    rechecks ``verify_authored_csv_sources`` with the same index before commit.
    Failed immutable staging may remain; nothing activates a live consumer.
    """
    import source_agent_publication as publication
    from tos_access.catalog_semantics import CatalogInputs
    from tos_access.prepared_source_dependencies import SourceDependencyLimits
    publication._require_vector(before_source_inputs)
    if publication.read_prepared_source_inputs_transaction(db, expected_binding=expected_binding,
            limits=publication_limits) != before_source_inputs:
        raise SourceReadError('authored extension source predecessor differs')
    if 'authored-corpus' in before_source_inputs.roots():
        raise SourceReadError('authored source addressing is already selected')
    output = Path(output)
    if output.exists() or output.is_symlink():
        raise SourceReadError('authored extension requires a fresh output namespace')
    encoded = _canonical_bytes(corpus_index)
    if len(encoded) > MAX_BOOTSTRAP_BYTES:
        raise SourceReadBudgetExceeded('authored corpus bootstrap input budget')
    corpus_index = json.loads(encoded)
    if (type(corpus_index.get('relation_edges')) is not list
            or type(corpus_index.get('relation_packs')) is not list):
        raise SourceReadError('authored corpus collections must be arrays')
    if len(corpus_index['relation_edges']) > MAX_ROWS or len(corpus_index['relation_packs']) > MAX_PACKS:
        raise SourceReadBudgetExceeded('authored corpus bootstrap row budget')
    dependencies = dependency_limits or SourceDependencyLimits()
    coverage = _verify_prepared_csv_membership(db, corpus_index, limits=dependencies,
                                              progress_owner=progress_owner)
    view = bootstrap_authored_csv_index(source_root, output, corpus_index, work_dir=work_dir)
    previous = before_source_inputs.value()
    after = publication.source_vector_inputs(roots={**before_source_inputs.roots(), 'authored-corpus': view},
        dependencies=previous['dependencies'], source_publication=previous['source_publication'])
    header = before_inputs.header
    header['source_revision'] = after.value()['source_revision']
    after_inputs = CatalogInputs(header, before_inputs.entity_type_registry,
        before_inputs.relation_type_registry, before_inputs.lenses,
        source_order_profile=before_inputs.source_order_profile)
    result = publication.bootstrap_agent_source_addressing_extension_transaction(db,
        expected_binding=expected_binding, before_source_inputs=before_source_inputs,
        after_source_inputs=after, added_root='authored-corpus', before_inputs=before_inputs,
        after_inputs=after_inputs, progress_owner=progress_owner, publication_limits=publication_limits,
        dependency_limits=dependencies, catalog_limits=catalog_limits, semantic_limits=semantic_limits)
    verify_authored_csv_sources(source_root, corpus_index)
    return {**result, **coverage, 'source_root_admission_verified': True,
            'source_root_admission_scope': 'exact-retained-authored-csv-records',
            'authored_source_files_verified': len(corpus_index['relation_packs'])}


class AuthoredCorpusReader:
    """Request-local addressed reader, bound by SourceOwnerBinding to its view."""
    def __init__(self, source_root, view, epoch, *, limits=None):
        self.source_root, self.view, self.epoch = Path(source_root), view, epoch
        self._reader = _SnapshotMutationReader(view, view.snapshot_digest, _Budget(limits or MutationLimits()))
        manifest = self._reader.manifest
        header = manifest['header']
        if (manifest['logical_schema'] != SCHEMA or set(manifest['collections']) != {'relations'}
                or set(header) != {'schema_version', 'row_count', 'pack_count'}
                or type(header['row_count']) is not int or not 0 <= header['row_count'] <= MAX_ROWS
                or type(header['pack_count']) is not int or not 0 <= header['pack_count'] <= MAX_PACKS):
            raise SourceReadError('authored corpus root contract differs')
        collection = manifest['collections']['relations']
        if (collection['key_field'] != 'key' or collection['order_fields'] != ['key']
                or collection['root']['count'] != header['row_count']):
            raise SourceReadError('authored corpus collection binding differs')

    def source_read_binding(self):
        return self.epoch

    def verify_current(self):
        self._reader.verify_binding()

    def _lookup(self, pack, edge):
        key = _key(pack, edge)
        descriptor = self._reader.manifest['collections']['relations']['root']
        prefix, hashed = '', hashlib.sha256(key.encode('utf-8')).hexdigest()
        try:
            while descriptor['kind'] == 'index':
                digit = hashed[len(prefix)]
                children = self._reader._children(descriptor, prefix)
                if digit not in children:
                    return None
                descriptor, prefix = children[digit], prefix + digit
            row = dict(self._reader._rows('relations', descriptor, prefix)).get(key)
        except ProjectionMutationBudgetExceeded as error:
            raise SourceReadBudgetExceeded('authored corpus lookup budget') from error
        if row is not None:
            if (type(row) is not dict or set(row) != {'key', 'target', 'record', 'source_ref', 'owner_branch', 'authority_layer'}
                    or type(row['record']) is not dict):
                raise SourceReadError('authored addressed row contract differs')
            selected_target = _target(row['target'])
            if (selected_target['layer'] != 'authored_csv_record'
                    or row['key'] != key or _key(selected_target['pack_id'], selected_target['edge_id']) != key
                    or row['source_ref'] != 'ToS/' + pack + '/edges.csv'
                    or row['owner_branch'] != owner_branch(row['source_ref'])
                    or row['authority_layer'] != authority_layer(row['source_ref'])):
                raise SourceReadError('authored addressed row binding differs')
            target = exact_csv_target({'pack_id': pack, 'edge_id': edge, 'properties': {
                'source_record': row['record'], 'source_row': row['target']['source_row'],
                'source_file_sha256': row['target']['source_file_sha256']}})
            if target is None or target != selected_target:
                raise SourceReadError('authored addressed content binding differs')
        return row

    def issue(self, pack, edge):
        row = self._lookup(pack, edge)
        return row['target'] if row is not None else None

    def read(self, target):
        target = _target(target)
        row = self._lookup(target['pack_id'], target['edge_id'])
        if row is None:
            return None
        if row['target'] != target:
            raise SourceReadError('authored CSV target differs from selected member')
        path = _path(self.source_root, target['pack_id'])
        try:
            result = read_exact_edge_row(path, source_file_sha256=target['source_file_sha256'],
                source_row=target['source_row'], source_record=row['record'])
        except ValueError as error:
            if 'budget' in str(error):
                raise SourceReadBudgetExceeded('authored CSV read budget') from error
            raise SourceReadError('authored CSV exact source binding differs') from error
        _path(self.source_root, target['pack_id'])
        record = result.pop('record')
        return record, {**result, 'source_ref': row['source_ref'],
                        'owner_branch': row['owner_branch'], 'authority_layer': row['authority_layer'],
                        'authored_root_sha256': self.view.snapshot_digest}
