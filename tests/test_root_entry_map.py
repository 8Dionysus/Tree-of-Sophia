from __future__ import annotations

import json
import unittest
from pathlib import Path

import sys


sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from root_entry_map_common import SURFACE_PAYLOAD, build_payload


class RootEntryMapTests(unittest.TestCase):
    def test_build_payload_stays_tree_first(self) -> None:
        payload = build_payload()

        self.assertEqual(payload["schema_version"], "tos_root_entry_map_v1")
        self.assertEqual(payload["schema_ref"], "schemas/root-entry-map.schema.json")
        self.assertEqual(payload["owner_repo"], "Tree-of-Sophia")
        self.assertEqual(payload["surface_kind"], "root_entry_map")
        self.assertEqual(payload["authority_ref"], SURFACE_PAYLOAD["authority_ref"])
        self.assertEqual(payload["public_root_ref"], "README.md")
        self.assertEqual(
            [route["route_id"] for route in payload["routes"]],
            [
                "current-tiny-entry",
                "tree-first-model",
                "bounded-export",
            ],
        )

    def test_current_tiny_entry_route_keeps_example_first(self) -> None:
        payload = build_payload()
        route = next(route for route in payload["routes"] if route["route_id"] == "current-tiny-entry")

        self.assertEqual(route["surface_ref"], "examples/tos_tiny_entry_route.example.json")
        self.assertEqual(
            route["verification_refs"],
            [
                "docs/TINY_ENTRY_ROUTE.md",
                "docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md",
            ],
        )

    def test_payload_is_json_serializable(self) -> None:
        payload = build_payload()
        rendered = json.dumps(payload, separators=(",", ":"))

        self.assertIn("root_entry_map", rendered)
        self.assertNotIn('"surface_ref":"scripts/', rendered)
        self.assertNotIn('"surface_ref":"src/', rendered)


if __name__ == "__main__":
    unittest.main()
