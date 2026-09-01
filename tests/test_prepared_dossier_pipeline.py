from __future__ import annotations

import json
import re
import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
from zipfile import BadZipFile


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
ROUTES_PATH = REPO_ROOT / "ToS/philosophy/atlas/dossiers/prepared-dossier-routes.json"
INTAKE_PATH = REPO_ROOT / "ToS/research-packets/deep-research/philosophy/dossiers/table-i-docx-intake.manifest.json"
COVERAGE_PATH = REPO_ROOT / "ToS/research-packets/deep-research/philosophy/dossiers/table-i-docx-extraction-coverage.json"
TABLE_II_INTAKE_PATH = REPO_ROOT / "ToS/research-packets/deep-research/philosophy/dossiers/table-ii-docx-intake.manifest.json"
TABLE_II_COVERAGE_PATH = REPO_ROOT / "ToS/research-packets/deep-research/philosophy/dossiers/table-ii-docx-extraction-coverage.json"
LABELS_PATH = REPO_ROOT / "ToS/philosophy/atlas/multilingual/content-labels.json"
DOSSIER_INDEX_PATH = REPO_ROOT / "ToS/philosophy/atlas/dossiers/index.jsonl"
TABLE_II_NODES_PATH = REPO_ROOT / "ToS/philosophy/graph-workbench/proposed-nodes/table-ii-prepared-dossiers.jsonl"
TABLE_II_RELATIONS_PATH = REPO_ROOT / "ToS/philosophy/graph-workbench/proposed-relations/table-ii-prepared-dossiers.jsonl"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from plant_prepared_dossiers import main as planting_main  # noqa: E402
from plant_prepared_dossiers import readiness_payload  # noqa: E402
from plant_prepared_dossiers import table_readiness  # noqa: E402
from plant_prepared_dossiers import validate_local_docx_contents  # noqa: E402
import plant_table_i_prepared_dossiers as planting_pipeline  # noqa: E402
from plant_table_i_prepared_dossiers import (  # noqa: E402
    dossier_local_node_alias,
    normalize_row_to_expand,
    resolve_relation_endpoints,
    row_value,
    table_family,
)


