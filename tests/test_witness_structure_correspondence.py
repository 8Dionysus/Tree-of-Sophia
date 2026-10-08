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



if __name__ == "__main__":
    unittest.main()
