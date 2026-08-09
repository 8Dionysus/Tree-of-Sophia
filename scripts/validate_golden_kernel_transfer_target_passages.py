#!/usr/bin/env python3
"""Validate tracked closure for private transfer target passage candidates."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator, FormatChecker


REPO_ROOT = Path(__file__).resolve().parents[1]
GOLD_ROOT = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
    "gold-sets/foundation-pilot-v1"
)
PLAN_PATH = GOLD_ROOT / "transfer-samples.json"
OUTPUT_PATH = GOLD_ROOT / "transfer-target-passage-candidates.v1.json"
ANCHOR_PATH = GOLD_ROOT / "transfer-target-passage-anchors.v1.jsonl"
PROVENANCE_PATH = GOLD_ROOT / "transfer-provenance.jsonl"
SCHEMA_PATH = Path(
    "ToS/contracts/transfer-target-passage-candidate-set.schema.json"
)
ANCHOR_SCHEMA_PATH = Path("ToS/contracts/source-anchor.schema.json")
EVENT_ID = (
    "tos.event.segmentation.golden-kernel-transfer-target-passages-v1."
    "2026-08-08"
)
EXPECTED_NONINTERSECTING = {
    ("tos-target-candidate-jenseits-p0399-random", "284"),
    ("tos-target-candidate-jenseits-p0279-hard", "46"),
    ("tos-target-candidate-antichrist-p0661-hard", "main:35"),
}
FALSE_EFFECTS = {
    "candidate_frame_changed",
    "tracked_target_text_created",
    "accepted_target_text_created",
    "source_passage_boundary_created",
    "source_to_target_passage_alignment_created",
    "translation_alignment_created",
    "target_unit_eligible",
    "target_gold_created",
    "semantic_work_opened",
    "human_work_scheduled",
    "canon_effect",
}


class ValidationFailure(RuntimeError):
    """Raised when the tracked passage-candidate closure is invalid."""


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
        raise ValidationFailure(f"cannot read {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise ValidationFailure(f"JSON root is not an object: {path}")
    return payload


def _read_jsonl(path: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        raise ValidationFailure(f"cannot read {path}: {exc}") from exc
    for line_number, line in enumerate(lines, start=1):
        if not line.strip():
            continue
        try:
            payload = json.loads(line)
        except json.JSONDecodeError as exc:
            raise ValidationFailure(
                f"cannot read {path}:{line_number}: {exc}"
            ) from exc
        if not isinstance(payload, dict):
            raise ValidationFailure(f"{path}:{line_number} is not an object")
        records.append(payload)
    return records


def _validate_schema(payload: Any, schema: dict[str, Any], label: str) -> None:
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    errors = sorted(validator.iter_errors(payload), key=lambda error: list(error.path))
    if errors:
        first = errors[0]
        location = ".".join(str(part) for part in first.path) or "<root>"
        raise ValidationFailure(f"{label} schema error at {location}: {first.message}")


def _candidate_key(candidate: dict[str, Any]) -> tuple[str, str]:
    return (candidate["frozen_page_candidate_id"], candidate["qualified_unit_key"])


def _validate_inputs(repo_root: Path, payload: dict[str, Any]) -> None:
    if payload["transfer_plan_sha256"] != _sha256_path(repo_root / PLAN_PATH):
        raise ValidationFailure("transfer plan digest drifted")
    seen: set[str] = set()
    for input_ref in payload["inputs"]:
        ref = input_ref["ref"]
        if ref in seen:
            raise ValidationFailure(f"duplicate input ref: {ref}")
        seen.add(ref)
        path = repo_root / ref
        if not path.is_file() or _sha256_path(path) != input_ref["sha256"]:
            raise ValidationFailure(f"input digest drifted: {ref}")
    witness = payload["target_witness"]
    rights_path = repo_root / witness["rights_ref"]
    if _sha256_path(rights_path) != witness["rights_sha256"]:
        raise ValidationFailure("target rights digest drifted")


def _validate_candidates(payload: dict[str, Any], plan: dict[str, Any]) -> None:
    candidates = payload["passage_candidates"]
    if len(candidates) != 35:
        raise ValidationFailure(f"expected 35 passage candidates, got {len(candidates)}")
    keys = [_candidate_key(candidate) for candidate in candidates]
    if len(set(keys)) != len(keys):
        raise ValidationFailure("passage candidate keys are not unique")
    plan_candidates = {
        candidate["unit_id"]: candidate
        for candidate in plan.get("candidate_target_units", [])
    }
    if len(plan_candidates) != 20:
        raise ValidationFailure("frozen transfer plan no longer has 20 candidates")
    for candidate in candidates:
        frozen = plan_candidates.get(candidate["frozen_page_candidate_id"])
        if frozen is None:
            raise ValidationFailure("passage candidate leaves the frozen frame")
        if (
            candidate["frozen_candidate_page"] != frozen["page"]
            or candidate["stratum"] != frozen["stratum"]
            or candidate["work_ref"] != frozen["work_ref"]
            or candidate["expression_ref"] != frozen["expression_ref"]
            or candidate["accepted_target_text"]
            or candidate["eligible_for_variant_execution"]
            or candidate["target_gold_status"] != "not_started"
            or candidate["human_review_performed"]
        ):
            raise ValidationFailure(
                f"authority or frozen-frame drift: {candidate['passage_candidate_id']}"
            )
        if not candidate["page_span"] or candidate["page_span"] != list(
            range(candidate["page_span"][0], candidate["page_span"][-1] + 1)
        ):
            raise ValidationFailure("passage page span must be contiguous")
        intersects = candidate["candidate_page_intersects_passage"]
        if intersects != (candidate["candidate_page_line_count"] > 0):
            raise ValidationFailure("candidate-page intersection/count mismatch")
        expected_status = (
            "proposed-intersecting-layer-exact"
            if intersects
            else "rejected-nonintersecting-layer-exact"
        )
        if candidate["status"] != expected_status:
            raise ValidationFailure("candidate status does not match intersection")
        if candidate["start"]["page"] > candidate["end_exclusive"]["page"]:
            raise ValidationFailure("passage boundary order is inverted")
        if not candidate["private_content_ref"].startswith(
            f"{GOLD_ROOT.as_posix()}/local-content/transfer-target-passages/v1/"
        ):
            raise ValidationFailure("private content ref leaves the ignored owner lane")
    actual_nonintersecting = {
        _candidate_key(candidate)
        for candidate in candidates
        if not candidate["candidate_page_intersects_passage"]
    }
    if actual_nonintersecting != EXPECTED_NONINTERSECTING:
        raise ValidationFailure(
            f"nonintersecting route set drifted: {sorted(actual_nonintersecting)}"
        )
    summary = payload["summary"]
    expected_summary = {
        "frozen_page_candidate_count": 20,
        "conservative_structural_route_count": 35,
        "layer_exact_passage_candidate_count": 35,
        "candidate_page_intersection_count": 32,
        "nonintersecting_route_count": 3,
        "accepted_target_passage_count": 0,
        "eligible_target_unit_count": 0,
        "target_gold_count": 0,
        "human_review_count": 0,
    }
    if summary != expected_summary:
        raise ValidationFailure(f"summary drifted: {summary}")
    effects = payload["effects"]
    if not effects["private_target_text_materialized"] or not effects[
        "layer_exact_target_boundary_candidates_created"
    ]:
        raise ValidationFailure("positive bounded effects are missing")
    if any(effects[name] for name in FALSE_EFFECTS):
        raise ValidationFailure("a prohibited authority effect became true")


def _validate_anchors(
    payload: dict[str, Any], anchors: list[dict[str, Any]], schema: dict[str, Any]
) -> None:
    candidates = payload["passage_candidates"]
    if len(anchors) != 35:
        raise ValidationFailure(f"expected 35 anchors, got {len(anchors)}")
    by_id: dict[str, dict[str, Any]] = {}
    for anchor in anchors:
        _validate_schema(anchor, schema, anchor.get("anchor_id", "anchor"))
        anchor_id = anchor["anchor_id"]
        if anchor_id in by_id:
            raise ValidationFailure(f"duplicate anchor: {anchor_id}")
        by_id[anchor_id] = anchor
    witness = payload["target_witness"]
    for candidate in candidates:
        anchor = by_id.get(candidate["passage_anchor_ref"])
        if anchor is None:
            raise ValidationFailure("passage candidate anchor is missing")
        selectors = anchor["selectors"]
        structural = [selector for selector in selectors if selector["type"] == "structural"]
        regions = [selector for selector in selectors if selector["type"] == "page_region"]
        positions = [selector for selector in selectors if selector["type"] == "text_position"]
        if (
            len(structural) != 1
            or not regions
            or len(positions) != 1
            or [region["page"] for region in regions] != candidate["page_span"]
            or positions[0]["start"] != 0
            or positions[0]["end"] != candidate["text_character_count"]
            or positions[0]["text_layer_ref"] != candidate["private_content_ref"]
            or anchor["item_id"] != witness["item_ref"]
            or anchor["file_id"] != witness["file_ref"]
            or anchor["file_sha256"] != witness["file_sha256"]
            or anchor["passage_id"] != candidate["passage_ref"]
            or anchor["provenance_event_ref"] != EVENT_ID
        ):
            raise ValidationFailure(
                f"anchor closure drift: {candidate['passage_anchor_ref']}"
            )
        expected_status = (
            "proposed" if candidate["candidate_page_intersects_passage"] else "rejected"
        )
        if anchor["status"] != expected_status:
            raise ValidationFailure("anchor status does not match route intersection")


def _validate_provenance(
    repo_root: Path,
    payload: dict[str, Any],
    events: list[dict[str, Any]],
) -> None:
    matches = [event for event in events if event.get("event_id") == EVENT_ID]
    if len(matches) != 1:
        raise ValidationFailure("passage materialization provenance must resolve once")
    event = matches[0]
    if (
        event.get("event_type") != "segmentation"
        or event.get("status") != "completed_with_warnings"
        or event.get("rights_basis_ref") != payload["target_witness"]["rights_ref"]
    ):
        raise ValidationFailure("passage provenance posture drifted")
    outputs = {output["ref"]: output for output in event.get("outputs", [])}
    for tracked_path in (OUTPUT_PATH, ANCHOR_PATH):
        ref = tracked_path.as_posix()
        output = outputs.get(ref)
        if output is None or output["sha256"] != _sha256_path(repo_root / tracked_path):
            raise ValidationFailure(f"tracked provenance output drifted: {ref}")
    candidates = payload["passage_candidates"]
    private_refs = {candidate["private_content_ref"] for candidate in candidates}
    private_outputs = {
        ref
        for ref, output in outputs.items()
        if output.get("role") == "gitignored-private-automatic-target-passage-candidate"
    }
    if private_outputs != private_refs:
        raise ValidationFailure("private provenance output set drifted")
    for candidate in candidates:
        if outputs[candidate["private_content_ref"]]["sha256"] != candidate[
            "private_content_sha256"
        ]:
            raise ValidationFailure("private digest/provenance mismatch")
    configuration = event.get("method", {}).get("configuration", {})
    if (
        configuration.get("frozen_page_candidates") != 20
        or configuration.get("conservative_routes") != 35
        or configuration.get("candidate_page_intersections") != 32
        or configuration.get("nonintersecting_routes") != 3
        or configuration.get("human_review_count") != 0
        or configuration.get("accepted_target_passages") != 0
        or configuration.get("eligible_target_units") != 0
        or configuration.get("target_gold_count") != 0
        or configuration.get("tracked_text_created") is not False
    ):
        raise ValidationFailure("provenance method effects drifted")


def _validate_no_tracked_private_content(repo_root: Path) -> None:
    result = subprocess.run(
        ["git", "ls-files", f"{GOLD_ROOT.as_posix()}/local-content"],
        cwd=repo_root,
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise ValidationFailure("cannot inspect tracked local-content paths")
    tracked = [line for line in result.stdout.splitlines() if line.strip()]
    allowed = [line for line in tracked if line.endswith("/local-content/README.md")]
    if tracked != allowed:
        raise ValidationFailure(f"private local content entered Git: {tracked}")


def _validate_local_content(
    *, payload: dict[str, Any], local_output_root: Path
) -> None:
    for candidate in payload["passage_candidates"]:
        relative = Path(candidate["private_content_ref"]).relative_to(
            "ToS/source-witnesses"
        )
        path = local_output_root / relative
        if (
            not path.is_file()
            or path.stat().st_size != candidate["private_content_bytes"]
            or _sha256_path(path) != candidate["private_content_sha256"]
            or (path.stat().st_mode & 0o777) != 0o600
        ):
            raise ValidationFailure(f"private passage content drifted: {path}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=REPO_ROOT)
    parser.add_argument(
        "--local-output-root",
        type=Path,
        help="optional explicit source-witness root for private digest/mode checks",
    )
    args = parser.parse_args(argv)
    repo_root = args.repo_root.resolve()
    try:
        payload = _read_json(repo_root / OUTPUT_PATH)
        plan = _read_json(repo_root / PLAN_PATH)
        anchors = _read_jsonl(repo_root / ANCHOR_PATH)
        events = _read_jsonl(repo_root / PROVENANCE_PATH)
        _validate_schema(payload, _read_json(repo_root / SCHEMA_PATH), "candidate set")
        _validate_inputs(repo_root, payload)
        _validate_candidates(payload, plan)
        _validate_anchors(
            payload, anchors, _read_json(repo_root / ANCHOR_SCHEMA_PATH)
        )
        _validate_provenance(repo_root, payload, events)
        _validate_no_tracked_private_content(repo_root)
        if args.local_output_root is not None:
            _validate_local_content(
                payload=payload,
                local_output_root=args.local_output_root.resolve(),
            )
    except ValidationFailure as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1
    print(
        "Golden-kernel transfer target passage validation passed "
        "(20 frozen pages, 35 layer-exact candidates, 32 intersections, "
        "3 rejected nonintersections; 0 accepted text/alignment/eligibility/"
        "gold/human/semantic/canon effects)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
