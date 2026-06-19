from __future__ import annotations

import importlib.util
import json
import runpy
import subprocess
import sys
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SUPPORT_DIR = REPO_ROOT / "tests" / "support"
if str(SUPPORT_DIR) not in sys.path:
    sys.path.insert(0, str(SUPPORT_DIR))

import script_inventory


REQUIRED_ENTRY_FIELDS = {
    "path",
    "family",
    "organ_lane",
    "owner_surface",
    "source_truth",
    "reads",
    "writes",
    "side_effects",
    "validation_lane",
    "ci_inclusion",
    "test_target",
    "disposition",
}
ALLOWED_ORGAN_LANES = {
    "source/topology",
    "source/canon-contract",
    "projection/generated",
    "public-compatibility",
    "mechanics/part-owned",
    "mechanics/package",
    "agent-skill/advisory",
    "release",
    "validation-authority",
    "route-card",
    "shared-library",
}
ALLOWED_CI_INCLUSION = {
    "release",
    "focused-tests",
    "inventory-only",
    "advisory-only",
}
SAFE_CLI_SMOKE_COMMANDS = (
    (".agents/skills/aoa-dry-run-first/scripts/dry_run_contract.py", "--template"),
    (".agents/skills/aoa-local-stack-bringup/scripts/bringup_contract.py", "--template"),
    (".agents/skills/aoa-safe-infra-change/scripts/infra_change_contract.py", "--template"),
)


def import_path_for(path: str) -> str:
    return path.removesuffix(".py").replace("/", ".").replace("-", "_")


