from __future__ import annotations

import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from tos_access.projection_store import (
    Collection, ProjectionReader, ProjectionStoreError, canonical_bytes,
    is_partitioned, load_projection, write_projection,
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


if __name__ == "__main__":
    unittest.main()
