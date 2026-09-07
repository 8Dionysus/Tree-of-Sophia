from __future__ import annotations

import json
import copy
import random
import tempfile
import sys
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource


ACCESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, (ACCESS_ROOT / "src").as_posix())

from tos_access.knowledge import (  # noqa: E402
    _content_revision,
    build_knowledge_graph,
    execute_knowledge_lens,
    focus_knowledge_node,
    knowledge_catalog,
    normalize_lens_spec,
    validate_knowledge_semantics,
    validate_semantic_registries,
    _normalize_node,
    _normalized_time,
)


class KnowledgeContractTests(unittest.TestCase):
    def test_property_ids_execute_from_snapshot_catalog_with_type_and_missing_guards(self):
        entities = copy.deepcopy(self.entity_type_registry)
        definition = {'property_id': 'tos.property.fixture-score', 'field': 'attributes.fixture_score',
            'labels': {'default': 'Fixture score', 'en': 'Fixture score', 'ru': 'Оценка теста'},
            'definition': 'Synthetic numeric value, not a historical judgment.', 'value_type': 'number',
            'applies_to': ['tos.entity.concept'], 'required': False, 'inherited': True,
            'unit': None, 'language': None, 'operators': ['eq', 'neq', 'gt', 'exists']}
        entities['property_definitions'].append(definition)
        graph = build_knowledge_graph(*self.fixture(), entity_type_registry=entities,
                                      relation_type_registry=self.relation_type_registry)
        node = next(n for n in graph['nodes'] if n['type_id'] == 'tos.entity.concept')
        node['attributes']['fixture_score'] = 3
        before = copy.deepcopy(graph)
        spec = {'schema_version': 'tos_lens_spec_v1', 'lens_id': 'property-test',
            'node_query': {'filters': [{'property_id': definition['property_id'], 'op': 'gt', 'value': 2}]},
            'relation_query': {'enabled': False}, 'detail': 'compact', 'explain': True}
        Draft202012Validator(self.schemas['lens-spec.v1.schema.json']).validate(spec)
        result = execute_knowledge_lens(graph, spec)
        self.assertEqual([n['id'] for n in result['nodes']], [node['id']])
        self.assertEqual(result['lens']['node_query'], normalize_lens_spec(spec)['node_query'])
        self.assertEqual(result['nodes'][0]['attributes'], {})
        self.assertEqual(graph, before)
        for change in ({'property_id': 'tos.property.unknown'}, {'op': 'contains'}, {'value': True},
                       {'value': '3'}, {'field': definition['field']}):
            invalid = copy.deepcopy(spec)
            invalid['node_query']['filters'][0].update(change)
            with self.subTest(change=change), self.assertRaises(ValueError):
                execute_knowledge_lens(graph, invalid)
        for value in (None, False, '', 'tos.property.bad\r', 'tos.property.bad\n', 'attributes.score'):
            invalid = copy.deepcopy(spec)
            invalid['node_query']['filters'][0]['property_id'] = value
            with self.subTest(property_id=value), self.assertRaises(ValueError):
                normalize_lens_spec(invalid)
            self.assertFalse(Draft202012Validator(self.schemas['lens-spec.v1.schema.json']).is_valid(invalid))
        unsafe = copy.deepcopy(graph)
        unsafe['query_properties'][-1]['field'] = 'attributes.__proto__.secret'
        with self.assertRaises(ValueError):
            execute_knowledge_lens(unsafe, spec)
        ambiguous = copy.deepcopy(graph)
        ambiguous['query_properties'].append(ambiguous['query_properties'][-1])
        with self.assertRaises(ValueError):
            execute_knowledge_lens(ambiguous, spec)
        # Unrelated types and unknown values are not proven unequal.
        spec['node_query']['filters'][0].update(op='neq', value=9)
        self.assertEqual([n['id'] for n in execute_knowledge_lens(graph, spec)['nodes']], [node['id']])
        node['attributes'].pop('fixture_score')
        self.assertEqual(execute_knowledge_lens(graph, spec)['nodes'], [])
        spec['node_query']['filters'][0].update(op='exists', value=False)
        found = execute_knowledge_lens(graph, spec)['nodes']
        self.assertIn(node['id'], {n['id'] for n in found})
        self.assertTrue(all('tos.entity.concept' in [n['type_id'], *n['semantics']['type_ancestors']] for n in found))

    def test_compact_scene_conserves_records_and_endpoint_closure_across_claim_topologies(self):
        from tos_access.knowledge import knowledge_scene
        prototype = build_knowledge_graph(*self.fixture())
        total_paths = 0
        for seed in range(16):
            rng = random.Random(seed)
            nodes = [{**copy.deepcopy(prototype['nodes'][0]), 'id': str(i),
                      'entity_id': 'tos.test.' + str(i if i < 6 else 6 + (i % 2)),
                      'type_id': 'tos.entity.claim' if i < 4 else 'tos.entity.thing', 'semantics': {}}
                     for i in range(8)]
            relations = []
            def edge(left, right, kind):
                relations.append({**copy.deepcopy(prototype['relations'][0]), 'id': 'edge:' + str(len(relations)),
                                  'from_id': str(left), 'to_id': str(right), 'relation_type_id': kind})
            for i in range(4):
                subject, object_id = str(rng.randrange(4, 8)), str(rng.randrange(4, 8))
                nodes[i]['semantics'] = {'claim': {'subject_node_id': subject, 'object_node_id': object_id,
                    'relation_type_id': 'tos.relation.correspondence-addressee', 'predicate_mapping_status': 'mapped'},
                    'assertion_contexts': [{'fields': {'polarity': {'value': rng.choice(['positive','negative','unknown'])},
                                                     'qualifiers': {'value': {'unknown': [None, False]}}}}]}
                edge(i, subject, 'tos.relation.has-subject')
                if (seed + i) % 5: edge(i, object_id, 'tos.relation.has-object')
                if (seed + i) % 7 == 0: edge(i, subject, 'tos.relation.has-subject')
                edge(i, rng.randrange(8), 'tos.relation.claim-supported-by')
                if (seed + i) % 3 == 0: edge(rng.randrange(8), i, 'tos.relation.related-to')
            # Same-identity projection links and cycles remain independently
            # accounted for, without imposing acyclicity on the corpus graph.
            edge(6, 6, 'tos.relation.projects')
            original = copy.deepcopy([nodes, relations])
            focus = str(seed % 8)
            scene = knowledge_scene(nodes, relations, focus)
            view = scene['compact']; total_paths += len(view['claim_paths'])
            vertices, folded = set(view['vertex_ids']), set(view['folded_vertex_ids'])
            self.assertFalse(vertices & folded)
            self.assertEqual(vertices | folded, {v['id'] for v in scene['vertices']})
            self.assertIn(scene['focus_vertex_id'], vertices)
            accounted = [*scene['collapsed_relation_ids'], *view['relation_ids']]
            by_relation = {r['id']: r for r in relations}
            by_node = {id: v['id'] for v in scene['vertices'] for id in v['node_ids']}
            for arc in scene['arcs']:
                if arc['relation_id'] in view['relation_ids']:
                    self.assertTrue({arc['from_id'], arc['to_id']} <= vertices)
            for path in view['claim_paths']:
                self.assertTrue({path['from_id'], path['to_id']} <= vertices)
                accounted.extend([*path['relation_ids'], *path['detail_relation_ids']])
                self.assertEqual(path['node_ids'][1], path['claim_node_id'])
                for relation_id, target in zip(path['relation_ids'], [path['node_ids'][0], path['node_ids'][2]]):
                    self.assertEqual(by_relation[relation_id]['from_id'], path['claim_node_id'])
                    self.assertEqual(by_relation[relation_id]['to_id'], target)
                self.assertFalse(path['reading']['standalone'])
            for retained in view['retained_claims']:
                self.assertIn(by_node[retained['node_id']], vertices)
            self.assertEqual(sorted(accounted), sorted(by_relation))
            self.assertEqual([nodes, relations], original)
            rng.shuffle(nodes); rng.shuffle(relations)
            self.assertEqual(knowledge_scene(nodes, relations, focus), scene)
        self.assertGreater(total_paths, 0)

    def test_scene_compact_claim_paths_preserve_expansion_and_do_not_assert_facts(self):
        from tos_access.knowledge import knowledge_scene
        graph = build_knowledge_graph(*self.fixture())
        node, edge = graph['nodes'][0], graph['relations'][0]
        graph['nodes'] = [{**copy.deepcopy(node), 'id': id, 'entity_id': 'tos.test.' + id,
                           'type_id': 'tos.entity.claim' if id in ('c', 'counter') else node['type_id']}
                          for id in ('subject', 'object', 'c', 'counter', 'evidence')]
        for n in graph['nodes']:
            if n['id'] in ('c', 'counter'):
                n['semantics'] = {'claim': {'subject_node_id': 'subject', 'object_node_id': 'object',
                    'relation_type_id': 'tos.relation.correspondence-addressee', 'predicate_mapping_status': 'mapped',
                    'review_status': 'contested' if n['id'] == 'counter' else 'unreviewed'},
                    'assertion_contexts': [{'fields': {'polarity': {'value': 'negative' if n['id'] == 'counter' else 'positive'},
                                                     'qualifiers': {'value': {'unknown_extension': False}}}}]}
                n['display_selection'] = {'fields': {'summary': {'content_available': True}, 'title': {'content_available': False}}}
        graph['relations'] = [{**copy.deepcopy(edge), 'id': claim + '-' + part, 'from_id': claim, 'to_id': target,
                               'relation_type_id': type_id}
                              for claim in ('c', 'counter') for part, target, type_id in (
                                  ('subject', 'subject', 'tos.relation.has-subject'),
                                  ('object', 'object', 'tos.relation.has-object'),
                                  ('evidence', 'evidence', 'tos.relation.claim-supported-by'))]
        before = copy.deepcopy(graph)
        scene = knowledge_scene(graph['nodes'], graph['relations'], 'subject')
        schema = self.schemas['knowledge-graph.v1.schema.json']
        validator = Draft202012Validator({'$ref': '#/$defs/scene', '$defs': schema['$defs']})
        validator.validate(scene)
        compact = scene['compact']
        self.assertEqual(compact['vertex_ids'], ['tos-scene:entity:tos.test.object', 'tos-scene:entity:tos.test.subject'])
        self.assertEqual(compact['relation_ids'], [])
        self.assertEqual(len(compact['claim_paths']), 2)
        self.assertEqual({p['claim_node_id'] for p in compact['claim_paths']}, {'c', 'counter'})
        for path in compact['claim_paths']:
            claim = path['claim_node_id']
            self.assertEqual(path['node_ids'], ['subject', claim, 'object'])
            self.assertEqual(path['relation_ids'], [claim + '-subject', claim + '-object'])
            self.assertEqual(path['detail_relation_ids'], [claim + '-evidence'])
            self.assertEqual(path['reading']['mode'], 'claim-with-mandatory-context')
            self.assertFalse(path['reading']['standalone'])
            self.assertEqual(path['reading']['wording_pointer'], '/display_selection/fields/summary')
            self.assertEqual(path['reading']['context_pointers'], ['/semantics', '/epistemic'])
            self.assertEqual(path['reading']['relation_context_ids'], [*path['relation_ids'], *path['detail_relation_ids']])
        self.assertEqual(graph, before)
        self.assertEqual(scene, knowledge_scene(list(reversed(graph['nodes'])), list(reversed(graph['relations'])), 'subject'))
        # Selecting the assertion or its grounds keeps that neighborhood explicit.
        for focus in ('c', 'evidence'):
            view = knowledge_scene(graph['nodes'], graph['relations'], focus)['compact']
            self.assertIn('tos-scene:entity:tos.test.' + focus, view['vertex_ids'])
            self.assertIn('c-subject', view['relation_ids'])
        # An unknown incident edge must not disappear behind a convenient line.
        graph['relations'].append({**edge, 'id': 'unexpected', 'from_id': 'c', 'to_id': 'evidence',
                                   'relation_type_id': 'tos.relation.related-to'})
        view = knowledge_scene(graph['nodes'], graph['relations'], 'subject')['compact']
        self.assertIn('unexpected', view['relation_ids'])
        self.assertIn('tos-scene:entity:tos.test.c', view['vertex_ids'])
        # A page with only one leg cannot invent the missing endpoint or path.
        partial = knowledge_scene(graph['nodes'], [r for r in graph['relations'] if r['id'] != 'counter-object'])['compact']
        self.assertFalse(partial['claim_paths'])
        # An incomplete Claim used as grounds must not be mistaken for a
        # disposable evidence-only vertex.
        evidence_claim = copy.deepcopy(before)
        evidence_claim['nodes'][-1]['type_id'] = 'tos.entity.claim'
        evidence_claim['nodes'][-1]['semantics'] = {}
        view = knowledge_scene(evidence_claim['nodes'], evidence_claim['relations'])['compact']
        self.assertIn('tos-scene:entity:tos.test.evidence', view['vertex_ids'])
        for case in ('unmapped', 'duplicate-leg', 'identity-collision'):
            changed = copy.deepcopy(before)
            c = next(n for n in changed['nodes'] if n['id'] == 'c')
            if case == 'unmapped': c['semantics']['claim']['predicate_mapping_status'] = 'unmapped'
            elif case == 'duplicate-leg': changed['relations'].append({**changed['relations'][0], 'id': 'duplicate'})
            else: changed['nodes'][0]['entity_id'] = c['entity_id']
            view = knowledge_scene(changed['nodes'], changed['relations'])['compact']
            self.assertNotIn('c', [p['claim_node_id'] for p in view['claim_paths']])
            self.assertTrue(any(r['node_id'] == 'c' for r in view['retained_claims']))
        invalid = copy.deepcopy(scene)
        invalid['compact']['claim_paths'][0]['reading']['standalone'] = True
        self.assertFalse(validator.is_valid(invalid))

    def test_overview_crosses_identity_carriers_without_a_relation_hop(self):
        from tos_access.exploration import ExplorationService
        graph = build_knowledge_graph(*self.fixture())
        node, edge = graph['nodes'][0], graph['relations'][0]
        graph['nodes'] = [{**node, 'id': id, 'native_id': id, 'entity_id': entity, 'source_graph': source}
                          for id, entity, source in [
                              ('p-nav', 'tos.agent.p', 'source-navigation'),
                              ('p-claim', 'tos.agent.p', 'source-claims'),
                              ('assertion', 'tos.claim.c', 'source-claims'),
                              ('w-claim', 'tos.work.w', 'source-claims'),
                              ('w-nav', 'tos.work.w', 'source-navigation')]]
        graph['relations'] = [
            {**edge, 'id': id, 'from_id': 'assertion', 'to_id': target, 'source_graph': 'source-claims',
             'predicate_id': predicate, 'relation_type_id': relation_type}
            for id, target, predicate, relation_type in [
                ('c-object', 'p-claim', 'has_object', 'tos.relation.has-object'),
                ('c-subject', 'w-claim', 'has_subject', 'tos.relation.has-subject')]]
        before = copy.deepcopy(graph)
        for center, other in [('p-nav', 'w-claim'), ('w-nav', 'p-claim')]:
            for profile in ['overview', 'all']:
                spec = {'schema_version': 'tos_lens_spec_v1', 'lens_id': 'identity-depth', 'explain': True,
                        'seed': {'focus_node_id': center}, 'node_query': {'enabled': False},
                        'traversal': {'depth': 2, 'profile': profile}}
                result = execute_knowledge_lens(graph, spec)
                ids = {n['id'] for n in result['nodes']}
                self.assertEqual(other in ids, profile == 'overview')
                if profile == 'overview':
                    carrier = center.replace('-nav', '-claim')
                    self.assertEqual(result['inclusion']['nodes'][carrier],
                                     {'kind': 'identity-carrier', 'via_node_id': center,
                                      'entity_id': next(n['entity_id'] for n in graph['nodes'] if n['id'] == center), 'depth': 0})
                    self.assertEqual(result['inclusion']['nodes'][other]['depth'], 2)
                service = ExplorationService(lambda: graph, work_limit=2)
                page = service.explore({'focus_node_id': center, 'max_depth': 2, 'profile': profile,
                                        'page_nodes': 1, 'page_relations': 1})
                found = set(); primary = []
                for _ in range(80):
                    found.update(n['id'] for n in page['nodes'])
                    primary.extend(page['page']['primary_node_ids'])
                    self.assertLessEqual(page['page']['work_units'], 2)
                    if not page['page']['next_cursor']: break
                    cursor = page['page']['next_cursor']
                    page = service.explore({'cursor': cursor})
                    self.assertEqual(service.explore({'cursor': cursor}), page)
                else: self.fail('identity continuation did not terminate')
                self.assertEqual(other in found, profile == 'overview')
                self.assertEqual(len(primary), len(set(primary)))
        limited = execute_knowledge_lens(graph, {**spec, 'seed': {'focus_node_id': 'p-nav'},
                    'traversal': {'depth': 2, 'profile': 'overview'}, 'limits': {'nodes': 1}})
        self.assertTrue(limited['counts']['identity_expansion_limited'])
        self.assertEqual(len(limited['nodes']), 1)
        excluded = execute_knowledge_lens(graph, {**spec, 'sources': ['source-navigation'],
                    'traversal': {'depth': 2, 'profile': 'overview'}})
        self.assertEqual([n['id'] for n in excluded['nodes']], ['w-nav'])
        for n in graph['nodes']: n['entity_id'] = 'shared-fallback'
        self.assertEqual(len(focus_knowledge_node(graph, 'p-nav', depth=2, profile='overview')['nodes']), 1)
        self.assertEqual(before['relations'], graph['relations'])

    def test_identity_shorter_path_promotes_an_already_queued_carrier(self):
        from tos_access.exploration import ExplorationService
        graph = build_knowledge_graph(*self.fixture())
        node, edge = graph['nodes'][0], graph['relations'][0]
        graph['nodes'] = [{**node, 'id': id, 'native_id': id, 'entity_id': 'tos.test.' + entity}
                          for id, entity in [('focus', 'f'), ('bridge', 'b'), ('early', 'shared'),
                                             ('late', 'shared'), ('target', 't')]]
        graph['relations'] = [{**edge, 'id': id, 'from_id': left, 'to_id': right}
                              for id, left, right in [('1', 'focus', 'bridge'), ('2', 'focus', 'early'),
                                                      ('3', 'bridge', 'late'), ('4', 'late', 'target')]]
        service = ExplorationService(lambda: graph, work_limit=2)
        page = service.explore({'focus_node_id': 'focus', 'max_depth': 2, 'page_nodes': 1, 'page_relations': 1})
        ids = set()
        for _ in range(80):
            ids.update(n['id'] for n in page['nodes'])
            if page['page']['next_cursor'] is None: break
            page = service.explore({'cursor': page['page']['next_cursor']})
        else: self.fail('promoted identity continuation did not terminate')
        self.assertIn('target', ids)

    def test_scene_groups_only_declared_identity_and_preserves_exact_carriers(self):
        graph = build_knowledge_graph(*self.fixture())
        prototype, edge = graph['nodes'][0], graph['relations'][0]
        graph['nodes'] = [{**prototype, 'id': id, 'native_id': id, 'entity_id': entity,
                           'source_graph': source}
                          for id, entity, source in [
                              ('nav', 'tos.test.person', 'source-navigation'),
                              ('claim-carrier', 'tos.test.person', 'source-claims'),
                              ('namesake', 'tos.test.other', 'source-claims'),
                              ('assertion', 'tos.claim.test', 'source-claims')]]
        graph['relations'] = [
            {**edge, 'id': 'projection', 'from_id': 'claim-carrier', 'to_id': 'nav',
             'relation_type_id': 'tos.relation.projects'},
            {**edge, 'id': 'assertion-subject', 'from_id': 'assertion', 'to_id': 'claim-carrier'},
            {**edge, 'id': 'contested-equivalence', 'from_id': 'nav', 'to_id': 'namesake',
             'relation_type_id': 'tos.relation.same-as'},
            {**edge, 'id': 'self-relation', 'from_id': 'nav', 'to_id': 'claim-carrier'},
        ]
        before = copy.deepcopy(graph)
        spec = {'schema_version': 'tos_lens_spec_v1', 'lens_id': 'scene-contract',
                'seed': {'focus_node_id': 'claim-carrier'}, 'traversal': {'depth': 0}}
        result = execute_knowledge_lens(graph, spec)
        scene = result['scene']
        self.assertEqual(len(scene['vertices']), 3)
        person = next(v for v in scene['vertices'] if v['entity_id'] == 'tos.test.person')
        self.assertEqual(person['node_ids'], ['claim-carrier', 'nav'])
        self.assertEqual(person['representative_node_id'], 'nav')
        self.assertEqual(scene['focus_vertex_id'], person['id'])
        self.assertEqual(scene['collapsed_relation_ids'], ['projection'])
        self.assertEqual({a['relation_id'] for a in scene['arcs']},
                         {'assertion-subject', 'contested-equivalence', 'self-relation'})
        self.assertEqual(next(a for a in scene['arcs'] if a['relation_id'] == 'self-relation')['from_id'], person['id'])
        for item in result['nodes']:
            original = next(n for n in before['nodes'] if n['id'] == item['id'])
            self.assertEqual({k: item[k] for k in original}, original)
        self.assertEqual(graph, before)
        Draft202012Validator(self.schemas['lens-result.v1.schema.json'], registry=self.registry).validate(result)
        graph['nodes'].reverse()
        graph['relations'].reverse()
        self.assertEqual(execute_knowledge_lens(graph, spec)['scene'], scene)
        # Source filters and paging cannot leak a carrier excluded from this packet.
        for update in ({'sources': ['source-claims']}, {'pagination': {'nodes': 1, 'relations': 1}}):
            page = execute_knowledge_lens(graph, {**spec, **update})
            self.assertEqual({id for v in page['scene']['vertices'] for id in v['node_ids']},
                             {n['id'] for n in page['nodes']})
            self.assertEqual({a['relation_id'] for a in page['scene']['arcs']} | set(page['scene']['collapsed_relation_ids']),
                             {r['id'] for r in page['relations']})
            self.assertEqual(next(v for v in page['scene']['vertices'] if 'claim-carrier' in v['node_ids'])['id'], person['id'])
        # Fallback/native IDs are not evidence that different carriers coincide.
        from tos_access.knowledge import knowledge_scene
        for node in graph['nodes']:
            node['entity_id'] = 'unqualified-shared-value'
        fallback = knowledge_scene(graph['nodes'], graph['relations'])
        self.assertEqual(len(fallback['vertices']), 4)
        self.assertTrue(all(v['entity_id'] is None for v in fallback['vertices']))
        self.assertEqual(fallback['collapsed_relation_ids'], [])

    def test_overview_does_not_infer_proximity_from_shared_record_maker(self):
        from tos_access.exploration import ExplorationService
        graph = build_knowledge_graph(*self.fixture())
        prototype = graph['nodes'][0]
        graph['nodes'] = [{**prototype, 'id': id, 'native_id': id, 'entity_id': 'tos.test.' + id}
                          for id in ('claim', 'subject', 'maker', 'unrelated')]
        edge = graph['relations'][0]
        graph['relations'] = [
            {**edge, 'id': 'subject-edge', 'from_id': 'claim', 'to_id': 'subject',
             'predicate_id': 'has_subject', 'relation_type_id': 'tos.relation.has-subject'},
            {**edge, 'id': 'maker-edge', 'from_id': 'claim', 'to_id': 'maker',
             'predicate_id': 'made_by', 'relation_type_id': 'tos.relation.made-by'},
            {**edge, 'id': 'other-edge', 'from_id': 'unrelated', 'to_id': 'maker',
             'predicate_id': 'made_by', 'relation_type_id': 'tos.relation.made-by'},
        ]
        for technical_type in ('tos.relation.made-by', 'tos.relation.generated-by'):
            for relation in graph['relations'][1:]:
                relation['relation_type_id'] = technical_type
            for profile, expected in [('overview', {'claim', 'subject'}),
                                      ('all', {'claim', 'subject', 'maker', 'unrelated'})]:
                with self.subTest(technical_type=technical_type, profile=profile):
                    result = focus_knowledge_node(graph, 'claim', depth=2, profile=profile)
                    self.assertEqual({n['id'] for n in result['nodes']}, expected)
                    page = ExplorationService(lambda: graph).explore(
                        {'focus_node_id': 'claim', 'max_depth': 2, 'profile': profile})
                    self.assertEqual({n['id'] for n in page['nodes']}, expected)
            # A raw predicate spelling alone does not classify an unknown extension.
            for relation in graph['relations'][1:]:
                relation['relation_type_id'] = 'tos.relation.related'
            result = focus_knowledge_node(graph, 'claim', depth=2)
            self.assertEqual({n['id'] for n in result['nodes']}, {'claim', 'subject', 'maker', 'unrelated'})

    def human_form_node(self):
        subject = {'id': 'tos.record.form-fixture', 'version': 1, 'digest': 'sha256:' + 'a' * 64}
        packet = {'schema_version': 'tos_human_form_materialization_v1',
                  'form': {'id': 'tos.form.fixture-fr', 'version': 1, 'digest': 'sha256:' + 'b' * 64},
                  'subject': subject, 'state': 'ready', 'role': 'statement', 'language': 'fr', 'script': 'Latn',
                  'display_text': 'Cette attribution n’est pas établie.',
                  'context': [{'slot': 'qualifiers', 'binding': {'record': subject, 'pointer': '/qualifiers'},
                               'value': {'negated': True, 'x-unknown': False, 'confidence': 0, 'condition': None}}],
                  'issues': [], 'admission': None, 'performs_semantic_assessment': False,
                  'standalone_reading': False, 'derivation': 'source-copy', 'dependencies': [subject]}
        return {'content_revision': 'c' * 64, 'attributes': {
            'source_record': {'record_id': subject['id'], 'record_version': subject['version']},
            'source_sha256': 'a' * 64, 'human_forms_source_ref': 'ToS/example.human-forms.json', 'human_forms': [packet]}}

    def test_source_form_selection_keeps_exact_wording_context_and_language_fallback(self):
        from tos_access.knowledge import select_human_forms
        node = self.human_form_node()
        before = copy.deepcopy(node)
        for language, reason in [('FR', 'exact-language'), ('fr-CA', 'less-specific-language'),
                                 ('auto', 'automatic'), ('de', 'fallback')]:
            result = select_human_forms(node, language)
            selected = result['roles']['statement']
            self.assertEqual(selected['state'], 'ready')
            self.assertEqual(selected['reason'], reason)
            self.assertEqual(selected['packet'], node['attributes']['human_forms'][0])
            self.assertEqual(result['content_revision'], node['content_revision'])
            self.assertFalse(result['performs_assessment'])
            schema = json.loads((ACCESS_ROOT / 'contracts/knowledge-graph.v1.schema.json').read_text())
            registry = Registry().with_resource(schema['$id'], Resource.from_contents(schema))
            Draft202012Validator({'$ref': schema['$id'] + '#/$defs/humanFormSelection'}, registry=registry).validate(result)
        self.assertEqual(select_human_forms(node, 'original')['roles']['statement']['state'], 'unavailable')
        self.assertEqual(node, before)
        result['roles']['statement']['packet']['display_text'] = 'result-only mutation'
        self.assertEqual(node, before)

    def test_source_form_selection_does_not_adjudicate_competing_forms(self):
        from tos_access.knowledge import select_human_forms
        node = self.human_form_node()
        alternative = copy.deepcopy(node['attributes']['human_forms'][0])
        alternative['form']['id'] = 'tos.form.competing-fr'
        alternative['form']['digest'] = 'sha256:' + 'e' * 64
        alternative['display_text'] = 'Une autre lecture demeure possible.'
        node['attributes']['human_forms'].append(alternative)
        result = select_human_forms(node, 'fr')
        self.assertEqual(result['roles']['statement']['state'], 'ambiguous')
        self.assertIsNone(result['roles']['statement']['packet'])

        for key in ('language', 'display_text'):
            broken = self.human_form_node()
            del broken['attributes']['human_forms'][0][key]
            self.assertEqual(select_human_forms(broken, 'fr')['state'], 'invalid')
        broken = self.human_form_node()
        broken['attributes']['human_forms'] = None
        self.assertEqual(select_human_forms(broken, 'fr')['state'], 'invalid')
        self.assertEqual(len(result['candidates']), 2)
        alternative['language'] = 'de'
        self.assertEqual(select_human_forms(node, 'fr')['roles']['statement']['state'], 'ready')
        self.assertEqual(select_human_forms(node, 'auto')['roles']['statement']['state'], 'ambiguous')

    def test_source_form_binding_and_nonready_states_cannot_emit_wording(self):
        from tos_access.knowledge import select_human_forms
        node = self.human_form_node()
        node['attributes']['source_sha256'] = 'd' * 64
        self.assertEqual(select_human_forms(node, 'fr')['state'], 'invalid')
        node = self.human_form_node()
        packet = node['attributes']['human_forms'][0]
        packet['state'] = 'restricted'
        self.assertEqual(select_human_forms(node, 'fr')['state'], 'invalid')
        packet.update(display_text=None, context=[])
        result = select_human_forms(node, 'fr')
        self.assertEqual(result['candidates'][0]['state'], 'restricted')
        self.assertEqual(result['roles']['statement']['state'], 'unavailable')
        self.assertIsNone(result['roles']['statement']['packet'])

    def test_source_form_budget_returns_a_ref_instead_of_truncating_context(self):
        from tos_access.knowledge import select_human_forms, HUMAN_FORM_SELECTION_BUDGET, _form_delivery_cost
        node = self.human_form_node()
        packet = node['attributes']['human_forms'][0]
        packet['context'][0]['value']['long_qualification'] = '界' * 15000
        result = select_human_forms(node, 'fr')
        selected = result['roles']['statement']
        self.assertEqual(selected['state'], 'over-budget')
        self.assertEqual(selected['form'], packet['form'])
        self.assertIsNone(selected['packet'])
        self.assertLessEqual(_form_delivery_cost(result), HUMAN_FORM_SELECTION_BUDGET)
        self.assertLessEqual(len(json.dumps(result, ensure_ascii=False).encode()), HUMAN_FORM_SELECTION_BUDGET)

    def test_original_selection_requires_intact_source_bound_language_context(self):
        from tos_access.knowledge import select_human_forms
        node = self.human_form_node()
        packet = node['attributes']['human_forms'][0]
        self.assertEqual(select_human_forms(node, 'original')['roles']['statement']['state'], 'unavailable')
        metadata = {'binding': {'record': {'id': 'tos.record.language-context', 'version': 1,
                                         'digest': 'sha256:' + 'd' * 64}, 'pointer': ''},
                    'value': {'language': 'fr', 'script': 'Latn', 'relation': 'original', 'source': None,
                              'x-source': {'unknown': False}}}
        packet['language_context'] = metadata
        packet['dependencies'].append(metadata['binding']['record'])
        self.assertEqual(select_human_forms(node, 'original')['state'], 'invalid')
        packet['context'].append({'slot': 'language_context', **copy.deepcopy(metadata)})
        selected = select_human_forms(node, 'original')['roles']['statement']
        self.assertEqual(selected['state'], 'ready')
        self.assertEqual(selected['reason'], 'original')
        self.assertEqual(selected['packet'], packet)
        # False and zero in a source qualification cannot collapse under equality.
        packet['context'][-1]['value']['x-source']['unknown'] = 0
        self.assertEqual(select_human_forms(node, 'original')['state'], 'invalid')

    def test_registered_predicates_keep_the_source_russian_vocabulary(self):
        import csv
        from tos_access.knowledge import _normalize_relation
        root = ACCESS_ROOT.parent
        registry = json.loads((root / 'ToS/doctrine/semantic-interchange/relation-types.v1.json').read_text())
        family = next(r for r in registry['relations'] if r['relation_type_id'] == 'tos.relation.canon-registered-predicate')
        with (root / family['owner_ref']).open() as stream:
            labels = {row['predicate_id']: row['predicate_ru'] for row in csv.DictReader(stream)}
        for mapping in family['source_mappings']:
            predicate = mapping['source_predicate_id']
            with self.subTest(source=mapping['source_graph'], predicate=predicate):
                self.assertEqual(mapping.get('labels', {}).get('ru'), labels[predicate])
                edge = _normalize_relation({'edge_id': 'e', 'from_id': 'a', 'to_id': 'b', 'predicate_id': predicate},
                    mapping['source_graph'], {}, relation_type_entries={family['relation_type_id']: family},
                    relation_type_mappings={(mapping['source_graph'], predicate, mapping['scope']): family['relation_type_id']})
                self.assertEqual(edge['display']['label']['ru'], labels[predicate])
                self.assertEqual(edge['predicate_id'], predicate)

    def test_entity_mapping_label_is_scoped_and_does_not_replace_authored_display(self):
        entry = {'type_id': 'tos.entity.meta', 'parent_type_ids': [],
                 'labels': {'default': 'Metadata', 'ru': 'Метаданные'},
                 'source_mappings': [{'source_graph': 'philosophy', 'source_kind_id': 'graph-view',
                                      'labels': {'default': 'Graph view', 'ru': 'Представление графа'}}]}
        options = {'entity_type_entries': {'tos.entity.meta': entry},
                   'entity_type_mappings': {(source, 'graph-view'): 'tos.entity.meta' for source in ('philosophy', 'repository')}}
        item = {'node_id': 'v', 'node_type': 'graph-view'}
        node = _normalize_node(item, 'philosophy', **options)
        self.assertEqual(node['display']['kind_label']['ru'], 'Представление графа')
        self.assertEqual(node['type_id'], 'tos.entity.meta')
        other = _normalize_node(item, 'repository', **options)
        self.assertEqual(other['display']['kind_label']['ru'], 'Метаданные')
        authored = _normalize_node({**item, 'display': {'kind_label': {'ru': 'Авторская подпись'}}}, 'philosophy', **options)
        self.assertEqual(authored['display']['kind_label']['ru'], 'Авторская подпись')

    def test_synthesized_description_keeps_machine_metadata_out_of_prose(self):
        node = _normalize_node({
            'node_id': 'work:sample', 'label': 'Так говорил Заратустра',
            'node_type': 'work', 'source_ref': 'ToS/work/sample.json',
            'status': 'candidate-not-reviewed', 'owner_branch': 'ToS/candidate-intake',
            'route_hint': 'review-ledger/pending',
            'properties': {'review_reason': 'machine_generated', 'source_document': 'payload/source.xml'},
        }, 'philosophy')
        prose = node['display']['summary']
        self.assertIn('описание', prose['ru'].lower())
        for value in ('ToS/', 'candidate-not-reviewed', 'review-ledger', 'machine_generated', 'payload/', 'review'):
            self.assertNotIn(value, json.dumps(prose, ensure_ascii=False))
        self.assertEqual(node['attributes']['status'], 'candidate-not-reviewed')
        self.assertEqual(node['attributes']['review_reason'], 'machine_generated')
        self.assertIn('ToS/work/sample.json', node['source_refs'])
        self.assertEqual(node['display']['summary_state'], 'metadata-synthesis')
        self.assertFalse(node['display']['provenance']['source_summary_available'])

    def test_synthesized_relation_statement_uses_available_russian_labels(self):
        from tos_access.knowledge import _relation_display
        left = {'display': {'title': {'default': 'Thus Spoke Zarathustra', 'ru': 'Так говорил Заратустра'}}}
        right = {'display': {'title': {'default': 'Friedrich Nietzsche', 'ru': 'Фридрих Ницше'}}}
        display = _relation_display({}, 'authored_by', left, right, ['ToS/work/sample.json'], {
            'labels': {'default': 'authored by', 'ru': 'написано автором'},
            'source_mappings': [{'source_predicate_id': 'authored_by'}],
        })
        self.assertEqual(display['statement']['ru'], 'Так говорил Заратустра — написано автором → Фридрих Ницше.')
        self.assertIn('пояснение', display['explanation']['ru'].lower())
        self.assertNotIn('ToS/', json.dumps(display['explanation']))
        self.assertFalse(display['provenance']['source_explanation_available'])

    def test_source_prose_and_translations_are_preserved_not_sanitized(self):
        from tos_access.knowledge import _node_display, _relation_display
        summary = {'default': 'Source review: ToS/example', 'ru': 'Авторское описание: review ToS/example'}
        display = _node_display({'display': {'summary': summary}}, 'work', ['ToS/example'])
        self.assertEqual(display['summary']['default'], summary['default'])
        self.assertEqual(display['summary']['ru'], summary['ru'])
        self.assertTrue(display['provenance']['source_summary_available'])
        translated = {'ru': 'Описание из источника', 'en': 'Source description'}
        display = _node_display({'display': {'summary': translated}}, 'work', ['ToS/example'])
        self.assertTrue(display['provenance']['source_summary_available'])
        self.assertEqual(display['summary_state'], 'source-derived')
        self.assertEqual(display['summary']['ru'], translated['ru'])
        relation = _relation_display({'display': {
            'statement': {'ru': 'Точная авторская формулировка'},
            'explanation': translated,
        }}, 'related_to', None, None, ['ToS/example'])
        self.assertEqual(relation['statement']['ru'], 'Точная авторская формулировка')
        self.assertEqual(relation['explanation']['ru'], translated['ru'])
        self.assertTrue(relation['provenance']['source_explanation_available'])

    def test_catalog_example_limit_does_not_limit_counts_or_types(self):
        from tos_access.knowledge import _attribute_catalog
        items = [{'source_graph': 'philosophy' if i < 5 else 'canon',
                  'attributes': {'labels': [f'value-{i}', None, {'nested': i}]}}
                 for i in range(8)]
        entry, = _attribute_catalog(items, 'node')
        self.assertEqual(entry, {'field': 'attributes.labels', 'item_count': 8,
                                'value_types': {'array': 8},
                                'array_item_types': {'string': 8, 'null': 8, 'object': 8},
                                'sources': ['canon', 'philosophy'],
                                'examples': [f'value-{i}' for i in range(5)]})

    def test_original_only_relation_prose_survives_compact_delivery(self):
        from tos_access.knowledge import _lens_carrier, _normalize_relation
        source_text = 'Werk B stammt nicht von Person A.'
        item = {'edge_id': 'source-negative', 'from_id': 'b', 'to_id': 'a',
                'predicate_id': 'authored_by', 'source_ref': 'ToS/fixture/attribution.json',
                'display': {'statement': {'original': source_text}}}
        relation = _normalize_relation(item, 'philosophy', {})
        for carrier in (relation, _lens_carrier(relation, 'compact')):
            with self.subTest(compact='source_record' not in carrier):
                # Both the agent/default reader and UI localized() must receive
                # the source negation, not an affirmative endpoint synthesis.
                self.assertEqual(carrier['display']['statement']['default'], source_text)
                self.assertEqual(carrier['display']['statement']['original'], source_text)
                self.assertIsNone(carrier['display']['statement']['ru'])
                self.assertIsNone(carrier['display']['statement']['en'])
                self.assertEqual(carrier['display']['provenance']['statement'], 'source-derived')
        self.assertEqual(relation['source_record']['payload'], item)

    def test_statement_provenance_distinguishes_source_text_from_endpoint_synthesis(self):
        from tos_access.knowledge import _relation_display
        for source in ('Exact source wording.', {'ru': 'Точная исходная формулировка'},
                       {'original': 'Nicht belegt.'}):
            display = _relation_display({'display': {'statement': source}}, 'authored_by', None, None, [])
            self.assertEqual(display['provenance']['statement'], 'source-derived')
        display = _relation_display({}, 'authored_by', None, None, [])
        self.assertEqual(display['provenance']['statement'], 'endpoint-label-synthesis')

    def test_language_forms_survive_source_normalization_schema_and_compact_delivery(self):
        from tos_access.knowledge import _normalize_relation, _lens_carrier, _display_field_catalog
        schema = json.loads((ACCESS_ROOT / 'contracts/knowledge-graph.v1.schema.json').read_text())
        validator = Draft202012Validator({'$ref': '#/$defs/localizedText', '$defs': schema['$defs']})
        # Synthetic prose checks transport fidelity, not historical attribution.
        for language in ('fr', 'de-Latn', 'zh-Hant', 'grc-Grek', 'x-research', 'i-klingon'):
            with self.subTest(language=language):
                prose = 'Attribution non établie; sous réserve de nouvelles sources.'
                item = {'edge_id': 'r', 'from_id': 'a', 'to_id': 'b', 'predicate_id': 'authored_by',
                        'display': {'statement': {language: prose}, 'explanation': {language: prose}}}
                relation = _normalize_relation(item, 'philosophy', {})
                for carrier in (relation, _lens_carrier(relation, 'compact')):
                    for field in ('statement', 'explanation'):
                        forms = carrier['display'][field]
                        validator.validate(forms)
                        self.assertEqual(forms[language], prose)
                        self.assertEqual(forms['default'], prose)
                        self.assertIsNone(forms['ru'])
                        self.assertIsNone(forms['en'])
                    self.assertEqual(carrier['display']['provenance']['statement'], 'source-derived')
                self.assertEqual(relation['source_record']['payload'], item)
                self.assertIn({'field': f'display.statement.{language}', 'available_item_count': 1},
                              _display_field_catalog([relation], 'relation'))

    def test_multilingual_source_titles_and_registry_labels_keep_new_languages(self):
        from tos_access.knowledge import _node_display, _relation_display
        node = _node_display({'node_id': 'technical-id', 'multilingual': {'label': {'grc-Grek': 'λόγος'}},
                              'properties': {'variant_labels': [{'language': 'la-Latn', 'value': 'ratio'}]}},
                             'concept', [], {'labels': {'default': 'concept', 'fr': 'concept philosophique'}})
        self.assertEqual(node['title']['default'], 'λόγος')
        self.assertEqual(node['title']['grc-Grek'], 'λόγος')
        self.assertEqual(node['title']['la-Latn'], 'ratio')
        self.assertEqual(node['kind_label']['fr'], 'concept philosophique')
        relation = _relation_display({}, 'related', None, None, [],
                                     {'labels': {'default': 'related', 'fr': 'en relation avec'}})
        self.assertEqual(relation['label']['fr'], 'en relation avec')
        self.assertIn('en relation avec', relation['statement']['fr'])

    def test_language_query_and_registry_schemas_share_the_transport_contract(self):
        schema = json.loads((ACCESS_ROOT / 'contracts/lens-spec.v1.schema.json').read_text())
        spec = {'schema_version': 'tos_lens_spec_v1', 'lens_id': 'language-test', 'language': 'fr-CA',
                'title': {'fr-CA': 'Lecture'},
                'node_query': {'filters': [{'field': 'display.title.grc-Grek', 'op': 'eq', 'value': 'λόγος'}]},
                'relation_query': {'filters': [{'field': 'display.statement.fr', 'op': 'contains', 'value': 'non'}]}}
        Draft202012Validator(schema).validate(spec)
        normalized = normalize_lens_spec(spec)
        Draft202012Validator(schema).validate(normalized)
        self.assertEqual(normalized['title']['default'], 'Lecture')
        self.assertEqual(normalized['title']['fr-CA'], 'Lecture')
        # This is a request-size boundary, not a source-language vocabulary.
        language_limit = 'fr' + '-abcdefgh' * 13 + '-abcdefgh'
        self.assertEqual(len(language_limit), 128)
        bounded = {**spec, 'language': language_limit}
        Draft202012Validator(schema).validate(bounded)
        self.assertEqual(normalize_lens_spec(bounded)['language'], language_limit)
        excessive = {**spec, 'language': 'fra' + language_limit[2:]}
        self.assertFalse(Draft202012Validator(schema).is_valid(excessive))
        with self.assertRaises(ValueError):
            normalize_lens_spec(excessive)
        for key in ('fr\n', 'fr_CA', '__proto__', 'script.js'):
            invalid = {**spec, 'title': {key: 'not a declared form key'}}
            self.assertFalse(Draft202012Validator(schema).is_valid(invalid))
            with self.assertRaises(ValueError):
                normalize_lens_spec(invalid)
        for path in ('semantic-entity-type-registry.schema.json', 'semantic-relation-type-registry.schema.json'):
            registry = json.loads((ACCESS_ROOT.parent / 'ToS/contracts' / path).read_text())
            validator = Draft202012Validator({'$ref': '#/$defs/localizedLabel', '$defs': registry['$defs']})
            validator.validate({'default': 'Thing', 'ru': None, 'en': None, 'fr-CA': 'Objet'})
            self.assertFalse(validator.is_valid({'default': 'Thing', 'ru': None, 'en': None, 'fr-CA': 42}))
        for field in ('display.title.__proto__', 'display.title.fr.name', 'display.summary_state.fr'):
            with self.assertRaises(ValueError):
                normalize_lens_spec({**spec, 'node_query': {'filters': [{'field': field, 'op': 'eq', 'value': 'no'}]}})

    def test_assertion_context_survives_compact_without_collapsing_unknown_false_or_conflict(self):
        from tos_access.knowledge import _lens_carrier, _normalize_relation, _stable_digest
        claim = {'claim_id': 'tos.claim.fixture.negative', 'claim_version': 2,
                 'polarity': 'negative', 'condition': None, 'qualifiers': {'x-unknown': {'value': False}},
                 'confidence': {'value': 0.2, 'meaning': 'maker_declared_uncertainty_not_truth_probability'},
                 'review_status': 'ambiguous', 'alternative_claim_refs': [],
                 'competing_claim_refs': ['tos.claim.fixture.alternative']}
        item = {'node_id': 'claim', 'node_type': 'claim', 'source_ref': 'ToS/fixture/claims.jsonl',
                'properties': {'source_claim': claim, 'review_status': 'accepted'}}
        node = _normalize_node(item, 'source-claims')
        context, = node['semantics']['assertion_contexts']
        self.assertEqual(context['source_record_digest'], _stable_digest(item))
        for field, value in claim.items():
            self.assertEqual(context['fields'][field]['value'], value)
            self.assertEqual(context['fields'][field]['source_pointer'], '/properties/source_claim/' + field)
        self.assertNotIn('attribution', context['fields'])
        self.assertIsNone(context['fields']['condition']['value'])
        self.assertEqual(context['fields']['alternative_claim_refs']['value'], [])
        self.assertFalse(context['fields']['qualifiers']['value']['x-unknown']['value'])
        self.assertEqual(context['conflicts'][0]['lower_priority']['value'], 'accepted')
        self.assertEqual(context['conflicts'][0]['higher_priority']['value'], 'ambiguous')
        typed_conflict = _normalize_node({'node_id': 'typed', 'qualifiers': {'value': False},
            'properties': {'qualifiers': {'value': 0}}}, 'philosophy')['semantics']['assertion_contexts'][0]
        self.assertEqual(len(typed_conflict['conflicts']), 1)
        self.assertEqual(_lens_carrier(node, 'compact')['semantics'], node['semantics'])
        relation = _normalize_relation({'edge_id': 'e', 'from_id': 'a', 'to_id': 'b',
            'predicate_id': 'attributed_to', 'claim_ref': claim['claim_id']}, 'source-claims', {},
            claim_contexts=[{**context, 'binding_role': 'referenced-claim'}])
        contexts = _lens_carrier(relation, 'compact')['semantics']['assertion_contexts']
        self.assertEqual([entry['binding_role'] for entry in contexts], ['carrier', 'referenced-claim'])
        self.assertEqual(contexts[1]['fields']['competing_claim_refs']['value'], claim['competing_claim_refs'])
        self.assertEqual(node['source_record']['payload'], item)
        schema = json.loads((ACCESS_ROOT / 'contracts/knowledge-graph.v1.schema.json').read_text())
        validator = Draft202012Validator({'$ref': '#/$defs/assertionContext', '$defs': schema['$defs']})
        for entry in contexts:
            validator.validate(entry)

    def test_display_selection_reports_exact_fallback_missing_and_ambiguous_forms(self):
        from tos_access.knowledge import select_display_form
        forms = {'default': 'Unspecified language', 'original': 'λόγος', 'fr': 'mot',
                 'zh-Hant': '詞', 'de': 'Wort', 'x-research': 'Unassessed wording'}
        for requested, key, reason in (
            ('FR', 'fr', 'exact-language'), ('fr-CA', 'fr', 'less-specific-language'),
            ('zh-Hant-TW', 'zh-Hant', 'less-specific-language'),
            ('de-DE-u-co-phonebk', 'de', 'less-specific-language'),
            ('x-research', 'x-research', 'exact-language'),
            ('es', 'default', 'fallback'), ('auto', 'default', 'automatic'),
            ('original', 'original', 'original-role'),
        ):
            with self.subTest(requested=requested):
                result = select_display_form(forms, requested, original_language='grc-Grek')
                self.assertEqual((result['selected_key'], result['reason'], result['text']),
                                 (key, reason, forms[key]))
                expected_language = 'grc-Grek' if key == 'original' else None if key == 'default' else key
                self.assertEqual(result['actual_language'], expected_language)
        self.assertIsNone(select_display_form(forms, 'original')['actual_language'])
        self.assertEqual(select_display_form({'fr': None}, 'fr')['reason'], 'missing')
        conflicting = select_display_form({'fr': 'oui', 'FR': 'non', 'default': 'fallback'}, 'fr-CA')
        self.assertEqual(conflicting['reason'], 'ambiguous-language-key')
        self.assertIsNone(conflicting['text'])
        self.assertEqual(conflicting['available_keys'], ['FR', 'default', 'fr'])

    def test_lens_display_selection_binds_revision_and_preserves_essential_context(self):
        from tos_access.knowledge import _lens_carrier
        node = _normalize_node({'node_id': 'c', 'node_type': 'claim',
            'multilingual': {'label': {'original': 'λόγος'},
                            'language': {'original_language': 'grc-Grek', 'script': 'Grek'}},
            'properties': {'claim_id': 'claim.c', 'polarity': 'negative', 'review_status': 'contested'}}, 'philosophy')
        schema = json.loads((ACCESS_ROOT / 'contracts/knowledge-graph.v1.schema.json').read_text())
        validator = Draft202012Validator({'$ref': '#/$defs/displaySelection', '$defs': schema['$defs']})
        before = copy.deepcopy(node)
        for language in ('original', 'fr-CA', 'auto'):
            full = _lens_carrier(node, 'full', language=language)
            compact = _lens_carrier(node, 'compact', language=language)
            selection = compact['display_selection']
            validator.validate(selection)
            self.assertEqual(selection, full['display_selection'])
            self.assertEqual(selection['content_revision'], node['content_revision'])
            self.assertFalse(selection['fields']['summary']['content_available'])
            self.assertEqual(selection['essential_context_pointers'], ['/semantics/assertion_contexts/0'])
            self.assertEqual(compact['semantics']['assertion_contexts'], node['semantics']['assertion_contexts'])
            if language == 'original':
                self.assertEqual(selection['fields']['title']['actual_language'], 'grc-Grek')
                self.assertIsNone(selection['fields']['summary']['actual_language'])
        self.assertEqual(node, before)
        for item in ({'node_id': 'only-id'}, {'node_id': 'only-path', 'path': 'ToS/fixture.json'}):
            missing = _lens_carrier(_normalize_node(item, 'philosophy'), 'compact', language='auto')
            self.assertFalse(missing['display_selection']['fields']['title']['content_available'])
        self.assertTrue(_lens_carrier(node, 'compact', language='original')['display_selection']['fields']['title']['content_available'])
        for unknown in ('grc', {'original_language': {'x-undecoded': 'grc'}}, {'original_language': 'not_a_tag'}):
            malformed = _normalize_node({'node_id': 'u', 'multilingual': {
                'label': {'original': 'λόγος'}, 'language': unknown}}, 'philosophy')
            result = _lens_carrier(malformed, 'compact', language='original')
            self.assertIsNone(result['display_selection']['fields']['title']['actual_language'])
            self.assertEqual(result['semantics']['language_context']['language'], unknown)

    def test_claim_context_change_invalidates_dependent_relation_without_reprocessing_others(self):
        from tos_access.knowledge import _normalize_relation, _assertion_context
        from tos_access.normalization_cache import NormalizationCache
        source = {'node_id': 'claim', 'properties': {'claim_id': 'c', 'polarity': 'negative'}}
        edge = {'edge_id': 'dependent', 'from_id': 'a', 'to_id': 'b', 'predicate_id': 'attributed_to', 'claim_ref': 'c'}
        unrelated = {'edge_id': 'other', 'from_id': 'x', 'to_id': 'y', 'predicate_id': 'next'}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'steps.sqlite'
            with NormalizationCache(path, 'processor-v1'):
                original = _normalize_relation(edge, 'philosophy', {}, claim_contexts=[_assertion_context(source)])
                other = _normalize_relation(unrelated, 'philosophy', {})
            source['properties']['polarity'] = 'uncertain'
            with NormalizationCache(path, 'processor-v1') as cache:
                revised = _normalize_relation(edge, 'philosophy', {}, claim_contexts=[_assertion_context(source)])
                self.assertEqual(_normalize_relation(unrelated, 'philosophy', {}), other)
            self.assertEqual((cache.misses, cache.hits), (1, 1))
            self.assertNotEqual(revised['content_revision'], original['content_revision'])
            self.assertEqual(revised['source_record'], original['source_record'])
            self.assertEqual(revised['id'], original['id'])

    def test_context_join_preserves_disagreeing_records_and_unknown_reference_shape(self):
        philosophy = {'nodes': [
            {'node_id': 'c1', 'node_type': 'claim', 'properties': {'claim_id': 'c', 'polarity': 'negative'}},
            {'node_id': 'c2', 'node_type': 'claim', 'properties': {'claim_id': 'c', 'polarity': 'positive'}},
        ], 'edges': [
            {'edge_id': 'e', 'from_id': 'c1', 'to_id': 'c2', 'predicate_id': 'related', 'claim_ref': 'c'},
            {'edge_id': 'unknown', 'from_id': 'c1', 'to_id': 'c2', 'claim_ref': {'new-shape': ['c']}},
        ]}
        graph = build_knowledge_graph({}, philosophy)
        edge = next(edge for edge in graph['relations'] if edge['native_id'] == 'e')
        contexts = [entry for entry in edge['semantics']['assertion_contexts'] if entry['binding_role'] == 'referenced-claim']
        self.assertEqual({entry['fields']['polarity']['value'] for entry in contexts}, {'negative', 'positive'})
        self.assertEqual(len({entry['source_record_digest'] for entry in contexts}), 2)
        unknown = next(edge for edge in graph['relations'] if edge['native_id'] == 'unknown')
        self.assertEqual(unknown['source_record']['payload']['claim_ref'], {'new-shape': ['c']})
        self.assertEqual(len(unknown['semantics']['assertion_contexts']), 1)

    def test_finalization_reuses_unchanged_revision_without_aliasing_source(self):
        from unittest.mock import patch
        from tos_access.knowledge import _final_node_value, _stamp_content_revision
        node = _normalize_node({'node_id': 'n', 'label': 'Ницше',
                                'properties': {'nested': {'value': 'original'}},
                                'view_ids': ['a', 'b']}, 'philosophy')
        with patch('tos_access.knowledge._stamp_content_revision', wraps=_stamp_content_revision) as stamp:
            unchanged = _final_node_value(node, None, [])
            same_membership = _final_node_value(node, None, ['a'])
            stamp.assert_not_called()
            self.assertEqual(unchanged, node)
            self.assertEqual(same_membership, node)
            unchanged['attributes']['nested']['value'] = 'changed'
            self.assertEqual(node['attributes']['nested']['value'], 'original')
            changed = _final_node_value(node, None, ['c'])
            stamp.assert_called_once()
            self.assertEqual(changed['content_revision'], _content_revision(changed))
            self.assertNotEqual(changed['content_revision'], node['content_revision'])
        updated = _final_node_value(node, ({'subject': 'n'}, {'claim_ref': 'c'}), [])
        self.assertEqual(updated['content_revision'], _content_revision(updated))
        self.assertNotEqual(updated['content_revision'], node['content_revision'])

    def test_stable_revision_wire_format_survives_repeated_and_unicode_values(self):
        import hashlib
        import struct
        from tos_access.knowledge import _stable_digest

        # Public revisions are shared with Worker cursors and inspector cards.
        # This deliberately simple encoder specifies the existing byte protocol,
        # independently of the production encoder's streaming or reuse strategy.
        def wire(value):
            if value is None:
                return b'n;'
            if isinstance(value, bool):
                return b'b1;' if value else b'b0;'
            if isinstance(value, (int, float)):
                return b'd' + struct.pack('>d', float(value) or 0.0).hex().encode() + b';'
            if isinstance(value, str):
                payload = value.encode('utf-8')
                return b's' + str(len(payload)).encode() + b':' + payload
            if isinstance(value, list):
                return b'a' + str(len(value)).encode() + b'[' + b''.join(map(wire, value)) + b']'
            keys = sorted(value, key=str)
            return b'o' + str(len(keys)).encode() + b'{' + b''.join(wire(str(k)) + wire(value[k]) for k in keys) + b'}'

        scalars = [None, True, False, 0, -0.0, 1, 1.0, 2**53 + 1, -2.5, '',
                   'Ницше', '𐀀', '\\"\n\x00', 'é', 'e\u0301', 'x' * 255, 'x' * 256, 'x' * 257]
        cases = scalars + [scalars, {'nested': scalars, 'source_ref': 'ToS/public.json'}]
        # Object keys take the same wire path as string values, including the
        # uncached long-key path. UTF-8 byte lengths are not character counts.
        cases.extend({key: {'nested': key}} for key in
                     ('', '𐀀' * 256, 'é' * 257, 'x' * 4096, '\\"\n\x00'))
        cases.append({10: 'numeric-key', '2': 'string-key', None: 'null-key'})
        # More distinct small strings than a bounded encoder cache can retain.
        cases.append([{'id': str(i), 'label': 'Ницше', 'enabled': True} for i in range(5000)])
        for value in cases:
            self.assertEqual(_stable_digest(value), hashlib.sha256(wire(value)).hexdigest())
        self.assertEqual(_stable_digest({'b': 2, 'a': 1}), _stable_digest({'a': 1, 'b': 2}))
        self.assertEqual(_stable_digest(0), _stable_digest(-0.0))
        self.assertNotEqual(_stable_digest(True), _stable_digest(1))
        for value in (float('nan'), float('inf'), -float('inf')):
            with self.assertRaises(ValueError):
                _stable_digest(value)
        with self.assertRaises(TypeError):
            _stable_digest({'unsupported': {1, 2}})

    def test_search_index_preserves_substring_filters_ranking_and_snapshot(self):
        from unittest.mock import patch
        from tos_access.knowledge import KnowledgeSearchIndex, search_knowledge_graph
        graph = build_knowledge_graph(*self.fixture())
        graph['nodes'][0]['attributes']['probe'] = {'nested':'Ницше \\"quote', 'number':12345}
        index = KnowledgeSearchIndex(graph)
        for query in ('', 'a', 'Ницше', 'ицш', '12345', '\\"', 'not present'):
            for options in ({}, {'offset':1,'limit':1}, {'sources':['philosophy']}, {'kind_ids':['concept']}):
                expected = search_knowledge_graph(graph,query,**options)
                with patch('tos_access.knowledge._searchable',side_effect=AssertionError('reserialized snapshot')):
                    actual = search_knowledge_graph(graph,query,search_index=index,**options)
                self.assertEqual(actual,expected)
        with self.assertRaisesRegex(ValueError,'snapshot'):
            search_knowledge_graph(copy.deepcopy(graph),'a',search_index=index)

    def test_inspection_index_preserves_aliases_edges_and_avoids_global_scans(self):
        from tos_access.knowledge import (
            KnowledgeGraphIndex, inspect_knowledge_node, inspect_knowledge_relation,
        )
        graph = build_knowledge_graph(*self.fixture())
        left, right = graph['nodes'][:2]
        left['entity_id'] = right['entity_id'] = 'shared-entity'
        left['native_id'] = right['native_id'] = 'ambiguous-native'
        # Exact identity must still win over an entity/native alias elsewhere.
        graph['nodes'].append({**copy.deepcopy(left), 'id': 'other-carrier',
                               'entity_id': left['id'], 'native_id': left['id']})
        edge = graph['relations'][0]
        graph['relations'].extend([
            {**copy.deepcopy(edge), 'id': 'self-loop', 'native_id': 'shared-edge',
             'from_id': left['id'], 'to_id': left['id']},
            {**copy.deepcopy(edge), 'id': 'connecting', 'native_id': 'shared-edge',
             'from_id': left['id'], 'to_id': right['id']},
            # The read adapter must not silently deduplicate input records.
            {**copy.deepcopy(edge), 'id': 'connecting', 'native_id': 'shared-edge',
             'from_id': left['id'], 'to_id': right['id']},
        ])
        node_cases = [(identifier, limit) for identifier in
                      (left['id'], right['id'], 'shared-entity', 'ambiguous-native')
                      for limit in (0, 1, 1000)]
        node_expected = [inspect_knowledge_node(graph, identifier, limit)
                         for identifier, limit in node_cases]
        relation_cases = [edge['id'], 'self-loop', 'connecting', 'shared-edge']
        relation_expected = [inspect_knowledge_relation(graph, identifier)
                             for identifier in relation_cases]
        index = KnowledgeGraphIndex(graph)

        class NoScan(list):
            def __iter__(self):
                raise AssertionError('inspection rescanned the complete graph')

        graph['nodes'] = NoScan(graph['nodes'])
        graph['relations'] = NoScan(graph['relations'])
        for (identifier, limit), expected in zip(node_cases, node_expected):
            self.assertEqual(inspect_knowledge_node(graph, identifier, limit, graph_index=index), expected)
        for identifier, expected in zip(relation_cases, relation_expected):
            self.assertEqual(inspect_knowledge_relation(graph, identifier, graph_index=index), expected)
        # For one resolved node the degree is already known. Even its incident
        # list need not be scanned to count or return a bounded prefix.
        index.adjacency[left['id']] = NoScan(index.adjacency[left['id']])
        for limit in (0, 1):
            expected = node_expected[node_cases.index((left['id'], limit))]
            self.assertEqual(inspect_knowledge_node(graph, left['id'], limit, graph_index=index), expected)
        for inspect in (inspect_knowledge_node, inspect_knowledge_relation):
            with self.assertRaises(KeyError):
                inspect(graph, 'missing', graph_index=index)
            with self.assertRaises(ValueError):
                inspect(graph, ' ', graph_index=index)
            with self.assertRaisesRegex(ValueError, 'snapshot'):
                inspect(dict(graph), left['id'], graph_index=index)
        with self.assertRaises(ValueError):
            inspect_knowledge_node(graph, left['id'], -1, graph_index=index)
        # Mutating a returned match list must not corrupt the index's buckets.
        packet = inspect_knowledge_node(graph, left['id'], 0, graph_index=index)
        packet['matches'].clear()
        self.assertEqual(inspect_knowledge_node(graph, left['id'], 0, graph_index=index), node_expected[0])

    def test_cursor_conserves_result_and_rejects_different_query_or_snapshot(self):
        from tos_access.lens_pagination import KnowledgeRevisionConflict
        graph = build_knowledge_graph(*self.fixture())
        for size in (1, 2, 7):
            spec = {'schema_version': 'tos_lens_spec_v1', 'lens_id': 'pages', 'detail': 'compact', 'explain': True}
            full = execute_knowledge_lens(graph, spec)
            seen_nodes, seen_relations = [], []
            spec['pagination'] = {'nodes': size, 'relations': size, 'cursor': None}
            for _ in range(len(full['nodes']) + len(full['relations']) + 1):
                page = execute_knowledge_lens(graph, spec)
                Draft202012Validator(self.schemas['lens-result.v1.schema.json'], registry=self.registry).validate(page)
                self.assertEqual(page['fingerprint'], full['fingerprint'])
                ids = {node['id'] for node in page['nodes']}
                self.assertTrue(all(r['from_id'] in ids and r['to_id'] in ids for r in page['relations']))
                seen_nodes.extend(page['page']['primary_node_ids'])
                seen_relations.extend(r['id'] for r in page['relations'])
                if not page['page']['has_more']:
                    break
                spec['pagination']['cursor'] = page['page']['next_cursor']
                for wrong in ({**spec, 'lens_id': 'different'},):
                    with self.assertRaises(KnowledgeRevisionConflict):
                        execute_knowledge_lens(graph, wrong)
                with self.assertRaises(KnowledgeRevisionConflict):
                    execute_knowledge_lens({**graph, 'source_revision': 'c' * 64}, spec)
            self.assertEqual(seen_nodes, [n['id'] for n in full['nodes']])
            self.assertEqual(seen_relations, [r['id'] for r in full['relations']])
            self.assertEqual(len(seen_nodes), len(set(seen_nodes)))
            self.assertEqual(len(seen_relations), len(set(seen_relations)))

    def test_path_conditions_join_at_selector_and_preserve_inclusion_causes(self):
        corpus, philosophy = self.fixture()
        graph = build_knowledge_graph(corpus, philosophy)
        # Generate bounded directed chains, including a cycle and disconnected
        # distractors. Every accepted witness must actually follow each step.
        template_node, template_relation = graph['nodes'][0], graph['relations'][0]
        for length in range(1, 5):
            nodes = [{**copy.deepcopy(template_node), 'id': f'philosophy:n{i}', 'source_graph': 'philosophy'} for i in range(6)]
            relations = [{**copy.deepcopy(template_relation), 'id': f'philosophy:e{i}', 'source_graph': 'philosophy',
                          'from_id': nodes[i]['id'], 'to_id': nodes[(i + 1) % 5]['id']} for i in range(5)]
            graph.update(nodes=nodes, relations=relations)
            spec = {'schema_version': 'tos_lens_spec_v1', 'lens_id': 'paths', 'sources': ['philosophy'], 'explain': True,
                    'seed': {'node_ids': [nodes[0]['id']]}, 'relation_query': {'enabled': False},
                    'path_query': [{'path_id': 'reachable', 'steps': [{} for _ in range(length)]}]}
            result = execute_knowledge_lens(graph, spec)
            self.assertEqual([n['id'] for n in result['nodes']], [nodes[0]['id']])
            witness = result['inclusion']['nodes'][nodes[0]['id']]['path_witnesses'][0]
            self.assertEqual(witness['node_ids'], [n['id'] for n in nodes[:length + 1]])
            self.assertEqual(witness['relation_ids'], [r['id'] for r in relations[:length]])
            Draft202012Validator(self.schemas['lens-result.v1.schema.json'], registry=self.registry).validate(result)
            spec['path_query'][0]['quantifier'] = 'not_exists'
            self.assertEqual(execute_knowledge_lens(graph, spec)['nodes'], [])
            spec['seed']['node_ids'] = [nodes[5]['id']]
            absent = execute_knowledge_lens(graph, spec)
            self.assertTrue(absent['inclusion']['nodes'][nodes[5]['id']]['path_witnesses'][0]['absence_in_scope'])
        # Paths cannot use an excluded endpoint even if their edge is in scope.
        nodes[1]['source_graph'] = 'repository'
        spec['seed']['node_ids'] = [nodes[0]['id']]
        self.assertEqual(len(execute_knowledge_lens(graph, spec)['nodes']), 1)
        focus = focus_knowledge_node(graph, nodes[0]['id'], sources=['philosophy'])
        explained = execute_knowledge_lens(graph, {**focus['lens'], 'explain': True})
        self.assertEqual(explained['inclusion']['nodes'][nodes[0]['id']]['kind'], 'focus')

    def test_path_contract_rejects_unbounded_and_ambiguous_conditions(self):
        base = {'schema_version': 'tos_lens_spec_v1', 'lens_id': 'invalid'}
        typed = normalize_lens_spec({**base, 'path_query': [{'path_id': 'typed', 'steps': [
            {'node_query': {'filters': [{'field': 'semantics.type_ancestors', 'op': 'contains', 'value': 'tos.entity.thing'}]}}
        ]}]})
        Draft202012Validator(self.schemas['lens-spec.v1.schema.json']).validate(typed)
        for paths in ([{'path_id': 'p', 'steps': []}], [{'path_id': 'p', 'steps': [{}] * 5}],
                      [{'path_id': 'p', 'steps': [{}]}] * 2,
                      [{'path_id': 'p', 'steps': [{'direction': 'sideways'}]}]):
            with self.assertRaises(ValueError):
                normalize_lens_spec({**base, 'path_query': paths})

    def test_inspection_and_search_identify_their_snapshot(self):
        from tos_access.knowledge import search_knowledge_graph, inspect_knowledge_node, inspect_knowledge_relation
        corpus, philosophy = self.fixture()
        graph = build_knowledge_graph(corpus, philosophy)
        for revision in ('a' * 64, 'b' * 64):
            graph['source_revision'] = revision
            packets = [search_knowledge_graph(graph), inspect_knowledge_node(graph, graph['nodes'][0]['id']),
                       inspect_knowledge_relation(graph, graph['relations'][0]['id'])]
            self.assertTrue(all(packet['source_revision'] == revision for packet in packets))

    def test_predicate_translation_depends_on_meaning_not_number_of_carriers(self):
        from tos_access.knowledge import _relation_display
        entry = {'labels': {'default': 'authored by', 'ru': 'написано автором'},
                 'source_mappings': [{'source_graph': source, 'source_predicate_id': 'authored_by'}
                                     for source in ('source-navigation', 'source-claims')]}
        display = _relation_display({}, 'authored_by', None, None, ['ToS/example'], entry)
        self.assertEqual(display['label']['ru'], 'написано автором')
        self.assertEqual(display['provenance']['label'], 'registry-label')
        entry['source_mappings'].append({'source_predicate_id': 'edited_by'})
        family = _relation_display({}, 'edited_by', None, None, ['ToS/example'], entry)
        self.assertIsNone(family['label']['ru'])
        authored = _relation_display({'display': {'label': {'default': 'exact', 'ru': 'авторская подпись'}}},
                                    'authored_by', None, None, ['ToS/example'], entry)
        self.assertEqual(authored['label']['ru'], 'авторская подпись')

    def test_compact_carrier_preserves_selection_and_full_inspection_source(self):
        corpus, philosophy = self.fixture()
        graph = build_knowledge_graph(corpus, philosophy)
        spec = {'schema_version': 'tos_lens_spec_v1', 'lens_id': 'compact-contract'}
        full = execute_knowledge_lens(graph, spec)
        compact = execute_knowledge_lens(graph, {**spec, 'detail': 'compact'})
        self.assertEqual([n['id'] for n in full['nodes']], [n['id'] for n in compact['nodes']])
        self.assertTrue(all(n['attributes'] == {} and 'source_record' not in n for n in compact['nodes']))
        self.assertTrue(all('source_record' in n for n in graph['nodes']))
        self.assertEqual(full['counts'], compact['counts'])
        Draft202012Validator(self.schemas['lens-result.v1.schema.json'], registry=self.registry).validate(compact)

    def test_default_normalized_spec_obeys_published_schema(self):
        spec = normalize_lens_spec({"schema_version": "tos_lens_spec_v1", "lens_id": "default-contract"})
        Draft202012Validator(self.schemas["lens-spec.v1.schema.json"]).validate(spec)

    def test_completed_normalization_steps_are_reused_and_dependencies_invalidate(self):
        from tos_access.normalization_cache import NormalizationCache
        corpus, philosophy = self.fixture()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'steps.sqlite'
            with NormalizationCache(path, 'processor-v1') as initial:
                first = build_knowledge_graph(corpus, philosophy)
            self.assertGreater(initial.misses, 0)
            with NormalizationCache(path, 'processor-v1') as repeat:
                self.assertEqual(build_knowledge_graph(corpus, philosophy), first)
            self.assertEqual(repeat.misses, 0)
            self.assertEqual(repeat.hits, initial.misses)
            philosophy['nodes'][0]['label'] = 'Changed title'
            with NormalizationCache(path, 'processor-v1') as changed:
                fresh = build_knowledge_graph(corpus, philosophy)
            self.assertGreater(changed.hits, 0)
            self.assertGreater(changed.misses, 0)
            self.assertEqual(fresh, build_knowledge_graph(corpus, philosophy))
            with NormalizationCache(path, 'processor-v2') as upgraded:
                build_knowledge_graph(corpus, philosophy)
            self.assertEqual(upgraded.hits, 0)

    def test_lossless_record_and_reference_is_not_identity(self):
        raw = {"node_id": "literal:date", "node_kind": "literal", "source_ref": "ToS/fixture.json",
               "custom": "outer", "multilingual": {"fr": "exemple"},
               "properties": {"claim_ref": "tos.claim.example", "custom": "inner", "value": "1883"}}
        node = _normalize_node(raw, "source-claims")
        self.assertNotEqual(node["entity_id"], "tos.claim.example")
        self.assertEqual(node["source_record"]["payload"], raw)
        self.assertEqual(node["source_record"]["field_map"]["attributes.custom"], "/properties/custom")
        self.assertEqual(len(node["source_record"]["digest"]), 64)
        raw["properties"]["custom"] = "modified after projection"
        self.assertEqual(node["source_record"]["payload"]["properties"]["custom"], "inner")

    def test_temporal_parsing_never_accepts_invalid_dates_or_loses_bounds(self):
        self.assertNotEqual(_normalized_time("2026-99-99")["normalization_status"], "source-literal-parsed")
        explicit = {'calendar': 'proleptic-gregorian', 'year_numbering': 'astronomical'}
        value = _normalized_time({"start": "1900", "end": "1800", **explicit})
        self.assertEqual(value["interval"], {"start": "1900", "end": "1800"})
        self.assertIn("reversed-interval", value["issues"])
        ancient = _normalized_time({'start': '-0500', 'end': '-0400', **explicit})
        self.assertEqual(ancient['issues'], [])
        self.assertLess(ancient['sort_start'], ancient['sort_end'])
        self.assertEqual(_normalized_time('2000-02')['sort_end'], 20000229)
        self.assertNotIn('sort_start', _normalized_time({'year': 1883, 'month': 'unknown'}))
        self.assertNotIn('sort_start', _normalized_time({'year': 1883, 'calendar': 'julian'}))
        self.assertEqual(_normalized_time({'temporal': {'start': '1883', 'end': '1885', **explicit}})['sort_end'], 18851231)

    def test_structured_time_does_not_invent_calendar_numbering_or_precision(self):
        explicit = {'calendar': 'proleptic-gregorian', 'year_numbering': 'astronomical'}
        for value in (
            {'year': 1883}, {'value': '1883', 'calendar': 'gregorian'},
            {'start': '1883', 'end': '1885'},
            {'value': '1883', **explicit, 'certainty': 'approximate'},
            {'value': '1883', **explicit, 'precision': 'unknown'},
            {'year': 1883.5, **explicit}, {'year': True, **explicit},
            {'year': 1883, 'month': 2, 'day': 30, **explicit},
            {'value': '-0500', **explicit, 'year_numbering': 'historical'},
            {'start': '1883', **explicit},
        ):
            with self.subTest(value=value):
                normalized = _normalized_time(value)
                self.assertNotIn('sort_start', normalized)
                self.assertEqual(normalized['raw'], value)
                self.assertTrue(normalized['issues'])
        self.assertEqual(_normalized_time({'value': '-0500', **explicit})['sort_start'], -4999899)

    def test_nested_time_context_preserves_calendar_and_reports_conflicts(self):
        explicit = {'calendar': 'gregorian', 'year_numbering': 'astronomical'}
        inner = {'start': '1883', 'end': '1885', **explicit}
        self.assertEqual(_normalized_time({'interval': inner})['sort_end'], 18851231)
        self.assertEqual(_normalized_time({'temporal': {'interval': inner}})['sort_end'], 18851231)
        for value in (
            {'interval': {**inner, 'calendar': 'julian'}},
            {'interval': inner, 'calendar': 'julian'},
            {'temporal': {'value': '1883', **explicit}, 'certainty': 'uncertain'},
            {'start': {'value': '1883', **explicit, 'calendar': 'julian'}, 'end': '1885', **explicit},
            {'interval': inner, 'year_numbering': 'historical'},
        ):
            with self.subTest(value=value):
                normalized = _normalized_time(value)
                self.assertNotIn('sort_start', normalized)
                self.assertEqual(normalized['raw'], value)

    def test_relative_and_unknown_time_do_not_borrow_absolute_dates(self):
        for value in (
            {'kind': 'relative-order', 'relative': {'relation': 'before', 'anchor_ref': 'tos.historical-event.fixture'}},
            {'kind': 'unknown-date', 'certainty': 'unknown'},
        ):
            normalized = _normalized_time(value)
            self.assertEqual(normalized['raw'], value)
            self.assertNotIn('sort_start', normalized)
            self.assertEqual(normalized['normalization_status'], 'structured-source')

    def test_registry_supersession_resolves_and_is_acyclic(self):
        entities = copy.deepcopy(self.entity_type_registry)
        entities["types"][1]["supersedes_type_id"] = "tos.entity.nonexistent"
        self.assertFalse(validate_semantic_registries(entities, self.relation_type_registry)["valid"])
        entities["types"][1]["supersedes_type_id"] = entities["types"][1]["type_id"]
        self.assertFalse(validate_semantic_registries(entities, self.relation_type_registry)["valid"])

    def test_abstract_nodes_and_unbound_reviewed_identity_are_rejected(self):
        corpus, philosophy = self.fixture()
        graph = build_knowledge_graph(corpus, philosophy, {}, self.entity_type_registry, self.relation_type_registry)
        abstract = copy.deepcopy(graph)
        abstract["nodes"][0]["type_id"] = "tos.entity.thing"
        self.assertFalse(validate_knowledge_semantics(abstract, self.entity_type_registry, self.relation_type_registry)["valid"])
        invalid = copy.deepcopy(graph)
        rel = invalid["relations"][0]
        rel["relation_type_id"] = "tos.relation.same-as"
        rel["source_refs"] = ["arbitrary-ref"]
        rel["epistemic"]["review_posture"] = "accepted"
        self.assertFalse(validate_knowledge_semantics(invalid, self.entity_type_registry, self.relation_type_registry)["valid"])

    def test_non_zarathustra_annotation_keeps_source_claim_evidence_route(self):
        sys.path.insert(0, str(self.repo_root / "scripts"))
        from tos_corpus_index_common import project_text_packet
        ref = "ToS/research-packets/foundation-laboratory-2026-07/semantic-annotation-v2-abc/variant-b-competing-sign-proposals.json"
        packet = json.loads((self.repo_root / ref).read_text())
        nodes, edges = project_text_packet(packet, ref)
        work = packet['source_scope']['work_ref']
        nodes.append({'node_id': work, 'node_kind': 'work', 'label': 'Public synthetic work', 'source_ref': ref})
        corpus = {'source_navigation': {'nodes': nodes, 'edges': edges}}
        graph = build_knowledge_graph(corpus, {}, {}, self.entity_type_registry, self.relation_type_registry)
        types = {n['type_id'] for n in graph['nodes']}
        self.assertTrue({'tos.entity.work', 'tos.entity.text-layer', 'tos.entity.anchor', 'tos.entity.occurrence',
                         'tos.entity.sign', 'tos.entity.annotation-claim', 'tos.entity.annotation-evidence'}.issubset(types))
        claims = [n for n in graph['nodes'] if n['type_id'] == 'tos.entity.annotation-claim']
        self.assertEqual(len(claims), len(packet['claims']))
        self.assertTrue(any(n['attributes']['competing_claim_refs'] for n in claims))
        self.assertFalse(any(n['epistemic']['review_posture'] == 'accepted' for n in claims))
        self.assertTrue(all(n['entity_id'] != work for n in claims))
        result = focus_knowledge_node(graph, work, depth=5, node_limit=1000, relation_limit=2000, profile='all')
        self.assertTrue(any(n['type_id'] == 'tos.entity.annotation-evidence' for n in result['nodes']))
        Draft202012Validator(self.schemas['knowledge-graph.v1.schema.json'], registry=self.registry).validate(graph)

    @classmethod
    def setUpClass(cls) -> None:
        cls.schemas = {
            name: json.loads((ACCESS_ROOT / "contracts" / name).read_text(encoding="utf-8"))
            for name in (
                "knowledge-graph.v1.schema.json",
                "lens-spec.v1.schema.json",
                "lens-result.v1.schema.json",
            )
        }
        cls.registry = Registry().with_resources(
            (schema["$id"], Resource.from_contents(schema))
            for schema in cls.schemas.values()
        )
        repo_root = ACCESS_ROOT.parent
        cls.repo_root = repo_root
        cls.entity_type_registry = json.loads(
            (
                repo_root
                / "ToS/doctrine/semantic-interchange/entity-types.v1.json"
            ).read_text(encoding="utf-8")
        )
        cls.relation_type_registry = json.loads(
            (
                repo_root
                / "ToS/doctrine/semantic-interchange/relation-types.v1.json"
            ).read_text(encoding="utf-8")
        )

    def fixture(self) -> tuple[dict[str, object], dict[str, object]]:
        corpus: dict[str, object] = {
            "nodes": [
                {
                    "node_id": "canon-a",
                    "label": "Canonical A",
                    "node_type": "concept",
                    "source_path": "ToS/canon/a.json",
                    "authority_layer": "canon",
                    "route_hint": "source-owned route",
                }
            ],
            "relation_packs": [
                {"pack_id": "canon/demo", "path": "ToS/canon/demo/edges.csv", "owner_branch": "ToS/canon"}
            ],
            "relation_edges": [
                {
                    "edge_id": "canon-edge",
                    "pack_id": "canon/demo",
                    "owner_branch": "ToS/canon",
                    "from_id": "canon-a",
                    "to_id": "canon-b",
                    "predicate_id": "supports",
                    "status": "canon",
                }
            ],
            "source_navigation": {"nodes": [], "edges": []},
            "branches": [
                {
                    "id": "canon",
                    "path": "ToS/canon",
                    "owner_surface": "ToS/canon/AGENTS.md",
                    "role": "canonical authored material",
                    "authority_layer": "canon",
                }
            ],
            "manifests": [],
            "resources": [
                {
                    "path": "ToS/canon/a.json",
                    "owner_branch": "ToS/canon",
                    "resource_kind": "json",
                    "sha256": "a" * 64,
                    "size_bytes": 42,
                    "authority_layer": "canon",
                }
            ],
            "graph_views": [
                {"view_id": "corpus-topology", "title": "Corpus", "layout_hint": "layered"},
                {"view_id": "route-graph", "title": "Canon", "layout_hint": "directed-route-graph"},
            ],
        }
        philosophy: dict[str, object] = {
            "nodes": [
                {
                    "node_id": "a",
                    "label": "Альфа",
                    "node_type": "candidate-node",
                    "view_ids": ["chronology"],
                    "graph_layers": ["conceptual-relation"],
                    "source_ref": "ToS/philosophy/a.jsonl",
                    "properties": {
                        "original_node_type": "concept",
                        "period": "fixture",
                        "private_marker": "preserved",
                        "variant_labels": [{"value": "Alpha", "language": "en"}],
                    },
                },
                {
                    "node_id": "b",
                    "label": "Бета",
                    "node_type": "candidate-node",
                    "view_ids": ["chronology"],
                    "graph_layers": ["conceptual-relation"],
                    "source_ref": "ToS/philosophy/b.jsonl",
                    "properties": {"original_node_type": "concept"},
                },
                {
                    "node_id": "c",
                    "label": "Гамма",
                    "node_type": "candidate-node",
                    "view_ids": ["chronology"],
                    "graph_layers": ["conceptual-relation"],
                    "source_ref": "ToS/philosophy/c.jsonl",
                    "properties": {"original_node_type": "work"},
                },
            ],
            "edges": [
                {
                    "edge_id": "e",
                    "from_id": "a",
                    "to_id": "b",
                    "predicate_id": "relates",
                    "view_ids": ["chronology"],
                    "graph_layers": ["conceptual-relation"],
                    "source_ref": "ToS/philosophy/e.jsonl",
                    "properties": {},
                },
                {
                    "edge_id": "f",
                    "from_id": "b",
                    "to_id": "c",
                    "predicate_id": "extends",
                    "view_ids": ["chronology"],
                    "graph_layers": ["conceptual-relation"],
                    "source_ref": "ToS/philosophy/f.jsonl",
                    "properties": {},
                },
            ],
            "views": [
                {"view_id": "chronology", "title": "Chronology", "review_intent": "fixture", "layout_hint": "timeline"}
            ],
        }
        return corpus, philosophy

    def test_normalized_graph_and_result_conform_to_public_schemas(self) -> None:
        corpus, philosophy = self.fixture()
        graph = build_knowledge_graph(corpus, philosophy)
        Draft202012Validator(self.schemas["knowledge-graph.v1.schema.json"], registry=self.registry).validate(graph)
        self.assertEqual(len(graph["source_revision"]), 64)
        self.assertEqual(graph["counts"]["display_coverage"]["node_titles"], graph["counts"]["nodes"])
        self.assertEqual(graph["counts"]["display_coverage"]["relation_statements"], graph["counts"]["relations"])
        node_ids = {item["id"] for item in graph["nodes"]}
        relation_ids = {item["id"] for item in graph["relations"]}
        self.assertEqual(len(node_ids), len(graph["nodes"]))
        self.assertEqual(len(relation_ids), len(graph["relations"]))
        self.assertTrue(all(len(item["content_revision"]) == 64 for item in graph["nodes"]))
        self.assertTrue(all(len(item["content_revision"]) == 64 for item in graph["relations"]))
        self.assertTrue(all(item["content_revision"] == _content_revision(item) for item in graph["nodes"]))
        self.assertTrue(all(item["content_revision"] == _content_revision(item) for item in graph["relations"]))
        self.assertTrue(
            all(item["from_id"] in node_ids and item["to_id"] in node_ids for item in graph["relations"])
        )
        philosophy_a = next(item for item in graph["nodes"] if item["id"] == "philosophy:a")
        self.assertEqual(philosophy_a["display"]["title"]["en"], "Alpha")
        resource = next(item for item in graph["nodes"] if item["native_id"] == "ToS/canon/a.json")
        self.assertEqual(resource["display"]["title"]["default"], "ToS/canon/a.json")
        self.assertEqual(resource["attributes"]["sha256"], "a" * 64)
        canon_relation = next(item for item in graph["relations"] if item["native_id"] == "canon-edge")
        self.assertEqual(canon_relation["source_refs"], ["ToS/canon/demo/edges.csv"])

        spec = {
            "schema_version": "tos_lens_spec_v1",
            "lens_id": "schema-smoke",
            "sources": ["philosophy"],
            "node_query": {"enabled": False},
            "relation_query": {"filters": [{"field": "predicate_id", "op": "eq", "value": "relates"}]},
            "composition": {"endpoint_policy": "independent", "group_by": ["kind_id"]},
            "limits": {"nodes": 10, "relations": 10, "groups": 10},
        }
        result = execute_knowledge_lens(graph, spec)
        Draft202012Validator(self.schemas["lens-result.v1.schema.json"], registry=self.registry).validate(result)
        self.assertEqual(result["source_revision"], graph["source_revision"])
        changed_revision_result = execute_knowledge_lens({**graph, "source_revision": "b" * 64}, spec)
        self.assertNotEqual(result["fingerprint"], changed_revision_result["fingerprint"])
        self.assertEqual(result["counts"]["missing_node_summaries"], 0)
        self.assertEqual(result["counts"]["missing_relation_explanations"], 0)
        self.assertTrue(all(item["source_refs"] for item in [*result["nodes"], *result["relations"]]))

    def test_relation_view_membership_participates_in_endpoint_content_revision(self) -> None:
        corpus, philosophy = self.fixture()
        philosophy["nodes"][1]["view_ids"] = []
        philosophy["edges"][1]["view_ids"] = []
        first = build_knowledge_graph(corpus, philosophy)
        first_node = next(item for item in first["nodes"] if item["id"] == "philosophy:b")
        philosophy["edges"][0]["view_ids"] = ["direct-only"]
        second = build_knowledge_graph(corpus, philosophy)
        second_node = next(item for item in second["nodes"] if item["id"] == "philosophy:b")
        self.assertEqual(first_node["view_ids"], ["chronology"])
        self.assertEqual(second_node["view_ids"], ["direct-only"])
        self.assertNotEqual(first_node["content_revision"], second_node["content_revision"])

    def test_catalog_turns_stored_views_into_lens_specs(self) -> None:
        corpus, philosophy = self.fixture()
        graph = build_knowledge_graph(corpus, philosophy)
        catalog = knowledge_catalog(graph, corpus, philosophy)
        self.assertEqual(catalog["source_revision"], graph["source_revision"])
        lens_ids = {item["lens_id"] for item in catalog["lenses"]}
        self.assertEqual(lens_ids, {"chronology", "corpus-topology", "route-graph"})
        self.assertEqual(catalog["capabilities"]["operator_value_contracts"]["gt"], "number")
        entity_routes = {
            item["route_id"]: item for item in catalog["capabilities"]["entity_routes"]
        }
        self.assertEqual(entity_routes["work"]["availability"], "available")
        self.assertEqual(entity_routes["word"]["availability"], "not_projected")
        self.assertEqual(entity_routes["word"]["available_confirming_predicate_ids"], [])
        self.assertEqual(entity_routes["author"]["confirming_predicate_ids"], ["authored_by"])
        size_field = next(
            item
            for item in catalog["capabilities"]["node_attribute_fields"]
            if item["field"] == "attributes.size_bytes"
        )
        self.assertEqual(size_field["value_types"], {"integer": 1})
        self.assertEqual(size_field["examples"], [42])
        source_facet = catalog["capabilities"]["facets"]["nodes"]["source_graph"]
        self.assertEqual(
            {item["value"] for item in source_facet},
            {"canon", "philosophy", "repository"},
        )
        for lens in catalog["lenses"]:
            Draft202012Validator(self.schemas["lens-spec.v1.schema.json"]).validate(lens)
        route = next(item for item in catalog["lenses"] if item["lens_id"] == "route-graph")
        result = execute_knowledge_lens(graph, route)
        self.assertEqual([item["native_id"] for item in result["relations"]], ["canon-edge"])

    def test_semantic_registries_are_hierarchical_acyclic_and_machine_readable(self) -> None:
        for schema_name, instance in (
            ("semantic-entity-type-registry.schema.json", self.entity_type_registry),
            ("semantic-relation-type-registry.schema.json", self.relation_type_registry),
        ):
            schema = json.loads(
                (
                    self.repo_root / "ToS/contracts" / schema_name
                ).read_text(encoding="utf-8")
            )
            Draft202012Validator(schema).validate(instance)

        report = validate_semantic_registries(
            self.entity_type_registry,
            self.relation_type_registry,
        )

        self.assertTrue(report["valid"], report["violations"])
        self.assertEqual(report["entity_type_count"], len(self.entity_type_registry["types"]))
        self.assertEqual(report["relation_type_count"], len(self.relation_type_registry["relations"]))
        entity_types = {item["type_id"]: item for item in self.entity_type_registry["types"]}
        relation_types = {
            item["relation_type_id"]: item
            for item in self.relation_type_registry["relations"]
        }
        self.assertIn("tos.entity.work", entity_types)
        self.assertIn("tos.entity.temporal-assertion", entity_types)
        self.assertIn("tos.entity.place", entity_types)
        self.assertNotIn("tos.entity.author", entity_types)
        self.assertEqual(
            relation_types["tos.relation.authored-by"]["domain_type_ids"],
            ["tos.entity.work", "tos.entity.document"],
        )
        self.assertEqual(
            relation_types["tos.relation.authored-by"]["range_type_ids"],
            ["tos.entity.agent"],
        )
        self.assertTrue(relation_types["tos.relation.same-as"]["evidence_required"])
        self.assertEqual(
            relation_types["tos.relation.same-as"]["review_requirement"],
            "accepted",
        )

    def test_claim_graph_time_space_and_identity_projections_are_first_class(self) -> None:
        corpus, philosophy = self.fixture()
        corpus["source_navigation"] = {
            "nodes": [
                {
                    "node_id": "tos.work.fixture.alpha",
                    "node_kind": "work",
                    "label": "Fixture work",
                    "source_ref": "ToS/source-witnesses/works/fixture/work.json",
                    "identity_status": "verified",
                    "properties": {
                        "source_record": {
                            "record_id": "tos.work.fixture.alpha",
                            "record_type": "work",
                            "preferred_label": "Fixture work",
                            "record_version": 3,
                            "same_as_posture": "no_equivalence_claim",
                            "supersedes_ref": None,
                        }
                    },
                }
            ],
            "edges": [],
        }
        bibliographic = {
            "schema_version": "tos_source_witness_bibliographic_graph_v1",
            "nodes": [
                {
                    "node_id": "identity:tos.work.fixture.alpha",
                    "node_kind": "identity",
                    "source_ref": "ToS/source-witnesses/works/fixture/work.json",
                    "source_sha256": "1" * 64,
                    "properties": {
                        "identity_ref": "tos.work.fixture.alpha",
                        "identity_kind": "work",
                        "preferred_label": "Fixture work",
                        "identity_status": "verified",
                    },
                },
                {
                    "node_id": "identity:tos.place.fixture-city",
                    "node_kind": "identity",
                    "source_ref": "ToS/source-witnesses/places/fixture-city/place.json",
                    "source_sha256": "2" * 64,
                    "properties": {
                        "identity_ref": "tos.place.fixture-city",
                        "identity_kind": "place",
                        "preferred_label": "Fixture City",
                        "identity_status": "provisional",
                    },
                },
                {
                    "node_id": "claim:tos.claim.work.fixture.alpha.first-publication",
                    "node_kind": "claim",
                    "source_ref": "ToS/source-witnesses/chronology/fixture.jsonl",
                    "source_sha256": "3" * 64,
                    "properties": {
                        "claim_ref": "tos.claim.work.fixture.alpha.first-publication",
                        "predicate": "first_publication_chronology",
                        "review_status": "unreviewed",
                        "epistemic_status": "reported",
                        "claim_version": 1,
                    },
                },
                {
                    "node_id": "literal:fixture-time",
                    "node_kind": "literal",
                    "source_ref": "ToS/source-witnesses/chronology/fixture.jsonl",
                    "source_sha256": "3" * 64,
                    "properties": {
                        "claim_ref": "tos.claim.work.fixture.alpha.first-publication",
                        "value_type": "object",
                        "value": {
                            "chronology_kind": "first_publication",
                            "calendar": "gregorian",
                            "interval": {
                                "start": "1883",
                                "end": "1885",
                                "start_precision": "year",
                                "end_precision": "year",
                                "boundary_meaning": "earliest_stage_to_sequence_completion",
                            },
                            "stages": [],
                            "ordering_warning": "Fixture warning",
                        },
                    },
                },
            ],
            "edges": [
                {
                    "edge_id": "edge:fixture:subject",
                    "edge_kind": "has_subject",
                    "from_id": "claim:tos.claim.work.fixture.alpha.first-publication",
                    "to_id": "identity:tos.work.fixture.alpha",
                    "claim_ref": "tos.claim.work.fixture.alpha.first-publication",
                    "review_status": "unreviewed",
                    "source_claim_file_ref": "ToS/source-witnesses/chronology/fixture.jsonl",
                    "source_claim_line": 1,
                },
                {
                    "edge_id": "edge:fixture:object",
                    "edge_kind": "has_object",
                    "from_id": "claim:tos.claim.work.fixture.alpha.first-publication",
                    "to_id": "literal:fixture-time",
                    "claim_ref": "tos.claim.work.fixture.alpha.first-publication",
                    "review_status": "unreviewed",
                    "source_claim_file_ref": "ToS/source-witnesses/chronology/fixture.jsonl",
                    "source_claim_line": 1,
                },
                {
                    "edge_id": "edge:fixture:place",
                    "edge_kind": "has_normalized_place",
                    "from_id": "claim:tos.claim.work.fixture.alpha.first-publication",
                    "to_id": "identity:tos.place.fixture-city",
                    "claim_ref": "tos.claim.work.fixture.alpha.first-publication",
                    "review_status": "unreviewed",
                    "source_claim_file_ref": "ToS/source-witnesses/chronology/fixture.jsonl",
                    "source_claim_line": 1,
                    "properties": {"spatial_roles": ["publication_place"]},
                },
            ],
            "claim_traces": [
                {
                    "claim_ref": "tos.claim.work.fixture.alpha.first-publication",
                    "claim_node_id": "claim:tos.claim.work.fixture.alpha.first-publication",
                    "predicate": "first_publication_chronology",
                    "subject_node_id": "identity:tos.work.fixture.alpha",
                    "object_node_id": "literal:fixture-time",
                    "normalized_identity_node_ids": ["identity:tos.place.fixture-city"],
                    "review_status": "unreviewed",
                    "epistemic_status": "reported",
                    "evidence_node_ids": [],
                }
            ],
        }

        graph = build_knowledge_graph(
            corpus,
            philosophy,
            bibliographic,
            self.entity_type_registry,
            self.relation_type_registry,
        )
        report = validate_knowledge_semantics(
            graph,
            self.entity_type_registry,
            self.relation_type_registry,
        )

        self.assertTrue(report["valid"], report["violations"])
        work_representations = [
            item for item in graph["nodes"]
            if item["entity_id"] == "tos.work.fixture.alpha"
        ]
        self.assertEqual(len(work_representations), 2)
        self.assertEqual({item["type_id"] for item in work_representations}, {"tos.entity.work"})
        self.assertTrue(
            any(
                item["relation_type_id"] == "tos.relation.projects"
                and item["from_id"] == "source-claims:identity:tos.work.fixture.alpha"
                and item["to_id"] == "source-navigation:tos.work.fixture.alpha"
                for item in graph["relations"]
            )
        )
        temporal = next(item for item in graph["nodes"] if item["id"] == "source-claims:literal:fixture-time")
        self.assertEqual(temporal["type_id"], "tos.entity.temporal-assertion")
        self.assertEqual(temporal["semantics"]["time"]["interval"]["start"], "1883")
        place = next(item for item in graph["nodes"] if item["entity_id"] == "tos.place.fixture-city")
        self.assertEqual(place["type_id"], "tos.entity.place")
        place_relation = next(
            item for item in graph["relations"]
            if item["relation_type_id"] == "tos.relation.has-normalized-place"
        )
        self.assertEqual(place_relation["semantics"]["space"]["roles"], ["publication_place"])
        claim = next(item for item in graph["nodes"] if item["type_id"] == "tos.entity.claim")
        self.assertEqual(
            claim["semantics"]["claim"]["relation_type_id"],
            "tos.relation.first-publication-chronology",
        )
        self.assertEqual(claim["semantics"]["claim"]["subject_entity_id"], "tos.work.fixture.alpha")

        catalog = knowledge_catalog(
            graph,
            corpus,
            philosophy,
            self.entity_type_registry,
            self.relation_type_registry,
        )
        self.assertEqual(catalog["semantic_registries"]["entity_types"]["unmapped_instance_count"], 0)
        self.assertIn(
            "tos.entity.temporal-assertion",
            {
                item["type_id"]
                for item in catalog["semantic_registries"]["entity_types"]["entries"]
            },
        )

    def test_unknown_native_vocabulary_is_explicitly_unmapped(self) -> None:
        corpus, philosophy = self.fixture()
        philosophy["nodes"][0]["properties"]["original_node_type"] = "future-unknown-kind"
        philosophy["edges"][0]["predicate_id"] = "future_unknown_predicate"
        graph = build_knowledge_graph(
            corpus,
            philosophy,
            {},
            self.entity_type_registry,
            self.relation_type_registry,
        )

        node = next(item for item in graph["nodes"] if item["id"] == "philosophy:a")
        relation = next(item for item in graph["relations"] if item["id"] == "philosophy:e")
        self.assertEqual(node["type_id"], "tos.entity.unmapped")
        self.assertEqual(node["type_mapping"]["status"], "unmapped")
        self.assertEqual(node["type_mapping"]["source_kind_id"], "future-unknown-kind")
        self.assertEqual(relation["relation_type_id"], "tos.relation.unmapped")
        self.assertEqual(relation["predicate_mapping"]["status"], "unmapped")

    def test_current_repository_projection_has_complete_registry_coverage(self) -> None:
        corpus = json.loads(
            (self.repo_root / "ToS/derived-exports/tos_corpus_index.min.json").read_text(
                encoding="utf-8"
            )
        )
        philosophy = json.loads(
            (
                self.repo_root
                / "ToS/derived-exports/philosophy_graph_projection.min.json"
            ).read_text(encoding="utf-8")
        )
        bibliographic = json.loads(
            (
                self.repo_root
                / "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json"
            ).read_text(encoding="utf-8")
        )

        graph = build_knowledge_graph(
            corpus,
            philosophy,
            bibliographic,
            self.entity_type_registry,
            self.relation_type_registry,
        )
        mapping = graph["counts"]["semantic_mapping"]
        self.assertEqual(mapping["unmapped_nodes"], 0)
        self.assertEqual(mapping["unmapped_relations"], 0)
        self.assertGreater(mapping["cross_layer_relations"], 0)
        self.assertEqual(graph["counts"]["semantic_validation"]["violations"], [])

        nodes_by_id = {node['id']: node for node in graph['nodes']}
        # Every source-owned metadata form travels into the ordinary reader,
        # not only the original Jenseits example. Do not freeze corpus counts.
        for source_node in bibliographic['nodes']:
            forms = source_node['properties'].get('human_forms')
            if forms is None:
                continue
            projected = nodes_by_id['source-claims:' + source_node['node_id']]
            self.assertEqual(projected['attributes']['human_forms'], forms)
            self.assertEqual(projected['attributes']['human_forms_source_ref'],
                             source_node['properties']['human_forms_source_ref'])
        # The real source form set travels through the existing graph and full
        # inspection. It is not yet a scene/hover selection contract.
        from tos_access.knowledge import inspect_knowledge_node
        form_subject = 'tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese'
        source_identity = next(node for node in bibliographic['nodes']
                               if node['properties'].get('identity_ref') == form_subject)
        packet = inspect_knowledge_node(graph, form_subject, relation_limit=0)
        projected_identity = next(node for node in packet['matches'] if node['source_graph'] == 'source-claims')
        self.assertEqual(projected_identity['attributes']['human_forms'], source_identity['properties']['human_forms'])
        self.assertEqual(len(projected_identity['attributes']['human_forms']), 3)
        self.assertTrue(all(form['context'] and form['admission'] is None
                            for form in projected_identity['attributes']['human_forms']))
        from tos_access.knowledge import _lens_carrier
        selected_identity = _lens_carrier(projected_identity, 'compact', language='ru')
        self.assertEqual(selected_identity['attributes'], {})
        self.assertEqual(selected_identity['human_form_selection']['roles']['name']['packet'],
                         source_identity['properties']['human_forms'][1])
        self.assertEqual(selected_identity['human_form_selection']['roles']['hover']['packet'],
                         source_identity['properties']['human_forms'][2])
        Draft202012Validator({'$ref': self.schemas['knowledge-graph.v1.schema.json']['$id'] + '#/$defs/node'},
                            registry=self.registry).validate(selected_identity)
        claims = [node for node in graph['nodes'] if node['type_id'] == 'tos.entity.claim']
        from tos_access.knowledge import _ASSERTION_FIELDS, _lens_carrier
        contexts_by_claim = {}
        for claim in claims:
            target = nodes_by_id[claim['semantics']['claim']['object_node_id']]
            self.assertNotEqual(claim['entity_id'], target['entity_id'])
            context, = claim['semantics']['assertion_contexts']
            source = claim['source_record']['payload']['properties']['source_claim']
            for field in set(source) & set(_ASSERTION_FIELDS):
                self.assertEqual(context['fields'][field]['value'], source[field])
            self.assertEqual(context['source_record_digest'], claim['source_record']['digest'])
            self.assertEqual(context['conflicts'], [])
            self.assertEqual(_lens_carrier(claim, 'compact')['semantics']['assertion_contexts'], [context])
            contexts_by_claim[source['claim_id']] = context
        for relation in graph['relations']:
            claim_ref = relation['attributes'].get('claim_ref')
            if relation['source_graph'] != 'source-claims' or claim_ref not in contexts_by_claim:
                continue
            governed, = [context for context in _lens_carrier(relation, 'compact')['semantics']['assertion_contexts']
                         if context['binding_role'] == 'referenced-claim']
            self.assertEqual(governed, {**contexts_by_claim[claim_ref], 'binding_role': 'referenced-claim'})
        subject_edge = next(edge for edge in graph['relations'] if edge['relation_type_id'] == 'tos.relation.has-subject')
        graph['relations'].append({**subject_edge, 'id': subject_edge['id'] + ':duplicate-subject'})
        report = validate_knowledge_semantics(graph, self.entity_type_registry, self.relation_type_registry)
        self.assertFalse(report['valid'])
        self.assertTrue(any('exactly one' in issue or 'per_subject_max' in issue for issue in report['violations']))
        graph['relations'].pop()

        focused = focus_knowledge_node(
            graph,
            "tos.work.friedrich-nietzsche.also-sprach-zarathustra",
            sources=[
                "canon",
                "source-navigation",
                "source-claims",
                "semantic-interchange",
            ],
            depth=5,
            node_limit=1000,
            relation_limit=2000,
        )
        focused_type_ids = {node["type_id"] for node in focused["nodes"]}
        self.assertEqual(focused["focus"]["resolved_by"], "entity_id")
        self.assertEqual(focused["focus"]["source_graph"], "source-navigation")
        self.assertTrue(
            {
                "tos.entity.agent",
                "tos.entity.source",
                "tos.entity.temporal-assertion",
                "tos.entity.place",
                "tos.entity.concept",
            }.issubset(focused_type_ids),
            sorted(focused_type_ids),
        )
        focused_relation_types = {
            relation["relation_type_id"] for relation in focused["relations"]
        }
        self.assertTrue(
            {
                "tos.relation.authored-by",
                "tos.relation.grounded-in",
                "tos.relation.has-object",
                "tos.relation.has-normalized-place",
            }.issubset(focused_relation_types),
            sorted(focused_relation_types),
        )
        grounding = next(
            relation
            for relation in focused["relations"]
            if relation["relation_type_id"] == "tos.relation.grounded-in"
        )
        self.assertEqual(
            grounding["attributes"]["derivation"],
            "authored-source-record-source-ref",
        )
        self.assertIn(
            "ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json",
            grounding["source_refs"],
        )
        authored_node_relations = [
            relation
            for relation in focused["relations"]
            if relation["relation_type_id"] == "tos.relation.canon-node-relation"
        ]
        self.assertTrue(authored_node_relations)
        self.assertTrue(
            all(
                relation["attributes"]["derivation"]
                == "authored-node-contract-relation"
                for relation in authored_node_relations
            )
        )

    def test_focus_is_explicit_and_neighborhood_stops_at_requested_depth(self) -> None:
        corpus, philosophy = self.fixture()
        graph = build_knowledge_graph(corpus, philosophy)

        self.assertEqual(
            normalize_lens_spec(
                {
                    "schema_version": "tos_lens_spec_v1",
                    "lens_id": "depth-five",
                    "traversal": {"depth": 5},
                }
            )["traversal"]["depth"],
            5,
        )
        with self.assertRaisesRegex(ValueError, "traversal.depth must be between 0 and 5"):
            normalize_lens_spec(
                {
                    "schema_version": "tos_lens_spec_v1",
                    "lens_id": "too-deep",
                    "traversal": {"depth": 6},
                }
            )

        result = focus_knowledge_node(
            graph,
            "a",
            sources=["philosophy"],
            depth=1,
            direction="either",
            node_limit=20,
            relation_limit=20,
        )

        Draft202012Validator(self.schemas["lens-result.v1.schema.json"], registry=self.registry).validate(result)
        self.assertEqual(result["lens"]["seed"]["focus_node_id"], "a")
        self.assertEqual(result["focus"]["requested_id"], "a")
        self.assertEqual(result["focus"]["node_id"], "philosophy:a")
        self.assertEqual(result["focus"]["display"]["title"]["default"], "Альфа")
        self.assertEqual(result["agent_summary"]["focus_node_id"], "philosophy:a")
        self.assertEqual({item["id"] for item in result["nodes"]}, {"philosophy:a", "philosophy:b"})
        self.assertEqual([item["id"] for item in result["relations"]], ["philosophy:e"])
        self.assertEqual(result["counts"]["eligible_relations"], 1)
        self.assertEqual(result["counts"]["truncated_relations"], 0)

        # Endpoint closure may add the other end of a selected relation, but it
        # must not recursively turn those additions into an unbounded traversal.
        closure = execute_knowledge_lens(
            graph,
            {
                "schema_version": "tos_lens_spec_v1",
                "lens_id": "bounded-either-closure",
                "sources": ["philosophy"],
                "seed": {"node_ids": ["a"]},
                "traversal": {"depth": 0},
                "composition": {"endpoint_policy": "either"},
                "limits": {"nodes": 20, "relations": 20, "groups": 20},
            },
        )
        self.assertEqual({item["id"] for item in closure["nodes"]}, {"philosophy:a", "philosophy:b"})
        self.assertEqual([item["id"] for item in closure["relations"]], ["philosophy:e"])

        with self.assertRaisesRegex(ValueError, "unknown ToS knowledge focus"):
            focus_knowledge_node(graph, "missing", sources=["philosophy"])
        duplicate = {
            **next(item for item in graph["nodes"] if item["id"] == "philosophy:a"),
            "id": "canon:duplicate-a",
            "source_graph": "canon",
        }
        ambiguous_graph = {**graph, "nodes": [*graph["nodes"], duplicate]}
        with self.assertRaisesRegex(ValueError, "ambiguous ToS knowledge focus"):
            focus_knowledge_node(ambiguous_graph, "a", sources=["philosophy", "canon"])

        source_navigation = {
            **next(item for item in graph["nodes"] if item["id"] == "philosophy:a"),
            "id": "source-navigation:tos.concept.a",
            "native_id": "tos.concept.a",
            "source_graph": "source-navigation",
        }
        source_navigation["content_revision"] = _content_revision(source_navigation)
        shared_identity_graph = {**graph, "nodes": [*graph["nodes"], source_navigation]}
        sorted_focus = execute_knowledge_lens(
            shared_identity_graph,
            {
                "schema_version": "tos_lens_spec_v1",
                "lens_id": "stable-focus-through-finalization",
                "sources": ["philosophy", "source-navigation"],
                "seed": {"focus_node_id": "tos.concept.a"},
                "traversal": {"depth": 0},
                "composition": {
                    "sort_nodes": [{"field": "id", "direction": "asc"}]
                },
            },
        )
        self.assertEqual(
            sorted_focus["focus"]["node_id"],
            "source-navigation:tos.concept.a",
        )

    def test_transport_contract_is_read_only_and_maps_every_backend_adapter(self) -> None:
        contract = json.loads((ACCESS_ROOT / "contracts/knowledge-api.v1.json").read_text(encoding="utf-8"))
        operations = {item["operation_id"]: item for item in contract["operations"]}
        self.assertEqual(
            set(operations),
            {
                "tos.knowledge.catalog",
                "tos.knowledge.contracts",
                "tos.knowledge.search",
                "tos.knowledge.node.inspect",
                "tos.knowledge.relation.inspect",
                "tos.knowledge.focus",
                "tos.lens.open",
                "tos.lens.compile",
            },
        )
        self.assertIn("creates no server state", operations["tos.lens.compile"]["post_semantics"])
        self.assertTrue(contract["invariants"]["source_revision_in_result_fingerprint"])
        self.assertTrue(contract["invariants"]["item_content_revisions_in_result_fingerprint"])
        self.assertFalse(contract["invariants"]["arbitrary_code_execution"])
        self.assertFalse(contract["invariants"]["listed_operations_create_checkpoints"])
        self.assertTrue(contract["invariants"]["durable_query_cache_only"])
        self.assertFalse(contract["invariants"]["authored_graph_writes"])

    def test_filter_operators_keep_typed_cross_runtime_semantics(self) -> None:
        corpus, philosophy = self.fixture()
        graph = build_knowledge_graph(corpus, philosophy)
        numeric_spec = {
            "schema_version": "tos_lens_spec_v1",
            "lens_id": "typed-number",
            "sources": ["repository"],
            "node_query": {
                "filters": [{"field": "attributes.size_bytes", "op": "gt", "value": 10}]
            },
            "relation_query": {"enabled": False},
            "limits": {"nodes": 10, "relations": 0},
        }
        result = execute_knowledge_lens(graph, numeric_spec)
        self.assertEqual([item["native_id"] for item in result["nodes"]], ["ToS/canon/a.json"])

        for invalid_filter in (
            {"field": "attributes.size_bytes", "op": "gt", "value": "10"},
            {"field": "kind_id", "op": "eq", "value": ["concept"]},
            {"field": "kind_id", "op": "prefix", "value": 1},
        ):
            invalid = {
                "schema_version": "tos_lens_spec_v1",
                "lens_id": "invalid-filter",
                "node_query": {"filters": [invalid_filter]},
            }
            self.assertFalse(Draft202012Validator(self.schemas["lens-spec.v1.schema.json"]).is_valid(invalid))
            with self.assertRaises(ValueError):
                normalize_lens_spec(invalid)

        for invalid_localized in (42, {"default": ""}, {"default": "valid", "html_markup": "no"}):
            invalid = {
                "schema_version": "tos_lens_spec_v1",
                "lens_id": "invalid-localized",
                "title": invalid_localized,
            }
            self.assertFalse(Draft202012Validator(self.schemas["lens-spec.v1.schema.json"]).is_valid(invalid))
            with self.assertRaises(ValueError):
                normalize_lens_spec(invalid)

        for invalid_focus in (42, "", "x" * 1025):
            invalid = {
                "schema_version": "tos_lens_spec_v1",
                "lens_id": "invalid-focus",
                "seed": {"focus_node_id": invalid_focus},
            }
            self.assertFalse(Draft202012Validator(self.schemas["lens-spec.v1.schema.json"]).is_valid(invalid))
            with self.assertRaises(ValueError):
                normalize_lens_spec(invalid)


if __name__ == "__main__":
    unittest.main()
