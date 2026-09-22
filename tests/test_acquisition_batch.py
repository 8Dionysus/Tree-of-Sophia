from __future__ import annotations

import hashlib
import json
from pathlib import Path
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

from jsonschema import Draft202012Validator


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import acquisition_batch as acquisition  # noqa: E402


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

    def _write_manifest(self, *, count: int = 2) -> tuple[dict[str, bytes], str]:
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
            rights_ref = f"{item_root}/rights.json"
            provenance_ref = f"{item_root}/provenance.jsonl"
            records = []
            for ref, kind, body in (
                (item_record_ref, "item", json.dumps({"item_id": item_ref}).encode()),
                (rights_ref, "rights", json.dumps({"rights": "local_only"}).encode()),
                (provenance_ref, "provenance", b"batch source observation\n"),
            ):
                path = self.metadata / ref
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(body)
                records.append({"ref": ref, "sha256": hashlib.sha256(body).hexdigest(), "kind": kind})
                all_record_refs.append(ref)
            payload = f"payload/{slug}.txt"
            body = f"payload {index}\n".encode()
            payload_ref = f"tos.file.sha256.{hashlib.sha256(body).hexdigest()}"
            url = f"https://provider.example/{slug}/r1/{slug}.txt"
            fetches[payload_ref] = body
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
                        "sha256": records[1]["sha256"],
                        "posture": "local_only",
                    },
                    "payload_files": [
                        {
                            "item_ref": item_ref,
                            "file_ref": payload_ref,
                            "item_root_ref": item_root,
                            "relative_path": payload,
                            "byte_size": len(body),
                            "sha256": hashlib.sha256(body).hexdigest(),
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
                "payload_file_refs": sorted(all_payload_refs),
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
        self.assertEqual(8 * 3 + 1, measurement["metadata_file_count"])
        legacy = self.root / "legacy" / "topology-before"
        legacy.mkdir(parents=True)
        for index in range(8):
            (legacy / f"{index}.json").write_bytes((bytes([index]) * 4096))
        legacy_bytes = sum(path.stat().st_size for path in legacy.iterdir())
        self.assertLess(measurement["metadata_bytes"], legacy_bytes)
        self.assertTrue((self.output / "receipts/preparation.json").is_file())
        self.assertTrue(
            (self.output / "source/ToS/source-witnesses/discovery/acquisition-batches/fixture-20260921/provenance-delta.json").is_file()
        )
        delta = json.loads(
            (self.output / "source/ToS/source-witnesses/discovery/acquisition-batches/fixture-20260921/provenance-delta.json").read_text(encoding="utf-8")
        )
        schema = json.loads(
            (ROOT / "ToS/contracts/acquisition-provenance-delta.schema.json").read_text(encoding="utf-8")
        )
        Draft202012Validator(schema).validate(delta)

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

    def test_manifest_digest_and_revision_binding_fail_closed(self) -> None:
        _fetches, manifest_sha = self._write_manifest(count=1)
        with self.assertRaisesRegex(acquisition.AcquisitionBatchError, "manifest SHA-256"):
            acquisition.load_manifest(self.manifest_path, expected_sha256="0" * 64)
        manifest = json.loads(self.manifest_path.read_text(encoding="utf-8"))
        manifest["provenance_delta"]["base_revision"] = "b" * 64
        self.manifest_path.write_text(json.dumps(manifest), encoding="utf-8")
        with self.assertRaisesRegex(acquisition.AcquisitionBatchError, "base revision"):
            acquisition.load_manifest(self.manifest_path)

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

        with self.assertRaisesRegex(acquisition.SourceIntegrityError, "selected record digest"):
            acquisition.acquire_batch(
                manifest_path=self.manifest_path,
                metadata_root=self.metadata,
                output_root=self.output,
                expected_manifest_sha256=manifest_sha,
                fetcher=should_not_fetch,
            )
        self.assertEqual([], fetch_calls)
        with self.assertRaisesRegex(acquisition.SourceIntegrityError, "selected record digest"):
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
            with self.assertRaisesRegex(acquisition.SourceIntegrityError, "selected record digest"):
                acquisition.acquire_batch(
                    manifest_path=self.manifest_path,
                    metadata_root=self.metadata,
                    output_root=self.output,
                    expected_manifest_sha256=manifest_sha,
                    fetcher=lambda payload: fetches[payload["file_ref"]],
                )
        self.assertEqual([], list((self.output / "receipts").glob("handoff-*.json")))

    def test_interrupted_prepare_is_rebuilt_before_acquisition(self) -> None:
        fetches, manifest_sha = self._write_manifest(count=1)
        self.output.mkdir()
        (self.output / "source").mkdir()
        (self.output / "payload").mkdir()
        (self.output / "receipts").mkdir()
        (self.output / "source/partial.tmp").write_bytes(b"interrupted")
        result = acquisition.acquire_batch(
            manifest_path=self.manifest_path,
            metadata_root=self.metadata,
            output_root=self.output,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
        )
        self.assertEqual("acquired-not-admitted", result["status"])
        self.assertTrue((self.output / "receipts/preparation.json").is_file())
        self.assertFalse((self.output / "source/partial.tmp").exists())

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


if __name__ == "__main__":
    unittest.main()
