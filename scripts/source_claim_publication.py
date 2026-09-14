"""Guarded initial Claim addition through every prepared reader lane.

Source creation has already committed. Failure never rolls it back. An exact
admitted predecessor, current source grant and explicit caller transaction are
required; this is neither bootstrap nor a consumer activation route.
"""
from contextlib import contextmanager
from dataclasses import replace
from pathlib import Path

import source_agent_publication as shared
from source_claim_catalog import claim_catalog_addition
from source_catalog_projection import SourceCatalogSnapshot, _read_owned
from bibliographic_claim_assembler import BibliographicClaimAssembler
from tos_access.catalog_semantics import CatalogInputs, CANONICAL_ORDER
from tos_access.prepared_source_dependencies import (
    SourceClaimDependencies, SourceDependencyChange, SourceDependencyLimits,
    _operation, _claim, _column, lookup_source_dependencies_transaction,
)
from tos_access.prepared_source_publication import apply_dependency_bound_prepared_delta_transaction
from tos_access.projection_mutation import (
    ProjectionChange, stage_projection_snapshot_changes, MutationLimits,
    _SnapshotMutationReader, _Budget as ProjectionBudget,
)
from tos_access.projection_store import _strict_json, _digest
from tos_access.source_assembly_normalization import normalize_source_assembly_candidate, AssemblyNormalizationLimits

PROFILE_KEY = 'claim-publication-profile'


class _CachedPartReader(_SnapshotMutationReader):
    """Keep already verified immutable bytes, bounded by the shared read budget."""
    def __init__(self, *args):
        self._parts, self.cache_hits = {}, 0
        super().__init__(*args)

    def _load(self, descriptor, prefix):
        self._descriptor(descriptor, prefix)
        key = (prefix, shared._json_bytes(descriptor, 65536))
        if key in self._parts:
            self.cache_hits += 1
            return self._parts[key]
        raw = super()._load(descriptor, prefix)
        self._parts[key] = raw
        return raw


class _ClaimRawRoots(shared._RawRoots):
    """The same declared raw-root ABI, with operation-local checked part reuse."""
    def __init__(self, inputs, limits):
        self.views, self.budget = inputs.roots(), ProjectionBudget(limits)
        self.readers = {name: _CachedPartReader(view, view.snapshot_digest, self.budget)
                        for name, view in self.views.items() if name != 'source-catalog'}
        for role, fields in shared.RAW_COLLECTIONS.items():
            manifest = self.readers[role].manifest
            if (manifest['logical_schema'] != shared.RAW_SCHEMAS[role]
                    or set(manifest['collections']) != set(fields)
                    or any(manifest['collections'][name]['key_field'] != key
                           or manifest['collections'][name]['order_fields'] != [key]
                           for name, key in fields.items())):
                raise ValueError('Claim publication raw-root collection profile differs')

    def accounting(self):
        return {**self.budget.usage,
                'cache_hits': sum(reader.cache_hits for reader in self.readers.values()),
                'cached_bytes': sum(len(raw) for reader in self.readers.values() for raw in reader._parts.values())}


def execution_profile_sha256():
    root = Path(__file__).resolve().parents[1]
    return shared._row_sha({'schema': 'tos_initial_claim_publication_v1', 'modules': {
        ref: _digest(_read_owned(root / ref, 1_048_576))
        for ref in ('scripts/source_claim_publication.py', 'scripts/source_claim_catalog.py')}})


def _assembled(addition, limits):
    view = addition.candidate.snapshot()
    catalog = SourceCatalogSnapshot(view, expected_root_sha256=view.snapshot_digest,
        trusted_baseline_sha256=view.snapshot_digest, limits=limits)
    assembler = BibliographicClaimAssembler(addition.root, catalog_snapshot=catalog)
    raw = {'nodes': {}, 'edges': {}, 'traces': {}}
    declarations = []
    for identity in addition.candidate.verification['claim_ids']:
        selected = catalog.get_claim(identity)
        assembled = assembler.assemble(identity, expected_row_sha256=selected.row_sha256)
        declarations.append(SourceClaimDependencies(claim_id=identity, source_entry=selected.entry,
            input_sha256=selected.entry['claim_sha256'], dependencies=assembled.dependencies))
        result = assembled.project()
        for field, rows, key in (('nodes', result.nodes, 'node_id'), ('edges', result.edges, 'edge_id')):
            for row in rows:
                shared._put_unique(raw[field], ('source-claims', row[key]), row)
        shared._put_unique(raw['traces'], identity, result.trace)
    assembler.verify_current()
    return catalog, assembler, raw, declarations


