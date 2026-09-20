from __future__ import annotations

import hashlib
import json
import math
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from tos_access.projection_store import (
    Collection, ProjectionReader, ProjectionStoreError, canonical_bytes,
    is_partitioned, load_projection, write_projection, _gzip, _atomic_write,
)


class ProjectionStoreTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.path = self.root / "example.min.json"
        self.header = {"schema_version": "example_v1", "source_navigation": {"counts": {"nodes": 300}}}
        self.rows = [{"id": f"node-{i:05}", "label": f"Название {i}", "properties": {"detail": "x" * 60}}
                     for i in range(300)]

    def build(self, rows=None, **kwargs):
        return write_projection(self.path, self.header, {
            "source_navigation/nodes": Collection(iter(rows if rows is not None else self.rows), "id"),
            "input_digests": Collection(iter([("source-a", "abc"), ("source-b", "def")]), None),
            "empty": Collection(iter(()), "id"),
        }, target_part_bytes=1024, **kwargs)

    def test_round_trip_and_explicit_order(self):
        self.build(list(reversed(self.rows)))
        reader = ProjectionReader(self.path)
        self.assertTrue(is_partitioned(self.path))
        self.assertEqual(reader.snapshot_digest, hashlib.sha256(self.path.read_bytes()).hexdigest())
        self.assertEqual(reader.metadata(), self.header)
        self.assertEqual(reader.get("source_navigation/nodes", "node-00012"), self.rows[12])
        self.assertIsNone(reader.get("source_navigation/nodes", "missing"))
        expected = {**self.header, "source_navigation": {"counts": {"nodes": 300}, "nodes": self.rows},
                    "input_digests": {"source-a": "abc", "source-b": "def"}, "empty": []}
        self.assertEqual(reader.materialize(), expected)
        self.assertEqual(load_projection(self.path), expected)

    def test_scoped_record_identity_preserves_repeated_local_ids(self):
        rows = [{"pack_id": pack, "edge_id": "m001", "label": pack}
                for pack in ("pack-b", "pack-a")]
        write_projection(self.path, {"schema_version": "example_v1"}, {
            "edges": Collection(rows, ("pack_id", "edge_id"))})
        reader = ProjectionReader(self.path)
        self.assertEqual(reader.get("edges", '["pack-a","m001"]'), rows[1])
        self.assertEqual(reader.materialize()["edges"], list(reversed(rows)))
        list(reader.closure_paths())

    def test_record_change_reuses_unrelated_content(self):
        self.build()
        before = {p.name for p in ProjectionReader(self.path).closure_paths() if p.suffix == ".gz"}
        rows = [dict(row) for row in self.rows]
        rows[12]["label"] = "changed"
        self.build(rows)
        after = {p.name for p in ProjectionReader(self.path).closure_paths() if p.suffix == ".gz"}
        self.assertEqual(len(before - after), 1)
        self.assertEqual(len(after - before), 1)
        self.assertGreater(len(before & after), 100)

    def test_reproducible_across_order_and_location(self):
        self.build()
        before = self.path.read_bytes()
        self.build(list(reversed(self.rows)))
        self.assertEqual(self.path.read_bytes(), before)
        other = self.root / "second" / self.path.name
        write_projection(other, self.header, {
            "source_navigation/nodes": Collection(self.rows, "id"),
            "input_digests": Collection([("source-b", "def"), ("source-a", "abc")], None),
            "empty": Collection([], "id"),
        }, target_part_bytes=1024)
        self.assertEqual(other.read_bytes(), before)

    def test_selective_read_ignores_unrelated_missing_leaf(self):
        self.build()
        probe = ProjectionReader(self.path, cache_bytes=0)
        touched = []
        original = probe._load
        def load(descriptor, prefix):
            touched.append(probe.path.parent / descriptor["path"])
            return original(descriptor, prefix)
        probe._load = load
        self.assertEqual(probe.get("source_navigation/nodes", self.rows[0]["id"]), self.rows[0])
        closure = list(ProjectionReader(self.path).closure_paths())
        unrelated = next(p for p in closure if p.suffix == ".gz" and p not in touched)
        unrelated.unlink()
        fresh = ProjectionReader(self.path)
        self.assertEqual(fresh.get("source_navigation/nodes", self.rows[0]["id"]), self.rows[0])
        self.assertLess(fresh.parts_read, 8)
        with self.assertRaises(ProjectionStoreError):
            list(fresh.closure_paths())

    def test_corruption_and_symlink_fail_closed(self):
        self.build()
        leaf = next(p for p in ProjectionReader(self.path).closure_paths() if p.suffix == ".gz")
        raw = leaf.read_bytes()
        leaf.write_bytes(bytes([raw[0] ^ 1]) + raw[1:])
        with self.assertRaises(ProjectionStoreError):
            list(ProjectionReader(self.path).closure_paths())
        leaf.unlink()
        outside = self.root / "outside"
        outside.write_bytes(raw)
        leaf.symlink_to(outside)
        with self.assertRaises(ProjectionStoreError):
            list(ProjectionReader(self.path).closure_paths())

    def test_manifest_path_escape_rejected_before_read(self):
        self.build()
        root = json.loads(self.path.read_bytes())
        root["collections"]["empty"]["root"]["path"] = "../outside"
        self.path.write_bytes(canonical_bytes(root))
        with self.assertRaises(ProjectionStoreError):
            ProjectionReader(self.path)

    def test_declared_decoded_size_cannot_raise_bound(self):
        self.build()
        root = json.loads(self.path.read_bytes())
        root["collections"]["empty"]["root"]["decoded_bytes"] = 1024 ** 3
        self.path.write_bytes(canonical_bytes(root))
        with self.assertRaises(ProjectionStoreError):
            ProjectionReader(self.path)

    def test_duplicate_identity_does_not_publish(self):
        self.build()
        before = self.path.read_bytes()
        with self.assertRaisesRegex(ProjectionStoreError, "duplicate"):
            self.build([self.rows[0], self.rows[0]])
        self.assertEqual(self.path.read_bytes(), before)

    def test_oversized_record_does_not_publish(self):
        self.build()
        before = self.path.read_bytes()
        with self.assertRaisesRegex(ProjectionStoreError, "individual record"):
            self.build([{"id": "large", "detail": "x" * (8 * 1024 * 1024)}])
        self.assertEqual(self.path.read_bytes(), before)

    def test_failed_input_iterator_does_not_publish(self):
        self.build()
        before = self.path.read_bytes()
        def fail():
            yield self.rows[0]
            raise RuntimeError("interrupted input")
        with self.assertRaisesRegex(RuntimeError, "interrupted"):
            self.build(fail())
        self.assertEqual(self.path.read_bytes(), before)

    def test_snapshot_change_is_detected(self):
        self.build()
        reader = ProjectionReader(self.path)
        self.build(self.rows + [{"id": "another", "label": "new"}])
        with self.assertRaisesRegex(ProjectionStoreError, "snapshot changed"):
            reader.require_current()

    def test_cache_is_bounded_and_closure_does_not_glob(self):
        self.build()
        unrelated = self.path.with_name(self.path.stem + ".parts") / "secret-payload"
        unrelated.write_bytes(b"not a runtime subject")
        reader = ProjectionReader(self.path, cache_bytes=500)
        paths = list(reader.closure_paths())
        self.assertNotIn(unrelated, paths)
        self.assertLessEqual(reader._cached_bytes, 500)

    def test_shared_part_cache_does_not_bypass_descriptor_integrity(self):
        self.build()
        root = json.loads(self.path.read_bytes())
        duplicate = json.loads(json.dumps(root["collections"]["empty"]))
        duplicate["root"]["size_bytes"] += 1
        root["collections"]["empty_other"] = duplicate
        self.path.write_bytes(canonical_bytes(root))
        reader = ProjectionReader(self.path)
        self.assertEqual(list(reader.iter_collection("empty")), [])
        with self.assertRaisesRegex(ProjectionStoreError, "size mismatch"):
            list(reader.iter_collection("empty_other"))

    def test_prune_removes_only_unreferenced_owned_objects(self):
        self.build()
        old = set(ProjectionReader(self.path).closure_paths())
        unrelated = self.path.with_name(self.path.stem + ".parts") / "unowned.txt"
        unrelated.write_text("keep")
        self.build(self.rows[:10], prune=True)
        new = set(ProjectionReader(self.path).closure_paths())
        self.assertTrue(unrelated.exists())
        self.assertTrue(all(not p.exists() for p in old - new))

    def test_prune_preserves_external_files_under_symlink_directory(self):
        self.build()
        part_dir = self.path.with_name(self.path.stem + ".parts")
        raw = b"external generated-looking bytes"
        digest = hashlib.sha256(raw).hexdigest()
        # This namespace may already contain a leaf; choose a free prefix.
        while (part_dir / digest[:2]).exists():
            raw += b"x"
            digest = hashlib.sha256(raw).hexdigest()
        outside = self.root / "outside"
        outside.mkdir()
        target = outside / (digest + ".jsonl.gz")
        target.write_bytes(raw)
        (part_dir / digest[:2]).symlink_to(outside, target_is_directory=True)
        self.build(self.rows[:1], prune=True)
        self.assertEqual(target.read_bytes(), raw)

    def test_duplicate_json_members_rejected(self):
        self.path.write_bytes(b'{"schema_version":"x","schema_version":"y"}')
        with self.assertRaisesRegex(ProjectionStoreError, "duplicate"):
            ProjectionReader(self.path)

    def test_mapping_stream_is_explicit(self):
        self.build()
        reader = ProjectionReader(self.path)
        self.assertEqual(dict(reader.iter_items("input_digests")), {"source-a": "abc", "source-b": "def"})
        self.assertEqual(sorted(reader.iter_collection("input_digests"), key=lambda x: x['key']),
                         [{"key": "source-a", "value": "abc"}, {"key": "source-b", "value": "def"}])

    def positional_manifest(self, rows, order=()):
        return write_projection(self.path, {"schema_version": "sequence_v1"}, {
            "items": Collection(rows, [], order),
        }, target_part_bytes=1024, work_dir=self.root)

    def rewrite_sequence_leaf(self, key, value):
        manifest = json.loads(self.path.read_bytes())
        descriptor = manifest["collections"]["items"]["root"]
        self.assertEqual(descriptor["kind"], "data")
        raw = canonical_bytes({"key": key, "value": value})
        stored = _gzip(raw)
        digest = hashlib.sha256(stored).hexdigest()
        part = self.path.with_name(self.path.stem + ".parts") / digest[:2] / (digest + ".jsonl.gz")
        part.parent.mkdir(parents=True, exist_ok=True)
        part.write_bytes(stored)
        descriptor.update(path=part.relative_to(self.path.parent).as_posix(),
                          sha256=digest, size_bytes=len(stored), decoded_bytes=len(raw),
                          decoded_sha256=hashlib.sha256(raw).hexdigest())
        self.path.write_bytes(canonical_bytes(manifest))

    def test_positional_sequence_preserves_order_and_duplicate_values(self):
        rows = [{"value": "repeat"}, {"value": "repeat"}, {"value": "tail"}]
        self.positional_manifest(rows)
        reader = ProjectionReader(self.path)
        expected = {f"{index:020d}": row for index, row in enumerate(rows)}
        items = list(reader.iter_items("items"))
        # The iterator follows hash partition placement. Check identity and
        # values independently; positional order is an export concern.
        self.assertEqual(dict(items), expected)
        self.assertEqual(dict(zip((key for key, _ in items), reader.iter_collection("items"))), expected)
        self.assertGreater(
            sum(1 for path in reader.closure_paths() if path.name.endswith(".jsonl.gz")), 1
        )
        self.assertEqual(reader.materialize(), {"schema_version": "sequence_v1", "items": rows})

    def test_positional_sequence_rejects_malformed_and_out_of_range_ordinals(self):
        for key in ("not-an-ordinal", "00000000000000000001"):
            with self.subTest(key=key):
                self.positional_manifest(["value"])
                self.rewrite_sequence_leaf(key, "value")
                with self.assertRaisesRegex(ProjectionStoreError, "invalid sequence position"):
                    list(ProjectionReader(self.path, cache_bytes=0).iter_items("items"))

    def test_positional_sequence_rejects_nonempty_order_fields(self):
        with self.assertRaisesRegex(ProjectionStoreError, "positional sequence cannot declare record ordering"):
            self.positional_manifest(["value"], order=("rank",))
        self.positional_manifest(["value"])
        manifest = json.loads(self.path.read_bytes())
        manifest["collections"]["items"]["order_fields"] = ["rank"]
        self.path.write_bytes(canonical_bytes(manifest))
        with self.assertRaisesRegex(ProjectionStoreError, "positional sequence cannot declare record ordering"):
            ProjectionReader(self.path)

    def numeric_manifest(self, header=None):
        return write_projection(self.path, header or {"schema_version": "numbers_v1"}, {
            "numbers": Collection([("value", None)], None),
        }, work_dir=self.root)

    def write_matched_numeric_leaf(self, value_json):
        manifest = self.numeric_manifest()
        raw = b'{"key":"value","value":' + value_json + b'}\n'
        stored = _gzip(raw)
        digest = hashlib.sha256(stored).hexdigest()
        part = self.path.with_name(self.path.stem + ".parts") / digest[:2] / (digest + ".jsonl.gz")
        part.parent.mkdir(parents=True, exist_ok=True)
        part.write_bytes(stored)
        manifest["collections"]["numbers"]["root"].update(
            path=part.relative_to(self.path.parent).as_posix(), sha256=digest,
            size_bytes=len(stored), decoded_bytes=len(raw), decoded_sha256=hashlib.sha256(raw).hexdigest())
        self.path.write_bytes(canonical_bytes(manifest))

    def test_nonfinite_root_numbers_are_rejected_during_strict_parse(self):
        manifest = self.numeric_manifest({"schema_version": "numbers_v1", "probe": "NON_FINITE"})
        for literal in (b"1e309", b"-1e309", b"NaN", b"Infinity", b"-Infinity"):
            with self.subTest(literal=literal):
                self.path.write_bytes(canonical_bytes(manifest).replace(b'"NON_FINITE"', literal))
                with self.assertRaisesRegex(ProjectionStoreError, "non-finite JSON number"):
                    ProjectionReader(self.path)

    def test_nonfinite_matched_digest_leaf_is_rejected(self):
        for literal in (b"1e309", b"-1e309", b"NaN", b"Infinity", b"-Infinity"):
            with self.subTest(literal=literal):
                self.write_matched_numeric_leaf(literal)
                reader = ProjectionReader(self.path, cache_bytes=0)
                with self.assertRaisesRegex(ProjectionStoreError, "non-finite JSON number"):
                    reader.get("numbers", "value")

    def test_finite_numeric_types_and_precision_are_preserved(self):
        self.write_matched_numeric_leaf(b'[1,1.0,9007199254740993,1e308,5e-324,-0.0,false]')
        values = ProjectionReader(self.path).get("numbers", "value")
        self.assertEqual([type(value) for value in values], [int, float, int, float, float, float, bool])
        self.assertEqual(values[:3], [1, 1.0, 9007199254740993])
        self.assertEqual(values[3:5], [1e308, 5e-324])
        self.assertEqual(math.copysign(1, values[5]), -1)
        self.assertTrue(all(math.isfinite(value) for value in values if type(value) is float))
        self.numeric_manifest({"schema_version": "numbers_v1", "int": 1, "float": 1.0, "large": 1e308})
        header = ProjectionReader(self.path).metadata()
        self.assertIs(type(header["int"]), int)
        self.assertIs(type(header["float"]), float)
        self.assertEqual(header["large"], 1e308)

    def test_writer_requires_nonempty_string_schema_before_publication(self):
        self.numeric_manifest()
        before = self.path.read_bytes()
        for header in ({}, {"schema_version": ""}, {"schema_version": None},
                       {"schema_version": False}, {"schema_version": 1}):
            with self.subTest(header=header):
                with self.assertRaisesRegex(ProjectionStoreError, "nonempty schema_version"):
                    write_projection(self.path, header, {"numbers": Collection([], None)}, work_dir=self.root)
                self.assertEqual(self.path.read_bytes(), before)

    def test_reader_requires_matching_nonempty_string_logical_schema(self):
        manifest = self.numeric_manifest()
        for logical, header in (("", {"schema_version": ""}), (None, {"schema_version": None}),
                                (None, {}), (False, {"schema_version": False}), (1, {"schema_version": 1}),
                                ("numbers_v1", {}), ("numbers_v1", {"schema_version": "other_v1"})):
            with self.subTest(logical=logical, header=header):
                manifest.update(logical_schema=logical, header=header)
                self.path.write_bytes(canonical_bytes(manifest))
                with self.assertRaisesRegex(ProjectionStoreError, "invalid partitioned projection manifest"):
                    ProjectionReader(self.path)

    def test_atomic_replacement_does_not_read_oversized_existing_target(self):
        target = self.root / "owned-output"
        target.write_bytes(b"x" * 4096)
        replacement = b"bounded replacement\n"
        original = Path.open

        def checked(path, *args, **kwargs):
            if path == target:
                raise AssertionError("oversized existing target must not be opened")
            return original(path, *args, **kwargs)

        with patch.object(Path, "open", checked), \
             patch.object(Path, "read_bytes", side_effect=AssertionError("unbounded existing read")):
            _atomic_write(target, replacement)
        self.assertEqual(target.read_bytes(), replacement)

    def test_atomic_same_size_comparison_is_bounded_and_keeps_equal_output(self):
        target = self.root / "owned-output"
        replacement = b"bounded replacement\n"
        original = Path.open
        reads = []

        class BoundedRead:
            def __init__(self, stream):
                self.stream = stream

            def __enter__(self):
                return self

            def __exit__(self, *args):
                return self.stream.__exit__(*args)

            def read(self, size=-1):
                if size != len(replacement) + 1:
                    raise AssertionError("comparison must use an explicit bounded read")
                reads.append(size)
                return self.stream.read(size)

        def checked(path, *args, **kwargs):
            stream = original(path, *args, **kwargs)
            return BoundedRead(stream) if path == target else stream

        for previous in (replacement, b"x" * len(replacement)):
            with self.subTest(equal=previous == replacement):
                target.write_bytes(previous)
                before_inode = target.stat().st_ino
                with patch.object(Path, "open", checked), \
                     patch.object(Path, "read_bytes", side_effect=AssertionError("unbounded existing read")):
                    _atomic_write(target, replacement)
                self.assertEqual(target.read_bytes(), replacement)
                if previous == replacement:
                    self.assertEqual(target.stat().st_ino, before_inode)
        self.assertEqual(reads, [len(replacement) + 1, len(replacement) + 1])


if __name__ == "__main__":
    unittest.main()
