from __future__ import annotations

import json
import sqlite3
import stat
import subprocess
import unittest
from contextlib import closing
from pathlib import Path


REPO = Path(__file__).resolve().parents[1]
WORK = Path("ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra")
ROUTE = REPO / WORK / "lexical-indexes/dta-antonovsky-parallel-candidates-v1"
ALIGN = REPO / WORK / "alignments/translation/dta-first-editions-to-antonovsky-1911-paragraph-v1"
PRIVATE = REPO / WORK / "gold-sets/foundation-pilot-v1/local-content/parallel-lexical-candidates-v1"


def jsonl(name: str):
    return [json.loads(x) for x in (ROUTE / name).read_text(encoding="utf-8").splitlines()]


class ParallelLexicalCandidateV1Tests(unittest.TestCase):
    def test_builder_parity(self):
        if not (PRIVATE / "antonovsky-1911-lexical-observation-v1.sqlite3").is_file():
            self.skipTest("private lexical databases are not present")
        result = subprocess.run(
            ["python", "scripts/build_zarathustra_parallel_lexical_candidates_v1.py", "--check"],
            cwd=REPO, text=True, capture_output=True,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_summary_and_complete_status_partition(self):
        summary = json.loads((ROUTE / "summary.v1.json").read_text(encoding="utf-8"))
        self.assertEqual(summary["parts"], 4)
        self.assertEqual(summary["german_token_occurrences"], 86287)
        self.assertGreater(summary["russian_exact_token_occurrences"], 90000)
        self.assertEqual(summary["parallel_status_counts"], {
            "ambiguous": 111, "deferred": 9, "proposed": 3303,
        })
        associations = jsonl("translation-surface-association-candidates.v1.jsonl")
        self.assertEqual(len(associations), summary["association_candidate_count"])
        self.assertEqual(sum(summary["association_status_counts"].values()), len(associations))
        self.assertEqual(summary["accepted_candidate_count"], 0)
        self.assertFalse(summary["semantic_equivalence_asserted"])

    def test_opaque_identity_and_source_withholding(self):
        associations = jsonl("translation-surface-association-candidates.v1.jsonl")
        ids = [x["candidate_id"] for x in associations]
        self.assertEqual(len(ids), len(set(ids)))
        for row in associations:
            self.assertNotIn("source_form", row)
            self.assertNotIn("target_form", row)
            self.assertFalse(row["accepted"])
            self.assertEqual(row["review_refs"], [])
            self.assertFalse(row["graph_effect"])
        for name in ("keyword-candidates.v1.jsonl", "repeated-sequences.v1.jsonl",
                     "russian-recurrence.v1.jsonl"):
            text = (ROUTE / name).read_text(encoding="utf-8")
            self.assertNotIn('"analysis_key"', text)
            self.assertNotIn('"sequence"', text)

    def test_positive_evidence_is_proposed_alignment_only(self):
        status = {}
        for line in (ALIGN / "alignment-spine.v1.jsonl").read_text(encoding="utf-8").splitlines():
            row = json.loads(line); status[row["alignment_id"]] = row["status"]
        for candidate in jsonl("translation-surface-association-candidates.v1.jsonl"):
            self.assertTrue(candidate["identity_universe_alignment_refs"])
            self.assertTrue(all(status[x] == "proposed" for x in candidate["supporting_alignment_refs"]))
            self.assertTrue(all(status[x] == "proposed" for x in candidate["identity_universe_alignment_refs"]))
            if candidate["status"] == "proposed":
                self.assertTrue(candidate["supporting_alignment_refs"])
                self.assertGreaterEqual(candidate["support"], 8)
                self.assertGreaterEqual(candidate["strict_support"], 4)
                self.assertLessEqual(candidate["source_candidate_rank"], 3)
                self.assertLessEqual(candidate["target_candidate_rank"], 3)

    def test_private_artifacts_are_closed_and_searchable(self):
        analysis = PRIVATE / "parallel-candidate-analysis.v1.json"
        database = PRIVATE / "antonovsky-1911-lexical-observation-v1.sqlite3"
        if not analysis.is_file() or not database.is_file():
            self.skipTest("private parallel lexical artifacts are not present")
        for path in (analysis, database):
            self.assertEqual(stat.S_IMODE(path.stat().st_mode), 0o600)
        summary = json.loads((ROUTE / "summary.v1.json").read_text(encoding="utf-8"))
        with closing(sqlite3.connect(database)) as db:
            count = db.execute("select count(*) from occurrences").fetchone()[0]
            hit = db.execute("select count(*) from unit_fts where normalized_text match 'заратустра'").fetchone()[0]
        self.assertEqual(count, summary["russian_exact_token_occurrences"])
        self.assertGreater(hit, 0)


if __name__ == "__main__":
    unittest.main()
