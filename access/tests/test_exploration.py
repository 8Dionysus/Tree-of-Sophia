from __future__ import annotations

import copy
import heapq
import json
import random
import sys
import tempfile
import threading
import unittest
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor
from http.server import ThreadingHTTPServer
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

ACCESS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ACCESS / "src"))
from tos_access.exploration import ExplorationService, ExplorationExpired, normalize_exploration
from tos_access.lens_pagination import KnowledgeRevisionConflict
from tos_access.http_server import build_handler


def graph_for(size=12, seed=0):
    rng = random.Random(seed)
    pairs = [(i, i + 1) for i in range(size - 1)]
    pairs += [(rng.randrange(size), rng.randrange(size)) for _ in range(size * 3)]
    return {
        "source_revision": "a" * 64,
        "nodes": [{"id": str(i), "source_graph": "philosophy", "content_revision": f"n{i}",
                   "display": {"title": {"default": str(i)}}, "source_refs": ["ToS/example.json"]}
                  for i in range(size)],
        "relations": [{"id": f"r{i:04d}", "from_id": str(a), "to_id": str(b),
                       "source_graph": "philosophy", "predicate_id": "related", "content_revision": f"r{i}"}
                      for i, (a, b) in enumerate(pairs)],
    }


def reference(graph, query):
    depths = {query["focus_node_id"]: 0}
    relations = set()
    queue = list(depths)
    for current in queue:
        if depths[current] >= query["max_depth"]:
            continue
        for edge in sorted(graph["relations"], key=lambda e: e["id"]):
            if query["direction"] == "outgoing" and edge["from_id"] != current:
                continue
            if query["direction"] == "incoming" and edge["to_id"] != current:
                continue
            if current not in (edge["from_id"], edge["to_id"]):
                continue
            target = edge["to_id"] if edge["from_id"] == current else edge["from_id"]
            relations.add(edge["id"])
            if target not in depths:
                depths[target] = depths[current] + 1
                queue.append(target)
    return set(depths), relations


