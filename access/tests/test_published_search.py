"""Actual-file compressed search joins, transport continuation and read bounds."""
import base64
import copy
from contextlib import closing
import json
import os
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

from tos_access import knowledge as k
from tos_access.compressed_search_store import (
    SearchStore, SearchInvalidRequest, SearchStaleBinding, SearchCursorError,
    SearchCursorExpired, SearchUnavailable, SearchBudgetExceeded,
)
from tos_access.prepared_publication import (
    publish_prepared, apply_prepared_delta, PublicationLimits, PreparedChange,
)
from tos_access.published_read_metadata import _compact, published_row_digest_key
from tos_access.published_read_model import PublishedKnowledgeReadModel, PublishedReadLimits
from tos_access.published_search import PublishedSearchService, PublishedSearchLimits, _decode, MAX_CURSOR_BYTES
from test_prepared_publication import fixture


class PublishedSearchTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="published-search-", dir=os.environ.get("TMPDIR"))
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / "published.sqlite"
        self.graph, self.catalog = fixture()

    def publish(self, *, read_limits=None, search_limits=None, publish_limits=None):
        self.binding = publish_prepared(self.path, graph=self.graph, catalog=self.catalog, limits=publish_limits)
        self.reader = PublishedKnowledgeReadModel(self.path, self.binding, limits=read_limits)
        return PublishedSearchService(self.reader, limits=search_limits)

    def drain(self, service, query="", *, limit=2, **filters):
        cursor, all_nodes, all_relations, all_ranks, pages = None, [], [], {"nodes": [], "relations": []}, []
        for _ in range(2000):
            page = service.search(query, cursor=cursor, limit=limit, **filters)
            pages.append(page)
            all_nodes.extend(page["nodes"])
            all_relations.extend(page["relations"])
            for kind in ("nodes", "relations"):
                all_ranks[kind].extend(page["ranks"][kind])
                self.assertLessEqual(page["work"][kind]["inner_pages"], 1)
                self.assertEqual(page["counts"]["returned_" + kind], len(page[kind]))
                if cursor is not None:
                    self.assertIsNone(page["counts"]["matching_" + kind])
            self.assertLessEqual(len(_compact(page).encode("utf-8")), self.reader.limits.max_response_bytes)
            self.assertLessEqual(page["work"]["read_bytes"], self.reader.limits.max_response_bytes)
            if not page["page"]["has_more"]:
                self.assertIsNone(page["page"]["next_cursor"])
                return all_nodes, all_relations, all_ranks, pages
            self.assertNotEqual(cursor, page["page"]["next_cursor"])
            cursor = page["page"]["next_cursor"]
            self.assertLessEqual(len(cursor), MAX_CURSOR_BYTES)
            # A fresh service/reader each page: no private cursor state survives.
            service = PublishedSearchService(PublishedKnowledgeReadModel(self.path, self.binding, limits=self.reader.limits), limits=service.limits)
        self.fail("bounded synthetic search did not drain")

    def test_native_rank_full_identity_order_and_filters_without_cold_graph(self):
        self.graph["nodes"][0]["display"]["title"] = {"en": "common", "ru": "Слово"}
        self.graph["nodes"][1]["display"]["title"] = {"en": "common prefix"}
        self.graph["nodes"][2]["kind_id"] = "other"
        self.graph["nodes"][2]["display"]["title"] = {"en": "Unicode İ Σ ß e\u0301"}
        service = self.publish()
        with patch("tos_access.core.ToSAccessCore.knowledge_graph", side_effect=AssertionError("cold graph")), \
             patch("tos_access.prepared_publication.publish_prepared", side_effect=AssertionError("publisher")):
            for query in ("", "common", "a", "A-native", '"key": "value"', "false", "Слово", "İ", "ß", "e\u0301", "no-match"):
                for filters in ({}, {"kind_ids": ["other", ""]}, {"sources": ["philosophy"]}, {"sources": ["canon"]}, {"sources": [""]}, {"kind_ids": [""]}, {"predicate_ids": ["missing"]}):
                    with self.subTest(query=query, filters=filters):
                        nodes, relations, ranks, _ = self.drain(service, query, **filters)
                        expected = k.search_knowledge_graph(self.graph, query, **filters, limit=100)
                        self.assertEqual(nodes, expected["nodes"])
                        self.assertEqual(relations, expected["relations"])
                        for kind, rows in (("nodes", nodes), ("relations", relations)):
                            expected_ranks = [k._knowledge_search_rank(row, query.strip().lower(), relation=kind == "relations")[0] for row in rows]
                            self.assertEqual([row["rank"] for row in ranks[kind]], expected_ranks)
                            self.assertTrue(all(row["explanation"] for row in ranks[kind]))

    def test_request_validation_precedes_any_database_open(self):
        service = self.publish()
        with patch.object(self.reader, "_connect", side_effect=AssertionError("opened DB")):
            for kwargs in ({"query": 12}, {"query": "x" * 257}, {"query": "İ" * 256},
                           {"kind_ids": "concept"}, {"kind_ids": [False]}, {"predicate_ids": [None]},
                           {"sources": ["unknown"]}, {"sources": [1]}, {"limit": True}, {"limit": 0},
                           {"kind_ids": ["x" * 65536]}, {"cursor": "!"}, {"cursor": "a" * (MAX_CURSOR_BYTES + 1)}):
                with self.subTest(kwargs=kwargs), self.assertRaises((SearchInvalidRequest, SearchCursorError)):
                    service.search(**kwargs)

    def test_body_deferral_is_lossless_and_does_not_repeat_inner_search(self):
        node = copy.deepcopy(self.graph["nodes"][0])
        relation = copy.deepcopy(self.graph["relations"][0])
        self.graph["nodes"] = [{**copy.deepcopy(node), "id": chr(97 + i) * 4096,
                                "entity_id": str(i), "native_id": str(i), "wide": "x" * 85000} for i in range(5)]
        self.graph["relations"] = [{**copy.deepcopy(relation), "id": f"r{i}",
                                     "from_id": self.graph["nodes"][0]["id"],
                                     "to_id": self.graph["nodes"][1]["id"], "wide": "x" * 75000} for i in range(5)]
        service = self.publish(read_limits=PublishedReadLimits(max_row_bytes=131072),
                               search_limits=PublishedSearchLimits(body_bytes=262144),
                               publish_limits=PublicationLimits(max_row_bytes=131072))
        nodes, relations, _, pages = self.drain(service, limit=5)
        self.assertEqual(nodes, self.graph["nodes"])
        self.assertEqual(relations, self.graph["relations"])
        self.assertGreater(len(pages), 4)
        self.assertEqual(len(pages[0]["nodes"]), 1)
        self.assertEqual(len(pages[0]["relations"]), 1)
        outer, _ = _decode(pages[0]["page"]["next_cursor"])
        self.assertEqual(len(outer["nodes"]["pending"]), 4)
        self.assertEqual(len(outer["nodes"]["pending"][0][2]), 64)
        self.assertNotIn("a" * 4096, pages[0]["page"]["next_cursor"])
        for page in pages[1:5]:
            self.assertEqual(page["work"]["nodes"]["inner_pages"], 0)
            self.assertEqual(page["work"]["relations"]["inner_pages"], 0)

    def test_empty_work_pages_and_long_text_resume_without_lost_body(self):
        self.graph["nodes"][0]["wide"] = "x" * 100000 + "needle"
        self.graph["nodes"][1]["wide"] = "x" * 100000 + "needXle"
        service = self.publish(search_limits=PublishedSearchLimits(candidate_budget=2, verification_bytes=8192))
        nodes, relations, _, pages = self.drain(service, "needle")
        self.assertEqual(nodes, [self.graph["nodes"][0]])
        self.assertEqual(relations, [])
        self.assertGreater(sum(not page["nodes"] and page["page"]["has_more"] for page in pages), 10)
        for page in pages:
            for kind in ("nodes", "relations"):
                work = page["work"][kind]["search"]
                if work:
                    self.assertLessEqual(work["operations"], 2)
                    self.assertLessEqual(work["verification_bytes"], 8192)

    def test_fetch_byte_budget_defers_before_loading_full_carriers(self):
        node = self.graph["nodes"][0]
        self.graph["nodes"] = [{**copy.deepcopy(node), "id": f"n{i}", "entity_id": f"e{i}",
                                "native_id": "n" * 250000, "wide": "x" * 500000} for i in range(10)]
        self.graph["relations"] = []
        service = self.publish(search_limits=PublishedSearchLimits(body_bytes=14 * 1024 * 1024))
        first = service.search(limit=20)
        self.assertTrue(first["work"]["nodes"]["deferred"])
        # Another body still fits the output allowance: duplicate indexed/native
        # identity fetches, not output truncation, forced the continuation.
        one = len(_compact(self.graph["nodes"][0]).encode())
        self.assertLess(first["work"]["nodes"]["body_bytes"] + one, service.body_per_kind)
        self.assertLess(len(first["nodes"]), len(self.graph["nodes"]))
        next_page = service.search(cursor=first["page"]["next_cursor"], limit=20)
        self.assertEqual(first["nodes"] + next_page["nodes"], self.graph["nodes"])
        self.assertEqual(next_page["work"]["nodes"]["inner_pages"], 0)
        self.assertFalse(next_page["page"]["has_more"])

    def test_mapping_and_selected_checksum_tampering_fail_closed(self):
        service = self.publish()
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("UPDATE prepared_documents SET id='foreign' WHERE kind='node' AND doc_id=1")
            db.commit()
        with self.assertRaisesRegex(SearchUnavailable, "exact search identity"):
            service.search(limit=1)
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("UPDATE prepared_documents SET id='a' WHERE kind='node' AND doc_id=1")
            raw = db.execute("SELECT json FROM knowledge_nodes WHERE id='a'").fetchone()[0]
            db.execute("UPDATE knowledge_nodes SET json=? WHERE id='a'", (raw.replace("common", "broken"),))
            db.commit()
        with self.assertRaisesRegex(SearchUnavailable, "checksum"):
            service.search(limit=1)

    def test_mapping_kind_and_full_row_index_closure_are_checked(self):
        service = self.publish()
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("UPDATE prepared_documents SET kind='relation' WHERE doc_id=1")
            db.commit()
        with self.assertRaisesRegex(SearchUnavailable, "address mapping"):
            service.search(limit=1)
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("UPDATE prepared_documents SET kind='node' WHERE doc_id=1")
            db.execute("UPDATE knowledge_nodes SET native_id='wrong' WHERE id='a'")
            db.commit()
        with self.assertRaisesRegex(SearchUnavailable, "identity/index"):
            service.search(limit=1)

    def test_hmac_query_expiry_restart_and_incarnation_rejection(self):
        service = self.publish()
        first = service.search(limit=1)
        cursor = first["page"]["next_cursor"]
        restarted = PublishedSearchService(PublishedKnowledgeReadModel(self.path, self.binding))
        self.assertEqual(restarted.search(cursor=cursor, limit=1)["nodes"], [self.graph["nodes"][1]])
        with self.assertRaises(SearchCursorError):
            restarted.search("other", cursor=cursor)
        raw = json.loads(base64.urlsafe_b64decode(cursor + "=" * (-len(cursor) % 4)))
        raw["state"]["nodes"]["pending"] = [[99, 3, "a" * 64]]
        tampered = base64.urlsafe_b64encode(_compact(raw).encode()).decode().rstrip("=")
        with self.assertRaises(SearchCursorError):
            restarted.search(cursor=tampered)
        state, _ = _decode(cursor)
        with patch("tos_access.published_search.time.time", return_value=state["expires"]):
            with self.assertRaises(SearchCursorExpired):
                restarted.search(cursor=cursor)
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("UPDATE search_header SET cursor_key=randomblob(32)")
            db.commit()
        with self.assertRaisesRegex(SearchCursorError, "integrity"):
            restarted.search(cursor=cursor)

    def test_restart_binding_dictionary_order_does_not_change_cursor_identity(self):
        service = self.publish()
        first = service.search(limit=1)
        def reordered(value):
            if isinstance(value, dict):
                return {key: reordered(value[key]) for key in reversed(value)}
            return value
        rebound = reordered(self.binding)
        self.assertEqual(rebound, self.binding)
        self.assertNotEqual(_compact(rebound), _compact(self.binding))
        restarted = PublishedSearchService(PublishedKnowledgeReadModel(self.path, rebound))
        expected = service.search(cursor=first["page"]["next_cursor"], limit=1)
        actual = restarted.search(cursor=first["page"]["next_cursor"], limit=1)
        self.assertEqual(actual["nodes"], expected["nodes"])
        self.assertEqual(actual["relations"], expected["relations"])
        self.assertEqual(actual["page"]["next_cursor"], expected["page"]["next_cursor"])

    def delta(self):
        header = {key: copy.deepcopy(value) for key, value in self.graph.items() if key not in ("nodes", "relations")}
        header["source_revision"] = "c" * 64
        catalog = {**self.catalog, "source_revision": header["source_revision"]}
        changed = {**self.graph["nodes"][0], "probe": {"new": "atomic"}}
        return apply_prepared_delta(self.path, expected_binding=self.binding, source_header=header,
                                   catalog=catalog, changes=[PreparedChange("update", "node", "a", changed)])

    def test_stale_selected_binding_and_continuation_are_rejected(self):
        service = self.publish()
        cursor = service.search(limit=1)["page"]["next_cursor"]
        new_binding = self.delta()
        with self.assertRaises(SearchStaleBinding):
            service.search()
        current = PublishedSearchService(PublishedKnowledgeReadModel(self.path, new_binding))
        with self.assertRaises(SearchStaleBinding):
            current.search(cursor=cursor)
        self.assertEqual(current.search("atomic")["nodes"][0]["id"], "a")

    def test_one_connection_search_and_body_join_reject_concurrent_publication(self):
        service = self.publish()
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("PRAGMA journal_mode=WAL")
        original = service._kind
        triggered = []
        def interleave(*args, **kwargs):
            result = original(*args, **kwargs)
            if not triggered:
                triggered.append(True)
                self.delta()
            return result
        with patch.object(service, "_kind", side_effect=interleave), \
             patch.object(self.reader, "_connect", wraps=self.reader._connect) as connect:
            with self.assertRaises(SearchStaleBinding):
                service.search()
            self.assertEqual(connect.call_count, 1)

    def test_capability_checks_selected_header_and_search_indexes_without_catalog(self):
        service = self.publish()
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("DELETE FROM edge_meta WHERE key='knowledge_catalog'")
            db.commit()
        self.assertTrue(service.capability()["available"])
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("DROP INDEX search_blocks_nonempty")
            db.commit()
        with self.assertRaises(SearchUnavailable):
            service.capability()

    def test_legacy_profiles_refused_before_database_and_no_budget_inflation(self):
        service = self.publish()
        for schema in ("tos_cloudflare_edge_read_model_v8", "tos_cloudflare_edge_read_model_v9"):
            binding = {**self.binding, "read_model_schema": schema}
            legacy = PublishedSearchService(PublishedKnowledgeReadModel(self.path, binding))
            with patch.object(legacy.reader, "_connect", side_effect=AssertionError("opened legacy DB")):
                self.assertFalse(legacy.capability()["available"])
                with self.assertRaises(SearchUnavailable):
                    legacy.search()
        with self.assertRaises(SearchInvalidRequest):
            PublishedSearchService(self.reader, limits=PublishedSearchLimits(body_bytes=1))
        tight = PublishedSearchService(PublishedKnowledgeReadModel(self.path, self.binding,
            limits=PublishedReadLimits(max_rows=100)))
        with self.assertRaises(SearchBudgetExceeded):
            tight.search()


if __name__ == "__main__":
    unittest.main()
