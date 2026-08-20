from __future__ import annotations

import hashlib
import importlib.util
import json
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = (
    REPO_ROOT / "scripts/record_golden_kernel_transfer_source_visible_review.py"
)
PRIVATE_SCHEMA_PATH = (
    REPO_ROOT
    / "ToS/contracts/private-transfer-source-visible-review-bundle.schema.json"
)
RECEIPT_SCHEMA_PATH = (
    REPO_ROOT / "ToS/contracts/transfer-source-visible-review-receipt.schema.json"
)
RECEIPT_PATH = (
    REPO_ROOT / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1/"
    "transfer-source-visible-review.jenseits-187.v1.json"
)


def _load_recorder():
    spec = importlib.util.spec_from_file_location(
        "tos_transfer_source_visible_review_recorder",
        SCRIPT_PATH,
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {SCRIPT_PATH}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


RECORDER = _load_recorder()


class GoldenKernelTransferSourceVisibleReviewTests(unittest.TestCase):
    def test_contract_schemas_are_valid(self) -> None:
        for path in (PRIVATE_SCHEMA_PATH, RECEIPT_SCHEMA_PATH):
            schema = json.loads(path.read_text(encoding="utf-8"))
            Draft202012Validator.check_schema(schema)

    def test_tracked_receipt_is_schema_valid_and_text_free(self) -> None:
        receipt = json.loads(RECEIPT_PATH.read_text(encoding="utf-8"))
        schema = json.loads(RECEIPT_SCHEMA_PATH.read_text(encoding="utf-8"))
        self.assertEqual(
            [],
            list(Draft202012Validator(schema).iter_errors(receipt)),
        )
        encoded = RECEIPT_PATH.read_text(encoding="utf-8")
        for forbidden in (
            "/srv/",
            "/home/",
            "automatic_candidate_text",
            "diplomatic_lines",
            '"text"',
            '"finding"',
        ):
            self.assertNotIn(forbidden, encoded)
        self.assertEqual(_sha256(SCRIPT_PATH), receipt["generator"]["sha256"])

    def test_source_aware_join_only_removes_line_end_hyphenation(self) -> None:
        lines = [
            {"text": "Behauptun-"},
            {"text": "gen wie ver-gessen"},
            {"text": "und weiter"},
        ]
        self.assertEqual(
            "Behauptungen wie ver-gessen und weiter",
            RECORDER.source_aware_text(lines),
        )

    def test_token_diff_keeps_discrepancy_aggregate_only(self) -> None:
        observed = RECORDER.token_diff(
            ["eins", "zwei", "drei", "vier"],
            ["eins", "zwo", "drei", "fuenf", "vier"],
        )
        self.assertEqual(4, observed["reference_token_count"])
        self.assertEqual(5, observed["candidate_token_count"])
        self.assertEqual(3, observed["equal_token_count"])
        self.assertEqual(1, observed["reference_missing_token_count"])
        self.assertEqual(2, observed["candidate_extra_token_count"])
        self.assertEqual(1, observed["replacement_block_count"])
        self.assertEqual(1, observed["insertion_block_count"])
        self.assertFalse(observed["exact_alpha_token_sequence"])
        self.assertNotIn("eins", json.dumps(observed))

    def test_candidate_receipt_opens_no_authority_or_human_debt(self) -> None:
        receipt = json.loads(RECEIPT_PATH.read_text(encoding="utf-8"))
        self.assertEqual(
            "machine-triangulated-candidate-not-admitted",
            receipt["status"],
        )
        self.assertTrue(
            receipt["comparisons"]["source_diplomatic_against_critical"][
                "exact_alpha_token_sequence"
            ]
        )
        self.assertFalse(
            receipt["comparisons"]["source_diplomatic_against_critical"][
                "historical_critical_equivalence_established"
            ]
        )
        effects = receipt["effects"]
        self.assertEqual(0, effects["human_debt_units"])
        for field in (
            "accepted_source_passage",
            "accepted_target_passage",
            "source_to_target_alignment",
            "eligible_for_variant_execution",
            "target_gold",
            "human_review_performed",
            "semantic_work_opened",
            "canon_effect",
            "publication_authorized",
        ):
            self.assertFalse(effects[field])


if __name__ == "__main__":
    unittest.main()
