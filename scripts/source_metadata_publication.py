"""Publish one source-created metadata identity without rebuilding the corpus.

The registry and source command own the kind and authority. This additive
reader operation accepts an exact initial, independent metadata subject, not
Sign issuance, compound bibliographic growth, corrections or implicit repair
of existing unresolved references. No source write or consumer switch occurs.
"""
from contextlib import contextmanager
from dataclasses import replace
from pathlib import Path

import source_agent_publication as shared
from source_catalog_projection import SourceCatalogSnapshot, _read_owned
from source_metadata_catalog import metadata_catalog_addition
from bibliographic_claim_assembler import BibliographicClaimAssembler
from tos_corpus_index_common import SourceNavigationRecordInput, project_source_navigation_record
import source_witness_bibliographic_graph_common as bibliography
from tos_access.catalog_semantics import CatalogInputs, CANONICAL_ORDER
from tos_access.prepared_source_dependencies import (
    ProgressHandlerOwner, SourceDependencyLimits, _operation,
    lookup_source_dependencies_transaction,
)
from tos_access.prepared_source_publication import apply_dependency_bound_prepared_delta_transaction
from tos_access.projection_mutation import MutationLimits
from tos_access.projection_store import _strict_json, _digest
from tos_access.addressed_replacement import _json_size

PROFILE_KEY = 'metadata-addition-publication-profile'


def execution_profile_sha256():
    root = Path(__file__).resolve().parents[1]
    return shared._row_sha({'schema': 'tos_initial_metadata_publication_v1', 'modules': {
        ref: _digest(_read_owned(root / ref, 1_048_576))
        for ref in ('scripts/source_metadata_publication.py', 'scripts/source_metadata_catalog.py')}})


def _assembled(addition, mutation_limits, limits):
    """Use the existing verified metadata reader and both full-build renderers."""
    view = addition.candidate.snapshot()
    catalog = SourceCatalogSnapshot(view, expected_root_sha256=view.snapshot_digest,
        trusted_baseline_sha256=view.snapshot_digest, limits=mutation_limits)
    assembler = BibliographicClaimAssembler(addition.root, catalog_snapshot=catalog)
    identity = addition.candidate.verification['record_id']
    selected = catalog.get(identity)
    material = assembler.objects[identity]
    # Cross-corpus grounding needs its external cohort, not omitted edges.
    if any(ref.startswith('ToS/canon/') for ref in material['_source_record'].get('source_refs', [])):
        raise ValueError('source-cited canon grounding requires its external metadata growth closure')
    item = assembler._metadata[identity]
    history = assembler.metadata_reader.exact_refs(identity)
    if (history['status'] != 'available' or history['current_ref'] != selected.source['record_ref']
            or history['refs'] != [history['current_ref']]):
        raise ValueError('metadata addition requires an exact initial record without predecessor history')
    resolved = assembler.metadata_reader.resolve(history['current_ref'])
    if resolved['status'] != 'available':
        raise ValueError('initial metadata version is unavailable')
    navigation = project_source_navigation_record(SourceNavigationRecordInput(
        selected.entry['record_type'], selected.entry, material['_source_record'],
        item['forms'], history, ((history['current_ref'], resolved),)))
    bibliographic = bibliography.project_bibliographic_identity(bibliography.BibliographicIdentityInput(
        selected.entry, material['_source_record'], material.get('_human_forms'),
        material.get('_human_forms_source_ref')))
    # Its exact current RecordVersion is retained as a separate node, using the
    # full renderer. No external incidence or compound membership is invented.
    node_ids = {row['node_id'] for row in navigation.nodes}
    if (navigation.diagnostics or any(edge['from_id'] not in node_ids or edge['to_id'] not in node_ids
                                     or edge.get('claim_ref') for edge in navigation.edges)):
        raise ValueError('initial metadata has dependent navigation outside this addition')
    raw = {'nodes': {**{('source-navigation', row['node_id']): row for row in navigation.nodes},
                     ('source-claims', bibliographic['node_id']): bibliographic},
           'edges': {('source-navigation', row['edge_id']): row for row in navigation.edges}, 'traces': {}}
    encoded = shared._cohort_bytes(raw, limits)
    assembler.verify_current()
    return catalog, assembler, encoded


