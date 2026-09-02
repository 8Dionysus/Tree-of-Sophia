#!/usr/bin/env python3
"""Build the reviewed open-work discovery queue from authored candidate records."""

from __future__ import annotations

import hashlib
import json
import re
from datetime import datetime, timezone
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit


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
TARGET_RESOLUTION_CATALOGS = {
    "work_ref": (Path("ToS/source-witnesses/catalog/works.jsonl"), "work"),
    "expression_ref": (Path("ToS/source-witnesses/catalog/expressions.jsonl"), "expression"),
    "edition_ref": (Path("ToS/source-witnesses/catalog/editions.jsonl"), "edition"),
    "item_ref": (Path("ToS/source-witnesses/catalog/items.jsonl"), "item"),
}


class QueueBuildError(RuntimeError):
    """Raised when authored queue inputs cannot support a deterministic build."""


def canonical_json(payload: object) -> str:
    return json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"))


def candidate_digest(candidate: dict[str, Any]) -> str:
    return hashlib.sha256(canonical_json(candidate).encode("utf-8")).hexdigest()


def target_digest(target: dict[str, Any]) -> str:
    return hashlib.sha256(canonical_json(target).encode("utf-8")).hexdigest()


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
    return [payload for payload, _ in _load_jsonl_with_lines(path, repo_root)]


def _load_jsonl_with_lines(
    path: Path,
    repo_root: Path,
) -> list[tuple[dict[str, Any], int]]:
    relative = path.relative_to(repo_root).as_posix()
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except FileNotFoundError as exc:
        raise QueueBuildError(f"{relative}: required JSONL file is missing") from exc
    except OSError as exc:
        raise QueueBuildError(f"{relative}: cannot read JSONL: {exc}") from exc

    payloads: list[tuple[dict[str, Any], int]] = []
    for line_number, raw_line in enumerate(lines, start=1):
        if not raw_line.strip():
            continue
        try:
            payload = json.loads(raw_line)
        except json.JSONDecodeError as exc:
            raise QueueBuildError(f"{relative}:{line_number}: cannot parse JSON: {exc}") from exc
        if not isinstance(payload, dict):
            raise QueueBuildError(f"{relative}:{line_number}: JSONL record must be an object")
        payloads.append((payload, line_number))
    return payloads


def _required_string(payload: dict[str, Any], key: str, location: str) -> str:
    value = payload.get(key)
    if not isinstance(value, str) or not value:
        raise QueueBuildError(f"{location}: {key} must be a non-empty string")
    return value


def _parse_timestamp(value: Any, *, location: str, field: str) -> datetime:
    if not isinstance(value, str) or not value:
        raise QueueBuildError(f"{location}: {field} must be a non-empty date-time")
    try:
        parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as exc:
        raise QueueBuildError(f"{location}: {field} is not a valid date-time: {value!r}") from exc
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)
    return parsed.astimezone(timezone.utc)


def _load_repo_json_ref(value: str, *, repo_root: Path, location: str) -> tuple[dict[str, Any], Path]:
    path = repo_root / _safe_relative_path(value)
    try:
        payload = _load_json(path, repo_root)
    except QueueBuildError as exc:
        raise QueueBuildError(f"{location}: {exc}") from exc
    return payload, path


def _validate_rights_evidence_refs(
    repo_root: Path,
    rights_result: Any,
    *,
    location: str,
) -> None:
    refs = rights_result.get("evidence_refs") if isinstance(rights_result, dict) else None
    if not isinstance(refs, list) or not refs:
        raise QueueBuildError(f"{location}: rights_result.evidence_refs must be a non-empty array")
    for index, reference in enumerate(refs, start=1):
        ref_location = f"{location}:rights_result.evidence_refs[{index}]"
        if not isinstance(reference, str) or not reference:
            raise QueueBuildError(f"{ref_location}: rights evidence ref must be a non-empty string")
        parsed = urlsplit(reference)
        if parsed.scheme:
            if parsed.scheme not in {"http", "https"} or not parsed.netloc:
                raise QueueBuildError(
                    f"{ref_location}: rights evidence ref must be an http(s) URI with a host"
                )
            continue
        try:
            path = _safe_relative_path(reference)
        except QueueBuildError as exc:
            raise QueueBuildError(
                f"{ref_location}: rights evidence ref must be a repository-relative file or http(s) URI"
            ) from exc
        if not (repo_root / path).is_file():
            raise QueueBuildError(
                f"{ref_location}: rights evidence ref does not resolve: {reference!r}"
            )


def _normalised_words(value: str) -> list[str]:
    value = value.casefold().replace("’", "'")
    return re.findall(r"[\w]+", value, flags=re.UNICODE)


def _contains_word_phrase(haystack: str, needle: str) -> bool:
    haystack_words = _normalised_words(haystack)
    needle_words = _normalised_words(needle)
    if not needle_words:
        return False
    width = len(needle_words)
    return any(
        haystack_words[index : index + width] == needle_words
        for index in range(len(haystack_words) - width + 1)
    )


def _validate_target_binding(
    candidate: dict[str, Any],
    discovery: dict[str, Any],
    *,
    receipt: dict[str, Any],
    location: str,
) -> None:
    candidate_target = candidate.get("target")
    discovery_target = discovery.get("target")
    if not isinstance(candidate_target, dict):
        raise QueueBuildError(f"{location}: candidate target must be an object")
    if not isinstance(discovery_target, dict):
        raise QueueBuildError(f"{location}: discovery target must be an object")

    candidate_kind = candidate_target.get("target_kind")
    discovery_kind = discovery_target.get("target_kind")
    compatible_kinds = {
        "work": {
            "work",
            "expression",
            "edition",
            "item",
            "artifact",
            "scholarly-composite",
            "translation",
            "critical-edition",
            "lexical-resource",
            "research-publication",
        },
        "scholarly-composite": {
            "work",
            "expression",
            "edition",
            "item",
            "artifact",
            "scholarly-composite",
            "translation",
            "critical-edition",
            "research-publication",
        },
    }
    if discovery_kind not in compatible_kinds.get(candidate_kind, set()):
        raise QueueBuildError(
            f"{location}: discovery target_kind {discovery_kind!r} is not compatible with "
            f"candidate target_kind {candidate_kind!r}"
        )

    candidate_refs = candidate_target.get("known_tos_refs")
    discovery_refs = discovery_target.get("known_tos_refs")
    if not isinstance(candidate_refs, list) or not isinstance(discovery_refs, list):
        raise QueueBuildError(f"{location}: candidate and discovery known_tos_refs must be arrays")
    if not all(isinstance(reference, str) and reference for reference in candidate_refs + discovery_refs):
        raise QueueBuildError(f"{location}: candidate and discovery known_tos_refs must contain strings")
    missing_refs = sorted(set(candidate_refs) - set(discovery_refs))
    if missing_refs:
        raise QueueBuildError(
            f"{location}: discovery target drops candidate known_tos_refs: {missing_refs}"
        )

    if not candidate_refs:
        label = candidate.get("preferred_label")
        description = discovery_target.get("description")
        if not isinstance(label, str) or not isinstance(description, str):
            raise QueueBuildError(f"{location}: target binding needs candidate label and discovery description")
        if not _contains_word_phrase(description, label):
            raise QueueBuildError(
                f"{location}: discovery target description does not identify candidate {label!r}"
            )

    for field, target in (
        ("candidate_target_sha256", candidate_target),
        ("discovery_target_sha256", discovery_target),
    ):
        supplied = receipt.get(field)
        if field == "discovery_target_sha256" and not isinstance(supplied, str):
            raise QueueBuildError(f"{location}: {field} is required to freeze the executed discovery target")
        if supplied is not None and supplied != target_digest(target):
            raise QueueBuildError(f"{location}: {field} does not bind the referenced target")


