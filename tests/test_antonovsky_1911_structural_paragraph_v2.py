from __future__ import annotations

import json
import subprocess
import sys
import unittest
from collections import Counter, defaultdict
from pathlib import Path


REPO = Path(__file__).resolve().parents[1]
ROUTE = REPO / (
    "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
    "technical-markup/antonovsky-1911-structural-paragraph-v2"
)
SOURCE_PDF = REPO / (
    "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
    "expressions/ru-antonovsky-1911/editions/saint-petersburg-prometey-1911-fourth/"
    "items/rsl-neb-scan-pdf/payload/antonovsky-prometey-1911-rsl-neb.pdf"
)


def jsonl(name: str) -> list[dict]:
    return [json.loads(line) for line in (ROUTE / name).read_text().splitlines()]


class Antonovsky1911StructuralParagraphV2Tests(unittest.TestCase):
    def test_tracked_route_validates(self) -> None:
        completed = subprocess.run(
            [sys.executable, "scripts/build_antonovsky_1911_structural_paragraph_v2.py",
             "--validate-tracked"], cwd=REPO, capture_output=True, text=True)
        self.assertEqual(completed.returncode, 0,
                         msg=f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}")

    def test_complete_counts_and_authority_boundary(self) -> None:
        summary = json.loads((ROUTE / "summary.v2.json").read_text())
        self.assertEqual(summary["status"], "agent_verified_complete")
        self.assertEqual(summary["physical_line_count"], 13071)
        self.assertEqual(summary["logical_row_count"], 10686)
        self.assertEqual(summary["reading_unit_count"], 81)
        self.assertEqual(summary["numbered_subpart_count"], 112)
        self.assertEqual(summary["prose_paragraph_count"], 3569)
        self.assertEqual(summary["continuous_verse_group_count"], 13)
        self.assertEqual(summary["verse_line_count"], 359)
        self.assertEqual(summary["prose_paragraph_counts_by_part"],
                         {"part_1": 838, "part_2": 889, "part_3": 991, "part_4": 851})
        self.assertEqual(summary["unresolved_boundary_conflict_count"], 0)
        self.assertEqual(summary["unresolved_challenger_disagreement_count"], 0)
        self.assertFalse(summary["source_text_included"])
        self.assertFalse(summary["semantic_authority"])

    def test_structure_paragraph_and_verse_closure(self) -> None:
        structures = jsonl("structure-spine.v2.jsonl")
        paragraphs = jsonl("paragraph-spine.v2.jsonl")
        groups = jsonl("verse-group-spine.v2.jsonl")
        verse_lines = jsonl("verse-line-spine.v2.jsonl")
        physical = jsonl("physical-line-spine.v2.jsonl")
        kinds = Counter(row["record_kind"] for row in structures)
        self.assertEqual(kinds["part_heading"], 4)
        self.assertEqual(kinds["reading_unit_heading"], 81)
        self.assertEqual(kinds["numbered_subpart_marker"], 112)
        self.assertEqual(sum(row["visual_only_heading_observation"] for row in structures), 2)
        structure_ids = {row["structure_unit_id"] for row in structures}
        self.assertTrue(all(row["parent_structure_unit_id"] in structure_ids
                            for row in structures if row["parent_structure_unit_id"]))
        structure_by_key = {
            row["technical_structure_key"]: row for row in structures
        }
        cycle_id = structure_by_key["part_1.cycle_heading"]["structure_unit_id"]
        self.assertEqual(
            structure_by_key["part_1.reading_01"]["parent_structure_unit_id"],
            structure_by_key["part_1"]["structure_unit_id"],
        )
        self.assertTrue(
            all(
                structure_by_key[f"part_1.reading_{ordinal:02d}"]["parent_structure_unit_id"]
                == cycle_id
                for ordinal in range(2, 24)
            )
        )
        reading_ordinals = defaultdict(list)
        subpart_ordinals = defaultdict(list)
        for row in paragraphs:
            reading_ordinals[row["reading_unit_ref"]].append(row["ordinal_within_reading_unit"])
            if row["numbered_subpart_ref"]:
                subpart_ordinals[row["numbered_subpart_ref"]].append(
                    row["ordinal_within_numbered_subpart"])
        self.assertEqual(len(reading_ordinals), 81)
        for values in [*reading_ordinals.values(), *subpart_ordinals.values()]:
            self.assertEqual(values, list(range(1, len(values) + 1)))
        prose_refs = {ref for row in paragraphs for ref in row["ordered_physical_line_refs"]}
        verse_refs = {ref for row in verse_lines for ref in row["ordered_physical_line_refs"]}
        self.assertTrue(prose_refs.isdisjoint(verse_refs))
        self.assertEqual(len(groups), 13)
        self.assertEqual(len(verse_lines), 359)
        self.assertEqual(len(physical), 13071)
        self.assertEqual(len(physical), len({row["physical_line_unit_id"] for row in physical}))

    def test_full_challenger_comparison_and_rebuild_parity(self) -> None:
        comparison = json.loads((ROUTE / "boundary-comparison.v2.json").read_text())
        self.assertEqual(comparison["evaluated_adjacent_pairs"], 13070)
        self.assertEqual(comparison["disagreement_count"], 358)
        self.assertEqual(comparison["unresolved_disagreement_count"], 0)
        self.assertEqual(sum(comparison["all_pair_results"].values()), 13070)
        if not SOURCE_PDF.is_file():
            self.skipTest("exact local Antonovsky 1911 PDF is not present")
        completed = subprocess.run(
            [sys.executable, "scripts/build_antonovsky_1911_structural_paragraph_v2.py", "--check"],
            cwd=REPO, capture_output=True, text=True)
        self.assertEqual(completed.returncode, 0,
                         msg=f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}")


if __name__ == "__main__":
    unittest.main()
