from __future__ import annotations

from contextlib import ExitStack, contextmanager
import hashlib
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch


SCRIPTS = Path(__file__).resolve().parents[1] / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import source_payload_batch as batch
import source_payload_import as importer


class FakeTransport:
    def __init__(self) -> None:
        self.closed = False

    def fetch(self, _object_key: str, _destination: Path) -> bool:
        raise AssertionError("fake batch transport fetch should be owned by import_plan")

    def put(
        self,
        _object_key: str,
        _source: Path,
        *,
        byte_size: int,
        media_type: str,
        storage_class: str,
    ) -> None:
        raise AssertionError("fake batch transport put should be owned by import_plan")

    def close(self) -> None:
        self.closed = True


class SourcePayloadBatchTests(unittest.TestCase):
    def setUp(self) -> None:
        self._temporary = tempfile.TemporaryDirectory()
        self.root = Path(self._temporary.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        self.payload_root = self.root / "payload-root"
        self.payload_root.mkdir()
        self.scratch_root = self.root / "scratch"
        self.scratch_root.mkdir()
        self.receipt_dir = self.root / "receipts"
        self.receipt_dir.mkdir()
        self.output_dir = self.root / "batch-output"
        self.account_id = "a" * 32
        self.bucket = "tos-source-payloads"
        self.wrangler = Path("/usr/bin/wrangler")
        self.contexts: dict[str, SimpleNamespace] = {}
        self.refs: list[str] = []
        self.plans_file: Path
        self.expected_plans_sha256: str

    def tearDown(self) -> None:
        self._temporary.cleanup()

    def _write_plans(self, count: int) -> None:
        entries: list[dict[str, str]] = []
        plans_root = self.repo / "ToS/source-witnesses/server-import/plans"
        plans_root.mkdir(parents=True)
        for index in range(count):
            ref = f"ToS/source-witnesses/server-import/plans/fixture-{index}.json"
            path = self.repo / ref
            path.write_text(f"fixture plan {index}\n", encoding="utf-8")
            digest = hashlib.sha256(path.read_bytes()).hexdigest()
            entries.append({"ref": ref, "sha256": digest})
            self.refs.append(ref)
            self.contexts[ref] = SimpleNamespace(
                path=path,
                plan_ref=ref,
                plan_sha256=digest,
                plan={"server_import_id": f"tos.server-import.fixture-{index}"},
            )
        self.plans_file = self.root / "plans-index.json"
        self.plans_file.write_text(
            json.dumps({"plans": entries}, sort_keys=True, indent=2) + "\n",
            encoding="utf-8",
        )
        self.expected_plans_sha256 = hashlib.sha256(self.plans_file.read_bytes()).hexdigest()

    @contextmanager
    def _patched_preflight(self):
        verified_calls: list[str] = []
        gate_calls: list[str] = []

        def load_plan(_repo_root: Path, plan_path: Path):
            return self.contexts[plan_path.resolve().relative_to(self.repo.resolve()).as_posix()]

        def enforce(plan, *, context, now=None):
            del plan, now
            gate_calls.append(context.plan_ref)

        def verify(context, *, payload_source_root, payload_source_layout="source-witness", file_ref=None):
            del payload_source_root, payload_source_layout, file_ref
            verified_calls.append(context.plan_ref)
            return [SimpleNamespace(byte_size=100)]

        with ExitStack() as stack:
            stack.enter_context(patch.object(batch.importer, "load_plan", side_effect=load_plan))
            stack.enter_context(patch.object(batch.importer, "enforce_transfer_gate", side_effect=enforce))
            stack.enter_context(patch.object(batch.importer, "verify_local", side_effect=verify))
            yield gate_calls, verified_calls

    def _run(self, *, transfer: bool, transport_factory=None) -> batch.BatchResult:
        return batch.run_batch(
            repo_root=self.repo,
            plans_file=self.plans_file,
            expected_plans_sha256=self.expected_plans_sha256,
            payload_source_root=self.payload_root,
            scratch_root=self.scratch_root,
            receipt_dir=self.receipt_dir,
            bucket=self.bucket,
            account_id=self.account_id,
            wrangler=self.wrangler,
            output_dir=self.output_dir,
            transfer=transfer,
            transport_factory=transport_factory,
        )

    def test_two_items_form_exact_preflight_closure_without_transport(self) -> None:
        self._write_plans(2)
        factories: list[FakeTransport] = []

        def factory() -> FakeTransport:
            transport = FakeTransport()
            factories.append(transport)
            return transport

        with self._patched_preflight() as (gate_calls, verified_calls):
            result = self._run(transfer=False, transport_factory=factory)

        self.assertEqual(self.refs, gate_calls)
        self.assertEqual(self.refs, verified_calls)
        self.assertEqual([], factories)
        self.assertEqual(2, result.summary["plans"])
        self.assertEqual(2, result.summary["files"])
        self.assertEqual(200, result.summary["bytes"])
        self.assertEqual(2, result.summary["local_verified_files"])
        self.assertTrue(result.summary["complete"])
        self.assertEqual([], result.journal_path.read_text(encoding="utf-8").splitlines())

    def test_plans_digest_mismatch_fails_before_network_or_plan_load(self) -> None:
        self._write_plans(2)
        factory_calls: list[bool] = []
        load_calls: list[bool] = []

        def factory() -> FakeTransport:
            factory_calls.append(True)
            return FakeTransport()

        with patch.object(batch.importer, "load_plan", side_effect=lambda *_args: load_calls.append(True)):
            with self.assertRaisesRegex(batch.BatchError, "SHA-256"):
                batch.run_batch(
                    repo_root=self.repo,
                    plans_file=self.plans_file,
                    expected_plans_sha256="0" * 64,
                    payload_source_root=self.payload_root,
                    scratch_root=self.scratch_root,
                    receipt_dir=self.receipt_dir,
                    bucket=self.bucket,
                    account_id=self.account_id,
                    wrangler=self.wrangler,
                    output_dir=self.output_dir,
                    transfer=True,
                    transport_factory=factory,
                )

        self.assertEqual([], factory_calls)
        self.assertEqual([], load_calls)

    def test_midbatch_failure_preserves_completed_plan_journal_and_summary(self) -> None:
        self._write_plans(2)
        transports: list[FakeTransport] = []

        def factory() -> FakeTransport:
            transport = FakeTransport()
            transports.append(transport)
            return transport

        def import_one(context, **kwargs):
            receipt = self.receipt_dir / f"{context.plan_ref.rsplit('/', 1)[-1]}.receipt.json"
            if context.plan_ref == self.refs[1]:
                raise importer.RemoteIntegrityError("planned second-plan failure")
            receipt.write_text("receipt-one\n", encoding="utf-8")
            return [
                SimpleNamespace(
                    receipt_path=receipt,
                    remote_status="uploaded",
                    upload_attempted=True,
                    receipt_reused=False,
                )
            ]

        with self._patched_preflight():
            with patch.object(batch.importer, "import_plan", side_effect=import_one):
                with self.assertRaises(importer.RemoteIntegrityError):
                    self._run(transfer=True, transport_factory=factory)

        summaries = sorted(self.output_dir.glob("batch-*.json"))
        journals = sorted(self.output_dir.glob("batch-*.jsonl"))
        self.assertEqual(1, len(summaries))
        self.assertEqual(1, len(journals))
        summary = json.loads(summaries[0].read_text(encoding="utf-8"))
        lines = journals[0].read_text(encoding="utf-8").splitlines()
        self.assertFalse(summary["complete"])
        self.assertEqual("RemoteIntegrityError", summary["failure_type"])
        self.assertEqual(1, summary["completed_plans"])
        self.assertEqual(1, len(lines))
        self.assertEqual(self.refs[0], json.loads(lines[0])["plan_ref"])
        self.assertTrue((self.receipt_dir / "fixture-0.json.receipt.json").is_file())
        self.assertTrue(transports[0].closed)

    def test_plan_change_after_preflight_aborts_before_next_import(self) -> None:
        self._write_plans(2)
        transports: list[FakeTransport] = []
        import_calls: list[str] = []

        def factory() -> FakeTransport:
            transport = FakeTransport()
            transports.append(transport)
            return transport

        def import_one(context, **kwargs):
            del kwargs
            import_calls.append(context.plan_ref)
            receipt = self.receipt_dir / "first.receipt.json"
            receipt.write_text("receipt-one\n", encoding="utf-8")
            # Simulate a source-plan edit while the long batch is between
            # plans. The next import must not use its cached context.
            if context.plan_ref == self.refs[0]:
                self.contexts[self.refs[1]].path.write_text(
                    "changed after preflight\n", encoding="utf-8"
                )
            return [
                SimpleNamespace(
                    receipt_path=receipt,
                    remote_status="uploaded",
                    upload_attempted=True,
                    receipt_reused=False,
                )
            ]

        with self._patched_preflight():
            with patch.object(batch.importer, "import_plan", side_effect=import_one):
                with self.assertRaisesRegex(batch.BatchError, "plan changed during batch"):
                    self._run(transfer=True, transport_factory=factory)

        self.assertEqual([self.refs[0]], import_calls)
        summaries = sorted(self.output_dir.glob("batch-*.json"))
        journals = sorted(self.output_dir.glob("batch-*.jsonl"))
        self.assertEqual(1, len(summaries))
        self.assertEqual(1, len(journals))
        summary = json.loads(summaries[0].read_text(encoding="utf-8"))
        entries = journals[0].read_text(encoding="utf-8").splitlines()
        self.assertEqual(1, summary["completed_plans"])
        self.assertFalse(summary["complete"])
        self.assertEqual(1, len(entries))
        self.assertEqual(self.refs[0], json.loads(entries[0])["plan_ref"])
        self.assertTrue((self.receipt_dir / "first.receipt.json").is_file())
        self.assertTrue(transports[0].closed)

    def test_repeat_run_reports_receipt_reuse_without_new_upload(self) -> None:
        self._write_plans(1)
        transports: list[FakeTransport] = []

        def factory() -> FakeTransport:
            transport = FakeTransport()
            transports.append(transport)
            return transport

        def import_one(context, **kwargs):
            receipt = self.receipt_dir / "fixture.receipt.json"
            if receipt.exists():
                return [
                    SimpleNamespace(
                        receipt_path=receipt,
                        remote_status="already-matched",
                        upload_attempted=False,
                        receipt_reused=True,
                    )
                ]
            receipt.write_text("receipt-one\n", encoding="utf-8")
            return [
                SimpleNamespace(
                    receipt_path=receipt,
                    remote_status="uploaded",
                    upload_attempted=True,
                    receipt_reused=False,
                )
            ]

        with self._patched_preflight():
            with patch.object(batch.importer, "import_plan", side_effect=import_one):
                first = self._run(transfer=True, transport_factory=factory)
                second = self._run(transfer=True, transport_factory=factory)

        self.assertEqual(2, len(transports))
        self.assertEqual(1, first.summary["uploaded_files"])
        self.assertEqual(0, first.summary["reused_receipts"])
        self.assertEqual(0, second.summary["uploaded_files"])
        self.assertEqual(1, second.summary["reused_receipts"])
        second_entry = json.loads(second.journal_path.read_text(encoding="utf-8").splitlines()[0])
        self.assertTrue(second_entry["outcomes"][0]["receipt_reused"])
        self.assertFalse(second_entry["outcomes"][0]["upload_attempted"])


if __name__ == "__main__":
    unittest.main()
