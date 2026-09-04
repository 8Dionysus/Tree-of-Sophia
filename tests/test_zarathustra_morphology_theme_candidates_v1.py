from __future__ import annotations

import json
import stat
import subprocess
import unittest
from collections import Counter
from pathlib import Path


REPO = Path(__file__).resolve().parents[1]
ROUTE = REPO / "ToS/candidate-intake/zarathustra/dta-antonovsky-morphology-themes-v1"
PRIVATE = REPO / "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/gold-sets/foundation-pilot-v1/local-content/morphology-themes-v1/morphology-theme-analysis.v1.json"


def jsonl(name: str) -> list[dict]:
    return [json.loads(line) for line in (ROUTE / name).read_text(encoding="utf-8").splitlines()]


class ZarathustraMorphologyThemeCandidateV1Tests(unittest.TestCase):
    def test_builder_parity(self):
        if not PRIVATE.is_file():
            self.skipTest("private morphology analysis is not present")
        result = subprocess.run(
            ["python", "scripts/build_zarathustra_morphology_theme_candidates_v1.py", "--check"],
            cwd=REPO, text=True, capture_output=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_complete_candidate_partitions_and_zero_promotion(self):
        summary = json.loads((ROUTE / "summary.v1.json").read_text(encoding="utf-8"))
        families = jsonl("morphological-family-candidates.v1.jsonl")
        clusters = jsonl("thematic-cluster-candidates.v1.jsonl")
        relations = jsonl("typed-relation-candidates.v1.jsonl")
        self.assertEqual(summary["parts"], 4)
        self.assertEqual(len(families), summary["morphological_family_candidate_count"])
        self.assertEqual(len(clusters), summary["thematic_cluster_candidate_count"])
        self.assertEqual(len(relations), summary["typed_relation_candidate_count"])
        self.assertEqual(Counter(x["status"] for x in families), Counter(summary["family_status_counts"]))
        self.assertEqual(Counter(x["relation_type"] for x in relations), Counter(summary["relation_type_counts"]))
        self.assertEqual(summary["accepted_candidate_count"], 0)
        self.assertEqual(summary["human_review_count"], 0)
        self.assertFalse(summary["semantic_relation_asserted"])
        self.assertFalse(summary["concept_identity_asserted"])
        self.assertFalse(summary["graph_effect"])
        self.assertFalse(summary["canon_effect"])

    def test_opaque_identity_source_withholding_and_no_review(self):
        rows = (
            jsonl("morphological-family-candidates.v1.jsonl")
            + jsonl("thematic-cluster-candidates.v1.jsonl")
            + jsonl("typed-relation-candidates.v1.jsonl")
        )
        ids = [next(value for key, value in row.items() if key.endswith("_id")) for row in rows]
        self.assertEqual(len(ids), len(set(ids)))
        for row in rows:
            self.assertFalse(row["accepted"])
            self.assertEqual(row["review_refs"], [])
            self.assertFalse(row["graph_effect"])
            self.assertNotIn("binding", row)
            self.assertNotIn("member_keys", row)
        tracked = "".join((ROUTE / name).read_text(encoding="utf-8") for name in (
            "morphological-family-candidates.v1.jsonl",
            "thematic-cluster-candidates.v1.jsonl",
            "typed-relation-candidates.v1.jsonl",
        ))
        for source_string in ("mensch", "menschen", "liebe", "человек", "людей", "любовь", "люблю"):
            self.assertNotIn(source_string, tracked.casefold())

    def test_named_probes_and_false_merge_controls(self):
        if not PRIVATE.is_file():
            self.skipTest("private morphology analysis is not present")
        private = json.loads(PRIVATE.read_text(encoding="utf-8"))
        probes = private["named_probe_examples"]
        self.assertTrue(any(
            row["status"] == "proposed" and {"mensch", "menschen"}.issubset(row["forms"])
            for row in probes["de_mensch"]
        ))
        self.assertFalse(any(
            row["status"] == "proposed" and {"liebe", "lieben"}.issubset(row["forms"])
            for row in probes["de_liebe"]
        ))
        self.assertTrue(any(
            row["status"] == "deferred" and {"человек", "люди", "людей"}.issubset(row["forms"])
            for row in probes["ru_human"]
        ))
        self.assertTrue(any(
            row["status"] == "deferred" and {"любовь", "люблю"}.issubset(row["forms"])
            for row in probes["ru_love"]
        ))
        for members in ({"друг", "другой"}, {"поэт", "поэтому"}):
            hits = [row for row in private["families"] if members.issubset(row["forms"])]
            self.assertTrue(hits)
            self.assertTrue(all(row["status"] != "proposed" for row in hits))

    def test_private_mode_provider_fixity_and_bilingual_clusters(self):
        if not PRIVATE.is_file():
            self.skipTest("private morphology analysis is not present")
        self.assertEqual(stat.S_IMODE(PRIVATE.stat().st_mode), 0o600)
        private = json.loads(PRIVATE.read_text(encoding="utf-8"))
        provider = private["russian_morphology_provider"]
        self.assertEqual(len(provider["dictionary_aff_sha256"]), 64)
        self.assertEqual(len(provider["dictionary_dic_sha256"]), 64)
        self.assertTrue(private["clusters"])
        for cluster in private["clusters"]:
            self.assertTrue(cluster["display_hint"]["de"])
            self.assertTrue(cluster["display_hint"]["ru"])
            self.assertFalse(cluster["concept_identity_asserted"])

    def test_independent_agent_audit_is_closed_and_non_human(self):
        audit = json.loads((ROUTE / "independent-agent-audit.v1.json").read_text(encoding="utf-8"))
        self.assertTrue(audit["german_full_census_challenger"]["two_full_builds_byte_identical"])
        self.assertTrue(audit["russian_full_census_challenger"]["two_full_builds_byte_identical"])
        controls = audit["integrated_controls"]
        self.assertTrue(controls["german_mixed_noun_verb_families_not_proposed"])
        self.assertTrue(controls["russian_suffix_false_merges_not_proposed"])
        self.assertEqual(controls["accepted_candidate_count"], 0)
        self.assertEqual(controls["human_review_count"], 0)
        self.assertFalse(controls["semantic_review_impersonated"])


if __name__ == "__main__":
    unittest.main()
