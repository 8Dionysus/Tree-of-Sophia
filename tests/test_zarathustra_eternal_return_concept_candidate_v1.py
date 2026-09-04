from __future__ import annotations

import hashlib
import json
import stat
import subprocess
import unittest
from collections import Counter
from pathlib import Path

from jsonschema import Draft202012Validator


REPO = Path(__file__).resolve().parents[1]
ROUTE = REPO / "ToS/candidate-intake/zarathustra/eternal-return-concept-candidate-v1"
PRIVATE = REPO / "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/local-content/eternal-return-concept-candidate-v1/eternal-return-analysis.v1.json"


def load(name: str) -> dict:
    return json.loads((ROUTE / name).read_text(encoding="utf-8"))


def jsonl(name: str) -> list[dict]:
    return [json.loads(line) for line in (ROUTE / name).read_text(encoding="utf-8").splitlines()]


class ZarathustraEternalReturnConceptCandidateV1Tests(unittest.TestCase):
    def test_builder_parity_and_sign_annotation_contract(self):
        if not PRIVATE.is_file():
            self.skipTest("private eternal-return source-return analysis is not present")
        result = subprocess.run(
            ["python", "scripts/build_zarathustra_eternal_return_concept_candidate_v1.py", "--check"],
            cwd=REPO, text=True, capture_output=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        schema = json.loads((REPO / "ToS/contracts/sign-annotation.schema.json").read_text(encoding="utf-8"))
        Draft202012Validator(schema).validate(load("concept-candidate.v1.json"))

    def test_complete_corpus_scan_and_evidence_partition(self):
        summary = load("summary.v1.json")
        coverage = load("coverage-receipt.v1.json")
        evidence = jsonl("evidence-spine.v1.jsonl")
        self.assertEqual(coverage["corpus_parts_scanned"], 4)
        self.assertEqual(coverage["alignment_units_scanned"], 3423)
        self.assertEqual(set(coverage["part_counts"]), {"1", "2", "3", "4"})
        self.assertEqual(len(evidence), summary["evidence_unit_count"])
        self.assertEqual(
            Counter(x["evidence_class"] for x in evidence),
            Counter(summary["evidence_class_counts"]),
        )
        self.assertGreater(summary["evidence_class_counts"]["core"], 0)
        self.assertEqual(summary["evidence_class_counts"]["excluded"], 2)
        self.assertTrue(coverage["source_return_verified_during_build"])
        self.assertEqual(coverage["bilingual_selected_evidence_count"], 206)
        self.assertEqual(coverage["one_sided_scope_residue_count"], 4)
        self.assertFalse(coverage["de_ru_parallel_evidence_complete"])

    def test_candidate_is_trackable_without_semantic_promotion(self):
        concept = load("concept-candidate.v1.json")
        summary = load("summary.v1.json")
        templates = jsonl("interpretation-templates.v1.jsonl")
        self.assertTrue(concept["annotation_id"].startswith("tos.annotation."))
        self.assertEqual(concept["sign_layer"], "concept_candidate")
        self.assertEqual(concept["review_status"], "unreviewed")
        self.assertEqual(concept["review_refs"], [])
        self.assertIsNone(concept["body"]["sign_id"])
        self.assertIsNone(concept["body"]["concept_id"])
        self.assertEqual(concept["body"]["materialized_claim_refs"], [])
        self.assertEqual(len(templates), summary["interpretation_template_count"])
        self.assertTrue(all(not row["materialized_claim"] for row in templates))
        self.assertTrue(all(not row["semantic_fact_asserted"] for row in templates))
        self.assertEqual(summary["accepted_candidate_count"], 0)
        self.assertEqual(summary["human_review_count"], 0)
        self.assertFalse(summary["sign_identity_asserted"])
        self.assertFalse(summary["concept_identity_asserted"])
        self.assertFalse(summary["graph_effect"])
        self.assertFalse(summary["canon_effect"])

    def test_five_axes_blocked_amor_fati_and_negative_controls(self):
        templates = {x["claim_code"]: x for x in jsonl("interpretation-templates.v1.jsonl")}
        axes = {
            "cosmological_recurrence",
            "temporality_and_augenblick",
            "existential_test_and_affirmation",
            "ring_and_wholeness",
            "life_joy_pain_affirmation",
        }
        self.assertTrue(axes.issubset(templates))
        self.assertTrue(all(templates[x]["evidence_refs"] for x in axes))
        self.assertEqual(templates["amor_fati_cross_work"]["status"], "deferred")
        self.assertEqual(templates["amor_fati_cross_work"]["evidence_refs"], [])
        controls = {
            x.get("exclusion_code") or x.get("control_code")
            for x in jsonl("exclusion-ledger.v1.jsonl")
        }
        self.assertEqual(controls, {
            "return_to_humans_not_eternal_return",
            "return_home_not_eternal_return",
            "bare_again_is_not_membership",
            "isolated_eternity_is_not_membership",
            "ring_without_recurrence_is_not_membership",
            "liturgical_eternity_is_not_membership",
            "speaker_flattening_forbidden",
            "alignment_proposal_is_not_semantic_equivalence",
        })

    def test_formula_census_private_return_and_source_withholding(self):
        if not PRIVATE.is_file():
            self.skipTest("private eternal-return source-return analysis is not present")
        formulas = {x["formula_code"]: x for x in jsonl("formula-candidates.v1.jsonl")}
        self.assertEqual(formulas["de_recurrence_noun"]["occurrence_count"], 12)
        self.assertEqual(formulas["de_ring_of_recurrence"]["occurrence_count"], 7)
        self.assertEqual(formulas["ru_ring_of_return"]["occurrence_count"], 6)
        self.assertEqual(formulas["ru_joy_wants_eternity"]["occurrence_count"], 3)
        self.assertEqual(stat.S_IMODE(PRIVATE.stat().st_mode), 0o600)
        private = json.loads(PRIVATE.read_text(encoding="utf-8"))
        tracked = {x["evidence_id"]: x for x in jsonl("evidence-spine.v1.jsonl")}
        for row in private["evidence"]:
            public = tracked[row["evidence_id"]]
            self.assertEqual(hashlib.sha256(row["de_text"].encode()).hexdigest(), public["de_exact_sha256"])
            self.assertEqual(hashlib.sha256(row["ru_text"].encode()).hexdigest(), public["ru_exact_sha256"])
        projection = "".join(
            (ROUTE / name).read_text(encoding="utf-8")
            for name in ("evidence-spine.v1.jsonl", "formula-candidates.v1.jsonl")
        ).casefold()
        for exact in ("ring der wiederkunft", "кольцу возвращения", "радость хочет вечности"):
            self.assertNotIn(exact, projection)

    def test_identity_issuance_is_unique_and_separate_from_labels(self):
        issuance = load("identity-issuance.v1.json")
        ids = [x["id"] for x in issuance["records"]]
        self.assertEqual(len(ids), len(set(ids)))
        bindings = "\n".join(x["binding"] for x in issuance["records"]).casefold()
        self.assertNotIn("ewige wiederkunft", bindings)
        self.assertNotIn("вечное возвращение", bindings)

    def test_independent_challengers_are_integrated_without_human_impersonation(self):
        audit = load("independent-agent-audit.v1.json")
        self.assertFalse(audit["human_review"])
        self.assertTrue(audit["semantic_design_challenger"]["five_axes_preserved"])
        source = audit["source_evidence_challenger"]
        self.assertEqual(source["alignment_groups_scanned"], 3423)
        self.assertEqual(source["occurrence_candidate_count"], 791)
        self.assertEqual(source["passage_candidate_count"], 171)
        self.assertEqual(source["formula_correspondence_count"], 34)
        self.assertEqual(len(source["alignment_gaps"]), 5)
        self.assertEqual(
            {x["gap_code"] for x in source["alignment_gaps"] if x["reading_ref"] == "p3.r13"},
            {"ru_formula_broken_by_letterspacing", "ru_formula_blocked_by_ocr_substitution"},
        )
        integrated = audit["integrated_controls"]
        self.assertTrue(integrated["ocr_and_letterspacing_gaps_not_silently_corrected"])
        self.assertEqual(integrated["accepted_candidate_count"], 0)
        self.assertEqual(integrated["human_review_count"], 0)


if __name__ == "__main__":
    unittest.main()
