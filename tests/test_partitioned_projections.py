"""Generic partitioned-carrier behavior, independent of retired ToS Python engines."""
from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest
from jsonschema import Draft202012Validator
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
import sys
sys.path.insert(0, str(ROOT / 'scripts'))
from partitioned_projection_common import (
    ProjectionReader, build_storage, check_partitioned_payload, disk_payload,
    json_chunks, write_partitioned_payload, schema_validator, DiskSequence,
)
from tos_access.projection_store import MAX_ROOT_BYTES


def _corpus_payload(diagnostics):
    return {
        "schema_version": "tos_corpus_index_v1",
        "diagnostics": diagnostics,
        "manifests": [],
        "nodes": [],
        "resources": [],
        "relation_packs": [],
        "relation_edges": [],
        "source_navigation": {"nodes": [], "edges": [], "rights": []},
    }


class PartitionedSourceProjectionTests(unittest.TestCase):
    def test_philosophy_carrier_preserves_numeric_view_order_in_source_reader(self):
        payload = {
            "schema_version": "tos_philosophy_graph_projection_v2",
            "nodes": [{"node_id": "a", "label": "source node"}],
            "edges": [], "clusters": [],
            "views": [{"view_id": str(order), "order": order, "node_ids": ["a"]}
                      for order in (10, 20, 100)],
            "review_packets": [{"packet_id": "retained-inline"}],
        }
        with tempfile.TemporaryDirectory() as directory, build_storage() as storage:
            path = Path(directory) / "ToS/derived-exports/philosophy_graph_projection.min.json"
            write_partitioned_payload(path, payload)
            reader = check_partitioned_payload(path, payload)
            self.assertEqual(reader.materialize(), payload)

    def test_streaming_schema_keeps_item_assertions_without_index_scans(self):
        rows = [{"id": str(index)} for index in range(100)]
        rows[37] = {"id": 37}
        schema = {"type": "array", "prefixItems": [{"type": "object"}],
                  "items": {"type": "object", "required": ["id"],
                            "properties": {"id": {"type": "string"}}}}
        errors = lambda validator, value: [(list(error.path), error.validator)
                                           for error in validator.iter_errors(value)]
        with build_storage() as storage:
            staged = storage.sequence(rows)
            with patch.object(DiskSequence, "__getitem__", side_effect=AssertionError("indexed scan")):
                self.assertEqual(errors(schema_validator(schema), staged),
                                 errors(Draft202012Validator(schema), rows))
                schema["items"] = False
                self.assertEqual(errors(schema_validator(schema), staged),
                                 errors(Draft202012Validator(schema), rows))

    def test_large_diagnostics_are_partitioned_and_all_readers_preserve_sequence(self):
        repeated = "x" * 100_000
        diagnostics = [
            {"level": "warning", "message": repeated},
            {"level": "warning", "message": "y" * 100_000},
            {"level": "warning", "message": repeated},
        ]
        payload = _corpus_payload(diagnostics)
        logical_bytes = json.dumps(payload, ensure_ascii=False, separators=(",", ":")).encode()
        self.assertGreater(len(logical_bytes), MAX_ROOT_BYTES)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "ToS/derived-exports/tos_corpus_index.min.json"
            write_partitioned_payload(path, payload)
            self.assertLessEqual(path.stat().st_size, MAX_ROOT_BYTES)
            reader = ProjectionReader(path)
            self.assertNotIn("diagnostics", reader.metadata())
            self.assertEqual(reader.manifest["collections"]["diagnostics"]["key_field"], [])
            expected_items = {f"{index:020d}": row for index, row in enumerate(diagnostics)}
            self.assertEqual(dict(reader.iter_items("diagnostics")), expected_items)
            self.assertEqual(reader.materialize(), payload)
            with build_storage() as storage:
                disk = disk_payload(reader, storage)
                self.assertEqual(json.loads("".join(json_chunks(disk))), payload)


if __name__ == '__main__':
    unittest.main()