def _validate_target_resolution(
    repo_root: Path,
    target_resolution: Any,
    *,
    location: str,
) -> None:
    if not isinstance(target_resolution, dict):
        raise QueueBuildError(f"{location}: target_resolution must be an object")
    identity_status = target_resolution.get("identity_status")
    if identity_status not in {"unresolved", "provisional", "reconciled"}:
        raise QueueBuildError(
            f"{location}: target_resolution.identity_status is unsupported: {identity_status!r}"
        )

    present_refs = [
        field
        for field in TARGET_RESOLUTION_CATALOGS
        if isinstance(target_resolution.get(field), str) and target_resolution.get(field)
    ]
    if identity_status == "unresolved" and present_refs:
        raise QueueBuildError(
            f"{location}: unresolved target_resolution must not declare identity refs: {present_refs}"
        )
    if identity_status == "reconciled" and not present_refs:
        raise QueueBuildError(
            f"{location}: reconciled target_resolution must declare at least one identity ref"
        )

    for field, (catalog_path, record_type) in TARGET_RESOLUTION_CATALOGS.items():
        reference = target_resolution.get(field)
        if reference is None:
            continue
        if not isinstance(reference, str) or not reference:
            raise QueueBuildError(f"{location}: target_resolution.{field} must be a string or null")
        records = _load_jsonl(repo_root / catalog_path, repo_root)
        matches = [record for record in records if record.get("record_id") == reference]
        if not matches:
            raise QueueBuildError(
                f"{location}: target_resolution.{field} does not resolve canonical {record_type} {reference!r}"
            )
        if len(matches) > 1:
            raise QueueBuildError(
                f"{location}: target_resolution.{field} resolves duplicate canonical {record_type} {reference!r}"
            )
        if matches[0].get("record_type") != record_type:
            raise QueueBuildError(
                f"{location}: target_resolution.{field} resolves a non-{record_type} catalog record"
            )


def _load_provenance_events(repo_root: Path) -> dict[str, tuple[dict[str, Any], str]]:
    events: dict[str, tuple[dict[str, Any], str]] = {}
    source_root = repo_root / "ToS/source-witnesses"
    for path in sorted(source_root.rglob("provenance.jsonl")):
        relative = path.relative_to(repo_root).as_posix()
        try:
            lines = path.read_text(encoding="utf-8").splitlines()
        except OSError as exc:
            raise QueueBuildError(f"{relative}: cannot read provenance JSONL: {exc}") from exc
        for line_number, raw_line in enumerate(lines, start=1):
            if not raw_line.strip():
                continue
            location = f"{relative}:{line_number}"
            try:
                event = json.loads(raw_line)
            except json.JSONDecodeError as exc:
                raise QueueBuildError(f"{location}: cannot parse provenance JSON: {exc}") from exc
            if not isinstance(event, dict):
                raise QueueBuildError(f"{location}: provenance event must be an object")
            event_id = _required_string(event, "event_id", location)
            if event_id in events:
                raise QueueBuildError(
                    f"{location}: duplicate provenance event {event_id!r}; first seen at {events[event_id][1]}"
                )
            events[event_id] = (event, location)
    return events


def _find_unique_record(
    repo_root: Path,
    *,
    root: Path,
    filename: str,
    key: str,
    value: str,
    location: str,
) -> tuple[dict[str, Any], Path]:
    matches: list[tuple[dict[str, Any], Path]] = []
    for path in sorted((repo_root / root).rglob(filename)):
        payload = _load_json(path, repo_root)
        if payload.get(key) == value:
            matches.append((payload, path))
    if not matches:
        raise QueueBuildError(f"{location}: {key} {value!r} has no canonical {filename} record")
    if len(matches) > 1:
        locations = ", ".join(path.relative_to(repo_root).as_posix() for _, path in matches)
        raise QueueBuildError(f"{location}: {key} {value!r} has duplicate canonical records: {locations}")
    return matches[0]


