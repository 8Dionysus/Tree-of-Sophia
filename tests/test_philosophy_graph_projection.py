from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from philosophy_graph_projection_common import (  # noqa: E402
    GRAPH_PROJECTION_PATH,
    _edge_semantic_layers,
    _node_matches_filters,
    _stable_digest,
    _view_fingerprint_material,
    build_payload,
    render_payload,
)
from philosophy_multilingual_common import english_label  # noqa: E402


class PhilosophyGraphProjectionTest(unittest.TestCase):
    def test_validator_rejects_noncanonical_encoding_with_one_rebuild(self) -> None:
        import validate_philosophy_graph_projection as validator

        # The semantic schema has its own coverage; exercise byte parity
        # without rebuilding the real graph for each equivalent encoding.
        payload = {"counts": {}}
        for text in (' {"counts": {}}\n', '{"counts": {}, "counts": {}}\n'):
            with (
                self.subTest(text=text),
                patch.object(validator, "build_payload", return_value=payload) as build,
                patch.object(validator, "validate_payload_schema"),
                patch.object(validator, "GRAPH_PROJECTION_PATH") as source,
            ):
                source.read_text.return_value = text
                self.assertEqual(json.loads(text), payload)
                with self.assertRaisesRegex(SystemExit, "canonical rebuild"):
                    validator.main()
                build.assert_called_once_with()

    def load_projection(self) -> dict[str, object]:
        return json.loads(GRAPH_PROJECTION_PATH.read_text(encoding="utf-8"))

    def test_generated_projection_matches_builder(self) -> None:
        expected = render_payload(build_payload())
        current = GRAPH_PROJECTION_PATH.read_text(encoding="utf-8")
        self.assertEqual(current, expected)

    def test_projection_has_expected_counts_and_boundary(self) -> None:
        payload = self.load_projection()
        self.assertEqual(payload["schema_version"], "tos_philosophy_graph_projection_v2")
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

    def test_views_reference_materialized_graph_with_source_refs(self) -> None:
        payload = self.load_projection()
        views = {view["view_id"]: view for view in payload["views"]}
        chronology = views["chronology"]
        self.assertEqual(chronology["layout_hint"], "timeline-lanes")
        self.assertGreater(len(chronology["node_ids"]), 0)
        self.assertGreater(len(chronology["edge_ids"]), 0)
        self.assertIn("ToS/philosophy/atlas/master-tables/table-i/rows.jsonl", chronology["source_refs"])
        source_evidence = views["source-evidence"]
        self.assertIn("evidence-relation", source_evidence["graph_layers"])
        self.assertGreater(len(source_evidence["source_refs"]), 0)
        self.assertIn("research packets remain preparation", source_evidence["source_posture"])
        self.assertIn("source-witness", source_evidence["collapse_rule"]["default_cluster_kinds"])

        node_ids = {node["node_id"] for node in payload["nodes"]}
        edge_ids = {edge["edge_id"] for edge in payload["edges"]}
        for view in views.values():
            self.assertTrue(set(view["node_ids"]) <= node_ids)
            self.assertTrue(set(view["edge_ids"]) <= edge_ids)

    def test_view_fingerprints_reproduce_from_exported_membership(self) -> None:
        payload = self.load_projection()
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        edges = {edge["edge_id"]: edge for edge in payload["edges"]}
        fingerprints = {
            row["view_id"]: row["fingerprint"]
            for row in payload["snapshot_review"]["current_snapshot"]["view_fingerprints"]
        }
        packets = {row["view_id"]: row for row in payload["review_packets"]}

        for view in payload["views"]:
            view_id = view["view_id"]
            clusters = [
                cluster for cluster in payload["clusters"] if view_id in cluster["view_ids"]
            ]
            material = _view_fingerprint_material(
                view_id=view_id,
                view_nodes=[nodes[node_id] for node_id in view["node_ids"]],
                view_edges=[edges[edge_id] for edge_id in view["edge_ids"]],
                view_clusters=clusters,
                graph_layers=view["graph_layers"],
                source_refs=view["source_refs"],
            )
            expected = _stable_digest(material)
            self.assertEqual(fingerprints[view_id], expected)
            self.assertEqual(
                packets[view_id]["changed_subgraph"]["current_view_fingerprint"],
                expected,
            )
            expected_layer_counts = []
            for layer in payload["graph_layers"]:
                layer_id = layer["layer_id"]
                layer_nodes = [node for node in material["nodes"] if layer_id in node["graph_layers"]]
                layer_edges = [edge for edge in material["edges"] if layer_id in edge["graph_layers"]]
                layer_clusters = [
                    cluster for cluster in clusters if layer_id in cluster["graph_layers"]
                ]
                layer_items = [*layer_nodes, *layer_edges, *layer_clusters]
                source_refs = {
                    item["source_ref"]
                    for item in layer_items
                    if item.get("source_ref")
                }
                source_refs.update(
                    ref
                    for item in layer_items
                    for ref in item.get("source_refs", [])
                )
                expected_layer_counts.append(
                    {
                        "layer_id": layer_id,
                        "node_count": len(layer_nodes),
                        "edge_count": len(layer_edges),
                        "cluster_count": len(layer_clusters),
                        "source_ref_count": len(source_refs),
                    }
                )
            self.assertEqual(packets[view_id]["layer_counts"], expected_layer_counts)

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

    def test_graph_projection_retains_table_ii_review_gates_and_resolved_endpoints(self) -> None:
        payload = self.load_projection()
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        edges = {edge["edge_id"]: edge for edge in payload["edges"]}

        candidate = nodes["candidate-node:table-ii-t2-49-node-001"]["properties"]
        self.assertEqual(candidate["review_posture"], "manual_review_required")
        self.assertEqual(candidate["master_status"], "B")
        self.assertEqual(candidate["master_confidence"], "3")

        relation = edges["edge:candidate-relation:table-ii-t2-49-relation-001"]["properties"]
        self.assertEqual(relation["review_posture"], "manual_review_required")
        self.assertEqual(relation["master_status"], "B")
        self.assertEqual(relation["master_confidence"], "3")

        first = edges["edge:candidate-relation:table-ii-t2-01-relation-001"]
        self.assertEqual(first["from_id"], "candidate-node:table-ii-t2-01-node-001")
        self.assertEqual(first["to_id"], "candidate-node:table-ii-t2-01-node-002")
        self.assertEqual(first["properties"]["endpoint_resolution"], "matched_nodes")

        self.assertEqual(nodes["atlas-row:T2-26"]["properties"]["dossier_intake_status"], "admitted")
        self.assertEqual(
            nodes["atlas-row:T2-51"]["properties"]["dossier_intake_status"],
            "admitted",
        )
        self.assertEqual(nodes["atlas-row:T3-45"]["properties"]["dossier_intake_status"], "admitted")
        self.assertEqual(nodes["atlas-row:T3-46"]["properties"]["dossier_intake_status"], "admitted")

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

    def test_source_ref_clusters_only_include_edges_with_same_source_ref(self) -> None:
        # Builder parity is checked separately; inspect that same committed
        # projection here, as the other graph-invariant tests do.
        payload = self.load_projection()
        edges = {edge["edge_id"]: edge for edge in payload["edges"]}
        source_ref_clusters = [
            cluster
            for cluster in payload["clusters"]
            if cluster["cluster_kind"] == "source-witness" and cluster["member_key"] == "source_ref"
        ]

        self.assertGreater(len(source_ref_clusters), 0)
        for cluster in source_ref_clusters:
            member_value = cluster["member_value"]
            for edge_id in cluster["member_edge_ids"]:
                edge = edges[edge_id]
                edge_refs = {edge["source_ref"], *edge.get("source_refs", [])}
                self.assertIn(member_value, edge_refs)

    def test_atlas_key_filters_narrow_broad_node_types(self) -> None:
        filters = {
            "node_types": ["atlas-node-type", "atlas-relation-kind"],
            "node_type_keys": ["concept"],
            "relation_kind_keys": ["influences"],
        }

        self.assertTrue(
            _node_matches_filters(
                {"node_type": "atlas-node-type", "label": "concept", "properties": {}},
                filters,
            )
        )
        self.assertFalse(
            _node_matches_filters(
                {"node_type": "atlas-node-type", "label": "medium", "properties": {}},
                filters,
            )
        )
        self.assertTrue(
            _node_matches_filters(
                {"node_type": "atlas-relation-kind", "label": "influences", "properties": {}},
                filters,
            )
        )
        self.assertFalse(
            _node_matches_filters(
                {"node_type": "atlas-relation-kind", "label": "preserved_in", "properties": {}},
                filters,
            )
        )

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

    def test_table_ii_prepared_dossiers_keep_reviewed_titles_in_graph_projection(self) -> None:
        payload = self.load_projection()
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        expected_en = {
            "T2-09": "Early and Classical Kalam",
            "T2-10": "Falsafa and the Avicennian Synthesis",
            "T2-16": "The Post-Mongol Persianate Scholastic Corridor",
            "T2-41": "Sanskrit Cosmopolitan Literature in Southeast Asia",
        }
        for dossier_id, title in expected_en.items():
            multilingual = nodes[f"atlas-dossier:{dossier_id}"]["multilingual"]
            self.assertEqual(multilingual["label"]["en"], f"ToS Deep Research: {dossier_id} — {title}")
            self.assertNotRegex(multilingual["label"]["en"], r"[А-Яа-яЁё]")
            self.assertEqual(multilingual["translation_status"]["en"], "reviewed")

    def test_draft_english_labels_preserve_russian_word_boundaries(self) -> None:
        self.assertEqual(english_label("ритуал"), ("ritual", "draft"))
        self.assertEqual(english_label("ритуальный"), ("ritual", "draft"))
        self.assertEqual(english_label("нормативно-ритуальный текст"), ("normative-ritual text", "draft"))
        self.assertEqual(
            english_label("Хеттское государственно-ритуальное письмо"),
            ("Hittite state-ritual writing", "draft"),
        )

    def test_table_ii_dossier_labels_use_reviewed_english_ledger_entries(self) -> None:
        self.assertEqual(
            english_label(
                "ToS Deep Research: T2-01 — Сирийские переводческие и богословско-философские школы"
            ),
            (
                "ToS Deep Research: T2-01 — Syriac Translation and Theological-Philosophical Schools",
                "reviewed",
            ),
        )

    def test_table_ii_mentions_do_not_become_dossier_titles(self) -> None:
        candidate = "Каббалистическая смежность без поглощения T2-20"
        self.assertEqual(english_label(candidate), (candidate, "draft"))
        linked = "T2-21 → logica vetus / XII-century schools"
        self.assertEqual(english_label(linked), (linked, "source"))
        self.assertEqual(
            english_label("T2-20 — Каббала как метафизико-мистический письменный узел"),
            ("T2-20 — Kabbalah as a Metaphysical-Mystical Written Node", "reviewed"),
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

    def test_low_confidence_russian_feminine_marks_evidence_relation(self) -> None:
        layers = _edge_semantic_layers(
            {
                "edge_id": "edge:test",
                "predicate_id": "influences",
                "properties": {"confidence": "низкая"},
            }
        )

        self.assertIn("evidence-relation", layers)

    def test_view_fingerprint_material_changes_when_content_changes(self) -> None:
        node = {
            "node_id": "node:test",
            "label": "Original label",
            "node_type": "candidate-node",
            "graph_layers": ["candidate-relation"],
            "view_ids": ["test-view"],
            "source_ref": "ToS/test.json",
            "properties": {"canon_status": "pre-canon"},
        }
        material = _view_fingerprint_material(
            view_id="test-view",
            view_nodes=[node],
            view_edges=[],
            view_clusters=[],
            graph_layers=["candidate-relation"],
            source_refs=["ToS/test.json"],
        )
        changed_node = dict(node)
        changed_node["label"] = "Changed label"
        changed_material = _view_fingerprint_material(
            view_id="test-view",
            view_nodes=[changed_node],
            view_edges=[],
            view_clusters=[],
            graph_layers=["candidate-relation"],
            source_refs=["ToS/test.json"],
        )

        self.assertNotEqual(_stable_digest(material), _stable_digest(changed_material))


if __name__ == "__main__":
    unittest.main()
