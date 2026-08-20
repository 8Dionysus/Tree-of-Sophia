#!/usr/bin/env python3
"""Build the hash-only Zarathustra exact-form recurrence observation projection.

The builder reads only the tracked lexical projection. It computes exact
frequency, structural range, and part-size-aware dispersion without opening the
private source-bearing database or creating a lemma, sign candidate, or human
task.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import tempfile
from fractions import Fraction
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_PLAN = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/lexical-indexes/"
    "dta-first-editions-parts-1-4-v1/recurrence-plan.v1.json"
)
PLAN_SCHEMA = Path("ToS/contracts/lexical-recurrence-plan.schema.json")
SOURCE_SCHEMA = Path("ToS/contracts/lexical-index-projection.schema.json")
OUTPUT_SCHEMA = Path("ToS/contracts/lexical-recurrence-projection.schema.json")
PROVENANCE_SCHEMA = Path("ToS/contracts/provenance-event.schema.json")
GENERATOR_REF = "scripts/build_zarathustra_recurrence_projection.py"
AUTHORITY_BOUNDARY = (
    "deterministic hash-only exact-form recurrence observation over the current "
    "tracked lexical projection; no accepted German, occurrence authority, "
    "morphology, lemma, lexeme, translation, sign candidate, sign, concept, "
    "relation, graph, canon, public route, or human backlog"
)


class RecurrenceProjectionError(RuntimeError):
    """Raised when recurrence evidence cannot be closed exactly."""


def load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise RecurrenceProjectionError(f"cannot read JSON {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise RecurrenceProjectionError(f"{path} must contain a JSON object")
    return payload


def canonical_json_bytes(payload: object) -> bytes:
    return (
        json.dumps(
            payload,
            ensure_ascii=False,
            sort_keys=True,
            separators=(",", ":"),
        )
        + "\n"
    ).encode("utf-8")


def sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    try:
        with path.open("rb") as handle:
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                digest.update(chunk)
    except OSError as exc:
        raise RecurrenceProjectionError(f"cannot hash {path}: {exc}") from exc
    return digest.hexdigest()


def validate_schema(payload: object, schema_path: Path, label: str) -> None:
    schema = load_json(schema_path)
    errors = sorted(
        Draft202012Validator(schema).iter_errors(payload),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        details = "; ".join(
            f"{list(error.absolute_path)}: {error.message}" for error in errors[:8]
        )
        raise RecurrenceProjectionError(
            f"{label} schema validation failed: {details}"
        )


def resolve_repo_path(relative: str) -> Path:
    root = REPO_ROOT.resolve()
    candidate = (root / relative).resolve()
    try:
        candidate.relative_to(root)
    except ValueError as exc:
        raise RecurrenceProjectionError(
            f"path escapes repository root: {relative}"
        ) from exc
    return candidate


def round_fraction_ties_to_even(value: Fraction, scale: int) -> int:
    """Return exact rational * scale rounded to nearest, with ties to even."""

    if value < 0:
        raise RecurrenceProjectionError("scaled recurrence value cannot be negative")
    scaled = value * scale
    quotient, remainder = divmod(scaled.numerator, scaled.denominator)
    comparison = remainder * 2 - scaled.denominator
    if comparison > 0 or (comparison == 0 and quotient % 2 == 1):
        quotient += 1
    return quotient


def _validate_source_identity(
    plan: dict[str, Any],
    source_path: Path,
    source: dict[str, Any],
) -> None:
    expected = plan["source_projection"]
    actual_digest = sha256_file(source_path)
    if actual_digest != expected["sha256"]:
        raise RecurrenceProjectionError(
            "tracked lexical projection digest does not match frozen recurrence plan"
        )
    if source.get("schema_version") != expected["schema_version"]:
        raise RecurrenceProjectionError("source projection schema version drift")
    summary = source.get("summary")
    if not isinstance(summary, dict):
        raise RecurrenceProjectionError("source projection summary missing")
    checks = {
        "source_item_count": expected["source_item_count"],
        "exact_form_row_count": expected["exact_form_row_count"],
        "token_occurrence_count": expected["token_occurrence_count"],
    }
    for field, expected_value in checks.items():
        if summary.get(field) != expected_value:
            raise RecurrenceProjectionError(
                f"source projection {field} drift: "
                f"{summary.get(field)!r} != {expected_value!r}"
            )


def _part_totals(
    source: dict[str, Any],
) -> tuple[list[dict[str, Any]], dict[str, int]]:
    source_items = source.get("source_items")
    if not isinstance(source_items, list) or len(source_items) != 4:
        raise RecurrenceProjectionError("source projection must contain four parts")
    ordered = sorted(source_items, key=lambda item: item.get("part_order", 0))
    if [item.get("part_order") for item in ordered] != [1, 2, 3, 4]:
        raise RecurrenceProjectionError("source part order must be exactly 1..4")
    totals: list[dict[str, Any]] = []
    by_item: dict[str, int] = {}
    for item in ordered:
        item_ref = item.get("item_ref")
        token_count = item.get("token_occurrence_count")
        if not isinstance(item_ref, str) or not item_ref:
            raise RecurrenceProjectionError("source part has no item_ref")
        if not isinstance(token_count, int) or token_count < 1:
            raise RecurrenceProjectionError("source part has invalid token total")
        if item_ref in by_item:
            raise RecurrenceProjectionError("duplicate source part item_ref")
        by_item[item_ref] = token_count
        totals.append(
            {
                "part_order": item["part_order"],
                "item_ref": item_ref,
                "token_count": token_count,
            }
        )
    if sum(by_item.values()) != source["summary"]["token_occurrence_count"]:
        raise RecurrenceProjectionError("source part token totals do not close")
    return totals, by_item


def _row_projection(
    row: dict[str, Any],
    *,
    part_tokens: dict[str, int],
    total_tokens: int,
    scale: int,
) -> dict[str, Any]:
    exact_digest = row.get("exact_form_sha256")
    form_key = row.get("form_key")
    if form_key != f"lexical-form:sha256:{exact_digest}":
        raise RecurrenceProjectionError("form key does not bind exact-form digest")
    occurrence_count = row.get("occurrence_count")
    if not isinstance(occurrence_count, int) or occurrence_count < 1:
        raise RecurrenceProjectionError("invalid exact-form occurrence count")
    item_hits = row.get("source_items")
    if not isinstance(item_hits, list) or not item_hits:
        raise RecurrenceProjectionError("exact form has no source-item hits")

    observed_counts = {item_ref: 0 for item_ref in part_tokens}
    section_range = 0
    page_range = 0
    seen_items: set[str] = set()
    for hit in item_hits:
        item_ref = hit.get("item_ref")
        item_count = hit.get("occurrence_count")
        if item_ref not in part_tokens:
            raise RecurrenceProjectionError("form row references unknown source part")
        if item_ref in seen_items:
            raise RecurrenceProjectionError("duplicate source part in form row")
        seen_items.add(item_ref)
        if not isinstance(item_count, int) or item_count < 1:
            raise RecurrenceProjectionError("invalid per-part occurrence count")
        observed_counts[item_ref] = item_count
        page_hits = hit.get("page_hits")
        section_hits = hit.get("section_hits")
        if not isinstance(page_hits, list) or not page_hits:
            raise RecurrenceProjectionError("form row has no page-return hits")
        if not isinstance(section_hits, list):
            raise RecurrenceProjectionError("form row section hits are not an array")
        if sum(page.get("occurrence_count", -1) for page in page_hits) != item_count:
            raise RecurrenceProjectionError("page-hit counts do not close")
        section_range += len(section_hits)
        page_range += len(page_hits)

    if sum(observed_counts.values()) != occurrence_count:
        raise RecurrenceProjectionError("per-part occurrence counts do not close")
    unsectioned = row.get("unsectioned_occurrence_count")
    if not isinstance(unsectioned, int) or unsectioned < 0:
        raise RecurrenceProjectionError("invalid unsectioned occurrence count")
    section_hit_total = sum(
        section.get("occurrence_count", -1)
        for hit in item_hits
        for section in hit["section_hits"]
    )
    if section_hit_total + unsectioned != occurrence_count:
        raise RecurrenceProjectionError("section-hit and unsectioned counts do not close")

    dp = sum(
        abs(
            Fraction(part_tokens[item_ref], total_tokens)
            - Fraction(observed_counts[item_ref], occurrence_count)
        )
        for item_ref in part_tokens
    ) / 2
    maximum_share = Fraction(max(observed_counts.values()), occurrence_count)
    return {
        "form_key": form_key,
        "exact_form_sha256": exact_digest,
        "normalized_form_sha256": row["normalized_form_sha256"],
        "occurrence_count": occurrence_count,
        "part_range": len(item_hits),
        "section_range": section_range,
        "page_range": page_range,
        "part_dp_millionths": round_fraction_ties_to_even(dp, scale),
        "maximum_part_share_millionths": round_fraction_ties_to_even(
            maximum_share, scale
        ),
        "source_editorial_occurrence_count": row[
            "source_editorial_occurrence_count"
        ],
        "unsectioned_occurrence_count": unsectioned,
    }


def build_projection(
    plan: dict[str, Any],
    source: dict[str, Any],
    *,
    plan_path: Path,
    source_path: Path,
) -> dict[str, Any]:
    part_totals, part_tokens = _part_totals(source)
    scale = plan["calculation_law"]["scale"]
    total_tokens = sum(part_tokens.values())
    source_rows = source.get("form_rows")
    if not isinstance(source_rows, list) or not source_rows:
        raise RecurrenceProjectionError("source projection has no exact-form rows")

    rows: list[dict[str, Any]] = []
    previous_digest: str | None = None
    for source_row in source_rows:
        digest = source_row.get("exact_form_sha256")
        if not isinstance(digest, str) or len(digest) != 64:
            raise RecurrenceProjectionError("invalid exact-form digest")
        if previous_digest is not None and digest <= previous_digest:
            raise RecurrenceProjectionError(
                "source exact-form rows are not in strict digest order"
            )
        previous_digest = digest
        rows.append(
            _row_projection(
                source_row,
                part_tokens=part_tokens,
                total_tokens=total_tokens,
                scale=scale,
            )
        )

    dp_values = [row["part_dp_millionths"] for row in rows]
    output = {
        "$schema": (
            "https://tree-of-sophia.local/ToS/contracts/"
            "lexical-recurrence-projection.schema.json"
        ),
        "schema_version": "tos_lexical_recurrence_projection_v1",
        "generated_or_authored": "generated_from_tracked_lexical_projection",
        "projection_id": (
            "lexical-recurrence-projection:"
            "zarathustra-dta-first-editions-parts-1-4-v1"
        ),
        "plan": {
            "ref": plan_path.relative_to(REPO_ROOT).as_posix(),
            "sha256": sha256_file(plan_path),
        },
        "source_projection": {
            "ref": source_path.relative_to(REPO_ROOT).as_posix(),
            "sha256": sha256_file(source_path),
            "schema_version": source["schema_version"],
        },
        "generator": {
            "ref": GENERATOR_REF,
            "sha256": sha256_file(REPO_ROOT / GENERATOR_REF),
        },
        "work_ref": plan["work_ref"],
        "segmentation_totals": {
            "part_count": len(part_totals),
            "section_count": source["summary"]["section_count"],
            "page_count": source["summary"]["body_page_count"],
            "token_count": total_tokens,
            "parts": part_totals,
        },
        "method_views": {
            "A": "absolute-frequency",
            "B": "structural-range",
            "C": "tupleized-part-size-aware-dispersion",
        },
        "summary": {
            "row_count": len(rows),
            "token_occurrence_count": sum(
                row["occurrence_count"] for row in rows
            ),
            "single_part_form_count": sum(
                row["part_range"] == 1 for row in rows
            ),
            "all_four_parts_form_count": sum(
                row["part_range"] == 4 for row in rows
            ),
            "singleton_form_count": sum(
                row["occurrence_count"] == 1 for row in rows
            ),
            "minimum_part_dp_millionths": min(dp_values),
            "maximum_part_dp_millionths": max(dp_values),
            "semantic_fields_populated": 0,
        },
        "rows": rows,
        "content_exposure": plan["content_exposure"],
        "rights_and_visibility": {
            "source_payload_visibility": "local-only",
            "tracked_projection_visibility": "local-only",
            "future_site_route": "blocked",
            "rights_review_required_before_public_route": True,
        },
        "semantic_boundary": plan["semantic_boundary"],
        "provenance_event_ref": plan["provenance_event_ref"],
        "authority_boundary": AUTHORITY_BOUNDARY,
    }
    return output


def build_provenance_event(
    plan: dict[str, Any],
    projection_bytes: bytes,
    *,
    plan_path: Path,
    source_path: Path,
) -> dict[str, Any]:
    research_path = resolve_repo_path(plan["research_ref"])
    output_ref = plan["output"]["ref"]
    timestamp = plan["output"]["recorded_at"]
    generator_digest = sha256_file(REPO_ROOT / GENERATOR_REF)
    return {
        "schema_version": "tos_provenance_event_v1",
        "event_id": plan["provenance_event_ref"],
        "event_type": "export",
        "started_at": timestamp,
        "ended_at": timestamp,
        "agent_refs": ["model:codex", "software:python-3.14.6"],
        "inputs": [
            {
                "ref": plan_path.relative_to(REPO_ROOT).as_posix(),
                "role": "frozen-exact-form-recurrence-observation-plan",
                "sha256": sha256_file(plan_path),
            },
            {
                "ref": source_path.relative_to(REPO_ROOT).as_posix(),
                "role": "tracked-hash-count-and-resource-lexical-projection",
                "sha256": sha256_file(source_path),
            },
            {
                "ref": plan["research_ref"],
                "role": "ordered-recurrence-method-research",
                "sha256": sha256_file(research_path),
            },
        ],
        "outputs": [
            {
                "ref": output_ref,
                "role": "tracked-hash-only-exact-form-recurrence-observation",
                "sha256": sha256_bytes(projection_bytes),
            }
        ],
        "method": {
            "maker_type": "software",
            "name": "tupleized-exact-form-recurrence-projection",
            "version": "1",
            "artifact_digest": generator_digest,
            "runtime": "Python 3.14.6 standard-library Fraction",
            "device": "CPU",
            "configuration": {
                "source_item_count": 4,
                "exact_form_rows": 11352,
                "token_occurrences": 86287,
                "method_views": [
                    "absolute-frequency",
                    "structural-range",
                    "tupleized-part-size-aware-dispersion",
                ],
                "dp_scale": plan["calculation_law"]["scale"],
                "rounding": plan["calculation_law"]["rounding"],
                "composite_score_created": False,
                "tracked_exact_strings": False,
                "sign_candidate_materialized": False,
                "human_work_scheduled": False,
            },
            "prompt_or_instruction_ref": plan_path.relative_to(REPO_ROOT).as_posix(),
        },
        "status": "completed_with_warnings",
        "warnings": [
            "frequency and dispersion remain source observations and cannot nominate a stable sign",
            "form hashes are low-entropy navigational fingerprints and do not provide confidentiality",
            "the tracked projection contains no exact strings, sequence, context, or occurrence positions",
            "source text, German competence, morphology, lemma, lexeme, translation, sign, semantics, publication, and human review remain outside this projection",
        ],
        "receipt_refs": [
            output_ref,
            plan_path.relative_to(REPO_ROOT).as_posix(),
            plan["research_ref"],
        ],
        "rights_basis_ref": None,
        "event_version": 1,
        "supersedes_event_ref": None,
    }


def write_or_check(path: Path, payload: bytes, *, check: bool) -> None:
    if check:
        try:
            current = path.read_bytes()
        except OSError as exc:
            raise RecurrenceProjectionError(
                f"cannot read generated artifact {path}: {exc}"
            ) from exc
        if current != payload:
            raise RecurrenceProjectionError(f"generated artifact drift: {path}")
        return

    path.parent.mkdir(parents=True, exist_ok=True)
    file_descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.",
        dir=path.parent,
    )
    temporary_path = Path(temporary_name)
    try:
        with os.fdopen(file_descriptor, "wb") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        os.chmod(temporary_path, 0o644)
        os.replace(temporary_path, path)
    finally:
        if temporary_path.exists():
            temporary_path.unlink()


def build(plan_path: Path, *, check: bool) -> dict[str, Any]:
    plan = load_json(plan_path)
    validate_schema(plan, REPO_ROOT / PLAN_SCHEMA, "recurrence plan")
    source_path = resolve_repo_path(plan["source_projection"]["ref"])
    source = load_json(source_path)
    validate_schema(source, REPO_ROOT / SOURCE_SCHEMA, "lexical source projection")
    _validate_source_identity(plan, source_path, source)

    projection = build_projection(
        plan,
        source,
        plan_path=plan_path,
        source_path=source_path,
    )
    validate_schema(
        projection,
        REPO_ROOT / OUTPUT_SCHEMA,
        "recurrence projection",
    )
    projection_bytes = canonical_json_bytes(projection)
    provenance = build_provenance_event(
        plan,
        projection_bytes,
        plan_path=plan_path,
        source_path=source_path,
    )
    validate_schema(
        provenance,
        REPO_ROOT / PROVENANCE_SCHEMA,
        "recurrence provenance event",
    )
    provenance_bytes = canonical_json_bytes(provenance)

    output_path = resolve_repo_path(plan["output"]["ref"])
    provenance_path = resolve_repo_path(plan["output"]["provenance_ref"])
    write_or_check(output_path, projection_bytes, check=check)
    write_or_check(provenance_path, provenance_bytes, check=check)
    return {
        "status": "ok",
        "mode": "check" if check else "write",
        "projection_ref": plan["output"]["ref"],
        "projection_sha256": sha256_bytes(projection_bytes),
        "provenance_ref": plan["output"]["provenance_ref"],
        "provenance_sha256": sha256_bytes(provenance_bytes),
        "summary": projection["summary"],
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--plan",
        type=Path,
        default=DEFAULT_PLAN,
        help="repository-relative recurrence plan path",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if the tracked projection or provenance differs",
    )
    arguments = parser.parse_args()
    plan_path = (
        arguments.plan
        if arguments.plan.is_absolute()
        else (REPO_ROOT / arguments.plan)
    ).resolve()
    try:
        plan_path.relative_to(REPO_ROOT.resolve())
    except ValueError as exc:
        raise SystemExit(f"plan escapes repository root: {arguments.plan}") from exc
    try:
        result = build(plan_path, check=arguments.check)
    except RecurrenceProjectionError as exc:
        raise SystemExit(str(exc)) from exc
    print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