def _validate_planting_refs(
    repo_root: Path,
    refs: Any,
    *,
    candidate: dict[str, Any],
    receipt: dict[str, Any],
    discovery: dict[str, Any],
    discoveries: dict[str, tuple[dict[str, Any], str]],
    acquisitions: list[Any],
    provenance_events: dict[str, tuple[dict[str, Any], str]],
    location: str,
) -> None:
    if not isinstance(refs, list):
        raise QueueBuildError(f"{location}: planting_refs must be an array")

    selection = candidate.get("selection")
    expected_atlas_row = selection.get("atlas_row_id") if isinstance(selection, dict) else None
    if not isinstance(expected_atlas_row, str) or not expected_atlas_row:
        raise QueueBuildError(f"{location}: candidate selection must expose atlas_row_id for planting binding")

    relation_refs = receipt.get("operational_relation_refs")
    if not isinstance(relation_refs, list) or not all(
        isinstance(reference, str) and reference for reference in relation_refs
    ):
        raise QueueBuildError(f"{location}: operational_relation_refs must be non-empty strings")
    receipt_context_refs = set(relation_refs) | {
        receipt.get("discovery_ref"),
        receipt.get("discovery_id"),
    }
    receipt_context_refs.discard(None)

    acquired_ids: set[str] = set()
    acquisition_context_refs: set[str] = set()
    for acquisition in acquisitions:
        if not isinstance(acquisition, dict) or acquisition.get("downloaded") is not True:
            continue
        for key in (
            "item_ref",
            "artifact_ref",
            "composite_ref",
            "representation_ref",
            "file_ref",
            "provenance_event_ref",
        ):
            value = acquisition.get(key)
            if isinstance(value, str) and value:
                acquisition_context_refs.add(value)
                if key in {"item_ref", "artifact_ref", "composite_ref"}:
                    acquired_ids.add(value)
        event_ref = acquisition.get("provenance_event_ref")
        event_entry = provenance_events.get(event_ref)
        if event_entry is not None:
            event = event_entry[0]
            for field in ("inputs", "outputs"):
                rows = event.get(field, [])
                if isinstance(rows, list):
                    acquisition_context_refs.update(
                        row.get("ref")
                        for row in rows
                        if isinstance(row, dict)
                        and isinstance(row.get("ref"), str)
                        and row.get("ref")
                    )

    discovery_target = discovery.get("target")
    discovery_target_refs = (
        set(discovery_target.get("known_tos_refs", []))
        if isinstance(discovery_target, dict)
        and isinstance(discovery_target.get("known_tos_refs"), list)
        and all(
            isinstance(reference, str) and reference
            for reference in discovery_target["known_tos_refs"]
        )
        else set()
    )
    supported_ids = discovery_target_refs | acquired_ids | {
        reference for reference in relation_refs if reference.startswith("tos.")
    }

    def resolve_discovery(ref: str, ref_location: str) -> tuple[dict[str, Any], str]:
        for discovery_id, (payload, discovery_location) in discoveries.items():
            if discovery_location == ref:
                return payload, discovery_id
        raise QueueBuildError(f"{ref_location}: planting discovery_ref does not resolve: {ref!r}")

    for index, ref in enumerate(refs, start=1):
        ref_location = f"{location}:planting_refs[{index}]"
        if not isinstance(ref, str) or not ref:
            raise QueueBuildError(f"{ref_location}: planting ref must be a non-empty repository-relative path")
        payload, _ = _load_repo_json_ref(ref, repo_root=repo_root, location=ref_location)
        if not isinstance(payload.get("planting_id"), str) or not payload["planting_id"]:
            raise QueueBuildError(f"{ref_location}: planting record must expose planting_id")

        if payload.get("atlas_row_id") != expected_atlas_row or payload.get("dossier_id") != expected_atlas_row:
            raise QueueBuildError(
                f"{ref_location}: planting atlas/dossier scope does not bind candidate atlas row {expected_atlas_row!r}"
            )

        source_witness = payload.get("source_witness")
        if not isinstance(source_witness, dict):
            raise QueueBuildError(f"{ref_location}: planting source_witness must be an object")
        identity_fields = [
            key
            for key in ("artifact_id", "composite_id", "work_id")
            if isinstance(source_witness.get(key), str) and source_witness.get(key)
        ]
        if len(identity_fields) != 1:
            raise QueueBuildError(
                f"{ref_location}: planting source_witness must identify exactly one source identity"
            )
        identity_field = identity_fields[0]
        source_id = source_witness[identity_field]
        source_record_ref = source_witness.get("record_ref")
        if not isinstance(source_record_ref, str) or not source_record_ref:
            raise QueueBuildError(f"{ref_location}: planting source_witness must bind record_ref")
        source_record, _ = _load_repo_json_ref(
            source_record_ref,
            repo_root=repo_root,
            location=ref_location,
        )
        record_key = "record_id" if identity_field == "work_id" else identity_field
        if source_record.get(record_key) != source_id:
            raise QueueBuildError(
                f"{ref_location}: planting source_witness {identity_field} disagrees with record_ref"
            )
        expected_record_root = {
            "artifact_id": "ToS/source-witnesses/artifacts/",
            "composite_id": "ToS/source-witnesses/scholarly-composites/",
            "work_id": "ToS/source-witnesses/works/",
        }[identity_field]
        if not source_record_ref.startswith(expected_record_root):
            raise QueueBuildError(
                f"{ref_location}: planting source_witness {identity_field} record_ref must stay under {expected_record_root}"
            )
        if (
            source_id not in supported_ids
            and source_record_ref not in relation_refs
            and source_record_ref not in acquisition_context_refs
        ):
            raise QueueBuildError(
                f"{ref_location}: planting source_witness {source_id!r} is not bound to the receipt discovery or acquisition records"
            )

        planting_discovery_ref = payload.get("discovery_ref")
        if not isinstance(planting_discovery_ref, str) or not planting_discovery_ref:
            raise QueueBuildError(
                f"{ref_location}: planting discovery_ref must be a non-empty repository-relative path"
            )
        planting_discovery, planting_discovery_id = resolve_discovery(
            planting_discovery_ref,
            ref_location,
        )
        planting_target = planting_discovery.get("target")
        planting_target_refs = (
            set(planting_target.get("known_tos_refs", []))
            if isinstance(planting_target, dict)
            and isinstance(planting_target.get("known_tos_refs"), list)
            else set()
        )
        discovery_context = {planting_discovery_ref, planting_discovery_id}
        if not (
            discovery_context & receipt_context_refs
            or source_record_ref in receipt_context_refs
            or source_id in receipt_context_refs
            or source_record_ref in acquisition_context_refs
            or source_id in acquisition_context_refs
        ):
            raise QueueBuildError(
                f"{ref_location}: planting discovery_ref {planting_discovery_ref!r} is not bound to the receipt records"
            )
        if planting_target_refs and source_id not in planting_target_refs:
            raise QueueBuildError(
                f"{ref_location}: planting discovery target does not identify source_witness {source_id!r}"
            )

        planting_event_ref = payload.get("provenance_event_ref")
        if not isinstance(planting_event_ref, str) or not planting_event_ref:
            raise QueueBuildError(
                f"{ref_location}: planting provenance_event_ref must be a non-empty event ID"
            )
        event_entry = provenance_events.get(planting_event_ref)
        if event_entry is None:
            raise QueueBuildError(
                f"{ref_location}: planting provenance event {planting_event_ref!r} does not resolve"
            )
        event = event_entry[0]
        event_inputs = event.get("inputs", [])
        event_outputs = event.get("outputs", [])
        if not isinstance(event_inputs, list) or not isinstance(event_outputs, list):
            raise QueueBuildError(
                f"{ref_location}: planting provenance event inputs and outputs must be arrays"
            )
        output_refs = {
            output.get("ref")
            for output in event_outputs
            if isinstance(output, dict) and isinstance(output.get("ref"), str)
        }
        input_refs = {
            input_ref.get("ref")
            for input_ref in event_inputs
            if isinstance(input_ref, dict) and isinstance(input_ref.get("ref"), str)
        }
        if ref not in output_refs:
            raise QueueBuildError(
                f"{ref_location}: planting provenance event does not output the planting record"
            )
        if source_record_ref not in output_refs | input_refs and source_id not in output_refs | input_refs:
            raise QueueBuildError(
                f"{ref_location}: planting provenance event does not bind source_witness {source_id!r}"
            )
        if not (
            planting_event_ref in receipt_context_refs
            or source_record_ref in receipt_context_refs
            or source_id in receipt_context_refs
            or source_record_ref in acquisition_context_refs
            or source_id in acquisition_context_refs
        ):
            raise QueueBuildError(
                f"{ref_location}: planting provenance_event_ref {planting_event_ref!r} is not bound to the receipt records"
            )


