"""Frozen source metadata fixture for native acquisition-handoff tests."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path
import shutil
import tempfile

ROOT = Path(__file__).resolve().parents[2]

def _canonical(value: object) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n").encode("utf-8")

class AcquisitionHandoffFixture:
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
        self.validator_sha256 = hashlib.sha256(b"tos-native-source-validator-test-fixture-v1").hexdigest()

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
                body = _canonical(manifest_value)
            elif filename == "rights.json":
                rights_value = json.loads((owner_item_root / filename).read_text())
                rights_value["scope_refs"] = (
                    [item_ref, payload_ref] if rights_scope_complete else [item_ref]
                )
                body = _canonical(rights_value)
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
                body = _canonical(
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
            path.chmod(0o644)
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
            destination.chmod(0o644)

        body = {
            "schema_version": "tos_corpus_snapshot_v1",
            "base_revision": None,
            "validator_sha256": self.validator_sha256,
            "files": files,
            "identities": {},
            "dependencies": {},
            "retirements": [],
        }
        revision = hashlib.sha256(_canonical(body)).hexdigest()
        manifest = {**body, "revision": revision}
        snapshot_path = self.accepted_store / "revisions" / revision / "snapshot.json"
        snapshot_path.parent.mkdir(parents=True)
        snapshot_path.write_bytes(_canonical(manifest))
        (self.accepted_store / "current.json").write_bytes(
            _canonical(
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

