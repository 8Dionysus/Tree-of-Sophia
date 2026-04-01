from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_ROOT = REPO_ROOT / "scripts"
if str(SCRIPTS_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_ROOT))

import validate_kag_export


def write_text(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8")


class ValidateQuestbookSurfaceTestCase(unittest.TestCase):
    def write_valid_surface(self, repo_root: Path) -> None:
        write_text(
            repo_root / "QUESTBOOK.md",
            (REPO_ROOT / "QUESTBOOK.md").read_text(encoding="utf-8"),
        )
        write_text(
            repo_root / "docs" / "QUESTBOOK_TOS_INTEGRATION.md",
            (REPO_ROOT / "docs" / "QUESTBOOK_TOS_INTEGRATION.md").read_text(
                encoding="utf-8"
            ),
        )
        write_text(
            repo_root / "schemas" / "quest.schema.json",
            (REPO_ROOT / "schemas" / "quest.schema.json").read_text(encoding="utf-8"),
        )
        write_text(
            repo_root / "schemas" / "quest_dispatch.schema.json",
            (REPO_ROOT / "schemas" / "quest_dispatch.schema.json").read_text(
                encoding="utf-8"
            ),
        )
        for quest_id in validate_kag_export.QUEST_IDS:
            write_text(
                repo_root / "quests" / f"{quest_id}.yaml",
                (REPO_ROOT / "quests" / f"{quest_id}.yaml").read_text(
                    encoding="utf-8"
                ),
            )
        write_text(
            repo_root / "examples" / "quest_catalog.min.example.json",
            (REPO_ROOT / "examples" / "quest_catalog.min.example.json").read_text(
                encoding="utf-8"
            ),
        )
        write_text(
            repo_root / "examples" / "quest_dispatch.min.example.json",
            (REPO_ROOT / "examples" / "quest_dispatch.min.example.json").read_text(
                encoding="utf-8"
            ),
        )

    def test_valid_questbook_surface_passes(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)

            with patch.object(validate_kag_export, "REPO_ROOT", repo_root):
                validate_kag_export.validate_questbook_surface()

    def test_missing_integration_doc_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            (repo_root / "docs" / "QUESTBOOK_TOS_INTEGRATION.md").unlink()

            with patch.object(validate_kag_export, "REPO_ROOT", repo_root):
                with self.assertRaises(validate_kag_export.ValidationError) as context:
                    validate_kag_export.validate_questbook_surface()

        self.assertIn("docs/QUESTBOOK_TOS_INTEGRATION.md", str(context.exception))

    def test_missing_quest_file_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            (repo_root / "quests" / "TOS-Q-0003.yaml").unlink()

            with patch.object(validate_kag_export, "REPO_ROOT", repo_root):
                with self.assertRaises(validate_kag_export.ValidationError) as context:
                    validate_kag_export.validate_questbook_surface()

        self.assertIn("TOS-Q-0003.yaml", str(context.exception))

    def test_wrong_repo_value_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            write_text(
                repo_root / "quests" / "TOS-Q-0002.yaml",
                (repo_root / "quests" / "TOS-Q-0002.yaml")
                .read_text(encoding="utf-8")
                .replace("repo: Tree-of-Sophia", "repo: aoa-kag"),
            )

            with patch.object(validate_kag_export, "REPO_ROOT", repo_root):
                with self.assertRaises(validate_kag_export.ValidationError) as context:
                    validate_kag_export.validate_questbook_surface()

        self.assertIn("repo must equal 'Tree-of-Sophia'", str(context.exception))

    def test_id_filename_mismatch_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            write_text(
                repo_root / "quests" / "TOS-Q-0004.yaml",
                (repo_root / "quests" / "TOS-Q-0004.yaml")
                .read_text(encoding="utf-8")
                .replace("id: TOS-Q-0004", "id: TOS-Q-9999"),
            )

            with patch.object(validate_kag_export, "REPO_ROOT", repo_root):
                with self.assertRaises(validate_kag_export.ValidationError) as context:
                    validate_kag_export.validate_questbook_surface()

        self.assertIn("id must equal 'TOS-Q-0004'", str(context.exception))

    def test_missing_public_safe_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            write_text(
                repo_root / "quests" / "TOS-Q-0001.yaml",
                (repo_root / "quests" / "TOS-Q-0001.yaml")
                .read_text(encoding="utf-8")
                .replace("public_safe: true", "public_safe: false"),
            )

            with patch.object(validate_kag_export, "REPO_ROOT", repo_root):
                with self.assertRaises(validate_kag_export.ValidationError) as context:
                    validate_kag_export.validate_questbook_surface()

        self.assertIn("public_safe must be true", str(context.exception))

    def test_missing_tracked_quest_reference_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            write_text(
                repo_root / "QUESTBOOK.md",
                (repo_root / "QUESTBOOK.md")
                .read_text(encoding="utf-8")
                .replace("`TOS-Q-0004`", "`TOS-Q-XXXX`"),
            )

            with patch.object(validate_kag_export, "REPO_ROOT", repo_root):
                with self.assertRaises(validate_kag_export.ValidationError) as context:
                    validate_kag_export.validate_questbook_surface()

        self.assertIn("TOS-Q-0004", str(context.exception))

    def test_operational_boundary_phrase_missing_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            write_text(
                repo_root / "docs" / "QUESTBOOK_TOS_INTEGRATION.md",
                (repo_root / "docs" / "QUESTBOOK_TOS_INTEGRATION.md")
                .read_text(encoding="utf-8")
                .replace(
                    "It is not the place where philosophical interpretation, authored knowledge, or source meaning becomes a task list.",
                    "It is a convenience layer for adjacent tasks.",
                ),
            )

            with patch.object(validate_kag_export, "REPO_ROOT", repo_root):
                with self.assertRaises(validate_kag_export.ValidationError) as context:
                    validate_kag_export.validate_questbook_surface()

        self.assertIn("philosophical interpretation, authored knowledge, or source meaning becomes a task list", str(context.exception))

    def test_example_projection_drift_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            write_text(
                repo_root / "examples" / "quest_dispatch.min.example.json",
                (repo_root / "examples" / "quest_dispatch.min.example.json")
                .read_text(encoding="utf-8")
                .replace(
                    '"source_path": "quests/TOS-Q-0004.yaml"',
                    '"source_path": "quests/TOS-Q-9999.yaml"',
                ),
            )

            with patch.object(validate_kag_export, "REPO_ROOT", repo_root):
                with self.assertRaises(validate_kag_export.ValidationError) as context:
                    validate_kag_export.validate_questbook_surface()

        self.assertIn("quest_dispatch.min.example.json", str(context.exception))


if __name__ == "__main__":
    unittest.main()
