from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


TESTS_ROOT = Path(__file__).resolve().parent
ACCESS_ROOT = TESTS_ROOT.parent
if str(TESTS_ROOT) not in sys.path:
    sys.path.insert(0, str(TESTS_ROOT))
if str(ACCESS_ROOT / "packaging") not in sys.path:
    sys.path.insert(0, str(ACCESS_ROOT / "packaging"))

import build_data_snapshot as data_snapshot_builder  # noqa: E402
from build_data_snapshot import (  # noqa: E402
    QUERY_STORE_RELATIVE_PATH,
    build_data_snapshot,
    verify_data_snapshot,
)
from data_compile_common import (  # noqa: E402
    _query_store_compiler_fingerprint,
    query_store_compiler_paths,
)
from fixture_support import write_fixture  # noqa: E402


INPUTS = {
    "corpus": "ToS/derived-exports/tos_corpus_index.min.json",
    "philosophy": "ToS/derived-exports/philosophy_graph_projection.min.json",
    "bibliographic": "ToS/derived-exports/graph/source-witness-bibliographic-claims.min.json",
    "entities": "ToS/doctrine/semantic-interchange/entity-types.v1.json",
    "predicates": "ToS/doctrine/semantic-interchange/relation-types.v1.json",
}
DATA_SUBJECT_PATHS = tuple(
    list(INPUTS.values())
    + [
        "ToS/contracts/semantic-entity-type-registry.schema.json",
        "ToS/contracts/semantic-relation-type-registry.schema.json",
        "ToS/derived-exports/epistemic_evidence_projection.min.json",
    ]
)
API_SUBJECT_PATHS = (
    "access/contracts/exploration-request.v1.schema.json",
    "access/contracts/exploration-result.v1.schema.json",
)


