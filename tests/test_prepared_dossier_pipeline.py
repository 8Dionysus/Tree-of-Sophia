from __future__ import annotations

import json
import re
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
ROUTES_PATH = REPO_ROOT / "ToS/philosophy/atlas/dossiers/prepared-dossier-routes.json"
INTAKE_PATH = REPO_ROOT / "ToS/research-packets/deep-research/philosophy/dossiers/table-i-docx-intake.manifest.json"
COVERAGE_PATH = REPO_ROOT / "ToS/research-packets/deep-research/philosophy/dossiers/table-i-docx-extraction-coverage.json"
LABELS_PATH = REPO_ROOT / "ToS/philosophy/atlas/multilingual/content-labels.json"
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
        self.assertEqual(len(table_i["expected_dossier_ids"]), 48)
        self.assertEqual(table_i["docx_sections"], ["1.1", "1.2", "1.3"])
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

    def test_route_map_keeps_frontier_and_manual_review_explicit(self) -> None:
        payload = json.loads(ROUTES_PATH.read_text(encoding="utf-8"))
        package = payload["packages"]["table-i"]
        self.assertEqual(payload["schema_version"], "tos_prepared_dossier_routes_v2")
        self.assertEqual(package["docx_sections"], ["1.1", "1.2", "1.3"])
        routes = {row["dossier_id"]: {**package["route_defaults"], **row} for row in package["routes"]}
        self.assertEqual(len(routes), 48)
        for dossier_id in ("A44", "A45", "A46", "A47", "A48"):
            self.assertEqual(routes[dossier_id]["review_posture"], "manual_review_required")
        self.assertEqual(routes["A48"]["route_kind"], "cross_chronology_frontier")
        self.assertTrue(routes["A48"]["branch_path"].startswith("ToS/philosophy/frontiers/"))
        self.assertIn("rongorongo_outside_table_i_chronology", routes["A48"]["route_constraints"])

    def test_tracked_intake_manifest_preserves_fixity_and_claim_limit(self) -> None:
        payload = json.loads(INTAKE_PATH.read_text(encoding="utf-8"))
        self.assertEqual(payload["file_count"], 48)
        self.assertEqual(payload["section_counts"], {"1.1": 17, "1.2": 13, "1.3": 18})
        self.assertEqual(
            payload["section_fingerprints"]["1.3"],
            "92e04bb3aeaa828f1b5feb5fb800915cc201365d6680be08ebd988cbab91b56b",
        )
        self.assertEqual(payload["capture_posture"]["origin_verification"], "unverified")
        self.assertIsNone(payload["capture_posture"]["author_identity"])
        self.assertIsNone(payload["capture_posture"]["session_or_export_id"])
        self.assertIn("does not prove authorship", payload["claim_limit"])

    def test_extraction_coverage_names_deferred_context_and_a28_prose_gap(self) -> None:
        payload = json.loads(COVERAGE_PATH.read_text(encoding="utf-8"))
        section = payload["sections"]["1.3"]
        self.assertEqual(section["table_body_row_count"], 3519)
        self.assertEqual(
            section["coverage_class_counts"],
            {
                "deferred_context": 1034,
                "identity_metadata_examined": 120,
                "structured_primary_extracted": 2365,
            },
        )
        a28 = next(row for row in payload["dossiers"] if row["dossier_id"] == "A28")
        self.assertEqual(a28["structured_risk_row_count"], 0)
        self.assertIn("structured_risk_table_absent", a28["diagnostics"])
        diagnostic = next(row for row in payload["diagnostics"] if row["dossier_id"] == "A28")
        self.assertEqual(diagnostic["posture"], "prose_only_not_synthesized")

    def test_new_dossier_titles_have_reviewed_ru_en_labels(self) -> None:
        payload = json.loads(LABELS_PATH.read_text(encoding="utf-8"))
        labels = payload["label_sets"]["dossier_titles"]
        new_ids = {
            "A24", "A25", "A26", "A27", "A28", "A29", "A30", "A31", "A32",
            "A33", "A34", "A38", "A39", "A44", "A45", "A46", "A47", "A48",
        }
        self.assertTrue(new_ids <= set(labels))
        self.assertTrue(all(labels[dossier_id]["ru"] for dossier_id in new_ids))
        self.assertTrue(all(labels[dossier_id]["en"] for dossier_id in new_ids))
        self.assertFalse(any(re.search(r"[А-Яа-яЁё]", labels[dossier_id]["en"]) for dossier_id in new_ids))
        self.assertEqual(
            labels["A48"],
            {
                "ru": "Анды и Рапа-Нуи: кипу и ронго-ронго как сравнительные пограничные случаи",
                "en": "Andes and Rapa Nui: Khipu and Rongorongo as Comparative Frontier Cases",
            },
        )


if __name__ == "__main__":
    unittest.main()
