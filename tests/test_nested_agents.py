from __future__ import annotations

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import agents_route_harness
import build_agents_route_currentness
import validate_nested_agents


class NestedAgentsRouteTests(unittest.TestCase):
    def test_inventory_inheritance_currentness_and_harness_are_green(self) -> None:
        self.assertEqual([], validate_nested_agents.run_validation(REPO_ROOT))
        cards = validate_nested_agents.discover_route_cards(REPO_ROOT)
        self.assertEqual(55, len(cards))
        self.assertIn(REPO_ROOT / ".github/AGENTS.md", cards)

        currentness = json.loads((REPO_ROOT / ".agents/agents-route.current.json").read_text(encoding="utf-8"))
        self.assertEqual(55, currentness["route_card_counts"]["tracked_exact_basename"])
        self.assertEqual(55, currentness["route_card_counts"]["discovered_by_inventory"])
        self.assertEqual([], currentness["route_card_counts"]["tracked_only"])
        self.assertEqual([], currentness["route_card_counts"]["discovered_untracked"])
        preserved = {entry["path"] for entry in currentness["preserved_non_cards"]}
        self.assertIn("docs/AGENTS_ROOT_REFERENCE.md", preserved)

        result = agents_route_harness.build_result(REPO_ROOT)
        self.assertEqual(15, result["task_count"])
        self.assertEqual(15, result["route_success_count"])
        self.assertEqual(0, result["behavioral_model_runs"])
        self.assertIsNone(result["behavioral_claim"])
        self.assertTrue(
            all(not task["selected_validation"]["unknown_lanes"] for task in result["tasks"])
        )

        corpus_task = next(
            task for task in result["tasks"] if task["id"] == "corpus_identity_provenance_rights"
        )
        self.assertEqual(
            ["AGENTS.md", "ToS/AGENTS.md", "ToS/doctrine/AGENTS.md"],
            corpus_task["inheritance_stack"],
        )
        self.assertFalse(corpus_task["owner_handoff"]["in_inheritance_stack"])
        self.assertTrue(corpus_task["owner_handoff"]["exists"])
        self.assertEqual(
            ["AGENTS.md", "ToS/AGENTS.md", "ToS/doctrine/AGENTS.md"],
            next(
                task for task in currentness["task_routes"]
                if task["id"] == "corpus_identity_provenance_rights"
            )["inheritance_stack"],
        )

    def test_duplicate_nested_card_is_a_negative_control(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root_card = root / "AGENTS.md"
            nested_card = root / "ToS" / "AGENTS.md"
            nested_card.parent.mkdir()
            root_card.write_text("# AGENTS.md\n", encoding="utf-8")
            nested_card.write_text(root_card.read_text(encoding="utf-8"), encoding="utf-8")
            issues = validate_nested_agents.duplicate_inherited_card_issues(root, [root_card, nested_card])
            self.assertEqual(1, len(issues))
            self.assertIn("byte-identical", issues[0][1])

    def test_stale_executable_reference_is_a_negative_control(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            card = root / "AGENTS.md"
            card.write_text("# AGENTS.md\npython scripts/not-present.py\n", encoding="utf-8")
            issues: list[tuple[str, str]] = []
            validate_nested_agents.validate_local_script_references(root, [card], issues)
            self.assertEqual(1, len(issues))
            self.assertIn("not-present.py", issues[0][1])

    def test_route_inventory_is_required_for_validator_admission(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            root_card = root / "AGENTS.md"
            root_card.write_text("# AGENTS.md\n", encoding="utf-8")
            issues: list[tuple[str, str]] = []
            validate_nested_agents.validate_route_inventory(root, [root_card], issues)
            self.assertEqual(
                [("docs/validation/agents_route_inventory.json", "route inventory is missing")],
                issues,
            )

            inventory = root / "docs/validation/agents_route_inventory.json"
            inventory.parent.mkdir(parents=True)
            inventory.write_text("{not-json", encoding="utf-8")
            issues = []
            validate_nested_agents.validate_route_inventory(root, [root_card], issues)
            self.assertEqual(
                [("docs/validation/agents_route_inventory.json", "route inventory is unreadable or malformed")],
                issues,
            )

    def test_missing_task_law_and_budget_are_negative_controls(self) -> None:
        inventory_path, inventory = build_agents_route_currentness.load_inventory(REPO_ROOT)
        del inventory_path
        discovered = set(build_agents_route_currentness.discover_route_cards(REPO_ROOT, inventory))
        task = copy.deepcopy(inventory["task_routes"][0])
        task["required_markers"].append("marker that cannot exist in this route")
        inventory["context_budget"]["task_overrides"] = {task["id"]: 1}
        result = agents_route_harness.evaluate_task(REPO_ROOT, inventory, task, discovered)
        self.assertFalse(result["route_success"])
        self.assertEqual(1, result["missing_task_specific_law"]["count"])
        self.assertIn("inherited_context_tokens>1", result["budget"]["violations"])

    def test_unknown_validation_lane_is_a_negative_control(self) -> None:
        inventory_path, inventory = build_agents_route_currentness.load_inventory(REPO_ROOT)
        del inventory_path
        discovered = set(build_agents_route_currentness.discover_route_cards(REPO_ROOT, inventory))
        task = copy.deepcopy(inventory["task_routes"][0])
        task["validation_lanes"].append("lane-that-is-not-in-the-command-authority-manifest")
        result = agents_route_harness.evaluate_task(REPO_ROOT, inventory, task, discovered)
        self.assertFalse(result["route_success"])
        self.assertEqual(
            ["lane-that-is-not-in-the-command-authority-manifest"],
            result["selected_validation"]["unknown_lanes"],
        )

    def test_currentness_output_overrides_normalize_relative_and_external_paths(self) -> None:
        builder = SCRIPTS / "build_agents_route_currentness.py"
        with tempfile.TemporaryDirectory(dir=REPO_ROOT) as relative_dir:
            relative_root = Path(relative_dir)
            relative_output = relative_root.relative_to(REPO_ROOT) / "route.json"
            relative_run = subprocess.run(
                [sys.executable, str(builder), "--output", str(relative_output)],
                cwd=REPO_ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(0, relative_run.returncode, relative_run.stderr)
            self.assertTrue((REPO_ROOT / relative_output).is_file())

        with tempfile.TemporaryDirectory() as external_dir:
            external_output = Path(external_dir) / "route.json"
            external_run = subprocess.run(
                [sys.executable, str(builder), "--output", str(external_output)],
                cwd=REPO_ROOT,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(0, external_run.returncode, external_run.stderr)
            self.assertTrue(external_output.is_file())

    def test_harness_canonical_result_is_deterministic_without_timing(self) -> None:
        first = agents_route_harness.build_result(REPO_ROOT)
        second = agents_route_harness.build_result(REPO_ROOT)
        self.assertEqual(first, second)
        self.assertTrue(all(task["time_to_owner_ms"] is None for task in first["tasks"]))
        volatile = agents_route_harness.build_result(REPO_ROOT, include_timing=True)
        self.assertTrue(all(task["time_to_owner_ms"] is not None for task in volatile["tasks"]))

    def test_harness_marks_dirty_tree_as_working_tree(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            subprocess.run(["git", "init", "--quiet"], cwd=root, check=True)
            subprocess.run(["git", "config", "user.email", "test@example.invalid"], cwd=root, check=True)
            subprocess.run(["git", "config", "user.name", "Route Test"], cwd=root, check=True)
            (root / "tracked.txt").write_text("tracked\n", encoding="utf-8")
            subprocess.run(["git", "add", "tracked.txt"], cwd=root, check=True)
            subprocess.run(["git", "commit", "--quiet", "-m", "initial"], cwd=root, check=True)
            self.assertNotEqual("working-tree", agents_route_harness.current_ref(root))
            (root / "dirty.txt").write_text("dirty\n", encoding="utf-8")
            self.assertEqual("working-tree", agents_route_harness.current_ref(root))


if __name__ == "__main__":
    unittest.main()
