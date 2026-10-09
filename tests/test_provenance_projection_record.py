"""Generic bounded carrier regression for large provenance graph records."""
from __future__ import annotations

import json
from pathlib import Path
import tempfile
import unittest

from jsonschema import Draft202012Validator
from tos_access.projection_store import MAX_PART_BYTES, ProjectionReader, canonical_bytes
from partitioned_projection_common import canonical_digest, write_partitioned_payload

ROOT = Path(__file__).resolve().parents[1]


class ProvenanceProjectionRecordTests(unittest.TestCase):
    @staticmethod
    def large_v1_event(event_id: str = "tos.event.synthetic-large-provenance") -> dict:
        inputs = [
            {"ref": f"tos.file.synthetic-input-{index:05d}-{'a' * 70}", "role": "input-" + "i" * 100_000, "sha256": "a" * 64}
            for index in range(24)
        ]
        outputs = [
            {"ref": f"tos.file.synthetic-output-{index:05d}-{'b' * 70}", "role": "output-" + "o" * 100_000, "sha256": "b" * 64}
            for index in range(24)
        ]
        return {
            "schema_version": "tos_provenance_event_v1", "event_id": event_id,
            "event_type": "annotation", "started_at": "2026-09-15T00:00:00Z",
            "ended_at": "2026-09-15T00:00:00Z", "agent_refs": ["software:test-provenance-projection"],
            "inputs": inputs, "outputs": outputs,
            "method": {"maker_type": "software", "name": "synthetic-provenance-test", "version": "1",
                       "configuration": {"aliases": ["synthetic:large-event"]}},
            "status": "completed_with_warnings", "event_version": 1,
        }

    def test_large_v1_event_is_retained_once_and_round_trips_under_part_bound(self):
        event = self.large_v1_event()
        event_raw = canonical_bytes(event)
        self.assertGreaterEqual(len(event_raw), 4 * 1024 * 1024)
        self.assertLess(len(event_raw), 5 * 1024 * 1024)
        Draft202012Validator(
            json.loads((ROOT / "ToS/contracts/provenance-event.schema.json").read_text(encoding="utf-8"))
        ).validate(event)
        source_ref = "ToS/source-witnesses/history/fixture/large-provenance.jsonl"
        node = {
            "node_id": f"event:{event['event_id']}", "node_kind": "provenance_event",
            "source_ref": source_ref, "source_line": 7, "source_sha256": canonical_digest(event),
            "properties": {"source_event": event, "event_ref": event["event_id"],
                           "event_type": event["event_type"], "started_at": event["started_at"],
                           "ended_at": event["ended_at"], "agent_refs": event["agent_refs"],
                           "method": event["method"], "status": event["status"],
                           "event_version": event["event_version"]},
        }
        self.assertEqual(node["properties"]["source_event"], event)
        self.assertEqual(node["source_ref"], source_ref)
        self.assertEqual(node["source_line"], 7)
        self.assertEqual(node["source_sha256"], canonical_digest(event))
        self.assertNotIn("inputs", node["properties"])
        self.assertNotIn("outputs", node["properties"])
        legacy_node = json.loads(json.dumps(node))
        legacy_node["properties"].update(event)
        self.assertLess(len(canonical_bytes(node)), MAX_PART_BYTES)
        self.assertGreater(len(canonical_bytes(legacy_node)), MAX_PART_BYTES)
        payload = {"schema_version": "tos_source_witness_bibliographic_graph_v1", "nodes": [node],
                   "edges": [], "claim_traces": [], "input_digests": {}}
        with tempfile.TemporaryDirectory(prefix="tos-provenance-record-") as directory:
            path = Path(directory) / "graph.json"
            write_partitioned_payload(path, payload)
            reader = ProjectionReader(path)
            self.assertEqual(reader.get("nodes", node["node_id"]), node)
            self.assertEqual(reader.materialize(), payload)
            descriptor = reader.manifest["collections"]["nodes"]["root"]
            self.assertEqual(descriptor["kind"], "data")
            self.assertLessEqual(descriptor["decoded_bytes"], MAX_PART_BYTES)
