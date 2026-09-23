from __future__ import annotations

import hashlib
from http.client import IncompleteRead, LineTooLong
import json
import os
from pathlib import Path
import subprocess
import sys
import stat
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import acquisition_batch as acquisition  # noqa: E402
import source_payload_custody as custody  # noqa: E402


class AcquisitionBatchTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.metadata = self.root / "metadata"
        self.metadata.mkdir()
        self.manifest_path = self.root / "selection.json"
        self.output = self.root / "handoff"

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _write_manifest(
        self,
        *,
        count: int = 2,
        shared_payload: bool = False,
        rights_posture: str = "local_only",
        rights_visibility: str | None = None,
        rights_redistribution: str | None = None,
    ) -> tuple[dict[str, bytes], str]:
        fetches: dict[str, bytes] = {}
        selections: list[dict] = []
        all_record_refs: list[str] = []
        all_payload_refs: list[str] = []
        for index in range(count):
            slug = f"fixture-{index}"
            item_ref = f"tos.item.fixture.{index}"
            item_root = (
                f"ToS/source-witnesses/works/fixture/expressions/en/"
                f"editions/pinned/items/{slug}"
            )
            item_record_ref = f"{item_root}/item.json"
            item_manifest_ref = f"{item_root}/item.manifest.json"
            rights_ref = f"{item_root}/rights.json"
            provenance_ref = f"{item_root}/provenance.jsonl"
            forensic_report_ref = f"{item_root}/forensic-report.md"
            resource_inventory_ref = f"{item_root}/resource-inventory.json"
            fixity_ref = f"{item_root}/fixity.sha256"
            payload = f"payload/{slug}.txt"
            payload_body = (
                b"shared content-addressed payload\n"
                if shared_payload
                else f"payload {index}\n".encode()
            )
            payload_ref = f"tos.file.sha256.{hashlib.sha256(payload_body).hexdigest()}"
            event_ref = f"tos.event.acquisition.fixture-{index}"
            rights_value = {
                "rights": "local_only",
                "scope_refs": [item_ref, payload_ref],
                "visibility": rights_visibility or "local_only",
            }
            if rights_redistribution is not None:
                rights_value["redistribution_posture"] = rights_redistribution
            rights_body = json.dumps(rights_value, sort_keys=True).encode()
            item_manifest_body = json.dumps(
                {
                    "schema_version": "tos_source_item_manifest_v1",
                    "item_id": item_ref,
                    "item_kind": "born_digital",
                    "embodiment_ref": f"tos.edition.fixture.{index}",
                    "storage_posture": "local_gitignored_payload",
                    "payload_files": [
                        {
                            "file_id": payload_ref,
                            "relative_path": payload,
                            "original_basename": f"{slug}.txt",
                            "media_type": "text/plain",
                            "byte_size": len(payload_body),
                            "sha256": hashlib.sha256(payload_body).hexdigest(),
                            "fixity_verified_at": "2026-09-21T12:00:00Z",
                        }
                    ],
                    "acquisition_event_ref": event_ref,
                    "rights_ref": rights_ref,
                    "provenance_ref": provenance_ref,
                    "forensic_report_ref": forensic_report_ref,
                    "resource_inventory_ref": resource_inventory_ref,
                    "visibility": "local_only",
                    "manifest_version": 1,
                },
                sort_keys=True,
            ).encode() + b"\n"
            provenance_body = (
                json.dumps(
                    {
                        "schema_version": "tos_provenance_event_v1",
                        "event_id": event_ref,
                        "event_type": "acquisition",
                        "status": "completed",
                        "event_version": 1,
                        "rights_basis_ref": rights_ref,
                        "outputs": [
                            {
                                "ref": f"{item_root}/{payload}",
                                "sha256": hashlib.sha256(payload_body).hexdigest(),
                            }
                        ],
                    },
                    sort_keys=True,
                )
                + "\n"
            ).encode()
            payload_sha256 = hashlib.sha256(payload_body).hexdigest()
            inventory_event_ref = f"tos.event.inventory.fixture-{index}"
            inventory_body = json.dumps(
                {
                    "$schema": "https://tree-of-sophia.local/ToS/contracts/source-resource-inventory.schema.json",
                    "schema_version": "tos_source_resource_inventory_v1",
                    "item_id": item_ref,
                    "generated_from_manifest_ref": item_manifest_ref,
                    "inventory_authority": "mechanical_metadata_only",
                    "source_text_included": False,
                    "files": [
                        {
                            "file_id": payload_ref,
                            "file_sha256": payload_sha256,
                            "media_type": "text/plain",
                            "profile": "plain_text_v1",
                            "summary": {"resource_count": 1},
                            "resources": [
                                {
                                    "resource_id": "fixture-resource",
                                    "resource_kind": "plain_text_file",
                                    "locator": {"container_order": 1},
                                    "structural_role": "member",
                                    "content_fingerprint": {
                                        "algorithm": "sha256",
                                        "normalization": "unicode-codepoints-preserved",
                                        "sha256": payload_sha256,
                                        "character_count": len(payload_body.decode("utf-8")),
                                    },
                                }
                            ],
                        }
                    ],
                    "generator": {
                        "name": "build_source_resource_inventories.py",
                        "version": "1",
                    },
                    "provenance_event_ref": inventory_event_ref,
                    "inventory_version": 1,
                    "authority_boundary": "Fixture metadata only; no source text is included.",
                },
                sort_keys=True,
            ).encode() + b"\n"
            provenance_event = {
                "schema_version": "tos_provenance_event_v1",
                "event_id": inventory_event_ref,
                "event_type": "forensic_inspection",
                "started_at": "2026-09-21T12:00:00Z",
                "ended_at": "2026-09-21T12:00:00Z",
                "agent_refs": ["software:tos-source-item-commands"],
                "inputs": [],
                "outputs": [
                    {
                        "ref": resource_inventory_ref,
                        "role": "tracked_text_free_resource_inventory",
                        "sha256": hashlib.sha256(inventory_body).hexdigest(),
                    }
                ],
                "method": {
                    "maker_type": "software",
                    "name": "build_source_resource_inventories.py",
                    "version": "1",
                    "configuration": {},
                },
                "status": "completed",
                "event_version": 1,
            }
            provenance_body += (
                json.dumps(provenance_event, sort_keys=True) + "\n"
            ).encode()
            forensic_report_body = b"Fixture forensic report; no interpretation was accepted.\n"
            fixity_body = f"{payload_sha256}  {payload}\n".encode()
            records = []
            for ref, kind, body in (
                (
                    item_record_ref,
                    "item",
                    json.dumps(
                        {
                            "record_type": "item",
                            "record_id": item_ref,
                            "item_manifest_ref": item_manifest_ref,
                        }
                    ).encode(),
                ),
                (item_manifest_ref, "manifest", item_manifest_body),
                (rights_ref, "rights", rights_body),
                (provenance_ref, "provenance", provenance_body),
                (forensic_report_ref, "discovery", forensic_report_body),
                (resource_inventory_ref, "discovery", inventory_body),
                (fixity_ref, "discovery", fixity_body),
            ):
                path = self.metadata / ref
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(body)
                records.append({"ref": ref, "sha256": hashlib.sha256(body).hexdigest(), "kind": kind})
                all_record_refs.append(ref)
            url = f"https://provider.example/{slug}/r1/{slug}.txt"
            fetches[payload_ref] = payload_body
            all_payload_refs.append(payload_ref)
            selections.append(
                {
                    "item_ref": item_ref,
                    "item_root_ref": item_root,
                    "provider": {
                        "name": "fixture-provider",
                        "revision": "r1",
                        "source_id": "fixture-collection-v1",
                        "source_url": "https://provider.example/fixture-collection-v1",
                    },
                    "records": records,
                    "rights": {
                        "ref": rights_ref,
                        "sha256": next(
                            row["sha256"] for row in records if row["ref"] == rights_ref
                        ),
                        "posture": rights_posture,
                    },
                    "payload_files": [
                        {
                            "item_ref": item_ref,
                            "file_ref": payload_ref,
                            "item_root_ref": item_root,
                            "relative_path": payload,
                            "byte_size": len(payload_body),
                            "sha256": hashlib.sha256(payload_body).hexdigest(),
                            "provider_url": url,
                            "provider_revision": "r1",
                            "provider_source_id": "fixture-collection-v1",
                            "media_type": "text/plain",
                        }
                    ],
                }
            )
        manifest = {
            "$schema": "https://tree-of-sophia.local/ToS/contracts/acquisition-batch.schema.json",
            "schema_version": "tos_acquisition_batch_v1",
            "batch_id": "tos.acquisition-batch.fixture-20260921",
            "batch_revision": 1,
            "prepared_at": "2026-09-21T12:00:00Z",
            "base_revision": "a" * 64,
            "selection": selections,
            "provenance_delta": {
                "event_ref": "tos.event.acquisition-batch.fixture-20260921",
                "event_version": 1,
                "change_kind": "batch_delta",
                "base_revision": "a" * 64,
                "record_refs": sorted(all_record_refs),
                "payload_file_refs": sorted(set(all_payload_refs)),
                "supersedes_event_ref": None,
            },
        }
        self.manifest_path.write_bytes(
            (json.dumps(manifest, ensure_ascii=False, indent=2) + "\n").encode()
        )
        return fetches, hashlib.sha256(self.manifest_path.read_bytes()).hexdigest()

    def test_prepare_copies_only_selected_records_and_zero_topology_preimages(self) -> None:
        _fetches, manifest_sha = self._write_manifest(count=8)
        result = acquisition.prepare_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.output,
            expected_manifest_sha256=manifest_sha,
        )
        self.assertEqual("prepared-not-acquired", result["status"])
        measurement = acquisition.measure_storage(self.output)
        self.assertEqual(0, measurement["topology_preimage_count"])
        self.assertEqual(0, measurement["topology_preimage_bytes"])
        self.assertEqual(8 * 7 + 1, measurement["metadata_file_count"])
        legacy = self.root / "legacy" / "topology-before"
        legacy.mkdir(parents=True)
        for index in range(8):
            (legacy / f"{index}.json").write_bytes((bytes([index]) * 8192))
        legacy_bytes = sum(path.stat().st_size for path in legacy.iterdir())
        self.assertLess(measurement["metadata_bytes"], legacy_bytes)
        self.assertTrue((self.output / "receipts/preparation.json").is_file())
        self.assertTrue(
            (self.output / "source/ToS/source-witnesses/discovery/acquisition-batches/fixture-20260921/provenance-delta.json").is_file()
        )
        for directory in ("source", "payload", "receipts"):
            self.assertEqual(0o700, stat.S_IMODE((self.output / directory).stat().st_mode))
        delta = json.loads(
            (self.output / "source/ToS/source-witnesses/discovery/acquisition-batches/fixture-20260921/provenance-delta.json").read_text(encoding="utf-8")
        )
        schema = json.loads(
            (ROOT / "ToS/contracts/acquisition-provenance-delta.schema.json").read_text(encoding="utf-8")
        )
        Draft202012Validator(schema).validate(delta)

    def test_prepared_custody_roots_must_remain_private(self) -> None:
        _fetches, manifest_sha = self._write_manifest(count=1)
        acquisition.prepare_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.output,
            expected_manifest_sha256=manifest_sha,
        )
        self.output.chmod(0o755)
        with self.assertRaisesRegex(acquisition.AcquisitionBatchError, "owner-only"):
            acquisition.verify_local(output_root=self.output)
        self.output.chmod(0o700)

    def test_failure_isolated_and_restart_retries_only_failed_file(self) -> None:
        fetches, manifest_sha = self._write_manifest(count=2)
        ordered = list(fetches)
        calls: list[str] = []
        def first_fetch(payload: dict) -> bytes:
            calls.append(payload["file_ref"])
            if payload["file_ref"] == ordered[0]:
                raise acquisition.SourceFetchError("temporary provider outage")
            return fetches[payload["file_ref"]]

        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.output,
            expected_manifest_sha256=manifest_sha,
            fetcher=first_fetch,
            max_attempts=1,
        )
        self.assertEqual("partially-acquired-not-admitted", result["status"])
        self.assertEqual("not-admitted", result["admission_status"])
        self.assertEqual(2, len(calls))
        self.assertEqual("incomplete", acquisition.verify_local(output_root=self.output)["status"])

        def retry_fetch(payload: dict) -> bytes:
            calls.append(payload["file_ref"])
            self.assertEqual(ordered[0], payload["file_ref"])
            return fetches[payload["file_ref"]]

        retried = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.output,
            expected_manifest_sha256=manifest_sha,
            fetcher=retry_fetch,
            max_attempts=1,
        )
        self.assertEqual("acquired-not-admitted", retried["status"])
        self.assertEqual("not-admitted", retried["admission_status"])
        self.assertEqual(3, len(calls))
        self.assertEqual("verified", acquisition.verify_local(output_root=self.output)["status"])
        journal = (self.output / "receipts/acquisition.jsonl").read_text(encoding="utf-8")
        self.assertIn('"status": "failed"', journal)
        self.assertIn('"status": "acquired"', journal)
        handoff = json.loads((self.output / retried["handoff_ref"]).read_text(encoding="utf-8"))
        self.assertEqual("not-admitted", handoff["admission_status"])
        self.assertTrue(handoff["restartable"])
        self.assertEqual(0, handoff["topology_preimages"])
        self.assertEqual(
            hashlib.sha256(
                (self.output / handoff["independent_fixity"]["ref"]).read_bytes()
            ).hexdigest(),
            handoff["independent_fixity"]["jsonl_sha256"],
        )
        self.assertEqual(
            hashlib.sha256(
                (self.output / handoff["independent_fixity"]["summary_ref"]).read_bytes()
            ).hexdigest(),
            handoff["independent_fixity"]["summary_sha256"],
        )
        self.assertEqual(
            hashlib.sha256(
                (self.output / handoff["provenance_delta"]["ref"]).read_bytes()
            ).hexdigest(),
            handoff["provenance_delta"]["sha256"],
        )
        self.assertEqual(
            "source/ToS/source-witnesses/discovery/acquisition-batches/fixture-20260921/provenance-delta.json",
            handoff["provenance_delta"]["ref"],
        )

    @unittest.skipUnless(hasattr(os, "link"), "payload hard-link boundary")
    def test_hardlinked_payload_is_rejected_without_blocking_other_items(self) -> None:
        fetches, manifest_sha = self._write_manifest(count=2)
        acquisition.prepare_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.output,
            expected_manifest_sha256=manifest_sha,
        )
        selections = json.loads(self.manifest_path.read_text(encoding="utf-8"))["selection"]
        first = selections[0]["payload_files"][0]
        destination = custody.payload_path(
            self.output / "payload", first["item_root_ref"], first["relative_path"]
        )
        destination.parent.mkdir(parents=True, exist_ok=True)
        outside_alias = self.root / "outside-payload.bin"
        outside_alias.write_bytes(fetches[first["file_ref"]])
        outside_alias.chmod(0o444)
        original = outside_alias.read_bytes()
        os.link(outside_alias, destination)

        fetch_calls: list[str] = []
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.output,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetch_calls.append(payload["item_ref"])
            or fetches[payload["file_ref"]],
            max_attempts=1,
        )

        self.assertEqual("partially-acquired-not-admitted", result["status"])
        self.assertEqual([selections[1]["item_ref"]], fetch_calls)
        self.assertEqual(original, outside_alias.read_bytes())
        handoff = json.loads((self.output / result["handoff_ref"]).read_text())
        custody_by_item = {row["item_ref"]: row for row in handoff["payload_custody"]}
        self.assertEqual("conflict", custody_by_item[selections[0]["item_ref"]]["status"])
        self.assertEqual("acquired", custody_by_item[selections[1]["item_ref"]]["status"])
        fixity_rows = acquisition._portable_jsonl_rows(
            self.output / result["fixity_ref"], label="fixity JSONL"
        )
        fixity_by_item = {row["item_ref"]: row for row in fixity_rows}
        self.assertEqual("missing-or-invalid", fixity_by_item[selections[0]["item_ref"]]["status"])
        self.assertEqual("verified", fixity_by_item[selections[1]["item_ref"]]["status"])
        self.assertEqual("incomplete", acquisition.verify_local(output_root=self.output)["status"])

    def test_invalid_payload_path_does_not_block_later_items(self) -> None:
        fetches, manifest_sha = self._write_manifest(count=2)
        acquisition.prepare_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.output,
            expected_manifest_sha256=manifest_sha,
        )
        selections = json.loads(self.manifest_path.read_text(encoding="utf-8"))["selection"]
        first_item_root = self.output / "payload" / Path(
            *selections[0]["item_root_ref"].split("/")[2:]
        )
        first_item_root.parent.mkdir(parents=True, exist_ok=True)
        outside = self.root / "outside-item-payload"
        outside.mkdir()
        first_item_root.symlink_to(outside, target_is_directory=True)

        fetch_calls: list[str] = []
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.output,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetch_calls.append(payload["item_ref"])
            or fetches[payload["file_ref"]],
            max_attempts=1,
        )

        self.assertEqual("partially-acquired-not-admitted", result["status"])
        self.assertEqual([selections[1]["item_ref"]], fetch_calls)
        handoff = json.loads((self.output / result["handoff_ref"]).read_text())
        custody_by_item = {row["item_ref"]: row for row in handoff["payload_custody"]}
        self.assertEqual("conflict", custody_by_item[selections[0]["item_ref"]]["status"])
        self.assertEqual("acquired", custody_by_item[selections[1]["item_ref"]]["status"])
        fixity_rows = acquisition._portable_jsonl_rows(
            self.output / result["fixity_ref"], label="fixity JSONL"
        )
        fixity_by_item = {row["item_ref"]: row for row in fixity_rows}
        self.assertEqual("missing-or-invalid", fixity_by_item[selections[0]["item_ref"]]["status"])
        self.assertEqual("verified", fixity_by_item[selections[1]["item_ref"]]["status"])
        self.assertEqual("incomplete", acquisition.verify_local(output_root=self.output)["status"])

    def test_payload_digest_rejects_unexpected_owner_on_open_descriptor(self) -> None:
        payload_path = self.root / "payload-owner.bin"
        payload_path.write_bytes(b"owned payload\n")
        payload_path.chmod(0o444)
        actual_owner = payload_path.stat().st_uid
        with self.assertRaisesRegex(custody.CustodyError, "owner differs"):
            custody.digest_file(
                payload_path,
                expected_mode=0o444,
                expected_owner_uid=actual_owner + 1,
                require_single_link=True,
            )

    def test_destination_verification_rejects_path_replaced_while_hashing(self) -> None:
        payload_path = self.root / "payload-race.bin"
        original_body = b"sealed bytes\n"
        payload_path.write_bytes(original_body)
        payload_path.chmod(0o444)
        moved_original = self.root / "payload-original.bin"
        replacement_body = b"replacement bytes\n"
        original_stat = os.stat
        replaced = False

        def replace_before_path_stat(path: str | Path, *args, **kwargs):
            nonlocal replaced
            if (
                Path(path) == payload_path
                and kwargs.get("follow_symlinks") is False
                and not replaced
            ):
                os.replace(payload_path, moved_original)
                payload_path.write_bytes(replacement_body)
                payload_path.chmod(0o444)
                replaced = True
            return original_stat(path, *args, **kwargs)

        expected = {
            "byte_size": len(original_body),
            "sha256": hashlib.sha256(original_body).hexdigest(),
            "file_ref": f"tos.file.sha256.{hashlib.sha256(original_body).hexdigest()}",
        }
        with patch.object(custody.os, "stat", side_effect=replace_before_path_stat):
            with self.assertRaisesRegex(
                acquisition.SourceIntegrityError, "pathname changed while hashing"
            ):
                acquisition._verify_destination(
                    payload_path, expected, expected_owner_uid=os.geteuid()
                )
        self.assertTrue(replaced)
        self.assertEqual(original_body, moved_original.read_bytes())
        self.assertEqual(replacement_body, payload_path.read_bytes())

    def test_shared_file_id_keeps_separate_item_custody_and_fixity_rows(self) -> None:
        fetches, manifest_sha = self._write_manifest(count=2, shared_payload=True)
        calls: list[str] = []
        item_refs = [
            selection["item_ref"]
            for selection in json.loads(self.manifest_path.read_text())["selection"]
        ]

        def fetch(payload: dict) -> bytes:
            calls.append(payload["item_ref"])
            if payload["item_ref"] == item_refs[1] and calls.count(item_refs[1]) == 1:
                raise acquisition.SourceFetchError("one Item destination temporarily unavailable")
            return fetches[payload["file_ref"]]

        first = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.output,
            expected_manifest_sha256=manifest_sha,
            fetcher=fetch,
            max_attempts=1,
        )
        self.assertEqual("partially-acquired-not-admitted", first["status"])
        self.assertEqual("incomplete", acquisition.verify_local(output_root=self.output)["status"])

        resumed = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.output,
            expected_manifest_sha256=manifest_sha,
            fetcher=fetch,
            max_attempts=1,
        )
        self.assertEqual("acquired-not-admitted", resumed["status"])
        local = acquisition.verify_local(output_root=self.output)
        self.assertEqual("verified", local["status"])
        self.assertEqual(2, len(local["rows"]))
        self.assertEqual(2, len(set(row["destination_ref"] for row in local["rows"])))
        self.assertEqual(2, len(set(row["item_ref"] for row in local["rows"])))
        self.assertEqual([item_refs[0], item_refs[1], item_refs[1]], calls)

        handoff = json.loads((self.output / resumed["handoff_ref"]).read_text())
        self.assertEqual(2, len(handoff["payload_custody"]))
        self.assertEqual(2, len({row["item_ref"] for row in handoff["payload_custody"]}))
        self.assertEqual(1, len({row["file_ref"] for row in handoff["payload_custody"]}))
        fixity_rows = acquisition._journal_rows(
            self.output / handoff["independent_fixity"]["ref"]
        )
        self.assertEqual(2, len(fixity_rows))
        self.assertEqual(2, len({row["destination_ref"] for row in fixity_rows}))

    def test_shared_file_id_requires_one_global_content_descriptor(self) -> None:
        for field, conflicting_value in (
            ("byte_size", 999),
            ("media_type", "application/pdf"),
        ):
            with self.subTest(field=field):
                self._write_manifest(count=2, shared_payload=True)
                manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
                manifest["selection"][1]["payload_files"][0][field] = conflicting_value
                self.manifest_path.write_text(
                    json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
                    encoding="utf-8",
                )
                caller_selected_sha = hashlib.sha256(
                    self.manifest_path.read_bytes()
                ).hexdigest()
                fetch_calls: list[str] = []

                with self.assertRaisesRegex(
                    acquisition.AcquisitionBatchError,
                    "payload File descriptor differs across Items",
                ):
                    acquisition.acquire_batch(
                        manifest_path=self.manifest_path,
                        metadata_root=self.metadata,
                        output_root=self.output,
                        expected_manifest_sha256=caller_selected_sha,
                        fetcher=lambda payload: fetch_calls.append(payload["item_ref"])
                        or b"unexpected",
                    )

                self.assertEqual([], fetch_calls)
                self.assertFalse(self.output.exists())

    def test_same_item_cannot_bind_one_file_id_to_two_destinations(self) -> None:
        _fetches, _manifest_sha = self._write_manifest(count=1)
        manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        selection = manifest["selection"][0]
        payload = selection["payload_files"][0]
        selection["payload_files"].insert(
            0,
            {**payload, "relative_path": "payload/not-in-item-manifest.txt"},
        )
        self.manifest_path.write_text(
            json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        caller_selected_sha = hashlib.sha256(self.manifest_path.read_bytes()).hexdigest()
        fetches: list[str] = []
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "duplicate payload File ID within Item",
        ):
            acquisition.acquire_batch(
                manifest_path=self.manifest_path,
                metadata_root=self.metadata,
                output_root=self.output,
                expected_manifest_sha256=caller_selected_sha,
                fetcher=lambda row: fetches.append(row["file_ref"]) or b"unexpected",
            )
        self.assertEqual([], fetches)
        self.assertFalse(self.output.exists())

    def test_resume_rejects_writable_preexisting_payload(self) -> None:
        fetches, manifest_sha = self._write_manifest(count=1)
        payload = json.loads(self.manifest_path.read_text())["selection"][0]["payload_files"][0]
        for mode in (0o644, 0o666):
            with self.subTest(mode=oct(mode)):
                output = self.root / f"output-mode-{mode:o}"
                acquired = acquisition.acquire_batch(
                    manifest_path=self.manifest_path,
                    metadata_root=self.metadata,
                    output_root=output,
                    expected_manifest_sha256=manifest_sha,
                    fetcher=lambda item: fetches[item["file_ref"]],
                )
                self.assertEqual("acquired-not-admitted", acquired["status"])
                destination = custody.payload_path(
                    output / "payload",
                    payload["item_root_ref"],
                    payload["relative_path"],
                )
                destination.chmod(mode)

                retry_calls: list[str] = []

                def unexpected_fetch(item: dict) -> bytes:
                    retry_calls.append(item["file_ref"])
                    return fetches[item["file_ref"]]

                resumed = acquisition.acquire_batch(
                    manifest_path=self.manifest_path,
                    metadata_root=self.metadata,
                    output_root=output,
                    expected_manifest_sha256=manifest_sha,
                    fetcher=unexpected_fetch,
                )
                self.assertEqual([], retry_calls)
                self.assertEqual("prepared-not-acquired", resumed["status"])
                self.assertEqual(
                    "incomplete", acquisition.verify_local(output_root=output)["status"]
                )
                self.assertEqual(mode, stat.S_IMODE(destination.stat().st_mode))

    def test_provider_wrong_bytes_do_not_overwrite_or_mislabel_acquisition(self) -> None:
        fetches, manifest_sha = self._write_manifest(count=1)
        file_ref, body = next(iter(fetches.items()))

        def wrong_fetch(_payload: dict) -> bytes:
            return b"wrong bytes"

        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.output,
            expected_manifest_sha256=manifest_sha,
            fetcher=wrong_fetch,
            max_attempts=2,
        )
        self.assertEqual("prepared-not-acquired", result["status"])
        self.assertEqual(0, result["verified_payload_count"])
        self.assertEqual("incomplete", acquisition.verify_local(output_root=self.output)["status"])
        destination = self.output / "payload/ToS/source-witnesses/works/fixture/expressions/en/editions/pinned/items/fixture-0/payload/fixture-0.txt"
        self.assertFalse(destination.exists())

    def test_truncated_http_response_is_a_retryable_source_failure(self) -> None:
        _fetches, _manifest_sha = self._write_manifest(count=1)
        payload = {
            "provider_url": "https://provider.example/truncated",
            "file_ref": "tos.file.sha256." + "0" * 64,
            "byte_size": 10,
        }

        class ProtocolFailureResponse:
            def __init__(self, error: BaseException) -> None:
                self.error = error

            def __enter__(self):
                return self

            def __exit__(self, _type, _value, _traceback):
                return False

            def read(self, _limit: int) -> bytes:
                raise self.error

        for error in (IncompleteRead(b"partial", 3), LineTooLong("chunk-size")):
            with self.subTest(error=type(error).__name__):
                with patch.object(
                    acquisition,
                    "urlopen",
                    return_value=ProtocolFailureResponse(error),
                ):
                    with self.assertRaisesRegex(
                        acquisition.SourceFetchError, "provider fetch failed"
                    ):
                        acquisition._fetch_url(payload)

        malformed = {**payload, "provider_url": "http://[::1/x"}
        with patch.object(acquisition, "urlopen") as urlopen:
            with self.assertRaisesRegex(
                acquisition.SourceFetchError, "provider fetch failed"
            ):
                acquisition._fetch_url(malformed)
            urlopen.assert_not_called()

    def test_public_payload_posture_cannot_widen_selected_rights(self) -> None:
        fetches, manifest_sha = self._write_manifest(
            count=1,
            rights_posture="public_payload",
            rights_visibility="local_only",
            rights_redistribution="not_authorized",
        )
        fetch_calls: list[str] = []
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "declared rights posture public_payload",
        ):
            acquisition.acquire_batch(
                manifest_path=self.manifest_path,
                metadata_root=self.metadata,
                output_root=self.output,
                expected_manifest_sha256=manifest_sha,
                fetcher=lambda payload: fetch_calls.append(payload["file_ref"])
                or fetches[payload["file_ref"]],
            )
        self.assertEqual([], fetch_calls)
        self.assertEqual([], list(self.output.glob("receipts/handoff-*.json")))

    def test_rights_visibility_must_match_item_manifest_before_fetch(self) -> None:
        fetches, manifest_sha = self._write_manifest(
            count=1,
            rights_posture="public_payload",
            rights_visibility="public_payload",
            rights_redistribution="authorized",
        )
        fetch_calls: list[str] = []
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "rights visibility differs from Item manifest visibility",
        ):
            acquisition.acquire_batch(
                manifest_path=self.manifest_path,
                metadata_root=self.metadata,
                output_root=self.output,
                expected_manifest_sha256=manifest_sha,
                fetcher=lambda payload: fetch_calls.append(payload["file_ref"])
                or fetches[payload["file_ref"]],
            )
        self.assertEqual([], fetch_calls)
        self.assertEqual([], list(self.output.glob("receipts/handoff-*.json")))

    def test_public_payload_posture_rejects_values_outside_rights_contract(self) -> None:
        cases = (
            ("public", "authorized"),
            ("public_payload", "allowed"),
            ("public_payload", "open"),
            ("public_payload", "public"),
        )
        for index, (visibility, redistribution) in enumerate(cases):
            with self.subTest(visibility=visibility, redistribution=redistribution):
                fetches, manifest_sha = self._write_manifest(
                    count=1,
                    rights_posture="public_payload",
                    rights_visibility=visibility,
                    rights_redistribution=redistribution,
                )
                output = self.root / f"public-posture-{index}"
                fetch_calls: list[str] = []
                with self.assertRaisesRegex(
                    acquisition.AcquisitionBatchError,
                    "declared rights posture public_payload",
                ):
                    acquisition.acquire_batch(
                        manifest_path=self.manifest_path,
                        metadata_root=self.metadata,
                        output_root=output,
                        expected_manifest_sha256=manifest_sha,
                        fetcher=lambda payload: fetch_calls.append(payload["file_ref"])
                        or fetches[payload["file_ref"]],
                    )
                self.assertEqual([], fetch_calls)
                self.assertEqual([], list(output.glob("receipts/handoff-*.json")))

    def test_selected_metadata_modes_are_rejected_before_provider_fetch(self) -> None:
        for mode in (0o600, 0o755):
            with self.subTest(mode=oct(mode)):
                fetches, manifest_sha = self._write_manifest(count=1)
                manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
                record = manifest["selection"][0]["records"][0]
                (self.metadata / record["ref"]).chmod(mode)
                output = self.root / f"metadata-mode-{mode:o}"
                fetch_calls: list[str] = []
                with self.assertRaisesRegex(
                    acquisition.AcquisitionBatchError,
                    "selected source mode is unsupported for candidate update",
                ):
                    acquisition.acquire_batch(
                        manifest_path=self.manifest_path,
                        metadata_root=self.metadata,
                        output_root=output,
                        expected_manifest_sha256=manifest_sha,
                        fetcher=lambda payload: fetch_calls.append(payload["file_ref"])
                        or fetches[payload["file_ref"]],
                    )
                self.assertEqual([], fetch_calls)
                self.assertFalse(output.exists())

    def test_selected_provenance_rejects_duplicate_keys_before_provider_fetch(self) -> None:
        fetches, _manifest_sha = self._write_manifest(count=1)
        manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        provenance_record = next(
            row
            for row in manifest["selection"][0]["records"]
            if row["kind"] == "provenance"
        )
        provenance_path = self.metadata / provenance_record["ref"]
        provenance_body = provenance_path.read_text(encoding="utf-8")
        self.assertIn('"status": "completed"', provenance_body)
        provenance_body = provenance_body.replace(
            '"status": "completed"',
            '"status": "failed", "status": "completed"',
            1,
        )
        provenance_path.write_text(provenance_body, encoding="utf-8")
        provenance_record["sha256"] = hashlib.sha256(
            provenance_path.read_bytes()
        ).hexdigest()
        self.manifest_path.write_text(
            json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        manifest_sha = hashlib.sha256(self.manifest_path.read_bytes()).hexdigest()
        fetch_calls: list[str] = []
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "prepared Item provenance is not valid JSONL",
        ):
            acquisition.acquire_batch(
                manifest_path=self.manifest_path,
                metadata_root=self.metadata,
                output_root=self.root / "duplicate-provenance",
                expected_manifest_sha256=manifest_sha,
                fetcher=lambda payload: fetch_calls.append(payload["file_ref"])
                or fetches[payload["file_ref"]],
            )
        self.assertEqual([], fetch_calls)

    def test_resume_rejects_unsupported_prepared_metadata_mode_before_provider_fetch(self) -> None:
        fetches, manifest_sha = self._write_manifest(count=1)
        manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        record = manifest["selection"][0]["records"][0]
        output = self.root / "prepared-metadata-mode"
        acquisition.prepare_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=output,
            expected_manifest_sha256=manifest_sha,
        )
        (output / "source" / record["ref"]).chmod(0o600)
        fetch_calls: list[str] = []
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "prepared selected source mode is unsupported for candidate update",
        ):
            acquisition.acquire_batch(
                manifest_path=self.manifest_path,
                metadata_root=self.metadata,
                output_root=output,
                expected_manifest_sha256=manifest_sha,
                fetcher=lambda payload: fetch_calls.append(payload["file_ref"])
                or fetches[payload["file_ref"]],
            )
        self.assertEqual([], fetch_calls)

    def test_manifest_digest_and_revision_binding_fail_closed(self) -> None:
        _fetches, manifest_sha = self._write_manifest(count=1)
        with self.assertRaisesRegex(acquisition.AcquisitionBatchError, "manifest SHA-256"):
            acquisition.load_manifest(self.manifest_path, expected_sha256="0" * 64)
        manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        manifest["provenance_delta"]["base_revision"] = "b" * 64
        self.manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        with self.assertRaisesRegex(acquisition.AcquisitionBatchError, "base revision"):
            acquisition.load_manifest(self.manifest_path)

    def test_manifest_requires_item_manifest_and_provenance_records(self) -> None:
        _fetches, _manifest_sha = self._write_manifest(count=1)
        manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        manifest["selection"][0]["records"] = [
            record
            for record in manifest["selection"][0]["records"]
            if record["kind"] != "manifest"
        ]
        manifest["provenance_delta"]["record_refs"] = sorted(
            record["ref"] for record in manifest["selection"][0]["records"]
        )
        self.manifest_path.write_bytes(
            (json.dumps(manifest, ensure_ascii=False, indent=2) + "\n").encode()
        )
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "schema validation failed",
        ):
            acquisition.load_manifest(self.manifest_path)

    def test_manifest_requires_the_selected_item_manifest_and_provenance_paths(self) -> None:
        fetches, _manifest_sha = self._write_manifest(count=1)
        manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        item_root = manifest["selection"][0]["item_root_ref"]
        manifest["selection"][0]["records"] = [
            {
                **record,
                "ref": (
                    f"{item_root}/batch-scope.json"
                    if record["kind"] == "manifest"
                    else record["ref"]
                ),
            }
            for record in manifest["selection"][0]["records"]
        ]
        self.manifest_path.write_bytes(
            (json.dumps(manifest, ensure_ascii=False, indent=2) + "\n").encode()
        )
        manifest_sha = hashlib.sha256(self.manifest_path.read_bytes()).hexdigest()
        fetch_calls: list[str] = []
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "selection must contain one Item record",
        ):
            acquisition.acquire_batch(
                manifest_path=self.manifest_path,
                metadata_root=self.metadata,
                output_root=self.output,
                expected_manifest_sha256=manifest_sha,
                fetcher=lambda payload: fetch_calls.append(payload["file_ref"]) or fetches[payload["file_ref"]],
            )
        self.assertEqual([], fetch_calls)
        self.assertFalse(self.output.exists())

    def test_manifest_rejects_non_source_record_reference(self) -> None:
        _fetches, _manifest_sha = self._write_manifest(count=1)
        manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        record = manifest["selection"][0]["records"][0]
        record["ref"] = "ToS/derived-exports/fixture.json"
        manifest["provenance_delta"]["record_refs"] = sorted(
            value["ref"] for value in manifest["selection"][0]["records"]
        )
        self.manifest_path.write_bytes(
            (json.dumps(manifest, ensure_ascii=False, indent=2) + "\n").encode()
        )
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "outside the corpus source admission boundary",
        ):
            acquisition.load_manifest(self.manifest_path)

    def test_manifest_rejects_conflicting_shared_record_kind(self) -> None:
        _fetches, _manifest_sha = self._write_manifest(count=2)
        manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        shared = dict(manifest["selection"][0]["records"][1])
        shared["kind"] = "provenance"
        manifest["selection"][1]["records"].append(shared)
        self.manifest_path.write_bytes(
            (json.dumps(manifest, ensure_ascii=False, indent=2) + "\n").encode()
        )
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "conflicting kind or digest",
        ):
            acquisition.load_manifest(self.manifest_path)

    def test_prepared_item_record_identity_is_bound_before_fetch(self) -> None:
        _fetches, manifest_sha = self._write_manifest(count=1)
        acquisition.prepare_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.output,
            expected_manifest_sha256=manifest_sha,
        )
        item_record = self.output / (
            "source/ToS/source-witnesses/works/fixture/expressions/en/"
            "editions/pinned/items/fixture-0/item.json"
        )
        item_record.write_bytes(b'{"record_id":"tos.item.other"}\n')
        fetch_calls: list[str] = []
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "Item record identity differs",
        ):
            acquisition.acquire_batch(
                manifest_path=self.manifest_path,
                metadata_root=self.metadata,
                output_root=self.output,
                expected_manifest_sha256=manifest_sha,
                fetcher=lambda payload: fetch_calls.append(payload["file_ref"]) or b"",
            )
        self.assertEqual([], fetch_calls)

    def test_item_record_requires_record_id_before_fetch(self) -> None:
        fetches, _manifest_sha = self._write_manifest(count=1)
        manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        selection = manifest["selection"][0]
        item_record = next(
            record for record in selection["records"] if record["kind"] == "item"
        )
        item_record_path = self.metadata / item_record["ref"]
        item_record_path.write_text(
            json.dumps(
                {
                    "record_type": "item",
                    "item_id": selection["item_ref"],
                    "item_manifest_ref": f"{selection['item_root_ref']}/item.manifest.json",
                }
            )
            + "\n",
            encoding="utf-8",
        )
        item_record["sha256"] = hashlib.sha256(item_record_path.read_bytes()).hexdigest()
        self.manifest_path.write_text(
            json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        manifest_sha = hashlib.sha256(self.manifest_path.read_bytes()).hexdigest()
        fetch_calls: list[str] = []
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "Item record identity differs",
        ):
            acquisition.acquire_batch(
                manifest_path=self.manifest_path,
                metadata_root=self.metadata,
                output_root=self.root / "missing-item-record-id",
                expected_manifest_sha256=manifest_sha,
                fetcher=lambda payload: fetch_calls.append(payload["file_ref"])
                or fetches[payload["file_ref"]],
            )
        self.assertEqual([], fetch_calls)
        self.assertEqual(
            [], list((self.root / "missing-item-record-id").glob("receipts/handoff-*.json"))
        )

    def test_item_manifest_companions_are_selected_and_fixed_before_fetch(self) -> None:
        for missing_companion in (
            "forensic_report_ref",
            "resource_inventory_ref",
            "fixity.sha256",
        ):
            with self.subTest(missing_companion=missing_companion):
                fetches, _manifest_sha = self._write_manifest(count=1)
                manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
                selection = manifest["selection"][0]
                item_root = selection["item_root_ref"]
                item_manifest = json.loads(
                    (self.metadata / item_root / "item.manifest.json").read_text(
                        encoding="utf-8"
                    )
                )
                missing_ref = (
                    f"{item_root}/{missing_companion}"
                    if missing_companion == "fixity.sha256"
                    else item_manifest[missing_companion]
                )
                selection["records"] = [
                    record
                    for record in selection["records"]
                    if record["ref"] != missing_ref
                ]
                manifest["provenance_delta"]["record_refs"] = sorted(
                    record["ref"] for record in selection["records"]
                )
                self.manifest_path.write_text(
                    json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
                    encoding="utf-8",
                )
                manifest_sha = hashlib.sha256(self.manifest_path.read_bytes()).hexdigest()
                output = self.root / f"missing-companion-{missing_companion.replace('.', '-')}"
                fetch_calls: list[str] = []
                expected_error = (
                    "Item fixity companion is not selected"
                    if missing_companion == "fixity.sha256"
                    else "Item manifest companion is not selected"
                )
                with self.assertRaisesRegex(
                    acquisition.AcquisitionBatchError,
                    expected_error,
                ):
                    acquisition.acquire_batch(
                        manifest_path=self.manifest_path,
                        metadata_root=self.metadata,
                        output_root=output,
                        expected_manifest_sha256=manifest_sha,
                        fetcher=lambda payload: fetch_calls.append(payload["file_ref"])
                        or fetches[payload["file_ref"]],
                    )
                self.assertEqual([], fetch_calls)
                self.assertEqual([], list(output.glob("receipts/handoff-*.json")))

        fetches, manifest_sha = self._write_manifest(count=1)
        fixity_path = self.metadata / (
            "ToS/source-witnesses/works/fixture/expressions/en/editions/pinned/"
            "items/fixture-0/fixity.sha256"
        )
        fixity_path.write_text("0" * 64 + "  payload/fixture-0.txt\n", encoding="utf-8")
        manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        selection = manifest["selection"][0]
        fixity_record = next(
            record for record in selection["records"] if record["ref"].endswith("/fixity.sha256")
        )
        fixity_record["sha256"] = hashlib.sha256(fixity_path.read_bytes()).hexdigest()
        self.manifest_path.write_text(
            json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        manifest_sha = hashlib.sha256(self.manifest_path.read_bytes()).hexdigest()
        fetch_calls: list[str] = []
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "Item fixity companion differs from manifest payloads",
        ):
            acquisition.acquire_batch(
                manifest_path=self.manifest_path,
                metadata_root=self.metadata,
                output_root=self.output,
                expected_manifest_sha256=manifest_sha,
                fetcher=lambda payload: fetch_calls.append(payload["file_ref"])
                or fetches[payload["file_ref"]],
            )
        self.assertEqual([], fetch_calls)
        self.assertEqual([], list(self.output.glob("receipts/handoff-*.json")))

        fetches, _manifest_sha = self._write_manifest(count=1)
        manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        selection = manifest["selection"][0]
        inventory_ref = next(
            record["ref"]
            for record in selection["records"]
            if record["ref"].endswith("/resource-inventory.json")
        )
        inventory_path = self.metadata / inventory_ref
        inventory = json.loads(inventory_path.read_text(encoding="utf-8"))
        inventory["files"][0]["file_sha256"] = "0" * 64
        inventory_path.write_text(
            json.dumps(inventory, ensure_ascii=False, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        inventory_record = next(
            record for record in selection["records"] if record["ref"] == inventory_ref
        )
        inventory_record["sha256"] = hashlib.sha256(inventory_path.read_bytes()).hexdigest()
        self.manifest_path.write_text(
            json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        manifest_sha = hashlib.sha256(self.manifest_path.read_bytes()).hexdigest()
        inventory_output = self.root / "inventory-mismatch"
        fetch_calls = []
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "Item resource inventory does not close over manifest payloads",
        ):
            acquisition.acquire_batch(
                manifest_path=self.manifest_path,
                metadata_root=self.metadata,
                output_root=inventory_output,
                expected_manifest_sha256=manifest_sha,
                fetcher=lambda payload: fetch_calls.append(payload["file_ref"])
                or fetches[payload["file_ref"]],
            )
        self.assertEqual([], fetch_calls)
        self.assertEqual([], list(inventory_output.glob("receipts/handoff-*.json")))

    def test_item_inventory_provenance_is_digest_bound_before_fetch(self) -> None:
        for mutation in ("missing_event", "wrong_inventory_digest"):
            with self.subTest(mutation=mutation):
                fetches, _manifest_sha = self._write_manifest(count=1)
                manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
                selection = manifest["selection"][0]
                inventory_record = next(
                    record
                    for record in selection["records"]
                    if record["ref"].endswith("/resource-inventory.json")
                )
                inventory_path = self.metadata / inventory_record["ref"]
                inventory = json.loads(inventory_path.read_text(encoding="utf-8"))
                provenance_record = next(
                    record
                    for record in selection["records"]
                    if record["kind"] == "provenance"
                )
                if mutation == "missing_event":
                    inventory["provenance_event_ref"] = (
                        "tos.event.unselected-inventory-fixture"
                    )
                else:
                    provenance_path = self.metadata / provenance_record["ref"]
                    provenance_rows = [
                        json.loads(line)
                        for line in provenance_path.read_text(encoding="utf-8").splitlines()
                        if line.strip()
                    ]
                    inventory_event = next(
                        row
                        for row in provenance_rows
                        if row["event_id"] == inventory["provenance_event_ref"]
                    )
                    output = next(
                        row
                        for row in inventory_event["outputs"]
                        if row["ref"] == inventory_record["ref"]
                    )
                    output["sha256"] = "0" * 64
                    provenance_path.write_text(
                        "".join(
                            json.dumps(row, sort_keys=True) + "\n"
                            for row in provenance_rows
                        ),
                        encoding="utf-8",
                    )
                    provenance_record["sha256"] = hashlib.sha256(
                        provenance_path.read_bytes()
                    ).hexdigest()
                inventory_path.write_text(
                    json.dumps(inventory, ensure_ascii=False, sort_keys=True) + "\n",
                    encoding="utf-8",
                )
                inventory_record["sha256"] = hashlib.sha256(
                    inventory_path.read_bytes()
                ).hexdigest()
                self.manifest_path.write_text(
                    json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
                    encoding="utf-8",
                )
                manifest_sha = hashlib.sha256(self.manifest_path.read_bytes()).hexdigest()
                fetch_calls: list[str] = []
                output_root = self.root / f"inventory-provenance-{mutation}"
                expected_error = (
                    "Item resource inventory provenance event is missing or ambiguous"
                    if mutation == "missing_event"
                    else "Item resource inventory provenance output is not digest-bound"
                )
                with self.assertRaisesRegex(
                    acquisition.AcquisitionBatchError,
                    expected_error,
                ):
                    acquisition.acquire_batch(
                        manifest_path=self.manifest_path,
                        metadata_root=self.metadata,
                        output_root=output_root,
                        expected_manifest_sha256=manifest_sha,
                        fetcher=lambda payload: fetch_calls.append(payload["file_ref"])
                        or fetches[payload["file_ref"]],
                    )
                self.assertEqual([], fetch_calls)
                self.assertEqual([], list(output_root.glob("receipts/handoff-*.json")))

    def test_item_record_must_bind_selected_manifest_before_fetch(self) -> None:
        _fetches, _manifest_sha = self._write_manifest(count=1)
        manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        selection = manifest["selection"][0]
        item_record = next(record for record in selection["records"] if record["kind"] == "item")
        item_ref = selection["item_ref"]
        item_record_path = self.metadata / item_record["ref"]
        item_record_path.write_bytes(
            json.dumps({"record_type": "item", "record_id": item_ref}).encode()
        )
        item_record["sha256"] = hashlib.sha256(item_record_path.read_bytes()).hexdigest()
        self.manifest_path.write_bytes(
            (json.dumps(manifest, ensure_ascii=False, indent=2) + "\n").encode()
        )
        manifest_sha = hashlib.sha256(self.manifest_path.read_bytes()).hexdigest()
        fetch_calls: list[str] = []

        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "Item record does not bind the selected manifest",
        ):
            acquisition.acquire_batch(
                manifest_path=self.manifest_path,
                metadata_root=self.metadata,
                output_root=self.output,
                expected_manifest_sha256=manifest_sha,
                fetcher=lambda payload: fetch_calls.append(payload["file_ref"])
                or _fetches[payload["file_ref"]],
            )

        self.assertEqual([], fetch_calls)
        self.assertFalse(list((self.output / "receipts").glob("handoff-*.json")))

    def test_selected_rights_mutation_rejects_resume_and_handoff(self) -> None:
        _fetches, manifest_sha = self._write_manifest(count=1)
        acquisition.prepare_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.output,
            expected_manifest_sha256=manifest_sha,
        )
        rights = self.output / (
            "source/ToS/source-witnesses/works/fixture/expressions/en/"
            "editions/pinned/items/fixture-0/rights.json"
        )
        rights.write_bytes(b'{"rights":"changed-after-prepare"}\n')
        fetch_calls: list[str] = []

        def should_not_fetch(payload: dict) -> bytes:
            fetch_calls.append(payload["file_ref"])
            return _fetches[payload["file_ref"]]

        with self.assertRaisesRegex(
            acquisition.SourceIntegrityError,
            "prepared rights bytes differ|selected record digest",
        ):
            acquisition.acquire_batch(
                manifest_path=self.manifest_path,
                metadata_root=self.metadata,
                output_root=self.output,
                expected_manifest_sha256=manifest_sha,
                fetcher=should_not_fetch,
            )
        self.assertEqual([], fetch_calls)
        with self.assertRaisesRegex(
            acquisition.SourceIntegrityError,
            "prepared rights bytes differ|selected record digest",
        ):
            acquisition.verify_local(output_root=self.output)
        self.assertEqual([], list((self.output / "receipts").glob("handoff-*.json")))

    def test_selected_metadata_mutation_before_handoff_is_rejected(self) -> None:
        fetches, manifest_sha = self._write_manifest(count=1)
        original_fixity = acquisition._fixity_receipt
        rights = self.output / (
            "source/ToS/source-witnesses/works/fixture/expressions/en/"
            "editions/pinned/items/fixture-0/rights.json"
        )

        def mutate_after_fixity(*args, **kwargs):
            result = original_fixity(*args, **kwargs)
            rights.write_bytes(b"changed-between-fixity-and-handoff\n")
            return result

        with patch.object(acquisition, "_fixity_receipt", side_effect=mutate_after_fixity):
            with self.assertRaisesRegex(
                acquisition.SourceIntegrityError,
                "prepared rights bytes differ|selected record digest",
            ):
                acquisition.acquire_batch(
                    manifest_path=self.manifest_path,
                    metadata_root=self.metadata,
                    output_root=self.output,
                    expected_manifest_sha256=manifest_sha,
                    fetcher=lambda payload: fetches[payload["file_ref"]],
                )
        self.assertEqual([], list((self.output / "receipts").glob("handoff-*.json")))

    def test_symlinked_acquisition_journal_is_rejected_for_read_and_append(self) -> None:
        _fetches, manifest_sha = self._write_manifest(count=1)
        acquisition.prepare_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.output,
            expected_manifest_sha256=manifest_sha,
        )
        journal = self.output / "receipts/acquisition.jsonl"
        outside = self.root / "outside-journal.jsonl"
        outside.write_text('{"status":"acquired"}\n', encoding="utf-8")
        journal.symlink_to(outside)
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "acquisition journal may not be a symlink",
        ):
            acquisition._journal_rows(journal)
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "acquisition journal may not be a symlink",
        ):
            acquisition._append_journal(journal, {"status": "failed"})
        self.assertEqual('{"status":"acquired"}\n', outside.read_text(encoding="utf-8"))

    @unittest.skipUnless(hasattr(os, "link"), "hard link boundary")
    def test_hardlinked_acquisition_journal_is_rejected_for_read_and_append(self) -> None:
        _fetches, manifest_sha = self._write_manifest(count=1)
        acquisition.prepare_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.output,
            expected_manifest_sha256=manifest_sha,
        )
        journal = self.output / "receipts/acquisition.jsonl"
        outside = self.root / "outside-journal.jsonl"
        outside.write_text('{"status":"acquired"}\n', encoding="utf-8")
        os.link(outside, journal)
        original = outside.read_bytes()

        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "acquisition journal must have one hard link",
        ):
            acquisition._journal_rows(journal)
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "acquisition journal must have one hard link",
        ):
            acquisition._append_journal(journal, {"status": "failed"})

        self.assertEqual(original, outside.read_bytes())

    def test_acquisition_journal_with_unexpected_owner_is_rejected(self) -> None:
        journal = self.root / "unexpected-owner-journal.jsonl"
        journal.write_text('{"status":"acquired"}\n', encoding="utf-8")
        original = journal.read_bytes()
        owner_uid = journal.stat().st_uid

        with patch.object(acquisition.os, "geteuid", return_value=owner_uid + 1):
            with self.assertRaisesRegex(
                acquisition.AcquisitionBatchError,
                "acquisition journal owner differs from current user",
            ):
                acquisition._journal_rows(journal)
            with self.assertRaisesRegex(
                acquisition.AcquisitionBatchError,
                "acquisition journal owner differs from current user",
            ):
                acquisition._append_journal(journal, {"status": "failed"})

        self.assertEqual(original, journal.read_bytes())

    def test_acquisition_journal_rejects_duplicate_json_keys(self) -> None:
        journal = self.root / "duplicate-journal.jsonl"
        journal.write_text(
            '{"file_ref":"tos.file.sha256.fixture",'
            '"status":"failed","status":"verified"}\n',
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "acquisition journal is malformed at line 1",
        ):
            acquisition._journal_rows(journal)

    @unittest.skipUnless(hasattr(os, "mkfifo"), "POSIX FIFO boundary")
    def test_fifo_acquisition_journal_read_and_append_fail_without_blocking(self) -> None:
        journal = self.root / "fifo-journal.jsonl"
        os.mkfifo(journal)
        script = """
import sys
from pathlib import Path
sys.path.insert(0, sys.argv[2])
import acquisition_batch as acquisition
journal = Path(sys.argv[1])
try:
    acquisition._journal_rows(journal)
except acquisition.AcquisitionBatchError:
    pass
else:
    raise SystemExit("FIFO journal read was accepted")
try:
    acquisition._append_journal(journal, {"status": "failed"})
except acquisition.AcquisitionBatchError:
    pass
else:
    raise SystemExit("FIFO journal append was accepted")
"""
        result = subprocess.run(
            [sys.executable, "-c", script, str(journal), str(ROOT / "scripts")],
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=10,
        )
        self.assertEqual(0, result.returncode, result.stderr)

    @unittest.skipUnless(hasattr(os, "mkfifo"), "POSIX FIFO boundary")
    def test_fifo_payload_destination_fails_fixity_without_blocking(self) -> None:
        payload = self.root / "fifo-payload.bin"
        os.mkfifo(payload)
        digest = hashlib.sha256(b"expected").hexdigest()
        script = """
import sys
from pathlib import Path
sys.path.insert(0, sys.argv[2])
import acquisition_batch as acquisition
payload = Path(sys.argv[1])
try:
    acquisition._verify_destination(
        payload,
        {"byte_size": 8, "sha256": sys.argv[3], "file_ref": "tos.file.sha256." + sys.argv[3]},
    )
except acquisition.SourceIntegrityError:
    pass
else:
    raise SystemExit("FIFO payload destination passed fixity")
"""
        result = subprocess.run(
            [sys.executable, "-c", script, str(payload), str(ROOT / "scripts"), digest],
            cwd=ROOT,
            capture_output=True,
            text=True,
            timeout=10,
        )
        self.assertEqual(0, result.returncode, result.stderr)

    @unittest.skipUnless(hasattr(os, "mkfifo"), "POSIX FIFO boundary")
    def test_prepared_source_fifo_is_rejected_before_provider_fetch(self) -> None:
        fetches, manifest_sha = self._write_manifest(count=1)
        output = self.root / "source-fifo"
        acquisition.prepare_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=output,
            expected_manifest_sha256=manifest_sha,
        )
        item_source = (
            output
            / "source/ToS/source-witnesses/works/fixture/expressions/en/"
            "editions/pinned/items/fixture-0"
        )
        os.mkfifo(item_source / "unselected.fifo")
        fetch_calls: list[str] = []
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "prepared source contains a special file",
        ):
            acquisition.acquire_batch(
                manifest_path=self.manifest_path,
                metadata_root=self.metadata,
                output_root=output,
                expected_manifest_sha256=manifest_sha,
                fetcher=lambda payload: fetch_calls.append(payload["file_ref"])
                or fetches[payload["file_ref"]],
            )
        self.assertEqual([], fetch_calls)

    def test_provider_urls_reject_embedded_credentials_before_output_or_fetch(self) -> None:
        cases = (
            (
                "payload-query-token",
                "payload_url",
                "https://provider.example/file?X-Amz-Signature=signed-secret",
            ),
            (
                "payload-userinfo",
                "payload_url",
                "https://operator:secret@provider.example/file",
            ),
            (
                "provider-source-query",
                "source_url",
                "https://provider.example/catalog?access_token=signed-secret",
            ),
        )
        for suffix, target, url in cases:
            with self.subTest(case=suffix):
                fetches, _manifest_sha = self._write_manifest(count=1)
                manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
                selection = manifest["selection"][0]
                if target == "payload_url":
                    selection["payload_files"][0]["provider_url"] = url
                else:
                    selection["provider"]["source_url"] = url
                self.manifest_path.write_text(
                    json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
                    encoding="utf-8",
                )
                manifest_sha = hashlib.sha256(self.manifest_path.read_bytes()).hexdigest()
                output = self.root / f"credential-url-{suffix}"
                fetch_calls: list[str] = []
                with self.assertRaisesRegex(
                    acquisition.AcquisitionBatchError,
                    "must not contain userinfo, a query, or a fragment",
                ) as raised:
                    acquisition.acquire_batch(
                        manifest_path=self.manifest_path,
                        metadata_root=self.metadata,
                        output_root=output,
                        expected_manifest_sha256=manifest_sha,
                        fetcher=lambda payload: fetch_calls.append(payload["file_ref"])
                        or fetches[payload["file_ref"]],
                    )
                self.assertNotIn("signed-secret", str(raised.exception))
                self.assertEqual([], fetch_calls)
                self.assertFalse(output.exists())

    def test_interrupted_prepare_is_rebuilt_before_acquisition(self) -> None:
        fetches, manifest_sha = self._write_manifest(count=1)
        self.output.mkdir()
        (self.output / "manifest.json").write_bytes(self.manifest_path.read_bytes())
        (self.output / "source").mkdir()
        (self.output / "payload").mkdir()
        (self.output / "receipts").mkdir()
        partial = (
            self.output
            / "source/ToS/source-witnesses/works/fixture/expressions/en/"
            "editions/pinned/items/fixture-0/item.json"
        )
        partial.parent.mkdir(parents=True)
        partial.write_bytes(b"interrupted")
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.output,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        self.assertEqual("acquired-not-admitted", result["status"])
        self.assertTrue((self.output / "receipts/preparation.json").is_file())
        self.assertNotEqual(b"interrupted", partial.read_bytes())

    def test_recovery_refuses_unproven_payload_receipt_or_foreign_data(self) -> None:
        _fetches, manifest_sha = self._write_manifest(count=1)
        cases = {
            "payload": "payload/foreign.bin",
            "receipt": "receipts/old-handoff.json",
            "foreign-source": "source/foreign.json",
        }
        for name, relative in cases.items():
            with self.subTest(name=name):
                output = self.root / f"partial-{name}"
                output.mkdir()
                (output / "manifest.json").write_bytes(self.manifest_path.read_bytes())
                for directory in ("source", "payload", "receipts"):
                    (output / directory).mkdir()
                evidence = output / relative
                evidence.parent.mkdir(parents=True, exist_ok=True)
                evidence.write_bytes(b"unowned evidence")
                with self.assertRaisesRegex(
                    acquisition.AcquisitionBatchError,
                    "not a recoverable interrupted preparation",
                ):
                    acquisition.acquire_batch(
                        manifest_path=self.manifest_path,
                        metadata_root=self.metadata,
                        output_root=output,
                        expected_manifest_sha256=manifest_sha,
                        fetcher=lambda payload: _fetches[payload["file_ref"]],
                    )
                self.assertTrue(evidence.exists())

    def test_concurrent_acquire_calls_are_serialized(self) -> None:
        fetches, manifest_sha = self._write_manifest(count=1)
        calls: list[str] = []
        calls_lock = threading.Lock()

        def fetch(payload: dict) -> bytes:
            with calls_lock:
                calls.append(payload["file_ref"])
            time.sleep(0.05)
            return fetches[payload["file_ref"]]

        results: list[dict] = []
        errors: list[BaseException] = []

        def run() -> None:
            try:
                results.append(
                    acquisition.acquire_batch(
                        manifest_path=self.manifest_path,
                        metadata_root=self.metadata,
                        output_root=self.output,
                        expected_manifest_sha256=manifest_sha,
                        fetcher=fetch,
                        max_attempts=1,
                    )
                )
            except BaseException as exc:  # pragma: no cover - assertion below reports it
                errors.append(exc)

        first = threading.Thread(target=run)
        second = threading.Thread(target=run)
        first.start()
        second.start()
        first.join()
        second.join()
        self.assertEqual([], errors)
        self.assertEqual(2, len(results))
        self.assertEqual(1, len(calls))
        self.assertEqual(2, len(list((self.output / "receipts").glob("handoff-*.json"))))

    def test_standalone_prepare_and_acquire_share_batch_lock(self) -> None:
        fetches, manifest_sha = self._write_manifest(count=1)
        entered_copy = threading.Event()
        original_copy = acquisition._copy_metadata_no_clobber
        results: list[dict] = []
        errors: list[BaseException] = []

        def slow_copy(*args, **kwargs):
            entered_copy.set()
            time.sleep(0.1)
            return original_copy(*args, **kwargs)

        def prepare() -> None:
            try:
                results.append(
                    acquisition.prepare_batch(
                        manifest_path=self.manifest_path,
                        metadata_root=self.metadata,
                        output_root=self.output,
                        expected_manifest_sha256=manifest_sha,
                    )
                )
            except BaseException as exc:  # pragma: no cover - assertion below reports it
                errors.append(exc)

        def acquire() -> None:
            try:
                results.append(
                    acquisition.acquire_batch(
                        manifest_path=self.manifest_path,
                        metadata_root=self.metadata,
                        output_root=self.output,
                        expected_manifest_sha256=manifest_sha,
                        fetcher=lambda payload: fetches[payload["file_ref"]],
                    )
                )
            except BaseException as exc:  # pragma: no cover - assertion below reports it
                errors.append(exc)

        with patch.object(acquisition, "_copy_metadata_no_clobber", side_effect=slow_copy):
            prepare_thread = threading.Thread(target=prepare)
            prepare_thread.start()
            self.assertTrue(entered_copy.wait(5))
            acquire_thread = threading.Thread(target=acquire)
            acquire_thread.start()
            prepare_thread.join()
            acquire_thread.join()
        self.assertEqual([], errors)
        self.assertEqual(
            {"prepared-not-acquired", "acquired-not-admitted"},
            {result["status"] for result in results},
        )
        self.assertTrue((self.output / "receipts/preparation.json").is_file())


if __name__ == "__main__":
    unittest.main()
