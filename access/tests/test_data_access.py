from __future__ import annotations

from contextlib import contextmanager
import gc
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
from typing import Any, Iterator
import unittest
from unittest.mock import patch


TESTS_ROOT = Path(__file__).resolve().parent
ACCESS_ROOT = TESTS_ROOT.parent
if str(TESTS_ROOT) not in sys.path:
    sys.path.insert(0, str(TESTS_ROOT))
if str(ACCESS_ROOT / "packaging") not in sys.path:
    sys.path.insert(0, str(ACCESS_ROOT / "packaging"))
if str(ACCESS_ROOT / "src") not in sys.path:
    sys.path.insert(0, str(ACCESS_ROOT / "src"))

import build_data_snapshot as data_snapshot_builder  # noqa: E402
import test_data_snapshot as _data_snapshot_tests  # noqa: E402
from tos_access.core import ToSAccessCore  # noqa: E402
from tos_access.data_access import (  # noqa: E402
    DataAccessUnavailable,
    check_data_path,
)
from tos_access.release_state import (  # noqa: E402
    PAIR_SCHEMA,
    ReleaseStateError,
    ReleaseStore,
    pair_id_for,
)


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _canonical(value: Any) -> bytes:
    return (
        json.dumps(
            value,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("utf-8")


@contextmanager
def _snapshot_fixture(corpus_revision: str = "a" * 64) -> Iterator[tuple[Path, Path, Path, Path, dict[str, Any]]]:
    """Build one small disposable snapshot from the existing test fixture."""
    helper = _data_snapshot_tests.DataSnapshotTests()
    workspace, software, data = helper._fixture()
    try:
        snapshot = workspace / "snapshot"
        manifest = data_snapshot_builder.build_data_snapshot(
            software,
            data,
            snapshot,
            corpus_revision=corpus_revision,
        )
        # A parent process must not accidentally turn an un-managed fixture
        # inspection into a release selection. Managed tests set this again.
        with patch.dict(os.environ, {"TOS_RELEASE_ROOT": ""}, clear=False):
            yield workspace, software, data, snapshot, manifest
    finally:
        shutil.rmtree(workspace)


@contextmanager
def _managed_environment(release_root: Path) -> Iterator[None]:
    """Select only the temporary release store through the real env route."""
    empty_overrides = {
        "TOS_DATA_ROOT": "",
        "TOS_ROOT": "",
        "AOA_TOS_ROOT": "",
        "TOS_QUERY_STORE_PATH": "",
        "TOS_CORPUS_INDEX_PATH": "",
        "TOS_PHILOSOPHY_GRAPH_PROJECTION_PATH": "",
        "TOS_BIBLIOGRAPHIC_GRAPH_PATH": "",
        "TOS_ENTITY_TYPE_REGISTRY_PATH": "",
        "TOS_RELATION_TYPE_REGISTRY_PATH": "",
        "TOS_PHILOSOPHY_POST_PLANTING_AUDIT_PATH": "",
        "TOS_EVIDENCE_PROJECTION_PATH": "",
        "TOS_RELEASE_ROOT": str(release_root),
    }
    with patch.dict(os.environ, empty_overrides, clear=False):
        yield


def _pair(manifest: dict[str, Any], snapshot: Path, *, software_seed: str) -> dict[str, Any]:
    compiler = manifest["compiler"]
    return {
        "schema_version": PAIR_SCHEMA,
        "software_sha256": (software_seed * 64)[:64],
        "data_revision": manifest["data_revision"],
        "data_manifest_sha256": _sha256(snapshot / "manifest.json"),
        "corpus_revision": manifest["corpus_revision"],
        "query_schema": compiler["schema"],
        "compiler_version": compiler["compiler_version"],
    }


def _bindings(workspace: Path, snapshot: Path, *, software_seed: str) -> dict[str, str]:
    return {
        "data_root": snapshot.absolute().as_posix(),
        "software_archive": (workspace / f"software-{software_seed}.zip").absolute().as_posix(),
    }


def _promote(
    store: ReleaseStore,
    pair: dict[str, Any],
    bindings: dict[str, str],
    expected_current: str | None,
) -> str:
    return store.promote(
        pair,
        bindings,
        expected_current=expected_current,
        verify_pair=lambda verified_pair, verified_bindings: None,
    )


class DataAccessIntegrationTests(unittest.TestCase):
    def test_discover_reads_verified_artifact_and_serves_query(self) -> None:
        with _snapshot_fixture() as (workspace, software, data, snapshot, manifest):
            del workspace, software, data, manifest
            core = ToSAccessCore.discover(tos_root=snapshot / "data")
            self.assertIsNotNone(core._data_guard)
            result = core.knowledge_search(
                "Альфа",
                sources=["philosophy"],
                limit=5,
            )
            self.assertEqual(result["nodes"][0]["id"], "philosophy:a")
            self.assertTrue(core.status()["index_exists"])

    def test_data_guard_rejects_corrupt_and_incompatible_artifact(self) -> None:
        with _snapshot_fixture() as (workspace, software, data, snapshot, manifest):
            del workspace, software, data, manifest
            member = snapshot / "data/ToS/derived-exports/tos_corpus_index.min.json"
            member.write_bytes(member.read_bytes() + b"\ncorrupt")
            with self.assertRaisesRegex(RuntimeError, "integrity mismatch"):
                ToSAccessCore.discover(tos_root=snapshot / "data")

        with _snapshot_fixture() as (workspace, software, data, snapshot, manifest):
            del workspace, software, data
            changed = dict(manifest)
            changed["compiler"] = dict(changed["compiler"])
            changed["compiler"]["compiler_version"] = "future-incompatible-v0"
            body = {key: value for key, value in changed.items() if key != "data_revision"}
            changed["data_revision"] = hashlib.sha256(_canonical(body)).hexdigest()
            (snapshot / "manifest.json").write_bytes(_canonical(changed))
            with self.assertRaisesRegex(RuntimeError, "incompatible"):
                ToSAccessCore.discover(tos_root=snapshot / "data")

    def test_managed_pair_binding_and_revocations_gate_existing_core(self) -> None:
        with _snapshot_fixture() as (workspace, software, data, snapshot, manifest):
            del software, data
            for kind in ("data", "corpus"):
                with self.subTest(kind=kind):
                    state = ReleaseStore(workspace / f"release-{kind}")
                    pair = _pair(manifest, snapshot, software_seed=kind[0])
                    bindings = _bindings(workspace, snapshot, software_seed=kind[0])
                    current = _promote(state, pair, bindings, None)
                    with _managed_environment(state.root):
                        core = ToSAccessCore.discover()
                        self.assertEqual(core.tos_root, snapshot / "data")
                        self.assertEqual(core._data_guard.pair, pair)
                        before = core.knowledge_search(
                            "Альфа",
                            sources=["philosophy"],
                            limit=5,
                        )
                        self.assertTrue(before["nodes"])
                        digest = pair["data_revision"] if kind == "data" else pair["corpus_revision"]
                        state.revoke(
                            kind,
                            digest,
                            reason=f"integration test {kind} withdrawal",
                            owner_ref="test_data_access",
                        )
                        with self.assertRaises(DataAccessUnavailable):
                            core.knowledge_search(
                                "Альфа",
                                sources=["philosophy"],
                                limit=5,
                            )
                    self.assertEqual(current, pair_id_for(pair))

    def test_rollback_uses_actual_core_and_refuses_revoked_previous(self) -> None:
        helper = _data_snapshot_tests.DataSnapshotTests()
        workspace, software, data = helper._fixture()
        try:
            snapshot_one = workspace / "snapshot-one"
            snapshot_two = workspace / "snapshot-two"
            manifest_one = data_snapshot_builder.build_data_snapshot(
                software,
                data,
                snapshot_one,
                corpus_revision="a" * 64,
            )
            manifest_two = data_snapshot_builder.build_data_snapshot(
                software,
                data,
                snapshot_two,
                corpus_revision="b" * 64,
            )
            state = ReleaseStore(workspace / "release")
            pair_one = _pair(manifest_one, snapshot_one, software_seed="1")
            pair_two = _pair(manifest_two, snapshot_two, software_seed="2")
            bindings_one = _bindings(workspace, snapshot_one, software_seed="1")
            bindings_two = _bindings(workspace, snapshot_two, software_seed="2")
            first_id = _promote(state, pair_one, bindings_one, None)
            second_id = _promote(state, pair_two, bindings_two, first_id)

            with _managed_environment(state.root):
                current_core = ToSAccessCore.discover()
                self.assertTrue(current_core.status()["index_exists"])
                self.assertEqual(
                    state.rollback(
                        expected_current=second_id,
                        verify_pair=lambda verified_pair, verified_bindings: None,
                    ),
                    first_id,
                )
                previous_core = ToSAccessCore.discover()
                self.assertTrue(previous_core.status()["index_exists"])

                promoted_again = _promote(state, pair_two, bindings_two, first_id)
                self.assertEqual(promoted_again, second_id)
                state.revoke(
                    "data",
                    pair_one["data_revision"],
                    reason="previous data must not be resurrected",
                    owner_ref="test_data_access",
                )
                still_current = ToSAccessCore.discover()
                self.assertTrue(still_current.status()["index_exists"])
                with self.assertRaisesRegex(ReleaseStateError, "revoked"):
                    state.rollback(
                        expected_current=second_id,
                        verify_pair=lambda verified_pair, verified_bindings: None,
                    )
                self.assertEqual(state.read_selection()["pair_id"], second_id)
        finally:
            shutil.rmtree(workspace)

    def test_check_data_path_is_member_specific_and_software_is_not_guarded(self) -> None:
        with _snapshot_fixture() as (workspace, software, data, snapshot, manifest):
            del data
            state = ReleaseStore(workspace / "release")
            pair = _pair(manifest, snapshot, software_seed="c")
            bindings = _bindings(workspace, snapshot, software_seed="c")
            _promote(state, pair, bindings, None)
            with _managed_environment(state.root):
                core = ToSAccessCore.discover()
                guard = core._data_guard
                self.assertIsNotNone(guard)
                assert guard is not None
                software_file = software / "unrelated-software.py"
                software_file.write_text("# outside selected data\n", encoding="utf-8")
                # Direct helper calls only guard a path while a decorated Core
                # method is active; software remains outside that boundary.
                self.assertIsNone(check_data_path(software_file))
                undeclared = snapshot / "data/undeclared.txt"
                undeclared.write_text("not in the manifest\n", encoding="utf-8")
                with self.assertRaises(DataAccessUnavailable):
                    guard.check_path(undeclared)

                member = core.index_path
                original = member.read_bytes()
                member.write_bytes(original + b"\nchanged")
                with self.assertRaises(DataAccessUnavailable):
                    guard.check_path(member)
                with self.assertRaises(DataAccessUnavailable):
                    core.status()

    def test_data_root_word_provider_shadow_never_executes(self) -> None:
        with _snapshot_fixture() as (workspace, software, data, snapshot, manifest):
            del software, data, manifest
            core = ToSAccessCore.discover(tos_root=snapshot / "data")
            sentinel = workspace / "data-root-provider-executed"
            shadow = snapshot / "data/scripts/prepare_zarathustra_word_analysis_v1.py"
            shadow.parent.mkdir(parents=True)
            shadow.write_text(
                "from pathlib import Path\n"
                f"Path({str(sentinel)!r}).write_text('executed', encoding='utf-8')\n"
                "raise AssertionError('data-root provider must not execute')\n",
                encoding="utf-8",
            )

            from tos_access import core as core_module

            original_program_path = core_module.program_path
            missing_program_provider = workspace / "missing-program-provider.py"

            def selected_program_path(relative: str | Path) -> Path:
                if Path(relative) == core_module.WORD_ANALYSIS_PROVIDER_RELATIVE_PATH:
                    return missing_program_provider
                return original_program_path(relative)

            with patch.object(
                core_module,
                "program_path",
                side_effect=selected_program_path,
            ):
                result = core.zarathustra_word_analysis_task("судьбы", "ru")
            self.assertFalse(result["available"])
            self.assertEqual(
                result["reason"],
                "local source-bound word-analysis provider is not installed",
            )
            self.assertFalse(sentinel.exists())

    def test_query_store_override_cannot_escape_selected_snapshot(self) -> None:
        with _snapshot_fixture() as (workspace, software, data, snapshot, manifest):
            del software, data
            state = ReleaseStore(workspace / "release")
            pair = _pair(manifest, snapshot, software_seed="e")
            bindings = _bindings(workspace, snapshot, software_seed="e")
            _promote(state, pair, bindings, None)
            selected_store = snapshot / "data/ToS/derived-exports/runtime/knowledge.sqlite3"
            external_store = workspace / "external-copy.sqlite3"
            shutil.copy2(selected_store, external_store)
            self.assertEqual(external_store.read_bytes(), selected_store.read_bytes())

            with _managed_environment(state.root):
                core = ToSAccessCore.discover()
                selected_pointer = state.read_selection()["pair_id"]
                with patch.dict(
                    os.environ,
                    {"TOS_QUERY_STORE_PATH": str(external_store)},
                    clear=False,
                ):
                    with self.assertRaises(DataAccessUnavailable):
                        core.knowledge_search(
                            "Альфа",
                            sources=["philosophy"],
                            limit=5,
                        )
                self.assertEqual(state.read_selection()["pair_id"], selected_pointer)

    def test_one_snapshot_guard_survives_second_core_collection_and_revocation(self) -> None:
        with _snapshot_fixture() as (workspace, software, data, snapshot, manifest):
            del software, data
            state = ReleaseStore(workspace / "release")
            pair = _pair(manifest, snapshot, software_seed="7")
            bindings = _bindings(workspace, snapshot, software_seed="7")
            _promote(state, pair, bindings, None)
            with _managed_environment(state.root):
                first = ToSAccessCore.discover()
                second = ToSAccessCore.discover()
                first_guard = first._data_guard
                second_guard = second._data_guard
                self.assertIsNotNone(first_guard)
                self.assertIsNotNone(second_guard)
                self.assertIsNot(first_guard, second_guard)
                del second
                gc.collect()

                changed = first.index_path
                changed.write_bytes(changed.read_bytes() + b"\nchanged after second Core GC")
                assert first_guard is not None
                with self.assertRaises(DataAccessUnavailable):
                    first_guard.check_path(changed)

                state.revoke(
                    "data",
                    pair["data_revision"],
                    reason="first Core must retain the withdrawal guard",
                    owner_ref="test_data_access",
                )
                with self.assertRaisesRegex(DataAccessUnavailable, "revoked"):
                    first.status()

    def test_interleaved_cores_keep_context_local(self) -> None:
        with _snapshot_fixture() as (workspace, software, data, snapshot, manifest):
            del manifest
            snapshot_b = workspace / "snapshot-b"
            data_snapshot_builder.build_data_snapshot(
                software,
                data,
                snapshot_b,
                corpus_revision="b" * 64,
            )
            core_a = ToSAccessCore.discover(tos_root=snapshot / "data")
            core_b = ToSAccessCore.discover(tos_root=snapshot_b / "data")
            original_query_store = ToSAccessCore._query_store
            events: list[str] = []

            def interleaving_query_store(core: ToSAccessCore):
                if core is core_a:
                    events.append("a-before-b")
                    nested = core_b.status()
                    self.assertTrue(nested["index_exists"])
                    events.append("a-after-b")
                return original_query_store(core)

            with patch.object(
                ToSAccessCore,
                "_query_store",
                new=interleaving_query_store,
            ):
                result = core_a.knowledge_search(
                    "Альфа",
                    sources=["philosophy"],
                    limit=5,
                )
            self.assertEqual(result["nodes"][0]["id"], "philosophy:a")
            self.assertEqual(events, ["a-before-b", "a-after-b"])

            member_a = core_a.tos_root / "ToS/derived-exports/runtime/knowledge.sqlite3"
            member_b = core_b.tos_root / "ToS/derived-exports/runtime/knowledge.sqlite3"
            member_a.write_bytes(member_a.read_bytes() + b"\nchanged-a")
            member_b.write_bytes(member_b.read_bytes() + b"\nchanged-b")
            # The nested B call must restore A, and the outer A call must then
            # restore the caller's empty context. Neither changed path is
            # checked by a leaked guard here.
            self.assertIsNone(check_data_path(member_a))
            self.assertIsNone(check_data_path(member_b))
            with self.assertRaises(DataAccessUnavailable):
                core_a.status()
            with self.assertRaises(DataAccessUnavailable):
                core_b.status()


if __name__ == "__main__":
    unittest.main()
