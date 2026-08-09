#!/usr/bin/env python3
"""Validate the text-free golden-kernel transfer readiness projection."""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator, FormatChecker


REPO_ROOT = Path(__file__).resolve().parents[1]
GOLD_ROOT = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
    "gold-sets/foundation-pilot-v1"
)
TARGET_PATH = GOLD_ROOT / "transfer-target-passage-candidates.v1.json"
SOURCE_PATH = GOLD_ROOT / "transfer-source-passage-candidates.v1.json"
OUTPUT_PATH = GOLD_ROOT / "transfer-route-readiness.v1.json"
PROVENANCE_PATH = GOLD_ROOT / "transfer-provenance.jsonl"
SCHEMA_PATH = Path("ToS/contracts/transfer-route-readiness-projection.schema.json")
PROVENANCE_SCHEMA_PATH = Path("ToS/contracts/provenance-event.schema.json")
BUILDER_PATH = Path("scripts/build_golden_kernel_transfer_route_readiness.py")
EVENT_ID = "tos.event.projection.golden-kernel-transfer-route-readiness-v1.2026-08-09"
EXPECTED_SUMMARY = {
    "conservative_route_count": 35,
    "source_candidate_available_route_count": 35,
    "target_candidate_available_route_count": 35,
    "dual_candidate_available_route_count": 35,
    "dual_candidate_frozen_page_intersection_count": 32,
    "dual_candidate_target_nonintersection_count": 3,
    "frozen_page_count": 20,
    "frozen_pages_with_dual_intersecting_route_count": 20,
    "source_to_target_alignment_count": 0,
    "eligible_target_unit_count": 0,
    "target_gold_count": 0,
    "human_review_count": 0,
}


class ReadinessValidationFailure(RuntimeError):
    """Raised when the tracked readiness closure drifts."""


