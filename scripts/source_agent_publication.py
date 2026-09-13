"""Offline selected Agent correction through source-owned prepared publication.

This profile is deliberately not a live compiler or a legacy snapshot adopter.
Its explicit bootstrap owns source encounter order and reducer membership. The
source command commits separately; publication failures leave a retryable source
successor, never an implicit source rollback.
"""
from contextlib import contextmanager
import copy
from dataclasses import dataclass, replace
import hashlib
from pathlib import Path

from tos_access import knowledge as k
from tos_access.catalog_semantics import CatalogInputs, CANONICAL_ORDER, order_key, catalog_digest
from tos_access.catalog_index import CatalogIndex
from tos_access.prepared_catalog import _body, _selected as _catalog_selected
from tos_access.prepared_publication import PreparedChange, PublicationLimits
from tos_access.prepared_source_binding import PreparedSourceInputs, read_prepared_source_inputs_transaction
from tos_access.prepared_source_dependencies import (
    ProgressHandlerOwner, SourceClaimDependencies, SourceDependencyLimits,
    lookup_source_dependencies_transaction, _operation, _claim, _current, _column,
)
from tos_access.prepared_source_publication import apply_dependency_bound_prepared_delta_transaction
from tos_access.projection_mutation import (
    ProjectionChange, ProjectionSnapshotView, MutationLimits, _SnapshotMutationReader,
    _Budget as _ProjectionBudget, stage_projection_snapshot_changes, _json_bytes,
)
from tos_access.projection_store import _strict_json, _digest
from tos_access.source_assembly_normalization import normalize_source_assembly_candidate
from source_catalog_projection import SourceCatalogSnapshot, stage_agent_catalog_transition

REVISION_PROFILE = 'tos_agent_source_root_vector_v1'
STATE_SCHEMA = 'tos_agent_publication_context_index_v1'
REQUIRED_ROOTS = frozenset(('source-catalog', 'source-navigation', 'bibliographic-claims'))
RAW_SCHEMAS = {'source-navigation': 'tos_agent_source_navigation_rows_v1',
               'bibliographic-claims': 'tos_agent_bibliographic_rows_v1'}
RAW_COLLECTIONS = {'source-navigation': {'nodes': 'node_id', 'edges': 'edge_id'},
                   'bibliographic-claims': {'nodes': 'node_id', 'edges': 'edge_id', 'claim_traces': 'claim_ref'}}
_CONTEXT_DDL = (
    'CREATE TABLE agent_context_state(singleton INTEGER PRIMARY KEY CHECK(singleton=1),json TEXT NOT NULL,sha256 TEXT NOT NULL)',
    'CREATE TABLE agent_context_nodes(id TEXT PRIMARY KEY,position INTEGER NOT NULL UNIQUE,keys_json TEXT NOT NULL,seal TEXT NOT NULL) WITHOUT ROWID',
    'CREATE TABLE agent_context_refs(source_graph TEXT NOT NULL,claim_ref TEXT NOT NULL,id TEXT NOT NULL,position INTEGER NOT NULL,PRIMARY KEY(source_graph,claim_ref,id)) WITHOUT ROWID',
    'CREATE TABLE agent_context_heads(source_graph TEXT NOT NULL,claim_ref TEXT NOT NULL,n INTEGER NOT NULL,xor_sha256 TEXT NOT NULL,sum_sha256 TEXT NOT NULL,seal TEXT NOT NULL,PRIMARY KEY(source_graph,claim_ref)) WITHOUT ROWID',
)


def _context_schema(b):
    expected = {sql.split()[2].split('(')[0]: sql for sql in _CONTEXT_DDL}
    slots = ','.join('?' for _ in expected)
    actual = dict(b.rows_from(f'SELECT name,{_column("sql", 4096)} FROM sqlite_master WHERE name IN ({slots})', tuple(expected)))
    if actual != expected:
        raise ValueError('Agent context physical schema differs; explicit bootstrap required')
    if b.one(f"SELECT name FROM sqlite_master WHERE type='trigger' AND tbl_name IN ({slots}) LIMIT 1", tuple(expected)):
        raise ValueError('Agent context triggers are outside the declared write mask')


def declaration_profile_sha256():
    import source_witness_bibliographic_graph_common as graph
    raw = Path(graph.__file__).read_bytes()
    return _row_sha({'schema': 'tos_bibliographic_dependency_enumerator_profile_v1',
                     'owner_source_sha256': _digest(raw)})


def execution_profile_sha256():
    import bibliographic_claim_assembler as assembler
    import source_catalog_projection as catalog
    import tos_corpus_index_common as navigation
    import metadata_version_reader as metadata
    from tos_access import source_assembly_normalization as normalization
    files = {'orchestration': Path(__file__), 'assembler': Path(assembler.__file__),
             'source_catalog': Path(catalog.__file__), 'source_navigation': Path(navigation.__file__),
             'metadata_reader': Path(metadata.__file__), 'source_normalization': Path(normalization.__file__)}
    return _row_sha({'schema': REVISION_PROFILE,
                     'owner_sources': {name: _digest(path.read_bytes()) for name, path in files.items()}})


def _profiles(inputs, catalog_inputs, declaration_profile):
    dependencies = inputs.value()['dependencies']
    normalization = k._normalization_binding(catalog_inputs.entity_type_registry, catalog_inputs.relation_type_registry)
    if (catalog_inputs.header['normalization_binding'] != normalization
            or declaration_profile != declaration_profile_sha256()
            or dependencies.get('declaration-profile') != declaration_profile
            or dependencies.get('agent-publication-profile') != execution_profile_sha256()
            or dependencies.get('normalization') != k._stable_digest(catalog_inputs.header['normalization_binding'])
            or dependencies.get('entity-registry') != k._stable_digest(catalog_inputs.entity_type_registry)
            or dependencies.get('relation-registry') != k._stable_digest(catalog_inputs.relation_type_registry)
            or 'nonparticipating-profile' not in dependencies):
        raise ValueError('explicit source/execution/normalization profile differs; bootstrap required')


