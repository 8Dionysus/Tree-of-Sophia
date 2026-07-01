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
        self.assertGreater(counts["clusters"], 0)
        self.assertEqual(counts["review_packets"], 11)
        self.assertGreaterEqual(counts["unresolved_review_surfaces"], 0)
        self.assertEqual(counts["diagnostics"], 0)
        self.assertEqual(payload["runtime_projection_boundary"]["runtime_owner"], "abyss-stack")
        self.assertEqual(payload["visibility_model"]["default_payload_mode"], "cluster-first")
        self.assertEqual(
            set(payload["visibility_model"]["layer_ids"]),
            {layer["layer_id"] for layer in payload["graph_layers"]},
        )
        self.assertEqual(payload["snapshot_review"]["snapshot_schema_version"], "tos_philosophy_graph_projection_snapshot_v1")
        self.assertEqual(payload["snapshot_review"]["diff_route"]["mode"], "fingerprint-ready")
        self.assertEqual(len(payload["snapshot_review"]["current_snapshot"]["projection_fingerprint"]), 64)
        self.assertEqual(len(payload["snapshot_review"]["current_snapshot"]["view_fingerprints"]), 11)
        self.assertEqual(
            payload["content_language_contract"]["source_ref"],
            "ToS/philosophy/atlas/multilingual/content-labels.json",
        )
        self.assertEqual(
            payload["content_language_contract"]["text_bearing_nodes_contract_ref"],
            "ToS/philosophy/atlas/multilingual/text-bearing-nodes.contract.json",
        )

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
        self.assertIn("research packets remain preparation", source_evidence["source_posture"])
        self.assertIn("source-witness", source_evidence["collapse_rule"]["default_cluster_kinds"])

    def test_global_nodes_and_edges_carry_view_and_layer_membership(self) -> None:
        payload = self.load_projection()
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        edges = {edge["edge_id"]: edge for edge in payload["edges"]}
        self.assertIn("chronology", nodes["atlas-row:A01"]["view_ids"])
        self.assertTrue(nodes["atlas-row:A01"]["graph_layers"])
        self.assertIn("chronology", edges["edge:row:A01:has-dossier:A01"]["view_ids"])
        self.assertEqual(
            set(edges["edge:row:A01:has-dossier:A01"]["graph_layers"]),
            {"evidence-relation", "source-relation"},
        )
        self.assertIn("candidate-node:table-i-a01-node-001", nodes)
        self.assertIn("candidate-relation", nodes["candidate-node:table-i-a01-node-001"]["graph_layers"])
        self.assertIn("edge:candidate-relation:table-i-a01-relation-001", edges)
        self.assertIn("script-decipherment", edges["edge:candidate-relation:table-i-a01-relation-001"]["view_ids"])

    def test_layer_counts_are_semantic_not_view_wide(self) -> None:
        payload = self.load_projection()
        layer_counts = {row["layer_id"]: row for row in payload["layer_counts"]}
        self.assertLess(layer_counts["canonical-relation"]["edge_count"], layer_counts["candidate-relation"]["edge_count"])
        self.assertLess(layer_counts["historical-relation"]["node_count"], layer_counts["evidence-relation"]["node_count"])
        packets = {packet["view_id"]: packet for packet in payload["review_packets"]}
        chronology_layers = {row["layer_id"]: row for row in packets["chronology"]["layer_counts"]}
        self.assertNotEqual(
            chronology_layers["evidence-relation"]["node_count"],
            chronology_layers["historical-relation"]["node_count"],
        )

    def test_clusters_preserve_membership_and_source_refs(self) -> None:
        payload = self.load_projection()
        node_ids = {node["node_id"] for node in payload["nodes"]}
        edge_ids = {edge["edge_id"] for edge in payload["edges"]}
        clusters = payload["clusters"]
        self.assertGreater(len(clusters), 0)
        kinds = {cluster["cluster_kind"] for cluster in clusters}
        self.assertIn("region", kinds)
        self.assertIn("source-witness", kinds)
        for cluster in clusters:
            self.assertTrue(cluster["source_refs"])
            self.assertTrue(set(cluster["member_node_ids"]) <= node_ids)
            self.assertTrue(set(cluster["member_edge_ids"]) <= edge_ids)

    def test_projected_nodes_and_clusters_carry_multilingual_labels(self) -> None:
        payload = self.load_projection()
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        dossier = nodes["atlas-dossier:A01"]["multilingual"]
        self.assertEqual(
            dossier["label"]["en"],
            "ToS Deep Research: A01 — Proto-Cuneiform and Accounting Ontologies",
        )
        self.assertEqual(
            dossier["label"]["ru"],
            "ToS Deep Research: A01 — Протоклинопись и учётные онтологии",
        )
        canon_cluster = next(
            cluster for cluster in payload["clusters"]
            if cluster["cluster_kind"] == "canon-candidate-status" and cluster["member_value"] == "A"
        )
        self.assertEqual(canon_cluster["multilingual"]["label"]["ru"], "Статус канона или кандидата: A")
        corpus_cluster = next(
            cluster for cluster in payload["clusters"]
            if cluster["cluster_kind"] == "corpus" and "A01" in cluster["member_value"]
        )
        self.assertEqual(
            corpus_cluster["multilingual"]["label"]["en"],
            "Corpus Or Prepared Source Document: ToS Deep Research: A01 — Proto-Cuneiform and Accounting Ontologies.docx",
        )

    def test_review_packets_are_compact_view_packets(self) -> None:
        payload = self.load_projection()
        views = {view["view_id"]: view for view in payload["views"]}
        packets = {packet["view_id"]: packet for packet in payload["review_packets"]}
        self.assertEqual(set(packets), set(views))
        canon = packets["canon-promotion"]
        self.assertEqual(canon["packet_id"], "review-packet:canon-promotion")
        self.assertIn("candidate_to_canon_pressure", canon)
        self.assertGreater(canon["candidate_to_canon_pressure"].get("pre-canon", 0), 0)
        self.assertEqual(canon["changed_subgraph"]["snapshot_mode"], "current-view-fingerprint")
        self.assertEqual(len(canon["changed_subgraph"]["current_view_fingerprint"]), 64)
        self.assertTrue(canon["recommended_human_review_route"].endswith("canon-promotion.graph.md"))
        chronology = packets["chronology"]
        self.assertLessEqual(len(chronology["cluster_summaries"]), 12)
        self.assertEqual(chronology["counts"]["weak_source_refs"], 0)


if __name__ == "__main__":
    unittest.main()
