from __future__ import annotations

import json
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


def read_text(relative_path: str) -> str:
    return (REPO_ROOT / relative_path).read_text(encoding="utf-8")


def load_json(relative_path: str) -> object:
    return json.loads((REPO_ROOT / relative_path).read_text(encoding="utf-8"))


class RoadmapParityTestCase(unittest.TestCase):
    def test_roadmap_keeps_root_entry_routes_in_current_phase(self) -> None:
        roadmap = read_text("ROADMAP.md")
        readme = read_text("README.md")
        changelog = read_text("CHANGELOG.md")
        payload = load_json("generated/root_entry_map.min.json")

        self.assertIn("> Current release: `v0.2.0`", readme)
        self.assertIn("## [0.2.0] - 2026-04-10", changelog)
        self.assertIn("`v0.2.0`", roadmap)
        self.assertIn("Current release contour", roadmap)
        self.assertIn("source-owned root-entry and export hardening", roadmap)
        self.assertIn("routed-language witness handling and source-first re-entry", roadmap)

        route_ids = {route["route_id"] for route in payload["routes"]}

        self.assertIn("current-tiny-entry", route_ids)
        self.assertIn("bounded-export", route_ids)
        self.assertIn("shared `node_id`", roadmap)
        self.assertIn("tiny-entry seam", roadmap)
        self.assertIn("bounded export seam", roadmap)
        self.assertIn("`aoa-kag`", roadmap)

        current_release_surfaces = [
            "README.md",
            "CHARTER.md",
            "BOUNDARIES.md",
            "docs/KNOWLEDGE_MODEL.md",
            "docs/NODE_CONTRACT.md",
            "docs/REVIEW_CHECKLIST.md",
            "docs/TINY_ENTRY_ROUTE.md",
            "examples/tos_tiny_entry_route.example.json",
            "generated/root_entry_map.min.json",
            "scripts/build_root_entry_map.py",
            "scripts/validate_root_entry_map.py",
            "scripts/validate_tiny_entry_route.py",
            "docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md",
            "sources/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/Z_1_1_1_de_ru_en.md",
            "tree/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json",
            "examples/source_node.example.json",
            "tree/concept/becoming/node.json",
            "examples/concept_node.example.json",
            "docs/KAG_EXPORT.md",
            "generated/kag_export.json",
            "generated/kag_export.min.json",
            "scripts/generate_kag_export.py",
            "scripts/validate_kag_export.py",
        ]
        for surface in current_release_surfaces:
            with self.subTest(surface=surface):
                self.assertTrue((REPO_ROOT / surface).exists(), surface)
                self.assertIn(surface, roadmap)


if __name__ == "__main__":
    unittest.main()
