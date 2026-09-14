"""Indexed execution preserves the current lens contract, not just membership.

All graphs and forms below are synthetic; no corpus loading or source assessment.
"""
from __future__ import annotations

import copy
from concurrent.futures import ThreadPoolExecutor
import itertools
from pathlib import Path
import random
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'src'))
from tos_access import knowledge as k
from tos_access.human_form_codec import decode_human_form_selection


def graph_for(size=14, seed=0):
    rng = random.Random(seed)
    nodes = [k._normalize_node({
        'node_id': f'n{i:02d}', 'node_type': 'concept',
        'label': f'Узел {i % 4}', 'summary': 'Не принят; synthetic only.',
        'source_ref': f'synthetic:node:{i}',
        'properties': {'fixture_score': i, 'unknown_future': {'false': False, 'zero': 0}},
    }, 'philosophy') for i in range(size)]
    for i, node in enumerate(nodes):
        node['entity_id'] = f'tos.synthetic.subject.{i // 2}'
        node['type_id'] = 'tos.entity.concept' if i % 3 else 'tos.entity.work'
    by_id = {node['id']: node for node in nodes}
    pairs = [(i, i + 1) for i in range(size - 1)]
    pairs += [(rng.randrange(size), rng.randrange(size)) for _ in range(size * 2)]
    relations = [k._normalize_relation({
        'edge_id': f'r{i:03d}', 'from_id': f'n{left:02d}', 'to_id': f'n{right:02d}',
        'predicate_id': ('supports', 'contradicts', 'related_to')[i % 3],
        'source_ref': f'synthetic:relation:{i}', 'properties': {'future': ['x', None]},
    }, 'philosophy', by_id) for i, (left, right) in enumerate(pairs)]
    # Keep a second source and a repeated native identity for scoping/ambiguity.
    other = copy.deepcopy(nodes[-1])
    other.update(id='repository:other', source_graph='repository')
    nodes.append(other)
    return {'source_revision': 'a' * 64, 'nodes': nodes, 'relations': relations,
            'query_properties': [{
                'property_id': 'tos.property.synthetic-score', 'field': 'attributes.fixture_score',
                'value_type': 'number', 'applies_to': ['tos.entity.concept'], 'inherited': True,
                'operators': ['eq', 'neq', 'gt', 'exists'],
            }], 'authority_boundary': {'synthetic_only': True, 'is_source': False}}


def lens(**changes):
    return {'schema_version': 'tos_lens_spec_v1', 'lens_id': 'synthetic-indexed',
            'sources': ['philosophy', 'repository'], 'explain': True, **changes}


def scenarios():
    for direction, depth, endpoint, profile in itertools.product(
            ('either', 'incoming', 'outgoing'), (0, 1, 2),
            ('both', 'either', 'independent'), ('overview', 'all')):
        yield lens(seed={'focus_node_id': 'philosophy:n00'}, node_query={'enabled': False},
                   traversal={'direction': direction, 'depth': depth, 'profile': profile},
                   composition={'endpoint_policy': endpoint},
                   limits={'nodes': 6, 'relations': 9, 'groups': 4})
    for quantifier, direction, length in itertools.product(
            ('exists', 'not_exists'), ('either', 'incoming', 'outgoing'), (1, 2, 3)):
        yield lens(seed={'node_ids': ['philosophy:n00', 'philosophy:n13']},
                   relation_query={'enabled': False},
                   path_query=[{'path_id': 'synthetic-path', 'quantifier': quantifier,
                                'steps': [{'direction': direction}] * length}])
    for operation, value in (('eq', 4), ('neq', 4), ('gt', 4), ('exists', True), ('exists', False)):
        yield lens(node_query={'filters': [
            {'property_id': 'tos.property.synthetic-score', 'op': operation, 'value': value}]},
            relation_query={'filters': [{'field': 'predicate_id', 'op': 'neq', 'value': 'supports'}]},
            composition={'endpoint_policy': 'either', 'group_by': ['kind_id'],
                         'sort_nodes': [{'field': 'display.title.default', 'direction': 'desc'},
                                        {'field': 'id', 'direction': 'asc'}],
                         'sort_relations': [{'field': 'predicate_id', 'direction': 'desc'},
                                            {'field': 'id', 'direction': 'desc'}]})
    yield lens(seed={'text_query': 'Не принят'}, detail='full')
    yield lens(seed={'text_query': 'absent synthetic string'}, detail='compact')


