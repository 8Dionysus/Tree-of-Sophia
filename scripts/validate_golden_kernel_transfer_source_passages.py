#!/usr/bin/env python3
"""Validate the fail-closed German transfer source-passage candidate set."""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from collections import Counter
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator, FormatChecker


REPO_ROOT = Path(__file__).resolve().parents[1]
GOLD_ROOT = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/also-sprach-zarathustra/"
    "gold-sets/foundation-pilot-v1"
)
TARGET_PATH = GOLD_ROOT / "transfer-target-passage-candidates.v1.json"
OUTPUT_PATH = GOLD_ROOT / "transfer-source-passage-candidates.v1.json"
ANCHOR_PATH = GOLD_ROOT / "transfer-source-passage-anchors.v1.jsonl"
PROVENANCE_PATH = GOLD_ROOT / "transfer-provenance.jsonl"
SCHEMA_PATH = Path(
    "ToS/contracts/transfer-source-passage-candidate-set.schema.json"
)
ANCHOR_SCHEMA_PATH = Path("ToS/contracts/source-anchor.schema.json")
EVENT_ID = (
    "tos.event.segmentation.golden-kernel-transfer-source-passages-v1."
    "2026-08-08"
)
EXPECTED_UNRESOLVED = {
    (
        "tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese",
        "32",
    ): ("start",),
    (
        "tos.work.friedrich-nietzsche.jenseits-von-gut-und-boese",
        "201",
    ): ("end_exclusive",),
    (
        "tos.work.friedrich-nietzsche.zur-genealogie-der-moral",
        "essay-1:10",
    ): ("end_exclusive",),
    (
        "tos.work.friedrich-nietzsche.der-antichrist",
        "main:8",
    ): ("start", "end_exclusive"),
    (
        "tos.work.friedrich-nietzsche.der-antichrist",
        "main:9",
    ): ("start",),
    (
        "tos.work.friedrich-nietzsche.der-antichrist",
        "main:43",
    ): ("end_exclusive",),
    (
        "tos.work.friedrich-nietzsche.der-antichrist",
        "main:44",
    ): ("start",),
}
EXPECTED_LAYER_COUNTS = {
    "abbyy-xml-paragraph": 12,
    "djvu-xml-line": 9,
    "poppler-pdf-bbox-line": 7,
}
EXPECTED_SUMMARY = {
    "conservative_source_route_count": 35,
    "materialized_source_passage_candidate_count": 28,
    "unresolved_source_boundary_count": 7,
    "accepted_source_passage_count": 0,
    "source_to_target_alignment_count": 0,
    "eligible_target_unit_count": 0,
    "target_gold_count": 0,
    "human_review_count": 0,
}
FALSE_EFFECTS = {
    "candidate_frame_changed",
    "tracked_source_text_created",
    "accepted_source_text_created",
    "source_to_target_passage_alignment_created",
    "translation_alignment_created",
    "target_unit_eligible",
    "target_gold_created",
    "semantic_work_opened",
    "human_work_scheduled",
    "canon_effect",
}


class ValidationFailure(RuntimeError):
    """Raised when source-passage candidate closure drifts."""


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
            record = json.loads(line)
        except json.JSONDecodeError as exc:
            raise ValidationFailure(
                f"cannot read {path}:{line_number}: {exc}"
            ) from exc
        if not isinstance(record, dict):
            raise ValidationFailure(f"{path}:{line_number} is not an object")
        records.append(record)
    return records


def _validate_schema(payload: Any, schema: dict[str, Any], label: str) -> None:
    validator = Draft202012Validator(schema, format_checker=FormatChecker())
    errors = sorted(validator.iter_errors(payload), key=lambda error: list(error.path))
    if errors:
        first = errors[0]
        location = ".".join(str(part) for part in first.path) or "<root>"
        raise ValidationFailure(f"{label} schema error at {location}: {first.message}")


def _validate_inputs(repo_root: Path, payload: dict[str, Any]) -> None:
    if payload["target_passage_candidate_set_ref"] != TARGET_PATH.as_posix():
        raise ValidationFailure("target passage candidate ref drifted")
    if payload["target_passage_candidate_set_sha256"] != _sha256_path(
        repo_root / TARGET_PATH
    ):
        raise ValidationFailure("target passage candidate digest drifted")
    seen: set[str] = set()
    for input_ref in payload["inputs"]:
        ref = input_ref["ref"]
        if ref in seen:
            raise ValidationFailure(f"duplicate input ref: {ref}")
        seen.add(ref)
        path = repo_root / ref
        if not path.is_file() or _sha256_path(path) != input_ref["sha256"]:
            raise ValidationFailure(f"input digest drifted: {ref}")


