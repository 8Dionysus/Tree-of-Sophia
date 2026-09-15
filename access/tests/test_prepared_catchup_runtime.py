"""Bounded D1 catch-up from an admitted predecessor to one current prepared file."""
import copy
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[2]
sys.path[:0] = [str(ROOT / "access/tests"), str(ROOT / "access/deploy/cloudflare-worker/scripts")]

import build_runtime as full
import prepared_delta_runtime as delta
import test_prepared_delta_runtime as prepared_fixture
from tos_access.prepared_publication import PreparedChange, SOURCE_ORDER_STRIDE
from tos_access.prepared_source_binding import (
    PreparedSourceInputs,
    apply_source_bound_prepared_delta_transaction,
)


class PreparedD1CatchupTests(unittest.TestCase):
    """Reuse the tiny prepared/D1 fixture without supplying its old DB copy."""

    @classmethod
    def setUpClass(cls):
        prepared_fixture.PreparedD1DeltaTests.setUpClass()

    def setUp(self):
        prepared_fixture.PreparedD1DeltaTests.setUp(self)

    def full(self, *args, **kwargs):
        return prepared_fixture.PreparedD1DeltaTests.full(self, *args, **kwargs)

    def change(self):
        return prepared_fixture.PreparedD1DeltaTests.change(self)

    def serving_rows(self, db=None):
        db = self.d1 if db is None else db
        return {
            table: db.execute(
                f"SELECT * FROM {table} ORDER BY " + ",".join(delta.PRIMARY_KEYS[table])
            ).fetchall()
            for table in delta.COLUMNS
            if db.execute(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?", (table,)
            ).fetchone()
        }

    def catchup(self, result, name="catchup.sql", *, before_source_inputs=None, **options):
        before_source_inputs = self.source.before if before_source_inputs is None else before_source_inputs
        for db in (self.d1, self.f.db):
            if not db.in_transaction:
                db.execute("BEGIN")
        try:
            return delta.build_prepared_catchup_sql(
                self.d1,
                self.f.db,
                self.root / name,
                expected_d1_revision="d" * 64,
                before_source_inputs=before_source_inputs,
                after_binding=result["binding"],
                **options,
            )
        finally:
            for db in (self.d1, self.f.db):
                db.rollback()

    def assert_full_producer_equivalent(self, oracle):
        actual = self.serving_rows()
        expected = self.serving_rows(db=oracle)
        for table in actual:
            if table in ("knowledge_search_documents", "knowledge_search_grams"):
                continue
            self.assertEqual(actual[table], expected[table], table)

        document_columns = ",".join(
            column for column in delta.DOCUMENT_COLUMNS if column != "position"
        )
        self.assertEqual(
            self.d1.execute(
                f"SELECT {document_columns} FROM knowledge_search_documents ORDER BY kind,id"
            ).fetchall(),
            oracle.execute(
                f"SELECT {document_columns} FROM knowledge_search_documents ORDER BY kind,id"
            ).fetchall(),
        )
        postings = (
            "SELECT g.kind,g.n,g.gram,d.id FROM knowledge_search_grams g "
            "JOIN knowledge_search_documents d ON d.kind=g.kind AND d.position=g.position "
            "ORDER BY g.kind,g.n,g.gram,d.id"
        )
        self.assertEqual(self.d1.execute(postings).fetchall(), oracle.execute(postings).fetchall())
        self.assertEqual(
            self.d1.execute(
                "SELECT id FROM knowledge_search_documents ORDER BY kind,id_lower,position"
            ).fetchall(),
            oracle.execute(
                "SELECT id FROM knowledge_search_documents ORDER BY kind,id_lower,position"
            ).fetchall(),
        )

    def test_catchup_matches_full_producer_replays_and_reverses(self):
        graph, result = self.change()
        original = self.serving_rows()
        with patch.object(full, "build_read_model_sql", side_effect=AssertionError("whole producer")):
            receipt = self.catchup(result, rollback_target=self.root / "rollback.sql")

        self.assertTrue(receipt["whole_manifest_reconciliation"])
        self.assertFalse(receipt["prepared_source_pairing_verified"])
        self.assertTrue(receipt["successor_prepared_source_pairing_verified"])
        self.assertFalse(receipt["predecessor_prepared_source_pairing_verified"])
        self.assertTrue(receipt["predecessor_source_admission_external"])
        self.assertFalse(receipt["d1_applied"])
        oracle = self.full(graph, result["catalog"], receipt["target_d1_revision"], "catchup-oracle")

        forward = (self.root / "catchup.sql").read_text(encoding="utf-8")
        reverse = (self.root / "rollback.sql").read_text(encoding="utf-8")
        self.d1.executescript(forward)
        published = self.serving_rows()
        self.assert_full_producer_equivalent(oracle)
        self.d1.executescript(forward)
        self.assertEqual(self.serving_rows(), published)
        self.d1.executescript(reverse)
        self.assertEqual(self.serving_rows(), original)
        self.d1.executescript(reverse)
        self.assertEqual(self.serving_rows(), original)

    def test_wrong_source_selection_refuses_before_sql(self):
        _, result = self.change()
        previous = self.source.before.value()
        wrong = PreparedSourceInputs(
            source_revision="f" * 64,
            source_publication=previous["source_publication"],
            dependencies=previous["dependencies"],
            roots=self.source.before.roots(),
        )
        with self.assertRaisesRegex(ValueError, "D1 predecessor source selection differs"):
            self.catchup(result, "wrong-source.sql", before_source_inputs=wrong)
        self.assertFalse((self.root / "wrong-source.sql").exists())

    def test_missing_digest_manifest_refuses_before_sql(self):
        _, result = self.change()
        key = self.d1.execute(
            "SELECT key FROM edge_meta WHERE key LIKE 'knowledge_node_digest:%' ORDER BY key LIMIT 1"
        ).fetchone()[0]
        self.d1.execute("DELETE FROM edge_meta WHERE key=?", (key,))
        self.d1.commit()
        with self.assertRaisesRegex(ValueError, "digest manifest does not cover every native row"):
            self.catchup(result, "missing-manifest.sql")
        self.assertFalse((self.root / "missing-manifest.sql").exists())

    def test_manifest_budget_refuses_before_sql(self):
        _, result = self.change()
        with self.assertRaisesRegex(ValueError, "digest manifest row budget exceeded"):
            self.catchup(
                result,
                "manifest-budget.sql",
                limits=delta.PreparedD1DeltaLimits(max_manifest_rows=1),
            )
        self.assertFalse((self.root / "manifest-budget.sql").exists())

    def test_orphan_and_malformed_digest_entries_refuse_before_sql(self):
        _, result = self.change()
        cases = (
            (
                "orphan",
                lambda: self.d1.execute(
                    "INSERT INTO edge_meta(key,part,json_chunk) VALUES (?,?,?)",
                    ("knowledge_node_digest:orphan", 0, json.dumps({"sha256": "a" * 64}, separators=(",", ":"))),
                ),
                "invalid or orphan digest manifest row",
            ),
            (
                "malformed",
                lambda: self.d1.execute(
                    "UPDATE edge_meta SET json_chunk=? WHERE key=? AND part=0",
                    ("{}", self.d1.execute(
                        "SELECT key FROM edge_meta WHERE key LIKE 'knowledge_node_digest:%' ORDER BY key LIMIT 1"
                    ).fetchone()[0]),
                ),
                "invalid digest manifest framing",
            ),
        )
        for name, mutate, message in cases:
            with self.subTest(name=name):
                self.d1.execute("BEGIN")
                mutate()
                with self.assertRaisesRegex(ValueError, message):
                    self.catchup(result, f"{name}-digest.sql")
                self.assertFalse((self.root / f"{name}-digest.sql").exists())

    def test_source_order_only_tie_change_is_reconciled_without_digest_change(self):
        original = self.serving_rows()
        old_tie = self.d1.execute(
            "SELECT id FROM knowledge_search_documents "
            "WHERE kind='nodes' AND id_lower='a' ORDER BY position"
        ).fetchall()
        self.assertEqual(old_tie, [("a",), ("A",)])

        nodes = {item["id"]: copy.deepcopy(item) for item in self.f.graph["nodes"]}
        self.f.db.execute("BEGIN IMMEDIATE")
        result = apply_source_bound_prepared_delta_transaction(
            self.f.db,
            expected_binding=self.f.binding,
            before_source_inputs=self.source.before,
            after_source_inputs=self.source.before,
            before_inputs=self.f.inputs(self.f.graph),
            after_inputs=self.f.inputs(copy.deepcopy(self.f.graph)),
            changes=[
                PreparedChange("update", "node", "a", nodes["a"], 3 * SOURCE_ORDER_STRIDE),
                PreparedChange("update", "node", "A", nodes["A"], SOURCE_ORDER_STRIDE),
            ],
        )
        self.f.db.commit()

        receipt = self.catchup(result, "tie-order.sql", rollback_target=self.root / "tie-order-reverse.sql")
        self.assertTrue(receipt["whole_manifest_reconciliation"])
        self.assertEqual(receipt["changed_prepared_rows"], 2)
        self.d1.executescript((self.root / "tie-order.sql").read_text(encoding="utf-8"))
        new_tie = self.d1.execute(
            "SELECT id FROM knowledge_search_documents "
            "WHERE kind='nodes' AND id_lower='a' ORDER BY position"
        ).fetchall()
        self.assertEqual(new_tie, [("A",), ("a",)])
        self.d1.executescript((self.root / "tie-order-reverse.sql").read_text(encoding="utf-8"))
        self.assertEqual(self.serving_rows(), original)

    def test_catchup_reconciles_multiple_committed_prepared_transitions(self):
        graph, first = self.change()
        self.f.db.execute("BEGIN")
        source_after_first = delta.Capture(delta.PreparedD1DeltaLimits()).local(
            self.f.db, first["binding"]
        )[2]
        self.f.db.rollback()

        graph2 = copy.deepcopy(graph)
        changed = next(item for item in graph2["nodes"] if item["id"] == "A")
        changed["display"]["title"] = "Second transition"
        self.f.db.execute("BEGIN IMMEDIATE")
        second = apply_source_bound_prepared_delta_transaction(
            self.f.db,
            expected_binding=first["binding"],
            before_source_inputs=source_after_first,
            after_source_inputs=source_after_first,
            before_inputs=self.f.inputs(graph),
            after_inputs=self.f.inputs(graph2),
            changes=[PreparedChange("update", "node", "A", changed)],
        )
        self.f.db.commit()
        graph2.update(second["source_header"])
        self.assertNotEqual(first["binding"], second["binding"])

        original = self.serving_rows()
        receipt = self.catchup(second, "multi-step.sql", rollback_target=self.root / "multi-step-reverse.sql")
        self.assertTrue(receipt["whole_manifest_reconciliation"])
        oracle = self.full(graph2, second["catalog"], receipt["target_d1_revision"], "multi-step-oracle")
        self.d1.executescript((self.root / "multi-step.sql").read_text(encoding="utf-8"))
        self.assert_full_producer_equivalent(oracle)
        self.d1.executescript((self.root / "multi-step-reverse.sql").read_text(encoding="utf-8"))
        self.assertEqual(self.serving_rows(), original)


if __name__ == "__main__":
    unittest.main()
