"""Search-v3 behavior and physical delta checks on bounded synthetic carriers."""
import copy
from contextlib import closing
import hashlib
import hmac
import json
import os
from pathlib import Path
import sqlite3
import tempfile
import time
import unittest
from unittest.mock import patch

from tos_access.compressed_search_store import (
    BLOCK_SIZE, MAX_ADDRESS, MAX_HEADER_BYTES, MIN_METADATA_BYTES, MIN_RESPONSE_BYTES,
    PreparedSearchDocument, SearchChange, SearchStore,
    SearchInvalidRequest, SearchStaleBinding, SearchCursorError,
    SearchCursorExpired, SearchUnavailable, SearchBudgetExceeded,
    decode_postings, encode_postings, order_key,
)
from tos_access.knowledge import _knowledge_search_rank


class CompressedSearchStoreTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="compressed-search-", dir=os.environ.get("TMPDIR"))
        self.path = Path(self.tmp.name) / "search.sqlite"
        self.binding = {"source_revision": "fixture-1", "prepared_sha256": "a" * 64}

    def tearDown(self):
        self.tmp.cleanup()

    @staticmethod
    def item(identifier, text="common alphabet", **extra):
        return {"id": identifier, "source_graph": "philosophy", "kind_id": "concept", "display": {"title": {"en": text}, "summary": {"ru": "Общее слово"}}, **extra}

    def publish(self, items, *, kind="node", first_id=1):
        documents = [PreparedSearchDocument.from_item(first_id + i, kind, item, (i + 1) * 1024) for i, item in enumerate(items)]
        report = SearchStore.publish_initial(self.path, binding=self.binding, documents=documents)
        return SearchStore(self.path, binding=self.binding), documents, report

    def test_explicit_bootstrap_budget_can_exceed_delta_ceiling_without_more_work(self):
        document = PreparedSearchDocument.from_item(1, "node", self.item("a"), 1)
        for maximum in (20_000_001, MAX_ADDRESS):
            with self.subTest(maximum=maximum), closing(sqlite3.connect(":memory:")) as db:
                db.execute("BEGIN IMMEDIATE")
                report = SearchStore.initialize_transaction(db, binding=self.binding,
                    documents=[document], max_mutations=maximum)
                self.assertGreater(report["mutations"], 0)
                self.assertLess(report["mutations"], 20_000_000)
                self.assertTrue(db.in_transaction)
                self.assertEqual(SearchStore.query_transaction(db, binding=self.binding, kind="node")
                                 ["matches"], [{"doc_id": 1, "id": "a", "rank": 3}])
                db.rollback()
        report = SearchStore.publish_initial(self.path, binding=self.binding,
            documents=[document], max_mutations=20_000_001)
        self.assertLess(report["mutations"], 20_000_000)
        self.assertEqual(SearchStore(self.path, binding=self.binding).query_page(kind="node")
                         ["matches"], [{"doc_id": 1, "id": "a", "rank": 3}])

    def test_invalid_bootstrap_budget_is_rejected_before_ddl_or_file_creation(self):
        document = PreparedSearchDocument.from_item(1, "node", self.item("a"), 1)
        for maximum in (MAX_ADDRESS + 1, True, 0, -1, 1.5):
            with self.subTest(maximum=maximum), closing(sqlite3.connect(":memory:")) as db:
                db.execute("BEGIN IMMEDIATE")
                db.execute("CREATE TABLE carrier (id TEXT)")
                db.execute("INSERT INTO carrier VALUES ('owner-work')")
                before = list(db.iterdump())
                changes = db.total_changes
                traced = []
                db.set_trace_callback(traced.append)
                with self.assertRaises(SearchInvalidRequest):
                    SearchStore.initialize_transaction(db, binding=self.binding,
                        documents=[document], max_mutations=maximum)
                db.set_trace_callback(None)
                self.assertEqual(traced, [])
                self.assertTrue(db.in_transaction)
                self.assertEqual(db.total_changes, changes)
                self.assertEqual(list(db.iterdump()), before)
                db.rollback()
            with self.assertRaises(SearchInvalidRequest):
                SearchStore.publish_initial(self.path, binding=self.binding,
                    documents=[document], max_mutations=maximum)
            self.assertFalse(self.path.exists())

    def test_delta_keeps_20m_ceiling_and_does_not_touch_owner_transaction(self):
        self.publish([self.item("a")])
        change = SearchChange("update", 1,
            PreparedSearchDocument.from_item(1, "node", self.item("changed"), 1))
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("BEGIN IMMEDIATE")
            db.execute("CREATE TABLE carrier (id TEXT)")
            db.execute("INSERT INTO carrier VALUES ('owner-work')")
            before, changes = list(db.iterdump()), db.total_changes
            cap = db.execute("PRAGMA max_page_count").fetchone()[0]
            for maximum in (20_000_001, MAX_ADDRESS):
                traced = []
                db.set_trace_callback(traced.append)
                with self.assertRaises(SearchInvalidRequest):
                    SearchStore.apply_delta_transaction(db, expected_binding=self.binding,
                        new_binding={"source_revision": "refused"}, changes=[change],
                        max_mutations=maximum)
                db.set_trace_callback(None)
                self.assertEqual(traced, [])
                self.assertTrue(db.in_transaction)
                self.assertEqual(db.total_changes, changes)
                self.assertEqual(list(db.iterdump()), before)
                self.assertEqual(db.execute("PRAGMA max_page_count").fetchone()[0], cap)
            db.rollback()
        self.assertEqual(SearchStore(self.path, binding=self.binding).query_page(kind="node")
                         ["matches"], [{"doc_id": 1, "id": "a", "rank": 3}])

    def test_small_bootstrap_budget_still_enforces_actual_writes_and_rollback(self):
        document = PreparedSearchDocument.from_item(1, "node", self.item("a"), 1)
        with closing(sqlite3.connect(":memory:")) as db:
            db.execute("BEGIN IMMEDIATE")
            db.execute("CREATE TABLE carrier (id TEXT)")
            db.execute("INSERT INTO carrier VALUES ('owner-work')")
            with self.assertRaisesRegex(SearchBudgetExceeded, "bootstrap mutation budget"):
                SearchStore.initialize_transaction(db, binding=self.binding,
                    documents=[document], max_mutations=1)
            self.assertTrue(db.in_transaction)
            self.assertEqual(db.execute("SELECT id FROM carrier").fetchone(), ("owner-work",))
            db.rollback()
            self.assertEqual(db.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(), [])
        with self.assertRaisesRegex(SearchBudgetExceeded, "bootstrap mutation budget"):
            SearchStore.publish_initial(self.path, binding=self.binding,
                documents=[document], max_mutations=1)
        self.assertFalse(self.path.exists())

    def drain(self, store, query="", *, kind="node", filters=None, page_size=3, candidate_budget=31, verification_bytes=8192):
        cursor = None
        result = []
        empty_work_pages = 0
        for count in range(5000):
            page = store.query_page(kind=kind, query=query, filters=filters, page_size=page_size, cursor=cursor, candidate_budget=candidate_budget, verification_bytes=verification_bytes)
            self.assertLessEqual(page["work"]["operations"], candidate_budget)
            self.assertLessEqual(page["work"]["verification_bytes"], verification_bytes)
            self.assertLessEqual(page["work"]["candidates"], candidate_budget)
            self.assertLessEqual(page["work"]["metadata_bytes"], MIN_METADATA_BYTES)
            self.assertEqual(page["work"]["response_bytes"], len(json.dumps(page, ensure_ascii=False, sort_keys=True).encode("utf-8", "surrogatepass")))
            self.assertEqual(page["returned_count"], len(page["matches"]))
            if cursor is not None:
                self.assertIsNone(page["total_matching"])
            result.extend(page["matches"])
            if not page["has_more"]:
                self.assertIsNone(page["next_cursor"])
                return result, empty_work_pages
            empty_work_pages += not bool(page["matches"])
            self.assertIsNotNone(page["next_cursor"])
            self.assertNotEqual(cursor, page["next_cursor"], "continuation must make progress")
            cursor = page["next_cursor"]
        self.fail("bounded fixture did not drain")

    @staticmethod
    def reference(items, query, kind="node", filters=None):
        needle = str(query).strip().lower()
        selected = [(i + 1, item) for i, item in enumerate(items) if (not needle or needle in json.dumps(item, ensure_ascii=False, sort_keys=True).lower()) and all(not values or any(type(item.get(k)) is type(v) and item.get(k) == v for v in values) for k, values in (filters or {}).items())]
        selected.sort(key=lambda pair: _knowledge_search_rank(pair[1], needle, relation=kind == "relation"))
        return [{"doc_id": address, "id": item.get("id"), "rank": _knowledge_search_rank(item, needle, relation=kind == "relation")[0]} for address, item in selected]

    def test_reference_complete_stream_short_common_unicode_ties_filters(self):
        items = [self.item("b", "abc"), self.item("A", "ABC title"), self.item("a", "other", technical="abc"), self.item("a\0", "quote\" slash\\ newline\n"), self.item("\U00010000", "İ Σ ΟΣ ß e\u0301"), self.item("\ue000", "absent")]
        items += [self.item(f"n{i:02}", "common alphabet" if i % 2 else "alphabet common", kind_id="odd" if i % 2 else "concept", marker=False if i % 3 else 0) for i in range(17)]
        store, _, _ = self.publish(items)
        for query in ("", "a", "ab", "abc", "common", "alphabet common", "never-present", "İ", "σ", "ς", "ß", "e\u0301", '"', "\\n", "false", "  a  "):
            for filters in (None, {"kind_id": ["odd"]}, {"source_graph": ["no-such-source"]}):
                with self.subTest(query=query, filters=filters):
                    actual, _ = self.drain(store, query, filters=filters)
                    self.assertEqual(actual, self.reference(items, query, filters=filters))

    def test_long_shared_identity_prefix_uses_rare_full_term_without_corpus_walk(self):
        needle = 'tos.zz-rare-target'
        items = [self.item(f'tos.common-{i:03}', technical='unrelated ' * 1000) for i in range(300)]
        items += [self.item(needle), self.item(needle + '-child')]
        store, _, _ = self.publish(items)
        page = store.query_page(kind='node', query=needle, page_size=10, candidate_budget=128)
        self.assertFalse(page['has_more'])
        self.assertEqual(page['matches'], self.reference(items, needle))
        self.assertLessEqual(page['work']['candidates'], 8)
        self.assertLess(page['work']['verification_bytes'], 8192)

    def test_authenticated_legacy_cursor_keeps_its_original_candidate_stream(self):
        needle = 'tos.rare-'
        items = [self.item('tos.common-a'), self.item('tos.common-b'), self.item('tos.rare-target')]
        store, _, _ = self.publish(items)
        encode = lambda value: json.dumps(value, ensure_ascii=False, sort_keys=True).encode('utf-8', 'surrogatepass')
        legacy = hashlib.sha256(store.header.encode('utf-8') + b'\0' + encode(['node', needle, {}])).hexdigest()
        # A real former prefix-stream predecessor absent from the new rare
        # full-text term: changing stream on resume would lose its bound member.
        state = {'query': legacy, 'phase': 1, 'after': 1, 'partial': None, 'expires': int(time.time()) + 900}
        cursor = {'state': state, 'mac': hmac.new(store.generation, encode(state), hashlib.sha256).hexdigest()}
        matches = []
        for _ in range(100):
            page = SearchStore(self.path, binding=self.binding).query_page(kind='node', query=needle,
                cursor=cursor, page_size=1, candidate_budget=16)
            matches.extend(page['matches'])
            cursor = page['next_cursor']
            if cursor is None:
                break
            self.assertEqual(cursor['state']['query'], legacy)
        else:
            self.fail('legacy continuation did not exhaust')
        self.assertEqual(matches, self.reference(items, needle))
        first = store.query_page(kind='node', query=needle, candidate_budget=2)
        self.assertNotEqual(first['next_cursor']['state']['query'], legacy)

    def test_relation_rank_and_typed_source_preservation(self):
        items = [{"id": f"r{i}", "source_graph": source, "predicate_id": "related", "display": {"label": {"en": title}, "statement": {"ru": "common statement"}}} for i, (source, title) in enumerate(((False, "common"), (0, "common prefix"), ("0", "other"), (None, "other")))]
        store, _, _ = self.publish(items, kind="relation")
        for filters in (None, {"source_graph": [False]}, {"source_graph": [0]}, {"source_graph": ["0"]}, {"source_graph": [None]}):
            actual, _ = self.drain(store, "common", kind="relation", filters=filters)
            self.assertEqual(actual, self.reference(items, "common", "relation", filters))

    def test_large_text_and_visible_value_resume_with_overlap_and_minimum_work(self):
        items = [self.item("a", "x" * 32766 + "boundary-needle" + "x" * 50000), self.item("b", "plain", technical="z" * 65532 + "boundary-needle" + "z" * 50000), self.item("c", "plain", technical="z" * 40000 + "boundaryXneedle")]
        store, _, _ = self.publish(items)
        actual, empty = self.drain(store, "boundary-needle", candidate_budget=2)
        self.assertEqual(actual, self.reference(items, "boundary-needle"))
        self.assertGreater(empty, 10)
        # False-positive grams require completing a big full-text scan too.
        actual, _ = self.drain(store, "zboundary-needle", candidate_budget=4)
        self.assertEqual(actual, self.reference(items, "zboundary-needle"))

    def test_query_lower_expansion_and_exact_serialization(self):
        long_query = "İ" * 256
        items = [self.item("a", long_query), self.item("b", "quote\" and\nnewline"), self.item("c", "x", technical={"key": "value"})]
        store, _, _ = self.publish(items)
        for query in (long_query, '"key": "value"', "quote\"", "and\nnewline", "\\n"):
            actual, _ = self.drain(store, query)
            self.assertEqual(actual, self.reference(items, query))
        with self.assertRaises(ValueError):
            store.query_page(kind="node", query="a" * 257)

    def test_cursor_binding_restart_integrity_and_counts(self):
        store, _, _ = self.publish([self.item("a"), self.item("b")])
        page = store.query_page(kind="node", page_size=1)
        restored = SearchStore(self.path, binding=self.binding)
        resumed = restored.query_page(kind="node", page_size=100, cursor=page["next_cursor"])
        self.assertEqual([x["id"] for x in resumed["matches"]], ["b"])
        self.assertIsNone(resumed["total_matching"])
        self.assertEqual(store.query_page(kind="node", page_size=100)["total_matching"], 2)
        cursor = copy.deepcopy(page["next_cursor"])
        cursor["state"]["phase"] = 3.0
        with self.assertRaises(ValueError):
            store.query_page(kind="node", cursor=cursor)
        with self.assertRaises(ValueError):
            store.query_page(kind="node", query="a", cursor=page["next_cursor"])
        with patch("tos_access.compressed_search_store.time.time", return_value=page["next_cursor"]["state"]["expires"] + 1):
            with self.assertRaises(ValueError):
                store.query_page(kind="node", cursor=page["next_cursor"])
        with self.assertRaises(ValueError):
            SearchStore(self.path, binding={"source_revision": "other"})
        with closing(sqlite3.connect(self.path)) as db:
            header = json.loads(db.execute("SELECT header FROM search_header").fetchone()[0])
            header["unicode_version"] = "wrong"
            db.execute("UPDATE search_header SET header=?", (json.dumps(header, ensure_ascii=False, sort_keys=True),))
            db.commit()
        with self.assertRaises(ValueError):
            store.query_page(kind="node")

    def test_delta_local_blocks_rollback_nonreuse_and_physical_size(self):
        items = [self.item(f"m{i:03}", "same") for i in range(270)]
        store, docs, publication = self.publish(items)
        before = store.storage_stats()
        self.assertEqual(before["database_bytes"], before["file_bytes"])
        self.assertGreater(before["database_bytes"], publication["payload_bytes_written"] // 100)
        self.assertEqual(before["tables"]["search_document_terms"]["rows"], 270)
        # Prefix replay across multiple blocks must not reread the metadata of
        # each predecessor, including near the end of a 256-address block.
        cursor, seen = None, []
        for _ in range(271):
            page = store.query_page(kind="node", page_size=1, cursor=cursor)
            self.assertLessEqual(page["work"]["metadata_rows"], 3)
            self.assertLess(page["work"]["metadata_bytes"], 4096)
            seen.extend(row["doc_id"] for row in page["matches"])
            if not page["has_more"]:
                break
            cursor = page["next_cursor"]
        self.assertEqual(seen, list(range(1, 271)))
        with closing(sqlite3.connect(self.path)) as db:
            blocks_before = dict(((term, fence), payload) for term, fence, payload in db.execute("SELECT term_id,lower_fence,payload FROM search_blocks"))
            keys_before = dict(db.execute("SELECT doc_id,sort_key FROM search_documents"))
        inserted = PreparedSearchDocument.from_item(271, "node", self.item("a000", "same"), 1024)
        next_binding = {"source_revision": "fixture-2"}
        delta = SearchStore.apply_delta(self.path, expected_binding=self.binding, new_binding=next_binding, changes=[SearchChange("insert", 271, inserted)])
        with self.assertRaises(ValueError):
            store.query_page(kind="node")
        store = SearchStore(self.path, binding=next_binding)
        with closing(sqlite3.connect(self.path)) as db:
            blocks_after = dict(((term, fence), payload) for term, fence, payload in db.execute("SELECT term_id,lower_fence,payload FROM search_blocks"))
            self.assertEqual(keys_before, {k: v for k, v in db.execute("SELECT doc_id,sort_key FROM search_documents") if k != 271})
            self.assertLessEqual(db.execute("SELECT max(posting_count) FROM search_blocks").fetchone()[0], BLOCK_SIZE)
        changed = sum(blocks_before.get(key) != payload for key, payload in blocks_after.items())
        self.assertEqual(changed, delta["blocks_written"])
        self.assertLess(changed, len(blocks_before) // 2)
        self.assertEqual(self.drain(store, "same", page_size=100, candidate_budget=4096)[0][0]["id"], "a000")
        with self.assertRaises(ValueError):
            SearchStore.apply_delta(self.path, expected_binding=next_binding, new_binding={"source_revision": "failed"}, changes=[SearchChange("delete", 271)], max_mutations=2)
        self.assertEqual(self.drain(store)[0][0]["id"], "a000")
        update = PreparedSearchDocument.from_item(271, "node", self.item("a000", "renamed"), 1024)
        updated_binding = {"source_revision": "fixture-3"}
        SearchStore.apply_delta(self.path, expected_binding=next_binding, new_binding=updated_binding, changes=[SearchChange("update", 271, update)])
        final_binding = {"source_revision": "fixture-4"}
        removed = SearchStore.apply_delta(self.path, expected_binding=updated_binding, new_binding=final_binding, changes=[SearchChange("delete", 271)])
        with self.assertRaises(ValueError):
            SearchStore.apply_delta(self.path, expected_binding=final_binding, new_binding={"source_revision": "reuse"}, changes=[SearchChange("insert", 271, inserted)])
        final = SearchStore(self.path, binding=final_binding)
        self.assertEqual(self.drain(final)[0], self.reference(items, ""))
        self.assertLess(removed["blocks_written"], len(blocks_before) // 2)
        print("COMPRESSED_SEARCH_FIXTURE " + json.dumps({"initial": before, "insert": delta, "delete": removed, "changed_blocks": changed, "initial_blocks": len(blocks_before)}, sort_keys=True))

    def test_same_lower_id_source_order_and_relocation(self):
        items = [self.item("AB"), self.item("ab"), self.item("B")]
        store, _, _ = self.publish(items)
        inserted = PreparedSearchDocument.from_item(4, "node", self.item("Ab"), 1536)
        binding2 = {"source_revision": "ties-2"}
        SearchStore.apply_delta(self.path, expected_binding=self.binding, new_binding=binding2, changes=[SearchChange("insert", 4, inserted)])
        store = SearchStore(self.path, binding=binding2)
        self.assertEqual([x["doc_id"] for x in self.drain(store, "a")[0]], [1, 4, 2, 3])
        moved = PreparedSearchDocument.from_item(4, "node", self.item("z"), 1536)
        binding3 = {"source_revision": "ties-3"}
        SearchStore.apply_delta(self.path, expected_binding=binding2, new_binding=binding3, changes=[SearchChange("update", 4, moved)])
        store = SearchStore(self.path, binding=binding3)
        self.assertEqual([x["doc_id"] for x in self.drain(store)[0]], [1, 2, 3, 4])

    def test_exact_native_type_id_filter_without_alias(self):
        items = [self.item("a", type_id="entity:concept"), self.item("b", type_id="entity:artifact")]
        store, _, _ = self.publish(items)
        filters = {"type_id": ["entity:artifact"]}
        self.assertEqual(self.drain(store, filters=filters)[0], self.reference(items, "", filters=filters))
        with self.assertRaises(ValueError):
            store.query_page(kind="node", filters={"node_type_id": ["entity:artifact"]})

    def test_duplicate_source_identity_refused_initial_insert_update_rollback(self):
        duplicates = [PreparedSearchDocument.from_item(i, "node", self.item("same"), i * 1024) for i in (1, 2)]
        with self.assertRaisesRegex(ValueError, "duplicate exact source identity"):
            SearchStore.publish_initial(self.path, binding=self.binding, documents=duplicates)
        self.assertFalse(self.path.exists())
        store, _, _ = self.publish([self.item("same"), self.item("other")])
        duplicate = PreparedSearchDocument.from_item(3, "node", self.item("same"), 3072)
        new_binding = {"source_revision": "identity-2"}
        with self.assertRaisesRegex(ValueError, "duplicate exact source identity"):
            SearchStore.apply_delta(self.path, expected_binding=self.binding, new_binding=new_binding, changes=[SearchChange("insert", 3, duplicate)])
        self.assertEqual([x["doc_id"] for x in self.drain(store)[0]], [2, 1])
        variant = PreparedSearchDocument.from_item(3, "node", self.item("Same"), 3072)
        SearchStore.apply_delta(self.path, expected_binding=self.binding, new_binding=new_binding, changes=[SearchChange("insert", 3, variant)])
        store = SearchStore(self.path, binding=new_binding)
        duplicate_update = PreparedSearchDocument.from_item(3, "node", self.item("other"), 3072)
        with self.assertRaisesRegex(ValueError, "duplicate exact source identity"):
            SearchStore.apply_delta(self.path, expected_binding=new_binding, new_binding={"source_revision": "identity-3"}, changes=[SearchChange("update", 3, duplicate_update)])
        self.assertEqual([x["id"] for x in self.drain(store)[0]], ["other", "same", "Same"])

    def test_publication_aba_rejects_old_reader_and_cursor(self):
        store, _, _ = self.publish([self.item("a"), self.item("b")])
        cursor = store.query_page(kind="node", page_size=1)["next_cursor"]
        temporary = {"source_revision": "temporary"}
        SearchStore.apply_delta(self.path, expected_binding=self.binding, new_binding=temporary, changes=[])
        SearchStore.apply_delta(self.path, expected_binding=temporary, new_binding=self.binding, changes=[])
        with self.assertRaises(ValueError):
            store.query_page(kind="node")
        reopened = SearchStore(self.path, binding=self.binding)
        with self.assertRaises(ValueError):
            reopened.query_page(kind="node", cursor=cursor)
        self.assertEqual(len(self.drain(reopened)[0]), 2)

    def test_compact_cursor_giant_identifier_and_lone_surrogate(self):
        items = [self.item("a" * 40000), self.item("\ud800")]
        store, _, _ = self.publish(items)
        page = store.query_page(kind="node", page_size=1)
        self.assertLess(len(json.dumps(page["next_cursor"])), 1024)
        self.assertEqual(self.drain(store)[0], self.reference(items, ""))

    def test_unchanged_term_update_writes_no_posting_blocks_and_plans_seek(self):
        store, docs, _ = self.publish([self.item("a", "same", technical="aaaa"), self.item("b", "same", technical="aaaa")])
        replacement = PreparedSearchDocument.from_item(1, "node", self.item("a", "same", technical="aaaaa"), 1024)
        newer = {"source_revision": "same-terms"}
        report = SearchStore.apply_delta(self.path, expected_binding=self.binding, new_binding=newer, changes=[SearchChange("update", 1, replacement)])
        self.assertEqual(report["blocks_written"], 0)
        with closing(sqlite3.connect(self.path)) as db:
            plan = " ".join(str(row) for row in db.execute("EXPLAIN QUERY PLAN SELECT lower_fence,payload FROM search_blocks INDEXED BY search_blocks_nonempty WHERE term_id=? AND lower_fence>? AND posting_count>0 ORDER BY lower_fence LIMIT 1", (1, b"")))
        self.assertIn("search_blocks_nonempty", plan)
        self.assertNotIn("TEMP B-TREE", plan)

    def test_delta_page_quota_refusal_preserves_publication(self):
        document = PreparedSearchDocument.from_item(1, "node", self.item("a"), 1024)
        SearchStore.publish_initial(self.path, binding=self.binding, documents=[document], max_bytes=131072)
        store = SearchStore(self.path, binding=self.binding)
        large = PreparedSearchDocument.from_item(2, "node", self.item("b", technical="x" * 200000), 1024)
        with self.assertRaises(sqlite3.DatabaseError):
            SearchStore.apply_delta(self.path, expected_binding=self.binding, new_binding={"source_revision": "over-quota"}, changes=[SearchChange("insert", 2, large)])
        self.assertEqual([x["id"] for x in self.drain(store)[0]], ["a"])
        self.assertLessEqual(store.storage_stats()["database_bytes"], 131072)

    def test_minimum_metadata_giant_ids_and_verified_response_deferral(self):
        items = [self.item(chr(97 + i) * 400000, "same", technical="x" * 70000 + "needle") for i in range(5)]
        store, _, _ = self.publish(items)
        cursor, seen = None, []
        for _ in range(1000):
            page = store.query_page(kind="node", query="needle", page_size=100, cursor=cursor,
                                    verification_bytes=8192, max_metadata_bytes=MIN_METADATA_BYTES,
                                    max_response_bytes=MIN_RESPONSE_BYTES)
            self.assertLessEqual(page["work"]["metadata_bytes"], MIN_METADATA_BYTES)
            self.assertLessEqual(page["work"]["response_bytes"], MIN_RESPONSE_BYTES)
            self.assertLessEqual(page["work"]["metadata_rows"], page["work"]["candidates"] + 2)
            seen.extend(row["doc_id"] for row in page["matches"])
            if not page["has_more"]:
                break
            self.assertNotEqual(cursor, page["next_cursor"])
            cursor = page["next_cursor"]
        else:
            self.fail("admitted giant singleton did not make bounded progress")
        self.assertEqual(seen, [1, 2, 3, 4, 5])
        # Larger metadata budget reaches the response cap after verifying the
        # next long row. Its saved matched state must not repeat text reads.
        page = store.query_page(kind="node", query="needle", page_size=100,
                                candidate_budget=4096, verification_bytes=8 * 1024 * 1024,
                                max_metadata_bytes=64 * 1024 * 1024,
                                max_response_bytes=MIN_RESPONSE_BYTES)
        self.assertEqual([row["doc_id"] for row in page["matches"]], [1, 2, 3, 4])
        self.assertEqual(page["next_cursor"]["state"]["partial"]["stage"], "matched")
        resumed = store.query_page(kind="node", query="needle", page_size=1,
                                   cursor=page["next_cursor"], verification_bytes=8192,
                                   max_metadata_bytes=MIN_METADATA_BYTES, max_response_bytes=MIN_RESPONSE_BYTES)
        self.assertEqual([row["doc_id"] for row in resumed["matches"]], [5])
        self.assertEqual(resumed["work"]["verification_bytes"], 0)

    def test_corrupted_cursor_predecessor_refused(self):
        store, _, _ = self.publish([self.item("a"), self.item("b"), self.item("c")])
        cursor = store.query_page(kind="node", page_size=1)["next_cursor"]
        with closing(sqlite3.connect(self.path)) as db:
            term = db.execute("SELECT term_id FROM search_terms WHERE plane=3 AND n=0").fetchone()[0]
            db.execute("UPDATE search_blocks SET posting_count=2,payload=? WHERE term_id=?", (encode_postings([2, 3]), term))
            db.commit()
        with self.assertRaisesRegex(ValueError, "predecessor is not a member"):
            store.query_page(kind="node", cursor=cursor)

    def test_complete_header_and_query_framing_caps(self):
        store, _, _ = self.publish([self.item("a")])
        with self.assertRaisesRegex(ValueError, "complete framed search header"):
            SearchStore(self.path, binding={"pad": "x" * MAX_HEADER_BYTES})
        with self.assertRaisesRegex(ValueError, "complete framed search query/filter"):
            store.query_page(kind="node", filters={"source_graph": ["x" * 65536]})
        for options in ({"max_metadata_bytes": MIN_METADATA_BYTES - 1}, {"max_response_bytes": MIN_RESPONSE_BYTES - 1}):
            with self.assertRaises(ValueError):
                store.query_page(kind="node", **options)
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("UPDATE search_header SET header=?", ("x" * (MAX_HEADER_BYTES + 1),))
            db.commit()
        with self.assertRaisesRegex(ValueError, "oversized stored search header"):
            store.query_page(kind="node")

    def test_codec_byte_order_and_invalid_publication(self):
        values = ["", "\0", "a", "a\0", "aA", "aa", "\ud800", "\ue000", "\U00010000", "\U0010ffff"]
        self.assertEqual(sorted(values, key=lambda x: order_key(x, 0)), sorted(values, key=str.lower))
        addresses = [MAX_ADDRESS, 1, 900, 4, 2]
        self.assertEqual(decode_postings(encode_postings(addresses)), addresses)
        for payload in (b"\x80", b"\x00", b"\x82\x00\x00"):
            with self.assertRaises(ValueError):
                decode_postings(payload)
        docs = [PreparedSearchDocument.from_item(1, "node", self.item("a"), 1024)] * 2
        with self.assertRaises(ValueError):
            SearchStore.publish_initial(self.path, binding=self.binding, documents=docs)
        self.assertFalse(self.path.exists())

    def test_owner_initial_schema_and_carriers_commit_or_rollback_together(self):
        document = PreparedSearchDocument.from_item(1, "node", self.item("a"), 1)
        with closing(sqlite3.connect(self.path)) as db:
            for commit in (False, True):
                db.execute("BEGIN IMMEDIATE")
                db.execute("CREATE TABLE carrier (doc_id INTEGER PRIMARY KEY, id TEXT)")
                db.execute("INSERT INTO carrier VALUES (1,'a')")
                traced = []
                db.set_trace_callback(traced.append)
                stats = SearchStore.initialize_transaction(db, binding=self.binding, documents=[document])
                page = SearchStore.query_transaction(db, binding=self.binding, kind="node")
                self.assertEqual(page["matches"], [{"doc_id": 1, "id": "a", "rank": 3}])
                self.assertGreater(stats["database_bytes"], 0)
                self.assertTrue(db.in_transaction)
                self.assertFalse(any(sql.split()[0].upper() in {"BEGIN", "COMMIT", "END", "ROLLBACK", "SAVEPOINT", "RELEASE"} for sql in traced))
                db.set_trace_callback(None)
                with closing(sqlite3.connect(self.path)) as other:
                    self.assertEqual(other.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(), [])
                (db.commit if commit else db.rollback)()
                self.assertEqual(db.execute("SELECT count(*) FROM sqlite_master WHERE type='table'").fetchone()[0] > 0, commit)
            self.assertEqual(db.execute("SELECT id FROM carrier").fetchone(), ("a",))
        self.assertEqual(SearchStore(self.path, binding=self.binding).query_page(kind="node")["matches"], page["matches"])

    def test_owner_delta_failure_never_autocommits_or_closes_connection(self):
        self.publish([self.item("a")])
        changed = PreparedSearchDocument.from_item(1, "node", self.item("b"), 1)
        new_binding = {"revision": "2"}
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("CREATE TABLE carrier (doc_id INTEGER PRIMARY KEY, id TEXT)")
            db.execute("INSERT INTO carrier VALUES (1,'a')")
            db.commit()
            for changes, error in (([SearchChange("update", 1, changed), SearchChange("delete", 99)], SearchInvalidRequest),
                                   ([SearchChange("update", 1, changed)], SearchBudgetExceeded)):
                db.execute("BEGIN IMMEDIATE")
                db.execute("UPDATE carrier SET id='b'")
                traced = []
                db.set_trace_callback(traced.append)
                with self.assertRaises(error):
                    SearchStore.apply_delta_transaction(db, expected_binding=self.binding, new_binding=new_binding, changes=changes,
                                                        max_mutations=1 if error is SearchBudgetExceeded else 100000)
                self.assertTrue(db.in_transaction)
                self.assertEqual(db.execute("SELECT id FROM carrier").fetchone(), ("b",))
                self.assertFalse(any(sql.split()[0].upper() in {"BEGIN", "COMMIT", "END", "ROLLBACK"} for sql in traced))
                db.set_trace_callback(None)
                db.rollback()
                self.assertEqual(db.execute("SELECT id FROM carrier").fetchone(), ("a",))
                self.assertEqual(db.execute("SELECT identifier FROM search_documents").fetchone()[0], b'"a"')
            for commit in (False, True):
                db.execute("BEGIN IMMEDIATE")
                db.execute("UPDATE carrier SET id='b'")
                SearchStore.apply_delta_transaction(db, expected_binding=self.binding, new_binding=new_binding, changes=[SearchChange("update", 1, changed)])
                page = SearchStore.query_transaction(db, binding=new_binding, kind="node")
                self.assertEqual(page["matches"][0]["id"], db.execute("SELECT id FROM carrier").fetchone()[0])
                with self.assertRaises(SearchStaleBinding):
                    SearchStore.query_transaction(db, binding=self.binding, kind="node")
                self.assertTrue(db.in_transaction)
                (db.commit if commit else db.rollback)()
            self.assertEqual(db.execute("SELECT id FROM carrier").fetchone(), ("b",))
        self.assertEqual(SearchStore(self.path, binding=new_binding).query_page(kind="node")["matches"][0]["id"], "b")

    def test_owner_initial_failure_leaves_schema_and_carrier_rollback_to_caller(self):
        document = PreparedSearchDocument.from_item(1, "node", self.item("a"), 1)
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("BEGIN IMMEDIATE")
            db.execute("CREATE TABLE carrier (id TEXT)")
            db.execute("INSERT INTO carrier VALUES ('a')")
            with self.assertRaises(SearchInvalidRequest):
                SearchStore.initialize_transaction(db, binding=self.binding, documents=[document, document])
            self.assertTrue(db.in_transaction)
            self.assertEqual(db.execute("SELECT id FROM carrier").fetchone(), ("a",))
            self.assertEqual(db.execute("SELECT count(*) FROM search_documents").fetchone(), (1,))
            with closing(sqlite3.connect(self.path)) as other:
                self.assertEqual(other.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(), [])
            db.rollback()
            self.assertEqual(db.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall(), [])

    def test_owner_query_checks_header_inside_existing_snapshot(self):
        self.publish([self.item("a"), self.item("b")])
        new_binding = {"revision": "new"}
        with closing(sqlite3.connect(self.path)) as reader, closing(sqlite3.connect(self.path)) as writer:
            writer.execute("PRAGMA journal_mode=WAL")
            writer.execute("CREATE TABLE carrier (doc_id INTEGER PRIMARY KEY, id TEXT)")
            writer.execute("INSERT INTO carrier VALUES (1,'a')")
            writer.commit()
            reader.execute("BEGIN")
            old_carrier = reader.execute("SELECT id FROM carrier").fetchone()[0]
            first = SearchStore.query_transaction(reader, binding=self.binding, kind="node", page_size=1)
            writer.execute("BEGIN IMMEDIATE")
            writer.execute("UPDATE carrier SET id='z'")
            changed = PreparedSearchDocument.from_item(1, "node", self.item("z"), 1)
            SearchStore.apply_delta_transaction(writer, expected_binding=self.binding, new_binding=new_binding, changes=[SearchChange("update", 1, changed)])
            writer.commit()
            old = SearchStore.query_transaction(reader, binding=self.binding, kind="node")
            self.assertEqual(old["matches"][0]["id"], old_carrier)
            resumed = SearchStore.query_transaction(reader, binding=self.binding, kind="node", cursor=first["next_cursor"])
            self.assertEqual(resumed["matches"][0]["id"], "b")
            self.assertTrue(reader.in_transaction)
            reader.rollback()
            reader.execute("BEGIN")
            with self.assertRaises(SearchStaleBinding):
                SearchStore.query_transaction(reader, binding=self.binding, kind="node")
            fresh = SearchStore.query_transaction(reader, binding=new_binding, kind="node")
            self.assertEqual(fresh["matches"][-1]["id"], reader.execute("SELECT id FROM carrier").fetchone()[0])

    def test_owner_transaction_required_and_wholefile_cap_preserved(self):
        document = PreparedSearchDocument.from_item(1, "node", self.item("a"), 1)
        with closing(sqlite3.connect(self.path)) as db:
            for call in (lambda: SearchStore.initialize_transaction(db, binding=self.binding, documents=[document]),
                         lambda: SearchStore.apply_delta_transaction(db, expected_binding=self.binding, new_binding={"revision": 2}, changes=[]),
                         lambda: SearchStore.query_transaction(db, binding=self.binding, kind="node")):
                with self.assertRaises(SearchInvalidRequest):
                    call()
                self.assertFalse(db.in_transaction)
            db.execute("PRAGMA max_page_count=64")
            db.execute("BEGIN")
            db.execute("CREATE TABLE carrier (payload BLOB)")
            db.execute("INSERT INTO carrier VALUES (zeroblob(8192))")
            SearchStore.initialize_transaction(db, binding=self.binding, documents=[document], max_bytes=1048576)
            self.assertEqual(db.execute("SELECT max_pages FROM search_header").fetchone()[0], 64)
            self.assertEqual(db.execute("PRAGMA max_page_count").fetchone()[0], 64)
            db.commit()
            db.execute("PRAGMA max_page_count=48")
            db.row_factory = sqlite3.Row
            db.execute("BEGIN")
            report = SearchStore.apply_delta_transaction(db, expected_binding=self.binding, new_binding={"revision": 2}, changes=[SearchChange("update", 1, document)])
            self.assertEqual(report["blocks_written"], 0)
            self.assertEqual(SearchStore.query_transaction(db, binding={"revision": 2}, kind="node")["matches"][0]["id"], "a")
            self.assertEqual(db.execute("PRAGMA max_page_count").fetchone()[0], 48)
            self.assertEqual(db.execute("SELECT max_pages FROM search_header").fetchone()[0], 48)
            db.rollback()
            db.execute("BEGIN")
            with self.assertRaises(SearchBudgetExceeded):
                SearchStore.initialize_transaction(db, binding=self.binding, documents=[], max_bytes=65536)
            self.assertTrue(db.in_transaction)
            db.rollback()
            db.row_factory = None
            db.execute("BEGIN")
            SearchStore.apply_delta_transaction(db, expected_binding=self.binding, new_binding={"revision": 2}, changes=[])
            db.commit()
        # SQLite's pragma is connection-local. Persisting the narrowed bound
        # prevents a later standalone writer from reverting to the original cap.
        SearchStore.apply_delta(self.path, expected_binding={"revision": 2}, new_binding={"revision": 3}, changes=[])
        with closing(sqlite3.connect(self.path)) as db:
            self.assertEqual(db.execute("SELECT max_pages FROM search_header").fetchone()[0], 48)

    def test_typed_errors_distinguish_expiry_binding_corruption_and_request(self):
        store, _, _ = self.publish([self.item("a"), self.item("b")])
        cursor = store.query_page(kind="node", page_size=1)["next_cursor"]
        with self.assertRaises(SearchInvalidRequest):
            store.query_page(kind="other")
        with self.assertRaises(SearchStaleBinding):
            SearchStore(self.path, binding={"revision": "wrong"})
        with self.assertRaises(SearchCursorError):
            store.query_page(kind="node", cursor={})
        malformed = copy.deepcopy(cursor)
        malformed["mac"] = "é" * 64
        with self.assertRaises(SearchCursorError):
            store.query_page(kind="node", cursor=malformed)
        with patch("tos_access.compressed_search_store.time.time", return_value=cursor["state"]["expires"]):
            with self.assertRaises(SearchCursorExpired):
                store.query_page(kind="node", cursor=cursor)
        with self.assertRaises(SearchUnavailable):
            SearchStore(self.path.with_name("missing.sqlite"), binding=self.binding)
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("UPDATE search_documents SET filters=?", (b"not json",))
            db.commit()
        with self.assertRaises(SearchUnavailable):
            store.query_page(kind="node")


if __name__ == "__main__":
    unittest.main()
