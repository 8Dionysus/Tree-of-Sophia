from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from philosophy_post_planting_audit_common import AUDIT_JSON_PATH, build_payload, render_payload  # noqa: E402


class PhilosophyPostPlantingAuditTest(unittest.TestCase):
    def load_audit(self) -> dict[str, object]:
        return json.loads(AUDIT_JSON_PATH.read_text(encoding="utf-8"))

    def test_generated_audit_matches_builder(self) -> None:
        self.assertEqual(AUDIT_JSON_PATH.read_text(encoding="utf-8"), render_payload(build_payload()))

    def test_audit_keeps_runtime_boundary_downstream(self) -> None:
        payload = self.load_audit()
        self.assertEqual(payload["runtime_projection_boundary"]["runtime_owner"], "abyss-stack")
        self.assertEqual(payload["owner_repo"], "Tree-of-Sophia")

    def test_audit_accounts_for_planted_table_i(self) -> None:
        payload = self.load_audit()
        self.assertEqual(payload["master_tables"]["table-i"]["row_count"], 48)
        self.assertEqual(payload["master_tables"]["table-i"]["dossier_available_count"], 30)
        self.assertEqual(payload["counts"]["prepared_dossiers"], 30)
        self.assertEqual(payload["branch_audit"]["prepared_branch_count"], 30)
        self.assertEqual(payload["graph_workbench_audit"]["proposed_node_count"], 1040)
        self.assertEqual(payload["graph_workbench_audit"]["proposed_relation_count"], 986)

    def test_audit_is_ready_for_first_graph_review(self) -> None:
        payload = self.load_audit()
        self.assertEqual(payload["counts"]["errors"], 0)
        self.assertEqual(payload["review_readiness"]["status"], "ready_for_first_graph_review")
        self.assertEqual(payload["graph_projection_audit"]["views"], 11)
        self.assertEqual(payload["graph_projection_audit"]["review_packets"], 11)
        self.assertTrue(payload["graph_projection_audit"]["snapshot_ready"])


if __name__ == "__main__":
    unittest.main()
