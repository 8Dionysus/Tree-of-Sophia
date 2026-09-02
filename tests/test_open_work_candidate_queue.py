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
    build_payload,
    candidate_digest,
    render_payload,
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
        "channels": [
            {
                "channel_id": "channel-originating-record",
                "endpoint_url": "https://example.test/source",
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

        self.assertEqual("open-work-candidate.earliest", build_payload(repo)["next_candidate_id"])

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
                "channels": [
                    {
                        "channel_id": "channel-originating-record",
                        "endpoint_url": "https://example.test/source",
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
