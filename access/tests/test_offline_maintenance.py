"""Explicit auxiliary attachment preserves source and publication boundaries."""
from contextlib import closing
from dataclasses import replace
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from tos_access import core as source, prepare as producer, knowledge as k
from tos_access import prepared_semantics as joined
from tos_access.catalog_semantics import CatalogInputs, SEQUENCE_ORDER
from tos_access.normalization_cache import active_cache
from tos_access.published_read_model import PublishedKnowledgeReadModel
from tos_access.published_search import PublishedSearchService
from test_offline_prepare import write_source, REPO


class OfflineMaintenanceTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="offline-maintenance-", dir=os.environ.get("TMPDIR"))
        self.addCleanup(self.tmp.cleanup)
        self.root, self.output = (Path(self.tmp.name) / name for name in ("source", "output"))
        write_source(self.root)

    def command(self, *extra):
        return subprocess.run([sys.executable, "-m", "tos_access.prepare", "--source-root", str(self.root),
            "--output-dir", str(self.output), *extra], capture_output=True, text=True, timeout=30,
            env={**os.environ, "PYTHONPATH": str(REPO / "access/src"), "PYTHONDONTWRITEBYTECODE": "1"})

    def tables(self):
        with closing(sqlite3.connect(self.output / "snapshot.sqlite")) as db:
            return {row[0] for row in db.execute("SELECT name FROM sqlite_master WHERE type='table'")}

    def assert_incomplete_rolled_back(self):
        self.assertTrue((self.output / "snapshot.sqlite").exists())
        self.assertFalse((self.output / "binding.json").exists())
        self.assertFalse((self.output / "completed.json").exists())
        self.assertNotIn("catalog_state", self.tables())
        self.assertNotIn("semantic_state", self.tables())

    def test_default_snapshot_and_prepare_packet_remain_unchanged(self):
        core = source.ToSAccessCore.discover(self.root)
        self.assertEqual(set(core.knowledge_snapshot_once()), {"graph", "catalog", "source_state"})
        with patch.object(producer, "bootstrap_prepared_maintenance_transaction", side_effect=AssertionError("default attachment")):
            result = producer.prepare(self.root, self.output)
        self.assertNotIn("maintenance", result)
        self.assertNotIn("catalog_state", self.tables())
        self.assertNotIn("semantic_state", self.tables())

    def test_exact_optin_inputs_are_captured_with_cache_disabled_and_copy_isolated(self):
        path = self.root / source.PHILOSOPHY_PROJECTION_RELATIVE_PATH
        philosophy = json.loads(path.read_text())
        philosophy["views"] = [{"view_id": "actual-lens", "title": str(self.root / "ToS/lens-title")}]
        path.write_text(json.dumps(philosophy))
        core = source.ToSAccessCore.discover(self.root)
        actual_inputs = core._knowledge_source_inputs(reader=source._read_json_file)
        original = CatalogInputs.from_graph
        seen = []
        def capture(*args, **kwargs):
            self.assertIsNone(active_cache.get())
            seen.append(True)
            return original(*args, **kwargs)
        sentinel = object()
        token = active_cache.set(sentinel)
        try:
            with patch.object(CatalogInputs, "from_graph", side_effect=capture):
                result = core.knowledge_snapshot_once(include_catalog_inputs=True)
            self.assertIs(active_cache.get(), sentinel)
        finally:
            active_cache.reset(token)
        captured = result["catalog_inputs"]
        self.assertEqual(captured.entity_type_registry, actual_inputs["entity_type_registry"])
        self.assertEqual(captured.relation_type_registry, actual_inputs["relation_type_registry"])
        self.assertEqual(captured.lenses, k.saved_lens_specs(actual_inputs["corpus"], philosophy))
        self.assertEqual(captured.source_order_profile, SEQUENCE_ORDER)
        self.assertEqual(captured.header, {key: value for key, value in result["graph"].items() if key not in ("nodes", "relations")})
        detached = captured.entity_type_registry
        detached["types"].clear()
        self.assertTrue(captured.entity_type_registry["types"])
        self.assertTrue(seen)
        with self.assertRaises(ValueError):
            core.knowledge_snapshot_once(include_catalog_inputs="yes")

    def test_drift_during_exact_input_capture_refuses_and_restores_cache_context(self):
        core = source.ToSAccessCore.discover(self.root)
        original_catalog = source.build_knowledge_catalog
        original_inputs = CatalogInputs.from_graph
        catalog_built = False
        def catalog(*args, **kwargs):
            nonlocal catalog_built
            result = original_catalog(*args, **kwargs)
            catalog_built = True
            return result
        def capture(*args, **kwargs):
            result = original_inputs(*args, **kwargs)
            if catalog_built:
                path = self.root / source.INDEX_RELATIVE_PATH
                path.write_text(path.read_text() + " ")
            return result
        sentinel = object()
        token = active_cache.set(sentinel)
        try:
            with patch.object(source, "build_knowledge_catalog", side_effect=catalog), \
                    patch.object(CatalogInputs, "from_graph", side_effect=capture), \
                    self.assertRaisesRegex(RuntimeError, "source changed"):
                core.knowledge_snapshot_once(include_catalog_inputs=True)
            self.assertIs(active_cache.get(), sentinel)
        finally:
            active_cache.reset(token)

    def test_real_optin_cli_buffered_and_bulk_keep_binding_catalog_search(self):
        path = self.root / source.PHILOSOPHY_PROJECTION_RELATIVE_PATH
        philosophy = json.loads(path.read_text())
        philosophy["views"] = [{"view_id": "portable-lens", "title": str(self.root / "ToS/lens-title")}]
        path.write_text(json.dumps(philosophy))
        base = producer.prepare(self.root, self.output)
        base_reader = PublishedKnowledgeReadModel(self.output / "snapshot.sqlite", base["binding"])
        expected_catalog = base_reader.catalog()
        expected_search = PublishedSearchService(base_reader).search("Alpha")
        for bulk in (False, True):
            self.output = Path(self.tmp.name) / ("bulk" if bulk else "buffered")
            flags = ["--attach-maintenance", "--maintenance-max-mutations", "200000"]
            if bulk:
                flags += ["--bulk-search-scratch-bytes", "1048576", "--bulk-search-scratch-mutations", "200000"]
            result = self.command(*flags)
            self.assertEqual(result.returncode, 0, result.stderr)
            receipt = json.loads(result.stdout)
            self.assertEqual(receipt, json.loads((self.output / "completed.json").read_text()))
            self.assertEqual(receipt["binding"], base["binding"])
            attached = receipt["maintenance"]
            self.assertEqual(attached["binding"], base["binding"])
            self.assertEqual(attached["mutation_budget_upper_bound"], base["publication_limits"]["max_mutations"] + 200000)
            self.assertGreater(attached["sql_mutations"], 0)
            self.assertLessEqual(attached["sql_mutations"], 200000)
            for key in ("publication_changed", "consumer_switched", "source_transition_verified", "semantic_acceptance"):
                self.assertFalse(attached[key])
            self.assertTrue({"catalog_state", "semantic_state"} <= self.tables())
            self.assertEqual({p.name for p in self.output.iterdir()}, {"snapshot.sqlite", "binding.json", "completed.json"})
            reader = PublishedKnowledgeReadModel(self.output / "snapshot.sqlite", receipt["binding"])
            self.assertEqual(reader.catalog(), expected_catalog)
            actual = PublishedSearchService(reader).search("Alpha")
            self.assertEqual(actual["nodes"], expected_search["nodes"])
            self.assertEqual(actual["relations"], expected_search["relations"])

    def test_missing_exact_handoff_is_not_reconstructed_from_catalog(self):
        original = source.ToSAccessCore.knowledge_snapshot_once
        def missing(core, **options):
            return original(core)
        with patch.object(source.ToSAccessCore, "knowledge_snapshot_once", missing), self.assertRaisesRegex(ValueError, "CatalogInputs"):
            producer.prepare(self.root, self.output, maintenance=producer.MaintenanceAttachmentLimits())
        self.assertFalse((self.output / "completed.json").exists())
        self.assertFalse((self.output / "snapshot.sqlite").exists())

    def test_api_caps_reach_kernel_in_unmarked_transaction_and_receipt(self):
        base = producer.MaintenanceAttachmentLimits()
        publication = producer.PublicationLimits(max_bytes=8 * 1024 * 1024, max_mutations=100000)
        maintenance = replace(base, max_mutations=123456,
            catalog_limits=replace(base.catalog_limits, max_index_bytes=4 * 1024 * 1024, max_catalog_entries=12345),
            semantic_limits=replace(base.semantic_limits, max_bytes=2 * 1024 * 1024, max_writes=200000,
                                    max_input_bytes=16 * 1024 * 1024))
        original = producer.bootstrap_prepared_maintenance_transaction
        def inspect(db, **options):
            self.assertTrue(db.in_transaction)
            self.assertFalse((self.output / "binding.json").exists())
            self.assertFalse((self.output / "completed.json").exists())
            self.assertEqual(options["limits"].max_mutations, 123456)
            self.assertEqual(options["limits"].max_bytes, 2 * 1024 * 1024)
            self.assertEqual(options["catalog_limits"].max_index_bytes, 2 * 1024 * 1024)
            self.assertEqual(options["semantic_limits"].max_bytes, 2 * 1024 * 1024)
            self.assertEqual(options["semantic_limits"].max_writes, 123456)
            self.assertEqual(options["catalog_limits"].max_catalog_entries, 12345)
            self.assertEqual(options["semantic_limits"].max_input_bytes, 16 * 1024 * 1024)
            return original(db, **options)
        with patch.object(producer, "bootstrap_prepared_maintenance_transaction", side_effect=inspect):
            result = producer.prepare(self.root, self.output, limits=publication, maintenance=maintenance)
        attached = result["maintenance"]
        self.assertEqual(attached["declared_limits"]["max_mutations"], 123456)
        self.assertEqual(attached["effective_limits"]["publication"]["max_bytes"], 2 * 1024 * 1024)
        self.assertEqual(attached["mutation_budget_upper_bound"], 223456)

    def test_attachment_stage_errors_and_interrupt_roll_back_both_indexes(self):
        for stage in ("semantic", "catalog", "interrupt"):
            self.output = Path(self.tmp.name) / stage
            name = "bootstrap_semantic_index_transaction" if stage == "semantic" else "bootstrap_prepared_catalog_transaction"
            original = getattr(joined, name)
            def fail(*args, **kwargs):
                original(*args, **kwargs)
                raise KeyboardInterrupt() if stage == "interrupt" else RuntimeError("injected stage failure")
            error = KeyboardInterrupt if stage == "interrupt" else RuntimeError
            with patch.object(joined, name, side_effect=fail), self.assertRaises(error):
                producer.prepare(self.root, self.output, maintenance=producer.MaintenanceAttachmentLimits())
            self.assert_incomplete_rolled_back()

    def test_source_and_selected_binding_drift_before_commit_roll_back(self):
        original = producer.bootstrap_prepared_maintenance_transaction
        for drift in ("source", "binding"):
            self.output = Path(self.tmp.name) / ("drift-" + drift)
            def drift_after(db, **options):
                result = original(db, **options)
                if drift == "source":
                    path = self.root / source.INDEX_RELATIVE_PATH
                    path.write_text(path.read_text() + " ")
                else:
                    db.execute("UPDATE knowledge_exploration_clock SET epoch=epoch+1")
                return result
            with patch.object(producer, "bootstrap_prepared_maintenance_transaction", side_effect=drift_after), self.assertRaises((ValueError, RuntimeError)):
                producer.prepare(self.root, self.output, maintenance=producer.MaintenanceAttachmentLimits())
            self.assert_incomplete_rolled_back()

    def test_registry_path_values_are_not_rewritten_or_rebound_to_force_reproduction(self):
        path = self.root / source.ENTITY_TYPE_REGISTRY_RELATIVE_PATH
        registry = json.loads(path.read_text())
        concept = next(item for item in registry["types"] if item["type_id"] == "tos.entity.concept")
        concept["labels"]["default"] = str(self.root / "ToS/registry-owned-label")
        path.write_text(json.dumps(registry))
        original = producer.bootstrap_prepared_maintenance_transaction
        def inspect(db, **options):
            self.assertEqual(options["inputs"].entity_type_registry, registry)
            self.assertEqual(options["inputs"].header["normalization_binding"]["entity_registry_digest"], k._stable_digest(registry))
            return original(db, **options)
        with patch.object(producer, "bootstrap_prepared_maintenance_transaction", side_effect=inspect), self.assertRaises(ValueError):
            producer.prepare(self.root, self.output, maintenance=producer.MaintenanceAttachmentLimits())
        self.assert_incomplete_rolled_back()

    def test_separate_write_and_shared_file_limits_refuse_without_completion(self):
        base = producer.MaintenanceAttachmentLimits()
        for name, limits in (("write", replace(base, max_mutations=1)),
                             ("catalog-bytes", replace(base, catalog_limits=replace(base.catalog_limits, max_index_bytes=4096))),
                             ("semantic-bytes", replace(base, semantic_limits=replace(base.semantic_limits, max_bytes=4096)))):
            self.output = Path(self.tmp.name) / name
            with self.assertRaises(ValueError):
                producer.prepare(self.root, self.output, maintenance=limits)
            self.assert_incomplete_rolled_back()

    def test_invalid_modes_and_caps_refuse_before_output_creation(self):
        for flags in (("--maintenance-max-mutations", "100"), ("--attach-maintenance", "--maintenance-max-mutations", "0")):
            result = self.command(*flags)
            self.assertEqual(result.returncode, 2)
            self.assertFalse(self.output.exists())
        result = self.command("--attach-maintenance", "--maintenance-max-mutations", str(2**53))
        self.assertEqual(result.returncode, 1)
        self.assertFalse(self.output.exists())
        with self.assertRaises(ValueError):
            producer.prepare(self.root, self.output, maintenance={})
        self.assertFalse(self.output.exists())

    def test_postcommit_marker_failure_is_incomplete_not_a_claimed_sql_rollback(self):
        original = producer._exclusive_json
        def fail(path, value):
            if path.name == "completed.json":
                raise OSError("injected marker failure")
            return original(path, value)
        with patch.object(producer, "_exclusive_json", side_effect=fail), self.assertRaises(OSError):
            producer.prepare(self.root, self.output, maintenance=producer.MaintenanceAttachmentLimits())
        self.assertFalse((self.output / "completed.json").exists())
        self.assertTrue({"catalog_state", "semantic_state"} <= self.tables())


if __name__ == "__main__":
    unittest.main()
