"""Bounded initial posting coalescing against the unchanged insertion writer."""
from contextlib import closing
import os
from pathlib import Path
import random
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

from tos_access import compressed_search_store as search


class ImmediateBootstrap(search._Writer):
    def __init__(self, db, maximum):
        super().__init__(db, maximum, mode="bootstrap")

    def flush(self):
        pass


class BufferedBootstrapTests(unittest.TestCase):
    binding = {"source_revision": "synthetic-buffered-search-v1"}

    @staticmethod
    def documents(size=600, *, random_order=False):
        identifiers = [f"id:{i:04d}" for i in range(size)]
        if random_order:
            random.Random(971).shuffle(identifiers)
        for index, identifier in enumerate(identifiers):
            yield search.PreparedSearchDocument(
                size - index, "node", identifier, index * 100,
                "common\u0000\U0001f680 needle" if index % 2 else "common\u0000\U0001f680 negative",
                (identifier,), ("common",), {"type_id": "synthetic"})

    def initialize(self, db, documents, writer=search._BootstrapWriter, **limits):
        with patch.object(search, "_BootstrapWriter", writer):
            return search.SearchStore.initialize_transaction(
                db, binding=self.binding, documents=documents,
                max_bytes=64 * 1024 * 1024, max_mutations=2_000_000, **limits)

    @staticmethod
    def logical_dump(db):
        result = {}
        for table in ("search_documents", "search_values", "search_text_chunks",
                      "search_terms", "search_blocks", "search_document_terms"):
            result[table] = sorted(db.execute(f"SELECT * FROM {table}"))
        # A fresh random cursor incarnation is intentionally not byte parity.
        result["search_header"] = list(db.execute(
            "SELECT singleton,header,high_water,max_pages FROM search_header"))
        return result

    def assert_parity(self, documents, writer=search._BootstrapWriter):
        documents = list(documents)
        with closing(sqlite3.connect(":memory:")) as reference, closing(sqlite3.connect(":memory:")) as actual:
            for db in (reference, actual):
                db.execute("BEGIN IMMEDIATE")
            before = self.initialize(reference, documents, ImmediateBootstrap)
            after = self.initialize(actual, documents, writer)
            self.assertEqual(self.logical_dump(actual), self.logical_dump(reference))
            self.assertTrue(actual.in_transaction)
            self.assertGreaterEqual(before["blocks_written"], after["blocks_written"])
            for query in ("", "common", "needle", "id:0001", "\u0000", "\U0001f680", "missing"):
                streams = []
                for db in (reference, actual):
                    cursor, stream = None, []
                    for _ in range(len(documents) * 2 + 2):
                        packet = search.SearchStore.query_transaction(
                            db, binding=self.binding, kind="node", query=query,
                            page_size=37, cursor=cursor)
                        stream.extend(packet["matches"])
                        if not packet["has_more"]:
                            break
                        cursor = packet["next_cursor"]
                    else:
                        self.fail("bounded fixture stream did not finish")
                    streams.append(stream)
                self.assertEqual(*streams)
            return before, after

    def test_monotonic_keys_and_nonmonotonic_addresses_keep_exact_splits_and_queries(self):
        before, after = self.assert_parity(self.documents())
        self.assertGreater(before["blocks_written"], 20 * after["blocks_written"])
        self.assertGreater(before["payload_bytes_written"], 20 * after["payload_bytes_written"])

    def test_arbitrary_source_order_and_range_reentry_match_original_writer(self):
        self.assert_parity(self.documents(random_order=True))

    def test_typed_id_case_unicode_prefix_and_source_order_ties(self):
        identifiers = ["A", "a", "İ", "i\u0307", "I", "i", "ß", "SS", "Σ", "σ", "ς",
                       "x", "x\u0000", "x\U0001f680", False, 0, 0.0, -0.0, None, "", 2**53 + 1]
        documents = [search.PreparedSearchDocument.from_item(index + 1, "node", {
            "id": identifier, "native_id": str(index), "display": {"title": {"ru": "Слово"}},
            "unknown": {"false": False, "zero": 0, "float": 1.0, "null": None}}, index * 10)
            for index, identifier in enumerate(identifiers)]
        self.assert_parity(documents)

    def test_entry_and_payload_eviction_never_changes_store_or_exceeds_limits(self):
        for caps in ((1, 32 * 1024 * 1024), (8192, 1), (3, 1400)):
            writers = []
            # Keep the constructor independent of the patched module factory.
            original = search._BootstrapWriter
            def writer(db, maximum, original=original):
                value = original(db, maximum, max_cached_blocks=caps[0], max_cached_bytes=caps[1])
                writers.append(value)
                return value
            with self.subTest(caps=caps):
                self.assert_parity(self.documents(35, random_order=True), writer)
                self.assertLessEqual(writers[0].peak_cached_blocks, caps[0])
                self.assertLessEqual(writers[0].peak_cached_bytes, caps[1])
                self.assertEqual(writers[0].cached_bytes, 0)
                self.assertFalse(writers[0].cache)

    def test_flush_failure_cannot_publish_header_or_commit_owner_transaction(self):
        class FailingFlush(search._BootstrapWriter):
            def flush(self):
                super().flush()
                raise search.SearchBudgetExceeded("synthetic final flush refusal")
        with closing(sqlite3.connect(":memory:")) as db:
            db.execute("BEGIN IMMEDIATE")
            db.execute("CREATE TABLE owner (id TEXT)")
            db.execute("INSERT INTO owner VALUES ('uncommitted')")
            with self.assertRaises(search.SearchBudgetExceeded):
                self.initialize(db, self.documents(1), FailingFlush)
            self.assertTrue(db.in_transaction)
            self.assertEqual(db.execute("SELECT * FROM search_header").fetchall(), [])
            self.assertEqual(db.execute("SELECT * FROM owner").fetchall(), [("uncommitted",)])
            db.rollback()
            self.assertFalse(db.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall())
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as folder:
            target = Path(folder) / "new.sqlite"
            with patch.object(search, "_BootstrapWriter", FailingFlush), self.assertRaises(search.SearchBudgetExceeded):
                search.SearchStore.publish_initial(target, binding=self.binding, documents=self.documents(1))
            self.assertFalse(target.exists())

    def test_split_range_reentry_and_eviction_compose(self):
        original = search._BootstrapWriter
        def limited(db, maximum):
            return original(db, maximum, max_cached_blocks=3, max_cached_bytes=1400)
        self.assert_parity(self.documents(300, random_order=True), limited)

    def test_mutation_budget_counts_final_dirty_block_flush(self):
        with closing(sqlite3.connect(":memory:")) as db:
            db.execute("BEGIN IMMEDIATE")
            writer = search._BootstrapWriter(db, 10000)
            for sql in search.DDL.split(";"):
                if sql.strip():
                    db.execute(sql)
            writer.replace(next(self.documents(1)), insert=True)
            self.assertTrue(writer.cache)
            self.assertEqual(writer.blocks_written, 0)
            writer.maximum = writer.mutations
            with self.assertRaises(search.SearchBudgetExceeded):
                writer.flush()
            self.assertGreater(writer.mutations, writer.maximum)
            self.assertEqual(db.execute("SELECT * FROM search_header").fetchall(), [])
            db.rollback()

    def test_duplicate_existence_and_post_bootstrap_delta_remain_authoritative(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as folder:
            target = Path(folder) / "search.sqlite"
            document = next(self.documents(1))
            with self.assertRaises(search.SearchInvalidRequest):
                search.SearchStore.publish_initial(target, binding=self.binding, documents=[document, document])
            self.assertFalse(target.exists())
            search.SearchStore.publish_initial(target, binding=self.binding, documents=[document])
            with patch.object(search, "_BootstrapWriter", side_effect=AssertionError("delta used bootstrap cache")):
                search.SearchStore.apply_delta(target, expected_binding=self.binding,
                    new_binding={"source_revision": "synthetic-deleted"},
                    changes=[search.SearchChange("delete", document.doc_id)])
            packet = search.SearchStore(target, binding={"source_revision": "synthetic-deleted"}).query_page(kind="node")
            self.assertEqual(packet["matches"], [])


if __name__ == "__main__":
    unittest.main()
