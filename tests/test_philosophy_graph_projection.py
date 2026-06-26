from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from philosophy_graph_projection_common import GRAPH_PROJECTION_PATH, build_payload, render_payload  # noqa: E402


class PhilosophyGraphProjectionTest(unittest.TestCase):
    def load_projection(self) -> dict[str, object]:
        return json.loads(GRAPH_PROJECTION_PATH.read_text(encoding="utf-8"))

    def test_generated_projection_matches_builder(self) -> None:
        expected = render_payload(build_payload())
        current = GRAPH_PROJECTION_PATH.read_text(encoding="utf-8")
        self.assertEqual(current, expected)

    def test_projection_has_expected_counts_and_boundary(self) -> None:
        payload = self.load_projection()
        counts = payload["counts"]
        self.assertEqual(counts["views"], 11)
        self.assertEqual(counts["graph_layers"], 7)
        self.assertGreater(counts["nodes"], 0)
        self.assertGreater(counts["edges"], 0)
        self.assertEqual(counts["diagnostics"], 0)
        self.assertEqual(payload["runtime_projection_boundary"]["runtime_owner"], "abyss-stack")

    def test_every_projected_edge_endpoint_exists(self) -> None:
        payload = self.load_projection()
        node_ids = {node["node_id"] for node in payload["nodes"]}
        for edge in payload["edges"]:
            self.assertIn(edge["from_id"], node_ids)
            self.assertIn(edge["to_id"], node_ids)

    def test_views_are_materialized_with_source_refs(self) -> None:
        payload = self.load_projection()
        views = {view["view_id"]: view for view in payload["views"]}
        chronology = views["chronology"]
        self.assertEqual(chronology["layout_hint"], "timeline-lanes")
        self.assertGreater(len(chronology["nodes"]), 0)
        self.assertGreater(len(chronology["edges"]), 0)
        self.assertIn("ToS/philosophy/atlas/master-tables/table-i/rows.jsonl", chronology["source_refs"])
        source_evidence = views["source-evidence"]
        self.assertIn("evidence-relation", source_evidence["graph_layers"])
        self.assertGreater(len(source_evidence["source_refs"]), 0)

    def test_global_nodes_and_edges_carry_view_and_layer_membership(self) -> None:
        payload = self.load_projection()
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        edges = {edge["edge_id"]: edge for edge in payload["edges"]}
        self.assertIn("chronology", nodes["atlas-row:A01"]["view_ids"])
        self.assertTrue(nodes["atlas-row:A01"]["graph_layers"])
        self.assertIn("chronology", edges["edge:row:A01:has-dossier:A01"]["view_ids"])
        self.assertIn("historical-relation", edges["edge:row:A01:has-dossier:A01"]["graph_layers"])


if __name__ == "__main__":
    unittest.main()
