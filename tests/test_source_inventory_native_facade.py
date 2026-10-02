from __future__ import annotations

import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import build_source_resource_inventories as inventory  # noqa: E402


def _minimal_pdf(width: int = 300, height: int = 400) -> bytes:
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {width} {height}] /Resources << >> /Contents 4 0 R >>".encode(),
        b"<< /Length 0 >>\nstream\n\nendstream",
    ]
    body = bytearray(b"%PDF-1.4\n")
    offsets = []
    for index, value in enumerate(objects, 1):
        offsets.append(len(body))
        body.extend(f"{index} 0 obj\n".encode())
        body.extend(value)
        body.extend(b"\nendobj\n")
    xref = len(body)
    body.extend(f"xref\n0 {len(objects) + 1}\n0000000000 65535 f \n".encode())
    for offset in offsets:
        body.extend(f"{offset:010d} 00000 n \n".encode())
    body.extend(
        f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    )
    return bytes(body)


class NativeInventoryFacadeTests(unittest.TestCase):
    def test_imported_file_api_is_wire_only(self):
        entry = {
            "file_id": "tos.file.fixture",
            "relative_path": "payload/source.txt",
            "media_type": "text/plain",
            "byte_size": 4,
            "sha256": "a" * 64,
        }
        result = {"profile": "plain_utf8_file_v1"}
        with patch.object(inventory, "_native_invoke", return_value=result) as invoke:
            self.assertIs(
                inventory.build_file_inventory(Path("/tmp/source.txt"), entry), result
            )
        invoke.assert_called_once_with(
            {
                "family": "inventory",
                "operation": "file",
                "payload_path": "/tmp/source.txt",
                "payload_entry": entry,
                "plain_text_profile": "plain_utf8_file_v1",
            }
        )

    def test_imported_item_builder_preserves_missing_payload_none(self):
        with tempfile.TemporaryDirectory() as temporary:
            repo = Path(temporary)
            manifest = repo / "ToS/source-witnesses/fixture/item.manifest.json"
            with patch.object(inventory, "_native_invoke", return_value=None) as invoke:
                self.assertIsNone(
                    inventory.build_inventory(
                        repo_root=repo,
                        manifest_path=manifest,
                        payload_source_root=repo / "payloads",
                        event_date="2026-10-01",
                    )
                )
            request = invoke.call_args.args[0]
            self.assertEqual(request["family"], "inventory")
            self.assertEqual(request["operation"], "item")
            self.assertEqual(
                request["item_manifest_ref"],
                "ToS/source-witnesses/fixture/item.manifest.json",
            )
            self.assertEqual(request["event_date"], "2026-10-01")

    def test_inventory_metadata_is_returned_by_native_owner(self):
        metadata = {
            "schema_ref": "native-schema-ref",
            "generator_version": "2",
            "authority_boundaries": {"1": "legacy", "2": "current"},
            "max_plain_utf8_bytes": 131072,
        }
        with patch.object(inventory, "_native_invoke", return_value=metadata) as invoke:
            self.assertEqual(inventory.inventory_metadata(), metadata)
            self.assertEqual(inventory.inventory_authority_boundary("1"), "legacy")
        self.assertEqual(invoke.call_count, 2)
        self.assertEqual(
            invoke.call_args.args[0],
            {"family": "inventory", "operation": "metadata"},
        )


NATIVE_OWNER_SELECTED = bool(
    os.environ.get("TOS_NATIVE_OWNER_COMMAND_BIN")
    or shutil.which("tos-native-owner-command")
)


