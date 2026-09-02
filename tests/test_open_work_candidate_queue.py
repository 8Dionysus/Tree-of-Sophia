from __future__ import annotations

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = REPO_ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from open_work_candidate_queue_common import (  # noqa: E402
    QUEUE_PATH,
    QueueBuildError,
    _has_independent_snapshot_witness,
    _validate_active_discovery_timings,
    _validate_receipt_acquisition_closure,
    _validate_receipt_version_timestamp_order,
    _validate_planting_refs,
    _validate_target_binding,
    build_payload,
    candidate_digest,
    render_payload,
    target_digest,
)


def _write_json(path: Path, payload: object) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def _write_jsonl(path: Path, payloads: list[object]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        "".join(json.dumps(payload, ensure_ascii=False, sort_keys=True) + "\n" for payload in payloads),
        encoding="utf-8",
    )


def _candidate(candidate_id: str, *, year: int, row_id: str, row_order: int) -> dict:
    label = candidate_id.rsplit(".", 1)[-1].replace("-", " ").title()
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/open-work-candidate.schema.json",
        "schema_version": "tos_open_work_candidate_v1",
        "candidate_id": candidate_id,
        "candidate_kind": "work",
        "preferred_label": label,
        "identity_posture": "unresolved-work-identity",
        "queue_status": "ready-for-discovery",
        "source_refs": [
            {
                "source_path": "ToS/philosophy/atlas/master-tables/table-i/rows.jsonl",
                "selector": {"row_id": row_id},
                "role": "chronology-and-work-lead",
                "evidence_ceiling": "queue-ordering-only",
            }
        ],
        "selection": {
            "chronology_sort_year": year,
            "chronology_basis": "reviewed written-fixation lower bound",
            "atlas_row_id": row_id,
            "atlas_row_order": row_order,
            "confidence": "reviewed-for-queue-order-only",
        },
        "target": {
            "target_kind": "work",
            "known_tos_refs": [],
            "description": f"Resolve {label} without manufacturing later source layers.",
            "required_properties": ["work identity", "earliest exact witness route"],
            "acceptable_substitutions": [],
            "languages": ["und"],
            "formats": ["bibliographic metadata"],
            "purpose_ref": "ToS/source-witnesses/discovery/candidates/README.md",
        },
        "rights_review_scope": {
            "jurisdictions": ["US"],
            "intended_uses": ["local-preservation", "research-analysis", "redistribution"],
            "layers": ["work", "expression", "edition", "digital-object"],
        },
        "dedupe_keys": [f"label:{label.lower()}"],
        "review": {
            "review_status": "reviewed",
            "reviewed_at": "2026-08-29",
            "maker": {"maker_type": "model", "agent_ref": "codex:test"},
            "rationale": "Synthetic reviewed queue fixture.",
            "limits": "No bibliographic, rights, text, semantic, or canon acceptance.",
        },
        "record_version": 1,
    }


def _timed_discovery(discovery_id: str) -> dict:
    label = discovery_id.removeprefix("tos.discovery.").replace("-", " ").title()
    return {
        "discovery_id": discovery_id,
        "status": "reconciled",
        "target": {
            "target_kind": "work",
            "known_tos_refs": [],
            "description": f"Resolve {label} without manufacturing later source layers.",
        },
        "started_at": "2026-08-29T12:00:00Z",
        "ended_at": "2026-08-29T12:00:00.125000Z",
        "channels": [
            {
                "channel_id": "channel-originating-record",
                "endpoint_url": "https://example.test/source",
                "queried_at": "2026-08-29T12:00:00Z",
                "elapsed_seconds": 0.125,
                "results": [],
            }
        ],
        "channel_comparison": [
            {
                "channel_id": "channel-originating-record",
                "human_minutes": 0,
                "machine_seconds": 0.125,
                "notes": "Measured automatically; no real-human review was performed.",
            }
        ],
    }