class PreparedDossierPipelineTest(unittest.TestCase):
    def test_row_to_expand_normalizes_numeric_and_annotated_values(self) -> None:
        self.assertEqual(normalize_row_to_expand("51", "T2-51"), "T2-51")
        self.assertEqual(normalize_row_to_expand("T3-09 (порядок запуска 9)", "T3-09"), "T3-09")
        self.assertNotEqual(normalize_row_to_expand("T3-09", "T2-09"), "T2-09")

    def test_dossier_pattern_supports_table_three_ids(self) -> None:
        self.assertEqual(planting_pipeline.extract_dossier_id(Path("T3-09 dossier.docx")), "T3-09")

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
            table_i["package_ready_to_plant"],
            table_i["local_docx_ids"] == table_i["expected_dossier_ids"],
        )

    def test_selected_readiness_still_reports_aggregate_planting_gate(self) -> None:
        package_readiness = {
            "table-i": {
                "table_id": "table-i",
                "supported": True,
                "package_ready_to_plant": False,
            },
            "table-ii": {
                "table_id": "table-ii",
                "supported": True,
                "package_ready_to_plant": True,
            },
            "table-iii": {
                "table_id": "table-iii",
                "supported": True,
                "package_ready_to_plant": False,
            },
        }

        with patch(
            "plant_prepared_dossiers.table_readiness",
            side_effect=lambda table_id: package_readiness[table_id],
        ) as readiness:
            payload = readiness_payload("table-ii")

        self.assertEqual(payload["selected_table_id"], "table-ii")
        self.assertFalse(payload["ready_to_plant"])
        self.assertEqual(
            payload["required_supported_package_readiness"],
            {"table-i": False, "table-ii": True, "table-iii": False},
        )
        self.assertEqual(payload["tables"], {"table-ii": package_readiness["table-ii"]})
        self.assertEqual(readiness.call_count, 3)

    def test_readiness_rejects_duplicate_local_dossier_ids(self) -> None:
        expected = table_readiness("table-i")["expected_dossier_ids"]
        with patch(
            "plant_prepared_dossiers.discover_local_docx_ids",
            return_value={"1.1": [*expected, "A01"]},
        ):
            table_i = table_readiness("table-i")

        self.assertFalse(table_i["local_docx_ids_unique"])
        self.assertEqual(table_i["duplicate_local_docx_ids"], ["A01"])
        self.assertEqual(len(table_i["local_docx_ids"]), len(expected) + 1)
        self.assertEqual(table_i["matched_local_docx_ids"], expected)
        self.assertEqual(table_i["missing_expected_docx_ids"], [])
        self.assertEqual(table_i["extra_local_docx_ids"], [])
        self.assertFalse(table_i["package_ready_to_plant"])

    def test_readiness_rejects_missing_or_duplicate_expected_master_rows(self) -> None:
        rows = [
            json.loads(line)
            for line in (
                REPO_ROOT / "ToS/philosophy/atlas/master-tables/table-i/rows.jsonl"
            ).read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        expected = table_readiness("table-i")["expected_dossier_ids"]
        cases = (
            ("missing", rows[1:], ["A01"], []),
            ("duplicate", [*rows, rows[0]], [], ["A01"]),
        )

        for name, master_rows, missing, duplicates in cases:
            with self.subTest(name=name):
                with (
                    patch("plant_prepared_dossiers.load_jsonl", return_value=master_rows),
                    patch(
                        "plant_prepared_dossiers.discover_local_docx_ids",
                        return_value={"1.1": expected},
                    ),
                ):
                    table_i = table_readiness("table-i")

                self.assertFalse(table_i["master_expected_ids_unique"])
                self.assertEqual(table_i["missing_expected_master_ids"], missing)
                self.assertEqual(table_i["duplicate_expected_master_ids"], duplicates)
                self.assertFalse(table_i["package_ready_to_plant"])

    def test_readiness_rejects_missing_or_duplicate_declared_missing_master_rows(self) -> None:
        rows = [
            json.loads(line)
            for line in (
                REPO_ROOT / "ToS/philosophy/atlas/master-tables/table-ii/rows.jsonl"
            ).read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        expected_docx = table_readiness("table-ii")["expected_dossier_ids"]
        missing_row = next(row for row in rows if row["row_id"] == "T2-51")
        cases = (
            ("missing", [row for row in rows if row["row_id"] != "T2-51"], ["T2-51"], []),
            ("duplicate", [*rows, missing_row], [], ["T2-51"]),
        )

        for name, master_rows, missing, duplicates in cases:
            with self.subTest(name=name):
                with (
                    patch("plant_prepared_dossiers.load_jsonl", return_value=master_rows),
                    patch(
                        "plant_prepared_dossiers.discover_local_docx_ids",
                        return_value={"2.1": expected_docx},
                    ),
                ):
                    table_ii = table_readiness("table-ii")

                self.assertFalse(table_ii["master_expected_ids_unique"])
                self.assertEqual(table_ii["missing_expected_master_ids"], missing)
                self.assertEqual(table_ii["duplicate_expected_master_ids"], duplicates)
                self.assertFalse(table_ii["package_ready_to_plant"])

    def test_readiness_rejects_unexpected_master_rows(self) -> None:
        rows = [
            json.loads(line)
            for line in (
                REPO_ROOT / "ToS/philosophy/atlas/master-tables/table-i/rows.jsonl"
            ).read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        unexpected = {**rows[-1], "row_id": "A49"}
        expected_docx = table_readiness("table-i")["expected_dossier_ids"]
        with (
            patch("plant_prepared_dossiers.load_jsonl", return_value=[*rows, unexpected]),
            patch(
                "plant_prepared_dossiers.discover_local_docx_ids",
                return_value={"1.1": expected_docx},
            ),
        ):
            table_i = table_readiness("table-i")

        self.assertEqual(table_i["unexpected_master_ids"], ["A49"])
        self.assertFalse(table_i["master_expected_ids_unique"])
        self.assertFalse(table_i["package_ready_to_plant"])

    def test_readiness_rejects_docx_content_or_identity_validation_errors(self) -> None:
        expected_docx = table_readiness("table-ii")["expected_dossier_ids"]
        validation_error = {
            "dossier_id": "T2-10",
            "path": "2.1/T2-10_corrupt.docx",
            "error_type": "BadZipFile",
            "message": "File is not a zip file",
        }
        with (
            patch(
                "plant_prepared_dossiers.discover_local_docx_ids",
                return_value={"2.1": expected_docx},
            ),
            patch(
                "plant_prepared_dossiers.validate_local_docx_contents",
                return_value=(validation_error,),
            ),
        ):
            table_ii = table_readiness("table-ii")

        self.assertTrue(table_ii["docx_content_validation_performed"])
        self.assertFalse(table_ii["docx_contents_valid"])
        self.assertEqual(table_ii["docx_validation_errors"], [validation_error])
        self.assertFalse(table_ii["package_ready_to_plant"])

    def test_docx_content_validation_reports_parser_failures(self) -> None:
        path = planting_pipeline.DOC_ROOT / "2.1" / "ToS Deep Research_ T2-10 — corrupt.docx"
        master_row = {"row_id": "T2-10", "table_id": "table-ii"}
        failures = (
            BadZipFile("File is not a zip file"),
            ValueError("T2-10 DOCX ROW_TO_EXPAND does not match its master-table identity"),
        )

        for failure in failures:
            with self.subTest(error_type=type(failure).__name__):
                validate_local_docx_contents.cache_clear()
                with (
                    patch("plant_prepared_dossiers.discover_docx", return_value=[path]),
                    patch("plant_prepared_dossiers.load_pipeline_jsonl", return_value=[master_row]),
                    patch("plant_prepared_dossiers.parse_dossier", side_effect=failure),
                ):
                    errors = validate_local_docx_contents("table-ii")

                self.assertEqual(len(errors), 1)
                self.assertEqual(errors[0]["dossier_id"], "T2-10")
                self.assertEqual(
                    errors[0]["path"],
                    "2.1/ToS Deep Research_ T2-10 — corrupt.docx",
                )
                self.assertEqual(errors[0]["error_type"], type(failure).__name__)
                self.assertIn(str(failure), errors[0]["message"])
        validate_local_docx_contents.cache_clear()

    def test_docx_content_validation_preflights_intake_package_metadata(self) -> None:
        path = planting_pipeline.DOC_ROOT / "2.1" / "ToS Deep Research_ T2-10 — malformed-metadata.docx"
        master_row = {"row_id": "T2-10", "table_id": "table-ii"}
        parsed = SimpleNamespace(dossier_id="T2-10")

        validate_local_docx_contents.cache_clear()
        with (
            patch("plant_prepared_dossiers.discover_docx", return_value=[path]),
            patch("plant_prepared_dossiers.load_pipeline_jsonl", return_value=[master_row]),
            patch("plant_prepared_dossiers.parse_dossier", return_value=parsed),
            patch(
                "plant_prepared_dossiers.docx_package_metadata",
                create=True,
                side_effect=ValueError("malformed docProps/custom.xml"),
            ),
        ):
            errors = validate_local_docx_contents("table-ii")

        self.assertEqual(len(errors), 1)
        self.assertEqual(errors[0]["dossier_id"], "T2-10")
        self.assertEqual(errors[0]["error_type"], "ValueError")
        self.assertIn("malformed docProps/custom.xml", errors[0]["message"])
        validate_local_docx_contents.cache_clear()

    def test_compatibility_entrypoint_rejects_failed_aggregate_readiness_before_reads_or_writes(self) -> None:
        failed_readiness = {
            "ready_to_plant": False,
            "required_supported_package_readiness": {"table-i": True, "table-ii": False},
        }
        with (
            patch("plant_prepared_dossiers.readiness_payload", return_value=failed_readiness),
            patch.object(
                planting_pipeline,
                "discover_docx",
                side_effect=AssertionError("DOCX discovery must follow aggregate readiness"),
            ),
            patch.object(planting_pipeline, "write_intake_and_coverage_surfaces") as write_package,
            patch.object(planting_pipeline, "update_atlas") as update_atlas,
        ):
            with self.assertRaisesRegex(SystemExit, "not ready.*table-ii"):
                planting_pipeline.main()

        write_package.assert_not_called()
        update_atlas.assert_not_called()

    def test_table_i_fallback_rejects_a_different_dossier_id_in_the_title(self) -> None:
        master_row = next(
            row
            for row in planting_pipeline.load_jsonl(
                REPO_ROOT / "ToS/philosophy/atlas/master-tables/table-i/rows.jsonl"
            )
            if row["row_id"] == "A03"
        )
        document = SimpleNamespace(
            paragraphs=[SimpleNamespace(text="ToS Deep Research: A05 — swapped dossier")],
            tables=[],
        )

        with patch.object(planting_pipeline, "load_docx_document", return_value=document):
            with self.assertRaisesRegex(ValueError, "title.*master-table identity"):
                planting_pipeline.parse_dossier(Path("A03.docx"), master_row, "table-i")

    def test_table_i_idless_title_requires_reviewed_route_identity(self) -> None:
        master_row = next(
            row
            for row in planting_pipeline.load_jsonl(
                REPO_ROOT / "ToS/philosophy/atlas/master-tables/table-i/rows.jsonl"
            )
            if row["row_id"] == "A23"
        )
        document = SimpleNamespace(
            paragraphs=[SimpleNamespace(text="Unreviewed replacement dossier without an id")],
            tables=[],
        )

        with patch.object(planting_pipeline, "load_docx_document", return_value=document):
            with self.assertRaisesRegex(ValueError, "reviewed Table I route"):
                planting_pipeline.parse_dossier(Path("A23.docx"), master_row, "table-i")

    def test_table_three_rejects_empty_accepted_input_title_override(self) -> None:
        master_row = next(
            row
            for row in planting_pipeline.load_jsonl(
                REPO_ROOT / "ToS/philosophy/atlas/master-tables/table-iii/rows.jsonl"
            )
            if row["row_id"] == "T3-65"
        )
        route = dict(planting_pipeline.PACKAGE_ROUTES["table-iii"]["T3-65"])
        route["accepted_input_title"] = ""
        document = SimpleNamespace(
            paragraphs=[SimpleNamespace(text="Unrelated dossier without identity metadata")],
            tables=[],
        )

        with (
            patch.dict(planting_pipeline.PACKAGE_ROUTES["table-iii"], {"T3-65": route}),
            patch.object(planting_pipeline, "load_docx_document", return_value=document),
        ):
            with self.assertRaisesRegex(ValueError, "accepted_input_title must be non-empty"):
                planting_pipeline.parse_dossier(Path("T3-65.docx"), master_row, "table-iii")

    def test_readiness_exposes_complete_table_ii_and_table_iii_packages(self) -> None:
        payload = readiness_payload()
        table_ii = payload["tables"]["table-ii"]
        self.assertTrue(table_ii["supported"])
        self.assertEqual(table_ii["row_count"], 58)
        self.assertEqual(table_ii["docx_sections"], ["2"])
        self.assertEqual(table_ii["master_alignment"], "58/58")
        self.assertEqual(table_ii["input_admission"], "58/58")
        self.assertEqual(table_ii["blocked_dossier_ids"], [])
        self.assertEqual(table_ii["missing_master_dossier_ids"], [])
        table_iii = payload["tables"]["table-iii"]
        self.assertTrue(table_iii["supported"])
        self.assertEqual(table_iii["master_alignment"], "84/84")
        self.assertEqual(table_iii["input_admission"], "84/84")
        self.assertEqual(table_iii["blocked_dossier_ids"], [])
        self.assertEqual(table_iii["missing_master_dossier_ids"], [])

    def test_readiness_rejects_drifted_identity_on_declared_missing_master_rows(self) -> None:
        rows = [
            json.loads(line)
            for line in (
                REPO_ROOT / "ToS/philosophy/atlas/master-tables/table-ii/rows.jsonl"
            ).read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        expected_docx = table_readiness("table-ii")["expected_dossier_ids"]
        cases = (
            ("table_id", {"table_id": "table-i"}, ["table_id_mismatch"]),
            ("normalized_row_id", {"normalized": {**next(row for row in rows if row["row_id"] == "T2-51")["normalized"], "row_id": "T2-99"}}, ["normalized_row_id_mismatch"]),
        )

        for name, replacement, expected_errors in cases:
            with self.subTest(name=name):
                drifted_rows = [
                    {**row, **replacement} if row["row_id"] == "T2-51" else row
                    for row in rows
                ]
                with (
                    patch("plant_prepared_dossiers.load_jsonl", return_value=drifted_rows),
                    patch(
                        "plant_prepared_dossiers.discover_local_docx_ids",
                        return_value={"2.1": expected_docx},
                    ),
                    patch("plant_prepared_dossiers.validate_local_docx_contents", return_value=()),
                ):
                    table_ii = table_readiness("table-ii")

                self.assertFalse(table_ii["master_expected_rows_valid"])
                invalid = next(
                    item
                    for item in table_ii["invalid_expected_master_rows"]
                    if item["dossier_id"] == "T2-51"
                )
                self.assertEqual(invalid["errors"], expected_errors)
                self.assertFalse(table_ii["package_ready_to_plant"])

    def test_planting_cli_is_explicitly_aggregate_only(self) -> None:
        payload = readiness_payload()
        self.assertEqual(
            payload["tables"]["table-i"]["planting_entrypoint"],
            "scripts/plant_prepared_dossiers.py --plant",
        )
        self.assertEqual(
            payload["tables"]["table-ii"]["planting_entrypoint"],
            "scripts/plant_prepared_dossiers.py --plant",
        )
        ready = {
            "ready_to_plant": True,
            "required_supported_package_readiness": {"table-i": True, "table-ii": True},
        }
        with (
            patch("plant_prepared_dossiers.readiness_payload", return_value=ready),
            patch("plant_prepared_dossiers.plant_supported_packages", return_value=0) as plant,
        ):
            with self.assertRaisesRegex(SystemExit, "aggregate-only"):
                planting_main(["--table", "table-ii", "--plant"])
            plant.assert_not_called()
            self.assertEqual(planting_main(["--plant"]), 0)
            plant.assert_called_once_with()

    def test_planting_cli_rejects_failed_aggregate_readiness_before_writes(self) -> None:
        failed_readiness = {
            "ready_to_plant": False,
            "required_supported_package_readiness": {"table-i": True, "table-ii": False},
        }
        with (
            patch("plant_prepared_dossiers.readiness_payload", return_value=failed_readiness),
            patch("plant_prepared_dossiers.plant_supported_packages", return_value=0) as plant,
        ):
            with self.assertRaisesRegex(SystemExit, "not ready.*table-ii"):
                planting_main(["--plant"])
            plant.assert_not_called()

    def test_route_map_keeps_frontier_and_manual_review_explicit(self) -> None:
        payload = json.loads(ROUTES_PATH.read_text(encoding="utf-8"))
        package = payload["packages"]["table-i"]
        self.assertEqual(payload["schema_version"], "tos_prepared_dossier_routes_v4")
        self.assertEqual(package["docx_sections"], ["1.1", "1.2", "1.3"])
        routes = {row["dossier_id"]: {**package["route_defaults"], **row} for row in package["routes"]}
        self.assertEqual(len(routes), 48)
        for dossier_id in ("A44", "A45", "A46", "A47", "A48"):
            self.assertEqual(routes[dossier_id]["review_posture"], "manual_review_required")
        self.assertEqual(routes["A48"]["route_kind"], "cross_chronology_frontier")
        self.assertTrue(routes["A48"]["branch_path"].startswith("ToS/philosophy/frontiers/"))
        self.assertIn("rongorongo_outside_table_i_chronology", routes["A48"]["route_constraints"])

        table_ii = payload["packages"]["table-ii"]
        table_ii_routes = {row["dossier_id"]: {**table_ii["route_defaults"], **row} for row in table_ii["routes"]}
        self.assertEqual(len(table_ii_routes), 58)
        self.assertIn("T2-26", table_ii_routes)
        self.assertEqual(table_ii["missing_master_dossier_ids"], [])
        self.assertTrue(all(
            row["branch_path"].startswith("ToS/philosophy/eras/medieval-worlds/")
            for row in table_ii["routes"] if row["dossier_id"] != "T2-56"
        ))
        t2_56 = table_ii_routes["T2-56"]
        self.assertTrue(t2_56["branch_path"].startswith("ToS/philosophy/frontiers/"))
        self.assertIn("not_a_readable_text_corpus_claim", t2_56["route_constraints"])

        for package in payload["packages"].values():
            for route in package["routes"]:
                if "accepted_input_title" in route:
                    self.assertTrue(route["accepted_input_title"].strip())

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

    def test_table_ii_fixity_coverage_and_quarantine_are_exact(self) -> None:
        intake = json.loads(TABLE_II_INTAKE_PATH.read_text(encoding="utf-8"))
        self.assertEqual(intake["file_count"], 58)
        self.assertEqual(intake["admitted_file_count"], 58)
        self.assertEqual(intake["quarantined_file_count"], 0)
        self.assertEqual(intake["artifact_trust_posture"]["verdict"], "unknown")
        self.assertFalse(intake["artifact_trust_posture"]["registered_trust_class"])
        self.assertEqual(intake["section_counts"], {"2": 58})
        self.assertEqual(
            intake["section_fingerprints"]["2"],
            "abd44eaedd9f77ea5f15c5ef66b87440422af49a79a9139308dcc71b7f08ed5a",
        )

        coverage = json.loads(TABLE_II_COVERAGE_PATH.read_text(encoding="utf-8"))
        self.assertEqual(coverage["summary"]["table_body_row_count"], 14271)
        self.assertEqual(
            coverage["summary"]["coverage_class_counts"],
            {
                "deferred_context": 4218,
                "identity_metadata_examined": 766,
                "structured_primary_extracted": 9287,
            },
        )
        self.assertEqual(coverage["summary"]["family_row_counts"]["proposed_nodes"], 2152)
        self.assertEqual(coverage["summary"]["family_row_counts"]["proposed_relations"], 2311)
        self.assertEqual(coverage["summary"]["family_row_counts"]["risk_control_source_needs"], 791)

    def test_table_ii_control_tos_risk_header_is_structured(self) -> None:
        self.assertEqual(
            table_family(("Риск", "Почему существенен", "Контроль ToS")),
            "risk_control_source_needs",
        )

    def test_table_iii_accepted_structured_headers_preserve_payloads(self) -> None:
        source_rows = planting_pipeline.load_jsonl(
            REPO_ROOT / "ToS/philosophy/atlas/dossiers/source-anchor-backlog.jsonl"
        )
        term_rows = planting_pipeline.load_jsonl(
            REPO_ROOT / "ToS/philosophy/atlas/dossiers/term-index.jsonl"
        )

        t3_44 = [
            row
            for row in source_rows
            if row["dossier_id"] == "T3-44"
            and row["anchor_kind"] == "corpus_or_edition_anchor"
        ]
        self.assertEqual(len(t3_44), 39)
        self.assertTrue(all(row["contribution"] for row in t3_44))

        for dossier_id, table_indexes, field in (
            ("T3-17", {29, 30}, "source_access"),
            ("T3-19", {27, 28}, "reliability"),
            ("T3-15", {5}, "control"),
            ("T3-19", {6}, "control"),
        ):
            affected = [
                row
                for row in source_rows
                if row["dossier_id"] == dossier_id
                and row["source_table_index"] in table_indexes
            ]
            self.assertTrue(affected)
            self.assertTrue(all(row[field] for row in affected))

        t3_82 = [
            row
            for row in term_rows
            if row["dossier_id"] == "T3-82" and row["source_table_index"] == 25
        ]
        self.assertEqual(len(t3_82), 30)
        self.assertTrue(all(row["transliteration"] for row in t3_82))

    def test_table_ii_observed_deferred_headers_keep_known_families(self) -> None:
        self.assertEqual(
            table_family(
                (
                    "Корпус / текст",
                    "Дата / слой",
                    "Язык",
                    "Жанр",
                    "Сохранность",
                    "Почему важен для ToS",
                )
            ),
            "corpora_texts_artifacts",
        )
        self.assertEqual(
            table_family(
                (
                    "Корпус/текст/артефакт",
                    "Дата/слой",
                    "Язык",
                    "Жанр/жанры",
                    "Сохранность",
                    "Почему важен для ToS",
                )
            ),
            "corpora_texts_artifacts",
        )
        for author_header, linked_texts_header in (
            ("Фигура / тип авторства", "Связанные тексты / линии"),
            ("Фигура / тип авторства", "Связанные тексты / практики"),
            ("Фигура / тип авторства", "Связанные тексты / процессы"),
            ("Фигура / тип", "Связанные тексты / режимы"),
            ("Фигура / тип", "Связанные тексты/проекты"),
        ):
            self.assertEqual(
                table_family(
                    (
                        author_header,
                        "Период",
                        "Роль",
                        linked_texts_header,
                        "Уверенность",
                    )
                ),
                "figures_authorship",
            )

        coverage = json.loads(TABLE_II_COVERAGE_PATH.read_text(encoding="utf-8"))
        family_counts = coverage["summary"]["family_row_counts"]
        self.assertEqual(family_counts["corpora_texts_artifacts"], 780)
        self.assertEqual(family_counts["figures_authorship"], 651)
        self.assertEqual(family_counts["other_context"], 503)

    def test_table_iii_header_aliases_keep_structured_families(self) -> None:
        self.assertEqual(
            table_family(("Node ID", "Тип", "Название", "Период", "Связи", "Приоритет")),
            "proposed_nodes",
        )
        self.assertEqual(
            table_family(("Исходный узел", "Отношение", "Целевой узел", "Комментарий", "Ув.")),
            "proposed_relations",
        )
        self.assertEqual(
            table_family(("Источник / previous node", "Что передано", "Канал", "Уверенность", "Примечание")),
            "incoming_transmissions",
        )
        self.assertEqual(
            table_family(("Следующий узел / эпоха", "Что передаётся", "Канал", "Уверенность", "Проверить дальше")),
            "outgoing_transmissions",
        )

    def test_row_value_reads_table_iii_aliases(self) -> None:
        row = {
            "Node ID": "T3-84-N01",
            "Тип": "concept",
            "Исходный узел": "T3-84-N01",
            "Отношение": "uses_language",
            "Целевой узел": "T3-84-N05",
            "Ув.": "high",
        }
        self.assertEqual(row_value(row, "Node ID"), "T3-84-N01")
        self.assertEqual(row_value(row, "Тип узла", "Тип"), "concept")
        self.assertEqual(row_value(row, "Source node", "Исходный узел"), "T3-84-N01")
        self.assertEqual(row_value(row, "Relation", "Отношение"), "uses_language")
        self.assertEqual(row_value(row, "Target node", "Целевой узел"), "T3-84-N05")
        self.assertEqual(row_value(row, "Уверенность", "Увер.", "Ув."), "high")

    def test_table_ii_quarantine_emits_no_semantic_output_and_b_rows_need_review(self) -> None:
        index_rows = [json.loads(line) for line in DOSSIER_INDEX_PATH.read_text(encoding="utf-8").splitlines() if line]
        node_rows = [json.loads(line) for line in TABLE_II_NODES_PATH.read_text(encoding="utf-8").splitlines() if line]
        relation_rows = [json.loads(line) for line in TABLE_II_RELATIONS_PATH.read_text(encoding="utf-8").splitlines() if line]
        self.assertEqual(sum(row.get("table_id") == "table-ii" for row in index_rows), 58)
        self.assertTrue(any(row["dossier_id"] == "T2-26" for row in index_rows + node_rows + relation_rows))

        master_rows = [
            json.loads(line)
            for line in (REPO_ROOT / "ToS/philosophy/atlas/master-tables/table-ii/rows.jsonl").read_text(encoding="utf-8").splitlines()
            if line
        ]
        manual_ids = {
            row["row_id"]
            for row in master_rows
            if row["normalized"]["status"] in {"B", "C"} or int(row["normalized"]["confidence"]) <= 3
        }
        admitted_by_id = {row["dossier_id"]: row for row in index_rows if row.get("table_id") == "table-ii"}
        self.assertTrue((manual_ids & set(admitted_by_id)) <= {
            dossier_id for dossier_id, row in admitted_by_id.items() if row["review_posture"] == "manual_review_required"
        })

    def test_table_ii_unqualified_relation_endpoints_resolve_within_their_dossier(self) -> None:
        relation_rows = [
            json.loads(line)
            for line in TABLE_II_RELATIONS_PATH.read_text(encoding="utf-8").splitlines()
            if line
        ]
        first = next(row for row in relation_rows if row["candidate_id"] == "table-ii-t2-01-relation-001")
        self.assertEqual(first["source_endpoint_label"], "N01")
        self.assertEqual(first["target_endpoint_label"], "N02")
        self.assertEqual(first["source_candidate_id"], "table-ii-t2-01-node-001")
        self.assertEqual(first["target_candidate_id"], "table-ii-t2-01-node-002")
        self.assertEqual(first["endpoint_resolution"], "matched_nodes")
        hyphenated = next(row for row in relation_rows if row["candidate_id"] == "table-ii-t2-02-relation-001")
        self.assertEqual(hyphenated["source_candidate_id"], "table-ii-t2-02-node-001")
        self.assertEqual(hyphenated["target_candidate_id"], "table-ii-t2-02-node-002")
        self.assertEqual(hyphenated["endpoint_resolution"], "matched_nodes")
        self.assertEqual(sum(row["endpoint_resolution"] == "matched_nodes" for row in relation_rows), 2012)

    def test_dossier_local_node_alias_accepts_observed_qualified_forms(self) -> None:
        self.assertEqual(dossier_local_node_alias("T2-02", "T2-02-CLC-01"), "CLC-01")
        self.assertEqual(dossier_local_node_alias("T2-03", "T2-03-N01"), "N01")
        self.assertEqual(dossier_local_node_alias("T2-04", "T204-N01"), "N01")
        self.assertEqual(dossier_local_node_alias("T2-38", "T2-38:N01"), "N01")
        self.assertEqual(dossier_local_node_alias("T2-01", "T2-01.N01"), "N01")
        self.assertIsNone(dossier_local_node_alias("T2-02", "T2-020-N01"))

    def test_decorated_unique_local_endpoint_alias_resolves_after_exact_lookup(self) -> None:
        dossier = SimpleNamespace(
            dossier_id="T2-03",
            node_rows=[
                {
                    "candidate_id": "table-ii-t2-03-node-020",
                    "original_node_id": "T2-03-N20",
                    "label": "Michael Psellos",
                },
                {
                    "candidate_id": "table-ii-t2-03-node-021",
                    "original_node_id": "T2-03-N21",
                    "label": "Italian transmission",
                },
            ],
            relation_rows=[
                {
                    "candidate_id": "table-ii-t2-03-relation-003",
                    "source_endpoint_label": "N20 Пселл",
                    "target_endpoint_label": "N21 Итал",
                }
            ],
        )

        _, relations = resolve_relation_endpoints([dossier])

        self.assertEqual(relations[0]["source_candidate_id"], "table-ii-t2-03-node-020")
        self.assertEqual(relations[0]["target_candidate_id"], "table-ii-t2-03-node-021")
        self.assertEqual(relations[0]["endpoint_resolution"], "matched_nodes")

    def test_unique_casefolded_local_endpoint_resolves_without_hiding_ambiguity(self) -> None:
        unique = SimpleNamespace(
            dossier_id="T2-06",
            node_rows=[
                {
                    "candidate_id": "table-ii-t2-06-node-031",
                    "original_node_id": "T2-06-N31",
                    "label": "Ascetic self-formation",
                },
            ],
            relation_rows=[
                {
                    "candidate_id": "table-ii-t2-06-relation-013",
                    "source_endpoint_label": "N31",
                    "target_endpoint_label": "ascetic self-formation",
                }
            ],
        )
        _, unique_relations = resolve_relation_endpoints([unique])
        self.assertEqual(unique_relations[0]["target_candidate_id"], "table-ii-t2-06-node-031")
        self.assertEqual(unique_relations[0]["endpoint_resolution"], "matched_nodes")

        ambiguous = SimpleNamespace(
            dossier_id="T2-99",
            node_rows=[
                {"candidate_id": "candidate-a", "original_node_id": "N01", "label": "Term"},
                {"candidate_id": "candidate-b", "original_node_id": "N02", "label": "term"},
            ],
            relation_rows=[
                {
                    "candidate_id": "relation-ambiguous",
                    "source_endpoint_label": "N01",
                    "target_endpoint_label": "TERM",
                }
            ],
        )
        _, ambiguous_relations = resolve_relation_endpoints([ambiguous])
        self.assertIsNone(ambiguous_relations[0]["target_candidate_id"])
        self.assertEqual(ambiguous_relations[0]["endpoint_resolution"], "label_endpoint")

    def test_all_packages_are_validated_before_any_companion_write(self) -> None:
        first = SimpleNamespace(table_id="table-i", dossier_id="A01")
        with (
            patch.object(planting_pipeline, "load_jsonl", return_value=[{"row_id": "A01"}]),
            patch.object(
                planting_pipeline,
                "discover_docx",
                side_effect=[[Path("A01.docx")], RuntimeError("table-ii malformed")],
            ),
            patch.object(planting_pipeline, "parse_dossier", return_value=first),
            patch("plant_prepared_dossiers.require_aggregate_readiness", return_value={"ready_to_plant": True}),
            patch.object(planting_pipeline, "write_intake_and_coverage_surfaces") as write_package,
            patch.object(planting_pipeline, "update_atlas") as update_atlas,
        ):
            with self.assertRaisesRegex(RuntimeError, "table-ii malformed"):
                planting_pipeline.main()
        write_package.assert_not_called()
        update_atlas.assert_not_called()

    def test_table_ii_titles_have_reviewed_ru_en_labels_without_false_t2_26(self) -> None:
        payload = json.loads(LABELS_PATH.read_text(encoding="utf-8"))
        labels = payload["label_sets"]["dossier_titles"]
        table_ii_ids = {f"T2-{value:02d}" for value in range(1, 59)}
        self.assertEqual({key for key in labels if key.startswith("T2-")}, table_ii_ids)
        self.assertTrue(all(labels[dossier_id]["ru"] and labels[dossier_id]["en"] for dossier_id in table_ii_ids))
        self.assertFalse(any(re.search(r"[А-Яа-яЁё]", labels[dossier_id]["en"]) for dossier_id in table_ii_ids))


if __name__ == "__main__":
    unittest.main()
