from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from corpus_archive import (  # noqa: E402
    CorpusArchiveError,
    capture_git,
    restore_capture,
    verify_capture,
)


def _git(root: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args], cwd=root, check=True, capture_output=True, text=True
    )
    return completed.stdout.strip()


class CorpusArchiveTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="tos-corpus-archive-")
        self.root = Path(self.temporary.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        _git(self.repo, "init", "--quiet")
        _git(self.repo, "config", "user.email", "fixture@example.invalid")
        _git(self.repo, "config", "user.name", "Synthetic Fixture")
        self._write("src/a.txt", b"alpha\n")
        self._write("src/nested/b.txt", b"beta\n")
        self._write("srcx/outside.txt", b"boundary\n")
        self._write("other.txt", b"other\n")
        self._write("src/run.sh", b"#!/bin/sh\necho fixture\n")
        (self.repo / "src/run.sh").chmod(0o755)
        _git(self.repo, "add", ".")
        _git(self.repo, "commit", "--quiet", "-m", "fixture")
        self.commit = _git(self.repo, "rev-parse", "HEAD")

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _write(self, relative: str, value: bytes) -> None:
        path = self.repo / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(value)

    def _capture(
        self,
        name: str = "capture",
        prefixes: list[str] | None = None,
        *,
        exclude_prefixes: list[str] | None = None,
        exclude_path_parts: list[str] | None = None,
    ) -> Path:
        output = self.root / name
        capture_git(
            self.repo,
            self.commit,
            prefixes or ["src"],
            output,
            exclude_prefixes=exclude_prefixes,
            exclude_path_parts=exclude_path_parts,
        )
        return output

    def _commit_files(self, files: dict[str, bytes]) -> None:
        for relative, value in files.items():
            self._write(relative, value)
        _git(self.repo, "add", ".")
        _git(self.repo, "commit", "--quiet", "-m", "excluded fixture")
        self.commit = _git(self.repo, "rev-parse", "HEAD")

    @staticmethod
    def _manifest(path: Path) -> dict:
        return json.loads((path / "capture.json").read_text(encoding="utf-8"))

    def test_capture_verify_restore_preserves_selected_bytes_and_modes(self) -> None:
        capture = self._capture()
        manifest = verify_capture(capture)
        self.assertEqual(manifest["schema_version"], "tos_corpus_capture_v1")
        self.assertEqual(manifest["source_git_commit"], self.commit)
        self.assertEqual(
            [json.loads(line)["path"] for line in (capture / "members.jsonl").read_text().splitlines()],
            ["src/a.txt", "src/nested/b.txt", "src/run.sh"],
        )
        destination = self.root / "restored"
        receipt = restore_capture(capture, destination)
        self.assertEqual(receipt["source_git_commit"], self.commit)
        self.assertEqual(receipt["member_count"], 3)
        self.assertEqual(receipt["source_bytes"], len(b"alpha\n") + len(b"beta\n") + len(b"#!/bin/sh\necho fixture\n"))
        for relative in ("src/a.txt", "src/nested/b.txt", "src/run.sh"):
            self.assertEqual((destination / relative).read_bytes(), (self.repo / relative).read_bytes())
        self.assertEqual((destination / "src/run.sh").stat().st_mode & 0o777, 0o755)
        self.assertEqual((destination / "src/a.txt").stat().st_mode & 0o777, 0o644)
        self.assertTrue((destination / "restore-receipt.json").is_file())

    def test_capture_is_deterministic_and_prefix_boundary_is_exact(self) -> None:
        first = self._capture("first", prefixes=["src/"])
        second = self._capture("second", prefixes=["src"])
        self.assertEqual((first / "source.tar.gz").read_bytes(), (second / "source.tar.gz").read_bytes())
        self.assertEqual((first / "members.jsonl").read_bytes(), (second / "members.jsonl").read_bytes())
        self.assertEqual((first / "capture.json").read_bytes(), (second / "capture.json").read_bytes())
        self.assertNotIn("srcx/outside.txt", (first / "members.jsonl").read_text())
        self.assertEqual(self._manifest(first)["schema_version"], "tos_corpus_capture_v1")
        self.assertNotIn("exclude_prefixes", self._manifest(first))
        self.assertNotIn("exclude_path_parts", self._manifest(first))

    def test_v2_capture_normalizes_exclusions_before_reading_blobs(self) -> None:
        self._commit_files(
            {
                "src/payload/secret.txt": b"payload sentinel\n",
                "src/.owner-local/private.txt": b"owner sentinel\n",
                "src/nested/keep.txt": b"kept\n",
            }
        )
        capture = self._capture(
            "v2",
            prefixes=["src/", "src"],
            exclude_prefixes=["src/payload/", "src/payload"],
            exclude_path_parts=[".owner-local", ".owner-local"],
        )
        manifest = verify_capture(capture)
        self.assertEqual(manifest["schema_version"], "tos_corpus_capture_v2")
        self.assertEqual(manifest["include_prefixes"], ["src"])
        self.assertEqual(manifest["exclude_prefixes"], ["src/payload"])
        self.assertEqual(manifest["exclude_path_parts"], [".owner-local"])
        members = [
            json.loads(line)
            for line in (capture / "members.jsonl").read_text(encoding="utf-8").splitlines()
        ]
        self.assertEqual(
            [member["path"] for member in members],
            ["src/a.txt", "src/nested/b.txt", "src/nested/keep.txt", "src/run.sh"],
        )
        with tarfile.open(capture / "source.tar.gz", mode="r:gz") as archive:
            self.assertEqual(archive.getnames(), [member["path"] for member in members])
            self.assertEqual(archive.extractfile("src/nested/keep.txt").read(), b"kept\n")

    def test_v2_verifier_rejects_forged_excluded_members_after_outer_rehash(self) -> None:
        self._commit_files(
            {
                "src/payload/secret.txt": b"payload sentinel\n",
                "src/.owner-local/private.txt": b"owner sentinel\n",
            }
        )
        for excluded_path, expected_text in (
            ("src/payload/secret.txt", "excluded prefix"),
            ("src/.owner-local/private.txt", "excluded path part"),
        ):
            capture = self._capture(
                excluded_path.replace("/", "-") + "-forged",
                exclude_prefixes=["src/payload"],
                exclude_path_parts=[".owner-local"],
            )
            members_path = capture / "members.jsonl"
            rows = [json.loads(line) for line in members_path.read_text().splitlines()]
            rows[0]["path"] = excluded_path
            members_path.write_bytes(
                b"".join(
                    json.dumps(row, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8") + b"\n"
                    for row in sorted(rows, key=lambda row: row["path"])
                )
            )
            manifest_path = capture / "capture.json"
            manifest = self._manifest(capture)
            manifest["members_sha256"] = hashlib.sha256(members_path.read_bytes()).hexdigest()
            manifest_path.write_bytes(
                json.dumps(manifest, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode("utf-8") + b"\n"
            )
            with self.assertRaisesRegex(CorpusArchiveError, expected_text):
                verify_capture(capture)

    def test_exclusion_lists_and_v2_manifest_are_strict(self) -> None:
        malformed = (
            {"exclude_prefixes": [""]},
            {"exclude_prefixes": ["/absolute"]},
            {"exclude_prefixes": ["src\\payload"]},
            {"exclude_prefixes": ["src/../payload"]},
            {"exclude_path_parts": [""]},
            {"exclude_path_parts": ["."]},
            {"exclude_path_parts": [".."]},
            {"exclude_path_parts": ["owner/local"]},
            {"exclude_path_parts": ["owner\\local"]},
            {"exclude_path_parts": ["owner\x01local"]},
        )
        for exclusions in malformed:
            with self.subTest(exclusions=exclusions):
                with self.assertRaisesRegex(CorpusArchiveError, "exclude"):
                    capture_git(self.repo, self.commit, ["src"], self.root / "bad", **exclusions)
                self.assertFalse((self.root / "bad").exists())

        capture = self._capture("v2-strict", exclude_path_parts=["nested"])
        manifest_path = capture / "capture.json"
        manifest = self._manifest(capture)
        manifest["exclude_path_parts"] = ["nested", "nested"]
        manifest_path.write_bytes(
            json.dumps(manifest, sort_keys=True, separators=(",", ":")).encode("utf-8") + b"\n"
        )
        with self.assertRaisesRegex(CorpusArchiveError, "exclude_path_parts"):
            verify_capture(capture)

    def test_v2_restore_contains_only_selected_members(self) -> None:
        self._commit_files(
            {
                "src/payload/secret.txt": b"payload sentinel\n",
                "src/.owner-local/private.txt": b"owner sentinel\n",
            }
        )
        capture = self._capture(
            "v2-restore",
            exclude_prefixes=["src/payload"],
            exclude_path_parts=[".owner-local"],
        )
        destination = self.root / "v2-restored"
        receipt = restore_capture(capture, destination)
        self.assertEqual(receipt["schema_version"], "tos_corpus_restore_receipt_v1")
        self.assertTrue((destination / "src/a.txt").is_file())
        self.assertFalse((destination / "src/payload/secret.txt").exists())
        self.assertFalse((destination / "src/.owner-local/private.txt").exists())

    def test_capture_reads_exact_commit_and_ignores_dirty_worktree(self) -> None:
        (self.repo / "src/a.txt").write_bytes(b"dirty working tree\n")
        capture = self._capture()
        self.assertEqual((self.root / "capture" / "members.jsonl").read_text().count("src/a.txt"), 1)
        restored = self.root / "restored-dirty"
        restore_capture(capture, restored)
        self.assertEqual((restored / "src/a.txt").read_bytes(), b"alpha\n")

    def test_existing_output_and_unsafe_git_entry_are_refused(self) -> None:
        existing = self.root / "existing"
        existing.mkdir()
        with self.assertRaises(CorpusArchiveError):
            capture_git(self.repo, self.commit, ["src"], existing)

        unsafe_repo = self.root / "unsafe-repo"
        unsafe_repo.mkdir()
        _git(unsafe_repo, "init", "--quiet")
        _git(unsafe_repo, "config", "user.email", "fixture@example.invalid")
        _git(unsafe_repo, "config", "user.name", "Synthetic Fixture")
        (unsafe_repo / "real.txt").write_bytes(b"target\n")
        (unsafe_repo / "link.txt").symlink_to("real.txt")
        _git(unsafe_repo, "add", "real.txt", "link.txt")
        _git(unsafe_repo, "commit", "--quiet", "-m", "unsafe")
        unsafe_commit = _git(unsafe_repo, "rev-parse", "HEAD")
        with self.assertRaises(CorpusArchiveError):
            capture_git(unsafe_repo, unsafe_commit, ["link.txt"], self.root / "unsafe-capture")
        self.assertFalse((self.root / "unsafe-capture" / "capture.json").exists())

    def test_archive_corruption_and_rehashed_manifest_are_refused(self) -> None:
        capture = self._capture()
        archive = capture / "source.tar.gz"
        corrupted = bytearray(archive.read_bytes())
        corrupted[len(corrupted) // 2] ^= 1
        archive.write_bytes(corrupted)
        manifest_path = capture / "capture.json"
        manifest = self._manifest(capture)
        manifest["archive_sha256"] = hashlib.sha256(corrupted).hexdigest()
        manifest["archive_size_bytes"] = len(corrupted)
        manifest_path.write_text(json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n")
        with self.assertRaises(CorpusArchiveError):
            verify_capture(capture)

    def test_git_blob_oid_tampering_is_refused_after_member_rehash(self) -> None:
        capture = self._capture("oid-tampered")
        members_path = capture / "members.jsonl"
        rows = [json.loads(line) for line in members_path.read_text().splitlines()]
        rows[0]["git_blob_oid"] = "0" * 40
        members_path.write_text(
            "".join(json.dumps(row, sort_keys=True, separators=(",", ":")) + "\n" for row in rows),
            encoding="utf-8",
        )
        manifest_path = capture / "capture.json"
        manifest = self._manifest(capture)
        manifest["members_sha256"] = hashlib.sha256(members_path.read_bytes()).hexdigest()
        manifest_path.write_text(
            json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n",
            encoding="utf-8",
        )
        with self.assertRaises(CorpusArchiveError):
            verify_capture(capture)

    def test_member_tampering_duplicate_and_incomplete_capture_are_refused(self) -> None:
        capture = self._capture()
        members = capture / "members.jsonl"
        lines = members.read_bytes().splitlines(keepends=True)
        members.write_bytes(lines[0] + lines[0] + b"".join(lines[1:]))
        manifest_path = capture / "capture.json"
        manifest = self._manifest(capture)
        manifest["members_sha256"] = hashlib.sha256(members.read_bytes()).hexdigest()
        manifest["member_count"] += 1
        manifest["source_bytes"] *= 1
        manifest_path.write_text(json.dumps(manifest, sort_keys=True, separators=(",", ":")) + "\n")
        with self.assertRaises(CorpusArchiveError):
            verify_capture(capture)

        incomplete = self.root / "incomplete"
        incomplete.mkdir()
        (incomplete / "source.tar.gz").write_bytes(b"partial")
        (incomplete / "members.jsonl").write_bytes(b"")
        with self.assertRaises(CorpusArchiveError):
            verify_capture(incomplete)

    def test_cli_capture_verify_restore(self) -> None:
        capture = self.root / "cli-capture"
        script = ROOT / "scripts" / "corpus_archive.py"
        captured = subprocess.run(
            [sys.executable, str(script), "capture", "--repo-root", str(self.repo), "--commit", self.commit,
             "--include-prefix", "src", "--output", str(capture)],
            check=False, capture_output=True, text=True,
        )
        self.assertEqual(captured.returncode, 0, captured.stderr)
        verified = subprocess.run(
            [sys.executable, str(script), "verify", "--capture", str(capture)],
            check=False, capture_output=True, text=True,
        )
        self.assertEqual(verified.returncode, 0, verified.stderr)
        restored = self.root / "cli-restored"
        restored_result = subprocess.run(
            [sys.executable, str(script), "restore", "--capture", str(capture), "--output", str(restored)],
            check=False, capture_output=True, text=True,
        )
        self.assertEqual(restored_result.returncode, 0, restored_result.stderr)
        self.assertEqual((restored / "src/a.txt").read_bytes(), b"alpha\n")


if __name__ == "__main__":
    unittest.main()
