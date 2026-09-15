from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from build_source_registry_coverage import (  # noqa: E402
    PACKET,
    build as build_coverage,
    compressed as coverage_compressed,
    markdown,
)
from build_source_registry_reconciliation import build as build_reconciliation  # noqa: E402
from source_registry_common import encoded  # noqa: E402


SNAPSHOT_ID = "fixture-snapshot"
OWNER_RECORD_REF = "ToS/source-witnesses/owner/fixture.json"
ITEM_MANIFEST_REF = "ToS/source-witnesses/owner/item.manifest.json"
PAYLOAD_REF = "ToS/source-witnesses/owner/payload/original.xml"
OWNER_RECORD_ID = "tos.owner.fixture"
REGISTRY_RECORD_ID = "registry:fixture"
PREPARATION_MANIFEST_REF = (
    "ToS/source-witnesses/discovery/fixture/manifest.json"
)


def _write_json(root: Path, relative: str, value: object) -> None:
    path = root / relative
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(encoded(value))


def _canonical_object(value: object) -> bytes:
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()


def _fixture(root: Path) -> None:
    packet = root / PACKET
    _write_json(packet, "current.json", {"snapshot_id": SNAPSHOT_ID})
    _write_json(
        packet / "snapshots" / SNAPSHOT_ID,
        "snapshot.json",
        {"documents": ["documents/fixture.json.gz"]},
    )
    document = {"corpus_id": "fixture", "document_id": "D1", "records": []}
    snapshot_document = packet / "snapshots" / SNAPSHOT_ID / "documents" / "fixture.json.gz"
    snapshot_document.parent.mkdir(parents=True, exist_ok=True)
    import gzip

    snapshot_document.write_bytes(gzip.compress(encoded(document), mtime=0))

    source_record = {
        "record_id": OWNER_RECORD_ID,
        "record_type": "work",
        "preferred_label": "Fixture owner record",
        "identity_status": "reported_unreviewed",
        "item_manifest_ref": ITEM_MANIFEST_REF,
        "variant_labels": [],
        "external_identifiers": [],
    }
    _write_json(root, OWNER_RECORD_REF, source_record)
    payload = b"fixture payload\n"
    _write_json(
        root,
        ITEM_MANIFEST_REF,
        {
            "item_id": "tos.item.fixture",
            "embodiment_ref": OWNER_RECORD_ID,
            "payload_files": [
                {
                    "relative_path": "payload/original.xml",
                    "sha256": hashlib.sha256(payload).hexdigest(),
                    "byte_size": len(payload),
                }
            ],
        },
    )
    catalog = root / "ToS/source-witnesses/catalog/fixture.jsonl"
    catalog.parent.mkdir(parents=True, exist_ok=True)
    catalog.write_bytes(
        (
            json.dumps(
                {
                    "source_record_ref": OWNER_RECORD_REF,
                    "record_id": OWNER_RECORD_ID,
                    "record_type": "work",
                    "preferred_label": "Fixture owner record",
                    "identity_status": "reported_unreviewed",
                    "record_sha256": hashlib.sha256(_canonical_object(source_record)).hexdigest(),
                }
            )
            + "\n"
        ).encode()
    )

    reconciliation_input = {
        "snapshot_id": SNAPSHOT_ID,
        "owner_sources": [{"record_id": OWNER_RECORD_ID, "record_type": "work"}],
        "records": [
            {
                "record_id": REGISTRY_RECORD_ID,
                "source_record_id": "reported-1",
                "corpus_id": "fixture",
                "document_id": "D1",
                "kind": "registry",
                "owner_matches": [],
            }
        ],
    }
    reconciliation_path = packet / "reconciliation.current.json.gz"
    reconciliation_path.parent.mkdir(parents=True, exist_ok=True)
    reconciliation_path.write_bytes(coverage_compressed(reconciliation_input))


