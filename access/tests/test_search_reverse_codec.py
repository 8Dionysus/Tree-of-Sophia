"""Durable reverse-frame, addressed mutation and format-boundary checks."""
from contextlib import closing
from dataclasses import replace
import json
import sqlite3
import unittest
from unittest.mock import patch

from tos_access import compressed_search_store as search
from tos_access.search_reverse_codec import (MAX_TERM_ID, MAX_TERMS, MAX_PAYLOAD_BYTES,
    ReverseCodecError, decode_reverse, encode_reverse, reverse_digest)


class ReverseCodecTests(unittest.TestCase):
    def test_sparse_dense_and_maximum_dictionary_addresses_roundtrip(self):
        for terms in ([1], [1, 127, 128, 16383, 16384, MAX_TERM_ID], range(1, MAX_TERMS + 1)):
            terms = list(terms)
            frame = encode_reverse(19, "relation", terms)
            self.assertEqual(list(decode_reverse(19, "relation", *frame)), terms)
            self.assertLessEqual(len(frame[1]), 9 * len(terms))
        with self.assertRaises(ReverseCodecError):
            encode_reverse(19, "relation", range(1, MAX_TERMS + 2))

    def test_encoder_refuses_empty_duplicate_unordered_boolean_and_overflow(self):
        for terms in ([], [0], [True], [1, 1], [2, 1], [MAX_TERM_ID + 1]):
            with self.subTest(terms=terms), self.assertRaises(ReverseCodecError):
                encode_reverse(1, "node", terms)

    def test_canonical_count_and_overflow_checks_even_with_valid_seal(self):
        maximum = encode_reverse(1, "node", [MAX_TERM_ID])[1]
        for count, payload in ((1, b"\0"), (1, b"\x81\0"), (1, b"\x80"),
                               (2, b"\x81\x01"), (1, b"\x01\x01"),
                               (2, maximum + b"\x01"), (1, b"\x81" * 10)):
            seal = reverse_digest(1, "node", count, payload)
            with self.subTest(payload=payload), self.assertRaises(ReverseCodecError):
                decode_reverse(1, "node", count, payload, seal)

    def test_frame_binds_identity_kind_count_and_bytes(self):
        count, payload, seal = encode_reverse(7, "node", [1, 5, 90])
        for doc, kind, n, data, digest in ((8, "node", count, payload, seal),
                (7, "relation", count, payload, seal), (7, "node", count + 1, payload, seal),
                (7, "node", count, payload[:-1] + b"a", seal),
                (7, "node", count, payload, b"x" * 32), (7, "node", True, payload, seal),
                (7, "node", count, payload, "x" * 32)):
            with self.assertRaises(ReverseCodecError):
                decode_reverse(doc, kind, n, data, digest)


