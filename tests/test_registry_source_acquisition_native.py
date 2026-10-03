"""Maintained, provider-free consumer checks for the native registry route."""
from __future__ import annotations

import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import acquisition_native
import acquire_registry_sources as facade
from tests.oracles.acquisition import prepare_registry_sources as preparation_builder
from tests.oracles.acquisition import acquire_registry_sources as oracle


def _sha(body: bytes) -> str:
    return hashlib.sha256(body).hexdigest()


def _git_blob(body: bytes) -> str:
    return hashlib.sha1(b"blob " + str(len(body)).encode() + b"\0" + body).hexdigest()


def _write_json(path: Path, value: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def _tree_snapshot(root: Path):
    return tuple(
        (
            path.relative_to(root).as_posix(),
            "directory" if path.is_dir() else "file",
            path.stat().st_mode & 0o777,
            None if path.is_dir() else path.read_bytes(),
        )
        for path in sorted(root.rglob("*"))
    )


def _fixture(base: Path, name: str):
    root = base / name / "repository"
    payload_root = base / name / "payload-root"
    root.mkdir(parents=True)
    payload_root.mkdir(parents=True, mode=0o700)
    payload_root.chmod(0o700)
    for schema in (ROOT / "ToS/contracts").glob("*.schema.json"):
        destination = root / "ToS/contracts" / schema.name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(schema, destination)

    body = b'{"fixture:1":"Exact fixture source text"}\n'
    pin = "d6d54741b7f2ddfeca82f02c3f95eb3990b4e351"
    basename = "fixture_root-pli-ms.json"
    upstream_path = f"root/pli/ms/sutta/{basename}"
    entry = {
        "upstream_path": upstream_path,
        "basename": basename,
        "byte_size": len(body),
        "git_blob_sha1": _git_blob(body),
        "media_type": "application/json",
        "url": f"https://raw.githubusercontent.com/suttacentral/bilara-data/{pin}/{upstream_path}",
    }
    target = {
        "slug": "fixture-sutta",
        "title": "Fixture Sutta",
        "family": "pali-canon",
        "provider": "bilara",
        "repository": "suttacentral/bilara-data",
        "pin": pin,
        "language": "pli",
        "script": "Latn",
        "expression": "pli-fixture-root",
        "edition": f"bilara-{pin[:12]}",
        "item": "git-root-segment-json",
        "coverage": {"kind": "bilara-root", "uid": "fixture", "file_count": 1},
        "version_description": "One synthetic pinned source file for the native fixture consumer.",
        "responsibility": "Fixture source only; no source assessment or publication authority.",
        "limits": ["Synthetic test bytes only; no provider request or source acceptance."],
        "files": [entry],
    }
    identity = f"{target['family']}.{target['slug']}"
    expression_identity = f"{identity}.{target['expression']}"
    edition_identity = f"{expression_identity}.{target['edition']}"
    target["ids"] = {
        "work": f"tos.work.{identity}",
        "expression": f"tos.expression.{expression_identity}",
        "edition": f"tos.edition.{edition_identity}",
        "item": f"tos.item.{edition_identity}.{target['item']}",
    }
    work = f"{facade.SOURCE}/works/{target['family']}/{target['slug']}"
    expression = f"{work}/expressions/{target['expression']}"
    edition = f"{expression}/editions/{target['edition']}"
    item_root = f"{edition}/items/{target['item']}"
    target["paths"] = {
        "work": f"{work}/work.json",
        "expression": f"{expression}/expression.json",
        "edition": f"{edition}/edition.json",
        "item": f"{item_root}/item.json",
        "item_root": item_root,
    }

    relation_root = root / facade.SOURCE / "relations"
    topology = {
        "event_id": facade.TOPOLOGY_EVENT,
        "event_version": 1,
        "method": {"configuration": {}},
        "inputs": [],
        "outputs": [],
    }
    _write_json(root / facade.TOPOLOGY, topology)
    for relative in (
        "work-expression/work-expression-claims.jsonl",
        "expression-edition/expression-edition-claims.jsonl",
        "edition-item/edition-item-claims.jsonl",
    ):
        path = relation_root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"")

    registry_dir = root / facade.SOURCE / "discovery/registry-first-planting-2026-09-08"
    metadata_ref = "ToS/source-witnesses/discovery/registry-first-planting-2026-09-08/evidence/bilara-tree.json"
    metadata_body = b'{"fixture":"metadata only"}\n'
    (root / metadata_ref).parent.mkdir(parents=True, exist_ok=True)
    (root / metadata_ref).write_bytes(metadata_body)
    snapshot_ref = "ToS/source-witnesses/discovery/registry-first-planting-2026-09-08/source-registry-snapshot.json"
    snapshot_body = b'{"fixture":"normalized registry snapshot"}\n'
    (root / snapshot_ref).write_bytes(snapshot_body)
    metadata = {
        "url": f"https://api.github.com/repos/suttacentral/bilara-data/git/trees/{pin}",
        "retained_ref": metadata_ref,
        "retained_sha256": _sha(metadata_body),
        "retained_byte_size": len(metadata_body),
        "started_at": "2026-10-01T00:00:00+00:00",
        "ended_at": "2026-10-01T00:00:01+00:00",
        "elapsed_seconds": 1.0,
    }
    package = preparation_builder.prepare_package(target, "2026-10-01T00:00:01+00:00")
    packages_ref = "ToS/source-witnesses/discovery/registry-first-planting-2026-09-08/prepared-source-packages.jsonl"
    package_bytes = (json.dumps(package, ensure_ascii=False, separators=(",", ":")) + "\n").encode()
    (root / packages_ref).write_bytes(package_bytes)
    manifest = {
        "schema_version": "tos_registry_first_planting_preparation_v1",
        "status": "prepared-not-acquired",
        "prepared_packages_ref": packages_ref,
        "prepared_packages_sha256": _sha(package_bytes),
        "source_registry_snapshot_ref": snapshot_ref,
        "source_registry_snapshot_sha256": _sha(snapshot_body),
        "provider_pins": {"bilara": pin},
        "metadata_observations": [metadata],
        "targets": [target],
        "totals": {"works": 1, "payload_files": 1, "payload_bytes": len(body)},
    }
    manifest_path = registry_dir / "manifest.json"
    _write_json(manifest_path, manifest)
    manifest_sha256 = _sha(manifest_path.read_bytes())
    (root / ".gitignore").write_text(
        "/ToS/source-witnesses/works/**/items/*/payload/*\n", encoding="utf-8"
    )
    subprocess.run(["git", "init", "-q", str(root)], check=True, capture_output=True)
    subprocess.run(["git", "-C", str(root), "config", "user.name", "Fixture"], check=True)
    subprocess.run(
        ["git", "-C", str(root), "config", "user.email", "fixture@example.invalid"],
        check=True,
    )
    subprocess.run(["git", "-C", str(root), "add", ".gitignore"], check=True)
    subprocess.run(
        ["git", "-C", str(root), "commit", "-q", "-m", "fixture root"],
        check=True,
        capture_output=True,
    )
    commit = subprocess.check_output(["git", "-C", str(root), "rev-parse", "HEAD"], text=True).strip()
    receipt_path = registry_dir / "preparation-checkpoint.json"
    _write_json(
        receipt_path,
        {
            "manifest_sha256": manifest_sha256,
            "commit": commit,
            "runtime_session_id": "native-registry-fixture",
            "checkpoint_review_ref": "fixture:checkpoint-reviewed",
            "passed_checks": ["fixture preparation closure"],
            "status": "passed",
        },
    )
    return root, payload_root, manifest_path, receipt_path, target, package, entry, body


