"""Prepared/native full-stream parity and bounded SQL query boundaries."""
from __future__ import annotations

import copy
import concurrent.futures
import hashlib
import json
import random
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from contextlib import closing
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "deploy/cloudflare-worker/scripts"))
import build_runtime as builder
from test_access_contract import write_fixture
from tos_access.core import ToSAccessCore
from tos_access.exploration import ExplorationExpired, ExplorationService
from tos_access.knowledge import OVERVIEW_EXCLUDED_PREDICATES, OVERVIEW_EXCLUDED_RELATION_TYPES
from tos_access.lens_pagination import KnowledgeRevisionConflict
from tos_access.published_exploration import PublishedExplorationService
from tos_access.published_checkpoints import PublishedCheckpointError, PublishedCheckpointClockRollback, _Transaction
from tos_access.published_read_metadata import TOP_KEY, _compact, published_snapshot_binding
from tos_access.published_read_model import PublishedKnowledgeReadModel, PublishedReadBudgetExceeded, PublishedReadLimits, _Read


class PublishedExplorationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        temporary = tempfile.TemporaryDirectory()
        cls.addClassCleanup(temporary.cleanup)
        cls.root = Path(temporary.name)
        write_fixture(cls.root)
        core = ToSAccessCore.discover(cls.root)
        snapshot = copy.deepcopy(core.knowledge_snapshot())
        graph = snapshot["graph"]
        node_template, edge_template = graph["nodes"][0], graph["relations"][0]
        # Controlled public synthetic topology; not semantic admission of edits.
        graph["nodes"] = []
        sources = ("philosophy", "canon", "source-navigation")
        for index in range(12):
            node = copy.deepcopy(node_template)
            node.update(id=f"node:{index:02d}", native_id="ambiguous" if index in (1, 2) else f"native-{index}",
                        entity_id=f"tos.exploration.entity.{index % 4}", source_graph=sources[index % 3],
                        content_revision=hashlib.sha256(f"node-{index}".encode()).hexdigest())
            node["transport_probe"] = {"false": False, "zero": 0, "unknown": [None, "Форма"]}
            graph["nodes"].append(node)
        rng = random.Random(7)
        pairs = [(index, (index + 1) % 12) for index in range(12)]
        pairs += [(rng.randrange(12), rng.randrange(12)) for _ in range(16)] + [(0, 0), (0, 1)]
        graph["relations"] = []
        for index, (left, right) in enumerate(pairs):
            edge = copy.deepcopy(edge_template)
            edge.update(id=f"edge:{index:03d}", native_id=f"edge-{index}",
                        from_id=f"node:{left:02d}", to_id=f"node:{right:02d}", source_graph=sources[index % 3],
                        predicate_id=sorted(OVERVIEW_EXCLUDED_PREDICATES)[0] if index % 7 == 0 else "related",
                        content_revision=hashlib.sha256(f"edge-{index}".encode()).hexdigest())
            if index % 11 == 0:
                edge["relation_type_id"] = sorted(OVERVIEW_EXCLUDED_RELATION_TYPES)[0]
            graph["relations"].append(edge)
        cls.graph = builder.normalize_paths(graph, cls.root)
        snapshot["graph"] = cls.graph
        target = cls.root / "runtime/read-model.sql"
        with patch.object(builder, "REPO_ROOT", cls.root), patch.object(ToSAccessCore, "knowledge_snapshot", return_value=snapshot):
            builder.build_read_model_sql(core, target, "a" * 64)
        cls.seed = cls.root / "published.sqlite"
        with closing(sqlite3.connect(cls.seed)) as db:
            db.executescript(target.read_text())
            db.executescript((builder.WORKER_ROOT / "migrations/0001-exploration.sql").read_text())
            top = json.loads("".join(row[0] for row in db.execute("SELECT json_chunk FROM edge_meta WHERE key=? ORDER BY part", (TOP_KEY,))))
            epoch = db.execute("SELECT epoch FROM knowledge_exploration_clock WHERE singleton=1").fetchone()[0]
            cls.binding = published_snapshot_binding(top, epoch)

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.path = Path(temporary.name) / "published.sqlite"
        shutil.copyfile(self.seed, self.path)
        self.reader = PublishedKnowledgeReadModel(self.path, self.binding)

    @staticmethod
    def comparable(packet):
        result = copy.deepcopy(packet)
        del result["snapshot_revision"]
        result["page"]["next_cursor"] = "opaque" if result["page"]["next_cursor"] else None
        return result

    def assert_stream(self, request, **limits):
        native = ExplorationService(lambda: self.graph, **limits)
        prepared = PublishedExplorationService(self.reader, **limits)
        native_request, prepared_request = request, request
        for number in range(500):
            expected = native.explore(native_request)
            actual = prepared.explore(prepared_request)
            self.assertEqual(self.comparable(actual), self.comparable(expected), f"page {number + 1}: {request}")
            if "cursor" in prepared_request:
                self.assertEqual(prepared.explore(prepared_request), actual)
                self.assertEqual(native.explore(native_request), expected)
            if actual["status"] != "paused":
                return actual
            native_request = {"cursor": expected["page"]["next_cursor"]}
            prepared_request = {"cursor": actual["page"]["next_cursor"]}
        self.fail("bounded synthetic exploration did not terminate")

    def test_v1_exact_full_stream_parity_for_profiles_direction_filters_and_saturation(self):
        for profile in ("overview", "all"):
            for direction in ("either", "incoming", "outgoing"):
                for size, work in ((1, 2), (3, 7), (100, 512)):
                    with self.subTest(profile=profile, direction=direction, size=size):
                        self.assert_stream({"focus_node_id": "node:00", "profile": profile, "direction": direction,
                                            "max_depth": 3, "page_nodes": size, "page_relations": size}, work_limit=work)
        for source in ("philosophy", "canon"):
            self.assert_stream({"focus_node_id": "tos.exploration.entity.0", "sources": [source],
                                "predicate_ids": ["related"], "max_depth": 2, "page_nodes": 1, "page_relations": 1}, work_limit=3)

    def test_v2_node_relation_origin_parity_and_zero_depth_closure(self):
        for kind, item in (("node", self.graph["nodes"][0]), ("relation", self.graph["relations"][0])):
            for depth in (0, 2):
                self.assert_stream({"schema_version": "tos_exploration_request_v2", "source_revision": self.graph["source_revision"],
                                    "origin": {"kind": kind, "id": item["id"], "content_revision": item["content_revision"]},
                                    "profile": "all", "max_depth": depth, "page_nodes": 1, "page_relations": 1}, work_limit=3)

    def test_session_limits_have_exact_native_terminal_packet(self):
        self.assert_stream({"focus_node_id": "node:00", "profile": "all", "max_depth": 5}, node_limit=2)
        self.assert_stream({"focus_node_id": "node:00", "profile": "all", "max_depth": 5}, relation_limit=1)

    def test_cold_requests_and_replay_never_build_graph_or_read_catalog(self):
        statements = []
        original = PublishedKnowledgeReadModel._connect
        def connect(reader):
            db = original(reader)
            db.set_trace_callback(statements.append)
            return db
        with patch.object(PublishedKnowledgeReadModel, "_connect", connect), patch.object(
                ExplorationService, "_index", side_effect=AssertionError("full index")), patch(
                "tos_access.core.build_knowledge_graph", side_effect=AssertionError("full graph")), patch(
                "tos_access.search_read_model.SQLiteKnowledgeSearchReadModel._snapshot_digest", side_effect=AssertionError("whole digest")):
            service = PublishedExplorationService(self.reader, work_limit=2)
            first = service.explore({"focus_node_id": "node:00", "max_depth": 3})
            request = {"cursor": first["page"]["next_cursor"]}
            result = service.explore(request)
            self.assertEqual(service.explore(request), result)
        self.assertFalse(any(" OFFSET " in sql.upper() or "COUNT(" in sql.upper() for sql in statements))
        self.assertFalse(any("key='knowledge_catalog'" in sql or "key='knowledge_top'" in sql for sql in statements))
        self.assertFalse(any(sql.startswith(("INSERT", "UPDATE", "CREATE", "DELETE")) for sql in statements))

    def test_expiry_eviction_restart_and_snapshot_conflicts(self):
        now = [0]
        service = PublishedExplorationService(self.reader, clock=lambda: now[0], ttl=10, work_limit=1)
        first = service.explore({"focus_node_id": "node:00"})
        request = {"cursor": first["page"]["next_cursor"]}
        second = service.explore(request)
        self.assertEqual(service.explore(request), second)
        with self.assertRaises(ExplorationExpired):
            PublishedExplorationService(self.reader).explore(request)
        self.assertFalse(service.capability()["restart_survival"])
        now[0] = 10
        with self.assertRaises(ExplorationExpired):
            service.explore(request)
        first = service.explore({"focus_node_id": "node:00"})
        request = {"cursor": first["page"]["next_cursor"]}
        second = service.explore(request)
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("UPDATE edge_meta SET json_chunk=json_chunk WHERE key='data_revision'")
            db.commit()
        with self.assertRaises(KnowledgeRevisionConflict):
            service.explore(request)  # Cached replay still checks the serving epoch.

    def test_failed_page_does_not_consume_its_cursor(self):
        service = PublishedExplorationService(self.reader, work_limit=2)
        first = service.explore({"focus_node_id": "node:00", "max_depth": 3})
        request = {"cursor": first["page"]["next_cursor"]}
        before = copy.deepcopy(service.records)
        normal = self.reader.limits
        self.reader.limits = PublishedReadLimits(max_rows=1)
        with self.assertRaises(PublishedReadBudgetExceeded):
            service.explore(request)
        self.assertEqual(service.records, before)
        self.reader.limits = normal
        self.assertEqual(service.explore(request), service.explore(request))

    def test_ambiguous_focus_bad_origin_and_cursor_are_explicit(self):
        service = PublishedExplorationService(self.reader)
        with self.assertRaisesRegex(ValueError, "ambiguous"):
            service.explore({"focus_node_id": "ambiguous"})
        with self.assertRaises(ValueError):
            service.explore({"focus_node_id": "unknown"})
        for value in ({"cursor": "bad"}, {"cursor": "a" * 64, "profile": "all"}):
            with self.assertRaises(ValueError):
                service.explore(value)
        with self.assertRaises(KnowledgeRevisionConflict):
            service.explore({"schema_version": "tos_exploration_request_v2", "source_revision": "f" * 64,
                             "origin": {"kind": "node", "id": "node:00", "content_revision": self.graph["nodes"][0]["content_revision"]}})

    def test_doubled_unrelated_population_and_degree_keep_first_page_reads_bounded(self):
        observations = []
        original = _Read.__init__
        def capture(read, *args, **kwargs):
            original(read, *args, **kwargs)
            observations.append(read)
        with closing(sqlite3.connect(self.path)) as db:
            for start in (0, 5000):
                db.executemany("INSERT INTO knowledge_nodes VALUES (?,?,?,?,?,?,?,?,?,?)",
                    ((f"zzz-node:{i}", f"tos.unrelated.{i}", f"unrelated-{i}", "philosophy", "kind", "type", "", "", "", "{}")
                     for i in range(start, start + 5000)))
                db.executemany("INSERT INTO knowledge_relations VALUES (?,?,?,?,?,?,?,?,?,?,?)",
                    ((f"zzz-edge:{i}", f"edge-{i}", "philosophy", "node:00", "node:01", "related", "type", "", "", "", "{}")
                     for i in range(start, start + 5000)))
                db.commit()
                with patch.object(_Read, "__init__", capture):
                    result = PublishedExplorationService(self.reader, work_limit=1, block_size=1).explore(
                        {"focus_node_id": "node:00", "profile": "all"})
                self.assertEqual(result["status"], "paused")
        self.assertEqual(observations[0].rows, observations[1].rows)
        self.assertEqual(observations[0].bytes, observations[1].bytes)
        self.assertLessEqual(observations[1].steps, observations[0].steps + 200)

    def persistent(self, **options):
        return PublishedExplorationService(self.reader, checkpoint_path=self.path.parent / "checkpoints.sqlite",
                                           work_limit=2, **options)

    def test_core_prepared_exploration_routes_without_fallback_and_persists_explicitly(self):
        checkpoint = self.path.parent / "core-checkpoints.sqlite"
        before = hashlib.sha256(self.path.read_bytes()).hexdigest()
        def core(**options):
            return ToSAccessCore.discover(self.root, published_read_model_path=self.path,
                                         published_read_model_expected=self.binding, **options)
        with patch("tos_access.core.build_knowledge_graph", side_effect=AssertionError("full graph")), patch(
                "tos_access.search_read_model.SQLiteKnowledgeSearchReadModel._snapshot_digest", side_effect=AssertionError("whole digest")):
            default = core()
            self.assertIs(default._exploration.reader, default._prepared_reader)
            self.assertFalse(default.knowledge_exploration_contracts()["capabilities"]["restart_survival"])
            self.assertFalse(checkpoint.exists())
            selected = core(published_exploration_checkpoint_path=checkpoint)
            capabilities = selected.knowledge_exploration_contracts()["capabilities"]
            self.assertTrue(capabilities["available"])
            self.assertTrue(capabilities["restart_survival"])
            self.assertEqual(capabilities["storage"], "owner-selected-private-sqlite")
            first = selected.knowledge_explore({"focus_node_id": "node:00", "profile": "all",
                                               "max_depth": 3, "page_nodes": 1, "page_relations": 1})
            request = {"cursor": first["page"]["next_cursor"]}
            result = selected.knowledge_explore(request)
            restarted = core(published_exploration_checkpoint_path=checkpoint)
            self.assertEqual(restarted.knowledge_explore(request), result)
        self.assertEqual(hashlib.sha256(self.path.read_bytes()).hexdigest(), before)
        with self.assertRaises(ValueError):
            ToSAccessCore.discover(self.root, published_exploration_checkpoint_path=checkpoint)

    def test_persistent_restart_concurrent_replay_and_successor_are_atomic(self):
        first = self.persistent().explore({"focus_node_id": "node:00", "max_depth": 3})
        request = {"cursor": first["page"]["next_cursor"]}
        services = [self.persistent(), self.persistent()]
        with concurrent.futures.ThreadPoolExecutor(2) as pool:
            packets = list(pool.map(lambda service: service.explore(request), services))
        self.assertEqual(packets[0], packets[1])
        self.assertEqual(self.persistent().explore(request), packets[0])
        self.assertTrue(services[0].capability()["restart_survival"])
        self.assertEqual((self.path.parent / "checkpoints.sqlite").stat().st_mode & 0o777, 0o600)
        following = {"cursor": packets[0]["page"]["next_cursor"]}
        program = """import json,sys
from unittest.mock import patch
from tos_access.published_read_model import PublishedKnowledgeReadModel
from tos_access.published_exploration import PublishedExplorationService
from tos_access.exploration import ExplorationService
reader = PublishedKnowledgeReadModel(sys.argv[1], json.loads(sys.argv[2]))
service = PublishedExplorationService(reader, checkpoint_path=sys.argv[3], work_limit=2)
with patch.object(ExplorationService, '_index', side_effect=AssertionError('cold full graph')):
    print(json.dumps(service.explore(json.loads(sys.argv[4]))))
"""
        def cold_process(_):
            run = subprocess.run([sys.executable, "-c", program, str(self.path), json.dumps(self.binding),
                                  str(services[0].checkpoints.path), json.dumps(following)],
                                 check=True, capture_output=True, text=True, timeout=10)
            return json.loads(run.stdout)
        with concurrent.futures.ThreadPoolExecutor(2) as pool:
            cold_packets = list(pool.map(cold_process, range(2)))
        self.assertEqual(cold_packets[0], cold_packets[1])
        self.assertEqual(services[0].explore(following), cold_packets[0])

    def test_persistent_failure_between_replay_and_successor_rolls_back(self):
        service = self.persistent()
        first = service.explore({"focus_node_id": "node:00", "max_depth": 3})
        request = {"cursor": first["page"]["next_cursor"]}
        original = _Transaction.put
        calls = []
        def fail_after_first(transaction, *args):
            calls.append(1)
            if len(calls) == 2:
                raise RuntimeError("simulated crash before successor commit")
            return original(transaction, *args)
        with closing(sqlite3.connect(service.checkpoints.path)) as db:
            before = db.execute("SELECT * FROM checkpoints ORDER BY token").fetchall()
        with patch.object(_Transaction, "put", fail_after_first), self.assertRaisesRegex(RuntimeError, "simulated crash"):
            service.explore(request)
        with closing(sqlite3.connect(service.checkpoints.path)) as db:
            self.assertEqual(before, db.execute("SELECT * FROM checkpoints ORDER BY token").fetchall())
        result = self.persistent().explore(request)
        self.assertEqual(self.persistent().explore(request), result)

    def test_persistent_expiry_clock_rollback_and_revoked_replay(self):
        now = [1000]
        service = self.persistent(clock=lambda: now[0], ttl=10)
        first = service.explore({"focus_node_id": "node:00"})
        request = {"cursor": first["page"]["next_cursor"]}
        result = service.explore(request)
        now[0] = 999
        with self.assertRaises(PublishedCheckpointClockRollback):
            service.explore(request)
        now[0] = 1001
        self.assertEqual(service.explore(request), result)
        now[0] = 1010
        with self.assertRaises(ExplorationExpired):
            service.explore(request)
        first = service.explore({"focus_node_id": "node:00"})
        request = {"cursor": first["page"]["next_cursor"]}
        service.explore(request)
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("UPDATE edge_meta SET json_chunk=json_chunk WHERE key='data_revision'")
            db.commit()
        with self.assertRaises(KnowledgeRevisionConflict):
            self.persistent(clock=lambda: now[0], ttl=10).explore(request)

    def test_persistent_busy_schema_mismatch_and_source_path_fail_closed(self):
        service = self.persistent()
        first = service.explore({"focus_node_id": "node:00"})
        request = {"cursor": first["page"]["next_cursor"]}
        with closing(sqlite3.connect(service.checkpoints.path)) as db:
            db.execute("BEGIN IMMEDIATE")
            with self.assertRaisesRegex(PublishedCheckpointError, "busy"):
                service.explore(request)
            db.rollback()
        self.assertEqual(service.explore(request), self.persistent().explore(request))
        with self.assertRaisesRegex(PublishedCheckpointError, "incompatible"):
            self.persistent(ttl=1)
        with closing(sqlite3.connect(service.checkpoints.path)) as db:
            db.execute("UPDATE checkpoint_meta SET config='wrong-version'")
            db.commit()
        with self.assertRaises(PublishedCheckpointError):
            service.explore(request)
        with self.assertRaises(PublishedCheckpointError):
            PublishedExplorationService(self.reader, checkpoint_path=self.path)
        alias = self.path.parent / "source-link.sqlite"
        alias.symlink_to(self.path)
        with self.assertRaises(PublishedCheckpointError):
            PublishedExplorationService(self.reader, checkpoint_path=alias)

    def test_persistent_bounded_capacity_refuses_dangling_successor_and_reuses_pages(self):
        service = self.persistent(max_checkpoints=1, max_bytes=20000)
        first = service.explore({"focus_node_id": "node:00", "max_depth": 3})
        request = {"cursor": first["page"]["next_cursor"]}
        with self.assertRaisesRegex(ExplorationExpired, "together"):
            service.explore(request)
        with closing(sqlite3.connect(service.checkpoints.path)) as db:
            raw = db.execute("SELECT raw FROM checkpoints WHERE token=?", (request["cursor"],)).fetchone()[0]
            self.assertIn("state", json.loads(raw))
        for _ in range(30):
            service.explore({"focus_node_id": "node:00"})
        with closing(sqlite3.connect(service.checkpoints.path)) as db:
            count, size = db.execute("SELECT count(*),sum(length(raw)) FROM checkpoints").fetchone()
            self.assertLessEqual(count, 1)
            self.assertLessEqual(size, 20000)
            self.assertEqual(db.execute("PRAGMA journal_mode").fetchone()[0], "delete")
        self.assertLessEqual(service.checkpoints.path.stat().st_size, service.capability()["database_byte_cap"])
        self.assertFalse(Path(str(service.checkpoints.path) + "-wal").exists())

    def test_memory_and_persistent_pair_admission_have_equal_capacity_refusal(self):
        query = {"focus_node_id": "node:00", "max_depth": 3}
        baseline = PublishedExplorationService(self.reader, work_limit=2)
        first = baseline.explore(query)
        baseline.explore({"cursor": first["page"]["next_cursor"]})
        sizes = [len(raw) for _, raw in baseline.records.values()]
        self.assertEqual(len(sizes), 2)
        byte_cap = max(sizes)
        self.assertLess(byte_cap, sum(sizes))
        for persistent in (False, True):
            for name, limits in (("count", {"max_checkpoints": 1}), ("bytes", {"max_bytes": byte_cap})):
                with self.subTest(persistent=persistent, limit=name):
                    service = PublishedExplorationService(self.reader, work_limit=2, **limits,
                        checkpoint_path=self.path.parent / f"capacity-{name}.sqlite" if persistent else None)
                    first = service.explore(query)
                    request = {"cursor": first["page"]["next_cursor"]}
                    if persistent:
                        with closing(sqlite3.connect(service.checkpoints.path)) as db:
                            before = db.execute("SELECT * FROM checkpoints ORDER BY token").fetchall()
                    else:
                        before = copy.deepcopy(service.records), service.stored_bytes
                    with self.assertRaisesRegex(ExplorationExpired, "together"):
                        service.explore(request)
                    if persistent:
                        with closing(sqlite3.connect(service.checkpoints.path)) as db:
                            self.assertEqual(before, db.execute("SELECT * FROM checkpoints ORDER BY token").fetchall())
                    else:
                        self.assertEqual(before, (service.records, service.stored_bytes))


if __name__ == "__main__":
    unittest.main()
