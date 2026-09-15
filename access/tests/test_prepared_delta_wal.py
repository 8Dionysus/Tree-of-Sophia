"""Same-file WAL snapshots for the prepared-to-D1 delta capture route."""

import json
from pathlib import Path
import sqlite3
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]
sys.path[:0] = [str(ROOT / "access/tests"), str(ROOT / "access/deploy/cloudflare-worker/scripts")]

import prepared_delta_runtime as delta
import test_prepared_delta_runtime as delta_fixtures
import test_prepared_source_binding as source_fixtures
from incremental_runtime import prepare_search_address_indexes_transaction


class PreparedDeltaWalSnapshotTests(unittest.TestCase):
    """Exercise one tiny prepared file without making a full prepared copy."""

    @classmethod
    def setUpClass(cls):
        source_fixtures.PreparedSourceBindingTests.setUpClass()

    def setUp(self):
        self.source = source_fixtures.PreparedSourceBindingTests()
        self.source.setUp()
        self.addCleanup(self.source.doCleanups)
        self.source.attach()
        self.f = self.source.f
        self.root = Path(self.f.tmp.name)

        # Reuse the existing tiny full-producer fixture helper; only the D1
        # oracle is in memory. The prepared predecessor remains on one file.
        self.d1 = delta_fixtures.PreparedD1DeltaTests.full(
            self, self.f.graph, self.f.catalog, "d" * 64, "d1"
        )
        self.d1.execute("BEGIN IMMEDIATE")
        prepare_search_address_indexes_transaction(self.d1, expected_revision="d" * 64)
        self.d1.commit()

        self.writer = self.f.db
        self.assertEqual(self.writer.execute("PRAGMA journal_mode=WAL").fetchone()[0], "wal")

    def _read_snapshot(self):
        reader = sqlite3.connect(
            self.f.path.as_uri() + "?mode=ro", uri=True, isolation_level=None
        )
        reader.execute("BEGIN")
        self.addCleanup(reader.close)
        self.addCleanup(reader.rollback)
        return reader

    @staticmethod
    def _node(reader, identifier="a"):
        row = reader.execute(
            "SELECT json FROM knowledge_nodes WHERE id=?", (identifier,)
        ).fetchone()
        return None if row is None else json.loads(row[0])

    def test_before_and_after_read_snapshots_share_prepared_file(self):
        graph, after_inputs = self.source.delta()
        before_reader = self._read_snapshot()
        old_capture = delta.Capture(delta.PreparedD1DeltaLimits())
        old_top, old_descriptor, old_source = old_capture.local(
            before_reader, self.f.binding
        )
        old_node = self._node(before_reader)
        self.assertEqual(old_node, self.f.graph["nodes"][0])
        self.assertEqual(old_descriptor["mode"], "bootstrap")
        self.assertEqual(old_source, self.source.before)

        # The writer is a separate connection to the same path. WAL allows the
        # pre-commit reader to retain its old snapshot while this commits.
        self.writer.execute("BEGIN IMMEDIATE")
        result = self.source.apply(graph, after_inputs)
        self.assertTrue(self.writer.in_transaction)
        self.assertEqual(self._node(before_reader), old_node)
        self.assertEqual(
            delta.Capture(delta.PreparedD1DeltaLimits()).local(
                before_reader, self.f.binding
            )[0],
            old_top,
        )
        self.writer.commit()

        after_reader = self._read_snapshot()
        new_capture = delta.Capture(delta.PreparedD1DeltaLimits())
        new_top, new_descriptor, new_source = new_capture.local(
            after_reader, result["binding"]
        )
        self.assertNotEqual(new_top, old_top)
        self.assertEqual(new_descriptor["mode"], "delta-history")
        self.assertEqual(
            new_descriptor["parent_data_revision"], self.f.binding["data_revision"]
        )
        self.assertEqual(new_source, after_inputs)
        new_node = self._node(after_reader)
        self.assertEqual(new_node, graph["nodes"][0])
        self.assertNotEqual(new_node, old_node)

        # All three read transactions are caller-held, as required by the
        # existing capture API. It emits SQL but does not mutate the D1 oracle.
        self.d1.execute("BEGIN")
        receipt = delta.build_prepared_delta_sql(
            self.d1,
            before_reader,
            after_reader,
            self.root / "same-file-delta.sql",
            expected_d1_revision="d" * 64,
            before_binding=self.f.binding,
            after_binding=result["binding"],
        )
        self.assertEqual(receipt["changed_prepared_rows"], 1)
        self.assertFalse(receipt["d1_applied"])
        self.d1.rollback()

        # Apply the emitted delta to the tiny D1 oracle and compare every
        # registered serving table with an independent full-producer oracle.
        graph.update(result["source_header"])
        oracle = delta_fixtures.PreparedD1DeltaTests.full(
            self, graph, result["catalog"], receipt["target_d1_revision"], "oracle"
        )
        sql = (self.root / "same-file-delta.sql").read_text(encoding="utf-8")
        self.d1.executescript(sql)
        for table in delta.COLUMNS:
            keys = ",".join(delta.PRIMARY_KEYS[table])
            actual = self.d1.execute(f"SELECT * FROM {table} ORDER BY {keys}").fetchall()
            expected = oracle.execute(f"SELECT * FROM {table} ORDER BY {keys}").fetchall()
            self.assertEqual(actual, expected, table)


if __name__ == "__main__":
    unittest.main()
