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
        self.assertEqual(payload["counts"]["dossiers"], 30)
        self.assertEqual(payload["counts"]["dossier_node_rows"], 1040)
        self.assertEqual(payload["counts"]["dossier_relation_rows"], 986)
        self.assertEqual(payload["counts"]["candidate_nodes"], 1040)
        self.assertEqual(payload["counts"]["candidate_relations"], 986)

    def test_projection_keeps_runtime_owner_downstream(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        self.assertEqual(payload["runtime_projection_boundary"]["runtime_owner"], "abyss-stack")
        self.assertEqual(payload["source_atlas_ref"], "ToS/philosophy/atlas/atlas.manifest.json")
        self.assertEqual(
            payload["content_language_contract"]["source_ref"],
            "ToS/philosophy/atlas/multilingual/content-labels.json",
        )
        self.assertEqual(payload["content_language_contract"]["display_languages"], ["original", "ru", "en"])
        self.assertEqual(
            payload["content_language_contract"]["language_registry_ref"],
            "ToS/philosophy/atlas/multilingual/language-registry.json",
        )
        self.assertEqual(
            payload["content_language_contract"]["text_bearing_nodes_contract_ref"],
            "ToS/philosophy/atlas/multilingual/text-bearing-nodes.contract.json",
        )

    def test_projection_links_rows_to_available_dossiers(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        edges = {
            (edge["from_id"], edge["predicate_id"], edge["to_id"])
            for edge in payload["edges"]
        }
        self.assertIn(("atlas-row:A01", "has_prepared_dossier", "atlas-dossier:A01"), edges)
        self.assertIn(("atlas-row:A11", "has_prepared_dossier", "atlas-dossier:A11"), edges)
        self.assertIn(("atlas-row:A43", "has_prepared_dossier", "atlas-dossier:A43"), edges)

    def test_projection_exposes_pre_canon_candidate_graph_material(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        node_ids = {node["node_id"] for node in payload["nodes"]}
        edge_predicates = {edge["predicate_id"] for edge in payload["edges"]}
        self.assertIn("candidate-node:table-i-a01-node-001", node_ids)
        self.assertIn("candidate-node:table-i-a43-node-001", node_ids)
        self.assertIn("uses_script", edge_predicates)
        self.assertIn("develops_concept", edge_predicates)

    def test_projection_exposes_graph_view_routes(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        node_ids = {node["node_id"] for node in payload["nodes"]}
        self.assertIn("graph-view:chronology", node_ids)
        self.assertIn("graph-view:transmission", node_ids)

    def test_projection_exposes_source_owned_multilingual_labels(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        philosophy = nodes["philosophy"]["multilingual"]
        self.assertEqual(philosophy["label"]["ru"], "Философия")
        self.assertEqual(philosophy["label"]["en"], "Philosophy")
        self.assertEqual(philosophy["translation_status"]["original"], "not_applicable")
        dossier = nodes["atlas-dossier:A01"]["multilingual"]
        self.assertEqual(
            dossier["label"]["ru"],
            "ToS Deep Research: A01 — Протоклинопись и учётные онтологии",
        )
        self.assertEqual(
            dossier["label"]["en"],
            "ToS Deep Research: A01 — Proto-Cuneiform and Accounting Ontologies",
        )
        self.assertEqual(dossier["translation_status"]["en"], "reviewed")
        concept = nodes["atlas-node-type:concept"]["multilingual"]
        self.assertEqual(concept["label"]["ru"], "концепт")
        self.assertEqual(concept["label"]["en"], "concept")


if __name__ == "__main__":
    unittest.main()