def outcome(call):
    try:
        return ('result', call())
    except Exception as error:
        return ('error', type(error), str(error))


class IndexedLensTests(unittest.TestCase):
    def assert_parity(self, graph, spec, index=None):
        index = index or k.KnowledgeGraphIndex(graph)
        expected = outcome(lambda: k.execute_knowledge_lens(graph, spec))
        actual = outcome(lambda: k.execute_knowledge_lens(graph, spec, graph_index=index))
        self.assertEqual(actual, expected)
        return actual

    def test_current_executor_ab_matrix_preserves_complete_packets_and_input(self):
        for seed in range(3):
            graph = graph_for(seed=seed)
            before = copy.deepcopy(graph)
            index = k.KnowledgeGraphIndex(graph)
            for number, spec in enumerate(scenarios()):
                with self.subTest(seed=seed, scenario=number):
                    result = self.assert_parity(graph, spec, index)
                    self.assertEqual(result[0], 'result')
            self.assertEqual(graph, before)

    def test_focus_resolution_exact_entity_native_scopes_and_errors(self):
        graph = graph_for()
        index = k.KnowledgeGraphIndex(graph)
        for identifier, sources in itertools.product(
                ('philosophy:n00', 'tos.synthetic.subject.0', 'n00', 'n13', 'absent'),
                (None, ['philosophy'], ['repository'])):
            with self.subTest(identifier=identifier, sources=sources):
                expected = outcome(lambda: k.focus_knowledge_node(graph, identifier, sources=sources))
                actual = outcome(lambda: k.focus_knowledge_node(
                    graph, identifier, sources=sources, graph_index=index))
                self.assertEqual(actual, expected)

    def test_duplicate_ids_and_missing_endpoints_keep_original_admission(self):
        graph = graph_for(size=4)
        graph['relations'] = graph['relations'][:2]
        graph['relations'][0].update(id='same', from_id='philosophy:n00', to_id='philosophy:n01')
        graph['relations'][1].update(id='same', from_id='philosophy:n02', to_id='philosophy:n03')
        spec = lens(seed={'focus_node_id': 'philosophy:n00'}, node_query={'enabled': False},
                    traversal={'depth': 1, 'profile': 'all'},
                    limits={'nodes': 6, 'relations': 6}, composition={'endpoint_policy': 'both'})
        result = self.assert_parity(graph, spec)
        self.assertEqual(result[0], 'result')
        self.assertEqual([r['id'] for r in result[1]['relations']], ['same', 'same'])
        self.assertEqual(len(result[1]['nodes']), 4)
        graph['nodes'].append(copy.deepcopy(graph['nodes'][0]))
        self.assert_parity(graph, spec)
        graph['relations'][1]['to_id'] = 'missing:endpoint'
        self.assert_parity(graph, spec)

    def test_errors_and_property_binding_precede_cached_execution(self):
        graph = graph_for()
        index = k.KnowledgeGraphIndex(graph)
        k.execute_knowledge_lens(graph, lens(), graph_index=index)
        invalid = [None, {}, lens(future_field=True), lens(sources=['unknown']),
                   lens(seed={'focus_node_id': 'absent'}), lens(traversal={'direction': 'sideways'}),
                   lens(node_query={'filters': [{'field': 'attributes.__proto__', 'op': 'eq', 'value': 1}]}),
                   lens(node_query={'filters': [{'property_id': 'tos.property.absent', 'op': 'eq', 'value': 1}]}),
                   lens(node_query={'filters': [{'property_id': 'tos.property.synthetic-score', 'op': 'eq', 'value': True}]}),
                   lens(path_query=[{'path_id': 'bad', 'steps': []}])]
        for spec in invalid:
            with self.subTest(spec=spec):
                self.assertEqual(self.assert_parity(graph, spec, index)[0], 'error')
        changed = copy.deepcopy(graph)
        changed['query_properties'].append(copy.deepcopy(changed['query_properties'][0]))
        self.assertEqual(self.assert_parity(changed, lens())[0], 'error')

    def test_snapshot_rejects_other_object_even_when_revision_is_equal(self):
        graph = graph_for()
        index = k.KnowledgeGraphIndex(graph)
        for other in (copy.deepcopy(graph), {**graph, 'source_revision': 'b' * 64}):
            with self.assertRaisesRegex(ValueError, 'different snapshot'):
                k.execute_knowledge_lens(other, lens(), graph_index=index)
            with self.assertRaisesRegex(ValueError, 'different snapshot'):
                k.focus_knowledge_node(other, 'n00', graph_index=index)

    def test_pagination_replay_and_binding_match_complete_original_packets(self):
        graph = graph_for()
        index = k.KnowledgeGraphIndex(graph)
        spec = lens(composition={'endpoint_policy': 'independent'},
                    limits={'nodes': 14, 'relations': 30}, pagination={'nodes': 3, 'relations': 3})
        seen = set()
        for _ in range(30):
            result = self.assert_parity(graph, spec, index)
            self.assertEqual(result[0], 'result')
            packet = result[1]
            self.assertEqual(k.execute_knowledge_lens(graph, spec, graph_index=index), packet)
            cursor = packet['page']['next_cursor']
            if cursor is None:
                break
            self.assertNotIn(cursor, seen)
            seen.add(cursor)
            spec = {**spec, 'pagination': {'nodes': 3, 'relations': 3, 'cursor': cursor}}
        else:
            self.fail('continuation did not exhaust the bounded selection')
        self.assertTrue(seen)
        stale = {**spec, 'pagination': {'nodes': 3, 'relations': 3, 'cursor': next(iter(seen))}, 'language': 'en'}
        self.assertEqual(self.assert_parity(graph, stale, index)[0], 'error')

    def test_negative_path_budget_is_not_turned_into_absence(self):
        graph = graph_for(size=2)
        edge = graph['relations'][0]
        graph['relations'] = [{**edge, 'id': f'loop:{i:03d}', 'from_id': 'philosophy:n00',
                               'to_id': 'philosophy:n00'} for i in range(24)]
        spec = lens(seed={'node_ids': ['philosophy:n00']}, relation_query={'enabled': False},
                    path_query=[{'path_id': 'bounded-negative', 'quantifier': 'not_exists',
                                 'steps': [{}, {}, {}, {'node_query': {'filters': [
                                     {'field': 'id', 'op': 'eq', 'value': 'absent'}]}}]}])
        result = self.assert_parity(graph, spec)
        self.assertEqual(result[0], 'error')
        self.assertIn('execution safety ceiling', result[2])

    def test_path_property_binding_and_excluded_endpoints_keep_scope_and_witnesses(self):
        graph = graph_for(size=4)
        graph['relations'] = graph['relations'][:3]
        spec = lens(sources=['philosophy'], seed={'node_ids': ['philosophy:n00']},
                    relation_query={'enabled': False},
                    path_query=[{'path_id': 'typed-neighbor', 'steps': [{
                        'node_query': {'filters': [{'property_id': 'tos.property.synthetic-score',
                                                   'op': 'eq', 'value': 1}]}}]}])
        result = self.assert_parity(graph, spec)
        self.assertEqual(result[0], 'result')
        self.assertEqual([node['id'] for node in result[1]['nodes']], ['philosophy:n00'])
        witness = result[1]['inclusion']['nodes']['philosophy:n00']['path_witnesses'][0]
        self.assertEqual(witness['node_ids'], ['philosophy:n00', 'philosophy:n01'])
        for change in ('missing-property', 'excluded-source', 'unrelated-type'):
            other = copy.deepcopy(graph)
            if change == 'missing-property':
                other['nodes'][1]['attributes'].pop('fixture_score')
            elif change == 'excluded-source':
                other['nodes'][1]['source_graph'] = 'repository'
            else:
                other['nodes'][1]['type_id'] = 'tos.entity.work'
            with self.subTest(change=change):
                self.assertEqual(self.assert_parity(other, spec)[1]['nodes'], [])
                negative = copy.deepcopy(spec)
                negative['path_query'][0]['quantifier'] = 'not_exists'
                result = self.assert_parity(other, negative)
                proof = result[1]['inclusion']['nodes']['philosophy:n00']['path_witnesses'][0]
                self.assertTrue(proof['absence_in_scope'])

    def test_warm_focus_reuses_scopes_plans_without_rescanning_source_arrays(self):
        graph = graph_for(size=40)
        index = k.KnowledgeGraphIndex(graph)
        spec = lens(seed={'focus_node_id': 'n00'}, node_query={'enabled': False},
                    traversal={'depth': 2}, limits={'nodes': 5, 'relations': 7})
        expected = k.execute_knowledge_lens(graph, spec, graph_index=index)
        original_objects = k._objects

        def no_source_scan(value):
            if value is graph['nodes'] or value is graph['relations']:
                self.fail('warm lens rescanned source arrays')
            return original_objects(value)

        with patch.object(k, '_objects', side_effect=no_source_scan), patch.object(
                k, '_matches_group', side_effect=AssertionError('warm relation plan was rebuilt')):
            self.assertEqual(k.execute_knowledge_lens(graph, spec, graph_index=index), expected)

    def test_cache_eviction_and_concurrent_borrowers_do_not_change_results(self):
        graph = graph_for()
        index = k.KnowledgeGraphIndex(graph)
        specs = [lens(sources=[source]) for source in k.KNOWLEDGE_SOURCES]
        specs += [lens(relation_query={'filters': [
            {'field': 'predicate_id', 'op': 'eq', 'value': predicate}]})
            for predicate in ('supports', 'contradicts', 'related_to')]
        expected = [k.execute_knowledge_lens(graph, spec) for spec in specs]
        with ThreadPoolExecutor(max_workers=3) as pool:
            actual = list(pool.map(lambda spec: k.execute_knowledge_lens(graph, spec, graph_index=index), specs * 2))
        self.assertEqual(actual, expected * 2)
        self.assertLessEqual(len(index._lens_scopes), 4)
        self.assertTrue(all(len(scope._plans) <= 2 for scope in index._lens_scopes.values()))
        for spec in specs:
            self.assert_parity(graph, spec, index)

    def test_shared_v2_human_forms_preserve_unknown_fields_and_mandatory_context(self):
        graph = graph_for()
        node = graph['nodes'][1]
        subject = {'id': node['entity_id'], 'version': 1, 'digest': 'sha256:' + 'a' * 64}
        node['attributes'].update(source_record={'record_id': subject['id'], 'record_version': 1},
                                  source_sha256='a' * 64)
        node['attributes']['human_forms'] = [{
            'schema_version': 'tos_human_form_materialization_v1',
            'form': {'id': 'tos.synthetic.form.' + role, 'version': 1, 'digest': 'sha256:' + 'b' * 64},
            'subject': subject, 'state': 'ready', 'role': role, 'language': 'ru', 'script': 'Cyrl',
            'display_text': 'Не доказано: ' + role, 'derivation': 'source-copy',
            'context': [{'slot': 'mandatory', 'binding': {'record': subject, 'pointer': '/qualifiers'},
                         'value': {'polarity': 'negative', 'unreviewed': True}}],
            'standalone_reading': False, 'performs_semantic_assessment': False,
            'admission': {'limits': ['Synthetic only', 'Not canon', role], 'is_semantic_evaluation': False},
            'unknown_future_key': {'null': None, 'false': False, 'zero': 0, 'empty': {}},
        } for role in ('name', 'caption', 'hover', 'statement')]
        before = copy.deepcopy(graph)
        index = k.KnowledgeGraphIndex(graph)
        for detail, language in itertools.product(('full', 'compact'), ('ru', 'en')):
            spec = lens(seed={'node_ids': [node['id']]}, language=language, detail=detail,
                        relation_query={'enabled': False})
            result = self.assert_parity(graph, spec, index)
            self.assertEqual(result[0], 'result')
            selected = result[1]['nodes'][0]
            wire = selected['human_form_selection']
            self.assertEqual(wire['schema_version'], 'tos_human_form_selection_v2')
            decoded = decode_human_form_selection(wire)
            if language == 'ru':
                self.assertEqual(decoded['roles']['caption']['state'], 'ready')
                packet = decoded['roles']['caption']['packet']
                self.assertEqual(packet['unknown_future_key'], {'null': None, 'false': False, 'zero': 0, 'empty': {}})
                self.assertEqual(packet['context'][0]['value']['polarity'], 'negative')
            self.assertEqual(selected['source_refs'], node['source_refs'])
        self.assertEqual(graph, before)


if __name__ == '__main__':
    unittest.main()
