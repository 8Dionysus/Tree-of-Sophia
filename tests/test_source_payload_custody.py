from __future__ import annotations

from dataclasses import replace
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import source_payload_custody as native_custody
from tests.oracles.acquisition import source_payload_custody as custody
import validate_source_witness_foundation as foundation

NATIVE_OWNER_SELECTED = bool(
    os.environ.get("TOS_NATIVE_OWNER_COMMAND_BIN")
    or shutil.which("tos-native-owner-command")
)


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

    def test_digest_rejects_intermediate_symlink_replacement_after_open(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            entry, source, _ = self._fixture(Path(temporary))
            path = custody.payload_path(
                source, entry.item_root_ref, entry.relative_path
            )
            item_directory = source.joinpath(*Path(entry.item_root_ref).parts[2:])
            moved_directory = Path(temporary) / "moved-item"
            real_open = custody.os.open
            replaced = False

            def replace_ancestor_after_file_open(name, flags, *args, **kwargs):
                nonlocal replaced
                fd = real_open(name, flags, *args, **kwargs)
                if name == "witness.txt" and kwargs.get("dir_fd") is not None and not replaced:
                    replaced = True
                    item_directory.rename(moved_directory)
                    item_directory.symlink_to(moved_directory, target_is_directory=True)
                return fd

            with mock.patch.object(custody.os, "open", side_effect=replace_ancestor_after_file_open):
                with self.assertRaisesRegex(custody.CustodyError, "symlink or invalid ancestor"):
                    custody.digest_file(path, custody_root=source)
            self.assertTrue(replaced)

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

    @unittest.skipUnless(NATIVE_OWNER_SELECTED, "native owner product not selected")
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


@unittest.skipUnless(NATIVE_OWNER_SELECTED, "native owner product not selected")
class NativeSourcePayloadCustodyTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.metadata = self.root / "metadata"
        self.source = self.root / "external-source"
        self.destination = self.root / "external-destination"
        self.metadata.mkdir()
        self.source.mkdir()
        self.destination.mkdir()
        self.item_ref = (
            "ToS/source-witnesses/works/fixture/expressions/en/editions/pinned/"
            "items/text"
        )
        self.manifest_ref = f"{self.item_ref}/item.manifest.json"
        self.body = b"native custody fixture\n"
        self.digest = hashlib.sha256(self.body).hexdigest()
        self.git_blob = hashlib.sha1(
            b"blob " + str(len(self.body)).encode() + b"\0" + self.body
        ).hexdigest()
        self.payload = native_custody.payload_path(
            self.source, self.item_ref, "payload/witness.txt"
        )
        self.payload.parent.mkdir(parents=True)
        self.payload.write_bytes(self.body)
        item_directory = self.metadata / self.item_ref
        item_directory.mkdir(parents=True)
        (item_directory / "item.manifest.json").write_text(
            json.dumps(
                {
                    "item_id": "tos.item.fixture",
                    "payload_files": [
                        {
                            "file_id": f"tos.file.sha256.{self.digest}",
                            "relative_path": "payload/witness.txt",
                            "byte_size": len(self.body),
                            "sha256": self.digest,
                        }
                    ],
                }
            )
            + "\n"
        )

    def _entry(self) -> native_custody.PayloadEntry:
        entries = native_custody.entries_from_item_manifest(
            self.metadata, self.manifest_ref, self.source
        )
        self.assertEqual(1, len(entries))
        return entries[0]

    def test_import_api_plan_copy_digest_and_no_clobber(self) -> None:
        entry = self._entry()
        direct_digest = native_custody.digest_file(
            self.payload, custody_root=self.source
        )
        self.assertEqual(self.digest, direct_digest.sha256)
        digest = native_custody.verify_entry(entry)
        self.assertEqual(len(self.body), digest.byte_size)
        self.assertEqual(self.digest, digest.sha256)
        self.assertEqual(self.git_blob, digest.git_blob_sha1)

        malformed = replace(entry, relative_path="payload/not-present.txt", sha256=17)
        with self.assertRaisesRegex(
            native_custody.CustodyError, "sha256 must be text or null"
        ):
            native_custody.verify_entry(malformed)

        self.assertEqual(
            "planned_copy",
            native_custody.plan_entries(
                [entry], destination_payload_root=self.destination
            )[0][0]["status"],
        )

        rows, duplicates = native_custody.copy_entries([entry], self.destination)
        self.assertEqual([], duplicates)
        self.assertEqual("copied", rows[0]["status"])
        copied = native_custody.destination_path(self.destination, entry)
        self.assertEqual(self.body, copied.read_bytes())
        self.assertEqual(0o444, copied.stat().st_mode & 0o7777)

        rows, _ = native_custody.copy_entries([entry], self.destination)
        self.assertEqual("already_present", rows[0]["status"])
        copied.chmod(0o644)
        rows, _ = native_custody.copy_entries([entry], self.destination)
        self.assertEqual("conflict", rows[0]["status"])
        self.assertEqual(self.body, copied.read_bytes())

    def test_import_api_registry_and_inventory_selection(self) -> None:
        registry = self.root / "registry.json"
        registry.write_text(
            json.dumps(
                {
                    "targets": [
                        {
                            "ids": {"item": "tos.item.fixture"},
                            "paths": {"item_root": self.item_ref},
                            "files": [
                                {
                                    "basename": "witness.txt",
                                    "byte_size": len(self.body),
                                    "git_blob_sha1": self.git_blob,
                                },
                                {
                                    "basename": "not-present.txt",
                                    "byte_size": 0,
                                    "git_blob_sha1": hashlib.sha1(b"blob 0\0").hexdigest(),
                                },
                            ],
                        }
                    ]
                }
            )
        )
        entries, missing = native_custody.entries_from_registry_manifest(
            registry, self.source, metadata_root=self.metadata
        )
        self.assertEqual(1, len(entries))
        self.assertEqual(1, len(missing))
        self.assertEqual(self.digest, entries[0].sha256)
        rows, _ = native_custody.plan_entries(entries)
        self.assertEqual("source_verified", rows[0]["status"])

        inventory = self.root / "inventory.json"
        inventory.write_text(
            json.dumps(
                {
                    "files": [
                        {
                            "source_root": str(self.source),
                            "manifest_ref": self.manifest_ref,
                            "item_ref": "tos.item.fixture",
                            "file_ref": f"tos.file.sha256.{self.digest}",
                            "relative_ref": f"{self.item_ref}/payload/witness.txt",
                            "byte_size": len(self.body),
                            "sha256": self.digest,
                            "source_present": True,
                        },
                        {"source_present": False},
                    ]
                }
            )
        )
        selected = native_custody.entries_from_inventory(inventory, only_present=True)
        self.assertEqual(1, len(selected))
        self.assertEqual(self.item_ref, selected[0].item_root_ref)
        self.assertEqual(self.digest, native_custody.verify_entry(selected[0]).sha256)

    def test_native_cli_verify_and_copy_emit_metadata_only_receipts(self) -> None:
        def run(command: str, receipt: Path, destination: Path | None = None):
            args = [
                sys.executable,
                str(SCRIPTS / "source_payload_custody.py"),
                command,
                "--metadata-root",
                str(self.metadata),
                "--item-manifest",
                self.manifest_ref,
                "--payload-source-root",
                str(self.source),
                "--receipt",
                str(receipt),
            ]
            if destination is not None:
                args.extend(["--destination-payload-root", str(destination)])
            return subprocess.run(args, capture_output=True, text=True, check=False)

        verify_receipt = self.root / "verify-receipt.json"
        verified = run("verify", verify_receipt)
        self.assertEqual(0, verified.returncode, verified.stderr)
        self.assertEqual("completed", json.loads(verified.stdout)["status"])
        receipt = json.loads(verify_receipt.read_text())
        self.assertEqual("source_verified", receipt["rows"][0]["status"])
        self.assertEqual(0o600, verify_receipt.stat().st_mode & 0o7777)

        copy_root = self.root / "cli-destination"
        copy_root.mkdir()
        copy_receipt = self.root / "copy-receipt.json"
        copied = run("copy", copy_receipt, copy_root)
        self.assertEqual(0, copied.returncode, copied.stderr)
        self.assertEqual("copied", json.loads(copy_receipt.read_text())["rows"][0]["status"])
        self.assertEqual(
            0o444,
            native_custody.payload_path(copy_root, self.item_ref, "payload/witness.txt")
            .stat()
            .st_mode
            & 0o7777,
        )
        self.assertNotIn(str(self.source), copy_receipt.read_text())
        self.assertNotIn(str(self.metadata), copy_receipt.read_text())

    def test_duplex_byte_publication_and_real_descriptor_negatives(self) -> None:
        caller_body = b"caller supplied custody bytes\n"
        caller_digest = native_custody.FileDigest(
            byte_size=len(caller_body),
            sha256=hashlib.sha256(caller_body).hexdigest(),
            git_blob_sha1=hashlib.sha1(
                b"blob " + str(len(caller_body)).encode() + b"\0" + caller_body
            ).hexdigest(),
        )
        target = self.destination / "caller-published.bin"
        self.assertEqual(
            "copied",
            native_custody.publish_bytes_no_clobber(
                target, caller_body, caller_digest, custody_root=self.destination
            ),
        )
        self.assertEqual(0o444, target.stat().st_mode & 0o7777)
        self.assertEqual(
            "already_present",
            native_custody.publish_bytes_no_clobber(
                target, caller_body, caller_digest, custody_root=self.destination
            ),
        )

        outside = self.root / "outside.bin"
        outside.write_bytes(self.body)
        self.payload.unlink()
        self.payload.symlink_to(outside)
        with self.assertRaises(native_custody.CustodyError):
            native_custody.digest_file(self.payload, custody_root=self.source)

        self.payload.unlink()
        self.payload.write_bytes(self.body)
        self.payload.chmod(0o444)
        alias = self.root / "hardlink.bin"
        os.link(self.payload, alias)
        with self.assertRaises(native_custody.CustodyError):
            native_custody.digest_file(
                self.payload,
                expected_mode=0o444,
                expected_owner_uid=os.geteuid(),
                require_single_link=True,
                custody_root=self.source,
            )

    def test_duplex_callback_accepts_bytes_larger_than_the_initial_wire_request(self) -> None:
        # Base64 would exceed the acquisition adapter's 16 MiB initial JSON
        # request budget if the payload were embedded inline.
        caller_body = b"x" * (13 * 1024 * 1024)
        git_blob = hashlib.sha1(
            b"blob " + str(len(caller_body)).encode() + b"\0" + caller_body
        ).hexdigest()
        expected = native_custody.FileDigest(
            byte_size=len(caller_body),
            sha256=hashlib.sha256(caller_body).hexdigest(),
            git_blob_sha1=git_blob,
        )
        target = self.destination / "large-caller-published.bin"
        self.assertEqual(
            "copied",
            native_custody.publish_bytes_no_clobber(
                target, caller_body, expected, custody_root=self.destination
            ),
        )
        self.assertEqual(len(caller_body), target.stat().st_size)
        self.assertEqual(0o444, target.stat().st_mode & 0o7777)


if __name__ == "__main__":
    unittest.main()
