from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import corpus_locator  # noqa: E402
import corpus_store  # noqa: E402


def _git(root: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args], cwd=root, check=True, capture_output=True, text=True
    )
    return completed.stdout.strip()


class CorpusLocatorTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="tos-corpus-locator-")
        self.root = Path(self.temporary.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        _git(self.repo, "init", "--quiet")
        _git(self.repo, "config", "user.email", "fixture@example.invalid")
        _git(self.repo, "config", "user.name", "Synthetic Fixture")

        self._write_repo("docs/a.txt", b"historical bytes\n")
        self._write_repo("bin/run.sh", b"#!/bin/sh\necho historical\n", mode=0o755)
        self._write_repo("gone.txt", b"present only in the first commit\n")
        self._write_repo("literal*", b"literal path\n")
        (self.repo / "unsafe-link").symlink_to("docs/a.txt")
        _git(self.repo, "add", ".")
        _git(self.repo, "commit", "--quiet", "-m", "first fixture")
        self.commit1 = _git(self.repo, "rev-parse", "HEAD")

        self._write_repo("docs/a.txt", b"newer bytes in the second commit\n")
        (self.repo / "gone.txt").unlink()
        _git(self.repo, "add", "-A")
        _git(self.repo, "commit", "--quiet", "-m", "second fixture")
        self.commit2 = _git(self.repo, "rev-parse", "HEAD")

        self.store = corpus_store.CorpusStore(self.root / "store")
        self.source_root = self.root / "sources"
        self.source_root.mkdir()
        self.validator_sha256 = "a" * 64
        self.corpus_old = self._admit_corpus(b"old corpus bytes\n", None)
        self.corpus_new = self._admit_corpus(b"new corpus bytes\n", self.corpus_old["revision"])

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _write_repo(self, relative: str, value: bytes, *, mode: int = 0o644) -> None:
        path = self.repo / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(value)
        os.chmod(path, mode)

    def _admit_corpus(self, value: bytes, base_revision: str | None) -> dict:
        source = self.source_root / "record.json"
        source.write_bytes(value)
        os.chmod(source, 0o644)

        def validate(
            candidate: corpus_store.CorpusCandidate,
            base: dict | None,
            affected: frozenset[str],
        ) -> corpus_store.ValidationIndex:
            del base, affected
            if candidate.read_bytes("records/a.json") != value:
                raise corpus_store.CorpusStoreError("fixture validator saw unexpected bytes")
            return corpus_store.ValidationIndex(
                {"stable:record-a": "records/a.json"}, {"records/a.json": []}
            )

        return self.store.admit(
            base_revision=base_revision,
            updates={
                "records/a.json": {
                    "source": source,
                    "sha256": hashlib.sha256(value).hexdigest(),
                    "size_bytes": len(value),
                    "mode": 0o644,
                }
            },
            retirements={},
            validator_sha256=self.validator_sha256,
            validate=validate,
        )

    def test_revision_resolves_by_immutable_id_or_exact_path(self) -> None:
        descriptor_by_id = corpus_locator.resolve_revision(
            self.store, self.corpus_old["revision"], source_id="stable:record-a"
        )
        descriptor_by_path = corpus_locator.resolve_revision(
            self.store, self.corpus_old["revision"], path="records/a.json"
        )
        self.assertEqual(descriptor_by_id, descriptor_by_path)
        self.assertEqual(
            descriptor_by_id,
            {
                "kind": "corpus",
                "revision": self.corpus_old["revision"],
                "path": "records/a.json",
                "sha256": hashlib.sha256(b"old corpus bytes\n").hexdigest(),
                "size_bytes": len(b"old corpus bytes\n"),
                "mode": 0o644,
            },
        )
        with self.assertRaises(corpus_locator.CorpusLocatorError):
            corpus_locator.resolve_revision(self.store, self.corpus_old["revision"])
        with self.assertRaises(corpus_locator.CorpusLocatorError):
            corpus_locator.resolve_revision(
                self.store,
                self.corpus_old["revision"],
                source_id="stable:record-a",
                path="records/a.json",
            )
        for bad_id in ("missing", ""):
            with self.assertRaises(corpus_locator.CorpusLocatorError):
                corpus_locator.resolve_revision(
                    self.store, self.corpus_old["revision"], source_id=bad_id
                )
        for bad_path in ("missing.json", "../records/a.json", "records//a.json", "/records/a.json"):
            with self.assertRaises(corpus_locator.CorpusLocatorError):
                corpus_locator.resolve_revision(
                    self.store, self.corpus_old["revision"], path=bad_path
                )
        for bad_revision in ("current", "HEAD", "0" * 64):
            with self.assertRaises(corpus_locator.CorpusLocatorError):
                corpus_locator.resolve_revision(self.store, bad_revision, path="records/a.json")

    def test_revision_lookup_hashes_only_the_selected_object(self) -> None:
        selected = self.corpus_old["revision"]
        selected_object = self.store.root / "objects" / hashlib.sha256(
            b"old corpus bytes\n"
        ).hexdigest()
        unrelated_object = self.store.root / "objects" / hashlib.sha256(
            b"new corpus bytes\n"
        ).hexdigest()
        self.assertTrue(unrelated_object.exists())
        os.chmod(unrelated_object, 0o644)
        unrelated_object.write_bytes(b"unrelated corruption")

        with patch.object(
            corpus_store, "digest_file", wraps=corpus_store.digest_file
        ) as digest_file:
            descriptor = corpus_locator.resolve_revision(
                self.store, selected, source_id="stable:record-a"
            )

        self.assertEqual(descriptor["sha256"], selected_object.name)
        self.assertEqual(
            [call.args[0] for call in digest_file.call_args_list], [selected_object]
        )

    def test_revision_restore_is_historical_exact_and_exclusive(self) -> None:
        descriptor = corpus_locator.resolve_revision(
            self.store, self.corpus_old["revision"], source_id="stable:record-a"
        )
        output = self.root / "restored" / "old.json"
        receipt = corpus_locator.restore_revision_source(self.store, descriptor, output)
        self.assertEqual(output.read_bytes(), b"old corpus bytes\n")
        self.assertEqual(output.stat().st_mode & 0o777, 0o644)
        self.assertEqual(receipt["sha256"], descriptor["sha256"])
        self.assertEqual(receipt["revision"], self.corpus_old["revision"])

        existing = self.root / "existing"
        existing.write_bytes(b"keep me")
        with self.assertRaises(corpus_locator.CorpusLocatorError):
            corpus_locator.restore_revision_source(self.store, descriptor, existing)
        self.assertEqual(existing.read_bytes(), b"keep me")

        broken = self.root / "broken"
        broken.symlink_to(self.root / "does-not-exist")
        with self.assertRaises(corpus_locator.CorpusLocatorError):
            corpus_locator.restore_revision_source(self.store, descriptor, broken)
        self.assertTrue(broken.is_symlink())

        for field, value in (("sha256", "b" * 64), ("size_bytes", 1), ("path", "other.json")):
            tampered = dict(descriptor)
            tampered[field] = value
            target = self.root / f"tampered-{field}"
            with self.assertRaises(corpus_locator.CorpusLocatorError):
                corpus_locator.restore_revision_source(self.store, tampered, target)
            self.assertFalse(target.exists())

        object_path = self.store.root / "objects" / descriptor["sha256"]
        os.chmod(object_path, 0o644)
        object_path.write_bytes(b"corrupt object")
        with self.assertRaisesRegex(corpus_locator.CorpusLocatorError, "corrupt corpus object"):
            corpus_locator.resolve_revision(
                self.store, descriptor["revision"], path=descriptor["path"]
            )
        corrupt_target = self.root / "corrupt-target"
        with self.assertRaises(corpus_locator.CorpusLocatorError):
            corpus_locator.restore_revision_source(self.store, descriptor, corrupt_target)
        self.assertFalse(corrupt_target.exists())

    def test_git_resolves_literal_historical_paths_and_restores_exact_bytes(self) -> None:
        descriptor = corpus_locator.resolve_git(self.repo, self.commit1, "docs/a.txt")
        self.assertEqual(descriptor["kind"], "git")
        self.assertEqual(descriptor["commit"], self.commit1)
        self.assertEqual(descriptor["path"], "docs/a.txt")
        self.assertEqual(descriptor["size_bytes"], len(b"historical bytes\n"))
        self.assertEqual(descriptor["mode"], 0o644)

        output = self.root / "git-restored" / "a.txt"
        receipt = corpus_locator.restore_git_source(self.repo, descriptor, output)
        self.assertEqual(output.read_bytes(), b"historical bytes\n")
        self.assertEqual(output.stat().st_mode & 0o777, 0o644)
        self.assertEqual(receipt["sha256"], hashlib.sha256(output.read_bytes()).hexdigest())

        executable = corpus_locator.resolve_git(self.repo, self.commit1, "bin/run.sh")
        self.assertEqual(executable["mode"], 0o755)
        executable_output = self.root / "run.sh"
        corpus_locator.restore_git_source(self.repo, executable, executable_output)
        self.assertEqual(executable_output.stat().st_mode & 0o777, 0o755)

        literal = corpus_locator.resolve_git(self.repo, self.commit1, "literal*")
        self.assertEqual(literal["path"], "literal*")
        self.assertEqual(literal["size_bytes"], len(b"literal path\n"))

        historical_deleted = corpus_locator.resolve_git(self.repo, self.commit1, "gone.txt")
        deleted_output = self.root / "gone.txt"
        corpus_locator.restore_git_source(self.repo, historical_deleted, deleted_output)
        self.assertEqual(deleted_output.read_bytes(), b"present only in the first commit\n")
        with self.assertRaises(corpus_locator.CorpusLocatorError):
            corpus_locator.resolve_git(self.repo, self.commit2, "gone.txt")

    def test_git_revalidates_descriptor_bounds_and_unsafe_sources(self) -> None:
        descriptor = corpus_locator.resolve_git(self.repo, self.commit1, "docs/a.txt")
        for field, value in (
            ("git_blob_oid", "0" * 40),
            ("size_bytes", descriptor["size_bytes"] + 1),
            ("mode", 0o755),
            ("path", "gone.txt"),
            ("commit", self.commit2),
        ):
            tampered = dict(descriptor)
            tampered[field] = value
            target = self.root / f"git-tampered-{field}"
            with self.assertRaises(corpus_locator.CorpusLocatorError):
                corpus_locator.restore_git_source(self.repo, tampered, target)
            self.assertFalse(target.exists())

        too_small = self.root / "too-small"
        with self.assertRaises(corpus_locator.CorpusLocatorError):
            corpus_locator.restore_git_source(
                self.repo, descriptor, too_small, max_bytes=descriptor["size_bytes"] - 1
            )
        self.assertFalse(too_small.exists())

        for unsafe in ("../docs/a.txt", "docs//a.txt", "/docs/a.txt", ".git/config", "docs\\a.txt"):
            with self.assertRaises(corpus_locator.CorpusLocatorError):
                corpus_locator.resolve_git(self.repo, self.commit1, unsafe)
        with self.assertRaises(corpus_locator.CorpusLocatorError):
            corpus_locator.resolve_git(self.repo, "HEAD", "docs/a.txt")
        with self.assertRaises(corpus_locator.CorpusLocatorError):
            corpus_locator.resolve_git(self.repo, "0" * 40, "docs/a.txt")
        with self.assertRaises(corpus_locator.CorpusLocatorError):
            corpus_locator.resolve_git(self.repo, self.commit1, "unsafe-link")

        alias = self.root / "repo-alias"
        alias.symlink_to(self.repo, target_is_directory=True)
        with self.assertRaises(corpus_locator.CorpusLocatorError):
            corpus_locator.resolve_git(alias, self.commit1, "docs/a.txt")

        existing = self.root / "git-existing"
        existing.write_bytes(b"preserve")
        with self.assertRaises(corpus_locator.CorpusLocatorError):
            corpus_locator.restore_git_source(self.repo, descriptor, existing)
        self.assertEqual(existing.read_bytes(), b"preserve")

        dangling = self.root / "git-dangling"
        dangling.symlink_to(self.root / "not-present")
        with self.assertRaises(corpus_locator.CorpusLocatorError):
            corpus_locator.restore_git_source(self.repo, descriptor, dangling)
        self.assertTrue(dangling.is_symlink())

    def test_exclusive_hardlink_rejects_a_path_that_appears_during_publish(self) -> None:
        descriptor = corpus_locator.resolve_git(self.repo, self.commit1, "docs/a.txt")
        output = self.root / "raced-output"

        def race(temporary: Path, destination: Path, *, follow_symlinks: bool) -> None:
            del temporary, follow_symlinks
            destination.write_bytes(b"concurrent owner")
            raise FileExistsError(destination)

        with patch.object(corpus_locator.os, "link", side_effect=race):
            with self.assertRaises(corpus_locator.CorpusLocatorError):
                corpus_locator.restore_git_source(self.repo, descriptor, output)
        self.assertEqual(output.read_bytes(), b"concurrent owner")

    def test_git_cli_resolve_and_restore_use_strict_descriptor_json(self) -> None:
        script = ROOT / "scripts" / "corpus_locator.py"
        descriptor_result = subprocess.run(
            [
                sys.executable,
                str(script),
                "git-resolve",
                "--git-root",
                str(self.repo),
                "--commit",
                self.commit1,
                "--path",
                "docs/a.txt",
            ],
            check=True,
            capture_output=True,
            text=True,
        )
        descriptor = json.loads(descriptor_result.stdout)
        expected = corpus_locator.resolve_git(self.repo, self.commit1, "docs/a.txt")
        self.assertEqual(descriptor, expected)
        self.assertEqual(
            descriptor_result.stdout,
            json.dumps(descriptor, sort_keys=True, separators=(",", ":")) + "\n",
        )

        descriptor_path = self.root / "git-descriptor.json"
        descriptor_path.write_text(json.dumps(descriptor, indent=2), encoding="utf-8")
        output = self.root / "cli-restore" / "a.txt"
        restore_result = subprocess.run(
            [
                sys.executable,
                str(script),
                "git-restore",
                "--git-root",
                str(self.repo),
                "--descriptor",
                str(descriptor_path),
                "--output",
                str(output),
            ],
            check=True,
            capture_output=True,
            text=True,
        )
        receipt = json.loads(restore_result.stdout)
        self.assertEqual(output.read_bytes(), b"historical bytes\n")
        self.assertEqual(receipt["kind"], "git")
        self.assertEqual(receipt["sha256"], hashlib.sha256(output.read_bytes()).hexdigest())

    def test_corpus_cli_requires_an_existing_store(self) -> None:
        script = ROOT / "scripts" / "corpus_locator.py"
        missing = self.root / "store-does-not-exist"
        result = subprocess.run(
            [
                sys.executable,
                str(script),
                "corpus-resolve",
                "--store",
                str(missing),
                "--revision",
                self.corpus_old["revision"],
                "--path",
                "records/a.json",
            ],
            capture_output=True,
            text=True,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(missing.exists())


if __name__ == "__main__":
    unittest.main()
