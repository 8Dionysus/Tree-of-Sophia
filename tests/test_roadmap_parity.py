from __future__ import annotations

import json
import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]


def read_text(relative_path: str) -> str:
    return (REPO_ROOT / relative_path).read_text(encoding="utf-8")


def load_json(relative_path: str) -> object:
    return json.loads((REPO_ROOT / relative_path).read_text(encoding="utf-8"))


def current_release(readme: str) -> str:
    match = re.search(r"Current release: `([^`]+)`", readme)
    if match is None:
        raise AssertionError("README.md must name the current release")
    return match.group(1)


class RoadmapParityTestCase(unittest.TestCase):
    def test_roadmap_keeps_root_entry_routes_in_current_phase(self) -> None:
        roadmap = read_text("ROADMAP.md")
        readme = read_text("README.md")
        changelog = read_text("CHANGELOG.md")
        payload = load_json("ToS/derived-exports/root_entry_map.min.json")
        release = current_release(readme)

        self.assertIn(f"## [{release.removeprefix('v')}]", changelog)
        self.assertIn(f"`{release}`", roadmap)
        self.assertIn("Current release contour", roadmap)

        route_ids = {route["route_id"] for route in payload["routes"]}

        self.assertIn("current-tiny-entry", route_ids)
        self.assertIn("bounded-export", route_ids)
        for term in ("root-entry", "tiny-entry", "bounded export", "aoa-kag"):
            with self.subTest(roadmap_term=term):
                self.assertIn(term, roadmap)

        current_release_surfaces = [
            "README.md",
            "CHARTER.md",
            "BOUNDARIES.md",
            "ToS/doctrine/KNOWLEDGE_MODEL.md",
            "ToS/doctrine/NODE_CONTRACT.md",
            "mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md",
            "ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md",
            "ToS/public-compatibility/tos_tiny_entry_route.example.json",
            "ToS/derived-exports/root_entry_map.min.json",
            "ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md",
            "ToS/source-witnesses/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/Z_1_1_1_de_ru_en.md",
            "ToS/canon/source/friedrich-nietzsche/thus-spoke-zarathustra/prologue-1/node.json",
            "ToS/public-compatibility/source_node.example.json",
            "ToS/canon/concept/becoming/node.json",
            "ToS/public-compatibility/concept_node.example.json",
            "mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md",
            "ToS/derived-exports/kag_export.json",
            "ToS/derived-exports/kag_export.min.json",
        ]
        for surface in current_release_surfaces:
            with self.subTest(surface=surface):
                self.assertTrue((REPO_ROOT / surface).exists(), surface)

        indexed_surfaces = {
            payload["authority_ref"],
            payload["public_root_ref"],
            payload["current_tiny_entry_ref"],
            payload["export_ref"],
        }
        for route in payload["routes"]:
            indexed_surfaces.add(route["surface_ref"])
            indexed_surfaces.update(route.get("verification_refs", []))

        for surface in indexed_surfaces:
            with self.subTest(indexed_surface=surface):
                self.assertTrue((REPO_ROOT / surface).exists(), surface)


if __name__ == "__main__":
    unittest.main()
