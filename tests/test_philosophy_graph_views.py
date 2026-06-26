from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from philosophy_graph_views_common import GRAPH_VIEW_CATALOG_PATH, build_payload, render_payload  # noqa: E402


class PhilosophyGraphViewsTest(unittest.TestCase):
    def load_catalog(self) -> dict[str, object]:
        return json.loads(GRAPH_VIEW_CATALOG_PATH.read_text(encoding="utf-8"))

    def test_generated_catalog_matches_builder(self) -> None:
        expected = render_payload(build_payload())
        current = GRAPH_VIEW_CATALOG_PATH.read_text(encoding="utf-8")
        self.assertEqual(current, expected)

    def test_catalog_has_expected_counts_and_boundary(self) -> None:
        payload = self.load_catalog()
        self.assertEqual(payload["counts"]["views"], 11)
        self.assertEqual(payload["counts"]["graph_layers"], 7)
        self.assertEqual(payload["counts"]["lens_review_contracts"], 11)
        self.assertEqual(payload["counts"]["diagnostics"], 0)
        self.assertEqual(payload["runtime_projection_boundary"]["runtime_owner"], "abyss-stack")
        self.assertEqual(payload["atlas_projection_ref"], "ToS/derived-exports/philosophy_atlas_projection.min.json")
        self.assertEqual(
            payload["lens_review_contract_ref"],
            "ToS/philosophy/graph-workbench/views/lens-review-contracts.json",
        )
        self.assertEqual(payload["default_lens_review_requirements"]["ui_mcp_payload_mode"], "cluster-first")

    def test_catalog_view_ids_match_atlas_projection_view_nodes(self) -> None:
        payload = self.load_catalog()
        projection = json.loads((REPO_ROOT / payload["atlas_projection_ref"]).read_text(encoding="utf-8"))
        catalog_ids = {view["view_id"] for view in payload["views"]}
        projection_ids = {
            node["node_id"].removeprefix("graph-view:")
            for node in projection["nodes"]
            if node["node_type"] == "graph-view"
        }
        self.assertEqual(catalog_ids, projection_ids)

    def test_catalog_preserves_route_card_lens_and_boundary(self) -> None:
        payload = self.load_catalog()
        views = {view["view_id"]: view for view in payload["views"]}
        self.assertIn("Shows translation, copying, commentary", views["transmission"]["lens"])
        self.assertIn("does not make research packets into source witnesses", views["source-evidence"]["boundary"])
        self.assertIn("research packets remain preparation", views["source-evidence"]["source_posture"])
        self.assertIn("candidate-to-canon pressure", views["canon-promotion"]["agent_packet_hint"])

    def test_catalog_exposes_useful_switching_filters(self) -> None:
        payload = self.load_catalog()
        views = {view["view_id"]: view for view in payload["views"]}
        chronology = views["chronology"]["current_projection_filters"]
        self.assertIn("formation", chronology["row_fields"])
        self.assertIn("canonization", chronology["row_fields"])
        concept = views["concept-lineage"]["current_projection_filters"]
        self.assertIn("concept", concept["node_type_keys"])
        self.assertIn("develops_concept", concept["relation_kind_keys"])
        canon_layers = set(views["canon-promotion"]["graph_layers"])
        self.assertTrue({"candidate-relation", "canonical-relation"} <= canon_layers)
        canon_clusters = set(views["canon-promotion"]["collapse_rule"]["default_cluster_kinds"])
        self.assertIn("canon-candidate-status", canon_clusters)
        chronology_order = views["chronology"]["ordering_hints"]
        self.assertIn("formation", chronology_order)


if __name__ == "__main__":
    unittest.main()