def _candidate_key(candidate: dict[str, Any]) -> tuple[str, str]:
    return candidate["work_ref"], candidate["qualified_unit_key"]


def _validate_candidates(
    payload: dict[str, Any], target_payload: dict[str, Any]
) -> None:
    candidates = payload["passage_candidates"]
    if len(candidates) != 35:
        raise ValidationFailure(f"expected 35 source routes, got {len(candidates)}")
    if len({candidate["source_passage_candidate_id"] for candidate in candidates}) != 35:
        raise ValidationFailure("source passage candidate IDs are not unique")
    target_by_id = {
        candidate["passage_candidate_id"]: candidate
        for candidate in target_payload["passage_candidates"]
    }
    if len(target_by_id) != 35:
        raise ValidationFailure("target candidate frame no longer contains 35 routes")
    if {candidate["target_passage_candidate_id"] for candidate in candidates} != set(
        target_by_id
    ):
        raise ValidationFailure("source candidate set does not close over target frame")

    materialized = []
    unresolved: dict[tuple[str, str], tuple[str, ...]] = {}
    for candidate in candidates:
        target = target_by_id[candidate["target_passage_candidate_id"]]
        if (
            candidate["frozen_page_candidate_id"]
            != target["frozen_page_candidate_id"]
            or candidate["qualified_unit_key"] != target["qualified_unit_key"]
            or candidate["work_ref"] != target["work_ref"]
            or candidate["source_structural_anchor_ref"]
            != target["source_structural_anchor_ref"]
            or candidate["human_review_performed"]
            or candidate["accepted_source_text"]
            or candidate["source_to_target_alignment_created"]
            or candidate["eligible_for_variant_execution"]
        ):
            raise ValidationFailure(
                f"authority or target-frame drift: {candidate['source_passage_candidate_id']}"
            )
        address = candidate["address_witness"]
        if address["file_ref"] != f"tos.file.sha256.{address['file_sha256']}":
            raise ValidationFailure("address witness file identity/fixity mismatch")
        if candidate["status"] == "boundary-unresolved":
            unresolved[_candidate_key(candidate)] = tuple(
                candidate["unresolved_boundaries"]
            )
            if any(
                candidate[field] is not None
                for field in (
                    "boundary_layer",
                    "start",
                    "end_exclusive",
                    "navigation_page_span",
                    "address_page_span",
                    "content_witness",
                    "source_passage_anchor_ref",
                    "private_content_ref",
                    "private_content_sha256",
                    "private_content_bytes",
                    "text_character_count",
                    "record_count",
                    "word_count",
                )
            ):
                raise ValidationFailure("unresolved route acquired candidate content")
            continue

        materialized.append(candidate)
        content = candidate["content_witness"]
        if content["file_ref"] != f"tos.file.sha256.{content['file_sha256']}":
            raise ValidationFailure("content witness file identity/fixity mismatch")
        navigation_span = candidate["navigation_page_span"]
        address_span = candidate["address_page_span"]
        if (
            not navigation_span
            or navigation_span
            != list(range(navigation_span[0], navigation_span[-1] + 1))
            or candidate["start"]["page"] > candidate["end_exclusive"]["page"]
            or candidate["start"]["page"] > navigation_span[0]
            or candidate["end_exclusive"]["page"] < navigation_span[-1]
            or not all(
                candidate["start"]["page"] <= page
                <= candidate["end_exclusive"]["page"]
                for page in navigation_span
            )
        ):
            raise ValidationFailure("materialized source boundary span drifted")
        work = candidate["work_ref"]
        relation = candidate["navigation_relation"]
        if work.endswith(".der-antichrist"):
            if (
                relation
                != "bounded-source-visible-two-page-offset-only-no-textual-identity"
                or content["item_ref"] == address["item_ref"]
                or content["file_ref"] == address["file_ref"]
                or address_span != [page - 2 for page in navigation_span]
            ):
                raise ValidationFailure("Antichrist no-identity navigation boundary drifted")
        elif work.endswith(".jenseits-von-gut-und-boese"):
            if (
                relation != "same-fixity-bound-Item-navigation-layer"
                or content["item_ref"] != address["item_ref"]
                or content["file_ref"] == address["file_ref"]
                or address_span != navigation_span
            ):
                raise ValidationFailure("Jenseits Item-layer relation drifted")
        elif work.endswith(".zur-genealogie-der-moral"):
            if (
                relation != "same-fixity-bound-file-page-index"
                or content != address
                or address_span != navigation_span
            ):
                raise ValidationFailure("Genealogie file-layer relation drifted")
        else:
            raise ValidationFailure(f"unexpected source work: {work}")
        expected_prefix = (
            f"{GOLD_ROOT.as_posix()}/local-content/transfer-source-passages/v1/"
        )
        if not candidate["private_content_ref"].startswith(expected_prefix):
            raise ValidationFailure("private source content leaves ignored owner lane")

    if unresolved != EXPECTED_UNRESOLVED:
        raise ValidationFailure(f"unresolved source boundary set drifted: {unresolved}")
    if Counter(candidate["boundary_layer"] for candidate in materialized) != Counter(
        EXPECTED_LAYER_COUNTS
    ):
        raise ValidationFailure("source boundary layer census drifted")
    if payload["summary"] != EXPECTED_SUMMARY:
        raise ValidationFailure(f"summary drifted: {payload['summary']}")
    effects = payload["effects"]
    if (
        not effects["private_source_text_materialized"]
        or not effects["layer_exact_source_boundary_candidates_created"]
        or any(effects[name] for name in FALSE_EFFECTS)
    ):
        raise ValidationFailure("source candidate authority effects drifted")