def source_vector_inputs(*, roots, dependencies, source_publication):
    """Versioned logical identity excludes namespace paths and derived headers.

    Each raw root is an independent input. The owner must explicitly bootstrap
    legacy five-file-hash publications before entering this revision profile.
    """
    if not REQUIRED_ROOTS <= roots.keys():
        raise ValueError('explicit catalog, navigation and bibliographic roots required')
    if any(not isinstance(view, ProjectionSnapshotView) for view in roots.values()):
        raise ValueError('immutable independent raw roots required')
    for view in roots.values():
        if 'source_revision' in view.metadata():
            raise ValueError('raw root cannot contain its derived source revision')
    for role, schema in RAW_SCHEMAS.items():
        if roots[role].metadata() != {'schema_version': schema}:
            raise ValueError('explicit independent raw-row profile required; legacy full roots need bootstrap')
    material = {'schema': REVISION_PROFILE,
        'roots': {role: view.snapshot_digest for role, view in roots.items()},
        'source_publication': source_publication, 'dependencies': dependencies}
    revision = _digest(_json_bytes(material, 1_048_576))
    return PreparedSourceInputs(source_revision=revision, source_publication=source_publication,
                                dependencies=dependencies, roots=roots)


def _require_vector(inputs):
    value = inputs.value()
    rebuilt = source_vector_inputs(roots=inputs.roots(), dependencies=value['dependencies'],
                                   source_publication=value['source_publication'])
    if rebuilt != inputs:
        raise ValueError('legacy or foreign source revision requires explicit bootstrap')


@dataclass(frozen=True)
class AgentPublicationLimits:
    max_nodes: int = 512
    max_relations: int = 1024
    max_claims: int = 64
    max_bytes: int = 16 * 1024 * 1024

    def __post_init__(self):
        if any(type(value) is not int or value < 1 for value in vars(self).values()):
            raise ValueError('positive bounded Agent publication limits required')


class _RawRoots:
    """Shared read budget across addressed raw roots; no materialize fallback."""
    def __init__(self, inputs, limits):
        self.views = inputs.roots()
        budget = _ProjectionBudget(limits)
        self.readers = {name: _SnapshotMutationReader(view, view.snapshot_digest, budget)
                        for name, view in self.views.items() if name != 'source-catalog'}
        for role, fields in RAW_COLLECTIONS.items():
            manifest = self.readers[role].manifest
            if (manifest['logical_schema'] != RAW_SCHEMAS[role] or set(manifest['collections']) != set(fields)
                    or any(manifest['collections'][name]['key_field'] != key
                           or manifest['collections'][name]['order_fields'] != [key]
                           for name, key in fields.items())):
                raise ValueError('raw row collection identity/order differs from Agent source profile')

    def get(self, role, collection, key, *, required=True):
        reader = self.readers[role]
        descriptor = reader.manifest['collections'][collection]['root']
        prefix, hashed = '', _digest(key.encode('utf-8'))
        while descriptor['kind'] == 'index':
            digit = hashed[len(prefix)]
            children = reader._children(descriptor, prefix)
            if digit not in children:
                if required:
                    raise ValueError('required addressed raw row is absent')
                return None
            descriptor, prefix = children[digit], prefix + digit
        row = dict(reader._rows(collection, descriptor, prefix)).get(key)
        if row is None and required:
            raise ValueError('required addressed raw row is absent')
        return row


def _context_keys(node):
    if node.get('kind_id') not in ('claim', 'annotation-claim'):
        return []
    result = set()
    for context in k._assertion_context_values([node], None):
        fields = context.get('fields', {})
        value = fields.get('claim_id', fields.get('claim_ref', {})).get('value')
        if isinstance(value, str):
            result.add((node['source_graph'], value))
    return sorted(result)


def _context_contribution(identity, position):
    return int(_row_sha([identity, position]), 16)


def _head_values(graph, ref, count, xor, total):
    values = [graph, ref, count, f'{xor:064x}', f'{total:064x}']
    return values + [_row_sha(values)]


def _context_head(b, graph, ref):
    row = b.one(f'SELECT n,{_column("xor_sha256", 64)},{_column("sum_sha256", 64)},{_column("seal", 64)} '
                'FROM agent_context_heads WHERE source_graph=? AND claim_ref=?', (graph, ref))
    if row is None:
        return None
    count, xor, total, seal = row
    if (type(count) is not int or count < 1 or type(xor) is not str or len(xor) != 64
            or type(total) is not str or len(total) != 64
            or _row_sha([graph, ref, count, xor, total]) != seal):
        raise ValueError('context group head checksum differs')
    return count, int(xor, 16), int(total, 16)


def _context_members(b, graph, ref, maximum):
    expected = _context_head(b, graph, ref)
    if expected is None:
        return []
    if expected[0] > maximum:
        raise ValueError('complete context contributor group exceeds bound')
    count = xor = total = 0
    result = []
    for identity, position in b.rows_from('SELECT id,position FROM agent_context_refs WHERE source_graph=? AND claim_ref=? ORDER BY position LIMIT ?',
                                          (graph, ref, maximum + 1)):
        count += 1
        value = _context_contribution(identity, position)
        xor, total = xor ^ value, (total + value) % (2**256)
        result.append(identity)
    if (count, xor, total) != expected:
        raise ValueError('complete context contributor group checksum differs')
    return result


