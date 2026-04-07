from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_ROOT = REPO_ROOT / "scripts"
if str(SCRIPTS_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_ROOT))

import validate_tiny_entry_route


def write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


class ValidateTinyEntryRouteTestCase(unittest.TestCase):
    def write_valid_surface(self, repo_root: Path) -> None:
        for relative_path in (
            Path("README.md"),
            Path("CHARTER.md"),
            Path("docs") / "TINY_ENTRY_ROUTE.md",
            Path("docs") / "ZARATHUSTRA_TRILINGUAL_ENTRY.md",
            Path("docs") / "KNOWLEDGE_MODEL.md",
            Path("docs") / "REVIEW_CHECKLIST.md",
            Path("examples") / "source_node.example.json",
            Path("examples") / "concept_node.example.json",
            Path("examples") / "tos_tiny_entry_route.example.json",
        ):
            write_text(
                repo_root / relative_path,
                (REPO_ROOT / relative_path).read_text(encoding="utf-8"),
            )

    def test_valid_tiny_entry_surface_passes(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)

            issues = validate_tiny_entry_route.run_validation(repo_root)

        self.assertEqual(issues, [])

    def test_root_surface_drift_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            route_path = repo_root / "examples" / "tos_tiny_entry_route.example.json"
            payload = json.loads(route_path.read_text(encoding="utf-8"))
            payload["root_surface"] = "docs/TINY_ENTRY_ROUTE.md"
            write_text(route_path, json.dumps(payload, ensure_ascii=False, indent=2) + "\n")

            issues = validate_tiny_entry_route.run_validation(repo_root)

        self.assertTrue(
            any("root_surface must stay 'README.md'" in message for _, message in issues)
        )

    def test_legacy_hop_mismatch_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            route_path = repo_root / "examples" / "tos_tiny_entry_route.example.json"
            payload = json.loads(route_path.read_text(encoding="utf-8"))
            payload["lineage_or_context_hop"] = "docs/TINY_ENTRY_ROUTE.md"
            write_text(route_path, json.dumps(payload, ensure_ascii=False, indent=2) + "\n")

            issues = validate_tiny_entry_route.run_validation(repo_root)

        self.assertTrue(
            any("lineage_or_context_hop must match bounded_hop" in message for _, message in issues)
        )

    def test_route_doc_missing_reentry_phrase_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            doc_path = repo_root / "docs" / "TINY_ENTRY_ROUTE.md"
            write_text(
                doc_path,
                doc_path.read_text(encoding="utf-8").replace(
                    "## Source-first re-entry",
                    "## Downstream restore",
                ),
            )

            issues = validate_tiny_entry_route.run_validation(repo_root)

        self.assertTrue(
            any("## Source-first re-entry" in message for _, message in issues)
        )

    def test_review_checklist_missing_validator_phrase_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            checklist_path = repo_root / "docs" / "REVIEW_CHECKLIST.md"
            write_text(
                checklist_path,
                checklist_path.read_text(encoding="utf-8").replace(
                    "python scripts/validate_tiny_entry_route.py",
                    "python scripts/validate_current_route.py",
                ),
            )

            issues = validate_tiny_entry_route.run_validation(repo_root)

        self.assertTrue(
            any("python scripts/validate_tiny_entry_route.py" in message for _, message in issues)
        )


if __name__ == "__main__":
    unittest.main()