def _sha256_path(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _read_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ReadinessValidationFailure(f"cannot read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise ReadinessValidationFailure(f"JSON root is not an object: {path}")
    return payload


def _read_jsonl(path: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        raise ReadinessValidationFailure(f"cannot read {path}: {exc}") from exc
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            record = json.loads(line)
        except json.JSONDecodeError as exc:
            raise ReadinessValidationFailure(
                f"cannot read {path}:{line_number}: {exc}"
            ) from exc
        if not isinstance(record, dict):
            raise ReadinessValidationFailure(f"{path}:{line_number} is not an object")
        records.append(record)
    return records


def _validate_schema(payload: dict[str, Any], schema: dict[str, Any]) -> None:
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    errors = sorted(validator.iter_errors(payload), key=lambda error: list(error.path))
    if errors:
        first = errors[0]
        location = ".".join(str(part) for part in first.path) or "<root>"
        raise ReadinessValidationFailure(
            f"readiness schema error at {location}: {first.message}"
        )


def _validate_routes(
    payload: dict[str, Any],
    target_payload: dict[str, Any],
    source_payload: dict[str, Any],
) -> None:
    targets = {
        candidate["passage_candidate_id"]: candidate
        for candidate in target_payload["passage_candidates"]
    }
    sources = {
        candidate["source_passage_candidate_id"]: candidate
        for candidate in source_payload["passage_candidates"]
    }
    routes = payload["routes"]
    if len(targets) != 35 or len(sources) != 35 or len(routes) != 35:
        raise ReadinessValidationFailure("route cardinality drifted")
    if {route["target_passage_candidate_id"] for route in routes} != set(targets):
        raise ReadinessValidationFailure("target route closure drifted")
    if {route["source_passage_candidate_id"] for route in routes} != set(sources):
        raise ReadinessValidationFailure("source route closure drifted")
    intersecting = 0
    page_routes: dict[str, list[dict[str, Any]]] = {}
    for route in routes:
        target = targets[route["target_passage_candidate_id"]]
        source = sources[route["source_passage_candidate_id"]]
        expected_source_id = route["target_passage_candidate_id"].replace(
            "tos-target-passage-candidate-", "tos-source-passage-candidate-"
        )
        expected_route_id = route["target_passage_candidate_id"].replace(
            "tos-target-passage-candidate-", "tos-transfer-route-readiness-"
        )
        intersection = target["status"] == "proposed-intersecting-layer-exact"
        expected_class = (
            "dual-private-layer-candidates-frozen-page-intersecting"
            if intersection
            else "dual-private-layer-candidates-target-nonintersecting"
        )
        if (
            route["route_readiness_id"] != expected_route_id
            or route["source_passage_candidate_id"] != expected_source_id
            or source["target_passage_candidate_id"]
            != route["target_passage_candidate_id"]
            or route["frozen_page_candidate_id"] != target["frozen_page_candidate_id"]
            or route["frozen_page_candidate_id"] != source["frozen_page_candidate_id"]
            or route["work_ref"] != target["work_ref"]
            or route["work_ref"] != source["work_ref"]
            or route["qualified_unit_key"] != target["qualified_unit_key"]
            or route["qualified_unit_key"] != source["qualified_unit_key"]
            or route["source_candidate_status"] != source["status"]
            or route["source_boundary_layer"] != source["boundary_layer"]
            or route["target_candidate_status"] != target["status"]
            or route["target_boundary_layer"] != target["source_layer"]
            or route["frozen_page_intersection"] is not intersection
            or target["candidate_page_intersects_passage"] is not intersection
            or route["readiness_class"] != expected_class
            or not source["private_content_ref"]
            or not target["private_content_ref"]
            or not source["private_content_sha256"]
            or not target["private_content_sha256"]
        ):
            raise ReadinessValidationFailure(
                f"derived route drifted: {route['route_readiness_id']}"
            )
        if any(
            route[field]
            for field in (
                "accepted_source_or_target_text",
                "source_to_target_passage_alignment",
                "eligible_for_variant_execution",
                "target_gold",
                "human_review_performed",
            )
        ):
            raise ReadinessValidationFailure("readiness route acquired authority")
        intersecting += int(intersection)
        page_routes.setdefault(route["frozen_page_candidate_id"], []).append(route)
    if intersecting != 32 or len(routes) - intersecting != 3:
        raise ReadinessValidationFailure("route readiness census drifted")

    pages = {page["frozen_page_candidate_id"]: page for page in payload["frozen_pages"]}
    if len(pages) != 20 or set(pages) != set(page_routes):
        raise ReadinessValidationFailure("frozen-page closure drifted")
    for page_id, page in pages.items():
        rows = page_routes[page_id]
        expected_refs = [row["route_readiness_id"] for row in rows]
        expected_intersecting = sum(row["frozen_page_intersection"] for row in rows)
        if (
            page["route_readiness_refs"] != expected_refs
            or page["dual_intersecting_route_count"] != expected_intersecting
            or page["dual_target_nonintersecting_route_count"]
            != len(rows) - expected_intersecting
            or expected_intersecting < 1
        ):
            raise ReadinessValidationFailure(
                f"frozen-page readiness drifted: {page_id}"
            )
    if payload["summary"] != EXPECTED_SUMMARY:
        raise ReadinessValidationFailure(f"summary drifted: {payload['summary']}")


def _validate_inputs_and_provenance(
    repo_root: Path,
    payload: dict[str, Any],
    events: list[dict[str, Any]],
    provenance_schema: dict[str, Any],
) -> None:
    for field, path in (
        ("target_candidate_set", TARGET_PATH),
        ("source_candidate_set", SOURCE_PATH),
    ):
        expected = {"ref": path.as_posix(), "sha256": _sha256_path(repo_root / path)}
        if payload[field] != expected:
            raise ReadinessValidationFailure(f"{field} digest closure drifted")
    matches = [event for event in events if event.get("event_id") == EVENT_ID]
    if len(matches) != 1:
        raise ReadinessValidationFailure("readiness provenance must resolve once")
    event = matches[0]
    _validate_schema(event, provenance_schema)
    if (
        event.get("event_type") != "export"
        or event.get("status") != "completed_with_warnings"
        or event.get("rights_basis_ref") is not None
        or event.get("method", {}).get("maker_type") != "software"
    ):
        raise ReadinessValidationFailure("readiness provenance posture drifted")
    outputs = event.get("outputs", [])
    if outputs != [
        {
            "ref": OUTPUT_PATH.as_posix(),
            "role": "tracked-text-free-transfer-route-readiness-projection",
            "sha256": _sha256_path(repo_root / OUTPUT_PATH),
        }
    ]:
        raise ReadinessValidationFailure("readiness provenance output drifted")
    expected_inputs = {
        TARGET_PATH.as_posix(): ("target-candidate-set", TARGET_PATH),
        SOURCE_PATH.as_posix(): ("source-candidate-set", SOURCE_PATH),
        SCHEMA_PATH.as_posix(): ("readiness-contract", SCHEMA_PATH),
        BUILDER_PATH.as_posix(): ("readiness-builder", BUILDER_PATH),
    }
    actual_inputs = {item["ref"]: item for item in event.get("inputs", [])}
    if set(actual_inputs) != set(expected_inputs):
        raise ReadinessValidationFailure("readiness provenance input set drifted")
    for ref, (role, path) in expected_inputs.items():
        if actual_inputs[ref] != {
            "ref": ref,
            "sha256": _sha256_path(repo_root / path),
            "role": role,
        }:
            raise ReadinessValidationFailure(f"readiness input drifted: {ref}")


def _validate_text_free(payload: dict[str, Any]) -> None:
    rendered = json.dumps(payload, ensure_ascii=False)
    for forbidden in (
        '"text"',
        '"words"',
        '"records"',
        '"private_content_ref"',
        '"private_content_sha256"',
    ):
        if forbidden in rendered:
            raise ReadinessValidationFailure(
                f"readiness projection exposed forbidden field {forbidden}"
            )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    args = parser.parse_args(argv)
    repo_root = args.repo_root.resolve()
    try:
        payload = _read_json(repo_root / OUTPUT_PATH)
        target_payload = _read_json(repo_root / TARGET_PATH)
        source_payload = _read_json(repo_root / SOURCE_PATH)
        events = _read_jsonl(repo_root / PROVENANCE_PATH)
        _validate_schema(payload, _read_json(repo_root / SCHEMA_PATH))
        _validate_routes(payload, target_payload, source_payload)
        _validate_inputs_and_provenance(
            repo_root,
            payload,
            events,
            _read_json(repo_root / PROVENANCE_SCHEMA_PATH),
        )
        _validate_text_free(payload)
    except ReadinessValidationFailure as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1
    print(
        "Golden-kernel transfer route readiness validation passed "
        "(35 dual private candidate routes; 32 frozen-page intersections, "
        "3 target nonintersections, all 20 pages covered; 0 alignment/"
        "eligibility/gold/human/semantic/publication/canon effects)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