def _body_bounded(b, kind, identity):
    b.queries += 3
    if b.queries > b.limits.max_queries:
        raise ValueError('Agent prepared row query budget exceeded')
    value = _body(b.db, kind, identity, PublicationLimits(max_row_bytes=b.limits.max_row_bytes))
    b.rows += 1
    b.read_bytes += len(_json_bytes(value, b.limits.max_row_bytes))
    if b.rows > b.limits.max_rows or b.read_bytes > b.limits.max_read_bytes:
        raise ValueError('Agent prepared row read budget exceeded')
    return value


def bootstrap_agent_context_index_transaction(db, *, source_root, expected_binding, source_inputs,
        catalog_inputs, declaration_profile_sha256, ordered_nodes, ordered_relations,
        source_dossier_refs, progress_owner, limits=None):
    """Explicit full offline attachment; never called by capture or publication.

    The node stream is the full builder's original context encounter order. Its
    full rows must match the already selected publication exactly. Source order
    is not guessed from canonical normalized output or hash-part traversal.
    """
    _require_vector(source_inputs)
    _profiles(source_inputs, catalog_inputs, declaration_profile_sha256)
    if db.execute('PRAGMA journal_mode').fetchone()[0] != 'wal':
        raise ValueError('explicit WAL bootstrap required for independent predecessor readers')
    if read_prepared_source_inputs_transaction(db, expected_binding=expected_binding) != source_inputs:
        raise ValueError('context bootstrap source selection differs')
    with _operation(db, limits, progress_owner) as b:
        from bibliographic_claim_assembler import BibliographicClaimAssembler
        view = source_inputs.roots()['source-catalog']
        catalog = SourceCatalogSnapshot(view, expected_root_sha256=view.snapshot_digest,
                                       trusted_baseline_sha256=view.snapshot_digest)
        assembler = BibliographicClaimAssembler(Path(source_root), catalog_snapshot=catalog)
        _current(b, expected_binding, source_inputs.digest, declaration_profile_sha256)
        if b.one('SELECT count(*) FROM source_dependency_claims')[0] != catalog.header['claim_count']:
            raise ValueError('source dependency bootstrap omitted or added a source Claim')
        for _, row in view.iter_items('claims'):
            b.rows += 1
            if b.rows > b.limits.max_rows:
                raise ValueError('source dependency bootstrap Claim budget exceeded')
            selected = catalog.get_claim(row['claim_id'])
            assembled = assembler.assemble(row['claim_id'], expected_row_sha256=selected.row_sha256)
            exact = SourceClaimDependencies(claim_id=row['claim_id'], source_entry=selected.entry,
                input_sha256=selected.entry['claim_sha256'], dependencies=assembled.dependencies)
            if _claim(b, row['claim_id']) != exact:
                raise ValueError('source dependency bootstrap declaration differs from exact enumerator')
        assembler.verify_current()
        for sql in _CONTEXT_DDL:
            b.execute(sql)
        seen = set()
        dossiers = set()
        for position, node in enumerate(ordered_nodes):
            if node['id'] in seen:
                raise ValueError('duplicate context bootstrap node')
            seen.add(node['id'])
            if _body_bounded(b, 'node', node['id']) != node:
                raise ValueError('context bootstrap row differs from selected prepared row')
            if node.get('source_dossier_ref') is not None:
                dossiers.add(node['source_dossier_ref'])
            keys = _context_keys(node)
            if node['source_graph'] == 'source-claims' and keys and (
                    node['kind_id'] != 'claim' or node['native_id'] != 'claim:' + node['entity_id']
                    or keys != [('source-claims', node['entity_id'])]):
                raise ValueError('descriptive profile requires self-owned bibliographic Claim context groups')
            if node.get('kind_id') in ('claim', 'annotation-claim'):
                encoded_keys = _json_bytes([list(key) for key in keys], b.limits.max_row_bytes).decode()
                b.execute('INSERT INTO agent_context_nodes VALUES (?,?,?,?)',
                          (node['id'], position, encoded_keys, _row_sha([node['id'], position, encoded_keys])))
                for graph, claim in keys:
                    b.execute('INSERT INTO agent_context_refs VALUES (?,?,?,?)', (graph, claim, node['id'], position))
                    count, xor, total = _context_head(b, graph, claim) or (0, 0, 0)
                    contribution = _context_contribution(node['id'], position)
                    b.execute('INSERT INTO agent_context_heads VALUES (?,?,?,?,?,?) ON CONFLICT(source_graph,claim_ref) '
                              'DO UPDATE SET n=excluded.n,xor_sha256=excluded.xor_sha256,sum_sha256=excluded.sum_sha256,seal=excluded.seal',
                              _head_values(graph, claim, count + 1, xor ^ contribution, (total + contribution) % (2**256)))
        count = b.one("SELECT count(*) FROM prepared_documents WHERE kind='node'")[0]
        if count != len(seen):
            raise ValueError('context bootstrap must cover every selected node')
        if dossiers != set(source_dossier_refs):
            raise ValueError('bootstrap dossier membership differs from selected graph')
        seen_relations = set()
        for row in ordered_relations:
            if row['id'] in seen_relations or _body_bounded(b, 'relation', row['id']) != row:
                raise ValueError('relation bootstrap differs from exact prepared source consumer')
            seen_relations.add(row['id'])
            ref = row['source_record']['payload'].get('claim_ref')
            if row['source_graph'] == 'source-claims' and isinstance(ref, str):
                if row['from_id'] != 'source-claims:claim:' + ref:
                    raise ValueError('nonincident bibliographic context consumer requires another profile')
        if b.one("SELECT count(*) FROM prepared_documents WHERE kind='relation'")[0] != len(seen_relations):
            raise ValueError('bootstrap must cover every selected relation consumer')
        state = {'schema': STATE_SCHEMA, 'binding': expected_binding,
                 'source_inputs_sha256': source_inputs.digest,
                 'dossier_refs': sorted(set(source_dossier_refs))}
        raw = _json_bytes(state, b.limits.max_state_bytes)
        b.execute('INSERT INTO agent_context_state VALUES (1,?,?)', (raw.decode(), _digest(raw)))
        return {'schema': STATE_SCHEMA, 'publication_changed': False,
                'source_completeness_verified': False, 'bootstrap_node_count': len(seen)}


