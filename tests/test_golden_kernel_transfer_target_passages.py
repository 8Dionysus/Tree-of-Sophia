from __future__ import annotations

import copy
import json
import sys
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import validate_golden_kernel_transfer_target_passages as validator


class GoldenKernelTransferTargetPassageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.payload = json.loads(
            (REPO_ROOT / validator.OUTPUT_PATH).read_text(encoding="utf-8")
        )
        cls.schema = json.loads(
            (REPO_ROOT / validator.SCHEMA_PATH).read_text(encoding="utf-8")
        )

    def test_release_safe_tracked_closure_passes(self) -> None:
        self.assertEqual(0, validator.main(["--repo-root", str(REPO_ROOT)]))

    def test_exact_counts_and_nonintersections_are_preserved(self) -> None:
        self.assertEqual(
            {
                "frozen_page_candidate_count": 20,
                "conservative_structural_route_count": 35,
                "layer_exact_passage_candidate_count": 35,
                "candidate_page_intersection_count": 32,
                "nonintersecting_route_count": 3,
                "accepted_target_passage_count": 0,
                "eligible_target_unit_count": 0,
                "target_gold_count": 0,
                "human_review_count": 0,
            },
            self.payload["summary"],
        )
        actual = {
            (
                candidate["frozen_page_candidate_id"],
                candidate["qualified_unit_key"],
            )
            for candidate in self.payload["passage_candidates"]
            if not candidate["candidate_page_intersects_passage"]
        }
        self.assertEqual(validator.EXPECTED_NONINTERSECTING, actual)

    def test_contract_rejects_false_acceptance_or_alignment(self) -> None:
        schema_validator = Draft202012Validator(self.schema)
        for mutation in ("accepted", "alignment", "gold"):
            with self.subTest(mutation=mutation):
                payload = copy.deepcopy(self.payload)
                if mutation == "accepted":
                    payload["passage_candidates"][0]["accepted_target_text"] = True
                elif mutation == "alignment":
                    payload["effects"][
                        "source_to_target_passage_alignment_created"
                    ] = True
                else:
                    payload["summary"]["target_gold_count"] = 1
                self.assertTrue(list(schema_validator.iter_errors(payload)))

    def test_tracked_manifest_contains_no_private_text_fields(self) -> None:
        rendered = json.dumps(self.payload, ensure_ascii=False)
        for forbidden in (
            '"automatic_candidate_text"',
            '"lines"',
            '"words"',
        ):
            self.assertNotIn(forbidden, rendered)
        for candidate in self.payload["passage_candidates"]:
            self.assertIn("/local-content/transfer-target-passages/v1/", candidate["private_content_ref"])
            self.assertFalse((REPO_ROOT / candidate["private_content_ref"]).exists())


if __name__ == "__main__":
    unittest.main()
