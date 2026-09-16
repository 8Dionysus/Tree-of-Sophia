from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from tos_access.projection_diff import (
    DiffLimits, ProjectionDiffError, ProjectionDiffBudgetExceeded,
    ProjectionDiffRequiresBootstrap, diff_projection_snapshots, diff_projections,
)
from tos_access.projection_mutation import ProjectionSnapshotView
from tos_access.projection_store import (
    Collection, ProjectionReader, ProjectionStoreError, MAX_ROOT_BYTES,
    canonical_bytes, write_projection, _gzip,
)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


class ProjectionDiffTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def build(self, name, rows, *, header=None, target=256, field="id", order=(), mapping=None):
        path = self.root / name / "example.min.json"
        write_projection(path, header or {"schema_version": "example_v1"}, {
            "nodes": Collection(rows, field, order),
            "mapping": Collection((mapping or {}).items(), None),
        }, target_part_bytes=target, work_dir=self.root)
        return ProjectionReader(path, cache_bytes=0)

    def diff(self, before, after, **kwargs):
        return diff_projections(before, after, expected_before_sha256=before.snapshot_digest,
                                expected_after_sha256=after.snapshot_digest,
                                trusted_baseline_sha256=before.snapshot_digest, **kwargs)

    def oracle(self, before, after):
        # Explicit tiny full materialization is test-only, never a diff path.
        aa, bb = before.materialize(), after.materialize()
        changes = []
        for collection in ("mapping", "nodes"):
            def keyed(document):
                return document[collection] if collection == "mapping" else {row["id"]: row for row in document[collection]}
            a, b = keyed(aa), keyed(bb)
            for key in sorted(a.keys() | b.keys()):
                if key in a and key in b and canonical_bytes(a[key]) == canonical_bytes(b[key]):
                    continue
                def side(rows):
                    return {"present": False, "sha256": None} if key not in rows else {
                        "present": True, "sha256": sha(canonical_bytes(rows[key])), "row": rows[key]}
                changes.append({"collection": collection, "key": key,
                                "operation": "insert" if key not in a else "delete" if key not in b else "replace",
                                "before": side(a), "after": side(b)})
        return changes

    def rows(self, count=50):
        return [{"id": f"node-{i:04}", "value": i, "detail": "x" * 40} for i in range(count)]

    def test_mixed_changes_and_header_match_full_materialized_oracle(self):
        rows = self.rows()
        changed = copy.deepcopy(rows[1:])
        changed[3]["unknown"] = {"深さ": [False, None, {"original": "Straße — λόγος"}]}
        changed.append({"id": "new", "value": {"source": "日本語"}})
        before = self.build("before", rows, mapping={"null-delete": None, "x": 1})
        after = self.build("after", list(reversed(changed)), mapping={"null-insert": None, "x": False},
                           header={"schema_version": "example_v1", "counts": {"nodes": len(changed)}, "label": "Слой"})
        expected = self.oracle(before, after)
        result = self.diff(before, after, include_rows=True)
        self.assertTrue(result["complete"])
        self.assertEqual(result["changes"], expected)
        self.assertEqual(result["header_change"]["before"], before.metadata())
        self.assertEqual(result["header_change"]["after"], after.metadata())
        self.assertFalse(result["target_closure_verified"])
        self.assertFalse(result["baseline_trust"]["established_by_diff"])

    def test_mapping_null_is_not_absence_and_numeric_types_are_framed(self):
        before = self.build("before", [], mapping={"null": None, "false": False, "int": 1, "zero": 0.0})
        after = self.build("after", [], mapping={"insert-null": None, "false": 0, "int": 1.0, "zero": -0.0})
        packet = self.diff(before, after, include_rows=True)
        self.assertEqual(packet["changes"], self.oracle(before, after))
        self.assertEqual(len(packet["changes"]), 5)
        null = next(row for row in packet["changes"] if row["key"] == "null")
        self.assertIsNone(null["before"]["row"])
        self.assertTrue(null["before"]["present"])
        self.assertNotEqual(null["before"]["sha256"], null["after"]["sha256"])

    def test_dictionary_and_input_order_do_not_change_logical_rows(self):
        a = [{"id": "a", "nested": {"z": [1, "e\u0301"], "a": False}}, {"id": "b", "value": "é"}]
        b = [{"value": "é", "id": "b"}, {"nested": {"a": False, "z": [1, "e\u0301"]}, "id": "a"}]
        before, after = self.build("one", a), self.build("two", b)
        self.assertEqual(self.diff(before, after)["changes"], [])
        b[1]["nested"]["z"][1] = "é"
        after = self.build("two", b)
        self.assertEqual(len(self.diff(before, after)["changes"]), 1)

    def test_identical_roots_need_no_part_reads_even_when_parts_absent(self):
        before, after = self.build("before", self.rows()), self.build("after", self.rows())
        for reader in (before, after):
            closure = list(reader.closure_paths())  # caller baseline validation, test only
            for path in closure[1:]:
                path.unlink()
        with patch.object(ProjectionReader, "_load", side_effect=AssertionError("part read")):
            result = self.diff(before, after, limits=DiffLimits(max_opened_parts=0, max_keys=0))
        self.assertEqual(result["changes"], [])
        self.assertFalse(result["target_closure_verified"])

    def test_changed_branch_skips_nonexistent_unrelated_leaves(self):
        rows = self.rows(80)
        after_rows = copy.deepcopy(rows)
        after_rows[0]["value"] = "changed"
        before, after = self.build("before", rows), self.build("after", after_rows)
        expected = self.oracle(before, after)
        old_paths = {p.name: p for p in before.closure_paths() if p.suffix == ".gz"}
        new_paths = {p.name: p for p in after.closure_paths() if p.suffix == ".gz"}
        unchanged = old_paths.keys() & new_paths.keys()
        self.assertGreater(len(unchanged), 50)
        for name in unchanged:
            old_paths[name].unlink()
            new_paths[name].unlink()
        result = self.diff(before, after, include_rows=True, limits=DiffLimits(max_opened_parts=8, max_keys=4))
        self.assertEqual(result["changes"], expected)

    def test_split_and_coalesce_trees(self):
        # One root data leaf versus deep radix index, in both directions.
        small = [{"id": "one", "value": 1}]
        large = self.rows(35) + [{"id": "one", "value": 2}]
        before, after = self.build("before", small), self.build("after", large)
        self.assertEqual(before.manifest["collections"]["nodes"]["root"]["kind"], "data")
        self.assertEqual(after.manifest["collections"]["nodes"]["root"]["kind"], "index")
        self.assertEqual(self.diff(before, after, include_rows=True)["changes"], self.oracle(before, after))
        self.assertEqual(self.diff(after, before, include_rows=True)["changes"], self.oracle(after, before))

    def test_partition_repacking_without_logical_changes(self):
        # Force rows sharing two hash digits so the different leaf targets
        # actually split/coalesce descendants, not just root relocation.
        rows = []
        for i in range(20000):
            identifier = f"key-{i}"
            if sha(identifier.encode()).startswith("ab"):
                rows.append({"id": identifier, "detail": "x" * 90})
                if len(rows) == 12:
                    break
        self.assertEqual(len(rows), 12)
        before, after = self.build("before", rows, target=256), self.build("after", rows, target=8192)
        self.assertEqual(self.diff(before, after)["changes"], [])
        self.assertEqual(self.diff(after, before)["changes"], [])

    def test_relocated_part_namespace_is_not_logical_identity(self):
        before = self.build("before", self.rows(20))
        path = self.root / "renamed-root.json"
        write_projection(path, before.metadata(), {"nodes": Collection(list(reversed(self.rows(20))), "id"),
                         "mapping": Collection([], None)}, target_part_bytes=256, work_dir=self.root)
        after = ProjectionReader(path)
        self.assertNotEqual(before.snapshot_digest, after.snapshot_digest)
        # Index JSON includes namespace paths, so indexes differ; data
        # semantic descriptors exclude only validated transport paths.
        for reader in (before, after):
            for leaf in [p for p in reader.closure_paths() if p.suffix == ".gz"]:
                leaf.unlink()
        self.assertEqual(self.diff(before, after, limits=DiffLimits(max_keys=0))["changes"], [])

    def test_changed_leaf_corruption_never_returns_complete(self):
        before, after = self.build("before", [{"id": "a"}]), self.build("after", [{"id": "a", "value": 2}])
        descriptor = after.manifest["collections"]["nodes"]["root"]
        path = after.path.parent / descriptor["path"]
        raw = path.read_bytes()
        path.write_bytes(b"!" + raw[1:])
        with self.assertRaises(ProjectionStoreError):
            self.diff(before, after)

    def test_declared_count_lies_fail_closed(self):
        before, after = self.build("before", [{"id": "a"}]), self.build("after", [{"id": "a", "value": 2}])
        for count in (0, 2):
            manifest = json.loads(after.path.read_bytes())
            manifest["collections"]["nodes"]["root"]["count"] = count
            after.path.write_bytes(canonical_bytes(manifest))
            selected = ProjectionReader(after.path)
            with self.assertRaisesRegex(ProjectionStoreError, "count mismatch"):
                self.diff(before, selected)

    def test_descriptor_difference_cannot_hide_behind_equal_hash(self):
        before, after = self.build("before", [{"id": "a"}]), self.build("after", [{"id": "a"}])
        manifest = json.loads(after.path.read_bytes())
        manifest["collections"]["nodes"]["root"]["size_bytes"] += 1
        after.path.write_bytes(canonical_bytes(manifest))
        after = ProjectionReader(after.path)
        with self.assertRaisesRegex(ProjectionStoreError, "size mismatch"):
            self.diff(before, after)

    def test_key_order_collection_and_schema_changes_require_bootstrap(self):
        before = self.build("before", [{"id": "a"}])
        for mutation in (
            lambda m: m.update(logical_schema="example_v2", header={"schema_version": "example_v2"}),
            lambda m: m["collections"]["nodes"].update(key_field="other"),
            lambda m: m["collections"]["nodes"].update(order_fields=["title"]),
            lambda m: m["collections"].pop("mapping"),
        ):
            after = self.build("after", [{"id": "a"}])
            manifest = json.loads(after.path.read_bytes())
            mutation(manifest)
            after.path.write_bytes(canonical_bytes(manifest))
            with self.assertRaisesRegex(ProjectionDiffRequiresBootstrap, "requires-bootstrap"):
                self.diff(before, ProjectionReader(after.path))

    def test_exact_bindings_and_explicit_baseline_trust_are_mandatory(self):
        before, after = self.build("before", []), self.build("after", [])
        for kwargs in ({"trusted_baseline_sha256": "f" * 64}, {"expected_before_sha256": "f" * 64},
                       {"expected_after_sha256": "f" * 64}, {"trusted_baseline_sha256": None}):
            arguments = {"expected_before_sha256": before.snapshot_digest,
                         "expected_after_sha256": after.snapshot_digest, "trusted_baseline_sha256": before.snapshot_digest}
            arguments.update(kwargs)
            with self.assertRaises(ProjectionDiffError):
                diff_projections(before, after, **arguments)

    def test_root_replacement_is_not_silently_refreshed(self):
        before, after = self.build("before", []), self.build("after", [])
        self.build("after", [{"id": "new"}])
        with self.assertRaisesRegex(ProjectionDiffError, "binding"):
            self.diff(before, after)

    def test_read_budgets_refuse_before_opening_changed_parts(self):
        before, after = self.build("before", [{"id": "a"}]), self.build("after", [{"id": "b"}])
        limits = [DiffLimits(max_opened_parts=0), DiffLimits(max_keys=0),
                  DiffLimits(max_decoded_bytes=2 * (MAX_ROOT_BYTES + 1))]
        for limit in limits:
            with self.subTest(limit=limit), patch.object(ProjectionReader, "_load", side_effect=AssertionError("premature read")):
                with self.assertRaises(ProjectionDiffBudgetExceeded):
                    self.diff(before, after, limits=limit)

    def test_physical_size_precheck_prevents_part_read(self):
        before, after = self.build("before", []), self.build("after", [{"id": "a"}])
        descriptor = after.manifest["collections"]["nodes"]["root"]
        target = after.path.parent / descriptor["path"]
        target.write_bytes(target.read_bytes() + b"!")
        original = Path.open
        def opened(path, *args, **kwargs):
            if path == target:
                raise AssertionError("read oversized stored part")
            return original(path, *args, **kwargs)
        with patch.object(Path, "open", opened), self.assertRaisesRegex(ProjectionStoreError, "size mismatch"):
            self.diff(before, after)

    def test_output_limits_are_exact_and_no_partial_packet_escapes(self):
        before, after = self.build("before", self.rows(4)), self.build("after", self.rows(2))
        for include_rows in (False, True):
            packet = self.diff(before, after, include_rows=include_rows)
            size = len(canonical_bytes(packet))
            self.assertEqual(self.diff(before, after, include_rows=include_rows,
                                       limits=DiffLimits(max_output_bytes=size)), packet)
            with self.assertRaises(ProjectionDiffBudgetExceeded):
                self.diff(before, after, include_rows=include_rows, limits=DiffLimits(max_output_bytes=size - 1))
        with self.assertRaises(ProjectionDiffBudgetExceeded):
            self.diff(before, after, limits=DiffLimits(max_opened_parts=3))

    def test_final_currentness_check_catches_mid_operation_replacement(self):
        before, after = self.build("before", [{"id": "a"}]), self.build("after", [{"id": "b"}])
        original = ProjectionReader._load
        def replaced(reader, descriptor, prefix):
            raw = original(reader, descriptor, prefix)
            if reader.path == after.path:
                manifest = json.loads(after.path.read_bytes())
                manifest["header"]["changed"] = True
                after.path.write_bytes(canonical_bytes(manifest))
            return raw
        with patch.object(ProjectionReader, "_load", replaced), self.assertRaisesRegex(ProjectionStoreError, "snapshot changed"):
            self.diff(before, after)

    def test_headers_and_digest_only_rows_obey_budget(self):
        before = self.build("before", [{"id": "a"}])
        after = self.build("after", [{"id": "a", "label": "changed"}],
                           header={"schema_version": "example_v1", "source": "x" * 3000})
        with self.assertRaises(ProjectionDiffBudgetExceeded):
            self.diff(before, after, limits=DiffLimits(max_output_bytes=2000))
        result = self.diff(before, after)
        self.assertNotIn("row", result["changes"][0]["after"])

    def test_scoped_composite_keys(self):
        rows = [{"pack": "p1", "edge": "e1"}, {"pack": "p2", "edge": "e1"}]
        before = self.build("before", rows, field=("pack", "edge"))
        after = self.build("after", [{**rows[0], "label": "changed"}, rows[1]], field=("pack", "edge"))
        changes = self.diff(before, after)["changes"]
        self.assertEqual([row["key"] for row in changes], ['["p1","e1"]'])

    def test_positional_sequence_snapshots_preserve_duplicate_order_and_ordinals(self):
        before_rows = [{"value": "repeat"}, {"value": "repeat"}, {"value": "tail"}]
        after_rows = copy.deepcopy(before_rows)
        after_rows[1]["value"] = "changed"

        def snapshot(name, rows):
            path = self.root / name / "sequence.min.json"
            write_projection(path, {"schema_version": "sequence_v1"}, {
                "items": Collection(rows, [], ()),
            }, target_part_bytes=256, work_dir=self.root)
            return ProjectionSnapshotView(path.read_bytes(), path)

        before, after = snapshot("before-sequence", before_rows), snapshot("after-sequence", after_rows)
        packet = diff_projection_snapshots(
            before, after,
            expected_before_sha256=before.snapshot_digest,
            expected_after_sha256=after.snapshot_digest,
            trusted_baseline_sha256=before.snapshot_digest,
            include_rows=True,
        )
        self.assertEqual([change["key"] for change in packet["changes"]], ["00000000000000000001"])
        self.assertEqual(packet["changes"][0]["before"]["row"], before_rows[1])
        self.assertEqual(packet["changes"][0]["after"]["row"], after_rows[1])

    def test_positional_sequence_diff_rejects_malformed_and_out_of_range_ordinals(self):
        for key in ("not-an-ordinal", "00000000000000000001"):
            with self.subTest(key=key):
                before = self.build("before", [], field=[], order=())
                after = self.build("after", ["value"], field=[], order=())
                manifest = json.loads(after.path.read_bytes())
                descriptor = manifest["collections"]["nodes"]["root"]
                self.rewrite_part(after, descriptor, canonical_bytes({"key": key, "value": "value"}))
                after.path.write_bytes(canonical_bytes(manifest))
                after = ProjectionReader(after.path)
                with self.assertRaisesRegex(ProjectionStoreError, "invalid sequence position"):
                    self.diff(before, after)

    def test_invalid_limits(self):
        for value in (-1, False, 1.5, None):
            with self.assertRaises(ValueError):
                DiffLimits(max_keys=value)

    def rewrite_part(self, reader, descriptor, raw):
        stored = _gzip(raw) if descriptor["kind"] == "data" else raw
        digest = sha(stored)
        suffix = ".jsonl.gz" if descriptor["kind"] == "data" else ".index.json"
        path = reader.path.parent / (reader.path.stem + ".parts") / digest[:2] / (digest + suffix)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(stored)
        descriptor.update(path=path.relative_to(reader.path.parent).as_posix(), sha256=digest,
                          size_bytes=len(stored), decoded_bytes=len(raw), decoded_sha256=sha(raw))

    def test_malformed_changed_index_child_fails_before_child_read(self):
        before, after = self.build("before", []), self.build("after", self.rows(3))
        manifest = json.loads(after.path.read_bytes())
        descriptor = manifest["collections"]["nodes"]["root"]
        index = json.loads((after.path.parent / descriptor["path"]).read_bytes())
        next(iter(index["children"].values()))["count"] = False
        self.rewrite_part(after, descriptor, canonical_bytes(index))
        after.path.write_bytes(canonical_bytes(manifest))
        after = ProjectionReader(after.path)
        with self.assertRaisesRegex(ProjectionStoreError, "invalid part count or size"):
            self.diff(before, after)

    def test_changed_leaf_with_valid_hash_still_checks_every_row_identity(self):
        before, after = self.build("before", []), self.build("after", [{"id": "a"}])
        manifest = json.loads(after.path.read_bytes())
        descriptor = manifest["collections"]["nodes"]["root"]
        self.rewrite_part(after, descriptor, canonical_bytes({"key": "a", "value": {"id": "different"}}))
        after.path.write_bytes(canonical_bytes(manifest))
        after = ProjectionReader(after.path)
        with self.assertRaisesRegex(ProjectionStoreError, "record identity"):
            self.diff(before, after)

    def test_stale_reader_cache_cannot_hide_changed_leaf_corruption(self):
        before, after = self.build("before", []), self.build("after", [{"id": "a"}])
        cached = ProjectionReader(after.path)
        cached.get("nodes", "a")
        descriptor = cached.manifest["collections"]["nodes"]["root"]
        path = after.path.parent / descriptor["path"]
        raw = path.read_bytes()
        path.write_bytes(b"!" + raw[1:])
        with self.assertRaises(ProjectionStoreError):
            self.diff(before, cached)

    def test_no_materialization_or_collection_iteration_fallback(self):
        before, after = self.build("before", self.rows(2)), self.build("after", self.rows(4))
        with patch.object(ProjectionReader, "materialize", side_effect=AssertionError("materialize")), \
             patch.object(ProjectionReader, "iter_items", side_effect=AssertionError("collection scan")), \
             patch.object(ProjectionReader, "closure_paths", side_effect=AssertionError("closure scan")):
            self.assertEqual(len(self.diff(before, after)["changes"]), 2)


if __name__ == "__main__":
    unittest.main()
