from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPT_PATH = REPO_ROOT / "scripts/record_zarathustra_visual_retrieval_result.py"
RESULT_PATH = (
    REPO_ROOT
    / "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/gold-sets/foundation-pilot-v1/"
    "visual-retrieval-result.c-qwen3-vl-embedding-2b.v1.json"
)
SCHEMA_PATH = (
    REPO_ROOT / "ToS/contracts/visual-retrieval-result-receipt.schema.json"
)


def _load_recorder():
    spec = importlib.util.spec_from_file_location(
        "tos_zarathustra_visual_retrieval_result_recorder",
        SCRIPT_PATH,
    )
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {SCRIPT_PATH}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


RECORDER = _load_recorder()


class ZarathustraVisualRetrievalResultTests(unittest.TestCase):
    def test_tracked_result_is_schema_valid_and_source_free(self) -> None:
        result = json.loads(RESULT_PATH.read_text(encoding="utf-8"))
        schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
        self.assertEqual(
            [],
            list(Draft202012Validator(schema).iter_errors(result)),
        )
        encoded = RESULT_PATH.read_text(encoding="utf-8")
        for forbidden in (
            "/srv/",
            "/home/",
            "сверхчеловек смысл земли",
            "Uebermensch ist der Sinn der Erde",
            "государство",
            "Sinn der Erde",
        ):
            self.assertNotIn(forbidden, encoded)
        self.assertFalse(
            result["rights_and_visibility"][
                "tracked_receipt_contains_source_strings"
            ]
        )
        self.assertFalse(
            result["rights_and_visibility"][
                "tracked_receipt_contains_query_strings"
            ]
        )
        self.assertFalse(
            result["quality_and_promotion"]["promotion_authorized"]
        )
        self.assertIsNone(result["quality_and_promotion"]["winner"])

    def test_trigger_scope_is_rare_and_exact(self) -> None:
        result = json.loads(RESULT_PATH.read_text(encoding="utf-8"))
        self.assertFalse(result["triggered_review"]["routine_review_scheduled"])
        self.assertTrue(result["triggered_review"]["review_opened"])
        self.assertEqual(0, result["triggered_review"]["human_debt_count"])
        self.assertEqual(
            [
                "tos-query-003",
                "tos-query-009",
                "tos-query-010",
                "tos-query-011",
                "tos-query-020",
            ],
            result["triggered_review"]["query_ids"],
        )
        self.assertEqual(
            "criteria-only-source-visible-ranking-review-no-retyping",
            result["triggered_review"]["review_scope"],
        )
        self.assertFalse(
            result["triggered_review"]["review_interface_materialized"]
        )

    def test_mechanical_counts_do_not_become_quality(self) -> None:
        result = json.loads(RESULT_PATH.read_text(encoding="utf-8"))
        advisory = result["advisory_results"]
        self.assertEqual(19, advisory["model_proposed_target_at_10"])
        self.assertEqual(19, advisory["evaluable_query_count"])
        self.assertEqual(1, advisory["coverage_recovery"])
        self.assertEqual(9, advisory["hard_negative_presence"])
        self.assertEqual(10, advisory["hard_negative_slots"])
        self.assertEqual(4, advisory["hard_negative_outranks_expected"])
        self.assertTrue(advisory["advisory_only"])
        quality = result["quality_and_promotion"]
        self.assertIsNone(quality["human_ndcg_at_10"])
        self.assertIsNone(quality["human_hard_negative_error_rate"])
        self.assertIsNone(quality["human_review_minutes"])

    def test_schema_rejects_simulated_human_completion(self) -> None:
        result = json.loads(RESULT_PATH.read_text(encoding="utf-8"))
        schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
        result["triggered_review"]["human_judgment_status"] = "completed"
        errors = list(Draft202012Validator(schema).iter_errors(result))
        self.assertTrue(errors)

    def test_file_set_record_contains_no_paths_or_content(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            first = root / "a.json"
            second = root / "b.json"
            first.write_text('{"query_id":"q1"}\n', encoding="utf-8")
            second.write_text('{"query_id":"q2"}\n', encoding="utf-8")
            observed = RECORDER.file_set_record(
                [first, second],
                root=root,
                source_bearing=False,
            )
        self.assertEqual(2, observed["file_count"])
        self.assertNotIn("q1", json.dumps(observed))
        self.assertNotIn("a.json", json.dumps(observed))


if __name__ == "__main__":
    unittest.main()
