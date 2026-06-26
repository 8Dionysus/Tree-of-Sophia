from __future__ import annotations

import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from plant_prepared_dossiers import readiness_payload  # noqa: E402


class PreparedDossierPipelineTest(unittest.TestCase):
    def test_readiness_exposes_table_i_package(self) -> None:
        payload = readiness_payload()
        table_i = payload["tables"]["table-i"]
        self.assertTrue(table_i["supported"])
        self.assertEqual(table_i["row_count"], 48)
        self.assertEqual(table_i["route_map_ref"], "ToS/philosophy/atlas/dossiers/prepared-dossier-routes.json")
        self.assertEqual(len(table_i["expected_dossier_ids"]), 30)
        self.assertEqual(
            sorted(table_i["matched_local_docx_ids"] + table_i["missing_expected_docx_ids"]),
            table_i["expected_dossier_ids"],
        )
        self.assertEqual(
            table_i["ready_to_plant"],
            table_i["local_docx_ids"] == table_i["expected_dossier_ids"],
        )

    def test_readiness_keeps_table_ii_and_iii_unplanted_until_routes_exist(self) -> None:
        payload = readiness_payload()
        self.assertFalse(payload["tables"]["table-ii"]["supported"])
        self.assertFalse(payload["tables"]["table-iii"]["supported"])
        self.assertIn("branch route map", payload["tables"]["table-ii"]["next_route"])


if __name__ == "__main__":
    unittest.main()