def _validate_acquisition_closure(
    repo_root: Path,
    acquisition: Any,
    *,
    candidate_id: str,
    discoveries: dict[str, tuple[dict[str, Any], str]],
    receipt_context_refs: set[str],
    receipt_discovery_id: str | None,
    receipt_discovery_ref: str | None,
    provenance_events: dict[str, tuple[dict[str, Any], str]],
    location: str,
) -> None:
    if not isinstance(acquisition, dict):
        raise QueueBuildError(f"{location}: acquisition must be an object")
    if acquisition.get("downloaded") is not True:
        for key in (
            "item_ref",
            "artifact_ref",
            "composite_ref",
            "representation_ref",
            "file_ref",
            "provenance_event_ref",
        ):
            if acquisition.get(key) is not None:
                raise QueueBuildError(f"{location}: non-downloaded acquisition must null {key}")
        return

    identity_fields = [
        key
        for key in ("item_ref", "artifact_ref", "composite_ref")
        if isinstance(acquisition.get(key), str) and acquisition.get(key)
    ]
    if len(identity_fields) != 1:
        raise QueueBuildError(
            f"{location}: downloaded acquisition must identify exactly one of item_ref, artifact_ref, composite_ref"
        )
    identity_field = identity_fields[0]
    file_ref = acquisition.get("file_ref")
    event_ref = acquisition.get("provenance_event_ref")
    if not isinstance(file_ref, str) or not file_ref:
        raise QueueBuildError(f"{location}: downloaded acquisition must bind file_ref")
    if not isinstance(event_ref, str) or not event_ref:
        raise QueueBuildError(f"{location}: downloaded acquisition must bind provenance_event_ref")
    event_entry = provenance_events.get(event_ref)
    if event_entry is None:
        raise QueueBuildError(f"{location}: provenance event {event_ref!r} does not resolve")
    event, event_location = event_entry
    if event.get("event_type") != "acquisition":
        raise QueueBuildError(f"{location}: {event_ref!r} is not an acquisition event")
    configuration = event.get("method", {}).get("configuration", {})
    configured_candidate = None
    if isinstance(configuration, dict):
        configured_candidate = configuration.get("candidate_id")
        if configured_candidate is not None and configured_candidate != candidate_id:
            raise QueueBuildError(
                f"{location}: provenance event {event_ref!r} belongs to {configured_candidate!r}, not {candidate_id!r}"
            )
    outputs = event.get("outputs")
    if not isinstance(outputs, list):
        raise QueueBuildError(f"{event_location}: acquisition event outputs must be an array")
    output_refs = {
        output.get("ref")
        for output in outputs
        if isinstance(output, dict) and isinstance(output.get("ref"), str)
    }
    if identity_field == "item_ref":
        if acquisition.get("representation_ref") is not None:
            raise QueueBuildError(f"{location}: item acquisition must not carry representation_ref")
        item_ref = acquisition[identity_field]
        if event_ref not in receipt_context_refs:
            raise QueueBuildError(
                f"{location}: item acquisition provenance_event_ref {event_ref!r} is not bound to the receipt route"
            )
        item, item_path = _find_unique_record(
            repo_root,
            root=Path("ToS/source-witnesses"),
            filename="item.manifest.json",
            key="item_id",
            value=acquisition[identity_field],
            location=location,
        )
        item_record_ref = item_path.relative_to(repo_root).as_posix()
        if not {item_ref, item_record_ref} & receipt_context_refs:
            raise QueueBuildError(
                f"{location}: item acquisition item_ref {item_ref!r} is not bound to the receipt route"
            )
        files = item.get("payload_files")
        if not isinstance(files, list) or not any(
            isinstance(payload_file, dict) and payload_file.get("file_id") == file_ref
            for payload_file in files
        ):
            raise QueueBuildError(f"{location}: item manifest does not bind file_ref {file_ref!r}")
        if file_ref not in output_refs:
            raise QueueBuildError(f"{location}: acquisition event does not output file_ref {file_ref!r}")
        return

    representation_ref = acquisition.get("representation_ref")
    if not isinstance(representation_ref, str) or not representation_ref:
        raise QueueBuildError(f"{location}: artifact/composite acquisition must bind representation_ref")
    representation, representation_path = _load_repo_json_ref(
        representation_ref,
        repo_root=repo_root,
        location=location,
    )
    if representation.get("file_id") != file_ref:
        raise QueueBuildError(f"{location}: representation does not bind file_ref {file_ref!r}")
    payload = representation.get("payload")
    if not isinstance(payload, dict) or payload.get("sha256") != file_ref.removeprefix("tos.file.sha256."):
        raise QueueBuildError(f"{location}: representation payload fixity does not bind file_ref {file_ref!r}")
    if representation_ref not in output_refs:
        raise QueueBuildError(
            f"{location}: acquisition event does not output representation_ref {representation_ref!r}"
        )

    if identity_field == "artifact_ref":
        artifact, artifact_path = _find_unique_record(
            repo_root,
            root=Path("ToS/source-witnesses/artifacts"),
            filename="artifact-witness.json",
            key="artifact_id",
            value=acquisition[identity_field],
            location=location,
        )
        if representation.get("artifact_id") != artifact["artifact_id"]:
            raise QueueBuildError(f"{location}: representation does not bind artifact_ref {acquisition[identity_field]!r}")
        if representation.get("artifact_ref") != artifact_path.relative_to(repo_root).as_posix():
            raise QueueBuildError(f"{location}: representation artifact_ref path disagrees with canonical artifact")
    else:
        composite, composite_path = _find_unique_record(
            repo_root,
            root=Path("ToS/source-witnesses/scholarly-composites"),
            filename="composite-witness.json",
            key="composite_id",
            value=acquisition[identity_field],
            location=location,
        )
        if representation.get("composite_id") != composite["composite_id"]:
            raise QueueBuildError(f"{location}: representation does not bind composite_ref {acquisition[identity_field]!r}")
        if representation.get("composite_ref") != composite_path.relative_to(repo_root).as_posix():
            raise QueueBuildError(f"{location}: representation composite_ref path disagrees with canonical composite")

    representation_discovery_ref = representation.get("discovery_ref")
    if not isinstance(representation_discovery_ref, str) or not representation_discovery_ref:
        raise QueueBuildError(f"{location}: representation must bind discovery_ref")
    representation_discovery_id = next(
        (
            discovery_id
            for discovery_id, (_, discovery_location) in discoveries.items()
            if discovery_location == representation_discovery_ref
        ),
        None,
    )
    if representation_discovery_id is None:
        raise QueueBuildError(
            f"{location}: representation discovery_ref does not resolve: {representation_discovery_ref!r}"
        )
    representation_event_ref = representation.get("provenance_event_ref")
    if representation_event_ref != event_ref:
        raise QueueBuildError(
            f"{location}: representation provenance_event_ref does not bind acquisition event {event_ref!r}"
        )
    route_bound = (
        representation_discovery_ref == receipt_discovery_ref
        and representation_discovery_id == receipt_discovery_id
    )
    if not route_bound and configured_candidate != candidate_id:
        raise QueueBuildError(
            f"{location}: representation discovery_ref {representation_discovery_ref!r} is not bound "
            f"to receipt discovery {receipt_discovery_ref!r} ({receipt_discovery_id!r}) "
            "or an explicit candidate binding"
        )


