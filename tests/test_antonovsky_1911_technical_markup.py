from __future__ import annotations

import json
import subprocess
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
ROUTE_ROOT = REPO_ROOT / (
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/technical-markup/"
    "antonovsky-1911-pdf-layout-v1"
)


class Antonovsky1911TechnicalMarkupTests(unittest.TestCase):
    def test_tracked_route_validates_without_private_source_layers(self) -> None:
        completed = subprocess.run(
            [
                sys.executable,
                "scripts/build_antonovsky_1911_technical_markup.py",
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

    def test_summary_preserves_the_proposed_nonsemantic_count_surface(self) -> None:
        summary = json.loads((ROUTE_ROOT / "summary.v1.json").read_text())
        self.assertEqual(summary["pdf_page_count"], 153)
        self.assertEqual(summary["observed_panel_count"], 297)
        self.assertEqual(summary["layout_block_paragraph_candidate_count"], 7225)
        self.assertEqual(summary["physical_line_counted_not_unitized"], 13071)
        self.assertEqual(summary["embedded_word_counted_not_unitized"], 97419)
        self.assertEqual(summary["tracked_unit_count"], 7676)
        self.assertEqual(summary["region_candidate_count"], 6)
        self.assertEqual(summary["heading_candidate_count"], 86)
        self.assertEqual(summary["segmentation_status"], "proposed")
        self.assertFalse(summary["accepted_russian_text"])
        self.assertFalse(summary["accepted_paragraphs"])
        self.assertFalse(summary["accepted_sections"])
        self.assertFalse(summary["source_target_alignment_created"])
        self.assertFalse(summary["german_numbering_copied"])
        self.assertFalse(summary["semantic_fields_materialized"])
        self.assertFalse(summary["source_text_included"])

    def test_citation_and_candidate_views_are_text_free_and_closed(self) -> None:
        citations = [
            json.loads(line)
            for line in (ROUTE_ROOT / "citation-spine.v1.jsonl")
            .read_text()
            .splitlines()
        ]
        headings = [
            json.loads(line)
            for line in (ROUTE_ROOT / "heading-candidates.v1.jsonl")
            .read_text()
            .splitlines()
        ]
        regions = [
            json.loads(line)
            for line in (ROUTE_ROOT / "region-candidates.v1.jsonl")
            .read_text()
            .splitlines()
        ]
        issuance = json.loads((ROUTE_ROOT / "identity-issuance.v1.json").read_text())

        self.assertEqual(len(citations), 7676)
        self.assertEqual(len(citations), len({row["unit_id"] for row in citations}))
        self.assertEqual(
            len(citations), len({row["display_citation"] for row in citations})
        )
        self.assertEqual(
            [row["source_locator"] for row in citations],
            [row["source_locator"] for row in issuance["units"]],
        )
        self.assertTrue(all(row["source_text_included"] is False for row in citations))
        self.assertTrue(all(row["semantic_promotion"] is False for row in citations))
        self.assertEqual(
            sum(row["structural_role"] == "source_attested_pdf_page" for row in citations),
            153,
        )
        self.assertEqual(
            sum(
                row["structural_role"] == "two_up_page_side_panel_candidate"
                for row in citations
            ),
            297,
        )
        self.assertEqual(sum(row["unit_kind"] == "paragraph" for row in citations), 7225)
        self.assertEqual(len(headings), 86)
        self.assertTrue(all(row["accepted_section"] is False for row in headings))
        self.assertTrue(all(row["source_text_included"] is False for row in headings))
        self.assertEqual(
            [row["region_candidate_id"] for row in regions],
            ["front_matter", "part_1", "part_2", "part_3", "part_4", "back_matter"],
        )
        self.assertEqual(
            [row["paragraph_candidate_count"] for row in regions],
            [323, 918, 1252, 2402, 2150, 180],
        )
        self.assertTrue(all(row["german_correspondence_created"] is False for row in regions))

    def test_model_screening_is_complete_proposed_and_not_a_packet_review(self) -> None:
        heading_screening = [
            json.loads(line)
            for line in (ROUTE_ROOT / "heading-screening.v1.jsonl")
            .read_text()
            .splitlines()
        ]
        paragraph_overlay = [
            json.loads(line)
            for line in (ROUTE_ROOT / "paragraph-segment-overlay.v1.jsonl")
            .read_text()
            .splitlines()
        ]
        screening_summary = json.loads(
            (ROUTE_ROOT / "screening-summary.v1.json").read_text()
        )
        packet = json.loads((ROUTE_ROOT / "source-text-unit.v1.json").read_text())
        provenance = [
            json.loads(line)
            for line in (ROUTE_ROOT / "provenance.jsonl").read_text().splitlines()
        ]

        self.assertEqual(len(heading_screening), 86)
        self.assertFalse(screening_summary["full_witness_heading_recall_claimed"])
        self.assertFalse(screening_summary["full_alignment_readiness_claimed"])
        self.assertEqual(
            [row["candidate_ordinal"] for row in heading_screening],
            list(range(1, 87)),
        )
        self.assertEqual(
            screening_summary["heading_technical_role_counts"],
            {
                "numbered_subsection_marker": 17,
                "part_title_heading": 1,
                "publisher_catalog_heading": 5,
                "reading_unit_heading_candidate": 57,
                "table_of_contents_heading": 1,
                "translator_preface_heading": 1,
                "work_subtitle_component": 1,
                "work_title_component_at_part_opening": 3,
            },
        )
        self.assertTrue(
            all(row["maker_kind"] == "model_source_visible" for row in heading_screening)
        )
        self.assertTrue(
            all(row["real_human_reviewer"] is False for row in heading_screening)
        )
        self.assertTrue(
            all(row["accepted_section"] is False for row in heading_screening)
        )

        self.assertEqual(screening_summary["affected_source_block_count"], 692)
        self.assertEqual(screening_summary["split_boundary_count"], 1372)
        self.assertEqual(screening_summary["proposed_segment_count"], 2064)
        self.assertEqual(
            screening_summary["effective_proposed_paragraph_candidate_count"],
            8597,
        )
        self.assertEqual(len(paragraph_overlay), 2064)
        self.assertEqual(
            len({row["source_block_unit_id"] for row in paragraph_overlay}), 692
        )
        self.assertTrue(
            all(row["accepted_paragraph"] is False for row in paragraph_overlay)
        )
        self.assertTrue(
            all(
                row["creates_new_text_unit_identity"] is False
                for row in paragraph_overlay
            )
        )
        self.assertTrue(all(row["source_text_included"] is False for row in paragraph_overlay))
        self.assertEqual(packet["reviews"], [])
        self.assertEqual(packet["segmentations"][0]["status"], "proposed")
        self.assertEqual(
            [row["event_type"] for row in provenance], ["segmentation", "annotation"]
        )

    def test_full_local_rebuild_parity_when_exact_pdf_is_present(self) -> None:
        plan = json.loads((ROUTE_ROOT / "plan.v1.json").read_text())
        manifest_path = REPO_ROOT / plan["source_item"]["manifest_ref"]
        manifest = json.loads(manifest_path.read_text())
        entry = next(
            row
            for row in manifest["payload_files"]
            if row["file_id"] == plan["source_item"]["file_ref"]
        )
        payload = manifest_path.parent / entry["relative_path"]
        if not payload.is_file():
            self.skipTest("exact local Antonovsky 1911 PDF is not present")
        completed = subprocess.run(
            [
                sys.executable,
                "scripts/build_antonovsky_1911_technical_markup.py",
                "--check",
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


if __name__ == "__main__":
    unittest.main()