@unittest.skipUnless(NATIVE_OWNER_SELECTED, "native owner command product not selected")
class NativeInventoryProductTests(unittest.TestCase):
    def test_selected_native_file_item_cli_build_and_check_consumer(self):
        with tempfile.TemporaryDirectory(prefix="tos-inventory-native-") as temporary:
            base = Path(temporary)
            repo = base / "repository"
            payload_root = base / "payload-root"
            item_ref = (
                "ToS/source-witnesses/works/fixture/editions/test/items/native-facade/"
                "item.manifest.json"
            )
            inventory_ref = item_ref.removesuffix("item.manifest.json") + "resource-inventory.json"
            relative_path = "payload/source.txt"
            item_root = repo / item_ref
            source_path = item_root.parent / relative_path
            external_path = payload_root / item_root.parent.relative_to(
                repo / "ToS/source-witnesses"
            ) / relative_path
            schema_path = repo / "ToS/contracts/source-resource-inventory.schema.json"
            source_path.parent.mkdir(parents=True)
            external_path.parent.mkdir(parents=True)
            schema_path.parent.mkdir(parents=True)
            schema_path.write_bytes(
                (ROOT / "ToS/contracts/source-resource-inventory.schema.json").read_bytes()
            )
            body = b"native inventory fixture\n"
            digest = hashlib.sha256(body).hexdigest()
            source_path.write_bytes(body)
            external_path.write_bytes(body)
            file_entry = {
                "file_id": f"tos.file.sha256.{digest}",
                "relative_path": relative_path,
                "original_basename": "source.txt",
                "media_type": "text/plain",
                "byte_size": len(body),
                "sha256": digest,
            }
            item_root.write_text(
                json.dumps(
                    {
                        "item_id": "tos.item.fixture.native-facade",
                        "payload_files": [file_entry],
                        "resource_inventory_ref": inventory_ref,
                    }
                ),
                encoding="utf-8",
            )

            direct_file = inventory.build_file_inventory(external_path, file_entry)
            self.assertEqual(direct_file["profile"], "plain_utf8_file_v1")
            self.assertEqual(direct_file["file_sha256"], digest)
            metadata = inventory.inventory_metadata()
            self.assertEqual(metadata["generator_version"], "2")

            wrapper = inventory.build_inventory(
                repo_root=repo,
                manifest_path=item_root,
                payload_source_root=payload_root,
                event_date="2026-10-01",
            )
            self.assertEqual(wrapper["files"][0]["profile"], "plain_utf8_file_v1")
            self.assertFalse((item_root.parent / "resource-inventory.json").exists())

            output, errors = io.StringIO(), io.StringIO()
            argv = [
                "build_source_resource_inventories.py",
                "--repo-root",
                str(repo),
                "--payload-source-root",
                str(payload_root),
                "--event-date",
                "2026-10-01",
            ]
            with contextlib.redirect_stdout(output), contextlib.redirect_stderr(errors):
                with patch.object(inventory.sys, "argv", argv):
                    self.assertEqual(inventory.main(), 0, errors.getvalue())
            self.assertIn("[ok] wrote 1 source resource inventories", output.getvalue())
            published = item_root.parent / "resource-inventory.json"
            actual = json.loads(published.read_bytes())
            self.assertEqual(actual["generated_from_manifest_ref"], item_ref)
            self.assertFalse(published.read_bytes().find(body) >= 0)

            output, errors = io.StringIO(), io.StringIO()
            with contextlib.redirect_stdout(output), contextlib.redirect_stderr(errors):
                with patch.object(inventory.sys, "argv", [*argv, "--check"]):
                    self.assertEqual(inventory.main(), 0, errors.getvalue())
            self.assertIn("[ok] verified 1 source resource inventories", output.getvalue())

    @unittest.skipUnless(
        Path("/usr/bin/pdfinfo").is_file() and Path("/usr/bin/pdfimages").is_file(),
        "Poppler PDF inventory backend not available",
    )
    def test_selected_native_pdf_consumer_uses_bounded_poppler_backend(self):
        with tempfile.TemporaryDirectory(prefix="tos-inventory-pdf-native-") as temporary:
            base = Path(temporary)
            repo = base / "repository"
            payload_root = base / "payload-root"
            item_ref = (
                "ToS/source-witnesses/works/fixture/editions/test/items/native-pdf/"
                "item.manifest.json"
            )
            inventory_ref = item_ref.removesuffix("item.manifest.json") + "resource-inventory.json"
            relative_path = "payload/source.pdf"
            item_root = repo / item_ref
            source_path = item_root.parent / relative_path
            external_path = payload_root / item_root.parent.relative_to(
                repo / "ToS/source-witnesses"
            ) / relative_path
            schema_path = repo / "ToS/contracts/source-resource-inventory.schema.json"
            source_path.parent.mkdir(parents=True)
            external_path.parent.mkdir(parents=True)
            schema_path.parent.mkdir(parents=True)
            schema_path.write_bytes(
                (ROOT / "ToS/contracts/source-resource-inventory.schema.json").read_bytes()
            )
            body = _minimal_pdf()
            digest = hashlib.sha256(body).hexdigest()
            source_path.write_bytes(body)
            external_path.write_bytes(body)
            file_entry = {
                "file_id": f"tos.file.sha256.{digest}",
                "relative_path": relative_path,
                "original_basename": "source.pdf",
                "media_type": "application/pdf",
                "byte_size": len(body),
                "sha256": digest,
            }
            item_root.write_text(
                json.dumps(
                    {
                        "item_id": "tos.item.fixture.native-pdf",
                        "payload_files": [file_entry],
                        "resource_inventory_ref": inventory_ref,
                    }
                ),
                encoding="utf-8",
            )

            direct_file = inventory.build_file_inventory(external_path, file_entry)
            self.assertEqual(direct_file["profile"], "pdf_pages_v1")
            self.assertEqual(direct_file["summary"]["page_count"], 1)
            self.assertEqual(
                direct_file["resources"][0]["locator"]["width_points"], 300
            )
            wrapper = inventory.build_inventory(
                repo_root=repo,
                manifest_path=item_root,
                payload_source_root=payload_root,
                event_date="2026-10-01",
            )
            self.assertEqual(wrapper["files"][0]["profile"], "pdf_pages_v1")

            output, errors = io.StringIO(), io.StringIO()
            argv = [
                "build_source_resource_inventories.py",
                "--repo-root",
                str(repo),
                "--payload-source-root",
                str(payload_root),
                "--event-date",
                "2026-10-01",
            ]
            with contextlib.redirect_stdout(output), contextlib.redirect_stderr(errors):
                with patch.object(inventory.sys, "argv", argv):
                    self.assertEqual(inventory.main(), 0, errors.getvalue())
            self.assertIn("[ok] wrote 1 source resource inventories", output.getvalue())
            published = item_root.parent / "resource-inventory.json"
            actual = json.loads(published.read_bytes())
            self.assertEqual(actual["generated_from_manifest_ref"], item_ref)
            self.assertFalse(published.read_bytes().find(body) >= 0)

            output, errors = io.StringIO(), io.StringIO()
            with contextlib.redirect_stdout(output), contextlib.redirect_stderr(errors):
                with patch.object(inventory.sys, "argv", [*argv, "--check"]):
                    self.assertEqual(inventory.main(), 0, errors.getvalue())
            self.assertIn("[ok] verified 1 source resource inventories", output.getvalue())


if __name__ == "__main__":
    unittest.main()