def _validate_receipt_acquisition_closure(
    repo_root: Path,
    receipt: dict[str, Any],
    *,
    candidate: dict[str, Any],
    discovery: dict[str, Any],
    discoveries: dict[str, tuple[dict[str, Any], str]],
    provenance_events: dict[str, tuple[dict[str, Any], str]],
    location: str,
) -> None:
    candidate_id = _required_string(receipt, "candidate_id", location)
    relation_refs = receipt.get("operational_relation_refs")
    if not isinstance(relation_refs, list) or not all(
        isinstance(reference, str) and reference for reference in relation_refs
    ):
        raise QueueBuildError(f"{location}: operational_relation_refs must be non-empty strings")
    receipt_context_refs = set(relation_refs) | {
        receipt.get("discovery_ref"),
        receipt.get("discovery_id"),
    }
    receipt_context_refs.discard(None)
    acquisitions = [receipt.get("acquisition")]
    additional = receipt.get("additional_acquisitions", [])
    if additional is not None:
        if not isinstance(additional, list):
            raise QueueBuildError(f"{location}: additional_acquisitions must be an array")
        acquisitions.extend(additional)
    rights_result = receipt.get("rights_result")
    rights_status = rights_result.get("status") if isinstance(rights_result, dict) else None
    downloaded = any(
        isinstance(acquisition, dict) and acquisition.get("downloaded") is True
        for acquisition in acquisitions
    )
    if downloaded and rights_status != "positive-for-acquisition":
        raise QueueBuildError(
            f"{location}: downloaded acquisition requires rights_result.status "
            f"positive-for-acquisition, got {rights_status!r}"
        )
    _validate_rights_evidence_refs(repo_root, rights_result, location=location)
    for index, acquisition in enumerate(acquisitions, start=1):
        _validate_acquisition_closure(
            repo_root,
            acquisition,
            candidate_id=candidate_id,
            discoveries=discoveries,
            receipt_context_refs=receipt_context_refs,
            receipt_discovery_id=receipt.get("discovery_id"),
            receipt_discovery_ref=receipt.get("discovery_ref"),
            provenance_events=provenance_events,
            location=f"{location}:acquisition[{index}]",
        )
    _validate_planting_refs(
        repo_root,
        receipt.get("planting_refs"),
        candidate=candidate,
        receipt=receipt,
        discovery=discovery,
        discoveries=discoveries,
        acquisitions=acquisitions,
        provenance_events=provenance_events,
        location=location,
    )
    if receipt.get("terminal_status") == "held_source_witness" and not downloaded and not receipt.get(
        "planting_refs"
    ):
        raise QueueBuildError(
            f"{location}: held_source_witness requires a resolved downloaded acquisition "
            "or an explicitly resolved pre-existing witness planting"
        )


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
    selection = candidate.get("selection")
    expected_atlas_row = selection.get("atlas_row_id") if isinstance(selection, dict) else None
    if not isinstance(expected_atlas_row, str) or not expected_atlas_row:
        raise QueueBuildError(f"{location}: candidate selection must expose atlas_row_id")
    for index, source_ref in enumerate(refs, start=1):
        ref_location = f"{location}:source_refs[{index}]"
        if not isinstance(source_ref, dict):
            raise QueueBuildError(f"{ref_location}: source ref must be an object")
        source_path_value = _required_string(source_ref, "source_path", ref_location)
        selector = source_ref.get("selector")
        if not isinstance(selector, dict) or not selector:
            raise QueueBuildError(f"{ref_location}: selector must be a non-empty object")
        source_path = repo_root / _safe_relative_path(source_path_value)
        records = (
            [_load_json(source_path, repo_root)]
            if source_path.suffix == ".json"
            else _load_jsonl(source_path, repo_root)
        )
        if not any(_selector_matches(record, selector) for record in records):
            raise QueueBuildError(
                f"{ref_location}: selector does not resolve in {source_path_value}: {selector}"
            )
        for selector_key in ("row_id", "dossier_id"):
            selected_row = selector.get(selector_key)
            if selected_row is not None and selected_row != expected_atlas_row:
                raise QueueBuildError(
                    f"{ref_location}: {selector_key} selector {selected_row!r} "
                    f"does not bind candidate selection atlas_row_id {expected_atlas_row!r}"
                )


def _load_candidates(repo_root: Path) -> list[tuple[dict[str, Any], str]]:
    ledger = repo_root / LEDGER_PATH
    candidates = _load_jsonl_with_lines(ledger, repo_root)
    seen: dict[str, str] = {}
    loaded: list[tuple[dict[str, Any], str]] = []
    for candidate, line_number in candidates:
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


