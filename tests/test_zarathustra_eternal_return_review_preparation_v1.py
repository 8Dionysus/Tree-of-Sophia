from __future__ import annotations

import hashlib
import json
import stat
import subprocess
import unittest
from collections import Counter
from pathlib import Path


REPO = Path(__file__).resolve().parents[1]
ROUTE = REPO / "ToS/candidate-intake/zarathustra/eternal-return-concept-candidate-v1/review-preparation-v1"
PRIVATE = REPO / (
    "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
    "gold-sets/foundation-pilot-v1/local-content/eternal-return-concept-candidate-v1/"
    "review-preparation-v1/review-preparation-analysis.v1.json"
)


def load(name: str) -> dict:
    return json.loads((ROUTE / name).read_text(encoding="utf-8"))


def jsonl(name: str) -> list[dict]:
    return [json.loads(line) for line in (ROUTE / name).read_text(encoding="utf-8").splitlines()]


class ZarathustraEternalReturnReviewPreparationV1Tests(unittest.TestCase):
    def test_builder_parity_and_complete_declared_scope(self):
        result = subprocess.run(
            ["python", "scripts/build_zarathustra_eternal_return_review_preparation_v1.py", "--check"],
            cwd=REPO, text=True, capture_output=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        coverage = load("coverage-receipt.v1.json")
        self.assertTrue(coverage["complete_for_declared_scope"])
        self.assertEqual(coverage["gap_candidates_prepared"], 5)
        self.assertEqual(coverage["speaker_candidates_prepared"], 159)
        self.assertEqual(coverage["speaker_reading_counts"], {
            "p3.r13": 50, "p3.r16": 34, "p3.r2": 31, "p4.r19": 44,
        })

    def test_five_gaps_are_source_returned_without_mutation(self):
        gaps = jsonl("gap-review-candidates.v1.jsonl")
        self.assertEqual(len(gaps), 5)
        self.assertTrue(all(row["observed_text_preserved"] for row in gaps))
        self.assertTrue(all(not row["proposal_applied_to_witness"] for row in gaps))
        self.assertTrue(all(not row["proposal_applied_to_alignment"] for row in gaps))
        self.assertTrue(all(row["review_status"] == "unreviewed" for row in gaps))
        target_gap = next(row for row in gaps if row["gap_code"] == "machine_target_gap_not_translation_omission")
        self.assertEqual(target_gap["alignment_shape"], "source_omission")
        self.assertEqual(len(target_gap["related_witness_units"]), 1)
        related = target_gap["related_witness_units"][0]
        self.assertEqual(related["text_unit_ref"], "tos.text-unit.sid-d523bc897647d01030f767c2faf5266b")
        self.assertEqual(related["text_sha256"], "0627928ac02cab8159b250950b4a9b23088c9fe5886ebedf86634be3488da558")

    def test_speaker_population_and_boundary_alternatives(self):
        speakers = jsonl("speaker-attribution-candidates.v1.jsonl")
        self.assertEqual(len(speakers), 159)
        self.assertEqual(Counter(row["attribution_status"] for row in speakers),
                         Counter({"proposed": 108, "ambiguous": 51}))
        roles = Counter(row["primary_role"] for row in speakers)
        self.assertEqual(roles["animals_eagle_and_serpent"], 19)
        self.assertEqual(roles["zarathustra_midnight_song_voice"], 34)
        self.assertEqual(roles["zarathustra_song_voice"], 33)
        self.assertEqual(roles["dwarf"], 1)
        midnight = [row for row in speakers if row["primary_role"] == "zarathustra_midnight_song_voice"]
        self.assertTrue(all("personified_midnight_or_bell_voice" in row["alternative_roles"] for row in midnight))
        self.assertTrue(all(not row["accepted"] and not row["materialized_claim"] for row in speakers))

    def test_five_axes_counterpressure_and_blocked_cross_work_relation(self):
        axes = {row["axis_code"]: row for row in load("interpretation-review-matrix.v1.json")["axes"]}
        primary = {
            "cosmological_recurrence", "temporality_and_augenblick",
            "existential_test_and_affirmation", "ring_and_wholeness",
            "life_joy_pain_affirmation",
        }
        self.assertTrue(all(axes[code]["status"] == "prepared_for_review" for code in primary))
        self.assertTrue(all(axes[code]["positive_evidence_refs"] for code in primary))
        self.assertTrue(all(axes[code]["counterpressure_evidence_refs"] for code in primary))
        self.assertTrue(all(axes[code]["review_questions"] for code in primary))
        self.assertEqual(axes["amor_fati_cross_work"]["status"], "blocked_cross_work")
        self.assertEqual(axes["amor_fati_cross_work"]["positive_evidence_refs"], [])
        self.assertTrue(all(not row["materialized_claim"] for row in axes.values()))

    def test_worklist_compresses_full_population_to_exception_bundles(self):
        worklist = load("review-worklist.v1.json")
        kinds = Counter(row["kind"] for row in worklist["work_items"])
        self.assertEqual(kinds, Counter({
            "gap_exception_bundle": 5,
            "speaker_boundary_bundle": 6,
            "interpretation_axis_bundle": 7,
        }))
        self.assertEqual(worklist["speaker_candidate_population"], 159)
        self.assertEqual(worklist["work_item_count"], 18)
        self.assertFalse(worklist["review_outcome_recorded"])
        self.assertIsNone(worklist["review_ledger_ref"])

    def test_private_exact_return_and_public_withholding(self):
        self.assertEqual(stat.S_IMODE(PRIVATE.stat().st_mode), 0o600)
        private = json.loads(PRIVATE.read_text(encoding="utf-8"))
        gaps = {row["alignment_ref"]: row for row in jsonl("gap-review-candidates.v1.jsonl")}
        for row in private["gaps"]:
            public = gaps[row["alignment_ref"]]
            self.assertEqual(hashlib.sha256(row["de_text"].encode()).hexdigest(), public["de_exact_sha256"])
            self.assertEqual(hashlib.sha256(row["ru_text"].encode()).hexdigest(), public["ru_exact_sha256"])
        public_text = "".join(path.read_text(encoding="utf-8") for path in ROUTE.iterdir() if path.is_file()).casefold()
        self.assertNotIn("lehrer der ewigen wiederkunft", public_text)
        self.assertNotIn("учитель вѣчнаго возвращенія", public_text)

    def test_zero_authority_promotion(self):
        summary = load("summary.v1.json")
        self.assertEqual(summary["witness_correction_count"], 0)
        self.assertEqual(summary["alignment_mutation_count"], 0)
        self.assertEqual(summary["accepted_candidate_count"], 0)
        self.assertEqual(summary["human_review_count"], 0)
        self.assertEqual(summary["materialized_claim_count"], 0)
        self.assertEqual(summary["review_ledger_write_count"], 0)
        self.assertFalse(summary["graph_effect"])
        self.assertFalse(summary["canon_effect"])


if __name__ == "__main__":
    unittest.main()
