"""Exact temporal comparison through actual selected prepared SQLite rows."""
import asyncio
from contextlib import closing, redirect_stdout
from dataclasses import replace
from http.client import HTTPConnection
from http.server import ThreadingHTTPServer
import io
import json
from pathlib import Path
import sqlite3
import tempfile
from threading import Thread
import unittest
from unittest.mock import patch

from tos_access.cli import main
from tos_access.compressed_search_store import SearchInvalidRequest
from tos_access.core import ToSAccessCore
from tos_access.http_server import build_handler
from tos_access.lens_pagination import KnowledgeRevisionConflict
from tos_access.mcp_server import build_server
from tos_access.prepared_publication import publish_prepared
from tos_access.published_read_metadata import _compact, emitted_row_digest, published_row_digest_key
from tos_access.published_read_model import (
    PublishedKnowledgeReadModel, PublishedReadLimits, PublishedReadModelError,
    PublishedReadBudgetExceeded, PublishedSnapshotConflict, _Read,
)
from tos_access.temporal_comparison import compare_temporal_claims, TemporalReadModelInvalid
import test_temporal_comparison as temporal_reference


class PublishedTemporalTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        temporal_reference.TemporalComparisonTests.setUpClass()
        cls.reference = temporal_reference.TemporalComparisonTests()

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="published-temporal-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.publications = 0
        self.graph, self.index, self.request = self.reference.fixture()
        self.select(self.graph)

    def select(self, graph, request=None):
        if request is not None:
            # Retain unchanged native normalized operands, not the fixture's
            # unrelated corpus. The reference still compares its full graph.
            identifiers = {request[side]["node_id"] for side in ("left", "right")}
            for node in graph["nodes"]:
                if node["id"] in identifiers:
                    claim = node.get("semantics", {}).get("claim", {})
                    identifiers.update(claim[key] for key in ("subject_node_id", "object_node_id")
                                       if isinstance(claim.get(key), str))
            graph = {**graph, "nodes": [node for node in graph["nodes"] if node["id"] in identifiers],
                     "relations": []}
        self.publications += 1
        self.path = self.root / f"publication-{self.publications}.sqlite"
        catalog = {"schema": "tos_knowledge_catalog_v1", "source_revision": graph["source_revision"], "lenses": []}
        self.binding = publish_prepared(self.path, graph=graph, catalog=catalog)
        self.reader = PublishedKnowledgeReadModel(self.path, self.binding)
        self.core = ToSAccessCore.discover(self.root, published_read_model_path=self.path,
                                          published_read_model_expected=self.binding)

    def expected(self):
        return compare_temporal_claims(self.graph, self.request, graph_index=self.index)

    def rewrite(self, identifier, raw, *, digest=True):
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("UPDATE knowledge_nodes SET json=? WHERE id=?", (raw, identifier))
            if digest:
                db.execute("UPDATE edge_meta SET json_chunk=? WHERE key=? AND part=0",
                           (_compact(emitted_row_digest(raw)), published_row_digest_key("node", identifier)))
            db.commit()

    def test_exact_reference_packet_one_snapshot_bounded_rows_and_no_source_fallback(self):
        calls, connections = [], []
        original_items, original_connect = _Read.items, self.reader._connect
        def items(read, kind, selector, args, limit, **kwargs):
            calls.append((kind, selector, args, limit))
            return original_items(read, kind, selector, args, limit, **kwargs)
        def connect():
            result = original_connect()
            connections.append(result)
            return result
        with patch.object(_Read, "items", items), patch.object(self.reader, "_connect", connect):
            self.assertEqual(self.reader.temporal_compare(self.request), self.expected())
        self.assertEqual(len(connections), 1)
        self.assertEqual(len(calls), 4)
        self.assertTrue(all((kind, selector, limit) == ("node", "id=?", 1) for kind, selector, _, limit in calls))
        with patch.object(ToSAccessCore, "knowledge_graph", side_effect=AssertionError("source fallback")), \
                patch("tos_access.core._read_json", side_effect=AssertionError("source read")), \
                patch("tos_access.core.build_knowledge_graph", side_effect=AssertionError("normalization")):
            self.assertEqual(self.core.knowledge_temporal_compare(self.request), self.expected())

    def test_document_native_packets_and_canonical_number_types(self):
        for name, (graph, index, request) in self.reference.document_transport_fixtures():
            with self.subTest(name=name):
                if name == "document-subject-duplicate":
                    # Prepared identities are unique: duplicate rows must not
                    # be admitted as a silently selected subject.
                    with self.assertRaisesRegex(SearchInvalidRequest,
                            r"UNIQUE constraint failed: prepared_documents.kind, prepared_documents.id"):
                        self.select(graph, request)
                    self.assertFalse(self.path.exists())
                    continue
                self.select(graph, request)
                expected = compare_temporal_claims(graph, request, graph_index=index)
                actual = self.reader.temporal_compare(request)
                self.assertEqual(_compact(actual), _compact(expected))
                if name == "document-native-numbers":
                    claim = actual["left"]["claim"]
                    original = index.node_ids[request["left"]["node_id"]][0]
                    self.assertEqual(claim["semantics"]["claim"]["source_canonical_json"],
                                     original["semantics"]["claim"]["source_canonical_json"])
                    extensions = claim["attributes"]["source_claim"]["object"]["extensions"]
                    self.assertIs(type(extensions["float"]), float)
                    self.assertIs(type(extensions["large_integer"]), int)
                    self.assertEqual(extensions["large_integer"], 9007199254740993)
                    self.assertEqual(_compact(extensions["negative_zero"]), "-0.0")

    def test_source_content_revision_and_exact_identity_errors_are_preserved(self):
        for request in ({**self.request, "source_revision": "0" * 64},
                        {**self.request, "left": {**self.request["left"], "content_revision": "0" * 64}}):
            with self.assertRaises(KnowledgeRevisionConflict):
                self.reader.temporal_compare(request)
        claim = self.index.node_ids[self.request["left"]["node_id"]][0]
        for identifier in (claim["native_id"], claim["entity_id"]):
            with self.assertRaises(KeyError):
                self.reader.temporal_compare({**self.request, "left": {**self.request["left"], "node_id": identifier}})
        with self.assertRaises(ValueError):
            self.reader.temporal_compare({**self.request, "left": {**self.request["left"], "node_id": " " + claim["id"]}})
        with self.assertRaises(ValueError):
            self.reader.temporal_compare({**self.request, "calendar": "gregorian"})

    def test_checked_json_and_damaged_semantic_containers_refuse(self):
        identifier = self.request["left"]["node_id"]
        claim = self.index.node_ids[identifier][0]
        for raw, digest, error in (("{bad", True, PublishedReadModelError),
                (_compact(claim)[:-1] + ',"id":"duplicate"}', True, PublishedReadModelError),
                (_compact(claim)[:-1] + ',"overflow":1e309}', True, PublishedReadModelError),
                (_compact({**claim, "unchecked": True}), False, PublishedReadModelError),
                (_compact({**claim, "semantics": None}), True, TemporalReadModelInvalid)):
            with self.subTest(error=error, raw=raw[-50:]):
                self.rewrite(identifier, raw, digest=digest)
                with self.assertRaises(error):
                    self.reader.temporal_compare(self.request)

    def test_duplicate_exact_rows_in_damaged_schema_refuse(self):
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("CREATE TABLE duplicate_rows AS SELECT * FROM knowledge_nodes")
            db.execute("INSERT INTO duplicate_rows SELECT * FROM knowledge_nodes WHERE id=?", (self.request["left"]["node_id"],))
            db.execute("DROP TABLE knowledge_nodes")
            db.execute("ALTER TABLE duplicate_rows RENAME TO knowledge_nodes")
            for name, columns in (("knowledge_nodes_native_idx", "native_id"),
                                  ("knowledge_nodes_entity_idx", "entity_id"),
                                  ("knowledge_nodes_identity_seek", "id")):
                db.execute(f"CREATE INDEX {name} ON knowledge_nodes({columns})")
            db.commit()
        with self.assertRaises(PublishedReadBudgetExceeded):
            self.reader.temporal_compare(self.request)

    def test_operand_row_budget_is_explicit(self):
        # Allow the small header but not the full, source-preserving Claim.
        reader = PublishedKnowledgeReadModel(self.path, self.binding,
                    limits=replace(PublishedReadLimits(), max_row_bytes=1024))
        with self.assertRaises(PublishedReadBudgetExceeded):
            reader.temporal_compare(self.request)

    def test_post_read_epoch_guard_rejects_concurrent_publication(self):
        from tos_access import temporal_comparison
        original = temporal_comparison.compare_temporal_operands
        with closing(sqlite3.connect(self.path)) as db:
            self.assertEqual(db.execute("PRAGMA journal_mode=WAL").fetchone()[0], "wal")
        def compare(*args):
            result = original(*args)
            with closing(sqlite3.connect(self.path)) as db:
                db.execute("UPDATE knowledge_exploration_clock SET epoch=epoch+1")
                db.commit()
            return result
        with patch.object(temporal_comparison, "compare_temporal_operands", compare):
            with self.assertRaises(PublishedSnapshotConflict):
                self.reader.temporal_compare(self.request)

    def test_actual_cli_http_and_native_mcp_use_prepared_packet_and_errors(self):
        expected = self.expected()
        binding_path = self.root / "selected.json"
        binding_path.write_text(json.dumps(self.binding), encoding="utf-8")
        args = ["--root", str(self.root), "--prepared-read-model", str(self.path),
                "--prepared-binding", str(binding_path)]
        server = ThreadingHTTPServer(("127.0.0.1", 0), build_handler(self.core, self.root))
        thread = Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with patch.object(ToSAccessCore, "knowledge_graph", side_effect=AssertionError("source fallback")):
                output = io.StringIO()
                with patch("sys.stdin", io.StringIO(json.dumps(self.request))), redirect_stdout(output):
                    main([*args, "knowledge", "temporal-compare", "-"])
                self.assertEqual(json.loads(output.getvalue()), expected)
                mcp = build_server(core=self.core)
                result = asyncio.run(mcp.call_tool("tos_knowledge_temporal_compare", {"request": self.request}))
                self.assertEqual(result[1], expected)
                cases = [(self.request, 200), ({**self.request, "source_revision": "0" * 64}, 409),
                         ({**self.request, "left": {**self.request["left"], "content_revision": "0" * 64}}, 409),
                         ({**self.request, "left": {**self.request["left"], "node_id": "missing"}}, 404),
                         ({**self.request, "calendar": "gregorian"}, 400)]
                for request, status in cases:
                    with closing(HTTPConnection("127.0.0.1", server.server_port, timeout=5)) as connection:
                        connection.request("POST", "/api/knowledge/temporal/compare", json.dumps(request), {"Content-Type": "application/json"})
                        response = connection.getresponse()
                        packet = json.loads(response.read())
                        self.assertEqual(response.status, status)
                        if status == 200:
                            self.assertEqual(packet, expected)
                for error, status in ((PublishedReadBudgetExceeded, 413), (PublishedSnapshotConflict, 409),
                                      (TemporalReadModelInvalid, 503), (PublishedReadModelError, 503)):
                    with patch.object(PublishedKnowledgeReadModel, "temporal_compare", side_effect=error("fixture refusal")), \
                            closing(HTTPConnection("127.0.0.1", server.server_port, timeout=5)) as connection:
                        connection.request("POST", "/api/knowledge/temporal/compare", json.dumps(self.request), {"Content-Type": "application/json"})
                        response = connection.getresponse()
                        response.read()
                        self.assertEqual(response.status, status)
        finally:
            server.shutdown()
            server.server_close()
            thread.join(timeout=5)


if __name__ == "__main__":
    unittest.main()