class MetadataAdditionPublication:
    def __init__(self, addition, source_inputs, binding, inputs, progress_owner,
                 limits, mutation_limits, dependency_limits):
        self.active, self.committed, self.rolled_back, self.db = True, False, False, None
        self.addition, self.source_inputs, self.progress_owner = addition, source_inputs, progress_owner
        self._binding_raw = shared._json_bytes(binding, 1_048_576)
        self.inputs = CatalogInputs(inputs.header, inputs.entity_type_registry,
            inputs.relation_type_registry, inputs.lenses, source_order_profile=CANONICAL_ORDER)
        self.limits, self.mutation_limits, self.dependency_limits = limits, mutation_limits, dependency_limits
        self.profile = execution_profile_sha256()
        shared._require_vector(source_inputs)
        shared._profiles(source_inputs, self.inputs, shared.declaration_profile_sha256())
        if source_inputs.value()['dependencies'].get(PROFILE_KEY) not in (None, self.profile):
            raise ValueError('metadata addition implementation requires explicit profile migration')
        self.catalog, self.assembler, self._raw_bytes = _assembled(addition, mutation_limits, limits)
        self.verify_current()

    @property
    def binding(self):
        return _strict_json(self._binding_raw)

    @property
    def raw(self):
        return shared._cohort_value(self._raw_bytes)

    def verify_current(self):
        if not self.active:
            raise ValueError('metadata addition source scope is closed')
        self.addition.verify_current()
        self.assembler.verify_current()
        shared._profiles(self.source_inputs, self.inputs, shared.declaration_profile_sha256())
        if execution_profile_sha256() != self.profile:
            raise ValueError('metadata addition implementation changed')

    def apply_transaction(self, db, *, publication_limits=None, catalog_limits=None, semantic_limits=None):
        if not self.active or self.db is not None or not db.in_transaction:
            raise ValueError('one active caller-owned metadata publication transaction required')
        self.db = db
        limit = publication_limits or shared.PublicationLimits()
        if limit.max_mutations <= 1:
            raise ValueError('metadata addition requires context-state finalization allowance')
        start = db.total_changes
        if shared.read_prepared_source_inputs_transaction(db, expected_binding=self.binding,
                limits=limit) != self.source_inputs:
            raise ValueError('prepared predecessor changed before metadata addition')
        shared._catalog_selected(db, self.binding, self.inputs, limit)
        record_id = self.addition.candidate.verification['record_id']
        lookup = dict(expected_binding=self.binding, source_inputs_sha256=self.source_inputs.digest,
            declaration_profile_sha256=shared.declaration_profile_sha256(), progress_owner=self.progress_owner,
            limits=self.dependency_limits)
        # Unknown-address dependencies cannot be safely declared unaffected.
        # Identified unresolved references retain their own exact address.
        for kind, ref in (('unresolved', None), ('unresolved', record_id), ('identity', record_id)):
            if lookup_source_dependencies_transaction(db, kind=kind, ref=ref, **lookup)['claim_ids']:
                raise ValueError('existing source dependencies require a broader metadata growth closure')
        raw = self.raw
        reader = shared._RawRoots(self.source_inputs, self.mutation_limits)
        with _operation(db, self.dependency_limits, self.progress_owner) as b:
            state = shared._state(b, self.binding, self.source_inputs)
            for (graph, native), row in raw['nodes'].items():
                if reader.get(shared._role(graph), 'nodes', native, required=False) is not None:
                    raise ValueError('new metadata carrier already belongs to predecessor raw inputs')
                if b.one('SELECT id FROM prepared_documents WHERE kind=? AND id=?',
                         ('node', graph + ':' + native)) is not None:
                    raise ValueError('new metadata carrier already belongs to prepared predecessor')
            for (graph, native), row in raw['edges'].items():
                if (reader.get(shared._role(graph), 'edges', native, required=False) is not None
                        or b.one('SELECT id FROM prepared_documents WHERE kind=? AND id=?',
                                 ('relation', graph + ':' + native)) is not None):
                    raise ValueError('new metadata version edge already belongs to predecessor')
            dossiers = set(state['dossier_refs'])
            for (graph, _), row in raw['nodes'].items():
                if graph == 'source-navigation':
                    candidate = shared.k._source_dossier_candidate(row, graph)
                    if candidate is not None:
                        dossiers.add(candidate)
        # Run the complete pure kernel only on this bounded new cohort, not
        # the corpus. Reuse its shared-identity edge rule rather than copy it.
        corpus = {'source_navigation': {
            'nodes': [row for (graph, _), row in raw['nodes'].items() if graph == 'source-navigation'],
            'edges': [row for (graph, _), row in raw['edges'].items() if graph == 'source-navigation']}}
        bib = {'nodes': [row for (graph, _), row in raw['nodes'].items() if graph == 'source-claims'],
               'edges': [], 'claim_traces': []}
        _json_size([corpus, bib, self.inputs.entity_type_registry, self.inputs.relation_type_registry],
                   self.limits.max_bytes)
        token = shared.k.active_cache.set(None)
        try:
            normalized = shared.k.build_knowledge_graph(corpus, {}, bib,
                self.inputs.entity_type_registry, self.inputs.relation_type_registry)
        finally:
            shared.k.active_cache.reset(token)
        # The complete kernel also renders repository scaffolding. Select the
        # exact supplied carriers and their entire incidence, not that unrelated
        # scaffold; never create a second repository root in the predecessor.
        selected_ids = {graph + ':' + native for graph, native in raw['nodes']}
        successor = {'nodes': [row for row in normalized['nodes'] if row['id'] in selected_ids],
                     'relations': []}
        if {row['id'] for row in successor['nodes']} != selected_ids:
            raise ValueError('normalizer did not preserve the exact metadata carrier cohort')
        for row in normalized['relations']:
            endpoints = {row['from_id'], row['to_id']}
            if endpoints & selected_ids:
                if not endpoints <= selected_ids:
                    raise ValueError('metadata normalization requires external incidence closure')
                successor['relations'].append(row)
        if (len(successor['nodes']) > self.limits.max_nodes
                or len(successor['relations']) > self.limits.max_relations):
            raise ValueError('metadata normalized cohort count budget exceeded')
        _json_size([successor['nodes'], successor['relations']], self.limits.max_bytes)
        if any(shared._context_keys(row) for row in successor['nodes']):
            raise ValueError('initial metadata addition unexpectedly carries Claim context')
        empty = {'nodes': {}, 'edges': {}, 'traces': {}}
        roots = shared._stage_raw(self.source_inputs, empty, raw, self.mutation_limits)
        roots['source-catalog'] = self.catalog.view
        after_source = shared.source_vector_inputs(roots=roots,
            dependencies={**self.source_inputs.value()['dependencies'], PROFILE_KEY: self.profile},
            source_publication=self.source_inputs.value()['source_publication'])
        header = self.inputs.header
        header['source_revision'] = after_source.value()['source_revision']
        after_inputs = CatalogInputs(header, self.inputs.entity_type_registry, self.inputs.relation_type_registry,
            self.inputs.lenses, source_order_profile=CANONICAL_ORDER)
        changes = shared._changes(db, {'nodes': [], 'relations': []}, successor,
            self.dependency_limits, self.progress_owner)
        result = apply_dependency_bound_prepared_delta_transaction(db, expected_binding=self.binding,
            before_source_inputs=self.source_inputs, after_source_inputs=after_source,
            before_inputs=self.inputs, after_inputs=after_inputs, changes=changes, dependency_changes=[],
            declaration_profile_sha256=shared.declaration_profile_sha256(), progress_owner=self.progress_owner,
            limits=replace(limit, max_mutations=limit.max_mutations - 1),
            dependency_limits=self.dependency_limits, catalog_limits=catalog_limits, semantic_limits=semantic_limits)
        with _operation(db, self.dependency_limits, self.progress_owner) as b:
            state = shared._state(b, self.binding, self.source_inputs)
            state.update(binding=result['binding'], source_inputs_sha256=after_source.digest,
                         dossier_refs=sorted(dossiers))
            encoded = shared._json_bytes(state, b.limits.max_state_bytes)
            b.execute('UPDATE agent_context_state SET json=?,sha256=? WHERE singleton=1',
                      (encoded.decode(), _digest(encoded)))
        if db.total_changes - start > limit.max_mutations:
            raise ValueError('metadata addition combined mutation budget exceeded; rollback required')
        self.verify_current()
        # The paired source header already retains the complete semantic
        # report. Do not duplicate that report and the whole read catalog in
        # a command receipt; consumers query the selected reader for them.
        receipt = {key: value for key, value in result.items() if key not in ('catalog', 'semantic_report')}
        self._result_raw = shared._json_bytes({**receipt, 'schema': 'tos_initial_metadata_publication_v1',
            'record_id': record_id, 'sql_mutations': db.total_changes - start,
            'changed_nodes': len(successor['nodes']), 'changed_relations': len(successor['relations']),
            'creation_request_digest': self.addition.receipt['request_digest'],
            'source_command_committed': True, 'prepared_committed': False,
            'initial_metadata_closure_verified': True, 'global_source_currentness_verified': False,
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
            raise ValueError('exact applied metadata publication transaction required')
        if db.total_changes != self._total_changes or db.execute('PRAGMA main.schema_version').fetchone() != self._schema:
            raise ValueError('caller wrote or changed schema after metadata publication; rollback required')
        result = self.result
        current = shared.read_prepared_source_inputs_transaction(db, expected_binding=result['binding'],
            limits=self._publication_limits)
        if current.digest != result['source_inputs_sha256']:
            raise ValueError('paired metadata source selection changed')
        self.verify_current()
        db.commit()
        self.committed = True
        return {**result, 'prepared_committed': True}

    def rollback_transaction(self, db):
        """Explicitly abandon this candidate, preserving the committed source."""
        if db is not self.db or not self.active or self.committed or not db.in_transaction:
            raise ValueError('exact active metadata publication transaction required for rollback')
        db.rollback()
        self.rolled_back = True
        self.active = False
        return {'prepared_committed': False, 'prepared_rolled_back': True,
                'source_command_committed': True, 'consumer_switched': False}


@contextmanager
def metadata_addition_publication(owner_config, *, source_inputs, expected_binding, catalog_inputs,
        expected_receipt_sha256, expected_request_digest, progress_owner,
        limits=None, mutation_limits=None, dependency_limits=None):
    if catalog_inputs.source_order_profile != CANONICAL_ORDER or type(progress_owner) is not ProgressHandlerOwner:
        raise ValueError('explicit canonical predecessor and progress owner required')
    limits = limits or shared.AgentPublicationLimits()
    mutation_limits = mutation_limits or MutationLimits()
    dependencies = dependency_limits or SourceDependencyLimits(max_claims=limits.max_claims)
    view = source_inputs.roots()['source-catalog']
    before = SourceCatalogSnapshot(view, expected_root_sha256=view.snapshot_digest,
        trusted_baseline_sha256=view.snapshot_digest, limits=mutation_limits)
    with metadata_catalog_addition(owner_config, before, expected_receipt_sha256=expected_receipt_sha256,
            expected_request_digest=expected_request_digest, mutation_limits=mutation_limits) as addition:
        operation = MetadataAdditionPublication(addition, source_inputs, expected_binding, catalog_inputs,
            progress_owner, limits, mutation_limits, dependencies)
        try:
            yield operation
            if operation.db is not None and not operation.committed and not operation.rolled_back:
                raise ValueError('use the guarded commit; unfinished metadata publication requires rollback')
        finally:
            operation.active = False