def _timing_receipt(discovery_id: str, discovery_ref: str, *, elapsed: float = 0.125) -> dict:
    return {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/open-work-channel-timing-receipt.schema.json",
        "schema_version": "tos_open_work_channel_timing_receipt_v1",
        "timing_id": "open-work-channel-timing.synthetic.2026-08-29.v1",
        "discovery_ref": discovery_ref,
        "discovery_id": discovery_id,
        "measured_at": "2026-08-29T12:00:00.125000Z",
        "measurements": [
            {
                "channel_id": "channel-originating-record",
                "measurement": {
                    "method": "monotonic-http-request-v1",
                    "clock": "python.time.perf_counter_ns",
                    "timing_scope": "request-through-first-16384-response-bytes",
                    "probe_url": "https://example.test/source",
                    "started_at": "2026-08-29T12:00:00Z",
                    "ended_at": "2026-08-29T12:00:00.125000Z",
                    "elapsed_seconds": elapsed,
                    "outcome": "success",
                    "http_status": 200,
                    "response_bytes_observed": 1,
                },
            }
        ],
        "claim_limit": "monotonic transport timing through the first 16384 response bytes only; not research, interpretation, rights-review, or human elapsed time",
        "record_version": 1,
    }


class OpenWorkCandidateQueueTest(unittest.TestCase):
    def make_repo(self) -> Path:
        root = Path(tempfile.mkdtemp())
        table_root = root / "ToS/philosophy/atlas/master-tables"
        _write_jsonl(table_root / "table-i/rows.jsonl", [{"row_id": "A04"}, {"row_id": "A05"}])
        _write_jsonl(table_root / "table-ii/rows.jsonl", [{"row_id": "T2-01"}])
        _write_jsonl(table_root / "table-iii/rows.jsonl", [{"row_id": "T3-01"}])
        _write_jsonl(
            root / "ToS/philosophy/atlas/dossiers/index.jsonl",
            [{"dossier_id": "A04"}, {"dossier_id": "A05"}],
        )
        _write_jsonl(
            root / "ToS/philosophy/atlas/dossiers/source-anchor-backlog.jsonl",
            [
                {"dossier_id": "A04", "anchor_kind": "corpus_or_edition_anchor", "source_row_index": 1},
                {"dossier_id": "A05", "anchor_kind": "corpus_or_edition_anchor", "source_row_index": 1},
            ],
        )
        _write_jsonl(
            root / "ToS/source-witnesses/catalog/works.jsonl",
            [{"record_id": "tos.work.synthetic.existing", "preferred_label": "Existing"}],
        )
        _write_json(
            root / "ToS/source-witnesses/discovery/runs/existing.v1.json",
            {"discovery_id": "tos.discovery.existing", "status": "reconciled"},
        )
        _write_jsonl(
            root / "ToS/source-witnesses/discovery/candidates/reviewed-candidates.jsonl",
            [
                _candidate("open-work-candidate.later", year=-2000, row_id="A05", row_order=5),
                _candidate("open-work-candidate.earliest", year=-2400, row_id="A04", row_order=4),
            ],
        )
        return root

    def test_selects_earliest_ready_candidate_and_reports_every_input_surface(self) -> None:
        repo = self.make_repo()
        payload = build_payload(repo)

        self.assertEqual("open-work-candidate.earliest", payload["next_candidate_id"])
        self.assertEqual(
            {
                "master_rows": 4,
                "accepted_dossiers": 2,
                "source_anchor_backlog_rows": 2,
                "catalog_works": 1,
                "discovery_runs": 1,
                "reviewed_candidates": 2,
                "terminal_receipts": 0,
            },
            payload["source_snapshot"]["counts"],
        )
        self.assertIn("not identity, rights, semantic, or canon authority", payload["authority_boundary"])

    def test_terminal_receipt_advances_queue_without_rewriting_candidate(self) -> None:
        repo = self.make_repo()
        before = build_payload(repo)
        earliest = before["candidates"][0]
        receipt = {
            "$schema": "https://tree-of-sophia.local/ToS/contracts/open-work-candidate-receipt.schema.json",
            "schema_version": "tos_open_work_candidate_receipt_v1",
            "receipt_id": "open-work-candidate-receipt.earliest.2026-08-29.v1",
            "candidate_id": earliest["candidate_id"],
            "candidate_record_sha256": earliest["candidate_sha256"],
            "queue_snapshot_sha256": before["queue_sha256"],
            "discovery_ref": "ToS/source-witnesses/discovery/runs/earliest.v1.json",
            "discovery_id": "tos.discovery.earliest",
            "discovery_target_sha256": target_digest(_timed_discovery("tos.discovery.earliest")["target"]),
            "timing_ref": "ToS/source-witnesses/discovery/timings/earliest.v1.json",
            "terminal_status": "metadata_only",
            "target_resolution": {
                "identity_status": "provisional",
                "work_ref": None,
                "expression_ref": None,
                "edition_ref": None,
                "item_ref": None,
                "summary": "Metadata route only.",
            },
            "rights_result": {
                "status": "insufficient-for-acquisition",
                "reviewed_jurisdictions": ["US"],
                "reviewed_layers": ["work", "digital-object"],
                "evidence_refs": ["ToS/source-witnesses/discovery/runs/earliest.v1.json"],
                "summary": "No exact reusable digital object admitted.",
            },
            "acquisition": {
                "downloaded": False,
                "item_ref": None,
                "artifact_ref": None,
                "representation_ref": None,
                "file_ref": None,
                "provenance_event_ref": None,
            },
            "planting_refs": [],
            "operational_relation_refs": [],
            "next_trigger": "A stronger exact rights record.",
            "issued_at": "2026-08-29T12:00:00Z",
            "maker": {"maker_type": "model", "agent_ref": "codex:test"},
            "record_version": 1,
        }
        _write_json(
            repo / "ToS/source-witnesses/discovery/runs/earliest.v1.json",
            _timed_discovery("tos.discovery.earliest"),
        )
        _write_json(
            repo / "ToS/source-witnesses/discovery/timings/earliest.v1.json",
            _timing_receipt(
                "tos.discovery.earliest",
                "ToS/source-witnesses/discovery/runs/earliest.v1.json",
            ),
        )
        _write_json(
            repo / "ToS/source-witnesses/discovery/candidates/receipts/earliest.2026-08-29.v1.json",
            receipt,
        )

        after = build_payload(repo)
        self.assertEqual("open-work-candidate.later", after["next_candidate_id"])
        states = {entry["candidate_id"]: entry["effective_status"] for entry in after["candidates"]}
        self.assertEqual("metadata_only", states["open-work-candidate.earliest"])
        self.assertEqual("ready-for-discovery", states["open-work-candidate.later"])

    def test_missing_review_source_selector_fails_closed(self) -> None:
        repo = self.make_repo()
        ledger = repo / "ToS/source-witnesses/discovery/candidates/reviewed-candidates.jsonl"
        candidates = [json.loads(line) for line in ledger.read_text(encoding="utf-8").splitlines()]
        candidates[0]["source_refs"][0]["selector"] = {"row_id": "A99"}
        _write_jsonl(ledger, candidates)

        with self.assertRaisesRegex(QueueBuildError, "selector does not resolve"):
            build_payload(repo)

    def test_review_source_selector_can_resolve_a_json_object(self) -> None:
        repo = self.make_repo()
        source_path = repo / "ToS/philosophy/atlas/master-tables/table-i/selected.json"
        _write_json(source_path, {"row_id": "A05"})
        ledger = repo / "ToS/source-witnesses/discovery/candidates/reviewed-candidates.jsonl"
        candidates = [json.loads(line) for line in ledger.read_text(encoding="utf-8").splitlines()]
        candidates[0]["source_refs"][0]["source_path"] = (
            "ToS/philosophy/atlas/master-tables/table-i/selected.json"
        )
        _write_jsonl(ledger, candidates)

        payload = build_payload(repo)
        self.assertEqual("open-work-candidate.earliest", payload["next_candidate_id"])
        self.assertIn(
            "ToS/philosophy/atlas/master-tables/table-i/selected.json",
            {entry["path"] for entry in payload["source_snapshot"]["inputs"]},
        )

    def test_review_source_selectors_must_bind_candidate_selection(self) -> None:
        for candidate_index, selector_key, wrong_row, source_path in (
            (
                0,
                "row_id",
                "A04",
                "ToS/philosophy/atlas/master-tables/table-i/rows.jsonl",
            ),
            (1, "dossier_id", "A05", "ToS/philosophy/atlas/dossiers/index.jsonl"),
        ):
            with self.subTest(selector_key=selector_key):
                repo = self.make_repo()
                ledger = repo / "ToS/source-witnesses/discovery/candidates/reviewed-candidates.jsonl"
                candidates = [
                    json.loads(line)
                    for line in ledger.read_text(encoding="utf-8").splitlines()
                ]
                source_ref = candidates[candidate_index]["source_refs"][0]
                source_ref["source_path"] = source_path
                source_ref["selector"] = {selector_key: wrong_row}
                _write_jsonl(ledger, candidates)

                with self.assertRaisesRegex(
                    QueueBuildError,
                    rf"{selector_key} selector .* does not bind candidate selection atlas_row_id",
                ):
                    build_payload(repo)

    def test_unreferenced_snapshot_packet_cannot_witness_receipt(self) -> None:
        repo = self.make_repo()
        snapshot_hash = "a" * 64
        packet = repo / "ToS/research-packets/unrelated-snapshot.md"
        packet.parent.mkdir(parents=True, exist_ok=True)
        packet.write_text(
            f"Candidate: `open-work-candidate.earliest`\n"
            f"Queue snapshot SHA-256: `{snapshot_hash}`\n"
            "Earliest\n",
            encoding="utf-8",
        )
        discovery = _timed_discovery("tos.discovery.earliest")

        self.assertFalse(
            _has_independent_snapshot_witness(
                repo,
                {
                    "receipt_id": "open-work-candidate-receipt.earliest.v1",
                    "candidate_id": "open-work-candidate.earliest",
                    "discovery_id": "tos.discovery.earliest",
                    "issued_at": "2026-08-29T12:00:00Z",
                    "queue_snapshot_sha256": snapshot_hash,
                },
                candidate_id="open-work-candidate.earliest",
                candidate_label="Earliest",
                discoveries={
                    "tos.discovery.earliest": (
                        discovery,
                        "ToS/source-witnesses/discovery/runs/earliest.v1.json",
                    )
                },
                provenance_events={},
            )
        )

    def test_candidate_source_ref_preserves_physical_jsonl_line_number(self) -> None:
        repo = self.make_repo()
        ledger = repo / "ToS/source-witnesses/discovery/candidates/reviewed-candidates.jsonl"
        original = ledger.read_text(encoding="utf-8")
        ledger.write_text("\n" + original, encoding="utf-8")

        payload = build_payload(repo)
        source_refs = {
            entry["candidate_id"]: entry["candidate_source_ref"]
            for entry in payload["candidates"]
        }
        self.assertTrue(source_refs["open-work-candidate.later"].endswith(":2"))
        self.assertTrue(source_refs["open-work-candidate.earliest"].endswith(":3"))

    def test_latest_terminal_receipt_rejects_unknown_timing_sentinels(self) -> None:
        repo = self.make_repo()
        before = build_payload(repo)
        earliest = before["candidates"][0]
        discovery_ref = "ToS/source-witnesses/discovery/runs/earliest.v1.json"
        _write_json(
            repo / discovery_ref,
            {
                "discovery_id": "tos.discovery.earliest",
                "status": "reconciled",
                "target": {
                    "target_kind": "work",
                    "known_tos_refs": [],
                    "description": "Resolve Earliest without manufacturing later source layers.",
                },
                "started_at": "2026-08-29T12:00:00Z",
                "ended_at": "2026-08-29T12:00:00.125000Z",
                "channels": [
                    {
                        "channel_id": "channel-originating-record",
                        "endpoint_url": "https://example.test/source",
                        "queried_at": "2026-08-29T12:00:00Z",
                        "elapsed_seconds": 0,
                        "results": [],
                    }
                ],
                "channel_comparison": [
                    {
                        "channel_id": "channel-originating-record",
                        "human_minutes": 0,
                        "machine_seconds": 0,
                        "notes": "Zero is an unknown timing sentinel.",
                    }
                ],
            },
        )
        _write_json(
            repo
            / "ToS/source-witnesses/discovery/candidates/receipts/earliest.2026-08-29.v1.json",
            {
                "$schema": "https://tree-of-sophia.local/ToS/contracts/open-work-candidate-receipt.schema.json",
                "schema_version": "tos_open_work_candidate_receipt_v1",
                "receipt_id": "open-work-candidate-receipt.earliest.2026-08-29.v1",
                "candidate_id": earliest["candidate_id"],
                "candidate_record_sha256": earliest["candidate_sha256"],
                "queue_snapshot_sha256": before["queue_sha256"],
                "discovery_ref": discovery_ref,
                "discovery_id": "tos.discovery.earliest",
                "discovery_target_sha256": target_digest(_timed_discovery("tos.discovery.earliest")["target"]),
                "timing_ref": "ToS/source-witnesses/discovery/timings/earliest.v1.json",
                "terminal_status": "metadata_only",
                "target_resolution": {
                    "identity_status": "provisional",
                    "work_ref": None,
                    "expression_ref": None,
                    "edition_ref": None,
                    "item_ref": None,
                    "summary": "Metadata route only.",
                },
                "rights_result": {
                    "status": "insufficient-for-acquisition",
                    "reviewed_jurisdictions": ["US"],
                    "reviewed_layers": ["work", "digital-object"],
                    "evidence_refs": [discovery_ref],
                    "summary": "No exact reusable digital object admitted.",
                },
                "acquisition": {
                    "downloaded": False,
                    "item_ref": None,
                    "artifact_ref": None,
                    "representation_ref": None,
                    "file_ref": None,
                    "provenance_event_ref": None,
                },
                "planting_refs": [],
                "operational_relation_refs": [],
                "next_trigger": "A stronger exact rights record.",
                "issued_at": "2026-08-29T12:00:00Z",
                "maker": {"maker_type": "model", "agent_ref": "codex:test"},
                "record_version": 1,
            },
        )
        _write_json(
            repo / "ToS/source-witnesses/discovery/timings/earliest.v1.json",
            _timing_receipt("tos.discovery.earliest", discovery_ref, elapsed=0),
        )

        with self.assertRaisesRegex(QueueBuildError, "measured elapsed_seconds"):
            build_payload(repo)

    def test_terminal_receipt_must_freeze_discovery_target_digest(self) -> None:
        discovery = _timed_discovery("tos.discovery.earliest")
        with self.assertRaisesRegex(QueueBuildError, "discovery_target_sha256 is required"):
            _validate_target_binding(
                _candidate(
                    "open-work-candidate.earliest",
                    year=-2400,
                    row_id="A04",
                    row_order=4,
                ),
                discovery,
                receipt={},
                location="synthetic-receipt",
            )

    def test_timing_measurement_must_match_channel_probe_timestamp(self) -> None:
        discovery = _timed_discovery("tos.discovery.earliest")
        timing = _timing_receipt(
            "tos.discovery.earliest",
            "ToS/source-witnesses/discovery/runs/earliest.v1.json",
        )
        timing["measurements"][0]["measurement"]["started_at"] = "2026-08-29T12:00:00.001000Z"

        with self.assertRaisesRegex(QueueBuildError, "started_at must equal channel queried_at"):
            _validate_active_discovery_timings(
                discovery,
                timing,
                discovery_ref="ToS/source-witnesses/discovery/runs/earliest.v1.json",
                location="synthetic-discovery",
            )

    def test_planting_source_witness_must_bind_to_receipt_route(self) -> None:
        repo = self.make_repo()
        discovery_id = "tos.discovery.earliest"
        discovery_ref = "ToS/source-witnesses/discovery/runs/earliest.v1.json"
        discovery = _timed_discovery(discovery_id)
        foreign_id = "tos.artifact.synthetic.foreign"
        foreign_record_ref = "ToS/source-witnesses/artifacts/synthetic/foreign/artifact-witness.json"
        _write_json(repo / foreign_record_ref, {"artifact_id": foreign_id})
        planting_ref = "ToS/source-witnesses/discovery/candidates/foreign-planting.json"
        _write_json(
            repo / planting_ref,
            {
                "planting_id": "tos.planting.synthetic.foreign",
                "atlas_row_id": "A04",
                "dossier_id": "A04",
                "source_witness": {
                    "artifact_id": foreign_id,
                    "record_ref": foreign_record_ref,
                },
                "discovery_ref": discovery_ref,
                "provenance_event_ref": "tos.event.discovery.synthetic.foreign",
            },
        )

        with self.assertRaisesRegex(QueueBuildError, "source_witness .*not bound to the receipt"):
            _validate_planting_refs(
                repo,
                [planting_ref],
                candidate=_candidate(
                    "open-work-candidate.earliest",
                    year=-2400,
                    row_id="A04",
                    row_order=4,
                ),
                receipt={
                    "discovery_ref": discovery_ref,
                    "discovery_id": discovery_id,
                    "operational_relation_refs": [],
                },
                discovery=discovery,
                discoveries={discovery_id: (discovery, discovery_ref)},
                acquisitions=[
                    {
                        "downloaded": False,
                        "item_ref": None,
                        "artifact_ref": None,
                        "composite_ref": None,
                        "representation_ref": None,
                        "file_ref": None,
                        "provenance_event_ref": None,
                    }
                ],
                provenance_events={},
                location="synthetic-receipt",
            )

    def test_downloaded_acquisition_requires_positive_rights_result(self) -> None:
        repo = self.make_repo()
        with self.assertRaisesRegex(QueueBuildError, "rights_result.status positive-for-acquisition"):
            _validate_receipt_acquisition_closure(
                repo,
                {
                    "candidate_id": "open-work-candidate.earliest",
                    "rights_result": {
                        "status": "blocked-human-legal-review",
                    },
                    "operational_relation_refs": [],
                    "acquisition": {"downloaded": True},
                    "planting_refs": [],
                },
                candidate=_candidate(
                    "open-work-candidate.earliest",
                    year=-2400,
                    row_id="A04",
                    row_order=4,
                ),
                discovery=_timed_discovery("tos.discovery.earliest"),
                discoveries={},
                provenance_events={},
                location="synthetic-receipt",
            )

    def test_held_source_witness_requires_resolved_witness(self) -> None:
        repo = self.make_repo()
        _write_json(
            repo / "ToS/source-witnesses/discovery/runs/earliest.v1.json",
            _timed_discovery("tos.discovery.earliest"),
        )
        with self.assertRaisesRegex(QueueBuildError, "held_source_witness requires a resolved"):
            _validate_receipt_acquisition_closure(
                repo,
                {
                    "candidate_id": "open-work-candidate.earliest",
                    "terminal_status": "held_source_witness",
                    "rights_result": {
                        "status": "insufficient-for-acquisition",
                        "evidence_refs": ["ToS/source-witnesses/discovery/runs/earliest.v1.json"],
                    },
                    "operational_relation_refs": [],
                    "acquisition": {
                        "downloaded": False,
                        "item_ref": None,
                        "artifact_ref": None,
                        "composite_ref": None,
                        "representation_ref": None,
                        "file_ref": None,
                        "provenance_event_ref": None,
                    },
                    "planting_refs": [],
                },
                candidate=_candidate(
                    "open-work-candidate.earliest",
                    year=-2400,
                    row_id="A04",
                    row_order=4,
                ),
                discovery=_timed_discovery("tos.discovery.earliest"),
                discoveries={},
                provenance_events={},
                location="synthetic-receipt",
            )

    def test_downloaded_representation_must_bind_receipt_route(self) -> None:
        repo = self.make_repo()
        candidate_id = "open-work-candidate.earliest"
        receipt_discovery_id = "tos.discovery.earliest"
        receipt_discovery_ref = "ToS/source-witnesses/discovery/runs/earliest.v1.json"
        foreign_discovery_id = "tos.discovery.foreign"
        foreign_discovery_ref = "ToS/source-witnesses/discovery/runs/foreign.v1.json"
        _write_json(repo / receipt_discovery_ref, _timed_discovery(receipt_discovery_id))
        _write_json(repo / foreign_discovery_ref, _timed_discovery(foreign_discovery_id))

        artifact_id = "tos.artifact.synthetic.foreign"
        artifact_ref = "ToS/source-witnesses/artifacts/synthetic/foreign/artifact-witness.json"
        _write_json(repo / artifact_ref, {"artifact_id": artifact_id})
        file_ref = "tos.file.sha256." + ("a" * 64)
        representation_ref = (
            "ToS/source-witnesses/artifacts/synthetic/foreign/representations/test/representation.json"
        )
        _write_json(
            repo / representation_ref,
            {
                "artifact_id": artifact_id,
                "artifact_ref": artifact_ref,
                "file_id": file_ref,
                "payload": {"sha256": "a" * 64},
                "discovery_ref": foreign_discovery_ref,
                "provenance_event_ref": "tos.event.acquisition.synthetic",
            },
        )
        provenance_event = {
            "event_type": "acquisition",
            "method": {"configuration": {"candidate_id": candidate_id}},
            "inputs": [],
            "outputs": [
                {"ref": representation_ref},
                {"ref": file_ref},
            ],
        }
        with self.assertRaisesRegex(QueueBuildError, "representation discovery_ref .*not bound"):
            _validate_receipt_acquisition_closure(
                repo,
                {
                    "candidate_id": candidate_id,
                    "rights_result": {
                        "status": "positive-for-acquisition",
                        "evidence_refs": [receipt_discovery_ref],
                    },
                    "discovery_ref": receipt_discovery_ref,
                    "discovery_id": receipt_discovery_id,
                    "operational_relation_refs": [],
                    "acquisition": {
                        "downloaded": True,
                        "item_ref": None,
                        "artifact_ref": artifact_id,
                        "composite_ref": None,
                        "representation_ref": representation_ref,
                        "file_ref": file_ref,
                        "provenance_event_ref": "tos.event.acquisition.synthetic",
                    },
                    "planting_refs": [],
                },
                candidate=_candidate(candidate_id, year=-2400, row_id="A04", row_order=4),
                discovery=_timed_discovery(receipt_discovery_id),
                discoveries={
                    receipt_discovery_id: (
                        _timed_discovery(receipt_discovery_id),
                        receipt_discovery_ref,
                    ),
                    foreign_discovery_id: (
                        _timed_discovery(foreign_discovery_id),
                        foreign_discovery_ref,
                    ),
                },
                provenance_events={
                    "tos.event.acquisition.synthetic": (provenance_event, "synthetic-event"),
                },
                location="synthetic-receipt",
            )

    def test_downloaded_item_must_bind_receipt_route(self) -> None:
        repo = self.make_repo()
        candidate_id = "open-work-candidate.earliest"
        item_id = "tos.item.synthetic"
        item_ref = "ToS/source-witnesses/works/synthetic/items/item.manifest.json"
        file_ref = "tos.file.sha256." + ("b" * 64)
        event_ref = "tos.event.acquisition.synthetic-item"
        _write_json(
            repo / item_ref,
            {
                "item_id": item_id,
                "payload_files": [{"file_id": file_ref}],
            },
        )
        _write_json(
            repo / "ToS/source-witnesses/discovery/runs/earliest.v1.json",
            _timed_discovery("tos.discovery.earliest"),
        )
        provenance_event = {
            "event_type": "acquisition",
            "method": {"configuration": {}},
            "inputs": [{"ref": "tos.work.synthetic"}],
            "outputs": [{"ref": file_ref}],
        }
        with self.assertRaisesRegex(QueueBuildError, "item acquisition item_ref .*not bound"):
            _validate_receipt_acquisition_closure(
                repo,
                {
                    "candidate_id": candidate_id,
                    "rights_result": {
                        "status": "positive-for-acquisition",
                        "evidence_refs": [
                            "ToS/source-witnesses/discovery/runs/earliest.v1.json"
                        ],
                    },
                    "discovery_ref": "ToS/source-witnesses/discovery/runs/earliest.v1.json",
                    "discovery_id": "tos.discovery.earliest",
                    "operational_relation_refs": ["tos.discovery.earliest", event_ref],
                    "acquisition": {
                        "downloaded": True,
                        "item_ref": item_id,
                        "artifact_ref": None,
                        "composite_ref": None,
                        "representation_ref": None,
                        "file_ref": file_ref,
                        "provenance_event_ref": event_ref,
                    },
                    "planting_refs": [],
                },
                candidate=_candidate(candidate_id, year=-2400, row_id="A04", row_order=4),
                discovery=_timed_discovery("tos.discovery.earliest"),
                discoveries={
                    "tos.discovery.earliest": (
                        _timed_discovery("tos.discovery.earliest"),
                        "ToS/source-witnesses/discovery/runs/earliest.v1.json",
                    )
                },
                provenance_events={event_ref: (provenance_event, "synthetic-event")},
                location="synthetic-receipt",
            )

    def test_downloaded_acquisition_requires_resolved_rights_evidence(self) -> None:
        repo = self.make_repo()
        with self.assertRaisesRegex(QueueBuildError, "rights evidence ref does not resolve"):
            _validate_receipt_acquisition_closure(
                repo,
                {
                    "candidate_id": "open-work-candidate.earliest",
                    "rights_result": {
                        "status": "positive-for-acquisition",
                        "evidence_refs": ["ToS/does-not-exist.json"],
                    },
                    "operational_relation_refs": [],
                    "acquisition": {"downloaded": True},
                    "planting_refs": [],
                },
                candidate=_candidate(
                    "open-work-candidate.earliest",
                    year=-2400,
                    row_id="A04",
                    row_order=4,
                ),
                discovery=_timed_discovery("tos.discovery.earliest"),
                discoveries={},
                provenance_events={},
                location="synthetic-receipt",
            )

    def test_downloaded_acquisition_rejects_malformed_external_rights_evidence(self) -> None:
        repo = self.make_repo()
        for evidence_ref in ("ftp://example.test/evidence", "https://"):
            with self.subTest(evidence_ref=evidence_ref):
                with self.assertRaisesRegex(QueueBuildError, "rights evidence ref must be an http\\(s\\) URI"):
                    _validate_receipt_acquisition_closure(
                        repo,
                        {
                            "candidate_id": "open-work-candidate.earliest",
                            "rights_result": {
                                "status": "positive-for-acquisition",
                                "evidence_refs": [evidence_ref],
                            },
                            "operational_relation_refs": [],
                            "acquisition": {"downloaded": True},
                            "planting_refs": [],
                        },
                        candidate=_candidate(
                            "open-work-candidate.earliest",
                            year=-2400,
                            row_id="A04",
                            row_order=4,
                        ),
                        discovery=_timed_discovery("tos.discovery.earliest"),
                        discoveries={},
                        provenance_events={},
                        location="synthetic-receipt",
                    )

    def test_non_downloaded_outcome_requires_resolved_rights_evidence(self) -> None:
        repo = self.make_repo()
        with self.assertRaisesRegex(QueueBuildError, "rights evidence ref does not resolve"):
            _validate_receipt_acquisition_closure(
                repo,
                {
                    "candidate_id": "open-work-candidate.earliest",
                    "terminal_status": "metadata_only",
                    "rights_result": {
                        "status": "insufficient-for-acquisition",
                        "evidence_refs": ["ToS/does-not-exist.json"],
                    },
                    "operational_relation_refs": [],
                    "acquisition": {"downloaded": False},
                    "planting_refs": [],
                },
                candidate=_candidate(
                    "open-work-candidate.earliest",
                    year=-2400,
                    row_id="A04",
                    row_order=4,
                ),
                discovery=_timed_discovery("tos.discovery.earliest"),
                discoveries={},
                provenance_events={},
                location="synthetic-receipt",
            )

    def test_superseding_receipt_cannot_be_backdated(self) -> None:
        receipts = [
            {
                "receipt_id": "open-work-candidate-receipt.earliest.v1",
                "candidate_id": "open-work-candidate.earliest",
                "record_version": 1,
                "issued_at": "2026-08-29T12:00:00Z",
            },
            {
                "receipt_id": "open-work-candidate-receipt.earliest.v2",
                "candidate_id": "open-work-candidate.earliest",
                "record_version": 2,
                "issued_at": "2026-08-29T11:59:00Z",
            },
        ]
        with self.assertRaisesRegex(QueueBuildError, "record_version 2 issued_at .*earlier than predecessor"):
            _validate_receipt_version_timestamp_order(receipts)

    def test_receipt_must_bind_the_exact_candidate_digest(self) -> None:
        repo = self.make_repo()
        payload = build_payload(repo)
        record = _candidate("open-work-candidate.earliest", year=-2400, row_id="A04", row_order=4)
        self.assertEqual(candidate_digest(record), payload["candidates"][0]["candidate_sha256"])

        stale = copy.deepcopy(record)
        stale["preferred_label"] = "Changed after receipt"
        self.assertNotEqual(candidate_digest(stale), payload["candidates"][0]["candidate_sha256"])

    def test_repository_queue_matches_its_source_owned_builder(self) -> None:
        self.assertEqual(
            QUEUE_PATH.read_text(encoding="utf-8"),
            render_payload(build_payload(REPO_ROOT)),
        )

    def test_repository_contract_validator_accepts_queue_and_authored_inputs(self) -> None:
        completed = subprocess.run(
            [sys.executable, "scripts/validate_open_work_candidate_queue.py"],
            cwd=REPO_ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(0, completed.returncode, completed.stderr)
        self.assertIn("[ok] validated reviewed open-work candidate queue", completed.stdout)


if __name__ == "__main__":
    unittest.main()