def _validate_anchors(
    payload: dict[str, Any], anchors: list[dict[str, Any]], schema: dict[str, Any]
) -> None:
    materialized = [
        candidate
        for candidate in payload["passage_candidates"]
        if candidate["status"] == "materialized-layer-exact-candidate"
    ]
    if len(anchors) != 28:
        raise ValidationFailure(f"expected 28 source anchors, got {len(anchors)}")
    by_id: dict[str, dict[str, Any]] = {}
    for anchor in anchors:
        _validate_schema(anchor, schema, anchor.get("anchor_id", "anchor"))
        if anchor["anchor_id"] in by_id:
            raise ValidationFailure(f"duplicate source anchor: {anchor['anchor_id']}")
        by_id[anchor["anchor_id"]] = anchor
    for candidate in materialized:
        anchor = by_id.get(candidate["source_passage_anchor_ref"])
        if anchor is None:
            raise ValidationFailure("materialized source candidate anchor is missing")
        selectors = anchor["selectors"]
        structural = [item for item in selectors if item["type"] == "structural"]
        regions = [item for item in selectors if item["type"] == "page_region"]
        positions = [item for item in selectors if item["type"] == "text_position"]
        content = candidate["content_witness"]
        if (
            len(structural) != 1
            or len(regions) != len(candidate["navigation_page_span"])
            or [region["page"] for region in regions]
            != candidate["navigation_page_span"]
            or len(positions) != 1
            or positions[0]["start"] != 0
            or positions[0]["end"] != candidate["text_character_count"]
            or positions[0]["text_layer_ref"] != candidate["private_content_ref"]
            or anchor["item_id"] != content["item_ref"]
            or anchor["file_id"] != content["file_ref"]
            or anchor["file_sha256"] != content["file_sha256"]
            or anchor["passage_id"] != candidate["source_passage_ref"]
            or anchor["status"] != "proposed"
            or anchor["provenance_event_ref"] != EVENT_ID
        ):
            raise ValidationFailure(
                f"source anchor closure drifted: {candidate['source_passage_anchor_ref']}"
            )


def _validate_provenance(
    repo_root: Path, payload: dict[str, Any], events: list[dict[str, Any]]
) -> None:
    matches = [event for event in events if event.get("event_id") == EVENT_ID]
    if len(matches) != 1:
        raise ValidationFailure("source passage provenance must resolve once")
    event = matches[0]
    if (
        event.get("event_type") != "segmentation"
        or event.get("status") != "completed_with_warnings"
        or event.get("rights_basis_ref") is not None
    ):
        raise ValidationFailure("source passage provenance posture drifted")
    event_inputs = {item["ref"]: item for item in event.get("inputs", [])}
    if event_inputs != {item["ref"]: item for item in payload["inputs"]}:
        raise ValidationFailure("candidate/provenance input closure drifted")
    outputs = {item["ref"]: item for item in event.get("outputs", [])}
    for tracked_path in (OUTPUT_PATH, ANCHOR_PATH):
        ref = tracked_path.as_posix()
        if outputs.get(ref, {}).get("sha256") != _sha256_path(repo_root / tracked_path):
            raise ValidationFailure(f"tracked provenance output drifted: {ref}")
    materialized = [
        candidate
        for candidate in payload["passage_candidates"]
        if candidate["status"] == "materialized-layer-exact-candidate"
    ]
    private_refs = {candidate["private_content_ref"] for candidate in materialized}
    private_outputs = {
        ref
        for ref, item in outputs.items()
        if item.get("role")
        == "gitignored-private-automatic-source-passage-candidate"
    }
    if private_outputs != private_refs:
        raise ValidationFailure("private source provenance output set drifted")
    for candidate in materialized:
        if outputs[candidate["private_content_ref"]]["sha256"] != candidate[
            "private_content_sha256"
        ]:
            raise ValidationFailure("private source digest/provenance mismatch")
    configuration = event.get("method", {}).get("configuration", {})
    expected_configuration = {
        "conservative_source_routes": 35,
        "materialized_source_passage_candidates": 28,
        "unresolved_source_boundaries": 7,
        "private_content_files": 28,
        "source_visible_ocr_marker_overrides": 1,
        "tracked_text_created": False,
        "human_review_count": 0,
        "accepted_source_passages": 0,
        "source_to_target_alignments": 0,
        "eligible_target_units": 0,
        "target_gold_count": 0,
    }
    if configuration != expected_configuration:
        raise ValidationFailure(f"provenance method effects drifted: {configuration}")


