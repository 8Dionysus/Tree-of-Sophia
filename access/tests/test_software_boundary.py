"""Behavior at the boundary between installed software and selected data."""
from __future__ import annotations

import importlib.util
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

ACCESS_ROOT = Path(__file__).resolve().parents[1]
REPO_ROOT = ACCESS_ROOT.parent
sys.path.insert(0, str(ACCESS_ROOT / "src"))

from tos_access.core import ToSAccessCore
from tos_access.locations import data_root, program_path
from tos_access import locations


def standalone_validator():
    spec = importlib.util.spec_from_file_location(
        "software_boundary_validator", ACCESS_ROOT / "packaging/validate_standalone.py"
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def copy_software_contracts(root: Path) -> None:
    shutil.copytree(ACCESS_ROOT / "contracts", root / "access/contracts")
    shutil.copytree(ACCESS_ROOT / "profiles", root / "access/profiles")
    (root / "ToS/contracts").mkdir(parents=True)
    shutil.copy2(
        REPO_ROOT / "ToS/contracts/epistemic-evidence-projection.schema.json",
        root / "ToS/contracts/epistemic-evidence-projection.schema.json",
    )


class SoftwareBoundaryTests(unittest.TestCase):
    def test_installed_package_cannot_fall_back_to_unrelated_library_paths(self):
        with tempfile.TemporaryDirectory() as temporary:
            package = Path(temporary) / "venv/lib/python3.12/site-packages/tos_access"
            access = package.parents[1]
            relative = Path("access/contracts/exploration-request.v1.schema.json")
            foreign = access.parent / relative
            foreign.parent.mkdir(parents=True)
            foreign.write_text('{"foreign":true}')
            foreign_web = access / "web/dist/assets/tos-graph.js"
            foreign_web.parent.mkdir(parents=True)
            foreign_web.write_text("foreign browser code")
            with patch.object(locations, "PACKAGE_ROOT", package), patch.object(locations, "ACCESS_ROOT", access):
                self.assertEqual(program_path(relative), package / "runtime_data" / relative)
                self.assertFalse(program_path(relative).exists())
                self.assertIsNone(locations.web_root())

    def test_program_validation_needs_no_corpus_or_runtime(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            copy_software_contracts(root)
            standalone_validator()._validate_contracts(root)
            self.assertFalse((root / "ToS/derived-exports").exists())
            self.assertFalse((root / "access/src").exists())

    def test_program_validation_rejects_invalid_schema_without_a_corpus(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            copy_software_contracts(root)
            path = root / "access/contracts/lens-spec.v1.schema.json"
            schema = json.loads(path.read_text())
            schema["type"] = "not-a-json-schema-type"
            path.write_text(json.dumps(schema))
            with self.assertRaisesRegex(RuntimeError, "invalid knowledge contract schema"):
                standalone_validator()._validate_contracts(root)

    def test_program_validation_rejects_authority_escalation(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            copy_software_contracts(root)
            path = root / "access/contracts/epistemic-packet.v1.schema.json"
            schema = json.loads(path.read_text())
            schema["properties"]["authority_boundary"]["properties"]["is_canon"]["const"] = True
            path.write_text(json.dumps(schema))
            with self.assertRaisesRegex(RuntimeError, "authority boundary must fail closed"):
                standalone_validator()._validate_contracts(root)

    def test_data_selection_does_not_discover_cwd_corpus(self):
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, {}, clear=True):
            root = Path(temporary)
            projection = root / "ToS/derived-exports/tos_corpus_index.min.json"
            projection.parent.mkdir(parents=True)
            projection.write_text('{"schema_version":"tos_corpus_index_v1"}')
            with patch("pathlib.Path.cwd", return_value=root):
                selected = data_root()
            self.assertNotEqual(selected, root)
            self.assertEqual(data_root(root), root)
            missing = root / "missing-explicit-data"
            with patch.dict(os.environ, {"TOS_DATA_ROOT": str(missing)}):
                self.assertEqual(data_root(), missing)

    def test_selected_data_cannot_replace_executable_api_contracts(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            contracts = root / "access/contracts"
            contracts.mkdir(parents=True)
            (contracts / "exploration-request.v1.schema.json").write_text('{"malicious_override":true}')
            core = ToSAccessCore.discover(tos_root=root)
            result = core.knowledge_exploration_contracts()
            expected = json.loads(program_path("access/contracts/exploration-request.v1.schema.json").read_text())
            self.assertEqual(result["request"], expected)
            self.assertNotIn("malicious_override", result["request"])


if __name__ == "__main__":
    unittest.main()
