from __future__ import annotations

import hashlib
import json
import stat
import subprocess
import sys
import unittest
from collections import Counter
from pathlib import Path


REPO = Path(__file__).resolve().parents[1]
ROUTE = REPO / (
    "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
    "alignments/translation/dta-first-editions-to-antonovsky-1911-paragraph-v1"
)
BUILDER = "scripts/build_zarathustra_de_ru_paragraph_alignment_v1.py"


def jsonl(name: str) -> list[dict]:
    return [json.loads(line) for line in (ROUTE / name).read_text().splitlines()]


class ZarathustraDeRuParagraphAlignmentV1Tests(unittest.TestCase):
    def run_builder(self, action: str) -> None:
        completed = subprocess.run(
            [sys.executable, BUILDER, action], cwd=REPO,
            capture_output=True, text=True, check=False,
        )
        self.assertEqual(
            completed.returncode, 0,
            msg=f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}",
        )

    def test_tracked_route_validates(self) -> None:
        self.run_builder("--validate-tracked")

    def test_complete_paragraph_coverage_and_machine_risk_union(self) -> None:
        coverage = json.loads((ROUTE / "coverage-receipt.v1.json").read_text())
        self.assertEqual(coverage["packet_count"], 4)
        self.assertEqual(coverage["reading_pair_count"], 81)
        self.assertEqual(coverage["reading_pairs_by_part"],
                         {"I": 23, "II": 22, "III": 16, "IV": 20})
        self.assertEqual(coverage["alignment_count"], 3423)
        self.assertEqual(coverage["source_paragraph_covered"], 3447)
        self.assertEqual(coverage["target_paragraph_covered"], 3569)
        self.assertEqual(coverage["source_paragraph_duplicate_count"], 0)
        self.assertEqual(coverage["target_paragraph_duplicate_count"], 0)
        self.assertEqual(coverage["status_counts"],
                         {"proposed": 3303, "ambiguous": 111, "deferred": 9})
        self.assertEqual(coverage["independent_challenger_same_group_count"], 3311)
        self.assertEqual(coverage["independent_challenger_disagreement_count"], 112)
        self.assertEqual(coverage["independent_challenger_disagreement_paragraph_unit_count"], 263)
        self.assertEqual(coverage["independent_challenger_disagreement_by_part"],
                         {"I": 2, "II": 8, "III": 53, "IV": 49})
        self.assertEqual(coverage["primary_profile_disagreement_count"], 20)
        self.assertEqual(
            coverage["primary_profile_and_independent_disagreement_overlap_count"], 12
        )
        self.assertEqual(coverage["primary_only_machine_risk_count"], 8)
        self.assertEqual(coverage["machine_risk_union_count"], 120)
        self.assertEqual(coverage["conflict_ledger_count"], 120)
        self.assertEqual(coverage["unaccounted_machine_risk_count"], 0)
        self.assertEqual(coverage["accepted_alignment_count"], 0)
        self.assertFalse(coverage["semantic_equivalence_asserted"])
        self.assertFalse(coverage["canon_effect"])

    def test_packets_preserve_scope_disagreement_and_authority_boundaries(self) -> None:
        issuance = json.loads((ROUTE / "identity-issuance.v1.json").read_text())
        issued = {
            row["id"] for rows in issuance["identities"].values() for row in rows
        }
        all_alignments = []
        for part in range(1, 5):
            packet = json.loads(
                (ROUTE / f"part-{part}.translation-alignment-packet.v1.json").read_text()
            )
            self.assertIn(packet["packet_id"], issued)
            self.assertEqual(packet["reviews"], [])
            self.assertEqual(packet["projections"], [])
            self.assertFalse(packet["authority_boundary"]["canon_effect"])
            self.assertTrue(packet["authority_boundary"]["alignment_is_claim_not_translation_truth"])
            for anchor in packet["target_side"]["anchors"]:
                self.assertIn(anchor["anchor_ref"], issued)
            for alignment in packet["alignments"]:
                self.assertIn(alignment["alignment_id"], issued)
                self.assertIn(alignment["claim_id"], issued)
                self.assertEqual(alignment["review_refs"], [])
            all_alignments.extend(packet["alignments"])

        self.assertEqual(len(all_alignments), 3423)
        self.assertEqual(Counter(row["status"] for row in all_alignments),
                         Counter({"proposed": 3303, "ambiguous": 111, "deferred": 9}))
        internal_nulls = [row for row in all_alignments
                          if row["correspondence_shape"] == "source_omission"]
        outside = [row for row in all_alignments
                   if row["correspondence_shape"] == "unresolved"]
        self.assertEqual(len(internal_nulls), 12)
        self.assertTrue(all(row["status"] == "ambiguous" for row in internal_nulls))
        self.assertTrue(all("not evidence of translator omission" in row["status_reason"]
                            for row in internal_nulls))
        self.assertEqual(
            Counter(row["status_reason"] for row in internal_nulls),
            Counter({
                "cross-kind/out-of-paragraph-scope candidate; not evidence of translator omission": 11,
                "structural-role mismatch; not evidence of translator omission": 1,
            }),
        )
        self.assertEqual(len(outside), 9)
        self.assertTrue(all(row["status"] == "deferred" for row in outside))
        self.assertTrue(all("not evidence of translator omission" in row["status_reason"]
                            for row in outside))

        spine = jsonl("alignment-spine.v1.jsonl")
        self.assertFalse(any(
            row["status"] == "proposed"
            for row in spine
            if not row["independent_challenger_same_paragraph_group"]
        ))
        conflicts = jsonl("conflict-ledger.v1.jsonl")
        self.assertEqual(len(conflicts), 120)
        self.assertTrue(all(row["machine_resolution"] == "none" for row in conflicts))

    def test_verse_exclusions_and_exact_private_selectors(self) -> None:
        exclusions = jsonl("scope-exclusions.v1.jsonl")
        source = [row for row in exclusions if row["side"] == "source"]
        target = [row for row in exclusions if row["side"] == "target"]
        self.assertEqual((len(source), sum(len(row["ordered_excluded_verse_line_refs"])
                                          for row in source)), (39, 368))
        self.assertEqual((len(target), sum(len(row["ordered_excluded_verse_line_refs"])
                                          for row in target)), (13, 359))
        self.assertTrue(all(row["used_as_structural_risk_qualification"]
                            for row in exclusions))

        packets = [
            json.loads(
                (ROUTE / f"part-{part}.translation-alignment-packet.v1.json").read_text()
            )
            for part in range(1, 5)
        ]
        private_layers = [
            REPO / packet[side_name]["text_layer_ref"]
            for packet in packets
            for side_name in ("source_side", "target_side")
        ]
        if not all(layer.is_file() for layer in private_layers):
            self.skipTest("exact local German and Russian private layers are not present")

        for packet in packets:
            for side_name in ("source_side", "target_side"):
                side = packet[side_name]
                layer = REPO / side["text_layer_ref"]
                self.assertEqual(stat.S_IMODE(layer.stat().st_mode), 0o600)
                payload = layer.read_bytes()
                self.assertEqual(hashlib.sha256(payload).hexdigest(),
                                 side["text_layer_sha256"])
                text = payload.decode("utf-8")
                for anchor in side["anchors"]:
                    selector = anchor["selector"]
                    exact = text[selector["start"]:selector["end"]].encode("utf-8")
                    self.assertEqual(hashlib.sha256(exact).hexdigest(),
                                     anchor["exact_sha256"])

    def test_full_local_rebuild_parity(self) -> None:
        packets = [
            json.loads(
                (ROUTE / f"part-{part}.translation-alignment-packet.v1.json").read_text()
            )
            for part in range(1, 5)
        ]
        private_layers = [
            REPO / packet[side_name]["text_layer_ref"]
            for packet in packets
            for side_name in ("source_side", "target_side")
        ]
        if not all(layer.is_file() for layer in private_layers):
            self.skipTest("exact local German and Russian private layers are not present")
        self.run_builder("--check")


if __name__ == "__main__":
    unittest.main()
