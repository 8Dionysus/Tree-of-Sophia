from __future__ import annotations

import copy
import json
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


if __name__ == "__main__":
    unittest.main()