class ScriptTopologyTests(unittest.TestCase):
    def test_script_topology_doc_names_inventory_and_boundaries(self) -> None:
        text = script_inventory.SCRIPT_TOPOLOGY_PATH.read_text(encoding="utf-8")
        for required in (
            "# ToS Script Topology",
            "## Command Authority",
            "## Script Families",
            "## Skill Helper Scripts",
            "validation_lanes.json",
            "skill_local_contract_tool",
            "Promotion Rule",
        ):
            with self.subTest(required=required):
                self.assertIn(required, text)

    def test_inventory_covers_every_active_script_surface(self) -> None:
        inventory = script_inventory.load_inventory()
        entries = script_inventory.inventory_entries()
        paths = [entry["path"] for entry in entries]

        self.assertEqual("docs/validation/SCRIPT_TOPOLOGY.md", inventory["owner_surface"])
        self.assertEqual("docs/validation/validation_lanes.json", inventory["command_authority"])
        self.assertEqual(len(paths), len(set(paths)))
        self.assertEqual(script_inventory.discovered_script_surfaces(), set(paths))

    def test_inventory_entries_are_complete_and_owner_routed(self) -> None:
        lanes = script_inventory.load_validation_lanes()

        for entry in script_inventory.inventory_entries():
            with self.subTest(path=entry.get("path")):
                self.assertEqual(REQUIRED_ENTRY_FIELDS, set(entry))
                self.assertIn(entry["organ_lane"], ALLOWED_ORGAN_LANES)
                self.assertIn(entry["validation_lane"], lanes)
                self.assertIn(entry["ci_inclusion"], ALLOWED_CI_INCLUSION)
                self.assertEqual("keep", entry["disposition"])
                self.assertTrue((REPO_ROOT / entry["path"]).is_file())
                self.assertTrue((REPO_ROOT / entry["owner_surface"]).exists())
                self.assertTrue((REPO_ROOT / entry["test_target"]).exists())
                self.assertIsInstance(entry["source_truth"], list)
                self.assertTrue(entry["source_truth"])
                self.assertIsInstance(entry["reads"], list)
                self.assertTrue(entry["reads"])
                self.assertIsInstance(entry["writes"], list)
                self.assertTrue(entry["side_effects"])

    def test_lane_commands_reference_inventoried_scripts(self) -> None:
        command_paths = script_inventory.command_script_paths()

        self.assertTrue(command_paths)
        self.assertTrue(command_paths <= script_inventory.inventory_paths())

    def test_skill_helper_scripts_are_not_hidden_hard_gates(self) -> None:
        hard_gate_paths = script_inventory.command_script_paths()

        for entry in script_inventory.inventory_entries():
            path = entry["path"]
            if not path.startswith(".agents/skills/"):
                continue
            with self.subTest(path=path):
                self.assertEqual("skill_local_contract_tool", entry["family"])
                self.assertEqual("agent-skill/advisory", entry["organ_lane"])
                self.assertEqual("advisory-only", entry["ci_inclusion"])
                self.assertEqual([], entry["writes"])
                self.assertNotIn(path, hard_gate_paths)

    def test_side_effect_boundaries_are_visible(self) -> None:
        for entry in script_inventory.inventory_entries():
            with self.subTest(path=entry["path"]):
                if entry["writes"]:
                    self.assertNotEqual("validation output only", entry["side_effects"])
                    self.assertNotEqual("active_naming", entry["validation_lane"])
                if entry["side_effects"] == "validation output only":
                    self.assertEqual([], entry["writes"])
                if entry["ci_inclusion"] == "advisory-only":
                    self.assertEqual([], entry["writes"])

    def test_python_scripts_load_without_running_main(self) -> None:
        for entry in script_inventory.inventory_entries():
            path = entry["path"]
            if not path.endswith(".py"):
                continue
            script_path = REPO_ROOT / path
            with self.subTest(path=path):
                old_path = list(sys.path)
                sys.path.insert(0, str(script_path.parent))
                sys.path.insert(0, str(REPO_ROOT / "scripts"))
                sys.path.insert(0, str(REPO_ROOT))
                try:
                    runpy.run_path(str(script_path), run_name=f"__script_inventory_smoke__:{path}")
                finally:
                    sys.path[:] = old_path

    def test_safe_cli_smoke_commands_stay_non_mutating(self) -> None:
        for command in SAFE_CLI_SMOKE_COMMANDS:
            with self.subTest(command=command):
                result = subprocess.run(
                    (sys.executable, *command),
                    cwd=REPO_ROOT,
                    check=False,
                    capture_output=True,
                    text=True,
                    timeout=15,
                )
                self.assertEqual(0, result.returncode, msg=f"stdout={result.stdout}\nstderr={result.stderr}")

    def test_active_route_docs_do_not_reference_missing_scripts(self) -> None:
        docs = [
            *REPO_ROOT.glob("**/AGENTS.md"),
            *REPO_ROOT.glob("docs/**/*.md"),
            REPO_ROOT / "README.md",
            REPO_ROOT / "ROADMAP.md",
        ]
        missing: dict[str, list[str]] = {}

        for doc in sorted(set(docs)):
            relative_doc = doc.relative_to(REPO_ROOT).as_posix()
            if "legacy/" in relative_doc or "__pycache__" in relative_doc:
                continue
            text = doc.read_text(encoding="utf-8")
            refs = {
                ref
                for raw_ref in (
                    match.group(1)
                    for match in script_inventory.SCRIPT_REF_RE.finditer(text)
                    if "__pycache__" not in match.group(1)
                )
                if (ref := script_inventory.resolve_local_script_ref(doc, raw_ref)) is not None
            }
            unresolved = sorted(ref for ref in refs if not (REPO_ROOT / ref).is_file())
            if unresolved:
                missing[relative_doc] = unresolved

        self.assertEqual({}, missing)

    def test_no_tracked_python_cache_residue_under_script_surfaces(self) -> None:
        result = subprocess.run(
            ("git", "ls-files", "*__pycache__*", "*.pyc"),
            cwd=REPO_ROOT,
            check=True,
            capture_output=True,
            text=True,
        )

        self.assertEqual("", result.stdout.strip())


if __name__ == "__main__":
    unittest.main()
