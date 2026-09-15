from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import source_payload_custody as custody
import validate_source_witness_foundation as foundation


class SourcePayloadCustodyTests(unittest.TestCase):
    def _fixture(self, root: Path, body: bytes = b"immutable witness\n") -> tuple[custody.PayloadEntry, Path, Path]:
        source = root / "source"
        destination = root / "destination"
        source.mkdir(parents=True, exist_ok=True)
        destination.mkdir(parents=True, exist_ok=True)
        item = "ToS/source-witnesses/works/fixture/expressions/en/editions/pinned/items/text"
        relative = "payload/witness.txt"
        source_path = custody.payload_path(source, item, relative)
        source_path.parent.mkdir(parents=True, exist_ok=True)
        source_path.write_bytes(body)
        digest = hashlib.sha256(body).hexdigest()
        entry = custody.PayloadEntry(
            item_id="tos.item.fixture",
            file_id=f"tos.file.sha256.{digest}",
            item_root_ref=item,
            relative_path=relative,
            byte_size=len(body),
            sha256=digest,
            source_root=source,
            manifest_ref=f"{item}/item.manifest.json",
            source_label="fixture",
        )
        return entry, source, destination

    def test_external_root_copy_readback_and_receipt(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            entry, _, destination = self._fixture(Path(temporary))
            rows, duplicates = custody.copy_entries([entry], destination)
            self.assertEqual([], duplicates)
            self.assertEqual("copied", rows[0]["status"])
            copied = custody.destination_path(destination, entry)
            self.assertEqual(b"immutable witness\n", copied.read_bytes())
            self.assertEqual(0o444, copied.stat().st_mode & 0o777)
            receipt = custody.write_receipt(
                Path(temporary) / "receipt.json",
                operation="copy",
                rows=rows,
                duplicates=duplicates,
            )
            self.assertEqual("tos.source_payload_custody_receipt.v1", json.loads(receipt.read_text())["schema_version"])

    def test_existing_different_bytes_are_conflict_and_never_overwritten(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            entry, _, destination = self._fixture(Path(temporary))
            target = custody.destination_path(destination, entry)
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(b"different bytes")
            rows, _ = custody.copy_entries([entry], destination)
            self.assertEqual("conflict", rows[0]["status"])
            self.assertEqual(b"different bytes", target.read_bytes())

    def test_path_and_symlink_escape_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "root"
            root.mkdir()
            with self.assertRaises(custody.CustodyError):
                custody.payload_path(root, "ToS/source-witnesses/works/../escape", "payload/x")
            outside = Path(temporary) / "outside"
            outside.mkdir()
            (root / "works").symlink_to(outside, target_is_directory=True)
            with self.assertRaises(custody.CustodyError):
                custody.payload_path(root, "ToS/source-witnesses/works/fixture", "payload/x")

    def test_registry_manifest_reports_absent_payload_without_inventing_bytes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            source.mkdir()
            item = "ToS/source-witnesses/works/fixture/expressions/en/editions/pinned/items/text"
            path = custody.payload_path(source, item, "payload/present.txt")
            path.parent.mkdir(parents=True)
            path.write_bytes(b"present")
            manifest = root / "registry.json"
            manifest.write_text(json.dumps({
                "targets": [{
                    "ids": {"item": "tos.item.fixture"},
                    "paths": {"item_root": item},
                    "files": [
                        {"basename": "present.txt", "byte_size": 7, "git_blob_sha1": hashlib.sha1(b"blob 7\0present").hexdigest()},
                        {"basename": "absent.txt", "byte_size": 6, "git_blob_sha1": "0" * 40},
                    ],
                }],
            }))
            entries, missing = custody.entries_from_registry_manifest(manifest, source)
            self.assertEqual(1, len(entries))
            self.assertEqual("absent.txt", missing[0]["relative_path"].split("/", 1)[1])
            rows, _ = custody.plan_entries(entries)
            self.assertEqual("source_verified", rows[0]["status"])

    def test_validator_reads_payload_from_explicit_external_root(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "metadata"
            external = Path(temporary) / "payload"
            external.mkdir(parents=True)
            item = root / "ToS/source-witnesses/works/fixture/expressions/en/editions/pinned/items/text"
            item.mkdir(parents=True)
            body = b"external payload"
            digest = hashlib.sha256(body).hexdigest()
            manifest = {
                "item_id": "tos.item.fixture",
                "payload_files": [{
                    "file_id": f"tos.file.sha256.{digest}",
                    "relative_path": "payload/text.txt",
                    "byte_size": len(body),
                    "sha256": digest,
                }],
            }
            manifest_path = item / "item.manifest.json"
            manifest_path.write_text(json.dumps(manifest))
            payload = custody.payload_path(
                external,
                "ToS/source-witnesses/works/fixture/expressions/en/editions/pinned/items/text",
                "payload/text.txt",
            )
            payload.parent.mkdir(parents=True)
            payload.write_bytes(body)
            issues = foundation.validate_payload_file(
                root,
                item,
                manifest["payload_files"][0],
                require_local_payloads=True,
                payload_source_root=external,
            )
            self.assertEqual([], issues)

    def test_item_manifest_rejects_empty_or_malformed_payload_files_and_item_id(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "metadata"
            external = Path(temporary) / "payload"
            external.mkdir(parents=True)
            item = root / "ToS/source-witnesses/works/fixture/expressions/en/editions/pinned/items/text"
            item.mkdir(parents=True)
            manifest_path = item / "item.manifest.json"
            valid_payload = {
                "file_id": "tos.file.sha256." + "a" * 64,
                "relative_path": "payload/text.txt",
                "byte_size": 1,
                "sha256": "a" * 64,
            }
            for item_id, payload_files in (
                ("tos.item.fixture", []),
                ("tos.item.fixture", [None]),
                ("tos.item.fixture", [valid_payload, "malformed"]),
                ("tos.work.fixture", [valid_payload]),
                ("tos.item.", [valid_payload]),
            ):
                manifest_path.write_text(json.dumps({"item_id": item_id, "payload_files": payload_files}))
                with self.subTest(item_id=item_id, payload_files=payload_files), self.assertRaises(custody.CustodyError):
                    custody.entries_from_item_manifest(
                        root,
                        "ToS/source-witnesses/works/fixture/expressions/en/editions/pinned/items/text/item.manifest.json",
                        external,
                    )

    def test_receipt_path_is_immutable(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "receipt.json"
            custody.write_receipt(path, operation="verify", rows=[], duplicates=[])
            with self.assertRaises(custody.CustodyError):
                custody.write_receipt(path, operation="verify-again", rows=[], duplicates=[])


if __name__ == "__main__":
    unittest.main()
