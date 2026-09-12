"""Actual-file atomic local publication and native reference preservation."""
import copy
from contextlib import closing
import json
import os
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

from tos_access.prepared_publication import (
    PreparedChange, PublicationLimits, SCHEMA, apply_prepared_delta,
    apply_prepared_delta_transaction, publish_prepared, publish_prepared_rows,
)
from tos_access.compressed_search_store import SearchStore
from tos_access.compressed_search_bootstrap import BulkBootstrapLimits
from tos_access.published_read_metadata import _compact, published_lens_metadata, LENS_META_KEY
from tos_access.published_read_model import PublishedKnowledgeReadModel, PublishedSnapshotConflict
from tos_access.published_lens import PublishedLensService
from tos_access import knowledge as k
from tos_access.knowledge import inspect_knowledge_node, inspect_knowledge_relation, execute_knowledge_lens
import test_compressed_search_store as search_reference
from test_indexed_lens import lens


def fixture():
    nodes = [k._normalize_node({"node_id": identifier, "node_type": "concept", "label": "common",
              "summary": "Слово", "source_ref": "ToS/synthetic.json"}, "philosophy") for identifier in ("a", "A", "b")]
    relation = k._normalize_relation({"edge_id": "r", "from_id": "a", "to_id": "b", "predicate_id": "related_to",
                  "source_ref": "ToS/synthetic.json"}, "philosophy", {n["id"]: n for n in nodes})
    for identifier, node in zip(("a", "A", "b"), nodes):
        node.update(id=identifier, entity_id=identifier + "-entity", native_id=identifier + "-native",
                    probe={"false": False, "zero": 0, "none": None, "key": "value"})
    relation.update(id="r", from_id="a", to_id="b")
    relations = [relation]
    graph = {"schema": "tos_knowledge_graph_v1", "source_revision": "a" * 64,
             "normalization_binding": {"schema": "tos_knowledge_graph_normalization_binding_v1",
                 **{key: "b" * 64 for key in ("processor_digest", "entity_registry_digest", "relation_registry_digest", "configuration_digest")}},
             "authority_boundary": {"source_owner": "Tree-of-Sophia", "is_source": False, "is_canon": False, "writes_to_tree": False},
             "query_properties": [], "nodes": nodes, "relations": relations}
    catalog = {"schema": "tos_knowledge_catalog_v1", "source_revision": graph["source_revision"], "lenses": []}
    return graph, catalog


class PreparedPublicationTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="prepared-publication-", dir=os.environ.get("TMPDIR"))
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / "publication.sqlite"
        self.graph, self.catalog = fixture()

    def publish(self):
        self.binding = publish_prepared(self.path, graph=self.graph, catalog=self.catalog)
        return PublishedKnowledgeReadModel(self.path, self.binding)

    def header(self, revision="c"):
        result = {key: copy.deepcopy(value) for key, value in self.graph.items() if key not in ("nodes", "relations")}
        result["source_revision"] = revision * 64
        catalog = {**self.catalog, "source_revision": result["source_revision"]}
        return result, catalog

    def delta(self, changes, revision="c", **kwargs):
        header, catalog = self.header(revision)
        return apply_prepared_delta(self.path, expected_binding=self.binding, source_header=header,
                                    catalog=catalog, changes=changes, **kwargs)

    def state(self):
        with closing(sqlite3.connect(self.path)) as db:
            return list(db.iterdump())

    def search(self, binding, query="", kind="node"):
        store = SearchStore(self.path, binding=binding)
        page = store.query_page(kind=kind, query=query, page_size=50)
        self.assertFalse(page["has_more"])
        return page["matches"]

    def test_exact_full_carriers_catalog_lens_and_default_json_search(self):
        reader = self.publish()
        self.assertEqual(reader.status()["read_model_schema"], SCHEMA)
        self.assertEqual(reader.catalog(), self.catalog)
        for item in self.graph["nodes"]:
            self.assertEqual(reader.node(item["id"]), inspect_knowledge_node(self.graph, item["id"]))
        for item in self.graph["relations"]:
            self.assertEqual(reader.relation(item["id"]), inspect_knowledge_relation(self.graph, item["id"]))
        spec = lens(seed={"node_ids": ["a"]})
        self.assertEqual(PublishedLensService(reader).execute(spec), execute_knowledge_lens(self.graph, spec))
        for query in ("", "common", '"key": "value"', "false", "Слово"):
            self.assertEqual(self.search(self.binding, query), search_reference.CompressedSearchStoreTests.reference(self.graph["nodes"], query))
        self.assertNotIn('"key": "value"', _compact(self.graph["nodes"][0]))
        self.assertEqual(self.path.stat().st_mode & 0o777, 0o600)
        self.assertLess(self.path.stat().st_size, 64 * 1024 * 1024)
        with closing(sqlite3.connect(self.path)) as db:
            tables = {row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}
            self.assertFalse(tables & {"knowledge_search_grams", "knowledge_search_documents", "edge_responses"})
            for kind in ("node", "relation"):
                expected = {item["id"]: _compact(item) for item in self.graph[kind + "s"]}
                self.assertEqual(dict(db.execute(f"SELECT id,json FROM knowledge_{kind}s")), expected)

    def test_repeatable_row_factory_matches_list_publication_without_retaining_rows(self):
        import weakref
        class Row(dict):
            pass
        references = []
        calls = []
        def rows(kind):
            calls.append(kind)
            for item in self.graph[kind + "s"]:
                # The writer may retain its current/previous loop item, but
                # never all transformed bodies from an earlier pass.
                self.assertLessEqual(sum(ref() is not None for ref in references), 2)
                row = Row(copy.deepcopy(item))
                references.append(weakref.ref(row))
                yield row
        reader = self.publish()
        header, catalog = self.header("a")
        path = self.path.with_name("stream.sqlite")
        binding = publish_prepared_rows(path, source_header=header, catalog=catalog, row_factory=rows)
        self.assertEqual(calls, ["node", "relation", "node", "relation"])
        self.assertEqual(binding, self.binding)
        streamed = PublishedKnowledgeReadModel(path, binding)
        self.assertEqual(streamed.catalog(), reader.catalog())
        for node in self.graph["nodes"]:
            self.assertEqual(streamed.node(node["id"]), reader.node(node["id"]))
        with closing(sqlite3.connect(path)) as actual, closing(sqlite3.connect(self.path)) as expected:
            for table in ("knowledge_nodes", "knowledge_relations", "knowledge_lens_order", "prepared_documents", "edge_meta"):
                self.assertEqual(actual.execute(f"SELECT * FROM {table} ORDER BY 1,2").fetchall(),
                                 expected.execute(f"SELECT * FROM {table} ORDER BY 1,2").fetchall())

    def test_bulk_publication_preserves_binding_full_readers_and_post_delta_search(self):
        reference = self.publish()
        target = self.path.with_name("bulk.sqlite")
        scratch = self.path.with_name("bulk-scratch.sqlite")
        with patch.object(SearchStore, "initialize_transaction", side_effect=AssertionError("implicit buffered fallback")):
            binding = publish_prepared(target, graph=self.graph, catalog=self.catalog,
                search_scratch_path=scratch,
                search_scratch_limits=BulkBootstrapLimits(8 * 1024 * 1024, 2_000_000))
        self.assertFalse(scratch.exists())
        self.assertEqual(binding, self.binding)
        reader = PublishedKnowledgeReadModel(target, binding)
        self.assertEqual(reader.catalog(), reference.catalog())
        for item in self.graph["nodes"]:
            self.assertEqual(reader.node(item["id"]), reference.node(item["id"]))
        self.assertEqual(reader.relation("r"), reference.relation("r"))
        spec = lens(seed={"node_ids": ["a"]})
        self.assertEqual(PublishedLensService(reader).execute(spec), PublishedLensService(reference).execute(spec))
        header, catalog = self.header()
        changed = copy.deepcopy(self.graph["nodes"][0])
        changed["probe"]["key"] = "corrected source"
        changes = [PreparedChange("update", "node", "a", changed)]
        actual_binding = apply_prepared_delta(target, expected_binding=binding,
            source_header=header, catalog=catalog, changes=changes)
        expected_binding = self.delta(changes)
        self.assertEqual(actual_binding, expected_binding)
        for query in ("", "common", '"key": "corrected source"', "false", "Слово"):
            self.assertEqual(SearchStore(target, binding=actual_binding).query_page(kind="node", query=query)["matches"],
                             self.search(expected_binding, query))
        with self.assertRaises(PublishedSnapshotConflict):
            reader.catalog()

    def test_bulk_combined_publication_budget_is_exact_including_carriers_and_scratch(self):
        scratch = self.path.with_name("scratch.sqlite")
        original = SearchStore.initialize_bulk_transaction
        observations = []
        def measured(db, **kwargs):
            report = original(db, **kwargs)
            observations.append({"combined": db.total_changes + report["scratch_mutations"] + 1,
                                 "search": report["mutations"], "allowance": kwargs["max_mutations"]})
            return report
        options = {"graph": self.graph, "catalog": self.catalog,
                   "search_scratch_path": scratch,
                   "search_scratch_limits": BulkBootstrapLimits(8 * 1024 * 1024, 2_000_000)}
        with patch.object(SearchStore, "initialize_bulk_transaction", side_effect=measured):
            expected = publish_prepared(self.path, **options)
            required = observations[0]["combined"]
            exact = self.path.with_name("exact.sqlite")
            self.assertEqual(publish_prepared(exact, **options, limits=PublicationLimits(max_mutations=required)), expected)
        self.assertEqual(observations[1]["combined"], required)
        self.assertEqual(observations[1]["search"], observations[1]["allowance"])
        refused = self.path.with_name("refused.sqlite")
        with self.assertRaisesRegex(ValueError, "mutation budget"):
            publish_prepared(refused, **options, limits=PublicationLimits(max_mutations=required - 1))
        self.assertFalse(refused.exists())
        self.assertFalse(scratch.exists())
        self.assertEqual(PublishedKnowledgeReadModel(self.path, expected).catalog(), self.catalog)

    def test_bulk_explicit_inputs_and_second_pass_failures_preserve_foreign_files(self):
        limits = BulkBootstrapLimits(8 * 1024 * 1024, 2_000_000)
        scratch = self.path.with_name("scratch.sqlite")
        for options in ({"search_scratch_path": scratch}, {"search_scratch_limits": limits},
                        {"search_scratch_path": scratch, "search_scratch_limits": {}}):
            with self.assertRaises(ValueError):
                publish_prepared(self.path, graph=self.graph, catalog=self.catalog, **options)
            self.assertFalse(self.path.exists())
            self.assertFalse(scratch.exists())
        scratch.touch()
        with self.assertRaises(FileExistsError):
            publish_prepared(self.path, graph=self.graph, catalog=self.catalog,
                search_scratch_path=scratch, search_scratch_limits=limits)
        self.assertFalse(self.path.exists())
        self.assertEqual(scratch.read_bytes(), b"")
        # Existing path is never adopted. A fresh path is disposed on a source
        # failure after carrier/search writes, without selecting a partial DB.
        fresh_scratch = self.path.with_name("fresh-scratch.sqlite")
        header, catalog = self.header("a")
        calls = {"node": 0, "relation": 0}
        def rows(kind):
            calls[kind] += 1
            yield from self.graph[kind + "s"]
            if kind == "relation" and calls[kind] == 2:
                raise RuntimeError("source second pass failed")
        with self.assertRaisesRegex(RuntimeError, "second pass"):
            publish_prepared_rows(self.path, source_header=header, catalog=catalog, row_factory=rows,
                search_scratch_path=fresh_scratch, search_scratch_limits=limits)
        self.assertFalse(self.path.exists())
        self.assertFalse(fresh_scratch.exists())
        self.assertEqual(scratch.read_bytes(), b"")

    def test_row_factory_change_or_failure_never_publishes_a_partial_snapshot(self):
        header, catalog = self.header("a")
        for defect in ("missing", "extra", "changed", "failed"):
            with self.subTest(defect=defect):
                calls = {"node": 0, "relation": 0}
                def rows(kind):
                    calls[kind] += 1
                    material = self.graph[kind + "s"]
                    if kind == "node" and calls[kind] == 2:
                        if defect == "missing":
                            material = material[:-1]
                        elif defect == "extra":
                            material = [*material, {**material[-1], "id": "extra"}]
                        elif defect == "changed":
                            material = [{**item, "new": True} for item in material]
                    yield from material
                    if kind == "relation" and calls[kind] == 2 and defect == "failed":
                        raise RuntimeError("injected iterator failure")
                with self.assertRaises((ValueError, RuntimeError)):
                    publish_prepared_rows(self.path, source_header=header, catalog=catalog, row_factory=rows)
                self.assertFalse(self.path.exists())
        with self.assertRaisesRegex(ValueError, "row collections"):
            publish_prepared_rows(self.path, source_header=self.graph, catalog=catalog, row_factory=lambda _: ())
        with self.assertRaisesRegex(ValueError, "factory"):
            publish_prepared_rows(self.path, source_header=header, catalog=catalog, row_factory=iter([]))
        with self.assertRaisesRegex(ValueError, "row/mutation budget"):
            publish_prepared_rows(self.path, source_header=header, catalog=catalog,
                                  row_factory=lambda kind: iter(self.graph[kind + "s"]),
                                  limits=PublicationLimits(max_mutations=3))
        self.assertFalse(self.path.exists())

    def test_exclusive_initial_and_failure_cleanup(self):
        self.publish()
        original = self.state()
        with self.assertRaises(FileExistsError):
            self.publish()
        self.assertEqual(original, self.state())
        for alteration in ("endpoint", "duplicate", "budget"):
            target = self.path.with_name(alteration + ".sqlite")
            graph = copy.deepcopy(self.graph)
            limits = PublicationLimits(max_mutations=1) if alteration == "budget" else None
            if alteration == "endpoint":
                graph["relations"][0]["to_id"] = "foreign"
            if alteration == "duplicate":
                graph["nodes"].append(graph["nodes"][0])
            with self.assertRaises((ValueError, sqlite3.Error)):
                publish_prepared(target, graph=graph, catalog=self.catalog, limits=limits)
            self.assertFalse(target.exists())
        graph = copy.deepcopy(self.graph)
        graph["nodes"][0]["oversize"] = "x" * 1_048_576
        target = self.path.with_name("oversize.sqlite")
        with self.assertRaisesRegex(ValueError, "row byte"):
            publish_prepared(target, graph=graph, catalog=self.catalog)
        self.assertFalse(target.exists())
        with self.assertRaises(sqlite3.Error):
            publish_prepared(target, graph=self.graph, catalog=self.catalog,
                             limits=PublicationLimits(max_bytes=65536))
        self.assertFalse(target.exists())

    def test_delta_stable_map_add_edit_delete_and_stale_aba(self):
        old_reader = self.publish()
        old_binding = self.binding
        original = copy.deepcopy(self.graph["nodes"][1])
        changed = {**original, "probe": {"edited": True}}
        inserted = {**copy.deepcopy(original), "id": "C", "native_id": "new", "entity_id": "new"}
        self.binding = self.delta([PreparedChange("update", "node", "A", changed),
                                   PreparedChange("insert", "node", "C", inserted, 123)])
        with closing(sqlite3.connect(self.path)) as db:
            mapping = {row[0]: row[1:] for row in db.execute("SELECT id,doc_id,source_order FROM prepared_documents WHERE kind='node'")}
        self.assertEqual(mapping["A"], (2, 2**32))
        self.assertEqual(mapping["C"], (5, 123))
        self.assertEqual([r["id"] for r in self.search(self.binding)], ["a", "A", "b", "C"])
        with self.assertRaises(PublishedSnapshotConflict):
            old_reader.catalog()
        with self.assertRaises(ValueError):
            SearchStore(self.path, binding=old_binding)
        header, catalog = self.header()
        with self.assertRaisesRegex(ValueError, "stale"):
            apply_prepared_delta(self.path, expected_binding=old_binding, source_header=header, catalog=catalog, changes=[])
        self.binding = self.delta([PreparedChange("delete", "node", "C"), PreparedChange("update", "node", "A", original)], "a")
        self.assertNotEqual(self.binding, old_binding)
        self.binding = self.delta([PreparedChange("insert", "node", "C", inserted, 123)], "d")
        with closing(sqlite3.connect(self.path)) as db:
            self.assertEqual(db.execute("SELECT doc_id FROM prepared_documents WHERE id='C'").fetchone()[0], 6)
            self.assertEqual(db.execute("SELECT high_water FROM prepared_state").fetchone(), db.execute("SELECT high_water FROM search_header").fetchone())

    def test_addressed_endpoint_closure_and_histograms(self):
        self.publish()
        before = self.state()
        with self.assertRaisesRegex(ValueError, "incident"):
            self.delta([PreparedChange("delete", "node", "a")])
        self.assertEqual(before, self.state())
        self.binding = self.delta([PreparedChange("delete", "relation", "r"), PreparedChange("delete", "node", "a")])
        graph = {**self.graph, "nodes": self.graph["nodes"][1:], "relations": [], "source_revision": "c" * 64}
        with closing(sqlite3.connect(self.path)) as db:
            actual = json.loads("".join(row[0] for row in db.execute("SELECT json_chunk FROM edge_meta WHERE key=? ORDER BY part", (LENS_META_KEY,))))
        self.assertEqual(actual, published_lens_metadata(graph))
        self.assertEqual(self.search(self.binding, kind="relation"), [])

    def test_full_owner_transaction_rollback_including_search_and_sentinel(self):
        self.publish()
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("CREATE TABLE sentinel(value TEXT)")
            db.commit()
        before = self.state()
        header, catalog = self.header()
        with closing(sqlite3.connect(self.path, isolation_level=None)) as db:
            db.execute("BEGIN IMMEDIATE")
            db.execute("INSERT INTO sentinel VALUES ('pending')")
            changed = {**self.graph["nodes"][0], "probe": {"new": "atomic marker"}}
            pending = apply_prepared_delta_transaction(db, expected_binding=self.binding, source_header=header,
                catalog=catalog, changes=[PreparedChange("update", "node", "a", changed)])
            page = SearchStore.query_transaction(db, binding=pending, kind="node", query="atomic marker")
            self.assertEqual([r["id"] for r in page["matches"]], ["a"])
            db.execute("ROLLBACK")
        self.assertEqual(before, self.state())
        self.assertEqual(self.search(self.binding, "atomic marker"), [])
        with self.assertRaises((ValueError, sqlite3.Error)):
            self.delta([PreparedChange("update", "node", "a", changed)], limits=PublicationLimits(max_mutations=1))
        self.assertEqual(before, self.state())
        # An actual SQL failure after compressed-search publication must undo
        # every lane, including its new cursor incarnation and whole rows.
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("CREATE TRIGGER refuse_finish BEFORE UPDATE ON prepared_state BEGIN SELECT RAISE(ABORT, 'fixture late failure'); END")
            db.commit()
        before = self.state()
        with self.assertRaisesRegex(sqlite3.IntegrityError, "late failure"):
            self.delta([PreparedChange("update", "node", "a", changed)])
        self.assertEqual(before, self.state())

    def test_foreign_binding_normalization_budget_and_highwater_refusals(self):
        self.publish()
        before = self.state()
        header, catalog = self.header()
        header["normalization_binding"]["processor_digest"] = "d" * 64
        with self.assertRaisesRegex(ValueError, "normalization"):
            apply_prepared_delta(self.path, expected_binding=self.binding, source_header=header, catalog=catalog, changes=[])
        self.assertEqual(before, self.state())
        with self.assertRaisesRegex(ValueError, "source order"):
            self.delta([PreparedChange("insert", "node", "C", {**self.graph["nodes"][0], "id": "C"})])
        self.assertEqual(before, self.state())
        with self.assertRaises(ValueError):
            self.delta([], limits=PublicationLimits(max_bytes=65536))
        self.assertEqual(before, self.state())
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("UPDATE prepared_state SET high_water=99")
            db.commit()
        with self.assertRaisesRegex(ValueError, "high-water"):
            self.delta([])

    def test_caller_page_cap_is_not_raised_and_order_change_is_explicit(self):
        self.publish()
        header, catalog = self.header()
        with closing(sqlite3.connect(self.path, isolation_level=None)) as db:
            maximum = db.execute("PRAGMA page_count").fetchone()[0] + 64
            db.execute(f"PRAGMA max_page_count={maximum}")
            db.execute("BEGIN IMMEDIATE")
            pending = apply_prepared_delta_transaction(db, expected_binding=self.binding, source_header=header,
                catalog=catalog, changes=[PreparedChange("update", "node", "a", self.graph["nodes"][0], 2**33)])
            self.assertLessEqual(db.execute("PRAGMA max_page_count").fetchone()[0], maximum)
            self.assertLessEqual(db.execute("SELECT max_pages FROM search_header").fetchone()[0], maximum)
            page = SearchStore.query_transaction(db, binding=pending, kind="node")
            self.assertEqual([r["id"] for r in page["matches"]], ["A", "a", "b"])
            db.execute("ROLLBACK")
        self.assertEqual([r["id"] for r in self.search(self.binding)], ["a", "A", "b"])


if __name__ == "__main__":
    unittest.main()
