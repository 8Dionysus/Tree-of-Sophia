"""Typed-origin traversal invariants; synthetic topology is not corpus truth."""
from __future__ import annotations

import copy
import asyncio
import heapq
import json
import random
import sys
import tempfile
import threading
import unittest
import urllib.error
import urllib.request
from http.server import ThreadingHTTPServer
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

ACCESS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ACCESS / 'src'))

from tos_access.exploration import ExplorationService, ExplorationExpired, normalize_exploration
from tos_access.exploration_origin import ExplorationReadModelInvalid, bind_origin
from tos_access.knowledge import (
    _normalize_node, _normalize_relation, _stable_digest, knowledge_scene,
    OVERVIEW_EXCLUDED_PREDICATES, OVERVIEW_EXCLUDED_RELATION_TYPES,
)
from tos_access.lens_pagination import KnowledgeRevisionConflict


def origin_graph(size=8, seed=0, identities=False):
    """Real normalization over a small explicitly synthetic cyclic multigraph."""
    rng = random.Random(seed)
    nodes = [_normalize_node({'node_id': str(i), 'node_type': 'test-origin',
                             'label': f'Synthetic node {i}', 'source_refs': ['test:origin-graph']}, 'philosophy')
             for i in range(size)]
    if identities:
        for i, node in enumerate(nodes):
            node['entity_id'] = f'tos.test.origin-group.{i % 3}'
            node['content_revision'] = _stable_digest(node)
    by_id = {n['id']: n for n in nodes}
    pairs = [(i, i + 1) for i in range(size - 1)]
    pairs += [(rng.randrange(size), rng.randrange(size)) for _ in range(size * 2)]
    relations = [_normalize_relation({'edge_id': f'r{i:03d}', 'from_id': str(a), 'to_id': str(b),
                                     'predicate_id': 'related_to', 'source_refs': ['test:origin-graph']},
                                    'philosophy', by_id) for i, (a, b) in enumerate(pairs)]
    return {'source_revision': 'a' * 64, 'nodes': nodes, 'relations': relations,
            'authority_boundary': {'is_source': False, 'writes_to_tree': False}}


def origin_query(graph, kind='relation', index=0, **options):
    selected = graph['nodes' if kind == 'node' else 'relations'][index]
    return {'schema_version': 'tos_exploration_request_v2', 'source_revision': graph['source_revision'],
            'origin': {name: selected[name] for name in ('id', 'content_revision')} | {'kind': kind}, **options}


def origin_reference(graph, request):
    """Independent zero/one-distance shortest-path oracle, not the BFS code."""
    q = normalize_exploration(request)
    nodes = {n['id']: n for n in graph['nodes']}
    relation = next((r for r in graph['relations'] if r['id'] == q['origin']['id']), None)
    roots = [q['origin']['id']] if q['origin']['kind'] == 'node' else list(dict.fromkeys([relation['from_id'], relation['to_id']]))
    distances, emitted = {id: 0 for id in roots}, set()
    queue = [(0, id) for id in roots]
    heapq.heapify(queue)
    seed_id = relation['id'] if q['origin']['kind'] == 'relation' else None
    while queue:
        depth, current = heapq.heappop(queue)
        if distances[current] != depth or depth >= q['max_depth']:
            continue
        candidates = []
        entity = nodes[current].get('entity_id', '')
        if q['profile'] == 'overview' and entity.startswith('tos.'):
            candidates.extend((n['id'], depth) for n in nodes.values()
                              if n['entity_id'] == entity and n['source_graph'] in q['sources'])
        for edge in graph['relations']:
            if edge['id'] == seed_id or edge['source_graph'] not in q['sources']:
                continue
            if q['predicate_ids'] and edge['predicate_id'] not in q['predicate_ids']:
                continue
            if q['profile'] == 'overview' and (edge['predicate_id'] in OVERVIEW_EXCLUDED_PREDICATES
                    or edge['relation_type_id'] in OVERVIEW_EXCLUDED_RELATION_TYPES):
                continue
            target = None
            if q['direction'] != 'incoming' and edge['from_id'] == current:
                target = edge['to_id']
            elif q['direction'] != 'outgoing' and edge['to_id'] == current:
                target = edge['from_id']
            if target is None or target not in nodes or nodes[target]['source_graph'] not in q['sources']:
                continue
            emitted.add(edge['id'])
            candidates.append((target, depth + 1))
        for target, distance in candidates:
            if distance < distances.get(target, q['max_depth'] + 1):
                distances[target] = distance
                heapq.heappush(queue, (distance, target))
    return set(distances), emitted, roots


class ExplorationOriginTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.schemas = {p.name: json.loads(p.read_text()) for p in (ACCESS / 'contracts').glob('*.schema.json')}
        cls.registry = Registry().with_resources((s['$id'], Resource.from_contents(s)) for s in cls.schemas.values())
        cls.validator = Draft202012Validator(cls.schemas['exploration-result.v2.schema.json'], registry=cls.registry)

    def collect(self, service, request, *, validate=True):
        packet, pages = service.explore(request), []
        for _ in range(1500):
            pages.append(packet)
            if validate:
                self.validator.validate(packet)
            origin, page = packet['origin'], packet['page']
            node_ids = [n['id'] for n in packet['nodes']]
            relation_ids = [r['id'] for r in packet['relations']]
            self.assertEqual(len(node_ids), len(set(node_ids)))
            self.assertEqual(len(relation_ids), len(set(relation_ids)))
            self.assertEqual(set(page['primary_node_ids']) | set(page['context_node_ids']), set(node_ids))
            self.assertFalse(set(page['primary_node_ids']) & set(page['context_node_ids']))
            self.assertEqual(set(page['primary_relation_ids']) | set(page['context_relation_ids']), set(relation_ids))
            self.assertFalse(set(page['primary_relation_ids']) & set(page['context_relation_ids']))
            self.assertLessEqual(len(page['primary_node_ids']), packet['query']['page_nodes'])
            self.assertLessEqual(len(page['primary_relation_ids']), packet['query']['page_relations'])
            self.assertLessEqual(len(node_ids), packet['query']['page_nodes'] + 2 * packet['query']['page_relations'] + 2)
            self.assertLessEqual(len(relation_ids), packet['query']['page_relations'] + 1)
            roots = [origin['id']] if origin['kind'] == 'node' else [v['node_id'] for v in origin['endpoints'].values()]
            self.assertTrue(set(roots) <= set(page['context_node_ids']))
            self.assertEqual(packet['scene']['focus_vertex_id'] is None, origin['kind'] == 'relation')
            if origin['kind'] == 'relation':
                self.assertEqual(page['context_relation_ids'], [origin['id']])
                self.assertIn(origin['id'], packet['scene']['compact']['relation_ids'])
            for edge in packet['relations']:
                self.assertTrue({edge['from_id'], edge['to_id']} <= set(node_ids))
            cursor = page['next_cursor']
            if cursor is None:
                return pages
            packet = service.explore({'cursor': cursor})
            self.assertEqual(packet, service.explore({'cursor': cursor}))
        self.fail('typed-origin exploration failed to terminate')

    def test_multisource_origin_conserves_reachability_and_progress(self):
        for seed in range(2):
            graph = origin_graph(seed=seed, identities=True)
            for direction in ('either', 'incoming', 'outgoing'):
                for depth in (0, 2):
                    for profile in ('overview', 'all'):
                        for size in (1, 3):
                            with self.subTest(seed=seed, direction=direction, depth=depth, profile=profile, size=size):
                                request = origin_query(graph, direction=direction, max_depth=depth, profile=profile,
                                                       page_nodes=size, page_relations=size)
                                expected_n, expected_r, roots = origin_reference(graph, request)
                                before = copy.deepcopy(graph)
                                pages = self.collect(ExplorationService(lambda: graph, work_limit=5), request, validate=False)
                                primary = [id for p in pages for id in p['page']['primary_node_ids']]
                                emitted = [id for p in pages for id in p['page']['primary_relation_ids']]
                                self.assertEqual(set(primary) | set(roots), expected_n)
                                self.assertEqual(set(emitted), expected_r)
                                self.assertEqual(len(primary), len(set(primary)))
                                self.assertEqual(len(emitted), len(set(emitted)))
                                self.assertEqual(pages[-1]['counts']['emitted_relations'], len(expected_r))
                                self.assertEqual(pages[-1]['counts']['discovered_nodes'], len(expected_n))
                                self.assertEqual(graph, before)
                                self.validator.validate(pages[0])
                                self.validator.validate(pages[-1])

    def test_depth_zero_self_loop_and_explicit_filtered_origin(self):
        for self_loop in (False, True):
            graph = origin_graph()
            relation = graph['relations'][0]
            if self_loop:
                relation['to_id'] = relation['from_id']
            relation['relation_type_id'] = 'tos.relation.projects'
            relation['predicate_id'] = 'has_text_unit'
            request = origin_query(graph, max_depth=0, predicate_ids=['not-the-origin'], page_nodes=1, page_relations=1)
            pages = self.collect(ExplorationService(lambda: graph), request)
            self.assertEqual(len(pages), 1)
            self.assertEqual(len(pages[0]['nodes']), 1 if self_loop else 2)
            self.assertEqual(pages[0]['page']['primary_relation_ids'], [])
            self.assertEqual(pages[0]['counts']['emitted_relations'], 0)
            self.assertEqual(pages[0]['origin']['endpoints']['from']['node_id'], relation['from_id'])
            self.assertEqual(pages[0]['origin']['endpoints']['to']['node_id'], relation['to_id'])

    def test_exact_node_v2_and_legacy_v1_are_distinct_contracts(self):
        graph = origin_graph()
        service = ExplorationService(lambda: graph)
        pages = self.collect(service, origin_query(graph, 'node', max_depth=1, page_nodes=1, page_relations=1))
        self.assertEqual(pages[0]['origin']['kind'], 'node')
        self.assertEqual(pages[0]['page']['context_relation_ids'], [])
        legacy = service.explore({'focus_node_id': '0', 'max_depth': 0})
        Draft202012Validator(self.schemas['exploration-result.v1.schema.json'], registry=self.registry).validate(legacy)
        self.assertEqual(legacy['focus'], {'node_id': 'philosophy:0'})
        self.assertNotIn('origin', legacy)
        self.assertEqual(legacy['page']['primary_node_ids'], ['philosophy:0'])

    def test_unicode_edge_whitespace_matches_portable_request_schema(self):
        validator = Draft202012Validator(self.schemas['exploration-request.v2.schema.json'], registry=self.registry)
        request = origin_query(origin_graph())
        whitespace = [*range(9, 14), *range(28, 33), 0x85, 0xa0, 0x1680,
                      *range(0x2000, 0x200b), 0x2028, 0x2029, 0x202f, 0x205f, 0x3000]
        cases = [(identifier, False) for point in whitespace
                 for identifier in (chr(point) + 'node', 'node' + chr(point))]
        cases.extend((identifier, True) for identifier in ('\ufeffnode', 'node\ufeff', '\u200bnode',
                                                            'node\u200b', 'node\ninside', 'a\u0085b', '😀'))
        for identifier, expected in cases:
            candidate = request | {'origin': request['origin'] | {'id': identifier}}
            with self.subTest(identifier=repr(identifier)):
                self.assertEqual(validator.is_valid(candidate), expected)
                if expected:
                    self.assertEqual(normalize_exploration(candidate)['origin']['id'], identifier)
                else:
                    with self.assertRaises(ValueError):
                        normalize_exploration(candidate)

    def test_binding_is_exact_bounded_and_rejects_drift_without_checkpoint(self):
        graph = origin_graph()
        request = origin_query(graph)
        nodes, relations = {n['id']: n for n in graph['nodes']}, {r['id']: r for r in graph['relations']}
        calls = []
        def lookup(kind, table, id):
            calls.append((kind, id))
            return table.get(id)
        bound, roots, seeds = bind_origin(normalize_exploration(request), graph['source_revision'],
            lambda id: lookup('node', nodes, id), lambda id: lookup('relation', relations, id))
        self.assertEqual(len(calls), 3)
        self.assertEqual(roots, ['philosophy:0', 'philosophy:1'])
        self.assertEqual(seeds, [request['origin']['id']])
        for changed, error in [({'source_revision': 'b' * 64}, KnowledgeRevisionConflict),
                               ({'origin': request['origin'] | {'id': 'r000'}}, KeyError),
                               ({'origin': request['origin'] | {'content_revision': 'b' * 64}}, KnowledgeRevisionConflict),
                               ({'sources': ['source-claims']}, ValueError)]:
            service = ExplorationService(lambda: graph)
            with self.subTest(changed=changed), self.assertRaises(error):
                service.explore(request | changed)
            self.assertEqual(service.records, {})

    def test_invalid_origin_and_structural_corruption_fail_explicitly(self):
        graph = origin_graph()
        request = origin_query(graph)
        for changed in ({'focus_node_id': '0'}, {'cursor': 'a' * 64}, {'schema_version': 'future'},
                        {'source_revision': None}, {'origin': []}, {'origin': request['origin'] | {'kind': []}},
                        {'origin': request['origin'] | {'extra': True}}, {'origin': request['origin'] | {'id': ' spaced '}}):
            with self.subTest(changed=changed), self.assertRaises(ValueError):
                ExplorationService(lambda: graph).explore(request | changed)
        request_validator = Draft202012Validator(self.schemas['exploration-request.v2.schema.json'], registry=self.registry)
        for invalid in (request | {'source_revision': 'a' * 64 + '\n'},
                        request | {'origin': request['origin'] | {'content_revision': 'a' * 64 + '\n'}},
                        request | {'origin': request['origin'] | {'id': 'selected\n'}}, {'cursor': 'a' * 64 + '\n'}):
            with self.subTest(invalid=invalid):
                self.assertFalse(request_validator.is_valid(invalid))
                with self.assertRaises(ValueError):
                    ExplorationService(lambda: graph).explore(invalid)
        for key in ('attributes', 'semantics', 'display', 'epistemic', 'predicate_mapping'):
            corrupt = copy.deepcopy(graph)
            corrupt['relations'][0][key] = None
            with self.subTest(key=key), self.assertRaises(ExplorationReadModelInvalid):
                ExplorationService(lambda: corrupt).explore(request)
        corrupt = copy.deepcopy(graph)
        corrupt['nodes'] = corrupt['nodes'][1:]
        with self.assertRaises(ExplorationReadModelInvalid):
            ExplorationService(lambda: corrupt).explore(request)
        duplicate = copy.deepcopy(graph)
        duplicate['relations'].append(copy.deepcopy(duplicate['relations'][0]))
        with self.assertRaises(KeyError):
            ExplorationService(lambda: duplicate).explore(request)

    def test_cursor_keeps_resolved_origin_and_rejects_changed_snapshot(self):
        holder = [origin_graph()]
        service = ExplorationService(lambda: holder[0], work_limit=3)
        first = service.explore(origin_query(holder[0], max_depth=3, page_nodes=1, page_relations=1))
        cursor = first['page']['next_cursor']
        self.assertIsNotNone(cursor)
        second = service.explore({'cursor': cursor})
        self.assertEqual(first['origin'], second['origin'])
        self.assertEqual(second, service.explore({'cursor': cursor}))
        holder[0] = copy.deepcopy(holder[0])
        holder[0]['nodes'][0]['content_revision'] = 'b' * 64
        with self.assertRaises(KnowledgeRevisionConflict):
            service.explore({'cursor': cursor})
        with self.assertRaises(ExplorationExpired):
            ExplorationService(lambda: holder[0]).explore({'cursor': cursor})

    def test_invalid_index_does_not_replace_last_good_snapshot(self):
        original = origin_graph()
        holder = [original]
        service = ExplorationService(lambda: holder[0])
        request = origin_query(original, max_depth=2, page_nodes=1, page_relations=1)
        first = service.explore(request)
        cursor = first['page']['next_cursor']
        expected = service.explore({'cursor': cursor})
        for collection, field in (('nodes', 'id'), ('nodes', 'content_revision'),
                                  ('relations', 'from_id'), ('relations', 'content_revision')):
            holder[0] = copy.deepcopy(original)
            del holder[0][collection][0][field]
            with self.subTest(collection=collection, field=field), self.assertRaises(ExplorationReadModelInvalid):
                service.explore(request)
            self.assertIs(service.graph, original)
            holder[0] = original
            self.assertEqual(service.explore({'cursor': cursor}), expected)

    def test_http_origin_continuation_and_explicit_binding_failures(self):
        from tos_access.http_server import build_handler
        original = origin_graph()
        holder = [original]
        service = ExplorationService(lambda: holder[0])
        class Core:
            knowledge_explore = staticmethod(service.explore)
        with tempfile.TemporaryDirectory() as temp:
            server = ThreadingHTTPServer(('127.0.0.1', 0), build_handler(Core(), Path(temp)))
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            url = f'http://127.0.0.1:{server.server_port}/api/knowledge/explore'
            def post(request):
                wire = urllib.request.Request(url, json.dumps(request).encode(), {'Content-Type': 'application/json'})
                with urllib.request.urlopen(wire) as response:
                    return json.load(response)
            try:
                request = origin_query(original, page_nodes=1, page_relations=1)
                first = post(request)
                cursor = first['page']['next_cursor']
                second = post({'cursor': cursor})
                self.validator.validate(second)
                self.assertEqual(second, post({'cursor': cursor}))
                self.assertEqual(second['origin'], first['origin'])
                for changed, status in (
                    (request | {'focus_node_id': '0'}, 400),
                    (request | {'source_revision': 'b' * 64}, 409),
                    (request | {'origin': request['origin'] | {'content_revision': 'b' * 64}}, 409),
                    (request | {'origin': request['origin'] | {'id': 'r000'}}, 404),
                    (request | {'sources': ['source-claims']}, 400),
                    ({'cursor': '0' * 64}, 410),
                ):
                    with self.subTest(request=changed), self.assertRaises(urllib.error.HTTPError) as raised:
                        post(changed)
                    self.assertEqual(raised.exception.code, status)
                    raised.exception.close()
                holder[0] = copy.deepcopy(original)
                del holder[0]['nodes'][0]['content_revision']
                with self.assertRaises(urllib.error.HTTPError) as raised:
                    post(request)
                self.assertEqual(raised.exception.code, 503)
                raised.exception.close()
            finally:
                server.shutdown()
                thread.join()
                server.server_close()

    def test_fixture_core_native_mcp_and_contract_discovery_use_same_v2_model(self):
        # Existing normalized public-source fixture, not historical evidence.
        from test_access_contract import write_fixture
        from tos_access.core import ToSAccessCore
        from tos_access.mcp_server import build_server
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            write_fixture(root)
            core = ToSAccessCore.discover(root)
            contracts = core.knowledge_exploration_contracts()
            self.assertEqual(contracts['request'], self.schemas['exploration-request.v1.schema.json'])
            self.assertEqual(contracts['result'], self.schemas['exploration-result.v1.schema.json'])
            self.assertEqual(contracts['request_v2'], self.schemas['exploration-request.v2.schema.json'])
            self.assertEqual(contracts['result_v2'], self.schemas['exploration-result.v2.schema.json'])
            self.assertEqual(contracts['capabilities']['v2_origin_kinds'], ['node', 'relation'])
            graph = core.knowledge_graph()
            request = origin_query(graph, page_nodes=1, page_relations=1, max_depth=3)
            local = self.collect(core._exploration, request)
            self.assertGreater(len(local), 1)
            mcp = build_server(root)
            async def check():
                discovered = await mcp._tool_manager.call_tool('tos_knowledge_exploration_contracts', {})
                self.assertEqual(discovered, contracts)
                packet = await mcp._tool_manager.call_tool('tos_knowledge_explore', {'request': request})
                for index, expected in enumerate(local):
                    self.validator.validate(packet)
                    # Disposable cursors belong to their own service instance.
                    self.assertEqual(packet | {'page': packet['page'] | {'next_cursor': None}},
                                     expected | {'page': expected['page'] | {'next_cursor': None}})
                    cursor = packet['page']['next_cursor']
                    if cursor is not None:
                        packet = await mcp._tool_manager.call_tool('tos_knowledge_explore', {'request': {'cursor': cursor}})
                    else:
                        self.assertEqual(index, len(local) - 1)
            asyncio.run(check())

    def test_selected_claim_leg_does_not_fold_into_a_different_line(self):
        graph = origin_graph(3)
        claim = graph['nodes'][0]
        claim['type_id'] = 'tos.entity.claim'
        claim['semantics']['claim'] = {'subject_node_id': graph['nodes'][1]['id'],
            'object_node_id': graph['nodes'][2]['id'], 'predicate_mapping_status': 'mapped',
            'relation_type_id': 'tos.relation.related-to'}
        graph['relations'] = [graph['relations'][0], graph['relations'][1]]
        for edge, target, type_id in zip(graph['relations'], graph['nodes'][1:],
                                       ('tos.relation.has-subject', 'tos.relation.has-object')):
            edge.update(from_id=claim['id'], to_id=target['id'], relation_type_id=type_id)
        scene = knowledge_scene(graph['nodes'], graph['relations'])
        self.assertEqual(len(scene['compact']['claim_paths']), 1)
        pages = self.collect(ExplorationService(lambda: graph), origin_query(graph, max_depth=1))
        self.assertEqual(pages[0]['scene']['compact']['claim_paths'], [])
        self.assertEqual(pages[0]['scene']['compact']['retained_claims'], [{'node_id': claim['id'], 'reason': 'focus-relation'}])


if __name__ == '__main__':
    unittest.main()
