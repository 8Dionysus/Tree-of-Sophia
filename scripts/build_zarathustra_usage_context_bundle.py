#!/usr/bin/env python3
"""Build one private, complete Zarathustra usage-context method control.

Exact context and occurrence positions are written only to an explicit ignored
local-output root. The tracked receipt and provenance contain fixity, counts,
structural closure, and authority boundaries, but no source string, sequence,
context, or occurrence position.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import sqlite3
import tempfile
from collections import defaultdict
from pathlib import Path
from typing import Any

from jsonschema import Draft202012Validator


REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_PLAN = Path(
    "ToS/source-witnesses/works/friedrich-nietzsche/"
    "also-sprach-zarathustra/lexical-indexes/"
    "dta-first-editions-parts-1-4-v1/usage-context-plan.v1.json"
)
PLAN_SCHEMA = Path("ToS/contracts/lexical-usage-context-plan.schema.json")
ROW_SCHEMA = Path("ToS/contracts/lexical-usage-context-row.schema.json")
RECEIPT_SCHEMA = Path("ToS/contracts/lexical-usage-context-receipt.schema.json")
PROVENANCE_SCHEMA = Path("ToS/contracts/provenance-event.schema.json")
GENERATOR_REF = "scripts/build_zarathustra_usage_context_bundle.py"
QUESTION_ID = "zarathustra-work-identity-control-context-v1"
AUTHORITY_BOUNDARY = (
    "private complete exact-form usage-context materialization for one "
    "preselected method control plus a tracked source-withholding receipt; "
    "no accepted German, sentence boundary, morphology, lemma, lexeme, "
    "translation correspondence, sign candidate, sign, concept, claim, "
    "relation, graph, canon, public route, or human backlog"
)
ROW_FIELDS = [
    "schema_version",
    "context_id",
    "question_id",
    "form_key",
    "exact_form_sha256",
    "occurrence_id",
    "item_ref",
    "part_order",
    "source_file_sha256",
    "token_ordinal",
    "page_resource_id",
    "section_resource_id",
    "text_node_path",
    "start_offset",
    "end_offset",
    "editorial_status",
    "target_exact_form",
    "left_exact_tokens",
    "right_exact_tokens",
    "left_token_count",
    "right_token_count",
    "requested_window_each_side",
    "page_start_clipped",
    "page_end_clipped",
    "source_database_sha256",
    "authority",
]


class UsageContextBuildError(RuntimeError):
    """Raised when the usage-context evidence cannot be closed exactly."""


def load_json(path: Path) -> dict[str, Any]:
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise UsageContextBuildError(f"cannot read JSON {path}: {exc}") from exc
    if not isinstance(payload, dict):
        raise UsageContextBuildError(f"{path} must contain a JSON object")
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
        raise UsageContextBuildError(f"cannot hash {path}: {exc}") from exc
    return digest.hexdigest()


def validate_schema(payload: object, schema_path: Path, label: str) -> None:
    schema = load_json(schema_path)
    errors = sorted(
        Draft202012Validator(schema).iter_errors(payload),
        key=lambda error: list(error.absolute_path),
    )
    if errors:
        detail = "; ".join(
            f"{list(error.absolute_path)}: {error.message}"
            for error in errors[:8]
        )
        raise UsageContextBuildError(f"{label} schema validation failed: {detail}")


def resolve_under(root: Path, relative: str) -> Path:
    resolved_root = root.resolve()
    candidate = (resolved_root / relative).resolve()
    try:
        candidate.relative_to(resolved_root)
    except ValueError as exc:
        raise UsageContextBuildError(
            f"path escapes explicit root: {relative}"
        ) from exc
    return candidate


def verify_ref_digest(ref: str, expected: str, label: str) -> Path:
    path = resolve_under(REPO_ROOT, ref)
    actual = sha256_file(path)
    if actual != expected:
        raise UsageContextBuildError(
            f"{label} digest drift: observed {actual}, expected {expected}"
        )
    return path


def verify_frozen_sources(plan: dict[str, Any]) -> None:
    source = plan["source_lexical_index"]
    control = plan["recurrence_control"]
    verify_ref_digest(
        source["index_plan_ref"], source["index_plan_sha256"], "index plan"
    )
    verify_ref_digest(
        source["tracked_projection_ref"],
        source["tracked_projection_sha256"],
        "lexical projection",
    )
    verify_ref_digest(control["plan_ref"], control["plan_sha256"], "recurrence plan")
    recurrence_path = verify_ref_digest(
        control["projection_ref"],
        control["projection_sha256"],
        "recurrence projection",
    )
    recurrence = load_json(recurrence_path)
    matches = [
        row
        for row in recurrence.get("rows", [])
        if row.get("exact_form_sha256") == control["exact_form_sha256"]
    ]
    if len(matches) != 1:
        raise UsageContextBuildError(
            "recurrence control must resolve to exactly one hash-only row"
        )
    observed = {
        field: matches[0].get(field)
        for field in control["expected_tuple"]
    }
    if observed != control["expected_tuple"]:
        raise UsageContextBuildError(
            f"recurrence control tuple drift: {observed!r}"
        )
    if matches[0].get("form_key") != control["form_key"]:
        raise UsageContextBuildError("recurrence form key drift")


def _context_id(plan_id: str, database_sha256: str, occurrence_id: str) -> str:
    payload = f"{plan_id}\n{database_sha256}\n{occurrence_id}".encode("utf-8")
    return f"usage-context:sha256:{sha256_bytes(payload)}"


def read_context_rows(
    database_path: Path,
    *,
    plan: dict[str, Any],
    database_sha256: str,
    row_schema: dict[str, Any],
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    try:
        connection = sqlite3.connect(
            f"file:{database_path.as_posix()}?mode=ro&immutable=1",
            uri=True,
        )
    except sqlite3.Error as exc:
        raise UsageContextBuildError(
            f"cannot open lexical database read-only: {exc}"
        ) from exc
    connection.row_factory = sqlite3.Row
    control = plan["recurrence_control"]
    policy = plan["context_policy"]
    window = policy["window_tokens_each_side"]
    validator = Draft202012Validator(row_schema)
    rows: list[dict[str, Any]] = []
    seen_context_ids: set[str] = set()
    seen_occurrence_ids: set[str] = set()
    page_refs: set[tuple[str, str]] = set()
    section_refs: set[tuple[str, str]] = set()
    part_pages: dict[tuple[int, str], set[str]] = defaultdict(set)
    part_sections: dict[tuple[int, str], set[str]] = defaultdict(set)
    part_counts: dict[tuple[int, str], int] = defaultdict(int)
    part_unsectioned: dict[tuple[int, str], int] = defaultdict(int)
    left_counts: list[int] = []
    right_counts: list[int] = []
    source_editorial_count = 0
    target_digest_match_count = 0
    source_file_digest_resolution_count = 0
    total_local_context_token_count = 0

    try:
        quick_check = connection.execute("PRAGMA quick_check").fetchone()
        if quick_check is None or quick_check[0] != "ok":
            raise UsageContextBuildError("lexical database quick_check failed")
        metadata = dict(connection.execute("SELECT key, value FROM metadata"))
        if metadata.get("plan_sha256") != plan["source_lexical_index"][
            "index_plan_sha256"
        ]:
            raise UsageContextBuildError("database index-plan digest drift")
        form = connection.execute(
            """
            SELECT form_key, exact_form, exact_form_sha256, occurrence_count
            FROM forms
            WHERE exact_form_sha256 = ?
            """,
            (control["exact_form_sha256"],),
        ).fetchone()
        if form is None:
            raise UsageContextBuildError("target exact-form hash is absent")
        if form["form_key"] != control["form_key"]:
            raise UsageContextBuildError("database form key drift")
        if sha256_bytes(form["exact_form"].encode("utf-8")) != form[
            "exact_form_sha256"
        ]:
            raise UsageContextBuildError("database target exact surface digest drift")
        if form["occurrence_count"] != control["expected_tuple"][
            "occurrence_count"
        ]:
            raise UsageContextBuildError("database target occurrence count drift")

        targets = connection.execute(
            """
            SELECT
              o.occurrence_id,
              o.item_ref,
              i.part_order,
              i.file_sha256 AS source_file_sha256,
              o.token_ordinal,
              o.form_key,
              o.exact_form,
              o.exact_form_sha256,
              o.page_resource_id,
              o.section_resource_id,
              o.text_node_path,
              o.start_offset,
              o.end_offset,
              o.editorial_status
            FROM occurrences AS o
            JOIN source_items AS i ON i.item_ref = o.item_ref
            WHERE o.exact_form_sha256 = ?
            ORDER BY i.part_order, o.token_ordinal
            """,
            (control["exact_form_sha256"],),
        ).fetchall()
        if len(targets) != form["occurrence_count"]:
            raise UsageContextBuildError("target occurrence query does not close")

        for target in targets:
            page_tokens = connection.execute(
                """
                SELECT token_ordinal, exact_form, occurrence_id
                FROM occurrences
                WHERE item_ref = ?
                  AND page_resource_id = ?
                  AND token_ordinal BETWEEN ? AND ?
                ORDER BY token_ordinal
                """,
                (
                    target["item_ref"],
                    target["page_resource_id"],
                    target["token_ordinal"] - window,
                    target["token_ordinal"] + window,
                ),
            ).fetchall()
            target_rows = [
                token
                for token in page_tokens
                if token["occurrence_id"] == target["occurrence_id"]
            ]
            if len(target_rows) != 1:
                raise UsageContextBuildError(
                    "target occurrence is not uniquely recoverable on its page"
                )
            left = [
                token["exact_form"]
                for token in page_tokens
                if token["token_ordinal"] < target["token_ordinal"]
            ]
            right = [
                token["exact_form"]
                for token in page_tokens
                if token["token_ordinal"] > target["token_ordinal"]
            ]
            if len(left) > window or len(right) > window:
                raise UsageContextBuildError("page-bounded context exceeds frozen window")
            if sha256_bytes(target["exact_form"].encode("utf-8")) != target[
                "exact_form_sha256"
            ]:
                raise UsageContextBuildError("target occurrence surface digest drift")
            if target["form_key"] != control["form_key"]:
                raise UsageContextBuildError("target occurrence form key drift")

            row = {
                "schema_version": "tos_lexical_usage_context_row_v1",
                "context_id": _context_id(
                    plan["plan_id"], database_sha256, target["occurrence_id"]
                ),
                "question_id": QUESTION_ID,
                "form_key": target["form_key"],
                "exact_form_sha256": target["exact_form_sha256"],
                "occurrence_id": target["occurrence_id"],
                "item_ref": target["item_ref"],
                "part_order": target["part_order"],
                "source_file_sha256": target["source_file_sha256"],
                "token_ordinal": target["token_ordinal"],
                "page_resource_id": target["page_resource_id"],
                "section_resource_id": target["section_resource_id"],
                "text_node_path": target["text_node_path"],
                "start_offset": target["start_offset"],
                "end_offset": target["end_offset"],
                "editorial_status": target["editorial_status"],
                "target_exact_form": target["exact_form"],
                "left_exact_tokens": left,
                "right_exact_tokens": right,
                "left_token_count": len(left),
                "right_token_count": len(right),
                "requested_window_each_side": window,
                "page_start_clipped": len(left) < window,
                "page_end_clipped": len(right) < window,
                "source_database_sha256": database_sha256,
                "authority": "unreviewed-source-visible-method-control",
            }
            if list(row) != ROW_FIELDS:
                raise UsageContextBuildError("private row field order drift")
            errors = list(validator.iter_errors(row))
            if errors:
                raise UsageContextBuildError(
                    "private row schema failure: " + errors[0].message
                )
            if row["context_id"] in seen_context_ids:
                raise UsageContextBuildError("duplicate derived context ID")
            if row["occurrence_id"] in seen_occurrence_ids:
                raise UsageContextBuildError("duplicate target occurrence ID")
            seen_context_ids.add(row["context_id"])
            seen_occurrence_ids.add(row["occurrence_id"])
            target_digest_match_count += 1
            source_file_digest_resolution_count += 1
            page_refs.add((row["item_ref"], row["page_resource_id"]))
            key = (row["part_order"], row["item_ref"])
            part_pages[key].add(row["page_resource_id"])
            part_counts[key] += 1
            if row["section_resource_id"] is None:
                part_unsectioned[key] += 1
            else:
                section_refs.add((row["item_ref"], row["section_resource_id"]))
                part_sections[key].add(row["section_resource_id"])
            if row["editorial_status"] != "witness-text":
                source_editorial_count += 1
            left_counts.append(row["left_token_count"])
            right_counts.append(row["right_token_count"])
            total_local_context_token_count += (
                row["left_token_count"] + 1 + row["right_token_count"]
            )
            rows.append(row)

        page_resolution_count = 0
        for item_ref, resource_id in page_refs:
            match = connection.execute(
                "SELECT 1 FROM pages WHERE item_ref = ? AND resource_id = ?",
                (item_ref, resource_id),
            ).fetchone()
            if match is None:
                raise UsageContextBuildError("page selector does not resolve")
            page_resolution_count += sum(
                row["item_ref"] == item_ref
                and row["page_resource_id"] == resource_id
                for row in rows
            )
        section_resolution_count = 0
        for item_ref, resource_id in section_refs:
            match = connection.execute(
                "SELECT 1 FROM sections WHERE item_ref = ? AND resource_id = ?",
                (item_ref, resource_id),
            ).fetchone()
            if match is None:
                raise UsageContextBuildError("section selector does not resolve")
            section_resolution_count += sum(
                row["item_ref"] == item_ref
                and row["section_resource_id"] == resource_id
                for row in rows
            )
    except sqlite3.Error as exc:
        raise UsageContextBuildError(f"cannot query lexical database: {exc}") from exc
    finally:
        connection.close()

    if not rows:
        raise UsageContextBuildError("usage-context query produced no rows")
    parts = [
        {
            "part_order": part_order,
            "item_ref": item_ref,
            "occurrence_count": part_counts[(part_order, item_ref)],
            "page_count": len(part_pages[(part_order, item_ref)]),
            "section_count": len(part_sections[(part_order, item_ref)]),
            "unsectioned_occurrence_count": part_unsectioned[(part_order, item_ref)],
        }
        for part_order, item_ref in sorted(part_counts)
    ]
    summary = {
        "row_count": len(rows),
        "target_occurrence_count": len(rows),
        "source_item_count": len(parts),
        "page_count": len(page_refs),
        "section_count": len(section_refs),
        "unsectioned_occurrence_count": sum(part_unsectioned.values()),
        "source_editorial_occurrence_count": source_editorial_count,
        "minimum_left_token_count": min(left_counts),
        "maximum_left_token_count": max(left_counts),
        "minimum_right_token_count": min(right_counts),
        "maximum_right_token_count": max(right_counts),
        "page_start_clipped_count": sum(count < window for count in left_counts),
        "page_end_clipped_count": sum(count < window for count in right_counts),
        "total_local_context_token_count": total_local_context_token_count,
        "semantic_fields_populated": 0,
    }
    identity_closure = {
        "unique_context_id_count": len(seen_context_ids),
        "unique_occurrence_id_count": len(seen_occurrence_ids),
        "target_digest_match_count": target_digest_match_count,
        "page_selector_resolution_count": page_resolution_count,
        "section_selector_resolution_count": section_resolution_count,
        "source_file_digest_resolution_count": source_file_digest_resolution_count,
        "complete_occurrence_census": len(rows)
        == plan["recurrence_control"]["expected_tuple"]["occurrence_count"],
    }
    return rows, {
        "summary": summary,
        "parts": parts,
        "identity_closure": identity_closure,
    }


def build_receipt(
    *,
    plan: dict[str, Any],
    plan_path: Path,
    database_path: Path,
    database_sha256: str,
    packet_bytes: bytes,
    aggregates: dict[str, Any],
) -> dict[str, Any]:
    source = plan["source_lexical_index"]
    control = plan["recurrence_control"]
    local = plan["local_bundle"]
    return {
        "$schema": (
            "https://tree-of-sophia.local/ToS/contracts/"
            "lexical-usage-context-receipt.schema.json"
        ),
        "schema_version": "tos_lexical_usage_context_receipt_v1",
        "generated_or_authored": "generated_from_local_lexical_projection",
        "receipt_id": (
            "lexical-usage-context-receipt:"
            "zarathustra-work-identity-control-v1"
        ),
        "plan": {
            "ref": plan_path.relative_to(REPO_ROOT).as_posix(),
            "sha256": sha256_file(plan_path),
        },
        "generator": {
            "ref": GENERATOR_REF,
            "sha256": sha256_file(REPO_ROOT / GENERATOR_REF),
        },
        "source_database": {
            "relative_path": source["local_database_relative_path"],
            "sha256": database_sha256,
            "bytes": database_path.stat().st_size,
            "quick_check": "ok",
        },
        "source_projections": {
            "index_plan": {
                "ref": source["index_plan_ref"],
                "sha256": source["index_plan_sha256"],
            },
            "lexical_projection": {
                "ref": source["tracked_projection_ref"],
                "sha256": source["tracked_projection_sha256"],
            },
            "recurrence_plan": {
                "ref": control["plan_ref"],
                "sha256": control["plan_sha256"],
            },
            "recurrence_projection": {
                "ref": control["projection_ref"],
                "sha256": control["projection_sha256"],
            },
        },
        "recurrence_control": {
            "form_key": control["form_key"],
            "exact_form_sha256": control["exact_form_sha256"],
            "selection_basis": control["selection_basis"],
            "observed_tuple": control["expected_tuple"],
        },
        "local_bundle": {
            "relative_path": local["relative_path"],
            "format": local["format"],
            "schema_ref": local["schema_ref"],
            "schema_version": local["schema_version"],
            "sha256": sha256_bytes(packet_bytes),
            "bytes": len(packet_bytes),
            "mode": local["mode"],
            "row_count": aggregates["summary"]["row_count"],
            "required_fields": ROW_FIELDS,
        },
        "context_policy": {
            "policy_id": plan["context_policy"]["policy_id"],
            "window_tokens_each_side": plan["context_policy"][
                "window_tokens_each_side"
            ],
            "boundary": plan["context_policy"]["boundary"],
            "sampling": plan["context_policy"]["sampling"],
            "row_order": plan["context_policy"]["row_order"],
            "sentence_boundary_claimed": plan["context_policy"][
                "sentence_boundary_claimed"
            ],
        },
        "summary": aggregates["summary"],
        "parts": aggregates["parts"],
        "identity_closure": aggregates["identity_closure"],
        "content_exposure": plan["content_exposure"],
        "rights_and_visibility": plan["rights_and_visibility"],
        "semantic_boundary": plan["semantic_boundary"],
        "provenance_event_ref": plan["provenance_event_ref"],
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def build_provenance(
    *,
    plan: dict[str, Any],
    plan_path: Path,
    receipt_path: Path,
    receipt_bytes: bytes,
    packet_bytes: bytes,
    database_sha256: str,
    aggregates: dict[str, Any],
) -> dict[str, Any]:
    source = plan["source_lexical_index"]
    control = plan["recurrence_control"]
    return {
        "schema_version": "tos_provenance_event_v1",
        "event_id": plan["provenance_event_ref"],
        "event_type": "export",
        "started_at": "2026-08-02T14:50:00Z",
        "ended_at": "2026-08-02T14:50:00Z",
        "agent_refs": [
            "software:python-" + platform.python_version(),
            "software:sqlite-" + sqlite3.sqlite_version,
        ],
        "inputs": [
            {
                "ref": plan_path.relative_to(REPO_ROOT).as_posix(),
                "role": "frozen-question-scoped-usage-context-plan",
                "sha256": sha256_file(plan_path),
            },
            {
                "ref": source["local_database_relative_path"],
                "role": "private-fixity-bound-source-bearing-lexical-database",
                "sha256": database_sha256,
            },
            {
                "ref": source["tracked_projection_ref"],
                "role": "tracked-hash-count-and-resource-lexical-projection",
                "sha256": source["tracked_projection_sha256"],
            },
            {
                "ref": control["projection_ref"],
                "role": "tracked-hash-only-recurrence-control-projection",
                "sha256": control["projection_sha256"],
            },
            {
                "ref": plan["research_ref"],
                "role": "ordered-usage-context-method-research",
                "sha256": sha256_file(resolve_under(REPO_ROOT, plan["research_ref"])),
            },
        ],
        "outputs": [
            {
                "ref": plan["local_bundle"]["relative_path"],
                "role": "private-source-bearing-complete-usage-context-bundle",
                "sha256": sha256_bytes(packet_bytes),
            },
            {
                "ref": receipt_path.relative_to(REPO_ROOT).as_posix(),
                "role": "tracked-source-withholding-usage-context-receipt",
                "sha256": sha256_bytes(receipt_bytes),
            },
        ],
        "method": {
            "maker_type": "software",
            "name": "page-bounded-exact-kwic-method-control",
            "version": "1",
            "artifact_digest": sha256_file(REPO_ROOT / GENERATOR_REF),
            "runtime": (
                f"Python {platform.python_version()} with "
                f"SQLite {sqlite3.sqlite_version}"
            ),
            "device": "CPU",
            "configuration": {
                "selection": "complete-preselected-exact-form-occurrence-census",
                "target_occurrences": aggregates["summary"][
                    "target_occurrence_count"
                ],
                "window_tokens_each_side": 24,
                "boundary": "same-item-and-page-only",
                "source_strings_local_only": True,
                "tracked_source_strings": False,
                "sentence_boundary_claimed": False,
                "future_challengers_scheduled": False,
                "human_work_scheduled": False,
            },
            "prompt_or_instruction_ref": plan["research_ref"],
        },
        "status": "completed_with_warnings",
        "warnings": [
            "the local bundle contains exact sequential source context and remains ignored mode-0600 local-only material",
            "a fixed page-bounded token window is a transparent concordance baseline and not an accepted sentence or sense boundary",
            "the preselected identity control is not a recurrence winner, sign candidate, or claim of one stable linguistic identity",
            "the tracked receipt contains no exact strings, sequence, context, occurrence positions, morphology, translation, semantic label, graph edge, or human judgment",
            "future public use requires independently reacquired publication material and a fresh rights and operator approval gate",
        ],
        "receipt_refs": [
            receipt_path.relative_to(REPO_ROOT).as_posix(),
            plan_path.relative_to(REPO_ROOT).as_posix(),
            plan["research_ref"],
        ],
        "rights_basis_ref": None,
        "event_version": 1,
        "supersedes_event_ref": None,
    }


def _atomic_write(path: Path, payload: bytes, *, mode: int | None = None) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, temporary_name = tempfile.mkstemp(
        prefix=f".{path.name}.", suffix=".tmp", dir=path.parent
    )
    temporary_path = Path(temporary_name)
    try:
        with os.fdopen(descriptor, "wb") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
        if mode is not None:
            os.chmod(temporary_path, mode)
        os.replace(temporary_path, path)
    finally:
        temporary_path.unlink(missing_ok=True)


def materialize(
    *,
    plan_path: Path,
    local_input_root: Path,
    local_output_root: Path,
    receipt_path: Path,
    provenance_path: Path,
    check: bool,
) -> dict[str, Any]:
    resolved_plan = plan_path.resolve()
    try:
        resolved_plan.relative_to(REPO_ROOT.resolve())
    except ValueError as exc:
        raise UsageContextBuildError(
            "plan must remain inside the Tree repository"
        ) from exc
    plan = load_json(resolved_plan)
    validate_schema(plan, REPO_ROOT / PLAN_SCHEMA, "plan")
    verify_frozen_sources(plan)

    source = plan["source_lexical_index"]
    database_path = resolve_under(
        local_input_root, source["local_database_relative_path"]
    )
    database_sha256 = sha256_file(database_path)
    if database_sha256 != source["local_database_sha256"]:
        raise UsageContextBuildError("local lexical database digest drift")
    if database_path.stat().st_size != source["local_database_bytes"]:
        raise UsageContextBuildError("local lexical database byte count drift")

    row_schema = load_json(REPO_ROOT / ROW_SCHEMA)
    rows, aggregates = read_context_rows(
        database_path,
        plan=plan,
        database_sha256=database_sha256,
        row_schema=row_schema,
    )
    expected = plan["recurrence_control"]["expected_tuple"]
    checks = {
        "row_count": expected["occurrence_count"],
        "target_occurrence_count": expected["occurrence_count"],
        "source_item_count": expected["part_range"],
        "page_count": expected["page_range"],
        "section_count": expected["section_range"],
        "unsectioned_occurrence_count": expected[
            "unsectioned_occurrence_count"
        ],
        "source_editorial_occurrence_count": expected[
            "source_editorial_occurrence_count"
        ],
    }
    for field, expected_value in checks.items():
        if aggregates["summary"][field] != expected_value:
            raise UsageContextBuildError(
                f"{field} drift: {aggregates['summary'][field]} != {expected_value}"
            )
    if len(aggregates["parts"]) != 4:
        raise UsageContextBuildError("part summary does not close over four Items")

    packet_bytes = b"".join(canonical_json_bytes(row) for row in rows)
    receipt = build_receipt(
        plan=plan,
        plan_path=resolved_plan,
        database_path=database_path,
        database_sha256=database_sha256,
        packet_bytes=packet_bytes,
        aggregates=aggregates,
    )
    validate_schema(receipt, REPO_ROOT / RECEIPT_SCHEMA, "receipt")
    receipt_bytes = canonical_json_bytes(receipt)
    provenance = build_provenance(
        plan=plan,
        plan_path=resolved_plan,
        receipt_path=receipt_path,
        receipt_bytes=receipt_bytes,
        packet_bytes=packet_bytes,
        database_sha256=database_sha256,
        aggregates=aggregates,
    )
    validate_schema(provenance, REPO_ROOT / PROVENANCE_SCHEMA, "provenance")
    provenance_bytes = canonical_json_bytes(provenance)
    packet_path = resolve_under(local_output_root, plan["local_bundle"]["relative_path"])

    if check:
        if not packet_path.is_file():
            raise UsageContextBuildError(f"private bundle is missing: {packet_path}")
        if packet_path.read_bytes() != packet_bytes:
            raise UsageContextBuildError(
                "private usage-context bundle is stale or non-deterministic"
            )
        if (packet_path.stat().st_mode & 0o777) != 0o600:
            raise UsageContextBuildError("private bundle mode must be 0600")
        if receipt_path.read_bytes() != receipt_bytes:
            raise UsageContextBuildError("tracked usage-context receipt is stale")
        if provenance_path.read_bytes() != provenance_bytes:
            raise UsageContextBuildError("tracked usage-context provenance is stale")
        return receipt

    _atomic_write(packet_path, packet_bytes, mode=0o600)
    _atomic_write(receipt_path, receipt_bytes)
    _atomic_write(provenance_path, provenance_bytes)
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", type=Path, default=REPO_ROOT / DEFAULT_PLAN)
    parser.add_argument(
        "--local-input-root",
        type=Path,
        required=True,
        help="explicit root containing the ignored lexical database",
    )
    parser.add_argument(
        "--local-output-root",
        type=Path,
        required=True,
        help="explicit root owning the ignored source-bearing context bundle",
    )
    parser.add_argument("--receipt", type=Path, default=None)
    parser.add_argument("--provenance", type=Path, default=None)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    try:
        plan = load_json(args.plan)
        receipt_path = (
            args.receipt.resolve()
            if args.receipt is not None
            else resolve_under(REPO_ROOT, plan["tracked_receipt_ref"])
        )
        provenance_path = (
            args.provenance.resolve()
            if args.provenance is not None
            else resolve_under(REPO_ROOT, plan["provenance_ref"])
        )
        receipt = materialize(
            plan_path=args.plan,
            local_input_root=args.local_input_root,
            local_output_root=args.local_output_root,
            receipt_path=receipt_path,
            provenance_path=provenance_path,
            check=args.check,
        )
    except (UsageContextBuildError, OSError) as exc:
        parser.error(str(exc))
    action = "verified" if args.check else "materialized"
    print(
        f"{action} {receipt['summary']['row_count']} private contexts; "
        f"tracked receipt sha256={sha256_file(receipt_path)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