def _normalize(db, operation):
    """Complete incidence of changed/shared cohort nodes, never recursive expansion."""
    limits, owner, new = operation.limits, operation.progress_owner, operation.raw
    reader = _ClaimRawRoots(operation.source_inputs, operation.mutation_limits)
    old = {'nodes': {}, 'edges': {}, 'traces': {}}
    for (graph, key), row in new['nodes'].items():
        previous = reader.get(shared._role(graph), 'nodes', key, required=False)
        if previous is not None:
            if shared._row_sha(previous) != shared._row_sha(row):
                raise ValueError('shared source carrier changed outside initial Claim addition')
            old['nodes'][graph, key] = previous
    for (graph, key), row in new['edges'].items():
        if reader.get(shared._role(graph), 'edges', key, required=False) is not None:
            raise ValueError('new Claim relation already belongs to predecessor raw inputs')
    for identity in new['traces']:
        if reader.get('bibliographic-claims', 'claim_traces', identity, required=False) is not None:
            raise ValueError('new Claim trace already belongs to predecessor')
    affected = {graph + ':' + native for graph, native in new['nodes']}
    if len(affected) > limits.max_nodes:
        raise ValueError('Claim addition node budget exceeded')
    with _operation(db, operation.dependency_limits, owner) as b:
        state = shared._state(b, operation.binding, operation.source_inputs)
        for declaration in operation.declarations:
            if _claim(b, declaration.value()['claim_id']) is not None:
                raise ValueError('new Claim declaration already belongs to predecessor')
        incident = shared._selected_relations(b, affected, limits.max_relations)
        prior_relations = {identity: shared._body_bounded(b, 'relation', identity) for identity in incident}
        specs = {identity: {'source_graph': row['source_graph'], 'record': row['source_record']['payload'],
            'identity_id': shared.k._addressed_relation_identity(row)} for identity, row in prior_relations.items()}
        for (graph, key), row in new['edges'].items():
            identity = graph + ':' + key
            if identity in specs:
                raise ValueError('new relation collides with prepared incidence')
            specs[identity] = {'source_graph': graph, 'record': row}
        if len(specs) > limits.max_relations:
            raise ValueError('Claim addition complete incidence budget exceeded')
        support, references = set(), set()
        for spec in specs.values():
            graph, row = spec['source_graph'], spec['record']
            support.update((row.get(end + '_source_graph') or graph) + ':' + row[end + '_id']
                           for end in ('from', 'to'))
            if isinstance(row.get('claim_ref'), str):
                references.add((graph, row['claim_ref']))
        old_traces = {}
        for graph, ref in sorted(references):
            if graph == 'source-claims' and ref not in new['traces']:
                if len(old_traces) + len(new['traces']) >= limits.max_claims:
                    raise ValueError('Claim addition governing trace budget exceeded')
                old_traces[ref] = reader.get('bibliographic-claims', 'claim_traces', ref)
            if ref in new['traces'] and graph == 'source-claims':
                if shared._context_head(b, graph, ref) is not None:
                    raise ValueError('new Claim context group is already occupied')
            else:
                support.update(shared._context_members(b, graph, ref, limits.max_nodes))
        for trace in (*old_traces.values(), *new['traces'].values()):
            support.update('source-claims:' + trace[field]
                           for field in ('claim_node_id', 'subject_node_id', 'object_node_id'))
        support -= affected
        if len(support | affected) > limits.max_nodes:
            raise ValueError('Claim addition endpoint/context closure budget exceeded')
        retained = {identity: shared._body_bounded(b, 'node', identity) for identity in support}
        previous_nodes = {graph + ':' + key: shared._body_bounded(b, 'node', graph + ':' + key)
                          for graph, key in old['nodes']}
        contexts = {}
        for identity, row in {**retained, **previous_nodes}.items():
            if row.get('kind_id') not in ('claim', 'annotation-claim'):
                continue
            indexed = b.one(f'SELECT position,{_column("keys_json", b.limits.max_row_bytes)},seal '
                            'FROM agent_context_nodes WHERE id=?', (identity,))
            if (indexed is None or shared._row_sha([identity, indexed[0], indexed[1]]) != indexed[2]
                    or _strict_json(indexed[1]) != [list(key) for key in shared._context_keys(row)]):
                raise ValueError('retained context contributor differs from admitted index')
            for graph, ref in shared._context_keys(row):
                if identity not in shared._context_members(b, graph, ref, limits.max_nodes):
                    raise ValueError('retained context contributor is omitted from its group')
            contexts[identity] = indexed[0]
        order = sorted(contexts, key=contexts.__getitem__)
        args = dict(retained_nodes=list(retained.values()), source_dossier_refs=state['dossier_refs'],
            normalization_binding=operation.inputs.header['normalization_binding'],
            entity_registry=operation.inputs.entity_type_registry, relation_registry=operation.inputs.relation_type_registry,
            limits=AssemblyNormalizationLimits(max_nodes=limits.max_nodes, max_retained_nodes=limits.max_nodes,
                max_relations=limits.max_relations, max_traces=limits.max_claims,
                max_input_bytes=limits.max_bytes, max_output_bytes=limits.max_bytes))
        previous = normalize_source_assembly_candidate(**args,
            node_records=[{'source_graph': graph, 'record': row} for (graph, _), row in old['nodes'].items()],
            relation_records=[specs[identity] for identity in prior_relations],
            claim_traces=list(old_traces.values()), context_node_order=order)
        if ({row['id']: row for row in previous['nodes']} != previous_nodes
                or {row['id']: row for row in previous['relations']} != prior_relations):
            raise ValueError('source closure does not reproduce exact prepared predecessor')
        new_contexts = ['source-claims:' + trace['claim_node_id'] for trace in new['traces'].values()]
        successor = normalize_source_assembly_candidate(**args,
            node_records=[{'source_graph': graph, 'record': row} for (graph, _), row in new['nodes'].items()],
            relation_records=list(specs.values()), claim_traces=[*old_traces.values(), *new['traces'].values()],
            context_node_order=[*order, *new_contexts])
        for row in successor['nodes']:
            if row['id'] in new_contexts:
                if shared._context_keys(row) != [('source-claims', row['entity_id'])]:
                    raise ValueError('new Claim must own one independent singleton context group')
            elif shared._context_keys(row) != shared._context_keys(previous_nodes.get(row['id'], {})):
                raise ValueError('addition cannot change another context group')
        return old, previous, successor, new_contexts, reader.accounting()


