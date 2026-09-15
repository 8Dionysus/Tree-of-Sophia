from __future__ import annotations

from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
from typing import Iterator
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

import publish_stats_release as publish  # noqa: E402
from corpus_store import CorpusStoreError, canonical  # noqa: E402
from downstream_status import DownstreamStatus  # noqa: E402


VALIDATOR_SHA256 = "1" * 64


def _write_owner(stats_root: Path, *, mode: str = "ok") -> Path:
    validator = stats_root / "scripts/validate_stats_protocol.py"
    validator.parent.mkdir(parents=True, exist_ok=True)
    validator.write_text(
        """from __future__ import annotations

import json
import sys
from pathlib import Path

mode = %r
port = Path(sys.argv[sys.argv.index("--port") + 1])
packet = port.parent / "packets/table-i-prepared-dossier-route-ratio.reference.json"
if mode == "reject-malformed":
    payload = json.loads(packet.read_text(encoding="utf-8"))
    if payload.get("semantic_status") == "malformed":
        print("owner rejected malformed semantic packet", file=sys.stderr)
        raise SystemExit(17)
elif mode == "mutate":
    port.write_bytes(port.read_bytes() + b" ")
elif mode == "reject":
    print("owner rejected stats port", file=sys.stderr)
    raise SystemExit(19)
"""
        % mode,
        encoding="utf-8",
    )
    return validator


def _write_json(path: Path, value: object) -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(canonical(value))
    return path


@contextmanager
def _fixture(*, owner_mode: str = "ok", malformed_packet: bool = False) -> Iterator[dict[str, Path]]:
    temporary = tempfile.TemporaryDirectory(prefix="tos-stats-release-")
    root = Path(temporary.name)
    source_root = root / "source"
    stats_root = root / "aoa-stats"
    release_root = root / "release-state"
    files = {
        "stats/AGENTS.md": b"# synthetic stats owner\n",
        "stats/README.md": b"# synthetic stats port\n",
        "stats/VALIDATION.md": b"# synthetic validation route\n",
    }
    for relative, payload in files.items():
        path = source_root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)
    _write_json(
        source_root / publish.PORT_PATH,
        {
            "schema_version": "synthetic_stats_port_v1",
            "evidence_posture": {
                "live_state": "reference_only",
                "privacy": "public",
                "raw_content_allowed": False,
            },
        },
    )
    packet = {
        "schema_version": "synthetic_stats_packet_v1",
        "observation_id": "synthetic:observation:one",
        "observed_at": "2026-09-14T00:00:00Z",
        "provenance": {"source_revision": "packet-source-revision"},
        "posture": {"live_state": "reference"},
    }
    if malformed_packet:
        packet["semantic_status"] = "malformed"
    _write_json(source_root / publish.PACKET_PATH, packet)
    _write_owner(stats_root, mode=owner_mode)
    try:
        yield {
            "root": root,
            "source_root": source_root,
            "stats_root": stats_root,
            "release_root": release_root,
        }
    finally:
        temporary.cleanup()


def _source_revision(source_root: Path) -> str:
    records = []
    for relative in sorted(publish.SOURCE_PATHS):
        path = source_root / relative
        payload = path.read_bytes()
        records.append(
            {
                "path": relative,
                "sha256": hashlib.sha256(payload).hexdigest(),
                "size_bytes": len(payload),
            }
        )
    return hashlib.sha256(
        canonical({"schema_version": publish.SOURCE_SCHEMA, "files": records})
    ).hexdigest()


