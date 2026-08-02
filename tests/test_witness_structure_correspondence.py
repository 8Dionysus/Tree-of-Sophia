from __future__ import annotations

import copy
import json
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_DIR = REPO_ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

import build_witness_structure_correspondence as builder
import build_jenseits_numbered_unit_label_correspondence as label_builder
import build_jenseits_polilov_numbered_unit_structure as target_builder
import validate_witness_structure_correspondence as validator


class WitnessStructureCorrespondenceTests(unittest.TestCase):
    def test_tracked_candidate_closes_over_resource_inventories(self) -> None:
        self.assertEqual([], validator.validate_structure_correspondence(REPO_ROOT))

    def test_parallel_candidate_closes_over_both_pdf_inventories(self) -> None:
        self.assertEqual(
            [],
            validator.validate_parallel_structure_correspondence(REPO_ROOT),
        )

    def test_numbered_unit_page_map_closes_over_exact_scan_inventory(self) -> None:
        self.assertEqual(
            [],
            validator.validate_numbered_unit_page_map(REPO_ROOT),
        )

    def test_target_numbered_unit_map_closes_over_exact_scan_and_boundary(
        self,
    ) -> None:
        self.assertEqual(
            [],
            validator.validate_target_numbered_unit_page_map(REPO_ROOT),
        )

    def test_target_map_materializes_only_visible_target_labels(self) -> None:
        payload = json.loads(
            (REPO_ROOT / validator.TARGET_NUMBERED_UNIT_MAP_PATH).read_text(
                encoding="utf-8"
            )
        )
        anchors = [
            json.loads(line)
            for line in (
                REPO_ROOT
                / validator.TARGET_NUMBERED_UNIT_ANCHOR_RECORDS_PATH
            )
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
        ]
        units = {
            unit["unit_key"]: unit for unit in payload["unit_starts"]
        }

        self.assertEqual(298, len(units))
        self.assertEqual(298, len(anchors))
        self.assertEqual(
            ["65a", "73a"],
            payload["summary"]["supplemental_numbered_units"],
        )
        self.assertNotIn("237a", units)
        self.assertEqual(
            ["237a"],
            payload["summary"]["source_only_nonmaterialized_numbered_units"],
        )
        self.assertEqual(
            "source_visible_ocr_disambiguation",
            units["6"]["basis"],
        )
        self.assertEqual(244, units["6"]["pdf_page"])
        self.assertEqual(291, units["65a"]["pdf_page"])
        self.assertEqual(292, units["73a"]["pdf_page"])
        self.assertEqual(399, units["285"]["pdf_page"])
        asymmetry = payload["numbering_asymmetries"][0]
        self.assertFalse(asymmetry["target_numbered_unit_materialized"])
        self.assertFalse(asymmetry["exact_translation_alignment_claimed"])
        self.assertFalse(payload["source_text_included"])
        self.assertEqual({"proposed"}, {anchor["status"] for anchor in anchors})
        self.assertEqual(
            {"structural", "page_region"},
            {
                selector["type"]
                for anchor in anchors
                for selector in anchor["selectors"]
            },
        )

    def test_target_contract_rejects_materialized_source_only_237a(self) -> None:
        payload = json.loads(
            (REPO_ROOT / validator.TARGET_NUMBERED_UNIT_MAP_PATH).read_text(
                encoding="utf-8"
            )
        )
        payload["numbering_asymmetries"][0][
            "target_numbered_unit_materialized"
        ] = True
        schema_validator = validator._validator(
            validator.TARGET_NUMBERED_UNIT_SCHEMA_PATH,
            REPO_ROOT,
        )

        self.assertTrue(list(schema_validator.iter_errors(payload)))

    def test_target_bbox_order_route_keeps_missing_units_explicit(self) -> None:
        matches, skipped, raw_matches = target_builder._ordered_candidate_matches(
            ["1", "2", "3", "4"],
            [
                {"page": 10, "normalized": "1", "raw": "1."},
                {"page": 11, "normalized": "3", "raw": "3."},
                {"page": 12, "normalized": "2", "raw": "2."},
                {"page": 13, "normalized": "4", "raw": "4."},
            ],
        )

        self.assertEqual({"1": 10, "3": 11, "4": 13}, matches)
        self.assertEqual(["2"], skipped)
        self.assertEqual({"1": "1.", "3": "3.", "4": "4."}, raw_matches)

    def test_numbered_label_pairings_close_over_both_independent_maps(
        self,
    ) -> None:
        self.assertEqual(
            [],
            validator.validate_numbered_unit_label_correspondence(REPO_ROOT),
        )

    def test_numbered_label_pairings_do_not_materialize_source_only_237a(
        self,
    ) -> None:
        payload = json.loads(
            (REPO_ROOT / validator.NUMBERED_UNIT_LABEL_MAP_PATH).read_text(
                encoding="utf-8"
            )
        )

        self.assertEqual(298, len(payload["pairings"]))
        self.assertNotIn(
            "237a",
            {pairing["unit_key"] for pairing in payload["pairings"]},
        )
        self.assertEqual(["237a"], payload["summary"]["source_only_unit_keys"])
        self.assertEqual([], payload["summary"]["target_only_unit_keys"])
        self.assertFalse(payload["method"]["source_to_target_text_compared"])
        self.assertFalse(payload["method"]["translation_alignment_inferred"])
        self.assertFalse(payload["summary"]["translation_alignment_claimed"])
        self.assertEqual(
            {"proposed"},
            {pairing["status"] for pairing in payload["pairings"]},
        )

    def test_numbered_label_contract_rejects_translation_alignment_claim(
        self,
    ) -> None:
        payload = json.loads(
            (REPO_ROOT / validator.NUMBERED_UNIT_LABEL_MAP_PATH).read_text(
                encoding="utf-8"
            )
        )
        payload["pairings"][0]["translation_alignment_claimed"] = True
        schema_validator = validator._validator(
            validator.NUMBERED_UNIT_LABEL_SCHEMA_PATH,
            REPO_ROOT,
        )

        self.assertTrue(list(schema_validator.iter_errors(payload)))

    def test_numbered_label_generation_is_deterministic_from_tracked_maps(
        self,
    ) -> None:
        expected_map = (
            REPO_ROOT / label_builder.MAP_PATH
        ).read_text(encoding="utf-8")
        expected_provenance = (
            REPO_ROOT / label_builder.PROVENANCE_PATH
        ).read_text(encoding="utf-8")

        actual_map, actual_provenance = label_builder.build_outputs(
            repo_root=REPO_ROOT,
            event_at="2026-08-02T05:10:00-06:00",
        )

        self.assertEqual(expected_map, actual_map)
        self.assertEqual(expected_provenance, actual_provenance)

    def test_numbered_unit_map_materializes_proposed_addresses_not_text(
        self,
    ) -> None:
        payload = json.loads(
            (REPO_ROOT / validator.NUMBERED_UNIT_MAP_PATH).read_text(
                encoding="utf-8"
            )
        )
        anchors = [
            json.loads(line)
            for line in (
                REPO_ROOT / validator.NUMBERED_UNIT_ANCHOR_RECORDS_PATH
            )
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
        ]

        self.assertEqual(299, len(payload["unit_starts"]))
        self.assertEqual(299, len(anchors))
        self.assertEqual(
            ["65a", "73a", "237a"],
            payload["summary"]["supplemental_numbered_units"],
        )
        repeated = next(
            unit
            for unit in payload["unit_starts"]
            if unit["unit_key"] == "237a"
        )
        self.assertEqual(189, repeated["pdf_page"])
        self.assertEqual(
            "source_visible_repeated_number_review",
            repeated["basis"],
        )
        self.assertFalse(payload["source_text_included"])
        self.assertEqual(
            {"structural", "page_region"},
            {
                selector["type"]
                for anchor in anchors
                for selector in anchor["selectors"]
            },
        )
        self.assertFalse(
            any(
                selector["type"] in {"text_quote", "text_position"}
                for anchor in anchors
                for selector in anchor["selectors"]
            )
        )
        self.assertEqual({"proposed"}, {anchor["status"] for anchor in anchors})

    def test_parallel_candidate_keeps_division_spans_distinct_from_exact_units(
        self,
    ) -> None:
        payload = json.loads(
            (REPO_ROOT / validator.PARALLEL_MAP_PATH).read_text(encoding="utf-8")
        )
        anchors = [
            json.loads(line)
            for line in (REPO_ROOT / validator.PARALLEL_ANCHOR_RECORDS_PATH)
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
        ]

        numbered_units = [
            number
            for division in payload["divisions"]
            if division["numbered_unit_span"] is not None
            for number in range(
                division["numbered_unit_span"]["first"],
                division["numbered_unit_span"]["last"] + 1,
            )
        ]
        self.assertEqual(list(range(1, 297)), numbered_units)
        self.assertEqual(11, len(payload["divisions"]))
        self.assertEqual(22, len(anchors))
        self.assertEqual(
            ["65a", "73a", "237a"],
            payload["summary"]["supplemental_numbered_units"],
        )
        self.assertEqual(
            0,
            payload["summary"]["exact_numbered_unit_start_pages_materialized"],
        )
        self.assertFalse(payload["source_text_included"])
        self.assertFalse(payload["summary"]["human_review_performed"])
        self.assertEqual(
            {"page_region"},
            {
                selector["type"]
                for anchor in anchors
                for selector in anchor["selectors"]
            },
        )
        self.assertEqual({"proposed"}, {anchor["status"] for anchor in anchors})

    def test_parallel_contract_rejects_translation_equivalence_claim(self) -> None:
        payload = json.loads(
            (REPO_ROOT / validator.PARALLEL_MAP_PATH).read_text(encoding="utf-8")
        )
        payload["divisions"][0]["translation_equivalence_claimed"] = True
        schema_validator = validator._validator(
            validator.PARALLEL_SCHEMA_PATH,
            REPO_ROOT,
        )

        self.assertTrue(list(schema_validator.iter_errors(payload)))

    def test_every_correspondence_has_three_stable_proposed_addresses(self) -> None:
        anchor_set = json.loads(
            (REPO_ROOT / validator.ANCHOR_SET_PATH).read_text(encoding="utf-8")
        )
        anchors = [
            json.loads(line)
            for line in (REPO_ROOT / validator.ANCHOR_RECORDS_PATH)
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
        ]

        self.assertEqual(82, len(anchor_set["bindings"]))
        self.assertEqual(246, len(anchors))
        self.assertEqual(246, len({anchor["anchor_id"] for anchor in anchors}))
        self.assertEqual({"proposed"}, {anchor["status"] for anchor in anchors})
        self.assertEqual(
            {"structural", "container_member", "page_region"},
            {
                selector["type"]
                for anchor in anchors
                for selector in anchor["selectors"]
            },
        )
        self.assertFalse(anchor_set["source_text_included"])
        self.assertFalse(
            any(
                selector["type"] in {"text_quote", "text_position"}
                for anchor in anchors
                for selector in anchor["selectors"]
            )
        )

    def test_anchor_generation_is_deterministic_from_the_tracked_map(self) -> None:
        map_payload = json.loads(
            (REPO_ROOT / validator.MAP_PATH).read_text(encoding="utf-8")
        )
        expected_set = json.loads(
            (REPO_ROOT / validator.ANCHOR_SET_PATH).read_text(encoding="utf-8")
        )
        expected_records = [
            json.loads(line)
            for line in (REPO_ROOT / validator.ANCHOR_RECORDS_PATH)
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
        ]

        actual_set, actual_records = builder.build_structure_anchors(map_payload)

        self.assertEqual(expected_set, actual_set)
        self.assertEqual(expected_records, actual_records)

    def test_unique_normalized_heading_selects_its_page(self) -> None:
        result = builder._choose_target_page(
            heading_tokens=["named", "division"],
            source_tokens=["named", "division", "source", "context"],
            target_tokens={
                10: ["unrelated", "page"],
                11: ["named", "division", "source", "context"],
                12: ["following", "page"],
            },
            first_page=10,
            last_page=12,
        )

        self.assertEqual(11, result["page"])
        self.assertEqual("normalized_heading_unique", result["mode"])
        self.assertEqual(1, result["normalized_heading_occurrence_count"])

    def test_repeated_heading_is_disambiguated_by_context(self) -> None:
        result = builder._choose_target_page(
            heading_tokens=["repeated", "heading"],
            source_tokens=[
                "repeated",
                "heading",
                "distinctive",
                "matching",
                "context",
            ],
            target_tokens={
                20: ["repeated", "heading", "other", "material"],
                21: ["other", "continuation"],
                30: [
                    "repeated",
                    "heading",
                    "distinctive",
                    "matching",
                    "context",
                ],
                31: ["matching", "continuation"],
            },
            first_page=20,
            last_page=31,
        )

        self.assertEqual(30, result["page"])
        self.assertEqual(
            "normalized_heading_context_disambiguated",
            result["mode"],
        )
        self.assertEqual(2, result["normalized_heading_occurrence_count"])
        self.assertGreater(result["score_margin"], 0)

    def test_selection_policy_excludes_non_primary_nested_divisions(self) -> None:
        primary = {
            "resource_kind": "tei_division",
            "structural_role": "division",
            "locator": {
                "tei_path": "TEI/text[1]/body[1]/div[3]",
                "tei_depth": 1,
            },
        }
        nested = copy.deepcopy(primary)
        nested["locator"] = {
            "tei_path": "TEI/text[1]/body[1]/div[3]/div[1]",
            "tei_depth": 2,
        }
        part_one_chapter = copy.deepcopy(nested)
        part_one_chapter["locator"]["tei_path"] = (
            "TEI/text[1]/body[1]/div[2]/div[1]"
        )

        self.assertTrue(
            builder._selected_division(
                part_label="III",
                resource=primary,
                heading_tokens=["named"],
            )
        )
        self.assertFalse(
            builder._selected_division(
                part_label="III",
                resource=nested,
                heading_tokens=["named"],
            )
        )
        self.assertTrue(
            builder._selected_division(
                part_label="I",
                resource=part_one_chapter,
                heading_tokens=["named"],
            )
        )


if __name__ == "__main__":
    unittest.main()
