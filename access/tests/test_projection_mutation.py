"""Synthetic bounded COW transport invariants, not source admission."""
from dataclasses import replace
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from tos_access.projection_store import (
    Collection, ProjectionReader, MAX_ROOT_BYTES, canonical_bytes, write_projection,
)
from tos_access.projection_diff import diff_projections
from tos_access.projection_mutation import (
    MISSING, MutationLimits, ProjectionChange, ProjectionHeaderChange,
    ProjectionSnapshotView, ProjectionMutationError, ProjectionMutationBudgetExceeded,
    ProjectionMutationRequiresBootstrap, stage_projection_changes,
    stage_projection_snapshot_changes,
)
import tos_access.projection_mutation as mutation
import tos_access.projection_diff as projection_diff


def sha(value):
    return hashlib.sha256(canonical_bytes(value)).hexdigest()


class ProjectionMutationTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.path = self.directory / "selected.json"
        self.header = {"schema_version": "fixture_v1", "revision": 1}
        self.rows = {f"key-{i}": {"x": i, "text": "я" * 30} for i in range(30)}
        self.write()

    def write(self, *, rows=None, collection=None, target=256):
        write_projection(self.path, self.header, {
            "records": collection or Collection(list((self.rows if rows is None else rows).items()), None),
        }, target_part_bytes=target, work_dir=self.directory)
        self.reader = ProjectionReader(self.path)
        self.original = self.path.read_bytes()

    def change(self, key="key-2", after=MISSING):
        if after is MISSING:
            after = {"x": 200, "unknown": {"nested": [None, False, -0.0]}}
        return ProjectionChange("records", key, True, sha(self.rows[key]), True, after)

    def stage(self, changes=None, **kwargs):
        return stage_projection_changes(self.reader,
            expected_before_sha256=self.reader.snapshot_digest,
            trusted_baseline_sha256=self.reader.snapshot_digest,
            changes=[self.change()] if changes is None else changes, **kwargs)

    def stage_snapshot(self, view=None, changes=None, **kwargs):
        view = view or ProjectionSnapshotView(self.original, self.path)
        return stage_projection_snapshot_changes(view,
            expected_before_sha256=view.snapshot_digest,
            trusted_baseline_sha256=view.snapshot_digest,
            changes=[self.change()] if changes is None else changes, **kwargs)

    def test_snapshot_chained_stages_never_read_or_select_root(self):
        view = ProjectionSnapshotView(self.original, self.path)
        self.path.unlink()
        with patch.object(projection_diff, "_root_size", side_effect=AssertionError("root stat")), \
                patch.object(ProjectionReader, "__init__", side_effect=AssertionError("root open")), \
                patch.object(ProjectionReader, "require_current", side_effect=AssertionError("currentness")), \
                patch.object(ProjectionReader, "materialize", side_effect=AssertionError("full export")), \
                patch.object(ProjectionReader, "iter_items", side_effect=AssertionError("full scan")):
            first = self.stage_snapshot(view)
            next_change = ProjectionChange("records", "key-2", True,
                sha(self.change().after_value), True, {"x": 300})
            second = self.stage_snapshot(first.snapshot(), [next_change],
                header_change=ProjectionHeaderChange(sha(self.header),
                    {"schema_version": "fixture_v1", "revision": 2}))
        self.assertFalse(self.path.exists())
        self.assertEqual(second.before_sha256, first.after_sha256)
        self.assertEqual(first.snapshot().lookup("records", "key-2")["value"], self.change().after_value)
        self.assertEqual(second.snapshot().lookup("records", "key-2")["value"], {"x": 300})
        self.assertEqual(second.snapshot().metadata()["revision"], 2)
        for candidate in (first, second):
            self.assertEqual(candidate.namespace_path, self.path)
            self.assertFalse(candidate.published)
            self.assertFalse(candidate.establishes_epoch)
            self.assertFalse(candidate.target_closure_verified)
            self.assertFalse(candidate.delta()["target_closure_verified"])

    def test_snapshot_ignores_other_selected_bytes_and_refuses_foreign_bindings(self):
        view = ProjectionSnapshotView(self.original, self.path)
        self.path.write_bytes(b"unrelated selected bytes")
        self.stage_snapshot(view)
        self.assertEqual(self.path.read_bytes(), b"unrelated selected bytes")
        for binding, trust in [(view.snapshot_digest, "0" * 64), ("0" * 64, "0" * 64),
                               ("bad", "bad")]:
            with self.subTest(binding=binding, trust=trust), patch.object(mutation, "_install") as install:
                with self.assertRaises(ProjectionMutationError):
                    stage_projection_snapshot_changes(view, expected_before_sha256=binding,
                        trusted_baseline_sha256=trust, changes=[])
                install.assert_not_called()
        with self.assertRaises(TypeError):
            self.stage_snapshot(self.reader)

    def test_snapshot_read_budgets_precede_decode_and_part_open(self):
        view = ProjectionSnapshotView(self.original, self.path)
        with patch.object(ProjectionReader, "_initialize_manifest", side_effect=AssertionError("decoded")):
            with self.assertRaises(ProjectionMutationBudgetExceeded):
                self.stage_snapshot(view, [], limits=replace(MutationLimits(),
                    max_decoded_bytes=len(self.original) - 1))
        no_op = self.stage_snapshot(view, [])
        self.assertEqual(dict(no_op.accounting)["decoded_bytes"], len(self.original))
        for kwargs in ({"max_opened_parts": 0}, {"max_stored_read_bytes": 0},
                       {"max_decoded_bytes": len(self.original)}):
            with self.subTest(limits=kwargs), patch.object(ProjectionReader, "_load",
                    side_effect=AssertionError("part opened")):
                with self.assertRaises(ProjectionMutationBudgetExceeded):
                    self.stage_snapshot(view, limits=replace(MutationLimits(), **kwargs))
        real_load = ProjectionReader._load
        def refuse_leaf(reader, descriptor, prefix):
            if descriptor["kind"] == "data":
                self.fail("leaf opened before key reservation")
            return real_load(reader, descriptor, prefix)
        with patch.object(ProjectionReader, "_load", new=refuse_leaf):
            with self.assertRaises(ProjectionMutationBudgetExceeded):
                self.stage_snapshot(view, limits=replace(MutationLimits(), max_keys=0))

    def test_snapshot_all_budgets_and_selected_candidate_parity(self):
        selected = self.stage()
        candidate = self.stage_snapshot()
        self.assertEqual(candidate.root_bytes, selected.root_bytes)
        self.assertEqual(candidate.delta_bytes, selected.delta_bytes)
        for name, value in candidate.accounting:
            if value:
                with self.subTest(name=name), self.assertRaises(ProjectionMutationBudgetExceeded):
                    self.stage_snapshot(limits=replace(MutationLimits(), **{"max_" + name: value - 1}))
        exact = MutationLimits(**{"max_" + name: value for name, value in candidate.accounting})
        self.assertEqual(self.stage_snapshot(limits=exact).root_bytes, candidate.root_bytes)

    def test_snapshot_malformed_touched_index_refuses_before_install(self):
        descriptor = self.reader.manifest["collections"]["records"]["root"]
        index = json.loads((self.directory / descriptor["path"]).read_bytes())
        index["count"] += 1
        raw = canonical_bytes(index)
        manifest = json.loads(self.original)
        root = manifest["collections"]["records"]["root"]
        digest = hashlib.sha256(raw).hexdigest()
        root.update(size_bytes=len(raw), decoded_bytes=len(raw),
                    sha256=digest, decoded_sha256=digest,
                    path=f"selected.parts/{digest[:2]}/{digest}.index.json")
        part = self.directory / root["path"]
        part.parent.mkdir(exist_ok=True)
        part.write_bytes(raw)
        view = ProjectionSnapshotView(canonical_bytes(manifest), self.path)
        with patch.object(mutation, "_install") as install:
            with self.assertRaisesRegex(ProjectionMutationError, "invalid partition directory"):
                self.stage_snapshot(view)
            install.assert_not_called()

    def test_snapshot_skipped_missing_parts_do_not_certify_closure(self):
        descriptor = self.reader.manifest["collections"]["records"]["root"]
        index = self.reader._children(descriptor, "")
        digit = hashlib.sha256(b"key-2").hexdigest()[0]
        untouched = next(child for key, child in index.items() if key != digit)
        (self.directory / untouched["path"]).unlink()
        candidate = self.stage_snapshot()
        self.assertFalse(candidate.target_closure_verified)
        self.assertFalse(candidate.delta()["target_closure_verified"])

    def test_candidate_reuses_namespace_and_never_selects_root(self):
        old_closure = self.reader.closure_paths()
        with patch.object(ProjectionReader, "materialize", side_effect=AssertionError("full export")), \
                patch.object(ProjectionReader, "iter_items", side_effect=AssertionError("full scan")), \
                patch.object(Path, "rglob", side_effect=AssertionError("namespace scan")), \
                patch.object(mutation.os, "replace", side_effect=AssertionError("root replacement")):
            candidate = self.stage()
        self.assertEqual(self.path.read_bytes(), self.original)
        self.assertEqual(self.reader.get("records", "key-2"), self.rows["key-2"])
        self.assertEqual(candidate.snapshot().lookup("records", "key-2")["value"], self.change().after_value)
        self.assertFalse(candidate.published)
        self.assertFalse(candidate.target_closure_verified)
        self.assertFalse(candidate.establishes_epoch)
        self.assertEqual(candidate.namespace_path, self.path)
        self.assertTrue(all(path.exists() for path in old_closure))
        self.assertTrue(all(ref.startswith("selected.parts/") for ref in candidate.created_parts))
        self.assertLess(dict(candidate.accounting)["keys"], len(self.rows))

    def test_null_absence_and_exact_json_identity(self):
        self.rows = {"null": None, "false": False, "float": 0.0, "minus": -0.0}
        self.write(target=1024)
        candidate = self.stage([
            ProjectionChange("records", "null", True, sha(None), False),
            ProjectionChange("records", "new", False, None, True, None),
            ProjectionChange("records", "false", True, sha(False), True, 0),
            ProjectionChange("records", "float", True, sha(0.0), True, 0),
            ProjectionChange("records", "minus", True, sha(-0.0), True, 0.0),
        ])
        view = candidate.snapshot()
        self.assertEqual(view.lookup("records", "null"), {"present": False})
        self.assertEqual(view.lookup("records", "new"), {"present": True, "value": None})
        self.assertEqual(len(candidate.delta()["changes"]), 5)
        self.assertIs(type(view.lookup("records", "false")["value"]), int)

    def test_view_is_immutable_not_current_reader_or_mutation_baseline(self):
        candidate = self.stage()
        view = candidate.snapshot()
        self.assertNotIsInstance(view, ProjectionReader)
        with self.assertRaises(ProjectionMutationError):
            view.require_current()
        with self.assertRaises(TypeError):
            diff_projections(self.reader, view, expected_before_sha256=self.reader.snapshot_digest,
                expected_after_sha256=view.snapshot_digest, trusted_baseline_sha256=self.reader.snapshot_digest)
        with self.assertRaises(TypeError):
            stage_projection_changes(view, expected_before_sha256=view.snapshot_digest,
                trusted_baseline_sha256=view.snapshot_digest, changes=[])
        copy = candidate.delta()
        copy["changes"].clear()
        self.assertEqual(len(candidate.delta()["changes"]), 1)
        with self.assertRaises(ProjectionMutationError):
            ProjectionSnapshotView(b" " * (MAX_ROOT_BYTES + 1), self.path)

    def test_split_delete_all_and_unchanged_descriptors(self):
        before = self.reader.manifest["collections"]["records"]["root"]
        candidate = self.stage()
        after = json.loads(candidate.root_bytes)["collections"]["records"]["root"]
        old_index = json.loads((self.directory / before["path"]).read_bytes())
        new_index = json.loads((self.directory / after["path"]).read_bytes())
        changed_digit = hashlib.sha256(b"key-2").hexdigest()[0]
        for digit, descriptor in old_index["children"].items():
            if digit != changed_digit:
                self.assertEqual(new_index["children"][digit], descriptor)
        empty = self.stage([ProjectionChange("records", key, True, sha(value), False)
                            for key, value in self.rows.items()])
        self.assertEqual(empty.snapshot().materialize()["records"], {})
        root = json.loads(empty.root_bytes)["collections"]["records"]["root"]
        self.assertEqual((root["kind"], root["prefix"], root["count"]), ("data", "", 0))
        self.write(rows={"only": None})
        changes = [ProjectionChange("records", key, False, None, True, value)
                   for key, value in self.rows.items()]
        split = self.stage(changes, target_part_bytes=256)
        self.assertEqual(split.snapshot().materialize()["records"], {"only": None, **self.rows})

    def test_full_writer_logical_parity_not_root_byte_parity(self):
        candidate = self.stage()
        expected = {**self.rows, "key-2": self.change().after_value}
        other = self.directory / "full.json"
        write_projection(other, self.header, {"records": Collection(expected.items(), None)},
                         target_part_bytes=512, work_dir=self.directory)
        self.assertEqual(candidate.snapshot().materialize(), ProjectionReader(other).materialize())

    def test_header_guards_and_total_order_profile(self):
        candidate = self.stage([], header_change=ProjectionHeaderChange(sha(self.header),
            {"schema_version": "fixture_v1", "revision": 2}))
        self.assertEqual(candidate.snapshot().metadata()["revision"], 2)
        for header, error in [({"schema_version": "other"}, ProjectionMutationRequiresBootstrap),
                              ({"schema_version": "fixture_v1", "records": []}, ProjectionMutationError),
                              ({"schema_version": None}, ProjectionMutationError)]:
            with self.subTest(header=header), self.assertRaises(error):
                self.stage([], header_change=ProjectionHeaderChange(sha(self.header), header))
        self.write(collection=Collection([{"id": "a", "group": 1}, {"id": "b", "group": 1}], "id", ("group",)))
        with self.assertRaises(ProjectionMutationRequiresBootstrap):
            self.stage([])
        self.write(collection=Collection([{"id": "a", "group": 1}], "id", ("group", "id")))
        self.stage([])
        self.write(collection=Collection([{"a": "x", "b": "y"}], ("a", "b"), ("b", "a")))
        self.stage([])

    def test_positional_sequence_changes_require_complete_publication(self):
        self.write(collection=Collection(["repeat", "repeat", "tail"], [], ()))
        change = ProjectionChange("records", "00000000000000000001", True,
                                  sha("repeat"), True, "changed")
        with self.assertRaisesRegex(ProjectionMutationRequiresBootstrap, "positional sequence changes"):
            self.stage([change])
        with self.assertRaisesRegex(ProjectionMutationRequiresBootstrap, "positional sequence changes"):
            self.stage_snapshot(changes=[change])

    def test_bad_changes_and_conflicts_have_no_part_writes(self):
        good = self.change()
        bad = [replace(good, before_present=1), replace(good, after_present=0),
               replace(good, before_sha256=sha(False)), replace(good, key="missing"),
               replace(good, collection="missing"), replace(good, after_value=(1, 2)),
               replace(good, after_value={1: "x"}), replace(good, after_value=float("nan")),
               replace(good, after_present=False), replace(good, after_value=MISSING),
               replace(good, before_present=False),
               ProjectionChange("records", "x", False, None, False)]
        for change in bad:
            with self.subTest(change=change), patch.object(mutation, "_install") as install:
                with self.assertRaises(ProjectionMutationError):
                    self.stage([change])
                install.assert_not_called()
        with patch.object(mutation, "_install") as install:
            with self.assertRaises(ProjectionMutationError):
                self.stage([good, good])
            install.assert_not_called()
        self.assertEqual(self.path.read_bytes(), self.original)

    def test_wrong_array_key_and_header_preconditions(self):
        self.write(collection=Collection([{"id": "a"}], "id"))
        with self.assertRaises(ProjectionMutationError):
            self.stage([ProjectionChange("records", "a", True, sha({"id": "a"}), True, {"id": "b"})])
        with self.assertRaises(ProjectionMutationError):
            self.stage([], header_change=ProjectionHeaderChange("0" * 64, self.header))

    def test_each_budget_refuses_and_exact_accounting_is_reusable(self):
        candidate = self.stage()
        usage = dict(candidate.accounting)
        for name, value in usage.items():
            if not value:
                continue
            with self.subTest(name=name), self.assertRaises(ProjectionMutationBudgetExceeded):
                self.stage(limits=replace(MutationLimits(), **{"max_" + name: value - 1}))
        # A repeated stage additionally reads matching immutable output objects.
        repeated = self.stage()
        exact = MutationLimits(**{"max_" + name: value for name, value in repeated.accounting})
        self.assertEqual(self.stage(limits=exact).root_bytes, candidate.root_bytes)
        self.assertEqual(self.path.read_bytes(), self.original)
        for value in (-1, True, 1.5):
            with self.assertRaises(ValueError):
                MutationLimits(max_keys=value)

    def test_read_budget_reserved_before_part_open(self):
        with patch.object(ProjectionReader, "_load", side_effect=AssertionError("part opened")):
            with self.assertRaises(ProjectionMutationBudgetExceeded):
                self.stage(limits=replace(MutationLimits(), max_opened_parts=0))
        with patch.object(mutation, "_install") as install:
            with self.assertRaises(ProjectionMutationBudgetExceeded):
                self.stage(limits=replace(MutationLimits(), max_written_parts=0))
            install.assert_not_called()

    def test_stale_binding_and_mutation_during_install_refuse(self):
        with self.assertRaises(ProjectionMutationError):
            stage_projection_changes(self.reader, expected_before_sha256=self.reader.snapshot_digest,
                trusted_baseline_sha256="0" * 64, changes=[])
        real = mutation._install
        def drift(path, raw, budget):
            result = real(path, raw, budget)
            self.path.write_bytes(self.original + b" ")
            return result
        with patch.object(mutation, "_install", side_effect=drift), self.assertRaises(ProjectionMutationError):
            self.stage()
        self.assertEqual(self.path.read_bytes(), self.original + b" ")

    def test_immutable_collision_never_overwrites_and_failure_never_returns_candidate(self):
        candidate = self.stage()
        path = self.directory / candidate.created_parts[0]
        path.write_bytes(b"corrupt")
        with self.assertRaises(ProjectionMutationError):
            self.stage()
        self.assertEqual(path.read_bytes(), b"corrupt")
        self.assertEqual(self.path.read_bytes(), self.original)

    def test_installed_parts_fsynced_and_replay_creates_no_parts(self):
        real = mutation.os.fsync
        with patch.object(mutation.os, "fsync", wraps=real) as sync:
            candidate = self.stage()
        self.assertGreaterEqual(sync.call_count, 4 * len(candidate.created_parts))
        self.assertEqual(self.stage().created_parts, ())

    def test_untouched_missing_part_is_not_fresh_availability_claim(self):
        descriptor = self.reader.manifest["collections"]["records"]["root"]
        index = self.reader._children(descriptor, "")
        digit = hashlib.sha256(b"key-2").hexdigest()[0]
        untouched = next(child for key, child in index.items() if key != digit)
        (self.directory / untouched["path"]).unlink()
        candidate = self.stage()
        self.assertFalse(candidate.target_closure_verified)

    def test_touched_corruption_fails_before_install(self):
        descriptor = self.reader.manifest["collections"]["records"]["root"]
        (self.directory / descriptor["path"]).write_bytes(b"corrupt")
        with patch.object(mutation, "_install") as install:
            with self.assertRaises(ProjectionMutationError):
                self.stage()
            install.assert_not_called()

    def test_lying_leaf_count_refuses_before_excess_row_parse(self):
        self.write(rows={"one": None})
        manifest = json.loads(self.original)
        manifest["collections"]["records"]["root"]["count"] = 0
        self.path.write_bytes(canonical_bytes(manifest))
        self.reader = ProjectionReader(self.path)
        row = canonical_bytes({"key": "one", "value": None}).strip()
        parse = projection_diff._strict_json
        def guarded(raw):
            if raw.strip() == row:
                self.fail("extra row parsed beyond its reserved count")
            return parse(raw)
        with patch.object(projection_diff, "_strict_json", side_effect=guarded):
            with self.assertRaises(ProjectionMutationError):
                self.stage([ProjectionChange("records", "one", True, sha(None), False)])

    def test_symlink_destination_and_namespace_are_not_followed(self):
        candidate = self.stage()
        part = self.directory / candidate.created_parts[0]
        outside = self.directory / "outside"
        outside.write_bytes(b"untouched")
        part.unlink()
        part.symlink_to(outside)
        with self.assertRaises(ProjectionMutationError):
            self.stage()
        self.assertEqual(outside.read_bytes(), b"untouched")
        # A fresh namespace collision must not create a child in its target.
        namespace = self.directory / "new.parts"
        external = self.directory / "external"
        external.mkdir()
        namespace.symlink_to(external, target_is_directory=True)
        with self.assertRaises(OSError):
            mutation._install(namespace / "ab" / "part", b"new", mutation._Budget(MutationLimits()))
        self.assertEqual(list(external.iterdir()), [])

    def test_mid_install_failure_returns_no_candidate_and_preserves_selected_root(self):
        real = mutation._install
        installed = []
        def interrupted(path, raw, budget):
            if installed:
                raise OSError("simulated installation interruption")
            result = real(path, raw, budget)
            installed.append(path)
            return result
        with patch.object(mutation, "_install", side_effect=interrupted), self.assertRaises(OSError):
            self.stage()
        self.assertEqual(len(installed), 1)
        self.assertTrue(installed[0].is_file())
        self.assertEqual(self.path.read_bytes(), self.original)
        self.assertEqual(self.reader.materialize()["records"], self.rows)

    def test_late_old_state_conflict_does_not_install_earlier_planned_parts(self):
        with patch.object(mutation, "_install") as install:
            with self.assertRaises(ProjectionMutationError):
                self.stage([self.change("key-2"), replace(self.change("key-20"), before_sha256="0" * 64)])
            install.assert_not_called()

    def test_input_limits_depth_and_empty_noop(self):
        no_op = self.stage([])
        self.assertEqual(no_op.root_bytes, self.original)
        self.assertEqual(no_op.created_parts, ())
        self.assertEqual(dict(no_op.accounting)["decoded_bytes"], 3 * (MAX_ROOT_BYTES + 1))
        deeply_nested = None
        for _ in range(130):
            deeply_nested = [deeply_nested]
        with self.assertRaises(ProjectionMutationError):
            self.stage([self.change(after=deeply_nested)])
        with self.assertRaises(ProjectionMutationBudgetExceeded):
            self.stage([self.change(after="x" * 500)], limits=replace(MutationLimits(), max_input_bytes=256))
        with self.assertRaises(ProjectionMutationBudgetExceeded):
            self.stage([self.change(after=[0] * 512 + [object()])],
                       limits=replace(MutationLimits(), max_input_bytes=256))


if __name__ == "__main__":
    unittest.main()
