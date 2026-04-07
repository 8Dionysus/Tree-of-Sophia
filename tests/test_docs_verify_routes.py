from __future__ import annotations

import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
README_PATH = REPO_ROOT / "README.md"
AGENTS_PATH = REPO_ROOT / "AGENTS.md"
CONTRIBUTING_PATH = REPO_ROOT / "CONTRIBUTING.md"
KAG_EXPORT_DOC_PATH = REPO_ROOT / "docs" / "KAG_EXPORT.md"


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


class DocsVerifyRoutesTestCase(unittest.TestCase):
    def test_readme_keeps_source_first_route_ahead_of_export_seam(self) -> None:
        readme = read_text(README_PATH)
        source_route = (
            "if you are new here and want the one real current public route: "
            "[docs/TINY_ENTRY_ROUTE](docs/TINY_ENTRY_ROUTE.md) and "
            "[docs/ZARATHUSTRA_TRILINGUAL_ENTRY](docs/ZARATHUSTRA_TRILINGUAL_ENTRY.md)"
        )
        export_seam = (
            "if you need the bounded downstream export seam for that route: "
            "[docs/KAG_EXPORT](docs/KAG_EXPORT.md)"
        )

        self.assertIn(source_route, readme)
        self.assertIn(export_seam, readme)
        self.assertLess(readme.index(source_route), readme.index(export_seam))

    def test_readme_verify_path_mentions_validator_tests_and_manual_review(self) -> None:
        readme = read_text(README_PATH)

        self.assertIn("python scripts/validate_tiny_entry_route.py", readme)
        self.assertIn("python scripts/validate_kag_export.py", readme)
        self.assertIn("python -m unittest discover -s tests", readme)
        self.assertIn("docs/REVIEW_CHECKLIST.md", readme)

    def test_agents_validation_section_mentions_full_bounded_battery(self) -> None:
        agents = read_text(AGENTS_PATH)

        self.assertIn("The current bounded read-only battery is:", agents)
        self.assertIn("python scripts/validate_tiny_entry_route.py", agents)
        self.assertIn("python scripts/validate_kag_export.py", agents)
        self.assertIn("python -m unittest discover -s tests", agents)
        self.assertIn("docs/REVIEW_CHECKLIST.md", agents)

    def test_contributing_mentions_tests_alongside_validator(self) -> None:
        contributing = read_text(CONTRIBUTING_PATH)

        self.assertIn("python scripts/validate_tiny_entry_route.py", contributing)
        self.assertIn("python scripts/validate_kag_export.py", contributing)
        self.assertIn("python -m unittest discover -s tests", contributing)
        self.assertIn("docs/REVIEW_CHECKLIST.md", contributing)

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


if __name__ == "__main__":
    unittest.main()
