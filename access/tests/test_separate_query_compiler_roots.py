from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import sqlite3
import tempfile
import unittest
import sys


TESTS_ROOT = Path(__file__).resolve().parent
if str(TESTS_ROOT) not in sys.path:
    sys.path.insert(0, str(TESTS_ROOT))

from fixture_support import write_fixture


INPUTS = {
    "corpus": "ToS/derived-exports/tos_corpus_index.min.json",
    "philosophy": "ToS/derived-exports/philosophy_graph_projection.min.json",
    "bibliographic": "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
    "entities": "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "predicates": "ToS/doctrine/semantic-interchange/relation-types.v1.json",
}


class SeparateQueryCompilerRootTests(unittest.TestCase):
    def _software_root(self, root: Path) -> Path:
        """Create a synthetic software checkout with no dependency on live data."""
        write_fixture(root)
        source = TESTS_ROOT.parent / "src" / "tos_access"
        shutil.copytree(
            source,
            root / "access" / "src" / "tos_access",
            ignore=shutil.ignore_patterns("__pycache__", "*.pyc"),
        )
        compiler = root / "access" / "packaging" / "data_compile_common.py"
        compiler.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(TESTS_ROOT.parent / "packaging" / compiler.name, compiler)
        return root

    @staticmethod
    def _compiler(root: Path):
        path = root / "access" / "packaging" / "data_compile_common.py"
        name = f"data_compile_common_fixture_{id(path)}"
        spec = importlib.util.spec_from_file_location(name, path)
        if spec is None or spec.loader is None:
            raise RuntimeError(f"unable to load synthetic compiler helpers: {path}")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        return module

    @staticmethod
    def _contract() -> tuple[dict[str, object], dict[str, object]]:
        subjects = [
            {"subject_id": subject_id, "source_path": relative}
            for subject_id, relative in INPUTS.items()
        ]
        allowlist = {"subjects": subjects}
        compiled_subject = {
            "subject_id": "runtime.knowledge-store",
            "output_path": "ToS/derived-exports/runtime/knowledge.sqlite3",
            "builder_module": "tos_access.knowledge_compile",
            "input_subject_ids": list(INPUTS),
        }
        return allowlist, compiled_subject

    @staticmethod
    def _bindings(root: Path) -> dict[str, str]:
        return {
            relative: hashlib.sha256((root / relative).read_bytes()).hexdigest()
            for relative in INPUTS.values()
        }

    @staticmethod
    def _metadata(path: Path) -> dict[str, object]:
        with sqlite3.connect(path) as db:
            return {
                key: json.loads(value)
                for key, value in db.execute("SELECT key, value FROM metadata")
            }

    def test_compiles_with_software_code_and_separate_data_root(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            software = self._software_root(workspace / "software")
            data = workspace / "data"
            write_fixture(data)
            self.assertFalse((data / "access" / "src").exists())

            corpus_path = data / INPUTS["corpus"]
            corpus = json.loads(corpus_path.read_text(encoding="utf-8"))
            corpus["owner_repo"] = "selected-data-root"
            corpus_path.write_text(json.dumps(corpus), encoding="utf-8")

            allowlist, compiled_subject = self._contract()
            compiler = self._compiler(software)
            output = workspace / "compiled.sqlite3"
            result, metadata = compiler._compile_query_store(
                software,
                output,
                allowlist,
                compiled_subject,
                data_root=data,
            )

            self.assertEqual(result, output.resolve())
            self.assertEqual(metadata["input_bindings"], self._bindings(data))
            self.assertEqual(
                metadata["compiler_sha256"],
                compiler._query_store_compiler_fingerprint(software),
            )
            compiled_metadata = self._metadata(output)
            self.assertEqual(compiled_metadata["snapshot_bindings"], self._bindings(data))
            self.assertEqual(compiled_metadata["corpus_header"]["owner_repo"], "selected-data-root")

    def test_distracting_compiler_code_in_data_root_is_never_executed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            software = self._software_root(workspace / "software")
            data = workspace / "data"
            write_fixture(data)
            sentinel = workspace / "data-root-compiler-executed"
            shadow_package = data / "access" / "src" / "tos_access"
            shadow_package.mkdir(parents=True)
            (shadow_package / "__init__.py").write_text("", encoding="utf-8")
            (shadow_package / "knowledge_compile.py").write_text(
                "from pathlib import Path\n"
                f"Path({str(sentinel)!r}).write_text('executed')\n"
                "raise RuntimeError('data-root compiler executed')\n",
                encoding="utf-8",
            )

            allowlist, compiled_subject = self._contract()
            compiler = self._compiler(software)
            result, _ = compiler._compile_query_store(
                software,
                workspace / "compiled.sqlite3",
                allowlist,
                compiled_subject,
                data_root=data,
            )

            self.assertTrue(result.is_file())
            self.assertFalse(sentinel.exists())

    def test_missing_selected_input_does_not_fall_back_to_software_root(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            software = self._software_root(workspace / "software")
            data = workspace / "data"
            write_fixture(data)
            (data / INPUTS["entities"]).unlink()
            output = workspace / "compiled.sqlite3"
            prior = b"preserve this output"
            output.write_bytes(prior)
            allowlist, compiled_subject = self._contract()

            with self.assertRaisesRegex(RuntimeError, "missing compiled query-store input: entities"):
                self._compiler(software)._compile_query_store(
                    software,
                    output,
                    allowlist,
                    compiled_subject,
                    data_root=data,
                )
            self.assertEqual(output.read_bytes(), prior)

    def test_invalid_selected_input_does_not_fall_back_to_software_root(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            software = self._software_root(workspace / "software")
            data = workspace / "data"
            write_fixture(data)
            (data / INPUTS["entities"]).write_text("{invalid json\n", encoding="utf-8")
            output = workspace / "compiled.sqlite3"
            prior = b"preserve this output too"
            output.write_bytes(prior)
            allowlist, compiled_subject = self._contract()

            with self.assertRaisesRegex(RuntimeError, "isolated query-store compilation failed"):
                self._compiler(software)._compile_query_store(
                    software,
                    output,
                    allowlist,
                    compiled_subject,
                    data_root=data,
                )
            self.assertEqual(output.read_bytes(), prior)

    def test_abi_probe_uses_data_bindings_and_software_fingerprint(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            workspace = Path(directory)
            software = self._software_root(workspace / "software")
            data = workspace / "data"
            write_fixture(data)
            allowlist, compiled_subject = self._contract()
            compiler = self._compiler(software)

            data_contract = data / "access" / "contracts" / "runtime-data.v1.json"
            data_contract.write_text('{"distractor": true}\n', encoding="utf-8")
            schema, version, bindings, fingerprint = compiler._query_store_compiler_contract(
                software,
                allowlist,
                compiled_subject,
                data_root=data,
            )

            self.assertEqual(schema, "tos_query_store_v1")
            self.assertEqual(version, "tos_offline_knowledge_v2")
            self.assertEqual(bindings, self._bindings(data))
            self.assertEqual(fingerprint, compiler._query_store_compiler_fingerprint(software))

            software_fingerprint = fingerprint
            (data / INPUTS["corpus"]).write_text(
                (data / INPUTS["corpus"]).read_text(encoding="utf-8") + "\n",
                encoding="utf-8",
            )
            _, _, changed_bindings, changed_fingerprint = compiler._query_store_compiler_contract(
                software,
                allowlist,
                compiled_subject,
                data_root=data,
            )
            self.assertNotEqual(changed_bindings[INPUTS["corpus"]], bindings[INPUTS["corpus"]])
            self.assertEqual(changed_fingerprint, software_fingerprint)


if __name__ == "__main__":
    unittest.main()