def _state(b, binding, inputs):
    _context_schema(b)
    if b.one('PRAGMA journal_mode')[0] != 'wal':
        raise ValueError('Agent publication requires the explicit WAL storage profile')
    row = b.one(f'SELECT CASE WHEN length(CAST(json AS BLOB))<=? THEN json END,{_column("sha256", 64)} FROM agent_context_state WHERE singleton=1',
                (b.limits.max_state_bytes,))
    if row is None or type(row[0]) is not str or _digest(row[0].encode()) != row[1]:
        raise ValueError('context index state missing or differs')
    value = _strict_json(row[0])
    if (value.get('schema') != STATE_SCHEMA or value.get('binding') != binding
            or value.get('source_inputs_sha256') != inputs.digest):
        raise ValueError('context index binds another publication')
    return value


def _put_unique(target, key, value):
    if key in target and _row_sha(target[key]) != _row_sha(value):
        raise ValueError('source cohort contributors disagree')
    target[key] = value


def _cohort(assembler, catalog, record_id, declarations):
    record = assembler.assemble_record(record_id, expected_row_sha256=catalog.get(record_id).row_sha256)
    navigation = record.project_navigation()
    nodes = {('source-navigation', row['node_id']): row for row in navigation.nodes}
    identity = record.project_bibliographic()
    nodes[('source-claims', identity['node_id'])] = identity
    edges = {('source-navigation', row['edge_id']): row for row in navigation.edges}
    traces = {}
    for declaration in declarations:
        claim_id = declaration['claim_id']
        selected = catalog.get_claim(claim_id)
        assembled = assembler.assemble(claim_id, expected_row_sha256=selected.row_sha256)
        rebuilt = SourceClaimDependencies(claim_id=claim_id, source_entry=selected.entry,
            input_sha256=selected.entry['claim_sha256'], dependencies=assembled.dependencies)
        if rebuilt.digest != declaration['digest']:
            raise ValueError('current enumerated Claim dependencies differ from retained reverse declaration')
        projected = assembled.project()
        for row in projected.nodes:
            _put_unique(nodes, ('source-claims', row['node_id']), row)
        for row in projected.edges:
            _put_unique(edges, ('source-claims', row['edge_id']), row)
        _put_unique(traces, projected.trace['claim_ref'], projected.trace)
    return {'nodes': nodes, 'edges': edges, 'traces': traces}


def _role(graph):
    return {'source-navigation': 'source-navigation', 'source-claims': 'bibliographic-claims'}[graph]


def _row_sha(row):
    return _digest(_json_bytes(row, 16 * 1024 * 1024))


def _stage_raw(before, old, new, mutation_limits):
    changes = {role: [] for role in ('source-navigation', 'bibliographic-claims')}
    for field, collection in (('nodes', 'nodes'), ('edges', 'edges')):
        for graph, key in old[field].keys() | new[field].keys():
            previous, successor = old[field].get((graph, key)), new[field].get((graph, key))
            if (previous is None) != (successor is None) or (previous is not None and _row_sha(previous) != _row_sha(successor)):
                changes[_role(graph)].append(ProjectionChange(collection, key,
                    previous is not None, None if previous is None else _row_sha(previous),
                    successor is not None, successor))
    # Descriptive record changes cannot alter source Claim traces.
    if _row_sha(old['traces']) != _row_sha(new['traces']):
        raise ValueError('descriptive Agent profile cannot change Claim trace membership or content')
    roots = before.roots()
    for role, delta in changes.items():
        if delta:
            roots[role] = stage_projection_snapshot_changes(roots[role],
                expected_before_sha256=roots[role].snapshot_digest,
                trusted_baseline_sha256=roots[role].snapshot_digest, changes=delta,
                limits=mutation_limits).snapshot()
    return roots


def _cohort_bytes(value, limits):
    if (len(value['nodes']) > limits.max_nodes or len(value['edges']) > limits.max_relations
            or len(value['traces']) > limits.max_claims):
        raise ValueError('captured source cohort exceeds declared count bounds')
    return _json_bytes({field: [[list(key), row] for key, row in value[field].items()]
                       for field in ('nodes', 'edges')} | {'traces': value['traces']}, limits.max_bytes)


def _cohort_value(raw):
    value = _strict_json(raw)
    return {field: {tuple(key): row for key, row in value[field]} for field in ('nodes', 'edges')} | {'traces': value['traces']}


@dataclass(frozen=True)
class CapturedAgentCorrection:
    source_root: Path
    record_id: str
    source_inputs: PreparedSourceInputs
    binding_raw: bytes
    cohort_raw: bytes
    declarations_raw: bytes
    catalog_inputs: CatalogInputs
    declaration_profile_sha256: str
    limits: AgentPublicationLimits
    mutation_limits: MutationLimits
    dependency_limits: SourceDependencyLimits

    @property
    def expected_binding(self):
        return _strict_json(self.binding_raw)