class PublishStatsReleaseTests(unittest.TestCase):
    def test_imported_consumer_change_is_bound_and_can_reject_same_port(self) -> None:
        with _fixture() as fixture:
            owner = fixture['stats_root']
            helper = owner / 'src/aoa_stats_builder/check.py'
            helper.parent.mkdir(parents=True)
            helper.write_text('ACCEPT = True\n')
            validator = owner / 'scripts/validate_stats_protocol.py'
            with validator.open('a') as stream:
                stream.write("\nimport runpy\nassert runpy.run_path(str(Path(__file__).resolve().parents[1] / 'src/aoa_stats_builder/check.py'))['ACCEPT']\n")
            first = publish.build_release(fixture['source_root'], owner, fixture['release_root'])
            helper.write_text('ACCEPT = False\n')
            with self.assertRaisesRegex(CorpusStoreError, 'validator rejected'):
                publish.build_release(fixture['source_root'], owner, fixture['release_root'])
            status = publish.status_release(fixture['release_root'], first['source_revision'])
            self.assertEqual(status['integration_revision'], first['integration_revision'])
            self.assertEqual(status['latest_attempt']['state'], 'failed')
            helper.write_text('ACCEPT = True  # revised owner implementation\n')
            second = publish.build_release(fixture['source_root'], owner, fixture['release_root'])
            self.assertEqual(first['source_revision'], second['source_revision'])
            self.assertNotEqual(first['validator_sha256'], second['validator_sha256'])
            self.assertNotEqual(first['integration_revision'], second['integration_revision'])

    @unittest.skipUnless(hasattr(os, 'mkfifo'), 'POSIX special-file boundary')
    def test_consumer_cannot_add_an_unmanifested_special_file(self) -> None:
        with _fixture() as fixture:
            validator = fixture['stats_root'] / 'scripts/validate_stats_protocol.py'
            with validator.open('a') as stream:
                stream.write("\nimport os\nos.mkfifo(port.parent / 'extra.pipe')\n")
            with self.assertRaisesRegex(CorpusStoreError, 'non-regular'):
                publish.build_release(fixture['source_root'], fixture['stats_root'], fixture['release_root'])
            state = DownstreamStatus(fixture['release_root'], 'stats').status(_source_revision(fixture['source_root']))
            self.assertIsNone(state['state']['last_success'])

    def test_numeric_overflow_is_rejected_before_consumer(self) -> None:
        with _fixture() as fixture:
            packet = fixture['source_root'] / publish.PACKET_PATH
            raw = packet.read_bytes().rstrip()
            packet.write_bytes(raw[:-1] + b',"overflow":1e999}\n')
            with self.assertRaisesRegex(CorpusStoreError, 'non-finite'):
                publish.build_release(fixture['source_root'], fixture['stats_root'], fixture['release_root'])

    def test_valid_build_and_status_return_exact_port_observation(self) -> None:
        with _fixture() as fixture:
            manifest = publish.build_release(
                fixture["source_root"], fixture["stats_root"], fixture["release_root"]
            )
            self.assertEqual(manifest["source_kind"], publish.SOURCE_KIND)
            self.assertEqual(manifest["source_revision"], _source_revision(fixture["source_root"]))
            self.assertEqual(
                manifest["evidence_posture"],
                {
                    "live_state": "reference_only",
                    "privacy": "public",
                    "raw_content_allowed": False,
                },
            )
            self.assertEqual(
                manifest["observation"],
                {
                    "observation_id": "synthetic:observation:one",
                    "observed_at": "2026-09-14T00:00:00Z",
                    "source_revision": "packet-source-revision",
                    "live_state": "reference",
                },
            )
            self.assertEqual(
                [entry["path"] for entry in manifest["files"]],
                sorted(publish.SOURCE_PATHS),
            )
            result = publish.status_release(
                fixture["release_root"], manifest["source_revision"]
            )
            self.assertEqual(result["source_kind"], publish.SOURCE_KIND)
            self.assertEqual(result["freshness"], "current")
            self.assertEqual(result["integration_revision"], manifest["integration_revision"])
            self.assertEqual(result["observation"], manifest["observation"])

    def test_malformed_semantic_packet_is_rejected_by_owner_without_release(self) -> None:
        with _fixture(owner_mode="reject-malformed", malformed_packet=True) as fixture:
            with self.assertRaisesRegex(CorpusStoreError, "validator rejected"):
                publish.build_release(
                    fixture["source_root"], fixture["stats_root"], fixture["release_root"]
                )
            self.assertFalse((fixture["release_root"] / "releases").exists())
            state = DownstreamStatus(fixture["release_root"], "stats").status(
                _source_revision(fixture["source_root"])
            )
            self.assertEqual(state["latest_attempt"]["state"], "failed")
            self.assertIsNone(state["state"]["last_success"])

    def test_consumer_mutation_is_rejected_before_publication(self) -> None:
        with _fixture(owner_mode="mutate") as fixture:
            with self.assertRaisesRegex(CorpusStoreError, "staged bytes changed"):
                publish.build_release(
                    fixture["source_root"], fixture["stats_root"], fixture["release_root"]
                )
            self.assertFalse((fixture["release_root"] / "releases").exists())
            state = DownstreamStatus(fixture["release_root"], "stats").status(
                _source_revision(fixture["source_root"])
            )
            self.assertEqual(state["latest_attempt"]["state"], "failed")

    def test_same_port_with_new_validator_has_new_immutable_identity(self) -> None:
        with _fixture() as fixture:
            first = publish.build_release(
                fixture["source_root"], fixture["stats_root"], fixture["release_root"]
            )
            _write_owner(fixture["stats_root"], mode="ok-v2")
            second = publish.build_release(
                fixture["source_root"], fixture["stats_root"], fixture["release_root"]
            )

            self.assertEqual(first["source_revision"], second["source_revision"])
            self.assertNotEqual(first["integration_revision"], second["integration_revision"])
            self.assertTrue(
                (
                    fixture["release_root"]
                    / "releases"
                    / first["integration_revision"]
                    / "integration.json"
                ).is_file()
            )
            self.assertTrue(
                (
                    fixture["release_root"]
                    / "releases"
                    / second["integration_revision"]
                    / "integration.json"
                ).is_file()
            )
            state = DownstreamStatus(fixture["release_root"], "stats").status(
                first["source_revision"]
            )
            self.assertNotEqual(
                state["state"]["previous_success"]["attempt_id"],
                state["state"]["last_success"]["attempt_id"],
                msg="success records are distinct attempts",
            )
            self.assertEqual(state["freshness"], "current")

    def test_failed_update_retains_prior_success_and_reference_observation(self) -> None:
        with _fixture() as fixture:
            first = publish.build_release(
                fixture["source_root"], fixture["stats_root"], fixture["release_root"]
            )
            _write_owner(fixture["stats_root"], mode="reject")
            with self.assertRaisesRegex(CorpusStoreError, "validator rejected"):
                publish.build_release(
                    fixture["source_root"], fixture["stats_root"], fixture["release_root"]
                )

            result = publish.status_release(fixture["release_root"], first["source_revision"])
            self.assertEqual(result["freshness"], "current")
            self.assertEqual(result["integration_revision"], first["integration_revision"])
            self.assertEqual(result["observation"], first["observation"])
            self.assertEqual(result["latest_attempt"]["state"], "failed")
            self.assertTrue(
                (
                    fixture["release_root"]
                    / "releases"
                    / first["integration_revision"]
                ).is_dir()
            )

    def test_status_missing_and_stale_do_not_create_state_or_change_observation(self) -> None:
        with tempfile.TemporaryDirectory(prefix="tos-stats-status-missing-") as raw:
            release_root = Path(raw) / "missing"
            result = publish.status_release(release_root, "a" * 64)
            self.assertEqual(result["source_kind"], publish.SOURCE_KIND)
            self.assertEqual(result["freshness"], "missing")
            self.assertIsNone(result["observation"])
            self.assertFalse(release_root.exists())

        with _fixture() as fixture:
            manifest = publish.build_release(
                fixture["source_root"], fixture["stats_root"], fixture["release_root"]
            )
            stale = publish.status_release(fixture["release_root"], "b" * 64)
            self.assertEqual(stale["freshness"], "stale")
            self.assertEqual(stale["observation"], manifest["observation"])

    def test_status_rejects_tampered_last_success_manifest_identity(self) -> None:
        with _fixture() as fixture:
            manifest = publish.build_release(
                fixture["source_root"], fixture["stats_root"], fixture["release_root"]
            )
            manifest_path = (
                fixture["release_root"]
                / "releases"
                / manifest["integration_revision"]
                / "integration.json"
            )
            tampered = json.loads(manifest_path.read_bytes())
            tampered["source_kind"] = "wrong_kind"
            manifest_path.write_bytes(canonical(tampered))
            with self.assertRaisesRegex(CorpusStoreError, "identity mismatch"):
                publish.status_release(fixture["release_root"], manifest["source_revision"])


if __name__ == "__main__":
    unittest.main()
