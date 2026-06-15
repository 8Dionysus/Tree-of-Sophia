from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
VALIDATION_LANES_PATH = REPO_ROOT / "scripts" / "validation_lanes.py"
RELEASE_CHECK_PATH = REPO_ROOT / "scripts" / "release_check.py"


def load_module(name: str, path: Path) -> object:
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"unable to load {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


validation_lanes = load_module("validation_lanes", VALIDATION_LANES_PATH)
release_check_source = RELEASE_CHECK_PATH.read_text(encoding="utf-8")


class ValidationLanesTestCase(unittest.TestCase):
    def test_lane_manifest_validates(self) -> None:
        self.assertEqual(validation_lanes.validate_manifest(REPO_ROOT), [])

    def test_release_check_has_no_hidden_command_list(self) -> None:
        self.assertNotIn("COMMANDS =", release_check_source)
        self.assertIn("command_sequence(RELEASE_SEQUENCE", release_check_source)

    def test_source_home_uses_lane_ids_not_shell_commands(self) -> None:
        source_home = json.loads((REPO_ROOT / "ToS" / "source_home.manifest.json").read_text(encoding="utf-8"))
        lanes = json.loads((REPO_ROOT / "docs" / "validation" / "validation_lanes.json").read_text(encoding="utf-8"))[
            "lanes"
        ]

        for branch in source_home["branches"]:
            self.assertNotIn("validators", branch)
            self.assertIn("validation_lanes", branch)
            for lane_id in branch["validation_lanes"]:
                self.assertIn(lane_id, lanes)
                self.assertNotIn("python ", lane_id)


if __name__ == "__main__":
    unittest.main()