def capture_agent_correction(db, *, source_root, record_id, expected_binding,
        catalog_inputs, declaration_profile_sha256, progress_owner,
        limits=None, mutation_limits=None, dependency_limits=None):
    """Capture current-only source inputs BEFORE invoking record.revise.

    The caller must close its transaction before this operation and before the
    separately authorized source command. This helper never executes commands.
    """
    from bibliographic_claim_assembler import BibliographicClaimAssembler
    import source_commands as source
    limits = limits or AgentPublicationLimits()
    mutation_limits = mutation_limits or MutationLimits()
    dependency_limits = dependency_limits or SourceDependencyLimits(max_claims=limits.max_claims)
    if db.in_transaction or catalog_inputs.source_order_profile != CANONICAL_ORDER:
        raise ValueError('capture requires no caller transaction and canonical explicit bootstrap')
    root = Path(source_root).absolute()
    with source._locked(root / 'ToS/source-witnesses/historical-create'):
        db.execute('BEGIN')
        try:
            inputs = read_prepared_source_inputs_transaction(db, expected_binding=expected_binding)
            _catalog_selected(db, expected_binding, catalog_inputs, PublicationLimits())
            _require_vector(inputs)
            _profiles(inputs, catalog_inputs, declaration_profile_sha256)
            selected = inputs.roots()['source-catalog']
            catalog = SourceCatalogSnapshot(selected, expected_root_sha256=selected.snapshot_digest,
                trusted_baseline_sha256=selected.snapshot_digest, limits=mutation_limits)
            if catalog.header['source_publication']['token'] != inputs.value()['source_publication']:
                raise ValueError('source catalog and prepared epoch disagree')
            lookup_args = dict(expected_binding=expected_binding, source_inputs_sha256=inputs.digest,
                declaration_profile_sha256=declaration_profile_sha256,
                progress_owner=progress_owner, limits=dependency_limits)
            unknown = lookup_source_dependencies_transaction(db, kind='unresolved', ref=None, **lookup_args)
            if unknown['claim_ids']:
                raise ValueError('unresolved source declarations prohibit bounded descriptive publication')
            result = lookup_source_dependencies_transaction(db, kind='identity', ref=record_id, **lookup_args)
            declarations = result['declarations']
            assembler = BibliographicClaimAssembler(root, catalog_snapshot=catalog)
            if (assembler.entity_registry != catalog_inputs.entity_type_registry
                    or assembler.navigation_registry != catalog_inputs.relation_type_registry):
                raise ValueError('selected source registries differ from normalization owner inputs')
            cohort = _cohort(assembler, catalog, record_id, declarations)
            raw = _RawRoots(inputs, mutation_limits)
            for field, collection in (('nodes', 'nodes'), ('edges', 'edges')):
                for (graph, key), row in cohort[field].items():
                    selected_row = raw.get(_role(graph), collection, key)
                    if _row_sha(selected_row) != _row_sha(row):
                        fields = sorted(name for name in selected_row.keys() | row.keys() if selected_row.get(name) != row.get(name))
                        raise ValueError(f'current projected cohort differs from selected raw root: {graph}:{key} fields={fields}')
            for key, trace in cohort['traces'].items():
                if _row_sha(raw.get('bibliographic-claims', 'claim_traces', key)) != _row_sha(trace):
                    raise ValueError('current Claim trace differs from selected raw root')
            with _operation(db, dependency_limits, progress_owner) as b:
                _state(b, expected_binding, inputs)
            assembler.verify_current()
            return CapturedAgentCorrection(root, record_id, inputs,
                _json_bytes(expected_binding, 1_048_576), _cohort_bytes(cohort, limits),
                _json_bytes(declarations, limits.max_bytes), CatalogInputs(catalog_inputs.header,
                    catalog_inputs.entity_type_registry, catalog_inputs.relation_type_registry,
                    catalog_inputs.lenses, source_order_profile=catalog_inputs.source_order_profile),
                declaration_profile_sha256, limits, mutation_limits, dependency_limits)
        finally:
            db.rollback()


def _selected_relations(b, node_ids, maximum):
    found = set()
    indexes = list(b.rows_from('PRAGMA index_list(knowledge_relations)'))
    for column in ('from_id', 'to_id'):
        name = 'knowledge_relations_' + ('from' if column == 'from_id' else 'to') + '_seek'
        actual = [row for row in b.rows_from(f'PRAGMA index_xinfo({name})') if row[5]]
        if (not any(row[1] == name and row[4] == 0 for row in indexes)
                or tuple(row[2] for row in actual) != (column, 'id')
                or any(row[3] != 0 or row[4] != 'BINARY' for row in actual)):
            raise ValueError('complete incidence requires exact prepared physical seek indexes')
    for identifier in node_ids:
        for column in ('from_id', 'to_id'):
            index = 'knowledge_relations_' + ('from' if column == 'from_id' else 'to') + '_seek'
            rows = b.rows_from(f'SELECT id FROM knowledge_relations INDEXED BY {index} WHERE {column}=? ORDER BY id LIMIT ?',
                              (identifier, maximum + 1))
            for (identity,) in rows:
                found.add(identity)
                if len(found) > maximum:
                    raise ValueError('complete Agent incidence exceeds relation budget')
    return found