class ExplorationTests(unittest.TestCase):
    def test_identity_expansion_matches_zero_one_distance_across_pages_and_cycles(self):
        # Independent shortest-path oracle; synthetic topology only, not ToS facts.
        for seed in range(5):
            graph = graph_for(size=8, seed=seed)
            for node in graph['nodes']:
                node['entity_id'] = 'tos.test.subject.' + str((int(node['id']) + seed) % 3)
            for direction in ('either', 'outgoing', 'incoming'):
                for max_depth in (1, 2, 3):
                    distances, expected_edges = {'0': 0}, set()
                    queue = [(0, '0')]
                    while queue:
                        depth, current = heapq.heappop(queue)
                        if depth != distances[current] or depth >= max_depth: continue
                        own_entity = graph['nodes'][int(current)]['entity_id']
                        candidates = [(n['id'], depth) for n in graph['nodes'] if n['entity_id'] == own_entity]
                        for edge in graph['relations']:
                            if direction != 'incoming' and edge['from_id'] == current:
                                candidates.append((edge['to_id'], depth + 1)); expected_edges.add(edge['id'])
                            if direction != 'outgoing' and edge['to_id'] == current:
                                candidates.append((edge['from_id'], depth + 1)); expected_edges.add(edge['id'])
                        for target, distance in candidates:
                            if distance < distances.get(target, max_depth + 1):
                                distances[target] = distance
                                heapq.heappush(queue, (distance, target))
                    for size in (1, 3):
                        with self.subTest(seed=seed, direction=direction, depth=max_depth, size=size):
                            pages = self.collect(ExplorationService(lambda: graph, work_limit=5),
                                {'focus_node_id': '0', 'max_depth': max_depth, 'direction': direction,
                                 'page_nodes': size, 'page_relations': size})
                            primary = [id for p in pages for id in p['page']['primary_node_ids']]
                            edges = [r['id'] for p in pages for r in p['relations']]
                            self.assertEqual(set(primary), set(distances))
                            self.assertEqual(set(edges), expected_edges)
                            self.assertEqual(len(primary), len(set(primary)))
                            self.assertEqual(len(edges), len(set(edges)))
                            self.assertTrue(all(len(p['nodes']) <= 1 + 3 * size for p in pages))
                            self.assertTrue(all(p['page']['work_units'] <= 5 for p in pages))

    def test_identity_carriers_are_expanded_once_not_quadratically(self):
        for size in (16, 32, 64):
            graph = graph_for(size=size)
            graph['relations'] = []
            for node in graph['nodes']: node['entity_id'] = 'tos.test.one-subject'
            pages = self.collect(ExplorationService(lambda: graph, work_limit=32),
                                 {'focus_node_id': '0', 'max_depth': 1, 'page_nodes': 7})
            self.assertEqual(sum(len(p['page']['primary_node_ids']) for p in pages), size)
            self.assertLessEqual(sum(p['page']['work_units'] for p in pages), 4 * size + 1)

    def collect(self, service, query):
        page = service.explore(query)
        pages = []
        for _ in range(1000):
            pages.append(page)
            cursor = page["page"]["next_cursor"]
            if cursor is None:
                return pages
            page = service.explore({"cursor": cursor})
            self.assertEqual(service.explore({"cursor": cursor}), page)
        self.fail("exploration did not terminate")

    def test_generated_cycles_parallel_edges_and_page_size_conserve_reachability(self):
        for seed in range(6):
            graph = graph_for(seed=seed)
            for direction in ("either", "outgoing", "incoming"):
                for depth in (0, 1, 3, 10):
                    query = {"focus_node_id": "0", "direction": direction, "max_depth": depth}
                    expected_n, expected_r = reference(graph, query)
                    for size in (1, 2, 7):
                        with self.subTest(seed=seed, direction=direction, depth=depth, size=size):
                            service = ExplorationService(lambda: graph, work_limit=7)
                            pages = self.collect(service, {**query, "page_nodes": size, "page_relations": size})
                            ns = [id for p in pages for id in p["page"]["primary_node_ids"]]
                            rs = [e["id"] for p in pages for e in p["relations"]]
                            self.assertEqual(len(ns), len(set(ns)))
                            self.assertEqual(len(rs), len(set(rs)))
                            self.assertEqual(set(ns), expected_n)
                            self.assertEqual(set(rs), expected_r)
                            self.assertEqual(pages[-1]["status"], "complete")
                            for p in pages:
                                ids = {n["id"] for n in p["nodes"]}
                                self.assertEqual({id for v in p['scene']['vertices'] for id in v['node_ids']}, ids)
                                self.assertEqual({a['relation_id'] for a in p['scene']['arcs']}
                                                 | set(p['scene']['collapsed_relation_ids']),
                                                 {r['id'] for r in p['relations']})
                                self.assertLessEqual(p["page"]["work_units"], 7)
                                self.assertLessEqual(len(p["page"]["primary_node_ids"]), size)
                                self.assertLessEqual(len(p["relations"]), size)
                                self.assertIn("0", ids)
                                for edge in p["relations"]:
                                    self.assertTrue({edge["from_id"], edge["to_id"]} <= ids)
                                    reason = p["inclusion"]["relations"][edge["id"]]
                                    self.assertIn(reason["via_node_id"], (edge["from_id"], edge["to_id"]))
                                self.assertEqual(set(p["inclusion"]["relations"]), {e["id"] for e in p["relations"]})

    def test_snapshot_content_not_only_source_revision_invalidates_replay(self):
        holder = [graph_for()]
        service = ExplorationService(lambda: holder[0])
        cursor = service.explore({"focus_node_id": "0", "page_nodes": 1})["page"]["next_cursor"]
        old = service.explore({"cursor": cursor})
        holder[0] = copy.deepcopy(holder[0])
        self.assertEqual(service.explore({"cursor": cursor}), old)
        holder[0] = copy.deepcopy(holder[0])
        holder[0]["nodes"][-1]["content_revision"] = "changed-normalization"
        with self.assertRaises(KnowledgeRevisionConflict):
            service.explore({"cursor": cursor})

    def test_expiry_eviction_and_restart_are_explicit(self):
        now = [0]
        graph = graph_for()
        service = ExplorationService(lambda: graph, clock=lambda: now[0], ttl=10, max_checkpoints=2)
        query = {"focus_node_id": "0", "page_nodes": 1}
        cursor = service.explore(query)["page"]["next_cursor"]
        for _ in range(2):
            service.explore(query)
        with self.assertRaises(ExplorationExpired):
            service.explore({"cursor": cursor})
        cursor = service.explore(query)["page"]["next_cursor"]
        now[0] = 10
        with self.assertRaises(ExplorationExpired):
            service.explore({"cursor": cursor})
        with self.assertRaises(ExplorationExpired):
            ExplorationService(lambda: graph).explore({"cursor": cursor})
        self.assertLessEqual(service.stored_bytes, service.max_bytes)

    def test_continuation_exceeds_lens_bounds_without_rescanning_snapshot(self):
        graph = graph_for(600)
        service = ExplorationService(lambda: graph)
        first = service.explore({"focus_node_id": "0", "max_depth": 10, "page_nodes": 100, "page_relations": 100})
        class NoIteration(list):
            def __iter__(self):
                raise AssertionError("page attempted to scan the whole graph")
        # The immutable snapshot identity is unchanged; page execution must use
        # the established index. This guard is deliberately test-only.
        graph["nodes"] = NoIteration(graph["nodes"])
        graph["relations"] = NoIteration(graph["relations"])
        nodes, edges = set(first["page"]["primary_node_ids"]), {e["id"] for e in first["relations"]}
        page = first
        while page["page"]["next_cursor"]:
            page = service.explore({"cursor": page["page"]["next_cursor"]})
            nodes.update(page["page"]["primary_node_ids"])
            edges.update(e["id"] for e in page["relations"])
        self.assertEqual(page["status"], "complete")
        self.assertEqual(len(nodes), 600)
        self.assertEqual(len(edges), 2399)

    def test_concurrent_replay_is_identical(self):
        graph = graph_for()
        service = ExplorationService(lambda: graph)
        cursor = service.explore({"focus_node_id": "0", "page_nodes": 1})["page"]["next_cursor"]
        with ThreadPoolExecutor(max_workers=8) as pool:
            pages = list(pool.map(lambda _: service.explore({"cursor": cursor}), range(16)))
        self.assertTrue(all(p == pages[0] for p in pages))

    def test_rejected_oversized_successor_does_not_cache_a_false_success(self):
        graph = graph_for(600)
        service = ExplorationService(lambda: graph)
        cursor = service.explore({"focus_node_id": "0", "page_nodes": 1})["page"]["next_cursor"]
        before = service.records[cursor]
        # Growth of the visited set can outgrow a cache even though its input
        # checkpoint fitted. Retry must fail consistently, not return a dead link.
        service.max_bytes = len(before[1])
        for _ in range(2):
            with self.assertRaises(ExplorationExpired):
                service.explore({"cursor": cursor})
            self.assertEqual(service.records[cursor], before)

    def test_work_and_session_limits_do_not_report_false_completion(self):
        graph = graph_for()
        for kwargs, reason in (({"node_limit": 2}, "session_nodes"), ({"relation_limit": 1}, "session_relations")):
            page = self.collect(ExplorationService(lambda: graph, **kwargs), {"focus_node_id": "0"})[-1]
            self.assertEqual(page["status"], "limit_reached")
            self.assertEqual(page["limit_reason"], reason)
        service = ExplorationService(lambda: graph, work_limit=1)
        pages = self.collect(service, {"focus_node_id": "0", "predicate_ids": ["absent"]})
        self.assertGreater(len(pages), 1)
        self.assertEqual(pages[-1]["status"], "complete")
        self.assertTrue(all(not p["relations"] for p in pages))

    def test_sources_profiles_and_direction_do_not_leak_endpoints(self):
        graph = graph_for(3)
        graph["nodes"][2]["source_graph"] = "canon"
        graph["relations"][0]["predicate_id"] = "has_anchor"
        graph["relations"][2]["source_graph"] = "canon"
        service = ExplorationService(lambda: graph)
        pages = self.collect(service, {"focus_node_id": "0", "sources": ["philosophy"]})
        ids = {n["id"] for p in pages for n in p["nodes"]}
        edges = {e["id"] for p in pages for e in p["relations"]}
        self.assertNotIn("2", ids)
        self.assertNotIn("r0000", edges)
        self.assertNotIn("r0002", edges)
        all_pages = self.collect(service, {"focus_node_id": "0", "profile": "all"})
        self.assertIn("r0000", {e["id"] for p in all_pages for e in p["relations"]})

    def test_request_schema_and_normalizer_reject_invalid_queries(self):
        schema = json.loads((ACCESS / "contracts/exploration-request.v1.schema.json").read_text())
        validator = Draft202012Validator(schema)
        for request in ({}, {"focus_node_id": " "}, {"focus_node_id": "0", "page_nodes": True},
                        {"focus_node_id": "0", "sources": []}, {"focus_node_id": "0", "sources": [1]},
                        {"focus_node_id": "0", "max_depth": 11}, {"focus_node_id": "0", "direction": []},
                        {"focus_node_id": "0", "extra": 1}):
            self.assertFalse(validator.is_valid(request), request)
            with self.assertRaises(ValueError):
                normalize_exploration(request)

    def test_http_continuation_and_failure_statuses(self):
        graph = graph_for()
        service = ExplorationService(lambda: graph)
        class Core:
            knowledge_explore = staticmethod(service.explore)
        with tempfile.TemporaryDirectory() as temp:
            server = ThreadingHTTPServer(("127.0.0.1", 0), build_handler(Core(), Path(temp)))
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            url = f"http://127.0.0.1:{server.server_port}/api/knowledge/explore"
            def post(request):
                req = urllib.request.Request(url, json.dumps(request).encode(), {"Content-Type": "application/json"})
                with urllib.request.urlopen(req) as response:
                    return json.load(response)
            try:
                first = post({"focus_node_id": "0", "page_nodes": 1})
                cursor = first["page"]["next_cursor"]
                page = post({"cursor": cursor})
                self.assertEqual(post({"cursor": cursor}), page)
                for request, status in (({"cursor": "bad"}, 400), ({"cursor": "0" * 64}, 410),
                                        ({"cursor": cursor, "max_depth": 2}, 400)):
                    with self.assertRaises(urllib.error.HTTPError) as raised:
                        post(request)
                    self.assertEqual(raised.exception.code, status)
                    raised.exception.close()
                graph = copy.deepcopy(graph)
                graph["source_revision"] = "b" * 64
                with self.assertRaises(urllib.error.HTTPError) as raised:
                    post({"cursor": cursor})
                self.assertEqual(raised.exception.code, 409)
                raised.exception.close()
            finally:
                server.shutdown()
                thread.join()
                server.server_close()

    def test_real_core_result_schemas_and_native_mcp_continuation(self):
        # Existing public-source fixture gives full normalized display/evidence
        # envelopes; generated tests above deliberately isolate graph topology.
        from test_access_contract import write_fixture
        from tos_access.core import ToSAccessCore
        from tos_access.mcp_server import build_server
        import asyncio
        schemas = [json.loads((ACCESS / "contracts" / name).read_text()) for name in
                   ("knowledge-graph.v1.schema.json", "exploration-request.v1.schema.json", "exploration-result.v1.schema.json")]
        registry = Registry().with_resources((s["$id"], Resource.from_contents(s)) for s in schemas)
        validator = Draft202012Validator(schemas[-1], registry=registry)
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            write_fixture(root)
            core = ToSAccessCore.discover(root)
            request = {"focus_node_id": "tos.work.fixture", "page_nodes": 1}
            pages = self.collect(core._exploration, request)
            for page in pages:
                validator.validate(page)
            self.assertGreater(len(pages), 1)
            server = build_server(root)
            async def check_mcp():
                first = await server._tool_manager.call_tool("tos_knowledge_explore", {"request": request})
                follow = await server._tool_manager.call_tool("tos_knowledge_explore", {"request": {"cursor": first["page"]["next_cursor"]}})
                validator.validate(follow)
                self.assertEqual(follow["page"]["number"], 2)
            asyncio.run(check_mcp())


if __name__ == "__main__":
    unittest.main()
