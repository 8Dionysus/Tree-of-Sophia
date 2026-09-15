"""Bounded bootstrap of the native source-navigation D1 product.

The fixture is deliberately assembled from the existing access-owned
``SourceNavigationDeltaTests`` setup.  Its navigation and rights projections
are separate products, while the source-navigation header remains the one
explicit full-product header.  These tests prove transport and rollback
guards only; they do not admit source, rights, canon, or semantic authority.
"""

import copy
import json
from dataclasses import replace
from pathlib import Path
import sqlite3
import sys
import unittest


ROOT = Path(__file__).resolve().parents[2]
sys.path[:0] = [
    str(ROOT / "access" / "src"),
    str(ROOT / "access" / "tests"),
    str(ROOT / "access" / "deploy" / "cloudflare-worker" / "scripts"),
]

from tos_access.prepared_source_binding import PreparedSourceInputs
from tos_access.projection_mutation import ProjectionSnapshotView
from tos_access.projection_store import (
    Collection,
    ProjectionStoreError,
    canonical_bytes,
    write_projection,
)

import build_runtime as full
import incremental_runtime
import prepared_delta_runtime as delta
import source_navigation_bootstrap_runtime as bootstrap
import source_navigation_rows as navigation_projection
import test_source_navigation_delta_runtime as delta_fixture


