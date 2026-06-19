from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SUPPORT_DIR = REPO_ROOT / "tests" / "support"
if str(SUPPORT_DIR) not in sys.path:
    sys.path.insert(0, str(SUPPORT_DIR))

import topology_inventory


REQUIRED_FIELDS = {
    "path",
    "home",
    "home_scope",
    "family",
    "validation_lane",
    "owner_surface",
    "protects",
    "coverage_authority",
    "mode",
    "runtime_cost",
    "focused_target",
    "failure_route",
    "disposition",
}
HOME_SCOPES = {"root", "mechanic-level", "part-local", "agent-lane"}
MODES = {"blocking", "release-only", "advisory"}
RUNTIME_COSTS = {"fast", "medium", "slow"}


class TestTopologyTests(unittest.TestCase):
    def test_topology_doc_names_home_boundaries(self) -> None:
        text = topology_inventory.TEST_TOPOLOGY_PATH.read_text(encoding="utf-8")

        for required in (
            "# ToS Test Topology",
            "## Route Shape",
            "## Home Scopes",
            "## Families",
            "## Inventory Rules",
            "docs/validation/validation_lanes.json",
            "root",
            "mechanic-level",
            "part-local",
            "agent-lane",
        ):
            with self.subTest(required=required):
                self.assertIn(required, text)

    def test_inventory_covers_every_active_test_file(self) -> None:
        inventory = topology_inventory.load_inventory()
        entries = topology_inventory.inventory_entries()
        paths = [entry["path"] for entry in entries]

        self.assertEqual("docs/testing/TEST_TOPOLOGY.md", inventory["owner_surface"])
        self.assertEqual("docs/validation/validation_lanes.json", inventory["command_authority"])
        self.assertEqual("docs/testing/TEST_TOPOLOGY.md", inventory["runner_authority"])
        self.assertEqual(len(paths), len(set(paths)))
        self.assertEqual(topology_inventory.discovered_test_files(), set(paths))

    def test_inventory_entries_are_complete_and_owner_routed(self) -> None:
        validation_lanes = topology_inventory.load_validation_lanes()

        for entry in topology_inventory.inventory_entries():
            with self.subTest(path=entry.get("path")):
                self.assertTrue(REQUIRED_FIELDS <= set(entry))
                self.assertTrue((REPO_ROOT / entry["path"]).is_file())
                self.assertTrue((REPO_ROOT / entry["owner_surface"]).exists())
                self.assertIn(entry["home_scope"], HOME_SCOPES)
                self.assertIn(entry["mode"], MODES)
                self.assertIn(entry["runtime_cost"], RUNTIME_COSTS)
                self.assertIn(entry["validation_lane"], validation_lanes)
                self.assertTrue(entry["protects"])
                self.assertTrue(entry["failure_route"].startswith("Fix "))
                self.assertFalse(topology_inventory.looks_like_command(entry["focused_target"]))
                self.assertFalse(topology_inventory.looks_like_command(entry["coverage_authority"]))
                self.assertNotIn("command", entry)

    def test_inventory_home_scopes_match_filesystem_topology(self) -> None:
        for entry in topology_inventory.inventory_entries():
            expected_scope, expected_home = topology_inventory.classify_test_home(entry["path"])
            with self.subTest(path=entry["path"]):
                self.assertEqual(expected_scope, entry["home_scope"])
                self.assertEqual(expected_home, entry["home"])
                self.assertTrue(entry["path"].startswith(f"{entry['home']}/"))

    def test_test_inventory_does_not_duplicate_command_authority(self) -> None:
        inventory = topology_inventory.load_inventory()
        inventory_text = json.dumps(inventory, sort_keys=True)
        topology_text = topology_inventory.TEST_TOPOLOGY_PATH.read_text(encoding="utf-8")

        self.assertNotIn("command_sequence", inventory_text)
        self.assertNotIn("python ", inventory_text)
        self.assertNotIn("python ", topology_text)
        self.assertNotIn("pytest ", inventory_text)

    def test_mechanics_owned_tests_name_mechanics_owner(self) -> None:
        mechanics_entries = [
            entry
            for entry in topology_inventory.inventory_entries()
            if entry["family"].startswith("mechanics_")
        ]

        self.assertTrue(mechanics_entries)
        for entry in mechanics_entries:
            with self.subTest(path=entry["path"]):
                self.assertTrue(entry["owner_surface"].startswith(("mechanics/", "scripts/")))
                if entry["home_scope"] == "part-local":
                    self.assertTrue(entry["home"].startswith("mechanics/"))
                    self.assertEqual("mechanics_local", entry["validation_lane"])
                elif entry["home_scope"] == "mechanic-level":
                    self.assertTrue(entry["home"].startswith("mechanics/"))
                    self.assertIn(entry["validation_lane"], {"mechanics_local", "experience_contracts"})
                else:
                    self.assertEqual("root", entry["home_scope"])
                    self.assertIn(entry["validation_lane"], {"experience_contracts", "generated_parity", "questbook_surface"})


if __name__ == "__main__":
    unittest.main()
