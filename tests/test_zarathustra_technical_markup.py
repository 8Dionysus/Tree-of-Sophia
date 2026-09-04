from __future__ import annotations

import json
import subprocess
import sys
import unittest
from collections import defaultdict
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
ROUTE_ROOT = REPO_ROOT / (
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/technical-markup/"
    "dta-first-editions-parts-1-4-v1"
)
PARALLEL_READINESS = ROUTE_ROOT.parent / "parallel-technical-readiness.v1.json"


class ZarathustraTechnicalMarkupTests(unittest.TestCase):
    def test_tracked_route_validates_without_private_source_layers(self) -> None:
        completed = subprocess.run(
            [
                sys.executable,
                "scripts/build_zarathustra_technical_markup.py",
                "--validate-tracked",
            ],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(
            completed.returncode,
            0,
            msg=f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}",
        )

    def test_generated_summary_holds_the_declared_nonsemantic_counts(self) -> None:
        summary = json.loads((ROUTE_ROOT / "summary.v1.json").read_text())
        self.assertEqual(summary["part_count"], 4)
        self.assertEqual(summary["major_structure_candidate_count"], 82)
        self.assertEqual(summary["major_reading_unit_candidate_count"], 81)
        self.assertEqual(summary["major_wrapper_candidate_count"], 1)
        self.assertEqual(summary["source_tag_counts"]["div"], 198)
        self.assertEqual(summary["unit_kind_counts"]["paragraph"], 3447)
        self.assertEqual(summary["unit_kind_counts"]["verse_group"], 39)
        self.assertEqual(summary["unit_kind_counts"]["verse_line"], 368)
        self.assertTrue(summary["display_citations_unique"])
        self.assertFalse(summary["source_text_included"])
        self.assertFalse(summary["semantic_fields_materialized"])
        self.assertFalse(summary["russian_parallel_structure_materialized"])

        verification = json.loads(
            (ROUTE_ROOT / "agent-verification.v1.json").read_text()
        )
        self.assertEqual(
            verification["verification_status"],
            "agent_verified_complete_technical",
        )
        self.assertEqual(
            verification["source_basis"]["major_reading_units_by_part"],
            {"I": 23, "II": 22, "III": 16, "IV": 20},
        )
        self.assertEqual(
            verification["source_basis"]["nested_section_count"], 116
        )
        self.assertEqual(
            verification["source_basis"]["paragraphs_outside_major_reading_units"],
            9,
        )
        self.assertTrue(all(verification["checks"].values()))
        self.assertFalse(
            verification["authority_boundary"]["editorial_hierarchy_accepted"]
        )
        self.assertFalse(
            verification["authority_boundary"]["linguistic_or_semantic_authority"]
        )

    def test_citation_spine_is_text_free_and_identity_preserving(self) -> None:
        rows = [
            json.loads(line)
            for line in (ROUTE_ROOT / "citation-spine.v1.jsonl")
            .read_text()
            .splitlines()
        ]
        self.assertTrue(rows)
        self.assertEqual(len(rows), len({row["unit_id"] for row in rows}))
        self.assertEqual(
            len(rows), len({row["display_citation"] for row in rows})
        )
        self.assertTrue(all(row["source_text_included"] is False for row in rows))
        self.assertTrue(all(row["semantic_promotion"] is False for row in rows))
        sections = [row for row in rows if row["unit_kind"] == "section"]
        self.assertTrue(sections)
        self.assertTrue(
            all(
                row["descendant_paragraph_count"] >= row["direct_paragraph_count"]
                and row["descendant_verse_line_count"]
                >= row["direct_verse_line_count"]
                for row in sections
            )
        )
        paragraphs = [row for row in rows if row["unit_kind"] == "paragraph"]
        self.assertEqual(len(paragraphs), 3447)
        self.assertTrue(
            all(
                row["nearest_major_unit_id"] is None
                or row["ordinal_within_nearest_major_kind"] is not None
                for row in paragraphs
            )
        )
        excluded = {"tei-pb-0143", "tei-pb-0144", "tei-div-0070"}
        observed_resources = {
            row["source_resource_id"]
            for row in rows
            if row["part_label"] == "IV" and row["source_resource_id"] is not None
        }
        self.assertTrue(excluded.isdisjoint(observed_resources))

    def test_all_part_hierarchies_and_ordinals_are_closed(self) -> None:
        rows = [
            json.loads(line)
            for line in (ROUTE_ROOT / "citation-spine.v1.jsonl")
            .read_text()
            .splitlines()
        ]
        by_part = defaultdict(list)
        for row in rows:
            by_part[row["part_label"]].append(row)

        self.assertEqual(
            {
                part: sum(
                    row["structural_role"] == "major_reading_unit_candidate"
                    for row in part_rows
                )
                for part, part_rows in by_part.items()
            },
            {"I": 23, "II": 22, "III": 16, "IV": 20},
        )
        self.assertEqual(
            sum(
                row["unit_kind"] == "paragraph"
                and row["nearest_major_unit_id"] is None
                for row in rows
            ),
            9,
        )

        ordinal_groups = defaultdict(list)
        for row in rows:
            display_class = (
                row["source_tag"]
                if row["unit_kind"] in {"milestone", "other"}
                else row["unit_kind"]
            )
            ordinal_groups[
                (row["part_label"], row["parent_unit_id"], display_class)
            ].append(row["ordinal_within_parent_kind"])
        for key, ordinals in ordinal_groups.items():
            self.assertEqual(
                sorted(ordinals),
                list(range(1, len(ordinals) + 1)),
                msg=f"non-contiguous sibling ordinals for {key}",
            )

        paragraph_groups = defaultdict(list)
        for row in rows:
            if row["unit_kind"] != "paragraph" or row["nearest_major_unit_id"] is None:
                continue
            paragraph_groups[
                (row["part_label"], row["nearest_major_unit_id"])
            ].append(row["ordinal_within_nearest_major_kind"])
        self.assertEqual(len(paragraph_groups), 81)
        for key, ordinals in paragraph_groups.items():
            self.assertEqual(
                sorted(ordinals),
                list(range(1, len(ordinals) + 1)),
                msg=f"non-contiguous paragraph ordinals for {key}",
            )

        for part_order in range(1, 5):
            packet = json.loads(
                (ROUTE_ROOT / f"part-{part_order}.source-text-unit.v1.json").read_text()
            )
            units = {row["unit_id"]: row for row in packet["units"]}
            anchors = {row["anchor_ref"]: row for row in packet["anchors"]}
            self.assertEqual(len(units), len(packet["units"]))
            self.assertEqual(len(anchors), len(packet["anchors"]))

            for unit_id, unit in units.items():
                for parent_id in unit["parent_unit_refs"]:
                    self.assertIn(parent_id, units)
                    self.assertIn(unit_id, units[parent_id]["ordered_child_unit_refs"])
                for child_id in unit["ordered_child_unit_refs"]:
                    self.assertIn(child_id, units)
                    self.assertIn(unit_id, units[child_id]["parent_unit_refs"])

                if unit["ordered_child_unit_refs"]:
                    parent_start = min(
                        anchors[ref]["selector"]["start"]
                        for ref in unit["ordered_anchor_refs"]
                    )
                    parent_end = max(
                        anchors[ref]["selector"]["end"]
                        for ref in unit["ordered_anchor_refs"]
                    )
                    child_ranges = []
                    for child_id in unit["ordered_child_unit_refs"]:
                        child = units[child_id]
                        child_start = min(
                            anchors[ref]["selector"]["start"]
                            for ref in child["ordered_anchor_refs"]
                        )
                        child_end = max(
                            anchors[ref]["selector"]["end"]
                            for ref in child["ordered_anchor_refs"]
                        )
                        self.assertGreaterEqual(child_start, parent_start)
                        self.assertLessEqual(child_end, parent_end)
                        child_ranges.append((child_start, child_end))
                    self.assertEqual(
                        child_ranges,
                        sorted(child_ranges),
                        msg=f"non-monotonic children for {unit_id}",
                    )

            visiting: set[str] = set()
            visited: set[str] = set()

            def visit(unit_id: str) -> None:
                self.assertNotIn(unit_id, visiting, msg=f"cycle at {unit_id}")
                if unit_id in visited:
                    return
                visiting.add(unit_id)
                for child_id in units[unit_id]["ordered_child_unit_refs"]:
                    visit(child_id)
                visiting.remove(unit_id)
                visited.add(unit_id)

            for unit_id in units:
                visit(unit_id)
            self.assertEqual(visited, set(units))

    def test_parallel_readiness_keeps_witnesses_separate(self) -> None:
        readiness = json.loads(PARALLEL_READINESS.read_text())
        self.assertEqual(
            readiness["verification_status"],
            "agent_verified_complete_technical_parallel",
        )
        german = readiness["witnesses"]["german_dta_first_editions"]
        russian = readiness["witnesses"]["russian_antonovsky_1911"]
        self.assertEqual(german["reading_unit_count"], 81)
        self.assertEqual(russian["reading_unit_count"], 81)
        self.assertEqual(
            german["reading_units_by_part"], russian["reading_units_by_part"]
        )
        self.assertTrue(all(readiness["checks"].values()))
        self.assertTrue(readiness["authority_boundary"]["technical_parallelism"])
        self.assertFalse(
            readiness["authority_boundary"]["translation_alignment_created"]
        )
        self.assertFalse(
            readiness["authority_boundary"]["ordinal_equality_creates_correspondence"]
        )
        self.assertFalse(
            readiness["authority_boundary"]["linguistic_or_semantic_authority"]
        )

    def test_full_local_rebuild_parity_when_exact_payloads_are_present(self) -> None:
        plan = json.loads((ROUTE_ROOT / "plan.v1.json").read_text())
        payloads = []
        for source in plan["source_items"]:
            manifest_path = REPO_ROOT / source["manifest_ref"]
            manifest = json.loads(manifest_path.read_text())
            entry = next(
                row
                for row in manifest["payload_files"]
                if row["file_id"] == source["file_ref"]
            )
            payloads.append(manifest_path.parent / entry["relative_path"])
        if not all(path.is_file() for path in payloads):
            self.skipTest("exact local DTA payloads are not present")
        completed = subprocess.run(
            [sys.executable, "scripts/build_zarathustra_technical_markup.py", "--check"],
            cwd=REPO_ROOT,
            check=False,
            capture_output=True,
            text=True,
        )
        self.assertEqual(
            completed.returncode,
            0,
            msg=f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}",
        )


if __name__ == "__main__":
    unittest.main()