class SourceNavigationBootstrapTests(unittest.TestCase):
    """Use one tiny prepared pair and an absent native D1 product."""

    OLD_D1_REVISION = "d" * 64
    NATIVE_TABLES = (
        "source_navigation_nodes",
        "source_navigation_node_payload",
        "source_navigation_edges",
        "source_navigation_edge_payload",
        "source_navigation_rights",
        "source_navigation_rights_payload",
    )

    @classmethod
    def setUpClass(cls):
        delta_fixture.SourceNavigationDeltaTests.setUpClass()

    def setUp(self):
        self.base = delta_fixture.SourceNavigationDeltaTests()
        self.base.setUp()
        self.addCleanup(self.base.doCleanups)
        self.f = self.base.f
        self.source = self.base.source
        self.root = self.base.root
        self.raw = copy.deepcopy(self.base.raw)
        self.navigation_header = {
            key: copy.deepcopy(value)
            for key, value in self.raw.items()
            if key not in ("nodes", "edges", "rights")
        }

        self.navigation_path = self.root / "navigation" / "navigation.json"
        self.navigation_view = self._write_navigation(self.raw)
        previous = self.source.before
        self.before_inputs = PreparedSourceInputs(
            source_revision=previous.value()["source_revision"],
            source_publication=previous.value()["source_publication"],
            dependencies=previous.value()["dependencies"],
            roots={**previous.roots(), "source-navigation": self.navigation_view},
        )
        self.source.before = self.before_inputs
        self.source.attach()
        self.old_binding = self.f.binding

        # Build a D1-shaped predecessor with no native navigation product.
        # The prepared DB above still carries the exact paired navigation root.
        self.d1 = self.base.full(
            self.f.graph,
            self.f.catalog,
            self.OLD_D1_REVISION,
            "absent",
            {},
        )
        self.d1.execute("BEGIN IMMEDIATE")
        incremental_runtime.prepare_search_address_indexes_transaction(
            self.d1, expected_revision=self.OLD_D1_REVISION
        )
        self.d1.commit()
        self.rights_view = self._write_rights(self.raw)

    def _write_navigation(self, raw):
        top = {
            key: copy.deepcopy(value)
            for key, value in raw.items()
            if key not in ("nodes", "edges", "rights")
        }
        write_projection(
            self.navigation_path,
            top,
            {
                "nodes": Collection(raw["nodes"], "node_id", ("node_id",)),
                "edges": Collection(raw["edges"], "edge_id", ("edge_id",)),
            },
            work_dir=self.root,
            prune=False,
        )
        return ProjectionSnapshotView(
            self.navigation_path.read_bytes(), self.navigation_path
        )

    def _write_rights(self, raw):
        path = self.root / "rights" / "rights.json"
        write_projection(
            path,
            {
                "schema_version": "tos_source_navigation_rights_v1",
                "navigation_header": copy.deepcopy(self.navigation_header),
            },
            {"rights": Collection(raw["rights"], "rights_id", ("rights_id",))},
            work_dir=self.root,
            prune=False,
        )
        return ProjectionSnapshotView(path.read_bytes(), path)

    def capture(self, *, limits=None, expected_rights_sha256=None,
                trusted_rights_sha256=None, target_name="bootstrap.sql",
                rollback_name="bootstrap-rollback.sql"):
        expected_rights_sha256 = (
            self.rights_view.snapshot_digest
            if expected_rights_sha256 is None
            else expected_rights_sha256
        )
        trusted_rights_sha256 = (
            expected_rights_sha256
            if trusted_rights_sha256 is None
            else trusted_rights_sha256
        )
        for db in (self.d1, self.f.db):
            db.execute("BEGIN")
        try:
            return bootstrap.build_source_navigation_bootstrap_sql(
                self.d1,
                self.f.db,
                self.root / target_name,
                expected_d1_revision=self.OLD_D1_REVISION,
                prepared_binding=self.old_binding,
                rights_view=self.rights_view,
                expected_rights_sha256=expected_rights_sha256,
                trusted_rights_sha256=trusted_rights_sha256,
                rollback_target=self.root / rollback_name,
                limits=limits,
            )
        finally:
            for db in (self.d1, self.f.db):
                if db.in_transaction:
                    db.rollback()

    @staticmethod
    def _ordered_rows(db, table, columns=None):
        columns = tuple(columns or delta.COLUMNS[table])
        order = ",".join(delta.PRIMARY_KEYS[table])
        return db.execute(
            f"SELECT {','.join(columns)} FROM {table} ORDER BY {order}"
        ).fetchall()

    def _native_metadata_state(self):
        return {
            "edge_meta": self._ordered_rows(self.d1, "edge_meta"),
            **{
                table: self._ordered_rows(self.d1, table)
                for table in self.NATIVE_TABLES
            },
        }

    def _normalized_state(self):
        excluded = {"edge_meta", *self.NATIVE_TABLES}
        return {
            table: self._ordered_rows(self.d1, table)
            for table in sorted(delta.COLUMNS)
            if table not in excluded
        }

    def _native_without_ord(self):
        oracle = self.base.full(self.f.graph, self.f.catalog, "e" * 64,
                                "full-native-oracle", self.raw)
        expected, actual = {}, {}
        json_columns = {"json", "properties_json", "source_refs_json", "scope_refs_json"}
        for table in self.NATIVE_TABLES:
            columns = tuple(
                column for column in delta.COLUMNS[table] if column != "ord"
            )
            for db, destination in ((self.d1, actual), (oracle, expected)):
                destination[table] = [tuple(
                    canonical_bytes(json.loads(value)).decode() if column in json_columns and value != "" else value
                    for column, value in zip(columns, row))
                    for row in self._ordered_rows(db, table, columns)]
        return actual, expected

    def _assert_native_oracle(self):
        actual, expected = self._native_without_ord()
        self.assertEqual(actual, expected)

    @staticmethod
    def _metadata(db, key):
        rows = db.execute(
            "SELECT part,json_chunk FROM edge_meta WHERE key=? ORDER BY part",
            (key,),
        ).fetchall()
        return json.loads("".join(row[1] for row in rows))

    @staticmethod
    def _split_publication(sql):
        marker = "INSERT OR REPLACE INTO tos_delta_publications SELECT"
        stage, rest = sql.split(marker, 1)
        return stage, marker + rest

    def _apply(self, name, *, stage_only=False):
        sql = (self.root / name).read_text()
        if stage_only:
            sql, _ = self._split_publication(sql)
        self.d1.executescript(sql)

    def _insert_projected_row(self, collection, item):
        projected = navigation_projection.project_rows(
            collection, 0, item, full.REPO_ROOT
        )
        table, rows = next(iter(projected.items()))
        columns = delta.COLUMNS[table]
        placeholders = ",".join("?" for _ in columns)
        self.d1.execute(
            f"INSERT INTO {table} ({','.join(columns)}) VALUES ({placeholders})",
            rows[0],
        )

    def test_bootstrap_forward_staging_replay_reverse_preserves_small_oracle(self):
        receipt = self.capture()
        before_native_metadata = self._native_metadata_state()
        before_normalized = self._normalized_state()

        self._apply("bootstrap.sql", stage_only=True)
        # Staging is not publication: the old normalized/native serving state
        # and unavailable header remain visible until the guarded insert.
        self.assertEqual(self._native_metadata_state(), before_native_metadata)
        self.assertEqual(self._normalized_state(), before_normalized)
        self.assertEqual(self._metadata(self.d1, "source_navigation_top"), {})

        self._apply("bootstrap.sql")
        self.assertEqual(receipt["counts"], {"nodes": 1, "edges": 1, "rights": 1})
        self.assertEqual(receipt["normalized_rows_changed"], 0)
        self.assertEqual(receipt["prepared_rows_changed"], 0)
        self.assertEqual(
            self._metadata(self.d1, "source_navigation_top"),
            self.navigation_header,
        )
        self._assert_native_oracle()
        self.assertEqual(self._normalized_state(), before_normalized)

        after_forward = self._native_metadata_state()
        self._apply("bootstrap.sql")
        self.assertEqual(self._native_metadata_state(), after_forward)
        self.assertEqual(self._normalized_state(), before_normalized)

        self._apply("bootstrap-rollback.sql")
        self.assertEqual(self._native_metadata_state(), before_native_metadata)
        self.assertEqual(self._normalized_state(), before_normalized)
        self.assertEqual(self._metadata(self.d1, "source_navigation_top"), {})

    def test_occupied_native_product_refuses_before_capture(self):
        self._insert_projected_row("nodes", self.raw["nodes"][0])
        self.d1.commit()
        with self.assertRaisesRegex(
            ValueError, "native product must be completely absent"
        ):
            self.capture()
        self.assertFalse((self.root / "bootstrap.sql").exists())
        self.assertFalse((self.root / "bootstrap.sql.next").exists())

    def test_native_insert_between_staging_and_publish_is_rejected(self):
        self.capture()
        self._apply("bootstrap.sql", stage_only=True)
        self._insert_projected_row("nodes", self.raw["nodes"][0])
        self.d1.commit()
        sql = (self.root / "bootstrap.sql").read_text()
        _, publication = self._split_publication(sql)
        with self.assertRaisesRegex(
            sqlite3.DatabaseError, "initial product is no longer empty"
        ):
            self.d1.executescript(publication)
        self.assertEqual(self._metadata(self.d1, "data_revision"),
                         {"sha256": self.OLD_D1_REVISION})

    def test_rights_digest_mismatch_refuses_without_output(self):
        with self.assertRaisesRegex(ValueError, "exact admitted rights snapshot"):
            self.capture(
                expected_rights_sha256="f" * 64,
                trusted_rights_sha256="f" * 64,
            )
        self.assertFalse((self.root / "bootstrap.sql").exists())

    def test_row_budget_refuses_complete_native_product(self):
        limits = replace(delta.PreparedD1DeltaLimits(), max_rows=2)
        with self.assertRaisesRegex(
            ValueError, "native initial product row budget exceeded"
        ):
            self.capture(limits=limits)
        self.assertFalse((self.root / "bootstrap.sql").exists())

    def test_tampered_rights_part_refuses_before_sql(self):
        manifest = json.loads(self.rights_view.root_bytes)
        descriptor = manifest["collections"]["rights"]["root"]
        part = self.rights_view.namespace_path.parent / descriptor["path"]
        part.write_bytes(part.read_bytes() + b"!")
        with self.assertRaisesRegex(
            ProjectionStoreError, "projection part size mismatch|digest mismatch"
        ):
            self.capture()
        self.assertFalse((self.root / "bootstrap.sql").exists())

    def test_prepared_delta_after_bootstrap_maintains_native_row(self):
        boot = self.capture()
        self._apply("bootstrap.sql")

        before_db = sqlite3.connect(":memory:")
        self.addCleanup(before_db.close)
        self.f.db.backup(before_db)

        successor_raw = copy.deepcopy(self.raw)
        successor_raw["nodes"][0]["label"] = "Исправлено"
        successor_view = self._write_navigation(successor_raw)
        graph, after_inputs = self.source.delta()
        after_roots = after_inputs.roots()
        after_roots["source-navigation"] = successor_view
        after_inputs = PreparedSourceInputs(
            source_revision=after_inputs.value()["source_revision"],
            source_publication=after_inputs.value()["source_publication"],
            dependencies=after_inputs.value()["dependencies"],
            roots=after_roots,
        )
        self.f.db.execute("BEGIN IMMEDIATE")
        prepared_result = self.source.apply(graph, after_inputs)
        self.f.db.commit()

        for db in (self.d1, before_db, self.f.db):
            db.execute("BEGIN")
        try:
            result = delta.build_prepared_delta_sql(
                self.d1,
                before_db,
                self.f.db,
                self.root / "successor.sql",
                expected_d1_revision=boot["target_d1_revision"],
                before_binding=self.old_binding,
                after_binding=prepared_result["binding"],
                rollback_target=self.root / "successor-rollback.sql",
            )
        finally:
            for db in (self.d1, before_db, self.f.db):
                if db.in_transaction:
                    db.rollback()
        self.assertEqual(
            result["source_navigation_product"],
            {"state": "maintained", "changed_rows": 1},
        )
        self.assertEqual(result["changed_prepared_rows"], 1)
        self._apply("successor.sql")
        self.assertEqual(
            self.d1.execute(
                "SELECT label FROM source_navigation_nodes WHERE node_id='person'"
            ).fetchone()[0],
            "Исправлено",
        )


if __name__ == "__main__":
    unittest.main()