class ReverseMutationTests(unittest.TestCase):
    binding = {"source_revision": "reverse-1"}

    def fixture(self):
        db = sqlite3.connect(":memory:")
        db.execute("BEGIN")
        docs = [search.PreparedSearchDocument.from_item(i, "node", {
            "id": name, "display": {"title": {"en": "common"}}, "unknown": [False, 1.0]}, i)
            for i, name in ((1, "a"), (2, "b"))]
        search.SearchStore.initialize_transaction(db, binding=self.binding, documents=docs)
        return db, docs

    def apply(self, db, changes, *, before=None, after=None, **options):
        return search.SearchStore.apply_delta_transaction(db, expected_binding=before or self.binding,
            new_binding=after or {"source_revision": "reverse-2"}, changes=changes, **options)

    def test_damaged_missing_and_oversized_selected_frame_refuse_before_dml(self):
        damage = (
            ("DELETE FROM search_document_terms WHERE doc_id=1", ()),
            ("UPDATE search_document_terms SET digest=zeroblob(32) WHERE doc_id=1", ()),
            ("UPDATE search_document_terms SET payload=zeroblob(?) WHERE doc_id=1", (MAX_PAYLOAD_BYTES + 1,)),
            ("UPDATE search_document_terms SET term_count=200001 WHERE doc_id=1", ()),
            ("UPDATE search_document_terms SET term_count=1.5 WHERE doc_id=1", ()),
            ("UPDATE search_document_terms SET payload='text' WHERE doc_id=1", ()),
            ("UPDATE search_document_terms SET payload=(SELECT payload FROM search_document_terms WHERE doc_id=2),"
             "digest=(SELECT digest FROM search_document_terms WHERE doc_id=2) WHERE doc_id=1", ()),
        )
        for sql, parameters in damage:
            with self.subTest(sql=sql):
                db, docs = self.fixture()
                with closing(db):
                    db.execute(sql, parameters)
                    before = db.total_changes
                    with self.assertRaises(search.SearchUnavailable):
                        self.apply(db, [search.SearchChange("update", 1, docs[0])])
                    self.assertEqual(db.total_changes, before)
                    self.assertTrue(db.in_transaction)

    def test_dictionary_existence_kind_and_sentinel_are_checked(self):
        for damage in ("missing", "kind", "sentinel"):
            db, _ = self.fixture()
            with self.subTest(damage=damage), closing(db):
                ids = list(search._reverse_terms(db, 1, "node"))
                if damage == "missing":
                    ids.append(MAX_TERM_ID)
                elif damage == "kind":
                    db.execute("UPDATE search_terms SET kind='relation' WHERE term_id=?", (ids[0],))
                else:
                    sentinel = db.execute("SELECT term_id FROM search_terms WHERE plane=3 AND n=0").fetchone()[0]
                    ids.remove(sentinel)
                count, payload, seal = encode_reverse(1, "node", ids)
                db.execute("UPDATE search_document_terms SET term_count=?,payload=?,digest=? WHERE doc_id=1", (count, payload, seal))
                before = db.total_changes
                with self.assertRaises(search.SearchUnavailable):
                    self.apply(db, [search.SearchChange("delete", 1)])
                self.assertEqual(db.total_changes, before)

    def test_noop_and_sort_move_keep_reverse_bytes_kind_move_reseals(self):
        db, docs = self.fixture()
        with closing(db):
            reverse = db.execute("SELECT * FROM search_document_terms WHERE doc_id=1").fetchone()
            reverse_writes = []
            def authorizer(action, table, column, database, trigger):
                if table == "search_document_terms" and action in (sqlite3.SQLITE_INSERT, sqlite3.SQLITE_UPDATE, sqlite3.SQLITE_DELETE):
                    reverse_writes.append(action)
                return sqlite3.SQLITE_OK
            db.set_authorizer(authorizer)
            second, third, fourth = ({"revision": i} for i in (2, 3, 4))
            self.apply(db, [search.SearchChange("update", 1, docs[0])], after=second)
            moved = replace(docs[0], source_order=900)
            self.apply(db, [search.SearchChange("update", 1, moved)], before=second, after=third)
            self.assertFalse(reverse_writes)
            self.assertEqual(db.execute("SELECT * FROM search_document_terms WHERE doc_id=1").fetchone(), reverse)
            self.apply(db, [search.SearchChange("update", 1, replace(moved, kind="relation"))], before=third, after=fourth)
            self.assertTrue(reverse_writes)
            self.assertTrue(search._reverse_terms(db, 1, "relation"))
            self.assertEqual(search.SearchStore.query_transaction(db, binding=fourth, kind="relation")["matches"][0]["doc_id"], 1)

    def test_unselected_damage_is_not_a_cohort_scan_and_queries_do_not_read_reverse(self):
        db, docs = self.fixture()
        with closing(db):
            db.execute("UPDATE search_document_terms SET digest=zeroblob(32) WHERE doc_id=2")
            self.apply(db, [search.SearchChange("update", 1, docs[0])])
            def authorizer(action, table, column, database, trigger):
                return sqlite3.SQLITE_DENY if action == sqlite3.SQLITE_READ and table == "search_document_terms" else sqlite3.SQLITE_OK
            db.set_authorizer(authorizer)
            page = search.SearchStore.query_transaction(db, binding={"source_revision": "reverse-2"}, kind="node")
            self.assertEqual(len(page["matches"]), 2)

    def test_reverse_write_and_forward_mutations_rollback_together(self):
        db, docs = self.fixture()
        with closing(db):
            db.commit()
            before = list(db.iterdump())
            db.execute("BEGIN")
            with self.assertRaises(search.SearchBudgetExceeded):
                self.apply(db, [search.SearchChange("delete", 1)], max_mutations=1)
            db.rollback()
            self.assertEqual(list(db.iterdump()), before)

    def test_old_storage_reader_writer_and_header_require_explicit_new_bootstrap(self):
        db, docs = self.fixture()
        with closing(db):
            original = db.execute("SELECT header FROM search_header").fetchone()[0]
            before = db.total_changes
            with patch.object(search, "STORAGE_VERSION", 2):
                with self.assertRaises(search.SearchStaleBinding):
                    self.apply(db, [search.SearchChange("delete", 1)])
                with self.assertRaises(search.SearchStaleBinding):
                    search.SearchStore.query_transaction(db, binding=self.binding, kind="node")
            self.assertEqual(db.total_changes, before)
            header = json.loads(original)
            header["storage_version"] = 2
            db.execute("UPDATE search_header SET header=?", (search._json(header),))
            before = db.total_changes
            with self.assertRaises(search.SearchStaleBinding):
                self.apply(db, [search.SearchChange("delete", 1)])
            self.assertEqual(db.total_changes, before)


if __name__ == "__main__":
    unittest.main()