def _stage_roots(operation, old):
    changes = []
    for field, collection, key_field in (('nodes', 'nodes', 'node_id'), ('edges', 'edges', 'edge_id')):
        for key, row in operation.raw[field].items():
            if key not in old[field]:
                changes.append(ProjectionChange(collection, row[key_field], False, None, True, row))
    changes.extend(ProjectionChange('claim_traces', key, False, None, True, row)
                   for key, row in operation.raw['traces'].items())
    roots = operation.source_inputs.roots()
    view = roots['bibliographic-claims']
    roots['bibliographic-claims'] = stage_projection_snapshot_changes(view,
        expected_before_sha256=view.snapshot_digest, trusted_baseline_sha256=view.snapshot_digest,
        changes=changes, limits=operation.mutation_limits).snapshot()
    roots['source-catalog'] = operation.addition.candidate.snapshot()
    return roots


class ClaimAdditionPublication:
    def __init__(self, addition, source_inputs, expected_binding, catalog_inputs, progress_owner,
                 limits, mutation_limits, dependency_limits):
        self.active, self.committed, self.db = True, False, None
        self.addition, self.source_inputs, self.progress_owner = addition, source_inputs, progress_owner
        self._binding_raw = shared._json_bytes(expected_binding, 1_048_576)
        self.inputs = CatalogInputs(catalog_inputs.header, catalog_inputs.entity_type_registry,
            catalog_inputs.relation_type_registry, catalog_inputs.lenses, source_order_profile=CANONICAL_ORDER)
        self.limits, self.mutation_limits, self.dependency_limits = limits, mutation_limits, dependency_limits
        self.profile = execution_profile_sha256()
        shared._require_vector(source_inputs)
        shared._profiles(source_inputs, self.inputs, shared.declaration_profile_sha256())
        if source_inputs.value()['dependencies'].get(PROFILE_KEY) not in (None, self.profile):
            raise ValueError('Claim publication execution profile requires explicit migration')
        self.catalog, self.assembler, raw, declarations = _assembled(addition, mutation_limits)
        self._declarations = tuple(declarations)
        self._raw_bytes = shared._cohort_bytes(raw, limits)
        self.verify_current()

    @property
    def binding(self):
        return _strict_json(self._binding_raw)

    @property
    def raw(self):
        return shared._cohort_value(self._raw_bytes)

    @property
    def declarations(self):
        return self._declarations

    def verify_current(self):
        if not self.active:
            raise ValueError('Claim publication source scope is closed')
        self.addition.verify_current()
        self.assembler.verify_current()
        shared._profiles(self.source_inputs, self.inputs, shared.declaration_profile_sha256())
        if execution_profile_sha256() != self.profile:
            raise ValueError('Claim publication implementation changed')

    def apply_transaction(self, db, *, publication_limits=None, catalog_limits=None, semantic_limits=None):
        if not self.active or self.db is not None or not db.in_transaction:
            raise ValueError('one active caller-owned Claim publication transaction required')
        self.db = db
        limit = publication_limits or shared.PublicationLimits()
        start = db.total_changes
        if shared.read_prepared_source_inputs_transaction(db, expected_binding=self.binding,
                limits=limit) != self.source_inputs:
            raise ValueError('prepared predecessor changed before Claim addition')
        shared._catalog_selected(db, self.binding, self.inputs, limit)
        lookup = dict(expected_binding=self.binding, source_inputs_sha256=self.source_inputs.digest,
            declaration_profile_sha256=shared.declaration_profile_sha256(), progress_owner=self.progress_owner,
            limits=self.dependency_limits)
        if lookup_source_dependencies_transaction(db, kind='unresolved', ref=None, **lookup)['claim_ids']:
            raise ValueError('unresolved source dependencies prevent complete Claim addition closure')
        for declaration in self.declarations:
            for kind, ref in (('claim', declaration.value()['claim_id']), ('identity', declaration.value()['claim_id'])):
                if lookup_source_dependencies_transaction(db, kind=kind, ref=ref, **lookup)['claim_ids']:
                    raise ValueError('existing source depends on newly added Claim; broader closure required')
        old, previous, successor, context_ids, raw_reads = _normalize(db, self)
        roots = _stage_roots(self, old)
        after_source = shared.source_vector_inputs(roots=roots,
            dependencies={**self.source_inputs.value()['dependencies'], PROFILE_KEY: self.profile},
            source_publication=self.source_inputs.value()['source_publication'])
        header = self.inputs.header
        header['source_revision'] = after_source.value()['source_revision']
        after_inputs = CatalogInputs(header, self.inputs.entity_type_registry, self.inputs.relation_type_registry,
            self.inputs.lenses, source_order_profile=CANONICAL_ORDER)
        changes = shared._changes(db, previous, successor, self.dependency_limits, self.progress_owner)
        reserve = 3 * len(context_ids) + 1
        if limit.max_mutations <= reserve:
            raise ValueError('Claim context finalizer reservation exceeds publication budget')
        result = apply_dependency_bound_prepared_delta_transaction(db, expected_binding=self.binding,
            before_source_inputs=self.source_inputs, after_source_inputs=after_source,
            before_inputs=self.inputs, after_inputs=after_inputs, changes=changes,
            dependency_changes=[SourceDependencyChange('insert', d.value()['claim_id'], declaration=d) for d in self.declarations],
            declaration_profile_sha256=shared.declaration_profile_sha256(), progress_owner=self.progress_owner,
            limits=replace(limit, max_mutations=limit.max_mutations - reserve),
            dependency_limits=self.dependency_limits, catalog_limits=catalog_limits, semantic_limits=semantic_limits)
        by_id = {row['id']: row for row in successor['nodes']}
        with _operation(db, self.dependency_limits, self.progress_owner) as b:
            state = shared._state(b, self.binding, self.source_inputs)
            last = b.one('SELECT position FROM agent_context_nodes ORDER BY position DESC LIMIT 1')
            position = -1 if last is None else last[0]
            for identity in context_ids:
                position += 1
                if position > 9_007_199_254_740_991:
                    raise ValueError('singleton context position budget exhausted')
                keys = shared._context_keys(by_id[identity])
                graph, ref = keys[0]
                if shared._context_head(b, graph, ref) is not None:
                    raise ValueError('new singleton context was occupied during publication')
                encoded = shared._json_bytes([list(key) for key in keys], b.limits.max_row_bytes).decode()
                b.execute('INSERT INTO agent_context_nodes VALUES (?,?,?,?)',
                    (identity, position, encoded, shared._row_sha([identity, position, encoded])))
                b.execute('INSERT INTO agent_context_refs VALUES (?,?,?,?)', (graph, ref, identity, position))
                contribution = shared._context_contribution(identity, position)
                b.execute('INSERT INTO agent_context_heads VALUES (?,?,?,?,?,?)',
                    shared._head_values(graph, ref, 1, contribution, contribution))
            state.update(binding=result['binding'], source_inputs_sha256=after_source.digest)
            raw = shared._json_bytes(state, b.limits.max_state_bytes)
            b.execute('UPDATE agent_context_state SET json=?,sha256=? WHERE singleton=1', (raw.decode(), _digest(raw)))
        if db.total_changes - start > limit.max_mutations:
            raise ValueError('combined Claim publication mutation budget exceeded; rollback required')
        self.verify_current()
        self._result_raw = shared._json_bytes({**result, 'schema': 'tos_initial_claim_publication_v1',
            'sql_mutations': db.total_changes - start, 'source_command_committed': True, 'prepared_committed': False,
            'claim_ids': [d.value()['claim_id'] for d in self.declarations],
            'normalized_nodes': len(successor['nodes']), 'normalized_relations': len(successor['relations']),
            'changed_nodes': sum(c.kind == 'node' for c in changes),
            'changed_relations': sum(c.kind == 'relation' for c in changes),
            'creation_request_digest': self.addition.receipt['request_digest'],
            'source_transition_verified': True, 'complete_incidence_verified': True,
            'singleton_context_addition': True, 'global_source_currentness_verified': False,
            'raw_projection_reads': raw_reads,
            'consumer_switched': False, 'is_semantic_acceptance': False}, self.limits.max_bytes)
        self._total_changes, self._schema = db.total_changes, db.execute('PRAGMA main.schema_version').fetchone()
        self._publication_limits = limit
        return self.result

    @property
    def result(self):
        return _strict_json(self._result_raw)

    def commit_transaction(self, db):
        if (db is not self.db or not db.in_transaction or not hasattr(self, '_result_raw')
                or not self.active or self.committed):
            raise ValueError('exact applied Claim publication transaction required')
        if db.total_changes != self._total_changes or db.execute('PRAGMA main.schema_version').fetchone() != self._schema:
            raise ValueError('caller wrote or changed schema after verified publication; rollback required')
        result = self.result
        current = shared.read_prepared_source_inputs_transaction(db, expected_binding=result['binding'],
            limits=self._publication_limits)
        if current.digest != result['source_inputs_sha256']:
            raise ValueError('Claim publication paired source selection changed')
        self.verify_current()
        db.commit()
        self.committed = True
        return {**result, 'prepared_committed': True}


@contextmanager
def claim_addition_publication(owner_config, *, source_inputs, expected_binding, catalog_inputs,
        expected_receipt_sha256, expected_request_digest, progress_owner,
        limits=None, mutation_limits=None, dependency_limits=None):
    if catalog_inputs.source_order_profile != CANONICAL_ORDER:
        raise ValueError('explicit canonical prepared predecessor required')
    limits = limits or shared.AgentPublicationLimits(max_claims=512)
    mutation_limits = mutation_limits or MutationLimits()
    dependencies = dependency_limits or SourceDependencyLimits(max_claims=limits.max_claims)
    view = source_inputs.roots()['source-catalog']
    before = SourceCatalogSnapshot(view, expected_root_sha256=view.snapshot_digest,
        trusted_baseline_sha256=view.snapshot_digest, limits=mutation_limits)
    with claim_catalog_addition(owner_config, before, expected_receipt_sha256=expected_receipt_sha256,
            expected_request_digest=expected_request_digest, mutation_limits=mutation_limits) as addition:
        operation = ClaimAdditionPublication(addition, source_inputs, expected_binding, catalog_inputs,
            progress_owner, limits, mutation_limits, dependencies)
        try:
            yield operation
        finally:
            operation.active = False
