from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from philosophy_atlas_projection_common import PROJECTION_PATH, build_payload, render_payload  # noqa: E402


class PhilosophyAtlasProjectionTest(unittest.TestCase):
    def test_generated_projection_matches_builder(self) -> None:
        expected = render_payload(build_payload())
        current = PROJECTION_PATH.read_text(encoding="utf-8")
        self.assertEqual(current, expected)

    def test_projection_has_expected_atlas_counts(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        self.assertEqual(payload["counts"]["master_tables"], 3)
        self.assertEqual(payload["counts"]["master_rows"], 190)
        self.assertEqual(payload["counts"]["dossiers"], 10)
        self.assertEqual(payload["counts"]["dossier_node_rows"], 308)
        self.assertEqual(payload["counts"]["dossier_relation_rows"], 289)

    def test_projection_keeps_runtime_owner_downstream(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        self.assertEqual(payload["runtime_projection_boundary"]["runtime_owner"], "abyss-stack")
        self.assertEqual(payload["source_atlas_ref"], "ToS/philosophy/atlas/atlas.manifest.json")

    def test_projection_links_rows_to_available_dossiers(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        edges = {
            (edge["from_id"], edge["predicate_id"], edge["to_id"])
            for edge in payload["edges"]
        }
        self.assertIn(("atlas-row:A01", "has_prepared_dossier", "atlas-dossier:A01"), edges)
        self.assertIn(("atlas-row:A11", "has_prepared_dossier", "atlas-dossier:A11"), edges)

    def test_projection_exposes_graph_view_routes(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        node_ids = {node["node_id"] for node in payload["nodes"]}
        self.assertIn("graph-view:chronology", node_ids)
        self.assertIn("graph-view:transmission", node_ids)


if __name__ == "__main__":
    unittest.main()