def _normalize_pair(db, captured, old, new, progress_owner):
    """Assemble complete changed-node incidence and exact context contributors."""
    limits = captured.limits
    if old['nodes'].keys() - new['nodes'].keys() or old['edges'].keys() - new['edges'].keys():
        raise ValueError('Agent descriptive profile cannot delete raw topology')
    changed = {key for key, value in new['nodes'].items()
               if key not in old['nodes'] or _row_sha(old['nodes'][key]) != _row_sha(value)}
    # A literal inherits its governing Claim's contexts even when its raw value
    # did not change. Native bibliographic literal IDs are Claim-scoped.
    for ref, trace in old['traces'].items():
        key = ('source-claims', trace['object_node_id'])
        value = old['nodes'].get(key)
        if (('source-claims', trace['claim_node_id']) in changed and value is not None
                and value.get('node_kind') == 'literal'):
            if value.get('properties', {}).get('claim_ref') != ref:
                raise ValueError('literal context ownership is not Claim-scoped')
            changed.add(key)
    if len(changed) > limits.max_nodes:
        raise ValueError('Agent changed node budget exceeded')
    selected_ids = {graph + ':' + native for graph, native in changed}
    raw = _RawRoots(captured.source_inputs, captured.mutation_limits)
    with _operation(db, captured.dependency_limits, progress_owner) as b:
        state = _state(b, captured.expected_binding, captured.source_inputs)
        edge_ids = _selected_relations(b, selected_ids, limits.max_relations)
        old_relations = {identity: _body_bounded(b, 'relation', identity) for identity in edge_ids}
        edge_specs = {identity: {'source_graph': row['source_graph'],
            'record': row['source_record']['payload'], 'identity_id': k._addressed_relation_identity(row)}
            for identity, row in old_relations.items()}
        for (graph, native), successor in new['edges'].items():
            if (graph, native) not in old['edges'] or _row_sha(old['edges'][graph, native]) != _row_sha(successor):
                identity = graph + ':' + native
                if identity in old_relations:
                    previous = old_relations[identity]
                    # Existing normalized identity is retained for cross-source adapters.
                    edge_specs[identity] = {**edge_specs[identity], 'record': successor}
                else:
                    if (graph, native) in old['edges']:
                        raise ValueError('changed source edge lies outside complete changed-node incidence')
                    edge_specs[identity] = {'source_graph': graph, 'record': successor}
        if len(edge_specs) > limits.max_relations:
            raise ValueError('successor incidence exceeds relation budget')
        retained_ids = set()
        claim_refs = set()
        for spec in edge_specs.values():
            graph, row = spec['source_graph'], spec['record']
            for end in ('from', 'to'):
                retained_ids.add((row.get(end + '_source_graph') or graph) + ':' + row[end + '_id'])
            if isinstance(row.get('claim_ref'), str):
                claim_refs.add((graph, row['claim_ref']))
        traces = dict(old['traces'])
        for graph, ref in claim_refs:
            if graph == 'source-claims' and ref not in traces:
                traces[ref] = raw.get('bibliographic-claims', 'claim_traces', ref)
        for trace in traces.values():
            retained_ids.update('source-claims:' + trace[field] for field in
                                ('claim_node_id', 'subject_node_id', 'object_node_id'))
        for graph, ref in claim_refs:
            for identity in _context_members(b, graph, ref, limits.max_nodes):
                retained_ids.add(identity)
        retained_ids -= selected_ids
        if len(retained_ids | selected_ids) > limits.max_nodes:
            raise ValueError('Agent endpoint/context closure exceeds node budget')
        retained = {identity: _body_bounded(b, 'node', identity) for identity in retained_ids}
        previous_nodes = {graph + ':' + native: _body_bounded(b, 'node', graph + ':' + native)
                          for graph, native in changed if (graph, native) in old['nodes']}
        contexts = {}
        for identity, row in {**retained, **previous_nodes}.items():
            if row.get('kind_id') in ('claim', 'annotation-claim'):
                indexed = b.one(f'SELECT position,{_column("keys_json", b.limits.max_row_bytes)},{_column("seal", 64)} '
                                'FROM agent_context_nodes WHERE id=?', (identity,))
                if (indexed is None or _row_sha([identity, indexed[0], indexed[1]]) != indexed[2]
                        or _strict_json(indexed[1]) != [list(key) for key in _context_keys(row)]):
                    raise ValueError('Claim context index differs from selected prepared contributor')
                for graph, ref in _context_keys(row):
                    if identity not in _context_members(b, graph, ref, limits.max_nodes):
                        raise ValueError('Claim context group omitted its selected contributor')
                contexts[identity] = indexed[0]
        ordered = sorted(contexts, key=contexts.__getitem__)
        args = dict(retained_nodes=list(retained.values()), claim_traces=list(traces.values()),
            source_dossier_refs=state['dossier_refs'], context_node_order=ordered,
            normalization_binding=captured.catalog_inputs.header['normalization_binding'],
            entity_registry=captured.catalog_inputs.entity_type_registry,
            relation_registry=captured.catalog_inputs.relation_type_registry)
        before_specs = [{'source_graph': graph, 'record': old['nodes'][(graph, key)]}
                        for graph, key in changed if (graph, key) in old['nodes']]
        before_args = {**args, 'relation_records': [{'source_graph': row['source_graph'],
            'record': row['source_record']['payload'], 'identity_id': k._addressed_relation_identity(row)}
            for row in old_relations.values()], 'node_records': before_specs}
        # New history endpoints do not exist in the predecessor closure.
        new_ids = selected_ids - previous_nodes.keys()
        before_args['retained_nodes'] = [row for row in args['retained_nodes'] if row['id'] not in new_ids]
        previous = normalize_source_assembly_candidate(**before_args)
        if ({row['id']: row for row in previous['nodes']} != previous_nodes
                or {row['id']: row for row in previous['relations']} != old_relations):
            raise ValueError('captured source closure does not reproduce exact prepared predecessor')
        successor = normalize_source_assembly_candidate(**args,
            node_records=[{'source_graph': graph, 'record': new['nodes'][(graph, key)]} for graph, key in changed],
            relation_records=list(edge_specs.values()))
        for row in successor['nodes']:
            old_row = previous_nodes.get(row['id'])
            if _context_keys(row) != ([] if old_row is None else _context_keys(old_row)):
                raise ValueError('descriptive correction cannot change context membership')
        for row in successor['relations']:
            old_row = old_relations.get(row['id'])
            if old_row is not None and any(row.get(key) != old_row.get(key)
                    for key in ('from_id', 'to_id', 'view_ids', 'relation_type_id')):
                raise ValueError('descriptive correction cannot change existing relation topology/views')
        return previous, successor


