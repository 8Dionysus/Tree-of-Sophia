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

import validate_mechanics_topology  # noqa: E402


def write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


class MechanicsRouteDocsTests(unittest.TestCase):
    def test_current_mechanics_route_docs_pass(self) -> None:
        self.assertEqual(validate_mechanics_topology.run_validation(REPO_ROOT), [])

    def test_missing_active_part_from_selection_map_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            write_text(
                repo_root / "mechanics" / "README.md",
                "## Task-to-owner map\n## Progressive disclosure\n"
                "topology.json validation_lanes.json docs/decisions/README.md\n"
                "[agon](agon/README.md)\n",
            )
            write_text(
                repo_root / "mechanics" / "agon" / "README.md",
                "PARTS.md PROVENANCE.md ROADMAP.md",
            )
            write_text(repo_root / "mechanics" / "agon" / "PARTS.md", "# Parts\n")

            issues: list[tuple[str, str]] = []
            validate_mechanics_topology.validate_route_map(
                repo_root,
                issues,
                {"agon": {}},
                {"agon": {"threshold-intake"}},
            )

        self.assertTrue(any("selection map" in message for _, message in issues))

    def test_stale_executable_reference_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            write_text(
                repo_root / "docs" / "validation" / "script_inventory.json",
                json.dumps({"script_surfaces": []}) + "\n",
            )
            write_text(
                repo_root / "mechanics" / "agon" / "parts" / "threshold-intake" / "README.md",
                "Run `python mechanics/agon/parts/threshold-intake/scripts/missing.py`.\n",
            )

            issues: list[tuple[str, str]] = []
            validate_mechanics_topology.validate_documentation_references(repo_root, issues)

        self.assertTrue(any("stale executable reference" in message for _, message in issues))

    def test_always_on_context_budget_is_enforced(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            write_text(repo_root / "mechanics" / "README.md", "one two three\n")

            issues: list[tuple[str, str]] = []
            validate_mechanics_topology.validate_context_budget(
                repo_root,
                issues,
                {
                    "always_on_context_budget": {
                        "surface": "mechanics/README.md",
                        "metric": "whitespace_tokens_v1",
                        "max_tokens": 2,
                    }
                },
            )

        self.assertTrue(any("always-on context exceeds" in message for _, message in issues))


if __name__ == "__main__":
    unittest.main()
