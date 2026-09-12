"""Bulk physical layout may differ; full search and later deltas must not."""
from contextlib import closing
from dataclasses import replace
import os
from pathlib import Path
import random
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

from tos_access import compressed_search_bootstrap as bulk
from tos_access import compressed_search_store as search


class BulkBootstrapTests(unittest.TestCase):
    binding = {"source_revision": "synthetic-bulk-search-v1"}
    limits = bulk.BulkBootstrapLimits(max_bytes=32 * 1024 * 1024, max_mutations=2_000_000)

    @staticmethod
    def documents(size=513):
        names = [f"id:{i:04d}" for i in range(size)]
        random.Random(5127).shuffle(names)
        special = ["A", "a", "İ", "i\u0307", "ß", "SS", "Σ", "σ", "ς", "x", "x\0", "x🚀", "x\ud800", False, 0, 0.0, -0.0, None, ""]
        for i, identifier in enumerate(names):
            if i < len(special):
                identifier = special[i]
            kind = "node" if i % 5 else "relation"
            item = {"id": identifier, "native_id": str(i), "type_id": "synthetic",
                    "display": {"title" if kind == "node" else "label": {"ru": "Общее common", "en": "Common"}},
                    "unknown_nested": {"answer": "needle" if i % 2 else "negative", "flag": False, "value": 1.0}}
            yield search.PreparedSearchDocument.from_item((size - i) * 3, kind, item, i * 100)

    def initialize(self, db, path, documents, **kwargs):
        return search.SearchStore.initialize_bulk_transaction(
            db, binding=self.binding, documents=documents, scratch_path=path,
            scratch_limits=kwargs.pop("scratch_limits", self.limits),
            max_mutations=kwargs.pop("max_mutations", 2_000_000),
            max_bytes=kwargs.pop("max_bytes", 64 * 1024 * 1024), **kwargs)

    @staticmethod
    def stream(db, binding, kind, query, **limits):
        cursor, values = None, []
        for _ in range(2000):
            packet = search.SearchStore.query_transaction(db, binding=binding,
                kind=kind, query=query, cursor=cursor, page_size=17, **limits)
            values.extend(packet["matches"])
            if not packet["has_more"]:
                return values
            cursor = packet["next_cursor"]
        raise AssertionError("fixture stream did not terminate")

    def assert_invariants(self, db):
        documents = dict(db.execute("SELECT doc_id,sort_key FROM search_documents"))
        # Fixture-only oracle: one scan, not a reverse-table scan per term.
        reverse = {}
        for address, term in db.execute("SELECT doc_id,term_id FROM search_document_terms"):
            reverse.setdefault(term, set()).add(address)
        for term, total in db.execute("SELECT term_id,posting_count FROM search_terms"):
            found, previous = [], None
            blocks = list(db.execute("SELECT lower_fence,posting_count,payload FROM search_blocks WHERE term_id=? ORDER BY lower_fence", (term,)))
            for index, (fence, count, payload) in enumerate(blocks):
                addresses = search.decode_postings(payload)
                self.assertEqual(count, len(addresses))
                self.assertLessEqual(count, search.BLOCK_SIZE)
                keys = [documents[address] for address in addresses]
                self.assertEqual(keys, sorted(keys))
                self.assertTrue(all(key >= fence for key in keys))
                if index + 1 < len(blocks):
                    self.assertTrue(all(key < blocks[index + 1][0] for key in keys))
                if keys and previous is not None:
                    self.assertLess(previous, keys[0])
                if keys:
                    previous = keys[-1]
                found.extend(addresses)
            self.assertEqual(len(found), total)
            self.assertEqual(len(found), len(set(found)))
            self.assertEqual(set(found), reverse.get(term, set()))

    def compare(self, old, actual, binding):
        for table in ("search_documents", "search_terms", "search_document_terms", "search_values", "search_text_chunks"):
            self.assertEqual(sorted(old.execute(f"SELECT * FROM {table}")), sorted(actual.execute(f"SELECT * FROM {table}")), table)
        for kind in ("node", "relation"):
            for query in ("", "common", "needle", "unknown_nested", "false", "Общее", "\0", "🚀", "\ud800", "i\u0307", "no-match"):
                self.assertEqual(self.stream(old, binding, kind, query, candidate_budget=29),
                                 self.stream(actual, binding, kind, query, candidate_budget=29), (kind, query))
        self.assert_invariants(actual)

    def test_513_exact_objects_oracle_and_post_bootstrap_delta_cursor_streams(self):
        documents = list(self.documents())
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as folder, closing(sqlite3.connect(":memory:")) as old, closing(sqlite3.connect(":memory:")) as actual:
            old.execute("BEGIN")
            actual.execute("BEGIN")
            search.SearchStore.initialize_transaction(old, binding=self.binding, documents=documents,
                max_bytes=64 * 1024 * 1024, max_mutations=2_000_000)
            report = self.initialize(actual, Path(folder) / "scratch.sqlite", iter(documents))
            self.assertFalse(list(Path(folder).iterdir()))
            self.assertEqual(report["main_mutations"], actual.total_changes)
            self.assertEqual(report["mutations"], actual.total_changes + report["scratch_mutations"])
            self.assertEqual(report["scratch_mutations"], actual.execute("SELECT count(*) FROM search_document_terms").fetchone()[0])
            self.assertEqual(report["high_water"], max(doc.doc_id for doc in documents))
            self.assertGreater(report["dictionary_hits"], 0)
            self.assertLessEqual(report["dictionary_peak_entries"], self.limits.max_cached_terms)
            self.assertLessEqual(report["dictionary_peak_bytes"], self.limits.max_cached_bytes)
            self.compare(old, actual, self.binding)
            previous = search.SearchStore.query_transaction(actual, binding=self.binding, kind="node", page_size=1)["next_cursor"]
            update = replace(documents[1], identifier="new-first", searchable="changed common", visible=("changed",))
            high = report["high_water"]
            inserted = [search.PreparedSearchDocument(high + i + 1, "node", name, i, "common", (name,), ("common",), {})
                        for i, name in enumerate(("!before", "id:0200-between", "zz-after"))]
            changes = [search.SearchChange("update", update.doc_id, update), search.SearchChange("delete", documents[2].doc_id)]
            changes += [search.SearchChange("insert", doc.doc_id, doc) for doc in inserted]
            after = {"source_revision": "synthetic-bulk-delta-v2"}
            for db in (old, actual):
                search.SearchStore.apply_delta_transaction(db, expected_binding=self.binding, new_binding=after, changes=changes)
            self.compare(old, actual, after)
            with self.assertRaises(search.SearchCursorError):
                search.SearchStore.query_transaction(actual, binding=after, kind="node", cursor=previous)

    def test_full_block_split_empty_fences_and_reinsertion_use_unchanged_delta(self):
        docs = [search.PreparedSearchDocument(i + 1, "node", f"k{i:03d}", i, "common", (), (), {}) for i in range(256)]
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as folder, closing(sqlite3.connect(":memory:")) as db:
            db.execute("BEGIN")
            self.initialize(db, Path(folder) / "scratch", docs)
            term = db.execute("SELECT term_id FROM search_terms WHERE plane=3 AND n=0").fetchone()[0]
            self.assertEqual(db.execute("SELECT posting_count FROM search_blocks WHERE term_id=?", (term,)).fetchall(), [(256,)])
            inserted = replace(docs[0], doc_id=257, identifier="!before")
            second = {"revision": 2}
            search.SearchStore.apply_delta_transaction(db, expected_binding=self.binding, new_binding=second,
                changes=[search.SearchChange("insert", 257, inserted)])
            self.assertEqual(db.execute("SELECT posting_count FROM search_blocks WHERE term_id=? ORDER BY lower_fence", (term,)).fetchall(), [(128,), (129,)])
            self.assert_invariants(db)
            third = {"revision": 3}
            search.SearchStore.apply_delta_transaction(db, expected_binding=second, new_binding=third,
                changes=[search.SearchChange("delete", i) for i in range(1, 258)])
            self.assertEqual(self.stream(db, third, "node", ""), [])
            search.SearchStore.apply_delta_transaction(db, expected_binding=third, new_binding={"revision": 4},
                changes=[search.SearchChange("insert", 258, replace(inserted, doc_id=258))])
            self.assert_invariants(db)

    def test_tiny_dictionary_and_batches_preserve_logical_tables(self):
        docs = list(self.documents(25))
        for cache_bytes in (1, 1024):
            with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as folder, closing(sqlite3.connect(":memory:")) as old, closing(sqlite3.connect(":memory:")) as actual:
                old.execute("BEGIN")
                actual.execute("BEGIN")
                search.SearchStore.initialize_transaction(old, binding=self.binding, documents=docs)
                report = self.initialize(actual, Path(folder) / "scratch", docs,
                    scratch_limits=replace(self.limits, max_cached_terms=1, max_cached_bytes=cache_bytes, batch_size=1))
                self.assertEqual(report["dictionary_peak_entries"], 0 if cache_bytes == 1 else 1)
                self.assertLessEqual(report["dictionary_peak_bytes"], cache_bytes)
                self.compare(old, actual, self.binding)

    def test_combined_mutations_charge_final_header_after_scratch_and_packing(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as folder:
            with closing(sqlite3.connect(":memory:")) as reference:
                reference.execute("BEGIN")
                report = self.initialize(reference, Path(folder) / "scratch", self.documents(1))
            with closing(sqlite3.connect(":memory:")) as db:
                db.execute("BEGIN")
                with self.assertRaises(search.SearchBudgetExceeded):
                    self.initialize(db, Path(folder) / "scratch", self.documents(1), max_mutations=report["mutations"] - 1)
                self.assertGreater(db.execute("SELECT count(*) FROM search_blocks").fetchone()[0], 0)
                self.assertEqual(db.execute("SELECT * FROM search_header").fetchall(), [])
                self.assertTrue(db.in_transaction)
                self.assertFalse(list(Path(folder).iterdir()))
                db.rollback()

    def test_empty_stream_and_caller_commit_survive_scratch_removal(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as folder, closing(sqlite3.connect(Path(folder) / "main.sqlite")) as db:
            db.execute("BEGIN")
            db.execute("CREATE TABLE owner (value TEXT)")
            db.execute("INSERT INTO owner VALUES ('uncommitted')")
            report = self.initialize(db, Path(folder) / "scratch.sqlite", [])
            self.assertTrue(db.in_transaction)
            self.assertEqual(report["high_water"], 0)
            self.assertEqual(report["scratch_mutations"], 0)
            self.assertEqual(db.execute("PRAGMA journal_mode").fetchone()[0], "delete")
            db.commit()
            self.assertEqual(search.SearchStore(Path(folder) / "main.sqlite", binding=self.binding).query_page(kind="node")["matches"], [])
            self.assertFalse((Path(folder) / "scratch.sqlite").exists())

    def test_invalid_limits_and_existing_paths_are_rejected_before_main_ddl(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as folder, closing(sqlite3.connect(":memory:")) as db:
            with self.assertRaises(search.SearchInvalidRequest):
                self.initialize(db, Path(folder) / "scratch", [])
            db.execute("BEGIN")
            before = db.execute("PRAGMA max_page_count").fetchone()[0]
            for limits in (replace(self.limits, max_bytes=True), replace(self.limits, max_mutations=0), replace(self.limits, batch_size=1025), replace(self.limits, max_cached_bytes=0)):
                with self.assertRaises(search.SearchInvalidRequest):
                    self.initialize(db, Path(folder) / "scratch", [], scratch_limits=limits)
            with self.assertRaises(search.SearchInvalidRequest):
                self.initialize(db, Path(folder) / "scratch", [], max_mutations=False)
            self.assertEqual(db.execute("PRAGMA max_page_count").fetchone()[0], before)
            self.assertFalse(db.execute("SELECT name FROM sqlite_master").fetchall())
            existing = Path(folder) / "existing"
            existing.touch()
            with self.assertRaises(FileExistsError):
                self.initialize(db, existing, [])
            link = Path(folder) / "symlink"
            link.symlink_to(existing)
            with self.assertRaises(FileExistsError):
                self.initialize(db, link, [])
            self.assertTrue(existing.exists())
            self.assertTrue(link.is_symlink())
            for suffix in ("-journal", "-wal", "-shm"):
                target = Path(folder) / ("new" + suffix)
                adjacent = target.with_name(target.name + suffix)
                adjacent.touch()
                with self.assertRaises(FileExistsError):
                    self.initialize(db, target, [])
                self.assertTrue(adjacent.exists())
                self.assertFalse(target.exists())
            self.assertFalse(db.execute("SELECT name FROM sqlite_master").fetchall())

    def test_main_scratch_write_and_page_refusals_leave_no_header_or_scratch(self):
        for options in ({"max_mutations": 1}, {"scratch_limits": replace(self.limits, max_mutations=1)},
                        {"scratch_limits": replace(self.limits, max_bytes=65536)}, {"max_bytes": 65536}):
            with self.subTest(options=options), tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as folder, closing(sqlite3.connect(":memory:")) as db:
                db.execute("BEGIN")
                with self.assertRaises(search.SearchBudgetExceeded):
                    self.initialize(db, Path(folder) / "scratch", self.documents(100), **options)
                self.assertFalse(list(Path(folder).iterdir()))
                if db.in_transaction:
                    if db.execute("SELECT name FROM sqlite_master WHERE name='search_header'").fetchone():
                        self.assertEqual(db.execute("SELECT * FROM search_header").fetchall(), [])
                    db.rollback()
                self.assertFalse(db.execute("SELECT name FROM sqlite_master").fetchall())

    def test_generator_and_pack_failures_preserve_owner_transaction_for_rollback(self):
        def failing():
            yield next(self.documents(1))
            raise RuntimeError("source failed")
        for fail_at in ("generator", "pack"):
            with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as folder, closing(sqlite3.connect(":memory:")) as db:
                db.execute("BEGIN")
                db.execute("CREATE TABLE owner (id TEXT)")
                db.execute("INSERT INTO owner VALUES ('pending')")
                original = bulk._pack
                def failing_pack(writer, scratch):
                    original(writer, scratch)
                    raise RuntimeError("pack failed")
                with patch.object(bulk, "_pack", failing_pack if fail_at == "pack" else original), self.assertRaises(RuntimeError):
                    self.initialize(db, Path(folder) / "scratch", failing() if fail_at == "generator" else self.documents(1))
                self.assertTrue(db.in_transaction)
                self.assertEqual(db.execute("SELECT * FROM owner").fetchall(), [("pending",)])
                self.assertEqual(db.execute("SELECT * FROM search_header").fetchall(), [])
                self.assertFalse(list(Path(folder).iterdir()))
                db.rollback()
                self.assertFalse(db.execute("SELECT name FROM sqlite_master").fetchall())

    def test_duplicate_document_and_sort_collision_keep_constraints(self):
        doc = next(self.documents(1))
        for other in (doc, replace(doc, doc_id=100), replace(doc, doc_id=100, identifier="ID:0000")):
            # The third case uses an actual equal-lower ID and source token.
            original = replace(doc, identifier="id:0000") if other.identifier == "ID:0000" else doc
            with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as folder, closing(sqlite3.connect(":memory:")) as db:
                db.execute("BEGIN")
                with self.assertRaises(search.SearchInvalidRequest):
                    self.initialize(db, Path(folder) / "scratch", [original, other])
                self.assertFalse(list(Path(folder).iterdir()))
                db.rollback()

    def test_temp_sort_refusal_does_not_execute_the_query(self):
        with closing(sqlite3.connect(":memory:")) as db:
            db.execute("CREATE TABLE input (value TEXT)")
            with self.assertRaises(search.SearchUnavailable):
                bulk._ordered(db, "SELECT value FROM input ORDER BY value")

    def test_replaced_scratch_inode_is_not_deleted(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as folder, closing(sqlite3.connect(":memory:")) as db:
            db.execute("BEGIN")
            scratch = Path(folder) / "scratch"
            retained = Path(folder) / "retained"
            original = bulk._pack
            def replace_inode(writer, temporary):
                original(writer, temporary)
                scratch.rename(retained)
                scratch.touch()
            with patch.object(bulk, "_pack", replace_inode), self.assertRaises(search.SearchUnavailable):
                self.initialize(db, scratch, self.documents(1))
            self.assertTrue(scratch.exists())
            self.assertTrue(retained.exists())
            db.rollback()


if __name__ == "__main__":
    unittest.main()