def _changes(db, old, new, limits, progress_owner):
    """Canonical neighbors allocate sparse insertion order, never append order."""
    result = []
    with _operation(db, limits, progress_owner) as b:
        for kind, field in (('node', 'nodes'), ('relation', 'relations')):
            before = {row['id']: row for row in old[field]}
            rows = sorted(new[field], key=lambda row: (row['source_graph'], row['id']))
            inserted = []
            for row in rows:
                previous = before.get(row['id'])
                if previous is not None and _row_sha(previous) == _row_sha(row):
                    continue
                token = None
                if previous is None:
                    key = order_key((row['source_graph'], row['id']))
                    neighbors = []
                    for operator, direction, default in (('<', 'DESC', 0), ('>', 'ASC', 9_007_199_254_740_991)):
                        found = b.one(f'SELECT c.id,c.source_order,p.source_order,c.row_digest,c.facts_digest,'
                            'CASE WHEN length(CAST(c.summary AS BLOB))<=? THEN c.summary END,c.seal '
                            'FROM catalog_contributors c JOIN prepared_documents p '
                            f'ON p.kind=c.kind AND p.id=c.id WHERE c.kind=? AND c.source_order{operator}? '
                            f'ORDER BY c.source_order {direction} LIMIT 1', (b.limits.max_row_bytes, kind, key))
                        if found is not None:
                            identity, order, position, row_digest, facts_digest, summary, seal = found
                            body = _body_bounded(b, kind, identity)
                            semantic = b.one('SELECT source_order FROM semantic_rows WHERE kind=? AND id=?', (kind, identity))
                            if (type(summary) is not str or CatalogIndex._seal(kind, identity, order, row_digest, facts_digest, summary) != seal
                                    or order_key((body['source_graph'], body['id'])) != order
                                    or catalog_digest(body) != row_digest or semantic != (position,)):
                                raise ValueError('canonical insertion neighbor differs across prepared indexes')
                        neighbors.append(default if found is None else found[2])
                    lower = max([neighbors[0], *[position for order, position in inserted if order < key]])
                    upper = neighbors[1]
                    if upper - lower < 2:
                        raise ValueError('canonical sparse insertion gap exhausted; explicit rebootstrap required')
                    token = (lower + upper) // 2
                    inserted.append((key, token))
                result.append(PreparedChange('insert' if previous is None else 'update', kind, row['id'], row, token))
    return result


