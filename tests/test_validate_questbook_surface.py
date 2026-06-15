from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS_ROOT = REPO_ROOT / "scripts"
if str(SCRIPTS_ROOT) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_ROOT))

import validate_questbook_surface as questbook_validator


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
            repo_root / questbook_validator.QUESTBOOK_INTEGRATION_PATH,
            (REPO_ROOT / questbook_validator.QUESTBOOK_INTEGRATION_PATH).read_text(encoding="utf-8"),
        )
        write_text(
            repo_root / questbook_validator.QUEST_SCHEMA_PATH,
            (REPO_ROOT / questbook_validator.QUEST_SCHEMA_PATH).read_text(encoding="utf-8"),
        )
        write_text(
            repo_root / questbook_validator.QUEST_DISPATCH_SCHEMA_PATH,
            (REPO_ROOT / questbook_validator.QUEST_DISPATCH_SCHEMA_PATH).read_text(encoding="utf-8"),
        )
        for quest_id in questbook_validator.QUEST_IDS:
            write_text(
                repo_root / "quests" / f"{quest_id}.yaml",
                (REPO_ROOT / "quests" / f"{quest_id}.yaml").read_text(
                    encoding="utf-8"
                ),
            )
        write_text(
            repo_root / questbook_validator.QUEST_CATALOG_EXAMPLE_PATH,
            (REPO_ROOT / questbook_validator.QUEST_CATALOG_EXAMPLE_PATH).read_text(encoding="utf-8"),
        )
        write_text(
            repo_root / questbook_validator.QUEST_DISPATCH_EXAMPLE_PATH,
            (REPO_ROOT / questbook_validator.QUEST_DISPATCH_EXAMPLE_PATH).read_text(encoding="utf-8"),
        )

    def test_valid_questbook_surface_passes(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)

            questbook_validator.validate_questbook_surface(repo_root)

    def test_missing_integration_doc_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            (repo_root / questbook_validator.QUESTBOOK_INTEGRATION_PATH).unlink()

            with self.assertRaises(questbook_validator.ValidationError) as context:
                questbook_validator.validate_questbook_surface(repo_root)

        self.assertIn("mechanics/questbook/parts/obligation-boundary/docs/QUESTBOOK_TOS_INTEGRATION.md", str(context.exception))

    def test_missing_quest_file_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            (repo_root / "quests" / "TOS-Q-0003.yaml").unlink()

            with self.assertRaises(questbook_validator.ValidationError) as context:
                questbook_validator.validate_questbook_surface(repo_root)

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

            with self.assertRaises(questbook_validator.ValidationError) as context:
                questbook_validator.validate_questbook_surface(repo_root)

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

            with self.assertRaises(questbook_validator.ValidationError) as context:
                questbook_validator.validate_questbook_surface(repo_root)

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

            with self.assertRaises(questbook_validator.ValidationError) as context:
                questbook_validator.validate_questbook_surface(repo_root)

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

            with self.assertRaises(questbook_validator.ValidationError) as context:
                questbook_validator.validate_questbook_surface(repo_root)

        self.assertIn("TOS-Q-0004", str(context.exception))

    def test_operational_boundary_phrase_missing_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            write_text(
                repo_root / questbook_validator.QUESTBOOK_INTEGRATION_PATH,
                (repo_root / questbook_validator.QUESTBOOK_INTEGRATION_PATH)
                .read_text(encoding="utf-8")
                .replace(
                    "It is not the place where philosophical interpretation, authored knowledge, or source meaning becomes a task list.",
                    "It is a convenience layer for adjacent tasks.",
                ),
            )

            with self.assertRaises(questbook_validator.ValidationError) as context:
                questbook_validator.validate_questbook_surface(repo_root)

        self.assertIn("philosophical interpretation, authored knowledge, or source meaning becomes a task list", str(context.exception))

    def test_example_projection_drift_fails(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            write_text(
                repo_root / questbook_validator.QUEST_DISPATCH_EXAMPLE_PATH,
                (repo_root / questbook_validator.QUEST_DISPATCH_EXAMPLE_PATH)
                .read_text(encoding="utf-8")
                .replace(
                    '"source_path": "quests/TOS-Q-0004.yaml"',
                    '"source_path": "quests/TOS-Q-9999.yaml"',
                ),
            )

            with self.assertRaises(questbook_validator.ValidationError) as context:
                questbook_validator.validate_questbook_surface(repo_root)

        self.assertIn("quest_dispatch.min.example.json", str(context.exception))

    def test_missing_activation_mode_fails_without_key_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            quest_path = repo_root / "quests" / "TOS-Q-0001.yaml"
            write_text(
                quest_path,
                quest_path.read_text(encoding="utf-8").replace(
                    "activation:\n  mode: immediate\n",
                    "activation: {}\n",
                ),
            )

            with self.assertRaises(questbook_validator.ValidationError) as context:
                questbook_validator.validate_questbook_surface(repo_root)

        message = str(context.exception)
        self.assertIn("activation", message)
        self.assertIn("mode", message)

    def test_dispatch_example_schema_violation_fails_before_projection_match(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            repo_root = Path(tmpdir) / "Tree-of-Sophia"
            self.write_valid_surface(repo_root)
            dispatch_path = repo_root / questbook_validator.QUEST_DISPATCH_EXAMPLE_PATH
            write_text(
                dispatch_path,
                dispatch_path.read_text(encoding="utf-8").replace(
                    '"activation_mode": "immediate"',
                    '"activation_mode": "not-a-valid-mode"',
                    1,
                ),
            )

            with self.assertRaises(questbook_validator.ValidationError) as context:
                questbook_validator.validate_questbook_surface(repo_root)

        self.assertIn("quest_dispatch.schema.json", str(context.exception))

    def test_build_dispatch_entry_reports_missing_activation_mode(self) -> None:
        quest_payload = questbook_validator.read_yaml(REPO_ROOT / "quests" / "TOS-Q-0001.yaml")
        self.assertIsInstance(quest_payload, dict)
        quest_payload["activation"] = {}

        with self.assertRaises(questbook_validator.ValidationError) as context:
            questbook_validator.build_expected_quest_dispatch_entry(
                "TOS-Q-0001",
                quest_payload,
            )

        self.assertIn("activation.mode", str(context.exception))


if __name__ == "__main__":
    unittest.main()
