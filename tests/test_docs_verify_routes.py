from __future__ import annotations

import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
README_PATH = REPO_ROOT / "README.md"
AGENTS_PATH = REPO_ROOT / "AGENTS.md"
CONTRIBUTING_PATH = REPO_ROOT / "CONTRIBUTING.md"
KAG_EXPORT_DOC_PATH = (
    REPO_ROOT / "mechanics" / "boundary-bridge" / "parts" / "derived-kag-seam" / "docs" / "KAG_EXPORT.md"
)


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


class DocsVerifyRoutesTestCase(unittest.TestCase):
    def test_readme_keeps_source_first_route_links_ahead_of_export_seam(self) -> None:
        readme = read_text(README_PATH)
        source_entry = "[ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE](ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md)"
        trilingual_entry = "[ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY](ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md)"
        export_seam = "[mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT](mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md)"

        self.assertIn(source_entry, readme)
        self.assertIn(trilingual_entry, readme)
        self.assertIn(export_seam, readme)
        self.assertLess(readme.index(source_entry), readme.index(export_seam))

    def test_readme_routes_validation_instead_of_carrying_commands(self) -> None:
        readme = read_text(README_PATH)

        self.assertIn("[AGENTS](AGENTS.md#verify)", readme)
        self.assertIn("[scripts](scripts/AGENTS.md)", readme)
        self.assertNotIn("python scripts/", readme)
        self.assertNotIn("python -m unittest", readme)

    def test_agents_validation_section_names_release_gate_and_local_routes(self) -> None:
        agents = read_text(AGENTS_PATH)

        self.assertIn("python scripts/release_check.py", agents)
        self.assertIn("python scripts/validate_tiny_entry_route.py", agents)
        self.assertIn("python scripts/validate_kag_export.py", agents)
        self.assertIn("mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md", agents)

    def test_contributing_routes_to_validation_owners(self) -> None:
        contributing = read_text(CONTRIBUTING_PATH)

        self.assertIn("AGENTS.md#verify", contributing)
        self.assertIn("scripts/AGENTS.md", contributing)
        self.assertIn("mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md", contributing)

    def test_kag_export_doc_separates_verification_from_regeneration(self) -> None:
        kag_export_doc = read_text(KAG_EXPORT_DOC_PATH)

        self.assertIn("## Current verification", kag_export_doc)
        self.assertIn("## Regeneration", kag_export_doc)
        self.assertLess(
            kag_export_doc.index("## Current verification"),
            kag_export_doc.index("## Regeneration"),
        )
        self.assertIn("python scripts/validate_kag_export.py", kag_export_doc)
        self.assertIn("python -m unittest discover -s tests", kag_export_doc)
        self.assertIn("python scripts/generate_kag_export.py", kag_export_doc)

    def test_readme_and_tiny_entry_doc_expose_root_entry_capsule(self) -> None:
        readme = read_text(README_PATH)
        tiny_entry_doc = read_text(REPO_ROOT / "ToS" / "zarathustra" / "public-entry" / "TINY_ENTRY_ROUTE.md")

        self.assertIn("ToS/derived-exports/root_entry_map.min.json", readme)
        self.assertIn("ToS/derived-exports/root_entry_map.min.json", tiny_entry_doc)
        self.assertIn("python scripts/build_root_entry_map.py --check", tiny_entry_doc)
        self.assertIn("python scripts/validate_root_entry_map.py", tiny_entry_doc)


if __name__ == "__main__":
    unittest.main()
