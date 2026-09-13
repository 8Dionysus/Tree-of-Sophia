from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import source_payload_import as importer


class MemoryTransport:
    """A byte-exact transport double, with no implicit overwrite semantics."""

    def __init__(self) -> None:
        self.objects: dict[str, bytes] = {}
        self.put_calls: list[str] = []
        self.put_sources: list[Path] = []
        self.fetch_calls: list[str] = []

    def fetch(self, object_key: str, destination: Path) -> bool:
        self.fetch_calls.append(object_key)
        if object_key not in self.objects:
            return False
        destination.write_bytes(self.objects[object_key])
        return True

    def put(
        self,
        object_key: str,
        source: Path,
        *,
        byte_size: int,
        media_type: str,
        storage_class: str,
    ) -> None:
        self.put_calls.append(object_key)
        self.put_sources.append(source)
        if object_key in self.objects:
            raise RuntimeError("the test transport refuses an overwrite")
        self.objects[object_key] = source.read_bytes()


class SourcePayloadImportTests(unittest.TestCase):
    def setUp(self) -> None:
        self._temporary = tempfile.TemporaryDirectory()
        self.root = Path(self._temporary.name)
        self.repo = self.root / "repo"
        (self.repo / "ToS/contracts").mkdir(parents=True)
        for name in (
            "server-import-contract.schema.json",
            "server-import-receipt.schema.json",
            "source-payload-rights-review.schema.json",
        ):
            shutil.copy2(REPO_ROOT / "ToS/contracts" / name, self.repo / "ToS/contracts" / name)

        self.item_ref = "tos.item.fixture.khuddakapatha"
        self.item_root = "ToS/source-witnesses/works/fixture/items/khuddakapatha"
        self.manifest_ref = f"{self.item_root}/item.manifest.json"
        self.rights_ref = f"{self.item_root}/rights.json"
        self.review_ref = "ToS/source-witnesses/server-import/reviews/fixture-rights-review.json"
        self.evidence_ref = "ToS/source-witnesses/server-import/reviews/fixture-license.txt"
        self.revocation_ref = "ToS/source-witnesses/server-import/reviews/fixture-revocation.md"
        for ref in (self.manifest_ref, self.rights_ref, self.review_ref, self.evidence_ref, self.revocation_ref):
            (self.repo / ref).parent.mkdir(parents=True, exist_ok=True)
        (self.repo / self.evidence_ref).write_text("CC0 evidence for the fixture\n", encoding="utf-8")
        (self.repo / self.revocation_ref).write_text("Check the licensor source before every transfer.\n", encoding="utf-8")

        self.payload_root = self.root / "payload-root"
        self.payload = self.payload_root / "works/fixture/items/khuddakapatha/payload/witness.txt"
        self.payload.parent.mkdir(parents=True)
        self.body = b"exact source witness\n"
        self.payload.write_bytes(self.body)
        self.file_sha = hashlib.sha256(self.body).hexdigest()
        self.file_ref = f"tos.file.sha256.{self.file_sha}"
        manifest = {
            "item_id": self.item_ref,
            "rights_ref": self.rights_ref,
            "payload_files": [
                {
                    "file_id": self.file_ref,
                    "relative_path": "payload/witness.txt",
                    "byte_size": len(self.body),
                    "sha256": self.file_sha,
                    "media_type": "text/plain",
                }
            ],
        }
        self._write_json(self.repo / self.manifest_ref, manifest)
        rights = {
            "item_id": self.item_ref,
            "scope_refs": [self.item_ref, self.file_ref],
            "assessment_status": "licensed",
            "permissions": ["Acquire and retain the exact files.", "Process the licensed digital layer."],
            "restrictions": [],
            "visibility": "controlled",
            "redistribution_posture": "authorized",
            "server_processing_posture": "authorized",
            "source": "https://example.test/cc0",
        }
        self._write_json(self.repo / self.rights_ref, rights)
        self.manifest_sha = importer.sha256_file(self.repo / self.manifest_ref)
        self.rights_sha = importer.sha256_file(self.repo / self.rights_ref)
        self.evidence_sha = importer.sha256_file(self.repo / self.evidence_ref)
        self.plan_path = self.repo / "ToS/source-witnesses/server-import/plans/fixture.json"
        self.plan_path.parent.mkdir(parents=True, exist_ok=True)
        self.plan = self._make_plan()
        self._write_review_and_plan()
        self.context = importer.load_plan(self.repo, self.plan_path)
        self.transport = MemoryTransport()
        self.receipt_dir = self.root / "receipts"

    def tearDown(self) -> None:
        self._temporary.cleanup()

    @staticmethod
    def _write_json(path: Path, value: dict) -> None:
        path.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n", encoding="utf-8")

    def _make_plan(self) -> dict:
        derivatives = {
            name: {"state": "prohibited", "conditions": ["exact-source-only fixture"]}
            for name in (
                "ocr",
                "transcription",
                "page_images",
                "snippets",
                "lexical_index",
                "embeddings",
                "alignments",
                "translations",
                "annotations",
                "search_projection",
                "graph_projection",
            )
        }
        return {
            "$schema": "https://tree-of-sophia.local/ToS/contracts/server-import-contract.schema.json",
            "schema_version": "tos_server_import_contract_v1",
            "server_import_id": "tos.server-import.fixture.khuddakapatha.20260913",
            "item_ref": self.item_ref,
            "manifest": {"ref": self.manifest_ref, "sha256": self.manifest_sha, "verified": True},
            "payload_files": [
                {
                    "file_ref": self.file_ref,
                    "relative_path": "payload/witness.txt",
                    "byte_size": len(self.body),
                    "sha256": self.file_sha,
                    "verified": True,
                }
            ],
            "rights_policy": {
                "rights_record_ref": self.rights_ref,
                "rights_record_sha256": self.rights_sha,
                "assessment_status": "open-licensed",
                "review_status": "agent-reviewed",
                "jurisdictions_reviewed": ["MX"],
                "permission_or_license_refs": [
                    "https://example.test/cc0",
                    self.evidence_ref,
                ],
                "expires_at": None,
                "recheck_before_transfer": True,
            },
            "access_class": "controlled-research",
            "allowed_derivatives": derivatives,
            "payload_transfer_authorized": True,
            "operator_transfer_approval": {
                "approved": True,
                "approved_by_real_human": True,
                "approved_at": "2026-09-13T00:00:00Z",
                "approval_ref": "ToS/source-witnesses/server-import/reviews/operator-approval.md",
            },
            "server_import_status": "approved-not-uploaded",
            "publication_status": "not-published",
            "server_receipt_refs": [],
            "takedown": {
                "public_contact_url": "https://example.test/contact",
                "procedure_ref": "ToS/source-witnesses/server-import/SERVER_IMPORT_PROTOCOL.md",
                "disable_supported": True,
                "delete_supported": True,
                "last_reviewed_at": "2026-09-13T00:00:00Z",
            },
            "provenance_event_refs": ["tos.event.server-import.fixture.20260913"],
            "server_copy_is_authority": False,
            "automatic_checkout_payload_discovery_allowed": False,
            "contract_version": 3,
        }

    def _write_review_and_plan(self) -> None:
        review = {
            "$schema": "https://tree-of-sophia.local/ToS/contracts/source-payload-rights-review.schema.json",
            "schema_version": "tos_source_payload_rights_review_v1",
            "review_id": "tos.review.fixture.khuddakapatha.20260913",
            "actor": {"kind": "model", "actor_ref": "model:test", "model": "test-model"},
            "reviewed_at": "2026-09-13T00:00:00Z",
            "scope": {
                "item_ref": self.item_ref,
                "file_refs": [self.file_ref],
                "manifest_ref": self.manifest_ref,
                "manifest_sha256": self.manifest_sha,
                "rights_record_ref": self.rights_ref,
                "rights_record_sha256": self.rights_sha,
            },
            "license_evidence": [
                {
                    "source_ref": "https://example.test/cc0",
                    "evidence_ref": self.evidence_ref,
                    "evidence_sha256": self.evidence_sha,
                    "kind": "primary-license",
                    "scope": "the exact fixture file and its source edition",
                }
            ],
            "decision": {
                "access_class": "controlled-research",
                "raw_cloud_retention": "controlled-research",
                "server_processing": "authorized",
                "publication": "not-published",
                "conditions": ["raw source stays private; no derivatives"],
                "expires_at": None,
                "revocation_check_ref": self.revocation_ref,
            },
        }
        review_path = self.repo / self.review_ref
        self._write_json(review_path, review)
        self.review_sha = importer.sha256_file(review_path)
        self.plan["rights_policy"]["rights_review"] = {"ref": self.review_ref, "sha256": self.review_sha}
        self._write_json(self.plan_path, self.plan)

    def _refresh_rights_bindings(self, rights: dict) -> None:
        self._write_json(self.repo / self.rights_ref, rights)
        self.rights_sha = importer.sha256_file(self.repo / self.rights_ref)
        review_path = self.repo / self.review_ref
        review = json.loads(review_path.read_text(encoding="utf-8"))
        review["scope"]["rights_record_sha256"] = self.rights_sha
        self._write_json(review_path, review)
        self.review_sha = importer.sha256_file(review_path)
        self.plan["rights_policy"]["rights_record_sha256"] = self.rights_sha
        self.plan["rights_policy"]["rights_review"]["sha256"] = self.review_sha
        self._write_json(self.plan_path, self.plan)
        self.context = importer.load_plan(self.repo, self.plan_path)

    def _import_one(self) -> importer.ImportOutcome:
        return importer.import_plan(
            self.context,
            payload_source_root=self.payload_root,
            transport=self.transport,
            receipt_dir=self.receipt_dir,
            bucket_alias="tos-source-payloads",
            file_ref=self.file_ref,
            scratch_root=self.root / "managed-scratch",
        )[0]

    def test_import_is_exact_and_repeat_reuses_revision_bound_receipt(self) -> None:
        first = self._import_one()
        self.assertEqual("uploaded", first.remote_status)
        self.assertTrue(first.upload_attempted)
        self.assertFalse(first.receipt_reused)
        self.assertEqual(1, len(self.transport.put_calls))
        receipt = importer.load_receipt(first.receipt_path, repo_root=self.repo)
        self.assertEqual(self.review_ref, receipt["identity"]["rights_review_ref"])
        self.assertEqual(self.review_sha, receipt["identity"]["rights_review_sha256"])
        self.assertNotIn(str(self.root), first.receipt_path.read_text(encoding="utf-8"))
        self.assertIn(self.context.plan_sha256[:16], first.receipt_path.name)

        second = self._import_one()
        self.assertEqual("already-matched", second.remote_status)
        self.assertFalse(second.upload_attempted)
        self.assertTrue(second.receipt_reused)
        self.assertEqual(first.receipt_path, second.receipt_path)
        self.assertEqual(1, len(self.transport.put_calls))

    def test_plan_file_selection_cannot_bypass_extra_manifest_inventory(self) -> None:
        extra = dict(self.context.plan["payload_files"][0])
        extra["file_ref"] = "tos.file.sha256." + "1" * 64
        extra["relative_path"] = "payload/second.txt"
        extra["byte_size"] = 1
        extra["sha256"] = "1" * 64
        self.context.plan["payload_files"].append(extra)
        with self.assertRaises(importer.PlanError):
            importer.verify_local(
                self.context,
                payload_source_root=self.payload_root,
                file_ref=self.file_ref,
            )

    def test_transport_receives_verified_snapshot_after_original_changes(self) -> None:
        original_fetch = self.transport.fetch

        def mutate_after_snapshot(object_key: str, destination: Path) -> bool:
            self.payload.write_bytes(b"changed after snapshot\n")
            return original_fetch(object_key, destination)

        self.transport.fetch = mutate_after_snapshot  # type: ignore[method-assign]
        outcome = self._import_one()
        self.assertEqual("uploaded", outcome.remote_status)
        receipt = importer.load_receipt(outcome.receipt_path, repo_root=self.repo)
        self.assertEqual(self.body, self.transport.objects[receipt["storage"]["object_key"]])
        self.assertNotEqual(self.payload, self.transport.put_sources[0])
        self.assertFalse(self.transport.put_sources[0].exists())

    def test_receipt_publication_race_reuses_same_binding(self) -> None:
        original_write = importer.write_json

        def competing_writer(path: Path, value: dict, *, immutable: bool = False) -> None:
            original_write(path, value, immutable=immutable)
            raise importer.ReceiptConflict("simulated concurrent timestamp race")

        with patch.object(importer, "write_json", side_effect=competing_writer):
            outcome = self._import_one()
        self.assertTrue(outcome.receipt_reused)
        self.assertTrue(outcome.receipt_path.is_file())

    def test_forged_review_digest_is_rejected_before_transfer(self) -> None:
        review_path = self.repo / self.review_ref
        review_path.write_text(review_path.read_text(encoding="utf-8") + "tampered\n", encoding="utf-8")
        with self.assertRaises(importer.PlanError):
            importer.load_plan(self.repo, self.plan_path)

    def test_changed_rights_bytes_are_rejected_even_with_loaded_plan(self) -> None:
        (self.repo / self.rights_ref).write_text("changed license record\n", encoding="utf-8")
        with self.assertRaises(importer.PlanError):
            self._import_one()
        self.assertEqual([], self.transport.put_calls)

    def test_wrong_local_bytes_are_rejected_without_remote_write(self) -> None:
        self.payload.write_bytes(b"wrong bytes\n")
        with self.assertRaises(importer.LocalIntegrityError):
            self._import_one()
        self.assertEqual([], self.transport.put_calls)

    def test_symlinked_payload_root_is_rejected(self) -> None:
        alias = self.root / "payload-alias"
        alias.symlink_to(self.payload_root, target_is_directory=True)
        with self.assertRaises(importer.LocalIntegrityError):
            importer.verify_local(
                self.context,
                payload_source_root=alias,
                file_ref=self.file_ref,
            )

    def test_existing_wrong_remote_bytes_fail_closed(self) -> None:
        self.transport.objects[
            f"blobs/sha256/{self.file_sha[:2]}/{self.file_sha}"
        ] = b"wrong remote bytes"
        with self.assertRaises(importer.RemoteIntegrityError):
            self._import_one()
        self.assertEqual([], self.transport.put_calls)

    def test_registry_disable_and_revoke_block_local_read(self) -> None:
        outcome = self._import_one()
        receipt = importer.load_receipt(outcome.receipt_path, repo_root=self.repo)
        registry = self.root / "publication-registry.json"
        importer.enable_publication(receipt, registry_path=registry, mode="controlled-research")
        output = self.root / "read-back.txt"
        importer.read_local(
            self.context,
            receipt,
            registry_path=registry,
            payload_source_root=self.payload_root,
            output=output,
        )
        self.assertEqual(self.body, output.read_bytes())
        with self.assertRaises(importer.PublicationError):
            importer.read_local(
                self.context,
                receipt,
                registry_path=registry,
                payload_source_root=self.payload_root,
                output=output,
            )
        importer.disable_publication(
            registry_path=registry,
            publication=receipt["publication"]["publication_id"],
            reason="fixture revocation",
            revoked=True,
        )
        with self.assertRaises(importer.PublicationError):
            importer.read_local(
                self.context,
                receipt,
                registry_path=registry,
                payload_source_root=self.payload_root,
                output=self.root / "revoked.txt",
            )

    def test_digest_consistent_restricted_rights_record_still_blocks_transfer(self) -> None:
        rights = json.loads((self.repo / self.rights_ref).read_text(encoding="utf-8"))
        rights["assessment_status"] = "restricted"
        rights["server_processing_posture"] = "denied"
        self._refresh_rights_bindings(rights)
        with self.assertRaises(importer.RightsGateError):
            self._import_one()
        self.assertEqual([], self.transport.put_calls)

    def test_registry_protected_remote_read_requires_current_plan(self) -> None:
        outcome = self._import_one()
        receipt = importer.load_receipt(outcome.receipt_path, repo_root=self.repo)
        registry = self.root / "publication-registry.json"
        importer.enable_publication(receipt, registry_path=registry, mode="controlled-research")
        with self.assertRaises(importer.PublicationError):
            importer.read_remote(
                receipt,
                transport=self.transport,
                registry_path=registry,
                output=self.root / "remote.txt",
            )

    def test_metadata_only_plan_cannot_transfer(self) -> None:
        denied = copy.deepcopy(self.plan)
        denied["server_import_id"] = "tos.server-import.fixture.metadata-only.20260913"
        denied["rights_policy"]["review_status"] = "human-reviewed"
        denied["rights_policy"].pop("rights_review")
        denied["access_class"] = "metadata-only"
        denied["payload_transfer_authorized"] = False
        denied["operator_transfer_approval"] = {
            "approved": False,
            "approved_by_real_human": False,
            "approved_at": None,
            "approval_ref": None,
        }
        denied["server_import_status"] = "blocked-rights"
        denied["publication_status"] = "metadata-only"
        denied_path = self.plan_path.with_name("metadata-only.json")
        self._write_json(denied_path, denied)
        denied_context = importer.load_plan(self.repo, denied_path)
        with self.assertRaises(importer.RightsGateError):
            importer.enforce_transfer_gate(denied_context.plan, context=denied_context)

    def test_agent_review_is_not_allowed_by_v1_schema(self) -> None:
        legacy_agent = copy.deepcopy(self.plan)
        legacy_agent["server_import_id"] = "tos.server-import.fixture.legacy-agent.20260913"
        legacy_agent["contract_version"] = 1
        legacy_path = self.plan_path.with_name("legacy-agent.json")
        self._write_json(legacy_path, legacy_agent)
        with self.assertRaises(importer.PlanError):
            importer.load_plan(self.repo, legacy_path)

    def test_legacy_human_review_remains_loadable(self) -> None:
        legacy = copy.deepcopy(self.plan)
        legacy["server_import_id"] = "tos.server-import.fixture.legacy-human.20260913"
        legacy["contract_version"] = 1
        legacy["rights_policy"]["review_status"] = "human-reviewed"
        legacy["rights_policy"].pop("rights_review")
        legacy_path = self.plan_path.with_name("legacy-human.json")
        self._write_json(legacy_path, legacy)
        context = importer.load_plan(self.repo, legacy_path)
        importer.enforce_transfer_gate(context.plan, context=context)

    def test_wrangler_adapter_sets_private_standard_upload_contract(self) -> None:
        executable = self.root / "wrangler"
        executable.touch()
        transport = importer.WranglerR2Transport(
            bucket="tos-source-payloads",
            executable=executable,
            cwd=self.root,
        )
        calls: list[list[str]] = []

        def fake_run(args: list[str]):
            calls.append(args)
            return importer.subprocess.CompletedProcess(args, 0, "", "")

        transport._run = fake_run  # type: ignore[method-assign]
        source = self.root / "source.bin"
        source.write_bytes(self.body)
        transport.put(
            "blobs/sha256/aa/" + "a" * 64,
            source,
            byte_size=len(self.body),
            media_type="text/plain",
            storage_class="Standard",
        )
        self.assertEqual(
            ["--remote", "--file", str(source), "--content-type", "text/plain", "--cache-control", "private,no-store", "--storage-class", "Standard"],
            calls[0][4:],
        )


if __name__ == "__main__":
    unittest.main()
