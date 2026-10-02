"""Native handoff consumer fixture; no external provider is contacted."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import acquisition_batch as native_batch  # noqa: E402
import acquisition_handoff_adapter as native_handoff  # noqa: E402
from tests import test_acquisition_handoff_adapter as fixture_module  # noqa: E402


@unittest.skipUnless(
    os.environ.get("TOS_NATIVE_OWNER_COMMAND_BIN")
    or shutil.which("tos-native-owner-command"),
    "native owner product not selected",
)
class NativeAcquisitionHandoffTests(unittest.TestCase):
    def setUp(self) -> None:
        self.fixture = fixture_module.AcquisitionHandoffAdapterTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.tearDown)

    def _produce_handoff(self) -> tuple[str, str, dict[str, object]]:
        fixture = self.fixture
        _fetches, _unused_sha, _item_root, records = fixture._write_manifest(
            base_revision="0" * 64
        )
        base_revision = fixture._write_accepted_base(records)
        fetches, manifest_sha, _item_root, _records = fixture._write_manifest(
            base_revision=base_revision
        )
        result = native_batch.acquire_batch(
            manifest_path=fixture.manifest_path,
            metadata_root=fixture.metadata,
            output_root=fixture.acquisition_root,
            expected_manifest_sha256=manifest_sha,
            fetcher=lambda payload: fetches[payload["file_ref"]],
            max_attempts=1,
        )
        return base_revision, manifest_sha, result

    def test_native_verify_and_adapter_acceptance_preserve_legacy_fate(self) -> None:
        fixture = self.fixture
        base_revision, manifest_sha, produced = self._produce_handoff()

        # Old sealed handoffs can predate private 0700 preparation roots.
        # The shared verifier's exact legacy behavior is to accept these roots
        # while still requiring the selected files and payload custody to bind.
        for name in ("", "source", "payload", "receipts"):
            (fixture.acquisition_root / name).chmod(0o755)

        verified = native_handoff.verify_handoff_for_intake(
            acquisition_root=fixture.acquisition_root,
            handoff_ref=produced["handoff_ref"],
            expected_base_revision=base_revision,
            expected_manifest_sha256=manifest_sha,
            repo_root=ROOT,
        )
        self.assertEqual(fixture.acquisition_root.resolve(), verified.root)
        self.assertEqual(manifest_sha, verified.context.manifest_sha256)
        self.assertEqual(
            fixture.manifest_path.read_bytes(), verified.context.raw_manifest
        )
        self.assertEqual(7, len(verified.selected_source_rows))
        self.assertEqual(1, len(verified.payloads))
        validation_binding = native_handoff.verify_validation_context(
            fixture._validation_context(),
            validator_sha256=fixture.validator_sha256,
        )
        self.assertEqual(
            str(ROOT.resolve()), validation_binding["admission_flags"]["grammar_root"]
        )

        with self.assertRaisesRegex(
            native_handoff.HandoffAdapterError,
            "manifest digest differs from caller-selected digest",
        ):
            native_handoff.verify_handoff_for_intake(
                acquisition_root=fixture.acquisition_root,
                handoff_ref=produced["handoff_ref"],
                expected_base_revision=base_revision,
                expected_manifest_sha256="f" * 64,
                repo_root=ROOT,
            )

        pointer_before = (fixture.accepted_store / "current.json").read_bytes()
        adapted = native_handoff.adapt_handoff(
            acquisition_root=fixture.acquisition_root,
            handoff_ref=produced["handoff_ref"],
            expected_manifest_sha256=manifest_sha,
            output_root=fixture.candidate,
            accepted_store_root=fixture.accepted_store,
            accepted_source_root=fixture.accepted_source,
            base_revision=base_revision,
            validator_sha256=fixture.validator_sha256,
            validation_context=fixture._validation_context(),
            repo_root=ROOT,
        )

        # Adaptation only returns after the native AdmissionBatch::read
        # consumer accepted this exact batch and source root.
        self.assertEqual("candidate-not-admitted", adapted["status"])
        self.assertEqual("not-admitted", adapted["admission_status"])
        self.assertEqual("not-run-transport-only", adapted["admission_preflight"])
        candidate_batch = fixture.candidate / adapted["candidate_batch_ref"]
        batch = json.loads(candidate_batch.read_bytes())
        self.assertEqual("tos_corpus_batch_v1", batch["schema_version"])
        self.assertEqual(base_revision, batch["base_revision"])
        self.assertEqual(7, len(batch["updates"]))
        receipt = json.loads(
            (fixture.candidate / "receipts/acquisition-handoff-adapter.json").read_bytes()
        )
        self.assertEqual("not-admitted", receipt["admission_status"])
        self.assertEqual("not-published", receipt["publication_status"])
        self.assertEqual(
            pointer_before,
            (fixture.accepted_store / "current.json").read_bytes(),
        )
        self.assertEqual(
            adapted["candidate_batch_sha256"],
            hashlib.sha256(candidate_batch.read_bytes()).hexdigest(),
        )


if __name__ == "__main__":
    unittest.main()
