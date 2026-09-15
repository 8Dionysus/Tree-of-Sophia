from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import unittest


TESTS_ROOT = Path(__file__).resolve().parent
ACCESS_ROOT = TESTS_ROOT.parent
SRC_ROOT = ACCESS_ROOT / "src"
PACKAGING_ROOT = ACCESS_ROOT / "packaging"
for path in (TESTS_ROOT, SRC_ROOT, PACKAGING_ROOT):
    if str(path) not in sys.path:
        sys.path.insert(0, str(path))

from build_data_snapshot import build_data_snapshot  # noqa: E402
from build_software_bundle import build_software_bundle  # noqa: E402
from fixture_support import write_fixture  # noqa: E402
from release_pair import _reader_abi, prepare_pair, verify_pair  # noqa: E402
import test_data_snapshot as _data_snapshot_tests  # noqa: E402
import test_software_bundle as _software_bundle_tests  # noqa: E402
from tos_access.release_state import ReleaseStateError, ReleaseStore  # noqa: E402
from tos_access.query_store import COMPILER_VERSION  # noqa: E402
from validate_software_bundle import verify_archive  # noqa: E402


class ReleasePairTests(unittest.TestCase):
    CORPUS_REVISIONS = ("a" * 64, "b" * 64, "c" * 64)

    @staticmethod
    def _sha256(path: Path) -> str:
        digest = hashlib.sha256()
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(block)
        return digest.hexdigest()

    @staticmethod
    def _git_commit(root: Path, message: str) -> str:
        if not (root / ".git").is_dir():
            subprocess.run(
                ["git", "init", "-q"],
                cwd=root,
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            subprocess.run(
                ["git", "config", "user.email", "fixture@example.test"],
                cwd=root,
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            subprocess.run(
                ["git", "config", "user.name", "Release Pair Fixture"],
                cwd=root,
                check=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
        subprocess.run(
            ["git", "add", "-A"],
            cwd=root,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        subprocess.run(
            ["git", "commit", "-qm", message],
            cwd=root,
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"],
            cwd=root,
            text=True,
        ).strip()

    def _fixture(self, revisions: tuple[str, ...] = (CORPUS_REVISIONS[0],)) -> tuple[
        tempfile.TemporaryDirectory[str], dict
    ]:
        temporary = tempfile.TemporaryDirectory(prefix="tos-release-pair-")
        root = Path(temporary.name)

        software_repo = root / "software-repo"
        _software_bundle_tests.SoftwareBundleBoundaryTests()._make_repo(software_repo)
        source_ref = self._git_commit(software_repo, "clean software fixture")
        software_archive = root / "software.zip"
        software_manifest = build_software_bundle(
            software_repo,
            software_archive,
            source_ref=source_ref,
        )

        data_software = _data_snapshot_tests.DataSnapshotTests()._software_root(
            root / "data-software"
        )
        data_source = root / "data-source"
        write_fixture(data_source)
        snapshots: dict[str, Path] = {}
        manifests: dict[str, dict] = {}
        for index, revision in enumerate(revisions):
            snapshot = root / f"data-snapshot-{index}"
            manifests[revision] = build_data_snapshot(
                data_software,
                data_source,
                snapshot,
                corpus_revision=revision,
            )
            snapshots[revision] = snapshot

        return temporary, {
            "root": root,
            "software_repo": software_repo,
            "source_ref": source_ref,
            "software_archive": software_archive,
            "software_manifest": software_manifest,
            "data_source": data_source,
            "snapshots": snapshots,
            "manifests": manifests,
        }

    @staticmethod
    def _bindings(fixture: dict, snapshot: Path) -> dict[str, str]:
        return {
            "data_root": str(snapshot.absolute()),
            "software_archive": str(fixture["software_archive"].absolute()),
        }

    @staticmethod
    def _run_bounded(function, *, timeout: float = 15.0):
        outcome: list[object] = []

        def invoke() -> None:
            try:
                outcome.append(function())
            except BaseException as exc:  # pragma: no cover - raised in caller below.
                outcome.append(exc)

        worker = threading.Thread(target=invoke, daemon=True)
        worker.start()
        worker.join(timeout)
        if worker.is_alive():
            raise AssertionError("release operation deadlocked inside pair verifier")
        if len(outcome) != 1:
            raise AssertionError("release operation did not return an outcome")
        result = outcome[0]
        if isinstance(result, BaseException):
            raise result
        return result

    def test_prepare_pair_records_exact_digests_and_reader_abi(self) -> None:
        temporary, fixture = self._fixture()
        try:
            revision = self.CORPUS_REVISIONS[0]
            snapshot = fixture["snapshots"][revision]
            pair = prepare_pair(fixture["software_archive"], snapshot)
            software = verify_archive(fixture["software_archive"])
            data = json.loads((snapshot / "manifest.json").read_text(encoding="utf-8"))
            schema, version = _reader_abi(fixture["software_archive"])

            self.assertFalse(software["source_dirty"])
            self.assertEqual(
                pair,
                {
                    "schema_version": "tos_access_release_pair_v1",
                    "software_sha256": self._sha256(fixture["software_archive"]),
                    "data_revision": data["data_revision"],
                    "data_manifest_sha256": self._sha256(snapshot / "manifest.json"),
                    "corpus_revision": revision,
                    "query_schema": schema,
                    "compiler_version": version,
                },
            )
            self.assertIsNone(
                verify_pair(pair, self._bindings(fixture, snapshot))
            )
        finally:
            temporary.cleanup()

    def test_mismatched_reader_abi_is_rejected_without_executing_archive_code(self) -> None:
        temporary, fixture = self._fixture()
        try:
            marker = fixture["root"] / "archive-code-executed"
            query_store = fixture["software_repo"] / "access/src/tos_access/query_store.py"
            source = query_store.read_text(encoding="utf-8")
            source = source.replace(
                "from __future__ import annotations\n",
                "from __future__ import annotations\n"
                f"from pathlib import Path as _ArchiveMarkerPath\n"
                f"_ArchiveMarkerPath({str(marker)!r}).write_text('executed', encoding='utf-8')\n",
                1,
            )
            source = source.replace(
                "SCHEMA = 'tos_query_store_v1'",
                "SCHEMA = 'tos_query_store_future_v1'",
                1,
            )
            self.assertIn(f"COMPILER_VERSION = {COMPILER_VERSION!r}", source)
            source = source.replace(
                f"COMPILER_VERSION = {COMPILER_VERSION!r}",
                "COMPILER_VERSION = 'tos_offline_knowledge_future_v1'",
                1,
            )
            query_store.write_text(source, encoding="utf-8")
            future_ref = self._git_commit(fixture["software_repo"], "future reader ABI")
            future_archive = fixture["root"] / "future-reader.zip"
            future_manifest = build_software_bundle(
                fixture["software_repo"],
                future_archive,
                source_ref=future_ref,
            )
            self.assertFalse(future_manifest["source_dirty"])

            with self.assertRaisesRegex(ValueError, "incompatible"):
                prepare_pair(future_archive, fixture["snapshots"][self.CORPUS_REVISIONS[0]])
            self.assertFalse(marker.exists())
        finally:
            temporary.cleanup()

    def test_dirty_software_archive_is_rejected(self) -> None:
        temporary, fixture = self._fixture()
        try:
            readme = fixture["software_repo"] / "access/README.md"
            readme.write_bytes(readme.read_bytes() + b"\ndirty fixture\n")
            dirty_archive = fixture["root"] / "dirty.zip"
            dirty_manifest = build_software_bundle(
                fixture["software_repo"],
                dirty_archive,
                source_ref=fixture["source_ref"],
                allow_dirty=True,
            )
            self.assertTrue(dirty_manifest["source_dirty"])
            with self.assertRaisesRegex(ValueError, "clean source-bound"):
                prepare_pair(dirty_archive, fixture["snapshots"][self.CORPUS_REVISIONS[0]])
        finally:
            temporary.cleanup()

    def test_corrupted_data_and_software_are_rejected(self) -> None:
        temporary, fixture = self._fixture()
        try:
            snapshot = fixture["snapshots"][self.CORPUS_REVISIONS[0]]
            data_member = snapshot / "data/ToS/derived-exports/tos_corpus_index.min.json"
            original_data = data_member.read_bytes()
            data_member.write_bytes(original_data + b"\ncorrupted release data\n")
            with self.assertRaisesRegex(RuntimeError, "member integrity mismatch"):
                prepare_pair(fixture["software_archive"], snapshot)
            data_member.write_bytes(original_data)

            archive = fixture["software_archive"]
            corrupted = bytearray(archive.read_bytes())
            corrupted[len(corrupted) // 2] ^= 1
            archive.write_bytes(corrupted)
            with self.assertRaisesRegex(RuntimeError, "archive digest mismatch"):
                prepare_pair(archive, snapshot)
        finally:
            temporary.cleanup()

    def test_promote_and_rollback_use_exact_pairs_and_revocation_cannot_be_bypassed(self) -> None:
        temporary, fixture = self._fixture(self.CORPUS_REVISIONS)
        try:
            pair_one = prepare_pair(
                fixture["software_archive"],
                fixture["snapshots"][self.CORPUS_REVISIONS[0]],
            )
            pair_two = prepare_pair(
                fixture["software_archive"],
                fixture["snapshots"][self.CORPUS_REVISIONS[1]],
            )
            pair_three = prepare_pair(
                fixture["software_archive"],
                fixture["snapshots"][self.CORPUS_REVISIONS[2]],
            )
            bindings_one = self._bindings(
                fixture, fixture["snapshots"][self.CORPUS_REVISIONS[0]]
            )
            bindings_two = self._bindings(
                fixture, fixture["snapshots"][self.CORPUS_REVISIONS[1]]
            )
            bindings_three = self._bindings(
                fixture, fixture["snapshots"][self.CORPUS_REVISIONS[2]]
            )
            store = ReleaseStore(fixture["root"] / "release-state")
            first_id = store.promote(
                pair_one,
                bindings_one,
                expected_current=None,
                verify_pair=verify_pair,
            )
            second_id = store.promote(
                pair_two,
                bindings_two,
                expected_current=first_id,
                verify_pair=verify_pair,
            )
            self.assertEqual(store.read_selection()["pair_id"], second_id)
            self.assertEqual(
                store.rollback(expected_current=second_id, verify_pair=verify_pair),
                first_id,
            )
            self.assertEqual(store.read_selection()["previous"], second_id)
            second_id = store.promote(
                pair_two,
                bindings_two,
                expected_current=first_id,
                verify_pair=verify_pair,
            )

            def revoke_previous(pair: dict, bindings: dict) -> None:
                del bindings
                store.revoke(
                    "data",
                    pair["data_revision"],
                    reason="previous pair test revocation",
                    owner_ref="test:release-pair",
                )

            with self.assertRaisesRegex(ReleaseStateError, "revoked"):
                self._run_bounded(
                    lambda: store.rollback(
                        expected_current=second_id,
                        verify_pair=revoke_previous,
                    )
                )
            self.assertEqual(store.read_selection()["pair_id"], second_id)

            def revoke_candidate(pair: dict, bindings: dict) -> None:
                del bindings
                store.revoke(
                    "data",
                    pair["data_revision"],
                    reason="verifier-side promotion revocation",
                    owner_ref="test:release-pair-verifier",
                )

            with self.assertRaisesRegex(ReleaseStateError, "revoked"):
                self._run_bounded(
                    lambda: store.promote(
                        pair_three,
                        bindings_three,
                        expected_current=second_id,
                        verify_pair=revoke_candidate,
                    )
                )
            self.assertEqual(store.read_selection()["pair_id"], second_id)
        finally:
            temporary.cleanup()

    def test_cli_prepare_and_status_report_the_real_pair(self) -> None:
        temporary, fixture = self._fixture()
        try:
            snapshot = fixture["snapshots"][self.CORPUS_REVISIONS[0]]
            pair = prepare_pair(fixture["software_archive"], snapshot)
            bindings = self._bindings(fixture, snapshot)
            pair_path = fixture["root"] / "pair.json"
            outside = fixture["root"] / "outside"
            outside.mkdir()
            script = PACKAGING_ROOT / "release_pair.py"
            prepare_result = subprocess.run(
                [
                    sys.executable,
                    str(script),
                    "prepare",
                    "--software",
                    str(fixture["software_archive"]),
                    "--data",
                    str(snapshot),
                    "--output",
                    str(pair_path),
                ],
                cwd=outside,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            self.assertEqual(prepare_result.returncode, 0, prepare_result.stderr)
            self.assertEqual(json.loads(prepare_result.stdout), pair)
            self.assertEqual(json.loads(pair_path.read_bytes()), pair)

            store = ReleaseStore(fixture["root"] / "release-state")
            pair_id = store.promote(
                pair,
                bindings,
                expected_current=None,
                verify_pair=verify_pair,
            )
            status_result = subprocess.run(
                [sys.executable, str(script), "status", "--state", str(store.root)],
                cwd=outside,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=False,
            )
            self.assertEqual(status_result.returncode, 0, status_result.stderr)
            status = json.loads(status_result.stdout)
            self.assertEqual(status["pair_id"], pair_id)
            self.assertEqual(status["pair"], pair)
            self.assertEqual(status["bindings"], bindings)
        finally:
            temporary.cleanup()


if __name__ == "__main__":
    unittest.main()
