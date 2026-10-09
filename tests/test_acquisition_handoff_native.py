"""Native acquisition-handoff consumer fixture; no external provider is contacted."""
from __future__ import annotations

import base64
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys_path = str(ROOT / "scripts")
import sys
if sys_path not in sys.path:
    sys.path.insert(0, sys_path)

from tests.fixtures.acquisition_handoff import AcquisitionHandoffFixture  # noqa: E402


class NativeCommandError(ValueError):
    pass


def _native_command() -> str:
    return os.environ.get("TOS_NATIVE_OWNER_COMMAND_BIN") or shutil.which("tos-native-owner-command") or ""


def _invoke_native(request: dict[str, object]) -> dict[str, object]:
    binary = _native_command()
    if not binary:
        raise NativeCommandError("native owner product not selected")
    completed = subprocess.run(
        [binary, "acquisition"],
        input=json.dumps(request, ensure_ascii=False, separators=(",", ":")) + "\n",
        text=True,
        capture_output=True,
        cwd=ROOT,
        check=False,
    )
    try:
        response = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise NativeCommandError(f"native acquisition returned invalid JSON: {error}") from error
    if completed.returncode != 0 or response.get("kind") != "result":
        raise NativeCommandError(response.get("message", completed.stderr.strip()))
    value = response.get("value")
    if not isinstance(value, dict):
        raise NativeCommandError("native acquisition result must be an object")
    return value


def _invoke_native_batch(request: dict[str, object], fetches: dict[str, bytes]) -> dict[str, object]:
    binary = _native_command()
    if not binary:
        raise NativeCommandError("native owner product not selected")
    process = subprocess.Popen(
        [binary, "acquisition"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        cwd=ROOT,
    )
    assert process.stdin is not None and process.stdout is not None
    process.stdin.write(json.dumps(request, ensure_ascii=False, separators=(",", ":")) + "\n")
    process.stdin.flush()
    while True:
        line = process.stdout.readline()
        if not line:
            stderr = process.stderr.read() if process.stderr else ""
            process.wait()
            raise NativeCommandError(stderr.strip() or "native acquisition ended before a result")
        try:
            message = json.loads(line)
        except json.JSONDecodeError as error:
            process.kill()
            process.wait()
            raise NativeCommandError(f"native acquisition returned invalid JSON: {error}") from error
        if message.get("kind") == "fetch":
            payload = message.get("payload")
            file_ref = payload.get("file_ref") if isinstance(payload, dict) else None
            body = fetches.get(file_ref) if isinstance(file_ref, str) else None
            if body is None:
                process.stdin.write(json.dumps({"body_base64": None, "error": "fixture payload not selected"}) + "\n")
            else:
                process.stdin.write(json.dumps({"body_base64": base64.b64encode(body).decode("ascii"), "error": None}) + "\n")
            process.stdin.flush()
            continue
        process.stdin.close()
        completed_code = process.wait()
        diagnostics = process.stderr.read() if process.stderr else ""
        if completed_code != 0 or message.get("kind") != "result":
            raise NativeCommandError(message.get("message", diagnostics.strip()))
        value = message.get("value")
        if not isinstance(value, dict):
            raise NativeCommandError("native acquisition batch result must be an object")
        return value


@unittest.skipUnless(
    os.environ.get("TOS_NATIVE_OWNER_COMMAND_BIN")
    or shutil.which("tos-native-owner-command"),
    "native owner product not selected",
)
class NativeAcquisitionHandoffTests(unittest.TestCase):
    def setUp(self) -> None:
        self.fixture = AcquisitionHandoffFixture()
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
        result = _invoke_native_batch({
            "family": "batch",
            "operation": "acquire",
            "repo_root": str(ROOT),
            "manifest_path": str(fixture.manifest_path),
            "metadata_root": str(fixture.metadata),
            "output_root": str(fixture.acquisition_root),
            "expected_manifest_sha256": manifest_sha,
            "max_attempts": 1,
            "fetch_callback": True,
        }, fetches)
        return base_revision, manifest_sha, result

    def test_native_verify_and_adapter_acceptance_preserve_legacy_fate(self) -> None:
        fixture = self.fixture
        base_revision, manifest_sha, produced = self._produce_handoff()

        # Old sealed handoffs can predate private 0700 preparation roots. The
        # native consumer preserves that bounded compatibility while binding
        # selected files and payload custody.
        for name in ("", "source", "payload", "receipts"):
            (fixture.acquisition_root / name).chmod(0o755)

        verify_request = {
            "family": "handoff",
            "operation": "verify",
            "acquisition_root": str(fixture.acquisition_root),
            "handoff_ref": produced["handoff_ref"],
            "expected_base_revision": base_revision,
            "expected_manifest_sha256": manifest_sha,
            "repo_root": str(ROOT),
        }
        verified = _invoke_native(verify_request)
        self.assertEqual(fixture.acquisition_root.resolve().as_posix(), verified["root"])
        self.assertEqual(manifest_sha, verified["context"]["manifest_sha256"])
        self.assertEqual(
            fixture.manifest_path.read_bytes(),
            base64.b64decode(verified["context"]["raw_manifest_base64"], validate=True),
        )
        self.assertEqual(7, len(verified["selected_source_rows"]))
        self.assertEqual(1, len(verified["payloads"]))

        context = fixture._validation_context()
        validation_binding = _invoke_native({
            "family": "handoff",
            "operation": "validation-context",
            "validation_context": context,
            "validator_sha256": fixture.validator_sha256,
        })
        self.assertEqual(
            str(ROOT.resolve()),
            validation_binding["admission_flags"]["grammar_root"],
        )

        with self.assertRaisesRegex(
            NativeCommandError,
            "manifest digest differs from caller-selected digest",
        ):
            _invoke_native({**verify_request, "expected_manifest_sha256": "f" * 64})

        pointer_before = (fixture.accepted_store / "current.json").read_bytes()
        adapted = _invoke_native({
            "family": "handoff",
            "operation": "adapt",
            "acquisition_root": str(fixture.acquisition_root),
            "handoff_ref": produced["handoff_ref"],
            "expected_manifest_sha256": manifest_sha,
            "output_root": str(fixture.candidate),
            "accepted_store_root": str(fixture.accepted_store),
            "accepted_source_root": str(fixture.accepted_source),
            "base_revision": base_revision,
            "validator_sha256": fixture.validator_sha256,
            "validation_context": context,
            "repo_root": str(ROOT),
        })

        # Adaptation returns only after native AdmissionBatch::read accepted
        # the exact batch and source root, and makes no admission/publication.
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
        self.assertEqual(pointer_before, (fixture.accepted_store / "current.json").read_bytes())
        self.assertEqual(
            adapted["candidate_batch_sha256"],
            hashlib.sha256(candidate_batch.read_bytes()).hexdigest(),
        )


if __name__ == "__main__":
    unittest.main()