class DataSnapshotTests(unittest.TestCase):
    def _software_root(self, root: Path) -> Path:
        """Create a software-only checkout with no source data of its own."""
        package_source = ACCESS_ROOT / "src/tos_access"
        shutil.copytree(
            package_source,
            root / "access/src/tos_access",
            ignore=shutil.ignore_patterns("__pycache__", "*.pyc", "runtime_data"),
        )
        builder_source = ACCESS_ROOT / "packaging/data_compile_common.py"
        (root / "access/packaging").mkdir(parents=True, exist_ok=True)
        shutil.copy2(builder_source, root / "access/packaging/data_compile_common.py")

        contract = json.loads(
            (ACCESS_ROOT / "contracts/runtime-data.v1.json").read_text(encoding="utf-8")
        )
        selected_ids = {
            "tos-exploration-request-contract",
            "tos-exploration-result-contract",
            "tos-corpus-index",
            "tos-philosophy-graph",
            "tos-source-witness-bibliographic-claim-graph",
            "tos-semantic-entity-type-registry",
            "tos-semantic-relation-type-registry",
            "tos-semantic-entity-type-registry-schema",
            "tos-semantic-relation-type-registry-schema",
            "tos-epistemic-evidence",
        }
        contract["subjects"] = [
            item for item in contract["subjects"] if item["subject_id"] in selected_ids
        ]
        contract_path = root / "access/contracts/runtime-data.v1.json"
        contract_path.parent.mkdir(parents=True, exist_ok=True)
        contract_path.write_bytes(
            json.dumps(contract, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
        )
        return root

    def _fixture(self) -> tuple[Path, Path, Path]:
        workspace = Path(tempfile.mkdtemp(prefix="data-snapshot-test-"))
        software = self._software_root(workspace / "software")
        data = workspace / "data"
        write_fixture(data)
        return workspace, software, data

    @staticmethod
    def _build(software: Path, data: Path, output: Path) -> dict:
        return build_data_snapshot(
            software,
            data,
            output,
            corpus_revision="a" * 64,
        )

    def test_separate_fixture_compiles_exact_data_closure_and_is_canonical(self) -> None:
        workspace, software, data = self._fixture()
        try:
            output = workspace / "snapshot"
            manifest = self._build(software, data, output)
            self.assertEqual(verify_data_snapshot(output), manifest)
            self.assertEqual(manifest["schema_version"], "tos_access_data_snapshot_v1")
            self.assertEqual(manifest["corpus_revision"], "a" * 64)

            expected_paths = {
                f"data/{relative}" for relative in DATA_SUBJECT_PATHS
            }
            expected_paths.add(f"data/{QUERY_STORE_RELATIVE_PATH}")
            self.assertEqual(
                {member["path"] for member in manifest["members"]}, expected_paths
            )
            self.assertTrue(
                all(path not in {member["path"] for member in manifest["members"]}
                    for path in (f"data/{relative}" for relative in API_SUBJECT_PATHS))
            )
            self.assertEqual(
                set(manifest["input_bindings"]), set(DATA_SUBJECT_PATHS)
            )
            raw_manifest = (output / "manifest.json").read_bytes()
            self.assertEqual(
                raw_manifest,
                (
                    json.dumps(
                        manifest,
                        ensure_ascii=False,
                        sort_keys=True,
                        separators=(",", ":"),
                    )
                    + "\n"
                ).encode("utf-8"),
            )
        finally:
            shutil.rmtree(workspace)

    def test_two_outputs_have_identical_bytes_and_revision(self) -> None:
        workspace, software, data = self._fixture()
        try:
            first = workspace / "first"
            second = workspace / "second"
            first_manifest = self._build(software, data, first)
            second_manifest = self._build(software, data, second)
            self.assertEqual(first_manifest, second_manifest)
            self.assertEqual(
                (first / "data/ToS/derived-exports/runtime/knowledge.sqlite3").read_bytes(),
                (second / "data/ToS/derived-exports/runtime/knowledge.sqlite3").read_bytes(),
            )
            self.assertEqual(
                sorted(path.relative_to(first).as_posix() for path in first.rglob("*")),
                sorted(path.relative_to(second).as_posix() for path in second.rglob("*")),
            )
        finally:
            shutil.rmtree(workspace)

    def test_verify_rejects_corruption_missing_and_extra_files(self) -> None:
        workspace, software, data = self._fixture()
        try:
            output = workspace / "snapshot"
            self._build(software, data, output)
            source = output / "data" / INPUTS["entities"]
            source.write_bytes(source.read_bytes() + b"\ncorrupted")
            with self.assertRaisesRegex(RuntimeError, "integrity mismatch"):
                verify_data_snapshot(output)

            source.unlink()
            with self.assertRaisesRegex(RuntimeError, "files differ"):
                verify_data_snapshot(output)

            source.write_bytes((data / INPUTS["entities"]).read_bytes())
            extra = output / "data" / "unexpected.bin"
            extra.write_bytes(b"extra")
            with self.assertRaisesRegex(RuntimeError, "files differ"):
                verify_data_snapshot(output)
        finally:
            shutil.rmtree(workspace)

    def test_invalid_input_and_compile_failure_leave_no_final_output(self) -> None:
        workspace, software, data = self._fixture()
        try:
            invalid = data / INPUTS["entities"]
            invalid.write_text("{invalid json\n", encoding="utf-8")
            output = workspace / "snapshot"
            with self.assertRaisesRegex(RuntimeError, "isolated query-store compilation failed"):
                self._build(software, data, output)
            self.assertFalse(output.exists())
            self.assertFalse(list(workspace.glob("tos-data-snapshot-*")))
        finally:
            shutil.rmtree(workspace)

    def test_existing_output_is_refused_without_replacement(self) -> None:
        workspace, software, data = self._fixture()
        try:
            output = workspace / "snapshot"
            output.mkdir()
            sentinel = output / "sentinel"
            sentinel.write_bytes(b"keep")
            with self.assertRaisesRegex(RuntimeError, "refusing to overwrite"):
                self._build(software, data, output)
            self.assertEqual(sentinel.read_bytes(), b"keep")
        finally:
            shutil.rmtree(workspace)

    def test_final_publish_does_not_replace_directory_created_after_precheck(self) -> None:
        workspace, software, data = self._fixture()
        try:
            output = workspace / "snapshot"
            original_publish = data_snapshot_builder._rename_noreplace

            def race(stage: Path, destination: Path) -> None:
                destination.mkdir()
                original_publish(stage, destination)

            with patch.object(data_snapshot_builder, "_rename_noreplace", side_effect=race):
                with self.assertRaisesRegex(RuntimeError, "refusing to overwrite"):
                    self._build(software, data, output)
            self.assertTrue(output.is_dir())
            self.assertEqual(list(output.iterdir()), [])
        finally:
            shutil.rmtree(workspace)

    def test_compiler_code_from_data_root_is_ignored(self) -> None:
        workspace, software, data = self._fixture()
        try:
            sentinel = workspace / "data-compiler-executed"
            shadow = data / "access/src/tos_access"
            shadow.mkdir(parents=True)
            (shadow / "__init__.py").write_text("", encoding="utf-8")
            (shadow / "knowledge_compile.py").write_text(
                "from pathlib import Path\n"
                f"Path({str(sentinel)!r}).write_text('executed')\n"
                "raise RuntimeError('data compiler executed')\n",
                encoding="utf-8",
            )
            output = workspace / "snapshot"
            self._build(software, data, output)
            self.assertFalse(sentinel.exists())
            self.assertTrue((output / "manifest.json").is_file())
        finally:
            shutil.rmtree(workspace)

    def test_exact_reuse_skips_compiler_and_preserves_revision(self) -> None:
        workspace, software, data = self._fixture()
        try:
            first = workspace / "first"
            first_manifest = self._build(software, data, first)
            core_path = software / "access/src/tos_access/core.py"
            core_path.write_text(
                core_path.read_text(encoding="utf-8")
                + "\n# UI/API-only cache reuse drift proof\n",
                encoding="utf-8",
            )
            original_loader = data_snapshot_builder._load_data_compile_common

            def fail_if_compiled(*args: object, **kwargs: object) -> None:
                raise AssertionError("matching snapshot reuse must not compile")

            def patched_loader(root: Path):
                helper, helper_name = original_loader(root)
                helper._compile_query_store = fail_if_compiled
                return helper, helper_name

            second = workspace / "second"
            with patch.object(
                data_snapshot_builder,
                "_load_data_compile_common",
                side_effect=patched_loader,
            ):
                second_manifest = build_data_snapshot(
                    software,
                    data,
                    second,
                    corpus_revision="a" * 64,
                    reuse_snapshot=first,
                )

            self.assertEqual(second_manifest, first_manifest)
            self.assertEqual(
                (second / "data" / QUERY_STORE_RELATIVE_PATH).read_bytes(),
                (first / "data" / QUERY_STORE_RELATIVE_PATH).read_bytes(),
            )
        finally:
            shutil.rmtree(workspace)

    def test_changed_input_ignores_valid_cache_and_compiles_new_revision(self) -> None:
        workspace, software, data = self._fixture()
        try:
            first = workspace / "first"
            first_manifest = self._build(software, data, first)
            corpus_path = data / INPUTS["corpus"]
            corpus_path.write_text(
                corpus_path.read_text(encoding="utf-8") + "\n",
                encoding="utf-8",
            )
            original_loader = data_snapshot_builder._load_data_compile_common
            compile_calls: list[None] = []

            def patched_loader(root: Path):
                helper, helper_name = original_loader(root)
                original_compile = helper._compile_query_store

                def counting_compile(*args: object, **kwargs: object):
                    compile_calls.append(None)
                    return original_compile(*args, **kwargs)

                helper._compile_query_store = counting_compile
                return helper, helper_name

            second_manifest = None
            second = workspace / "second"
            with patch.object(
                data_snapshot_builder,
                "_load_data_compile_common",
                side_effect=patched_loader,
            ):
                second_manifest = build_data_snapshot(
                    software,
                    data,
                    second,
                    corpus_revision="a" * 64,
                    reuse_snapshot=first,
                )

            self.assertEqual(len(compile_calls), 1)
            self.assertIsNotNone(second_manifest)
            self.assertNotEqual(second_manifest["data_revision"], first_manifest["data_revision"])
        finally:
            shutil.rmtree(workspace)

    def test_compiler_source_change_ignores_valid_cache_and_compiles_new_revision(self) -> None:
        workspace, software, data = self._fixture()
        try:
            first = workspace / "first"
            first_manifest = self._build(software, data, first)
            data_access_path = software / "access/src/tos_access/data_access.py"
            data_access_path.write_text(
                data_access_path.read_text(encoding="utf-8")
                + "\n# compiler dependency cache invalidation proof\n",
                encoding="utf-8",
            )
            original_loader = data_snapshot_builder._load_data_compile_common
            compile_calls: list[None] = []

            def patched_loader(root: Path):
                helper, helper_name = original_loader(root)
                original_compile = helper._compile_query_store

                def counting_compile(*args: object, **kwargs: object):
                    compile_calls.append(None)
                    return original_compile(*args, **kwargs)

                helper._compile_query_store = counting_compile
                return helper, helper_name

            second = workspace / "second"
            with patch.object(
                data_snapshot_builder,
                "_load_data_compile_common",
                side_effect=patched_loader,
            ):
                second_manifest = build_data_snapshot(
                    software,
                    data,
                    second,
                    corpus_revision="a" * 64,
                    reuse_snapshot=first,
                )

            self.assertEqual(len(compile_calls), 1)
            self.assertNotEqual(
                second_manifest["compiler"]["compiler_sha256"],
                first_manifest["compiler"]["compiler_sha256"],
            )
            self.assertNotEqual(second_manifest["data_revision"], first_manifest["data_revision"])
        finally:
            shutil.rmtree(workspace)

    def test_compiler_fingerprint_tracks_readable_context_and_helper(self) -> None:
        workspace, software, data = self._fixture()
        try:
            compiler_paths = query_store_compiler_paths(software)
            self.assertIn(
                "access/src/tos_access/readable_context.py", compiler_paths
            )
            helper_relative = "access/src/tos_access/normalization_cache.py"
            self.assertIn(helper_relative, compiler_paths)
            for relative in ("access/src/tos_access/readable_context.py", helper_relative):
                with self.subTest(dependency=relative):
                    before = _query_store_compiler_fingerprint(software)
                    helper = software / relative
                    helper.write_bytes(
                        helper.read_bytes() + b"\n# cache dependency fingerprint probe\n"
                    )
                    self.assertNotEqual(before, _query_store_compiler_fingerprint(software))
        finally:
            shutil.rmtree(workspace)

    def test_compiler_fingerprint_excludes_core_and_web_ui_changes(self) -> None:
        workspace, software, data = self._fixture()
        try:
            before = _query_store_compiler_fingerprint(software)
            core = software / "access/src/tos_access/core.py"
            core.write_bytes(core.read_bytes() + b"\n# serving adapter probe\n")
            self.assertEqual(before, _query_store_compiler_fingerprint(software))

            web_index = software / "access/web/index.html"
            web_index.parent.mkdir(parents=True)
            web_index.write_text("<!-- UI-only probe -->\n", encoding="utf-8")
            self.assertEqual(before, _query_store_compiler_fingerprint(software))
        finally:
            shutil.rmtree(workspace)

    def test_clean_compiler_import_isolated_from_lazy_core_export(self) -> None:
        workspace, software, data = self._fixture()
        try:
            environment = dict(os.environ)
            environment["PYTHONPATH"] = str(software / "access/src")
            probe = subprocess.run(
                [
                    sys.executable,
                    "-c",
                    (
                        "import sys; "
                        "import tos_access.knowledge_compile; "
                        "assert 'tos_access.core' not in sys.modules; "
                        "from tos_access import ToSAccessCore; "
                        "assert ToSAccessCore.__name__ == 'ToSAccessCore'; "
                        "print('ok')"
                    ),
                ],
                cwd=workspace,
                env=environment,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            self.assertEqual(probe.returncode, 0, probe.stderr)
            self.assertEqual(probe.stdout.strip(), "ok")
        finally:
            shutil.rmtree(workspace)

    def test_corrupt_reuse_snapshot_fails_before_publishing_output(self) -> None:
        workspace, software, data = self._fixture()
        try:
            reuse = workspace / "reuse"
            self._build(software, data, reuse)
            query_store = reuse / "data" / QUERY_STORE_RELATIVE_PATH
            query_store.write_bytes(query_store.read_bytes() + b"\ncorrupted")
            output = workspace / "new-snapshot"
            with self.assertRaisesRegex(RuntimeError, "member integrity mismatch"):
                build_data_snapshot(
                    software,
                    data,
                    output,
                    corpus_revision="a" * 64,
                    reuse_snapshot=reuse,
                )
            self.assertFalse(output.exists())
        finally:
            shutil.rmtree(workspace)

    def test_source_revision_is_only_supplied_identity(self) -> None:
        workspace, software, data = self._fixture()
        try:
            output = workspace / "snapshot"
            manifest = self._build(software, data, output)
            self.assertNotIn("source_ref", manifest)
            self.assertNotIn("source_git", json.dumps(manifest))
            self.assertNotIn("timestamp", json.dumps(manifest))
            with self.assertRaisesRegex(RuntimeError, "64 lowercase"):
                build_data_snapshot(software, data, workspace / "bad", corpus_revision="short")
        finally:
            shutil.rmtree(workspace)

    def test_future_query_store_abi_requires_explicit_integrity_only_mode(self) -> None:
        workspace, software, data = self._fixture()
        try:
            output = workspace / "snapshot"
            manifest = self._build(software, data, output)
            future_version = "tos_offline_knowledge_future_v999"
            query_store = output / "data" / QUERY_STORE_RELATIVE_PATH
            with sqlite3.connect(query_store) as connection:
                connection.execute(
                    "UPDATE metadata SET value=? WHERE key='compiler_version'",
                    (json.dumps(future_version, separators=(",", ":")),),
                )
                connection.commit()

            query_member = next(
                member
                for member in manifest["members"]
                if member["path"] == f"data/{QUERY_STORE_RELATIVE_PATH}"
            )
            query_member["sha256"] = hashlib.sha256(query_store.read_bytes()).hexdigest()
            query_member["size_bytes"] = query_store.stat().st_size
            manifest["compiler"]["compiler_version"] = future_version
            body = {
                key: manifest[key]
                for key in (
                    "schema_version",
                    "corpus_revision",
                    "input_bindings",
                    "compiler",
                    "members",
                )
            }
            manifest["data_revision"] = hashlib.sha256(
                (
                    json.dumps(
                        body,
                        ensure_ascii=False,
                        sort_keys=True,
                        separators=(",", ":"),
                    )
                    + "\n"
                ).encode("utf-8")
            ).hexdigest()
            (output / "manifest.json").write_bytes(
                (
                    json.dumps(
                        manifest,
                        ensure_ascii=False,
                        sort_keys=True,
                        separators=(",", ":"),
                    )
                    + "\n"
                ).encode("utf-8")
            )

            import tos_access.data_snapshot as portable_verifier

            with patch.object(
                portable_verifier.sqlite3,
                "connect",
                side_effect=AssertionError("default compatibility must precede SQLite"),
            ):
                with self.assertRaisesRegex(RuntimeError, "ABI is incompatible"):
                    verify_data_snapshot(output)
            self.assertEqual(
                verify_data_snapshot(output, require_compatible=False), manifest
            )
        finally:
            shutil.rmtree(workspace)

    def test_portable_verifier_runs_from_installed_copy_without_packaging(self) -> None:
        workspace, software, data = self._fixture()
        try:
            output = workspace / "snapshot"
            manifest = self._build(software, data, output)
            installed = workspace / "installed"
            shutil.copytree(
                ACCESS_ROOT / "src/tos_access",
                installed / "tos_access",
                ignore=shutil.ignore_patterns("__pycache__", "*.pyc", "runtime_data"),
            )
            outside = workspace / "outside"
            outside.mkdir()
            environment = dict(os.environ)
            environment["PYTHONPATH"] = str(installed)
            result = subprocess.run(
                [
                    sys.executable,
                    "-c",
                    (
                        "import json, sys; "
                        "from pathlib import Path; "
                        "from tos_access.data_snapshot import verify_data_snapshot; "
                        "print(json.dumps(verify_data_snapshot(Path(sys.argv[1]))))"
                    ),
                    str(output),
                ],
                cwd=outside,
                env=environment,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(json.loads(result.stdout), manifest)
            self.assertFalse((installed / "access/packaging").exists())
        finally:
            shutil.rmtree(workspace)

    def test_cli_roundtrip_and_corruption_reject_preserves_manifest(self) -> None:
        workspace, software, data = self._fixture()
        try:
            output = workspace / "cli-snapshot"
            builder_script = ACCESS_ROOT / "packaging/build_data_snapshot.py"
            environment = dict(os.environ)
            environment.pop("PYTHONPATH", None)
            build = subprocess.run(
                [
                    sys.executable,
                    str(builder_script),
                    "build",
                    "--software-root",
                    str(software),
                    "--data-root",
                    str(data),
                    "--corpus-revision",
                    "a" * 64,
                    "--output",
                    str(output),
                ],
                cwd=workspace,
                env=environment,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            self.assertEqual(build.returncode, 0, build.stderr)
            manifest = json.loads(build.stdout)
            self.assertEqual(verify_data_snapshot(output), manifest)

            verify = subprocess.run(
                [sys.executable, str(builder_script), "verify", str(output)],
                cwd=workspace,
                env=environment,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            self.assertEqual(verify.returncode, 0, verify.stderr)
            self.assertEqual(json.loads(verify.stdout)["data_revision"], manifest["data_revision"])

            manifest_bytes = (output / "manifest.json").read_bytes()
            member = output / "data" / INPUTS["entities"]
            member.write_bytes(member.read_bytes() + b"\ncorrupted")
            rejected = subprocess.run(
                [sys.executable, str(builder_script), "verify", str(output)],
                cwd=workspace,
                env=environment,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            self.assertNotEqual(rejected.returncode, 0)
            self.assertIn("member integrity mismatch", rejected.stderr)
            self.assertEqual((output / "manifest.json").read_bytes(), manifest_bytes)
        finally:
            shutil.rmtree(workspace)


if __name__ == "__main__":
    unittest.main()