class AgentCorrectionPublication:
    """A source-lock-scoped candidate; cannot survive its context manager."""
    def __init__(self, captured, transaction_id, token, progress_owner):
        from bibliographic_claim_assembler import BibliographicClaimAssembler
        import source_metadata_transactions as transactions
        self.captured, self.transaction_id, self.token = captured, transaction_id, token
        self.progress_owner, self.active, self.committed = progress_owner, True, False
        self.db = None
        root = captured.source_inputs.roots()['source-catalog']
        before_catalog = SourceCatalogSnapshot(root, expected_root_sha256=root.snapshot_digest,
            trusted_baseline_sha256=root.snapshot_digest, limits=captured.mutation_limits)
        self.catalog_candidate = stage_agent_catalog_transition(captured.source_root, before_catalog,
            transaction_id=transaction_id, expected_publication_token=token, mutation_limits=captured.mutation_limits)
        retained = transactions.inspect_transaction(captured.source_root, transaction_id)
        authorization = retained['plan']['authorization']
        if (authorization['record_id'] != captured.record_id
                or set(authorization['request']['fields']) - {'preferred_label', 'notes', 'field_languages'}):
            raise ValueError('prepared Agent profile forbids source reference/membership edits')
        self.transition_digest = retained['manifest_sha256']
        view = self.catalog_candidate.snapshot()
        self.catalog = SourceCatalogSnapshot(view, expected_root_sha256=view.snapshot_digest,
            trusted_baseline_sha256=view.snapshot_digest, limits=captured.mutation_limits)
        self.assembler = BibliographicClaimAssembler(captured.source_root, catalog_snapshot=self.catalog)
        self._new_raw = _cohort_bytes(_cohort(self.assembler, self.catalog, captured.record_id,
                           _strict_json(captured.declarations_raw)), captured.limits)
        self.verify_current()

    def verify_current(self):
        import source_metadata_transactions as transactions
        if not self.active:
            raise ValueError('source publication scope already closed')
        self.assembler.verify_current()
        _profiles(self.captured.source_inputs, self.captured.catalog_inputs, self.captured.declaration_profile_sha256)
        current = transactions.inspect_transaction(self.captured.source_root, self.transaction_id)
        if (current['status'] != 'committed' or not current['is_current_publication']
                or current['publication']['token'] != self.token
                or current['manifest_sha256'] != self.transition_digest):
            raise ValueError('retained source successor is no longer exact current publication')

    def apply_transaction(self, db, *, publication_limits=None, catalog_limits=None, semantic_limits=None):
        if not self.active or self.db is not None or not db.in_transaction:
            raise ValueError('one active explicit publication transaction required')
        self.db = db
        captured = self.captured
        publication_limits = publication_limits or PublicationLimits()
        if publication_limits.max_mutations <= 1:
            raise ValueError('Agent publication requires final context-binding mutation allowance')
        start = db.total_changes
        if read_prepared_source_inputs_transaction(db, expected_binding=captured.expected_binding) != captured.source_inputs:
            raise ValueError('prepared predecessor advanced after Agent capture')
        old, new = _cohort_value(captured.cohort_raw), _cohort_value(self._new_raw)
        previous, successor = _normalize_pair(db, captured, old, new, self.progress_owner)
        roots = _stage_raw(captured.source_inputs, old, new, captured.mutation_limits)
        roots['source-catalog'] = self.catalog_candidate.snapshot()
        after_source = source_vector_inputs(roots=roots,
            dependencies=captured.source_inputs.value()['dependencies'], source_publication=self.token)
        header = captured.catalog_inputs.header
        header['source_revision'] = after_source.value()['source_revision']
        after_inputs = CatalogInputs(header, captured.catalog_inputs.entity_type_registry,
            captured.catalog_inputs.relation_type_registry, captured.catalog_inputs.lenses,
            source_order_profile=CANONICAL_ORDER)
        changes = _changes(db, previous, successor, captured.dependency_limits, self.progress_owner)
        result = apply_dependency_bound_prepared_delta_transaction(db,
            expected_binding=captured.expected_binding, before_source_inputs=captured.source_inputs,
            after_source_inputs=after_source, before_inputs=captured.catalog_inputs, after_inputs=after_inputs,
            changes=changes, dependency_changes=[], declaration_profile_sha256=captured.declaration_profile_sha256,
            progress_owner=self.progress_owner, limits=replace(publication_limits, max_mutations=publication_limits.max_mutations - 1),
            dependency_limits=captured.dependency_limits, catalog_limits=catalog_limits, semantic_limits=semantic_limits)
        with _operation(db, captured.dependency_limits, self.progress_owner) as b:
            state = _state(b, captured.expected_binding, captured.source_inputs)
            state.update(binding=result['binding'], source_inputs_sha256=after_source.digest)
            raw = _json_bytes(state, b.limits.max_state_bytes)
            b.execute('UPDATE agent_context_state SET json=?,sha256=? WHERE singleton=1', (raw.decode(), _digest(raw)))
        if db.total_changes - start > publication_limits.max_mutations:
            raise ValueError('Agent publication combined SQLite mutation allowance exceeded; rollback required')
        self.verify_current()
        result = {**result, 'sql_mutations': db.total_changes - start,
            'normalized_node_count': len(successor['nodes']), 'normalized_relation_count': len(successor['relations']),
            'changed_nodes': sum(change.kind == 'node' for change in changes),
            'changed_relations': sum(change.kind == 'relation' for change in changes),
            'reverse_dependent_claims': len(_strict_json(captured.declarations_raw)),
            'schema': 'tos_agent_correction_publication_v1',
            'transaction_id': self.transaction_id, 'source_manifest_sha256': self.transition_digest,
            'source_command_committed': True, 'prepared_committed': False,
            'source_transition_verified': True, 'descriptive_closure_verified': True,
            'cross_filesystem_atomic': False, 'global_source_currentness_verified': False,
            'is_semantic_acceptance': False}
        self._result_raw = _json_bytes(result, captured.limits.max_bytes)
        self._applied_total_changes = db.total_changes
        # DDL does not increment total_changes. Retain SQLite's schema cookie
        # as well so a later DROP INDEX or CREATE TRIGGER cannot slip through
        # the guarded commit after all publication lanes were verified.
        self._applied_schema_version = db.execute('PRAGMA main.schema_version').fetchone()
        return self.result

    @property
    def result(self):
        return _strict_json(self._result_raw)

    def commit_transaction(self, db):
        """Explicit caller-requested commit while holding the source writer lock."""
        if db is not self.db or not db.in_transaction or not hasattr(self, '_result_raw') or self.committed:
            raise ValueError('exact successfully applied caller transaction required')
        if db.total_changes != self._applied_total_changes:
            raise ValueError('caller wrote after verified publication; complete rollback required')
        if db.execute('PRAGMA main.schema_version').fetchone() != self._applied_schema_version:
            raise ValueError('schema changed after verified publication; complete rollback required')
        result = self.result
        paired = read_prepared_source_inputs_transaction(db, expected_binding=result['binding'])
        if paired.digest != result['source_inputs_sha256']:
            raise ValueError('paired source selection changed before guarded commit')
        self.verify_current()
        db.commit()
        self.committed = True
        return {**result, 'prepared_committed': True}


@contextmanager
def agent_correction_publication(captured, *, transaction_id, expected_source_token, progress_owner):
    """Reacquire source-owner lock; keep it through the explicit prepared commit.

    Failure requires complete SQLite rollback by the caller. Immutable staged
    parts may remain unselected. No source rollback, live selection, or fallback
    bootstrap is performed here.
    """
    import source_commands as source
    if type(captured) is not CapturedAgentCorrection or type(progress_owner) is not ProgressHandlerOwner:
        raise ValueError('exact captured correction and progress owner required')
    with source._locked(captured.source_root / 'ToS/source-witnesses/historical-create'):
        candidate = AgentCorrectionPublication(captured, transaction_id, expected_source_token, progress_owner)
        try:
            yield candidate
            if candidate.db is not None and not candidate.committed:
                raise ValueError('use the guarded commit; unfinished application requires caller rollback')
        finally:
            candidate.active = False
