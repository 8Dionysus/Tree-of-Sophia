"""Full-builder parity for bounded source cohorts, not source admission."""
import copy
from dataclasses import replace
import json
from pathlib import Path
import unittest
from unittest.mock import patch

from tos_access import knowledge as k
from tos_access.source_assembly_normalization import (
    AssemblyNormalizationLimits, normalize_source_assembly_candidate)
from tos_access.normalization_cache import active_cache
from source_assembly_fixture import SourceAssemblyFixture


class SourceAssemblyNormalizationTests(unittest.TestCase):
    def setUp(self):
        code_root = Path(__file__).resolve().parents[2]
        source_root = Path(__file__).resolve().parent / "fixtures" / "source-assembly"
        self.fixture = SourceAssemblyFixture(code_root=code_root, source_root=source_root)

    def arguments(self, projection, graph, entities, relations, selected=None):
        by_id = {node['id']: node for node in graph['nodes']}
        native = {node['node_id']: node for node in projection['nodes']}
        if selected is None:
            selected = {'source-claims:' + key for key in native}
        edges = [row for row in graph['relations'] if selected & {row['from_id'], row['to_id']}]
        retained = {row[field] for row in edges for field in ('from_id', 'to_id')} - selected
        traces = [row for row in projection['claim_traces']
                  if 'source-claims:' + row['claim_node_id'] in selected | retained]
        for trace in traces:
            retained.update('source-claims:' + trace[field] for field in
                            ('claim_node_id', 'subject_node_id', 'object_node_id'))
        retained -= selected
        args = dict(node_records=[{'source_graph': 'source-claims', 'record': raw}
                    for key, raw in native.items() if 'source-claims:' + key in selected],
            relation_records=[{'source_graph': row['source_graph'], 'record': row['source_record']['payload'],
                               'identity_id': k._addressed_relation_identity(row)} for row in edges],
            retained_nodes=[by_id[key] for key in sorted(retained)], claim_traces=traces,
            source_dossier_refs=sorted({node['source_dossier_ref'] for node in graph['nodes']
                                       if 'source_dossier_ref' in node}),
            context_node_order=[node['id'] for node in graph['nodes']
                                if node['id'] in selected | retained and node['kind_id'] in ('claim', 'annotation-claim')],
            normalization_binding=graph['normalization_binding'], entity_registry=entities, relation_registry=relations)
        return args, selected, {row['id'] for row in edges}

    def assert_matches(self, result, graph, selected, relations):
        self.assertEqual(sorted(result['nodes'], key=lambda row: row['id']),
                         [row for row in graph['nodes'] if row['id'] in selected])
        self.assertEqual(sorted(result['relations'], key=lambda row: row['id']),
                         [row for row in graph['relations'] if row['id'] in relations])

    def test_full_raw_cohort_uses_shared_kernels_without_full_build_or_input_mutation(self):
        with self.fixture.historical_fixture() as (root, _history, _real, claims, rebuild):
            claims[0]['counterevidence_refs'] = claims[1]['evidence_refs']
            claims[0]['alternative_claim_refs'] = [claims[1]['claim_id']]
            projection = rebuild()
            graph, entities, relations = self.fixture.historical_knowledge(root, projection)
            args, selected, edge_ids = self.arguments(projection, graph, entities, relations)
            before = copy.deepcopy(args)
            with patch.object(k, 'build_knowledge_graph', side_effect=AssertionError('full graph fallback')):
                result = normalize_source_assembly_candidate(**args)
            self.assert_matches(result, graph, selected, edge_ids)
            self.assertEqual(before, args)
            from test_builder_ownership import mutable_ids
            self.assertFalse(mutable_ids(result) & mutable_ids(args))
            self.assertTrue(all(value is False for value in result['scope'].values()))
            self.assertFalse(result['is_semantic_acceptance'])

    def test_agent_correction_recomputes_claim_and_relations_beyond_agent_incidence(self):
        with self.fixture.historical_fixture() as (root, _history, real, claims, rebuild):
            claims[0]['evidence_refs'] = [real[0]['record_id']]
            before = rebuild()
            old_graph, _, _ = self.fixture.historical_knowledge(root, before)
            path = root / 'ToS/source-witnesses/agents/friedrich-nietzsche/agent.json'
            agent = json.loads(path.read_bytes())
            agent.update(preferred_label='Synthetic changed Agent label', record_version=agent['record_version'] + 1)
            path.write_text(json.dumps(agent))
            after = rebuild()
            graph, entities, relations = self.fixture.historical_knowledge(root, after)
            old_raw = {node['node_id']: node for node in before['nodes']}
            selected = {'source-claims:' + node['node_id'] for node in after['nodes']
                        if old_raw.get(node['node_id']) != node}
            args, selected, edge_ids = self.arguments(after, graph, entities, relations, selected)
            self.assertLess(len(selected), len(graph['nodes']) - 1)
            old_by_id = {node['id']: node for node in old_graph['nodes']}
            # Unchanged endpoint rows come from the predecessor, not the oracle.
            args['retained_nodes'] = [old_by_id[node['id']] for node in args['retained_nodes']]
            result = normalize_source_assembly_candidate(**args)
            self.assert_matches(result, graph, selected, edge_ids)
            rebuilt_nodes = {node['id']: node for node in old_graph['nodes']}
            rebuilt_relations = {row['id']: row for row in old_graph['relations']}
            rebuilt_nodes.update((node['id'], node) for node in result['nodes'])
            rebuilt_relations.update((row['id'], row) for row in result['relations'])
            self.assertEqual(sorted(rebuilt_nodes.values(), key=lambda row: row['id']), graph['nodes'])
            self.assertEqual(sorted(rebuilt_relations.values(), key=lambda row: row['id']), graph['relations'])
            agent_id = next(node['id'] for node in graph['nodes'] if node['entity_id'] == real[0]['record_id'])
            self.assertTrue(any(agent_id not in (row['from_id'], row['to_id']) for row in result['relations']))

    def test_required_trace_endpoint_context_and_duplicate_frames_fail_closed(self):
        with self.fixture.historical_fixture() as (root, _history, _real, _claims, rebuild):
            projection = rebuild()
            graph, entities, relations = self.fixture.historical_knowledge(root, projection)
            args, _, _ = self.arguments(projection, graph, entities, relations)
            for change in ({'claim_traces': []}, {'context_node_order': []},
                    {'node_records': args['node_records'][1:]},
                    {'node_records': args['node_records'] * 2},
                    {'relation_records': args['relation_records'] * 2},
                    {'claim_traces': args['claim_traces'] * 2},
                    {'normalization_binding': {}}):
                with self.subTest(change=next(iter(change))), self.assertRaises(ValueError):
                    normalize_source_assembly_candidate(**{**args, **change})

    def test_literal_claim_context_replaces_old_uncertainty_without_losing_source_fields(self):
        with self.fixture.historical_fixture() as (root, _history, _real, claims, rebuild):
            claims[0].update(predicate='historical_dating', object={
                'kind': 'date-assertion', 'role': 'historical-time',
                'calendar': 'proleptic-gregorian', 'year_numbering': 'astronomical',
                'certainty': 'approximate', 'value': '1883',
                'source_wording': {'text': 'Uncertain synthetic date only', 'language': 'en'},
                'extensions': {'uninterpreted': [None, False, 0, []]}}, epistemic_status='disputed')
            before = rebuild()
            old, _, _ = self.fixture.historical_knowledge(root, before)
            claims[0]['claim_version'] += 1
            claims[0]['qualifiers']['negated'] = False
            after = rebuild()
            graph, entities, relations = self.fixture.historical_knowledge(root, after)
            args, selected, edges = self.arguments(after, graph, entities, relations)
            result = normalize_source_assembly_candidate(**args)
            self.assert_matches(result, graph, selected, edges)
            literal = next(node for node in result['nodes'] if node['source_record']['payload']['node_kind'] == 'literal')
            previous = next(node for node in old['nodes'] if node['id'] == literal['id'])
            self.assertNotEqual(literal['semantics']['assertion_contexts'], previous['semantics']['assertion_contexts'])
            self.assertEqual(literal['source_record']['payload']['properties']['value']['extensions'],
                             {'uninterpreted': [None, False, 0, []]})

    def test_source_bound_descriptor_and_retained_digest_are_checked(self):
        with self.fixture.historical_fixture() as (root, _history, _real, _claims, rebuild):
            projection = rebuild()
            graph, entities, relations = self.fixture.historical_knowledge(root, projection)
            claim = next(node for node in graph['nodes'] if node['kind_id'] == 'claim')
            args, _, _ = self.arguments(projection, graph, entities, relations, {claim['id']})
            self.assertTrue(args['retained_nodes'])
            for corruption in ('descriptor', 'retained'):
                changed = copy.deepcopy(args)
                if corruption == 'descriptor':
                    changed['node_records'][0]['record']['properties']['navigation_descriptor']['invented'] = False
                else:
                    changed['retained_nodes'][0]['source_record']['payload']['corrupt'] = True
                with self.subTest(corruption=corruption), self.assertRaises(ValueError):
                    normalize_source_assembly_candidate(**changed)

    def test_declared_budgets_and_ambient_cache_restore(self):
        with self.fixture.historical_fixture() as (root, _history, _real, _claims, rebuild):
            projection = rebuild()
            graph, entities, relations = self.fixture.historical_knowledge(root, projection)
            args, _, _ = self.arguments(projection, graph, entities, relations)
            initial = normalize_source_assembly_candidate(**args)
            for override in ({'max_nodes': 0}, {'max_relations': 0}, {'max_traces': 0},
                    {'max_input_bytes': initial['accounting']['input_bytes'] - 1},
                    {'max_output_bytes': initial['accounting']['output_bytes'] - 1}):
                marker = object()
                token = active_cache.set(marker)
                try:
                    with self.assertRaises(ValueError):
                        normalize_source_assembly_candidate(**args, limits=replace(AssemblyNormalizationLimits(), **override))
                    self.assertIs(active_cache.get(), marker)
                finally:
                    active_cache.reset(token)
            exact = replace(AssemblyNormalizationLimits(),
                max_input_bytes=initial['accounting']['input_bytes'], max_output_bytes=initial['accounting']['output_bytes'])
            self.assertEqual(normalize_source_assembly_candidate(**args, limits=exact), initial)

    def test_record_version_and_inherited_view_finalize_without_replacing_current_identity(self):
        from test_knowledge_contract import KnowledgeContractTests
        fixture = KnowledgeContractTests()
        fixture.setUpClass()
        raw, view = fixture._metadata_version_fixture()
        from tos_corpus_index_common import SourceNavigationRecordInput, project_source_navigation_record
        current = {**view['record'], 'record_version': view['record']['record_version'] + 1,
                   'preferred_label': 'Synthetic current Agent'}
        current_ref = {'id': current['record_id'], 'version': current['record_version'],
                       'digest': 'sha256:' + k._exact_record_digest(current)}
        provenance = {'source': {'archive_blob_ref': None, 'source_ref': 'test:synthetic-history'},
                      'fixture': 'synthetic-not-source-verification'}
        refs = [view['record_ref'], current_ref]
        history = {'status': 'available', 'reason': 'synthetic-current-and-retained',
            'record_id': current['record_id'], 'current_ref': current_ref, 'refs': refs,
            'provenance': provenance, 'grants_current_use': False,
            'performs_assessment': False, 'writes_to_source': False}
        resolutions = tuple((ref, {'status': 'available', 'reason': 'synthetic-exact-version',
            'version_status': status, 'record': record, 'provenance': provenance})
            for ref, record, status in zip(refs, (view['record'], current), ('historical', 'current')))
        projected = project_source_navigation_record(SourceNavigationRecordInput('agent',
            {'record_id': current['record_id'], 'source_record_ref': 'test:synthetic-history',
             'preferred_label': current['preferred_label']}, current, history=history, versions=resolutions))
        edges = [dict(row, view_ids=['fixture-inherited-view']) for row in projected.edges]
        corpus = {'source_navigation': {'nodes': list(projected.nodes), 'edges': edges}}
        graph = k.build_knowledge_graph(corpus, {}, {}, fixture.entity_type_registry, fixture.relation_type_registry)
        args = dict(node_records=[{'source_graph': 'source-navigation', 'record': item} for item in projected.nodes],
            relation_records=[{'source_graph': 'source-navigation', 'record': edge} for edge in edges], retained_nodes=[], claim_traces=[],
            source_dossier_refs=[], context_node_order=[], normalization_binding=graph['normalization_binding'],
            entity_registry=fixture.entity_type_registry, relation_registry=fixture.relation_type_registry)
        result = normalize_source_assembly_candidate(**args)
        self.assert_matches(result, graph, {row['id'] for row in graph['nodes'] if row['source_graph'] == 'source-navigation'},
                            {row['id'] for row in graph['relations']})
        version = next(row for row in result['nodes'] if row['kind_id'] == 'record-version')
        self.assertNotEqual(version['entity_id'], view['record_ref']['id'])
        self.assertFalse(version['semantics']['record_version']['grants_current_use'])


if __name__ == '__main__':
    unittest.main()
