"""Exact full-wrapper parity and transaction/budget fences, not admission proof."""
import copy
from contextlib import closing
from dataclasses import replace
import json
import os
from pathlib import Path
import random
import sqlite3
import tempfile
import unittest
from unittest.mock import patch

from tos_access import knowledge as k
from tos_access import semantic_index as s
from tos_access.prepared_publication import (
    PreparedChange, SOURCE_ORDER_STRIDE, apply_prepared_delta_transaction, publish_prepared,
)
from tos_access.published_read_metadata import _compact, emitted_row_digest, published_row_digest_key
from test_prepared_publication import fixture


class SemanticIndexTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        root = Path(__file__).resolve().parents[2]
        cls.entities = json.loads((root / "ToS/doctrine/semantic-interchange/entity-types.v1.json").read_text())
        cls.relations = json.loads((root / "ToS/doctrine/semantic-interchange/relation-types.v1.json").read_text())

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="semantic-index-", dir=os.environ.get("TMPDIR"))
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / "prepared.sqlite"
        self.graph, self.catalog = fixture()
        self.graph["normalization_binding"] = k._normalization_binding(self.entities, self.relations)
        self.serial = 0

    def full(self, graph=None):
        return k.validate_knowledge_semantics(graph or self.graph, self.entities, self.relations)

    def open(self, *, bootstrap=True):
        self.graph["counts"] = {"semantic_validation": self.full()}
        self.binding = publish_prepared(self.path, graph=self.graph, catalog=self.catalog)
        self.db = sqlite3.connect(self.path, isolation_level=None)
        self.addCleanup(self.db.close)
        if bootstrap:
            self.db.execute("BEGIN IMMEDIATE")
            report = s.bootstrap_semantic_index_transaction(self.db, binding=self.binding,
                entity_registry=self.entities, relation_registry=self.relations,
                ordered_rows=lambda kind: iter(self.graph[kind + "s"]))
            self.assertEqual(report, self.full())
            s.verify_semantic_index_binding_transaction(self.db, self.binding)
            self.db.execute("COMMIT")
        return self.db

    def candidate(self, changes):
        graph = copy.deepcopy(self.graph)
        for change in changes:
            rows = graph[change.kind + "s"]
            if change.operation != "insert":
                rows[:] = [row for row in rows if row["id"] != change.identifier]
            if change.operation != "delete":
                rows.append(copy.deepcopy(change.item))
        for kind in s.KINDS:
            orders = dict(self.db.execute("SELECT id,source_order FROM prepared_documents WHERE kind=?", (kind,)))
            for change in changes:
                if change.kind == kind and change.source_order is not None:
                    orders[change.identifier] = change.source_order
            graph[kind + "s"].sort(key=lambda row: orders[row["id"]])
        return graph

    def delta(self, changes, *, commit=True):
        candidate = self.candidate(changes)
        self.serial += 1
        revision = f"{self.serial:064x}"
        self.db.execute("BEGIN IMMEDIATE")
        # A bounded delta must not invoke the complete wrapper as its validator.
        with patch.object(k, "validate_knowledge_semantics", side_effect=AssertionError("full scan")):
            report = s.apply_semantic_delta_transaction(self.db, expected_binding=self.binding,
                new_source_revision=revision, changes=changes,
                entity_registry=self.entities, relation_registry=self.relations)
        self.assertEqual(report, self.full(candidate))
        candidate.update(source_revision=revision)
        candidate["counts"] = {"semantic_validation": report}
        catalog = {**self.catalog, "source_revision": revision}
        header = {key: value for key, value in candidate.items() if key not in ("nodes", "relations")}
        binding = apply_prepared_delta_transaction(self.db, expected_binding=self.binding,
            source_header=header, catalog=catalog, changes=changes)
        receipt = s.verify_semantic_index_binding_transaction(self.db, binding)
        self.assertEqual(receipt["binding"], binding)
        if commit:
            self.db.execute("COMMIT")
            self.binding, self.graph, self.catalog = binding, candidate, catalog
        return report

    def node(self, identifier, type_id="tos.entity.concept", **extra):
        node = copy.deepcopy(self.graph["nodes"][0])
        node.update(id=identifier, entity_id=identifier + "-entity", type_id=type_id)
        node.update(extra)
        return node

    def relation(self, identifier, left="a", right="b", type_id="tos.relation.related", **extra):
        row = copy.deepcopy(self.graph["relations"][0])
        row.update(id=identifier, from_id=left, to_id=right, relation_type_id=type_id, **extra)
        return row

    def claim(self, identifier, *, entity="same-claim", left="a", right="b", evidence=None):
        return self.node(identifier, "tos.entity.claim", semantics={"claim": {
            "claim_id": entity, "claim_version": 1, "subject_node_id": left,
            "object_node_id": right, "subject_entity_id": left + "-entity",
            "object_entity_id": right + "-entity", "relation_type_id": "tos.relation.same-as",
            "evidence_node_ids": [] if evidence is None else evidence}},
            **{"entity_id": entity})

    def test_bootstrap_uses_exact_rows_and_does_not_inherit_valid(self):
        self.open()
        self.assertFalse(self.full()["valid"])
        state = json.loads(self.db.execute("SELECT json FROM semantic_state").fetchone()[0])
        self.assertEqual(state["report_digest"], emitted_row_digest(_compact(self.full()))["sha256"])
        self.assertFalse(state["pending"])

    def test_node_relation_insert_update_delete_and_reordering(self):
        self.open()
        node = self.node("Ω")
        self.delta([PreparedChange("insert", "node", "Ω", node, SOURCE_ORDER_STRIDE // 2)])
        changed = copy.deepcopy(node)
        changed.update(type_id="foreign", source_refs=[])
        self.delta([PreparedChange("update", "node", "Ω", changed)])
        rel = self.relation("new", "Ω", "a", epistemic={"review_posture": "not-recorded"})
        self.delta([PreparedChange("insert", "relation", "new", rel, 1)])
        self.delta([PreparedChange("update", "relation", "new", rel, SOURCE_ORDER_STRIDE * 8)])
        self.delta([PreparedChange("delete", "relation", "new"), PreparedChange("delete", "node", "Ω")])
        self.assertEqual(self.db.execute("SELECT COUNT(*) FROM semantic_pending").fetchone()[0], 0)

    def test_last_claim_wins_order_and_dependent_same_as_evidence_review(self):
        first = self.claim("claim-first", evidence=["evidence"])
        last = self.claim("claim-last", left="b", right="a", evidence=["evidence"])
        evidence = self.node("evidence", "tos.entity.evidence")
        review = self.node("review", "tos.entity.review", attributes={"claim_ref": "same-claim", "claim_version": 1, "decision": "accepted"})
        same = self.relation("same", type_id="tos.relation.same-as", attributes={"claim_ref": "same-claim", "review_node_id": "review"},
                             epistemic={"review_posture": "accepted"}, source_refs=["synthetic"])
        self.graph["nodes"].extend([first, last, evidence, review])
        self.graph["relations"].append(same)
        self.open()
        error = "same_as relation same lacks resolved evidence and exact-version review"
        self.assertNotIn(error, self.full()["violations"])
        bad = copy.deepcopy(evidence)
        bad["type_id"] = "tos.entity.concept"
        self.assertIn(error, self.delta([PreparedChange("update", "node", "evidence", bad)])["violations"])
        self.assertNotIn(error, self.delta([PreparedChange("update", "node", "evidence", evidence)])["violations"])
        self.delta([PreparedChange("update", "node", "claim-first", first, SOURCE_ORDER_STRIDE * 8)])
        broken = copy.deepcopy(first)
        broken["semantics"]["claim"]["object_entity_id"] = "wrong"
        self.assertIn(error, self.delta([PreparedChange("update", "node", "claim-first", broken)])["violations"])
        self.assertNotIn(error, self.delta([PreparedChange("delete", "node", "claim-first")])["violations"])

    def test_claim_outgoing_exactness_gaps_and_non_global_minimums(self):
        claim = self.claim("claim")
        self.graph["nodes"].append(claim)
        self.open()
        error = "claim claim must have exactly one consistent tos.relation.has-subject"
        self.assertIn(error, self.full()["violations"])
        subject = self.relation("subject", "claim", "a", "tos.relation.has-subject")
        obj = self.relation("object", "claim", "b", "tos.relation.has-object")
        report = self.delta([PreparedChange("insert", "relation", "subject", subject, 1),
                             PreparedChange("insert", "relation", "object", obj, 2)])
        self.assertNotIn(error, report["violations"])
        self.assertEqual(report["gaps"][-1], {"id": "claim", "kind": "claim-evidence-not-projected"})
        subject["to_id"] = "b"
        self.assertIn(error, self.delta([PreparedChange("update", "relation", "subject", subject)])["violations"])
        self.assertIn(error, self.delta([PreparedChange("delete", "relation", "subject")])["violations"])

    def test_scoped_cardinality_python_scalar_equality_and_repair(self):
        self.relations = copy.deepcopy(self.relations)
        entry = next(row for row in self.relations["relations"] if row["relation_type_id"] == "tos.relation.same-as")
        entry["cardinality"]["per_subject_max"] = 1
        self.graph["normalization_binding"] = k._normalization_binding(self.entities, self.relations)
        self.graph["relations"].extend([
            self.relation("one", type_id="tos.relation.same-as", attributes={"claim_ref": True}),
            self.relation("two", type_id="tos.relation.same-as", attributes={"claim_ref": 1.0}),
        ])
        self.open()
        error = "a violates tos.relation.same-as per_subject_max=1"
        self.assertIn(error, self.full()["violations"])
        two = copy.deepcopy(self.graph["relations"][-1])
        two["attributes"]["claim_ref"] = "1"
        self.assertNotIn(error, self.delta([PreparedChange("update", "relation", "two", two)])["violations"])
        self.delta([PreparedChange("delete", "relation", "one")])

    def test_many_deltas_match_full_report_without_unrelated_row_reads(self):
        self.graph["nodes"].extend(self.node(f"unrelated-{i}") for i in range(20))
        self.open()
        rng = random.Random(741)
        for step in range(12):
            node = copy.deepcopy(self.graph["nodes"][0])
            node["source_refs"] = ["synthetic"] if rng.randrange(2) else []
            node["type_id"] = rng.choice(["tos.entity.place", "tos.entity.concept", "foreign"])
            original = s._stored_row
            touched = []
            def record(budget, kind, identifier):
                touched.append(identifier)
                return original(budget, kind, identifier)
            with patch.object(s, "_stored_row", side_effect=record):
                self.delta([PreparedChange("update", "node", "a", node)])
            self.assertFalse(any(identifier.startswith("unrelated-") for identifier in touched))

    def test_ordered_bootstrap_refuses_substitution_omission_and_duplicate_token(self):
        self.open(bootstrap=False)
        for mode in ("substitution", "omission", "order", "duplicate"):
            with self.subTest(mode=mode):
                self.db.execute("BEGIN IMMEDIATE")
                rows = copy.deepcopy(self.graph)
                if mode == "substitution":
                    rows["nodes"][0]["entity_id"] = "foreign"
                elif mode == "omission":
                    rows["nodes"].pop()
                elif mode == "order":
                    rows["nodes"].reverse()
                else:
                    self.db.execute("UPDATE prepared_documents SET source_order=0 WHERE kind='node'")
                with self.assertRaises(ValueError):
                    s.bootstrap_semantic_index_transaction(self.db, binding=self.binding,
                        entity_registry=self.entities, relation_registry=self.relations,
                        ordered_rows=lambda kind: iter(rows[kind + "s"]))
                self.db.execute("ROLLBACK")
                self.assertFalse(self.db.execute("SELECT 1 FROM sqlite_master WHERE name='semantic_state'").fetchone())

    def test_unordered_existing_row_bootstrap_and_empty_graph(self):
        self.graph["nodes"], self.graph["relations"] = [], []
        self.open(bootstrap=False)
        self.db.execute("BEGIN IMMEDIATE")
        report = s.bootstrap_semantic_index_transaction(self.db, binding=self.binding,
            entity_registry=self.entities, relation_registry=self.relations)
        self.assertEqual(report, self.full())
        s.verify_semantic_index_binding_transaction(self.db, self.binding)
        self.db.execute("ROLLBACK")

    def test_pending_refuses_second_delta_and_wrong_final_report_then_rolls_back(self):
        self.open()
        before = list(self.db.iterdump())
        row = copy.deepcopy(self.graph["nodes"][0])
        row["source_refs"] = []
        changes = [PreparedChange("update", "node", "a", row)]
        self.db.execute("BEGIN IMMEDIATE")
        args = dict(expected_binding=self.binding, new_source_revision="c" * 64, changes=changes,
                    entity_registry=self.entities, relation_registry=self.relations)
        report = s.apply_semantic_delta_transaction(self.db, **args)
        with self.assertRaisesRegex(ValueError, "pending"):
            s.apply_semantic_delta_transaction(self.db, **args)
        with self.assertRaisesRegex(ValueError, "final binding"):
            s.verify_semantic_index_binding_transaction(self.db, self.binding)
        header = {key: copy.deepcopy(value) for key, value in self.graph.items() if key not in ("nodes", "relations")}
        header.update(source_revision="c" * 64, counts={"semantic_validation": {**report, "valid": True}})
        new = apply_prepared_delta_transaction(self.db, expected_binding=self.binding, source_header=header,
            catalog={**self.catalog, "source_revision": "c" * 64}, changes=changes)
        with self.assertRaisesRegex(ValueError, "computed report"):
            s.verify_semantic_index_binding_transaction(self.db, new)
        self.db.execute("ROLLBACK")
        self.assertEqual(list(self.db.iterdump()), before)

    def test_exact_row_digest_registry_processor_and_final_replacement_drift_refused(self):
        self.open()
        change = PreparedChange("update", "node", "a", copy.deepcopy(self.graph["nodes"][0]))
        for mutation in ("row", "registry", "processor"):
            with self.subTest(mutation=mutation):
                self.db.execute("BEGIN IMMEDIATE")
                entities = copy.deepcopy(self.entities)
                expected = copy.deepcopy(self.binding)
                if mutation == "row":
                    self.db.execute("UPDATE knowledge_nodes SET json='{}' WHERE id='a'")
                elif mutation == "registry":
                    entities["property_definitions"] = []
                else:
                    state = json.loads(self.db.execute("SELECT json FROM semantic_state").fetchone()[0])
                    state["dependencies"]["projector"] = "0" * 64
                    self.db.execute("UPDATE semantic_state SET json=?", (_compact(state),))
                with self.assertRaises(ValueError):
                    s.apply_semantic_delta_transaction(self.db, expected_binding=expected,
                        new_source_revision="c" * 64, changes=[change],
                        entity_registry=entities, relation_registry=self.relations)
                self.db.execute("ROLLBACK")

    def test_operation_wide_budgets_and_transaction_requirement(self):
        self.open()
        row = self.graph["nodes"][0]
        args = dict(expected_binding=self.binding, new_source_revision="c" * 64,
            changes=[PreparedChange("update", "node", "a", row)],
            entity_registry=self.entities, relation_registry=self.relations)
        with self.assertRaisesRegex(ValueError, "transaction"):
            s.apply_semantic_delta_transaction(self.db, **args)
        for name in ("max_rows", "max_queries", "max_writes", "max_read_bytes", "max_input_bytes",
                     "max_input_values", "max_row_bytes", "max_output_bytes", "max_output_items", "max_bytes"):
            with self.subTest(limit=name):
                before = list(self.db.iterdump())
                self.db.execute("BEGIN IMMEDIATE")
                with self.assertRaises(ValueError):
                    s.apply_semantic_delta_transaction(self.db, **args,
                        limits=replace(s.SemanticIndexLimits(), **{name: 1}))
                self.db.execute("ROLLBACK")
                self.assertEqual(list(self.db.iterdump()), before)

    def test_canonical_profile_refusal_and_missing_relation_sentinel_parity(self):
        self.graph["relations"][0]["id"] = "<missing relation id>"
        self.open()
        self.assertIn("duplicate or missing relation id <missing relation id>", self.full()["violations"])
        self.db.execute("BEGIN IMMEDIATE")
        with self.assertRaisesRegex(ValueError, "canonical"):
            s.apply_semantic_delta_transaction(self.db, expected_binding=self.binding,
                new_source_revision="c" * 64, changes=[PreparedChange("insert", "node", " alias ", self.node(" alias "), 1)],
                entity_registry=self.entities, relation_registry=self.relations)
        self.db.execute("ROLLBACK")

    def test_bootstrap_recomputes_and_refuses_foreign_header_valid_flag(self):
        self.graph["counts"] = {"semantic_validation": {**self.full(), "valid": True}}
        self.binding = publish_prepared(self.path, graph=self.graph, catalog=self.catalog)
        with closing(sqlite3.connect(self.path)) as db:
            db.execute("BEGIN IMMEDIATE")
            with self.assertRaisesRegex(ValueError, "computed report"):
                s.bootstrap_semantic_index_transaction(db, binding=self.binding,
                    entity_registry=self.entities, relation_registry=self.relations)
            db.rollback()
            self.assertFalse(db.execute("SELECT 1 FROM sqlite_master WHERE name='semantic_state'").fetchone())

    def test_final_extra_or_different_prepared_replacements_refuse(self):
        self.open()
        for mode in ("extra", "different", "missing"):
            with self.subTest(mode=mode):
                self.db.execute("BEGIN IMMEDIATE")
                changed = copy.deepcopy(self.graph["nodes"][0])
                changes = [PreparedChange("update", "node", "a", changed)]
                report = s.apply_semantic_delta_transaction(self.db, expected_binding=self.binding,
                    new_source_revision="c" * 64, changes=changes,
                    entity_registry=self.entities, relation_registry=self.relations)
                actual = list(changes)
                if mode == "extra":
                    actual.append(PreparedChange("update", "node", "A", copy.deepcopy(self.graph["nodes"][1])))
                elif mode == "different":
                    changed["source_refs"] = ["different"]
                else:
                    actual = []
                header = {key: copy.deepcopy(value) for key, value in self.graph.items() if key not in ("nodes", "relations")}
                header.update(source_revision="c" * 64, counts={"semantic_validation": report})
                new = apply_prepared_delta_transaction(self.db, expected_binding=self.binding, source_header=header,
                    catalog={**self.catalog, "source_revision": "c" * 64}, changes=actual)
                with self.assertRaisesRegex(ValueError, "(change set|carrier/order)"):
                    s.verify_semantic_index_binding_transaction(self.db, new)
                self.db.execute("ROLLBACK")

    def test_mutable_registry_and_binding_inputs_detached_before_generator(self):
        self.open()
        self.db.execute("BEGIN IMMEDIATE")
        entities = copy.deepcopy(self.entities)
        binding = copy.deepcopy(self.binding)
        original = copy.deepcopy(self.graph["nodes"][0])
        def changes():
            entities["property_definitions"] = []
            binding["publication_epoch"] = 999
            yield PreparedChange("update", "node", "a", original)
        report = s.apply_semantic_delta_transaction(self.db, expected_binding=binding,
            new_source_revision="c" * 64, changes=changes(),
            entity_registry=entities, relation_registry=self.relations)
        self.assertEqual(report, self.full())
        state = json.loads(self.db.execute("SELECT json FROM semantic_state").fetchone()[0])
        self.assertEqual(state["next_epoch"], self.binding["publication_epoch"] + 1)
        self.db.execute("ROLLBACK")

    def test_valid_baseline_and_projection_endpoint_dependency(self):
        entries, mappings, fallback = k._entity_registry_indexes(self.entities)
        self.graph["nodes"] = [k._normalize_node({"node_id": "valid", "node_type": "concept",
            "label": "Synthetic", "source_ref": "synthetic:fixture"}, "philosophy",
            entity_type_entries=entries, entity_type_mappings=mappings, fallback_type_id=fallback)]
        self.graph["relations"] = []
        self.assertTrue(self.full()["valid"], self.full()["violations"])
        self.open()
        duplicate = copy.deepcopy(self.graph["nodes"][0])
        duplicate["id"] = "another"
        self.delta([PreparedChange("insert", "node", "another", duplicate, 1)])
        relation = {"id": "projection", "from_id": self.graph["nodes"][0]["id"], "to_id": "another",
            "relation_type_id": "tos.relation.projects", "source_graph": "semantic-interchange",
            "predicate_id": "projects", "predicate_mapping": {"status": "mapped", "source_predicate_id": "projects"},
            "source_refs": ["synthetic:fixture"], "epistemic": {"review_posture": "not-recorded"}}
        report = self.delta([PreparedChange("insert", "relation", "projection", relation, 0)])
        error = "projection relation projection connects different entity_id values"
        self.assertNotIn(error, report["violations"])
        duplicate["entity_id"] = "different-entity"
        self.assertIn(error, self.delta([PreparedChange("update", "node", "another", duplicate)])["violations"])

    def test_existing_order_bootstrap_without_supplied_rows(self):
        self.graph["nodes"].reverse()
        self.open(bootstrap=False)
        self.db.execute("BEGIN IMMEDIATE")
        self.assertEqual(s.bootstrap_semantic_index_transaction(self.db, binding=self.binding,
            entity_registry=self.entities, relation_registry=self.relations), self.full())
        actual = [row[0] for row in self.db.execute("SELECT id FROM semantic_rows WHERE kind='node' ORDER BY source_order")]
        self.assertEqual(actual, [row["id"] for row in self.graph["nodes"]])
        self.db.execute("ROLLBACK")

    def test_operation_totals_exact_boundary_and_one_below(self):
        self.open()
        args = dict(expected_binding=self.binding, new_source_revision="c" * 64,
            changes=[PreparedChange("update", "node", "a", copy.deepcopy(self.graph["nodes"][0]))],
            entity_registry=self.entities, relation_registry=self.relations)
        budgets = []
        original = s._Budget
        def capture(db, limits):
            budget = original(db, limits)
            budgets.append(budget)
            return budget
        self.db.execute("BEGIN IMMEDIATE")
        with patch.object(s, "_Budget", side_effect=capture):
            s.apply_semantic_delta_transaction(self.db, **args)
        observed = budgets[-1]
        totals = {"max_rows": observed.rows, "max_queries": observed.queries,
                  "max_read_bytes": observed.read_bytes,
                  "max_writes": self.db.total_changes - observed.initial_writes}
        self.db.execute("ROLLBACK")
        for field, total in totals.items():
            for adjustment in (0, -1):
                with self.subTest(field=field, adjustment=adjustment):
                    self.db.execute("BEGIN IMMEDIATE")
                    limits = replace(s.SemanticIndexLimits(), **{field: total + adjustment})
                    if adjustment:
                        with self.assertRaises(ValueError):
                            s.apply_semantic_delta_transaction(self.db, **args, limits=limits)
                    else:
                        self.assertEqual(s.apply_semantic_delta_transaction(self.db, **args, limits=limits), self.full())
                    self.db.execute("ROLLBACK")


if __name__ == "__main__":
    unittest.main()
