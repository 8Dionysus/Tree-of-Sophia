from __future__ import annotations

import copy
import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
MODULE_PATH = REPO_ROOT / "scripts" / "validate_tree_node_contracts.py"
SPEC = importlib.util.spec_from_file_location("validate_tree_node_contracts", MODULE_PATH)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"unable to load validator module from {MODULE_PATH}")
validate_tree_node_contracts = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(validate_tree_node_contracts)


def load_json(path: Path) -> object:
    return json.loads(path.read_text(encoding="utf-8"))


class ValidateTreeNodeContractsTests(unittest.TestCase):
    def test_duplicate_witness_languages_fail_contract_validation(self) -> None:
        payload = load_json(REPO_ROOT / "examples" / "source_node.example.json")
        assert isinstance(payload, dict)
        payload = copy.deepcopy(payload)
        payload["language_witnesses"][1]["language"] = payload["language_witnesses"][0]["language"]

        with tempfile.TemporaryDirectory() as tmp:
            repo_root = Path(tmp) / "Tree-of-Sophia"
            schema_path = repo_root / "schemas" / "tos-node-contract.schema.json"
            node_path = repo_root / "tree" / "source" / "test-route" / "node.json"
            schema_path.parent.mkdir(parents=True, exist_ok=True)
            node_path.parent.mkdir(parents=True, exist_ok=True)
            schema_path.write_text(
                (REPO_ROOT / "schemas" / "tos-node-contract.schema.json").read_text(encoding="utf-8"),
                encoding="utf-8",
            )
            node_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")

            issues = validate_tree_node_contracts.run_validation(repo_root)

        self.assertIn(
            (
                "tree/source/test-route/node.json",
                "language_witnesses contains a duplicate language: de",
            ),
            issues,
        )


if __name__ == "__main__":
    unittest.main()
