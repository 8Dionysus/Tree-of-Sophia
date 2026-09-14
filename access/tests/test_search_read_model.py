from __future__ import annotations

import tempfile
import unittest
import base64
import json
import gc
import sqlite3
import weakref
from concurrent.futures import ThreadPoolExecutor
from unittest.mock import patch
from pathlib import Path

import sys

ACCESS_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, (ACCESS_ROOT / "src").as_posix())

from tos_access.knowledge import search_knowledge_graph  # noqa: E402
from tos_access.search_read_model import (  # noqa: E402
    SearchReadModelBuildError,
    SearchReadModelError,
    SearchReadModelSnapshotError,
    SearchReadModelUnindexedError,
    SQLiteKnowledgeSearchReadModel,
)


class SearchReadModelTests(unittest.TestCase):
    def test_connection_lifetime_follows_last_reader_and_explicit_close(self):
        with tempfile.TemporaryDirectory() as raw:
            for explicit in (False, True):
                with self.subTest(explicit=explicit):
                    model = SQLiteKnowledgeSearchReadModel.build(
                        self.graph(), Path(raw) / 'search.sqlite', max_bytes=4 * 1024 * 1024)
                    connection = model.connection
                    reference = weakref.ref(model)
                    reader = model
                    del model
                    gc.collect()
                    self.assertIs(reference(), reader)
                    self.assertEqual(len(reader.ranked_page('nodes', 'common').rows), 3)
                    if explicit:
                        reader.close()
                        reader.close()
                        with self.assertRaises(sqlite3.ProgrammingError):
                            connection.execute('SELECT 1')
                    del reader
                    gc.collect()
                    self.assertIsNone(reference())
                    with self.assertRaises(sqlite3.ProgrammingError):
                        connection.execute('SELECT 1')

    @staticmethod
    def graph() -> dict:
        def node(identifier: str, title: str) -> dict:
            return {
                "id": identifier,
                "native_id": identifier,
                "source_graph": "philosophy",
                "kind_id": "concept",
                "display": {
                    "title": {"default": title},
                    "kind_label": {"default": "concept"},
                    "summary": {"default": title},
                },
            }

        return {
            "schema": "tos_knowledge_graph_v1",
            "source_revision": "search-read-model-fixture",
            "authority_boundary": {},
            "nodes": [
                node("node:0", "Straße — common needle"),
                node("node:1", "STRASSE — common needle"),
                node("node:2", "Another common needle"),
            ],
            "relations": [],
        }

    def test_complete_model_has_exact_lower_substring_parity_and_short_grams(self):
        graph = self.graph()
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "search.sqlite"
            model = SQLiteKnowledgeSearchReadModel.build(graph, path, max_bytes=4 * 1024 * 1024)
            try:
                for query in ("", "a", "co", "common", "needle", "SÜ", "strasse", "missing"):
                    expected = search_knowledge_graph(graph, query)
                    page = model.candidate_page("nodes", query, page_size=50)
                    actual_ids = [row["id"] for row in page.rows]
                    expected_ids = [item["id"] for item in expected["nodes"]]
                    if query.lower() == "strasse":
                        self.assertEqual(actual_ids, ["node:1"])
                    elif query.lower() == "missing":
                        self.assertEqual(actual_ids, [])
                    else:
                        self.assertEqual(set(actual_ids), set(expected_ids))
                    self.assertTrue(page.verified_chars >= 0)
            finally:
                model.close()
            with SQLiteKnowledgeSearchReadModel.open(graph, path) as reopened:
                self.assertEqual(
                    [row["id"] for row in reopened.candidate_page("nodes", "needle", page_size=50).rows],
                    ["node:0", "node:1", "node:2"],
                )

    def test_common_query_uses_bounded_position_cursor(self):
        graph = self.graph()
        with tempfile.TemporaryDirectory() as raw:
            model = SQLiteKnowledgeSearchReadModel.build(graph, Path(raw) / "search.sqlite", max_bytes=4 * 1024 * 1024)
            try:
                first = model.candidate_page("nodes", "common", page_size=1)
                self.assertEqual(first.candidate_rows, 1)
                self.assertTrue(first.has_more)
                self.assertIsNotNone(first.next_cursor)
                second = model.candidate_page("nodes", "common", cursor=first.next_cursor, page_size=1)
                self.assertEqual(second.candidate_rows, 1)
                self.assertNotEqual(first.rows[0]["id"], second.rows[0]["id"])
                self.assertIsNotNone(second.next_cursor)
                third = model.candidate_page("nodes", "common", cursor=second.next_cursor, page_size=2)
                self.assertEqual([row["id"] for row in third.rows], ["node:2"])
                self.assertFalse(third.has_more)
            finally:
                model.close()

    def test_lookahead_seed_is_not_returned_or_consumed_by_filtered_page(self):
        graph = self.graph()
        graph["nodes"][2] = {**graph["nodes"][2], "source_graph": "canon"}
        with tempfile.TemporaryDirectory() as raw:
            model = SQLiteKnowledgeSearchReadModel.build(graph, Path(raw) / "search.sqlite", max_bytes=4 * 1024 * 1024)
            try:
                first = model.candidate_page("nodes", "common", sources=["canon"], page_size=2)
                self.assertEqual(first.rows, ())
                self.assertEqual(first.candidate_rows, 2)
                self.assertTrue(first.has_more)
                second = model.candidate_page(
                    "nodes", "common", sources=["canon"], cursor=first.next_cursor, page_size=2
                )
                self.assertEqual([row["id"] for row in second.rows], ["node:2"])
                self.assertFalse(second.has_more)
            finally:
                model.close()

    def test_cursor_is_bound_to_source_revision_and_filters(self):
        graph = self.graph()
        with tempfile.TemporaryDirectory() as raw:
            model = SQLiteKnowledgeSearchReadModel.build(graph, Path(raw) / "search.sqlite", max_bytes=4 * 1024 * 1024)
            try:
                first = model.candidate_page("nodes", "common", page_size=1)
                with self.assertRaises(SearchReadModelSnapshotError):
                    model.candidate_page("nodes", "common", sources=["canon"], cursor=first.next_cursor, page_size=1)
                with self.assertRaises(SearchReadModelSnapshotError):
                    model.candidate_page("relations", "common", cursor=first.next_cursor, page_size=1)
            finally:
                model.close()

    def test_selected_source_document_digest_detects_same_length_mutation(self):
        graph = self.graph()
        with tempfile.TemporaryDirectory() as raw:
            model = SQLiteKnowledgeSearchReadModel.build(graph, Path(raw) / "search.sqlite", max_bytes=4 * 1024 * 1024)
            try:
                graph["nodes"][0]["native_id"] = "node:X"
                with self.assertRaises(SearchReadModelSnapshotError):
                    model.candidate_page("nodes", "common", page_size=1)
            finally:
                model.close()

    def test_opaque_lone_surrogate_is_kept_as_a_searchable_json_carrier(self):
        graph = self.graph()
        graph["nodes"][0]["attributes"] = {"opaque": "x" + chr(0xD800) + "y"}
        with tempfile.TemporaryDirectory() as raw:
            model = SQLiteKnowledgeSearchReadModel.build(graph, Path(raw) / "search.sqlite", max_bytes=4 * 1024 * 1024)
            try:
                page = model.candidate_page("nodes", "abc", page_size=50)
                self.assertEqual(page.rows, ())
                self.assertEqual(
                    model.candidate_page("nodes", "needle", page_size=50).rows[0]["id"],
                    "node:0",
                )
            finally:
                model.close()

    def test_disk_budget_failure_never_publishes_partial_model(self):
        graph = self.graph()
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "search.sqlite"
            with self.assertRaises(SearchReadModelBuildError):
                SQLiteKnowledgeSearchReadModel.build(graph, path, max_bytes=4 * 1024)
            self.assertFalse(path.exists())
            self.assertEqual(list(Path(raw).glob("*.tmp")), [])

    def test_cursor_expires_and_replaced_model_is_rejected(self):
        graph = self.graph()
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "search.sqlite"
            model = SQLiteKnowledgeSearchReadModel.build(graph, path, max_bytes=4 * 1024 * 1024)
            try:
                with patch("tos_access.search_read_model.time.time", return_value=1000):
                    page = model.candidate_page("nodes", "common", page_size=1)
                with patch("tos_access.search_read_model.time.time", return_value=1000 + 15 * 60 + 1):
                    with self.assertRaises(SearchReadModelSnapshotError):
                        model.candidate_page("nodes", "common", cursor=page.next_cursor, page_size=1)
                replacement = SQLiteKnowledgeSearchReadModel.build(graph, path, max_bytes=4 * 1024 * 1024)
                replacement.close()
                with self.assertRaises(SearchReadModelSnapshotError):
                    model.candidate_page("nodes", "common", page_size=1)
            finally:
                model.close()

    def test_source_filter_continues_over_bounded_seed_pages(self):
        graph = self.graph()
        graph["nodes"].append({**graph["nodes"][0], "id": "node:canon", "source_graph": "canon"})
        with tempfile.TemporaryDirectory() as raw:
            model = SQLiteKnowledgeSearchReadModel.build(graph, Path(raw) / "search.sqlite", max_bytes=4 * 1024 * 1024)
            try:
                page = model.candidate_page("nodes", "common", sources=["canon"], page_size=1)
                rows = list(page.rows)
                steps = 0
                while page.has_more and page.next_cursor is not None and steps < 10:
                    page = model.candidate_page(
                        "nodes", "common", sources=["canon"], cursor=page.next_cursor, page_size=1
                    )
                    rows.extend(page.rows)
                    steps += 1
                self.assertEqual([row["id"] for row in rows], ["node:canon"])
                self.assertFalse(page.has_more)
            finally:
                model.close()

    def test_verification_budget_is_checked_before_loading_document_bytes(self):
        graph = self.graph()
        graph["nodes"][0]["attributes"] = {"payload": "x" * 100_000}
        with tempfile.TemporaryDirectory() as raw:
            model = SQLiteKnowledgeSearchReadModel.build(graph, Path(raw) / "search.sqlite", max_bytes=16 * 1024 * 1024)
            try:
                with self.assertRaises(SearchReadModelError):
                    model.candidate_page("nodes", "payload", page_size=1, max_verify_chars=10)
                page = model.candidate_page("nodes", "payload", page_size=1, max_verify_chars=200_000)
                self.assertNotIn("search_text", page.rows[0])
            finally:
                model.close()

    def test_query_and_filter_inputs_are_bounded_before_work(self):
        graph = self.graph()
        with tempfile.TemporaryDirectory() as raw:
            model = SQLiteKnowledgeSearchReadModel.build(graph, Path(raw) / "search.sqlite", max_bytes=4 * 1024 * 1024)
            try:
                with self.assertRaises(SearchReadModelError):
                    model.candidate_page("nodes", 123)
                with self.assertRaises(SearchReadModelError):
                    model.candidate_page("nodes", "x" * 257)
                with self.assertRaises(SearchReadModelError):
                    model.candidate_page("nodes", " " * 257)
                with self.assertRaises(SearchReadModelError):
                    model.candidate_page("nodes", "İ" * 129)
                with self.assertRaises(SearchReadModelError):
                    model.candidate_page("nodes", "x", sources=(str(value) for value in range(101)))
                with self.assertRaises(SearchReadModelError):
                    model.candidate_page("nodes", "x", sources=["x"] * 100, kind_ids=["concept"])
                with self.assertRaises(SearchReadModelError):
                    model.candidate_page("nodes", "x", sources=["x" * 257])
            finally:
                model.close()

    def test_ranked_page_preserves_transport_rank_and_continuation(self):
        graph = self.graph()
        with tempfile.TemporaryDirectory() as raw:
            model = SQLiteKnowledgeSearchReadModel.build(graph, Path(raw) / "search.sqlite", max_bytes=4 * 1024 * 1024)
            try:
                first = model.ranked_page("nodes", "strasse", page_size=1)
                self.assertEqual([row["id"] for row in first.rows], ["node:1"])
                self.assertEqual(first.rows[0]["search_rank"], 1)
                self.assertFalse(first.has_more)
                page = model.ranked_page("nodes", "common", page_size=1)
                rows = list(page.rows)
                while page.has_more:
                    page = model.ranked_page("nodes", "common", cursor=page.next_cursor, page_size=1)
                    rows.extend(page.rows)
                self.assertEqual([row["id"] for row in rows], ["node:0", "node:1", "node:2"])
                self.assertTrue(all(row["search_rank"] == 2 for row in rows))
            finally:
                model.close()

    def test_ranked_page_rejects_short_queries_in_explicit_indexed_mode(self):
        graph = self.graph()
        with tempfile.TemporaryDirectory() as raw:
            model = SQLiteKnowledgeSearchReadModel.build(graph, Path(raw) / "search.sqlite", max_bytes=4 * 1024 * 1024)
            try:
                with self.assertRaises(SearchReadModelUnindexedError):
                    model.ranked_page("nodes", "ab", page_size=1)
            finally:
                model.close()

    def test_ranked_page_skips_single_gram_false_positive_before_continuation(self):
        graph = self.graph()
        graph["nodes"] = [
            {
                **graph["nodes"][0],
                "id": "node:false-positive",
                "native_id": "node:false-positive",
                "display": {
                    "title": {"default": "defabcde"},
                    "kind_label": {"default": "concept"},
                    "summary": {"default": "defabcde"},
                },
            },
            {
                **graph["nodes"][0],
                "id": "node:actual",
                "native_id": "node:actual",
                "display": {
                    "title": {"default": "abcdef"},
                    "kind_label": {"default": "concept"},
                    "summary": {"default": "abcdef"},
                },
            },
        ]
        with tempfile.TemporaryDirectory() as raw:
            model = SQLiteKnowledgeSearchReadModel.build(graph, Path(raw) / "search.sqlite", max_bytes=4 * 1024 * 1024)
            try:
                page = model.ranked_page("nodes", "abcdef", page_size=1)
                self.assertEqual([row["id"] for row in page.rows], ["node:actual"])
                self.assertFalse(page.has_more)
                self.assertGreaterEqual(page.candidate_rows, 2)
            finally:
                model.close()

    def test_ranked_page_rejects_non_strict_inner_cursor_fields(self):
        graph = self.graph()
        with tempfile.TemporaryDirectory() as raw:
            model = SQLiteKnowledgeSearchReadModel.build(graph, Path(raw) / "search.sqlite", max_bytes=4 * 1024 * 1024)
            try:
                first = model.ranked_page("nodes", "common", page_size=1)
                self.assertIsNotNone(first.next_cursor)
                cursor = first.next_cursor

                def decode(value: str) -> dict:
                    padded = value + "=" * (-len(value) % 4)
                    return json.loads(base64.urlsafe_b64decode(padded.encode("ascii")))

                def encode(value: dict) -> str:
                    return base64.urlsafe_b64encode(
                        json.dumps(value, separators=(",", ":"), sort_keys=True).encode("utf-8")
                    ).decode("ascii").rstrip("=")

                payload = decode(cursor)
                malformed = [
                    {**payload, "extra": True},
                    {**payload, "n": False},
                    {**payload, "gram": ""},
                    {**payload, "rank": True},
                    {**payload, "rank": 4},
                    {**payload, "id": ""},
                    {**payload, "id": "NODE:0"},
                    {**payload, "position": True},
                    {**payload, "position": -1},
                    {**payload, "expires_at": True},
                    {**payload, "filters_digest": "wrong"},
                ]
                for candidate in malformed:
                    with self.subTest(candidate=candidate):
                        with self.assertRaises(SearchReadModelError):
                            model.ranked_page("nodes", "common", cursor=encode(candidate), page_size=1)
                expired = {**payload, "expires_at": 0}
                with self.assertRaises(SearchReadModelSnapshotError):
                    model.ranked_page("nodes", "common", cursor=encode(expired), page_size=1)
            finally:
                model.close()

    def test_ranked_page_supports_concurrent_readers_on_one_carrier(self):
        graph = self.graph()
        with tempfile.TemporaryDirectory() as raw:
            model = SQLiteKnowledgeSearchReadModel.build(graph, Path(raw) / "search.sqlite", max_bytes=4 * 1024 * 1024)
            try:
                with ThreadPoolExecutor(max_workers=4) as pool:
                    packets = list(pool.map(lambda _: model.ranked_page("nodes", "common", page_size=2), range(8)))
                self.assertTrue(all([row["id"] for row in packet.rows] == ["node:0", "node:1"] for packet in packets))
            finally:
                model.close()

    def test_restart_open_rejects_same_revision_with_changed_search_snapshot(self):
        graph = self.graph()
        with tempfile.TemporaryDirectory() as raw:
            path = Path(raw) / "search.sqlite"
            model = SQLiteKnowledgeSearchReadModel.build(graph, path, max_bytes=4 * 1024 * 1024)
            model.close()
            changed = self.graph()
            changed["nodes"][0]["display"]["title"]["default"] = "Changed but same revision"
            with self.assertRaises(SearchReadModelSnapshotError):
                SQLiteKnowledgeSearchReadModel.open(changed, path)


if __name__ == "__main__":
    unittest.main()
