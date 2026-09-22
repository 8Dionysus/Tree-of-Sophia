from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import acquisition_batch as acquisition  # noqa: E402
import acquisition_handoff_adapter as adapter  # noqa: E402
import corpus_admit  # noqa: E402


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
        (self.accepted_store / "current.json").write_text(
            json.dumps(
                {
                    "schema_version": "tos_corpus_pointer_v1",
                    "current": "a" * 64,
                    "previous": None,
                }
            ),
            encoding="utf-8",
        )
        self.accepted_source = self.root / "accepted" / "source"
        self.accepted_source.mkdir(parents=True)
        self.candidate = self.root / "candidate"

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def _write_manifest(self) -> tuple[dict[str, bytes], str, str]:
        item_ref = "tos.item.fixture.adapter"
        item_root = (
            "ToS/source-witnesses/works/fixture/expressions/en/"
            "editions/pinned/items/adapter"
        )
        records: list[dict[str, str]] = []
        for ref, kind, body in (
            (
                f"{item_root}/item.json",
                "item",
                json.dumps({"item_id": item_ref}).encode(),
            ),
            (
                f"{item_root}/rights.json",
                "rights",
                b'{"rights":"local_only"}\n',
            ),
            (
                f"{item_root}/provenance.jsonl",
                "provenance",
                b"adapter fixture source observation\n",
            ),
        ):
            path = self.metadata / ref
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(body)
            records.append(
                {"ref": ref, "kind": kind, "sha256": hashlib.sha256(body).hexdigest()}
            )
        payload_body = b"adapter fixture payload\n"
        payload_sha = hashlib.sha256(payload_body).hexdigest()
        payload_ref = f"tos.file.sha256.{payload_sha}"
        manifest = {
            "$schema": "https://tree-of-sophia.local/ToS/contracts/acquisition-batch.schema.json",
            "schema_version": "tos_acquisition_batch_v1",
            "batch_id": "tos.acquisition-batch.adapter-fixture-20260922",
            "batch_revision": 1,
            "prepared_at": "2026-09-22T12:00:00Z",
            "base_revision": "a" * 64,
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
                        "sha256": records[1]["sha256"],
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
                "base_revision": "a" * 64,
                "record_refs": sorted(row["ref"] for row in records),
                "payload_file_refs": [payload_ref],
                "supersedes_event_ref": None,
            },
        }
        self.manifest_path.write_bytes(
            (json.dumps(manifest, ensure_ascii=False, indent=2) + "\n").encode()
        )
        return {payload_ref: payload_body}, hashlib.sha256(self.manifest_path.read_bytes()).hexdigest(), item_root

    def test_producer_to_consumer_fixture_emits_valid_batch_input(self) -> None:
        fetches, manifest_sha, _item_root = self._write_manifest()
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
            output_root=self.candidate,
            accepted_store_root=self.accepted_store,
            accepted_source_root=self.accepted_source,
            base_revision="a" * 64,
            validator_sha256="b" * 64,
            repo_root=ROOT,
        )
        self.assertEqual("candidate-not-admitted", adapted["status"])
        batch_path = self.candidate / adapted["candidate_batch_ref"]
        batch, updates, retirements = corpus_admit.read_batch(
            batch_path, self.candidate / "source"
        )
        self.assertEqual("tos_corpus_batch_v1", batch["schema_version"])
        self.assertEqual(3, len(updates))
        self.assertEqual({}, retirements)
        self.assertEqual("a" * 64, batch["base_revision"])
        self.assertEqual("not-admitted", json.loads(
            (self.candidate / "receipts/acquisition-handoff-adapter.json").read_text()
        )["admission_status"])
        self.assertTrue(
            (self.candidate / "payload/works/fixture/expressions/en/editions/pinned/items/adapter/payload/adapter.txt").is_file()
        )

    def test_adapter_rejects_different_accepted_base_bytes(self) -> None:
        fetches, manifest_sha, item_root = self._write_manifest()
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
        with self.assertRaisesRegex(adapter.HandoffAdapterError, "replace accepted"):
            adapter.adapt_handoff(
                acquisition_root=self.acquisition_root,
                handoff_ref=result["handoff_ref"],
                output_root=self.candidate,
                accepted_store_root=self.accepted_store,
                accepted_source_root=self.accepted_source,
                base_revision="a" * 64,
                validator_sha256="b" * 64,
                repo_root=ROOT,
            )
        self.assertFalse(self.candidate.exists())


if __name__ == "__main__":
    unittest.main()
