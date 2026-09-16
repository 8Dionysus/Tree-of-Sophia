"""Bounded regression coverage for large provenance graph records."""

from __future__ import annotations

import copy
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[1]
for directory in (
    ROOT / "scripts",
    ROOT / "access/src",
    ROOT / "access/tests",
):
    if str(directory) not in sys.path:
        sys.path.insert(0, str(directory))

import source_witness_bibliographic_graph_common as graph
from partitioned_projection_common import (
    build_storage,
    canonical_digest,
    json_chunks,
    write_partitioned_payload,
)
from source_assembly_fixture import SourceAssemblyFixture
from tos_access.projection_store import MAX_PART_BYTES, ProjectionReader, canonical_bytes


class ProvenanceProjectionRecordTests(unittest.TestCase):
    @staticmethod
    def large_v1_event(event_id: str = "tos.event.synthetic-large-provenance") -> dict:
        """Return a schema-shaped event whose complete source body is 4--5 MiB."""

        inputs = [
            {
                "ref": f"tos.file.synthetic-input-{index:05d}-{'a' * 70}",
                "role": "input-" + "i" * 100_000,
                "sha256": "a" * 64,
            }
            for index in range(24)
        ]
        outputs = [
            {
                "ref": f"tos.file.synthetic-output-{index:05d}-{'b' * 70}",
                "role": "output-" + "o" * 100_000,
                "sha256": "b" * 64,
            }
            for index in range(24)
        ]
        return {
            "schema_version": "tos_provenance_event_v1",
            "event_id": event_id,
            "event_type": "annotation",
            "started_at": "2026-09-15T00:00:00Z",
            "ended_at": "2026-09-15T00:00:00Z",
            "agent_refs": ["software:test-provenance-projection"],
            "inputs": inputs,
            "outputs": outputs,
            "method": {
                "maker_type": "software",
                "name": "synthetic-provenance-test",
                "version": "1",
                "configuration": {"aliases": ["synthetic:large-event"]},
            },
            "status": "completed_with_warnings",
            "event_version": 1,
        }

    @staticmethod
    def small_v1_event(event_id: str) -> dict:
        return {
            "schema_version": "tos_provenance_event_v1",
            "event_id": event_id,
            "event_type": "annotation",
            "started_at": "2026-09-07T00:00:00Z",
            "ended_at": "2026-09-07T00:00:01Z",
            "agent_refs": ["software:test-provenance-projection"],
            "inputs": [
                {"ref": "ToS/source-witnesses/history/fixture/input.json", "role": "input", "sha256": "a" * 64}
            ],
            "outputs": [
                {"ref": "ToS/source-witnesses/history/fixture/output.json", "role": "output", "sha256": "b" * 64}
            ],
            "method": {
                "maker_type": "software",
                "name": "synthetic-provenance-test",
                "version": "2",
                "configuration": {"aliases": ["synthetic:changed-event"]},
            },
            "status": "completed",
            "event_version": 1,
        }

    def test_large_v1_event_is_retained_once_and_round_trips_under_part_bound(self):
        event = self.large_v1_event()
        event_raw = canonical_bytes(event)
        self.assertGreaterEqual(len(event_raw), 4 * 1024 * 1024)
        self.assertLess(len(event_raw), 5 * 1024 * 1024)
        Draft202012Validator(
            json.loads((ROOT / "ToS/contracts/provenance-event.schema.json").read_text(encoding="utf-8"))
        ).validate(event)

        indexed = {
            "payload": event,
            "source_ref": "ToS/source-witnesses/history/fixture/large-provenance.jsonl",
            "source_line": 7,
            "source_sha256": canonical_digest(event),
        }
        node = graph._event_node(indexed, repo_root=ROOT)
        properties = node["properties"]
        self.assertEqual(properties["source_event"], event)
        self.assertEqual(node["source_ref"], indexed["source_ref"])
        self.assertEqual(node["source_line"], indexed["source_line"])
        self.assertEqual(node["source_sha256"], indexed["source_sha256"])
        self.assertEqual(properties["event_ref"], event["event_id"])
        self.assertEqual(properties["agent_refs"], event["agent_refs"])
        self.assertEqual(properties["method"]["configuration"]["aliases"], ["synthetic:large-event"])
        self.assertNotIn("inputs", properties)
        self.assertNotIn("outputs", properties)

        legacy_node = copy.deepcopy(node)
        legacy_node["properties"] = {
            **event,
            "event_ref": event["event_id"],
            "event_type": event["event_type"],
            "started_at": event["started_at"],
            "ended_at": event["ended_at"],
            "agent_refs": event["agent_refs"],
            "method": event["method"],
            "status": event["status"],
            "event_version": event["event_version"],
            "source_event": dict(event),
        }
        self.assertLess(len(canonical_bytes(node)), MAX_PART_BYTES)
        self.assertGreater(len(canonical_bytes(legacy_node)), MAX_PART_BYTES)

        payload = {
            "schema_version": "tos_source_witness_bibliographic_graph_v1",
            "nodes": [node],
            "edges": [],
            "claim_traces": [],
            "input_digests": {},
        }
        with tempfile.TemporaryDirectory(prefix="tos-provenance-record-") as directory:
            path = Path(directory) / "graph.json"
            write_partitioned_payload(path, payload)
            reader = ProjectionReader(path)
            self.assertEqual(reader.get("nodes", node["node_id"]), node)
            self.assertEqual(reader.materialize(), payload)
            descriptor = reader.manifest["collections"]["nodes"]["root"]
            self.assertEqual(descriptor["kind"], "data")
            self.assertLessEqual(descriptor["decoded_bytes"], MAX_PART_BYTES)

    def test_adjacent_event_cache_reuses_and_switches_and_disk_matches_list(self):
        fixture = SourceAssemblyFixture(
            code_root=ROOT,
            source_root=ROOT / "access/tests/fixtures/source-assembly",
        )
        with fixture.historical_fixture() as (root, _history, _real, claims, rebuild):
            original_event = claims[0]["provenance_event_ref"]
            with patch.object(graph, "_event_node", wraps=graph._event_node) as event_node:
                list_projection = rebuild()
            self.assertEqual(event_node.call_count, 1)
            self.assertEqual(
                len([node for node in list_projection["nodes"] if node["node_kind"] == "provenance_event"]),
                1,
            )

            changed_event = self.small_v1_event("tos.event.historical-fixture-changed")
            event_path = root / "ToS/source-witnesses/history/fixture/provenance.jsonl"
            event_path.write_text(
                event_path.read_text(encoding="utf-8")
                + "\n"
                + json.dumps(changed_event, ensure_ascii=False)
                + "\n",
                encoding="utf-8",
            )
            claims[1]["provenance_event_ref"] = changed_event["event_id"]
            claims[2]["provenance_event_ref"] = changed_event["event_id"]

            with patch.object(graph, "_event_node", wraps=graph._event_node) as event_node:
                list_projection = rebuild()
            self.assertEqual(event_node.call_count, 2)
            event_nodes = [
                node for node in list_projection["nodes"] if node["node_kind"] == "provenance_event"
            ]
            self.assertEqual(
                {node["properties"]["event_ref"] for node in event_nodes},
                {original_event, changed_event["event_id"]},
            )
            self.assertEqual(len(event_nodes), 2)
            changed_node = next(
                node for node in event_nodes
                if node["properties"]["event_ref"] == changed_event["event_id"]
            )
            self.assertEqual(changed_node["properties"]["source_event"], changed_event)

            with build_storage() as storage:
                with patch.object(graph, "_event_node", wraps=graph._event_node) as event_node:
                    disk_projection = graph.build_payload(root, storage=storage)
                self.assertEqual(event_node.call_count, 2)
                disk_json = json.loads("".join(json_chunks(disk_projection)))
            self.assertEqual(disk_json, list_projection)


if __name__ == "__main__":
    unittest.main()