def _index_unique_channel_rows(
    rows: list[Any],
    *,
    field: str,
    location: str,
) -> dict[str, dict[str, Any]]:
    indexed: dict[str, dict[str, Any]] = {}
    first_locations: dict[str, str] = {}
    for index, row in enumerate(rows, start=1):
        row_location = f"{location}[{index}]"
        if not isinstance(row, dict):
            raise QueueBuildError(f"{row_location}: row must be an object")
        channel_id = row.get(field)
        if not isinstance(channel_id, str) or not channel_id:
            raise QueueBuildError(f"{row_location}: {field} must be a non-empty string")
        if channel_id in indexed:
            raise QueueBuildError(
                f"{row_location}: duplicate {field} {channel_id!r}; "
                f"first seen at {first_locations[channel_id]}"
            )
        indexed[channel_id] = row
        first_locations[channel_id] = row_location
    return indexed


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
    discovery_started_at = _parse_timestamp(
        discovery.get("started_at"),
        location=location,
        field="discovery.started_at",
    )
    discovery_ended_at = _parse_timestamp(
        discovery.get("ended_at"),
        location=location,
        field="discovery.ended_at",
    )
    if discovery_ended_at < discovery_started_at:
        raise QueueBuildError(f"{location}: discovery ended_at precedes started_at")
    measurement_rows = timing.get("measurements")
    if not isinstance(measurement_rows, list) or not measurement_rows:
        raise QueueBuildError(f"{location}: timing receipt must contain measurements")
    measurement_rows_by_id = _index_unique_channel_rows(
        measurement_rows,
        field="channel_id",
        location=f"{location}:measurements",
    )
    comparison_by_id = _index_unique_channel_rows(
        comparisons,
        field="channel_id",
        location=f"{location}:channel_comparison",
    )
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
        measurement_row = measurement_rows_by_id.get(channel_id)
        measurement = measurement_row.get("measurement") if measurement_row else None
        if not isinstance(measurement, dict):
            raise QueueBuildError(
                f"{channel_location}: active queue discovery requires an external timing measurement"
            )
        if measurement.get("probe_url") != channel.get("endpoint_url"):
            raise QueueBuildError(
                f"{channel_location}: timing measurement must bind the channel endpoint_url"
            )
        if measurement.get("method") != TIMING_METHOD:
            raise QueueBuildError(
                f"{channel_location}: timing_measurement.method must be {TIMING_METHOD!r}"
            )
        if measurement.get("clock") != TIMING_CLOCK:
            raise QueueBuildError(
                f"{channel_location}: timing_measurement.clock must be {TIMING_CLOCK!r}"
            )
        queried_at = _parse_timestamp(
            channel.get("queried_at"),
            location=channel_location,
            field="queried_at",
        )
        measurement_started_at = _parse_timestamp(
            measurement.get("started_at"),
            location=channel_location,
            field="timing_measurement.started_at",
        )
        measurement_ended_at = _parse_timestamp(
            measurement.get("ended_at"),
            location=channel_location,
            field="timing_measurement.ended_at",
        )
        if measurement_started_at != queried_at:
            raise QueueBuildError(
                f"{channel_location}: timing_measurement.started_at must equal channel queried_at"
            )
        if measurement_ended_at < measurement_started_at:
            raise QueueBuildError(f"{channel_location}: timing_measurement ended_at precedes started_at")
        if measurement_started_at < discovery_started_at or measurement_ended_at > discovery_ended_at:
            raise QueueBuildError(
                f"{channel_location}: timing measurement interval must stay within discovery interval"
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
    if set(measurement_rows_by_id) != channel_ids:
        raise QueueBuildError(f"{location}: timing receipt must cover the exact active channel set")


def _load_receipts(
    repo_root: Path,
    *,
    candidates_by_id: dict[str, dict[str, Any]],
    discoveries: dict[str, tuple[dict[str, Any], str]],
) -> tuple[list[dict[str, Any]], dict[str, dict[str, Any]]]:
    receipt_root = repo_root / RECEIPT_ROOT
    receipt_paths = sorted(receipt_root.glob("*.json")) if receipt_root.is_dir() else []
    provenance_events = _load_provenance_events(repo_root)
    receipts: list[dict[str, Any]] = []
    latest_by_candidate: dict[str, dict[str, Any]] = {}
    seen_receipt_ids: set[str] = set()
    seen_versions: dict[str, set[int]] = {}

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
        _validate_target_binding(
            candidate,
            discovery[0],
            receipt=receipt,
            location=location,
        )
        _validate_target_resolution(
            repo_root,
            receipt.get("target_resolution"),
            location=location,
        )
        _validate_receipt_acquisition_closure(
            repo_root,
            receipt,
            candidate=candidate,
            discovery=discovery[0],
            discoveries=discoveries,
            provenance_events=provenance_events,
            location=location,
        )
        version = receipt.get("record_version")
        if not isinstance(version, int) or version < 1:
            raise QueueBuildError(f"{location}: record_version must be a positive integer")
        if version in seen_versions.setdefault(candidate_id, set()):
            raise QueueBuildError(
                f"{location}: duplicate terminal receipt version {version} for {candidate_id!r}"
            )
        seen_versions[candidate_id].add(version)
        previous = latest_by_candidate.get(candidate_id)
        if previous is None or version > previous["record_version"]:
            latest_by_candidate[candidate_id] = receipt
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


def _snapshot_path_from_ref(
    repo_root: Path,
    value: Any,
    *,
    location: str,
) -> Path | None:
    if not isinstance(value, str) or not value:
        return None
    if urlsplit(value).scheme:
        return None
    path = _safe_relative_path(value)
    if not (repo_root / path).is_file():
        raise QueueBuildError(f"{location}: snapshot input does not resolve: {value!r}")
    return path


def _snapshot_closure_inputs(
    repo_root: Path,
    *,
    receipts: list[dict[str, Any]],
    discoveries: dict[str, tuple[dict[str, Any], str]],
    provenance_events: dict[str, tuple[dict[str, Any], str]] | None,
) -> list[Path]:
    paths: list[Path] = []
    source_root = repo_root / "ToS/source-witnesses"
    if source_root.is_dir():
        paths.extend(
            path.relative_to(repo_root)
            for path in sorted(source_root.rglob("provenance.jsonl"))
        )
    if provenance_events is not None:
        for discovery, _ in discoveries.values():
            event_refs = discovery.get("provenance_event_refs", [])
            if not isinstance(event_refs, list):
                continue
            for event_ref in event_refs:
                event_entry = provenance_events.get(event_ref)
                if event_entry is None:
                    continue
                event = event_entry[0]
                method = event.get("method")
                if isinstance(method, dict):
                    prompt_path = _snapshot_path_from_ref(
                        repo_root,
                        method.get("prompt_or_instruction_ref"),
                        location=f"snapshot provenance event {event_ref}:prompt_or_instruction_ref",
                    )
                    if prompt_path is not None and prompt_path.as_posix().startswith("ToS/research-packets/"):
                        paths.append(prompt_path)
                outputs = event.get("outputs", [])
                if not isinstance(outputs, list):
                    continue
                for output in outputs:
                    if not isinstance(output, dict):
                        continue
                    output_path = output.get("ref")
                    if not isinstance(output_path, str) or not output_path.startswith("ToS/research-packets/"):
                        continue
                    path = _snapshot_path_from_ref(
                        repo_root,
                        output_path,
                        location=f"snapshot provenance event {event_ref}:output",
                    )
                    if path is not None:
                        paths.append(path)

    canonical_records = {
        "item_ref": (Path("ToS/source-witnesses"), "item.manifest.json", "item_id"),
        "artifact_ref": (Path("ToS/source-witnesses/artifacts"), "artifact-witness.json", "artifact_id"),
        "composite_ref": (
            Path("ToS/source-witnesses/scholarly-composites"),
            "composite-witness.json",
            "composite_id",
        ),
    }

    for receipt in receipts:
        receipt_id = receipt.get("receipt_id", "receipt")
        location = f"snapshot closure {receipt_id}"
        rights_result = receipt.get("rights_result")
        evidence_refs = rights_result.get("evidence_refs", []) if isinstance(rights_result, dict) else []
        if isinstance(evidence_refs, list):
            for index, reference in enumerate(evidence_refs, start=1):
                path = _snapshot_path_from_ref(
                    repo_root,
                    reference,
                    location=f"{location}:rights_result.evidence_refs[{index}]",
                )
                if path is not None:
                    paths.append(path)

        planting_refs = receipt.get("planting_refs", [])
        if isinstance(planting_refs, list):
            for index, reference in enumerate(planting_refs, start=1):
                ref_location = f"{location}:planting_refs[{index}]"
                planting_path = _snapshot_path_from_ref(
                    repo_root,
                    reference,
                    location=ref_location,
                )
                if planting_path is None:
                    continue
                paths.append(planting_path)
                planting = _load_json(repo_root / planting_path, repo_root)
                source_witness = planting.get("source_witness")
                if isinstance(source_witness, dict):
                    source_record = _snapshot_path_from_ref(
                        repo_root,
                        source_witness.get("record_ref"),
                        location=f"{ref_location}:source_witness.record_ref",
                    )
                    if source_record is not None:
                        paths.append(source_record)
                planting_discovery = _snapshot_path_from_ref(
                    repo_root,
                    planting.get("discovery_ref"),
                    location=f"{ref_location}:discovery_ref",
                )
                if planting_discovery is not None:
                    paths.append(planting_discovery)

        acquisitions: list[Any] = [receipt.get("acquisition")]
        additional_acquisitions = receipt.get("additional_acquisitions", [])
        if isinstance(additional_acquisitions, list):
            acquisitions.extend(additional_acquisitions)
        for index, acquisition in enumerate(acquisitions, start=1):
            if not isinstance(acquisition, dict) or acquisition.get("downloaded") is not True:
                continue
            acquisition_location = f"{location}:acquisition[{index}]"
            representation = _snapshot_path_from_ref(
                repo_root,
                acquisition.get("representation_ref"),
                location=f"{acquisition_location}:representation_ref",
            )
            if representation is not None:
                paths.append(representation)
            for field, (record_root, filename, record_key) in canonical_records.items():
                identity = acquisition.get(field)
                if not isinstance(identity, str) or not identity:
                    continue
                _, record_path = _find_unique_record(
                    repo_root,
                    root=record_root,
                    filename=filename,
                    key=record_key,
                    value=identity,
                    location=acquisition_location,
                )
                paths.append(record_path.relative_to(repo_root))

    return paths


def _snapshot(
    repo_root: Path,
    *,
    candidates: list[tuple[dict[str, Any], str]],
    receipts: list[dict[str, Any]],
    discoveries: dict[str, tuple[dict[str, Any], str]],
    provenance_events: dict[str, tuple[dict[str, Any], str]] | None = None,
    excluded_paths: set[Path] | None = None,
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
    input_paths.extend(
        _safe_relative_path(source_ref["source_path"])
        for candidate, _ in candidates
        for source_ref in candidate["source_refs"]
    )
    input_paths.extend(
        _snapshot_closure_inputs(
            repo_root,
            receipts=receipts,
            discoveries=discoveries,
            provenance_events=provenance_events,
        )
    )
    receipt_root = repo_root / RECEIPT_ROOT
    if receipt_root.is_dir():
        input_paths.extend(path.relative_to(repo_root) for path in sorted(receipt_root.glob("*.json")))
    timing_root = repo_root / TIMING_ROOT
    if timing_root.is_dir():
        input_paths.extend(path.relative_to(repo_root) for path in sorted(timing_root.glob("*.json")))
    excluded = {path.as_posix() for path in (excluded_paths or set())}
    unique_paths = sorted(
        {path for path in input_paths if path.as_posix() not in excluded},
        key=lambda path: path.as_posix(),
    )
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


def _candidate_source_line(location: str) -> int:
    try:
        return int(location.rsplit(":", 1)[1])
    except (IndexError, ValueError) as exc:
        raise QueueBuildError(f"candidate source location has no physical line number: {location!r}") from exc


def _candidate_sort_key(
    candidate: dict[str, Any],
    *,
    source_line: int | None = None,
) -> tuple[int, int, int, str]:
    selection = candidate["selection"]
    return (
        selection["chronology_sort_year"],
        selection["atlas_row_order"],
        source_line if source_line is not None else 0,
        candidate["candidate_id"],
    )


def _queue_sha256(payload: dict[str, Any]) -> str:
    without_fingerprint = {key: value for key, value in payload.items() if key != "queue_sha256"}
    return hashlib.sha256(canonical_json(without_fingerprint).encode("utf-8")).hexdigest()


def _receipt_sort_key(receipt: dict[str, Any]) -> tuple[datetime, int, str]:
    issued_at = receipt.get("issued_at")
    if not isinstance(issued_at, str) or not issued_at:
        raise QueueBuildError("receipt issued_at must be a non-empty date-time")
    parsed = _parse_timestamp(issued_at, location="receipt", field="issued_at")
    version = receipt.get("record_version")
    if not isinstance(version, int) or version < 1:
        raise QueueBuildError("receipt record_version must be a positive integer")
    receipt_id = _required_string(receipt, "receipt_id", "receipt")
    return parsed, version, receipt_id


def _discovery_started_at(
    discovery: tuple[dict[str, Any], str],
) -> datetime:
    payload, location = discovery
    return _parse_timestamp(payload.get("started_at"), location=location, field="started_at")


def _validate_receipt_version_timestamp_order(receipts: list[dict[str, Any]]) -> None:
    by_candidate: dict[str, list[tuple[int, datetime, str]]] = {}
    seen_versions: dict[str, set[int]] = {}
    for receipt in receipts:
        receipt_id = _required_string(receipt, "receipt_id", "receipt")
        candidate_id = _required_string(receipt, "candidate_id", receipt_id)
        issued_at, version, _ = _receipt_sort_key(receipt)
        if version in seen_versions.setdefault(candidate_id, set()):
            raise QueueBuildError(
                f"{receipt_id}: duplicate record_version {version} for candidate {candidate_id!r}"
            )
        seen_versions[candidate_id].add(version)
        by_candidate.setdefault(candidate_id, []).append((version, issued_at, receipt_id))

    for candidate_id, entries in by_candidate.items():
        entries.sort(key=lambda entry: entry[0])
        previous_version: int | None = None
        previous_issued_at: datetime | None = None
        for version, issued_at, receipt_id in entries:
            if previous_issued_at is not None and issued_at < previous_issued_at:
                raise QueueBuildError(
                    f"{receipt_id}: record_version {version} issued_at {issued_at.isoformat()} "
                    f"is earlier than predecessor record_version {previous_version} for candidate {candidate_id!r}"
                )
            previous_version = version
            previous_issued_at = issued_at


def _build_queue_payload(
    repo_root: Path,
    *,
    candidates_with_locations: list[tuple[dict[str, Any], str]],
    receipts: list[dict[str, Any]],
    discoveries: dict[str, tuple[dict[str, Any], str]],
    provenance_events: dict[str, tuple[dict[str, Any], str]] | None = None,
    excluded_paths: set[Path] | None = None,
) -> dict[str, Any]:
    latest_receipts: dict[str, dict[str, Any]] = {}
    for receipt in receipts:
        candidate_id = _required_string(receipt, "candidate_id", "receipt")
        previous = latest_receipts.get(candidate_id)
        if previous is None or receipt["record_version"] > previous["record_version"]:
            latest_receipts[candidate_id] = receipt

    snapshot = _snapshot(
        repo_root,
        candidates=candidates_with_locations,
        receipts=receipts,
        discoveries=discoveries,
        provenance_events=provenance_events,
        excluded_paths=excluded_paths,
    )
    queue_entries: list[dict[str, Any]] = []
    for candidate, location in sorted(
        candidates_with_locations,
        key=lambda item: _candidate_sort_key(
            item[0], source_line=_candidate_source_line(item[1])
        ),
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


def _reconstruct_pre_run_queue_sha256(
    repo_root: Path,
    *,
    current_receipt: dict[str, Any],
    ordered_receipts: list[dict[str, Any]],
    candidates_with_locations: list[tuple[dict[str, Any], str]],
    discoveries: dict[str, tuple[dict[str, Any], str]],
    provenance_events: dict[str, tuple[dict[str, Any], str]] | None = None,
) -> str:
    current_key = _receipt_sort_key(current_receipt)
    prior_receipts = [
        receipt
        for receipt in ordered_receipts
        if _receipt_sort_key(receipt) < current_key
    ]
    future_receipts = [
        receipt
        for receipt in ordered_receipts
        if _receipt_sort_key(receipt) >= current_key
    ]
    future_discovery_ids = {receipt.get("discovery_id") for receipt in future_receipts}
    pre_run_discoveries = {}
    for discovery_id, entry in discoveries.items():
        if discovery_id in future_discovery_ids:
            continue
        if _discovery_started_at(entry) <= current_key[0]:
            pre_run_discoveries[discovery_id] = entry
    excluded_paths: set[Path] = set()
    future_receipt_ids = {receipt["receipt_id"] for receipt in future_receipts}
    receipt_root = repo_root / RECEIPT_ROOT
    if receipt_root.is_dir():
        for path in sorted(receipt_root.glob("*.json")):
            receipt = _load_json(path, repo_root)
            if receipt.get("receipt_id") in future_receipt_ids:
                excluded_paths.add(path.relative_to(repo_root))
    for receipt in future_receipts:
        for key in ("discovery_ref", "timing_ref"):
            value = receipt.get(key)
            if isinstance(value, str) and value:
                excluded_paths.add(Path(value))
    prior_timing_refs = {
        _safe_relative_path(receipt["timing_ref"])
        for receipt in prior_receipts
        if isinstance(receipt.get("timing_ref"), str) and receipt.get("timing_ref")
    }
    timing_root = repo_root / TIMING_ROOT
    if timing_root.is_dir():
        for path in sorted(timing_root.glob("*.json")):
            relative = path.relative_to(repo_root)
            if relative in prior_timing_refs:
                continue
            timing = _load_json(path, repo_root)
            measured_at = _parse_timestamp(
                timing.get("measured_at"),
                location=relative.as_posix(),
                field="measured_at",
            )
            if measured_at > current_key[0]:
                excluded_paths.add(relative)
    payload = _build_queue_payload(
        repo_root,
        candidates_with_locations=candidates_with_locations,
        receipts=prior_receipts,
        discoveries=pre_run_discoveries,
        provenance_events=provenance_events,
        excluded_paths=excluded_paths,
    )
    return payload["queue_sha256"]


def _has_independent_snapshot_witness(
    repo_root: Path,
    receipt: dict[str, Any],
    *,
    candidate_id: str,
    candidate_label: str,
    discoveries: dict[str, tuple[dict[str, Any], str]],
    provenance_events: dict[str, tuple[dict[str, Any], str]],
) -> bool:
    discovery = discoveries[receipt["discovery_id"]][0]
    event_refs = discovery.get("provenance_event_refs", [])
    if not isinstance(event_refs, list):
        event_refs = []
    snapshot_hash = receipt.get("queue_snapshot_sha256")
    receipt_issued_at = _parse_timestamp(
        receipt.get("issued_at"),
        location=f"receipt {receipt.get('receipt_id', 'unknown')}",
        field="issued_at",
    )
    for event_ref in event_refs:
        event_entry = provenance_events.get(event_ref)
        if event_entry is None:
            continue
        event = event_entry[0]
        configuration = event.get("method", {}).get("configuration", {})
        if not isinstance(configuration, dict):
            continue
        if (
            configuration.get("candidate_id") == candidate_id
            and configuration.get("frozen_queue_snapshot_sha256") == snapshot_hash
        ):
            return True

    # Legacy iterations predate the queue validator but their pre-run
    # provenance event names the research packet and records the reviewed
    # candidate ledger as an input.  Do not search the repository: an
    # unrelated packet must never be able to witness a receipt's snapshot.
    for event_ref in event_refs:
        event_entry = provenance_events.get(event_ref)
        if event_entry is None:
            continue
        event = event_entry[0]
        configuration = event.get("method", {}).get("configuration", {})
        if not isinstance(configuration, dict) or configuration.get("candidate_id") != candidate_id:
            continue
        started_at = event.get("started_at")
        try:
            if _parse_timestamp(started_at, location=event_ref, field="started_at") > receipt_issued_at:
                continue
        except QueueBuildError:
            continue
        event_inputs = event.get("inputs")
        if not isinstance(event_inputs, list):
            continue
        ledger_input = next(
            (
                input_ref
                for input_ref in event_inputs
                if isinstance(input_ref, dict)
                and input_ref.get("ref") == LEDGER_PATH.as_posix()
            ),
            None,
        )
        ledger_digest = ledger_input.get("sha256") if isinstance(ledger_input, dict) else None
        if not isinstance(ledger_digest, str) or not re.fullmatch(r"[0-9a-f]{64}", ledger_digest):
            continue

        prompt_ref = event.get("method", {}).get("prompt_or_instruction_ref")
        try:
            packet_path = _safe_relative_path(prompt_ref)
        except (QueueBuildError, TypeError):
            continue
        if not packet_path.as_posix().startswith("ToS/research-packets/"):
            continue
        packet = repo_root / packet_path
        if not packet.is_file():
            continue
        output = next(
            (
                output_ref
                for output_ref in event.get("outputs", [])
                if isinstance(output_ref, dict) and output_ref.get("ref") == packet_path.as_posix()
            ),
            None,
        )
        output_digest = output.get("sha256") if isinstance(output, dict) else None
        if not isinstance(output_digest, str) or not re.fullmatch(r"[0-9a-f]{64}", output_digest):
            continue
        try:
            packet_text = packet.read_text(encoding="utf-8")
        except OSError:
            continue
        if not re.search(
            rf"(?im)^\s*Candidate:\s*`{re.escape(candidate_id)}`\s*$",
            packet_text,
        ):
            continue
        if not re.search(
            rf"(?im)^\s*(?:Frozen\s+)?Queue snapshot SHA-256:\s*`{re.escape(snapshot_hash)}`\s*$",
            packet_text,
        ):
            continue
        if _contains_word_phrase(packet_text, candidate_label):
            return True
    return False


def _validate_receipt_transition_history(
    repo_root: Path,
    *,
    candidates_with_locations: list[tuple[dict[str, Any], str]],
    receipts: list[dict[str, Any]],
    discoveries: dict[str, tuple[dict[str, Any], str]],
    provenance_events: dict[str, tuple[dict[str, Any], str]] | None = None,
) -> None:
    candidates_by_id = {candidate["candidate_id"]: candidate for candidate, _ in candidates_with_locations}
    _validate_receipt_version_timestamp_order(receipts)
    ordered_receipts = sorted(receipts, key=_receipt_sort_key)
    status_by_candidate = {
        candidate["candidate_id"]: candidate["queue_status"]
        for candidate, _ in candidates_with_locations
    }
    previous_by_candidate: dict[str, dict[str, Any]] = {}
    if provenance_events is None:
        provenance_events = _load_provenance_events(repo_root)
    validated_snapshot_ids: set[str] = set()

    for receipt in ordered_receipts:
        receipt_id = _required_string(receipt, "receipt_id", "receipt")
        candidate_id = _required_string(receipt, "candidate_id", receipt_id)
        candidate = candidates_by_id[candidate_id]
        snapshot_hash = receipt.get("queue_snapshot_sha256")
        if not isinstance(snapshot_hash, str) or len(snapshot_hash) != 64 or any(
            character not in "0123456789abcdef" for character in snapshot_hash
        ):
            raise QueueBuildError(f"{receipt_id}: queue_snapshot_sha256 must be a lowercase SHA-256 digest")

        previous = previous_by_candidate.get(candidate_id)
        if previous is None:
            if candidate["candidate_kind"] not in QUEUEABLE_KINDS:
                raise QueueBuildError(f"{receipt_id}: terminal receipt cannot advance a non-queueable candidate")
            candidate_line = next(
                _candidate_source_line(location)
                for item, location in candidates_with_locations
                if item["candidate_id"] == candidate_id
            )
            # The reviewed ledger is append-only for this route.  A historical
            # receipt may predate later candidates that now sort earlier by a
            # broad chronology year, so the pre-run frontier is bounded by the
            # candidate's authored ledger line until an exact snapshot is
            # available for that run.
            historical_frontier = [
                (item, location)
                for item, location in candidates_with_locations
                if _candidate_source_line(location) <= candidate_line
            ]
            expected = next(
                (
                    item["candidate_id"]
                    for item, location in sorted(
                        historical_frontier,
                        key=lambda pair: _candidate_sort_key(
                            pair[0], source_line=_candidate_source_line(pair[1])
                        ),
                    )
                    if item["candidate_kind"] in QUEUEABLE_KINDS
                    and status_by_candidate[item["candidate_id"]] == READY_STATUS
                ),
                None,
            )
            if candidate_id != expected:
                raise QueueBuildError(
                    f"{receipt_id}: receipt candidate {candidate_id!r} was not the current next_candidate_id {expected!r}"
                )
            status_by_candidate[candidate_id] = receipt["terminal_status"]
        else:
            if _receipt_sort_key(receipt) < _receipt_sort_key(previous):
                raise QueueBuildError(f"{receipt_id}: superseding receipt is earlier than its predecessor")
            if snapshot_hash != previous.get("queue_snapshot_sha256"):
                raise QueueBuildError(f"{receipt_id}: superseding receipt changed the frozen queue snapshot")

        reconstructed = _reconstruct_pre_run_queue_sha256(
            repo_root,
            current_receipt=receipt,
            ordered_receipts=ordered_receipts,
            candidates_with_locations=candidates_with_locations,
            discoveries=discoveries,
            provenance_events=provenance_events,
        )
        if reconstructed == snapshot_hash:
            validated_snapshot_ids.add(receipt_id)
        elif previous is not None and previous["receipt_id"] in validated_snapshot_ids:
            validated_snapshot_ids.add(receipt_id)
        elif _has_independent_snapshot_witness(
            repo_root,
            receipt,
            candidate_id=candidate_id,
            candidate_label=candidate["preferred_label"],
            discoveries=discoveries,
            provenance_events=provenance_events,
        ):
            validated_snapshot_ids.add(receipt_id)
        else:
            raise QueueBuildError(
                f"{receipt_id}: queue_snapshot_sha256 does not resolve to the pre-run queue "
                "or an independent frozen-snapshot provenance witness"
            )
        previous_by_candidate[candidate_id] = receipt


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
    provenance_events = _load_provenance_events(repo_root)
    _validate_receipt_transition_history(
        repo_root,
        candidates_with_locations=candidates_with_locations,
        receipts=receipts,
        discoveries=discoveries,
        provenance_events=provenance_events,
    )
    return _build_queue_payload(
        repo_root,
        candidates_with_locations=candidates_with_locations,
        receipts=receipts,
        discoveries=discoveries,
        provenance_events=provenance_events,
    )


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