def _add_prepared_not_installed_target(root: Path) -> None:
    ids = {
        kind: f"tos.prepared.{kind}.fixture"
        for kind in ("work", "expression", "edition", "item")
    }
    paths = {
        kind: f"ToS/source-witnesses/prepared/{kind}.json"
        for kind in ("work", "expression", "edition", "item")
    }
    paths["item_root"] = "ToS/source-witnesses/prepared/item"
    target = {
        "title": "Prepared fixture version",
        "ids": ids,
        "paths": paths,
        "files": [{"basename": "original.xml", "byte_size": 4}],
        "registry_sources": [{"entry_id": REGISTRY_RECORD_ID}],
    }
    _write_json(
        root,
        PREPARATION_MANIFEST_REF,
        {
            "schema_version": "tos_registry_first_planting_preparation_v1",
            "targets": [target],
        },
    )
    manifest_path = root / PREPARATION_MANIFEST_REF
    _write_json(
        root,
        str(Path(PREPARATION_MANIFEST_REF).parent / "preparation-checkpoint-receipt.json"),
        {
            "status": "passed",
            "checkpoint_review_ref": "review:prepared-fixture",
            "manifest_sha256": hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
        },
    )


def _tree_snapshot(root: Path) -> dict[str, bytes]:
    return {
        path.relative_to(root).as_posix(): path.read_bytes()
        for path in sorted(root.rglob("*"))
        if path.is_file() and not path.is_symlink()
    }


