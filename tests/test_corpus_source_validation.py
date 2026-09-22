from __future__ import annotations

from contextlib import contextmanager
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from typing import Iterator
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import corpus_source_validation as source_validation  # noqa: E402
import build_source_witness_catalog  # noqa: E402
from corpus_archive import capture_git, restore_capture  # noqa: E402
from corpus_store import CorpusStoreError  # noqa: E402
import validate_source_witness_foundation as foundation  # noqa: E402
import source_record_profiles  # noqa: E402


def _git(root: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    )
    return completed.stdout.strip()


def _write(root: Path, relative: str, content: bytes) -> Path:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(content)
    return path


def _canonical(value: object) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
        + "\n"
    ).encode("utf-8")


class SourceSnapshotMembershipTests(unittest.TestCase):
    def test_snapshot_membership_is_exact_nested_and_context_local(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-source-membership-") as raw:
            root = Path(raw) / "snapshot"
            manifest = _write(
                root,
                "ToS/contracts/manifest.json",
                _canonical({"$schema": "https://json-schema.org/draft/2020-12/schema", "type": "object"}),
            )
            private = root / "ToS/source-witnesses/owner-local/private.json"
            relative_manifest = manifest.relative_to(root).as_posix()
            self.assertFalse((root / ".git").exists())
            self.assertIsNone(foundation._git_tracked(root, manifest))
            self.assertIsNone(foundation._git_ignored(root, private))

            other_root = Path(raw) / "other"
            other_manifest = _write(other_root, relative_manifest, b"{}\n")
            with patch.object(
                foundation.subprocess,
                "run",
                side_effect=AssertionError("snapshot membership must not invoke git"),
            ):
                with foundation.source_snapshot_membership(
                    root,
                    frozenset({relative_manifest}),
                ):
                    self.assertTrue(foundation._git_tracked(root, manifest))
                    self.assertFalse(foundation._git_ignored(root, manifest))
                    self.assertFalse(foundation._git_tracked(root, private))
                    self.assertTrue(foundation._git_ignored(root, private))
                    with self.assertRaisesRegex(ValueError, "root differs"):
                        foundation._git_tracked(other_root, other_manifest)

                    with foundation.source_snapshot_membership(
                        other_root,
                        frozenset({relative_manifest}),
                    ):
                        self.assertTrue(foundation._git_tracked(other_root, other_manifest))
                        self.assertFalse(foundation._git_ignored(other_root, other_manifest))

                    # The nested selection is restored to the outer root.
                    self.assertTrue(foundation._git_tracked(root, manifest))
                    self.assertTrue(foundation._git_ignored(root, private))

            # Successful exit clears the selection, so a no-Git root again
            # yields unknown rather than retaining the old membership.
            self.assertIsNone(foundation._git_tracked(root, manifest))
            self.assertIsNone(foundation._git_ignored(root, private))

            with self.assertRaisesRegex(RuntimeError, "membership failure"):
                with foundation.source_snapshot_membership(
                    root,
                    frozenset({relative_manifest}),
                ):
                    raise RuntimeError("membership failure")
            self.assertIsNone(foundation._git_tracked(root, manifest))
            self.assertIsNone(foundation._git_ignored(root, private))


class ValidatorIdentityTests(unittest.TestCase):
    def test_identity_tracks_transitive_local_code_not_unrelated_packaging(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-validator-identity-") as raw:
            root = Path(raw)
            software_root = root / "software"
            entry_modules = (
                "corpus_source_validation",
                "validate_source_witness_foundation",
                "build_source_witness_catalog",
            )
            for module in entry_modules:
                _write(
                    software_root,
                    f"scripts/{module}.py",
                    b"from local_validator_helper import HELPER_VALUE\nENTRY = HELPER_VALUE\n",
                )
            helper = _write(
                software_root,
                "scripts/local_validator_helper.py",
                b"HELPER_VALUE = 'v1'\n",
            )
            packaging = _write(
                software_root,
                "scripts/packaging.py",
                b"PACKAGE_VALUE = 'v1'\n",
            )
            grammar = _grammar_root(root)

            with patch.object(source_validation, "SOFTWARE_ROOT", software_root):
                initial = source_validation.validator_identity(grammar)
                packaging.write_bytes(b"PACKAGE_VALUE = 'unrelated change'\n")
                self.assertEqual(initial, source_validation.validator_identity(grammar))
                helper.write_bytes(b"HELPER_VALUE = 'v2'\n")
                self.assertNotEqual(initial, source_validation.validator_identity(grammar))
                helper.write_bytes(b"from claim_version_reader import LIMIT\n")
                mechanic = _write(software_root,
                    'mechanics/growth-cycle/parts/branch-growth-cycle/scripts/claim_version_reader.py',
                    b'LIMIT = 8\n')
                initial = source_validation.validator_identity(grammar)
                mechanic.write_bytes(b'LIMIT = 32\n')
                self.assertNotEqual(initial, source_validation.validator_identity(grammar))


class SourceIndexCatalogReuseTests(unittest.TestCase):
    def test_fresh_catalog_rows_replace_the_second_source_scan(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-source-index-") as raw:
            root = Path(raw)
            record_ref = "ToS/source-witnesses/work.json"
            claim_ref = "ToS/source-witnesses/membership-claims.jsonl"
            _write(root, record_ref, _canonical({"record_id": "tos.work.example"}))
            _write(root, claim_ref, _canonical({"claim_id": "tos.claim.example", "evidence_refs": []}))
            paths = {record_ref, claim_ref}
            manifest_ref = Path("ToS/source-witnesses/catalog/catalog.manifest.json")
            records_ref = Path("ToS/source-witnesses/catalog/works.jsonl")
            claims_ref = Path("ToS/source-witnesses/catalog/claims.jsonl")
            catalog_outputs = {
                manifest_ref: _canonical({
                    "record_files": {"work": records_ref.as_posix()},
                    "claim_file": claims_ref.as_posix(),
                }).decode(),
                records_ref: _canonical({
                    "record_id": "tos.work.example",
                    "source_record_ref": record_ref,
                }).decode(),
                claims_ref: _canonical({
                    "claim_id": "tos.claim.example",
                    "source_claim_file_ref": claim_ref,
                }).decode(),
            }

            class Profiles:
                def __init__(self, selected_root):
                    self.root = selected_root

                def native_semantic_identities(self):
                    return {}

            with patch.object(source_record_profiles, "SourceRecordProfiles", Profiles), \
                    patch.object(build_source_witness_catalog, "collect_records",
                                 side_effect=AssertionError("catalog reuse must not rescan records")), \
                    patch.object(build_source_witness_catalog, "collect_claims",
                                 side_effect=AssertionError("catalog reuse must not rescan claims")):
                index = source_validation.source_index(
                    root,
                    paths,
                    catalog_outputs=catalog_outputs,
                )

            self.assertEqual(
                index.identities,
                {
                    "tos.claim.example": claim_ref,
                    "tos.work.example": record_ref,
                },
            )


@contextmanager
def _temporary_git_fixture() -> Iterator[tuple[Path, str, Path]]:
    temporary = tempfile.TemporaryDirectory(prefix="tos-source-validation-")
    root = Path(temporary.name)
    repo = root / "repo"
    repo.mkdir()
    _git(repo, "init", "--quiet")
    _git(repo, "config", "user.email", "fixture@example.invalid")
    _git(repo, "config", "user.name", "Synthetic Fixture")
    _write(repo, "ToS/contracts/tiny.schema.json", _canonical({"type": "object"}))
    _write(repo, "ToS/derived-exports/demo.json", b'{"generated":true}\n')
    _write(repo, "ToS/source-witnesses/payload/image.bin", b"\x00\x01synthetic image\n")
    _write(repo, "ToS/source-witnesses/notes.md", b"# retained source\n")
    _write(repo, "docs/decisions/tiny.md", b"# tiny decision\n")
    _git(repo, "add", ".")
    _git(repo, "commit", "--quiet", "-m", "source validation fixture")
    commit = _git(repo, "rev-parse", "HEAD")
    try:
        yield root, commit, repo
    finally:
        temporary.cleanup()


def _grammar_root(root: Path) -> Path:
    grammar = root / "grammar"
    _write(
        grammar,
        "ToS/contracts/tiny.schema.json",
        _canonical({"$schema": "https://json-schema.org/draft/2020-12/schema", "type": "object"}),
    )
    return grammar


class SourceValidatorEvidenceTests(unittest.TestCase):
    def test_real_capture_restore_supplies_only_retained_evidence(self) -> None:
        with _temporary_git_fixture() as (root, commit, repo):
            capture = root / "capture"
            capture_git(
                repo,
                commit,
                ["ToS"],
                capture,
            )
            history = root / "history"
            restore_capture(capture, history)
            grammar = _grammar_root(root)
            baseline = source_validation.SourceValidator(grammar)
            validator = source_validation.SourceValidator(
                grammar,
                historical_capture=capture,
                historical_root=history,
            )
            evidence_paths = {entry["path"] for entry in validator.evidence}
            self.assertEqual(
                evidence_paths,
                {
                    "ToS/derived-exports/demo.json",
                    "ToS/source-witnesses/payload/image.bin",
                },
            )
            self.assertTrue(source_validation.is_source_member("ToS/source-witnesses/notes.md"))
            self.assertFalse(source_validation.is_source_member("ToS/derived-exports/demo.json"))
            self.assertFalse(source_validation.is_source_member("ToS/source-witnesses/payload/image.bin"))
            self.assertNotEqual(validator.sha256, baseline.sha256)

            admitted = root / "admitted"
            source_md = _write(admitted, "ToS/source-witnesses/notes.md", b"# source remains admitted\n")
            admitted_paths = {
                "ToS/contracts/tiny.schema.json",
                source_md.relative_to(admitted).as_posix(),
            }
            validator._supply_evidence(admitted)
            self.assertEqual(source_md.read_bytes(), b"# source remains admitted\n")
            self.assertTrue(evidence_paths.isdisjoint(admitted_paths))
            for relative in sorted(evidence_paths):
                self.assertEqual(
                    (admitted / relative).read_bytes(),
                    (history / relative).read_bytes(),
                )

            with self.assertRaisesRegex(CorpusStoreError, "both"):
                source_validation.SourceValidator(grammar, historical_capture=capture)
            with self.assertRaisesRegex(CorpusStoreError, "both"):
                source_validation.SourceValidator(grammar, historical_root=history)

    def test_multiple_historical_packs_are_exact_and_order_independent(self) -> None:
        with _temporary_git_fixture() as (root, commit, repo):
            source_capture = root / "capture-source"
            capture_git(repo, commit, ["ToS"], source_capture)
            docs_capture = root / "capture-docs"
            capture_git(repo, commit, ["docs/decisions"], docs_capture)
            source_history = root / "history-source"
            restore_capture(source_capture, source_history)
            docs_history = root / "history-docs"
            restore_capture(docs_capture, docs_history)
            grammar = _grammar_root(root)

            validator = source_validation.SourceValidator(
                grammar,
                historical_capture=[source_capture, docs_capture],
                historical_root=[source_history, docs_history],
            )
            expected = {
                "ToS/derived-exports/demo.json": source_history,
                "ToS/source-witnesses/payload/image.bin": source_history,
                "docs/decisions/tiny.md": docs_history,
            }
            self.assertEqual({entry["path"] for entry in validator.evidence}, set(expected))
            admitted = root / "admitted-multiple"
            validator._supply_evidence(admitted)
            for relative, history in expected.items():
                self.assertEqual(
                    (admitted / relative).read_bytes(),
                    (history / relative).read_bytes(),
                )

            reverse = source_validation.SourceValidator(
                grammar,
                historical_capture=[docs_capture, source_capture],
                historical_root=[docs_history, source_history],
            )
            self.assertEqual(validator.sha256, reverse.sha256)

            with self.assertRaisesRegex(CorpusStoreError, "overlap"):
                source_validation.SourceValidator(
                    grammar,
                    historical_capture=[source_capture, source_capture],
                    historical_root=[source_history, source_history],
                )
            with self.assertRaisesRegex(CorpusStoreError, "every pack"):
                source_validation.SourceValidator(
                    grammar,
                    historical_capture=[source_capture, docs_capture],
                    historical_root=[source_history],
                )

    def test_corrupt_overlap_and_symlinked_historical_evidence_are_rejected(self) -> None:
        with _temporary_git_fixture() as (root, commit, repo):
            capture = root / "capture"
            capture_git(
                repo,
                commit,
                ["ToS"],
                capture,
            )
            grammar = _grammar_root(root)

            evidence_relative = "ToS/derived-exports/demo.json"
            for name, mutate in (
                (
                    "corrupt",
                    lambda history: (history / evidence_relative).write_bytes(b"corrupt\n"),
                ),
                (
                    "overlap",
                    lambda history: None,
                ),
                (
                    "symlink",
                    lambda history: _replace_with_symlink(history / evidence_relative, root / "outside.bin"),
                ),
            ):
                history = root / f"history-{name}"
                restore_capture(capture, history)
                mutate(history)
                destination = root / f"admitted-{name}"
                if name == "overlap":
                    _write(destination, evidence_relative, b"already source\n")
                validator = source_validation.SourceValidator(
                    grammar,
                    historical_capture=capture,
                    historical_root=history,
                )
                with self.subTest(name=name), self.assertRaisesRegex(
                    CorpusStoreError,
                    "historical validation evidence is missing, changed or overlaps source",
                ):
                    validator._supply_evidence(destination)


class PayloadSnapshotMembershipTests(unittest.TestCase):
    def test_explicit_payload_root_uses_snapshot_membership_without_git(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-payload-membership-") as raw:
            root = Path(raw)
            repo_root = root / "snapshot"
            item_directory = repo_root / "ToS/source-witnesses/works/tiny"
            payload_source_root = root / "payload-source"
            payload = b"exact synthetic payload\n"
            payload_path = payload_source_root / "works/tiny/payload/image.bin"
            payload_path.parent.mkdir(parents=True)
            payload_path.write_bytes(payload)
            stable_ref = "ToS/source-witnesses/works/tiny/payload/image.bin"
            payload_entry = {
                "relative_path": "payload/image.bin",
                "byte_size": len(payload),
                "sha256": hashlib.sha256(payload).hexdigest(),
            }

            with patch.object(
                foundation.subprocess,
                "run",
                side_effect=AssertionError("explicit payload snapshot must not invoke git"),
            ):
                with foundation.source_snapshot_membership(repo_root, frozenset()):
                    excluded_issues = foundation.validate_payload_file(
                        repo_root,
                        item_directory,
                        payload_entry,
                        require_local_payloads=True,
                        payload_source_root=payload_source_root,
                    )
                with foundation.source_snapshot_membership(repo_root, frozenset({stable_ref})):
                    included_issues = foundation.validate_payload_file(
                        repo_root,
                        item_directory,
                        payload_entry,
                        require_local_payloads=True,
                        payload_source_root=payload_source_root,
                    )

            self.assertEqual(excluded_issues, [])
            self.assertEqual(payload_path.read_bytes(), payload)
            self.assertIn(
                (stable_ref, "local payload is not ignored by Git"),
                included_issues,
            )


def _replace_with_symlink(path: Path, target: Path) -> None:
    target.write_bytes(b"outside\n")
    path.unlink()
    path.symlink_to(target)


if __name__ == "__main__":
    unittest.main()
