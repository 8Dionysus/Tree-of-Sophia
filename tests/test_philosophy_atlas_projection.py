from __future__ import annotations

import json
import sys
import unittest
from collections import defaultdict
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from philosophy_atlas_projection_common import (  # noqa: E402
    ENDPOINT_ALIASES_REF,
    PROJECTION_PATH,
    build_payload,
    render_payload,
)


class PhilosophyAtlasProjectionTest(unittest.TestCase):
    def test_generated_projection_matches_builder(self) -> None:
        expected = render_payload(build_payload())
        current = PROJECTION_PATH.read_text(encoding="utf-8")
        self.assertEqual(current, expected)

    def test_projection_has_expected_atlas_counts(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        self.assertEqual(payload["counts"]["master_tables"], 3)
        self.assertEqual(payload["counts"]["master_rows"], 190)
        self.assertEqual(payload["counts"]["dossiers"], 97)
        self.assertEqual(payload["counts"]["dossier_node_rows"], 3551)
        self.assertEqual(payload["counts"]["dossier_relation_rows"], 3536)
        self.assertEqual(payload["counts"]["candidate_nodes"], 3551)
        self.assertEqual(payload["counts"]["candidate_relations"], 3536)
        self.assertEqual(payload["counts"]["candidate_endpoint_placeholders"], 300)

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
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        edges = {
            (edge["from_id"], edge["predicate_id"], edge["to_id"])
            for edge in payload["edges"]
        }
        self.assertIn(("atlas-row:A01", "has_prepared_dossier", "atlas-dossier:A01"), edges)
        self.assertIn(("atlas-row:A11", "has_prepared_dossier", "atlas-dossier:A11"), edges)
        self.assertIn(("atlas-row:A43", "has_prepared_dossier", "atlas-dossier:A43"), edges)
        self.assertIn(("atlas-row:A48", "has_prepared_dossier", "atlas-dossier:A48"), edges)
        self.assertIn(("atlas-row:T2-01", "has_prepared_dossier", "atlas-dossier:T2-01"), edges)
        self.assertNotIn(("atlas-row:T2-26", "has_prepared_dossier", "atlas-dossier:T2-26"), edges)
        self.assertEqual(
            nodes["atlas-row:T2-26"]["properties"]["dossier_intake_status"],
            "blocked_master_identity_mismatch",
        )
        self.assertEqual(
            nodes["atlas-row:T2-51"]["properties"]["dossier_intake_status"],
            "input_not_supplied",
        )

    def test_exact_admitted_dossier_endpoints_use_existing_dossier_nodes(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        edges = {edge["edge_id"]: edge for edge in payload["edges"]}
        nodes = {node["node_id"]: node for node in payload["nodes"]}

        self.assertEqual(
            edges["edge:candidate-relation:table-ii-t2-11-relation-035"]["to_id"],
            "atlas-dossier:T2-16",
        )
        self.assertEqual(
            edges["edge:candidate-relation:table-ii-t2-31-relation-039"]["to_id"],
            "atlas-dossier:T2-35",
        )
        self.assertEqual(
            edges["edge:candidate-relation:table-i-a18-relation-030"]["to_id"],
            "atlas-dossier:A41",
        )
        self.assertEqual(
            sum(
                edge["edge_id"].startswith("edge:candidate-relation:table-ii-")
                and edge["to_id"].startswith("atlas-dossier:T2-")
                for edge in payload["edges"]
            ),
            13,
        )
        admitted_ids = {
            node_id.removeprefix("atlas-dossier:")
            for node_id in nodes
            if node_id.startswith("atlas-dossier:")
        }
        self.assertFalse(
            any(
                node["node_type"] == "candidate-endpoint" and node["label"] in admitted_ids
                for node in payload["nodes"]
            )
        )

    def test_endpoint_placeholders_preserve_all_observed_roles(self) -> None:
        payload = build_payload()
        endpoint_ids = {
            node["node_id"]
            for node in payload["nodes"]
            if node["node_type"] == "candidate-endpoint"
        }
        observed_roles: dict[str, set[str]] = defaultdict(set)
        for edge in payload["edges"]:
            if not edge["edge_id"].startswith("edge:candidate-relation:"):
                continue
            if edge["from_id"] in endpoint_ids:
                observed_roles[edge["from_id"]].add("source")
            if edge["to_id"] in endpoint_ids:
                observed_roles[edge["to_id"]].add("target")

        for node in payload["nodes"]:
            if node["node_type"] != "candidate-endpoint":
                continue
            expected_roles = sorted(observed_roles[node["node_id"]])
            self.assertEqual(node["properties"]["endpoint_roles"], expected_roles)
            self.assertEqual(
                node["properties"]["endpoint_role"],
                expected_roles[0] if len(expected_roles) == 1 else "source_and_target",
            )

        shared_id = "candidate-endpoint:T2-16:e1d923d12c45"
        shared = next(node for node in payload["nodes"] if node["node_id"] == shared_id)
        self.assertEqual(shared["properties"]["endpoint_roles"], ["source", "target"])
        self.assertEqual(shared["properties"]["endpoint_role"], "source_and_target")

    def test_reviewed_qualified_endpoints_resolve_inside_target_dossiers(self) -> None:
        payload = build_payload()
        edges = {edge["edge_id"]: edge for edge in payload["edges"]}
        expected_targets = {
            "edge:candidate-relation:table-ii-t2-03-relation-038": "candidate-node:table-ii-t2-02-node-001",
            "edge:candidate-relation:table-ii-t2-11-relation-008": "candidate-node:table-ii-t2-09-node-001",
            "edge:candidate-relation:table-ii-t2-11-relation-009": "candidate-node:table-ii-t2-10-node-010",
            "edge:candidate-relation:table-ii-t2-11-relation-017": "candidate-node:table-ii-t2-10-node-019",
            "edge:candidate-relation:table-ii-t2-11-relation-019": "candidate-node:table-ii-t2-10-node-019",
        }

        for edge_id, target_id in expected_targets.items():
            edge = edges[edge_id]
            self.assertEqual(edge["to_id"], target_id)
            self.assertEqual(
                edge["properties"]["projection_endpoint_resolution"],
                "reviewed_qualified_alias",
            )
            self.assertEqual(edge["properties"]["endpoint_alias_ref"], ENDPOINT_ALIASES_REF)

    def test_projection_exposes_pre_canon_candidate_graph_material(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        node_ids = {node["node_id"] for node in payload["nodes"]}
        edge_predicates = {edge["predicate_id"] for edge in payload["edges"]}
        self.assertIn("candidate-node:table-i-a01-node-001", node_ids)
        self.assertIn("candidate-node:table-i-a43-node-001", node_ids)
        self.assertIn("candidate-node:table-i-a48-node-001", node_ids)
        self.assertIn("candidate-node:table-ii-t2-01-node-001", node_ids)
        self.assertFalse(any(node_id.startswith("candidate-node:table-ii-t2-26-") for node_id in node_ids))
        self.assertIn("uses_script", edge_predicates)
        self.assertIn("develops_concept", edge_predicates)

    def test_projection_preserves_table_ii_manual_review_gates(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        edges = {edge["edge_id"]: edge for edge in payload["edges"]}

        dossier = nodes["atlas-dossier:T2-49"]["properties"]
        self.assertEqual(dossier["review_posture"], "manual_review_required")
        self.assertEqual(dossier["master_status"], "B")
        self.assertEqual(dossier["master_confidence"], "3")

        candidate = nodes["candidate-node:table-ii-t2-49-node-001"]["properties"]
        self.assertEqual(candidate["review_posture"], "manual_review_required")
        self.assertEqual(candidate["master_status"], "B")
        self.assertEqual(candidate["master_confidence"], "3")

        relation = edges["edge:candidate-relation:table-ii-t2-49-relation-001"]["properties"]
        self.assertEqual(relation["review_posture"], "manual_review_required")
        self.assertEqual(relation["master_status"], "B")
        self.assertEqual(relation["master_confidence"], "3")

        self.assertEqual(
            sum(
                node["node_type"] == "candidate-node"
                and node["properties"].get("table_id") == "table-ii"
                and node["properties"].get("review_posture") == "manual_review_required"
                for node in payload["nodes"]
            ),
            594,
        )
        self.assertEqual(
            sum(
                edge["edge_id"].startswith("edge:candidate-relation:table-ii-")
                and edge["properties"].get("review_posture") == "manual_review_required"
                for edge in payload["edges"]
            ),
            649,
        )

        gated_endpoints = [
            node
            for node in payload["nodes"]
            if node["node_type"] == "candidate-endpoint"
            and node["properties"].get("table_id") == "table-ii"
            and node["properties"].get("review_posture") == "manual_review_required"
        ]
        self.assertEqual(len(gated_endpoints), 112)
        self.assertTrue(
            all(
                node["properties"].get("review_reason")
                and node["properties"].get("master_status")
                and node["properties"].get("master_confidence")
                for node in gated_endpoints
            )
        )

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

    def test_table_ii_prepared_dossiers_use_reviewed_context_titles(self) -> None:
        payload = json.loads(PROJECTION_PATH.read_text(encoding="utf-8"))
        nodes = {node["node_id"]: node for node in payload["nodes"]}
        expected = {
            "T2-09": ("Калām ранний и классический", "Early and Classical Kalam"),
            "T2-10": ("Falsafa и авиценновский синтез", "Falsafa and the Avicennian Synthesis"),
            "T2-16": (
                "Постмонгольский персидатский схоластический коридор",
                "The Post-Mongol Persianate Scholastic Corridor",
            ),
            "T2-41": (
                "Санскритская космопольная книжность ЮВА",
                "Sanskrit Cosmopolitan Literature in Southeast Asia",
            ),
        }
        for dossier_id, (ru_title, en_title) in expected.items():
            multilingual = nodes[f"atlas-dossier:{dossier_id}"]["multilingual"]
            self.assertEqual(multilingual["label"]["ru"], f"ToS Deep Research: {dossier_id} — {ru_title}")
            self.assertEqual(multilingual["label"]["en"], f"ToS Deep Research: {dossier_id} — {en_title}")
            self.assertNotRegex(multilingual["label"]["en"], r"[А-Яа-яЁё]")
            self.assertEqual(multilingual["translation_status"]["ru"], "reviewed")
            self.assertEqual(multilingual["translation_status"]["en"], "reviewed")


if __name__ == "__main__":
    unittest.main()