class SourceRegistryReaderRootTests(unittest.TestCase):
    def test_builders_are_explicit_and_portable_owner_file_projection_ignores_payload_presence(self) -> None:
        with tempfile.TemporaryDirectory(prefix="source-registry-reader-") as directory:
            root = Path(directory) / "source"
            _fixture(root)

            before = build_reconciliation(root)
            owner = before["owner_sources"][0]
            self.assertEqual(
                owner["item_files"],
                [
                    {
                        "path": PAYLOAD_REF,
                        "expected_sha256": hashlib.sha256(b"fixture payload\n").hexdigest(),
                    }
                ],
            )
            self.assertNotIn("exists", owner["item_files"][0])
            self.assertNotIn("fixity_matches", owner["item_files"][0])

            payload = root / PAYLOAD_REF
            payload.parent.mkdir(parents=True, exist_ok=True)
            payload.write_bytes(b"fixture payload\n")
            with_payload = build_reconciliation(root)
            payload.unlink()
            without_payload_again = build_reconciliation(root)
            outside = root.parent / "payload-outside.xml"
            outside.write_bytes(b"different local bytes")
            payload.symlink_to(outside)
            with_symlink_payload = build_reconciliation(root)
            payload.unlink()
            self.assertEqual(encoded(before), encoded(with_payload))
            self.assertEqual(encoded(before), encoded(without_payload_again))
            self.assertEqual(encoded(before), encoded(with_symlink_payload))

            _add_prepared_not_installed_target(root)
            coverage = build_coverage(root)
            self.assertEqual(coverage["snapshot_id"], SNAPSHOT_ID)
            self.assertEqual(
                coverage["targets"][0]["status"], "prepared_version_not_installed"
            )
            source_markdown = markdown(coverage, root)
            moved_markdown = markdown(coverage, root.parent / "separate-output")
            self.assertEqual(source_markdown, moved_markdown)
            logical_link = os.path.relpath(PREPARATION_MANIFEST_REF, PACKET)
            self.assertIn(f"]({logical_link})", moved_markdown)
            self.assertIn("не установлено", moved_markdown)

    def test_cli_reads_selected_root_from_outside_repo_and_writes_only_explicit_output_root(self) -> None:
        with tempfile.TemporaryDirectory(prefix="source-registry-reader-cli-") as directory:
            temp = Path(directory)
            outside = temp / "outside cwd"
            outside.mkdir()
            source_root = outside / "selected source root"
            output_root = outside / "explicit output root"
            _fixture(source_root)
            source_before = _tree_snapshot(source_root)

            coverage_script = ROOT / "scripts/build_source_registry_coverage.py"
            coverage = subprocess.run(
                [
                    sys.executable,
                    str(coverage_script),
                    "--source-root",
                    str(source_root),
                    "--output-root",
                    str(output_root),
                ],
                cwd=outside,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(coverage.returncode, 0, coverage.stderr)
            self.assertTrue((output_root / PACKET / "coverage.current.json.gz").is_file())
            self.assertTrue((output_root / PACKET / "COVERAGE.md").is_file())

            reconciliation_script = ROOT / "scripts/build_source_registry_reconciliation.py"
            reconciliation = subprocess.run(
                [
                    sys.executable,
                    str(reconciliation_script),
                    "--source-root",
                    str(source_root),
                    "--output-root",
                    str(output_root),
                ],
                cwd=outside,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(reconciliation.returncode, 0, reconciliation.stderr)
            self.assertTrue(
                (output_root / PACKET / "reconciliation.current.json.gz").is_file()
            )
            self.assertEqual(source_before, _tree_snapshot(source_root))
            self.assertFalse((source_root / PACKET / "COVERAGE.md").exists())

            check = subprocess.run(
                [
                    sys.executable,
                    str(coverage_script),
                    "--source-root",
                    str(source_root),
                    "--output-root",
                    str(output_root),
                    "--check",
                ],
                cwd=outside,
                check=False,
                capture_output=True,
                text=True,
            )
            self.assertEqual(check.returncode, 0, check.stderr)

            relative_output_root = outside / "relative output root"
            for script, output_name in (
                (coverage_script, "COVERAGE.md"),
                (reconciliation_script, "reconciliation.current.json.gz"),
            ):
                with self.subTest(relative_script=script.name):
                    relative = subprocess.run(
                        [
                            sys.executable,
                            str(script),
                            "--source-root",
                            source_root.name,
                            "--output-root",
                            relative_output_root.name,
                        ],
                        cwd=outside,
                        check=False,
                        capture_output=True,
                        text=True,
                    )
                    self.assertEqual(relative.returncode, 0, relative.stderr)
                    self.assertTrue(
                        (relative_output_root / PACKET / output_name).is_file()
                    )
            self.assertEqual(source_before, _tree_snapshot(source_root))

            for selector in (("--remaining",), ("--document", "D1"), ("--verify-local",)):
                with self.subTest(selector=selector):
                    stdout_only = subprocess.run(
                        [
                            sys.executable,
                            str(coverage_script),
                            "--source-root",
                            str(source_root),
                            *selector,
                        ],
                        cwd=outside,
                        check=False,
                        capture_output=True,
                        text=True,
                    )
                    self.assertEqual(stdout_only.returncode, 0, stdout_only.stderr)
                    if selector != ("--verify-local",):
                        self.assertIn(REGISTRY_RECORD_ID, stdout_only.stdout)

    def test_cli_requires_explicit_source_and_output_for_writes(self) -> None:
        with tempfile.TemporaryDirectory(prefix="source-registry-reader-required-") as directory:
            temp = Path(directory)
            source_root = temp / "source"
            _fixture(source_root)
            outside = temp / "outside"
            outside.mkdir()
            commands = (
                [sys.executable, str(ROOT / "scripts/build_source_registry_coverage.py")],
                [
                    sys.executable,
                    str(ROOT / "scripts/build_source_registry_coverage.py"),
                    "--source-root",
                    str(source_root),
                ],
                [
                    sys.executable,
                    str(ROOT / "scripts/build_source_registry_reconciliation.py"),
                    "--source-root",
                    str(source_root),
                ],
                [
                    sys.executable,
                    str(ROOT / "scripts/build_source_registry_coverage.py"),
                    "--source-root",
                    str(temp / "absent source"),
                    "--output-root",
                    str(temp / "output"),
                ],
                [
                    sys.executable,
                    str(ROOT / "scripts/build_source_registry_coverage.py"),
                    "--source-root",
                    str(source_root),
                    "--output-root",
                    str(source_root),
                ],
            )
            for command in commands:
                with self.subTest(command=command):
                    result = subprocess.run(
                        command,
                        cwd=outside,
                        check=False,
                        capture_output=True,
                        text=True,
                    )
                    self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
