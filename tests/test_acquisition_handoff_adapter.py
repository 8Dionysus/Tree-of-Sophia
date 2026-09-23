from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import acquisition_batch as acquisition  # noqa: E402
import acquisition_handoff_adapter as adapter  # noqa: E402
import corpus_admit  # noqa: E402
from corpus_source_validation import SourceValidator  # noqa: E402
from corpus_store import CorpusStore, canonical  # noqa: E402


class AcquisitionHandoffAdapterTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.metadata = self.root / "metadata"
        self.metadata.mkdir()
        self.manifest_path = self.root / "selection.json"
        self.acquisition_root = self.root / "acquisition"
        self.accepted_store = self.root / "accepted-store"
        self.accepted_store.mkdir()
        self.accepted_source = self.root / "accepted" / "source"
        self.accepted_source.mkdir(parents=True)
        self.candidate = self.root / "candidate"
        self.validator_sha256 = SourceValidator(ROOT).sha256

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _validation_context(self, validator_sha256: str | None = None) -> dict[str, object]:
        return {
            "schema_version": "tos_corpus_validation_context_v1",
            "validator_sha256": self.validator_sha256 if validator_sha256 is None else validator_sha256,
            "grammar_root_ref": str(ROOT),
            "historical_evidence": [],
        }

    def _write_manifest(
        self,
        *,
        base_revision: str,
        manifest_payload_matches: bool = True,
        extra_manifest: bool = False,
        provenance_output_ref: str = "destination",
        provenance_output_sha256: str | None = None,
        provenance_rights_ref: str | None = None,
        rights_scope_complete: bool = True,
        provenance_event_type: str = "acquisition",
        provenance_event_status: str | None = None,
    ) -> tuple[dict[str, bytes], str, str, list[dict[str, str]]]:
        item_ref = "tos.item.sid-9a5249d273634cf6b2eb96b5e7719fa8"
        item_root = (
            "ToS/source-witnesses/works/tree-of-sophia/scoped-research-selection/"
            "expressions/english-20260910/editions/repository-82e7e281/"
            "items/acquired-note-utf8-20260910"
        )
        records: list[dict[str, str]] = []
        owner_item_root = ROOT / item_root
        payload_body = b"adapter fixture payload\n"
        payload_sha = hashlib.sha256(payload_body).hexdigest()
        payload_ref = f"tos.file.sha256.{payload_sha}"
        inventory_event_ref = (
            "tos.event.adapter-inventory-"
            + hashlib.sha256(item_ref.encode()).hexdigest()[:12]
        )
        old_manifest = json.loads((owner_item_root / "item.manifest.json").read_text())
        old_payload = old_manifest["payload_files"][0]
        for filename, kind in (
            ("item.json", "item"),
            ("item.manifest.json", "manifest"),
            ("rights.json", "rights"),
            ("provenance.jsonl", "provenance"),
            ("forensic-report.md", "discovery"),
            ("resource-inventory.json", "discovery"),
            ("fixity.sha256", "discovery"),
        ):
            ref = f"{item_root}/{filename}"
            if filename == "item.manifest.json":
                manifest_payload = {
                    **old_payload,
                    "file_id": payload_ref,
                    "relative_path": "payload/adapter.txt",
                    "byte_size": len(payload_body),
                    "sha256": payload_sha,
                    "media_type": "text/plain",
                }
                if not manifest_payload_matches:
                    manifest_payload = {
                        **manifest_payload,
                        "file_id": "tos.file.sha256." + ("0" * 64),
                        "relative_path": "payload/unbound.txt",
                        "byte_size": len(payload_body) + 1,
                        "sha256": "0" * 64,
                    }
                manifest_value = {
                    **old_manifest,
                    "payload_files": [manifest_payload],
                }
                body = canonical(manifest_value)
            elif filename == "rights.json":
                rights_value = json.loads((owner_item_root / filename).read_text())
                rights_value["scope_refs"] = (
                    [item_ref, payload_ref] if rights_scope_complete else [item_ref]
                )
                body = canonical(rights_value)
            elif filename == "provenance.jsonl":
                old_sha = old_payload["sha256"]
                old_file_ref = old_payload["file_id"]
                old_destination = f"{item_root}/{old_payload['relative_path']}"
                old_rights_ref = f"{item_root}/rights.json"
                output_ref = {
                    "destination": f"{item_root}/payload/adapter.txt",
                    "file": payload_ref,
                }.get(provenance_output_ref, provenance_output_ref)
                output_sha256 = (
                    payload_sha
                    if provenance_output_sha256 is None
                    else provenance_output_sha256
                )
                body = (owner_item_root / filename).read_bytes()
                body = (
                    body.replace(old_sha.encode(), output_sha256.encode())
                    .replace(old_file_ref.encode(), payload_ref.encode())
                    .replace(old_destination.encode(), output_ref.encode())
                )
                if provenance_rights_ref is not None:
                    if provenance_rights_ref == "wrong":
                        provenance_rights_ref = f"{item_root}/other-rights.json"
                    body = body.replace(
                        old_rights_ref.encode(), provenance_rights_ref.encode()
                    )
                if provenance_event_type != "acquisition" or provenance_event_status is not None:
                    event_rows = [
                        json.loads(line)
                        for line in body.decode().splitlines()
                        if line.strip()
                    ]
                    if provenance_event_type != "acquisition":
                        event_rows[0]["event_type"] = provenance_event_type
                    if provenance_event_status is not None:
                        event_rows[0]["status"] = provenance_event_status
                    body = b"".join(
                        (json.dumps(row, sort_keys=True) + "\n").encode()
                        for row in event_rows
                    )
            elif filename == "resource-inventory.json":
                body = canonical(
                    {
                        "$schema": "https://tree-of-sophia.local/ToS/contracts/source-resource-inventory.schema.json",
                        "schema_version": "tos_source_resource_inventory_v1",
                        "item_id": item_ref,
                        "generated_from_manifest_ref": f"{item_root}/item.manifest.json",
                        "inventory_authority": "mechanical_metadata_only",
                        "source_text_included": False,
                        "files": [
                            {
                                "file_id": payload_ref,
                                "file_sha256": payload_sha,
                                "media_type": "text/plain",
                                "profile": "plain_text_v1",
                                "summary": {"resource_count": 1},
                                "resources": [
                                    {
                                        "resource_id": "adapter-fixture-resource",
                                        "resource_kind": "plain_text_file",
                                        "locator": {"container_order": 1},
                                        "structural_role": "member",
                                        "content_fingerprint": {
                                            "algorithm": "sha256",
                                            "normalization": "unicode-codepoints-preserved",
                                            "sha256": payload_sha,
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
                    }
                )
            elif filename == "fixity.sha256":
                body = f"{payload_sha}  payload/adapter.txt\n".encode()
            elif filename == "forensic-report.md":
                body = b"Fixture forensic report; no interpretation was accepted.\n"
            else:
                body = (owner_item_root / filename).read_bytes()
            path = self.metadata / ref
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(body)
            records.append(
                {"ref": ref, "kind": kind, "sha256": hashlib.sha256(body).hexdigest()}
            )
        inventory_ref = f"{item_root}/resource-inventory.json"
        inventory_path = self.metadata / inventory_ref
        inventory_event = {
            "schema_version": "tos_provenance_event_v1",
            "event_id": inventory_event_ref,
            "event_type": "forensic_inspection",
            "started_at": "2026-09-22T12:00:00Z",
            "ended_at": "2026-09-22T12:00:00Z",
            "agent_refs": ["software:tos-source-item-commands"],
            "inputs": [],
            "outputs": [
                {
                    "ref": inventory_ref,
                    "role": "tracked_text_free_resource_inventory",
                    "sha256": hashlib.sha256(inventory_path.read_bytes()).hexdigest(),
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
        provenance_ref = f"{item_root}/provenance.jsonl"
        provenance_path = self.metadata / provenance_ref
        with provenance_path.open("ab") as stream:
            stream.write((json.dumps(inventory_event, sort_keys=True) + "\n").encode())
        provenance_record = next(record for record in records if record["ref"] == provenance_ref)
        provenance_record["sha256"] = hashlib.sha256(provenance_path.read_bytes()).hexdigest()
        if extra_manifest:
            extra_ref = f"{item_root}/batch-scope.json"
            extra_body = b'{"scope":"fixture-batch"}\n'
            extra_path = self.metadata / extra_ref
            extra_path.parent.mkdir(parents=True, exist_ok=True)
            extra_path.write_bytes(extra_body)
            records.append(
                {
                    "ref": extra_ref,
                    "kind": "manifest",
                    "sha256": hashlib.sha256(extra_body).hexdigest(),
                }
            )
        manifest = {
            "$schema": "https://tree-of-sophia.local/ToS/contracts/acquisition-batch.schema.json",
            "schema_version": "tos_acquisition_batch_v1",
            "batch_id": "tos.acquisition-batch.adapter-fixture-20260922",
            "batch_revision": 1,
            "prepared_at": "2026-09-22T12:00:00Z",
            "base_revision": base_revision,
            "selection": [
                {
                    "item_ref": item_ref,
                    "item_root_ref": item_root,
                    "provider": {
                        "name": "fixture-provider",
                        "revision": "r1",
                        "source_id": "adapter-fixture-v1",
                        "source_url": "https://provider.example/adapter-fixture-v1",
                    },
                    "records": records,
                    "rights": {
                        "ref": f"{item_root}/rights.json",
                        "sha256": records[2]["sha256"],
                        "posture": "local_only",
                    },
                    "payload_files": [
                        {
                            "item_ref": item_ref,
                            "file_ref": payload_ref,
                            "item_root_ref": item_root,
                            "relative_path": "payload/adapter.txt",
                            "byte_size": len(payload_body),
                            "sha256": payload_sha,
                            "provider_url": "https://provider.example/adapter/r1/adapter.txt",
                            "provider_revision": "r1",
                            "provider_source_id": "adapter-fixture-v1",
                            "media_type": "text/plain",
                        }
                    ],
                }
            ],
            "provenance_delta": {
                "event_ref": "tos.event.acquisition-batch.adapter-fixture-20260922",
                "event_version": 1,
                "change_kind": "batch_delta",
                "base_revision": base_revision,
                "record_refs": sorted(row["ref"] for row in records),
                "payload_file_refs": [payload_ref],
                "supersedes_event_ref": None,
            },
        }
        self.manifest_path.write_bytes(
            (json.dumps(manifest, ensure_ascii=False, indent=2) + "\n").encode()
        )
        return (
            {payload_ref: payload_body},
            hashlib.sha256(self.manifest_path.read_bytes()).hexdigest(),
            item_root,
            records,
        )

    def _add_second_item_reusing_file(self) -> str:
        manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        first = manifest["selection"][0]
        first_item_ref = first["item_ref"]
        first_item_root = first["item_root_ref"]
        first_manifest = json.loads(
            (self.metadata / first_item_root / "item.manifest.json").read_text(
                encoding="utf-8"
            )
        )
        first_event_ref = first_manifest["acquisition_event_ref"]
        first_inventory_ref = f"{first_item_root}/resource-inventory.json"
        first_inventory = json.loads(
            (self.metadata / first_inventory_ref).read_text(encoding="utf-8")
        )
        first_inventory_event_ref = first_inventory["provenance_event_ref"]
        first_provenance_ref = f"{first_item_root}/provenance.jsonl"
        first_provenance_rows = [
            json.loads(line)
            for line in (self.metadata / first_provenance_ref)
            .read_text(encoding="utf-8")
            .splitlines()
            if line.strip()
        ]
        second_item_ref = "tos.item.shared-file-second"
        second_item_root = (
            f"{first_item_root.rsplit('/', 1)[0]}/acquired-note-shared-file-test"
        )
        second_event_ref = "tos.event.acquisition.shared-file-second"
        second_inventory_event_ref = "tos.event.inventory.shared-file-second"
        second_event_ids = {}
        for row in first_provenance_rows:
            old_event_id = row["event_id"]
            if old_event_id == first_event_ref:
                new_event_id = second_event_ref
            elif old_event_id == first_inventory_event_ref:
                new_event_id = second_inventory_event_ref
            else:
                suffix = hashlib.sha256(old_event_id.encode()).hexdigest()[:16]
                new_event_id = f"tos.event.shared-file-second.{suffix}"
            second_event_ids[old_event_id] = new_event_id
        second = json.loads(json.dumps(first))
        second["item_ref"] = second_item_ref
        second["item_root_ref"] = second_item_root
        second_records: list[dict[str, str]] = []
        for record in first["records"]:
            new_ref = record["ref"].replace(first_item_root, second_item_root, 1)
            body = (self.metadata / record["ref"]).read_bytes()
            for before, after in (
                (first_item_root.encode(), second_item_root.encode()),
                (first_item_ref.encode(), second_item_ref.encode()),
            ):
                body = body.replace(before, after)
            for old_event_id, new_event_id in sorted(
                second_event_ids.items(), key=lambda item: len(item[0]), reverse=True
            ):
                body = body.replace(old_event_id.encode(), new_event_id.encode())
            destination = self.metadata / new_ref
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(body)
            second_records.append(
                {
                    "ref": new_ref,
                    "kind": record["kind"],
                    "sha256": hashlib.sha256(body).hexdigest(),
                }
            )
        second_inventory_ref = f"{second_item_root}/resource-inventory.json"
        second_inventory_path = self.metadata / second_inventory_ref
        second_inventory_sha = hashlib.sha256(second_inventory_path.read_bytes()).hexdigest()
        second_provenance_ref = f"{second_item_root}/provenance.jsonl"
        second_provenance_path = self.metadata / second_provenance_ref
        second_provenance_rows = [
            json.loads(line)
            for line in second_provenance_path.read_text(encoding="utf-8").splitlines()
            if line.strip()
        ]
        inventory_event = next(
            row
            for row in second_provenance_rows
            if row["event_id"] == second_inventory_event_ref
        )
        inventory_output = next(
            row
            for row in inventory_event["outputs"]
            if row["ref"] == second_inventory_ref
        )
        inventory_output["sha256"] = second_inventory_sha
        second_provenance_path.write_text(
            "".join(
                json.dumps(row, sort_keys=True) + "\n"
                for row in second_provenance_rows
            ),
            encoding="utf-8",
        )
        next(row for row in second_records if row["ref"] == second_provenance_ref)["sha256"] = (
            hashlib.sha256(second_provenance_path.read_bytes()).hexdigest()
        )
        second["records"] = second_records
        second["rights"]["ref"] = f"{second_item_root}/rights.json"
        second["rights"]["sha256"] = next(
            row["sha256"] for row in second_records if row["kind"] == "rights"
        )
        second["payload_files"] = [
            {
                **first["payload_files"][0],
                "item_ref": second_item_ref,
                "item_root_ref": second_item_root,
            }
        ]
        manifest["selection"].append(second)
        manifest["provenance_delta"]["record_refs"] = sorted(
            row["ref"] for selection in manifest["selection"] for row in selection["records"]
        )
        manifest["provenance_delta"]["payload_file_refs"] = sorted(
            {
                payload["file_ref"]
                for selection in manifest["selection"]
                for payload in selection["payload_files"]
            }
        )
        self.manifest_path.write_text(
            json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        return hashlib.sha256(self.manifest_path.read_bytes()).hexdigest()

    def _write_accepted_base(self, records: list[dict[str, str]]) -> str:
        """Create a tiny cryptographically valid accepted CorpusStore base."""

        files = []
        for record in sorted(records, key=lambda row: row["ref"]):
            source = self.metadata / record["ref"]
            files.append(
                {
                    "path": record["ref"],
                    "sha256": record["sha256"],
                    "size_bytes": source.stat().st_size,
                    "mode": 0o644,
                }
            )
            destination = self.accepted_source / record["ref"]
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)

        body = {
            "schema_version": "tos_corpus_snapshot_v1",
            "base_revision": None,
            "validator_sha256": self.validator_sha256,
            "files": files,
            "identities": {},
            "dependencies": {},
            "retirements": [],
        }
        revision = hashlib.sha256(canonical(body)).hexdigest()
        manifest = {**body, "revision": revision}
        snapshot_path = self.accepted_store / "revisions" / revision / "snapshot.json"
        snapshot_path.parent.mkdir(parents=True)
        snapshot_path.write_bytes(canonical(manifest))
        (self.accepted_store / "current.json").write_bytes(
            canonical(
                {
                    "schema_version": "tos_corpus_pointer_v1",
                    "current": revision,
                    "previous": None,
                }
            )
        )
        objects = self.accepted_store / "objects"
        objects.mkdir(exist_ok=True)
        for record in files:
            shutil.copyfile(self.metadata / record["path"], objects / record["sha256"])
        return revision

    def test_producer_to_consumer_fixture_emits_valid_batch_input(self) -> None:
        # The accepted base is a real, cryptographically bound CorpusStore
        # snapshot.  Its selected source view is populated from current ToS
        # owner records, while admission remains a no-op replay against that
        # exact base rather than a semantic acceptance claim.
        fetches, _unused_manifest_sha, _item_root, records = self._write_manifest(base_revision="0" * 64)
        base_revision = self._write_accepted_base(records)
        fetches, manifest_sha, _item_root, _records = self._write_manifest(base_revision=base_revision)
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        adapted = adapter.adapt_handoff(
            acquisition_root=self.acquisition_root,
            handoff_ref=result["handoff_ref"],
            expected_manifest_sha256=manifest_sha,
            output_root=self.candidate,
            accepted_store_root=self.accepted_store,
            accepted_source_root=self.accepted_source,
            base_revision=base_revision,
            validator_sha256=self.validator_sha256,
            validation_context=self._validation_context(),
            repo_root=ROOT,
        )
        self.assertEqual("candidate-not-admitted", adapted["status"])
        self.assertEqual("not-run-transport-only", adapted["admission_preflight"])
        batch_path = self.candidate / adapted["candidate_batch_ref"]
        batch, updates, retirements = corpus_admit.read_batch(
            batch_path, self.candidate / "source"
        )
        self.assertEqual("tos_corpus_batch_v1", batch["schema_version"])
        self.assertEqual(7, len(updates))
        self.assertEqual({}, retirements)
        self.assertEqual(base_revision, batch["base_revision"])
        self.assertEqual("not-admitted", json.loads(
            (self.candidate / "receipts/acquisition-handoff-adapter.json").read_text()
        )["admission_status"])
        adapter_receipt = json.loads(
            (self.candidate / "receipts/acquisition-handoff-adapter.json").read_text()
        )
        self.assertEqual(
            manifest_sha, adapter_receipt["caller_expected_manifest_sha256"]
        )
        self.assertEqual(
            "transport-bound; downstream-source-validator-required",
            adapter_receipt["validation_context_posture"],
        )
        handoff = json.loads(
            (self.acquisition_root / result["handoff_ref"]).read_text()
        )
        evidence = adapter_receipt["evidence"]
        self.assertEqual(evidence["handoff.json"]["ref"], adapter_receipt["handoff_ref"])
        self.assertEqual(evidence["handoff.json"]["source_ref"], adapter_receipt["handoff_source_ref"])
        self.assertEqual(
            evidence["provenance-delta.json"]["ref"],
            adapter_receipt["provenance_delta_ref"],
        )
        self.assertEqual(
            evidence["fixity.jsonl"]["ref"],
            adapter_receipt["fixity_ref"],
        )
        self.assertEqual(
            evidence["fixity-summary.json"]["ref"],
            adapter_receipt["fixity_summary_ref"],
        )
        for name, expected_source in {
            "manifest.json": self.acquisition_root / "manifest.json",
            "handoff.json": self.acquisition_root / result["handoff_ref"],
            "provenance-delta.json": self.acquisition_root / handoff["provenance_delta"]["ref"],
            "fixity.jsonl": self.acquisition_root / handoff["independent_fixity"]["ref"],
            "fixity-summary.json": self.acquisition_root / handoff["independent_fixity"]["summary_ref"],
        }.items():
            candidate_evidence = self.candidate / evidence[name]["ref"]
            self.assertEqual(expected_source.read_bytes(), candidate_evidence.read_bytes())
            self.assertEqual(
                evidence[name]["sha256"],
                hashlib.sha256(candidate_evidence.read_bytes()).hexdigest(),
            )
            self.assertEqual(
                expected_source.relative_to(self.acquisition_root).as_posix(),
                evidence[name]["source_ref"],
            )
        context_path = self.candidate / adapter_receipt["validation_context_ref"]
        self.assertEqual(
            adapter_receipt["validation_context_sha256"],
            hashlib.sha256(context_path.read_bytes()).hexdigest(),
        )
        self.assertEqual(
            adapter_receipt["validator_sha256"],
            json.loads(context_path.read_text())["validator_sha256"],
        )
        self.assertTrue(
            (self.candidate / "payload/works/tree-of-sophia/scoped-research-selection/expressions/english-20260910/editions/repository-82e7e281/items/acquired-note-utf8-20260910/payload/adapter.txt").is_file()
        )

        admitted_root = self.root / "admitted-store"
        shutil.copytree(self.accepted_store, admitted_root)
        receipt = corpus_admit.admit_batch(
            admitted_root,
            batch_path,
            self.candidate / "source",
            ROOT,
            payload_source_root=self.candidate / "payload",
        )
        self.assertEqual(base_revision, receipt["revision"])
        self.assertEqual(self.validator_sha256, receipt["validator_sha256"])
        admitted = CorpusStore(admitted_root)
        self.assertEqual(base_revision, admitted.current())
        self.assertEqual(
            admitted.load(base_revision, verify_objects=True),
            CorpusStore(self.accepted_store).load(base_revision),
        )
        replay = corpus_admit.admit_batch(
            admitted_root,
            batch_path,
            self.candidate / "source",
            ROOT,
            payload_source_root=self.candidate / "payload",
        )
        self.assertEqual(receipt, replay)

    def test_adapter_refuses_to_replace_output_created_during_staging(self) -> None:
        fetches, _unused_manifest_sha, _item_root, records = self._write_manifest(
            base_revision="0" * 64
        )
        base_revision = self._write_accepted_base(records)
        fetches, manifest_sha, _item_root, _records = self._write_manifest(
            base_revision=base_revision
        )
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )

        original_copy = adapter._copy_no_clobber
        raced_output_created = False

        def create_empty_output_after_preflight(
            source: Path, destination: Path, *, sha256: str, byte_size: int
        ) -> None:
            nonlocal raced_output_created
            original_copy(source, destination, sha256=sha256, byte_size=byte_size)
            if not raced_output_created:
                # adapt_handoff already checked that this destination was new.
                self.candidate.mkdir()
                raced_output_created = True

        with patch.object(
            adapter,
            "_copy_no_clobber",
            side_effect=create_empty_output_after_preflight,
        ):
            with self.assertRaisesRegex(
                adapter.HandoffAdapterError,
                "cannot publish adapter output without replacement",
            ):
                adapter.adapt_handoff(
                    acquisition_root=self.acquisition_root,
                    handoff_ref=result["handoff_ref"],
                    expected_manifest_sha256=manifest_sha,
                    output_root=self.candidate,
                    accepted_store_root=self.accepted_store,
                    accepted_source_root=self.accepted_source,
                    base_revision=base_revision,
                    validator_sha256=self.validator_sha256,
                    validation_context=self._validation_context(),
                    repo_root=ROOT,
                )

        self.assertTrue(raced_output_created)
        self.assertTrue(self.candidate.is_dir())
        self.assertEqual([], list(self.candidate.iterdir()))
        self.assertEqual([], list(self.root.glob(".candidate.adapter-*")))

    def test_intake_rejects_coherently_replaced_handoff_against_caller_digest(self) -> None:
        _original_fetches, caller_selected_sha, _item_root, _records = self._write_manifest(
            base_revision="a" * 64
        )
        replacement_fetches, replacement_sha, _item_root, _records = self._write_manifest(
            base_revision="a" * 64,
            extra_manifest=True,
        )
        self.assertNotEqual(caller_selected_sha, replacement_sha)
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=replacement_sha,
            fetcher=lambda payload: replacement_fetches[payload["file_ref"]],
        )

        # The replacement handoff is internally coherent: the producer sealed
        # it from its replacement manifest, metadata, payload, and receipts.
        verified_replacement = adapter.verify_handoff_for_intake(
            acquisition_root=self.acquisition_root,
            handoff_ref=result["handoff_ref"],
            expected_manifest_sha256=replacement_sha,
            expected_base_revision="a" * 64,
            repo_root=ROOT,
        )
        self.assertEqual(replacement_sha, verified_replacement.context.manifest_sha256)

        # A consumer retaining the original selection digest must reject that
        # self-consistent replacement before accepting any source evidence.
        with self.assertRaisesRegex(
            adapter.HandoffAdapterError, "caller-selected digest"
        ):
            adapter.verify_handoff_for_intake(
                acquisition_root=self.acquisition_root,
                handoff_ref=result["handoff_ref"],
                expected_manifest_sha256=caller_selected_sha,
                expected_base_revision="a" * 64,
                repo_root=ROOT,
            )

    def test_shared_intake_verifier_accepts_original_handoff_without_store_load(self) -> None:
        fetches, manifest_sha, _item_root, _records = self._write_manifest(base_revision="a" * 64)
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        verified = adapter.verify_handoff_for_intake(
            acquisition_root=self.acquisition_root,
            handoff_ref=result["handoff_ref"],
            expected_manifest_sha256=manifest_sha,
            expected_base_revision="a" * 64,
            repo_root=ROOT,
        )
        context = adapter.verify_validation_context(
            self._validation_context(),
            validator_sha256=self.validator_sha256,
        )
        self.assertEqual("acquired-not-admitted", verified.handoff["acquisition_status"])
        self.assertEqual(7, len(verified.selected_source_rows))
        self.assertEqual(1, len(verified.payloads))
        self.assertEqual(self.validator_sha256, context["validator_sha256"])
        actual_uid = os.geteuid()
        with patch.object(acquisition.os, "geteuid", return_value=actual_uid + 1):
            portable = adapter.verify_handoff_for_intake(
                acquisition_root=self.acquisition_root,
                handoff_ref=result["handoff_ref"],
                expected_manifest_sha256=manifest_sha,
                expected_base_revision="a" * 64,
                repo_root=ROOT,
            )
        self.assertEqual(manifest_sha, portable.context.manifest_sha256)

    def test_shared_file_across_items_preserves_custody_binding_and_rejects_loss(self) -> None:
        fetches, _manifest_sha, _item_root, _records = self._write_manifest(
            base_revision="a" * 64
        )
        manifest_sha = self._add_second_item_reusing_file()
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        verified = adapter.verify_handoff_for_intake(
            acquisition_root=self.acquisition_root,
            handoff_ref=result["handoff_ref"],
            expected_manifest_sha256=manifest_sha,
            expected_base_revision="a" * 64,
            repo_root=ROOT,
        )
        self.assertEqual(2, len(verified.payloads))
        self.assertEqual(
            {"tos.item.sid-9a5249d273634cf6b2eb96b5e7719fa8", "tos.item.shared-file-second"},
            {payload["item_ref"] for payload in verified.payloads},
        )
        self.assertEqual(14, len(verified.selected_source_rows))
        self.assertEqual(
            {"tos.item.sid-9a5249d273634cf6b2eb96b5e7719fa8", "tos.item.shared-file-second"},
            {
                row["item_ref"]
                for row in verified.selected_source_rows
                if row["kind"] == "rights"
            },
        )
        handoff_path = self.acquisition_root / result["handoff_ref"]
        original_handoff = handoff_path.read_bytes()

        # A row for only one destination cannot claim complete custody for the
        # two Item bindings, even though both carry the same content File ID.
        handoff = json.loads(original_handoff)
        handoff["payload_custody"].pop()
        handoff_path.chmod(0o644)
        handoff_path.write_text(
            json.dumps(handoff, ensure_ascii=False, sort_keys=True) + "\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(
            adapter.HandoffAdapterError,
            "handoff payload custody closure differs from manifest",
        ):
            adapter.verify_handoff_for_intake(
                acquisition_root=self.acquisition_root,
                handoff_ref=result["handoff_ref"],
                expected_manifest_sha256=manifest_sha,
                expected_base_revision="a" * 64,
                repo_root=ROOT,
            )

        handoff_path.write_bytes(original_handoff)
        handoff_path.chmod(0o444)
        second_payload = next(
            payload
            for payload in verified.payloads
            if payload["item_ref"] == "tos.item.shared-file-second"
        )
        item_suffix = Path(*second_payload["item_root_ref"].split("/")[2:])
        (self.acquisition_root / "payload" / item_suffix / second_payload["relative_path"]).unlink()
        with self.assertRaisesRegex(adapter.HandoffAdapterError, "payload custody differs"):
            adapter.verify_handoff_for_intake(
                acquisition_root=self.acquisition_root,
                handoff_ref=result["handoff_ref"],
                expected_manifest_sha256=manifest_sha,
                expected_base_revision="a" * 64,
                repo_root=ROOT,
            )

    def test_shared_verifier_accepts_file_id_provenance_and_extra_manifest(self) -> None:
        fetches, manifest_sha, _item_root, records = self._write_manifest(
            base_revision="a" * 64,
            extra_manifest=True,
            provenance_output_ref="file",
        )
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        verified = adapter.verify_handoff_for_intake(
            acquisition_root=self.acquisition_root,
            handoff_ref=result["handoff_ref"],
            expected_manifest_sha256=manifest_sha,
            expected_base_revision="a" * 64,
            repo_root=ROOT,
        )
        self.assertEqual(len(records), len(verified.selected_source_rows))
        self.assertEqual(1, len(verified.payloads))

    def test_handoff_routes_translate_payload_symlink_errors(self) -> None:
        fetches, _old_sha, _item_root, records = self._write_manifest(
            base_revision="0" * 64
        )
        base_revision = self._write_accepted_base(records)
        fetches, manifest_sha, item_root, _records = self._write_manifest(
            base_revision=base_revision
        )
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        payload_ref, payload_bytes = next(iter(fetches.items()))
        payload_path = (
            self.acquisition_root
            / "payload"
            / Path(*item_root.split("/")[2:])
            / "payload/adapter.txt"
        )
        replacement = self.root / "replacement-payload.txt"
        replacement.write_bytes(payload_bytes)

        original_item_binding_check = adapter._verify_item_payload_bindings

        def replace_after_item_binding(context: object, root: Path) -> None:
            original_item_binding_check(context, root)
            payload_path.unlink()
            payload_path.symlink_to(replacement)

        with patch.object(
            adapter,
            "_verify_item_payload_bindings",
            side_effect=replace_after_item_binding,
        ):
            with self.assertRaisesRegex(
                adapter.HandoffAdapterError, "payload custody differs"
            ):
                adapter.verify_handoff_for_intake(
                    acquisition_root=self.acquisition_root,
                    handoff_ref=result["handoff_ref"],
                    expected_manifest_sha256=manifest_sha,
                    expected_base_revision=base_revision,
                    repo_root=ROOT,
                )

        payload_path.unlink()
        payload_path.write_bytes(payload_bytes)
        payload_path.chmod(0o444)
        original_handoff_verifier = adapter._verify_handoff

        def replace_after_handoff_verification(**kwargs: object) -> object:
            verified = original_handoff_verifier(**kwargs)
            payload_path.unlink()
            payload_path.symlink_to(replacement)
            return verified

        with patch.object(
            adapter,
            "_verify_handoff",
            side_effect=replace_after_handoff_verification,
        ):
            with self.assertRaisesRegex(
                adapter.HandoffAdapterError,
                f"cannot materialize payload custody: {payload_ref}",
            ):
                adapter.adapt_handoff(
                    acquisition_root=self.acquisition_root,
                    handoff_ref=result["handoff_ref"],
                    expected_manifest_sha256=manifest_sha,
                    output_root=self.candidate,
                    accepted_store_root=self.accepted_store,
                    accepted_source_root=self.accepted_source,
                    base_revision=base_revision,
                    validator_sha256=self.validator_sha256,
                    validation_context=self._validation_context(),
                    repo_root=ROOT,
                )
        self.assertFalse(self.candidate.exists())

    def test_shared_verifier_rejects_wrong_file_id_digest_and_rights(self) -> None:
        controls = (
            {
                "name": "file-id",
                "kwargs": {"provenance_output_ref": "tos.file.sha256." + "0" * 64},
            },
            {
                "name": "digest",
                "kwargs": {"provenance_output_sha256": "0" * 64},
            },
            {
                "name": "rights",
                "kwargs": {"provenance_rights_ref": "wrong"},
            },
        )
        for control in controls:
            with self.subTest(control=control["name"]):
                fetches, manifest_sha, _item_root, _records = self._write_manifest(
                    base_revision="a" * 64,
                    **control["kwargs"],
                )
                with self.assertRaisesRegex(
                    acquisition.AcquisitionBatchError,
                    "Item provenance does not bind acquired payload|Item provenance does not name the selected acquisition event",
                ):
                    acquisition.acquire_batch(
                        manifest_path=self.manifest_path,
                        metadata_root=self.metadata,
                        output_root=self.acquisition_root,
                        expected_manifest_sha256=manifest_sha,
                        fetcher=lambda payload: fetches[payload["file_ref"]],
                        repo_root=ROOT,
                    )
                self.assertEqual([], list(self.acquisition_root.glob("receipts/handoff-*.json")))
                shutil.rmtree(self.acquisition_root)

    def test_shared_verifier_rejects_provider_custody_mutation(self) -> None:
        fetches, manifest_sha, _item_root, _records = self._write_manifest(
            base_revision="a" * 64,
        )
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        handoff_path = self.acquisition_root / result["handoff_ref"]
        handoff = json.loads(handoff_path.read_text())
        handoff["payload_custody"][0]["provider_revision"] = "unreviewed-provider"
        handoff_path.write_bytes(canonical(handoff))
        with self.assertRaisesRegex(
            adapter.HandoffAdapterError,
            "handoff payload digest binding differs",
        ):
            adapter.verify_handoff_for_intake(
                acquisition_root=self.acquisition_root,
                handoff_ref=result["handoff_ref"],
                expected_manifest_sha256=manifest_sha,
                expected_base_revision="a" * 64,
                repo_root=ROOT,
            )

    def test_shared_verifier_rejects_provenance_delta_base_mutation(self) -> None:
        fetches, manifest_sha, _item_root, _records = self._write_manifest(
            base_revision="a" * 64,
        )
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        handoff_path = self.acquisition_root / result["handoff_ref"]
        handoff = json.loads(handoff_path.read_text())
        handoff["provenance_delta"]["base_revision"] = "b" * 64
        handoff_path.write_bytes(canonical(handoff))
        with self.assertRaisesRegex(
            adapter.HandoffAdapterError,
            "provenance delta base differs",
        ):
            adapter.verify_handoff_for_intake(
                acquisition_root=self.acquisition_root,
                handoff_ref=result["handoff_ref"],
                expected_manifest_sha256=manifest_sha,
                expected_base_revision="a" * 64,
                repo_root=ROOT,
            )

    def test_shared_verifier_rejects_fixity_run_mixing(self) -> None:
        fetches, manifest_sha, _item_root, _records = self._write_manifest(
            base_revision="a" * 64,
        )
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        handoff_path = self.acquisition_root / result["handoff_ref"]
        handoff = json.loads(handoff_path.read_text())
        summary_path = self.acquisition_root / handoff["independent_fixity"]["summary_ref"]
        summary = json.loads(summary_path.read_text())
        summary["run_id"] = "20260922T000000Z"
        summary_path.write_bytes(canonical(summary))
        handoff["independent_fixity"]["summary_sha256"] = hashlib.sha256(
            summary_path.read_bytes()
        ).hexdigest()
        handoff_path.write_bytes(canonical(handoff))
        with self.assertRaisesRegex(
            adapter.HandoffAdapterError,
            "fixity summary does not bind complete handoff",
        ):
            adapter.verify_handoff_for_intake(
                acquisition_root=self.acquisition_root,
                handoff_ref=result["handoff_ref"],
                expected_manifest_sha256=manifest_sha,
                expected_base_revision="a" * 64,
                repo_root=ROOT,
            )

    def test_shared_verifier_rejects_unsupported_fixity_summary_schema(self) -> None:
        fetches, manifest_sha, _item_root, _records = self._write_manifest(
            base_revision="a" * 64,
        )
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        handoff_path = self.acquisition_root / result["handoff_ref"]
        handoff = json.loads(handoff_path.read_text())
        summary_path = self.acquisition_root / handoff["independent_fixity"]["summary_ref"]
        original_summary = json.loads(summary_path.read_text())
        for schema_version in (None, "tos_acquisition_independent_fixity_v0"):
            with self.subTest(schema_version=schema_version):
                summary = dict(original_summary)
                if schema_version is None:
                    summary.pop("schema_version", None)
                else:
                    summary["schema_version"] = schema_version
                summary_path.write_bytes(canonical(summary))
                handoff["independent_fixity"]["summary_sha256"] = hashlib.sha256(
                    summary_path.read_bytes()
                ).hexdigest()
                handoff_path.write_bytes(canonical(handoff))
                with self.assertRaisesRegex(
                    adapter.HandoffAdapterError,
                    "fixity summary schema is unsupported",
                ):
                    adapter.verify_handoff_for_intake(
                        acquisition_root=self.acquisition_root,
                        handoff_ref=result["handoff_ref"],
                        expected_manifest_sha256=manifest_sha,
                        expected_base_revision="a" * 64,
                        repo_root=ROOT,
                    )

    def test_shared_verifier_rejects_duplicate_json_keys_in_fixity_rows(self) -> None:
        fetches, manifest_sha, _item_root, _records = self._write_manifest(
            base_revision="a" * 64,
        )
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        handoff_path = self.acquisition_root / result["handoff_ref"]
        handoff = json.loads(handoff_path.read_text())
        fixity_path = self.acquisition_root / handoff["independent_fixity"]["ref"]
        fixity_body = fixity_path.read_text(encoding="utf-8")
        self.assertIn('"status": "verified"', fixity_body)
        fixity_path.write_text(
            fixity_body.replace(
                '"status": "verified"',
                '"status": "failed", "status": "verified"',
                1,
            ),
            encoding="utf-8",
        )
        fixity_sha = hashlib.sha256(fixity_path.read_bytes()).hexdigest()
        summary_path = self.acquisition_root / handoff["independent_fixity"]["summary_ref"]
        summary = json.loads(summary_path.read_text())
        summary["fixity_jsonl_sha256"] = fixity_sha
        summary_path.write_bytes(canonical(summary))
        summary_sha = hashlib.sha256(summary_path.read_bytes()).hexdigest()
        handoff["independent_fixity"].update(
            sha256=fixity_sha,
            jsonl_sha256=fixity_sha,
            summary_sha256=summary_sha,
        )
        handoff_path.write_bytes(canonical(handoff))
        with self.assertRaisesRegex(
            adapter.HandoffAdapterError,
            "fixity JSONL is malformed: fixity JSONL is malformed at line 1",
        ):
            adapter.verify_handoff_for_intake(
                acquisition_root=self.acquisition_root,
                handoff_ref=result["handoff_ref"],
                expected_manifest_sha256=manifest_sha,
                expected_base_revision="a" * 64,
                repo_root=ROOT,
            )

    def test_shared_verifier_rejects_rights_scope_and_event_identity(self) -> None:
        controls = ("rights", "event", "status")
        for control in controls:
            with self.subTest(control=control):
                fetches, manifest_sha, _item_root, _records = self._write_manifest(
                    base_revision="a" * 64,
                    rights_scope_complete=control != "rights",
                    provenance_event_type="acquisition"
                    if control != "event"
                    else "forensic_inspection",
                    provenance_event_status="failed" if control == "status" else None,
                )
                if control == "rights":
                    expected = "Item rights scope does not cover"
                else:
                    expected = "selected acquisition event"
                fetch_calls: list[str] = []
                with self.assertRaisesRegex(acquisition.AcquisitionBatchError, expected):
                    acquisition.acquire_batch(
                        manifest_path=self.manifest_path,
                        metadata_root=self.metadata,
                        output_root=self.acquisition_root,
                        expected_manifest_sha256=manifest_sha,
                        fetcher=lambda payload: fetch_calls.append(payload["file_ref"]) or fetches[payload["file_ref"]],
                    )
                self.assertEqual([], fetch_calls)
                self.assertEqual([], list(self.acquisition_root.glob("receipts/handoff-*.json")))
                shutil.rmtree(self.acquisition_root)

    def test_shared_verifier_rejects_fixity_git_blob_digest(self) -> None:
        fetches, manifest_sha, _item_root, _records = self._write_manifest(
            base_revision="a" * 64,
        )
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        handoff_path = self.acquisition_root / result["handoff_ref"]
        handoff = json.loads(handoff_path.read_text())
        fixity_path = self.acquisition_root / handoff["independent_fixity"]["ref"]
        row = json.loads(fixity_path.read_text().splitlines()[0])
        row["git_blob_sha1"] = "0" * 40
        fixity_path.write_text(json.dumps(row, sort_keys=True) + "\n")
        fixity_sha = hashlib.sha256(fixity_path.read_bytes()).hexdigest()
        summary_path = self.acquisition_root / handoff["independent_fixity"]["summary_ref"]
        summary = json.loads(summary_path.read_text())
        summary["fixity_jsonl_sha256"] = fixity_sha
        summary_path.write_bytes(canonical(summary))
        summary_sha = hashlib.sha256(summary_path.read_bytes()).hexdigest()
        handoff["independent_fixity"].update(
            sha256=fixity_sha,
            jsonl_sha256=fixity_sha,
            summary_sha256=summary_sha,
        )
        handoff_path.write_bytes(canonical(handoff))
        with self.assertRaisesRegex(
            adapter.HandoffAdapterError,
            "fixity Git blob digest differs",
        ):
            adapter.verify_handoff_for_intake(
                acquisition_root=self.acquisition_root,
                handoff_ref=result["handoff_ref"],
                expected_manifest_sha256=manifest_sha,
                expected_base_revision="a" * 64,
                repo_root=ROOT,
            )

    def test_shared_verifier_rejects_fixity_relative_path_mutation(self) -> None:
        fetches, manifest_sha, _item_root, _records = self._write_manifest(
            base_revision="a" * 64,
        )
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        handoff_path = self.acquisition_root / result["handoff_ref"]
        handoff = json.loads(handoff_path.read_text())
        fixity_path = self.acquisition_root / handoff["independent_fixity"]["ref"]
        row = json.loads(fixity_path.read_text().splitlines()[0])
        row["relative_path"] = "payload/unbound.txt"
        fixity_path.write_text(json.dumps(row, sort_keys=True) + "\n")
        fixity_sha = hashlib.sha256(fixity_path.read_bytes()).hexdigest()
        summary_path = self.acquisition_root / handoff["independent_fixity"]["summary_ref"]
        summary = json.loads(summary_path.read_text())
        summary["fixity_jsonl_sha256"] = fixity_sha
        summary_path.write_bytes(canonical(summary))
        summary_sha = hashlib.sha256(summary_path.read_bytes()).hexdigest()
        handoff["independent_fixity"].update(
            sha256=fixity_sha,
            jsonl_sha256=fixity_sha,
            summary_sha256=summary_sha,
        )
        handoff_path.write_bytes(canonical(handoff))
        with self.assertRaisesRegex(
            adapter.HandoffAdapterError,
            "fixity row binding differs",
        ):
            adapter.verify_handoff_for_intake(
                acquisition_root=self.acquisition_root,
                handoff_ref=result["handoff_ref"],
                expected_manifest_sha256=manifest_sha,
                expected_base_revision="a" * 64,
                repo_root=ROOT,
            )

    def test_shared_verifier_rejects_non_item_manifest_target(self) -> None:
        fetches, manifest_sha, item_root, _records = self._write_manifest(
            base_revision="a" * 64,
            extra_manifest=True,
        )
        acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        context = acquisition.load_manifest(
            self.manifest_path,
            repo_root=ROOT,
            expected_sha256=manifest_sha,
        )
        context.manifest["selection"][0]["records"] = [
            record
            for record in context.manifest["selection"][0]["records"]
            if record["ref"] != f"{item_root}/item.manifest.json"
        ]
        with self.assertRaisesRegex(
            adapter.HandoffAdapterError,
            "selection has no selected Item manifest record",
        ):
            adapter._verify_item_payload_bindings(context, self.acquisition_root)

    def test_shared_verifier_rejects_item_without_manifest_binding(self) -> None:
        _fetches, _manifest_sha, _item_root, _records = self._write_manifest(
            base_revision="a" * 64,
        )
        manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        selection = manifest["selection"][0]
        item_record = next(record for record in selection["records"] if record["kind"] == "item")
        item_path = self.metadata / item_record["ref"]
        item_value = json.loads(item_path.read_text(encoding="utf-8"))
        item_value.pop("item_manifest_ref")
        item_path.write_bytes(canonical(item_value))
        item_record["sha256"] = hashlib.sha256(item_path.read_bytes()).hexdigest()
        self.manifest_path.write_bytes(canonical(manifest))
        manifest_sha = hashlib.sha256(self.manifest_path.read_bytes()).hexdigest()

        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "Item record does not bind the selected manifest",
        ):
            acquisition.prepare_batch(
                manifest_path=self.manifest_path,
                metadata_root=self.metadata,
                output_root=self.acquisition_root,
                expected_manifest_sha256=manifest_sha,
            )

        context = acquisition.load_manifest(
            self.manifest_path,
            repo_root=ROOT,
            expected_sha256=manifest_sha,
        )
        with self.assertRaisesRegex(
            adapter.HandoffAdapterError,
            "Item record does not bind the selected manifest",
        ):
            adapter._verify_item_payload_bindings(context, self.acquisition_root)

    def test_item_provenance_binding_uses_manifest_ref_after_record_reordering(self) -> None:
        fetches, _unused_manifest_sha, item_root, records = self._write_manifest(base_revision="0" * 64)
        base_revision = self._write_accepted_base(records)
        fetches, _manifest_sha, item_root, _records = self._write_manifest(base_revision=base_revision)

        secondary_ref = f"{item_root}/provenance-secondary.jsonl"
        secondary_body = b'{"kind":"unrelated-evidence"}\n'
        secondary_path = self.metadata / secondary_ref
        secondary_path.write_bytes(secondary_body)
        secondary_record = {
            "ref": secondary_ref,
            "kind": "provenance",
            "sha256": hashlib.sha256(secondary_body).hexdigest(),
        }
        manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        selection = manifest["selection"][0]
        selection["records"].insert(0, secondary_record)
        manifest["provenance_delta"]["record_refs"] = sorted(
            record["ref"] for record in selection["records"]
        )
        self.manifest_path.write_bytes(canonical(manifest))
        manifest_sha = hashlib.sha256(self.manifest_path.read_bytes()).hexdigest()

        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        adapted = adapter.adapt_handoff(
            acquisition_root=self.acquisition_root,
            handoff_ref=result["handoff_ref"],
            expected_manifest_sha256=manifest_sha,
            output_root=self.candidate,
            accepted_store_root=self.accepted_store,
            accepted_source_root=self.accepted_source,
            base_revision=base_revision,
            validator_sha256=self.validator_sha256,
            validation_context=self._validation_context(),
            repo_root=ROOT,
        )
        self.assertEqual("candidate-not-admitted", adapted["status"])
        self.assertTrue(
            (self.candidate / "source" / secondary_ref).is_file()
        )

    def test_adapter_rejects_different_accepted_base_bytes(self) -> None:
        fetches, _unused_manifest_sha, item_root, records = self._write_manifest(base_revision="0" * 64)
        base_revision = self._write_accepted_base(records)
        fetches, manifest_sha, item_root, _records = self._write_manifest(base_revision=base_revision)
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        accepted = self.accepted_source / f"{item_root}/rights.json"
        accepted.parent.mkdir(parents=True, exist_ok=True)
        accepted.write_bytes(b"different accepted bytes\n")
        with self.assertRaisesRegex(adapter.HandoffAdapterError, "accepted source view differs"):
            adapter.adapt_handoff(
                acquisition_root=self.acquisition_root,
                handoff_ref=result["handoff_ref"],
                expected_manifest_sha256=manifest_sha,
                output_root=self.candidate,
                accepted_store_root=self.accepted_store,
                accepted_source_root=self.accepted_source,
                base_revision=base_revision,
                validator_sha256=self.validator_sha256,
                validation_context=self._validation_context(),
                repo_root=ROOT,
            )
        self.assertFalse(self.candidate.exists())

    def test_adapter_rejects_accepted_mode_change(self) -> None:
        fetches, _unused_manifest_sha, _item_root, records = self._write_manifest(
            base_revision="0" * 64
        )
        original_revision = self._write_accepted_base(records)
        original_snapshot = json.loads(
            (
                self.accepted_store
                / "revisions"
                / original_revision
                / "snapshot.json"
            ).read_text()
        )
        snapshot_body = {
            key: value for key, value in original_snapshot.items() if key != "revision"
        }
        snapshot_body["files"][0]["mode"] = 0o755
        new_revision = hashlib.sha256(canonical(snapshot_body)).hexdigest()
        new_snapshot = {**snapshot_body, "revision": new_revision}
        new_snapshot_path = self.accepted_store / "revisions" / new_revision / "snapshot.json"
        new_snapshot_path.parent.mkdir(parents=True)
        new_snapshot_path.write_bytes(canonical(new_snapshot))
        (self.accepted_store / "current.json").write_bytes(
            canonical(
                {
                    "schema_version": "tos_corpus_pointer_v1",
                    "current": new_revision,
                    "previous": None,
                }
            )
        )
        fetches, manifest_sha, _item_root, _records = self._write_manifest(
            base_revision=new_revision
        )
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        with self.assertRaisesRegex(
            adapter.HandoffAdapterError,
            "accepted source mode is unsupported",
        ):
            adapter.adapt_handoff(
                acquisition_root=self.acquisition_root,
                handoff_ref=result["handoff_ref"],
                expected_manifest_sha256=manifest_sha,
                output_root=self.candidate,
                accepted_store_root=self.accepted_store,
                accepted_source_root=self.accepted_source,
                base_revision=new_revision,
                validator_sha256=self.validator_sha256,
                validation_context=self._validation_context(),
                repo_root=ROOT,
            )
        self.assertFalse(self.candidate.exists())

    def test_adapter_rejects_unsupported_mode_for_new_source_record(self) -> None:
        _fetches, _unused_manifest_sha, _item_root, _records = self._write_manifest(
            base_revision="0" * 64
        )
        base_revision = self._write_accepted_base([])
        fetches, manifest_sha, _item_root, records = self._write_manifest(
            base_revision=base_revision
        )
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        selected_source = self.acquisition_root / "source" / records[0]["ref"]
        selected_source.chmod(0o600)

        with self.assertRaisesRegex(
            adapter.HandoffAdapterError,
            "selected source mode is unsupported for candidate update",
        ):
            adapter.adapt_handoff(
                acquisition_root=self.acquisition_root,
                handoff_ref=result["handoff_ref"],
                expected_manifest_sha256=manifest_sha,
                output_root=self.candidate,
                accepted_store_root=self.accepted_store,
                accepted_source_root=self.accepted_source,
                base_revision=base_revision,
                validator_sha256=self.validator_sha256,
                validation_context=self._validation_context(),
                repo_root=ROOT,
            )
        self.assertFalse(self.candidate.exists())

    def test_adapter_rejects_unbound_empty_accepted_source_view(self) -> None:
        fetches, _unused_manifest_sha, _item_root, records = self._write_manifest(base_revision="0" * 64)
        base_revision = self._write_accepted_base(records)
        fetches, manifest_sha, _item_root, _records = self._write_manifest(base_revision=base_revision)
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        empty_view = self.root / "empty-accepted" / "source"
        empty_view.mkdir(parents=True)
        with self.assertRaisesRegex(adapter.HandoffAdapterError, "missing selected base member"):
            adapter.adapt_handoff(
                acquisition_root=self.acquisition_root,
                handoff_ref=result["handoff_ref"],
                expected_manifest_sha256=manifest_sha,
                output_root=self.candidate,
                accepted_store_root=self.accepted_store,
                accepted_source_root=empty_view,
                base_revision=base_revision,
                validator_sha256=self.validator_sha256,
                validation_context=self._validation_context(),
                repo_root=ROOT,
            )
        self.assertFalse(self.candidate.exists())

    def test_adapter_rejects_accepted_validator_drift_before_candidate(self) -> None:
        fetches, _unused_manifest_sha, _item_root, records = self._write_manifest(base_revision="0" * 64)
        base_revision = self._write_accepted_base(records)
        fetches, manifest_sha, _item_root, _records = self._write_manifest(base_revision=base_revision)
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        adapted = adapter.adapt_handoff(
            acquisition_root=self.acquisition_root,
            handoff_ref=result["handoff_ref"],
            expected_manifest_sha256=manifest_sha,
            output_root=self.candidate,
            accepted_store_root=self.accepted_store,
            accepted_source_root=self.accepted_source,
            base_revision=base_revision,
            validator_sha256="0" * 64,
            validation_context=self._validation_context("0" * 64),
            repo_root=ROOT,
        )
        self.assertEqual("explicit-grammar-update-required", adapted["admission_preflight"])
        receipt = json.loads(
            (self.candidate / "receipts/acquisition-handoff-adapter.json").read_text()
        )
        self.assertEqual("explicit-grammar-update-required", receipt["validator_transition"])
        self.assertEqual("0" * 64, receipt["validator_sha256"])

    def test_adapter_rejects_item_payload_manifest_mismatch(self) -> None:
        fetches, _unused_manifest_sha, _item_root, records = self._write_manifest(
            base_revision="0" * 64,
        )
        base_revision = self._write_accepted_base(records)
        fetches, manifest_sha, _item_root, _records = self._write_manifest(
            base_revision=base_revision,
            manifest_payload_matches=False,
        )
        fetch_calls: list[str] = []
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "Item manifest payload closure differs",
        ):
            acquisition.acquire_batch(
                manifest_path=self.manifest_path,
                metadata_root=self.metadata,
                output_root=self.acquisition_root,
                expected_manifest_sha256=manifest_sha,
                fetcher=lambda payload: fetch_calls.append(payload["file_ref"]) or fetches[payload["file_ref"]],
            )
        self.assertEqual([], fetch_calls)
        self.assertEqual([], list(self.acquisition_root.glob("receipts/handoff-*.json")))
        self.assertFalse(self.candidate.exists())

    def test_item_manifest_rejects_unselected_provenance_ref(self) -> None:
        fetches, _unused_manifest_sha, item_root, records = self._write_manifest(
            base_revision="0" * 64,
        )
        base_revision = self._write_accepted_base(records)
        fetches, _manifest_sha, item_root, _records = self._write_manifest(
            base_revision=base_revision,
        )
        item_manifest_ref = f"{item_root}/item.manifest.json"
        item_manifest_path = self.metadata / item_manifest_ref
        item_manifest = json.loads(item_manifest_path.read_text(encoding="utf-8"))
        item_manifest["provenance_ref"] = f"{item_root}/not-selected.jsonl"
        item_manifest_path.write_bytes(canonical(item_manifest))
        manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        manifest_record = next(
            record
            for record in manifest["selection"][0]["records"]
            if record["ref"] == item_manifest_ref
        )
        manifest_record["sha256"] = hashlib.sha256(item_manifest_path.read_bytes()).hexdigest()
        self.manifest_path.write_bytes(canonical(manifest))
        manifest_sha = hashlib.sha256(self.manifest_path.read_bytes()).hexdigest()
        fetch_calls: list[str] = []
        with self.assertRaisesRegex(
            acquisition.AcquisitionBatchError,
            "Item manifest identity or rights/provenance binding differs",
        ):
            acquisition.acquire_batch(
                manifest_path=self.manifest_path,
                metadata_root=self.metadata,
                output_root=self.acquisition_root,
                expected_manifest_sha256=manifest_sha,
                fetcher=lambda payload: fetch_calls.append(payload["file_ref"]) or fetches[payload["file_ref"]],
            )
        self.assertEqual([], fetch_calls)
        self.assertEqual([], list(self.acquisition_root.glob("receipts/handoff-*.json")))

    def test_adapter_requires_explicit_validation_context(self) -> None:
        fetches, _unused_manifest_sha, _item_root, records = self._write_manifest(
            base_revision="0" * 64,
        )
        base_revision = self._write_accepted_base(records)
        fetches, manifest_sha, _item_root, _records = self._write_manifest(
            base_revision=base_revision,
        )
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        with self.assertRaisesRegex(adapter.HandoffAdapterError, "explicit validation context"):
            adapter.adapt_handoff(
                acquisition_root=self.acquisition_root,
                handoff_ref=result["handoff_ref"],
                expected_manifest_sha256=manifest_sha,
                output_root=self.candidate,
                accepted_store_root=self.accepted_store,
                accepted_source_root=self.accepted_source,
                base_revision=base_revision,
                validator_sha256=self.validator_sha256,
                validation_context=None,
                repo_root=ROOT,
            )
        self.assertFalse(self.candidate.exists())


if __name__ == "__main__":
    unittest.main()
