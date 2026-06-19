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
TINY_ENTRY_ROUTE = "ToS/zarathustra/public-entry/TINY_ENTRY_ROUTE.md"
TRILINGUAL_ENTRY = "ToS/zarathustra/prologue-1/TRILINGUAL_ENTRY.md"
KAG_EXPORT_DOC = "mechanics/boundary-bridge/parts/derived-kag-seam/docs/KAG_EXPORT.md"
KAG_EXPORT_VALIDATOR = "mechanics/boundary-bridge/parts/derived-kag-seam/scripts/validate_kag_export.py"
KAG_EXPORT_BUILDER = "mechanics/boundary-bridge/parts/derived-kag-seam/scripts/generate_kag_export.py"
ROOT_ENTRY_MAP = "ToS/derived-exports/root_entry_map.min.json"
ROOT_ENTRY_BUILDER = "scripts/build_root_entry_map.py"
ROOT_ENTRY_VALIDATOR = "scripts/validate_root_entry_map.py"
REVIEW_CHECKLIST = "mechanics/audit/parts/review-ledger-route/docs/REVIEW_CHECKLIST.md"


def read_text(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def assert_route_refs(testcase: unittest.TestCase, text: str, *refs: str) -> None:
    for ref in refs:
        with testcase.subTest(ref=ref):
            testcase.assertIn(ref, text)
            testcase.assertTrue((REPO_ROOT / ref).exists(), ref)


class DocsVerifyRoutesTestCase(unittest.TestCase):
    def test_readme_keeps_source_first_route_links_ahead_of_export_seam(self) -> None:
        readme = read_text(README_PATH)

        assert_route_refs(self, readme, TINY_ENTRY_ROUTE, TRILINGUAL_ENTRY, KAG_EXPORT_DOC)
        self.assertLess(readme.index(TINY_ENTRY_ROUTE), readme.index(KAG_EXPORT_DOC))

    def test_readme_routes_validation_instead_of_carrying_commands(self) -> None:
        readme = read_text(README_PATH)

        self.assertIn("[AGENTS](AGENTS.md#verify)", readme)
        self.assertIn("[scripts](scripts/AGENTS.md)", readme)
        self.assertNotIn("python scripts/", readme)
        self.assertNotIn("python -m unittest", readme)

    def test_agents_validation_section_names_release_gate_and_local_routes(self) -> None:
        agents = read_text(AGENTS_PATH)

        assert_route_refs(
            self,
            agents,
            "scripts/release_check.py",
            "scripts/validate_tiny_entry_route.py",
            KAG_EXPORT_VALIDATOR,
            REVIEW_CHECKLIST,
        )

    def test_contributing_routes_to_validation_owners(self) -> None:
        contributing = read_text(CONTRIBUTING_PATH)

        self.assertIn("AGENTS.md#verify", contributing)
        assert_route_refs(self, contributing, "scripts/AGENTS.md", REVIEW_CHECKLIST)

    def test_kag_export_doc_separates_verification_from_regeneration(self) -> None:
        kag_export_doc = read_text(KAG_EXPORT_DOC_PATH)

        self.assertIn("## Current verification", kag_export_doc)
        self.assertIn("## Regeneration", kag_export_doc)
        self.assertLess(
            kag_export_doc.index("## Current verification"),
            kag_export_doc.index("## Regeneration"),
        )
        assert_route_refs(self, kag_export_doc, KAG_EXPORT_VALIDATOR, KAG_EXPORT_BUILDER)

    def test_readme_and_tiny_entry_doc_expose_root_entry_capsule(self) -> None:
        readme = read_text(README_PATH)
        tiny_entry_doc = read_text(REPO_ROOT / "ToS" / "zarathustra" / "public-entry" / "TINY_ENTRY_ROUTE.md")

        assert_route_refs(self, readme, ROOT_ENTRY_MAP)
        assert_route_refs(self, tiny_entry_doc, ROOT_ENTRY_MAP, ROOT_ENTRY_BUILDER, ROOT_ENTRY_VALIDATOR)


if __name__ == "__main__":
    unittest.main()
