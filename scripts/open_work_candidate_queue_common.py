#!/usr/bin/env python3
"""Build the reviewed open-work discovery queue from authored candidate records."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any, Iterable


REPO_ROOT = Path(__file__).resolve().parents[1]
CANDIDATE_ROOT = Path("ToS/source-witnesses/discovery/candidates")
LEDGER_PATH = CANDIDATE_ROOT / "reviewed-candidates.jsonl"
RECEIPT_ROOT = CANDIDATE_ROOT / "receipts"
TIMING_ROOT = Path("ToS/source-witnesses/discovery/timings")
QUEUE_RELATIVE_PATH = CANDIDATE_ROOT / "queue.current.json"
QUEUE_PATH = REPO_ROOT / QUEUE_RELATIVE_PATH

MASTER_ROW_PATHS = (
    Path("ToS/philosophy/atlas/master-tables/table-i/rows.jsonl"),
    Path("ToS/philosophy/atlas/master-tables/table-ii/rows.jsonl"),
    Path("ToS/philosophy/atlas/master-tables/table-iii/rows.jsonl"),
)
DOSSIER_INDEX_PATH = Path("ToS/philosophy/atlas/dossiers/index.jsonl")
BACKLOG_PATH = Path("ToS/philosophy/atlas/dossiers/source-anchor-backlog.jsonl")
WORK_CATALOG_PATH = Path("ToS/source-witnesses/catalog/works.jsonl")
DISCOVERY_RUN_ROOT = Path("ToS/source-witnesses/discovery/runs")

READY_STATUS = "ready-for-discovery"
TERMINAL_STATUSES = {
    "held_source_witness",
    "metadata_only",
    "publication_candidate",
    "deferred",
    "blocked",
    "exhausted_for_now",
    "duplicate",
    "rejected",
    "superseded",
}
QUEUEABLE_KINDS = {"work", "work-like-corpus"}
TIMING_METHOD = "monotonic-http-request-v1"
TIMING_CLOCK = "python.time.perf_counter_ns"


class QueueBuildError(RuntimeError):
    """Raised when authored queue inputs cannot support a deterministic build."""


def canonical_json(payload: object) -> str:
    return json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def candidate_digest(candidate: dict[str, Any]) -> str:
    return hashlib.sha256(canonical_json(candidate).encode("utf-8")).hexdigest()


def _safe_relative_path(value: str) -> Path:
    path = Path(value)
    if path.is_absolute() or ".." in path.parts:
        raise QueueBuildError(f"unsafe repository-relative path: {value!r}")
    return path


def _load_json(path: Path, repo_root: Path) -> dict[str, Any]:
    relative = path.relative_to(repo_root).as_posix()
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise QueueBuildError(f"{relative}: required JSON file is missing") from exc
    except (OSError, json.JSONDecodeError) as exc:
        raise QueueBuildError(f"{relative}: cannot read JSON: {exc}") from exc
    if not isinstance(payload, dict):
        raise QueueBuildError(f"{relative}: JSON root must be an object")
    return payload


def _load_jsonl(path: Path, repo_root: Path) -> list[dict[str, Any]]:
    relative = path.relative_to(repo_root).as_posix()
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError as exc:
        raise QueueBuildError(f"{relative}: required JSONL file is missing") from exc
    except OSError as exc:
        raise QueueBuildError(f"{relative}: cannot read JSONL: {exc}") from exc

    payloads: list[dict[str, Any]] = []
    for line_number, raw_line in enumerate(lines, start=1):
        if not raw_line.strip():
            continue
        try:
            payload = json.loads(raw_line)
        except json.JSONDecodeError as exc:
            raise QueueBuildError(f"{relative}:{line_number}: cannot parse JSON: {exc}") from exc
        if not isinstance(payload, dict):
            raise QueueBuildError(f"{relative}:{line_number}: JSONL record must be an object")
        payloads.append(payload)
    return payloads


def _required_string(payload: dict[str, Any], key: str, location: str) -> str:
    value = payload.get(key)
    if not isinstance(value, str) or not value:
        raise QueueBuildError(f"{location}: {key} must be a non-empty string")
    return value


def _selector_matches(payload: dict[str, Any], selector: dict[str, Any]) -> bool:
    return all(payload.get(key) == value for key, value in selector.items())


def _validate_source_refs(
    candidate: dict[str, Any],
    *,
    repo_root: Path,
    location: str,
) -> None:
    refs = candidate.get("source_refs")
    if not isinstance(refs, list) or not refs:
        raise QueueBuildError(f"{location}: source_refs must be a non-empty array")
    for index, source_ref in enumerate(refs, start=1):
        ref_location = f"{location}:source_refs[{index}]"
        if not isinstance(source_ref, dict):
            raise QueueBuildError(f"{ref_location}: source ref must be an object")
        source_path_value = _required_string(source_ref, "source_path", ref_location)
        selector = source_ref.get("selector")
        if not isinstance(selector, dict) or not selector:
            raise QueueBuildError(f"{ref_location}: selector must be a non-empty object")
        source_path = repo_root / _safe_relative_path(source_path_value)
        records = _load_jsonl(source_path, repo_root)
        if not any(_selector_matches(record, selector) for record in records):
            raise QueueBuildError(
                f"{ref_location}: selector does not resolve in {source_path_value}: {selector}"
            )


def _load_candidates(repo_root: Path) -> list[tuple[dict[str, Any], str]]:
    ledger = repo_root / LEDGER_PATH
    candidates = _load_jsonl(ledger, repo_root)
    seen: dict[str, str] = {}
    loaded: list[tuple[dict[str, Any], str]] = []
    for line_number, candidate in enumerate(candidates, start=1):
        location = f"{LEDGER_PATH.as_posix()}:{line_number}"
        candidate_id = _required_string(candidate, "candidate_id", location)
        if candidate_id in seen:
            raise QueueBuildError(
                f"{location}: duplicate candidate_id {candidate_id!r}; first seen at {seen[candidate_id]}"
            )
        seen[candidate_id] = location
        if candidate.get("review", {}).get("review_status") != "reviewed":
            raise QueueBuildError(f"{location}: candidate is not reviewed")
        selection = candidate.get("selection")
        if not isinstance(selection, dict):
            raise QueueBuildError(f"{location}: selection must be an object")
        if not isinstance(selection.get("chronology_sort_year"), int):
            raise QueueBuildError(f"{location}: chronology_sort_year must be an integer")
        if not isinstance(selection.get("atlas_row_order"), int):
            raise QueueBuildError(f"{location}: atlas_row_order must be an integer")
        _validate_source_refs(candidate, repo_root=repo_root, location=location)
        loaded.append((candidate, location))
    return loaded


def _discovery_records(repo_root: Path) -> dict[str, tuple[dict[str, Any], str]]:
    records: dict[str, tuple[dict[str, Any], str]] = {}
    root = repo_root / DISCOVERY_RUN_ROOT
    for path in sorted(root.glob("*.json")):
        payload = _load_json(path, repo_root)
        location = path.relative_to(repo_root).as_posix()
        discovery_id = _required_string(payload, "discovery_id", location)
        if discovery_id in records:
            raise QueueBuildError(
                f"{location}: duplicate discovery_id {discovery_id!r}; "
                f"first seen at {records[discovery_id][1]}"
            )
        records[discovery_id] = (payload, location)
    return records


def _validate_active_discovery_timings(
    discovery: dict[str, Any],
    timing: dict[str, Any],
    *,
    discovery_ref: str,
    location: str,
) -> None:
    channels = discovery.get("channels")
    comparisons = discovery.get("channel_comparison")
    if not isinstance(channels, list) or not channels:
        raise QueueBuildError(f"{location}: active discovery must contain timed channels")
    if not isinstance(comparisons, list):
        raise QueueBuildError(f"{location}: active discovery must contain channel_comparison")

    if timing.get("discovery_id") != discovery.get("discovery_id"):
        raise QueueBuildError(f"{location}: timing receipt does not bind discovery_id")
    if timing.get("discovery_ref") != discovery_ref:
        raise QueueBuildError(f"{location}: timing receipt does not bind discovery_ref")
    measurement_rows = timing.get("measurements")
    if not isinstance(measurement_rows, list) or not measurement_rows:
        raise QueueBuildError(f"{location}: timing receipt must contain measurements")
    measurement_by_id = {
        entry.get("channel_id"): entry.get("measurement")
        for entry in measurement_rows
        if isinstance(entry, dict) and isinstance(entry.get("channel_id"), str)
    }
    comparison_by_id = {
        comparison.get("channel_id"): comparison
        for comparison in comparisons
        if isinstance(comparison, dict) and isinstance(comparison.get("channel_id"), str)
    }
    channel_ids: set[str] = set()
    for index, channel in enumerate(channels, start=1):
        channel_location = f"{location}:channels[{index}]"
        if not isinstance(channel, dict):
            raise QueueBuildError(f"{channel_location}: channel must be an object")
        channel_id = _required_string(channel, "channel_id", channel_location)
        if channel_id in channel_ids:
            raise QueueBuildError(f"{channel_location}: duplicate channel_id {channel_id!r}")
        channel_ids.add(channel_id)

        elapsed = channel.get("elapsed_seconds")
        if not isinstance(elapsed, (int, float)) or isinstance(elapsed, bool) or elapsed <= 0:
            raise QueueBuildError(
                f"{channel_location}: active queue discovery requires measured elapsed_seconds > 0"
            )
        measurement = measurement_by_id.get(channel_id)
        if not isinstance(measurement, dict):
            raise QueueBuildError(
                f"{channel_location}: active queue discovery requires an external timing measurement"
            )
        if measurement.get("method") != TIMING_METHOD:
            raise QueueBuildError(
                f"{channel_location}: timing_measurement.method must be {TIMING_METHOD!r}"
            )
        if measurement.get("clock") != TIMING_CLOCK:
            raise QueueBuildError(
                f"{channel_location}: timing_measurement.clock must be {TIMING_CLOCK!r}"
            )
        measured_elapsed = measurement.get("elapsed_seconds")
        if (
            not isinstance(measured_elapsed, (int, float))
            or isinstance(measured_elapsed, bool)
            or measured_elapsed <= 0
            or abs(float(measured_elapsed) - float(elapsed)) > 0.000001
        ):
            raise QueueBuildError(
                f"{channel_location}: timing_measurement must bind the measured elapsed_seconds"
            )

        comparison = comparison_by_id.get(channel_id)
        if comparison is None:
            raise QueueBuildError(
                f"{location}: channel_comparison does not cover active channel {channel_id!r}"
            )
        machine_seconds = comparison.get("machine_seconds")
        if (
            not isinstance(machine_seconds, (int, float))
            or isinstance(machine_seconds, bool)
            or machine_seconds <= 0
            or abs(float(machine_seconds) - float(elapsed)) > 0.000001
        ):
            raise QueueBuildError(
                f"{location}: comparison for {channel_id!r} must bind measured machine_seconds"
            )
        notes = comparison.get("notes")
        if isinstance(notes, str) and any(
            marker in notes.lower() for marker in ("unknown sentinel", "timer sentinel", "not instrumented")
        ):
            raise QueueBuildError(
                f"{location}: comparison for {channel_id!r} still declares unknown timing"
            )

    if set(comparison_by_id) != channel_ids:
        raise QueueBuildError(f"{location}: channel_comparison must cover the exact active channel set")
    if set(measurement_by_id) != channel_ids:
        raise QueueBuildError(f"{location}: timing receipt must cover the exact active channel set")


def _load_receipts(
    repo_root: Path,
    *,
    candidates_by_id: dict[str, dict[str, Any]],
    discoveries: dict[str, tuple[dict[str, Any], str]],
) -> tuple[list[dict[str, Any]], dict[str, dict[str, Any]]]:
    receipt_root = repo_root / RECEIPT_ROOT
    receipt_paths = sorted(receipt_root.glob("*.json")) if receipt_root.is_dir() else []
    receipts: list[dict[str, Any]] = []
    latest_by_candidate: dict[str, dict[str, Any]] = {}
    seen_receipt_ids: set[str] = set()

    for path in receipt_paths:
        receipt = _load_json(path, repo_root)
        location = path.relative_to(repo_root).as_posix()
        receipt_id = _required_string(receipt, "receipt_id", location)
        if receipt_id in seen_receipt_ids:
            raise QueueBuildError(f"{location}: duplicate receipt_id {receipt_id!r}")
        seen_receipt_ids.add(receipt_id)
        candidate_id = _required_string(receipt, "candidate_id", location)
        candidate = candidates_by_id.get(candidate_id)
        if candidate is None:
            raise QueueBuildError(f"{location}: receipt references unknown candidate {candidate_id!r}")
        expected_digest = candidate_digest(candidate)
        if receipt.get("candidate_record_sha256") != expected_digest:
            raise QueueBuildError(
                f"{location}: candidate_record_sha256 does not bind current candidate {candidate_id!r}"
            )
        terminal_status = receipt.get("terminal_status")
        if terminal_status not in TERMINAL_STATUSES:
            raise QueueBuildError(f"{location}: unsupported terminal_status {terminal_status!r}")
        discovery_id = _required_string(receipt, "discovery_id", location)
        discovery = discoveries.get(discovery_id)
        if discovery is None:
            raise QueueBuildError(f"{location}: receipt references unknown discovery_id {discovery_id!r}")
        discovery_ref = _required_string(receipt, "discovery_ref", location)
        if discovery[1] != discovery_ref:
            raise QueueBuildError(
                f"{location}: discovery_ref {discovery_ref!r} does not resolve discovery_id {discovery_id!r}"
            )
        version = receipt.get("record_version")
        if not isinstance(version, int) or version < 1:
            raise QueueBuildError(f"{location}: record_version must be a positive integer")
        previous = latest_by_candidate.get(candidate_id)
        if previous is None or version > previous["record_version"]:
            latest_by_candidate[candidate_id] = receipt
        elif version == previous["record_version"]:
            raise QueueBuildError(
                f"{location}: duplicate terminal receipt version {version} for {candidate_id!r}"
            )
        receipts.append(receipt)

    for candidate_id, receipt in latest_by_candidate.items():
        discovery_id = receipt["discovery_id"]
        discovery, discovery_location = discoveries[discovery_id]
        timing_ref = _required_string(
            receipt,
            "timing_ref",
            f"latest receipt for {candidate_id}",
        )
        timing = _load_json(repo_root / _safe_relative_path(timing_ref), repo_root)
        _validate_active_discovery_timings(
            discovery,
            timing,
            discovery_ref=receipt["discovery_ref"],
            location=f"{discovery_location} (latest receipt for {candidate_id})",
        )
    return receipts, latest_by_candidate


def _sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _snapshot(
    repo_root: Path,
    *,
    candidates: list[tuple[dict[str, Any], str]],
    receipts: list[dict[str, Any]],
    discoveries: dict[str, tuple[dict[str, Any], str]],
) -> dict[str, Any]:
    master_rows = sum(len(_load_jsonl(repo_root / path, repo_root)) for path in MASTER_ROW_PATHS)
    dossiers = _load_jsonl(repo_root / DOSSIER_INDEX_PATH, repo_root)
    backlog = _load_jsonl(repo_root / BACKLOG_PATH, repo_root)
    works = _load_jsonl(repo_root / WORK_CATALOG_PATH, repo_root)

    input_paths: list[Path] = [
        *MASTER_ROW_PATHS,
        DOSSIER_INDEX_PATH,
        BACKLOG_PATH,
        WORK_CATALOG_PATH,
        LEDGER_PATH,
    ]
    input_paths.extend(Path(location) for _, location in discoveries.values())
    receipt_root = repo_root / RECEIPT_ROOT
    if receipt_root.is_dir():
        input_paths.extend(path.relative_to(repo_root) for path in sorted(receipt_root.glob("*.json")))
    timing_root = repo_root / TIMING_ROOT
    if timing_root.is_dir():
        input_paths.extend(path.relative_to(repo_root) for path in sorted(timing_root.glob("*.json")))
    unique_paths = sorted(set(input_paths), key=lambda path: path.as_posix())
    inputs = [
        {
            "path": path.as_posix(),
            "sha256": _sha256_file(repo_root / path),
        }
        for path in unique_paths
    ]
    fingerprint_source = "".join(f"{entry['path']}\0{entry['sha256']}\n" for entry in inputs)

    return {
        "input_sha256": hashlib.sha256(fingerprint_source.encode("utf-8")).hexdigest(),
        "inputs": inputs,
        "counts": {
            "master_rows": master_rows,
            "accepted_dossiers": len(dossiers),
            "source_anchor_backlog_rows": len(backlog),
            "catalog_works": len(works),
            "discovery_runs": len(discoveries),
            "reviewed_candidates": len(candidates),
            "terminal_receipts": len(receipts),
        },
    }


def _candidate_sort_key(candidate: dict[str, Any]) -> tuple[int, int, str]:
    selection = candidate["selection"]
    return (
        selection["chronology_sort_year"],
        selection["atlas_row_order"],
        candidate["candidate_id"],
    )


def _queue_sha256(payload: dict[str, Any]) -> str:
    without_fingerprint = {key: value for key, value in payload.items() if key != "queue_sha256"}
    return hashlib.sha256(canonical_json(without_fingerprint).encode("utf-8")).hexdigest()


def build_payload(repo_root: Path = REPO_ROOT) -> dict[str, Any]:
    candidates_with_locations = _load_candidates(repo_root)
    candidates_by_id = {
        candidate["candidate_id"]: candidate for candidate, _ in candidates_with_locations
    }
    discoveries = _discovery_records(repo_root)
    receipts, latest_receipts = _load_receipts(
        repo_root,
        candidates_by_id=candidates_by_id,
        discoveries=discoveries,
    )
    snapshot = _snapshot(
        repo_root,
        candidates=candidates_with_locations,
        receipts=receipts,
        discoveries=discoveries,
    )

    queue_entries: list[dict[str, Any]] = []
    for candidate, location in sorted(
        candidates_with_locations,
        key=lambda item: _candidate_sort_key(item[0]),
    ):
        candidate_id = candidate["candidate_id"]
        latest_receipt = latest_receipts.get(candidate_id)
        effective_status = (
            latest_receipt["terminal_status"]
            if latest_receipt is not None
            else candidate["queue_status"]
        )
        queue_entries.append(
            {
                "candidate_id": candidate_id,
                "candidate_kind": candidate["candidate_kind"],
                "preferred_label": candidate["preferred_label"],
                "selection": candidate["selection"],
                "effective_status": effective_status,
                "candidate_source_ref": location,
                "candidate_sha256": candidate_digest(candidate),
                "terminal_receipt_id": (
                    latest_receipt["receipt_id"] if latest_receipt is not None else None
                ),
                "discovery_id": (
                    latest_receipt["discovery_id"] if latest_receipt is not None else None
                ),
            }
        )

    next_candidate_id = next(
        (
            entry["candidate_id"]
            for entry in queue_entries
            if entry["candidate_kind"] in QUEUEABLE_KINDS
            and entry["effective_status"] == READY_STATUS
        ),
        None,
    )
    status_counts: dict[str, int] = {}
    for entry in queue_entries:
        status = entry["effective_status"]
        status_counts[status] = status_counts.get(status, 0) + 1

    payload: dict[str, Any] = {
        "$schema": "https://tree-of-sophia.local/ToS/contracts/open-work-candidate-queue.schema.json",
        "schema_version": "tos_open_work_candidate_queue_v1",
        "owner_surface": LEDGER_PATH.as_posix(),
        "receipt_root": RECEIPT_ROOT.as_posix(),
        "timing_root": TIMING_ROOT.as_posix(),
        "generated_by": "scripts/build_open_work_candidate_queue.py",
        "source_snapshot": snapshot,
        "counts": {
            "candidates": len(queue_entries),
            "by_effective_status": dict(sorted(status_counts.items())),
        },
        "next_candidate_id": next_candidate_id,
        "candidates": queue_entries,
        "authority_boundary": (
            "generated queue navigation over reviewed candidate records and terminal receipts; "
            "not identity, rights, semantic, or canon authority"
        ),
    }
    payload["queue_sha256"] = _queue_sha256(payload)
    return payload


def render_payload(payload: dict[str, Any]) -> str:
    return json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True) + "\n"


def check_output(repo_root: Path, rendered: str) -> list[str]:
    path = repo_root / QUEUE_RELATIVE_PATH
    try:
        current = path.read_text(encoding="utf-8")
    except FileNotFoundError:
        return [f"{QUEUE_RELATIVE_PATH.as_posix()}: generated queue is missing"]
    if current != rendered:
        return [f"{QUEUE_RELATIVE_PATH.as_posix()}: generated queue is stale"]
    return []


def write_output(repo_root: Path, rendered: str) -> None:
    path = repo_root / QUEUE_RELATIVE_PATH
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(rendered, encoding="utf-8")