@unittest.skipUnless(
    os.environ.get("TOS_NATIVE_OWNER_COMMAND_BIN") or shutil.which("tos-native-owner-command"),
    "native owner command product not selected",
)
class NativeRegistryAcquisitionTests(unittest.TestCase):
    def _fetcher(self, entry: dict, body: bytes):
        def fetch(request: dict) -> bytes:
            self.assertEqual(request["url"], entry["url"])
            self.assertEqual(request["expected_byte_size"], entry["byte_size"])
            self.assertEqual(request["expected_git_blob_sha1"], entry["git_blob_sha1"])
            return body

        return fetch

    def _acquire(self, fixture, *, command_line: bool):
        root, payload_root, manifest_path, receipt_path, target, _, entry, body = fixture
        fetcher = self._fetcher(entry, body)
        request = {
            "family": "registry",
            "operation": "registry.acquire",
            "root": str(root),
            "manifest_path": str(manifest_path),
            "preparation_receipt_path": str(receipt_path),
            "payload_source_root": str(payload_root),
            "target_slugs": [target["slug"]],
        }
        if not command_line:
            return acquisition_native.invoke(request, fetcher=fetcher)
        output, errors = io.StringIO(), io.StringIO()
        argv = [
            "acquire_registry_sources.py",
            "acquire",
            "--manifest",
            str(manifest_path),
            "--preparation-receipt",
            str(receipt_path),
            "--payload-source-root",
            str(payload_root),
            "--target",
            target["slug"],
        ]
        with contextlib.redirect_stdout(output), contextlib.redirect_stderr(errors):
            with patch.object(facade, "ROOT", root), patch.object(facade.sys, "argv", argv):
                code = facade.main(fetcher=fetcher)
        self.assertEqual(code, 0, errors.getvalue())
        self.assertEqual(errors.getvalue(), "")
        return json.loads(output.getvalue())

    def test_native_api_and_cli_acquire_match_retained_oracle_and_receipts(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-registry-native-fixture-") as directory:
            base = Path(directory)
            api_fixture = _fixture(base, "api")
            cli_fixture = _fixture(base, "cli")
            api_result = self._acquire(api_fixture, command_line=False)
            cli_result = self._acquire(cli_fixture, command_line=True)
            api_item = api_result["results"][0]
            cli_item = cli_result
            self.assertEqual(api_item, cli_item)
            self.assertEqual(api_item["status"], "local-payload-and-owner-package-verified")
            self.assertFalse(api_item["textual_acceptance"])

            for fixture in (api_fixture, cli_fixture):
                root, payload_root, manifest_path, _, target, _, entry, body = fixture
                item_root = root / target["paths"]["item_root"]
                observed = json.loads((item_root / "forensic-observations.json").read_bytes())
                self.assertEqual(observed, oracle.inspect_payloads(target, [(entry, body)]))
                item_manifest = json.loads((item_root / "item.manifest.json").read_bytes())
                payload_file = item_manifest["payload_files"][0]
                self.assertEqual(payload_file["file_id"], "tos.file.sha256." + _sha(body))
                self.assertEqual(payload_file["sha256"], _sha(body))
                self.assertEqual(payload_file["byte_size"], len(body))
                self.assertEqual((payload_root / target["paths"]["item_root"].removeprefix("ToS/source-witnesses/") / "payload" / entry["basename"]).read_bytes(), body)
                log_rows = [json.loads(line) for line in (manifest_path.parent / "acquisition-transfers.jsonl").read_text().splitlines()]
                completed = [row for row in log_rows if row["status"] == "completed"]
                self.assertEqual(len(completed), 1)
                receipt = completed[0]
                self.assertEqual(receipt["http_status"], 200)
                self.assertEqual(receipt["final_url"], entry["url"])
                self.assertEqual(receipt["sha256"], _sha(body))
                self.assertEqual(receipt["expected_git_blob_sha1"], entry["git_blob_sha1"])
                events = [json.loads(line) for line in (item_root / "provenance.jsonl").read_text().splitlines()]
                acquisition = next(row for row in events if row["event_type"] == "acquisition")
                self.assertEqual(acquisition["event_id"], item_manifest["acquisition_event_ref"])
                self.assertTrue(any(row["ref"] == payload_file["file_id"] for row in acquisition["outputs"]))
                self.assertTrue((root / facade.TOPOLOGY).is_file())

    def test_malformed_selector_and_payload_root_fail_before_fetch_or_writes(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-registry-native-invalid-") as directory:
            root, payload_root, manifest_path, receipt_path, target, _, _, body = _fixture(
                Path(directory), "invalid"
            )
            request = {
                "family": "registry",
                "operation": "registry.acquire",
                "root": str(root),
                "manifest_path": str(manifest_path),
                "preparation_receipt_path": str(receipt_path),
                "payload_source_root": str(payload_root),
                "target_slugs": [target["slug"]],
            }
            invalid_requests = (
                {**request, "target_slugs": target["slug"]},
                {**request, "payload_source_root": {"path": str(payload_root)}},
            )
            for invalid in invalid_requests:
                with self.subTest(invalid=invalid):
                    before_repository = _tree_snapshot(root)
                    before_payloads = _tree_snapshot(payload_root)
                    fetch_calls = []

                    def fetcher(payload: dict) -> bytes:
                        fetch_calls.append(payload)
                        return body

                    with self.assertRaises(acquisition_native.NativeAcquisitionError):
                        acquisition_native.invoke(invalid, fetcher=fetcher)
                    self.assertEqual(fetch_calls, [])
                    self.assertEqual(_tree_snapshot(root), before_repository)
                    self.assertEqual(_tree_snapshot(payload_root), before_payloads)


if __name__ == "__main__":
    unittest.main()
