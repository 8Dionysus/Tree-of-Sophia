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
import validate_witness_structure_correspondence as validator


class WitnessStructureCorrespondenceTests(unittest.TestCase):
    def test_tracked_candidate_closes_over_resource_inventories(self) -> None:
        self.assertEqual([], validator.validate_structure_correspondence(REPO_ROOT))

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
