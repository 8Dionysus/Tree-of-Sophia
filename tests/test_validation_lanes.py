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
        test_command = tests[0][1]
        self.assertIn("pytest", test_command)
        self.assertIn("tests", test_command)
        self.assertNotIn("unittest", test_command)

    def test_feedback_maps_product_source_to_existing_access_tests(self) -> None:
        selected = release_check.feedback_test_paths(("access/src/tos_access/core.py",), REPO_ROOT)

        self.assertEqual(
            selected,
            (
                "access/tests/test_access_contract.py",
                "access/tests/test_cloudflare_tunnel_deploy.py",
                "access/tests/test_http_security.py",
            ),
        )

    def test_feedback_maps_active_naming_script_to_its_direct_test(self) -> None:
        self.assertEqual(
            release_check.feedback_test_paths(
                ("scripts/validate_active_naming.py",), REPO_ROOT
            ),
            ("tests/test_validate_active_naming.py",),
        )

    def test_feedback_unions_independent_implementation_routes(self) -> None:
        selected = release_check.feedback_test_paths(
            (
                "scripts/validate_active_naming.py",
                "access/src/tos_access/core.py",
                "scripts/source_witness_bibliographic_graph_common.py",
                "scripts/tos_corpus_index_common.py",
                "scripts/release_check.py",
            ),
            REPO_ROOT,
        )

        expected = (
            "access/tests/test_access_contract.py",
            "access/tests/test_cloudflare_tunnel_deploy.py",
            "access/tests/test_http_security.py",
            "tests/test_source_witness_bibliographic_graph.py",
            "tests/test_tos_corpus_index.py",
            "tests/test_validate_active_naming.py",
            "tests/test_validation_lanes.py",
        )
        self.assertEqual(selected, expected)

    def test_feedback_explicit_access_contract_test_covers_its_helper_consumers(self) -> None:
        self.assertEqual(
            release_check.feedback_test_paths(
                ("access/tests/test_access_contract.py",), REPO_ROOT
            ),
            (
                "access/tests/test_access_contract.py",
                "access/tests/test_cloudflare_tunnel_deploy.py",
                "access/tests/test_http_security.py",
            ),
        )

    def test_feedback_unknown_shared_and_unsupported_paths_fall_back(self) -> None:
        for path in (
            "README.md",
            "ToS/source-witnesses/README.md",
            "ToS/source-witnesses/catalog/works.jsonl",
            "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
            "ToS/derived-exports/tos_corpus_index.min.json",
            "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
            "alignments/translation/dta-first-editions-to-antonovsky-1911-paragraph-v1/"
            "coverage-receipt.v1.json",
            "ToS/canon/concept/becoming/node.json",
            "mechanics/experience/AGENTS.md",
            "ToS/contracts/tos-node.schema.json",
            "ToS/contracts/source-witness-bibliographic-graph.schema.json",
            "access/web/src/main.ts",
            "mechanics/release-support/README.md",
            "tests/conftest.py",
            "tests/test_source_witness_foundation.py",
            "tests/test_zarathustra_authored_canon_evidence_bridge.py",
        ):
            with self.subTest(path=path):
                self.assertIsNone(release_check.feedback_test_paths((path,), REPO_ROOT))

    def test_feedback_returns_full_fallback_when_focus_test_is_missing(self) -> None:
        missing_repo = REPO_ROOT / "does-not-exist"
        self.assertIsNone(
            release_check.feedback_test_paths(("access/src/tos_access/core.py",), missing_repo)
        )

    def test_feedback_rejects_unsafe_paths(self) -> None:
        for path in ("../README.md", "/tmp/README.md", "tests\\test.py"):
            with self.subTest(path=path), self.assertRaises(ValueError):
                release_check.feedback_test_paths((path,), REPO_ROOT)

    def test_feedback_command_reuses_manifest_pytest_flags(self) -> None:
        steps = validation_lanes.command_sequence("release_check", REPO_ROOT)
        target = ("tests/test_validate_active_naming.py",)

        command = release_check.feedback_pytest_command(steps, target)

        self.assertEqual(command[:-1], steps[-1][1][:-1])
        self.assertEqual(command[-1], target[0])

    def test_feedback_test_env_disables_plugin_autoload_by_default(self) -> None:
        with mock.patch.dict(release_check.os.environ, {}, clear=True):
            environment = release_check.feedback_test_env()

        self.assertEqual(environment[release_check.FEEDBACK_PYTEST_AUTOLOAD_ENV], "1")

    def test_feedback_test_env_honors_explicit_plugin_autoload_override(self) -> None:
        for value in ("0", ""):
            with self.subTest(value=value), mock.patch.dict(
                release_check.os.environ,
                {release_check.FEEDBACK_PYTEST_AUTOLOAD_ENV: value},
                clear=True,
            ):
                environment = release_check.feedback_test_env()

            self.assertEqual(environment[release_check.FEEDBACK_PYTEST_AUTOLOAD_ENV], value)

    def test_normal_run_step_environment_is_not_feedback_modified(self) -> None:
        completed = mock.Mock(returncode=0)
        with mock.patch.dict(release_check.os.environ, {}, clear=True), mock.patch.object(
            release_check.subprocess, "run", return_value=completed
        ) as run:
            self.assertEqual(release_check.run_step("normal", ["true"]), 0)

        self.assertNotIn(release_check.FEEDBACK_PYTEST_AUTOLOAD_ENV, run.call_args.kwargs["env"])

    def test_focused_feedback_passes_isolated_environment_to_selected_step(self) -> None:
        with mock.patch.dict(release_check.os.environ, {}, clear=True), mock.patch.object(
            release_check, "run_step", return_value=0
        ) as run_step:
            self.assertEqual(
                release_check.main(
                    ["--feedback", "--changed-path", "scripts/validate_active_naming.py"]
                ),
                0,
            )

        self.assertEqual(
            run_step.call_args.kwargs["env"][release_check.FEEDBACK_PYTEST_AUTOLOAD_ENV],
            "1",
        )

    def test_feedback_fallback_keeps_normal_run_step_environment(self) -> None:
        with mock.patch.dict(release_check.os.environ, {}, clear=True), mock.patch.object(
            release_check, "run_step", return_value=0
        ) as run_step:
            self.assertEqual(
                release_check.main(["--feedback", "--changed-path", "README.md"]),
                0,
            )

        self.assertTrue(run_step.call_args_list)
        self.assertNotIn("env", run_step.call_args.kwargs)

    def test_normal_release_path_does_not_pass_feedback_environment(self) -> None:
        with mock.patch.dict(release_check.os.environ, {}, clear=True), mock.patch.object(
            release_check, "run_step", return_value=0
        ) as run_step:
            self.assertEqual(release_check.main(["--phase", "checks"]), 0)

        self.assertTrue(run_step.call_args_list)
        self.assertTrue(all("env" not in call.kwargs for call in run_step.call_args_list))

    def test_large_generated_checks_rebuild_once_in_the_validator(self) -> None:
        manifest = json.loads(
            (REPO_ROOT / "docs" / "validation" / "validation_lanes.json").read_text(encoding="utf-8")
        )
        sequences = manifest["command_sequences"]
        for sequence_id in ("generated_parity", "graph_exports", "release_check"):
            commands = [tuple(step["command"]) for step in sequences[sequence_id]]
            with self.subTest(sequence=sequence_id):
                self.assertNotIn(
                    ("python", "scripts/build_tos_corpus_index.py", "--check"),
                    commands,
                )
                self.assertIn(
                    ("python", "scripts/validate_tos_corpus_index.py"),
                    commands,
                )
                self.assertNotIn(
                    ("python", "scripts/build_philosophy_graph_projection.py", "--check"),
                    commands,
                )
                self.assertIn(
                    ("python", "scripts/validate_philosophy_graph_projection.py"),
                    commands,
                )

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
