from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import sys
import unittest
from unittest import mock


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
with mock.patch.object(sys, "path", [str(REPO_ROOT / "scripts"), *sys.path]):
    release_check = load_module("release_check", RELEASE_CHECK_PATH)
release_check_source = RELEASE_CHECK_PATH.read_text(encoding="utf-8")


class ValidationLanesTestCase(unittest.TestCase):
    def test_lane_manifest_validates(self) -> None:
        self.assertEqual(validation_lanes.validate_manifest(REPO_ROOT), [])

    def test_test_inventory_uses_known_validation_lanes(self) -> None:
        lanes = json.loads((REPO_ROOT / "docs" / "validation" / "validation_lanes.json").read_text(encoding="utf-8"))[
            "lanes"
        ]
        test_inventory = json.loads((REPO_ROOT / "tests" / "test_inventory.json").read_text(encoding="utf-8"))[
            "tests"
        ]

        for test_entry in test_inventory:
            with self.subTest(path=test_entry["path"]):
                self.assertIn(test_entry["validation_lane"], lanes)

    def test_release_covers_blocking_validation_lanes(self) -> None:
        manifest = json.loads((REPO_ROOT / "docs" / "validation" / "validation_lanes.json").read_text(encoding="utf-8"))
        lanes = manifest["lanes"]
        release_coverage = set(lanes["release"]["covers_lanes"])
        blocking_lanes = {
            lane_id
            for lane_id, lane in lanes.items()
            if lane_id != "release" and lane.get("mode") == "blocking"
        }

        self.assertEqual(blocking_lanes - release_coverage, set())

    def test_release_check_has_no_hidden_command_list(self) -> None:
        self.assertNotIn("COMMANDS =", release_check_source)
        self.assertIn("command_sequence(RELEASE_SEQUENCE", release_check_source)

    def test_release_phases_are_exact_complements(self) -> None:
        steps = validation_lanes.command_sequence("release_check", REPO_ROOT)

        checks = release_check.select_steps(steps, "checks")
        tests = release_check.select_steps(steps, "tests")

        self.assertEqual(checks + tests, steps)
        self.assertEqual(tests, [steps[-1]])
        self.assertEqual(tests[0][0], "run tests")

    def test_release_phase_split_fails_closed_on_manifest_drift(self) -> None:
        with self.assertRaisesRegex(ValueError, "exactly one final"):
            release_check.select_steps([("other", ["python", "other.py"])], "checks")
        with self.assertRaisesRegex(ValueError, "exactly one final"):
            release_check.select_steps(
                [("run tests", ["python", "tests.py"]), ("other", ["python", "other.py"])],
                "tests",
            )

    def test_repo_validation_requires_checks_and_repository_tests(self) -> None:
        workflow = (REPO_ROOT / ".github/workflows/repo-validation.yml").read_text(encoding="utf-8")

        self.assertIn("python scripts/release_check.py --phase checks", workflow)
        self.assertIn("python scripts/release_check.py --phase tests", workflow)
        self.assertIn("needs.repository_tests.result", workflow)

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

    def test_run_sequence_executes_manifest_order_from_repo_root(self) -> None:
        steps = validation_lanes.command_sequence("route_docs", REPO_ROOT)
        completed = mock.Mock(returncode=0)

        with mock.patch.object(validation_lanes.subprocess, "run", return_value=completed) as run:
            result = validation_lanes.run_sequence("route_docs", REPO_ROOT)

        self.assertEqual(result, 0)
        self.assertEqual([call.args[0] for call in run.call_args_list], [command for _, command in steps])
        self.assertTrue(all(call.kwargs["cwd"] == REPO_ROOT for call in run.call_args_list))
        self.assertTrue(all(call.kwargs["check"] is False for call in run.call_args_list))

    def test_run_sequence_stops_at_first_failure(self) -> None:
        steps = validation_lanes.command_sequence("route_docs", REPO_ROOT)
        outcomes = [mock.Mock(returncode=7), mock.Mock(returncode=0)]

        with mock.patch.object(validation_lanes.subprocess, "run", side_effect=outcomes) as run:
            result = validation_lanes.run_sequence("route_docs", REPO_ROOT)

        self.assertEqual(result, 7)
        self.assertEqual(run.call_count, 1)
        self.assertEqual(run.call_args.args[0], steps[0][1])


if __name__ == "__main__":
    unittest.main()
