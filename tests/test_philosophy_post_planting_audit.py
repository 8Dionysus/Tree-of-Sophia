from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from philosophy_post_planting_audit_common import (  # noqa: E402
    AUDIT_JSON_PATH,
    build_payload,
    render_markdown,
    render_payload,
)


class PhilosophyPostPlantingAuditTest(unittest.TestCase):
    def load_audit(self) -> dict[str, object]:
        return json.loads(AUDIT_JSON_PATH.read_text(encoding="utf-8"))

    def test_generated_audit_matches_builder(self) -> None:
        self.assertEqual(AUDIT_JSON_PATH.read_text(encoding="utf-8"), render_payload(build_payload()))

    def test_audit_keeps_runtime_boundary_downstream(self) -> None:
        payload = self.load_audit()
        self.assertEqual(payload["runtime_projection_boundary"]["runtime_owner"], "abyss-stack")
        self.assertEqual(payload["owner_repo"], "Tree-of-Sophia")

    def test_audit_accounts_for_supported_prepared_dossier_packages(self) -> None:
        payload = build_payload()
        self.assertEqual(payload["master_tables"]["table-i"]["row_count"], 48)
        self.assertEqual(payload["master_tables"]["table-i"]["dossier_available_count"], 48)
        self.assertEqual(payload["master_tables"]["table-ii"]["row_count"], 58)
        self.assertEqual(payload["master_tables"]["table-ii"]["dossier_available_count"], 49)
        table_ii = payload["master_tables"]["table-ii"]
        self.assertEqual(table_ii["dossier_unavailable_count"], 9)
        self.assertEqual(
            table_ii["unavailable_row_ids_by_intake_status"],
            {
                "blocked_master_identity_mismatch": ["T2-26"],
                "input_not_supplied": [
                    "T2-51",
                    "T2-52",
                    "T2-53",
                    "T2-54",
                    "T2-55",
                    "T2-56",
                    "T2-57",
                    "T2-58",
                ],
            },
        )
        self.assertNotIn("missing_row_ids", table_ii)
        markdown = render_markdown(payload)
        self.assertIn("blocked_master_identity_mismatch): `T2-26`", markdown)
        self.assertIn("input_not_supplied): `T2-51`, `T2-52`", markdown)
        self.assertEqual(payload["counts"]["prepared_dossiers"], 97)
        self.assertEqual(payload["branch_audit"]["prepared_branch_count"], 97)
        self.assertEqual(payload["graph_workbench_audit"]["proposed_node_count"], 3551)
        self.assertEqual(payload["graph_workbench_audit"]["proposed_relation_count"], 3536)
        self.assertEqual(payload["graph_workbench_audit"]["language_packet_count"], 678)
        self.assertEqual(
            payload["graph_workbench_audit"]["language_packet_count"],
            payload["graph_workbench_audit"]["text_bearing_node_count"],
        )

    def test_audit_is_ready_for_first_graph_review(self) -> None:
        payload = self.load_audit()
        self.assertEqual(payload["counts"]["errors"], 0)
        self.assertEqual(payload["review_readiness"]["status"], "ready_for_first_graph_review")
        self.assertEqual(payload["graph_projection_audit"]["views"], 11)
        self.assertEqual(payload["graph_projection_audit"]["review_packets"], 11)
        self.assertTrue(payload["graph_projection_audit"]["snapshot_ready"])

    def test_audit_builder_inventory_names_every_supported_package_surface(self) -> None:
        inventory = json.loads(
            (REPO_ROOT / "docs/validation/script_inventory.json").read_text(encoding="utf-8")
        )
        entry = next(
            row
            for row in inventory["script_surfaces"]
            if row["path"] == "scripts/build_philosophy_post_planting_audit.py"
        )
        self.assertTrue(
            {
                "ToS/philosophy/graph-workbench/proposed-nodes/table-i-prepared-dossiers.jsonl",
                "ToS/philosophy/graph-workbench/proposed-nodes/table-ii-prepared-dossiers.jsonl",
                "ToS/philosophy/graph-workbench/proposed-relations/table-i-prepared-dossiers.jsonl",
                "ToS/philosophy/graph-workbench/proposed-relations/table-ii-prepared-dossiers.jsonl",
                "ToS/philosophy/graph-workbench/language-packets/table-i-text-bearing-nodes.jsonl",
                "ToS/philosophy/graph-workbench/language-packets/table-ii-text-bearing-nodes.jsonl",
                "ToS/philosophy/graph-workbench/branch-fragments/table-i-prepared-dossier-branches.json",
                "ToS/philosophy/graph-workbench/branch-fragments/table-ii-prepared-dossier-branches.json",
            }
            <= set(entry["source_truth"])
        )
        self.assertIn("aggregate supported prepared-dossier", entry["side_effects"])

    def test_planting_interface_routes_to_executable_owners(self) -> None:
        text = (REPO_ROOT / "ToS/philosophy/graph-workbench/PLANTING_INTERFACE.md").read_text(encoding="utf-8")

        for route in (
            "scripts/plant_prepared_dossiers.py",
            "scripts/AGENTS.md",
            "docs/validation/validation_lanes.json",
        ):
            with self.subTest(route=route):
                self.assertIn(route, text)
                self.assertTrue((REPO_ROOT / route).exists())


if __name__ == "__main__":
    unittest.main()