def _validate_no_tracked_private_content(
    repo_root: Path, payload: dict[str, Any]
) -> None:
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
    for candidate in payload["passage_candidates"]:
        ref = candidate["private_content_ref"]
        if ref is None:
            continue
        if (repo_root / ref).exists():
            raise ValidationFailure(f"private content entered checkout: {ref}")
        ignored = subprocess.run(
            ["git", "check-ignore", "-q", ref], cwd=repo_root, check=False
        )
        if ignored.returncode != 0:
            raise ValidationFailure(f"private content ref is not ignored: {ref}")


def _validate_local_content(
    *, payload: dict[str, Any], local_output_root: Path
) -> None:
    expected_paths: set[Path] = set()
    for candidate in payload["passage_candidates"]:
        if candidate["status"] != "materialized-layer-exact-candidate":
            continue
        relative = Path(candidate["private_content_ref"]).relative_to(
            "ToS/source-witnesses"
        )
        path = local_output_root / relative
        expected_paths.add(path)
        if (
            not path.is_file()
            or path.stat().st_size != candidate["private_content_bytes"]
            or _sha256_path(path) != candidate["private_content_sha256"]
            or (path.stat().st_mode & 0o777) != 0o600
        ):
            raise ValidationFailure(f"private source content drifted: {path}")
        private = _read_json(path)
        records = private.get("records")
        automatic_text = private.get("automatic_candidate_text")
        if (
            private.get("source_passage_candidate_id")
            != candidate["source_passage_candidate_id"]
            or private.get("target_passage_candidate_id")
            != candidate["target_passage_candidate_id"]
            or private.get("content_witness") != candidate["content_witness"]
            or private.get("boundary_layer") != candidate["boundary_layer"]
            or not isinstance(records, list)
            or len(records) != candidate["record_count"]
            or not isinstance(automatic_text, str)
            or len(automatic_text) != candidate["text_character_count"]
            or sum(len(record.get("words", [])) for record in records)
            != candidate["word_count"]
        ):
            raise ValidationFailure(f"private source payload closure drifted: {path}")
    candidate_dir = (
        local_output_root
        / GOLD_ROOT.relative_to("ToS/source-witnesses")
        / "local-content/transfer-source-passages/v1"
    )
    actual_paths = set(candidate_dir.glob("*.json")) if candidate_dir.is_dir() else set()
    if actual_paths != expected_paths:
        raise ValidationFailure("private source candidate file set drifted")


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
        target_payload = _read_json(repo_root / TARGET_PATH)
        anchors = _read_jsonl(repo_root / ANCHOR_PATH)
        events = _read_jsonl(repo_root / PROVENANCE_PATH)
        _validate_schema(payload, _read_json(repo_root / SCHEMA_PATH), "candidate set")
        _validate_inputs(repo_root, payload)
        _validate_candidates(payload, target_payload)
        _validate_anchors(
            payload, anchors, _read_json(repo_root / ANCHOR_SCHEMA_PATH)
        )
        _validate_provenance(repo_root, payload, events)
        _validate_no_tracked_private_content(repo_root, payload)
        if args.local_output_root is not None:
            _validate_local_content(
                payload=payload,
                local_output_root=args.local_output_root.resolve(),
            )
    except ValidationFailure as exc:
        print(f"[error] {exc}", file=sys.stderr)
        return 1
    print(
        "Golden-kernel transfer source passage validation passed "
        "(35 routes, 28 private layer-exact candidates, 7 unresolved source "
        "boundaries; 0 accepted German/alignment/eligibility/gold/human/"
        "semantic/canon effects)."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
