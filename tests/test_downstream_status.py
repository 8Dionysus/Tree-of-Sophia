from __future__ import annotations

import json
from pathlib import Path
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from downstream_status import DownstreamStatus, DownstreamStatusError  # noqa: E402


SOURCE_A = "a" * 64
SOURCE_B = "b" * 64
ARTIFACT_A = "c" * 64
ARTIFACT_B = "d" * 64
MANIFEST_A = "e" * 64
MANIFEST_B = "f" * 64


class DownstreamStatusTests(unittest.TestCase):
    def test_first_status_is_missing_without_creating_anything(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-downstream-status-") as raw:
            parent = Path(raw)
            root = parent / "missing" / "state"
            result = DownstreamStatus(root, "kag").status(SOURCE_A)

            self.assertEqual(
                result,
                {"state": None, "freshness": "missing", "latest_attempt": None},
            )
            self.assertFalse(root.exists())
            self.assertEqual(list(parent.iterdir()), [])

    def test_success_records_attempt_and_current_status(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-downstream-status-") as raw:
            root = Path(raw) / "state"
            status = DownstreamStatus(root, "stats")
            attempt_id = status.begin(SOURCE_A)

            running = status.status(SOURCE_A)
            self.assertEqual(running["freshness"], "missing")
            self.assertEqual(running["latest_attempt"]["attempt_id"], attempt_id)
            self.assertEqual(running["latest_attempt"]["state"], "running")
            self.assertTrue((root / ".lock").is_file())

            status.succeed(
                attempt_id,
                artifact_revision=ARTIFACT_A,
                artifact_manifest_sha256=MANIFEST_A,
            )
            result = status.status(SOURCE_A)
            accepted = result["state"]
            self.assertEqual(result["freshness"], "current")
            self.assertEqual(result["latest_attempt"]["state"], "succeeded")
            self.assertEqual(accepted["consumer"], "stats")
            self.assertEqual(accepted["last_success"]["attempt_id"], attempt_id)
            self.assertEqual(accepted["last_success"]["source_revision"], SOURCE_A)
            self.assertEqual(accepted["last_success"]["artifact_revision"], ARTIFACT_A)
            self.assertEqual(accepted["last_success"]["artifact_manifest_sha256"], MANIFEST_A)
            self.assertIsNone(accepted["previous_success"])

    def test_failed_update_keeps_previous_success_and_is_visible(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-downstream-status-") as raw:
            status = DownstreamStatus(Path(raw) / "state", "kag")
            first = status.begin(SOURCE_A)
            status.succeed(
                first,
                artifact_revision=ARTIFACT_A,
                artifact_manifest_sha256=MANIFEST_A,
            )
            second = status.begin(SOURCE_B)
            status.fail(second, "owner validator failed")

            current = status.status(SOURCE_A)
            self.assertEqual(current["freshness"], "current")
            self.assertEqual(current["state"]["last_success"]["attempt_id"], first)
            self.assertEqual(current["latest_attempt"]["attempt_id"], second)
            self.assertEqual(current["latest_attempt"]["state"], "failed")
            self.assertEqual(current["latest_attempt"]["error"], "owner validator failed")
            self.assertEqual(status.status(SOURCE_B)["freshness"], "stale")

    def test_status_reports_stale_for_a_different_expected_source(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-downstream-status-") as raw:
            status = DownstreamStatus(Path(raw) / "state", "kag")
            attempt = status.begin(SOURCE_A)
            status.succeed(
                attempt,
                artifact_revision=ARTIFACT_A,
                artifact_manifest_sha256=MANIFEST_A,
            )

            self.assertEqual(status.status(SOURCE_B)["freshness"], "stale")

    def test_newer_begin_invalidates_older_attempt(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-downstream-status-") as raw:
            status = DownstreamStatus(Path(raw) / "state", "stats")
            first = status.begin(SOURCE_A)
            second = status.begin(SOURCE_B)

            with self.assertRaisesRegex(DownstreamStatusError, "stale"):
                status.succeed(
                    first,
                    artifact_revision=ARTIFACT_A,
                    artifact_manifest_sha256=MANIFEST_A,
                )
            latest = status.status(SOURCE_A)
            self.assertEqual(latest["freshness"], "missing")
            self.assertEqual(latest["latest_attempt"]["attempt_id"], second)
            self.assertEqual(latest["latest_attempt"]["source_revision"], SOURCE_B)

            status.succeed(
                second,
                artifact_revision=ARTIFACT_B,
                artifact_manifest_sha256=MANIFEST_B,
            )
            self.assertEqual(status.status(SOURCE_B)["freshness"], "current")

    def test_malformed_state_and_invalid_digests_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-downstream-status-") as raw:
            root = Path(raw) / "state"
            root.mkdir()
            (root / ".lock").write_bytes(b"")
            (root / "state.json").write_text(
                json.dumps(
                    {
                        "schema_version": "downstream_status_v1",
                        "consumer": "kag",
                        "latest": {
                            "attempt_id": "0" * 32,
                            "source_revision": "not-a-sha",
                            "started_at": "2026-09-14T00:00:00Z",
                            "state": "running",
                        },
                        "last_success": None,
                        "previous_success": None,
                    }
                )
                + "\n",
                encoding="utf-8",
            )

            with self.assertRaisesRegex(DownstreamStatusError, "canonical"):
                DownstreamStatus(root, "kag").status(SOURCE_A)

            (root / "state.json").write_text(
                '{"consumer":"kag","last_success":null,"latest":{"attempt_id":"00000000000000000000000000000000","source_revision":"not-a-sha","started_at":"2026-09-14T00:00:00Z","state":"running"},"previous_success":null,"schema_version":"downstream_status_v1"}\n',
                encoding="utf-8",
            )
            with self.assertRaisesRegex(DownstreamStatusError, "source_revision"):
                DownstreamStatus(root, "kag").status(SOURCE_A)

            with self.assertRaisesRegex(DownstreamStatusError, "consumer"):
                DownstreamStatus(root, "stats").status(SOURCE_A)

    def test_symlink_root_ancestor_and_managed_files_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-downstream-status-") as raw:
            parent = Path(raw)
            real_root = parent / "real"
            real_root.mkdir()
            root_link = parent / "root-link"
            root_link.symlink_to(real_root, target_is_directory=True)
            with self.assertRaisesRegex(DownstreamStatusError, "symlink"):
                DownstreamStatus(root_link, "kag")

            real_ancestor = parent / "real-ancestor"
            real_ancestor.mkdir()
            ancestor_link = parent / "ancestor-link"
            ancestor_link.symlink_to(real_ancestor, target_is_directory=True)
            with self.assertRaisesRegex(DownstreamStatusError, "symlink"):
                DownstreamStatus(ancestor_link / "state", "kag")

            state_root = parent / "file-links"
            state_root.mkdir()
            outside = parent / "outside"
            outside.write_text("{}\n", encoding="utf-8")
            (state_root / "state.json").symlink_to(outside)
            with self.assertRaisesRegex(DownstreamStatusError, "symlink"):
                DownstreamStatus(state_root, "kag").status(SOURCE_A)

            lock_root = parent / "lock-link"
            lock_root.mkdir()
            (lock_root / ".lock").symlink_to(outside)
            with self.assertRaisesRegex(DownstreamStatusError, "symlink"):
                DownstreamStatus(lock_root, "kag").status(SOURCE_A)

    def test_invalid_transition_and_digest_inputs_do_not_advance_state(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-downstream-status-") as raw:
            status = DownstreamStatus(Path(raw) / "state", "kag")
            with self.assertRaises(DownstreamStatusError):
                status.begin("bad")
            attempt = status.begin(SOURCE_A)
            with self.assertRaises(DownstreamStatusError):
                status.succeed(attempt, artifact_revision="bad", artifact_manifest_sha256=MANIFEST_A)
            with self.assertRaises(DownstreamStatusError):
                status.fail(attempt, "")
            self.assertEqual(status.status(SOURCE_A)["latest_attempt"]["state"], "running")


if __name__ == "__main__":
    unittest.main()
